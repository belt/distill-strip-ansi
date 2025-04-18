#!/usr/bin/env python3
"""Generate doc/BENCHMARKS.md from Criterion JSON + resource snapshots.

Usage:
    ./bin/generate-benchmarks-md.py                # run benchmarks + generate
    ./bin/generate-benchmarks-md.py --no-run       # re-render from cached data (~1 sec)
    ./bin/generate-benchmarks-md.py --output path.md
                                                   # write to a different file
    ./bin/generate-benchmarks-md.py --max-size 64M # cap per-bench input size
    ./bin/generate-benchmarks-md.py --features iai-callgrind
                                                   # forward cargo features to every bench
    ./bin/generate-benchmarks-md.py --features a --features b
                                                   # repeatable; joined with ','
    BENCH_QUICK=1 ./bin/generate-benchmarks-md.py  # fast pass (not publishable)

Environment:
    BENCH_QUICK=1     Lower statistical power; use for iteration only.
    BENCH_MAX_SIZE    Alternative to --max-size (e.g. 64M, 1G, 0=unlimited).
    CARGO_TARGET_DIR  Forced to ./target by this script so criterion artifacts
                      land in one place regardless of any global cargo config.
"""
from __future__ import annotations

import argparse
import json
import math
import os
import platform
import re
import subprocess
import sys
import time
from dataclasses import dataclass
from datetime import date
from pathlib import Path

# Table width cap for the md_table → _md_list fallback. 80 is the
# prose-comfort target (also markdownlint-cli2 MD013 default).
# Per-size ecosystem tables fit by dropping the CPU column — it's
# dominated by criterion harness overhead and doesn't vary per
# crate, so it lives only in the Crate Footprints summary.
MAX_TABLE_WIDTH = 80


# ═══════════════════════════════════════════════════════════════════
# Formatting
# ═══════════════════════════════════════════════════════════════════


def fmt_time(ns: float | None) -> str:
    if ns is None:
        return "—"
    if ns < 1_000:
        return f"{ns:.1f} ns"
    if ns < 1_000_000:
        return f"{ns / 1_000:.1f} µs"
    return f"{ns / 1_000_000:.1f} ms"


def fmt_mibs(ns: float | None, nbytes: int) -> str:
    if not ns or ns <= 0:
        return "—"
    return f"{(nbytes / (ns / 1e9)) / (1024 * 1024):.0f}"


def fmt_gibs(ns: float | None, nbytes: int) -> str:
    if not ns or ns <= 0:
        return "—"
    return f"{(nbytes / (ns / 1e9)) / (1024 ** 3):.1f}"


def fmt_ratio(ns: float | None, base: float | None) -> str:
    if not ns or not base or base <= 0:
        return "—"
    r = ns / base
    if 0.95 <= r <= 1.05:
        return "~1.0×"
    return f"{r:.1f}×"


def fmt_bytes(b: int | float | None) -> str:
    if b is None:
        return "—"
    b = int(abs(b))
    if b == 0:
        return "0"
    if b >= 1024 * 1024 * 1024:
        return f"{b / (1024 * 1024 * 1024):.1f} GiB"
    if b >= 1024 * 1024:
        return f"{b / (1024 * 1024):.1f} MiB"
    if b >= 1024:
        return f"{b / 1024:.1f}K"
    return f"{b}B"


def fmt_size_label(nbytes: int) -> str:
    if nbytes >= 1024 * 1024 * 1024:
        return f"{nbytes // (1024 * 1024 * 1024)} GiB"
    if nbytes >= 1024 * 1024:
        return f"{nbytes // (1024 * 1024)} MiB"
    if nbytes >= 1024:
        return f"{nbytes // 1024} KiB"
    return f"{nbytes} B"


def fmt_cpu_us(us: int | None) -> str:
    if not us:
        return "—"
    if us < 1_000:
        return f"{us} µs"
    if us < 1_000_000:
        return f"{us / 1_000:.1f} ms"
    return f"{us / 1_000_000:.1f} s"


def fmt_cv(cv: float | None) -> str:
    """Render coefficient of variation (std_dev / mean) as a percentage.

    Kept for the internal details-section only. Ecosystem tables
    now hide CV and instead append a `⚠` marker to the time column
    via `mark_noisy` when CV > NOISY_CV_THRESHOLD; readers get "is
    this number trustworthy" without losing table width to a column
    that carries no decisional value.
    """
    if cv is None:
        return "—"
    pct = cv * 100.0
    if pct < 10:
        return f"{pct:.2f}%"
    return f"{pct:.1f}%"


# CV above this threshold flags a cell as "noisy" — shows a ⚠
# suffix on the time/throughput value so the reader knows to
# cross-check with an iai-callgrind run. 3% is the sweet spot
# between "actionable signal" and "too many false alarms" on
# a bench config of 200×9s per cell.
NOISY_CV_THRESHOLD = 0.03


def mark_noisy(text: str, cv: float | None) -> str:
    """Append a noise marker when CV exceeds the threshold.

    Returns `text` unchanged when CV is None (unavailable) or
    below threshold. Padding lives on the caller side — the
    marker is a zero-width-ish visual hint, not a column.
    """
    if cv is None or cv < NOISY_CV_THRESHOLD:
        return text
    return f"{text} ⚠"


