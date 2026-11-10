# Specular — Scientific images (FITS, XISF): stretch, limits, planes, channels, coordinates. Español.

keys-mode-science = Imágenes científicas
cheatsheet-science = Imágenes científicas
cheatsheet-science-look = Estiramiento y límites
cheatsheet-science-planes = Planos y canales
# machine-drafted, needs native review
cheatsheet-science-channels-1-9 = Mostrar u ocultar los canales 1 a 9
cheatsheet-science-channels-10-16 = Mostrar u ocultar los canales 10 a 16
palette-science-stretch = Estiramiento siguiente
palette-science-limits = Límites siguientes
palette-science-next-plane = Plano siguiente
palette-science-prev-plane = Plano anterior
palette-science-next-axis = Recorrer otro eje
palette-science-composite = Combinar los canales en color
palette-science-channel-1 = Mostrar u ocultar el canal 1
palette-science-channel-2 = Mostrar u ocultar el canal 2
palette-science-channel-3 = Mostrar u ocultar el canal 3
palette-science-channel-4 = Mostrar u ocultar el canal 4
palette-science-channel-5 = Mostrar u ocultar el canal 5
palette-science-channel-6 = Mostrar u ocultar el canal 6
palette-science-channel-7 = Mostrar u ocultar el canal 7
palette-science-channel-8 = Mostrar u ocultar el canal 8
palette-science-channel-9 = Mostrar u ocultar el canal 9
# machine-drafted, needs native review
palette-science-channel-10 = Mostrar u ocultar el canal 10
palette-science-channel-11 = Mostrar u ocultar el canal 11
palette-science-channel-12 = Mostrar u ocultar el canal 12
palette-science-channel-13 = Mostrar u ocultar el canal 13
palette-science-channel-14 = Mostrar u ocultar el canal 14
palette-science-channel-15 = Mostrar u ocultar el canal 15
palette-science-channel-16 = Mostrar u ocultar el canal 16
palette-science-grid = Mostrar u ocultar la cuadrícula de coordenadas
palette-science-reset = Aspecto automático
palette-science-level = Nivelar mapas de altura (restar el plano medio)
palette-science-wiggle = Trazas sísmicas wiggle con área variable, sobre los colores
# machine-drafted, needs native review
palette-science-wireframe = Aristas de la malla no estructurada (alámbrico), sobre los colores
palette-science-import-lut = Tabla de colores desde un archivo (ImageJ .lut)…
palette-science-hillshade = Sombrear el relieve de los modelos de elevación (sombreado)
palette-science-fft = Espectro de potencia (FFT) del plano
palette-science-catalogue = Superponer un catálogo de fuentes (VOTable, tabla FITS, CSV)…
palette-science-clear-catalogues = Quitar los catálogos superpuestos
# A catalogue overlaid on a sky image: its name (the table's or the file's), how many of its
# sources fall on the image, and how many of those are drawn (at most a set number).
science-catalogue-toast = { $name }: { $count ->
        [one] una fuente en la imagen
       *[other] { $count } fuentes en la imagen
    }
