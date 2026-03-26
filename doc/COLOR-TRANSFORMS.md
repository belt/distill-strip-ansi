# Color Transforms

Color depth reduction and palette remapping for terminal output.
Two independent features sharing a common SGR rewriting core.

Related docs:
[ANSI-REFERENCE.md](ANSI-REFERENCE.md) (SGR encoding),
[DESIGN.md](DESIGN.md) (parser/filter architecture),
[PRESETS.md](PRESETS.md) (capability gradient).

## Features

Two orthogonal Cargo feature flags, two orthogonal CLI flags.
Neither implies the other. Both imply `transform` (which implies
`filter`). Transform features live in the `distill-ansi` binary;
`strip-ansi` remains focused on stripping.

| Feature           | Flag            | Binary         | Purpose              |
| ----------------- | --------------- | -------------- | -------------------- |
| `downgrade-color` | `--color-depth` | `distill-ansi` | Reduce color depth   |
| `color-palette`   | `--palette`     | `distill-ansi` | Remap named palette  |

Combined: `--color-depth 256 --palette high-contrast-rg`
applies palette first, then depth reduction.

## Color Depth Reduction

`--color-depth {truecolor,256,16,greyscale,mono}`

### Downgrade Paths

```text
Truecolor → 256        nearest in 6x6x6 cube or greyscale ramp
Truecolor → 16         nearest basic ANSI color by RGB distance
      256 → 16         lookup table (256 entries)
      Any → greyscale  luminance weighting to 24-shade ramp
      Any → mono       strip color params, keep style params
```

### SGR Param Encoding

See [ANSI-REFERENCE.md](ANSI-REFERENCE.md) for full SGR encoding.
The rewriter operates on these color subsequences within SGR:

```text
38;2;R;G;B    foreground truecolor
48;2;R;G;B    background truecolor
38;5;N        foreground 256-color
48;5;N        background 256-color
30-37         foreground basic (standard)
40-47         background basic (standard)
90-97         foreground basic (bright)
100-107       background basic (bright)
```

Non-color params (bold, italic, underline, dim, reverse, etc.)
pass through unchanged. The rewriter parses the full param list,
replaces only color subsequences, and reassembles.

### 256-Color Space

The xterm 256-color palette has three regions:

```text
  0-7      standard colors (= basic ANSI 30-37)
  8-15     bright colors   (= basic ANSI 90-97)
 16-231    6x6x6 RGB cube
232-255    24-shade greyscale ramp
```

#### 6x6x6 Cube (indices 16-231)

Index = 16 + 36r + 6g + b, where r,g,b in 0..5.

Channel values for each axis position:

| Axis    | 0    | 1    | 2    | 3    | 4    | 5    |
| ------- | ---- | ---- | ---- | ---- | ---- | ---- |
| Value   | 0x00 | 0x5F | 0x87 | 0xAF | 0xD7 | 0xFF |
| Decimal |    0 |   95 |  135 |  175 |  215 |  255 |

Step sizes: 0 to 95 (95), then 40, 40, 40, 40.

#### Greyscale Ramp (indices 232-255)

24 shades from dark to light, excluding pure black and white:

```text
232: #080808    238: #444444    244: #808080    250: #bcbcbc
233: #121212    239: #4e4e4e    245: #8a8a8a    251: #c6c6c6
234: #1c1c1c    240: #585858    246: #949494    252: #d0d0d0
235: #262626    241: #626262    247: #9e9e9e    253: #dadada
236: #303030    242: #6c6c6c    248: #a8a8a8    254: #e4e4e4
237: #3a3a3a    243: #767676    249: #b2b2b2    255: #eeeeee
```

Formula: value = 8 + 10 * (index - 232).

### Truecolor to 256 Algorithm

```text
1. Compute cube candidate:
   r_idx = nearest_axis(R)    // quantize to 0..5
   g_idx = nearest_axis(G)
   b_idx = nearest_axis(B)
   cube_index = 16 + 36*r_idx + 6*g_idx + b_idx
   cube_rgb = (AXIS[r_idx], AXIS[g_idx], AXIS[b_idx])

2. Compute greyscale candidate:
   lum = round((R + G + B) / 3)    // simple average for ramp
   grey_idx = clamp((lum - 8) / 10 + 232, 232, 255)
   grey_val = 8 + 10 * (grey_idx - 232)

3. Pick closer by squared Euclidean distance:
   d_cube = (R-cube_r)^2 + (G-cube_g)^2 + (B-cube_b)^2
   d_grey = (R-grey_val)^2 + (G-grey_val)^2 + (B-grey_val)^2
   result = if d_grey < d_cube { grey_idx } else { cube_index }
```

