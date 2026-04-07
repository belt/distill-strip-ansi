# Security Model

How this crate defends against ANSI escape sequence attacks.
Assumes ECMA-48 familiarity ([ANSI-REFERENCE.md](ANSI-REFERENCE.md)).
CVE-specific coverage in [CVE-MITIGATION.md](CVE-MITIGATION.md).

## Threat Model

This crate sits between untrusted byte streams and a terminal.
Two attack classes:

1. **Escape injection** (CWE-150) — attacker embeds ANSI
   sequences in data (filenames, URLs, logs, build output)
   that reaches a terminal unsanitized. Effects: spoofed
   prompts, hidden text, clipboard hijack, terminal DoS.

2. **Echoback** — attacker embeds *query* sequences that make
   the terminal write responses into stdin. If the queried
   state is attacker-controlled, the terminal executes
   commands on the user's behalf. No interaction required
   for full echoback variants.

## Echoback Vectors

Three sequence types enable echoback:

| Vector        | Sequence     | Kind        |
| ------------- | ------------ | ----------- |
| DECRQSS       | DCS `$ q` …  | `Dcs`       |
| OSC 50 query  | OSC `50 ; ?` | `Osc`       |
| CSI 21t title | CSI `21 t`   | `CsiWindow` |

**DECRQSS**: terminal echoes DCS body verbatim into stdin.
Full echoback = arbitrary chars including newlines/ctrl.

**OSC 50**: terminal echoes attacker-set font name. In Zsh
vi-mode, BEL terminator triggers `list-expand` → code exec.

**CSI 21t**: terminal echoes attacker-set window title into
stdin. Two-step: OSC plants payload, CSI triggers echo.
Stripping either half breaks the chain.

## Preset Security Properties

| Preset     | DCS | OSC  | CSI 21t | Echoback risk | --unsafe |
| ---------- | --- | ---- | ------- | ------------- | -------- |
| `dumb`     | ✗   | ✗    | ✗       | None          |          |
| `color`    | ✗   | ✗    | ✗       | None          |          |
| `vt100`    | ✗   | ✗    | ✗       | None          |          |
| `tmux`     | ✗   | ✗    | ✓       | Low ¹         |          |
| `sanitize` | ✗   | safe | ✗       | None ²        |          |
| `xterm`    | ✗   | ✓    | ✓       | Medium ³      | YES      |
| `full`     | ✓   | ✓    | ✓       | User-accepted | YES      |

✗ stripped, ✓ preserved

¹ Title-planting (OSC) stripped → CSI 21t query has no
attacker-controlled payload to echo.

² `sanitize` is the auto-detect ceiling. Strips all known
echoback vectors: DECRQSS (DCS), OSC 50/52, CSI 21t/6n.
Preserves safe OSC: Title, Hyperlink, Notify, WorkingDir,
ShellInteg.

³ OSC 50 font queries and CSI 21t pass through. Requires
`--unsafe` to acknowledge the risk.

## Architectural Defenses

| Property                       | Prevents                     |
| ------------------------------ | ---------------------------- |
| `#![forbid(unsafe_code)]`      | Memory corruption            |
| 1-byte parser, zero heap       | Memory exhaustion DoS        |
| CAN/SUB abort (§5.6)           | Unbounded sequence body      |
| StEsc loop ≤ 2 iterations      | Infinite re-entry loops      |
| `memchr` SIMD + byte FSM       | CPU exhaustion               |
| `Cow::Borrowed` fast paths     | Allocation amplification     |
| Default → `dumb` for pipes     | Unsafe default on pipes      |
| `detect_preset_untrusted()`    | `FORCE_*` env var abuse      |
| `try_strip_str()` fallible     | Panic in library context     |
| `contains_ansi_c1()` detect    | 8-bit C1 bypass (Latin-1)    |
| Cross-chunk strip verified     | Split-sequence evasion       |
| Auto-detect caps at sanitize   | Echoback via default path    |
| `--unsafe` gate for xterm/full | Accidental echoback exposure |
| `--check-threats` scan mode    | Undetected echoback in CI    |

## Unicode Homograph Defense

`distill-ansi` normalizes Unicode compatibility forms that
enable visual deception in terminal output. Built-in mappings
(~254 chars) are active by default and cover:

