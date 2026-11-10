# Specular — Scientific images (FITS, XISF): stretch, limits, planes, channels, coordinates. Dansk.

keys-mode-science = Videnskabelige billeder
cheatsheet-science = Videnskabelige billeder
cheatsheet-science-look = Stræk og grænser
cheatsheet-science-planes = Planer og kanaler
# machine-drafted, needs native review
cheatsheet-science-channels-1-9 = Vis eller skjul kanal 1 til 9
cheatsheet-science-channels-10-16 = Vis eller skjul kanal 10 til 16
palette-science-stretch = Næste stræk
palette-science-limits = Næste grænser
palette-science-next-plane = Næste plan
palette-science-prev-plane = Forrige plan
palette-science-next-axis = Gennemløb en anden akse
palette-science-composite = Kombiner kanaler i farve
palette-science-channel-1 = Vis eller skjul kanal 1
palette-science-channel-2 = Vis eller skjul kanal 2
palette-science-channel-3 = Vis eller skjul kanal 3
palette-science-channel-4 = Vis eller skjul kanal 4
palette-science-channel-5 = Vis eller skjul kanal 5
palette-science-channel-6 = Vis eller skjul kanal 6
palette-science-channel-7 = Vis eller skjul kanal 7
palette-science-channel-8 = Vis eller skjul kanal 8
palette-science-channel-9 = Vis eller skjul kanal 9
# machine-drafted, needs native review
palette-science-channel-10 = Vis eller skjul kanal 10
palette-science-channel-11 = Vis eller skjul kanal 11
palette-science-channel-12 = Vis eller skjul kanal 12
palette-science-channel-13 = Vis eller skjul kanal 13
palette-science-channel-14 = Vis eller skjul kanal 14
palette-science-channel-15 = Vis eller skjul kanal 15
palette-science-channel-16 = Vis eller skjul kanal 16
palette-science-grid = Vis eller skjul koordinatgitteret
palette-science-reset = Automatisk udseende
palette-science-level = Udjævn højdekort (træk middelplanet fra)
palette-science-wiggle = Seismiske wiggle-spor med variabelt areal over farverne
# machine-drafted, needs native review
palette-science-wireframe = Kanter i ustrukturerede net (trådmodel) over farverne
palette-science-import-lut = Farvetabel fra en fil (ImageJ .lut)…
palette-science-hillshade = Skyg relieffet i højdemodeller (hillshade)
palette-science-fft = Effektspektrum (FFT) af planet
palette-science-catalogue = Læg et kildekatalog over billedet (VOTable, FITS-tabel, CSV)…
palette-science-clear-catalogues = Fjern de overlagte kataloger
# A catalogue overlaid on a sky image: its name (the table's or the file's), how many of its
# sources fall on the image, and how many of those are drawn (at most a set number).
science-catalogue-toast = { $name }: { $count ->
        [one] én kilde på billedet
       *[other] { $count } kilder på billedet
    }
