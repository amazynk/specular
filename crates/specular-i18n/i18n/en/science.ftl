# Specular — Scientific images (FITS, XISF, DigitalMicrograph, MRC, Gwyddion, Igor): stretch, limits, planes, channels, coordinates. English.

keys-mode-science = Scientific images
cheatsheet-science = Scientific images
cheatsheet-science-look = Stretch and limits
cheatsheet-science-planes = Planes and channels
cheatsheet-science-channels-1-9 = Show or hide channels 1 to 9
cheatsheet-science-channels-10-16 = Show or hide channels 10 to 16
palette-science-stretch = Next stretch
palette-science-limits = Next limits
palette-science-next-plane = Next plane
palette-science-prev-plane = Previous plane
palette-science-next-axis = Step another axis
palette-science-composite = Composite channels in colour
palette-science-channel-1 = Show or hide channel 1
palette-science-channel-2 = Show or hide channel 2
palette-science-channel-3 = Show or hide channel 3
palette-science-channel-4 = Show or hide channel 4
palette-science-channel-5 = Show or hide channel 5
palette-science-channel-6 = Show or hide channel 6
palette-science-channel-7 = Show or hide channel 7
palette-science-channel-8 = Show or hide channel 8
palette-science-channel-9 = Show or hide channel 9
palette-science-channel-10 = Show or hide channel 10
palette-science-channel-11 = Show or hide channel 11
palette-science-channel-12 = Show or hide channel 12
palette-science-channel-13 = Show or hide channel 13
palette-science-channel-14 = Show or hide channel 14
palette-science-channel-15 = Show or hide channel 15
palette-science-channel-16 = Show or hide channel 16
palette-science-grid = Show or hide the coordinate grid
palette-science-reset = Automatic look
palette-science-level = Level height maps (subtract the mean plane)
palette-science-wiggle = Seismic wiggle traces with variable area, over the colours
palette-science-wireframe = Unstructured mesh edges (wireframe), over the colours
palette-science-import-lut = Colour table from a file (ImageJ .lut)…
palette-science-hillshade = Shade the relief of elevation models (hillshade)
palette-science-fft = Power spectrum (FFT) of the plane
palette-science-catalogue = Overlay a source catalogue (VOTable, FITS table, CSV)…
palette-science-clear-catalogues = Remove the overlaid catalogues
# A catalogue overlaid on a sky image: its name (the table's or the file's), how many of its
# sources fall on the image, and how many of those are drawn (at most a set number).
science-catalogue-toast = { $name }: { $count ->
        [one] one source on the image
       *[other] { $count } sources on the image
    }
