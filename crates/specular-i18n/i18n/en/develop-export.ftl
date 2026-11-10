# Specular — Develop export. English.
# Exports are new files; the photo and its edit are never changed.

palette-develop-export = Export…
palette-develop-export-help = Write the edited photo, or several, as new files.

## The dialog
develop-export-title = Export
develop-export-start = Export
develop-export-preset = Preset
develop-export-preset-custom = Custom
develop-export-preset-jpeg = JPEG, full size
develop-export-preset-web = Web, 2048 px
develop-export-preset-tiff16 = 16-bit TIFF for print
develop-export-preset-exr = OpenEXR, linear
develop-export-save-preset = Save preset
develop-export-preset-saved = Saved as preset “{ $name }”.
develop-export-preset-exists = This is already the preset “{ $name }”.
develop-export-delete-preset = Delete preset
develop-export-delete-preset-ask = Delete “{ $name }”?
develop-export-delete-preset-confirm = Delete the preset “{ $name }”? Press Delete again to delete it. Files already exported stay.
develop-export-preset-deleted = Deleted the preset “{ $name }”. Files already exported stay.
develop-export-preset-name = { $format } · { $space } · { $size }
develop-export-size-full = full size
develop-export-format-exr = OpenEXR
develop-export-depth-half = 16-bit float
develop-export-depth-float = 32-bit float
develop-export-space = Colour space
develop-export-space-srgb = sRGB
develop-export-space-display-p3 = Display P3
develop-export-space-rec2020 = Rec. 2020
develop-export-space-adobe-rgb = Adobe RGB (compatible)
# HDR output of an export. PQ and HLG are HDR files; a gain map is an SDR photo that also shows in HDR where the screen can.
develop-export-hdr = HDR
develop-export-hdr-off = Off (SDR)
develop-export-hdr-pq = PQ
develop-export-hdr-hlg = HLG
develop-export-hdr-gain-map = Gain map

## Progress and the end
develop-export-progress = Exporting { $done } of { $total }
develop-export-progress-eta = Exporting { $done } of { $total } · about { $time } left
develop-export-done = { $count ->
    [one] Exported { $count } image
   *[other] Exported { $count } images
}
develop-export-stopped = Stopped after { $count }
develop-export-running = An export or conversion is already running.
develop-export-row = Export
develop-export-row-detail = { $done } of { $total }
develop-export-starting = Starting
develop-export-no-gpu = Export needs a graphics device, and none could be opened.