def fmt_duration(secs: float) -> str:
    if secs < 60:
        return f"{secs:.0f}s"
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
    # Coefficient of variation (std_dev / mean). None when unavailable.
    # Lower is better: <0.02 = tight, 0.02-0.05 = OK, >0.05 = noisy.
    cv: float | None = None
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
        stats = self._read_estimates(group, bench, size)
        return stats[0] if stats else None

    def _read_estimates(
        self,
        group: str,
        bench: str,
        size: int | None = None,
    ) -> tuple[float, float | None] | None:
        """Return (median_ns, cv) tuple, or None when not yet benched.

        CV = std_dev / mean. Unavailable on criterion runs that
        didn't populate the mean/std_dev arms.
        """
        if size is not None:
            est = self.criterion_dir / group / bench / str(size) / "new" / "estimates.json"
        else:
            est = self.criterion_dir / group / bench / "new" / "estimates.json"
        if not est.exists():
            return None
        with open(est) as f:
            data = json.load(f)
        median = data.get("median", {}).get("point_estimate")
        if median is None:
            return None
        mean = data.get("mean", {}).get("point_estimate")
        std_dev = data.get("std_dev", {}).get("point_estimate")
        cv = (std_dev / mean) if (mean and std_dev and mean > 0) else None
        return (median, cv)

    def get_point(self, bench_key: str, workload: str, size: int) -> BenchPoint:
        stats = self._read_estimates("ecosystem", f"{bench_key}_{workload}", size)
        if stats is None:
            old = {"distill": "distill_strip", "strip_ansi_escapes": "strip_ansi_escapes",
                   "console": "console_strip"}
            if bench_key in old:
                stats = self._read_estimates("ecosystem", old[bench_key], size)
        ns, cv = stats if stats else (None, None)

        crate_name = dict((k, n) for k, n, _ in CRATES).get(bench_key, bench_key)
        res = self.resources.get(crate_name, {}).get(str(size), {})

        return BenchPoint(
            ns=ns,
            cv=cv,
            rss_before=res.get("rss_before", 0),
            rss_after=res.get("rss_after", 0),
            rss_delta=res.get("rss_delta"),
            peak_rss=res.get("peak_rss", 0),
            cpu_user_us=res.get("cpu_user_us", 0),
            cpu_sys_us=res.get("cpu_sys_us", 0),
        )

    def get_internal(self, group: str, bench: str, size: int) -> BenchPoint:
        stats = self._read_estimates(group, bench, size)
        if stats is None:
            return BenchPoint()
        return BenchPoint(ns=stats[0], cv=stats[1])


# ═══════════════════════════════════════════════════════════════════
# iai-callgrind (instruction counts) — target/iai/**/summary.json
# ═══════════════════════════════════════════════════════════════════


# Fixed size ladder from `benches/harness/src/iai_inputs.rs`.
# Kept in sync manually so the Python parser stays import-free.
IAI_SIZES: dict[str, int] = {
    "tiny":   256,
    "small":  4 * 1024,
    "medium": 64 * 1024,
    "large":  1024 * 1024,
    "xlarge": 16 * 1024 * 1024,
}

# Byte patterns used by iai_cargo() / iai_osc8() in the harness.
# Same literals — we compute length in Python instead of guessing.
_IAI_CARGO_LINE = b"\x1b[0m\x1b[1m\x1b[32m   Compiling\x1b[0m memchr v2.7.1\n"
_IAI_OSC8_LINE = b"\x1b]8;;https://docs.rs/memchr/2.7.1\x07memchr\x1b]8;;\x07 v2.7.1\n"
IAI_CARGO_BYTES = len(_IAI_CARGO_LINE) * 100
IAI_OSC8_BYTES = len(_IAI_OSC8_LINE) * 50

# Map from the CRATES bench_key to the iai layout on disk:
#   target/iai/<package>/<bench>/<group>/<fn>.<id>/summary.json
IAI_ECO_BENCHES: dict[str, tuple[str, str, str]] = {
    # bench_key → (package, bench_binary, group)
    "distill":            ("ecosystem-bench", "distill_iai",       "distill"),
    "fast_strip":         ("ecosystem-bench", "fast_strip_iai",    "fast_strip"),
    "console":            ("ecosystem-bench", "console_iai",       "console_strip"),
    "strip_ansi_escapes": ("ecosystem-bench", "strip_escapes_iai", "strip_escapes"),
}


@dataclass
class IaiPoint:
    """Single iai-callgrind measurement — deterministic, host-independent."""
    ir: int | None = None           # instructions retired
    l1_hits: int | None = None      # L1 (I1 + D1) hits
    ll_hits: int | None = None      # L2/L3 hits
    ram_hits: int | None = None     # DRAM hits
    cycles: int | None = None       # estimated cycles (iai formula)
    size: int = 0                    # payload bytes (for per-MiB normalisation)


class IaiData:
    """Reader for `target/iai/**/summary.json` produced by iai-callgrind.

    Only constructed when iai output is present on disk. Every getter
    returns `None` for missing metrics so the report logic can fall
    back gracefully — a partial iai run shouldn't break the whole doc.
    """

    def __init__(self, target_dir: Path) -> None:
        self.iai_dir = target_dir / "iai"

    def available(self) -> bool:
        """True when any summary.json exists under target/iai/."""
        if not self.iai_dir.exists():
            return False
        # Any one hit is enough — don't walk the whole tree.
        return any(self.iai_dir.rglob("summary.json"))

    def _load(self, path: Path) -> dict | None:
        if not path.exists():
            return None
        try:
            with open(path) as f:
                return json.load(f)
        except (OSError, json.JSONDecodeError):
            return None

    @staticmethod
    def _metric(summary: dict, key: str) -> int | None:
        """Extract one Callgrind metric from a summary.json dict.

        iai's JSON wraps each counter in `{metrics: {<variant>: …}}`.
        The variant is one of:

        - `Left` — first (and only) run, no baseline yet
        - `Both: [curr, prev]` — rerun with baseline present
        - `Right` — baseline only (shouldn't happen for live reports)

        We want the current run, which is `Left` or `Both[0]`.
        """
        try:
            node = (summary["profiles"][0]["summaries"]["total"]
                    ["summary"]["Callgrind"][key]["metrics"])
        except (KeyError, IndexError, TypeError):
            return None
        if "Left" in node:
            m = node["Left"]
        elif "Both" in node:
            both = node["Both"]
            if not both:
                return None
            m = both[0]
        elif "Right" in node:
            m = node["Right"]
        else:
            return None
        val = m.get("Int", m.get("Float"))
        if val is None:
            return None
        try:
            return int(val)
        except (TypeError, ValueError):
            return None

    def _point(self, path: Path, size: int) -> IaiPoint | None:
        s = self._load(path)
        if s is None:
            return None
        return IaiPoint(
            ir=self._metric(s, "Ir"),
            l1_hits=self._metric(s, "L1hits"),
            ll_hits=self._metric(s, "LLhits"),
            ram_hits=self._metric(s, "RamHits"),
            cycles=self._metric(s, "EstimatedCycles"),
            size=size,
        )

    def eco_point(
        self, bench_key: str, workload: str, size: int,
    ) -> IaiPoint | None:
        """Fetch an iai point corresponding to an ecosystem bench cell.

        `workload` is one of "dirty", "cargo", "osc8" — the same keys
        used by the criterion ecosystem bench. For "dirty", `size` is
        matched to an `IAI_SIZES` tier: exact first, nearest on a
        log2 scale otherwise. Criterion sizes rarely land exactly on
        a tier (256, 512, 1024, 2048, …, 24 MiB, …) so the nearest-
        log2 fallback keeps the column populated across every row.
        """
        if bench_key not in IAI_ECO_BENCHES:
            return None
        pkg, bin_, group = IAI_ECO_BENCHES[bench_key]
        base = self.iai_dir / pkg / bin_ / group
        if workload == "dirty":
            label = self._pick_tier_label(size)
            if label is None:
                return None
            tier_bytes = IAI_SIZES[label]
            return self._point(base / f"bench_dirty.{label}" / "summary.json", tier_bytes)
        if workload == "cargo":
            return self._point(
                base / "bench_fixture.cargo" / "summary.json",
                IAI_CARGO_BYTES,
            )
        if workload == "osc8":
            return self._point(
                base / "bench_fixture.osc8" / "summary.json",
                IAI_OSC8_BYTES,
            )
        return None

    @staticmethod
    def _pick_tier_label(size: int) -> str | None:
        """Pick the IAI_SIZES tier closest to `size` on a log2 axis.

        Exact match wins. Otherwise we fall back to the tier whose
        `log2(tier)` is closest — this avoids skewing small sizes
        to XLARGE and vice versa. Returns None on invalid input.
        """
        if size <= 0:
            return None
        for label, tier in IAI_SIZES.items():
            if tier == size:
                return label
        target = math.log2(size)
        return min(
            IAI_SIZES,
            key=lambda k: abs(math.log2(IAI_SIZES[k]) - target),
        )

    def internals_point(
        self, fn_name: str, bench_id: str | None, size: int,
    ) -> IaiPoint | None:
        """Fetch a point from the `internals_iai` bench binary.

        Layout: `target/iai/distill-strip-ansi/internals_iai/
        internals_iai/<fn>.<id>/summary.json`

        `fn_name` is the `#[library_benchmark]` function name;
        `bench_id` is the `#[bench::<id>(…)]` label (e.g. "tiny",
        "cargo") or None for bench fns with no parametrisation
        (`augment_protanopia_256`). `size` is the payload bytes for
        Ir/MiB normalisation.
        """
        base = (
            self.iai_dir
            / "distill-strip-ansi"
            / "internals_iai"
            / "internals_iai"
        )
        if bench_id is None:
            dir_ = base / fn_name
        else:
            dir_ = base / f"{fn_name}.{bench_id}"
        return self._point(dir_ / "summary.json", size)


