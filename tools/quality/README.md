# Image quality runners

Specular's display, colour and decode paths are measured against independent
references. The fast checks run as ordinary tests in `cargo test`. The slow runs
(`#[ignore]`) and the oracle generators use Python packages in a venv that lives
under the build directory, never in the user's home:

    CARGO_TARGET_DIR=target/quality tools/quality/setup-venv.sh   # -> $CARGO_TARGET_DIR/venv
    VENV=target/quality/venv/bin/python

The pinned oracles are numpy, scipy, pillow, pydicom 3.0.2, colour-science 0.4.7 and
rawpy 0.27.1 (LibRaw 0.22.1), plus the `lcms2` crate (Little CMS) as a
dev-dependency of specular-color. None of these is linked into the app. The tests
never contact the network. The fetch script is the only downloader, and its
output is gitignored.

| area | default test (fast) | slow / corpus run | oracle generator |
|---|---|---|---|
| ICC | `specular-color --test icc_conformance` | same, `-- --ignored` (needs `fetch-icc.sh`) | lcms2 at test time |
| Resampling | `specular-render --lib resample_tests` (GPU) | none | analytic |
| 8-bit output | `specular-render --lib output_tests` (GPU) | none | the source codes |
| HDR maths | `specular-core --test hdr_reference` | none | `hdr_reference.py` |
| DICOM display | `specular-dicom --test display_conformance` | none | `dicom_reference.py` |
| RAW develop | none | `specular-raw --test rawpy_dump -- --ignored` + `raw_vs_rawpy.py` | rawpy at run time |
| RAW per camera | none | `specular-raw --test raw_cameras -- --ignored` (needs `fetch-raw-cameras.sh`) | `rawpy_linear.py` at run time |
| Pass rates | none | `tools/ci/oracle-pass-rates.sh` | each runner's own |

Set `SPECULAR_REQUIRE_GPU=1` so GPU tests fail instead of skipping when there is no adapter.

## ICC: against Little CMS

    tools/quality/fetch-icc.sh            # ICC registry + Little CMS testbed -> tools/corpus/quality/icc
    SPECULAR_QUALITY_OUT=target/quality/quality \
      cargo test -p specular-color --test icc_conformance -- --include-ignored --nocapture

The test writes `icc-conformance.md` with one row per profile × intent × direction.

| direction | what is measured | tolerance ΔE00 (mean / max) | result, 2026-10-24 (median mean / worst max) |
|---|---|---|---|
| `fwd` | device → Lab D50 on a device grid | 0.05 / 0.5 | 0.0006 / 0.048 |
| `inv` | Lab → device (B2A / inverse shaper), both back through lcms | 0.1 / 1.0 | 0.0002 / 0.34 |
| `gpu` | baked `SourceLut` (curves or half-float 33³ cube, shader interpolation) → sRGB, against lcms with BPC | 0.2 / 2.5 | 0.005 / 2.1 |
| `disp` | `Display` curves/cube, sRGB-encoded BT.709 → monitor profile | 0.2 / 2.5 | 0.011 / 0.17 |

The run covers 161 profiles and 1821 measurements, with 0 out of tolerance (worst
cases: `fwd` and `inv` sRGB_ICC_v4_Appearance relative, `gpu` linear_RIMM-RGB_v4
perceptual, `disp` lcms-test4 relative). Fixed on 2026-10-24: the darkest colour of a
Lab device space is L* = 0, a* = b* = 0, not the device zero; since black points keep
their tint (Little CMS 2.19), perceptual on ITULab.icc and LCMSLABI.ICM was off by up
to ΔE00 95. The synthetic `lab_v2_mft2_lab` profile guards it. Eleven
profiles are skipped because they are named-colour profiles or have no device → PCS
tags. The fast test runs the same four directions on synthetic profiles written
byte by byte: every `para` type, `curv`, gray on XYZ and Lab, `mft1`, `mft2`,
`mAB `/`mBA `, RGB, CMYK and Lab devices, and v2 and v4.

The display path on the GPU is checked against these CPU transforms:

- the final pass (`specular-gfx` `final_pass_matches_cpu_reference`) against
  `Display::apply`: max 0.00057 (sRGB), 0.00055 (P3 curves), 0.00049 (3D table), on
  0..1 encoded values; tolerance 0.002 (half an 8-bit code);
- tagged images through the source transform (`specular-render` `colour_tests`) against
  moxcms: Display P3 and Adobe RGB, sRGB or P3 storage, mean ΔE00 ≤ 0.10, max ≤ 0.31.

Known deviations, exempted by name:

- **`Probev2_ICCv4.icc`**: its D2Bx tags deliberately disagree with A2Bx. lcms
  prefers the float tags; we read the 16-bit ones, as the spec allows.
- **`Gray_linear.icc`**: its gamma-0 `curv` makes lcms map black to white. We keep
  black black.

## Resampling: GPU display path

The tests draw charts through the image renderer into a half-float target, in
linear light before the compositor's encode and dither.

| metric | tolerance | measured |
|---|---|---|
| placement at 1× and nearest zooms (4, 5, 8) | < 0.125 8-bit code | 0.08 |
| edge position at 0.31, 0.5, 0.62, 1, 1.37, 2–5× (nine phases each, both axes) | mean over phases ≤ 0.015 px; each ≤ 0.03 (kernel), 0.06 (clamp), 0.1 (bilinear) | mean ≤ 0.006; each ≤ 0.008 / 0.043 / 0.080 |
| impulse centroid at 0.25–3× (Lanczos kernel, clamp off) | ≤ 0.03 output px | ≤ 0.022 |
| impulse DC gain (Lanczos kernel, clamp off) | ± 2 % | −1.0 % to +1.7 % |
| impulse with the anti-ringing clamp (reported) | centroid ≤ 0.1 px | ≤ 0.058; gain +28 % to +49 % |
| ringing at hard edges and one-texel lines (0.7–4×, overview and tiles, RGB and channel views) | ≤ 1 8-bit code over/undershoot | 0 |
| linear-light downscale mean (checker, stripes, grid; 0.125–0.75×) | ± 0.01 | ≤ 0.0018 |
| downscale moiré (worst pixel) | < 0.06 | ≤ 0.042 |
| zone-plate pass band (Lanczos) | 0.9–1.1 | 0.92–1.04 |
| zone-plate aliasing above 1.3 × Nyquist | < 0.05 | ≤ 0.031 (bilinear reaches 0.17) |
| slanted-edge MTF50 at 1× | 0.603 ± 0.03 cy/px | 0.611 |
| slanted-edge MTF50 at 1.5–4× (Lanczos with the anti-ringing clamp) | > 0.38 and > bilinear | 0.438–0.451 (bilinear 0.355–0.384) |
| shallow 16-bit / half-float ramps | ≥ 25 levels per 4 codes, error < 0.5 code | 91–92 levels, ≤ 0.08 code |