science-catalogue-capped = { $name }: { $shown } af { $count } kilder på billedet vist
science-catalogue-none = { $name }: ingen kilder på billedet
science-catalogue-failed = Ikke et katalog med himmelpositioner: { $name }
science-catalogues-cleared = Kataloger fjernet
# The file dialog's filter name for catalogue files.
science-catalogue-filter = Kildekataloger
# A GW sky map's credible region: the smallest area holding this share of the probability,
# drawn as a contour labelled e.g. "90 %".
science-credible = { PERCENT($percent) }
palette-science-ion-image = Ionbillede ved en m/z (massespektrometri)…
disabled-not-science = Kun til videnskabelige billeder (astronomi, elektron- og scanningsprobemikroskopi)
disabled-one-plane = Billedet har ét plan
disabled-one-axis = Billedet har ingen anden akse at gennemløbe
disabled-no-channels = Billedet har ingen sådan kanal at kombinere
disabled-no-coordinates = Billedet har ingen himmelkoordinater
disabled-not-height-map = Kun til højdekort (Gwyddion, Igor)
disabled-not-elevation = Kun til georefererede højdemodeller med ét bånd
disabled-not-seismic = Kun til seismiske data (SEG-Y, Seismic Unix)
disabled-not-mass-spec = Kun til massespektrometribilleder (imzML)
# machine-drafted, needs native review
disabled-not-ugrid = Kun til ustrukturerede net (UGRID-, ICON- og MPAS-NetCDF)
disabled-not-bayer = Kun til råbilleder fra farvekameraer (FITS med Bayer-mønster, SER-video)
palette-debayer = Vis farvekamerabilleder debayeret eller rå
palette-bayer-pattern = Næste Bayer-mønster for denne fil
science-debayer-on = Debayeret ({ $pattern })
science-debayer-off = Rå farvefiltermatrix
science-bayer-pattern = Bayer-mønster { $pattern }
# machine-drafted, needs native review
palette-annotations = Vis eller skjul annoteringer
disabled-no-annotations = Billedet har ingen annoteringer ved siden af sig (NDPA, Aperio eller ASAP XML)
science-annotations-on = Annoteringer vist
science-annotations-off = Annoteringer skjult
science-stretch-auto = Automatisk (pr. farve)
# Stretches: how values between the limits map to grey levels.
science-stretch-linear = Lineær
science-stretch-sqrt = Kvadratrod
science-stretch-log = Logaritmisk
science-stretch-asinh = Asinh
science-stretch-histeq = Histogramudligning
# Limits: which values show black and white. $percent is e.g. 99.5.
science-limits-zscale = Zscale
science-limits-minmax = Min/maks
science-limits-percentile = { PERCENT($percent) }
# Seismic gain: limits symmetric about zero at the $percent percentile of the amplitudes.
science-limits-symmetric = ±{ PERCENT($percent) }
# The display range the file stores (ImageJ's min and max), used until other limits are chosen.
science-limits-file = Filens interval
# Toasts when the look changes.
science-stretch-toast = Stræk: { $stretch }
science-limits-toast = Grænser: { $limits }
science-reset-toast = Automatisk udseende
# An imported colour table: $name is its file name.
science-lut-toast = Farvetabel { $name }
science-lut-failed = Ikke en farvetabel: { $name }
# The file dialog's filter name for .lut files.
science-lut-filter = ImageJ-farvetabeller
science-composite-on = Kanaler kombineret i farve
science-composite-off = Én kanal i gråtoner
science-channel-on = { $channel } vist
science-channel-off = { $channel } skjult
# machine-drafted, needs native review
# A channel without a name in the file, in the shown/hidden toasts.
science-channel-unnamed = Kanal { $number }
science-grid-on = Koordinatgitter vist
science-grid-off = Koordinatgitter skjult
# X-ray diffraction frames: the same action shows circles of equal resolution (d-spacing).
science-rings-on = Opløsningsringe vist
science-rings-off = Opløsningsringe skjult
# A resolution ring's label: its d-spacing in ångström, e.g. "2.5 Å".
science-ring = { $d } Å
# Seismic wiggle traces: drawn once the traces are at least 4 screen pixels apart.
science-wiggle-on = Wiggle-spor til (zoom ind for at se dem)
science-wiggle-off = Wiggle-spor fra
# machine-drafted, needs native review
# Unstructured meshes: the edges drawn over the faces; too many to draw in view until the
# user zooms in.
science-wireframe-on = Netkanter til
science-wireframe-off = Netkanter fra
science-wireframe-dense = Netkanter til (zoom ind for at se dem)
# Plane levelling (scanning probe height maps): the best-fit plane is subtracted for display.
science-level-on = Planudjævning til
science-level-off = Planudjævning fra
# Relief shading of an elevation model. $azimuth is the sun's direction in degrees clockwise
# from north, $altitude its height above the horizon in degrees.
science-hillshade-on = Hillshade til: sol ved { $azimuth }°, { $altitude }° oppe
science-hillshade-off = Hillshade fra
# The power spectrum view: the plane's centred 2-D Fourier power, log scaled.
science-fft-on = Effektspektrum vist
science-fft-off = Effektspektrum skjult
science-axis-toast = Gennemløber: { $axis }
# Readout lines (bottom corner).
# The value under the pointer, with the values' unit when the file states one.
science-value = Værdi { $value }
science-value-unit = Værdi { $value } { $unit }
# machine-drafted, needs native review
science-value-flags = Værdi { $value } (bit { $bits })
# machine-drafted, needs native review
# A class value under the pointer and its name, e.g. "Value 4 · Vegetation".
science-value-class = Værdi { $value } · { $class }
# machine-drafted, needs native review
# Sentinel-2 scene classes (L2A SCL, ESA's names) and L1C cloud masks.
s2-class-no-data = Ingen data
s2-class-defective = Mættet eller defekt
s2-class-dark = Pixels i mørke områder
s2-class-cloud-shadow = Skyskygger
s2-class-vegetation = Vegetation
s2-class-not-vegetated = Uden vegetation
s2-class-water = Vand
s2-class-unclassified = Uklassificeret
s2-class-cloud-medium = Skyer, middel sandsynlighed
s2-class-cloud-high = Skyer, høj sandsynlighed
s2-class-cirrus = Tynde cirrusskyer
s2-class-snow = Sne eller is
s2-class-clear = Skyfri
s2-class-opaque-cloud = Uigennemsigtige skyer
# Several channels under the pointer: "R 12.5  G 3.1  B 0.4".
science-values = { $values }
# Sky position under the pointer, sexagesimal then degrees.
science-radec = RA { $ra }  Dek { $dec }
science-radec-deg = { $ra }°  { $dec }°
# Galactic or ecliptic longitude and latitude.
science-lonlat = l { $lon }°  b { $lat }°
science-wcs-unsupported = WCS understøttes ikke: { $reason }
# machine-drafted, needs native review
# A B1950 (FK4) image: what is converted between its frame and another. Its own position
# stays B1950 in the readout. ICRS, FK4 and SAMP are names and stay as they are.
science-fk4-markers = Katalogmarkører: ICRS → FK4 omregnet
science-fk4-markers-galactic = Katalogmarkører: galaktisk → FK4 omregnet
science-fk4-samp = SAMP: FK4 → ICRS omregnet
# The look: stretch and limits.
science-look = { $stretch } · { $limits }
# Shown under the look while plane levelling is on.
science-levelled = Plan udjævnet
# Shown under the look while the hillshade is on.
science-hillshaded = Hillshade: sol ved { $azimuth }°, { $altitude }° oppe
# Shown in the readout while the power spectrum is shown.
science-fft = Effektspektrum (log₁₀)
# The spectrum's value under the pointer: log₁₀ of the power.
science-fft-power = log₁₀ effekt { $value }
# Spatial frequency under the pointer along x and y, and its unit ("1/µm", "1/px").
science-fft-frequency = Frekvens u { $u }  v { $v } { $unit }
# The period of that frequency: "Period 2.5 nm".
science-fft-period = Periode { $period }
# Crystal orientation under the pointer (EBSD): Bunge Euler angles in degrees.
science-euler = φ₁ { $phi1 }  Φ { $phi }  φ₂ { $phi2 }
# The crystal phase under the pointer and its symmetry ("Iron fcc (m-3m)").
science-phase = Fase { $phase } ({ $symmetry })
# Title of the spectrum plot of the pixel under the pointer (an EDS map).
science-spectrum-title = Spektrum ved { $x }, { $y }
# Physical position under the pointer (microscopy): "x 1.25 µm  y 3.5 µm".
science-position = x { $x }  y { $y }
# machine-drafted, needs native review
# CERN ROOT histograms: the bin under the pointer on one axis. $axis is the axis title
# ("p_{T}", or "x"), $range the bin's label or its edges ("20 GeV – 25 GeV"), $bin its
# number counted from 1.
science-root-bin = { $axis } { $range } (bin { $bin })
# A ROOT graph's point under the pointer.
science-root-point = x { $x }  y { $y }
# Under- and overflow bins (fills outside the axis ranges) hold something but are not shown.
science-root-flow = { $count ->
        [one] { $count } under-/overflow-bin skjult (sum { $sum })
       *[other] { $count } under-/overflow-bins skjult (sum { $sum })
    }
