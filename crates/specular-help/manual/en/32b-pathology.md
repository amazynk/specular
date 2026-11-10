---
id: pathology
title: Pathology slides
section: modes
contexts: pathology
keywords: pathology, slide, whole slide, wsi, stain, h&e, hematoxylin, haematoxylin, eosin, dab, ihc, colour deconvolution, color deconvolution, macenko, normalisation, normalization, coco, geojson, qupath, label mask, ome-tiff, annotations, export
---
Tools for brightfield slides and other colour microscope images.

## Stains {#stains}
- [[key:stain-separation]]: separates the stains: off, H&E, H-DAB, then H&E-DAB
- [[key:stain-1]], [[key:stain-2]], [[key:stain-3]]: show or hide each stain
- [[key:stain-mode]]: each stain as light, or as its amount (optical density)

On a slide, the channel keys (1 to 3) also show or hide the stains.

The image itself is not changed.

## Stain vectors {#estimate}
- [[key:stain-estimate]]: measures the stains in the last region you drew
- [[key:stain-normalise]]: matches the colours to a reference slide
- [[key:stain-reference]]: makes the last region the reference
- [[key:stain-reset]]: back to the published vectors

Draw the region with the [measuring tools](measure) first.

## Export and import {#export}
[[key:export-annotations]] saves the slide's annotations, and the regions and points you drew:
- COCO JSON: for training data sets
- GeoJSON: opens in QuPath with its classes and colours
- Label masks: PNG or TIFF tiles, or one OME-TIFF pyramid, for a region at a level

[[key:import-annotations]] opens QuPath GeoJSON or COCO files.

Coordinates are full-resolution pixels.
