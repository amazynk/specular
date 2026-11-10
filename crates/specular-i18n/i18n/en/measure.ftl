# The measurement tool (lengths, angles, regions and their statistics over any image).

palette-measure = Measure
palette-measure-copy-csv = Copy measurements as CSV
palette-measure-export = Export measurements…
toast-copied-measurements = Measurements copied as CSV

measure-tool-title = Measure
measure-tool-list = Measurements
measure-tool-count = { $count ->
    [0] No measurements
    [one] One measurement
   *[other] { $count } measurements
}
measure-tool-empty = Pick a tool and draw on the image.

measure-tool-point = Point
measure-tool-line = Line
measure-tool-polyline = Polyline
measure-tool-angle = Angle
measure-tool-rect = Rectangle
measure-tool-ellipse = Ellipse
measure-tool-polygon = Polygon
measure-tool-freehand = Freehand
measure-tool-calibrate = Calibrate
measure-tool-spot = Spot

measure-tool-undo = Undo
measure-tool-delete = Delete
measure-tool-copy = Copy CSV
measure-tool-export = Export…
measure-tool-import = Import…
measure-tool-done = Done
measure-tool-unit = Unit: { $unit }
measure-tool-apply = Apply

measure-tool-hint-line = Drag, or click two points. Arrows move the crosshair, Space places a point.
measure-tool-hint-angle = Click three points; the second is the vertex.
measure-tool-hint-rect = Drag, or click two corners.
measure-tool-hint-ellipse = Drag, or click two corners of its box.
measure-tool-hint-polygon = Click the corners; Enter or a click on the first corner closes it.
measure-tool-hint-calibrate = Draw a line over something of known length.
measure-tool-hint-spot = Click a point to read its temperature.
measure-tool-known-length = Known length: { $value } { $unit } (type the number, U changes the unit, Enter applies)
measure-tool-calibrated = Calibrated: one pixel is { $size } { $unit }
measure-tool-bad-length = Type a length greater than zero.

measure-tool-item = { $n }. { $shape }
measure-tool-area = Area { $value }
measure-tool-mean = Mean { $mean } ± { $sd }
measure-tool-means = Mean { $values }
measure-tool-range = Range { $min } – { $max } over { $count } px
measure-tool-pixels = Over { $count } px
measure-tool-stats-pending = Reading pixels…
measure-tool-stats-none = No pixel values here
measure-tool-approximate = From a reduced copy (approximate)

measure-tool-deleted = Measurement deleted
measure-tool-undone = Undone
measure-tool-nothing = No measurements on this image
measure-tool-exported = Measurements saved to { $name }
measure-tool-export-failed = Could not save the measurements: { $reason }
measure-tool-imported = { $count ->
    [one] One measurement imported.
   *[other] { $count } measurements imported.
}
measure-tool-skipped = { $count ->
    [one] One shape could not be read.
   *[other] { $count } shapes could not be read.
}
measure-tool-import-failed = Could not read the measurements: { $reason }
measure-tool-too-large = The file is too large.
# Saving measurements as DICOM SR (.dcm) from an image that is not a DICOM file.
measure-tool-sr-not-dicom = DICOM SR needs a DICOM image. Save as CSV or GeoJSON instead.

measure-source-uncalibrated = Pixels (not calibrated)
# Where a calibration came from: DICOM's Pixel Spacing attribute (0028,0030), the spacing in
# the patient. Translate as the attribute is known in the locale's medical imaging software.
measure-source-dicom-pixel-spacing = DICOM Pixel Spacing
measure-source-dicom-calibrated = DICOM calibrated spacing
measure-source-dicom-imager-pixel-spacing = DICOM detector spacing (at the detector)
measure-source-dicom-magnification = DICOM spacing corrected for magnification
measure-source-dicom-ultrasound-region = DICOM ultrasound region
measure-source-tiff-resolution = TIFF resolution
measure-source-exif-resolution = EXIF resolution
measure-source-ome-physical-size = OME physical size
measure-source-fits-wcs = FITS world coordinates
measure-source-vendor-tag = Vendor metadata
measure-source-drawing-units = Drawing units
measure-source-drawing-scale = From the drawing scale
# A drawing's own scale. $scale is a ratio as written on drawings (1:50) or the file's own
# text (1 in = 4 ft); $viewport is the number of the view on the sheet.
measure-source-drawing-scale-ratio = From the drawing (scale { $scale })
measure-source-drawing-scale-viewport = From the drawing (scale { $scale }, viewport { $viewport })
measure-source-map-scale = Map scale at the centre (approximate; Maps measures on the Earth)
measure-source-user = Your calibration
