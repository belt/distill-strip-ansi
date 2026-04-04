# ANSI Escape Sequence Reference

Dense reference for contributors. Covers the terminal control
vocabulary this crate parses: byte ranges, sequence structure,
and real-world usage. External links carry the full spec weight.

## Standards Lineage

| Standard              | Scope                      | Link          |
| --------------------- | -------------------------- | ------------- |
| ECMA-48 (5th ed 1991) | C0/C1, CSI/OSC/DCS, FSM   | [PDF][ecma48] |
| ANSI X3.64 (withdrawn) | US adoption of ECMA-48    | superseded    |
| ISO/IEC 6429          | Intl equiv of ECMA-48      | same content  |
| XTerm ctlseqs         | De-facto: OSC, DCS, modes  | [doc][xterm]  |

ECMA-48 is the canonical source. XTerm ctlseqs documents the
extensions that real terminals actually implement.

[ecma48]: https://ecma-international.org/publications-and-standards/standards/ecma-48/
[xterm]: https://invisible-island.net/xterm/ctlseqs/ctlseqs.html

## Byte Taxonomy

### C0 Control Codes (0x00–0x1F)

Single-byte controls. Most are irrelevant to escape parsing
except these:

| Byte   | Name | Role in parsing                     |
| ------ | ---- | ----------------------------------- |
| `0x07` | BEL  | Terminates OSC strings (legacy)     |
| `0x18` | CAN  | Aborts in-progress sequence (§5.6)  |
| `0x1A` | SUB  | Aborts in-progress sequence (§5.6)  |
| `0x1B` | ESC  | Introduces all multi-byte sequences |

CAN/SUB abort is critical for robustness on malformed streams.
Without it, a broken sequence consumes all subsequent bytes as
"sequence body" until an accidental terminator appears.

### C1 Control Codes (0x80–0x9F)

8-bit equivalents of `ESC + 0x40..0x5F`. Rarely used in modern
UTF-8 terminals because they collide with valid UTF-8 lead bytes.
This crate handles the 7-bit (`ESC`-prefixed) forms only, which
is standard practice.

| 8-bit  | 7-bit equiv | Name |
| ------ | ----------- | ---- |
| `0x9B` | `ESC [`     | CSI  |
| `0x9D` | `ESC ]`     | OSC  |
| `0x90` | `ESC P`     | DCS  |
| `0x9E` | `ESC ^`     | PM   |
| `0x98` | `ESC X`     | SOS  |
| `0x9F` | `ESC _`     | APC  |

Reference: [ECMA-48 §5.3][ecma48], [Wikipedia C0/C1][c0c1]

[c0c1]: https://en.wikipedia.org/wiki/C0_and_C1_control_codes

## Escape Sequence Types

Every sequence begins with `ESC` (0x1B). The next byte determines
the sequence type. This section maps each type to its ECMA-48
definition and this crate's parser state.

### Fe — Escape Sequences (ESC + 0x40–0x5F)

Two-byte sequences: `ESC` followed by a single byte in the Fe
range. Most are introducers for longer sequences (CSI, OSC, DCS),
but some are standalone.

```text
ESC + byte
      │
      ├─ 0x40..0x5F  →  Fe (standalone or introducer)
      ├─ 0x20..0x2F  →  intermediate byte(s), then final
      └─ 0x30..0x7E  →  Fp/Fs private/standard (2-byte)
```

Standalone Fe examples:

| Sequence       | Name  | Purpose                 |
| -------------- | ----- | ----------------------- |
| `ESC 7` (0x37) | DECSC | Save cursor position    |
| `ESC 8` (0x38) | DECRC | Restore cursor position |
| `ESC c`        | RIS   | Full terminal reset     |
| `ESC D`        | IND   | Index (move down)       |
| `ESC M`        | RI    | Reverse index (move up) |
| `ESC E`        | NEL   | Next line               |

Parser state: `EscapeStart` → byte in 0x30..0x7E → `Ground`.
Classifier: `SeqKind::Fe`, `SeqGroup::Fe`.

### ESC + Intermediate Bytes

Multi-byte escapes with one or more intermediate bytes (0x20–0x2F)
before the final byte (0x30–0x7E). Used for character set
designation.

```text
ESC ( B     ← designate ASCII to G0
│   │ │
│   │ └─ final byte (0x30..0x7E)
│   └─── intermediate byte (0x20..0x2F)
└─────── ESC
```

Parser state: `EscapeStart` → `EscIntermediate` → `Ground`.

### CSI — Control Sequence Introducer (ESC [)