science-catalogue-capped = { $name }: { $shown } of { $count } sources on the image shown
science-catalogue-none = { $name }: no sources on the image
science-catalogue-failed = Not a catalogue with sky positions: { $name }
science-catalogues-cleared = Catalogues removed
# The file dialog's filter name for catalogue files.
science-catalogue-filter = Source catalogues
# A GW sky map's credible region: the smallest area holding this share of the probability,
# drawn as a contour labelled e.g. "90 %".
science-credible = { PERCENT($percent) }
palette-science-ion-image = Ion image at an m/z (mass spectrometry)…
disabled-not-science = Only for scientific images (astronomy, electron and scanning probe microscopy)
disabled-one-plane = The image has one plane
disabled-one-axis = The image has no other axis to step
disabled-no-channels = The image has no such channel to composite
disabled-no-coordinates = The image has no sky coordinates
disabled-not-height-map = Only for height maps (Gwyddion, Igor)
disabled-not-elevation = Only for georeferenced elevation models of one band
disabled-not-seismic = Only for seismic data (SEG-Y, Seismic Unix)
disabled-not-mass-spec = Only for mass spectrometry images (imzML)
disabled-not-ugrid = Only for unstructured meshes (UGRID, ICON and MPAS NetCDF)
disabled-not-bayer = Only for colour camera raw frames (FITS with a Bayer pattern, SER video)
palette-debayer = Show colour camera frames debayered or raw
palette-bayer-pattern = Next Bayer pattern for this file
# $pattern is four letters such as "RGGB".
science-debayer-on = Debayered ({ $pattern })
science-debayer-off = Raw colour filter array
science-bayer-pattern = Bayer pattern { $pattern }
palette-annotations = Show or hide annotations
disabled-no-annotations = The image has no annotations beside it (NDPA, Aperio or ASAP XML)
science-annotations-on = Annotations shown
science-annotations-off = Annotations hidden
science-stretch-auto = Auto (each colour)
# Stretches: how values between the limits map to grey levels.
science-stretch-linear = Linear
science-stretch-sqrt = Square root
science-stretch-log = Logarithmic
science-stretch-asinh = Asinh
science-stretch-histeq = Histogram equalisation
# Limits: which values show black and white. $percent is e.g. 99.5.
science-limits-zscale = Zscale
science-limits-minmax = Min/max
science-limits-percentile = { PERCENT($percent) }
# Seismic gain: limits symmetric about zero at the $percent percentile of the amplitudes.
science-limits-symmetric = ±{ PERCENT($percent) }
# The display range the file stores (ImageJ's min and max), used until other limits are chosen.
science-limits-file = File range
# Toasts when the look changes.
science-stretch-toast = Stretch: { $stretch }
science-limits-toast = Limits: { $limits }
science-reset-toast = Automatic look
# An imported colour table: $name is its file name.
science-lut-toast = Colour table { $name }
science-lut-failed = Not a colour table: { $name }
# The file dialog's filter name for .lut files.
science-lut-filter = ImageJ colour tables
science-composite-on = Channels composited in colour
science-composite-off = One channel in grey
science-channel-on = { $channel } shown
science-channel-off = { $channel } hidden
# A channel without a name in the file, in the shown/hidden toasts.
science-channel-unnamed = Channel { $number }
science-grid-on = Coordinate grid shown
science-grid-off = Coordinate grid hidden
# X-ray diffraction frames: the same action shows circles of equal resolution (d-spacing).
science-rings-on = Resolution rings shown
science-rings-off = Resolution rings hidden
# A resolution ring's label: its d-spacing in ångström, e.g. "2.5 Å".
science-ring = { $d } Å
# Seismic wiggle traces: drawn once the traces are at least 4 screen pixels apart.
science-wiggle-on = Wiggle traces on (zoom in to see them)
science-wiggle-off = Wiggle traces off
# Unstructured meshes: the edges drawn over the faces; too many to draw in view until the
# user zooms in.
science-wireframe-on = Mesh edges on
science-wireframe-off = Mesh edges off
science-wireframe-dense = Mesh edges on (zoom in to see them)
# Plane levelling (scanning probe height maps): the best-fit plane is subtracted for display.
science-level-on = Plane levelling on
science-level-off = Plane levelling off
# Relief shading of an elevation model. $azimuth is the sun's direction in degrees clockwise
# from north, $altitude its height above the horizon in degrees.
science-hillshade-on = Hillshade on: sun at { $azimuth }°, { $altitude }° up
science-hillshade-off = Hillshade off
# The power spectrum view: the plane's centred 2-D Fourier power, log scaled.
science-fft-on = Power spectrum shown
science-fft-off = Power spectrum hidden
science-axis-toast = Stepping: { $axis }
# Readout lines (bottom corner).
# The value under the pointer, with the values' unit when the file states one.
science-value = Value { $value }
science-value-unit = Value { $value } { $unit }
# A data quality value under the pointer and the numbers of its set bits (flags), e.g.
# "Value 9 (bits 0, 3)".
science-value-flags = Value { $value } (bits { $bits })
# A class value under the pointer and its name, e.g. "Value 4 · Vegetation".
science-value-class = Value { $value } · { $class }
# Sentinel-2 scene classes (L2A SCL, ESA's names) and L1C cloud masks.
s2-class-no-data = No data
s2-class-defective = Saturated or defective
s2-class-dark = Dark area pixels
s2-class-cloud-shadow = Cloud shadows
s2-class-vegetation = Vegetation
s2-class-not-vegetated = Not vegetated
s2-class-water = Water
s2-class-unclassified = Unclassified
s2-class-cloud-medium = Cloud medium probability
s2-class-cloud-high = Cloud high probability
s2-class-cirrus = Thin cirrus
s2-class-snow = Snow or ice
s2-class-clear = Clear
s2-class-opaque-cloud = Opaque clouds
# Several channels under the pointer: "R 12.5  G 3.1  B 0.4".
science-values = { $values }
# Sky position under the pointer, sexagesimal then degrees.
science-radec = RA { $ra }  Dec { $dec }
science-radec-deg = { $ra }°  { $dec }°
# Galactic or ecliptic longitude and latitude.
science-lonlat = l { $lon }°  b { $lat }°
# The file's world coordinates cannot be followed, shown instead of the sky position and of
# the scale bar. $reason is technical and stays untranslated: a FITS projection code
# ("projection TNX"), a distortion ("distortion TPD"), a missing table ("missing table
# WCSDVARR,1"), "unit pc", "unpaired celestial axes" or "pole".
science-wcs-unsupported = WCS not supported: { $reason }
# A B1950 (FK4) image: what is converted between its frame and another. Its own position
# stays B1950 in the readout. ICRS, FK4 and SAMP are names and stay as they are.
science-fk4-markers = Catalogue markers: ICRS → FK4 converted
science-fk4-markers-galactic = Catalogue markers: galactic → FK4 converted
science-fk4-samp = SAMP: FK4 → ICRS converted
# The look: stretch and limits.
science-look = { $stretch } · { $limits }
# Shown under the look while plane levelling is on.
science-levelled = Plane levelled
# Shown under the look while the hillshade is on.
science-hillshaded = Hillshade: sun at { $azimuth }°, { $altitude }° up
# Shown in the readout while the power spectrum is shown.
science-fft = Power spectrum (log₁₀)
# The spectrum's value under the pointer: log₁₀ of the power.
science-fft-power = log₁₀ power { $value }
# Spatial frequency under the pointer along x and y, and its unit ("1/µm", "1/px").
science-fft-frequency = Frequency u { $u }  v { $v } { $unit }
# The period of that frequency: "Period 2.5 nm".
science-fft-period = Period { $period }
# Crystal orientation under the pointer (EBSD): Bunge Euler angles in degrees.
science-euler = φ₁ { $phi1 }  Φ { $phi }  φ₂ { $phi2 }
# The crystal phase under the pointer and its symmetry ("Iron fcc (m-3m)").
science-phase = Phase { $phase } ({ $symmetry })
# Title of the spectrum plot of the pixel under the pointer (an EDS map).
science-spectrum-title = Spectrum at { $x }, { $y }
# Physical position under the pointer (microscopy): "x 1.25 µm  y 3.5 µm".
science-position = x { $x }  y { $y }
# CERN ROOT histograms: the bin under the pointer on one axis. $axis is the axis title
# ("p_{T}", or "x"), $range the bin's label or its edges ("20 GeV – 25 GeV"), $bin its
# number counted from 1.
science-root-bin = { $axis } { $range } (bin { $bin })
# A ROOT graph's point under the pointer.
science-root-point = x { $x }  y { $y }
# Under- and overflow bins (fills outside the axis ranges) hold something but are not shown.
science-root-flow = { $count ->
    [one] { $count } under/overflow bin hidden (sum { $sum })
   *[other] { $count } under/overflow bins hidden (sum { $sum })
}
# Bins of different widths are drawn on an even grid of pixels.
science-root-resampled = Variable-width bins resampled to an even grid
# A flow cytometry plot (FCS) under the pointer: each axis's parameter and its value.
science-cytometry-at = { $x } { $xv }  { $y } { $yv }
# A cytometry histogram under the pointer: its parameter and the value.
science-cytometry-at-x = { $x } { $xv }
# Events in the plot's bin (or the histogram's bar) under the pointer.
science-cytometry-events = { $count ->
    [one] { $count } event
   *[other] { $count } events
}
# The plot's axis scales ($x, $y: Linear, Log, Logicle or Arcsinh).
science-cytometry-scales = x { $x }, y { $y }
science-cytometry-scale-x = x { $x }
# Spillover compensation is applied, from the file's keyword ($keyword, e.g. $SPILLOVER).
science-cytometry-compensated = Compensated ({ $keyword })
science-cytometry-uncompensated = Not compensated
# X-ray diffraction frames: what lies under the pointer. q is the momentum transfer
# (per ångström), d the lattice spacing (ångström), 2θ the scattering angle. The symbols
# stay as they are: "q 0.512 Å⁻¹  d 12.27 Å  2θ 4.56°".
science-scatter = q { $q } Å⁻¹  d { $d } Å  2θ { $tth }°
# Seismic readout: the trace under the pointer in a 3-D volume ($inline, $crossline the
# header numbers) or a 2-D line; $trace counts from 1 in file order, $cdp is the CDP number.
science-seismic-cell = Inline { $inline }  Crossline { $crossline }
science-seismic-trace = Trace { $trace }  CDP { $cdp }
# A cell of a 3-D volume with no trace in the file.
science-seismic-gap = No trace
# Two-way time (ms) or depth of the row under the pointer.
science-seismic-time = Time { $time } ms
science-seismic-depth = Depth { $depth } { $unit }
# The trace's CDP (or source) X and Y with the coordinate scalar applied.
science-seismic-xy = X { $x }  Y { $y }
# Unstructured mesh readout: the face (cell) under the pointer, counting from 0 as in the
# file's connectivity, and the node nearest to the pointer when the values sit on nodes.
science-mesh-face = Face { $face }
science-mesh-face-node = Face { $face }  Node { $node }
# The pointer lies between the mesh's faces (land in an ocean mesh, outside its boundary).
science-mesh-outside = Off the mesh
# Position on an axis past the image: "Plane 3 of 40", or with its world value.
science-plane = { $axis } { $index } of { $count }
science-plane-world = { $axis } { $index } of { $count } ({ $world })
science-axis-unnamed = Plane
science-axis-extension = Image
science-axis-channel = Channel
# A column of a HEALPix sky map (probability, distance…) shown as the image.
science-axis-field = Field
# X-ray event lists binned into an image: the bin size, in the event list's sky pixels
# ("Bin 2 of 4 (8 px)"), and the energy band of the events counted.
science-axis-bin = Bin
science-bin-size = { $size } px
science-axis-band = Energy
science-band-all = all energies
# Complex-valued arrays (NumPy, MATLAB, Zarr, TDMS): which part of each value is shown.
science-axis-part = Part
science-part-magnitude = magnitude
science-part-real = real part
science-part-imaginary = imaginary part
science-part-phase = phase (radians)
# Complex radar (SAR) images: intensity in decibels.
science-part-db = intensity (dB)
# The frames of a thermal sequence (FLIR SEQ/CSQ), in time.
science-axis-frame = Frame
science-readout = Value and coordinates
# Accessible names of the plane sliders.
science-sliders = Planes
# The scale bar's length.
science-scale-arcsec = { $value }″
science-scale-arcmin = { $value }′
science-scale-deg = { $value }°
# A physical length, e.g. "20 nm" (electron microscopy images).
science-scale-length = { $value } { $unit }
# A length along a named axis of a plot, e.g. "20 GeV (p_{T})".
science-scale-axis = { $length } ({ $axis })
science-scale-a11y = Scale bar: { $length }
# Mass spectrometry images (imzML). The image is the total ion current (each spot's summed
# intensities) until an m/z is typed; then that ion's image, its intensities summed over
# m/z ± the tolerance. $window is e.g. "760.5851 ± 5 ppm".
science-ion-tic = Total ion current
science-ion-window = Ion image m/z { $window }
# The m/z prompt: $text is what is typed so far (opens on the base peak under the pointer).
science-ion-prompt = Ion image at m/z: { $text }▏
# $tolerance is e.g. "10 ppm" or "0.1 Da"; a tolerance can also be typed after the m/z
# ("760.585 5 ppm"). Tab steps the tolerance; Enter on an empty prompt shows the total ion
# current again.
science-ion-prompt-hint = ± { $tolerance } · Tab tolerance · Enter show · Esc cancel
science-ion-bad = Not an m/z: { $text }
# The spot under the pointer as the file numbers it (from 1).
science-ms-spot = Spot x { $x }  y { $y }
science-ms-spot-z = Spot x { $x }  y { $y }  z { $z }
science-ms-no-spectrum = No spectrum at this spot
# Thermal stills (FLIR radiometric JPEGs and FFF, InfiRay IRG, InfraTec IRB): each pixel's
# value is a temperature.
palette-science-temperature-unit = Temperature unit (°C, °F, K)
palette-science-isotherm = Isotherm above or below the temperature under the pointer
palette-science-thermal-parameters = Emissivity and reflected temperature…
palette-science-export-temperatures = Export temperatures (float TIFF or CSV)…
palette-thermal-visible = Visible photo of a thermal image
disabled-not-thermal = Only for thermal images
disabled-not-radiometric = The file stores temperatures: its parameters cannot be changed
disabled-no-visible-photo = The thermal image has no visible photo
# $unit is "°C", "°F" or "K".
science-temperature-unit-toast = Temperatures in { $unit }
# The isotherm paints every pixel at or above (below) $level, e.g. "36.5 °C", in one colour.
science-isotherm-above = Isotherm above { $level }
science-isotherm-below = Isotherm below { $level }
science-isotherm-off = Isotherm off
# The object parameters prompt, one line per field; $text is what is typed so far (the
# field being edited ends in a cursor).
science-thermal-emissivity-prompt = Emissivity: { $text }
science-thermal-reflected-prompt = Reflected temperature ({ $unit }): { $text }
science-thermal-prompt-hint = Tab next field · Enter recompute · Esc cancel
science-thermal-bad = Not a valid value: { $text }
# $emissivity is e.g. "0.95", $reflected e.g. "20 °C".
science-thermal-applied = Recomputed for emissivity { $emissivity }, reflected { $reflected }
science-thermal-conditions = Emissivity { $emissivity } · reflected { $reflected }
# The save dialog's filter names.
science-thermal-export-tiff = 32-bit float TIFF
science-thermal-export-csv = CSV
science-thermal-export-done = Temperatures saved to { $name }
science-thermal-export-failed = Could not save the temperatures: { $reason }
# An ion image or the temperatures could not be read; $reason says why.
science-values-failed = Couldn’t show the values: { $reason }
# Thermal sequences (FLIR SEQ/CSQ): playback and each measurement over time.
palette-science-thermal-play = Play or pause the thermal sequence
palette-science-thermal-series-csv = Save temperatures over time (CSV)…
disabled-not-thermal-sequence = Only for thermal sequences
thermal-seq-title = Temperatures over time
# $time is seconds from the first frame, e.g. "0.12 s".
thermal-seq-frame = Frame { $frame } of { $frames } · { $time }
thermal-seq-play = Play
thermal-seq-pause = Pause
thermal-seq-playing = Playing
thermal-seq-paused = Paused
thermal-seq-save-csv = Save CSV…
thermal-seq-hint = Add a spot, line or area (M) to plot it over time.
thermal-seq-reading = Reading frame { $done } of { $total }…
thermal-seq-failed = Couldn’t read the frames: { $reason }
thermal-seq-no-series = Nothing to save yet: add a spot, line or area.
# For screen readers: each measurement's value at the frame shown, e.g.
# "1 Spot: 36.4 °C, 2 Rectangle: 30.1 °C".
thermal-seq-chart = Chart: { $values }
# The report (PDF) of the image shown: a thermal report for a thermal image, an inspection report for any other.
palette-thermal-report = Report (PDF)…
thermal-report-heading = Thermal report
thermal-report-default-title = Thermal inspection report
thermal-report-title-label = Title
thermal-report-notes-label = Report notes
thermal-report-photo = Include the visible photo
thermal-report-folder = Add the other thermal images in this folder
thermal-report-save = Save PDF…
thermal-report-busy = Writing the report…
thermal-report-nothing = The temperatures aren’t shown yet.
thermal-report-saved = Report saved to { $name } ({ $count ->
    [one] 1 image
   *[other] { $count } images
})
thermal-report-failed = Couldn’t save the report: { $reason }
# In the PDF.
thermal-report-notes = Report notes
thermal-report-measurements = Measurements
thermal-report-no-measurements = No measurements.
thermal-report-thermal-caption = Thermal image
thermal-report-visible-caption = Visible photo
thermal-report-col-measurement = Measurement
thermal-report-col-min = Min
thermal-report-col-max = Max
thermal-report-col-mean = Mean
thermal-report-col-emissivity = Emissivity
thermal-report-whole-image = Whole image
thermal-report-camera = Camera
thermal-report-lens = Lens
thermal-report-date = Taken
thermal-report-place = Place
thermal-report-frame = Frame
thermal-report-emissivity = Emissivity
thermal-report-reflected = Reflected temperature
thermal-report-distance = Distance
thermal-report-humidity = Humidity
thermal-report-range = Range
thermal-report-format = Format
thermal-report-page = Page { $page } of { $pages }
