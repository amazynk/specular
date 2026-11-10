# Specular — Scientific images (FITS, XISF): stretch, limits, planes, channels, coordinates. Čeština.

keys-mode-science = Vědecké snímky
cheatsheet-science = Vědecké snímky
cheatsheet-science-look = Roztažení a meze
cheatsheet-science-planes = Roviny a kanály
# machine-drafted, needs native review
cheatsheet-science-channels-1-9 = Zobrazit nebo skrýt kanály 1 až 9
cheatsheet-science-channels-10-16 = Zobrazit nebo skrýt kanály 10 až 16
palette-science-stretch = Další roztažení
palette-science-limits = Další meze
palette-science-next-plane = Další rovina
palette-science-prev-plane = Předchozí rovina
palette-science-next-axis = Procházet jinou osu
palette-science-composite = Složit kanály do barev
palette-science-channel-1 = Zobrazit nebo skrýt kanál 1
palette-science-channel-2 = Zobrazit nebo skrýt kanál 2
palette-science-channel-3 = Zobrazit nebo skrýt kanál 3
palette-science-channel-4 = Zobrazit nebo skrýt kanál 4
palette-science-channel-5 = Zobrazit nebo skrýt kanál 5
palette-science-channel-6 = Zobrazit nebo skrýt kanál 6
palette-science-channel-7 = Zobrazit nebo skrýt kanál 7
palette-science-channel-8 = Zobrazit nebo skrýt kanál 8
palette-science-channel-9 = Zobrazit nebo skrýt kanál 9
# machine-drafted, needs native review
palette-science-channel-10 = Zobrazit nebo skrýt kanál 10
palette-science-channel-11 = Zobrazit nebo skrýt kanál 11
palette-science-channel-12 = Zobrazit nebo skrýt kanál 12
palette-science-channel-13 = Zobrazit nebo skrýt kanál 13
palette-science-channel-14 = Zobrazit nebo skrýt kanál 14
palette-science-channel-15 = Zobrazit nebo skrýt kanál 15
palette-science-channel-16 = Zobrazit nebo skrýt kanál 16
palette-science-grid = Zobrazit nebo skrýt souřadnicovou síť
palette-science-reset = Automatický vzhled
palette-science-level = Vyrovnat výškové mapy (odečíst střední rovinu)
palette-science-wiggle = Seismické stopy wiggle s proměnnou plochou přes barvy
# machine-drafted, needs native review
palette-science-wireframe = Hrany nestrukturované sítě (drátový model) přes barvy
palette-science-import-lut = Tabulka barev ze souboru (ImageJ .lut)…
palette-science-hillshade = Stínovat reliéf výškových modelů (stínování reliéfu)
palette-science-fft = Výkonové spektrum (FFT) roviny
palette-science-catalogue = Překrýt katalog zdrojů (VOTable, tabulka FITS, CSV)…
palette-science-clear-catalogues = Odebrat překryté katalogy
# A catalogue overlaid on a sky image: its name (the table's or the file's), how many of its
# sources fall on the image, and how many of those are drawn (at most a set number).
science-catalogue-toast = { $name }: { $count ->
        [one] jeden zdroj na snímku
        [few] { $count } zdroje na snímku
        [many] { $count } zdroje na snímku
       *[other] { $count } zdrojů na snímku
    }
