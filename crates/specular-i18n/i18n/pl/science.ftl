# Specular — Scientific images (FITS, XISF): stretch, limits, planes, channels, coordinates. Polski.

keys-mode-science = Obrazy naukowe
cheatsheet-science = Obrazy naukowe
cheatsheet-science-look = Rozciągnięcie i granice
cheatsheet-science-planes = Płaszczyzny i kanały
# machine-drafted, needs native review
cheatsheet-science-channels-1-9 = Pokaż lub ukryj kanały od 1 do 9
cheatsheet-science-channels-10-16 = Pokaż lub ukryj kanały od 10 do 16
palette-science-stretch = Następne rozciągnięcie
palette-science-limits = Następne granice
palette-science-next-plane = Następna płaszczyzna
palette-science-prev-plane = Poprzednia płaszczyzna
palette-science-next-axis = Przechodź po innej osi
palette-science-composite = Złóż kanały w kolorze
palette-science-channel-1 = Pokaż lub ukryj kanał 1
palette-science-channel-2 = Pokaż lub ukryj kanał 2
palette-science-channel-3 = Pokaż lub ukryj kanał 3
palette-science-channel-4 = Pokaż lub ukryj kanał 4
palette-science-channel-5 = Pokaż lub ukryj kanał 5
palette-science-channel-6 = Pokaż lub ukryj kanał 6
palette-science-channel-7 = Pokaż lub ukryj kanał 7
palette-science-channel-8 = Pokaż lub ukryj kanał 8
palette-science-channel-9 = Pokaż lub ukryj kanał 9
# machine-drafted, needs native review
palette-science-channel-10 = Pokaż lub ukryj kanał 10
palette-science-channel-11 = Pokaż lub ukryj kanał 11
palette-science-channel-12 = Pokaż lub ukryj kanał 12
palette-science-channel-13 = Pokaż lub ukryj kanał 13
palette-science-channel-14 = Pokaż lub ukryj kanał 14
palette-science-channel-15 = Pokaż lub ukryj kanał 15
palette-science-channel-16 = Pokaż lub ukryj kanał 16
palette-science-grid = Pokaż lub ukryj siatkę współrzędnych
palette-science-reset = Automatyczny wygląd
palette-science-level = Wyrównaj mapy wysokości (odejmij płaszczyznę średnią)
palette-science-wiggle = Sejsmiczne trasy wiggle z wypełnionym polem (variable area) na kolorach
# machine-drafted, needs native review
palette-science-wireframe = Krawędzie siatki niestrukturalnej (szkielet) na kolorach
palette-science-import-lut = Tablica kolorów z pliku (ImageJ .lut)…
palette-science-hillshade = Cieniuj rzeźbę modeli wysokościowych (cieniowanie terenu)
palette-science-fft = Widmo mocy (FFT) płaszczyzny
palette-science-catalogue = Nałóż katalog źródeł (VOTable, tabela FITS, CSV)…
palette-science-clear-catalogues = Usuń nałożone katalogi
# A catalogue overlaid on a sky image: its name (the table's or the file's), how many of its
# sources fall on the image, and how many of those are drawn (at most a set number).
science-catalogue-toast = { $name }: { $count ->
        [one] jedno źródło na obrazie
        [few] { $count } źródła na obrazie
        [many] { $count } źródeł na obrazie
       *[other] { $count } źródła na obrazie
    }