# ── iai value formatting (lives near the rest of fmt_* helpers) ───


def fmt_ir(ir: int | None) -> str:
    """Render an instruction count in engineering units (K/M/G)."""
    if ir is None or ir <= 0:
        return "—"
    if ir < 1_000:
        return str(ir)
    if ir < 1_000_000:
        return f"{ir / 1_000:.1f}K"
    if ir < 1_000_000_000:
        return f"{ir / 1_000_000:.1f}M"
    return f"{ir / 1_000_000_000:.1f}G"


def fmt_ir_per_mib(ir: int | None, nbytes: int) -> str:
    """Instructions per MiB — host-independent CPU budget currency."""
    if ir is None or ir <= 0 or nbytes <= 0:
        return "—"
    per_mib = ir / (nbytes / (1024 * 1024))
    return fmt_ir(int(per_mib))


# ═══════════════════════════════════════════════════════════════════
# Environment
# ═══════════════════════════════════════════════════════════════════


class Environment:
    @staticmethod
    def detect() -> list[tuple[str, str]]:
        pairs: list[tuple[str, str]] = []
        pairs.append(("CPU", Environment._detect_cpu()))
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
    def _detect_cpu() -> str:
        """Detect CPU model string across macOS, Linux, and BSD.

        Order of attempts:
          1. macOS: `sysctl -n machdep.cpu.brand_string`
          2. Linux: first `model name` line in /proc/cpuinfo;
             fallback to `Hardware` (ARM/Raspberry Pi) or `cpu model` (PowerPC)
          3. FreeBSD/OpenBSD: `sysctl -n hw.model`
          4. Python platform.processor() (often empty on Linux, fine on Windows)
          5. "unknown" sentinel
        """
        # macOS
        try:
            out = subprocess.check_output(
                ["sysctl", "-n", "machdep.cpu.brand_string"],
                stderr=subprocess.DEVNULL, text=True).strip()
            if out:
                return out
        except (subprocess.CalledProcessError, FileNotFoundError):
            pass

        # Linux / /proc/cpuinfo
        cpuinfo = Path("/proc/cpuinfo")
        if cpuinfo.exists():
            try:
                text = cpuinfo.read_text()
                for key in ("model name", "Hardware", "cpu model", "Processor"):
                    for line in text.splitlines():
                        if line.startswith(key):
                            _, _, val = line.partition(":")
                            val = val.strip()
                            if val:
                                return val
            except OSError:
                pass

        # BSD
        try:
            out = subprocess.check_output(
                ["sysctl", "-n", "hw.model"],
                stderr=subprocess.DEVNULL, text=True).strip()
            if out:
                return out
        except (subprocess.CalledProcessError, FileNotFoundError):
            pass

        # Python fallback (Windows: `x86_64 Intel64 Family 6 Model 165 ...`)
        proc = platform.processor()
        if proc:
            return proc

        return "unknown"

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
    def target_cpu() -> str:
        """Discover the target-cpu rustflag for the current triple.

        Reads `.cargo/config.toml` (workspace-local only — doesn't
        walk up to ~/.cargo/config.toml on purpose; the workspace
        config is authoritative for benchmark reproducibility).

        Precedence:
          1. CARGO_BUILD_RUSTFLAGS env override for `-C target-cpu=`
          2. Triple-specific `[target.<triple>] rustflags` entry
          3. Generic `[build] rustflags`
          4. None → "default" (rustc's generic baseline)

        Triple detection uses `rustc -vV` which is always available
        when cargo is; it gives the true default-host triple, not
        what target/ happens to hold.
        """
        env_flags = os.environ.get("CARGO_BUILD_RUSTFLAGS", "")
        cpu = Environment._cpu_from_rustflags(env_flags)
        if cpu:
            return cpu

        triple = Environment._host_triple()
        cfg = Environment._read_cargo_config(Path(".cargo/config.toml"))
        if not cfg:
            return "default"

        # Triple-specific
        if triple:
            tcfg = cfg.get("target", {}).get(triple, {})
            cpu = Environment._cpu_from_rustflags(tcfg.get("rustflags", []))
            if cpu:
                return cpu

        # Generic [build]
        cpu = Environment._cpu_from_rustflags(cfg.get("build", {}).get("rustflags", []))
        if cpu:
            return cpu

        return "default"

    @staticmethod
    def _host_triple() -> str | None:
        try:
            out = subprocess.check_output(
                ["rustc", "-vV"],
                stderr=subprocess.DEVNULL, text=True)
        except (subprocess.CalledProcessError, FileNotFoundError):
            return None
        for line in out.splitlines():
            if line.startswith("host:"):
                return line.split(":", 1)[1].strip()
        return None

    @staticmethod
    def _cpu_from_rustflags(flags: str | list[str]) -> str | None:
        """Extract the target-cpu=VALUE pair from a rustflags setting.

        Accepts either the list form (`["-C", "target-cpu=X"]`) or
        the string form (`"-C target-cpu=X"`) — both are valid
        cargo config shapes.
        """
        if isinstance(flags, str):
            tokens = flags.split()
        else:
            tokens = list(flags)
        # Look for `-C target-cpu=X` (two tokens) and `-Ctarget-cpu=X` (one).
        i = 0
        while i < len(tokens):
            t = tokens[i]
            if t == "-C" and i + 1 < len(tokens):
                if tokens[i + 1].startswith("target-cpu="):
                    return tokens[i + 1].split("=", 1)[1]
                i += 2
                continue
            if t.startswith("-Ctarget-cpu="):
                return t.split("=", 1)[1]
            i += 1
        return None

    @staticmethod
    def _read_cargo_config(path: Path) -> dict | None:
        """Read a Cargo config.toml, using Python 3.11+ tomllib."""
        if not path.exists():
            return None
        try:
            import tomllib
        except ImportError:
            # Python < 3.11 fallback via `tomli` if installed.
            try:
                import tomli as tomllib  # type: ignore[no-redef]
            except ImportError:
                return None
        try:
            with open(path, "rb") as f:
                return tomllib.load(f)
        except (OSError, tomllib.TOMLDecodeError):
            return None

    @staticmethod
    def dep_count(spec: str) -> str:
        try:
            out = subprocess.check_output(
                ["cargo", "tree", "-p", spec, "-e", "normal",
                 "--depth", "999", "--prefix", "none"],
                stderr=subprocess.DEVNULL, text=True)
            crates = {ln.strip().split()[0] for ln in out.strip().splitlines() if ln.strip()}
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
            crates = {ln.strip().split()[0] for ln in out.strip().splitlines() if ln.strip()}
            return str(len(crates) - 1)
        except (subprocess.CalledProcessError, FileNotFoundError):
            return "—"