`output_tests` measure the whole SDR path at the 8-bit output (renderer, compositor,
final encode and dither):

| metric | tolerance | measured |
|---|---|---|
| 8-bit sRGB image at 1×, 4×, 5×, 8×, settled and moving: pixels not equal to the source code | 0 | 0 (was 24–25 % one code off) |
| 16-bit grey at 1×: worst error / worst 16×16 patch bias (overview, and detail tiles) | < 1 / < 0.25 code | 0.91 / 0.18, tiles 0.91 / 0.18 |
| 16-bit detail tiles: stored codes not equal to the source (RGB 700×530, grey 600×520) | 0 | 0 of 721 112 |
| 8-bit values held in 16 bits (× 257), from tiles at 1×, 4×, 5×, 8× | 0 | 0 |
| tile VRAM once a 16-bit image arrives (256 MB budget) | ≤ budget | 120 8-bit + 60 16-bit layers, 267.6 MB |

Fixed on 2026-10-24:

- **Exact sRGB decode.** The texture unit's sRGB decode keeps about 9 bits on AMD (0.34
  code off). Tiles and 8-bit overviews are now read through a UNORM view and decoded in
  the shader.
- **Undithered zone.** The final pass leaves values within 0.125 code of an output code
  undithered, so 1:1 output is bit-exact; everything else is dithered as before.
- **16-bit detail tiles.** Images with samples wider than 8 bits (SDR) kept their detail
  tiles as 8-bit codes. They now go into a second tile array of 16-bit sRGB codes
  (`Rgba16Uint`, decoded in the shader): a 16-bit sRGB source without a colour transform is
  stored exactly. The first such image splits the tile budget evenly between the two arrays.
- **Decode cost.** The shader decode of 8-bit codes (pow per channel per tap) made a
  downscaling Lanczos draw 41 % slower than with no decode. A 256-entry table (`R32Float`
  texture, exact to f32) brings it to 19 %. AMD Radeon 860M (RADV), 2048² image from tiles
  into 1600 × 1000, median ms per draw, machine under load (`bench_tile_decode`):

  | draw | no decode | pow (before) | table (now) | degree-7 polynomial |
  |---|---|---|---|---|
  | 1:1 Lanczos | 0.89 | 1.04 | 1.04 | 1.07 |
  | 0.8× Lanczos | 3.57 | 5.04 | 4.24 | 4.80 |
  | 2.5× Lanczos | 2.27 | 3.09 | 2.56 | 2.96 |
  | 1:1 bilinear | 0.66 | 0.73 | 0.69 | 0.67 |

- **Mip edges.** Mip levels clamped only overshoot (no brighter than any tap), which moved
  every edge in a reduced level towards the dark side: 0.033–0.059 px on average at
  0.31–0.62×. The clamp is now symmetric, to the taps' range; it also removes negative
  lobes below 0.5×, so impulses gain energy there (+28 % kernel, +69 % with the clamp).

Known limits:

- **The half-float target truncates**, which sets the ramp floor and the 0.08 placement
  error.
- **HDR images have no detail tiles**: they draw from their half-float overview alone.
- **The anti-ringing clamp** limits each Lanczos output to the range of the texels under
  the kernel's main lobe (the nearest 2×2 when magnifying), so edges keep their
  sharpness without halos. It removes the negative lobes' undershoot on an isolated
  impulse, hence the DC gain on one-texel impulses; the kernel itself is gated with the
  clamp off.

## HDR maths

`hdr_reference.py` writes `crates/specular-core/tests/data/hdr_reference.txt` (1270
lines). Its references are:

- colour-science's sRGB, PQ (ST 2084) and HLG (BT.2100) EOTFs;
- the BT.2020 and Display P3 matrices;
- float64 ISO 21496-1 / Ultra HDR gain-map weight and apply.

The comparison is relative error with a 1e-3 floor:

| kind | values | tolerance | worst |
|---|---|---|---|
| sRGB EOTF | 257 | 2e-6 | 2.5e-7 |
| PQ EOTF | 257 | 1e-4 | 4.5e-5 |
| HLG OOTF·EOTF | 387 | 2e-5 | 3.8e-7 |
| primaries matrices | 18 | 5e-4 | 5.2e-7 |
| gain-map weight | 24 | 1e-6 | 1.6e-7 |
| gain-map apply | 600 | 2e-6 | 5.4e-7 |

The tone-map knee is also checked for monotonicity, slope 1 at the knee and
exact content-peak → display-peak. specular-render's `hdr_tests` check the GPU
against these CPU functions.

**What needs a real HDR display.** None of the following can be measured offscreen:

- **Absolute PQ luminance**: 203-nit reference white, and nits out of the panel,
  need a colorimeter on an HDR display.
- **Peak / headroom detection**: the values the compositor reports for the output.
- **The compositor's scRGB / PQ handoff**: whether it clips, tone maps or
  double-encodes.
- **The look of gain-map images on screen**: Ultra HDR and ISO 21496-1 images at
  several headroom values, judged against a reference viewer.

## DICOM display pipeline

    $VENV tools/quality/dicom_reference.py           # writes tests/data/{display,gsdf}_oracle.tsv
    $VENV tools/quality/dicom_reference.py --check   # verifies they are up to date

There are 14 cases covering:

