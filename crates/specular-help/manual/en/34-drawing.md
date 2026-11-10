---
id: drawing
title: Drawings and plans
section: modes
contexts: drawing
keywords: dxf, dwg, cad, shx, shape font, big font, plan, drawing, scale, to scale, viewport, measure, print, markup, mark up, cloud, revision cloud, stamp, annotation, xfdf, compare, overlay, revision, layers, paper, lineweights, svg, pdf, hpgl, gerber, excellon, drill, kicad, odb++, odb, ipc-2581, ipc2581, pcb, landxml, survey, tin, surface, contour, alignment, station, profile, board, schematic, vector, spectrum, spectra, jcamp, spc, mzml, mzxml, nmr, bruker, plot
---
CAD, plotter and vector drawings, circuit boards and spectra stay sharp at any zoom.

Drawing commands are unavailable for other images.

## Layers {#layers}
- [[key:drawing-layers]]: lists the layers; click one to show or hide it
- [[key:drawing-all-layers]]: shows every layer again

## Paper and lines {#paper}
- [[key:drawing-paper]]: white sheet or dark background
- [[key:drawing-lineweights]]: real line widths or all thin

## SHX fonts {#shx}
- Text in an SHX style draws with that font's own shapes when the font is found.
- Fonts are looked for in the drawing's folder, then in Settings › Drawings › SHX font folders.
- Specular has no SHX fonts of its own; add the folder where yours are.
- A font not found is drawn with a similar font. Info › Substituted fonts names it and its styles.

## Measuring and printing to scale {#scale}
- PDF drawings with a scale per view and DXF or DWG drawings measure in the drawing's own units.
- Each measurement uses the view under it. Its label names the scale and the view.
- Values are written the way the drawing writes them, such as 5 ft 3 1/2 in.
- A layout with its own plot scale, such as 1:50, measures and prints at that scale.
- Calibrating from a known length replaces the drawing's scale for this page.
- To print to scale, choose Image size: To scale in the print dialog.
- Scale: As drawn keeps the drawing's scale, or pick another, such as 1:100.
- A sheet larger than the paper prints over several pages.
- Each page has a scale bar and a note. Measure the bar to check the printer did not resize the page.

## Marking up drawings {#markup}
- [[key:markup]]: opens the markup bar. It works on PDF sheets, DXF drawings and images.
- Tools: cloud, text, arrow with a label, stamp, length, area and photo pin.
- Drag a rectangle for a cloud, or click its corners and press Enter.
- Stamps say Approved, Rejected, Reviewed or your own words, with the date and your name (Settings › General › Your name).
- Lengths and areas use the drawing's scale, as the measuring tool does. Add measurements turns that tool's measurements into markup.
- A photo pin keeps the photo's path; the photo is not copied. Select the pin and click it again, or press Enter, to open the photo.
- Select: click an item, drag to move it, Enter to edit its text, Delete to delete it (it asks first).
- Text takes several lines: Shift+Enter starts a new one.
- Undo and redo: Ctrl+Z and Ctrl+Shift+Z.
- Markup is saved next to the file as you work: plan.pdf.markup.xfdf for a PDF, photo.jpg.markup.json for others. The drawing itself is never changed.
- Acrobat and other PDF tools can import the .xfdf file.
- Export PDF…: a copy of the PDF with the markup as PDF annotations, at the same size and scale. An image exports as a PDF page with its markup.
- Export PNG…: the page with its markup, at the size it is shown.

## Comparing sheets {#compare}
- [[key:compare-sheets]]: compares this sheet with another revision.
- In a PDF with several pages, the next page is the other sheet to start with. Other sheet… picks a file: a PDF or a scan.
- The sheets are aligned automatically. Align tries again. If that fails, use Pick 2 points: click a point on this sheet, then the same point on the other, twice.
- Overlay: lines only on this sheet are red, only on the other cyan, on both black.
- Difference: darker where the sheets differ.
- Fade: a slider between the two sheets. Blink switches between them.
- Changed regions are outlined. Cloud N changes adds them to the markup as clouds.

## Circuit boards {#boards}
- A folder or ZIP of a board's Gerber and drill files opens as one board.
- Pages: every layer stacked, then the top and the bottom in board colours.
- Each file is a layer of the layers panel.
- An ODB++ job (its folder, or a .tgz, .tar or .zip of it) opens the same way.
- An IPC-2581 file (.xml or .cvg) opens the same way.
- ODB++ and IPC-2581 boards also show components with their reference designators, and their text.
- An ODB++ job with several steps (a board and its panel) has these pages for each step.
- One Gerber, drill or KiCad file opens alone.
- A KiCad schematic has a page per sheet.
- The Info panel shows the board size and each file's role.

## Survey data {#survey}
- A LandXML file opens as a plan: its surfaces coloured by height, with contour lines.
- Parcels and survey points are drawn on the plan, with their names.
- Page 2 shows the surfaces and alignments in 3D. Turn and zoom as with any 3D model.
- Alignments show their stations; spirals are drawn as true clothoids.
- Each alignment with profiles has a chart page: stations across, elevations up.
- The Info panel shows the coordinate system, each surface's areas and volume, and each alignment's length.

## Spectra {#spectra}
- JCAMP-DX, SPC, mzML, mzXML and Bruker NMR folders open as plots.
- Zoom and pan as in a drawing; the axes follow.
- The values under the pointer show at the top right.
- mzML: page 1 is the run's total ion current, then a page per spectrum.
- mzML and mzXML files compressed with gzip (.mzML.gz) open too.
- Bruker NMR without a processed spectrum: Specular transforms the FID and phases it automatically. The Info panel says how.

## Keys {#keys}
::: keys drawing