# ═══════════════════════════════════════════════════════════════════
# Report builder
# ═══════════════════════════════════════════════════════════════════


class BenchmarkReport:
    def __init__(
        self,
        data: BenchData,
        target_dir: Path,
        iai: IaiData | None = None,
    ) -> None:
        self.data = data
        self.target_dir = target_dir
        self.iai = iai
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

    @staticmethod
    def _load_target_cpu_notes(target_cpu: str) -> str:
        """Load per-target prose from `etc/target-cpu-notes/`.

        Selection order:
          1. `etc/target-cpu-notes/<target_cpu>.md` (exact match)
          2. `etc/target-cpu-notes/default.md`
          3. hard-coded single-line fallback

        The filename is the raw `-C target-cpu=…` value, so adding
        support for a new target is drop-a-file, not edit-a-script.

        HTML comments at the top of each file are stripped —
        they're maintainer notes, not prose meant for the rendered
        doc.
        """
        base = Path("etc/target-cpu-notes")
        for candidate in (base / f"{target_cpu}.md", base / "default.md"):
            if candidate.exists():
                text = candidate.read_text()
                # Strip all leading HTML comment blocks (including
                # markdownlint disable directives) — they're
                # maintainer hints, never rendered prose.
                while True:
                    stripped = re.sub(
                        r"\A\s*<!--.*?-->\s*", "", text, count=1, flags=re.S,
                    )
                    if stripped == text:
                        break
                    text = stripped
                return text
        return (
            f"No per-target notes found for `{target_cpu}`. "
            "Drop a file at `etc/target-cpu-notes/<target-cpu>.md` "
            "to add prose here."
        )

    def _render_template(self, template: str) -> str:
        """Fill {{MARKER}} placeholders in the template with dynamic data."""
        sections: dict[str, str] = {}

        # HIGHLIGHTS — dynamic bullet lines only; static bullets
        # ("Zero allocation…", "O(n)…", "No temp files…") live in
        # the template.
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

        # HOWTO — only the runtime-duration hint is dynamic now;
        # all the static prose (pinning, mise tasks, invocations,
        # bench-suite list, harness description) lives in the
        # template. Emit "" when bench hasn't run (keeps the
        # template sentence grammatical without an estimate).
        wall = self.data.wall_secs
        sections["BENCH_DURATION_HINT"] = f" (~{fmt_duration(wall)})" if wall > 0 else ""

        # TEST_DATA_TABLE — cache-tier table shown in HOWTO.
        self.lines = []
        self._test_data_table_content()
        sections["TEST_DATA_TABLE"] = "\n".join(self.lines).strip()

        # DETAILS — per-size Dirty tables + Cargo/OSC 8 comparisons.
        self.lines = []
        self._details_content()
        sections["DETAILS"] = "\n".join(self.lines).strip()

        # EXTENDED_CAPS — distill-only features table.
        self.lines = []
        self._unique_features_content()
        sections["EXTENDED_CAPS"] = "\n".join(self.lines).strip()

        # SCALING — per-crate bar charts.
        self.lines = []
        self._scaling_content()
        sections["SCALING"] = "\n".join(self.lines).strip()

        # COMPLEXITY_SUMMARY — summary table only; surrounding
        # prose lives in the template.
        self.lines = []
        self._complexity_summary_content()
        sections["COMPLEXITY_SUMMARY"] = "\n".join(self.lines).strip()

        result = template
        for key, value in sections.items():
            result = result.replace("{{" + key + "}}", value)

        # Collapse any resulting triple+ newlines to double.
        result = re.sub(r'\n{3,}', '\n\n', result)
        return result

    def generate_reproduce(self, template: str) -> str:
        """Render the reproduction/setup companion doc.

        The repro template shares environment + version placeholders
        with the main doc, plus its own `{{MISE_TASKS}}` / `{{IAI_STATUS}}`
        sections. No bench numbers cross this boundary — keep the
        two docs independently readable.
        """
        sections: dict[str, str] = {}

        self.lines = []
        self._environment_content()
        sections["ENV"] = "\n".join(self.lines).strip()

        self.lines = []
        self._versions_content()
        sections["VERSIONS"] = "\n".join(self.lines).strip()

        self.lines = []
        self._test_data_table_content()
        sections["TEST_DATA_TABLE"] = "\n".join(self.lines).strip()

        # target-cpu lives in .cargo/config.toml per-triple; surface
        # the detected value so the build-config section stays
        # truthful across hosts.
        target_cpu = Environment.target_cpu()
        sections["TARGET_CPU"] = target_cpu
        sections["TARGET_CPU_NOTES"] = self._load_target_cpu_notes(target_cpu).strip()

        # Build-config table rendered from the generator so the
        # target-cpu cell aligns correctly — template substitution
        # changes cell width on replace and breaks MD060.
        target_cpu_cell = f"`{target_cpu}`"
        sections["BUILD_CONFIG_TABLE"] = md_table(
            [Col("Setting"), Col("Value"), Col("Effect")],
            [
                ["`codegen-units`", "`1`",                 "Full optimizer visibility"],
                ["`lto`",           '`"thin"`',             "Cross-module inlining"],
                ["`panic`",         '`"abort"`',            "No unwind tables"],
                ["`strip`",         'release `"symbols"`',  "Smaller ship binaries"],
                ["`strip`",         "bench `false`",        "Keeps iai wrapper symbols"],
                ["`debug`",         "bench `line-tables`",  "callgrind line attribution"],
                ["`target-cpu`",    target_cpu_cell,        "ISA-level tuning (see below)"],
            ],
        )

        # Report whether iai-callgrind data is on disk so the repro
        # doc can mention deterministic numbers as "yes, already ran"
        # vs "run mise x bench:callgrind first". Broken across lines
        # to keep prose under the 80-col limit.
        if self.iai is not None:
            sections["IAI_STATUS"] = (
                "Instruction-count data is present in `target/iai/` — "
                "`doc/BENCHMARKS.md`\nincludes `Ir/MiB` columns where "
                "applicable."
            )
        else:
            sections["IAI_STATUS"] = (
                "No iai-callgrind data yet. Run `mise x bench:callgrind` "
                "(requires\n`valgrind`) to add instruction-count columns "
                "to the results doc on the\nnext regenerate."
            )

        wall = self.data.wall_secs
        sections["BENCH_DURATION_HINT"] = f" (~{fmt_duration(wall)})" if wall > 0 else ""

        result = template
        for key, value in sections.items():
            result = result.replace("{{" + key + "}}", value)
        return re.sub(r'\n{3,}', '\n\n', result)

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
        """Emit only the dynamic highlight bullets.

        Static bullets (`Cow::Borrowed`, O(n), no disk I/O) are in
        the template — those are invariants of the codebase, not
        measured properties of a bench run.
        """
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
        env_rows.append(["target-cpu", f"`{Environment.target_cpu()}`"])
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
        """Procedural fallback path — template preferred.

        The template at `doc/BENCHMARKS.md.in` owns the full HOWTO
        prose (pinning advice, mise tasks, direct invocations,
        bench-suite list, harness description). The procedural
        path emits a minimal stub pointing readers at the template
        when it can't be found — this should never run in practice,
        as the template is committed to the repo.
        """
        self.emit("", "## HOWTO: Reproduce", "")
        self._howto_content()

    def _howto_content(self) -> None:
        wall = self.data.wall_secs
        est = f" (~{fmt_duration(wall)})" if wall > 0 else ""
        self.emit(
            "`doc/BENCHMARKS.md.in` not found — rendering a minimal",
            "HOWTO. Restore the template for the full reproduction guide.",
            "",
            "```bash",
            f"./bin/generate-benchmarks-md.py  # Full run{est}",
            "BENCH_QUICK=1 ./bin/generate-benchmarks-md.py  # Fast iteration",
            "./bin/generate-benchmarks-md.py --no-run  # Re-render from cache",
            "```",
        )
        self.emit("", "### Test Data Strategy", "")
        self._test_data_table_content()

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

        # Extended-capabilities table is a separate placeholder in
        # the template ({{EXTENDED_CAPS}}). Don't duplicate it here.

    def _test_data_table_content(self) -> None:
        """Emit just the test-data cache-tier table."""
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

    def _complexity_summary_content(self) -> None:
        """Emit just the complexity-summary table.

        Surrounding prose (`Complexity estimated per memory tier…`)
        lives in the template.
        """
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

    def _scaling(self) -> None:
        """Procedural fallback: bar charts + complexity summary + prose.

        Template path emits these as separate placeholders —
        {{SCALING}} (bar charts only) and {{COMPLEXITY_SUMMARY}}
        (summary table only).
        """
        self.emit(
            "", "## Scaling", "",
            "Dirty throughput (MiB/s) across input sizes.",
            "Constant bar length = O(n). Shrinking = super-linear.",
            "",
            "RSS Δ and CPU shown at largest size only — small-size",
            "values are dominated by benchmark harness overhead.",
        )
        self._scaling_content()
        self.emit("", "### Complexity Summary", "")
        self._complexity_summary_content()
        self.emit(
            "",
            "Complexity estimated per memory tier (L1/L2/L3/DRAM) —",
            "throughput steps between tiers are hardware, not algorithmic.",
        )

    def _scaling_content(self) -> None:
        """Emit just the per-crate bar charts.

        Surrounding prose and the complexity summary live in the
        template as separate sections.
        """
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

    def _unique_features(self) -> None:
        """Benchmark features only distill-strip-ansi offers.

        Procedural fallback for when the template isn't available —
        calls `_unique_features_content` after emitting the header.
        """
        self.emit(
            "", "### Extended Capabilities", "",
            "Additional features available in `distill-strip-ansi`.",
            "",
        )
        self._unique_features_content()

    def _unique_features_content(self) -> None:
        """Emit only the extended-capabilities table.

        Ordered by pipeline path-of-operations:
        classify → filter/threats → streaming → unicode →
        color pipeline (passthrough baseline, transforms, augments).

        Columns mirror the ecosystem tables (sans `×`, since this
        table is distill-only): Feature, Time, MiB/s, RSS Δ, and
        Ir/MiB when iai-callgrind data is on disk. CPU is dropped
        for width — same reason as the ecosystem tables.
        """
        cache = self.data.cache_info
        l1 = cache.get("l1d", 32768)
        l2 = cache.get("l2", 262144)
        l3 = cache.get("l3", 12582912)

        @dataclass
        class Feature:
            # Display name in the first column.
            name: str
            # Criterion path: `target/criterion/<group>/<bench>/…`.
            group: str
            bench: str
            # Size for criterion lookup. None → auto-discover; use
            # `equiv_bytes` to override what we divide by for MiB/s
            # and Ir/MiB when the bench measures non-byte payloads
            # (e.g. 256 RGB transforms = 768 bytes).
            size: int | None = None
            equiv_bytes: int | None = None
            # iai-callgrind path: `<iai_fn>[.<iai_id>]/summary.json`
            # under `target/iai/distill-strip-ansi/internals_iai/
            # internals_iai/`. None on either → no iai lookup, cell
            # shows "—". We only map features that have a live
            # internals_iai bench; add one here when you add the
            # matching `#[library_benchmark]` over in
            # `benches/internals_iai.rs`.
            iai_fn: str | None = None
            iai_id: str | None = None

        features = [
            # ── Classify ──
            Feature("Classify (parse only)", "classifier", "cargo_classify",
                    iai_fn="classifier_cargo_no_detail", iai_id="cargo"),
            Feature("Classify + detail", "classifier", "cargo_classify_detail",
                    iai_fn="classifier_cargo", iai_id="cargo"),
            # ── Filter ──
            Feature("Filter: SGR mask", "filter_detail", "sgr_mask",
                    iai_fn="filter_sgr_mask", iai_id="cargo"),
            Feature("Filter: sanitize preset", "filter_detail", "sanitize_preset",
                    iai_fn="filter_sanitize_preset", iai_id="cargo"),
            # ── Threat scan (security, adjacent to filter) ──
            Feature("Threat scan (clean)", "check_threats", "scan_clean",
                    iai_fn="threat_scan", iai_id="clean_cargo"),
            Feature("Threat scan (dirty)", "check_threats", "scan_only",
                    iai_fn="threat_scan_dirty", iai_id="dirty_cargo"),
            # ── Streaming (strip delivery across cache tiers) ──
            Feature("Streaming (L1)", "stream", "strip_slices", size=l1,
                    iai_fn="stream_dirty",
                    iai_id=IaiData._pick_tier_label(l1) or ""),
            Feature("Streaming (L2)", "stream", "strip_slices", size=l2,
                    iai_fn="stream_dirty",
                    iai_id=IaiData._pick_tier_label(l2) or ""),
            Feature("Streaming (L3)", "stream", "strip_slices", size=l3,
                    iai_fn="stream_dirty",
                    iai_id=IaiData._pick_tier_label(l3) or ""),
            # ── Unicode normalize ──
            Feature("Unicode normalize", "unicode_normalize", "real_world_cargo",
                    iai_fn="unicode_normalize_cargo", iai_id="cargo"),
            # ── Color pipeline: passthrough baseline ──
            Feature("Transform: passthrough", "transform", "passthrough",
                    iai_fn="transform_passthrough", iai_id="passthrough"),
            # ── Color pipeline: depth transforms ──
            # Each criterion row has a dedicated iai bench so the
            # Ir/MiB column is fully populated — see
            # `benches/internals_iai.rs` for the 1:1 mapping.
            Feature("Transform: truecolor→mono", "transform", "truecolor_to_mono",
                    iai_fn="transform_to_mono", iai_id="truecolor_to_mono"),
            Feature("Transform: truecolor→grey", "transform", "truecolor_to_greyscale",
                    iai_fn="transform_truecolor_to_grey",
                    iai_id="truecolor_to_greyscale"),
            Feature("Transform: truecolor→16", "transform", "truecolor_to_16",
                    iai_fn="transform_truecolor_to_16", iai_id="truecolor_to_16"),
            Feature("Transform: truecolor→256", "transform", "truecolor_to_256",
                    iai_fn="transform_truecolor_to_256", iai_id="truecolor_to_256"),
            Feature("Transform: 256→16", "transform", "256_to_16",
                    iai_fn="transform_256_to_16", iai_id="color256_to_16"),
            Feature("Transform: 256→grey", "transform", "256_to_greyscale",
                    iai_fn="transform_256_to_grey", iai_id="color256_to_greyscale"),
            Feature("Transform: basic→mono", "transform", "basic_to_mono",
                    iai_fn="transform_basic_to_mono", iai_id="basic_to_mono"),
            # ── Color pipeline: vision augmentation ──
            # 256 RGB transforms per iteration = 768 bytes (3 per color).
            # sRGB roundtrip = 256 single-channel values.
            Feature("Augment: protanopia", "augment_color", "protanopia_256",
                    equiv_bytes=768, iai_fn="augment_protanopia_256"),
            Feature("Augment: deuteranopia", "augment_color", "deuteranopia_256",
                    equiv_bytes=768, iai_fn="augment_deuteranopia_256"),
            Feature("Augment: sRGB roundtrip", "augment_color", "srgb_roundtrip_256",
                    equiv_bytes=256, iai_fn="augment_srgb_roundtrip_256"),
        ]

        # Decide whether to show the Ir/MiB column up front, by
        # scanning for any feature that produced iai data.
        iai_present = False
        if self.iai is not None:
            for f in features:
                if f.iai_fn and self.iai.internals_point(
                    f.iai_fn, f.iai_id or None, f.equiv_bytes or 1,
                ) is not None:
                    iai_present = True
                    break

        rows: list[list[str]] = []
        any_noisy = False
        for f in features:
            ns: float | None = None
            cv: float | None = None
            actual_size = f.size
            if f.size is not None:
                stats = self.data._read_estimates(f.group, f.bench, f.size)
                if stats:
                    ns, cv = stats
            elif f.equiv_bytes is not None:
                stats = self.data._read_estimates(f.group, f.bench, None)
                if stats:
                    ns, cv = stats
                actual_size = f.equiv_bytes
            else:
                # Discover size from Criterion dirs.
                group_dir = self.data.criterion_dir / f.group / f.bench
                if group_dir.exists():
                    for d in sorted(group_dir.iterdir()):
                        if d.is_dir() and d.name.isdigit():
                            actual_size = int(d.name)
                            stats = self.data._read_estimates(
                                f.group, f.bench, actual_size,
                            )
                            if stats:
                                ns, cv = stats
                                break

            t = fmt_time(ns)
            if cv is not None and cv >= NOISY_CV_THRESHOLD:
                any_noisy = True
            m = fmt_mibs(ns, actual_size) if (ns and actual_size) else "—"

            row = [f.name, mark_noisy(t, cv), m]
            if iai_present:
                iai_cell = "—"
                if f.iai_fn and self.iai is not None and actual_size:
                    iai_pt = self.iai.internals_point(
                        f.iai_fn, f.iai_id or None, actual_size,
                    )
                    if iai_pt is not None:
                        iai_cell = fmt_ir_per_mib(iai_pt.ir, actual_size)
                row.append(iai_cell)
            rows.append(row)

        cols = [Col("Feature"), Col("Time", "right"), Col("MiB/s", "right")]
        if iai_present:
            cols.append(Col("Ir/MiB", "right"))
        table = md_table(cols, rows)
        if any_noisy:
            table += (
                f"\n\n⚠ marks cells where CV ≥ {NOISY_CV_THRESHOLD * 100:.0f}% — "
                "re-run `mise x bench:callgrind`\nfor a deterministic "
                "`Ir/MiB` check."
            )
        self.emit(table)

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
        """Render per-crate comparison — compact, units in headers.

        When iai-callgrind data is available for this size+workload,
        inject an `Ir/MiB` column so the reader sees the host-independent
        CPU budget right next to wall-clock throughput. The column is
        omitted entirely when iai data is absent — we don't clutter
        criterion-only tables with empty cells.
        """
        base = self.data.get_point("distill", workload, size)

        # Check if iai has anything to say about this cell before
        # deciding the column layout.
        iai_present = False
        if self.iai is not None:
            for bk, _, _ in CRATES:
                if self.iai.eco_point(bk, workload, size) is not None:
                    iai_present = True
                    break

        rows: list[list[str]] = []
        any_noisy = False
        for bk, _, display in CRATES:
            pt = self.data.get_point(bk, workload, size)
            t = mark_noisy(fmt_time(pt.ns), pt.cv)
            if pt.cv is not None and pt.cv >= NOISY_CV_THRESHOLD:
                any_noisy = True
            m = fmt_mibs(pt.ns, size)
            r = "base" if bk == "distill" else fmt_ratio(pt.ns, base.ns)
            # `RSS Δ` is allocator page retention after the last
            # iteration — order-dependent (earlier benches pay for
            # Vec growth that later benches inherit for free) and
            # not a faithful per-crate signal. The Crate Footprints
            # summary reports peak RSS at a fair measurement point;
            # per-size tables stay focused on time + instructions.
            row = [display, t, m, r]
            if iai_present:
                iai_pt = self.iai.eco_point(bk, workload, size) if self.iai else None
                row.append(fmt_ir_per_mib(iai_pt.ir if iai_pt else None, size))
            rows.append(row)

        cols = [
            Col("Crate"), Col("Time", "right"),
            Col("MiB/s", "right"), Col("×", "right"),
        ]
        if iai_present:
            cols.append(Col("Ir/MiB", "right"))
        table = md_table(cols, rows)
        if any_noisy:
            table += (
                f"\n\n⚠ marks cells where CV ≥ {NOISY_CV_THRESHOLD * 100:.0f}% — "
                "re-run `mise x bench:callgrind`\nfor a deterministic "
                "`Ir/MiB` check."
            )
        return table

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

