# Specular — Scientific images (FITS, XISF): stretch, limits, planes, channels, coordinates. Italiano.

keys-mode-science = Immagini scientifiche
cheatsheet-science = Immagini scientifiche
cheatsheet-science-look = Stretch e limiti
cheatsheet-science-planes = Piani e canali
# machine-drafted, needs native review
cheatsheet-science-channels-1-9 = Mostra o nascondi i canali da 1 a 9
cheatsheet-science-channels-10-16 = Mostra o nascondi i canali da 10 a 16
palette-science-stretch = Stretch successivo
palette-science-limits = Limiti successivi
palette-science-next-plane = Piano successivo
palette-science-prev-plane = Piano precedente
palette-science-next-axis = Scorri un altro asse
palette-science-composite = Componi i canali a colori
palette-science-channel-1 = Mostra o nascondi il canale 1
palette-science-channel-2 = Mostra o nascondi il canale 2
palette-science-channel-3 = Mostra o nascondi il canale 3
palette-science-channel-4 = Mostra o nascondi il canale 4
palette-science-channel-5 = Mostra o nascondi il canale 5
palette-science-channel-6 = Mostra o nascondi il canale 6
palette-science-channel-7 = Mostra o nascondi il canale 7
palette-science-channel-8 = Mostra o nascondi il canale 8
palette-science-channel-9 = Mostra o nascondi il canale 9
# machine-drafted, needs native review
palette-science-channel-10 = Mostra o nascondi il canale 10
palette-science-channel-11 = Mostra o nascondi il canale 11
palette-science-channel-12 = Mostra o nascondi il canale 12
palette-science-channel-13 = Mostra o nascondi il canale 13
palette-science-channel-14 = Mostra o nascondi il canale 14
palette-science-channel-15 = Mostra o nascondi il canale 15
palette-science-channel-16 = Mostra o nascondi il canale 16
palette-science-grid = Mostra o nascondi la griglia delle coordinate
palette-science-reset = Aspetto automatico
palette-science-level = Livella le mappe di altezza (sottrai il piano medio)
palette-science-wiggle = Tracce sismiche wiggle ad area variabile, sopra i colori
# machine-drafted, needs native review
palette-science-wireframe = Spigoli della mesh non strutturata (wireframe), sopra i colori
palette-science-import-lut = Tabella dei colori da un file (ImageJ .lut)…
palette-science-hillshade = Ombreggia il rilievo dei modelli di elevazione (hillshade)
palette-science-fft = Spettro di potenza (FFT) del piano
palette-science-catalogue = Sovrapponi un catalogo di sorgenti (VOTable, tabella FITS, CSV)…
palette-science-clear-catalogues = Rimuovi i cataloghi sovrapposti
# A catalogue overlaid on a sky image: its name (the table's or the file's), how many of its
# sources fall on the image, and how many of those are drawn (at most a set number).
science-catalogue-toast = { $name }: { $count ->
        [one] una sorgente nell’immagine
       *[other] { $count } sorgenti nell’immagine
    }
