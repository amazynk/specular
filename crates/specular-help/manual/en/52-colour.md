---
id: colour
title: Colour
section: reference
contexts: colour
keywords: colour, color, icc, profile, srgb, display p3, adobe rgb, hdr, wide gamut, calibration, rendering intent, ocio, opencolorio, aces, view transform, exr
---
Specular reads each image's colour profile and converts it to your display's profile.

## Colour management {#management}
Settings › Colour sets:

- The display profile. **Automatic** uses the one your system reports.
- How colours outside the display's range are brought in.
- How to show images without a profile.

## HDR {#hdr}
HDR photos show their full brightness on displays that support it.

On other displays, the brightest areas are compressed to fit, so no detail is lost.

## OpenColorIO {#ocio}
EXR, Radiance HDR and PFM images can be shown through an OpenColorIO config.

- Choose the config in Settings › Colour, or set `$OCIO` to it. None is bundled.
- Pick the display your monitor is, the view (such as ACES 1.0 SDR Video) and the images' colour space.
- The view replaces the HDR compression for these images.

Supported: the ACES 1.3 configs' SDR views, matrices, curves, logs, CDL and LUT files (.cube, .spi1d, .spi3d, CLF). HDR views and looks are refused with a reason.

## Checking colours {#checking}
- The [pixel inspector](inspect#pixels) shows exact values.
- The [colour vision](inspect#colour-vision) views show the image as people with colour blindness see it.
