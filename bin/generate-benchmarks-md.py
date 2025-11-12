#!/usr/bin/env python3
"""Generate doc/BENCHMARKS.md from Criterion JSON + resource snapshots.

Usage:
    ./bin/generate-benchmarks-md.py                # run benchmarks + generate
    ./bin/generate-benchmarks-md.py --no-run       # generate from existing data (~1 sec)
"""
from __future__ import annotations

import argparse
import json
import os
import platform
import re
import subprocess
import sys
from dataclasses import dataclass
from datetime import date
from pathlib import Path

MAX_TABLE_WIDTH = 78


# ═══════════════════════════════════════════════════════════════════
# Formatting
# ═══════════════════════════════════════════════════════════════════


def fmt_time(ns: float | None) -> str:
    if ns is None: return "—"
    if ns < 1_000: return f"{ns:.1f} ns"
    if ns < 1_000_000: return f"{ns / 1_000:.1f} µs"
    return f"{ns / 1_000_000:.1f} ms"


def fmt_mibs(ns: float | None, nbytes: int) -> str:
    if not ns or ns <= 0: return "—"
    return f"{(nbytes / (ns / 1e9)) / (1024 * 1024):.0f}"


def fmt_gibs(ns: float | None, nbytes: int) -> str:
    if not ns or ns <= 0: return "—"
    return f"{(nbytes / (ns / 1e9)) / (1024 ** 3):.1f}"


def fmt_ratio(ns: float | None, base: float | None) -> str:
    if not ns or not base or base <= 0: return "—"
    r = ns / base
    if 0.95 <= r <= 1.05: return "~1.0×"
    return f"{r:.1f}×"


def fmt_bytes(b: int | float | None) -> str:
    if b is None: return "—"
    b = int(abs(b))
    if b == 0: return "0"
    if b >= 1024 * 1024 * 1024: return f"{b / (1024 * 1024 * 1024):.1f} GiB"
    if b >= 1024 * 1024: return f"{b / (1024 * 1024):.1f} MiB"
    if b >= 1024: return f"{b / 1024:.1f}K"
    return f"{b}B"


def fmt_size_label(nbytes: int) -> str:
    if nbytes >= 1024 * 1024 * 1024: return f"{nbytes // (1024 * 1024 * 1024)} GiB"
    if nbytes >= 1024 * 1024: return f"{nbytes // (1024 * 1024)} MiB"
    if nbytes >= 1024: return f"{nbytes // 1024} KiB"
    return f"{nbytes} B"


def fmt_cpu_us(us: int | None) -> str:
    if not us: return "—"
    if us < 1_000: return f"{us} µs"
    if us < 1_000_000: return f"{us / 1_000:.1f} ms"
    return f"{us / 1_000_000:.1f} s"