- **fullwidth_ascii** — Homograph confusion: `Ａdmin` → `Admin`
- **math_latin_bold** — Filter evasion: `𝐇𝐞𝐥𝐥𝐨` → `Hello`
- **latin_ligatures** — Grep breakage: `ﬁle` → `file`
- **enclosed_circled_letters** — Filter evasion: `Ⓗⓔⓛⓛⓞ` → `Hello`
- **superscript_subscript** — Spoofed notation: `x²` → `x2`

These characters appear identical or near-identical to their
ASCII equivalents but have different codepoints, breaking
`grep`, `diff`, pattern matching, and human visual inspection.

Fullwidth ASCII (U+FF01–FF5E) is the primary vector: an
attacker can craft filenames, URLs, or log messages using
`Ａ` (U+FF21) instead of `A` (U+0041). Math bold and circled
letters serve the same purpose in social media spam and
phishing.

Latin ligatures (ﬀ, ﬁ, ﬂ, ﬃ, ﬄ, ﬅ, ﬆ) are common in
copy-paste from PDFs. `ﬁle` does not match `file` in any
text search tool.

Removing security-tagged builtins is permitted without `--unsafe`
— homoglyphs are a human-factors risk, not a machine-exploitable
attack. The `--unsafe` flag is reserved for ANSI echoback vectors.

```sh
distill-ansi --no-unicode-map @security
```

Additional canonicalization mappings (CJK, Japanese, Korean,
Arabic, Greek) are available via `--unicode-map` for users
who need full Unicode normalization. See
[UNICODE-NORMALIZATION.md](UNICODE-NORMALIZATION.md).

## Known Limitations

**CsiWindow conflates some query and action.** `CSI 21 t` (title
report, dangerous) and `CSI 8;rows;cols t` (resize, benign) share
final byte `t`. The `CsiQuery` sub-kind uses `first_param`
inspection to split these: `first_param=21` → CsiQuery (stripped
by `sanitize`), other → CsiWindow (preserved). Similarly,
`CSI 6n` (cursor position report) is classified as CsiQuery.

**DCS granularity.** `sanitize` strips all DCS. DECRQSS (`DCS $q`)
is specifically detected via `dcs_is_query` for threat reporting,
but benign DCS (sixel, tmux passthrough) are also stripped. A
future preset between `sanitize` and `xterm` could preserve benign
DCS while stripping DECRQSS specifically.

## Design Decisions

**Sanitize-as-ceiling**: auto-detect caps at `sanitize`. Echoback
vectors stripped regardless of terminal capabilities. `--unsafe`
to bypass for pen-testing/terminal dev.

**`--check-threats`**: scan mode for CI pipelines. Detects echoback
vectors (DECRQSS, OSC 50, OSC 52, CSI 21t, CSI 6n) and reports
in structured key=value format to stderr:

```text
[strip-ansi:threat] type=X line=N pos=N offset=N len=N cve=X
```

Exit 77 on detection (fail mode), or strip + report (strip mode).
`--no-threat-report` suppresses stderr while preserving exit codes.

**Graduated presets**: `dumb ⊂ color ⊂ vt100 ⊂ tmux ⊂ sanitize
⊂ xterm ⊂ full`. Each level is a strict superset. Users choose
capability level; the preset gradient IS the security model.

**`--unsafe` gate**: `xterm` and `full` require explicit opt-in.
Hidden from short help. Accepted silently with safe presets (no-op).

## References

- [DGL: ANSI Terminal Security in 2023][dgl] — 10 CVEs, echoback taxonomy
- [HD Moore: Terminal Emulator Security Issues (2003)][hdm]
- [CWE-150: Improper Neutralization of Escape Sequences][cwe150]
- [Trail of Bits: ANSI in MCP (2025)][tob]
- [solid-snail: npm search RCE (2023)][snail]

[dgl]: https://dgl.cx/2023/09/ansi-terminal-security
[hdm]: https://hdm.io/writing/termulation.txt
[cwe150]: https://cwe.mitre.org/data/definitions/150.html
[tob]: https://blog.trailofbits.com/2025/04/29/deceiving-users-with-ansi-terminal-codes-in-mcp/
[snail]: https://blog.solidsnail.com/posts/npm-esc-seq
