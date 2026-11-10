# DICOM grey display conformance

This document covers how Specular maps stored grey pixel values to displayed values (PS3.3 C.11),
and its Grayscale Standard Display Function maths (PS3.14). It is a statement of what is tested.
It makes no claim of diagnostic quality (see DESIGN.md).

## Pipeline

All of this lives in `crates/formats/specular-dicom/src/display.rs`.

| Stage | Standard | Implementation |
|---|---|---|
| Modality rescale | PS3.3 C.11.1.1.2 | Stored value × Rescale Slope + Rescale Intercept. A per-frame Pixel Value Transformation is used if present. RT Dose Grid Scaling serves as the slope. |
| Modality LUT | PS3.3 C.11.1.1.1 | Used when there is no rescale. Inputs below the first mapped value take the first entry; inputs above the last take the last entry. A negative first value is read as a two's-complement word for signed pixel data. |
| Window, LINEAR | PS3.3 C.11.2.1.2.1 | Uses c − ½ and w − 1. Output is 0 at x ≤ c − ½ − (w−1)/2 and 1 above c − ½ + (w−1)/2. Width 1 thresholds. Widths below 1 are skipped as the default. |
| Window, LINEAR_EXACT | PS3.3 C.11.2.1.3.2 | (x − c)/w + ½, clamped to 0..1. Width must be greater than 0. |
| Window, SIGMOID | PS3.3 C.11.2.1.3.1 | 1 / (1 + exp(−4 (x − c)/w)). |
| Multiple windows | PS3.3 C.11.2.1.2 | The first valid pair is the default. The others are offered as presets. |
| VOI LUT | PS3.3 C.11.2.1.1 | Clamp-indexed. Output is divided by 2^n − 1, where n is the descriptor's bit count (8–16). n is widened when the data exceed it. |
| Presentation LUT | PS3.3 C.11.6 | The VOI output d ∈ 0..1 selects entry ⌊d·(N−1)⌋. Output is divided by 2^n − 1. |
| Presentation LUT Shape INVERSE, MONOCHROME1 | PS3.3 C.11.6.1, C.7.6.3.1.2 | Output is 1 − d. MONOCHROME1 with INVERSE inverts once, which is the DX convention. |

Grey renders are rounded to 16 bits. The GPU window shader (`specular-render`, `medical.wgsl`) uses the same formulas. `renderer/medical_tests.rs` checks it against the CPU path to within 1e-6.

### Deliberate deviations

- **Float Pixel Data treats LINEAR as LINEAR_EXACT.** The ½ and 1 offsets exist for integer stored values. On a 0..1 parametric map, a 0.5/1 window would otherwise turn into a threshold.
- **LUT bit counts are widened to fit the data.** Some files declare 8 or 12 bits but hold larger values. pydicom keeps the declared count, so its output for such files is not in range.

## Oracle comparison

`tools/quality/dicom_reference.py` builds 14 cases. For each it writes the data elements, the stored values and pydicom 3.0.2's result for that pipeline into `tests/data/display_oracle.tsv`.

The test `tests/display_conformance.rs` then takes each case through these steps:

1. Build a Part 10 file with the crate's own writer.
2. Parse and decode it.
3. Render it with the default VOI.
4. Compare every value with pydicom's.

The test passes when each value is within half a 16-bit step, that is, equal after rounding.

| Case | Values | Worst \|Δ\| (of 65535) |
|---|---|---|
| CT: signed 12-bit, intercept −1024, LINEAR 40/400 | 173 | 0.496 |
| 16-bit, slope 0.37, intercept −12.5, LINEAR_EXACT | 314 | 0.475 |
| SIGMOID, slope 2 | 167 | 0.500 |
| LINEAR, width 1 (threshold) | 167 | 0.000 |
| LINEAR, centre 1023.5, width 2 | 167 | 0.500 |
| Two windows, the first applied | 170 | 0.460 |
| Modality LUT, 16-bit, first 100 | 173 | 0.494 |
| Modality LUT, first −500, signed pixels | 174 | 0.493 |
| VOI LUT, 12-bit entries | 174 | 0.484 |
| VOI LUT, 8-bit entries | 174 | 0.000 |
| VOI LUT after intercept −1024 (first −1024) | 170 | 0.000 |
| Presentation LUT, 256 × 12-bit | 606 | 0.498 |
| Presentation LUT Shape INVERSE | 173 | 0.496 |
| MONOCHROME1 | 173 | 0.496 |
| **Total** | **2975** | **0 mismatches** |

