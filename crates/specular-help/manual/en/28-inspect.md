---
id: inspect
title: Inspecting images
section: modes
contexts: inspect
keywords: histogram, pixel, inspector, channels, clipping, focus peaking, false colour, colour blindness, info, metadata, exif, grid
---
These tools never change the image.

## Information {#info}
- [[key:info]]: information panel: size, format, colour profile, camera settings, location and other metadata
- [[key:hud]]: short summary over the image

## Histogram and clipping {#histogram}
- [[key:histogram]]: how bright the pixels are
- [[key:clipping]]: marks pure white and pure black areas, where detail is lost
- [[key:focus-peaking]]: outlines the sharpest edges, to check focus

## Pixels {#pixels}
- [[key:pixel-inspector]]: exact value of the pixel under the pointer
- [[key:pixel-grid]]: grid between pixels when zoomed in far

## Channels {#channels}
- [[key:channel-red]]: red only
- [[key:channel-green]]: green only
- [[key:channel-blue]]: blue only
- [[key:channel-alpha]]: transparency only
- [[key:channel-luma]]: brightness only
- [[key:false-colour]]: maps brightness to colours
- [[key:next-colour-map]]: chooses the colour map
- [[key:hilo]]: marks the lowest and highest values in colour

Press a channel key again to show all channels.

Single-channel views are unavailable for channels the image lacks, such as transparency in a JPEG, and for greyscale images.

## Colour vision {#colour-vision}
[[key:colour-vision]] shows how the image looks with common colour blindness: protanopia, deuteranopia, tritanopia and achromatopsia.
