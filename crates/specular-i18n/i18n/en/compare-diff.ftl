# Compare: split, signed difference, flicker, alignment, metrics and the pixel probe
# (DESIGN.md §6.3).

## Actions (palette, cheat sheet, remote control)
palette-compare-with = Compare with…
palette-compare-with-help = Compare this image with another one you choose.
palette-compare-side = Compare side by side
palette-compare-side-help = Show the compared images next to each other.
palette-compare-split = Compare with a split
palette-compare-split-help = Show the second image over the first, cut at a divider you can drag.
palette-compare-split-direction = Turn the split divider
palette-compare-split-direction-help = Put the divider across instead of down, or back.
palette-compare-overlay = Compare as overlay
palette-compare-overlay-help = Show the second image over the first, blended.
palette-compare-difference = Compare difference
palette-compare-difference-help = Show where the images differ; again for the next kind.
palette-compare-flicker = Flicker between compared images
palette-compare-flicker-help = Show the other image at once.
palette-compare-auto-flicker = Flicker automatically
palette-compare-auto-flicker-help = Switch between the two images by itself, or stop.
palette-compare-align = Align compared images
palette-compare-align-help = Line up images of different sizes: top-left, centred, scaled, or found automatically.
palette-compare-metric-space = Switch metric space
palette-compare-metric-space-help = Measure in linear light or in encoded sRGB.
palette-compare-whole-image = Measure the whole image
palette-compare-whole-image-help = Measure the whole image instead of what is in view, or back.
palette-compare-probe = Compare pixel probe
palette-compare-probe-help = Show both images' values under the pointer.
palette-compare-link = Link or unlink compared images
palette-compare-link-help = Zoom and pan each of two images on its own, or together again.
palette-compare-close = Close compare
palette-compare-close-help = Go back to the gallery.
disabled-compare-closed = Open compare first
disabled-compare-not-two = Works with two images

## Cheat sheet
cheatsheet-compare-split = Split or side by side
cheatsheet-compare-split-direction = Divider across or down
cheatsheet-compare-auto-flicker = Flicker automatically
cheatsheet-compare-align = Next alignment
cheatsheet-compare-space = Metric space
cheatsheet-compare-whole = Whole image or in view
cheatsheet-compare-probe = Pixel probe
cheatsheet-compare-link = Link or unlink two images

## Settings
settings-gallery-compare-metric-space = Compare metric space
settings-gallery-compare-metric-space-help = Where compare measures MAE, MSE, PSNR and SSIM. Linear light is what the screen shows. sRGB matches other tools on 8-bit files.
settings-gallery-compare-metric-space-linear = Linear light
settings-gallery-compare-metric-space-srgb = Encoded sRGB
settings-gallery-compare-flicker-limit = Fastest auto-flicker
settings-gallery-compare-flicker-limit-help = Image changes per second at most. Above 3, flashing can trigger seizures.

## Toolbar and captions
compare-split = Split
compare-split-down = Divider down
compare-split-across = Divider across
compare-link = Linked
compare-auto-flicker = Auto
compare-flicker-rate = { $rate } changes a second
compare-gain = Gain ×{ $gain }
compare-probe = Probe
compare-kind-signed = Signed error
compare-kind-on-image = Error over image
compare-align-top-left = Top-left
compare-align-centre = Centred
compare-align-scaled = B scaled to A
compare-align-auto = Auto align
compare-space-linear = Linear light
compare-space-srgb = sRGB
compare-space-help-linear = Measured in linear light, as displayed after tone mapping. PSNR here is not the usual 8-bit sRGB PSNR.
compare-space-help-srgb = Measured on encoded sRGB values, as other tools do for 8-bit files. HDR above SDR white counts as white.
compare-space-help-common = ΔE2000 is always CIELAB (D65), SDR white as L* 100, HDR clipped. Images larger than { $size } pixels are measured from a { $size }-pixel copy.
compare-region-view = In view
compare-region-whole = Whole image
compare-ssim = SSIM { $ssim }
compare-ssim-small = SSIM needs 11 pixels
compare-delta-e = ΔE { $mean } ({ PERCENT(95) }: { $p95 }, max { $max })
compare-delta-e-hdr = HDR clipped for ΔE
compare-aligned-centre = Centred, { $width } × { $height } compared
compare-aligned-scaled = B scaled { $scale }×, { $width } × { $height } compared
compare-aligned-auto = Aligned { $angle }° { $scale }×, { $width } × { $height } compared
compare-aligning = Aligning…
compare-align-failed = Couldn’t align these images. Back to top-left.
compare-split-label = { $b } over { $a }, divider at { PERCENT($percent) }
compare-showing-which = Showing { $which }: { $name }
compare-auto-flicker-on = Flickering, { $rate } changes a second
compare-auto-flicker-off = Flicker stopped
compare-linked = Images linked
compare-unlinked = Images unlinked: each pans on its own
compare-with-title = Compare with
compare-probe-delta-e = ΔE2000
compare-probe-a11y = A: { $a }. B: { $b }. ΔE2000 { $delta }.
compare-a11y-divider = Split divider
compare-a11y-space = Metric space
compare-a11y-align = Alignment