- rescale and modality LUT;
- LINEAR / LINEAR_EXACT / SIGMOID windows;
- VOI LUTs of 8, 12 and 16 bits;
- presentation LUTs;
- MONOCHROME1.

The cases total 2975 sample values, with pydicom as the oracle. Tolerance is 0.5/65535
on the 16-bit output. The result is **0 mismatches**, worst 0.5/65535.

GSDF (PS3.14) is checked against colour-science's `eotf_DICOMGSDF` and inverse:

| quantity | tolerance | worst |
|---|---|---|
| L (relative) | 1e-9 | 4.8e-10 |
| j | 1e-5 | 4.9e-7 |
| L at 4 × 65 P-values, display ranges 0.05–3000 cd/m² (relative) | 1e-8 | 4.4e-10 |

The GPU path (medical setting `display_function = gsdf`, display black and white
luminance, defaults 0.5 and 350 cd/m²) is checked against the same 260 P-values in
specular-render's `gsdf_display_matches_colour_science`. The tolerance is set by the
half-float target (11 significant bits, truncated): linear error ≤ 1.5e-3 relative
+ 2e-5. The worst is 2.9e-3 relative luminance, 0.12 JND.

`docs/dicom-conformance.md` covers the details and the pydicom quirks worked
around in the oracle.

## RAW: linear develop against LibRaw

    SPECULAR_RAW_CORPUS=$PWD/tools/corpus/large/raw SPECULAR_QUALITY_OUT=target/quality/quality/raw \
      cargo test -p specular-raw --test rawpy_dump -- --ignored
    SPECULAR_RAW_CORPUS=$PWD/tools/corpus/large/raw $VENV tools/quality/raw_vs_rawpy.py target/quality/quality/raw

**What is compared.** Both sides develop the same way: half size, as-shot WB, linear
Rec.709, 2×2 block means. `SPECULAR_QUALITY_SIZE=full` develops at full size with
4×4 means, and `SPECULAR_QUALITY_ONLY=<substring>` filters by path.

**How it is compared.**

- Each pair is aligned within ±12 px.
- One exposure gain is fitted.
- Unclipped pixels are compared in CIELAB, with p99 luminance as white.

**Output.** Per file, the runner reports:

- mean and p95 ΔE00;
- R/G and B/G ratios against LibRaw.

It writes `report.md` and deletes the dumps unless `--keep` is given. A dump is about 1.4 GB.

This is a regression and triage tool, not a pass/fail gate. LibRaw is a different
converter, so it is not ground truth: it ignores DNG profiles and look tables, and
uses its own matrices and demosaic.

| group | files | median mean ΔE00 | worst |
|---|---|---|---|
| Canon (CR2, CR3) | 23 | 1.20 | 3.96 |
| Nikon NEF | 11 | 0.61 | 2.37 |
| Olympus / OM System | 7 | 0.6 | 1.72 |
| Panasonic | 6 | 0.45 | 1.52 |
| Pentax | 4 | 2.58 | 4.70 |
| Sony | 2 | 0.76 | 0.93 |
| Fujifilm (X-Trans, Bayer) | 17 | 2.45 | 5.81 |
| Apple ProRAW (linear DNG) | 1 | 0.69 | 0.69 |
| Adobe DNG Converter files | 60 | 2.33 | 14.75 |
| **all** | **131** | **1.78** | **14.75** (70 < 2, 113 < 5) |

The remaining differences above 5 have identified causes:

- **DNG profiles** (ProfileHueSatMap / LookTable), which LibRaw ignores.
- **Fujifilm demosaic**: X-Trans demosaics differ.

Fixed causes:

- **Sony SR2 ColorMatrix (0x7800)** is the camera's rendering matrix into its selected
  output space, not a calibration. The ILCE-7RM5 file was shot in Adobe RGB and its matrix
  is close to identity, so blue was twice LibRaw's. It is no longer used, as in LibRaw.
  Sony bodies outside the Adobe table now get the generic matrix: ILCE-7RM5 went from
  6.70 to 0.58 and DSC-RX100M7 from 1.44 to 0.93. Adobe data for these bodies would be better.
- **Panasonic RW2 formats up to 4** keep a +15 offset in the samples that the per-channel
  black tags leave out. The table's black level is now added to the tags, giving 144/143
  as in LibRaw. DC-GH5 went from 6.59 to 0.03; formats 5 and 6 already removed the offset
  in the decoder.

Files LibRaw or we refuse are listed without a ΔE: Canon sRAW/mRAW, Nikon HE,
RW2 format 8, lossy-JPEG DNG, GPR and vendor-compressed 3FR/FFF/DCR/ERF.

## RAW per camera: linear data against LibRaw

    tools/quality/fetch-raw-cameras.sh       # 22 files, 21 bodies, ~460 MB -> $CARGO_TARGET_DIR/corpus/raw-cameras
    SPECULAR_RAWPY=$VENV SPECULAR_RAW_CAMERAS=$CARGO_TARGET_DIR/corpus/raw-cameras \
      cargo test -p specular-raw --test raw_cameras -- --ignored --nocapture

Everything before the look is compared, per file:

- sensor values (bit-exact inside the crop);
- black and white levels;
- highlights: where the data clips, the develop's white level and the clipped greens;
- as-shot white balance;
- the colour matrix, as ΔE00 over the 24 ColorChecker patches (reported, not gated);
- camera-space linear data, no white balance or matrix, in black-subtracted raw units:
  the half-size develop pixel by pixel, the full demosaic in 2×2 means.

`SPECULAR_QUALITY_OUT` also writes `raw-cameras.tsv`. Result on 2026-10-24: 22 of 22 pass.

