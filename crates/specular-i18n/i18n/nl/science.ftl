# Specular — Scientific images (FITS, XISF): stretch, limits, planes, channels, coordinates. Nederlands.

keys-mode-science = Wetenschappelijke beelden
cheatsheet-science = Wetenschappelijke beelden
cheatsheet-science-look = Stretch en grenzen
cheatsheet-science-planes = Vlakken en kanalen
# machine-drafted, needs native review
cheatsheet-science-channels-1-9 = Kanalen 1 tot en met 9 tonen of verbergen
cheatsheet-science-channels-10-16 = Kanalen 10 tot en met 16 tonen of verbergen
palette-science-stretch = Volgende stretch
palette-science-limits = Volgende grenzen
palette-science-next-plane = Volgend vlak
palette-science-prev-plane = Vorig vlak
palette-science-next-axis = Andere as doorlopen
palette-science-composite = Kanalen in kleur combineren
palette-science-channel-1 = Kanaal 1 tonen of verbergen
palette-science-channel-2 = Kanaal 2 tonen of verbergen
palette-science-channel-3 = Kanaal 3 tonen of verbergen
palette-science-channel-4 = Kanaal 4 tonen of verbergen
palette-science-channel-5 = Kanaal 5 tonen of verbergen
palette-science-channel-6 = Kanaal 6 tonen of verbergen
palette-science-channel-7 = Kanaal 7 tonen of verbergen
palette-science-channel-8 = Kanaal 8 tonen of verbergen
palette-science-channel-9 = Kanaal 9 tonen of verbergen
# machine-drafted, needs native review
palette-science-channel-10 = Kanaal 10 tonen of verbergen
palette-science-channel-11 = Kanaal 11 tonen of verbergen
palette-science-channel-12 = Kanaal 12 tonen of verbergen
palette-science-channel-13 = Kanaal 13 tonen of verbergen
palette-science-channel-14 = Kanaal 14 tonen of verbergen
palette-science-channel-15 = Kanaal 15 tonen of verbergen
palette-science-channel-16 = Kanaal 16 tonen of verbergen
palette-science-grid = Coördinatenraster tonen of verbergen
palette-science-reset = Automatische weergave
palette-science-level = Hoogtekaarten nivelleren (gemiddeld vlak aftrekken)
palette-science-wiggle = Seismische wiggle-sporen met variabel oppervlak, over de kleuren
# machine-drafted, needs native review
palette-science-wireframe = Randen van ongestructureerde roosters (draadmodel), over de kleuren
palette-science-import-lut = Kleurentabel uit een bestand (ImageJ .lut)…
palette-science-hillshade = Reliëf van hoogtemodellen schaduwen (hillshade)
palette-science-fft = Vermogensspectrum (FFT) van het vlak
palette-science-catalogue = Broncatalogus over het beeld leggen (VOTable, FITS-tabel, CSV)…
palette-science-clear-catalogues = Overgelegde catalogi verwijderen
# A catalogue overlaid on a sky image: its name (the table's or the file's), how many of its
# sources fall on the image, and how many of those are drawn (at most a set number).
science-catalogue-toast = { $name }: { $count ->
        [one] één bron op het beeld
       *[other] { $count } bronnen op het beeld
    }
