---
id: figures
title: Figures and key images
section: tasks
contexts: figure, key-images, teaching
keywords: figure, export, svg, pdf, tiff, png, scale bar, colour bar, legend, inset, channels, key image, slides, pptx, powerpoint, teaching, anonymise, case
---
## Figures {#figure}

1. Set up the view: window, colour map, measurements.
2. Press [[key:export-figure]].
3. Choose the format, region and parts.
4. Choose **Export**.

The image is exactly what you see, at 1:1, unless you pick a size.

The scale bar comes from the image's calibration. The dialog shows where it comes from.

The colour bar appears when a colour map or window is on.

An inset shows the last rectangle you drew, enlarged, with its outline on the image.

The file holds the provenance: file, calibration, display settings and Specular version. **Provenance** can also draw it under the figure.

SVG and PDF keep the scale bar, text and measurements as vectors.

## Key images {#key-images}

1. Press [[key:toggle-key-image]] on each image you want. Press it again to unmark.
2. Run **Copy key images** to paste them elsewhere as one picture.
3. Or run **Export key images…** for slides (PPTX) or a folder of PNGs.

Each slide has one image. Its notes list the measurements.

Key images last until you close Specular.

## Teaching files {#teaching}

1. Mark the key images.
2. Run **Export teaching file…**.
3. Add case notes.
4. Choose **Export**.

Patient details are left out of titles, notes and file names.

For Radiopaedia or a teaching archive, choose **Teaching case folder** as the format.
The folder holds the images, anonymised DICOM copies, a "For Teaching" manifest and `case.txt` with the notes.
It is always anonymised.

## Teaching quiz {#quiz}

1. Mark the key images, or export a teaching case folder.
2. Run **Teaching quiz**. Without key images, choose a case folder.
3. Each case opens with its measurements and notes hidden.
4. Press Space or R to reveal them.
5. Press → or ← for the next or previous case. Esc ends the quiz.

Burned-in text is blacked out: your masks, else the text Specular finds. An image declared to have burned-in text where none is found is left out.

A folder also gets anonymised DICOM copies.

> Check every image before sharing.