| check | tolerance | result |
|---|---|---|
| sensor values | 0 mismatches | 0 in all 21 comparable files (iPhone ProRAW is linear) |
| black / white | ≤ 1 / exact | all, except the 1300D's white (known: dcraw's table 13584, LibRaw's own 14274) |
| highlights: develop white where the data clips | LibRaw's largest sample (its default develop's maximum) | equal in all 14 files that clip |
| highlights: clipped green blocks below full scale (magenta) | 0 | 0 of 778 222 blocks |
| white balance | 0.5 % | ≤ 0.18 % (6D: known, LibRaw reads none) |
| half size: gain / mean error | 0.5 % / 0.25 % of range | ≤ 0.17 % / ≤ 0.09 % |
| full demosaic: gain / mean error | 1 % / 1 % | ≤ 0.73 % (300D) / ≤ 0.41 % |
| matrix (reported) | none | 0.00 where both use the same dcraw-era table (1300D, GH5, Minolta, Kodak, Hasselblad); 0.5–9.4 max where we use newer Adobe or camera DNG data |

LibRaw's own AHD, VNG and bilinear differ by up to 1.0 % in gain on the 300D file, which
sets the full-demosaic gain tolerance. LibRaw's half size of the linear iPhone DNG is
rescaled, so that check is known; its full size matches exactly.

Fixed on 2026-10-24:

- **Sony classic ARW** clipped at the SR2 WhiteLevel (15360) although the samples run to
  about 16300; the raw IFD's WhiteLevel (16380) is used where present, as for lossless ARW.
- **Canon masked black** included about ten lit columns left of the sensor border (1300D:
  black 2097 instead of 2047; 5D Mark III: 18 columns, +9). The masked area now stops 32
  columns short where the border is wide enough.
- **Canon 1300D** (ColorData8, version 14) black levels are read: 2046 2045 2046 2046.
- **Canon white levels.** ColorData's specular white sits below where the data clips: on the
  EOS RP at ISO 100 the samples run evenly from 14558 to 16383, and on the 90D, R and R10
  (specular 13036–14338) the data clips at 16383 too, so highlights clipped early. The white
  is now the 14-bit maximum, as in LibRaw, unless dcraw's table has the body.
- **Clipped highlights.** Bodies clip at one level per ISO, below the table's white on some
  (6D at ISO 100: 15025 for 15490; 5D Mark III 15464; 7D Mark II 13583), and channels a few
  units apart (300D: 4002 and 4007). A clipped green then stayed below full scale while red
  and blue, scaled by white balance, reached it: magenta (6D: green at 0.965). The develop
  now finds the clip in the data (a top value shared by at least four samples, in the top
  quarter of the range), maps samples at it to full scale per channel before white balance
  and lowers the white to it, as LibRaw's default develop lowers its maximum.
- **Epson R-D1s** white balance gains are now 567 and 431 (/65536), as LibRaw 0.22 reads
  them. Against the camera's own JPEG, R/G went from 0.960 to 1.003 and B/G from 1.087 to
  1.026.

Known: LibRaw reads no white balance from the 6D file (unity) and its develop of that file
is off (green seven times red in camera space); the sensor values, levels and highlight
checks still run.

## Fidelity: option matrices against two references

    tools/corpus/fetch-fidelity.sh              # matrices + PngSuite, OpenEXR, AV1 AVIF, HEIF suites (~130 MB)
    SPECULAR_RAWPY=$VENV cargo test -p specular-convert --test fidelity -- --ignored --nocapture
    tools/quality/fetch-raw-cameras.sh --broad  # 29 more bodies (~400 MB) -> raw-cameras-broad
    SPECULAR_RAWPY=$VENV SPECULAR_RAW_CAMERAS=$CARGO_TARGET_DIR/corpus/raw-cameras-broad \
      cargo test -p specular-raw --test raw_cameras -- --ignored --nocapture

The venv needs numpy, pillow, tifffile, imagecodecs, rawpy, OpenEXR and pillow-heif; FFmpeg,
ImageMagick, heif-dec and exiftool from the system. `fidelity-oracle.py gen` writes the
matrices with the reference encoders; each lossless file gets its source array beside it
(`<file>.npy`), an exact third reference.

Per file the runner decodes through the registry twice: the samples (stored orientation, no
colour management) and the picture the viewer shows (`convert_bytes`: ICC to sRGB, HDR
tone-mapped, oriented). The script then:

- decodes it with two independent references (below) and takes their agreement, or the
  generator's array, as the truth; a disagreement is printed and ours must match one;
- compares within half a step of the coarser sample depth (one source step where samples
  are widened), 1/255 mean and 10/255 at the 99.9th percentile for lossy codecs, 0.2 %
  relative for float;
- classifies a difference: size or crop window, orientation (which of the eight), shift up
  to 2 px, channel swap, inversion, fitted range or gamma, premultiplied or straight alpha,
  edge-only, at strong edges only; a consistent signed bias is flagged even when it passes;
- checks the shown picture against the reference brought to sRGB (embedded ICC with Little
  CMS, PNG gAMA, DPX and float TIFF linear) and oriented by exiftool's orientation, and the
  Info orientation against exiftool's (or the generator's for HEIF irot/imir).

