---
id: medical-compare
title: Comparing series
section: modes
keywords: compare, linked, link, sync, synchronise, reference lines, cursor, prior, viewport, layout, frame of reference, hanging
---
Shows several series of a folder or disc side by side. Open a series first.

> Specular is not a medical device. Do not use it for diagnosis.

## Viewports {#layout}
- [[key:compare-series]]: opens or closes the viewports
- [[key:compare-layout]]: next layout, up to 3 × 3
- [[key:next-viewport]], [[key:prev-viewport]]: picks the active viewport; a click does too

Drag a series from the strip below the viewports onto a viewport. A click on it shows it in the active viewport.

With the Studies browser open, drag a series out of it onto a viewport, or press Enter to show it in the active one.

## Scrolling together {#sync}
Series with the same frame of reference scroll together. Each viewport goes to the slice at the same position, whatever the slice thickness or gantry tilt.

A viewport says when that position falls in a gap or outside its series.

Crossing planes, such as axial and sagittal, do not scroll together. They show reference lines.

- [[key:sync-window]]: window/level for the active viewport only, for the same modality, or for all
- [[key:sync-zoom]]: the same size in millimetres in every viewport; panning moves them all

## Reference lines and the 3D cursor {#lines}
- [[key:reference-lines]]: shows where the active slice, and its thickness, cuts the other viewports

Shift+click a point. The other viewports go to the slice holding it and mark it.

## Priors {#priors}
A prior study usually has another frame of reference, so it does not follow.

1. Scroll both viewports to the same anatomy.
2. Press [[key:align-series]].

They now scroll together. The alignment is kept for this pair of studies until Specular closes.

- [[key:link-series]]: how the active viewport follows: by frame of reference, by patient position as stored, or not at all