science-catalogue-capped = { $name }: zobrazeno { $shown } z { $count } zdrojů na snímku
science-catalogue-none = { $name }: žádné zdroje na snímku
science-catalogue-failed = Nejde o katalog s polohami na obloze: { $name }
science-catalogues-cleared = Katalogy odebrány
# The file dialog's filter name for catalogue files.
science-catalogue-filter = Katalogy zdrojů
# A GW sky map's credible region: the smallest area holding this share of the probability,
# drawn as a contour labelled e.g. "90 %".
science-credible = { PERCENT($percent) }
palette-science-ion-image = Iontový obraz při m/z (hmotnostní spektrometrie)…
disabled-not-science = Pouze pro vědecké snímky (astronomie, elektronová a rastrovací sondová mikroskopie)
disabled-one-plane = Snímek má jednu rovinu
disabled-one-axis = Snímek nemá jinou osu k procházení
disabled-no-channels = Snímek nemá takový kanál ke skládání
disabled-no-coordinates = Snímek nemá nebeské souřadnice
disabled-not-height-map = Pouze pro výškové mapy (Gwyddion, Igor)
disabled-not-elevation = Pouze pro georeferencované výškové modely s jedním pásmem
disabled-not-seismic = Pouze pro seismická data (SEG-Y, Seismic Unix)
disabled-not-mass-spec = Pouze pro obrazy z hmotnostní spektrometrie (imzML)
# machine-drafted, needs native review
disabled-not-ugrid = Pouze pro nestrukturované sítě (NetCDF UGRID, ICON a MPAS)
disabled-not-bayer = Jen pro surové snímky barevných kamer (FITS s Bayerovou maskou, video SER)
palette-debayer = Zobrazit snímky barevných kamer po debayeringu nebo surové
palette-bayer-pattern = Další Bayerova maska pro tento soubor
science-debayer-on = Po debayeringu ({ $pattern })
science-debayer-off = Surová maska barevných filtrů
science-bayer-pattern = Bayerova maska { $pattern }
# machine-drafted, needs native review
palette-annotations = Zobrazit nebo skrýt anotace
disabled-no-annotations = Vedle snímku nejsou žádné anotace (NDPA, Aperio nebo ASAP XML)
science-annotations-on = Anotace zobrazeny
science-annotations-off = Anotace skryty
science-stretch-auto = Automaticky (pro každou barvu)
# Stretches: how values between the limits map to grey levels.
science-stretch-linear = Lineární
science-stretch-sqrt = Druhá odmocnina
science-stretch-log = Logaritmické
science-stretch-asinh = Asinh
science-stretch-histeq = Ekvalizace histogramu
# Limits: which values show black and white. $percent is e.g. 99.5.
science-limits-zscale = Zscale
science-limits-minmax = Min/max
science-limits-percentile = { PERCENT($percent) }
# Seismic gain: limits symmetric about zero at the $percent percentile of the amplitudes.
science-limits-symmetric = ±{ PERCENT($percent) }
# The display range the file stores (ImageJ's min and max), used until other limits are chosen.
science-limits-file = Rozsah ze souboru
# Toasts when the look changes.
science-stretch-toast = Roztažení: { $stretch }
science-limits-toast = Meze: { $limits }
science-reset-toast = Automatický vzhled
# An imported colour table: $name is its file name.
science-lut-toast = Tabulka barev { $name }
science-lut-failed = Není tabulka barev: { $name }
# The file dialog's filter name for .lut files.
science-lut-filter = Tabulky barev ImageJ
science-composite-on = Kanály složeny do barev
science-composite-off = Jeden kanál v odstínech šedi
science-channel-on = Zobrazeno: { $channel }
science-channel-off = Skryto: { $channel }
# machine-drafted, needs native review
# A channel without a name in the file, in the shown/hidden toasts.
science-channel-unnamed = Kanál { $number }
science-grid-on = Souřadnicová síť zobrazena
science-grid-off = Souřadnicová síť skryta
# X-ray diffraction frames: the same action shows circles of equal resolution (d-spacing).
science-rings-on = Kruhy rozlišení zobrazeny
science-rings-off = Kruhy rozlišení skryty
# A resolution ring's label: its d-spacing in ångström, e.g. "2.5 Å".
science-ring = { $d } Å
# Seismic wiggle traces: drawn once the traces are at least 4 screen pixels apart.
science-wiggle-on = Stopy wiggle zapnuty (přibližte pro zobrazení)
science-wiggle-off = Stopy wiggle vypnuty
# machine-drafted, needs native review
# Unstructured meshes: the edges drawn over the faces; too many to draw in view until the
# user zooms in.
science-wireframe-on = Hrany sítě zapnuty
science-wireframe-off = Hrany sítě vypnuty
science-wireframe-dense = Hrany sítě zapnuty (přibližte pro zobrazení)
# Plane levelling (scanning probe height maps): the best-fit plane is subtracted for display.
science-level-on = Vyrovnání roviny zapnuto
science-level-off = Vyrovnání roviny vypnuto
# Relief shading of an elevation model. $azimuth is the sun's direction in degrees clockwise
# from north, $altitude its height above the horizon in degrees.
science-hillshade-on = Stínování reliéfu zapnuto: slunce na { $azimuth }°, { $altitude }° nad obzorem
science-hillshade-off = Stínování reliéfu vypnuto
# The power spectrum view: the plane's centred 2-D Fourier power, log scaled.
science-fft-on = Výkonové spektrum zobrazeno
science-fft-off = Výkonové spektrum skryto
science-axis-toast = Procházení: { $axis }
# Readout lines (bottom corner).
# The value under the pointer, with the values' unit when the file states one.
science-value = Hodnota { $value }
science-value-unit = Hodnota { $value } { $unit }
# machine-drafted, needs native review
science-value-flags = Hodnota { $value } (bity { $bits })
# machine-drafted, needs native review
# A class value under the pointer and its name, e.g. "Value 4 · Vegetation".
science-value-class = Hodnota { $value } · { $class }
# machine-drafted, needs native review
# Sentinel-2 scene classes (L2A SCL, ESA's names) and L1C cloud masks.
s2-class-no-data = Žádná data
s2-class-defective = Saturované nebo vadné
s2-class-dark = Pixely tmavých oblastí
s2-class-cloud-shadow = Stíny oblačnosti
s2-class-vegetation = Vegetace
s2-class-not-vegetated = Bez vegetace
s2-class-water = Voda
s2-class-unclassified = Neklasifikováno
s2-class-cloud-medium = Oblačnost se střední pravděpodobností
s2-class-cloud-high = Oblačnost s vysokou pravděpodobností
s2-class-cirrus = Tenké cirry
s2-class-snow = Sníh nebo led
s2-class-clear = Jasno
s2-class-opaque-cloud = Neprůhledná oblačnost
# Several channels under the pointer: "R 12.5  G 3.1  B 0.4".
science-values = { $values }
# Sky position under the pointer, sexagesimal then degrees.
science-radec = RA { $ra }  Dec { $dec }
science-radec-deg = { $ra }°  { $dec }°
# Galactic or ecliptic longitude and latitude.
science-lonlat = l { $lon }°  b { $lat }°
science-wcs-unsupported = WCS nepodporováno: { $reason }
# machine-drafted, needs native review
# A B1950 (FK4) image: what is converted between its frame and another. Its own position
# stays B1950 in the readout. ICRS, FK4 and SAMP are names and stay as they are.
science-fk4-markers = Značky katalogu: převedeno ICRS → FK4
science-fk4-markers-galactic = Značky katalogu: převedeno galaktické → FK4
science-fk4-samp = SAMP: převedeno FK4 → ICRS
# The look: stretch and limits.
science-look = { $stretch } · { $limits }
# Shown under the look while plane levelling is on.
science-levelled = Rovina vyrovnána
# Shown under the look while the hillshade is on.
science-hillshaded = Stínování reliéfu: slunce na { $azimuth }°, { $altitude }° nad obzorem
# Shown in the readout while the power spectrum is shown.
science-fft = Výkonové spektrum (log₁₀)
# The spectrum's value under the pointer: log₁₀ of the power.
science-fft-power = log₁₀ výkonu { $value }
# Spatial frequency under the pointer along x and y, and its unit ("1/µm", "1/px").
science-fft-frequency = Frekvence u { $u }  v { $v } { $unit }
# The period of that frequency: "Period 2.5 nm".
science-fft-period = Perioda { $period }
# Crystal orientation under the pointer (EBSD): Bunge Euler angles in degrees.
science-euler = φ₁ { $phi1 }  Φ { $phi }  φ₂ { $phi2 }
# The crystal phase under the pointer and its symmetry ("Iron fcc (m-3m)").
science-phase = Fáze { $phase } ({ $symmetry })
# Title of the spectrum plot of the pixel under the pointer (an EDS map).
science-spectrum-title = Spektrum v bodě { $x }, { $y }
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
science-root-flow = Skryté biny podtečení/přetečení: { $count } (součet { $sum })
# Bins of different widths are drawn on an even grid of pixels.
science-root-resampled = Biny proměnné šířky převzorkovány na rovnoměrnou mřížku
# machine-drafted, needs native review
# A flow cytometry plot (FCS) under the pointer: each axis's parameter and its value.
science-cytometry-at = { $x } { $xv }  { $y } { $yv }
# A cytometry histogram under the pointer: its parameter and the value.
science-cytometry-at-x = { $x } { $xv }
# Events in the plot's bin (or the histogram's bar) under the pointer.
science-cytometry-events = { $count ->
    [one] { $count } událost
    [few] { $count } události
    [many] { $count } události
   *[other] { $count } událostí
}
# The plot's axis scales ($x, $y: Linear, Log, Logicle or Arcsinh).
science-cytometry-scales = x { $x }, y { $y }
science-cytometry-scale-x = x { $x }
# Spillover compensation is applied, from the file's keyword ($keyword, e.g. $SPILLOVER).
science-cytometry-compensated = Kompenzováno ({ $keyword })
science-cytometry-uncompensated = Nekompenzováno
# X-ray diffraction frames: what lies under the pointer. q is the momentum transfer
# (per ångström), d the lattice spacing (ångström), 2θ the scattering angle. The symbols
# stay as they are: "q 0.512 Å⁻¹  d 12.27 Å  2θ 4.56°".
science-scatter = q { $q } Å⁻¹  d { $d } Å  2θ { $tth }°
# Seismic readout: the trace under the pointer in a 3-D volume ($inline, $crossline the
# header numbers) or a 2-D line; $trace counts from 1 in file order, $cdp is the CDP number.
science-seismic-cell = Inline { $inline }  Crossline { $crossline }
science-seismic-trace = Stopa { $trace }  CDP { $cdp }
# A cell of a 3-D volume with no trace in the file.
science-seismic-gap = Žádná stopa
# Two-way time (ms) or depth of the row under the pointer.
science-seismic-time = Čas { $time } ms
science-seismic-depth = Hloubka { $depth } { $unit }
# The trace's CDP (or source) X and Y with the coordinate scalar applied.
science-seismic-xy = X { $x }  Y { $y }
# machine-drafted, needs native review
# Unstructured mesh readout: the face (cell) under the pointer, counting from 0 as in the
# file's connectivity, and the node nearest to the pointer when the values sit on nodes.
science-mesh-face = Stěna { $face }
science-mesh-face-node = Stěna { $face }  Uzel { $node }
# The pointer lies between the mesh's faces (land in an ocean mesh, outside its boundary).
science-mesh-outside = Mimo síť
# Position on an axis past the image: "Plane 3 of 40", or with its world value.
science-plane = { $axis } { $index } z { $count }
science-plane-world = { $axis } { $index } z { $count } ({ $world })
science-axis-unnamed = Rovina
science-axis-extension = Snímek
science-axis-channel = Kanál
# A column of a HEALPix sky map (probability, distance…) shown as the image.
science-axis-field = Pole
# X-ray event lists binned into an image: the bin size, in the event list's sky pixels
# ("Bin 2 of 4 (8 px)"), and the energy band of the events counted.
science-axis-bin = Bin
science-bin-size = { $size } px
science-axis-band = Energie
science-band-all = všechny energie
# machine-drafted, needs native review
# Complex-valued arrays (NumPy, MATLAB, Zarr, TDMS): which part of each value is shown.
science-axis-part = Část
science-part-magnitude = absolutní hodnota
science-part-real = reálná část
science-part-imaginary = imaginární část
science-part-phase = fáze (radiány)
# machine-drafted, needs native review
# Complex radar (SAR) images: intensity in decibels.
science-part-db = intenzita (dB)
# machine-drafted, needs native review
# The frames of a thermal sequence (FLIR SEQ/CSQ), in time.
science-axis-frame = Snímek
science-readout = Hodnota a souřadnice
# Accessible names of the plane sliders.
science-sliders = Roviny
# The scale bar's length.
science-scale-arcsec = { $value }″
science-scale-arcmin = { $value }′
science-scale-deg = { $value }°
# A physical length, e.g. "20 nm" (electron microscopy images).
science-scale-length = { $value } { $unit }
# machine-drafted, needs native review
# A length along a named axis of a plot, e.g. "20 GeV (p_{T})".
science-scale-axis = { $length } ({ $axis })
science-scale-a11y = Měřítko: { $length }
# Mass spectrometry images (imzML). The image is the total ion current (each spot's summed
# intensities) until an m/z is typed; then that ion's image, its intensities summed over
# m/z ± the tolerance. $window is e.g. "760.5851 ± 5 ppm".
science-ion-tic = Celkový iontový proud
science-ion-window = Iontový obraz m/z { $window }
# The m/z prompt: $text is what is typed so far (opens on the base peak under the pointer).
science-ion-prompt = Iontový obraz při m/z: { $text }▏
# $tolerance is e.g. "10 ppm" or "0.1 Da"; a tolerance can also be typed after the m/z
# ("760.585 5 ppm"). Tab steps the tolerance; Enter on an empty prompt shows the total ion
# current again.
science-ion-prompt-hint = ± { $tolerance } · Tab tolerance · Enter zobrazit · Esc zrušit
science-ion-bad = Není hodnota m/z: { $text }
# The spot under the pointer as the file numbers it (from 1).
science-ms-spot = Bod x { $x }  y { $y }
science-ms-spot-z = Bod x { $x }  y { $y }  z { $z }
science-ms-no-spectrum = V tomto bodě není žádné spektrum
# machine-drafted, needs native review
palette-science-temperature-unit = Jednotka teploty (°C, °F, K)
palette-science-isotherm = Izoterma nad nebo pod teplotou pod ukazatelem
palette-science-thermal-parameters = Emisivita a odražená teplota…
palette-science-export-temperatures = Exportovat teploty (TIFF s plovoucí čárkou nebo CSV)…
palette-thermal-visible = Viditelná fotografie termosnímku
disabled-not-thermal = Pouze pro termosnímky
disabled-not-radiometric = Soubor ukládá teploty: jeho parametry nelze změnit
disabled-no-visible-photo = Termosnímek nemá viditelnou fotografii
science-temperature-unit-toast = Teploty v { $unit }
science-isotherm-above = Izoterma nad { $level }
science-isotherm-below = Izoterma pod { $level }
science-isotherm-off = Izoterma vypnuta
science-thermal-emissivity-prompt = Emisivita: { $text }
science-thermal-reflected-prompt = Odražená teplota ({ $unit }): { $text }
science-thermal-prompt-hint = Tab další pole · Enter přepočítat · Esc zrušit
science-thermal-bad = Neplatná hodnota: { $text }
science-thermal-applied = Přepočteno pro emisivitu { $emissivity }, odraženou teplotu { $reflected }
science-thermal-conditions = Emisivita { $emissivity } · odražená { $reflected }
science-thermal-export-tiff = 32bitový TIFF s plovoucí čárkou
science-thermal-export-csv = CSV
science-thermal-export-done = Teploty uloženy do { $name }
science-thermal-export-failed = Teploty se nepodařilo uložit: { $reason }
# machine-drafted, needs native review
science-values-failed = Hodnoty nelze zobrazit: { $reason }
# machine-drafted, needs native review
# Thermal sequences (FLIR SEQ/CSQ): playback and each measurement over time.
palette-science-thermal-play = Přehrát nebo pozastavit termální sekvenci
palette-science-thermal-series-csv = Uložit teploty v čase (CSV)…
disabled-not-thermal-sequence = Pouze pro termální sekvence
thermal-seq-title = Teploty v čase
# $time is seconds from the first frame, e.g. "0.12 s".
thermal-seq-frame = Snímek { $frame } z { $frames } · { $time }
thermal-seq-play = Přehrát
thermal-seq-pause = Pozastavit
thermal-seq-playing = Přehrává se
thermal-seq-paused = Pozastaveno
thermal-seq-save-csv = Uložit CSV…
thermal-seq-hint = Přidejte měřicí bod, úsečku nebo oblast (M) a zobrazí se jejich průběh v čase.
thermal-seq-reading = Čte se snímek { $done } z { $total }…
thermal-seq-failed = Snímky se nepodařilo přečíst: { $reason }
thermal-seq-no-series = Zatím není co uložit: přidejte měřicí bod, úsečku nebo oblast.
# For screen readers: each measurement's value at the frame shown, e.g.
# "1 Spot: 36.4 °C, 2 Rectangle: 30.1 °C".
thermal-seq-chart = Graf: { $values }
# machine-drafted, needs native review
# The report (PDF) of the image shown: a thermal report for a thermal image, an inspection report for any other.
palette-thermal-report = Zpráva (PDF)…
thermal-report-heading = Termografická zpráva
thermal-report-default-title = Zpráva z termografické kontroly
thermal-report-title-label = Název
thermal-report-notes-label = Poznámky
thermal-report-photo = Přiložit viditelnou fotografii
thermal-report-folder = Přidat ostatní termosnímky z této složky
thermal-report-save = Uložit PDF…
thermal-report-busy = Zpráva se zapisuje…
thermal-report-nothing = Teploty se zatím nezobrazují.
thermal-report-saved = Zpráva uložena do { $name } ({ $count ->
    [one] { $count } snímek
    [few] { $count } snímky
    [many] { $count } snímku
   *[other] { $count } snímků
})
thermal-report-failed = Zprávu se nepodařilo uložit: { $reason }
# In the PDF.
thermal-report-notes = Poznámky
thermal-report-measurements = Měření
thermal-report-no-measurements = Žádná měření.
thermal-report-thermal-caption = Termosnímek
thermal-report-visible-caption = Viditelná fotografie
thermal-report-col-measurement = Měření
thermal-report-col-min = Min.
thermal-report-col-max = Max.
thermal-report-col-mean = Průměr
thermal-report-col-emissivity = Emisivita
thermal-report-whole-image = Celý snímek
thermal-report-camera = Fotoaparát
thermal-report-lens = Objektiv
thermal-report-date = Pořízeno
thermal-report-place = Místo
thermal-report-frame = Snímek
thermal-report-emissivity = Emisivita
thermal-report-reflected = Odražená teplota
thermal-report-distance = Vzdálenost
thermal-report-humidity = Vlhkost
thermal-report-range = Rozsah
thermal-report-format = Formát
thermal-report-page = Strana { $page } z { $pages }