science-catalogue-capped = { $name }: se muestran { $shown } de { $count } fuentes en la imagen
science-catalogue-none = { $name }: ninguna fuente en la imagen
science-catalogue-failed = No es un catálogo con posiciones celestes: { $name }
science-catalogues-cleared = Catálogos quitados
# The file dialog's filter name for catalogue files.
science-catalogue-filter = Catálogos de fuentes
# A GW sky map's credible region: the smallest area holding this share of the probability,
# drawn as a contour labelled e.g. "90 %".
science-credible = { PERCENT($percent) }
palette-science-ion-image = Imagen iónica a una m/z (espectrometría de masas)…
disabled-not-science = Solo para imágenes científicas (astronomía, microscopía electrónica y de sonda de barrido)
disabled-one-plane = La imagen tiene un solo plano
disabled-one-axis = La imagen no tiene otro eje que recorrer
disabled-no-channels = La imagen no tiene ese canal para combinar
disabled-no-coordinates = La imagen no tiene coordenadas celestes
disabled-not-height-map = Solo para mapas de altura (Gwyddion, Igor)
disabled-not-elevation = Solo para modelos de elevación georreferenciados de una banda
disabled-not-seismic = Solo para datos sísmicos (SEG-Y, Seismic Unix)
disabled-not-mass-spec = Solo para imágenes de espectrometría de masas (imzML)
# machine-drafted, needs native review
disabled-not-ugrid = Solo para mallas no estructuradas (NetCDF UGRID, ICON y MPAS)
disabled-not-bayer = Solo para fotogramas en bruto de cámaras en color (FITS con patrón Bayer, vídeo SER)
palette-debayer = Mostrar los fotogramas de cámara en color interpolados o en bruto
palette-bayer-pattern = Siguiente patrón Bayer para este archivo
science-debayer-on = Interpolado ({ $pattern })
science-debayer-off = Matriz de filtros de color en bruto
science-bayer-pattern = Patrón Bayer { $pattern }
# machine-drafted, needs native review
palette-annotations = Mostrar u ocultar las anotaciones
disabled-no-annotations = La imagen no tiene anotaciones junto a ella (NDPA, Aperio o ASAP XML)
science-annotations-on = Anotaciones visibles
science-annotations-off = Anotaciones ocultas
science-stretch-auto = Automático (por color)
# Stretches: how values between the limits map to grey levels.
science-stretch-linear = Lineal
science-stretch-sqrt = Raíz cuadrada
science-stretch-log = Logarítmico
science-stretch-asinh = Asinh
science-stretch-histeq = Ecualización del histograma
# Limits: which values show black and white. $percent is e.g. 99.5.
science-limits-zscale = Zscale
science-limits-minmax = Mín/máx
science-limits-percentile = { PERCENT($percent) }
# Seismic gain: limits symmetric about zero at the $percent percentile of the amplitudes.
science-limits-symmetric = ±{ PERCENT($percent) }
# The display range the file stores (ImageJ's min and max), used until other limits are chosen.
science-limits-file = Rango del archivo
# Toasts when the look changes.
science-stretch-toast = Estiramiento: { $stretch }
science-limits-toast = Límites: { $limits }
science-reset-toast = Aspecto automático
# An imported colour table: $name is its file name.
science-lut-toast = Tabla de colores { $name }
science-lut-failed = No es una tabla de colores: { $name }
# The file dialog's filter name for .lut files.
science-lut-filter = Tablas de colores de ImageJ
science-composite-on = Canales combinados en color
science-composite-off = Un canal en gris
science-channel-on = { $channel } visible
science-channel-off = { $channel } oculto
# machine-drafted, needs native review
# A channel without a name in the file, in the shown/hidden toasts.
science-channel-unnamed = Canal { $number }
science-grid-on = Cuadrícula de coordenadas visible
science-grid-off = Cuadrícula de coordenadas oculta
# X-ray diffraction frames: the same action shows circles of equal resolution (d-spacing).
science-rings-on = Anillos de resolución visibles
science-rings-off = Anillos de resolución ocultos
# A resolution ring's label: its d-spacing in ångström, e.g. "2.5 Å".
science-ring = { $d } Å
# Seismic wiggle traces: drawn once the traces are at least 4 screen pixels apart.
science-wiggle-on = Trazas wiggle activadas (acerque el zoom para verlas)
science-wiggle-off = Trazas wiggle desactivadas
# machine-drafted, needs native review
# Unstructured meshes: the edges drawn over the faces; too many to draw in view until the
# user zooms in.
science-wireframe-on = Aristas de la malla activadas
science-wireframe-off = Aristas de la malla desactivadas
science-wireframe-dense = Aristas de la malla activadas (acerque para verlas)
# Plane levelling (scanning probe height maps): the best-fit plane is subtracted for display.
science-level-on = Nivelación del plano activada
science-level-off = Nivelación del plano desactivada
# Relief shading of an elevation model. $azimuth is the sun's direction in degrees clockwise
# from north, $altitude its height above the horizon in degrees.
science-hillshade-on = Sombreado activado: sol a { $azimuth }°, { $altitude }° de altura
science-hillshade-off = Sombreado desactivado
# The power spectrum view: the plane's centred 2-D Fourier power, log scaled.
science-fft-on = Espectro de potencia visible
science-fft-off = Espectro de potencia oculto
science-axis-toast = Recorriendo: { $axis }
# Readout lines (bottom corner).
# The value under the pointer, with the values' unit when the file states one.
science-value = Valor { $value }
science-value-unit = Valor { $value } { $unit }
# machine-drafted, needs native review
science-value-flags = Valor { $value } (bits { $bits })
# machine-drafted, needs native review
# A class value under the pointer and its name, e.g. "Value 4 · Vegetation".
science-value-class = Valor { $value } · { $class }
# machine-drafted, needs native review
# Sentinel-2 scene classes (L2A SCL, ESA's names) and L1C cloud masks.
s2-class-no-data = Sin datos
s2-class-defective = Saturado o defectuoso
s2-class-dark = Píxeles de zonas oscuras
s2-class-cloud-shadow = Sombras de nubes
s2-class-vegetation = Vegetación
s2-class-not-vegetated = Sin vegetación
s2-class-water = Agua
s2-class-unclassified = Sin clasificar
s2-class-cloud-medium = Nubes de probabilidad media
s2-class-cloud-high = Nubes de probabilidad alta
s2-class-cirrus = Cirros finos
s2-class-snow = Nieve o hielo
s2-class-clear = Despejado
s2-class-opaque-cloud = Nubes opacas
# Several channels under the pointer: "R 12.5  G 3.1  B 0.4".
science-values = { $values }
# Sky position under the pointer, sexagesimal then degrees.
science-radec = AR { $ra }  Dec { $dec }
science-radec-deg = { $ra }°  { $dec }°
# Galactic or ecliptic longitude and latitude.
science-lonlat = l { $lon }°  b { $lat }°
science-wcs-unsupported = WCS no compatible: { $reason }
# machine-drafted, needs native review
# A B1950 (FK4) image: what is converted between its frame and another. Its own position
# stays B1950 in the readout. ICRS, FK4 and SAMP are names and stay as they are.
science-fk4-markers = Marcadores del catálogo: ICRS → FK4 convertido
science-fk4-markers-galactic = Marcadores del catálogo: galácticas → FK4 convertido
science-fk4-samp = SAMP: FK4 → ICRS convertido
# The look: stretch and limits.
science-look = { $stretch } · { $limits }
# Shown under the look while plane levelling is on.
science-levelled = Plano nivelado
# Shown under the look while the hillshade is on.
science-hillshaded = Sombreado: sol a { $azimuth }°, { $altitude }° de altura
# Shown in the readout while the power spectrum is shown.
science-fft = Espectro de potencia (log₁₀)
# The spectrum's value under the pointer: log₁₀ of the power.
science-fft-power = log₁₀ potencia { $value }
# Spatial frequency under the pointer along x and y, and its unit ("1/µm", "1/px").
science-fft-frequency = Frecuencia u { $u }  v { $v } { $unit }
# The period of that frequency: "Period 2.5 nm".
science-fft-period = Periodo { $period }
# Crystal orientation under the pointer (EBSD): Bunge Euler angles in degrees.
science-euler = φ₁ { $phi1 }  Φ { $phi }  φ₂ { $phi2 }
# The crystal phase under the pointer and its symmetry ("Iron fcc (m-3m)").
science-phase = Fase { $phase } ({ $symmetry })
# Title of the spectrum plot of the pixel under the pointer (an EDS map).
science-spectrum-title = Espectro en { $x }, { $y }
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
        [one] { $count } bin de underflow/overflow oculto (suma { $sum })
        [many] { $count } bins de underflow/overflow ocultos (suma { $sum })
       *[other] { $count } bins de underflow/overflow ocultos (suma { $sum })
    }
