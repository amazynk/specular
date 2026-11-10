---
id: compare
title: Compare
section: modes
contexts: compare
keywords: compare, side by side, split, swipe, difference, flicker, overlay, two images, synchronised, psnr, ssim, delta e, align
---
Compare shows up to four images side by side. Zoom and pan stay in step.

## Starting a comparison {#start}
1. In the [gallery](gallery), select two to four images.
2. Press C, or choose **Compare** in the palette.

From the viewer, choose **Compare with…** in the palette and pick the other image.

From a terminal: `specular --compare A B`. Up to four files; `--compare-mode split` starts in a view.

## Ways to compare {#views}
- **Side by side**: every image at the same zoom and position.
- **Split**: the second image over the first, cut at a divider.
- **Overlay**: one image over another, blended.
- **Flicker**: one image, then the other, in turn.
- **Difference**: where the images differ, with measurements.

Channel views, such as red only or brightness only, apply to all images at once. See [Inspecting images](inspect).

With two images side by side, L unlinks them: each pans on its own, for shots that do not line up. L again links them.

## Split {#split}
Drag the divider, or press [ and ] to move it.
H turns the divider across, and back.
The first image is on the side you start reading from.

## Overlay and flicker {#overlay}
[ and ] blend the images.

T shows the other image. Shift+T switches by itself; press it again to stop.
The corner says which image shows, A or B.

Auto-flicker switches at most 3 times a second.
Settings › Gallery raises the limit. Fast flashing can trigger seizures.

## Difference {#difference}
D shows the difference, then its next kind:
- **Absolute error**, **Squared error** and **Relative error**: false colour from black (no difference) to pale yellow.
- **Signed error**: blue where the first image is darker, red where it is lighter.
- **Error over image**: the error in false colour over the first image, dimmed.

[ and ] change the gain: how much the difference is amplified. The bar shows it as "Gain ×16".

## Different sizes {#sizes}
A cycles how images of different sizes line up:
- **Top-left**: pixel for pixel from the top-left corner.
- **Centred**: pixel for pixel from the centre.
- **B scaled to A**: the second image resized to the first. For an export against its original.
- **Auto**: found from the pictures, with the turn and scale it found. Unrelated images go back to top-left.

Measurements cover only where the images overlap. The caption gives that size.

## Measurements {#metrics}
The difference view measures what is in view. Shift+M measures the whole image.
- MAE, MSE and PSNR: average error and peak signal-to-noise ratio.
- SSIM: structural similarity, 1 for identical images. An 11-pixel Gaussian window (σ 1.5), as published by Wang et al.
- ΔE2000: colour difference, average, 95th percentile and largest. Below about 1 is invisible.

M switches the metric space:
- **Linear light**: as displayed, after tone mapping. The default.
- **sRGB**: encoded values, as other tools measure 8-bit files.

ΔE is always in CIELAB (D65), with SDR white as L* 100. Brighter HDR values count as white.

Images larger than 2048 pixels are measured from a 2048-pixel copy. The caption says so.

## Pixel probe {#probe}
P shows both images' values under the pointer: position, file value, displayed sRGB and linear light, and their ΔE2000.
Hold Alt to freeze it while you move away.

## Keys {#keys}
::: keys compare
