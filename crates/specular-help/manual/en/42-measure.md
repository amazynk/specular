---
id: measure
title: How do I measure distances and areas?
section: tasks
contexts: measure
keywords: measure, ruler, distance, length, area, angle, scale, calibrate, csv, export, millimetres, geometry, vet, veterinary, vhs, vlas, norberg, tpa, tplo, distraction, femoral, offset, leg length, acetabular, cephalometric, steiner, downs, ndt, radiograph, weld, diconde, profile, grey value, optical density, snr, snrn, iqi, duplex wire, single wire, srb, isrb, crack, crack width
---
1. Press [[key:measure]].
2. Click the start and end of a length, or drag out a shape.
3. Read the result next to the shape or in the measurements list.

Results are in real units when the file states its scale, as microscope and medical images and engineering drawings do. Otherwise they are in pixels.

For drawings, see [measuring and printing to scale](drawing#scale).

To set the scale yourself, measure something of known length.

On thermal images, areas and lines show their coldest, hottest and mean temperature, and a spot (S) its own.

## Keeping the results {#export}
- [[key:measure-copy-csv]]: copies the measurements as a table for a spreadsheet
- [[key:measure-export]]: saves them to a file

For medical images, see [measuring in medical images](medical#measure).

## Vet, orthopaedic and dental geometry {#geometry}
With the Medical profile on, the bar has Geometry (G).
- Pick a template with its button, or T and Shift+T: VHS, VLAS, Norberg angle, TPA, distraction index, femoral head, femoral offset, leg length, acetabular angle, Steiner, Downs.
- Place each point the bar asks for, in order. Escape starts again.
- The results show in the list with their units and where the scale comes from.
- Drag a placed point to move it. The results follow. Ctrl+Z undoes the move.
- CSV lists each result and each point by name.

You place every point. Nothing is detected or graded.

## Radiographs and weld inspection {#ndt}
Inspect (I) holds the inspection probes. Its second button, or [ and ], picks one.
- Line profile: drag a line. The list plots the grey values along it.
- SNR: drag a region in a plain area. It grows to at least 20 × 55 pixels. SNRn uses the SRb.
- Duplex-wire IQI: drag a line across all the wire pairs. The first pair with a dip under 20 % gives the SRb. The iSRb interpolates the 20 % point between pairs.
- Single-wire IQI: drag a short line across the thinnest wire you see. The list names the wire and its diameter.
- Density point: click a point and type its known density. Two points calibrate grey to density.

IQI probes need millimetres: calibrate on a known length first, unless the file states its scale.

DICONDE and DICOM files give their stored grey values, and densities when they state them.

The scale, the density points, the SRb and the iSRb are kept beside the file in a small `.inspect.json`.

## Crack widths {#crack}
1. Calibrate on a known length, such as a ruler in the photo.
2. In Inspect, pick Crack width.
3. Click along the crack, then press Enter.

The width is read across the crack at 10 points, edge to edge at half the darkness. The list shows the mean, narrowest and widest.

## Inspection report {#report}
[[key:thermal-report]] on any photo saves an inspection report as PDF.
- Each photo gets a page: the photo with the measurements drawn and numbered, a table of them, its facts and place.
- Add the other photos in this folder: one page each, with facts and place.
- Also save a Word document: the same report as a .docx beside the PDF.
- Also save a KMZ: the photos' places, with small copies, beside the PDF.

On thermal images the same key makes the thermal report, with the same choices.