| format | references | compared | pass / fail / skip |
|---|---|---|---|
| TIFF | tifffile (+ imagecodecs), Pillow (libtiff), generator | 158 | 157 / 1 / 0 (1 known) |
| PNG | libpng (imagecodecs), Pillow, generator | 204 | 191 / 0 / 13 (PngSuite's corrupt `x*` files: both refuse) |
| OpenEXR | OpenEXR, FFmpeg, generator | 110 | 103 / 1 / 6 (1 known; luminance-chroma files have no second reference here) |
| JPEG XL | libjxl, generator | 26 | 26 / 0 / 0 |
| AVIF | libavif (imagecodecs, clap/irot/imir applied here), libheif (heif-dec), Pillow, generator | 197 | 193 / 4 / 0 |
| HEIC | pillow-heif (libheif), heif-dec | 22 | 22 / 0 / 0 |
| DPX | FFmpeg, ImageMagick | 34 | 34 / 0 / 0 |

Notes per format, what is tested and what not:

- **TIFF.** Tested: 8–64-bit unsigned, signed (shown offset binary) and float samples;
  1, 3 and 4 samples; both byte orders; contiguous and planar; none, LZW, Deflate, ZSTD,
  PackBits, LZMA, LERC, JPEG (YCbCr 1×1, 2×1, 2×2 and RGB), WebP lossless, JPEG XL;
  predictors 2 and 3; strips and tiles; straight and associated alpha; MinIsWhite 8/16;
  palette 8; grey 1, 2, 4 and 12-bit; CMYK; orientation 1–8; ICC (Adobe RGB, 8 and 16-bit);
  BigTIFF; Pillow's libtiff writer. Known: untagged CMYK goes through a generic CMYK profile,
  the references convert naively. Untested here: 4-bit palette (tifffile writes a
  non-conforming colour map), CIELab, LogLuv, old-style JPEG (the tiff-variants runner has
  them), pages after the first.
- **PNG.** Tested: all of PngSuite (every depth and colour type, Adam7, gAMA, sBIT, tRNS,
  filters, chunk order, damaged files); generated grey, grey+alpha, RGB, RGBA at 8 and 16
  bits; palettes of 1–8 bits with tRNS; grey and RGB tRNS; iCCP; eXIf orientation. A gAMA
  profile applies to grey images too (it did in the viewer, not in exports). Untested:
  APNG frames after the first, cICP HDR, cHRM without gAMA.
- **OpenEXR.** Tested: half and float × Y, RGB, RGBA × all ten compressions; tiles; data
  window offset and overscan; decreasing line order; chromaticities (Bradford to BT.709);
  alpha (associated in the file, straight in ours); the openexr-images ScanLines, Tiles,
  TestImages, Chromaticities, DisplayWindow, Beachball and v2 sets. We show the data window
  (a design decision; FFmpeg shows the display window). Known: WideFloatRange (float32
  beyond the half-float raster's range). Untested: luminance-chroma (RY/BY) against a
  second reader, deep data, mip/rip levels, parts after the first.
- **JPEG XL.** Tested: grey, grey+alpha, RGB, RGBA × 8, 16-bit and float × lossless and
  distance 1, efforts 1–9. The conformance suite has its own runner with libjxl's reference
  decodes (specular-jxl `tests/conformance.rs`). Untested here: animation, layers, CMYK
  (refused), orientation.
- **AVIF.** Tested: 8, 10, 12-bit × 4:4:4, 4:2:2, 4:2:0, 4:0:0 × alpha; lossless 4:4:4;
  irot/imir; the AV1 AVIF test files (Apple multilayer and grids, Link-U orientation and
  clap, Microsoft range, HDR, alpha, 4K, Netflix AVIS, Xiph layers and film grain). libavif
  and libheif disagree on files whose colr box and AV1 header disagree on range; we follow
  the colr box, as libavif does. Untested: operating points other than 0 (`a1op`), layer
  selection (`lsel`) of a lower layer, AVIS frames after the first.
- **HEIC.** Tested: 8-bit RGB, RGBA, grey; 10-bit RGB, RGBA; lossy and 4:4:4; libheif's
  example and Nokia's content (grids, overlays, thumbnails). Untested: PQ/HLG HEIC, depth
  and other auxiliary images, irot/imir in the matrix (the gainmap and HEVC runners cover
  others).
- **DPX.** Tested: ImageMagick's 8, 10, 12, 16-bit × RGB, RGBA, grey × both byte orders;
  FFmpeg's rgb24, rgba, rgb48le, rgba64be, gbrp10/12le, gray, gray10/12le, gray16be;
  transfer "linear" shown linear. The film runner covers log, YCbCr and packing methods.
- **RAW (broad).** 29 bodies from 16 makers (above), the RAW per camera runner's checks:
  17 pass, 11 fail, 1 refused (Fujifilm S2Pro, SuperCCD). Known oracle limits: LibRaw applies
  no DNG opcodes (iPhone 7 Plus, DJI vignetting and bad pixels), rescales linear DNG at
  half size, reads no white balance from the D1. Failing: Nikon P7000 NRW and Sony R1 SR2
  (sensor values differ: our unpacking), Pentax K-x, Olympus E-PL1 and Sony NEX-5N (white
  level: a makernote level or a 5-sample pile below LibRaw's), Samsung NX100 (black 1.2
  apart), Leica D-LUX 4 (white 4097 against 4095), Panasonic LX3 (blue 5 % in the full
  demosaic), Fujifilm S5600 and X-T1 (half size of SuperCCD and X-Trans: LibRaw's differs).

## Fidelity, extra set: the second format group

    tools/corpus/fetch-fidelity.sh --extra    # matrices + suites -> corpus/fidelity-extra (~50 MB)
    SPECULAR_FIDELITY_CORPUS=$CARGO_TARGET_DIR/corpus/fidelity-extra SPECULAR_RAWPY=$VENV \
      cargo test -p specular-convert --test fidelity -- --ignored --nocapture

The same runner and classifier as above (pass-rate runner `fidelity-extra`). The venv also needs
psd-tools, OpenImageIO and opencv-python-headless; the system tools are libjpeg-turbo's and
libwebp's command-line programs, ImageMagick, FFmpeg and exiftool. Suites, pinned by commit
or SHA-256: libjpeg-turbo's test images, libwebp's test vectors, the pygif GIF test suite
(expected frames), BMP Suite 2.8 (expected pictures), the JPEG 2000 conformance codestreams
(openjpeg-data) and psd-tools' test files. The writers for GIF, BMP, PSD, Radiance, PFM and
Cineon are byte-level (every variant reachable); the references read their output back.

What the round added to the method:

- **Frames and pages.** An animation's composited frames after the first (`file#k`) and a
  document's pages after the first (`file@k`: MPF images, PSD layers) are rows of their own.
- **Planes.** YCbCr rasters also dump their planes. WebP planes must equal `dwebp -yuv`
  (range-expanded) exactly; JPEG planes libjpeg-turbo's within one code. A JPEG whose planes
  pass is not judged on its RGB: libjpeg replicates 4×-subsampled chroma, the viewer filters it.
- **Exact suites.** The GIF and BMP suites' expected pictures outrank the decoders, like the
  generator's array.

| format | references | rows now | pass / fail / skip (first run → now) |
|---|---|---|---|
| JPEG | Pillow, FFmpeg, libjpeg-turbo 3, jpegsof3 (lossless), planes | 99 | 72 / 24 / 3 → 96 / 3 / 0 (3 known) |
| WebP | Pillow, FFmpeg, OpenImageIO (animation), dwebp planes | 188 | 155 / 30 / 3 → 176 / 12 / 0 (12 known) |
| GIF | Pillow, FFmpeg, ImageMagick, pygif expected frames | 173 | 154 / 11 / 18 (183 rows: images, not frames) → 155 / 12 / 6 (12 known) |
| BMP | Pillow, ImageMagick, OpenImageIO, BMP Suite pictures, generator | 116 | 97 / 13 / 6 → 112 / 3 / 1 (3 known) |
| JPEG 2000 | OpenJPEG, OpenJPH, FFmpeg, generator | 92 | 81 / 5 / 5 (91 rows) → 85 / 2 / 5 (2 known) |
| PSD/PSB | psd-tools, ImageMagick, OpenImageIO, LittleCMS (CMYK, Lab), generator | 476 | 160 / 45 / 271 → 373 / 24 / 79 (24 known) |
| Radiance HDR, PFM | OpenImageIO, FFmpeg, imagecodecs, generator | 18 | 14 / 4 / 0 → 18 / 0 / 0 |
| Cineon | OpenImageIO, ImageMagick (code values), generator | 34 | 10 / 23 / 1 → 34 / 0 / 0 |

The first run's failures were mostly the harness (unclamped YCbCr, JFIF chroma offset, chroma
upsampling convention, references reading orientation-less, a Cineon writer bug); the rest
are below.

- **Fixed.** GIF images without a graphic control extension showed one by one: the pygif
  suite's high-colour picture showed a quarter. They now join the next image, and images that
  tile the picture form one still (gif.rs). 32-bit PSDs: the "Linear RGB" profile was handed
  on with the composite already sRGB-coded (shown far too bright); a "merged transparency"
  block (`Mt16`/`Mt32`/`Mtrn`) was ignored (transparent backgrounds opaque); the white matte
  came off in sRGB, not linear light; an exact sRGB encoding replaces a 14-bit table (half a
  code off near black). PSDs whose writer is not named un-matte when the samples are matted.
  BMP with BI_JPEG/BI_PNG pixels opens through the JPEG/PNG decoder.
- **Known, by design or convention.** Untagged CMYK through the generic printing model;
  Lab out-of-gamut clipping (mean ≤ 0.5/255 from LittleCMS in gamut); GIF disposal 2 clears to
  transparent (browsers, the suite) where Pillow and FFmpeg fill the background colour; PSDs
  without a merged image are refused with the reason; the fourth channel of a PSD without a
  layer section taken as alpha; a raw J2K codestream with four components and no colour
  specification shown RGB + alpha; empty GIFs refused.
- **Known, open.** WebP: the VP8 planes are bit-exact, but the decoder expands them to full
  range in 8 bits and clips samples outside 16–235/16–240, so saturated pixels are up to
  25/255 off libwebp (fixed: a studio-range YUV raster). BMP: OS/2
  Huffman 1D and RLE24, and 64-bit BMP, are refused with a reason.
- **Oracle bugs.** OpenImageIO and FFmpeg ignore Radiance orientation strings other than
  `-Y +X` and old-style runs; FFmpeg reads no `#?RGBE` magic, no animated WebP, scales 1–4-bit
  JPEG 2000 by a shift, mis-reads RGB lossless 12/16-bit JPEG and widens 8-bit to 16 by a
  shift; OpenImageIO ignores BMP bit-field masks (and reads the X byte as alpha), and reads
  only 10-bit Cineon in 32-bit words; ImageMagick reads only big-endian 10-bit Cineon,
  4-bit BMP fields by a shift and gives a wrong Lab; Pillow and FFmpeg fill the GIF background
  colour on disposal 2; psd-tools un-mattes PSDs whose extra channel is not transparency.
- **Untested here.** JPEG arithmetic hierarchical, DNL; WebP frames after the eighth; JPEG
  2000 ROI and component subsampling against a second decoder (OpenJPEG via imagecodecs
  refuses subsampled components); PSD layer pages with masks, fill opacity or CMYK/Lab;
  XYZE Radiance files; DPX log through this runner (the film runner has it).

## Fidelity: follow-up fixes

Same runners (`fidelity`, `fidelity-extra`, `raw-broad`). New references: OpenEXR's own RGBA
interface for luminance-chroma files (`tools/corpus/exr-rgba-oracle.cc`, built by
`fetch-fidelity.sh exr` against the system's OpenEXR 3.1 library), OpenImageIO's
`ImageBufAlgo.flatten` for deep EXR, libavif per AVIS frame and heif-dec per top-level item.

| runner / format | before | now |
|---|---|---|
| raw-broad | 17 / 11 / 1 | 24 / 4 / 1 (2 of the 24 known: LibRaw's own demosaics spread as far) |
| raw-cameras (main corpus) | 22 / 22 | 22 / 22 |
| AVIF av1-avif (frames and pages are rows) | 169 / 23 / 0 | 176 / 16 / 0 (16 known) |
| OpenEXR openexr-images | 42 / 1 / 6 | 47 / 2 / 0 (2 known) |
| WebP gen + libwebp-test-data | 176 / 12 / 0 | 188 / 0 / 0 |

- **RAW.** Coolpix P NRW: 12 bits MSB-first over little-endian 32-bit words (98 % of the
  samples were wrong), white balance from NRWData. Sony DSC-R1: big-endian samples in a
  little-endian file. Pentax: the makernote's image area is the crop (the K-x codes 43 junk
  columns reaching 3872). Samsung 4704-wide bodies: black from both masked strips. Stated
  whites as LibRaw states them: the full scale for ORF, R1, Coolpix P and RW2 up to format 4,
  and for ARW2 without a raw-IFD WhiteLevel (the NEX-5N's SR2 15360 sits below data piling up
  at 16404). The develop's clip detection is a histogram rule now (a code holding 4× the
  samples per code in use of the band below, within 4/128 of the range under the top): it
  takes a clip spread by noise (E-PL1: 20 000 samples per channel at 3928–3958, 5 at the top
  value), a small isolated one (DJI FC220: 19 at 16380) and one with a tail (ARW2 codes in
  steps of 32), and leaves an even tail alone. Open: Panasonic LX3 blue in the full demosaic
  (+5.2 %; LibRaw's own demosaics spread 3.1 %, a textbook Malvar on LibRaw's mosaic 2.3 %);
  S5600 and X-T1 half size; DJI bad-pixel opcode.
- **AVIF.** `a1op` selects the operating point and `lsel` the layer (every layer out, the
  one asked for kept). Lower-layer items are the primary picture at half size (mean
  3.9–7.2/255 off heif-dec's top layer box-filtered). Known oracle limits: libheif 1.21
  ignores `lsel`/`a1op`, libavif reads no other items. The Link-U crops are exact (window
  272, 39, 385×330 per ISO/IEC 23008-12); what remains is the YUV conversion against libavif
  (0.75/255 mean on the uncropped file). AVIS alpha is exact in every frame; libavif's blue
  sits 6–7/255 below both libheif and ours on alpha_video.
- **WebP.** Lossy WebP is a studio-range `Yuv` raster (VP8's planes as decoded); the GPU and
  the CPU reference convert it. Against libwebp's RGB (Pillow), 81 lossy files: 81.7 % of
  samples exact, 99.99 % within 1, max 1.5/255. `specular_webp::image::to_rgba` (TIFF's WebP
  tiles, still frames) uses libwebp's arithmetic: within 1 of dwebp.
- **OpenEXR.** Luminance-chroma: XYZ chromaticities fell back to Rec. 709 luminance weights
  (green 2–6 % off); subsampled chroma is interpolated linearly (was nearest). Against the RGB
  originals: Rec709_YC 0.56 % mean relative error (OpenEXR's own 0.52 %), XYZ_YC 0.30 %
  (0.28 %). The four deep files compare with OpenImageIO's flatten.
- **COLR v1.** Paint graphs draw in thumbnails and the decoded font picture (solid, linear,
  radial and sweep gradients, every affine paint, glyph clips, layers, reused glyphs,
  composites); variations and clip boxes are not applied. The in-app specimen's colour row
  still draws COLR v0 and bitmaps only (swash).
- **Not done.** JPEG XL CMYK: the decoder has no colour management (CMYK needs the embedded
  profile's CMYK→RGB inside its render pipeline); the conformance case `cmyk_layers` stays
  refused.

## Fidelity: PNG/APNG, BMP, TIFF, PSD, HEIC and DPX

Same runners (`fidelity`, `fidelity-extra`, `video-fidelity`, `raw-broad`). New references: an APNG
compositor written from the APNG specification (`ref_apng_spec` in `fidelity-oracle.py`:
dispose and blend ops in float on an 8-bit canvas) beside FFmpeg's APNG decoder and Pillow;
pillow-heif and heif-dec per top-level item for HEIC pages. New corpus: an APNG matrix (every
dispose op with both blend ops, sub-rectangle frames with partial alpha, a hidden default
image, a palette animation), the FATE MPEG-2 streams as programme streams (`*-ps.mpg`),
H.264/HEVC bottom chroma siting (locations 4 and 5) and a really colocated AV1 stream
(`av1_metadata` sets the sequence header's `chroma_sample_position`; libaom wrote 0).
`fetch-raw-cameras.sh` takes `ONLY=<regex>` to fetch a few bodies.

| runner / format | before | now |
|---|---|---|
| MPEG-2 FATE (now also as programme streams) | 1 / 3 / 0 | 5 / 2 / 0 (2 known: sony-ct3 pixel aspect) |
| VP9 gen + libvpx (4:4:0) | 20 / 2 / 0 | 22 / 0 / 0 |
| H.264 / HEVC gen (bottom siting added) | 26, 19 | 28, 20, all pass |
| raw-broad RAF / RW2 / DNG | 0/2/1, 0/1, 7/1 | 1/1/1, 1/0, 8/0 (LX3 and X-T1 known) |
| PNG gen (APNG added) | 23 / 0 | 53 / 0 |
| PngSuite | 168 / 0 / 13 | 168 / 0 / 13 (13 corrupt files both refuse) |
| TIFF gen | 157 / 1 (pages of a broken generator: 23 more failures) | 157 / 1 (cmyk8 known) |
| HEIC libheif + Nokia (pages are rows) | 12 / 7 | 19 / 0 |
| DPX gen | 34 / 0 | 34 / 0 |
| BMP Suite | 64 / 5 / 1 (rlecut pair regressed) | 66 / 3 / 1 (3 known) |
| GIF gen + pygif | 155 / 12 / 6 | 155 / 12 / 6 (all known disposal and empty-file policies) |
| PSD psd-tools | 327 / 19 / 79 | 352 / 9 / 80 (9 known) |

- **MPEG-2.** The decoder was bit exact on all three FATE streams; the losses were in the
  demuxer. A bare stream was one piece whose 16-bit in-piece offset wrapped past 64 KiB, so
  pictures after the first were read from the wrong place (tcela-6: 1 of 4 frames,
  hhi_burst_422: 3 of 4). In programme streams copied from bare ones only B pictures carry a
  PTS: the I picture got a time before the first PTS and was dropped, and GOPs without a time
  ahead of the first timed one were stacked on it. Now every picture matches libavcodec in
  both containers. sony-ct3 keeps its known pixel-aspect reading.
- **Chroma.** 4:4:0 and 4:1:1 stay as coded (VP9 profile 1, Motion JPEG, DV NTSC) and are
  interpolated by the GPU shader and `yuv::to_rgb` (they were widened by repeating rows or
  columns); still and alpha pictures interpolate them before conversion. Bottom siting (H.273
  4, 5) is a new `YuvEncoding::bottom_y` (shader flag bit 6); AV1 colocated chroma reaches the
  player through `specular_avif::video::VideoPicture::cosited_y`. RGB Motion JPEG keeps its
  samples as G, B, R planes under the identity matrix. The Movie group of the Info panel names
  non-square pixels ("Pixel shape 8:9, shown stretched").
- **RAW.** DJI FC220: the 2126 differing sensor samples are exactly the OpcodeList1
  FixBadPixelsList pixels (2169 listed), which we apply and LibRaw does not; the sensor check
  counts them apart (`DngOpcodes::fixes_pixel`). Panasonic LX3: LibRaw's AHD is the outlier
  (blue 0.961 of its half size; its bilinear 1.000, VNG 1.008, PPG 1.015, DCB 1.014; ours
  1.010), known. Fujifilm X-T1 half size: LibRaw keeps one sample per colour of each 2x2 block
  (dcraw's shrink), we average each colour over the 3x3 window around it; an emulation of each
  against LibRaw's output gives 0.15 % and 0.62 % mean, known. Open: S5600 (SuperCCD) half
  size 0.47 % mean; S2Pro refused (frame size does not fit the raw data).
- **Colour fonts.** Thumbnails draw CBDT bitmaps (image formats 17, 18, 19; index formats
  1–5), also in faces whose glyphs have no outlines (they showed an empty box). SVG-in-OT
  glyphs stay outlines: the specimen renderer (swash) does not draw them either.
- **TIFF.** The generator wrote one-channel arrays as (H, W, 1), which tifffile stores as H
  pages of W×1: gray1/2/4/12 and MinIsWhite were page stacks and every page after the first
  failed. Fixed in the generator; the decoder was right.
- **HEIC.** Pages were compared with the primary image; now with the same top-level item.
- **BMP.** Pixels RLE data leaves undefined (delta skips, early line or bitmap ends, cut-off
  data) are transparent, as browsers show them and BMP Suite prefers (q/pal4rlecut,
  q/pal8rlecut); they took palette colour 0.
- **PSD.** Files saved without a merged image are composited from their pixel layers
  (`specular_psd::Psd::composites_layers`): transparency, masks, opacity and fill opacity,
  the W3C blend modes (separable ones per channel; hue, saturation, colour and luminosity in
  RGB), clipping to the base layer, isolated and pass-through groups, hidden layers. Fill and
  adjustment layers, effects and vector masks are not drawn; the encoding row says how many
  are left out. Files with only fill or shape layers stay refused with that reason.

## Per-format pass rates over time

    tools/ci/oracle-pass-rates.sh [--only icc,raw-cameras,...] [--check] [--update] [--history]

The script runs every registered oracle test (66 runners, `--list`) over its corpus. It
counts files that pass, fail or are skipped, from one `ORACLE<TAB>format<TAB>file<TAB>status`
line per file (or per case for synthetic suites). All runners print these lines except
archive, which counts as one unit. A runner whose corpus is missing is listed as `no-corpus`.

- **Runners.** Each row names the cargo test and filter, the corpus (`${ENV_VAR:-default}`,
  so a corpus outside `tools/corpus` works), and optionally the test flag: `--ignored` by
  default, `--include-ignored` where the oracle is a plain test.
- **Corpora.** Each fetch script takes an output-directory variable (see its header).

Result on 2026-10-24, all corpora fetched except dng-profile's (needs Adobe's
`dng_validate`); 4.9 GB:

| runner | passed / compared | failures |
|---|---|---|
| icc | 150 / 150 (11 skipped) | |
| raw-cameras | 22 / 22 | |
| raw-more | 35 / 35 | four-colour files (Pro70 CRW, F828 SRF) compared as their mosaic |
| raw-rest | 30 / 30 | |
| hevc | 187 / 187 | |
| jp2-jpx, jpeg-lossless-arith | 1 / 1, 1540 / 1540 | |
| gainmap | 6 / 6 | |
| dicom-display (GSDF) | 4 / 4 | |
| film | 114 / 114 (4 damaged files refused) | code values against ImageMagick, OpenImageIO and FFmpeg (at least one agrees; each failing reader is shown wrong in the round-3 notes) |
| legacy2 | 143 / 143 | |
| flif, jbig, jbig2, wsq | 297 / 297, 386 / 386, 111 / 111, 59 / 59 | |
| wsi, em, spm, spm-afm | 9 / 9, 137 / 137, 21 / 21, 11 / 11 | |
| asdf | 117 / 117 | |
| jcamp, nmr, mzml | 180 / 180, 26 / 26, 351 / 351 | |
| gds, kicad | 1298 / 1298, 232 / 232 | |
| pcb | 393 / 393 (12 not compared) | the test asserts; `ORACLE_WRONG` lists the files where gerbonara or pygerber is shown wrong, with the evidence |
| cad, hpgl | 273 / 273, 28 / 28 | |
| points | 76 / 76 | PCD has no oracle |
| mesh-x3d, mesh-ifc | 56 / 56, 128 / 132 | IFC: IFC4x3 sweeps along alignments (2), signals placed along an alignment, ISSUE_171's curved sweeps; curved advanced B-reps need OpenCASCADE (not compared) |
| terrain, fft, archive | 4 / 4, 7 / 7, 1 / 1 | |
| video-encode, h264enc | 2 / 2, 59 / 59 | |

- **Output.** Each run writes `target/oracle-rates/<date>.tsv` (not committed) and
  compares it with the previous run.
- **Committed summary.** `tools/ci/oracle-pass-rates.tsv` holds the accepted rates;
  `--update` rewrites it.
- **Drops.** A fall in pass rate or passed count is printed as `DROP`; `--check` exits 1
  on a drop against the summary.
- **History.** `--history` prints every dated run side by side.

## Develop: darktable-cli (developer machine only)

    tools/quality/darktable-oracle.sh RAW_DIR OUT_DIR [SPECULAR]

Renders each raw with darktable-cli and with `specular develop export` at defaults (1024 px,
8-bit sRGB) and prints CIEDE2000 p50/p95/p99/max per raw (`darktable_compare.py`). A gross-error
check, never in tests or CI, never a speed comparison. Without darktable-cli it says so and stops;
nothing installs or bundles it. darktable keeps its own config in OUT_DIR. Results go in
`docs/develop-quality.md`.
