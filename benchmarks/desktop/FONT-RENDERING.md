# Font pixels: Chromium comparison

Measured on Windows at 150% DPI using the same Arial files, text, logical sizes,
and 1100 × 720 client. [Original-resolution comparison](results/windows/electron-ui/font-comparison.png):
Electron, Incular before, Incular after. This change improves precision; it does
not claim identical Chromium pixels.

## Corrected in the WGPU renderer

- Preserve physical font size to 1/64 pixel. A 13 logical-pixel font at 150%
  now rasterizes at 19.5 ppem, instead of rounding to 20 while keeping 19.5 ppem
  layout advances.
- Rasterize at quarter-pixel X/Y device origins, including compositor translation
  and DPI. Place the resulting mask on integer device pixels, avoiding another
  bilinear resampling of an already antialiased bitmap.
- Carry fractions across pixel boundaries, including negative positions.
- Keep affine sampling for rotated/skewed/scaled compositor text. Keep R8 masks,
  on-demand outlines, the font LRU, and the existing atlas memory budget. At most
  16 phase variants can exist per glyph/size; variants are generated only when
  used and are evicted with their atlas pages. No idle rendering is introduced.

Skia also stores two bits of fractional position per axis in its
[glyph cache identity](https://raw.githubusercontent.com/google/skia/main/src/core/SkGlyph.h).
This is fractional positioning, distinct from RGB LCD subpixel antialiasing.

## Measured screenshot difference

The comparison script uses fixed, unscaled physical-pixel rectangles and reports
mean absolute RGB-channel error against Electron (0 is identical, 255 maximum).
The foreground union includes any pixel below 230 in either image; it avoids
letting large blank backgrounds dominate. These errors include positioning,
hinting and color differences and are not a universal font-quality score.

| Region | Before | After |
| --- | ---: | ---: |
| Description | 73.06 | 66.71 |
| First row title region | 64.09 | 61.91 |
| Toolbar heading | 38.03 | 33.57 |

[Raw measurements and crop coordinates](results/windows/electron-ui/font-pixel-comparison.json).
All three sampled regions improved; none is pixel-identical.

## What exact Windows Chromium rendering still requires

Chromium's Skia Windows port uses
[DirectWrite glyph-run analysis](https://skia.googlesource.com/skia/+/main/src/ports/SkScalerContext_win_dw.cpp),
including font-dependent grid fitting/rendering modes, grayscale or LCD masks,
and fractional positioning. Its final appearance also depends on
[Skia coverage/gamma adjustment](https://raw.githubusercontent.com/google/skia/main/src/core/SkMaskGamma.cpp)
and [Chromium's Windows contrast/gamma settings](https://raw.githubusercontent.com/chromium/chromium/main/ui/gfx/font_util_win.cc).
Incular currently uses unhinted grayscale outlines and linear-light compositing.
The remaining stroke-weight/edge differences are expected from those differences.

A compatible next implementation needs a portable glyph-raster provider contract,
a DirectWrite provider in `incular-windows`, and injection through the desktop host
into the shared WGPU atlas. DirectWrite must rasterize the exact shaped font face
and glyph IDs, including custom font bytes and collection faces. LCD masks need
per-channel coverage blending and a grayscale fallback for transparent/effect
layers and unsuitable transforms. Gamma/contrast must be matched to the output
color space; blindly applying Skia's nonlinear-output LUT to a linear target is
incorrect. A full Skia dependency alone would not guarantee a match.

Pixel identity must be checked with pinned Chromium/Skia versions, the same OS
font files and display settings, matching baselines, and matching target color
space. It cannot be promised across different operating systems or font versions.

## Validation and reproduction

`lazy_glyph_rasterization` checks independent rasterizer coverage/baselines,
fractional em size, ink centroid/area across all 16 phases, warmed-cache reuse,
and memory-budget enforcement. `glyph_position` checks DPI/translation, negative
coordinates, pixel carries and affine fallback. The existing renderer, shared
atlas and parsed-font retention tests remain in place.

```powershell
cargo build --release -p incular --example issue_tracker --no-default-features --features desktop,controls
python benchmarks/desktop/capture.py --states --output benchmarks/desktop/results/windows/electron-ui/final
node benchmarks/desktop/capture-electron-states.cjs
python benchmarks/desktop/compare-visuals.py
```

The last command requires Pillow. Screenshot collection is separate from strict
memory/CPU measurement. See full validation logs (historical artifact removed; latest evidence is in results/README.md)
and [the hardware benchmark report](AMD-HARDWARE.md). Subsequent
[upload batching](GPU-UPLOADS.md) preserves all five captures byte-for-byte while
reducing GPU staging allocations. Its validation is recorded separately in
[validation-uploads](results/validation-uploads/status.json).