def fmt_duration(secs: float) -> str:
    if secs < 60: return f"{secs:.0f}s"
    m = int(secs // 60)
    s = int(secs % 60)
    return f"{m}m{s}s"


# ═══════════════════════════════════════════════════════════════════
# Markdown rendering (table or list, width-aware)
# ═══════════════════════════════════════════════════════════════════


@dataclass
class Col:
    header: str
    align: str = "left"


def md_table(cols: list[Col], rows: list[list[str]]) -> str:
    n = len(cols)
    w = [max(3, len(c.header)) for c in cols]
    for row in rows:
        for i, cell in enumerate(row[:n]):
            w[i] = max(w[i], len(cell))

    total = sum(w) + 3 * n + 1  # "| " + " | " * (n-1) + " |"

    if total > MAX_TABLE_WIDTH:
        return _md_list(cols, rows)

    def pad(t: str, width: int, a: str) -> str:
        return t.rjust(width) if a == "right" else t.ljust(width)

    def sep(width: int, a: str) -> str:
        return "-" * (width - 1) + ":" if a == "right" else "-" * width

    lines = [
        "| " + " | ".join(pad(c.header, w[i], c.align) for i, c in enumerate(cols)) + " |",
        "| " + " | ".join(sep(w[i], c.align) for i, c in enumerate(cols)) + " |",
    ]
    for row in rows:
        cells = [pad(row[i] if i < len(row) else "", w[i], cols[i].align) for i in range(n)]
        lines.append("| " + " | ".join(cells) + " |")
    return "\n".join(lines)


def _md_list(cols: list[Col], rows: list[list[str]]) -> str:
    """Fallback: render as definition list when table is too wide."""
    lines: list[str] = []
    for row in rows:
        # First column is the "term".
        lines.append(f"- {row[0] if row else '—'}")
        for i, col in enumerate(cols[1:], 1):
            val = row[i] if i < len(row) else "—"
            lines.append(f"  {col.header}: {val}")
    return "\n".join(lines)


# ═══════════════════════════════════════════════════════════════════
# Data layer
# ═══════════════════════════════════════════════════════════════════

CRATES = [
    ("distill",             "distill-strip-ansi",  "`distill-strip-ansi`"),
    ("fast_strip",          "fast-strip-ansi",     "`fast-strip-ansi`"),
    ("console",             "console",             "`console`"),
    ("strip_ansi_escapes",  "strip-ansi-escapes",  "`strip-ansi-escapes`"),
]

CRATE_METADATA_NAMES = [
    "distill-strip-ansi", "fast-strip-ansi", "console",
    "strip-ansi-escapes", "criterion",
]


@dataclass
class BenchPoint:
    ns: float | None = None
    rss_before: int = 0
    rss_after: int = 0
    rss_delta: int | None = None
    peak_rss: int = 0
    cpu_user_us: int = 0
    cpu_sys_us: int = 0


class BenchData:
    def __init__(self, target_dir: Path) -> None:
        self.criterion_dir = target_dir / "criterion"
        self.resources: dict = {}
        self.meta: dict = {}
        self._load(target_dir / "criterion" / "bench-resources.json")

    def _load(self, path: Path) -> None:
        if not path.exists():
            return
        with open(path) as f:
            data = json.load(f)
        self.meta = data.get("meta", {})
        self.resources = data.get("crates", data)  # compat with old format

    @property
    def wall_secs(self) -> float:
        return self.meta.get("wall_secs", 0.0)

    @property
    def cache_info(self) -> dict:
        return self.meta.get("cache_sizes", {})

    def discover_sizes(self) -> list[int]:
        """Find all sizes that have Criterion data."""
        sizes = set()
        eco = self.criterion_dir / "ecosystem"
        if eco.exists():
            for bench_dir in eco.iterdir():
                if bench_dir.is_dir() and bench_dir.name not in ("report", "_warmup"):
                    for size_dir in bench_dir.iterdir():
                        if size_dir.is_dir() and size_dir.name.isdigit():
                            sizes.add(int(size_dir.name))
        return sorted(sizes)

    def discover_bench_sizes(self, bench_name: str) -> list[int]:
        """Find sizes for a specific benchmark (e.g. 'ours_dirty')."""
        sizes = []
        bench_dir = self.criterion_dir / "ecosystem" / bench_name
        if bench_dir.exists():
            for d in bench_dir.iterdir():
                if d.is_dir() and d.name.isdigit():
                    sizes.append(int(d.name))
        return sorted(sizes)

    def discover_single_size(self, bench_name: str) -> int | None:
        """Find the single size for a fixture benchmark (e.g. 'ours_cargo')."""
        sizes = self.discover_bench_sizes(bench_name)
        return sizes[0] if len(sizes) == 1 else None

    def read_median(self, group: str, bench: str, size: int | None = None) -> float | None:
        if size is not None:
            est = self.criterion_dir / group / bench / str(size) / "new" / "estimates.json"
        else:
            # No size subdirectory (e.g. bench_function without BenchmarkId).
            est = self.criterion_dir / group / bench / "new" / "estimates.json"
        if not est.exists():
            return None
        with open(est) as f:
            return json.load(f).get("median", {}).get("point_estimate")

    def get_point(self, bench_key: str, workload: str, size: int) -> BenchPoint:
        ns = self.read_median("ecosystem", f"{bench_key}_{workload}", size)
        if ns is None:
            old = {"distill": "distill_strip", "strip_ansi_escapes": "strip_ansi_escapes",
                   "console": "console_strip"}
            if bench_key in old:
                ns = self.read_median("ecosystem", old[bench_key], size)

        crate_name = dict((k, n) for k, n, _ in CRATES).get(bench_key, bench_key)
        res = self.resources.get(crate_name, {}).get(str(size), {})

        return BenchPoint(
            ns=ns,
            rss_before=res.get("rss_before", 0),
            rss_after=res.get("rss_after", 0),
            rss_delta=res.get("rss_delta"),
            peak_rss=res.get("peak_rss", 0),
            cpu_user_us=res.get("cpu_user_us", 0),
            cpu_sys_us=res.get("cpu_sys_us", 0),
        )

    def get_internal(self, group: str, bench: str, size: int) -> BenchPoint:
        return BenchPoint(ns=self.read_median(group, bench, size))


# ═══════════════════════════════════════════════════════════════════
# Environment
# ═══════════════════════════════════════════════════════════════════


class Environment:
    @staticmethod
    def detect() -> list[tuple[str, str]]:
        pairs: list[tuple[str, str]] = []
        try:
            cpu = subprocess.check_output(
                ["sysctl", "-n", "machdep.cpu.brand_string"],
                stderr=subprocess.DEVNULL, text=True).strip()
        except (subprocess.CalledProcessError, FileNotFoundError):
            cpu = platform.processor() or "unknown"
        pairs.append(("CPU", cpu))
        pairs.append(("Arch", platform.machine()))
        try:
            ver = subprocess.check_output(
                ["sw_vers", "-productVersion"],
                stderr=subprocess.DEVNULL, text=True).strip()
            pairs.append(("OS", f"macOS {ver}"))
        except (subprocess.CalledProcessError, FileNotFoundError):
            pairs.append(("OS", f"{platform.system()} {platform.release()}"))
        try:
            rv = subprocess.check_output(["rustc", "--version"], text=True).strip().split()[1]
            pairs.append(("Rust", rv))
        except (subprocess.CalledProcessError, FileNotFoundError):
            pairs.append(("Rust", "unknown"))
        pairs.append(("Date", date.today().isoformat()))
        return pairs

    @staticmethod
    def crate_versions() -> list[tuple[str, str]]:
        try:
            out = subprocess.check_output(
                ["cargo", "metadata", "--format-version", "1"],
                stderr=subprocess.DEVNULL, text=True)
            meta = json.loads(out)
        except (subprocess.CalledProcessError, FileNotFoundError):
            return [(n, "—") for n in CRATE_METADATA_NAMES]
        versions: dict[str, str] = {}
        for pkg in meta["packages"]:
            name = pkg["name"]
            if name in CRATE_METADATA_NAMES:
                if name not in versions or (name == "console" and pkg["version"].startswith("0.15")):
                    versions[name] = pkg["version"]
        return [(n, versions.get(n, "—")) for n in CRATE_METADATA_NAMES]

    @staticmethod
    def dep_count(spec: str) -> str:
        try:
            out = subprocess.check_output(
                ["cargo", "tree", "-p", spec, "-e", "normal",
                 "--depth", "999", "--prefix", "none"],
                stderr=subprocess.DEVNULL, text=True)
            crates = {l.strip().split()[0] for l in out.strip().splitlines() if l.strip()}
            return str(len(crates) - 1)
        except (subprocess.CalledProcessError, FileNotFoundError):
            return "—"

    @staticmethod
    def dep_count_lib(spec: str, features: str) -> str:
        """Count library-only deps (no CLI features)."""
        try:
            out = subprocess.check_output(
                ["cargo", "tree", "-p", spec, "-e", "normal",
                 "--no-default-features", "--features", features,
                 "--depth", "999", "--prefix", "none"],
                stderr=subprocess.DEVNULL, text=True)
            crates = {l.strip().split()[0] for l in out.strip().splitlines() if l.strip()}
            return str(len(crates) - 1)
        except (subprocess.CalledProcessError, FileNotFoundError):
            return "—"


# ═══════════════════════════════════════════════════════════════════
# Report builder
# ═══════════════════════════════════════════════════════════════════


class BenchmarkReport:
    def __init__(self, data: BenchData, target_dir: Path) -> None:
        self.data = data
        self.target_dir = target_dir
        self.lines: list[str] = []
        # Discover sizes from actual criterion data — never hardcode.
        self.dirty_sizes = data.discover_bench_sizes("distill_dirty")
        self.cargo_size = data.discover_single_size("distill_cargo")
        self.osc8_size = data.discover_single_size("distill_osc8")

    def emit(self, *lines: str) -> None:
        self.lines.extend(lines)

    def generate(self) -> str:
        template = self._load_template()
        if template:
            return self._render_template(template)
        # Fallback: procedural generation (no template found).
        self._title()
        self._notation()
        self._highlights()
        self._environment()
        self._footprints()
        self._howto()
        self._details()
        self._scaling()
        return re.sub(r'\n{3,}', '\n\n', "\n".join(self.lines))

    def _load_template(self) -> str | None:
        """Load doc/BENCHMARKS.md.in if it exists."""
        tmpl_path = Path("doc/BENCHMARKS.md.in")
        if tmpl_path.exists():
            return tmpl_path.read_text()
        return None

    def _render_template(self, template: str) -> str:
        """Fill {{MARKER}} placeholders in the template with dynamic data."""
        sections: dict[str, str] = {}

        # HIGHLIGHTS
        self.lines = []
        self._highlights_content()
        sections["HIGHLIGHTS"] = "\n".join(self.lines).strip()

        # ENV
        self.lines = []
        self._environment_content()
        sections["ENV"] = "\n".join(self.lines).strip()

        # VERSIONS
        self.lines = []
        self._versions_content()
        sections["VERSIONS"] = "\n".join(self.lines).strip()

        # FOOTPRINTS
        self.lines = []
        self._footprints_content()
        sections["FOOTPRINTS"] = "\n".join(self.lines).strip()

        # HOWTO
        self.lines = []
        self._howto_content()
        sections["HOWTO"] = "\n".join(self.lines).strip()

        # DETAILS
        self.lines = []
        self._details_content()
        sections["DETAILS"] = "\n".join(self.lines).strip()

        # SCALING
        self.lines = []
        self._scaling_content()
        sections["SCALING"] = "\n".join(self.lines).strip()

        result = template
        for key, value in sections.items():
            result = result.replace("{{" + key + "}}", value)

        # Collapse any resulting triple+ newlines to double.
        result = re.sub(r'\n{3,}', '\n\n', result)
        return result

    def _title(self) -> None:
        self.emit(
            "# Benchmarks", "",
            "Criterion.rs statistical benchmarks across the Rust ANSI",
            "stripping ecosystem: `distill-strip-ansi`, `fast-strip-ansi`,",
            "`strip-ansi-escapes`, and `console`.",
        )

    def _notation(self) -> None:
        self.emit("", "## Symbolic Notation", "")
        self.emit(md_table(
            [Col("Symbol"), Col("Meaning")],
            [
                ["ns",       "nanoseconds (10⁻⁹ s)"],
                ["µs",       "microseconds (10⁻⁶ s)"],
                ["ms",       "milliseconds (10⁻³ s)"],
                ["MiB/s",    "mebibytes/sec (2²⁰ B/s)"],
                ["GiB/s",    "gibibytes/sec (2³⁰ B/s)"],
                ["×",        "multiplier (baseline = distill)"],
                ["RSS Δ",    "memory retained after bench"],
                ["CPU",      "user+sys CPU time (bench)"],
            ],
        ))

    def _highlights(self) -> None:
        self.emit("", "## Highlights for Humans", "")
        self._highlights_content()

    def _highlights_content(self) -> None:
        d = self.data
        # Use 4 KiB dirty if available, otherwise closest size.
        highlight_size = 4096
        if highlight_size not in self.dirty_sizes and self.dirty_sizes:
            highlight_size = min(self.dirty_sizes, key=lambda s: abs(s - 4096))
        ours = d.get_point("distill", "dirty", highlight_size)
        # Use largest clean size available.
        clean_sizes = [s for s in self.dirty_sizes if d.get_point("distill", "clean", s).ns]
        clean_size = clean_sizes[-1] if clean_sizes else 16384
        clean = d.get_internal("strip", "clean", clean_size) if clean_size <= 16384 \
            else d.get_point("distill", "clean", clean_size)

        if ours.ns:
            self.emit(f"- {fmt_mibs(ours.ns, highlight_size)} MiB/s dirty throughput ({fmt_size_label(highlight_size)}, ~20% ANSI)")
        if clean.ns:
            self.emit(f"- {fmt_gibs(clean.ns, clean_size)} GiB/s clean fast path ({fmt_size_label(clean_size)})")
        self.emit(
            "- Zero allocation on clean input (`Cow::Borrowed`)",
            "- O(n) linear scaling — constant MiB/s to 1 GiB+",
            "- No temp files, no disk I/O — pure in-memory",
        )

    def _environment(self) -> None:
        self.emit("", "## Environmental Concerns", "")
        self._environment_content()
        self.emit("", "### Crate Versions", "")
        self._versions_content()

    def _environment_content(self) -> None:
        env = Environment.detect()
        cache = self.data.cache_info
        wall = self.data.wall_secs
        env_rows = [[k, v] for k, v in env]
        if cache:
            env_rows.append(["L1d", fmt_bytes(cache.get("l1d"))])
            env_rows.append(["L2", fmt_bytes(cache.get("l2"))])
            env_rows.append(["L3", fmt_bytes(cache.get("l3"))])
            env_rows.append(["RAM", fmt_bytes(cache.get("ram"))])
        env_rows.append(["Sizes", f"{len(self.dirty_sizes)} tiers (hardware-adaptive)"])
        if wall > 0:
            env_rows.append(["Bench time", fmt_duration(wall)])
        self.emit(md_table([Col("Key"), Col("Value")], env_rows))

    def _versions_content(self) -> None:
        versions = Environment.crate_versions()
        self.emit(md_table(
            [Col("Crate"), Col("Version", "right")],
            [[f"`{n}`", v] for n, v in versions],
        ))

    def _footprints(self) -> None:
        self.emit(
            "", "## Crate Footprints", "",
        )
        self._footprints_content()
        self.emit(
            "",
            "No crate uses temp files or disk I/O — stdin only.",
            "Peak RSS, RSS Δ, and CPU measured at largest bench size.",
            "RSS Δ reflects allocator page retention after the last",
            "Criterion iteration — not a leak. CPU is user+sys time",
            "for the benchmark (not wall clock). Resource snapshots",
            "captured via `task_info` (macOS) / `getrusage` (POSIX)",
            "outside the timed loop — no measurement overhead.",
        )

    def _footprints_content(self) -> None:
        bin_path = self.target_dir / "release" / "strip-ansi"
        bin_size = fmt_bytes(bin_path.stat().st_size) if bin_path.exists() else "—"

        def crate_resources(crate_name: str) -> tuple[str, str, str]:
            """Returns (peak_rss, rss_delta, cpu_total) from largest size."""
            sizes_data = self.data.resources.get(crate_name, {})
            # Try dirty_sizes in reverse (largest first).
            for sz in reversed(self.dirty_sizes):
                snap = sizes_data.get(str(sz), {})
                if snap.get("peak_rss"):
                    cpu = snap.get("cpu_user_us", 0) + snap.get("cpu_sys_us", 0)
                    return (
                        fmt_bytes(snap["peak_rss"]),
                        fmt_bytes(snap.get("rss_delta")),
                        fmt_cpu_us(cpu),
                    )
            # Fallback: grab the entry with the largest size key.
            if sizes_data and isinstance(sizes_data, dict):
                numeric_keys = sorted(
                    (int(k) for k in sizes_data if k.isdigit()),
                    reverse=True,
                )
                for k in numeric_keys:
                    snap = sizes_data.get(str(k), {})
                    if snap.get("peak_rss"):
                        cpu = snap.get("cpu_user_us", 0) + snap.get("cpu_sys_us", 0)
                        return (
                            fmt_bytes(snap["peak_rss"]),
                            fmt_bytes(snap.get("rss_delta")),
                            fmt_cpu_us(cpu),
                        )
            return "—", "—", "—"

        # Library dep counts (no CLI features) for apples-to-apples.
        lib_features = "std,filter,transform,downgrade-color,augment-color,unicode-normalize"

        cols = [Col("Crate"), Col("Deps", "right"), Col("Peak RSS", "right"),
                Col("RSS Δ", "right"), Col("CPU", "right")]
        rows = []
        for _, crate_name, display in CRATES:
            peak, delta, cpu = crate_resources(crate_name)
            if crate_name == "distill-strip-ansi":
                deps = Environment.dep_count_lib(crate_name, lib_features)
            else:
                deps = Environment.dep_count(crate_name)
            rows.append([display, deps, peak, delta, cpu])
        self.emit(md_table(cols, rows))

        # Binary note (CLI includes clap and other deps beyond library).
        cli_deps = Environment.dep_count("distill-strip-ansi")
        self.emit(
            "",
            f"`strip-ansi` binary: {bin_size}, {cli_deps} deps",
            "(includes `clap` for CLI argument parsing).",
        )

    def _howto(self) -> None:
        self.emit("", "## HOWTO: Reproduce", "")
        self._howto_content()

    def _howto_content(self) -> None:
        wall = self.data.wall_secs
        est = f" (~{fmt_duration(wall)})" if wall > 0 else ""

        self.emit(
            "```bash",
            f"# Quick run: up to 2×L3 cache{est}",
            "./bin/generate-benchmarks-md.py",
            "",
            "# Full run: all sizes including GiB-scale (~30 min)",
            "./bin/generate-benchmarks-md.py --max-size 0",
            "",
            "# Custom cap",
            "./bin/generate-benchmarks-md.py --max-size 64M",
            "",
            "# Report only from existing data (~1 sec)",
            "./bin/generate-benchmarks-md.py --no-run",
            "```",
            "",
            "The generator runs five bench suites then renders this doc:",
            "",
            "- `cargo bench --bench internals` — library internals:",
            "  strip, stream, classifier, filter, threats, transforms,",
            "  augments, unicode normalize",
            "- `cargo bench -p ecosystem-bench --bench distill`",
            "- `cargo bench -p ecosystem-bench --bench fast_strip`",
            "- `cargo bench -p ecosystem-bench --bench console_bench`",
            "- `cargo bench -p ecosystem-bench --bench strip_escapes`",
            "",
            "Each ecosystem bench uses the same harness",
            "(`distill-bench-harness`): identical sizes, config",
            "(10 samples, 3s measurement, 1s warmup), and RSS/CPU",
            "capture. Sizes are hardware-adaptive — the bench detects",
            "L1/L2/L3 cache sizes and RAM, then picks boundary points.",
        )

        self.emit("", "### Test Data Strategy", "")
        cache = self.data.cache_info
        l1 = fmt_bytes(cache.get("l1d")) if cache.get("l1d") else "32K"
        l2 = fmt_bytes(cache.get("l2")) if cache.get("l2") else "256K"
        l3 = fmt_bytes(cache.get("l3")) if cache.get("l3") else "12 MiB"
        self.emit(md_table(
            [Col("Tier"), Col("Source"), Col("Why")],
            [
                [f"≤{l1}",        "fixture or generated", "L1 cache"],
                [f"{l1}–{l2}",    "generated in RAM",     "L2 cache"],
                [f"{l2}–{l3}",    "generated in RAM",     "L3 boundary"],
                [f">{l3}",        "generated in RAM",     "DRAM bandwidth"],
            ],
        ))
        self.emit(
            "",
            "Each size selects the closest `tests/fixtures/*.raw.txt`",
            "file that contains ANSI sequences (0.25×–4× tolerance).",
            "When no fixture fits, synthetic ~20% ANSI data is generated.",
            "Fixtures above ~1 KiB with ANSI are rare, so most tiers",
            "use generated data.",
        )

    def _details(self) -> None:
        self.emit(
            "", "## Details That Matter", "",
            "All crates: `&[u8]` input. `console`: `&str`",
            "(conversion outside timed loop). `distill-strip-ansi`",
            "used as baseline (Relative = time / baseline time).",
        )
        self._details_content()

    def _details_content(self) -> None:

        # Representative detail sizes: smallest, 4K, a cache boundary,
        # a large size, and the largest.
        detail_sizes = self._pick_detail_sizes()

        for size in detail_sizes:
            label = fmt_size_label(size)
            self.emit("", f"### Dirty {label}", "")
            self.emit(self._eco_card("dirty", size))

        # Real-world workloads — sizes discovered from criterion data.
        cargo_size = self.cargo_size or 0
        osc8_size = self.osc8_size or 0

        cargo_label = fmt_size_label(cargo_size) if cargo_size else "?"
        osc8_label = fmt_size_label(osc8_size) if osc8_size else "?"

        self.emit("", f"### Cargo Output ({cargo_label})", "")
        self.emit(self._eco_card("cargo", cargo_size))

        self.emit("", f"### OSC 8 Hyperlinks ({osc8_label})", "")
        self.emit(self._eco_card("osc8", osc8_size))

        # ── Features unique to distill-strip-ansi ──
        self._unique_features()

    def _scaling(self) -> None:
        self.emit(
            "", "## Scaling", "",
            "Dirty throughput (MiB/s) across input sizes.",
            "Constant bar length = O(n). Shrinking = super-linear.",
            "",
            "RSS Δ and CPU shown at largest size only — small-size",
            "values are dominated by benchmark harness overhead.",
        )
        self._scaling_content()

    def _scaling_content(self) -> None:
        largest = self.dirty_sizes[-1] if self.dirty_sizes else 0
        versions = dict(Environment.crate_versions())

        # Find global max for consistent bar scale across crates.
        max_v = 1
        for bk, _, _ in CRATES:
            for sz in self.dirty_sizes:
                pt = self.data.get_point(bk, "dirty", sz)
                if pt.ns and pt.ns > 0:
                    max_v = max(max_v, int((sz / (pt.ns / 1e9)) / (1024 * 1024)))
        bar_width = 30

        for bench_key, crate_name, display in CRATES:
            mibs_vals: list[float] = []
            sizes_with_data: list[int] = []
            for size in self.dirty_sizes:
                pt = self.data.get_point(bench_key, "dirty", size)
                if pt.ns and pt.ns > 0:
                    sizes_with_data.append(size)
                    mibs_vals.append((size / (pt.ns / 1e9)) / (1024 * 1024))

            o_class = self._estimate_complexity(mibs_vals, sizes_with_data)
            ver = versions.get(crate_name, "")
            ver_suffix = f" v{ver}" if ver and ver != "—" else ""

            largest_pt = self.data.get_point(bench_key, "dirty", largest)
            notes = []
            if largest_pt.rss_delta is not None:
                notes.append(f"RSS Δ {fmt_bytes(largest_pt.rss_delta)}")
            cpu = largest_pt.cpu_user_us + largest_pt.cpu_sys_us
            if cpu:
                notes.append(f"CPU {fmt_cpu_us(cpu)}")
            suffix = f" · {' · '.join(notes)}" if notes else ""

            self.emit(f"### {display}{ver_suffix} — {o_class}{suffix}", "", "```text")
            for size in self.dirty_sizes:
                pt = self.data.get_point(bench_key, "dirty", size)
                label = fmt_size_label(size)
                if pt.ns and pt.ns > 0:
                    mibs = (size / (pt.ns / 1e9)) / (1024 * 1024)
                    bar_len = int(mibs / max_v * bar_width)
                    bar = "█" * bar_len
                    self.emit(f"{label:>7} {bar} {mibs:.0f}")
                else:
                    self.emit(f"{label:>7} —")
            self.emit("```", "")

        # Complexity summary with context.
        self.emit("", "### Complexity Summary", "")
        rows = []
        for bk, _, display in CRATES:
            dirty_sizes_with_data = []
            dirty_v = []
            for sz in self.dirty_sizes:
                pt = self.data.get_point(bk, "dirty", sz)
                if pt.ns and pt.ns > 0:
                    dirty_sizes_with_data.append(sz)
                    dirty_v.append((sz / (pt.ns / 1e9)) / (1024 * 1024))
            clean_sizes_with_data = []
            clean_v = []
            for sz in self.dirty_sizes:
                pt = self.data.get_point(bk, "clean", sz)
                if pt.ns and pt.ns > 0:
                    clean_sizes_with_data.append(sz)
                    clean_v.append((sz / (pt.ns / 1e9)) / (1024 * 1024))
            rows.append([display,
                         self._estimate_complexity(dirty_v, dirty_sizes_with_data),
                         self._estimate_complexity(clean_v, clean_sizes_with_data)])
        self.emit(md_table(
            [Col("Crate"), Col("Dirty"), Col("Clean")],
            rows,
        ))
        self.emit(
            "",
            "Complexity estimated per memory tier (L1/L2/L3/DRAM) —",
            "throughput steps between tiers are hardware, not algorithmic.",
        )

    def _unique_features(self) -> None:
        """Benchmark features only distill-strip-ansi offers.

        Ordered by pipeline path-of-operations:
        classify → filter/threats → streaming → unicode →
        color pipeline (passthrough baseline, transforms, augments).
        """
        self.emit(
            "", "### Extended Capabilities", "",
            "Additional features available in `distill-strip-ansi`.",
            "",
        )

        cols = [Col("Feature"), Col("Time", "right"), Col("MiB/s", "right"),
                Col("RSS Δ", "right"), Col("CPU", "right")]

        cache = self.data.cache_info
        l1 = cache.get("l1d", 32768)
        l2 = cache.get("l2", 262144)
        l3 = cache.get("l3", 12582912)

        # Each feature: (display_name, criterion_group, bench_name, size, description)
        #
        # Pipeline order:
        #   classify → filter → threat scan → streaming →
        #   unicode normalize → color pipeline (passthrough,
        #   transforms, augments)
        features = [
            # ── Classify ──
            ("Classify (parse only)", "classifier", "cargo_classify", None,
             "ClassifyingParser overhead on cargo output"),
            ("Classify + detail", "classifier", "cargo_classify_detail", None,
             "Full sequence classification with parameter extraction"),
            # ── Filter ──
            ("Filter: SGR mask", "filter_detail", "sgr_mask", None,
             "Selective SGR filtering by color depth"),
            ("Filter: sanitize preset", "filter_detail", "sanitize_preset", None,
             "Security-aware preset filtering"),
            # ── Threat scan (security, adjacent to filter) ──
            ("Threat scan (clean)", "check_threats", "scan_clean", None,
             "Echoback vector detection on cargo output"),
            ("Threat scan (dirty)", "check_threats", "scan_only", None,
             "Echoback detection with embedded threats"),
            # ── Streaming (strip delivery across cache tiers) ──
            ("Streaming (L1)", "stream", "strip_slices", l1,
             "StripStream push API at L1 cache size"),
            ("Streaming (L2)", "stream", "strip_slices", l2,
             "StripStream push API at L2 cache size"),
            ("Streaming (L3)", "stream", "strip_slices", l3,
             "StripStream push API at L3 cache size"),
            # ── Unicode normalize ──
            ("Unicode normalize", "unicode_normalize", "real_world_cargo", None,
             "Homograph normalization on cargo output"),
            # ── Color pipeline: passthrough baseline ──
            ("Transform: passthrough", "transform", "passthrough", None,
             "Transform overhead (no color change)"),
            # ── Color pipeline: depth transforms ──
            ("Transform: truecolor→mono", "transform", "truecolor_to_mono", None,
             "Color stripping for accessibility (screen readers)"),
            ("Transform: truecolor→grey", "transform", "truecolor_to_greyscale", None,
             "Greyscale for e-ink / low-vision displays"),
            ("Transform: truecolor→16", "transform", "truecolor_to_16", None,
             "Downgrade for legacy terminals"),
            ("Transform: truecolor→256", "transform", "truecolor_to_256", None,
             "Downgrade for 256-color terminals"),
            ("Transform: 256→16", "transform", "256_to_16", None,
             "256-color to basic ANSI"),
            ("Transform: 256→grey", "transform", "256_to_greyscale", None,
             "256-color to greyscale"),
            ("Transform: basic→mono", "transform", "basic_to_mono", None,
             "Strip basic colors, keep styles"),
            # ── Color pipeline: vision augmentation ──
            # 256 RGB transforms per iteration = 768 bytes (3 per color).
            # sRGB roundtrip = 256 single-channel values.
            ("Augment: protanopia", "augment_color", "protanopia_256", None,
             "Color vision simulation (red-green)", 768),
            ("Augment: deuteranopia", "augment_color", "deuteranopia_256", None,
             "Color vision simulation (red-green)", 768),
            ("Augment: sRGB roundtrip", "augment_color", "srgb_roundtrip_256", None,
             "sRGB linearization pipeline", 256),
        ]

        rows: list[list[str]] = []
        for entry in features:
            if len(entry) == 6:
                name, group, bench, size, _desc, equiv_bytes = entry
            else:
                name, group, bench, size, _desc = entry
                equiv_bytes = None
            ns = None
            actual_size = size
            if size is not None:
                ns = self.data.read_median(group, bench, size)
            elif equiv_bytes is not None:
                # Element-based benchmarks (no size subdirectory).
                ns = self.data.read_median(group, bench, None)
                actual_size = equiv_bytes
            else:
                # Discover size from Criterion dirs.
                group_dir = self.data.criterion_dir / group / bench
                if group_dir.exists():
                    for d in sorted(group_dir.iterdir()):
                        if d.is_dir() and d.name.isdigit():
                            actual_size = int(d.name)
                            ns = self.data.read_median(group, bench, actual_size)
                            if ns is not None:
                                break

            t = fmt_time(ns)
            if ns and actual_size:
                m = fmt_mibs(ns, actual_size)
            else:
                m = "—"

            # Resource data: keyed as "{group}/{bench}" in internals tracker.
            res_key = f"{group}/{bench}"
            res_size = str(actual_size) if actual_size else ""
            res = self.data.resources.get(res_key, {}).get(res_size, {})
            # Fallback: if exact size miss, grab first available entry.
            if not res:
                entries = self.data.resources.get(res_key, {})
                if entries and isinstance(entries, dict):
                    first_key = next(iter(entries), None)
                    if first_key and isinstance(entries.get(first_key), dict):
                        res = entries[first_key]
            rss_d = fmt_bytes(res.get("rss_delta"))
            cpu_total = res.get("cpu_user_us", 0) + res.get("cpu_sys_us", 0)
            cpu_s = fmt_cpu_us(cpu_total) if cpu_total else "—"

            rows.append([name, t, m, rss_d, cpu_s])

        self.emit(md_table(cols, rows))

    # ── Helpers ─────────────────────────────────────────────────

    def _pick_detail_sizes(self) -> list[int]:
        """Pick representative sizes: cache boundaries + 2 beyond."""
        ds = self.dirty_sizes
        if len(ds) <= 6:
            return ds
        cache = self.data.cache_info
        picks = {ds[0]}  # smallest
        if 4096 in ds:
            picks.add(4096)  # typical CI chunk
        # Cache boundaries if known.
        for key in ("l1d", "l2", "l3"):
            c = cache.get(key, 0)
            if c:
                # Find the closest size at or above the cache size.
                for s in ds:
                    if s >= c:
                        picks.add(s)
                        break
        # Two steps beyond L3.
        l3 = cache.get("l3", 0)
        if l3:
            beyond = [s for s in ds if s > l3 * 2]
            for s in beyond[:2]:
                picks.add(s)
        # Always include the largest.
        picks.add(ds[-1])
        return sorted(picks)

    def _eco_card(self, workload: str, size: int) -> str:
        """Render per-crate comparison — compact, units in headers."""
        base = self.data.get_point("distill", workload, size)
        rows: list[list[str]] = []
        for bk, _, display in CRATES:
            pt = self.data.get_point(bk, workload, size)
            t = fmt_time(pt.ns)
            m = fmt_mibs(pt.ns, size)
            r = "baseline" if bk == "distill" else fmt_ratio(pt.ns, base.ns)
            cpu = pt.cpu_user_us + pt.cpu_sys_us
            rows.append([
                display, t, m, r,
                fmt_bytes(pt.rss_delta) if pt.rss_delta is not None else "—",
                fmt_cpu_us(cpu) if cpu else "—",
            ])
        return md_table(
            [Col("Crate"), Col("Time", "right"), Col("MiB/s", "right"),
             Col("×", "right"), Col("RSS Δ", "right"), Col("CPU", "right")],
            rows,
        )

    def _estimate_complexity(self, mibs: list[float], sizes: list[int] | None = None) -> str:
        """Estimate O-notation from time scaling per memory tier.

        Checks time(2x)/time(x) ratio within each tier. Requires a
        supermajority (≥75%) of same-tier ratios to exceed the threshold
        before classifying as super-linear. Median alone is insufficient
        — a single noisy pair at the median position could flip the result.

        Thresholds (v0.6.0):
          ≤ 1.5 → O(n)       (accommodates TLB/DRAM bandwidth variance)
          ≤ 2.5 → O(n log n)
          ≤ 5.0 → O(n²)

        Prior threshold of 1.3 false-positived on systems without CPU
        isolation: TLB pressure at 32 MiB (8192 pages > 1536 L2 TLB
        entries on Haswell) adds ~30% measured overhead that isn't
        algorithmic. Combined with supermajority, this eliminates
        hardware-induced false classifications.
        """
        if len(mibs) < 3 or not sizes or len(sizes) != len(mibs):
            return "O(?)"

        cache = self.data.cache_info
        boundaries = sorted(set(filter(None, [
            cache.get("l1d", 0), cache.get("l2", 0), cache.get("l3", 0),
        ])))

        def tier_of(sz: int) -> int:
            for i, b in enumerate(boundaries):
                if sz <= b:
                    return i
            return len(boundaries)

        ratios: list[float] = []
        for i in range(1, len(sizes)):
            if mibs[i] <= 0 or mibs[i - 1] <= 0:
                continue
            if tier_of(sizes[i]) != tier_of(sizes[i - 1]):
                continue
            size_ratio = sizes[i] / sizes[i - 1]
            if size_ratio <= 0:
                continue
            time_i = sizes[i] / mibs[i]
            time_prev = sizes[i - 1] / mibs[i - 1]
            if time_prev > 0:
                ratios.append((time_i / time_prev) / size_ratio)

        if not ratios:
            # No same-tier pairs — use all pairs.
            for i in range(1, len(sizes)):
                if mibs[i] <= 0 or mibs[i - 1] <= 0:
                    continue
                size_ratio = sizes[i] / sizes[i - 1]
                if size_ratio <= 0:
                    continue
                time_i = sizes[i] / mibs[i]
                time_prev = sizes[i - 1] / mibs[i - 1]
                if time_prev > 0:
                    ratios.append((time_i / time_prev) / size_ratio)

        if not ratios:
            return "O(?)"

        # Require at least 2 same-tier pairs for a meaningful
        # supermajority vote. A single pair is too noisy — cache
        # boundary effects at exactly L1d/L2/L3 size can produce
        # ratio > 1.5 without algorithmic super-linearity.
        if len(ratios) < 2:
            return "O(n)" if ratios[0] <= 2.5 else "O(?)"

        # Supermajority: ≥75% of ratios must exceed threshold.
        def exceeds_pct(threshold: float) -> bool:
            count = sum(1 for r in ratios if r > threshold)
            return count >= len(ratios) * 0.75

        if not exceeds_pct(1.5):
            return "O(n)"
        if not exceeds_pct(2.5):
            return "O(n log n)"
        if not exceeds_pct(5.0):
            return "O(n²)"
        return "O(n²+)"


# ═══════════════════════════════════════════════════════════════════
# Main
# ═══════════════════════════════════════════════════════════════════

BENCH_CMD = ["cargo", "bench", "--all-features", "--bench", "internals"]
ECOSYSTEM_BENCH_CMDS = [
    ["cargo", "bench", "-p", "ecosystem-bench", "--bench", "distill"],
    ["cargo", "bench", "-p", "ecosystem-bench", "--bench", "fast_strip"],
    ["cargo", "bench", "-p", "ecosystem-bench", "--bench", "console_bench"],
    ["cargo", "bench", "-p", "ecosystem-bench", "--bench", "strip_escapes"],
]
OUTPUT_FILE = Path("doc/BENCHMARKS.md")
TEMPLATE_FILE = Path("doc/BENCHMARKS.md.in")
TARGET_DIR = Path("target")


def main() -> None:
    parser = argparse.ArgumentParser(description="Generate doc/BENCHMARKS.md")
    parser.add_argument("--no-run", action="store_true")
    parser.add_argument("--max-size", default=None,
        help="Cap input size (e.g. 64M, 1G). Default: 2×L3. 0=unlimited.")
    parser.add_argument("--output", "-o", type=Path, default=OUTPUT_FILE)
    args = parser.parse_args()

    if not args.no_run:
        # CARGO_TARGET_DIR forces both cargo and criterion to use
        # ./target — overrides any global [build] target-dir config.
        env = dict(os.environ)
        env["CARGO_TARGET_DIR"] = str(TARGET_DIR.resolve())
        if args.max_size:
            env["BENCH_MAX_SIZE"] = args.max_size
        failed: list[str] = []
        for cmd in [BENCH_CMD] + ECOSYSTEM_BENCH_CMDS:
            print(f"Running: {' '.join(cmd)}", file=sys.stderr)
            r = subprocess.run(cmd, env=env)
            if r.returncode != 0:
                failed.append(" ".join(cmd))
                print(f"  ⚠ failed (exit {r.returncode}), continuing…",
                      file=sys.stderr)
        if failed:
            print(f"\n⚠ {len(failed)} bench(es) failed — report may be incomplete:",
                  file=sys.stderr)
            for f in failed:
                print(f"  • {f}", file=sys.stderr)
            print(file=sys.stderr)

    data = BenchData(TARGET_DIR)
    report = BenchmarkReport(data, TARGET_DIR)
    doc = report.generate()
    args.output.write_text(doc)
    print(f"Wrote {args.output}", file=sys.stderr)


if __name__ == "__main__":
    main()