# iai-callgrind variants — parallel list, run only when the
# `iai-callgrind` feature is active. Always paired with
# `-- --save-summary=json` so the parser has something to chew on;
# without that flag, iai writes only the `.out`/`.log` callgrind
# files and not the JSON shape we consume.
IAI_BENCH_CMD = [
    "cargo", "bench", "--features", "iai-callgrind", "--bench", "internals_iai",
]
IAI_ECOSYSTEM_BENCH_CMDS = [
    ["cargo", "bench", "-p", "ecosystem-bench", "--features", "iai-callgrind",
     "--bench", "distill_iai"],
    ["cargo", "bench", "-p", "ecosystem-bench", "--features", "iai-callgrind",
     "--bench", "fast_strip_iai"],
    ["cargo", "bench", "-p", "ecosystem-bench", "--features", "iai-callgrind",
     "--bench", "console_iai"],
    ["cargo", "bench", "-p", "ecosystem-bench", "--features", "iai-callgrind",
     "--bench", "strip_escapes_iai"],
]

OUTPUT_FILE = Path("doc/BENCHMARKS.md")
TEMPLATE_FILE = Path("doc/BENCHMARKS.md.in")
# Companion template for reproduction/setup notes — rendered
# alongside BENCHMARKS.md when present. Keeps the results-facing
# doc focused on numbers; maintainers read the separate file.
REPRO_OUTPUT_FILE = Path("doc/BENCHMARKS-REPRODUCE.md")
REPRO_TEMPLATE_FILE = Path("doc/BENCHMARKS-REPRODUCE.md.in")
TARGET_DIR = Path("target")