The `nearest_axis` function maps a 0-255 value to the nearest
of the 6 cube axis values. Boundary thresholds:

```text
  0-47   → 0 (0x00)
 48-114  → 1 (0x5F)
115-154  → 2 (0x87)
155-194  → 3 (0xAF)
195-234  → 4 (0xD7)
235-255  → 5 (0xFF)
```

### 256 to 16 Algorithm

Indices 0-15 map to themselves (identity).

Indices 16-231 (cube): convert cube index back to RGB, then find
nearest basic color by squared Euclidean distance against the 16
standard ANSI RGB values.

Indices 232-255 (greyscale): convert to grey value, then find
nearest among black (0), white (7), bright black (8), bright
white (15) by distance.

### Standard ANSI 16-Color RGB Values

Terminal-dependent, but the xterm defaults are the reference.
The downgrader uses these for distance calculations.

Standard (SGR 30-37 fg, 40-47 bg):

```text
 0  Black       (  0,   0,   0)
 1  Red         (128,   0,   0)
 2  Green       (  0, 128,   0)
 3  Yellow      (128, 128,   0)
 4  Blue        (  0,   0, 128)
 5  Magenta     (128,   0, 128)
 6  Cyan        (  0, 128, 128)
 7  White       (192, 192, 192)
```

Bright (SGR 90-97 fg, 100-107 bg):

```text
 8  Br Black    (128, 128, 128)
 9  Br Red      (255,   0,   0)
10  Br Green    (  0, 255,   0)
11  Br Yellow   (255, 255,   0)
12  Br Blue     (  0,   0, 255)
13  Br Magenta  (255,   0, 255)
14  Br Cyan     (  0, 255, 255)
15  Br White    (255, 255, 255)
```

### Greyscale Conversion

Luminance-weighted conversion using Rec. 709 coefficients
(matches sRGB primaries):

```text
Y = 0.2126 * R + 0.7152 * G + 0.0722 * B
```

Map Y to the nearest greyscale ramp index (232-255):

```text
grey_idx = clamp(round((Y - 8) / 10) + 232, 232, 255)
```

For basic-16 greyscale: map Y to nearest of black (0),
bright black (8), white (7), bright white (15).

### Monochrome Mode

Strip all color-setting SGR params, keep everything else:

```text
Strip:    30-37, 38, 39, 40-47, 48, 49, 90-97, 100-107
Keep:     0 (reset), 1-29, 50-89, 98-99
```

When `38` or `48` is encountered, also consume the following
color specification params (`5;N` or `2;R;G;B`).

## Color Palette Remapping

`--palette NAME`

Independent of `--color-depth`. Remaps colors through a named
palette to optimize distinguishability for different types of
color perception.

### Design Constraint

Palette names are neutral and functional. They do not reference
medical conditions. A user selects a palette because it works
for them, not because they must self-identify. Documentation
explains what each palette optimizes, but the CLI flag is just
a name.

### Transform Pipeline

When both `--palette` and `--color-depth` are specified:

```text
Input SGR → extract color → palette transform → depth reduce → emit
```

Palette first, depth second. The palette optimizes colors in the
source color space, then depth reduction maps to the target space.

### Color Space for Transforms

All palette transforms operate in linear RGB, not sRGB. Terminal
color values (0-255 per channel) are sRGB-encoded. The pipeline:

```text
1. sRGB decode: linearize each channel
2. Apply 3x3 palette transform matrix
3. Clamp to [0, 1]
4. sRGB encode: gamma-compress back to 0-255
5. (Optional) depth reduce to target
```

#### sRGB Linearization (IEC 61966-2-1)

```text
Decode (sRGB → linear):
  if C_srgb <= 0.04045:
    C_linear = C_srgb / 12.92
  else:
    C_linear = ((C_srgb + 0.055) / 1.055) ^ 2.4

Encode (linear → sRGB):
  if C_linear <= 0.0031308:
    C_srgb = C_linear * 12.92
  else:
    C_srgb = 1.055 * C_linear ^ (1/2.4) - 0.055
```

