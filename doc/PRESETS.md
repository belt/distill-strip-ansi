# Terminal Presets

## Why Presets

ANSI escape sequences span a wide spectrum — from basic color codes
that a 1980s VT100 can render, to hyperlinks and inline images that
only the latest terminal emulators understand. When stripping, the
right question isn't "strip or don't strip" but "strip what the
output can't handle."

Presets answer that question with a single word.

Two naming conventions coexist deliberately. **Standard presets** use
terminal-standard names (`dumb`, `vt100`, `xterm`) for users who
think in terminal capabilities. **Aliases** use use-case names
(`pipe`, `ci`, `pager`) for users who think in terms of what they're
doing. Both resolve to the same filter configuration — the
distinction improves discoverability for users who think in terms of
their use case rather than terminal standards.

## Default Behavior

Without `--preset`, the tool auto-detects what stdout can handle,
capping at `sanitize` to prevent echoback vectors from passing
through by default:

```text
stdout is not a TTY     →  dumb      (strip everything)
TERM=dumb               →  dumb
NO_COLOR is set         →  dumb
no color support        →  dumb
color detected          →  sanitize  (safe CSI + safe OSC + Fe)
```

Auto-detect never selects `xterm` or `full`. Those presets require
`--unsafe` to acknowledge the echoback risk.

To force strip-all behavior regardless of terminal detection, use
`--preset dumb`.

## Preset Reference

### `dumb`

Strip all escape sequences. Nothing is preserved.

Alias: `pipe`

```text
Preserved: (nothing)
Stripped:  everything
```

When to use:
- Writing to files or pipes
- `TERM=dumb` environments
- Log processing where no ANSI is wanted
- Forcing strip-all: `--preset dumb`

### `color`

Preserve SGR (Select Graphic Rendition) only — colors and text
styles like bold, italic, underline, dim, and reverse.

Aliases: `ci`, `pager`

```text
Preserved: CsiSgr
Stripped:  cursor, erase, scroll, mode, OSC, DCS, everything else
```

When to use:
- Piping to `less -R` (which only passes SGR)
- CI environments (GitHub Actions, GitLab CI)
- Color-aware log viewers
- Any context where you want colors but not cursor control

### `vt100`

Preserve SGR, cursor movement, and erase sequences. The classic
terminal capability set.

```text
Preserved: CsiSgr, CsiCursor, CsiErase
Stripped:  scroll, mode, OSC, DCS, everything else
```

When to use:
- Serial consoles
- Basic remote terminals
- Simple TUI applications
- Environments with minimal terminal emulation

### `tmux`

Preserve all CSI sequences and Fe escapes, but strip OSC and
string-type sequences. Matches what terminal multiplexers typically
pass through.

Alias: `screen`

```text
Preserved: all CSI (SGR, cursor, erase, scroll, mode, window,
           device status, other), Fe
Stripped:  OSC, DCS, APC, PM, SOS, SS2, SS3
```

When to use:
- Output destined for tmux or GNU screen
- Environments that handle CSI but filter OSC
- When you need full CSI but not window titles or hyperlinks

### `sanitize`

Preserve safe CSI, safe OSC, and Fe sequences while stripping all
known echoback vectors. This is the auto-detect ceiling — the
highest preset selected without `--unsafe`.

Alias: `safe`

```text
Preserved: CsiSgr, CsiCursor, CsiErase, CsiScroll, CsiMode,
           CsiWindow, CsiOther, Fe,
           OscTitle, OscHyperlink, OscNotify, OscWorkingDir,
           OscShellInteg
Stripped:  CsiQuery, CsiDeviceStatus, OscClipboard, OscOther,
           DCS, APC, PM, SOS, SS2, SS3
```

Security properties — every known echoback vector is stripped:

| Vector   | Sequence    | Stripped as  |
| -------- | ----------- | ------------ |
| DECRQSS  | DCS `$ q`   | DCS (all)    |
| OSC 50   | font query  | OscOther     |
| OSC 52   | clipboard   | OscClipboard |
| CSI 21t  | title query | CsiQuery     |
| CSI 6n   | cursor pos  | CsiQuery     |

When to use:
- Processing untrusted input (build logs, CI output)
- Default for piped terminal output
- Any context where security matters more than full OSC pass-through

### `xterm`

Preserve all CSI, OSC, and Fe sequences. The standard modern
terminal capability set — covers colors, cursor control, alternate
screen, mouse tracking, window titles, and hyperlinks.

**Requires `--unsafe`** — preserves dangerous sequences including
OSC 50 (font query echoback) and CSI 21t (title report echoback).

```text
Preserved: all CSI, OSC, Fe
Stripped:  DCS, APC, PM, SOS, SS2, SS3
```

When to use:
- Modern terminal emulators when you accept the echoback risk
- Pen-testing terminal escape handling
- Terminal development and debugging

### `full`

Preserve all escape sequences. Nothing is stripped.

**Requires `--unsafe`** — preserves all sequences including DCS
(DECRQSS echoback vector).

Alias: `modern`

```text
Preserved: everything
Stripped:  (nothing)
```

When to use:
- Fully capable terminals when you accept all risks
- Terminal escape sequence development
- Pass-through scenarios with trusted input only

## Combining Presets with Flags

`--preset` sets the base configuration. `--no-strip-*` flags are
applied on top, additively. This lets you start from a preset and
selectively preserve additional sequence types:

```bash
# Start from color preset, also preserve cursor movement
strip-ansi --preset color --no-strip-csi-cursor

# Start from vt100, also preserve OSC (window titles)
strip-ansi --preset vt100 --no-strip-osc
```

## Preset Gradient

The presets form a strict gradient — each level preserves everything
the previous level does, plus more:

```text
dumb ⊂ color ⊂ vt100 ⊂ tmux ⊂ sanitize ⊂ xterm ⊂ full
                                   ↑           ↑
                            auto-detect     --unsafe
                              ceiling       required
```

| Preset   | SGR | Curs | Erase | Scroll | Mode | Win | Fe | OSC¹ | DCS+ | --unsafe |
| -------- | --- | ---- | ----- | ------ | ---- | --- | -- | ---- | ---- | -------- |
| dumb     |     |      |       |        |      |     |    |      |      |          |
| color    | ✓   |      |       |        |      |     |    |      |      |          |
| vt100    | ✓   | ✓    | ✓     |        |      |     |    |      |      |          |
| tmux     | ✓   | ✓    | ✓     | ✓      | ✓    | ✓   | ✓  |      |      |          |
| sanitize | ✓   | ✓    | ✓     | ✓      | ✓    | ✓   | ✓  | safe |      |          |
| xterm    | ✓   | ✓    | ✓     | ✓      | ✓    | ✓   | ✓  | ✓    |      | YES      |
| full     | ✓   | ✓    | ✓     | ✓      | ✓    | ✓   | ✓  | ✓    | ✓    | YES      |

¹ `sanitize` preserves Title, Hyperlink, Notify, WorkingDir,
ShellInteg. Strips Clipboard, Other (including OSC 50).