Inputs are sampled across the full stored range. Every value within 3 of each window edge and each LUT end is included.

### Bugs found

- **12-bit VOI LUTs and Presentation LUTs came out 16× too dark.** They were divided by 65535 rather than 4095 because the descriptor's bit count was rounded up to the 16-bit storage word. `pixel.rs` now keeps the nominal bit count separately in `Lut::nominal`.
- **The Presentation LUT index rounded down at exact entries.** For example, 273/4095 should give entry 17 of 255 but gave 16, because of f64 error. A 1e-9 bias fixes it.

### Oracle caveats

These come from pydicom 3.0.2 and were worked around in the generator:

- **8-bit VOI LUTs:** `apply_voi` indexes them in `uint8`, so an input 256 or more above the first mapped value wraps around. The generator clamps inputs to the LUT's range first, which the standard treats as equivalent.
- **Modality LUT bits:** `apply_modality_lut` fails on any LUT bit count other than 8 or 16, raising `uint12`. That matches PS3.3, which allows only 8 or 16 for Modality LUTs.
- **Windowing range:** `apply_windowing` scales its output to a range that depends on the dataset. The generator windows into a fixed 0..65535 range and divides.
- **Presentation LUT input:** `apply_presentation_lut` rescales its input by the input's own minimum and maximum. Each Presentation LUT case therefore includes VOI outputs of exactly 0 and 1.

## GSDF (PS3.14)

`crates/formats/specular-dicom/src/gsdf.rs` provides:

- `luminance(j)`: equation (1), JND index to cd/m².
- `jnd_index(L)`: equation (2).
- `p_luminance(p, Lmin, Lmax)`: P-values in equal JND steps between a display's measured black and white.

`gsdf_oracle.tsv` holds colour-science 0.4.7's `eotf_DICOMGSDF` and `eotf_inverse_DICOMGSDF` results:

| Quantity | Points | Worst error |
|---|---|---|
| L(j), j = 1..1023 | 1023 | 4.8e-10 relative |
| j(L), L = 0.05..3993 cd/m², log-spaced | 200 | 4.9e-7 JND |

Equation (2) is a fit to the inverse of equation (1). Round trips agree to within 0.1 JND, as the unit test checks. j = 1 gives 0.0500 cd/m².

| p_luminance(P, Lmin, Lmax), 4 display ranges × 65 P-values | 260 | 4.4e-10 relative |

**Display.** The medical setting `display_function = gsdf` shows greys in equal JND steps between the display black and white luminance entered in settings (`display_black`, `display_white`; defaults 0.5 and 350 cd/m², DESIGN.md §7.3). The medical shader maps each P-value through equation (1) between j(Lmin) and j(Lmax), normalises to the display's range and encodes it for the display, assumed to respond as sRGB; the display colour profile still applies. specular-render's `gsdf_display_matches_colour_science` checks the GPU output against the 260 P-values above: the worst is 0.12 JND, limited by the half-float target. Colour maps (pseudo-colour) are not remapped.

## Regenerating

```
tools/quality/setup-venv.sh
$CARGO_TARGET_DIR/venv/bin/python tools/quality/dicom_reference.py          # rewrite the oracles
$CARGO_TARGET_DIR/venv/bin/python tools/quality/dicom_reference.py --check  # verify they are current
cargo test -p specular-dicom --test display_conformance -- --nocapture
```