science-catalogue-capped = { $name }: { $shown } van { $count } bronnen op het beeld getoond
science-catalogue-none = { $name }: geen bronnen op het beeld
science-catalogue-failed = Geen catalogus met hemelposities: { $name }
science-catalogues-cleared = Catalogi verwijderd
# The file dialog's filter name for catalogue files.
science-catalogue-filter = Broncatalogi
# A GW sky map's credible region: the smallest area holding this share of the probability,
# drawn as a contour labelled e.g. "90 %".
science-credible = { PERCENT($percent) }
palette-science-ion-image = Ionenbeeld bij een m/z (massaspectrometrie)…
disabled-not-science = Alleen voor wetenschappelijke beelden (astronomie, elektronen- en scanning-probe-microscopie)
disabled-one-plane = Het beeld heeft één vlak
disabled-one-axis = Het beeld heeft geen andere as om te doorlopen
disabled-no-channels = Het beeld heeft dat kanaal niet om te combineren
disabled-no-coordinates = Het beeld heeft geen hemelcoördinaten
disabled-not-height-map = Alleen voor hoogtekaarten (Gwyddion, Igor)
disabled-not-elevation = Alleen voor gegeorefereerde hoogtemodellen met één band
disabled-not-seismic = Alleen voor seismische data (SEG-Y, Seismic Unix)
disabled-not-mass-spec = Alleen voor massaspectrometriebeelden (imzML)
# machine-drafted, needs native review
disabled-not-ugrid = Alleen voor ongestructureerde roosters (UGRID-, ICON- en MPAS-NetCDF)
disabled-not-bayer = Alleen voor ruwe beelden van kleurcamera’s (FITS met Bayerpatroon, SER-video)
palette-debayer = Beelden van kleurcamera’s gedebayerd of ruw tonen
palette-bayer-pattern = Volgend Bayerpatroon voor dit bestand
science-debayer-on = Gedebayerd ({ $pattern })
science-debayer-off = Ruwe kleurfiltermatrix
science-bayer-pattern = Bayerpatroon { $pattern }
# machine-drafted, needs native review
palette-annotations = Annotaties tonen of verbergen
disabled-no-annotations = Naast het beeld staan geen annotaties (NDPA, Aperio of ASAP XML)
science-annotations-on = Annotaties getoond
science-annotations-off = Annotaties verborgen
science-stretch-auto = Automatisch (per kleur)
# Stretches: how values between the limits map to grey levels.
science-stretch-linear = Lineair
science-stretch-sqrt = Vierkantswortel
science-stretch-log = Logaritmisch
science-stretch-asinh = Asinh
science-stretch-histeq = Histogramegalisatie
# Limits: which values show black and white. $percent is e.g. 99.5.
science-limits-zscale = Zscale
science-limits-minmax = Min/max
science-limits-percentile = { PERCENT($percent) }
# Seismic gain: limits symmetric about zero at the $percent percentile of the amplitudes.
science-limits-symmetric = ±{ PERCENT($percent) }
# The display range the file stores (ImageJ's min and max), used until other limits are chosen.
science-limits-file = Bereik uit bestand
# Toasts when the look changes.
science-stretch-toast = Stretch: { $stretch }
science-limits-toast = Grenzen: { $limits }
science-reset-toast = Automatische weergave
# An imported colour table: $name is its file name.
science-lut-toast = Kleurentabel { $name }
science-lut-failed = Geen kleurentabel: { $name }
# The file dialog's filter name for .lut files.
science-lut-filter = ImageJ-kleurentabellen
science-composite-on = Kanalen in kleur gecombineerd
science-composite-off = Eén kanaal in grijs
science-channel-on = { $channel } getoond
science-channel-off = { $channel } verborgen
# machine-drafted, needs native review
# A channel without a name in the file, in the shown/hidden toasts.
science-channel-unnamed = Kanaal { $number }
science-grid-on = Coördinatenraster getoond
science-grid-off = Coördinatenraster verborgen
# X-ray diffraction frames: the same action shows circles of equal resolution (d-spacing).
science-rings-on = Resolutieringen getoond
science-rings-off = Resolutieringen verborgen
# A resolution ring's label: its d-spacing in ångström, e.g. "2.5 Å".
science-ring = { $d } Å
# Seismic wiggle traces: drawn once the traces are at least 4 screen pixels apart.
science-wiggle-on = Wiggle-sporen aan (zoom in om ze te zien)
science-wiggle-off = Wiggle-sporen uit
# machine-drafted, needs native review
# Unstructured meshes: the edges drawn over the faces; too many to draw in view until the
# user zooms in.
science-wireframe-on = Roosterranden aan
science-wireframe-off = Roosterranden uit
science-wireframe-dense = Roosterranden aan (zoom in om ze te zien)
# Plane levelling (scanning probe height maps): the best-fit plane is subtracted for display.
science-level-on = Vlaknivellering aan
science-level-off = Vlaknivellering uit
# Relief shading of an elevation model. $azimuth is the sun's direction in degrees clockwise
# from north, $altitude its height above the horizon in degrees.
science-hillshade-on = Hillshade aan: zon op { $azimuth }°, { $altitude }° hoog
science-hillshade-off = Hillshade uit
# The power spectrum view: the plane's centred 2-D Fourier power, log scaled.
science-fft-on = Vermogensspectrum zichtbaar
science-fft-off = Vermogensspectrum verborgen
science-axis-toast = Doorlopen: { $axis }
# Readout lines (bottom corner).
# The value under the pointer, with the values' unit when the file states one.
science-value = Waarde { $value }
science-value-unit = Waarde { $value } { $unit }
# machine-drafted, needs native review
science-value-flags = Waarde { $value } (bits { $bits })
# machine-drafted, needs native review
# A class value under the pointer and its name, e.g. "Value 4 · Vegetation".
science-value-class = Waarde { $value } · { $class }
# machine-drafted, needs native review
# Sentinel-2 scene classes (L2A SCL, ESA's names) and L1C cloud masks.
s2-class-no-data = Geen gegevens
s2-class-defective = Verzadigd of defect
s2-class-dark = Pixels van donkere gebieden
s2-class-cloud-shadow = Wolkschaduwen
s2-class-vegetation = Vegetatie
s2-class-not-vegetated = Niet begroeid
s2-class-water = Water
s2-class-unclassified = Niet geclassificeerd
s2-class-cloud-medium = Bewolking, gemiddelde waarschijnlijkheid
s2-class-cloud-high = Bewolking, hoge waarschijnlijkheid
s2-class-cirrus = Dunne cirrus
s2-class-snow = Sneeuw of ijs
s2-class-clear = Onbewolkt
s2-class-opaque-cloud = Ondoorzichtige wolken
# Several channels under the pointer: "R 12.5  G 3.1  B 0.4".
science-values = { $values }
# Sky position under the pointer, sexagesimal then degrees.
science-radec = RA { $ra }  Dec { $dec }
science-radec-deg = { $ra }°  { $dec }°
# Galactic or ecliptic longitude and latitude.
science-lonlat = l { $lon }°  b { $lat }°
science-wcs-unsupported = WCS niet ondersteund: { $reason }
# machine-drafted, needs native review
# A B1950 (FK4) image: what is converted between its frame and another. Its own position
# stays B1950 in the readout. ICRS, FK4 and SAMP are names and stay as they are.
science-fk4-markers = Catalogusmarkeringen: ICRS → FK4 omgerekend
science-fk4-markers-galactic = Catalogusmarkeringen: galactisch → FK4 omgerekend
science-fk4-samp = SAMP: FK4 → ICRS omgerekend
# The look: stretch and limits.
science-look = { $stretch } · { $limits }
# Shown under the look while plane levelling is on.
science-levelled = Vlak genivelleerd
# Shown under the look while the hillshade is on.
science-hillshaded = Hillshade: zon op { $azimuth }°, { $altitude }° hoog
# Shown in the readout while the power spectrum is shown.
science-fft = Vermogensspectrum (log₁₀)
# The spectrum's value under the pointer: log₁₀ of the power.
science-fft-power = log₁₀ vermogen { $value }
# Spatial frequency under the pointer along x and y, and its unit ("1/µm", "1/px").
science-fft-frequency = Frequentie u { $u }  v { $v } { $unit }
# The period of that frequency: "Period 2.5 nm".
science-fft-period = Periode { $period }
# Crystal orientation under the pointer (EBSD): Bunge Euler angles in degrees.
science-euler = φ₁ { $phi1 }  Φ { $phi }  φ₂ { $phi2 }
# The crystal phase under the pointer and its symmetry ("Iron fcc (m-3m)").
science-phase = Fase { $phase } ({ $symmetry })
# Title of the spectrum plot of the pixel under the pointer (an EDS map).
science-spectrum-title = Spectrum op { $x }, { $y }
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
        [one] { $count } under-/overflowbin verborgen (som { $sum })
       *[other] { $count } under-/overflowbins verborgen (som { $sum })
    }