def _extract_bench_name(cmd: list[str]) -> str:
    """Pull the bench-binary name out of a `cargo bench --bench X` command."""
    try:
        i = cmd.index("--bench")
        return cmd[i + 1]
    except (ValueError, IndexError):
        return "?"


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Generate doc/BENCHMARKS.md from Criterion JSON + "
                    "resource snapshots. Runs cargo bench across the "
                    "internals and ecosystem suites by default.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="Environment: BENCH_QUICK=1 for fast iteration; "
               "BENCH_MAX_SIZE as an alternative to --max-size.",
    )
    parser.add_argument(
        "--no-run", action="store_true",
        help="Skip cargo bench; re-render doc/BENCHMARKS.md from "
             "existing target/criterion data (~1 sec).",
    )
    parser.add_argument(
        "--max-size", default=None, metavar="SIZE",
        help="Cap per-bench input size (e.g. 64M, 1G). Default: 2×L3. "
             "0=unlimited. Exported as BENCH_MAX_SIZE to bench processes.",
    )
    parser.add_argument(
        "--output", "-o", type=Path, default=OUTPUT_FILE, metavar="PATH",
        help=f"Output markdown path for the results doc "
             f"(default: {OUTPUT_FILE}).",
    )
    parser.add_argument(
        "--reproduce-output", type=Path, default=REPRO_OUTPUT_FILE, metavar="PATH",
        help=f"Output path for the reproduction/setup doc, if the "
             f"template at {REPRO_TEMPLATE_FILE} exists "
             f"(default: {REPRO_OUTPUT_FILE}).",
    )
    parser.add_argument(
        "--features", "-F", action="append", default=[],
        metavar="FEATURES",
        help="Cargo feature(s) to forward to every bench invocation. "
             "Repeatable or comma-separated (e.g. --features iai-callgrind "
             "or -F a -F b). The literal `all-features` is treated as the "
             "cargo `--all-features` flag — it's not a real feature name, "
             "but it's the intuitive thing to type.",
    )
    args = parser.parse_args()

    # Normalise the `-F` / `--features` values. Two subtleties:
    #
    # 1. The literal string "all-features" is the cargo *flag*
    #    `--all-features`, not a feature name, but it's the
    #    intuitive thing to type after `-F`. Detect it up front and
    #    route it to the flag path — otherwise cargo rejects the
    #    command with "does not contain this feature: all-features".
    # 2. Comma-separated values are split, blanks stripped, and
    #    duplicates removed preserving first-seen order so
    #    `-F a,b -F a` behaves the same as `-F a,b`.
    features: list[str] = []
    want_all_features = False
    for chunk in args.features:
        for f in chunk.split(","):
            f = f.strip()
            if not f:
                continue
            if f == "all-features":
                want_all_features = True
                continue
            if f not in features:
                features.append(f)

    def with_features(cmd: list[str]) -> list[str]:
        """Forward feature flags to a cargo bench command.

        Produces a copy of `cmd` with, in order:
          - `--all-features` appended when the user asked for it
            (and not already in `cmd` — some built-in bench
            commands hardcode it, e.g. the internals criterion
            bench).
          - `--features <csv>` appended for the user's explicit list.

        Cargo unions `--all-features` and `--features X`, so the two
        coexist cleanly.
        """
        out = list(cmd)
        if want_all_features and "--all-features" not in out:
            out.append("--all-features")
        if features:
            out += ["--features", ",".join(features)]
        return out

    if not args.no_run:
        # CARGO_TARGET_DIR forces both cargo and criterion to use
        # ./target — overrides any global [build] target-dir config.
        env = dict(os.environ)
        env["CARGO_TARGET_DIR"] = str(TARGET_DIR.resolve())
        if args.max_size:
            env["BENCH_MAX_SIZE"] = args.max_size

        # iai-callgrind is additive: `--features iai-callgrind` does
        # NOT replace the criterion run, it appends an instruction-
        # count pass *after* the wall-clock pass. This way
        # `--max-size 1G --features iai-callgrind` still expands the
        # criterion size ladder (iai uses its own fixed tier set),
        # and readers get both throughput and Ir/MiB for the same
        # session.
        iai_mode = "iai-callgrind" in features
        criterion_run = [with_features(cmd) for cmd in [BENCH_CMD] + ECOSYSTEM_BENCH_CMDS]
        if iai_mode:
            # iai-callgrind commands already carry their own
            # `--features iai-callgrind`, so we don't re-apply the
            # feature list via with_features() — Cargo rejects a
            # duplicate `--features` flag. We do still honour
            # `-F all-features` here: insert the flag directly so
            # the iai pass exercises every feature the user asked
            # the criterion pass to exercise.
            def _iai_cmd(cmd: list[str]) -> list[str]:
                out = list(cmd)
                if want_all_features and "--all-features" not in out:
                    # Insert before `--bench`/`-p` so it reads naturally.
                    out.insert(2, "--all-features")
                return out + ["--", "--save-summary=json"]

            iai_run = [
                _iai_cmd(cmd)
                for cmd in [IAI_BENCH_CMD] + IAI_ECOSYSTEM_BENCH_CMDS
            ]
            run_set = criterion_run + iai_run
        else:
            run_set = criterion_run

        # iai-callgrind is chatty: ~6 lines per bench × dozens of
        # benches drowns the terminal. Route its stdout to a log
        # file and print one concise line per bench. The summary
        # JSON is the source of truth anyway — interactive output
        # is just "did it finish, and how fast".
        iai_log = TARGET_DIR / "iai" / "run.log"
        if iai_mode:
            iai_log.parent.mkdir(parents=True, exist_ok=True)
            iai_log.write_text("")
            # Suppress iai's own progress logging beyond warnings.
            env.setdefault("IAI_CALLGRIND_LOG", "warn")

        failed: list[str] = []
        for cmd in run_set:
            # Detect iai vs criterion per-command: iai benches carry
            # `--save-summary=json` in their trailing args, criterion
            # benches don't. This matters because the additive run
            # set (criterion + iai) interleaves both in a single
            # pass and we want output appropriate for each.
            is_iai = "--save-summary=json" in cmd
            if is_iai:
                bench_name = _extract_bench_name(cmd)
                print(f"  callgrind: {bench_name}…", file=sys.stderr, end="", flush=True)
                t0 = time.monotonic()
                with open(iai_log, "a") as logf:
                    logf.write(f"\n=== {' '.join(cmd)} ===\n")
                    logf.flush()
                    r = subprocess.run(cmd, env=env, stdout=logf, stderr=logf)
                elapsed = time.monotonic() - t0
                status = "ok" if r.returncode == 0 else f"FAIL ({r.returncode})"
                print(f" {status} [{fmt_duration(elapsed)}]", file=sys.stderr)
            else:
                print(f"Running: {' '.join(cmd)}", file=sys.stderr)
                r = subprocess.run(cmd, env=env)
            if r.returncode != 0:
                failed.append(" ".join(cmd))
                if not is_iai:
                    print(f"  ⚠ failed (exit {r.returncode}), continuing…",
                          file=sys.stderr)

        if iai_mode and iai_log.exists():
            print(
                f"  iai full log: {iai_log} ({iai_log.stat().st_size:,} bytes)",
                file=sys.stderr,
            )

        if failed:
            print(f"\n⚠ {len(failed)} bench(es) failed — report may be incomplete:",
                  file=sys.stderr)
            for f in failed:
                print(f"  • {f}", file=sys.stderr)
            print(file=sys.stderr)

    data = BenchData(TARGET_DIR)
    iai = IaiData(TARGET_DIR)
    # Only pass IaiData to the report when there's something on disk.
    # With --no-run after a prior iai session, we auto-pick it up;
    # without any iai output we render the criterion-only view so the
    # doc stays usable for contributors who never run valgrind.
    report = BenchmarkReport(data, TARGET_DIR, iai=iai if iai.available() else None)
    doc = report.generate()
    args.output.write_text(doc)
    print(f"Wrote {args.output}", file=sys.stderr)

    # Reproduction/setup doc: rendered from its own template when
    # present. Uses the same Environment detection + mise task prose
    # but doesn't touch any bench numbers — keep separable concerns
    # in separate files.
    repro_tmpl = REPRO_TEMPLATE_FILE
    if repro_tmpl.exists():
        repro = report.generate_reproduce(repro_tmpl.read_text())
        args.reproduce_output.write_text(repro)
        print(f"Wrote {args.reproduce_output}", file=sys.stderr)


if __name__ == "__main__":
    main()