# Bins of different widths are drawn on an even grid of pixels.
science-root-resampled = Bins med variabel bredde omsamplet til et jævnt gitter
# machine-drafted, needs native review
# A flow cytometry plot (FCS) under the pointer: each axis's parameter and its value.
science-cytometry-at = { $x } { $xv }  { $y } { $yv }
# A cytometry histogram under the pointer: its parameter and the value.
science-cytometry-at-x = { $x } { $xv }
# Events in the plot's bin (or the histogram's bar) under the pointer.
science-cytometry-events = { $count ->
    [one] { $count } hændelse
   *[other] { $count } hændelser
}
# The plot's axis scales ($x, $y: Linear, Log, Logicle or Arcsinh).
science-cytometry-scales = x { $x }, y { $y }
science-cytometry-scale-x = x { $x }
# Spillover compensation is applied, from the file's keyword ($keyword, e.g. $SPILLOVER).
science-cytometry-compensated = Kompenseret ({ $keyword })
science-cytometry-uncompensated = Ikke kompenseret
# X-ray diffraction frames: what lies under the pointer. q is the momentum transfer
# (per ångström), d the lattice spacing (ångström), 2θ the scattering angle. The symbols
# stay as they are: "q 0.512 Å⁻¹  d 12.27 Å  2θ 4.56°".
science-scatter = q { $q } Å⁻¹  d { $d } Å  2θ { $tth }°
# Seismic readout: the trace under the pointer in a 3-D volume ($inline, $crossline the
# header numbers) or a 2-D line; $trace counts from 1 in file order, $cdp is the CDP number.
science-seismic-cell = Inline { $inline }  Crossline { $crossline }
science-seismic-trace = Spor { $trace }  CDP { $cdp }
# A cell of a 3-D volume with no trace in the file.
science-seismic-gap = Intet spor
# Two-way time (ms) or depth of the row under the pointer.
science-seismic-time = Tid { $time } ms
science-seismic-depth = Dybde { $depth } { $unit }
# The trace's CDP (or source) X and Y with the coordinate scalar applied.
science-seismic-xy = X { $x }  Y { $y }
# machine-drafted, needs native review
# Unstructured mesh readout: the face (cell) under the pointer, counting from 0 as in the
# file's connectivity, and the node nearest to the pointer when the values sit on nodes.
science-mesh-face = Flade { $face }
science-mesh-face-node = Flade { $face }  Knude { $node }
# The pointer lies between the mesh's faces (land in an ocean mesh, outside its boundary).
science-mesh-outside = Uden for nettet
# Position on an axis past the image: "Plane 3 of 40", or with its world value.
science-plane = { $axis } { $index } af { $count }
science-plane-world = { $axis } { $index } af { $count } ({ $world })
science-axis-unnamed = Plan
science-axis-extension = Billede
science-axis-channel = Kanal
# A column of a HEALPix sky map (probability, distance…) shown as the image.
science-axis-field = Felt
# X-ray event lists binned into an image: the bin size, in the event list's sky pixels
# ("Bin 2 of 4 (8 px)"), and the energy band of the events counted.
science-axis-bin = Bin
science-bin-size = { $size } px
science-axis-band = Energi
science-band-all = alle energier
# machine-drafted, needs native review
# Complex-valued arrays (NumPy, MATLAB, Zarr, TDMS): which part of each value is shown.
science-axis-part = Del
science-part-magnitude = modul
science-part-real = realdel
science-part-imaginary = imaginærdel
science-part-phase = fase (radianer)
# machine-drafted, needs native review
# Complex radar (SAR) images: intensity in decibels.
science-part-db = intensitet (dB)
# machine-drafted, needs native review
# The frames of a thermal sequence (FLIR SEQ/CSQ), in time.
science-axis-frame = Billede
science-readout = Værdi og koordinater
# Accessible names of the plane sliders.
science-sliders = Planer
# The scale bar's length.
science-scale-arcsec = { $value }″
science-scale-arcmin = { $value }′
science-scale-deg = { $value }°
# A physical length, e.g. "20 nm" (electron microscopy images).
science-scale-length = { $value } { $unit }
# machine-drafted, needs native review
# A length along a named axis of a plot, e.g. "20 GeV (p_{T})".
science-scale-axis = { $length } ({ $axis })
science-scale-a11y = Målestok: { $length }
# Mass spectrometry images (imzML). The image is the total ion current (each spot's summed
# intensities) until an m/z is typed; then that ion's image, its intensities summed over
# m/z ± the tolerance. $window is e.g. "760.5851 ± 5 ppm".
science-ion-tic = Total ionstrøm
science-ion-window = Ionbillede m/z { $window }
# The m/z prompt: $text is what is typed so far (opens on the base peak under the pointer).
science-ion-prompt = Ionbillede ved m/z: { $text }▏
# $tolerance is e.g. "10 ppm" or "0.1 Da"; a tolerance can also be typed after the m/z
# ("760.585 5 ppm"). Tab steps the tolerance; Enter on an empty prompt shows the total ion
# current again.
science-ion-prompt-hint = ± { $tolerance } · Tab tolerance · Enter vis · Esc annuller
science-ion-bad = Ikke en m/z: { $text }
# The spot under the pointer as the file numbers it (from 1).
science-ms-spot = Punkt x { $x }  y { $y }
science-ms-spot-z = Punkt x { $x }  y { $y }  z { $z }
science-ms-no-spectrum = Intet spektrum i dette punkt
# machine-drafted, needs native review
palette-science-temperature-unit = Temperaturenhed (°C, °F, K)
palette-science-isotherm = Isoterm over eller under temperaturen under markøren
palette-science-thermal-parameters = Emissivitet og reflekteret temperatur…
palette-science-export-temperatures = Eksportér temperaturer (float-TIFF eller CSV)…
palette-thermal-visible = Synligt foto til et termisk billede
disabled-not-thermal = Kun til termiske billeder
disabled-not-radiometric = Filen gemmer temperaturer: dens parametre kan ikke ændres
disabled-no-visible-photo = Det termiske billede har intet synligt foto
science-temperature-unit-toast = Temperaturer i { $unit }
science-isotherm-above = Isoterm over { $level }
science-isotherm-below = Isoterm under { $level }
science-isotherm-off = Isoterm fra
science-thermal-emissivity-prompt = Emissivitet: { $text }
science-thermal-reflected-prompt = Reflekteret temperatur ({ $unit }): { $text }
science-thermal-prompt-hint = Tab næste felt · Enter genberegn · Esc annuller
science-thermal-bad = Ikke en gyldig værdi: { $text }
science-thermal-applied = Genberegnet for emissivitet { $emissivity }, reflekteret { $reflected }
science-thermal-conditions = Emissivitet { $emissivity } · reflekteret { $reflected }
science-thermal-export-tiff = 32-bit float-TIFF
science-thermal-export-csv = CSV
science-thermal-export-done = Temperaturer gemt i { $name }
science-thermal-export-failed = Kunne ikke gemme temperaturerne: { $reason }
# machine-drafted, needs native review
science-values-failed = Kunne ikke vise værdierne: { $reason }
# machine-drafted, needs native review
# Thermal sequences (FLIR SEQ/CSQ): playback and each measurement over time.
palette-science-thermal-play = Afspil eller sæt den termiske sekvens på pause
palette-science-thermal-series-csv = Gem temperaturer over tid (CSV)…
disabled-not-thermal-sequence = Kun til termiske sekvenser
thermal-seq-title = Temperaturer over tid
# $time is seconds from the first frame, e.g. "0.12 s".
thermal-seq-frame = Billede { $frame } af { $frames } · { $time }
thermal-seq-play = Afspil
thermal-seq-pause = Pause
thermal-seq-playing = Afspiller
thermal-seq-paused = På pause
thermal-seq-save-csv = Gem CSV…
thermal-seq-hint = Tilføj et målepunkt, en linje eller et område (M) for at vise det over tid.
thermal-seq-reading = Læser billede { $done } af { $total }…
thermal-seq-failed = Kunne ikke læse billeder: { $reason }
thermal-seq-no-series = Intet at gemme endnu: tilføj et målepunkt, en linje eller et område.
# For screen readers: each measurement's value at the frame shown, e.g.
# "1 Spot: 36.4 °C, 2 Rectangle: 30.1 °C".
thermal-seq-chart = Diagram: { $values }
# machine-drafted, needs native review
# The report (PDF) of the image shown: a thermal report for a thermal image, an inspection report for any other.
palette-thermal-report = Rapport (PDF)…
thermal-report-heading = Termografirapport
thermal-report-default-title = Rapport over termografisk inspektion
thermal-report-title-label = Titel
thermal-report-notes-label = Noter
thermal-report-photo = Medtag det synlige foto
thermal-report-folder = Tilføj de andre termiske billeder i denne mappe
thermal-report-save = Gem PDF…
thermal-report-busy = Skriver rapporten…
thermal-report-nothing = Temperaturerne vises ikke endnu.
thermal-report-saved = Rapporten er gemt i { $name } ({ $count ->
    [one] { $count } billede
   *[other] { $count } billeder
})
thermal-report-failed = Kunne ikke gemme rapporten: { $reason }
# In the PDF.
thermal-report-notes = Noter
thermal-report-measurements = Målinger
thermal-report-no-measurements = Ingen målinger.
thermal-report-thermal-caption = Termisk billede
thermal-report-visible-caption = Synligt foto
thermal-report-col-measurement = Måling
thermal-report-col-min = Min.
thermal-report-col-max = Maks.
thermal-report-col-mean = Middel
thermal-report-col-emissivity = Emissivitet
thermal-report-whole-image = Hele billedet
thermal-report-camera = Kamera
thermal-report-lens = Objektiv
thermal-report-date = Taget
thermal-report-place = Sted
thermal-report-frame = Billede
thermal-report-emissivity = Emissivitet
thermal-report-reflected = Reflekteret temperatur
thermal-report-distance = Afstand
thermal-report-humidity = Luftfugtighed
thermal-report-range = Område
thermal-report-format = Format
thermal-report-page = Side { $page } af { $pages }