science-catalogue-capped = { $name }: mostrate { $shown } di { $count } sorgenti nell’immagine
science-catalogue-none = { $name }: nessuna sorgente nell’immagine
science-catalogue-failed = Non è un catalogo con posizioni celesti: { $name }
science-catalogues-cleared = Cataloghi rimossi
# The file dialog's filter name for catalogue files.
science-catalogue-filter = Cataloghi di sorgenti
# A GW sky map's credible region: the smallest area holding this share of the probability,
# drawn as a contour labelled e.g. "90 %".
science-credible = { PERCENT($percent) }
palette-science-ion-image = Immagine ionica a un valore m/z (spettrometria di massa)…
disabled-not-science = Solo per immagini scientifiche (astronomia, microscopia elettronica e a scansione di sonda)
disabled-one-plane = L’immagine ha un solo piano
disabled-one-axis = L’immagine non ha altri assi da scorrere
disabled-no-channels = L’immagine non ha questo canale da comporre
disabled-no-coordinates = L’immagine non ha coordinate celesti
disabled-not-height-map = Solo per mappe di altezza (Gwyddion, Igor)
disabled-not-elevation = Solo per modelli di elevazione georeferenziati a una banda
disabled-not-seismic = Solo per dati sismici (SEG-Y, Seismic Unix)
disabled-not-mass-spec = Solo per immagini di spettrometria di massa (imzML)
# machine-drafted, needs native review
disabled-not-ugrid = Solo per mesh non strutturate (NetCDF UGRID, ICON e MPAS)
disabled-not-bayer = Solo per fotogrammi grezzi di camere a colori (FITS con schema Bayer, video SER)
palette-debayer = Mostra i fotogrammi delle camere a colori con debayer o grezzi
palette-bayer-pattern = Schema Bayer successivo per questo file
science-debayer-on = Con debayer ({ $pattern })
science-debayer-off = Matrice di filtri colore grezza
science-bayer-pattern = Schema Bayer { $pattern }
# machine-drafted, needs native review
palette-annotations = Mostra o nascondi le annotazioni
disabled-no-annotations = L’immagine non ha annotazioni accanto (NDPA, Aperio o ASAP XML)
science-annotations-on = Annotazioni visibili
science-annotations-off = Annotazioni nascoste
science-stretch-auto = Automatico (per colore)
# Stretches: how values between the limits map to grey levels.
science-stretch-linear = Lineare
science-stretch-sqrt = Radice quadrata
science-stretch-log = Logaritmico
science-stretch-asinh = Asinh
science-stretch-histeq = Equalizzazione dell’istogramma
# Limits: which values show black and white. $percent is e.g. 99.5.
science-limits-zscale = Zscale
science-limits-minmax = Min/max
science-limits-percentile = { PERCENT($percent) }
# Seismic gain: limits symmetric about zero at the $percent percentile of the amplitudes.
science-limits-symmetric = ±{ PERCENT($percent) }
# The display range the file stores (ImageJ's min and max), used until other limits are chosen.
science-limits-file = Intervallo del file
# Toasts when the look changes.
science-stretch-toast = Stretch: { $stretch }
science-limits-toast = Limiti: { $limits }
science-reset-toast = Aspetto automatico
# An imported colour table: $name is its file name.
science-lut-toast = Tabella dei colori { $name }
science-lut-failed = Non è una tabella dei colori: { $name }
# The file dialog's filter name for .lut files.
science-lut-filter = Tabelle dei colori ImageJ
science-composite-on = Canali composti a colori
science-composite-off = Un canale in grigio
science-channel-on = { $channel } visibile
science-channel-off = { $channel } nascosto
# machine-drafted, needs native review
# A channel without a name in the file, in the shown/hidden toasts.
science-channel-unnamed = Canale { $number }
science-grid-on = Griglia delle coordinate visibile
science-grid-off = Griglia delle coordinate nascosta
# X-ray diffraction frames: the same action shows circles of equal resolution (d-spacing).
science-rings-on = Anelli di risoluzione visibili
science-rings-off = Anelli di risoluzione nascosti
# A resolution ring's label: its d-spacing in ångström, e.g. "2.5 Å".
science-ring = { $d } Å
# Seismic wiggle traces: drawn once the traces are at least 4 screen pixels apart.
science-wiggle-on = Tracce wiggle attive (ingrandisci per vederle)
science-wiggle-off = Tracce wiggle disattivate
# machine-drafted, needs native review
# Unstructured meshes: the edges drawn over the faces; too many to draw in view until the
# user zooms in.
science-wireframe-on = Spigoli della mesh attivati
science-wireframe-off = Spigoli della mesh disattivati
science-wireframe-dense = Spigoli della mesh attivati (ingrandisci per vederli)
# Plane levelling (scanning probe height maps): the best-fit plane is subtracted for display.
science-level-on = Livellamento del piano attivo
science-level-off = Livellamento del piano disattivato
# Relief shading of an elevation model. $azimuth is the sun's direction in degrees clockwise
# from north, $altitude its height above the horizon in degrees.
science-hillshade-on = Hillshade attivo: sole a { $azimuth }°, { $altitude }° di altezza
science-hillshade-off = Hillshade disattivato
# The power spectrum view: the plane's centred 2-D Fourier power, log scaled.
science-fft-on = Spettro di potenza visibile
science-fft-off = Spettro di potenza nascosto
science-axis-toast = Scorrimento: { $axis }
# Readout lines (bottom corner).
# The value under the pointer, with the values' unit when the file states one.
science-value = Valore { $value }
science-value-unit = Valore { $value } { $unit }
# machine-drafted, needs native review
science-value-flags = Valore { $value } (bit { $bits })
# machine-drafted, needs native review
# A class value under the pointer and its name, e.g. "Value 4 · Vegetation".
science-value-class = Valore { $value } · { $class }
# machine-drafted, needs native review
# Sentinel-2 scene classes (L2A SCL, ESA's names) and L1C cloud masks.
s2-class-no-data = Nessun dato
s2-class-defective = Saturato o difettoso
s2-class-dark = Pixel di aree scure
s2-class-cloud-shadow = Ombre di nubi
s2-class-vegetation = Vegetazione
s2-class-not-vegetated = Non vegetato
s2-class-water = Acqua
s2-class-unclassified = Non classificato
s2-class-cloud-medium = Nubi a media probabilità
s2-class-cloud-high = Nubi ad alta probabilità
s2-class-cirrus = Cirri sottili
s2-class-snow = Neve o ghiaccio
s2-class-clear = Sereno
s2-class-opaque-cloud = Nubi opache
# Several channels under the pointer: "R 12.5  G 3.1  B 0.4".
science-values = { $values }
# Sky position under the pointer, sexagesimal then degrees.
science-radec = AR { $ra }  Dec { $dec }
science-radec-deg = { $ra }°  { $dec }°
# Galactic or ecliptic longitude and latitude.
science-lonlat = l { $lon }°  b { $lat }°
science-wcs-unsupported = WCS non supportato: { $reason }
# machine-drafted, needs native review
# A B1950 (FK4) image: what is converted between its frame and another. Its own position
# stays B1950 in the readout. ICRS, FK4 and SAMP are names and stay as they are.
science-fk4-markers = Indicatori del catalogo: ICRS → FK4 convertito
science-fk4-markers-galactic = Indicatori del catalogo: galattico → FK4 convertito
science-fk4-samp = SAMP: FK4 → ICRS convertito
# The look: stretch and limits.
science-look = { $stretch } · { $limits }
# Shown under the look while plane levelling is on.
science-levelled = Piano livellato
# Shown under the look while the hillshade is on.
science-hillshaded = Hillshade: sole a { $azimuth }°, { $altitude }° di altezza
# Shown in the readout while the power spectrum is shown.
science-fft = Spettro di potenza (log₁₀)
# The spectrum's value under the pointer: log₁₀ of the power.
science-fft-power = log₁₀ potenza { $value }
# Spatial frequency under the pointer along x and y, and its unit ("1/µm", "1/px").
science-fft-frequency = Frequenza u { $u }  v { $v } { $unit }
# The period of that frequency: "Period 2.5 nm".
science-fft-period = Periodo { $period }
# Crystal orientation under the pointer (EBSD): Bunge Euler angles in degrees.
science-euler = φ₁ { $phi1 }  Φ { $phi }  φ₂ { $phi2 }
# The crystal phase under the pointer and its symmetry ("Iron fcc (m-3m)").
science-phase = Fase { $phase } ({ $symmetry })
# Title of the spectrum plot of the pixel under the pointer (an EDS map).
science-spectrum-title = Spettro in { $x }, { $y }
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
        [one] { $count } bin di underflow/overflow nascosto (somma { $sum })
        [many] { $count } bin di underflow/overflow nascosti (somma { $sum })
       *[other] { $count } bin di underflow/overflow nascosti (somma { $sum })
    }