The workhorse of terminal control. Structure:

```text
ESC [ <params> <intermediates> <final>
      │         │               │
      │         │               └─ 0x40..0x7E (determines function)
      │         └─────────────── 0x20..0x2F (rare, modifiers)
      └──────────────────────── 0x30..0x3F (digits, semicolons, ?)
```

Parameter bytes encode numeric arguments separated by `;`.
The `?` prefix (0x3F) marks private-mode sequences.

Parser states: `EscapeStart` → `CsiParam` → optional
`CsiIntermediate` → final byte → `Ground`.

#### CSI Sub-Kinds (classified by final byte)

This crate classifies CSI sequences into 8 sub-kinds for
selective filtering:

| Sub-kind     | Final byte(s) | Function        | Example         |
| ------------ | ------------- | --------------- | --------------- |
| SGR          | `m`           | Graphic rendit. | `ESC[31m`       |
| Cursor       | `A`-`H`, `f`  | Cursor movement | `ESC[5A`        |
| Erase        | `J`, `K`      | Erase disp/ln   | `ESC[2J`        |
| Scroll       | `S`, `T`      | Scroll up/down  | `ESC[3S`        |
| Mode         | `h`, `l`      | Set/reset mode  | `ESC[?25h`      |
| DeviceStatus | `n`, `c`      | Device status   | `ESC[6n`        |
| Window       | `t`           | Window manip    | `ESC[8;40;132t` |
| Other        | all else      | Catch-all       | `ESC[4;1r`      |

SGR is by far the most common in CI/CD output — it's what
`--color=always` flags produce. Filtering SGR alone strips
colors while preserving cursor control.

Reference: [ECMA-48 §5.4][ecma48], [XTerm CSI][xterm-csi]

[xterm-csi]: https://invisible-island.net/xterm/ctlseqs/ctlseqs.html#h3-Functions-using-CSI-_-ordered-by-the-final-character_s_

#### SGR Parameter Encoding

SGR (`ESC[<params>m`) is worth understanding in detail because
it dominates real-world log output.

```text
ESC [ 38;5;196 m        ← 256-color red foreground
      ├──┤ ├─┤
      │    │  └── color index (0–255)
      │    └───── 5 = 256-color mode
      └────────── 38 = set foreground

ESC [ 38;2;255;0;0 m    ← 24-bit RGB red foreground
      ├──┤ ├─┤
      │    │  └── R;G;B values
      │    └───── 2 = RGB mode
      └────────── 38 = set foreground
```

Common SGR parameters:

| Code        | Effect                 | Reset |
| ----------- | ---------------------- | ----- |
| 0           | Reset all attributes   | —     |
| 1           | Bold / bright          | 22    |
| 2           | Dim / faint            | 22    |
| 3           | Italic                 | 23    |
| 4           | Underline              | 24    |
| 7           | Inverse / reverse      | 27    |
| 30–37       | Standard fg colors     | 39    |
| 40–47       | Standard bg colors     | 49    |
| 38;5;N      | 256-color foreground   | 39    |
| 48;5;N      | 256-color background   | 49    |
| 38;2;R;G;B  | 24-bit foreground      | 39    |
| 48;2;R;G;B  | 24-bit background      | 49    |
| 90–97       | Bright fg colors       | 39    |
| 100–107     | Bright bg colors       | 49    |

Reference: [ECMA-48 §8.3.117][ecma48], [XTerm SGR][xterm-sgr]

[xterm-sgr]: https://invisible-island.net/xterm/ctlseqs/ctlseqs.html#h3-Functions-using-CSI-_-ordered-by-the-final-character_s_

### OSC — Operating System Command (ESC ])

Variable-length string sequences for terminal metadata.
Terminated by BEL (0x07) or ST (`ESC \`).

```text
ESC ] <number> ; <payload> BEL
                           ─or─
ESC ] <number> ; <payload> ESC \
                           └─┘ ST (String Terminator)