science-catalogue-capped = { $name }: wyświetlono { $shown } z { $count } źródeł na obrazie
science-catalogue-none = { $name }: brak źródeł na obrazie
science-catalogue-failed = To nie jest katalog z pozycjami na niebie: { $name }
science-catalogues-cleared = Usunięto katalogi
# The file dialog's filter name for catalogue files.
science-catalogue-filter = Katalogi źródeł
# A GW sky map's credible region: the smallest area holding this share of the probability,
# drawn as a contour labelled e.g. "90 %".
science-credible = { PERCENT($percent) }
palette-science-ion-image = Obraz jonowy dla m/z (spektrometria mas)…
disabled-not-science = Tylko dla obrazów naukowych (astronomia, mikroskopia elektronowa i z sondą skanującą)
disabled-one-plane = Obraz ma jedną płaszczyznę
disabled-one-axis = Obraz nie ma innej osi do przechodzenia
disabled-no-channels = Obraz nie ma takiego kanału do złożenia
disabled-no-coordinates = Obraz nie ma współrzędnych niebieskich
disabled-not-height-map = Tylko dla map wysokości (Gwyddion, Igor)
disabled-not-elevation = Tylko dla georeferencjonowanych modeli wysokościowych z jednym pasmem
disabled-not-seismic = Tylko dla danych sejsmicznych (SEG-Y, Seismic Unix)
disabled-not-mass-spec = Tylko dla obrazów spektrometrii mas (imzML)
# machine-drafted, needs native review
disabled-not-ugrid = Tylko dla siatek niestrukturalnych (NetCDF UGRID, ICON i MPAS)
disabled-not-bayer = Tylko dla surowych klatek z kamer kolorowych (FITS ze wzorem Bayera, wideo SER)
palette-debayer = Pokazuj klatki z kamer kolorowych po debayeryzacji lub surowe
palette-bayer-pattern = Następny wzór Bayera dla tego pliku
science-debayer-on = Po debayeryzacji ({ $pattern })
science-debayer-off = Surowa matryca filtrów barwnych
science-bayer-pattern = Wzór Bayera { $pattern }
# machine-drafted, needs native review
palette-annotations = Pokaż lub ukryj adnotacje
disabled-no-annotations = Obok obrazu nie ma adnotacji (NDPA, Aperio ani ASAP XML)
science-annotations-on = Adnotacje pokazane
science-annotations-off = Adnotacje ukryte
science-stretch-auto = Automatycznie (dla każdego koloru)
# Stretches: how values between the limits map to grey levels.
science-stretch-linear = Liniowe
science-stretch-sqrt = Pierwiastek kwadratowy
science-stretch-log = Logarytmiczne
science-stretch-asinh = Asinh
science-stretch-histeq = Wyrównanie histogramu
# Limits: which values show black and white. $percent is e.g. 99.5.
science-limits-zscale = Zscale
science-limits-minmax = Min/maks
science-limits-percentile = { PERCENT($percent) }
# Seismic gain: limits symmetric about zero at the $percent percentile of the amplitudes.
science-limits-symmetric = ±{ PERCENT($percent) }
# The display range the file stores (ImageJ's min and max), used until other limits are chosen.
science-limits-file = Zakres z pliku
# Toasts when the look changes.
science-stretch-toast = Rozciągnięcie: { $stretch }
science-limits-toast = Granice: { $limits }
science-reset-toast = Automatyczny wygląd
# An imported colour table: $name is its file name.
science-lut-toast = Tablica kolorów { $name }
science-lut-failed = To nie jest tablica kolorów: { $name }
# The file dialog's filter name for .lut files.
science-lut-filter = Tablice kolorów ImageJ
science-composite-on = Kanały złożone w kolorze
science-composite-off = Jeden kanał w skali szarości
science-channel-on = Pokazano: { $channel }
science-channel-off = Ukryto: { $channel }
# machine-drafted, needs native review
# A channel without a name in the file, in the shown/hidden toasts.
science-channel-unnamed = Kanał { $number }
science-grid-on = Siatka współrzędnych pokazana
science-grid-off = Siatka współrzędnych ukryta
# X-ray diffraction frames: the same action shows circles of equal resolution (d-spacing).
science-rings-on = Pierścienie rozdzielczości pokazane
science-rings-off = Pierścienie rozdzielczości ukryte
# A resolution ring's label: its d-spacing in ångström, e.g. "2.5 Å".
science-ring = { $d } Å
# Seismic wiggle traces: drawn once the traces are at least 4 screen pixels apart.
science-wiggle-on = Trasy wiggle włączone (powiększ, aby je zobaczyć)
science-wiggle-off = Trasy wiggle wyłączone
# machine-drafted, needs native review
# Unstructured meshes: the edges drawn over the faces; too many to draw in view until the
# user zooms in.
science-wireframe-on = Krawędzie siatki włączone
science-wireframe-off = Krawędzie siatki wyłączone
science-wireframe-dense = Krawędzie siatki włączone (powiększ, aby je zobaczyć)
# Plane levelling (scanning probe height maps): the best-fit plane is subtracted for display.
science-level-on = Wyrównanie płaszczyzny włączone
science-level-off = Wyrównanie płaszczyzny wyłączone
# Relief shading of an elevation model. $azimuth is the sun's direction in degrees clockwise
# from north, $altitude its height above the horizon in degrees.
science-hillshade-on = Cieniowanie terenu włączone: słońce na { $azimuth }°, { $altitude }° nad horyzontem
science-hillshade-off = Cieniowanie terenu wyłączone
# The power spectrum view: the plane's centred 2-D Fourier power, log scaled.
science-fft-on = Widmo mocy pokazane
science-fft-off = Widmo mocy ukryte
science-axis-toast = Przechodzenie: { $axis }
# Readout lines (bottom corner).
# The value under the pointer, with the values' unit when the file states one.
science-value = Wartość { $value }
science-value-unit = Wartość { $value } { $unit }
# machine-drafted, needs native review
science-value-flags = Wartość { $value } (bity { $bits })
# machine-drafted, needs native review
# A class value under the pointer and its name, e.g. "Value 4 · Vegetation".
science-value-class = Wartość { $value } · { $class }
# machine-drafted, needs native review
# Sentinel-2 scene classes (L2A SCL, ESA's names) and L1C cloud masks.
s2-class-no-data = Brak danych
s2-class-defective = Nasycone lub uszkodzone
s2-class-dark = Piksele ciemnych obszarów
s2-class-cloud-shadow = Cienie chmur
s2-class-vegetation = Roślinność
s2-class-not-vegetated = Bez roślinności
s2-class-water = Woda
s2-class-unclassified = Niesklasyfikowane
s2-class-cloud-medium = Chmury o średnim prawdopodobieństwie
s2-class-cloud-high = Chmury o wysokim prawdopodobieństwie
s2-class-cirrus = Cienkie cirrusy
s2-class-snow = Śnieg lub lód
s2-class-clear = Bezchmurnie
s2-class-opaque-cloud = Nieprzezroczyste chmury
# Several channels under the pointer: "R 12.5  G 3.1  B 0.4".
science-values = { $values }
# Sky position under the pointer, sexagesimal then degrees.
science-radec = RA { $ra }  Dec { $dec }
science-radec-deg = { $ra }°  { $dec }°
# Galactic or ecliptic longitude and latitude.
science-lonlat = l { $lon }°  b { $lat }°
science-wcs-unsupported = WCS nieobsługiwany: { $reason }
# machine-drafted, needs native review
# A B1950 (FK4) image: what is converted between its frame and another. Its own position
# stays B1950 in the readout. ICRS, FK4 and SAMP are names and stay as they are.
science-fk4-markers = Znaczniki katalogu: przeliczono ICRS → FK4
science-fk4-markers-galactic = Znaczniki katalogu: przeliczono galaktyczne → FK4
science-fk4-samp = SAMP: przeliczono FK4 → ICRS
# The look: stretch and limits.
science-look = { $stretch } · { $limits }
# Shown under the look while plane levelling is on.
science-levelled = Płaszczyzna wyrównana
# Shown under the look while the hillshade is on.
science-hillshaded = Cieniowanie terenu: słońce na { $azimuth }°, { $altitude }° nad horyzontem
# Shown in the readout while the power spectrum is shown.
science-fft = Widmo mocy (log₁₀)
# The spectrum's value under the pointer: log₁₀ of the power.
science-fft-power = log₁₀ mocy { $value }
# Spatial frequency under the pointer along x and y, and its unit ("1/µm", "1/px").
science-fft-frequency = Częstotliwość u { $u }  v { $v } { $unit }
# The period of that frequency: "Period 2.5 nm".
science-fft-period = Okres { $period }
# Crystal orientation under the pointer (EBSD): Bunge Euler angles in degrees.
science-euler = φ₁ { $phi1 }  Φ { $phi }  φ₂ { $phi2 }
# The crystal phase under the pointer and its symmetry ("Iron fcc (m-3m)").
science-phase = Faza { $phase } ({ $symmetry })
# Title of the spectrum plot of the pixel under the pointer (an EDS map).
science-spectrum-title = Widmo w punkcie { $x }, { $y }
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
science-root-flow = Ukryte biny niedomiaru/nadmiaru (underflow/overflow): { $count } (suma { $sum })
# Bins of different widths are drawn on an even grid of pixels.
science-root-resampled = Biny o zmiennej szerokości przepróbkowane do równomiernej siatki
# machine-drafted, needs native review
# A flow cytometry plot (FCS) under the pointer: each axis's parameter and its value.
science-cytometry-at = { $x } { $xv }  { $y } { $yv }
# A cytometry histogram under the pointer: its parameter and the value.
science-cytometry-at-x = { $x } { $xv }
# Events in the plot's bin (or the histogram's bar) under the pointer.
science-cytometry-events = { $count ->
    [one] { $count } zdarzenie
    [few] { $count } zdarzenia
    [many] { $count } zdarzeń
   *[other] { $count } zdarzenia
}
# The plot's axis scales ($x, $y: Linear, Log, Logicle or Arcsinh).
science-cytometry-scales = x { $x }, y { $y }
science-cytometry-scale-x = x { $x }
# Spillover compensation is applied, from the file's keyword ($keyword, e.g. $SPILLOVER).
science-cytometry-compensated = Skompensowano ({ $keyword })
science-cytometry-uncompensated = Bez kompensacji
# X-ray diffraction frames: what lies under the pointer. q is the momentum transfer
# (per ångström), d the lattice spacing (ångström), 2θ the scattering angle. The symbols
# stay as they are: "q 0.512 Å⁻¹  d 12.27 Å  2θ 4.56°".
science-scatter = q { $q } Å⁻¹  d { $d } Å  2θ { $tth }°
# Seismic readout: the trace under the pointer in a 3-D volume ($inline, $crossline the
# header numbers) or a 2-D line; $trace counts from 1 in file order, $cdp is the CDP number.
science-seismic-cell = Inline { $inline }  Crossline { $crossline }
science-seismic-trace = Trasa { $trace }  CDP { $cdp }
# A cell of a 3-D volume with no trace in the file.
science-seismic-gap = Brak trasy
# Two-way time (ms) or depth of the row under the pointer.
science-seismic-time = Czas { $time } ms
science-seismic-depth = Głębokość { $depth } { $unit }
# The trace's CDP (or source) X and Y with the coordinate scalar applied.
science-seismic-xy = X { $x }  Y { $y }
# machine-drafted, needs native review
# Unstructured mesh readout: the face (cell) under the pointer, counting from 0 as in the
# file's connectivity, and the node nearest to the pointer when the values sit on nodes.
science-mesh-face = Ściana { $face }
science-mesh-face-node = Ściana { $face }  Węzeł { $node }
# The pointer lies between the mesh's faces (land in an ocean mesh, outside its boundary).
science-mesh-outside = Poza siatką
# Position on an axis past the image: "Plane 3 of 40", or with its world value.
science-plane = { $axis } { $index } z { $count }
science-plane-world = { $axis } { $index } z { $count } ({ $world })
science-axis-unnamed = Płaszczyzna
science-axis-extension = Obraz
science-axis-channel = Kanał
# A column of a HEALPix sky map (probability, distance…) shown as the image.
science-axis-field = Pole
# X-ray event lists binned into an image: the bin size, in the event list's sky pixels
# ("Bin 2 of 4 (8 px)"), and the energy band of the events counted.
science-axis-bin = Bin
science-bin-size = { $size } px
science-axis-band = Energia
science-band-all = wszystkie energie
# machine-drafted, needs native review
# Complex-valued arrays (NumPy, MATLAB, Zarr, TDMS): which part of each value is shown.
science-axis-part = Część
science-part-magnitude = moduł
science-part-real = część rzeczywista
science-part-imaginary = część urojona
science-part-phase = faza (radiany)
# machine-drafted, needs native review
# Complex radar (SAR) images: intensity in decibels.
science-part-db = intensywność (dB)
# machine-drafted, needs native review
# The frames of a thermal sequence (FLIR SEQ/CSQ), in time.
science-axis-frame = Klatka
science-readout = Wartość i współrzędne
# Accessible names of the plane sliders.
science-sliders = Płaszczyzny
# The scale bar's length.
science-scale-arcsec = { $value }″
science-scale-arcmin = { $value }′
science-scale-deg = { $value }°
# A physical length, e.g. "20 nm" (electron microscopy images).
science-scale-length = { $value } { $unit }
# machine-drafted, needs native review
# A length along a named axis of a plot, e.g. "20 GeV (p_{T})".
science-scale-axis = { $length } ({ $axis })
science-scale-a11y = Podziałka: { $length }
# Mass spectrometry images (imzML). The image is the total ion current (each spot's summed
# intensities) until an m/z is typed; then that ion's image, its intensities summed over
# m/z ± the tolerance. $window is e.g. "760.5851 ± 5 ppm".
science-ion-tic = Całkowity prąd jonowy
science-ion-window = Obraz jonowy m/z { $window }
# The m/z prompt: $text is what is typed so far (opens on the base peak under the pointer).
science-ion-prompt = Obraz jonowy dla m/z: { $text }▏
# $tolerance is e.g. "10 ppm" or "0.1 Da"; a tolerance can also be typed after the m/z
# ("760.585 5 ppm"). Tab steps the tolerance; Enter on an empty prompt shows the total ion
# current again.
science-ion-prompt-hint = ± { $tolerance } · Tab tolerancja · Enter pokaż · Esc anuluj
science-ion-bad = To nie jest m/z: { $text }
# The spot under the pointer as the file numbers it (from 1).
science-ms-spot = Punkt x { $x }  y { $y }
science-ms-spot-z = Punkt x { $x }  y { $y }  z { $z }
science-ms-no-spectrum = Brak widma w tym punkcie
# machine-drafted, needs native review
palette-science-temperature-unit = Jednostka temperatury (°C, °F, K)
palette-science-isotherm = Izoterma powyżej lub poniżej temperatury pod wskaźnikiem
palette-science-thermal-parameters = Emisyjność i temperatura odbita…
palette-science-export-temperatures = Eksportuj temperatury (zmiennoprzecinkowy TIFF lub CSV)…
palette-thermal-visible = Zdjęcie w świetle widzialnym dla termogramu
disabled-not-thermal = Tylko dla termogramów
disabled-not-radiometric = Plik przechowuje temperatury: jego parametrów nie można zmienić
disabled-no-visible-photo = Termogram nie ma zdjęcia w świetle widzialnym
science-temperature-unit-toast = Temperatury w { $unit }
science-isotherm-above = Izoterma powyżej { $level }
science-isotherm-below = Izoterma poniżej { $level }
science-isotherm-off = Izoterma wyłączona
science-thermal-emissivity-prompt = Emisyjność: { $text }
science-thermal-reflected-prompt = Temperatura odbita ({ $unit }): { $text }
science-thermal-prompt-hint = Tab następne pole · Enter przelicz · Esc anuluj
science-thermal-bad = Nieprawidłowa wartość: { $text }
science-thermal-applied = Przeliczono dla emisyjności { $emissivity }, temperatury odbitej { $reflected }
science-thermal-conditions = Emisyjność { $emissivity } · odbita { $reflected }
science-thermal-export-tiff = 32-bitowy zmiennoprzecinkowy TIFF
science-thermal-export-csv = CSV
science-thermal-export-done = Zapisano temperatury do { $name }
science-thermal-export-failed = Nie można zapisać temperatur: { $reason }
# machine-drafted, needs native review
science-values-failed = Nie można pokazać wartości: { $reason }
# machine-drafted, needs native review
# Thermal sequences (FLIR SEQ/CSQ): playback and each measurement over time.
palette-science-thermal-play = Odtwórz lub wstrzymaj sekwencję termowizyjną
palette-science-thermal-series-csv = Zapisz temperatury w czasie (CSV)…
disabled-not-thermal-sequence = Tylko dla sekwencji termowizyjnych
thermal-seq-title = Temperatury w czasie
# $time is seconds from the first frame, e.g. "0.12 s".
thermal-seq-frame = Klatka { $frame } z { $frames } · { $time }
thermal-seq-play = Odtwórz
thermal-seq-pause = Wstrzymaj
thermal-seq-playing = Odtwarzanie
thermal-seq-paused = Wstrzymano
thermal-seq-save-csv = Zapisz CSV…
thermal-seq-hint = Dodaj punkt pomiarowy, linię lub obszar (M), aby wykreślić jego przebieg w czasie.
thermal-seq-reading = Odczyt klatki { $done } z { $total }…
thermal-seq-failed = Nie można odczytać klatek: { $reason }
thermal-seq-no-series = Na razie nie ma czego zapisać: dodaj punkt pomiarowy, linię lub obszar.
# For screen readers: each measurement's value at the frame shown, e.g.
# "1 Spot: 36.4 °C, 2 Rectangle: 30.1 °C".
thermal-seq-chart = Wykres: { $values }
# machine-drafted, needs native review
# The report (PDF) of the image shown: a thermal report for a thermal image, an inspection report for any other.
palette-thermal-report = Raport (PDF)…
thermal-report-heading = Raport termowizyjny
thermal-report-default-title = Raport z badania termowizyjnego
thermal-report-title-label = Tytuł
thermal-report-notes-label = Uwagi
thermal-report-photo = Dołącz zdjęcie w świetle widzialnym
thermal-report-folder = Dodaj pozostałe termogramy z tego folderu
thermal-report-save = Zapisz PDF…
thermal-report-busy = Tworzenie raportu…
thermal-report-nothing = Temperatury nie są jeszcze wyświetlane.
thermal-report-saved = Raport zapisano w { $name } ({ $count ->
    [one] { $count } obraz
    [few] { $count } obrazy
    [many] { $count } obrazów
   *[other] { $count } obrazu
})
thermal-report-failed = Nie można zapisać raportu: { $reason }
# In the PDF.
thermal-report-notes = Uwagi
thermal-report-measurements = Pomiary
thermal-report-no-measurements = Brak pomiarów.
thermal-report-thermal-caption = Termogram
thermal-report-visible-caption = Zdjęcie w świetle widzialnym
thermal-report-col-measurement = Pomiar
thermal-report-col-min = Min.
thermal-report-col-max = Maks.
thermal-report-col-mean = Średnia
thermal-report-col-emissivity = Emisyjność
thermal-report-whole-image = Cały obraz
thermal-report-camera = Kamera
thermal-report-lens = Obiektyw
thermal-report-date = Wykonano
thermal-report-place = Miejsce
thermal-report-frame = Klatka
thermal-report-emissivity = Emisyjność
thermal-report-reflected = Temperatura odbita
thermal-report-distance = Odległość
thermal-report-humidity = Wilgotność
thermal-report-range = Zakres
thermal-report-format = Format
thermal-report-page = Strona { $page } z { $pages }
