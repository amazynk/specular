# Specular — Print and contact sheets (DESIGN.md §6.5). English.

palette-print = Print…

print-title = Print
print-images = Images
print-images-current = This image
print-images-folder = Folder ({ $count })
print-images-selection = Selection ({ $count })
print-images-picked = Picks ({ $count })
print-layout = Layout
print-layout-single = One per page
print-layout-sheet = Contact sheet
print-paper = Paper
print-orientation = Orientation
print-orientation-portrait = Portrait
print-orientation-landscape = Landscape
print-margins = Margins
print-size = Image size
print-size-fit = Fit to page
print-size-fill = Fill page
print-size-exact = Exact width
print-size-scale = To scale
print-width = Width
# Printing a drawing to scale: as the drawing states it, or a ratio such as 1:50.
print-scale = Scale
print-scale-as-drawn = As drawn
# Beside the scale bar on the printout. $scale is a ratio (1:50); $length the bar's length (2 m).
print-scale-note = { $name } at { $scale }. The bar is { $length }.
# When a sheet is larger than the paper it prints over several pages.
print-scale-tile = Page { $n } of { $count } (row { $row }, column { $column })
print-columns = Columns
print-rows = Rows
print-header = Header
print-on = On
print-off = Off
print-captions = Captions
print-captions-off = None
print-captions-name = File name
print-captions-name-date = Name and date
print-units = Units
print-units-cm = Centimetres
print-units-in = Inches
print-resolution = Resolution
print-dpi = { $dpi } dpi
print-copies = Copies
print-page = Page { $page } of { $pages }
print-previous-page = Previous page
print-next-page = Next page
print-save-pdf = Save as PDF…
print-print = Print…
print-preparing = Preparing the preview…
print-writing = Writing page { $page } of { $pages }…
print-saved = Saved { $file }
print-missing = { $count ->
    [one] One image could not be read and is shown as a grey box.
   *[other] { $count } images could not be read and are shown as grey boxes.
}
print-sent = Sent to the printer
print-failed = Could not print: { $reason }
print-save-failed = Could not save the PDF: { $reason }
print-unavailable = Printing is not available here. Save as a PDF instead.