# Bins of different widths are drawn on an even grid of pixels.
science-root-resampled = Bins met variabele breedte herbemonsterd naar een gelijkmatig raster
# machine-drafted, needs native review
# A flow cytometry plot (FCS) under the pointer: each axis's parameter and its value.
science-cytometry-at = { $x } { $xv }  { $y } { $yv }
# A cytometry histogram under the pointer: its parameter and the value.
science-cytometry-at-x = { $x } { $xv }
# Events in the plot's bin (or the histogram's bar) under the pointer.
science-cytometry-events = { $count ->
    [one] { $count } gebeurtenis
   *[other] { $count } gebeurtenissen
}
# The plot's axis scales ($x, $y: Linear, Log, Logicle or Arcsinh).
science-cytometry-scales = x { $x }, y { $y }
science-cytometry-scale-x = x { $x }
# Spillover compensation is applied, from the file's keyword ($keyword, e.g. $SPILLOVER).
science-cytometry-compensated = Gecompenseerd ({ $keyword })
science-cytometry-uncompensated = Niet gecompenseerd
# X-ray diffraction frames: what lies under the pointer. q is the momentum transfer
# (per ångström), d the lattice spacing (ångström), 2θ the scattering angle. The symbols
# stay as they are: "q 0.512 Å⁻¹  d 12.27 Å  2θ 4.56°".
science-scatter = q { $q } Å⁻¹  d { $d } Å  2θ { $tth }°
# Seismic readout: the trace under the pointer in a 3-D volume ($inline, $crossline the
# header numbers) or a 2-D line; $trace counts from 1 in file order, $cdp is the CDP number.
science-seismic-cell = Inline { $inline }  Crossline { $crossline }
science-seismic-trace = Spoor { $trace }  CDP { $cdp }
# A cell of a 3-D volume with no trace in the file.
science-seismic-gap = Geen spoor
# Two-way time (ms) or depth of the row under the pointer.
science-seismic-time = Tijd { $time } ms
science-seismic-depth = Diepte { $depth } { $unit }
# The trace's CDP (or source) X and Y with the coordinate scalar applied.
science-seismic-xy = X { $x }  Y { $y }
# machine-drafted, needs native review
# Unstructured mesh readout: the face (cell) under the pointer, counting from 0 as in the
# file's connectivity, and the node nearest to the pointer when the values sit on nodes.
science-mesh-face = Vlak { $face }
science-mesh-face-node = Vlak { $face }  Knoop { $node }
# The pointer lies between the mesh's faces (land in an ocean mesh, outside its boundary).
science-mesh-outside = Buiten het rooster
# Position on an axis past the image: "Plane 3 of 40", or with its world value.
science-plane = { $axis } { $index } van { $count }
science-plane-world = { $axis } { $index } van { $count } ({ $world })
science-axis-unnamed = Vlak
science-axis-extension = Beeld
science-axis-channel = Kanaal
# A column of a HEALPix sky map (probability, distance…) shown as the image.
science-axis-field = Veld
# X-ray event lists binned into an image: the bin size, in the event list's sky pixels
# ("Bin 2 of 4 (8 px)"), and the energy band of the events counted.
science-axis-bin = Bin
science-bin-size = { $size } px
science-axis-band = Energie
science-band-all = alle energieën
# machine-drafted, needs native review
# Complex-valued arrays (NumPy, MATLAB, Zarr, TDMS): which part of each value is shown.
science-axis-part = Deel
science-part-magnitude = modulus
science-part-real = reëel deel
science-part-imaginary = imaginair deel
science-part-phase = fase (radialen)
# machine-drafted, needs native review
# Complex radar (SAR) images: intensity in decibels.
science-part-db = intensiteit (dB)
# machine-drafted, needs native review
# The frames of a thermal sequence (FLIR SEQ/CSQ), in time.
science-axis-frame = Frame
science-readout = Waarde en coördinaten
# Accessible names of the plane sliders.
science-sliders = Vlakken
# The scale bar's length.
science-scale-arcsec = { $value }″
science-scale-arcmin = { $value }′
science-scale-deg = { $value }°
# A physical length, e.g. "20 nm" (electron microscopy images).
science-scale-length = { $value } { $unit }
# machine-drafted, needs native review
# A length along a named axis of a plot, e.g. "20 GeV (p_{T})".
science-scale-axis = { $length } ({ $axis })
science-scale-a11y = Schaalbalk: { $length }
# Mass spectrometry images (imzML). The image is the total ion current (each spot's summed
# intensities) until an m/z is typed; then that ion's image, its intensities summed over
# m/z ± the tolerance. $window is e.g. "760.5851 ± 5 ppm".
science-ion-tic = Totale ionenstroom
science-ion-window = Ionenbeeld m/z { $window }
# The m/z prompt: $text is what is typed so far (opens on the base peak under the pointer).
science-ion-prompt = Ionenbeeld bij m/z: { $text }▏
# $tolerance is e.g. "10 ppm" or "0.1 Da"; a tolerance can also be typed after the m/z
# ("760.585 5 ppm"). Tab steps the tolerance; Enter on an empty prompt shows the total ion
# current again.
science-ion-prompt-hint = ± { $tolerance } · Tab tolerantie · Enter tonen · Esc annuleren
science-ion-bad = Geen m/z: { $text }
# The spot under the pointer as the file numbers it (from 1).
science-ms-spot = Punt x { $x }  y { $y }
science-ms-spot-z = Punt x { $x }  y { $y }  z { $z }
science-ms-no-spectrum = Geen spectrum op dit punt
# machine-drafted, needs native review
palette-science-temperature-unit = Temperatuureenheid (°C, °F, K)
palette-science-isotherm = Isotherm boven of onder de temperatuur onder de aanwijzer
palette-science-thermal-parameters = Emissiviteit en gereflecteerde temperatuur…
palette-science-export-temperatures = Temperaturen exporteren (float-TIFF of CSV)…
palette-thermal-visible = Zichtbare foto van een warmtebeeld
disabled-not-thermal = Alleen voor warmtebeelden
disabled-not-radiometric = Het bestand slaat temperaturen op: de parameters kunnen niet worden gewijzigd
disabled-no-visible-photo = Het warmtebeeld heeft geen zichtbare foto
science-temperature-unit-toast = Temperaturen in { $unit }
science-isotherm-above = Isotherm boven { $level }
science-isotherm-below = Isotherm onder { $level }
science-isotherm-off = Isotherm uit
science-thermal-emissivity-prompt = Emissiviteit: { $text }
science-thermal-reflected-prompt = Gereflecteerde temperatuur ({ $unit }): { $text }
science-thermal-prompt-hint = Tab volgend veld · Enter herberekenen · Esc annuleren
science-thermal-bad = Geen geldige waarde: { $text }
science-thermal-applied = Herberekend voor emissiviteit { $emissivity }, gereflecteerd { $reflected }
science-thermal-conditions = Emissiviteit { $emissivity } · gereflecteerd { $reflected }
science-thermal-export-tiff = 32-bits float-TIFF
science-thermal-export-csv = CSV
science-thermal-export-done = Temperaturen opgeslagen in { $name }
science-thermal-export-failed = Kan de temperaturen niet opslaan: { $reason }
# machine-drafted, needs native review
science-values-failed = Kan de waarden niet tonen: { $reason }
# machine-drafted, needs native review
# Thermal sequences (FLIR SEQ/CSQ): playback and each measurement over time.
palette-science-thermal-play = De warmtebeeldreeks afspelen of pauzeren
palette-science-thermal-series-csv = Temperaturen in de tijd opslaan (CSV)…
disabled-not-thermal-sequence = Alleen voor warmtebeeldreeksen
thermal-seq-title = Temperaturen in de tijd
# $time is seconds from the first frame, e.g. "0.12 s".
thermal-seq-frame = Frame { $frame } van { $frames } · { $time }
thermal-seq-play = Afspelen
thermal-seq-pause = Pauzeren
thermal-seq-playing = Speelt af
thermal-seq-paused = Gepauzeerd
thermal-seq-save-csv = CSV opslaan…
thermal-seq-hint = Voeg een meetpunt, lijn of gebied toe (M) om het in de tijd te volgen.
thermal-seq-reading = Frame { $done } van { $total } lezen…
thermal-seq-failed = Kan de frames niet lezen: { $reason }
thermal-seq-no-series = Nog niets om op te slaan: voeg een meetpunt, lijn of gebied toe.
# For screen readers: each measurement's value at the frame shown, e.g.
# "1 Spot: 36.4 °C, 2 Rectangle: 30.1 °C".
thermal-seq-chart = Grafiek: { $values }
# machine-drafted, needs native review
# The report (PDF) of the image shown: a thermal report for a thermal image, an inspection report for any other.
palette-thermal-report = Rapport (PDF)…
thermal-report-heading = Thermografierapport
thermal-report-default-title = Rapport thermografische inspectie
thermal-report-title-label = Titel
thermal-report-notes-label = Notities
thermal-report-photo = De zichtbare foto opnemen
thermal-report-folder = De andere warmtebeelden in deze map toevoegen
thermal-report-save = PDF opslaan…
thermal-report-busy = Rapport schrijven…
thermal-report-nothing = De temperaturen worden nog niet getoond.
thermal-report-saved = Rapport opgeslagen in { $name } ({ $count ->
    [one] { $count } beeld
   *[other] { $count } beelden
})
thermal-report-failed = Kan het rapport niet opslaan: { $reason }
# In the PDF.
thermal-report-notes = Notities
thermal-report-measurements = Metingen
thermal-report-no-measurements = Geen metingen.
thermal-report-thermal-caption = Warmtebeeld
thermal-report-visible-caption = Zichtbare foto
thermal-report-col-measurement = Meting
thermal-report-col-min = Min
thermal-report-col-max = Max
thermal-report-col-mean = Gemiddelde
thermal-report-col-emissivity = Emissiviteit
thermal-report-whole-image = Hele beeld
thermal-report-camera = Camera
thermal-report-lens = Objectief
thermal-report-date = Gemaakt
thermal-report-place = Plaats
thermal-report-frame = Frame
thermal-report-emissivity = Emissiviteit
thermal-report-reflected = Gereflecteerde temperatuur
thermal-report-distance = Afstand
thermal-report-humidity = Luchtvochtigheid
thermal-report-range = Bereik
thermal-report-format = Formaat
thermal-report-page = Pagina { $page } van { $pages }
