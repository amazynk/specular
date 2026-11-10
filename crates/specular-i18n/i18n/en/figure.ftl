# Figures, key images and teaching files.

## Command palette
palette-export-figure = Export figure…
palette-export-figure-help = Save the view as a figure with a scale bar, colour bar and labels.
palette-toggle-key-image = Mark key image
palette-toggle-key-image-help = Mark this image as a key image, or unmark it.
palette-copy-key-images = Copy key images
palette-copy-key-images-help = Copy the key images as one picture.
palette-export-key-images = Export key images…
palette-export-key-images-help = Save the key images as slides or a folder of PNGs.
palette-export-teaching-file = Export teaching file…
palette-export-teaching-file-help = Save the key images without patient details, for teaching.

## The figure dialog
figure-dialog-title = Export figure
figure-format = Format
figure-format-png = PNG, 16-bit
figure-format-tiff = TIFF, 16-bit
figure-region = Region
figure-region-visible = Visible area
figure-region-whole = Whole image
figure-size = Size
figure-size-pixels = Image pixels (1:1)
# $width is a width in pixels.
figure-size-width = { $width } px wide
figure-scale-bar = Scale bar
figure-scale-bar-none = No calibration
figure-colour-bar = Colour bar
figure-colour-bar-none = No colour map or window
figure-inset = Inset
figure-inset-rectangle = Last rectangle
figure-inset-none = Draw a rectangle first
figure-channels = Channels
figure-channels-merged = Merged
figure-channels-split = Split, and merged
figure-channels-one = One channel
figure-measurements = Measurements
# $count is the number of measurements.
figure-measurements-on = { $count ->
    [one] On ({ $count })
   *[other] On ({ $count })
}
figure-measurements-none = None
figure-title-row = Title
figure-provenance = Provenance
figure-provenance-metadata = In the file
figure-provenance-drawn = In the file and drawn
figure-file = File
figure-choose = Choose…
figure-on = On
figure-off = Off
# $source is where the value comes from (a calibration, a colour map).
figure-on-from = On · { $source }
figure-export = Export
figure-working = Exporting…
figure-not-full-res = Full-resolution pixels are not loaded. Zoom to { PERCENT(100) } or pick a size.
figure-channel-red = Red
figure-channel-green = Green
figure-channel-blue = Blue
# $map is a colour map's name; $lo and $hi the values at its ends; $unit their unit.
figure-legend-map = { $map }, { $lo } to { $hi } { $unit }
# $lo and $hi are the window's ends; $unit their unit (HU).
figure-legend-window = Window { $lo } to { $hi } { $unit }
# $preset is a channel-colour preset's id.
figure-display-channels = Channel colours { $preset }
# $channel is the channel shown (red, green, blue, alpha, luma).
figure-display-channel = Channel { $channel }
figure-display-hilo = HiLo marks
figure-display-as-stored = As stored
# $value is the calibration (source and spacing).
figure-caption-calibration = Calibration: { $value }
# $value is the display settings.
figure-caption-display = Display: { $value }
figure-not-image = Figures are made from an image in the viewer.
figure-not-ready = The image is still loading.
# $width and $height are in pixels.
figure-too-large = Too large: { $width } × { $height } px. Pick the visible area or a size.

## Key images
figure-keys-title = Export key images
figure-teaching-title = Export teaching file
figure-keys-images = Images
# $count is the number of key images.
figure-keys-count = { $count ->
    [one] { $count } key image
   *[other] { $count } key images
}
figure-keys-pptx = Slides (PPTX)
figure-keys-folder = Folder of PNGs
figure-keys-preset = Preset
figure-keys-standard = Standard
figure-keys-teaching = Teaching file
figure-keys-overlays-on = Drawn, and in the notes
figure-keys-masks = Burned-in text
figure-keys-masks-auto = Blacked out
figure-keys-notes = Case notes
figure-keys-destination = Save to
figure-keys-empty = No key images yet. Mark an image as a key image first.
figure-keys-teaching-note = Patient details are left out and burned-in text is blacked out. Check every image before sharing.
figure-keys-none-left = No image could be exported without patient details.
figure-keys-copied = Key images copied
# $count is how many key images there are now.
figure-key-marked = { $count ->
    [one] Marked as key image ({ $count })
   *[other] Marked as key image ({ $count })
}
# $count is how many key images are left.
figure-key-unmarked = { $count ->
    [one] No longer a key image ({ $count } left)
   *[other] No longer a key image ({ $count } left)
}
# $n is a frame or page number.
figure-key-frame = Image { $n }
# $n is the key image's number in the set.
figure-key-n = Key image { $n }

## Results
# $name is the file or folder written.
figure-saved = Saved { $name }
# $name is the file or folder written; $count how many images were left out.
figure-saved-skipped = { $count ->
    [one] Saved { $name }. { $count } image was left out: its burned-in text could not be found.
   *[other] Saved { $name }. { $count } images were left out: their burned-in text could not be found.
}
# $reason says why.
figure-failed = Could not export: { $reason }
# $name is a file that is already in the folder.
figure-folder-taken = { $name } is already in that folder. Choose an empty folder.

## The viewer
figure-key-badge = Key image
