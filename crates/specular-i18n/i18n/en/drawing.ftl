# Specular — Drawings (DXF): layers, paper, lineweights, model space and layouts. English.

keys-mode-drawing = Drawings
cheatsheet-drawing = Drawings
palette-drawing-layers = Show or hide the layers panel
palette-drawing-all-layers = Show every layer, or the layers as saved
palette-drawing-paper = Swap dark and light paper
palette-drawing-lineweights = Show or hide lineweights
disabled-not-drawing = Only for drawings (DXF)
# A drawing's page: model space (layouts are shown by their own name).
page-label-model = Model
# Names Specular makes up when the file has none: an HP-GL plotter pen (shown as a layer),
# a page of a plot, an unnamed map layer (numbered from 1), and the single sheet of a map.
cad-pen-name = Pen { $n }
cad-page-name = Page { $n }
cad-layer-name = Layer { $n }
cad-map-name = Map
# A board's views (its Gerber files together): every layer stacked, and its top and bottom
# in board colours.
cad-board-layers-name = Layers
cad-board-top-name = Top
cad-board-bottom-name = Bottom
# A plot's layers: its frame, a trace without a name (numbered from 1), and the contour
# lines of a 2-D spectrum above and below zero.
cad-chart-frame-name = Frame
cad-trace-name = Trace { $n }
cad-contours-positive-name = Positive contours
cad-contours-negative-name = Negative contours
# The page of a survey file (LandXML) showing its surfaces and alignments in 3D.
cad-model-3d-name = 3D model
# The layers panel's title (and its name for screen readers).
drawing-layers-title = Layers · { $shown } of { $total } shown
# Toasts.
drawing-layers-all-toast = Every layer shown
drawing-layers-saved-toast = Layers as saved in the drawing
drawing-paper-dark-toast = Dark paper
drawing-paper-light-toast = Light paper
drawing-lineweights-on = Lineweights shown
drawing-lineweights-off = Lineweights off: thin lines

# Markup on drawings and images (Alt+M): kept in a file next to the drawing, never in it.
palette-markup = Mark up
palette-markup-help = Clouds, text, arrows, stamps, measurements and photo pins, saved next to the file.
palette-compare-sheets = Compare sheets
palette-compare-sheets-help = Overlay another revision of this sheet, aligned, with the changes outlined.
markup-title = Markup
markup-list = Markup on this page
markup-count = { $count ->
    [0] No markup on this page
    [one] 1 item on this page
   *[other] { $count } items on this page
}
markup-empty = Pick a tool and click on the sheet.
markup-item = { $n }. { $kind }
markup-tool-select = Select
markup-tool-cloud = Cloud
markup-tool-text = Text
markup-tool-arrow = Arrow
markup-tool-stamp = Stamp
markup-tool-length = Length
markup-tool-area = Area
markup-tool-pin = Photo pin
markup-hint-select = Click an item to select it. Drag to move it. Enter edits its text.
markup-hint-cloud = Drag a rectangle, or click corners and press Enter.
markup-hint-text = Click where the text goes.
markup-hint-arrow = Drag from the label to the point, or click both.
markup-hint-stamp = Click where the stamp goes.
markup-hint-length = Drag, or click both ends.
markup-hint-area = Click the corners, then press Enter.
markup-hint-pin = Click a point, then choose a photo.
markup-kind-cloud = Cloud
markup-kind-text = Text
markup-kind-arrow = Arrow
markup-kind-stamp = Stamp
markup-kind-length = Length
markup-kind-area = Area
markup-kind-pin = Photo
markup-colour = Colour: { $colour }
markup-colour-red = Red
markup-colour-blue = Blue
markup-colour-green = Green
markup-colour-orange = Orange
markup-colour-magenta = Magenta
markup-colour-black = Black
markup-width = Line: { $width }
markup-width-thin = Thin
markup-width-medium = Medium
markup-width-thick = Thick
markup-stamp = Stamp: { $words }
markup-stamp-approved = Approved
markup-stamp-rejected = Rejected
markup-stamp-reviewed = Reviewed
markup-stamp-custom = Your words
markup-undo = Undo
markup-redo = Redo
markup-delete = Delete
markup-open-photo = Open photo
markup-from-measurements = Add measurements
markup-from-measurements-done = { $count ->
    [one] 1 measurement added as markup
   *[other] { $count } measurements added as markup
}
markup-export-pdf = Export PDF…
markup-export-png = Export PNG…
markup-done = Done
markup-ask-text = Text (Enter to place, Esc to cancel)
markup-ask-label = Label (optional; Enter to place)
markup-ask-stamp = Stamp words (Enter to place)
markup-ask-edit = New text (Enter to save)
markup-ask-new-line = Shift+Enter: new line
markup-added = { $kind } added
markup-moved = Moved
markup-pin-hint = Click the pin again or press Enter to open its photo.
markup-deleted = Deleted
markup-undone = Undone
markup-redone = Redone
markup-delete-ask = Delete { $name }?
markup-delete-detail = It is removed from the markup file. The drawing itself is not changed.
markup-delete-ok = Delete
markup-photo-missing = { $name } is no longer there.
markup-load-failed = Markup not read: { $reason }
markup-save-failed = Markup not saved: { $reason }
markup-sidecar-too-large = the markup file is too large
markup-pdf-unreadable = the PDF could not be read
markup-exported = Saved { $name }
markup-export-failed = Not exported: { $reason }
markup-export-same-file = choose another name; the original is never overwritten
markup-export-encrypted = the PDF is encrypted. Export PNG instead.
markup-export-damaged = the PDF's structure is damaged. Export PNG instead.
sheets-title = Compare sheets
sheets-other = Other sheet…
sheets-other-named = Other: { $name }
sheets-page = Page { $page } of { $count }
sheets-auto = Align
sheets-pick = Pick 2 points
sheets-overlay = Overlay
sheets-difference = Difference
sheets-blend = Fade
sheets-blink = Blink
sheets-slider = Other sheet { PERCENT($percent) }
sheets-clouds = { $count ->
    [0] No changes
    [one] Cloud 1 change
   *[other] Cloud { $count } changes
}
sheets-done = Done
sheets-choose = Choose the other sheet.
sheets-reading = Reading the sheet…
sheets-aligning = Aligning…
sheets-aligned = Aligned: scale { $scale }, turned { $angle }°, fit within { $error } px. { $changes ->
    [0] No changes found.
    [one] 1 change.
   *[other] { $changes } changes.
}
sheets-align-failed = Could not align automatically. Pick 2 points.
sheets-pick-this = Point { $n }: click it on this sheet.
sheets-pick-other = Point { $n }: click the same point on the other sheet.
sheets-pick-same = The two points are the same. Pick 2 points again.
sheets-read-failed = Sheet not read: { $reason }
sheets-failed = That step failed. Try again or pick another sheet.
sheets-legend-this = This sheet only
sheets-legend-other = Other sheet only
sheets-legend-both = Both
sheets-cloud-text = Changed
sheets-clouds-added = { $count ->
    [0] No clouds added
    [one] 1 cloud added to the markup
   *[other] { $count } clouds added to the markup
}
sheets-opened = Compare sheets
sheets-closed = Comparison closed

## Settings › Drawings

settings-section-drawings = Drawings
settings-drawings-shx-folders = SHX font folders
settings-drawings-shx-folders-help = Folders with SHX fonts for DXF and DWG text, separated by ;. The drawing’s own folder is searched first.
# A folder in the SHX font folders setting that does not exist or is not a folder.
settings-error-shx-folder = Not a folder: { $folder }
