# Inspection probes in the measurement tool (NDT): radiograph evaluation and crack widths.

ndt-tool = Inspect
ndt-probe = Probe: { $name }
ndt-probe-profile = Line profile
ndt-probe-snr = SNR
ndt-probe-duplex-iqi = Duplex-wire IQI
ndt-probe-wire-iqi = Single-wire IQI
ndt-probe-crack = Crack width
ndt-probe-density = Density point

ndt-hint-profile = Drag a line to plot the grey values along it. [ and ] change the probe.
ndt-hint-snr = Drag a region in a plain area. It grows to at least 20 × 55 pixels.
ndt-hint-duplex-iqi = Drag a line across all the wire pairs, along the IQI.
ndt-hint-wire-iqi = Drag a short line across the thinnest wire you can see.
ndt-hint-crack = Click along the crack, then press Enter. Widths are read at 10 points.
ndt-hint-density = Click a point to read its grey value. Type its known density to calibrate.

ndt-refused-needs-scale = Set the scale first: wire sizes need millimetres. Use Calibrate on a known length.
ndt-refused-nothing-found = Nothing stands out of the noise along this line.
ndt-refused-too-small = The image is smaller than the 20 × 55 pixel SNR region.
ndt-refused-no-noise = This region has no noise to measure (all pixels are the same).

ndt-reading = Reading…
ndt-no-pixels = The pixels could not be read.

ndt-profile-range = Grey { $min } to { $max }
ndt-profile-mean = Mean grey { $mean }
ndt-profile-density = Density { $min } to { $max }

ndt-snr = SNR { $value }
ndt-snrn = SNRn { $value } (SRb { $srb } µm)
ndt-snrn-needs-srb = SNRn needs the SRb: measure a duplex IQI or type it.
ndt-snr-stats = Mean { $mean }, σ { $sd }, region { $w } × { $h } px

ndt-duplex-result = First unresolved pair { $pair }: SRb { $srb } µm, unsharpness { $unsharpness }
ndt-duplex-all-resolved = Every pair found is resolved.
ndt-duplex-dips = Dips: { $dips }
# Interpolated values at the 20 % dip (ISO 19232-5:2018).
ndt-duplex-isrb = Interpolated: iSRb { $isrb } µm, unsharpness { $unsharpness }

ndt-wire-result = Wire { $wire }, { $diameter }
ndt-wire-measured = Measured width { $width } (FWHM), about { $diameter } across

ndt-crack-mean = Mean width { $width }
ndt-crack-range = Narrowest { $min }, widest { $max }
ndt-crack-cuts = Measured at { $count ->
        [one] { $count } point
       *[other] { $count } points
    }

ndt-density-value = Grey { $grey }, density { $density }
ndt-density-grey = Grey { $grey }
ndt-density-uncalibrated = No density calibration yet: type known densities at two points.

ndt-ask-density = Known density here: { $value } (Enter applies, Enter on empty skips)
ndt-ask-srb = Basic spatial resolution SRb: { $value } µm (Enter applies, Esc skips)
ndt-bad-density = Type a density from 0 to 10.
ndt-bad-srb = Type the SRb in micrometres, for example 80.
ndt-srb-set = SRb set to { $srb } µm for this image.
ndt-density-one-more = Density point kept. Type the density at one more point.
ndt-density-calibrated = Density calibrated from two points.
ndt-sidecar-failed = Could not save the inspection settings beside the file: { $reason }

# Drone photos on the gallery's map, and KML/KMZ export.
map-height-above-takeoff = { $metres } m above take-off
map-export-kml = Save photos as KMZ or KML…
map-export-kmz-full = Save photos as KMZ with full photos…
map-kml-heading = Heading { $degrees }°
map-kml-height = { $metres } m above take-off
map-kml-nothing = No photos with a position to save.
map-kml-saved = Saved { $count ->
        [one] { $count } photo
       *[other] { $count } photos
    } to { $name }.
map-kml-failed = Could not save the map file: { $reason }

# The inspection report (any photo), and the report's extra outputs.
inspection-report-heading = Inspection report
inspection-report-default-title = Inspection report
inspection-report-folder = Add the other photos in this folder
inspection-report-docx = Also save a Word document (DOCX)
inspection-report-kmz = Also save a KMZ of the photos' places
inspection-report-caption = Photo with the measurements
inspection-report-col-value = Value
inspection-report-col-scale = Scale from
inspection-report-docx-failed = The Word document could not be written.
inspection-report-unreadable = None of the photos could be read.