Where C_srgb is in [0, 1] (divide 0-255 by 255 first).

For performance in a terminal context, a 256-entry lookup table
for decode and a 4096-entry table for encode are sufficient.
Alternatively, the fast approximation `C ^ 2.2` / `C ^ (1/2.2)`
is adequate for terminal colors where perceptual precision is
limited by the display.

### Palette Categories

#### Universal Palettes

Optimized for maximum distinguishability across all vision types.
Based on Color Universal Design (CUD) principles.

Reference palette: Okabe-Ito (Okabe and Ito, 2002). Eight colors
designed to be unambiguous for all forms of color vision:

```text
Black           #000000   (  0,   0,   0)
Orange          #E69F00   (230, 159,   0)
Sky Blue        #56B4E9   ( 86, 180, 233)
Bluish Green    #009E73   (  0, 158, 115)
Yellow          #F0E442   (240, 228,  66)
Blue            #0072B2   (  0, 114, 178)
Vermillion      #D55E00   (213,  94,   0)
Reddish Purple  #CC79A7   (204, 121, 167)
```

Source: [Color Universal Design](https://jfly.uni-koeln.de/color/),
Okabe and Ito (2002).

#### Tol Qualitative Schemes

Paul Tol (SRON) designed multiple colorblind-safe schemes with
different trade-offs:

Bright (7 colors, default qualitative):

```text
#4477AA  #EE6677  #228833  #CCBB44  #66CCEE  #AA3377  #BBBBBB
```

High-contrast (3 colors, also works in greyscale):

```text
#004488  #DDAA33  #BB5566
```

Vibrant (7 colors, alternative qualitative):

```text
#EE7733  #0077BB  #33BBEE  #EE3377  #CC3311  #009988  #BBBBBB
```

Source: [Paul Tol's Notes](https://personal.sron.nl/~pault/),
technical note issue 3.2 (2021-08-18).

#### Axis-Optimized Palettes

Palettes that maximize contrast along specific color confusion
axes. Named by the axis they optimize, not by condition.

```text
high-contrast-rg   red-green distinction    L/M cone overlap
high-contrast-by   blue-yellow distinction  S cone isolation
high-contrast-rb   red-blue distinction     L/S separation
```

These use remapping strategies that shift confusable colors away
from the confusion axis into distinguishable regions.

#### Extended-Gamut Palette

For observers with enhanced color discrimination (tetrachromacy).
Maximizes information density by using more of the available
color space rather than clustering around "safe" center values.

Strategy: when downgrading, prefer colors that are maximally
spread across the full gamut. Avoid collapsing subtle hue
differences that standard palettes treat as equivalent.

### CVD Simulation Matrices

The palette transforms build on established color vision
deficiency (CVD) simulation research. These matrices model how
colors appear under different types of color vision, which
informs the design of optimized palettes.

The simulation pipeline for all methods:

```text
sRGB input → linearize → [3x3 matrix] → clamp → sRGB encode
```

#### Vienot et al. (1999)

Single 3x3 matrix in linear RGB. Covers protanopia and
deuteranopia (red-green). Simple and fast.

Protanopia (linear RGB):

```text
[ 0.10889  0.89111 -0.00000 ]
[ 0.10889  0.89111  0.00000 ]
[ 0.00447 -0.00447  1.00000 ]
```

Deuteranopia (linear RGB):

```text
[ 0.29275  0.70725  0.00000 ]
[ 0.29275  0.70725 -0.00000 ]
[-0.02234  0.02234  1.00000 ]
```

Source: Vienot, Brettel, and Mollon (1999). "Digital video
colourmaps for checking the legibility of displays by
dichromats." Color Research and Application, 24(4), 243-252.

Matrices from [DaltonLens](https://daltonlens.org/cvd-simulation-svg-filters/).

#### Brettel et al. (1997)

Two half-plane projection matrices per deficiency type, selected
by dot product with a separation plane normal. Required for
accurate tritanopia simulation (Vienot 1999 does not cover it).

```text
normal_rgb = precomputed separation plane normal in RGB
if dot(normal_rgb, rgb_linear) >= 0:
    rgb_out = matrix_H1 * rgb_linear
else:
    rgb_out = matrix_H2 * rgb_linear
```

For protanopia and deuteranopia, the two half-plane matrices
produce results very close to the single Vienot matrix. The
half-plane approach matters primarily for tritanopia.

Source: Brettel, Vienot, and Mollon (1997). "Computerized
simulation of color appearance for dichromats." Journal of the
Optical Society of America A, 14(10), 2647-2655.

#### Machado et al. (2009)

Parameterized by severity (0.0 = normal, 1.0 = full dichromacy).
Produces a 3x3 matrix for any severity level. Based on a
physiological model with opponent-color stage.

Useful for anomalous trichromacy (partial deficiency) where the
observer has shifted but not absent cone response.

Pre-computed matrices at severity 1.0 are equivalent to
Brettel/Vienot for full dichromacy. Intermediate severities
enable finer-grained palette optimization.

Source: Machado, Oliveira, and Fernandes (2009). "A
physiologically-based model for simulation of color vision
deficiency." IEEE Transactions on Visualization and Computer
Graphics, 15(6), 1291-1298.

### From Simulation to Optimization

Simulation matrices model what a dichromat sees. Palette
optimization is the inverse problem: given what a dichromat
sees, choose source colors that maximize perceived contrast.

The daltonization approach:

```text
1. Simulate: rgb_sim = CVD_matrix * rgb_linear
2. Compute error: err = rgb_linear - rgb_sim
3. Redistribute error into visible channels:
   rgb_corrected = rgb_linear + redistribute(err)
```

The redistribution step shifts lost information from the
confused channel into channels the observer can distinguish.
For red-green deficiency, this typically means shifting red-green
contrast into blue-yellow and luminance contrast.

For terminal palettes specifically, the approach is simpler:
pre-compute an optimized 16-color or 256-color palette where
every pair of colors is distinguishable under the target CVD
simulation. This is a static lookup table, not a per-pixel
transform.

### Tetrachromacy Considerations

Tetrachromacy (4 cone types) occurs in an estimated 12% of women
(carriers of anomalous trichromacy genes), though functional
tetrachromacy (actually using the 4th channel for discrimination)
is rare.

RGB displays cannot address a 4th cone independently. However,
palette design can account for tetrachromatic perception:

- Standard palettes collapse metameric pairs (colors that look
  identical to trichromats). Tetrachromats may distinguish these.
- Extended-gamut palettes avoid this collapse by preferring
  spectrally distinct colors over metameric equivalents.
- In practice: use more saturated, spectrally pure colors and
  avoid desaturated pastels that rely on metameric mixing.

Research: Jordan and Mollon (2019). "Tetrachromacy: the
mysterious case of extra-ordinary color vision." Current Opinion
in Behavioral Sciences, 30, 130-134. Jordan et al. (2010). "The
dimensionality of color vision in carriers of anomalous
trichromacy." Journal of Vision, 10(8), 12.

### Greyscale Luminance Weighting

For palette transforms that target greyscale output, the
luminance coefficients depend on the color space:

```text
Rec. 709:  0.2126 R + 0.7152 G + 0.0722 B   (sRGB, default)
Rec. 601:  0.299  R + 0.587  G + 0.114  B   (legacy NTSC/PAL)
```

Rec. 709 is correct for modern terminals (sRGB assumption).
Apply to linear RGB values, not sRGB-encoded values, for
perceptually accurate luminance.

## Architecture Integration

### Two-Binary Model

Transform features live in a separate binary (`distill-ansi`)
that shares the `strip_ansi` library crate with `strip-ansi`.
No code duplication, no forks, no bloat in the stripping binary.

```text
strip-ansi       stripping (existing, unchanged)
distill-ansi     transforms (new: depth + palette)
strip_ansi       shared lib (parser, classifier, filter,
                 strip, stream, writer, + transform modules)
```

Source layout:

```text
src/
  main.rs             strip-ansi entry point (existing)
  distill_ansi_main.rs     distill-ansi entry point (new)
  lib.rs              shared library
  ...existing modules...
  sgr_rewrite.rs      SGR param parser/rewriter (new)
  downgrade.rs        color depth reduction (new)
  palette.rs          palette transforms (new)
```

### Feature Flags

```text
                filter (existing)
                   │
               transform (implies filter)
               ╱           ╲
     downgrade-color    color-palette
               ╲           ╱
              distill-ansi-cli
              + terminal-detect + clap + sigpipe
```

```toml
[[bin]]
name = "distill-ansi"
path = "src/distill_ansi_main.rs"
required-features = ["distill-ansi-cli"]

[features]
transform = ["filter"]
downgrade-color = ["transform"]
color-palette = ["transform"]
distill-ansi-cli = ["std", "downgrade-color", "color-palette",
               "terminal-detect", "dep:clap", "dep:sigpipe"]
```

`strip-ansi` uses the `cli` feature — zero transform code
compiled in. `distill-ansi` uses `distill-ansi-cli`. Library
consumers can pick `downgrade-color` and/or `color-palette`
independently without either binary.

### Existing Infrastructure

The filter system provides the foundation
(see [DESIGN.md](DESIGN.md)):

- `SgrContent` bitfield tracks color depth per sequence
  (BASIC, EXTENDED, TRUECOLOR)
- `detect_sgr_mask()` probes target terminal color capability
- `FilterConfig.sgr_preserve_mask` makes strip/preserve
  decisions based on color depth
- `ClassifyingParser` identifies SGR sequences at EndSeq
- `filter_strip_core` buffers sequence bytes in `seq_buf`

### Transform Decision Point

In `filter_strip_core`, at `SeqAction::EndSeq` for CsiSgr:

```text
1. Is palette or depth transform configured?
   NO  → existing strip/preserve path (unchanged)
   YES → continue

2. Re-parse SGR params from seq_buf
3. For each color param:
   a. If palette set: apply palette transform
   b. If depth reduction needed: downgrade
4. Emit rewritten sequence to output
```

### Streaming Considerations

The buffered API (`filter_strip`, `filter_strip_into`) handles
transforms naturally since `seq_buf` already captures sequence
bytes and output is `Vec<u8>`.

The streaming API (`FilterStream`/`FilterSlices`) yields borrowed
`&[u8]` slices. Transforms produce new bytes that do not exist
in the input. Options:

- Phase 1: buffered-only (covers pipe/file use case)
- Phase 2: `TransformSlice` enum
  (`Borrowed(&[u8])` / `Owned(SmallVec<[u8; 32]>)`)

## References

### Color Science

- Brettel, Vienot, and Mollon (1997). Computerized simulation of
  color appearance for dichromats. JOSA A, 14(10), 2647-2655.
- Vienot, Brettel, and Mollon (1999). Digital video colourmaps
  for checking the legibility of displays by dichromats. Color
  Research and Application, 24(4), 243-252.
- Machado, Oliveira, and Fernandes (2009). A physiologically-based
  model for simulation of color vision deficiency. IEEE TVCG,
  15(6), 1291-1298.
- Jordan and Mollon (2019). Tetrachromacy: the mysterious case of
  extra-ordinary color vision. Current Opinion in Behavioral
  Sciences, 30, 130-134.
- Jordan et al. (2010). The dimensionality of color vision in
  carriers of anomalous trichromacy. Journal of Vision, 10(8), 12.

### Palettes

- Okabe and Ito (2002). Color Universal Design (CUD). How to make
  figures and presentations that are friendly to colorblind people.
  [jfly.uni-koeln.de/color/](https://jfly.uni-koeln.de/color/)
- Tol, Paul (2021). Colour schemes. SRON technical note, issue 3.2.
  [personal.sron.nl/~pault/](https://personal.sron.nl/~pault/)
- Wong, Bang (2011). Points of view: Color blindness. Nature
  Methods, 8(6), 441.

### Color Space

- IEC 61966-2-1:1999. Multimedia systems and equipment: colour
  measurement and management. Part 2-1: default RGB colour space,
  sRGB. (sRGB gamma/linearization specification)
- DaltonLens project. Understanding LMS-based color blindness
  simulations. [daltonlens.org](https://daltonlens.org/)

### Terminal Color

- XTerm ctlseqs: SGR parameters.
  [invisible-island.net](https://invisible-island.net/xterm/ctlseqs/ctlseqs.html)
- ECMA-48 5th edition (1991), section 8.3.117 (SGR).