```

| OSC # | Purpose                        | Example               |
| ----- | ------------------------------ | --------------------- |
| 0     | Set icon name + window title   | `ESC]0;My Title BEL`  |
| 1     | Set icon name                  |                       |
| 2     | Set window title               | `ESC]2;Build Log BEL` |
| 7     | Set working directory (iTerm2) |                       |
| 8     | Hyperlinks                     | see example below     |
| 9     | Desktop notification (iTerm2)  |                       |
| 52    | Clipboard access               |                       |
| 133   | Shell integration / prompts    |                       |
| 1337  | iTerm2 proprietary             |                       |

OSC 8 hyperlinks are increasingly common in modern CLI tools
(cargo, rustc, gcc). They wrap visible text with clickable URLs.

Parser states: `EscapeStart` → `OscString` → (`OscStEsc` if
`ESC` seen) → `Ground`.
Classifier: `SeqKind::Osc`, `SeqGroup::Osc`.

Reference: [XTerm OSC][xterm-osc]

[xterm-osc]: https://invisible-island.net/xterm/ctlseqs/ctlseqs.html#h3-Operating-System-Commands

### DCS — Device Control String (ESC P)

Variable-length sequences for device-specific data. Same
termination as OSC (ST = `ESC \`).

```text
ESC P <params> <final> <data...> ESC \
      │         │       │         └─┘ ST
      │         │       └─ passthrough data
      │         └─ 0x40..0x7E (function)
      └─ 0x30..0x3F (parameter bytes)