# Bins of different widths are drawn on an even grid of pixels.
science-root-resampled = Bin a larghezza variabile ricampionati su una griglia uniforme
# machine-drafted, needs native review
# A flow cytometry plot (FCS) under the pointer: each axis's parameter and its value.
science-cytometry-at = { $x } { $xv }  { $y } { $yv }
# A cytometry histogram under the pointer: its parameter and the value.
science-cytometry-at-x = { $x } { $xv }
# Events in the plot's bin (or the histogram's bar) under the pointer.
science-cytometry-events = { $count ->
    [one] { $count } evento
    [many] { $count } di eventi
   *[other] { $count } eventi
}
# The plot's axis scales ($x, $y: Linear, Log, Logicle or Arcsinh).
science-cytometry-scales = x { $x }, y { $y }
science-cytometry-scale-x = x { $x }
# Spillover compensation is applied, from the file's keyword ($keyword, e.g. $SPILLOVER).
science-cytometry-compensated = Compensato ({ $keyword })
science-cytometry-uncompensated = Non compensato
# X-ray diffraction frames: what lies under the pointer. q is the momentum transfer
# (per ångström), d the lattice spacing (ångström), 2θ the scattering angle. The symbols
# stay as they are: "q 0.512 Å⁻¹  d 12.27 Å  2θ 4.56°".
science-scatter = q { $q } Å⁻¹  d { $d } Å  2θ { $tth }°
# Seismic readout: the trace under the pointer in a 3-D volume ($inline, $crossline the
# header numbers) or a 2-D line; $trace counts from 1 in file order, $cdp is the CDP number.
science-seismic-cell = Inline { $inline }  Crossline { $crossline }
science-seismic-trace = Traccia { $trace }  CDP { $cdp }
# A cell of a 3-D volume with no trace in the file.
science-seismic-gap = Nessuna traccia
# Two-way time (ms) or depth of the row under the pointer.
science-seismic-time = Tempo { $time } ms
science-seismic-depth = Profondità { $depth } { $unit }
# The trace's CDP (or source) X and Y with the coordinate scalar applied.
science-seismic-xy = X { $x }  Y { $y }
# machine-drafted, needs native review
# Unstructured mesh readout: the face (cell) under the pointer, counting from 0 as in the
# file's connectivity, and the node nearest to the pointer when the values sit on nodes.
science-mesh-face = Faccia { $face }
science-mesh-face-node = Faccia { $face }  Nodo { $node }
# The pointer lies between the mesh's faces (land in an ocean mesh, outside its boundary).
science-mesh-outside = Fuori dalla mesh
# Position on an axis past the image: "Plane 3 of 40", or with its world value.
science-plane = { $axis } { $index } di { $count }
science-plane-world = { $axis } { $index } di { $count } ({ $world })
science-axis-unnamed = Piano
science-axis-extension = Immagine
science-axis-channel = Canale
# A column of a HEALPix sky map (probability, distance…) shown as the image.
science-axis-field = Campo
# X-ray event lists binned into an image: the bin size, in the event list's sky pixels
# ("Bin 2 of 4 (8 px)"), and the energy band of the events counted.
science-axis-bin = Bin
science-bin-size = { $size } px
science-axis-band = Energia
science-band-all = tutte le energie
# machine-drafted, needs native review
# Complex-valued arrays (NumPy, MATLAB, Zarr, TDMS): which part of each value is shown.
science-axis-part = Parte
science-part-magnitude = modulo
science-part-real = parte reale
science-part-imaginary = parte immaginaria
science-part-phase = fase (radianti)
# machine-drafted, needs native review
# Complex radar (SAR) images: intensity in decibels.
science-part-db = intensità (dB)
# machine-drafted, needs native review
# The frames of a thermal sequence (FLIR SEQ/CSQ), in time.
science-axis-frame = Fotogramma
science-readout = Valore e coordinate
# Accessible names of the plane sliders.
science-sliders = Piani
# The scale bar's length.
science-scale-arcsec = { $value }″
science-scale-arcmin = { $value }′
science-scale-deg = { $value }°
# A physical length, e.g. "20 nm" (electron microscopy images).
science-scale-length = { $value } { $unit }
# machine-drafted, needs native review
# A length along a named axis of a plot, e.g. "20 GeV (p_{T})".
science-scale-axis = { $length } ({ $axis })
science-scale-a11y = Barra di scala: { $length }
# Mass spectrometry images (imzML). The image is the total ion current (each spot's summed
# intensities) until an m/z is typed; then that ion's image, its intensities summed over
# m/z ± the tolerance. $window is e.g. "760.5851 ± 5 ppm".
science-ion-tic = Corrente ionica totale
science-ion-window = Immagine ionica m/z { $window }
# The m/z prompt: $text is what is typed so far (opens on the base peak under the pointer).
science-ion-prompt = Immagine ionica a m/z: { $text }▏
# $tolerance is e.g. "10 ppm" or "0.1 Da"; a tolerance can also be typed after the m/z
# ("760.585 5 ppm"). Tab steps the tolerance; Enter on an empty prompt shows the total ion
# current again.
science-ion-prompt-hint = ± { $tolerance } · Tab tolleranza · Invio mostra · Esc annulla
science-ion-bad = Non è un valore m/z: { $text }
# The spot under the pointer as the file numbers it (from 1).
science-ms-spot = Punto x { $x }  y { $y }
science-ms-spot-z = Punto x { $x }  y { $y }  z { $z }
science-ms-no-spectrum = Nessuno spettro in questo punto
# machine-drafted, needs native review
palette-science-temperature-unit = Unità di temperatura (°C, °F, K)
palette-science-isotherm = Isoterma sopra o sotto la temperatura sotto il puntatore
palette-science-thermal-parameters = Emissività e temperatura riflessa…
palette-science-export-temperatures = Esporta temperature (TIFF in virgola mobile o CSV)…
palette-thermal-visible = Foto visibile di un’immagine termica
disabled-not-thermal = Solo per immagini termiche
disabled-not-radiometric = Il file memorizza temperature: i suoi parametri non si possono modificare
disabled-no-visible-photo = L’immagine termica non ha una foto visibile
science-temperature-unit-toast = Temperature in { $unit }
science-isotherm-above = Isoterma sopra { $level }
science-isotherm-below = Isoterma sotto { $level }
science-isotherm-off = Isoterma disattivata
science-thermal-emissivity-prompt = Emissività: { $text }
science-thermal-reflected-prompt = Temperatura riflessa ({ $unit }): { $text }
science-thermal-prompt-hint = Tab campo successivo · Invio ricalcola · Esc annulla
science-thermal-bad = Valore non valido: { $text }
science-thermal-applied = Ricalcolato per emissività { $emissivity }, riflessa { $reflected }
science-thermal-conditions = Emissività { $emissivity } · riflessa { $reflected }
science-thermal-export-tiff = TIFF in virgola mobile a 32 bit
science-thermal-export-csv = CSV
science-thermal-export-done = Temperature salvate in { $name }
science-thermal-export-failed = Impossibile salvare le temperature: { $reason }
# machine-drafted, needs native review
science-values-failed = Impossibile mostrare i valori: { $reason }
# machine-drafted, needs native review
# Thermal sequences (FLIR SEQ/CSQ): playback and each measurement over time.
palette-science-thermal-play = Riproduci o metti in pausa la sequenza termica
palette-science-thermal-series-csv = Salva le temperature nel tempo (CSV)…
disabled-not-thermal-sequence = Solo per sequenze termiche
thermal-seq-title = Temperature nel tempo
# $time is seconds from the first frame, e.g. "0.12 s".
thermal-seq-frame = Fotogramma { $frame } di { $frames } · { $time }
thermal-seq-play = Riproduci
thermal-seq-pause = Pausa
thermal-seq-playing = In riproduzione
thermal-seq-paused = In pausa
thermal-seq-save-csv = Salva CSV…
thermal-seq-hint = Aggiungi un punto, una linea o un’area (M) per tracciarne l’andamento nel tempo.
thermal-seq-reading = Lettura del fotogramma { $done } di { $total }…
thermal-seq-failed = Impossibile leggere i fotogrammi: { $reason }
thermal-seq-no-series = Ancora niente da salvare: aggiungi un punto, una linea o un’area.
# For screen readers: each measurement's value at the frame shown, e.g.
# "1 Spot: 36.4 °C, 2 Rectangle: 30.1 °C".
thermal-seq-chart = Grafico: { $values }
# machine-drafted, needs native review
# The report (PDF) of the image shown: a thermal report for a thermal image, an inspection report for any other.
palette-thermal-report = Rapporto (PDF)…
thermal-report-heading = Rapporto termografico
thermal-report-default-title = Rapporto di ispezione termografica
thermal-report-title-label = Titolo
thermal-report-notes-label = Note
thermal-report-photo = Includi la foto visibile
thermal-report-folder = Aggiungi le altre immagini termiche di questa cartella
thermal-report-save = Salva PDF…
thermal-report-busy = Scrittura del rapporto…
thermal-report-nothing = Le temperature non sono ancora visualizzate.
thermal-report-saved = Rapporto salvato in { $name } ({ $count ->
    [one] { $count } immagine
    [many] { $count } immagini
   *[other] { $count } immagini
})
thermal-report-failed = Impossibile salvare il rapporto: { $reason }
# In the PDF.
thermal-report-notes = Note
thermal-report-measurements = Misure
thermal-report-no-measurements = Nessuna misura.
thermal-report-thermal-caption = Immagine termica
thermal-report-visible-caption = Foto visibile
thermal-report-col-measurement = Misura
thermal-report-col-min = Min
thermal-report-col-max = Max
thermal-report-col-mean = Media
thermal-report-col-emissivity = Emissività
thermal-report-whole-image = Immagine intera
thermal-report-camera = Fotocamera
thermal-report-lens = Obiettivo
thermal-report-date = Scattata
thermal-report-place = Luogo
thermal-report-frame = Fotogramma
thermal-report-emissivity = Emissività
thermal-report-reflected = Temperatura riflessa
thermal-report-distance = Distanza
thermal-report-humidity = Umidità
thermal-report-range = Intervallo
thermal-report-format = Formato
thermal-report-page = Pagina { $page } di { $pages }