# Bins of different widths are drawn on an even grid of pixels.
science-root-resampled = Bins de anchura variable remuestreados a una cuadrícula uniforme
# machine-drafted, needs native review
# A flow cytometry plot (FCS) under the pointer: each axis's parameter and its value.
science-cytometry-at = { $x } { $xv }  { $y } { $yv }
# A cytometry histogram under the pointer: its parameter and the value.
science-cytometry-at-x = { $x } { $xv }
# Events in the plot's bin (or the histogram's bar) under the pointer.
science-cytometry-events = { $count ->
    [one] { $count } evento
    [many] { $count } de eventos
   *[other] { $count } eventos
}
# The plot's axis scales ($x, $y: Linear, Log, Logicle or Arcsinh).
science-cytometry-scales = x { $x }, y { $y }
science-cytometry-scale-x = x { $x }
# Spillover compensation is applied, from the file's keyword ($keyword, e.g. $SPILLOVER).
science-cytometry-compensated = Compensado ({ $keyword })
science-cytometry-uncompensated = Sin compensar
# X-ray diffraction frames: what lies under the pointer. q is the momentum transfer
# (per ångström), d the lattice spacing (ångström), 2θ the scattering angle. The symbols
# stay as they are: "q 0.512 Å⁻¹  d 12.27 Å  2θ 4.56°".
science-scatter = q { $q } Å⁻¹  d { $d } Å  2θ { $tth }°
# Seismic readout: the trace under the pointer in a 3-D volume ($inline, $crossline the
# header numbers) or a 2-D line; $trace counts from 1 in file order, $cdp is the CDP number.
science-seismic-cell = Inline { $inline }  Crossline { $crossline }
science-seismic-trace = Traza { $trace }  CDP { $cdp }
# A cell of a 3-D volume with no trace in the file.
science-seismic-gap = Sin traza
# Two-way time (ms) or depth of the row under the pointer.
science-seismic-time = Tiempo { $time } ms
science-seismic-depth = Profundidad { $depth } { $unit }
# The trace's CDP (or source) X and Y with the coordinate scalar applied.
science-seismic-xy = X { $x }  Y { $y }
# machine-drafted, needs native review
# Unstructured mesh readout: the face (cell) under the pointer, counting from 0 as in the
# file's connectivity, and the node nearest to the pointer when the values sit on nodes.
science-mesh-face = Cara { $face }
science-mesh-face-node = Cara { $face }  Nodo { $node }
# The pointer lies between the mesh's faces (land in an ocean mesh, outside its boundary).
science-mesh-outside = Fuera de la malla
# Position on an axis past the image: "Plane 3 of 40", or with its world value.
science-plane = { $axis } { $index } de { $count }
science-plane-world = { $axis } { $index } de { $count } ({ $world })
science-axis-unnamed = Plano
science-axis-extension = Imagen
science-axis-channel = Canal
# A column of a HEALPix sky map (probability, distance…) shown as the image.
science-axis-field = Campo
# X-ray event lists binned into an image: the bin size, in the event list's sky pixels
# ("Bin 2 of 4 (8 px)"), and the energy band of the events counted.
science-axis-bin = Bin
science-bin-size = { $size } px
science-axis-band = Energía
science-band-all = todas las energías
# machine-drafted, needs native review
# Complex-valued arrays (NumPy, MATLAB, Zarr, TDMS): which part of each value is shown.
science-axis-part = Parte
science-part-magnitude = módulo
science-part-real = parte real
science-part-imaginary = parte imaginaria
science-part-phase = fase (radianes)
# machine-drafted, needs native review
# Complex radar (SAR) images: intensity in decibels.
science-part-db = intensidad (dB)
# machine-drafted, needs native review
# The frames of a thermal sequence (FLIR SEQ/CSQ), in time.
science-axis-frame = Fotograma
science-readout = Valor y coordenadas
# Accessible names of the plane sliders.
science-sliders = Planos
# The scale bar's length.
science-scale-arcsec = { $value }″
science-scale-arcmin = { $value }′
science-scale-deg = { $value }°
# A physical length, e.g. "20 nm" (electron microscopy images).
science-scale-length = { $value } { $unit }
# machine-drafted, needs native review
# A length along a named axis of a plot, e.g. "20 GeV (p_{T})".
science-scale-axis = { $length } ({ $axis })
science-scale-a11y = Barra de escala: { $length }
# Mass spectrometry images (imzML). The image is the total ion current (each spot's summed
# intensities) until an m/z is typed; then that ion's image, its intensities summed over
# m/z ± the tolerance. $window is e.g. "760.5851 ± 5 ppm".
science-ion-tic = Corriente iónica total
science-ion-window = Imagen iónica m/z { $window }
# The m/z prompt: $text is what is typed so far (opens on the base peak under the pointer).
science-ion-prompt = Imagen iónica a m/z: { $text }▏
# $tolerance is e.g. "10 ppm" or "0.1 Da"; a tolerance can also be typed after the m/z
# ("760.585 5 ppm"). Tab steps the tolerance; Enter on an empty prompt shows the total ion
# current again.
science-ion-prompt-hint = ± { $tolerance } · Tab tolerancia · Intro mostrar · Esc cancelar
science-ion-bad = No es una m/z: { $text }
# The spot under the pointer as the file numbers it (from 1).
science-ms-spot = Punto x { $x }  y { $y }
science-ms-spot-z = Punto x { $x }  y { $y }  z { $z }
science-ms-no-spectrum = No hay espectro en este punto
# machine-drafted, needs native review
palette-science-temperature-unit = Unidad de temperatura (°C, °F, K)
palette-science-isotherm = Isoterma por encima o por debajo de la temperatura bajo el puntero
palette-science-thermal-parameters = Emisividad y temperatura reflejada…
palette-science-export-temperatures = Exportar temperaturas (TIFF de coma flotante o CSV)…
palette-thermal-visible = Foto visible de una imagen térmica
disabled-not-thermal = Solo para imágenes térmicas
disabled-not-radiometric = El archivo guarda temperaturas: sus parámetros no se pueden cambiar
disabled-no-visible-photo = La imagen térmica no tiene foto visible
science-temperature-unit-toast = Temperaturas en { $unit }
science-isotherm-above = Isoterma por encima de { $level }
science-isotherm-below = Isoterma por debajo de { $level }
science-isotherm-off = Isoterma desactivada
science-thermal-emissivity-prompt = Emisividad: { $text }
science-thermal-reflected-prompt = Temperatura reflejada ({ $unit }): { $text }
science-thermal-prompt-hint = Tab campo siguiente · Intro recalcular · Esc cancelar
science-thermal-bad = Valor no válido: { $text }
science-thermal-applied = Recalculado para emisividad { $emissivity }, reflejada { $reflected }
science-thermal-conditions = Emisividad { $emissivity } · reflejada { $reflected }
science-thermal-export-tiff = TIFF de coma flotante de 32 bits
science-thermal-export-csv = CSV
science-thermal-export-done = Temperaturas guardadas en { $name }
science-thermal-export-failed = No se pudieron guardar las temperaturas: { $reason }
# machine-drafted, needs native review
science-values-failed = No se pudieron mostrar los valores: { $reason }
# machine-drafted, needs native review
# Thermal sequences (FLIR SEQ/CSQ): playback and each measurement over time.
palette-science-thermal-play = Reproducir o pausar la secuencia térmica
palette-science-thermal-series-csv = Guardar las temperaturas en el tiempo (CSV)…
disabled-not-thermal-sequence = Solo para secuencias térmicas
thermal-seq-title = Temperaturas en el tiempo
# $time is seconds from the first frame, e.g. "0.12 s".
thermal-seq-frame = Fotograma { $frame } de { $frames } · { $time }
thermal-seq-play = Reproducir
thermal-seq-pause = Pausa
thermal-seq-playing = Reproduciendo
thermal-seq-paused = En pausa
thermal-seq-save-csv = Guardar CSV…
thermal-seq-hint = Añade un punto, una línea o un área (M) para ver su curva en el tiempo.
thermal-seq-reading = Leyendo el fotograma { $done } de { $total }…
thermal-seq-failed = No se pudieron leer los fotogramas: { $reason }
thermal-seq-no-series = Aún no hay nada que guardar: añade un punto, una línea o un área.
# For screen readers: each measurement's value at the frame shown, e.g.
# "1 Spot: 36.4 °C, 2 Rectangle: 30.1 °C".
thermal-seq-chart = Gráfico: { $values }
# machine-drafted, needs native review
# The report (PDF) of the image shown: a thermal report for a thermal image, an inspection report for any other.
palette-thermal-report = Informe (PDF)…
thermal-report-heading = Informe térmico
thermal-report-default-title = Informe de inspección termográfica
thermal-report-title-label = Título
thermal-report-notes-label = Notas
thermal-report-photo = Incluir la foto visible
thermal-report-folder = Añadir las demás imágenes térmicas de esta carpeta
thermal-report-save = Guardar PDF…
thermal-report-busy = Escribiendo el informe…
thermal-report-nothing = Las temperaturas aún no se muestran.
thermal-report-saved = Informe guardado en { $name } ({ $count ->
    [one] { $count } imagen
    [many] { $count } imágenes
   *[other] { $count } imágenes
})
thermal-report-failed = No se pudo guardar el informe: { $reason }
# In the PDF.
thermal-report-notes = Notas
thermal-report-measurements = Mediciones
thermal-report-no-measurements = Sin mediciones.
thermal-report-thermal-caption = Imagen térmica
thermal-report-visible-caption = Foto visible
thermal-report-col-measurement = Medición
thermal-report-col-min = Mín.
thermal-report-col-max = Máx.
thermal-report-col-mean = Media
thermal-report-col-emissivity = Emisividad
thermal-report-whole-image = Imagen completa
thermal-report-camera = Cámara
thermal-report-lens = Objetivo
thermal-report-date = Tomada
thermal-report-place = Lugar
thermal-report-frame = Fotograma
thermal-report-emissivity = Emisividad
thermal-report-reflected = Temperatura reflejada
thermal-report-distance = Distancia
thermal-report-humidity = Humedad
thermal-report-range = Rango
thermal-report-format = Formato
thermal-report-page = Página { $page } de { $pages }