```

Real-world DCS usage:

| Sequence         | Purpose                     |
| ---------------- | --------------------------- |
| `ESC P + q ...`  | Request termcap/terminfo    |
| `ESC P $ q ...`  | Request status (DECRQSS)    |
| `ESC P q ...`    | Sixel graphics data         |
| `ESC P tmux;...` | tmux passthrough            |

Parser states: `EscapeStart` → `DcsEntry` → `DcsParam` →
`DcsPassthrough` → (`DcsStEsc` if `ESC` seen) → `Ground`.
Classifier: `SeqKind::Dcs`, `SeqGroup::Dcs`.

Reference: [XTerm DCS][xterm-dcs]

[xterm-dcs]: https://invisible-island.net/xterm/ctlseqs/ctlseqs.html#h3-Device-Control-functions

### APC, PM, SOS — String Sequences

Three string-type sequences with identical structure: introducer,
arbitrary payload, terminated by ST. Rarely seen in practice.

| Type | Introducer | ECMA-48 purpose           |
| ---- | ---------- | ------------------------- |
| APC  | `ESC _`    | Application Program Cmd   |
| PM   | `ESC ^`    | Privacy Message           |
| SOS  | `ESC X`    | Start of String           |

This crate collapses all three into a single `StringPassthrough`
parser state since their byte-level behavior is identical: consume
everything until ST or CAN/SUB abort.

Classifier: `SeqKind::Apc`/`Pm`/`Sos`, `SeqGroup::Apc`/`Pm`/`Sos`
(distinct kinds despite shared parser state).

### SS2, SS3 — Single Shifts (ESC N, ESC O)

Two-byte sequences: `ESC N` or `ESC O` followed by exactly one
character byte. Invoke the G2 or G3 character set for that single
character. Rare in modern terminals but part of ECMA-48.

```text
ESC N <char>    ← SS2: one char from G2
ESC O <char>    ← SS3: one char from G3
```

Parser states: `EscapeStart` → `Ss2`/`Ss3` → consume one byte →
`Ground`.
Classifier: `SeqKind::Ss2`/`Ss3`, `SeqGroup::Ss2`/`Ss3`.

## Sequence Termination

### String Terminator (ST)

`ESC \` (0x1B 0x5C) — terminates OSC, DCS, APC, PM, SOS.

The parser handles ST via "StEsc re-entry": when `ESC` appears
inside a string state, the parser transitions to an `*StEsc`
state. If the next byte is `\`, the sequence ends. Otherwise,
the byte is re-interpreted as a new escape introducer (the loop
runs at most 2 iterations, no recursion).

### BEL Terminator

`0x07` — legacy OSC terminator. Widely supported. Simpler than
ST (single byte). This crate accepts BEL as an OSC terminator.

### CAN/SUB Abort (ECMA-48 §5.6)

`0x18` (CAN) and `0x1A` (SUB) immediately abort any in-progress
escape sequence, returning the parser to ground state. This
prevents malformed sequences from consuming unbounded input.

Without CAN/SUB handling, a truncated `ESC [` with no final byte
would swallow all subsequent output as "CSI parameter bytes"
until a byte in 0x40–0x7E accidentally terminates it.

## State Machine Overview

```text
                ┌────────────────────────┐
                │            Ground      │
                │ (content bytes → Emit) │
                └───────────────┬────────┘
                                │ ESC (0x1B)
                                ▼
                 ┌──────────────────────┐
                 │      EscapeStart     │
                 │ (classify next byte) │
                 └─┬──┬──┬──┬──┬──┬─────┘
                   │  │  │  │  │  │
        ┌──────────┘  │  │  │  │  └──────────┐
        ▼             │  │  │  │             ▼
 ┌────────────┐       │  │  │  │    ┌─────────────────┐
 │  CsiParam  │       │  │  │  │    │ EscIntermediate │
 │   → CsiInt │       │  │  │  │    │    → Ground     │
 │   → Ground │       │  │  │  │    └─────────────────┘
 └────────────┘       │  │  │  │
        ┌─────────────┘  │  │  └─────────┐
        ▼                │  │            ▼
 ┌────────────┐          │  │    ┌────────────────┐
 │  OscString │          │  │    │ StringPassthru │
 │   → OscSt  │          │  │    │ (APC/PM/SOS)   │
 │   → Ground │          │  │    │  → StringStEsc │
 └────────────┘          │  │    │  → Ground      │
        ┌────────────────┘  │    └────────────────┘
        ▼                   ▼
 ┌────────────┐      ┌───────────┐
 │  DcsEntry  │      │  Ss2/Ss3  │
 │   → Param  │      │  +1 byte  │
 │   → Pass   │      │  → Ground │
 │   → DcsSt  │      └───────────┘
 │   → Ground │
 └────────────┘
```

15 states, all transitions driven by byte ranges. No heap, no
recursion, no backtracking. The StEsc states (OscStEsc, DcsStEsc,
StringStEsc) handle the `ESC` ambiguity inside string sequences
with a bounded re-entry loop (max 2 iterations).

## Real-World Patterns in CI/CD Output

What you actually encounter when processing build logs:

### Colored compiler output (`--color=always`)

```text
\x1b[0m\x1b[1m\x1b[38;5;9merror\x1b[0m\x1b[1m: expected `;`\x1b[0m
│      │      │              │      │                       │
│      │      │              │      │                       └─ SGR reset
│      │      │              │      └─ SGR bold
│      │      │              └─ SGR reset
│      │      └─ SGR 256-color red fg
│      └─ SGR bold
└─ SGR reset
```

Typical: 5–10 SGR sequences per diagnostic line. All CSI with
final byte `m`.

### Progress indicators (Docker, cargo)

```text
\x1b[2K\x1b[1G  Compiling foo v0.1.0
│       │
│       └─ CSI cursor to column 1 (CsiCursor, final 'G')
└─ CSI erase entire line (CsiErase, final 'K')
```

Build tools overwrite lines using erase + cursor repositioning.

### Hyperlinks (modern rustc, cargo)

```text
\x1b]8;;https://doc.rust-lang.org/E0308\x07E0308\x1b]8;;\x07
│                                       │      │           │
│                                       │      │           └─ OSC close
│                                       │      └─ visible text
│                                       └─ BEL terminator
└─ OSC 8 hyperlink open
```

### Window title (terminal multiplexers)

```text
\x1b]0;user@host:~/project\x07
│                          │
│                          └─ BEL terminator
└─ OSC 0: set window title
```

### tmux passthrough

```text
\x1bPtmux;\x1b\x1b]8;;url\x07text\x1b\x1b]8;;\x07\x1b\\
│                                                       │
│                                                       └─ ST (DCS end)
└─ DCS with tmux prefix
```

## Mapping to Crate API

| Concept      | Crate type                        | Size    |
| ------------ | --------------------------------- | ------- |
| Byte class   | `Parser` → `Action::Emit`/`Skip` | 1 byte  |
| Sequence ID  | `ClassifyingParser` → `SeqAction` | 3 bytes |
| Group taxon  | `SeqGroup` (9 variants)           | 1 byte  |
| Kind taxon   | `SeqKind` (17 variants, 8 CSI)    | 1 byte  |
| Strip policy | `FilterConfig` (bitfield+SmallVec) | —      |
| Stream strip | `StripStream` (1 byte state)      | 1 byte  |
| Stream filter | `FilterStream` (ClassifyingParser) | 4 bytes |

## Further Reading

| Resource                    | Covers                       |
| --------------------------- | ---------------------------- |
| [ECMA-48 PDF][ecma48]       | Full standard, code tables   |
| [XTerm ctlseqs][xterm]      | Sequences supported by xterm |
| [vt100.net][vt100]          | DEC terminal manuals         |
| [Wikipedia ANSI][wiki]      | Accessible overview          |
| [Williams' parser][pwilliams] | State diagram (vte basis)  |
| [console_codes(4)][console4] | Linux console escape docs   |

[vt100]: https://vt100.net/
[wiki]: https://en.wikipedia.org/wiki/ANSI_escape_code
[pwilliams]: https://vt100.net/emu/dec_ansi_parser
[console4]: https://man7.org/linux/man-pages/man4/console_codes.4.html
