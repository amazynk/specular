# Specular — Pathology: stain separation, annotation export and import, label volumes over volumes. English.

## Palette
palette-stain-separation = Separate stains
palette-stain-1 = Show or hide stain 1
palette-stain-2 = Show or hide stain 2
palette-stain-3 = Show or hide stain 3
palette-stain-mode = Stains as light or optical density
palette-stain-estimate = Estimate stains from region
palette-stain-normalise = Normalise stain colours
palette-stain-reference = Use region as stain reference
palette-stain-reset = Published stain vectors
palette-export-annotations = Export annotations…
palette-import-annotations = Import annotations…
palette-label-volume = Overlay label volume…

## Disabled reasons
disabled-not-colour-image = Needs a colour image
disabled-stains-off = Turn on stain separation first
disabled-no-region = Draw a region first
disabled-no-slide-annotations = No annotations, regions or points to export
disabled-no-label-target = Open a volume in multiplanar views, or a medical image

## Stain names
stain-hematoxylin = Haematoxylin
stain-eosin = Eosin
stain-dab = DAB
stain-residual = Residual
stain-set-he = H&E
stain-set-hdab = H-DAB
stain-set-hedab = H&E-DAB

## Stain toasts and legend
stain-off = Stain separation off
# { $set }: H&E, H-DAB or H&E-DAB.
stain-on = Stains: { $set }
stain-shown = { $stain } shown
stain-hidden = { $stain } hidden
stain-mode-light = Stains as light
stain-mode-od = Stains as optical density
# The legend's heading: "H&E · light", "H-DAB · optical density".
stain-legend-light = { $set } · light
stain-legend-od = { $set } · optical density
stain-legend-estimated = Estimated from region
stain-legend-normalised = Normalised
stain-legend-published = Published vectors
# { $pixels }: stained pixels used.
stain-estimated = Stains estimated from { $pixels } pixels
stain-estimate-failed = Cannot estimate stains: { $reason }
stain-estimate-background = the region is mostly background
stain-estimate-one-colour = the region holds one colour
stain-estimate-three = estimate needs H&E or H-DAB
stain-estimate-no-pixels = the region's pixels are not loaded yet
stain-reference-set = Region set as stain reference
stain-normalise-on = Stain colours normalised
stain-normalise-off = Stain colours as scanned
stain-reset-done = Published stain vectors

## Annotation export dialog
annot-export-title = Export annotations
annot-export-format = Format
annot-export-format-coco = COCO JSON
annot-export-format-geojson = GeoJSON (QuPath)
annot-export-format-masks = Label masks
annot-export-output = Masks as
annot-export-output-png = PNG tiles
annot-export-output-tiff = TIFF tiles
annot-export-output-ome = OME-TIFF pyramid
annot-export-region = Region
annot-export-region-slide = Whole slide
annot-export-region-view = Visible area
annot-export-region-drawn = Last drawn region
annot-export-level = Level
# { $level }: 0 for full resolution; { $width } × { $height } its size in pixels.
annot-export-level-value = { $level } · { $width } × { $height }
annot-export-tile = Tile size
annot-export-empty = Empty tiles
annot-export-empty-skip = Skip
annot-export-empty-keep = Write
annot-export-what = { $count ->
    [one] 1 shape
   *[other] { $count } shapes
}, { $classes ->
    [one] 1 class
   *[other] { $classes } classes
}
annot-export-lines = Lines are left out of COCO and masks.
annot-export-export = Export…
annot-export-writing = Writing…
# { $files }: files written.
annot-export-done = { $files ->
    [one] Exported 1 file
   *[other] Exported { $files } files
}
annot-export-failed = Export failed: { $reason }
annot-import-done = { $count ->
    [one] Imported 1 annotation
   *[other] Imported { $count } annotations
}
annot-import-failed = Cannot import: { $reason }
annot-import-skipped = { $count ->
    [one] 1 shape had no usable outline
   *[other] { $count } shapes had no usable outline
}
annot-file-coco = COCO JSON
annot-file-geojson = GeoJSON

## Label volumes
label-volume-file = Label volume
label-volume-table = Colour table
# { $file }: the label volume's file name; { $count }: its labels.
label-volume-loaded = { $count ->
    [one] { $file }: 1 label
   *[other] { $file }: { $count } labels
}
label-volume-failed = Cannot overlay { $file }: { $reason }
label-volume-outside = { $file } does not overlap this volume
label-volume-not-whole = its values are not whole numbers
label-volume-colour = it holds colour, not labels
label-volume-empty = it has no labels
label-volume-too-many = it has too many labels
label-volume-too-large = it is too large to resample
label-volume-degenerate = its orientation is invalid
# A label without a name in the colour table.
label-volume-unnamed = Label { $value }
