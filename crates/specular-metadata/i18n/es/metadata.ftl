# machine-drafted, needs native review
# Panel de metadatos de Specular — español (los números y las fechas llegan ya formateados por ICU4X).

## Títulos de los grupos del resumen

group-camera = Cámara
group-exposure = Exposición
group-lens = Objetivo
group-date = Fecha y hora
group-location = Ubicación
# machine-drafted, needs native review
# A DJI drone photo: the gimbal's and the aircraft's attitude, the height above take-off.
group-drone = Dron
label-gimbal = Estabilizador
label-flight = Aeronave
label-relative-altitude = Sobre el punto de despegue
drone-attitude = guiñada { $yaw }, cabeceo { $pitch }, alabeo { $roll }
# A DJI thermal camera's radiometric JPEG: its photo opens, its temperatures can't.
label-thermal-data = Temperaturas
drone-thermal-refused = No se muestra: la licencia del SDK de DJI no lo permite
drone-thermal-refused-detail = Leer las temperaturas de DJI requiere el Thermal SDK de DJI, cuya licencia solo permite usarlo a los programas que lo incluyen. Se muestran la imagen y sus metadatos.
group-image = Imagen
group-file = Archivo
group-rights = Derechos y descripción
group-credentials = Credenciales de contenido
# machine-drafted, needs native review
group-medical = Imagen médica
# machine-drafted, needs native review
group-inspection = Componente e inspección
group-nde-image = Imagen de inspección (DICONDE)
# machine-drafted, needs native review
# Industrial CT volumes (Zeiss Xradia, Amira, Bruker SkyScan, Nikon X-Tek, VGI, raw volumes).
group-industrial-image = Imagen de inspección
group-science = Imagen científica
group-em = Microscopía electrónica
group-spm = Microscopía de sonda de barrido
group-slide = Portaobjetos
group-microscopy = Microscopía
# X-ray diffraction detector frames (CBF, EDF, SMV, Bruker, MarCCD, R-AXIS).
group-diffraction = Difracción
group-surface = Topografía de superficie
group-ebsd = Difracción de electrones retrodispersados
group-seismic = Datos sísmicos
# machine-drafted, needs native review
group-asdf = ASDF
# machine-drafted, needs native review
# CERN ROOT files (particle physics histograms and graphs).
group-root = ROOT
# machine-drafted, needs native review
# CASA images (radio interferometry: the .image, .residual, .model directories tclean writes).
group-casa = Imagen CASA
# machine-drafted, needs native review
# GIFTI files: brain surfaces and the values on their vertices (Human Connectome Project
# and FreeSurfer exports).
group-gifti = GIFTI
# CIFTI-2 files: maps over cortical surfaces and subcortical voxels ("grayordinates").
group-cifti = CIFTI
# White-matter fibre tracts (TrackVis, MRtrix).
group-tractography = Tractografía
# machine-drafted, needs native review
group-annotations = Anotaciones
# Imaging mass spectrometry (imzML): a mass spectrum per pixel.
group-mass-spec = Imagen por espectrometría de masas
# machine-drafted, needs native review
group-thermal = Imagen térmica
# machine-drafted, needs native review
# Flow cytometry list-mode data (FCS): events of a cell sorter or analyser.
group-cytometry = Citometría de flujo
# machine-drafted, needs native review
group-spectroscopy = Espectroscopía
# machine-drafted, needs native review
# Unstructured meshes of ocean, coastal and climate models (UGRID, ICON, MPAS in NetCDF).
group-ugrid = Malla no estructurada
group-drawing = Dibujo
# machine-drafted, needs native review
group-volume = Volumen

## Etiquetas de los elementos del resumen

label-camera = Cámara
label-software = Software
label-application = Aplicación
label-shutter = Velocidad de obturación
label-aperture = Apertura
label-iso = ISO
label-focal-length = Distancia focal
label-exposure-bias = Compensación de la exposición
label-program = Modo
label-metering = Modo de medición
label-white-balance = Balance de blancos
label-flash = Flash
label-lens = Objetivo
label-lens-make = Fabricante del objetivo
label-lens-range = Rango del objetivo
# machine-drafted, needs native review
label-focus-mode = Modo de enfoque
label-stabilisation = Estabilización de imagen
label-taken = Tomada
label-digitised = Digitalizada
label-edited = Última edición
label-place = Lugar
label-coordinates = Coordenadas
label-altitude = Altitud
label-direction = Dirección
label-accuracy = Precisión
label-gps-time = Hora GPS
label-dimensions = Dimensiones
label-encoding = Codificación
# machine-drafted, needs native review
label-motion = Movimiento
motion-kind-motion-photo = Foto en movimiento
motion-kind-micro-video = Foto en movimiento (MicroVideo)
motion-kind-samsung = Foto en movimiento de Samsung
motion-kind-live-photo = Live Photo
motion-duration = { $seconds } s
label-colour-profile = Perfil de color
label-orientation = Orientación
label-file-name = Nombre
label-folder = Carpeta
label-file-size = Tamaño
label-format = Formato
label-modified = Modificado
label-title = Título
label-description = Descripción
label-creator = Autor
label-copyright = Copyright
label-keywords = Palabras clave
label-rating = Valoración
label-colour-label = Etiqueta
label-credentials = Credenciales de contenido

## Grupos de «Todos los campos» (los nombres de etiqueta se mantienen como en los estándares)

fields-ifd0 = IFD0 (imagen principal)
fields-exif = Exif
fields-gps = GPS
fields-interop = Interoperabilidad
fields-ifd1 = IFD1 (miniatura)
fields-icc = Cabecera del perfil ICC
fields-xmp = XMP: { $namespace } ({ $prefix })
xmp-namespace = { $prefix ->
    [dc] Dublin Core
    [xmp] Básico
    [xmpRights] Derechos
    [xmpMM] Gestión de medios
    [xmpDM] Medios dinámicos
    [tiff] TIFF
    [exif] Exif
    [exifEX] Exif 2.3+
    [aux] Accesorios de cámara
    [photoshop] Photoshop
    [crs] Ajustes de Camera Raw
    [lr] Lightroom
    [Iptc4xmpCore] IPTC Core
    [Iptc4xmpExt] IPTC Extension
    [plus] Licencias PLUS
    [GPano] Fotoesfera
    [GCamera] Cámara de Google
    [hdrgm] Mapa de ganancia HDR
    [mwg-rs] Regiones
    [pdf] PDF
    [stEvt] Eventos del recurso
    [stRef] Referencias del recurso
   *[other] Otros
}
unknown-tag = Etiqueta desconocida { $number } ({ $context })
blob-bytes = { $num ->
    [one] ({ $num } byte)
    [many] ({ $num } de bytes)
   *[other] ({ $num } bytes)
}
maker-note-bytes = ({ $num ->
        [one] { $num } byte
       *[other] { $num } bytes
    }, nota del fabricante, sin descodificar)
more-values = { $shown }, … ({ $total ->
        [one] { $total } valor
       *[other] { $total } valores
    })

## Elementos de unión

list-separator = {", "}
one-line-separator = {" · "}

## Exposición

exposure-time-fraction = 1/{ $denominator } s
exposure-time-seconds = { $seconds } s
aperture-value = f/{ $f }
iso-value = ISO { $iso }
focal-length-value = { $mm } mm
focal-length-with-equiv = { $mm } mm (equiv. { $equiv } mm)
focal-length-equiv-only = equiv. { $equiv } mm
exposure-bias-value = { $sign ->
    [plus] +{ $ev } EV
    [minus] −{ $ev } EV
   *[zero] 0 EV
}
exposure-program = { $value ->
    [1] Manual
    [2] Programa (automático)
    [3] Prioridad a la apertura
    [4] Prioridad a la velocidad
    [5] Creativo (profundidad de campo)
    [6] Acción (obturación rápida)
    [7] Retrato
    [8] Paisaje
   *[other] Desconocido
}
metering-mode = { $value ->
    [1] Promedio
    [2] Promedio ponderado al centro
    [3] Puntual
    [4] Multipunto
    [5] Matricial (evaluativa)
    [6] Parcial
   *[other] Otro
}
# machine-drafted, needs native review
# Canon maker note CameraSettings FocusMode; 256–258 are the live-view variants of 0–2.
canon-focus-mode = { $value ->
    [0] AF foto a foto
    [1] AF AI Servo
    [2] AF AI Foco
    [3] Enfoque manual
    [4] Único
    [5] Continuo
    [6] Enfoque manual
    [16] Enfoque fijo (Pan Focus)
    [256] AF foto a foto (Visión en directo)
    [257] AF AI Servo (Visión en directo)
    [258] AF AI Foco (Visión en directo)
    [512] Enfoque Movie Snap
    [519] AF servo de vídeo
   *[other] Otro
}
# Canon maker note CameraSettings ImageStabilization; 256–260 repeat 0–4.
canon-stabilisation = { $value ->
    [0] Desactivada
    [1] Activada
    [2] Solo al disparar
    [3] Barrido
    [4] Dinámica
    [256] Desactivada
    [257] Activada
    [258] Solo al disparar
    [259] Barrido
    [260] Dinámica
   *[other] Otro
}
# machine-drafted, needs native review
panasonic-focus-mode = { $value ->
    [1] Automático
    [2] Enfoque manual
    [4] Automático (botón de enfoque)
    [5] Automático (continuo)
    [6] AF-S
    [7] AF-C
    [8] AF-F
   *[other] Otro
}
# machine-drafted, needs native review
panasonic-stabilisation = { $value ->
    [2] Activado (óptico)
    [3] Desactivado
    [4] Activado (modo 2)
    [5] Activado (óptico, barrido)
    [6] Activado (solo cuerpo)
    [7] Activado (solo cuerpo, barrido)
    [9] Dual IS
    [10] Dual IS (barrido)
    [11] Dual IS 2
    [12] Dual IS 2 (barrido)
   *[other] Otro
}
label-comment = Comentario
fields-tags = Etiquetas { $kind }
group-picture = Ajustes de imagen
fields-fujifilm = Nota del fabricante Fujifilm
label-film-simulation = Simulación de película
label-dynamic-range = Rango dinámico
label-highlight-tone = Altas luces
label-shadow-tone = Sombras
label-colour-setting = Color
label-sharpness = Nitidez
label-noise-reduction = Reducción de ruido
label-clarity = Claridad
label-grain-effect = Efecto de grano
label-colour-chrome = Color cromado
label-colour-chrome-blue = Color cromado azul
label-shutter-type = Tipo de obturador
label-drive-mode = Modo de disparo
fuji-step = { $sign ->
    [plus] +{ $n }
    [minus] −{ $n }
   *[zero] 0
}
fuji-film-mode = { $value ->
    [0] Provia (estándar)
    [256] Retrato de estudio
    [272] Retrato de estudio (más saturación)
    [288] Astia (suave)
    [304] Retrato de estudio (más nítido)
    [512] Velvia (vivo)
    [768] Retrato de estudio Ex
    [1024] Velvia
    [1280] Pro Neg. Std
    [1281] Pro Neg. Hi
    [1536] Classic Chrome
    [1792] Eterna
    [2048] Classic Neg.
    [2304] Eterna Bleach Bypass
    [2560] Nostalgic Neg.
    [2816] Reala Ace
   *[other] Otro
}
fuji-monochrome = { $value ->
    [768] Monocromo
    [769] Monocromo, filtro rojo
    [770] Monocromo, filtro amarillo
    [771] Monocromo, filtro verde
    [784] Sepia
    [1280] Acros
    [1281] Acros, filtro rojo
    [1282] Acros, filtro amarillo
    [1283] Acros, filtro verde
   *[other] Monocromo
}
fuji-white-balance = { $value ->
    [0] Automático
    [1] Automático (prioridad al blanco)
    [2] Automático (prioridad al ambiente)
    [256] Luz de día
    [512] Nublado
    [768] Fluorescente luz de día
    [769] Fluorescente blanco día
    [770] Fluorescente blanco
    [771] Fluorescente blanco cálido
    [772] Fluorescente blanco cálido de salón
    [1024] Incandescente
    [1280] Flash
    [1536] Bajo el agua
    [3840] Personalizado 1
    [3841] Personalizado 2
    [3842] Personalizado 3
    [3843] Personalizado 4
    [3844] Personalizado 5
    [4080] Temperatura de color
   *[other] Otro
}
fuji-white-balance-kelvin = { $kelvin } K
fuji-dynamic-range = DR{ $percent }
fuji-dynamic-range-auto = Automático (DR{ $percent })
fuji-dynamic-range-setting = { $value ->
    [0] Automático
    [256] Estándar (DR100)
    [512] Amplio 1 (DR230)
    [513] Amplio 2 (DR400)
   *[other] Otro
}
fuji-effect = { $value ->
    [0] Desactivado
    [32] Débil
    [64] Fuerte
   *[other] Otro
}
fuji-grain-size = { $value ->
    [16] pequeño
    [32] grande
   *[other] otro
}
fuji-grain = { $strength }, { $size }
fuji-shutter-type = { $value ->
    [0] Mecánico
    [1] Electrónico
    [2] Electrónico (exposiciones largas)
    [3] Primera cortinilla electrónica
   *[other] Otro
}
fuji-drive-mode = { $value ->
    [0] Foto a foto
    [1] Ráfaga, velocidad baja
    [2] Ráfaga, velocidad alta
   *[other] Otro
}
fuji-focus-mode = { $value ->
    [0] Manual
    [1] AF-S
    [2] AF-C
    [100] Automático
    [101] Manual
   *[other] Otro
}
fuji-stabilisation = { $kind } ({ $mode })
fuji-stabilisation-kind = { $value ->
    [0] Ninguna
    [1] Óptica
    [2] Desplazamiento del sensor
    [3] Objetivo
    [258] Desplazamiento del sensor y digital
    [512] Digital
   *[other] Otro
}
fuji-stabilisation-mode = { $value ->
    [0] Desactivado
    [1] activada, continua
    [2] activada, al disparar
   *[other] Otro
}
light-source = { $value ->
    [1] Luz de día
    [2] Fluorescente
    [3] Tungsteno
    [4] Flash
    [9] Buen tiempo
    [10] Nublado
    [11] Sombra
    [12] Fluorescente luz de día
    [13] Fluorescente blanco diurno
    [14] Fluorescente blanco frío
    [15] Fluorescente blanco
    [16] Fluorescente blanco cálido
    [17] Luz estándar A
    [18] Luz estándar B
    [19] Luz estándar C
    [20] D55
    [21] D65
    [22] D75
    [23] D50
    [24] Tungsteno de estudio
   *[other] Otra
}
white-balance-auto = Automático
white-balance-manual = Manual
white-balance-manual-with-light = Manual ({ $light })
flash-none = Sin función de flash
flash-fired = Flash disparado
flash-not-fired = Flash no disparado
flash-forced-on = forzado activado
flash-forced-off = forzado desactivado
flash-auto = modo automático
flash-return-not-detected = luz de retorno no detectada
flash-return-detected = luz de retorno detectada
flash-red-eye = reducción de ojos rojos

## Objetivo

lens-focal-single = { $mm } mm
lens-focal-range = { $min }–{ $max } mm
lens-aperture-single = f/{ $f }
lens-aperture-range = f/{ $min }–{ $max }
lens-range = { $focal } { $aperture }

## Fecha y hora

date-with-weekday = { $weekday }, { $date }
date-time = { $date }, { $time }
date-time-zone = { $datetime } ({ $zone })
zone-utc = UTC
zone-offset = UTC{ $offset }
gps-date-time = { $date }, { $time } UTC
detail-zone-from-gps = Zona horaria deducida del reloj GPS

## Ubicación

coordinates = { $lat }° { $latRef ->
    [south] S
   *[north] N
}, { $lon }° { $lonRef ->
    [west] O
   *[east] E
}
altitude = { $ref ->
    [above] { $m } m sobre el nivel del mar
    [below] { $m } m bajo el nivel del mar
   *[sea] Al nivel del mar
}
compass-point = { $point ->
    [n] norte
    [nne] nornoreste
    [ne] noreste
    [ene] estenoreste
    [e] este
    [ese] estesureste
    [se] sureste
    [sse] sursureste
    [s] sur
    [ssw] sursuroeste
    [sw] suroeste
    [wsw] oestesuroeste
    [w] oeste
    [wnw] oestenoroeste
    [nw] noroeste
   *[nnw] nornoroeste
}
facing = Orientada al { $point } ({ $degrees }°)
detail-true-north = Respecto al norte geográfico
detail-magnetic-north = Respecto al norte magnético
accuracy = ±{ $m } m
distance-km = { $km } km
place = { $city }, { $country }
place-city-only = { $city }
place-near = cerca de { $place }
place-detail = { $areas } · a { $distance } de { $city }
place-detail-distance = A { $distance } de { $city }

## Imagen

dimensions = { $width } × { $height }
dimensions-megapixels = { $width } × { $height } ({ $mp } MP)
encoding-bit-depth = { $bits } bits
encoding-progressive = { $format }, progresivo
encoding-baseline = { $format }, secuencial (baseline)
encoding-hierarchical = { $format }, jerárquico
# machine-drafted, needs native review
encoding-alpha = { $format } { $alpha ->
    [premultiplied] con alfa premultiplicado
    [transparency] con transparencia
   *[alpha] con alfa
}
# [Lab], [Palette] and the variants after them: machine-drafted, needs native review
colour-model = { $model ->
    [YCbCr] YCbCr
    [RGB] RGB
    [Greyscale] Escala de grises
    [CMYK] CMYK
    [YCCK] YCCK
    [Lab] L*a*b*
    [Palette] Color indexado
    [Multichannel] Multicanal
    [Duotone] Duotono
    [YCgCo] YCgCo
    [XYB] XYB
    [YC] Luminancia/crominancia (YC)
    [Bayer] RAW Bayer
    [X-Trans] RAW X-Trans
    [CFA] RAW con matriz de filtros de color
    [LinearRaw] RAW lineal
    [RGBE] RGBE (exponente compartido)
    [XYZE] XYZE (exponente compartido)
    [R] Un canal (R)
    [RG] Dos canales (RG)
   *[other] { $model }
}
adobe-rgb-from-exif = Adobe RGB (según la etiqueta EXIF; sin perfil incrustado)
orientation = { $value ->
    [1] Vertical (no requiere giro)
    [2] Reflejada horizontalmente (aplicado)
    [3] Girada 180° (aplicado)
    [4] Reflejada verticalmente (aplicado)
    [5] Reflejada horizontalmente y girada 90° en sentido antihorario (aplicado)
    [6] Girada 90° en sentido horario (aplicado)
    [7] Reflejada horizontalmente y girada 90° en sentido horario (aplicado)
    [8] Girada 90° en sentido antihorario (aplicado)
   *[other] Desconocida ({ $value })
}
orientation-short = { $value ->
    [1] Vertical
    [2] Reflejada horizontalmente
    [3] Girada 180°
    [4] Reflejada verticalmente
    [5] Reflejada horizontalmente y girada 90° en sentido antihorario
    [6] Girada 90° en sentido horario
    [7] Reflejada horizontalmente y girada 90° en sentido horario
    [8] Girada 90° en sentido antihorario
   *[other] valor { $value }
}

## Archivo

file-size-bytes = { $num ->
    [one] { $num } byte
    [many] { $num } de bytes
   *[other] { $num } bytes
}
file-size-unit = { $num } { $unit ->
    [kb] KB
    [mb] MB
    [gb] GB
    [tb] TB
   *[pb] PB
}

## Derechos

rating-rejected = Rechazada
rating-stars = { $stars } ({ $n } de 5)

## Credenciales de contenido

c2pa-present = Presentes (sin verificar)
c2pa-detail = Este archivo contiene un manifiesto C2PA; su firma no se ha comprobado.

## Valores de la cabecera ICC

icc-class = { $class ->
    [scnr] Dispositivo de entrada
    [mntr] Dispositivo de visualización
    [prtr] Dispositivo de salida
    [link] Vínculo de dispositivos
    [spac] Espacio de color
    [abst] Abstracto
    [nmcl] Color con nombre
   *[other] Desconocida
}
icc-intent = { $intent ->
    [0] Perceptual
    [1] Colorimétrico relativo
    [2] Saturación
    [3] Colorimétrico absoluto
   *[other] Desconocido
}
icc-platform = { $platform ->
    [APPL] Apple
    [MSFT] Microsoft
    [SGI] Silicon Graphics
    [SUNW] Sun Microsystems
    [none] No especificada
   *[other] Desconocida
}
icc-coded = { $name } ({ $code })

# machine-drafted, needs native review
label-modality = Modalidad
label-body-part = Región anatómica
label-study = Estudio
label-series = Serie
label-slice-thickness = Grosor de corte
label-pixel-spacing = Espaciado de píxeles
label-imager-pixel-spacing = Espaciado de píxeles en el detector
label-frames = Fotogramas
label-transfer-syntax = Sintaxis de transferencia
label-document = Documento
label-document-type = Tipo de documento
medical-spacing = { $row } × { $column } mm
medical-mm = { $mm } mm
medical-lossy = { $name } (compresión con pérdida)
detail-pixel-spacing = Distancia entre los centros de los píxeles en el paciente (filas × columnas).
# machine-drafted, needs native review
# Pixel spacing of an industrial inspection image (DICONDE, CT of an object).
detail-pixel-spacing-object = Distancia entre los centros de los píxeles en el objeto (filas × columnas).
detail-imager-pixel-spacing = Medido en el detector, sin corregir la ampliación: las distancias en el paciente suelen ser menores.

## Inspección industrial (DICONDE)

# machine-drafted, needs native review
label-nde-component-shape = Forma del componente
label-nde-component-manufacturer = Fabricante del componente
label-nde-material = Material
label-nde-thickness = Espesor del material
label-nde-pipe-diameter = Diámetro de la tubería
label-nde-source = Fuente
label-nde-environment = Entorno
label-nde-iqi = Indicador de calidad de imagen
label-nde-indications = Indicaciones
label-nde-standard = Norma

## Volúmenes de investigación (NIfTI, Analyze, NRRD, MetaImage)

# machine-drafted, needs native review
label-voxel-size = Tamaño de vóxel
label-data-type = Tipo de datos
label-time-points = Puntos temporales
label-orientation-source = Orientación desde
label-intent = Valores
label-compression = Compresión
volume-voxel-size = { $x } × { $y } × { $z } mm
volume-orient-sform = NIfTI sform
volume-orient-qform = NIfTI qform
volume-orient-analyze = Cabecera Analyze (poco fiable)
volume-orient-space = Direcciones espaciales NRRD
volume-orient-transform = Transformación MetaImage
volume-orient-spacings = Solo espaciado de vóxel
volume-orient-none = No indicada
volume-orient-vox2ras = FreeSurfer vox2ras
volume-orient-mgh-default = Predeterminado de FreeSurfer (poco fiable)
volume-orient-minc = Cosenos directores de MINC
volume-orient-object = Coordenadas del objeto (sin orientación del paciente)
volume-content-projections = Proyecciones (adquisición sin procesar)
volume-content-reconstruction = Volumen reconstruido
volume-coords-rectilinear = Rectilínea (separación irregular, mostrada uniforme)
# Scan parameters and tags a FreeSurfer MGH file keeps after its voxels.
label-volume-tr = Tiempo de repetición
label-volume-te = Tiempo de eco
label-volume-ti = Tiempo de inversión
label-volume-flip = Ángulo de inclinación
label-volume-fov = Campo de visión
label-volume-field = Intensidad de campo
label-volume-pedir = Codificación de fase
label-volume-cmdline = Línea de órdenes
label-volume-xform = Transformación de Talairach
label-volume-content = Contenido
label-volume-projections = Proyecciones
label-volume-angles = Rango de rotación
label-volume-angle-step = Paso de rotación
label-volume-kv = Tensión del tubo
label-volume-current = Corriente del tubo
label-volume-power = Potencia de la fuente
label-volume-exposure = Exposición
label-volume-sod = Fuente–objeto
label-volume-odd = Objeto–detector
label-volume-sdd = Fuente–detector
label-volume-magnification = Aumento óptico
label-volume-binning = Binning
label-volume-objective = Objetivo
label-volume-filter = Filtro de la fuente
label-volume-camera = Detector
label-volume-instrument = Instrumento
label-volume-software = Software
label-volume-date = Adquirido
label-volume-ring = Corrección de artefactos de anillo
label-volume-smoothing = Suavizado
label-volume-coords = Rejilla
# machine-drafted, needs native review
# An eye scan (OCT): which eye.
label-volume-eye = Ojo
volume-eye-right = Derecho (OD)
volume-eye-left = Izquierdo (OS)
detail-volume-unreliable = El archivo no indica hacia dónde apuntan sus ejes en el paciente: izquierda y derecha pueden estar invertidas, así que no se muestran etiquetas de orientación.
# machine-drafted, needs native review
# An industrial CT volume: its axes are the scanner's, there is no patient.
detail-volume-object = Los ejes son los del escáner, no los de un paciente, así que no se muestran etiquetas de orientación.

## Imágenes científicas (FITS, XISF)

label-object = Objeto
label-telescope = Telescopio
label-instrument = Instrumento
label-filter = Filtro
label-observed = Observado
label-exposure-time = Exposición
label-sample = Muestras
label-unit = Unidad
label-hdu = Extensión
label-centre = Centro
label-centre-deg = Centro (grados)
label-frame = Sistema de coordenadas
# machine-drafted, needs native review
science-frame-galactic = Galáctico
science-frame-ecliptic = Eclíptico
label-projection = Proyección
label-pixel-scale = Escala de píxel
science-seconds = { $seconds } s
science-arcsec-px = { $arcsec }″ por píxel
label-distortion = Distorsión
label-wcs = Coordenadas del cielo
science-wcs-unsupported = No compatible: { $reason }
# machine-drafted, needs native review
science-wcs-approximate = Aproximadas (solo apuntado, sin distorsión)
label-observer = Observador
label-bayer = Patrón Bayer
label-capture-start = Inicio de captura (UTC)
label-capture-end = Fin de captura (UTC)
label-frame-rate = Velocidad de fotogramas
science-fps = { $fps } fotogramas/s
science-frames-cut = { $frames } de { $declared }
# The HDF5 dataset or NetCDF variable shown.
label-dataset = Conjunto de datos
label-pixel-size = Tamaño de píxel
# "0.25 nm per pixel"; $size may be "0.25 × 0.5".
science-pixel-size = { $size } { $unit } por píxel
# Microscopy (DigitalMicrograph, MRC, Gwyddion, Igor).
label-microscope = Microscopio
label-voltage = Tensión de aceleración
label-magnification = Aumento
label-imaging-mode = Modo
label-detector = Detector
label-acquired = Adquirido
label-field-of-view = Campo de visión
science-kilovolts = { $kv } kV
science-magnification = { $value }×
label-vendor = Fabricante
label-scanner = Escáner
label-objective = Objetivo
label-pyramid-levels = Niveles
# machine-drafted, needs native review
label-emissivity = Emisividad
label-object-distance = Distancia al objeto
label-reflected-temperature = Temperatura reflejada
label-atmospheric-temperature = Temperatura atmosférica
label-humidity = Humedad relativa
label-raw-range = Rango en bruto
label-temperature-range = Rango de temperaturas
thermal-temperature = { $value } { $unit }
thermal-metres = { $value } m
thermal-percent = { PERCENT($value) }
thermal-range = { $low } – { $high }
slide-levels = { $count }: { $sizes }
# X-ray diffraction: the experiment geometry from a detector frame's header.
label-wavelength = Longitud de onda
# Ångström, e.g. "0.97625 Å".
xtal-angstrom = { $value } Å
label-photon-energy = Energía del fotón
xtal-kev = { $value } keV
# Sample to detector distance.
label-detector-distance = Distancia al detector
xtal-mm = { $value } mm
# Where the direct beam meets the detector, in pixels from the image's top-left corner.
label-beam-centre = Centro del haz
xtal-beam-centre = x { $x }, y { $y } px
# The detector arm's swing angle (2θ).
label-detector-swing = 2θ del detector
xtal-degrees = { $value }°
# Goniometer angles (Greek letters are the field's usual names).
label-phi = Fi (φ)
label-omega = Omega (ω)
label-chi = Ji (χ)
label-kappa = Kappa (κ)
# The sample's rotation during the exposure: start angle plus width, e.g. "12.5° + 0.1°".
label-oscillation = Oscilación
xtal-oscillation = { $start }° + { $width }°
# $axis is the axis name from the file, e.g. "phi" or "omega".
xtal-oscillation-axis = { $start }° + { $width }° alrededor de { $axis }
# The count at which a pixel saturates.
label-overload = Valor de saturación
# The finest lattice spacing (d) recorded at the detector's edge and in its corners.
label-resolution = Resolución
xtal-resolution = { $edge } Å en el borde, { $corner } Å en las esquinas
label-geometry = Geometría
xtal-curved = Placa de imagen curva: sin q, d ni anillos de resolución
xtal-incomplete = Cabecera incompleta: sin q, d ni anillos de resolución
# All-fields group title: "FITS header".
fields-science-header = Cabecera { $format }
# machine-drafted, needs native review
fields-asdf-tree = Árbol ASDF
label-asdf-model = Modelo de datos
label-asdf-version = Versión del archivo
label-asdf-standard = Estándar ASDF
label-asdf-library = Escrito por
# machine-drafted, needs native review
# All-fields group title for a CERN ROOT file's list of keys (objects).
fields-root-keys = Claves ROOT
# CERN ROOT files: the ROOT version that wrote the file (e.g. "6.28/04"), its compression,
# the number of keys (stored objects), and the object shown: class (e.g. "TH2F"), its path in
# the file, title, number of entries and sum of weights.
label-root-version = Versión de ROOT
label-root-compression = Compresión
label-root-keys = Claves
label-root-class = Clase
label-root-name = Nombre
label-root-title = Título
label-root-entries = Entradas
label-root-sum-weights = Suma de pesos
# A graph's number of points.
label-root-points = Puntos
# $axis is "x", "y" or "z".
label-root-axis = Eje { $axis }
# $quantity is the axis title with its unit, e.g. "Energy [GeV]"; $min and $max the axis
# limits.
root-axis-fixed = { $quantity }: { $bins ->
        [one] { $bins } bin
        [many] { $bins } bins
       *[other] { $bins } bins
    }, de { $min } a { $max }
root-axis-variable = { $quantity }: { $bins ->
        [one] { $bins } bin variable
        [many] { $bins } bins variables
       *[other] { $bins } bins variables
    }, de { $min } a { $max }
label-root-mean-rms = Media y RMS { $axis }
root-mean-rms = { $mean } ± { $rms }
# Trees (event data) are listed only, with their entry counts.
label-root-trees = Árboles
root-tree = { $name } ({ $entries ->
        [one] { $entries } entrada
        [many] { $entries } entradas
       *[other] { $entries } entradas
    })
# machine-drafted, needs native review
# CASA images: what the image is (from its directory name), its restoring beam (major and
# minor axis, position angle), the beams of its planes when they differ, the rest frequency,
# spectral reference frame (e.g. "LSRK") and velocity definition of the frequency labels,
# the default mask (e.g. "mask0") and the casacore storage manager with its byte order.
label-casa-type = Tipo de imagen
casa-type-image = Imagen restaurada
casa-type-residual = Residuo
casa-type-model = Modelo
casa-type-pb = Haz primario
casa-type-mask = Máscara de limpieza
casa-type-psf = Función de dispersión de punto
casa-type-sumwt = Suma de pesos
casa-type-weight = Pesos
label-casa-beam = Haz de restauración
casa-beam = { $major }″ × { $minor }″, AP { $pa }°
label-casa-plane-beams = Haces por plano
casa-plane-beams = { $count ->
        [one] { $count } haz
        [many] { $count } de haces
       *[other] { $count } haces
    }, eje mayor de { $min }″ a { $max }″
label-casa-rest-frequency = Frecuencia en reposo
casa-ghz = { $ghz } GHz
label-casa-spectral-frame = Sistema de referencia espectral
label-casa-velocity = Definición de velocidad
casa-velocity-radio = Radio
casa-velocity-optical = Óptica
casa-velocity-relativistic = Relativista
label-casa-mask = Máscara predeterminada
label-casa-storage = Almacenamiento
casa-storage-big = { $manager }, big-endian
casa-storage-little = { $manager }, little-endian
# machine-drafted, needs native review
# Unstructured meshes: the mesh's name in the file, the convention describing it (UGRID,
# ICON, MPAS), its counts, face shapes, coordinates and where the values sit.
label-ugrid-name = Malla
label-ugrid-convention = Convención
label-ugrid-nodes = Nodos
label-ugrid-faces = Caras
label-ugrid-edges = Aristas
label-ugrid-shapes = Formas de las caras
label-ugrid-coordinates = Coordenadas
label-ugrid-values = Valores en
label-ugrid-split = Divididas para el dibujo
# $n is the formatted count, $count the number (for the plural).
ugrid-triangles = { $count ->
    [one] { $count } triángulo
    [many] { $count } de triángulos
   *[other] { $count } triángulos
}
ugrid-quads = { $count ->
    [one] { $count } cuadrilátero
    [many] { $count } de cuadriláteros
   *[other] { $count } cuadriláteros
}
# Faces of five or more nodes; $max is the most nodes of one face.
ugrid-polygons = { $count ->
    [one] { $count } polígono de { $max } nodos
    [many] { $count } de polígonos de hasta { $max } nodos
   *[other] { $count } polígonos de hasta { $max } nodos
}
ugrid-lonlat = Longitud y latitud
ugrid-projected = x e y proyectadas
ugrid-projected-unit = x e y proyectadas ({ $unit })
ugrid-on-faces = Caras (dibujadas planas)
ugrid-on-nodes = Nodos (interpolados entre las caras)
# A mesh file without data variables: the image shows how many nodes each face has.
ugrid-grid-only = Ninguno: la imagen muestra los nodos por cara
# Faces cut at the antimeridian or closed to a pole so they draw on a flat map.
ugrid-split = { $count ->
    [one] { $count } cara en el antimeridiano o en un polo
    [many] { $count } de caras en el antimeridiano o en un polo
   *[other] { $count } caras en el antimeridiano o en un polo
}
# machine-drafted, needs native review
# Slide annotations (NDP.view, Aperio ImageScope, ASAP sidecar files).
label-annotation-file = Archivo
# "slide.xml (Aperio ImageScope)"
annotation-source = { $file } ({ $format })
label-annotation-shapes = Formas
label-annotation-groups = Grupos
# Shapes or points over the reader's limits, left out.
label-annotation-skipped = Omitidas
annotation-polygon = Polígono
annotation-polyline = Línea
annotation-rectangle = Rectángulo
annotation-ellipse = Elipse
annotation-point = Punto
annotation-points = Puntos
annotation-ruler = Regla
annotation-arrow = Flecha
# The list of annotations is cut short: "and 1,200 more".
annotation-more = y { $count } más
# All-fields group title: "Annotations in slide.xml".
fields-annotations-header = Anotaciones en { $file }
label-drawing-version = Versión
label-drawing-units = Unidades
label-drawing-extents = Extensión
label-drawing-sheet = Hoja
label-drawing-layouts = Presentaciones
label-drawing-layers = Capas
label-drawing-entities = Entidades
label-drawing-codepage = Página de códigos
label-drawing-missing-fonts = Fuentes sustituidas
# machine-drafted, needs native review
# A font file the drawing names that was not found: the styles naming it, and the font drawing its text instead.
drawing-missing-font = { $file } (estilos { $styles }), sustituida por { $substitute }
# A Big Font (double-byte text) that was not found; its characters come from other fonts.
drawing-missing-bigfont = { $file } (estilos { $styles }), sustituida por fuentes alternativas
# Survey data (LandXML).
label-survey-crs = Sistema de coordenadas
survey-crs-none = No indicado
label-survey-project = Nombre del proyecto
label-survey-application = Escrito por
label-survey-surface = Modelo de terreno
# A TIN surface: its name, counts, plan and sloped areas, the volume above its lowest point, its elevations and the contour interval drawn; unit is "m" or "ft" (areas in square units, volume in cubic units).
survey-surface = { $name }: { $points ->
        [one] { $points } punto
       *[other] { $points } puntos
    }, { $triangles ->
        [one] { $triangles } triángulo
       *[other] { $triangles } triángulos
    }; área en planta { $area } { $unit }², área real { $area3d } { $unit }²; { $volume } { $unit }³ sobre el punto más bajo; cotas { $zmin }–{ $zmax } { $unit }; curvas de nivel cada { $interval } { $unit }
label-survey-alignment = Eje
# An alignment: its name, length and first and last stations ("1+234.50").
survey-alignment = { $name }: { $length } { $unit }, PK { $start } a { $end }
# SHX shape fonts found and used for the drawing's text.
label-drawing-shx-fonts = Fuentes SHX
label-drawing-warnings = Problemas
label-plot-paper = Papel
# "2 of 5": the page shown and how many the plot has.
plot-page-of = { $page } de { $pages }
label-plot-page = Página
label-plot-pens = Plumillas usadas
# The printer language a plot came wrapped in, e.g. "PCL 5".
label-plot-job = Trabajo de impresión
# What a plot draws: strokes, fills and labels.
label-plot-items = Elementos dibujados
# Width × height of what is drawn, in drawing units: "1,250 × 800 mm".
drawing-extents = { $width } × { $height } { $unit }
drawing-extents-unitless = { $width } × { $height }
drawing-model-space = Modelo
# "24, 3 off or frozen"
drawing-layers-off = { $num }, { $off } desactivadas o inutilizadas
# DXF $INSUNITS.
drawing-units = { $code ->
    [1] Pulgadas
    [2] Pies
    [3] Millas
    [4] Milímetros
    [5] Centímetros
    [6] Metros
    [7] Kilómetros
    [8] Micropulgadas
    [9] Milésimas de pulgada
    [10] Yardas
    [11] Ångströms
    [12] Nanómetros
    [13] Micras
    [14] Decímetros
    [15] Decámetros
    [16] Hectómetros
    [17] Gigámetros
    [18] Unidades astronómicas
    [19] Años luz
    [20] Pársecs
    [21] Pies topográficos de EE. UU.
   *[other] Sin unidades
}
# All-fields group title: "DXF entities".
fields-drawing-entities = Entidades { $format }

## Layouts de circuitos integrados (GDSII)

# machine-drafted, needs native review
label-layout-library = Biblioteca
label-layout-units = Unidades
# The database unit (the grid coordinates are stored in) and the user unit, with SI symbols:
# "1 nm database unit, 1 µm user unit".
layout-units = { $db } unidad de base de datos, { $user } unidad de usuario
# The top cell shown (each top cell is a page).
label-layout-cell = Celda
label-layout-cells = Celdas
# Cells no other cell places.
label-layout-top-cells = Celdas superiores
# "TOP, TEST, PAD and 12 more"
layout-top-cells-more = { $names } y { $more } más
# Layer/datatype pairs.
label-layout-layers = Capas
# Polygons, paths and boxes, as stored (each cell once).
label-layout-shapes = Formas
label-layout-texts = Etiquetas
# Cell references and the placements they make: "1,204 (88,320 placements)".
label-layout-references = Referencias
layout-references = { $refs } ({ $placements } { $placements ->
    [one] instancia
    [many] de instancias
   *[other] instancias
})
# Shapes with every cell placement expanded.
label-layout-flat-shapes = Formas al aplanar
label-layout-bounds = Límites
# Corners of what the cell draws, micrometres: "-12.5, 0 to 2,900, 3,500 µm".
layout-bounds = { $x0 }, { $y0 } a { $x1 }, { $y1 } µm

## Placas de circuito y esquemas (Gerber, Excellon, KiCad)

# machine-drafted, needs native review
group-board = Placa
group-schematic = Esquema
# The board view shown (its layers, its top or bottom), or the schematic sheet.
label-pcb-view = Vista
label-pcb-project = Proyecto
label-pcb-revision = Revisión
label-pcb-date = Fecha
# The CAD program that wrote the file.
label-pcb-generator = Creado con
# Width × height of the board outline: "100 × 80 mm".
label-pcb-size = Tamaño de la placa
label-pcb-thickness = Grosor
label-pcb-copper-layers = Capas de cobre
label-pcb-sheets = Hojas
# The board's layers or files, each with its role: "board-F_Cu.gbr (Top copper)".
label-pcb-layers = Capas
pcb-layer-role = { $name } ({ $role })
label-pcb-components = Huellas
label-pcb-symbols = Símbolos
label-pcb-nets = Redes
label-pcb-tracks = Pistas
label-pcb-wires = Cables
label-pcb-vias = Vías
# Drilled holes and how many drill sizes: "1,204 in 9 sizes".
label-pcb-holes = Taladros
pcb-holes = { $holes } en { $sizes } { $sizes ->
    [one] tamaño
    [many] de tamaños
   *[other] tamaños
}
label-pcb-apertures = Aperturas
# What a board layer or file is, shown after its name.
pcb-role-inner-copper = Cobre interior { $n }
pcb-role-top-copper = Cobre superior
pcb-role-bottom-copper = Cobre inferior
pcb-role-top-solder-mask = Máscara de soldadura superior
pcb-role-bottom-solder-mask = Máscara de soldadura inferior
pcb-role-top-silkscreen = Serigrafía superior
pcb-role-bottom-silkscreen = Serigrafía inferior
pcb-role-top-paste = Pasta superior
pcb-role-bottom-paste = Pasta inferior
pcb-role-top-solder-paste = Pasta de soldadura superior
pcb-role-bottom-solder-paste = Pasta de soldadura inferior
pcb-role-top-adhesive = Adhesivo superior
pcb-role-bottom-adhesive = Adhesivo inferior
pcb-role-top-courtyard = Área de ocupación superior
pcb-role-bottom-courtyard = Área de ocupación inferior
pcb-role-top-fabrication = Fabricación superior
pcb-role-bottom-fabrication = Fabricación inferior
pcb-role-board-outline = Contorno de la placa
pcb-role-plated-drill = Taladrado metalizado
pcb-role-non-plated-drill = Taladrado no metalizado
pcb-role-drill = Taladrado
pcb-role-drill-holes = Taladros
pcb-role-mechanical = Mecánica
pcb-role-margin = Margen
pcb-role-drawings = Dibujos
pcb-role-comments = Comentarios
pcb-role-engineering-change-1 = Cambio de ingeniería 1
pcb-role-engineering-change-2 = Cambio de ingeniería 2
pcb-role-vias = Vías
pcb-role-other = Otra
pcb-role-unknown = Desconocida
label-pcb-macros = Macros de apertura

## Espectros (JCAMP-DX, SPC, mzML, mzXML, Bruker NMR)

# machine-drafted, needs native review
group-spectrum = Espectro
label-spectrum-format = Formato
label-spectrum-technique = Técnica
label-spectrum-title = Título
# The page shown: "3 of 1,204: Scan 1532".
label-spectrum-page = Página
spectrum-page-of = { $page } de { $pages }: { $name }
# An axis with its unit: "Wavenumber (cm⁻¹)".
spectrum-axis = { $title } ({ $unit })
label-spectrum-x = Eje X
label-spectrum-y = Eje Y
label-spectrum-nucleus = Núcleo
label-spectrum-frequency = Frecuencia del espectrómetro
label-spectrum-solvent = Disolvente
label-spectrum-pulse-program = Secuencia de pulsos
label-spectrum-spectra = Espectros
label-spectrum-points = Puntos
label-spectrum-instrument = Instrumento
# Who or what lab measured it.
label-spectrum-origin = Origen
label-spectrum-date = Medido
# How Specular made the page from the raw data (an FID transformed, phased automatically).
label-spectrum-processing = Procesado
spectrum-technique-ir = Infrarrojo
spectrum-technique-raman = Raman
spectrum-technique-uv-vis = UV-Vis
spectrum-technique-fluorescence = Fluorescencia
spectrum-technique-nmr = RMN
spectrum-technique-ms = Espectro de masas
spectrum-technique-chromatogram = Cromatograma
spectrum-technique-other = Espectro

## Modelos 3D (STL, OBJ, PLY, glTF, 3MF)

# machine-drafted, needs native review
group-mesh = Modelo 3D
label-triangles = Triángulos
label-vertices = Vértices
label-materials = Materiales
label-textures = Texturas
label-up-axis = Eje vertical
# Textures the model uses, some of which could not be read: "3 (1 missing)".
mesh-textures-missing = { $count } ({ $missing } ausentes)

## Estructuras moleculares (PDB, mmCIF, CIF, SDF, XYZ)

# machine-drafted, needs native review
group-structure = Estructura
# The entry's identifier: a PDB ID such as 1CRN, or the CIF data block's name.
label-structure-id = Entrada
label-structure-classification = Clasificación
# How the structure was determined: X-ray diffraction, electron microscopy, solution NMR…
label-structure-method = Método
# The crystallographic or cryo-EM resolution, in ångströms.
label-structure-resolution = Resolución
structure-resolution = { $value } Å
label-structure-deposited = Depositada
label-structure-formula = Fórmula
label-structure-chains = Cadenas
# Residues (amino acids, nucleotides) in the polymer chains.
label-structure-residues = Residuos
label-structure-atoms = Átomos
# Atoms after the unit cell was filled by symmetry, and those the file lists: "96 (12 in the asymmetric unit)".
structure-atoms-cell = { $count } ({ $asymmetric } en la unidad asimétrica)
label-structure-bonds = Enlaces
# Models in the file (an NMR ensemble), of which the first is shown.
label-structure-models = Modelos
# Molecules (SDF records) or data blocks in the file, of which the first is shown.
label-structure-records = Registros
structure-first-of = { $count } (se muestra el primero)
# Small molecules bound to the polymer, by their three-letter codes.
label-structure-ligands = Ligandos
# A list cut short: "HEM, NAG, SO4 and 9 more".
structure-ligands-more = { $list } y { $more } más
label-structure-cell = Celda unidad
# Unit cell edge lengths (Å) and angles (degrees).
structure-cell = a = { $a } Å, b = { $b } Å, c = { $c } Å, α = { $alpha }°, β = { $beta }°, γ = { $gamma }°
label-structure-space-group = Grupo espacial
label-structure-secondary = Estructura secundaria
structure-ss-file = Del archivo
structure-ss-computed = Calculada a partir de la cadena principal

## HDF4 y HDF-EOS2

# machine-drafted, needs native review
group-hdf4 = HDF4
# The HDF library version that wrote the file, e.g. "NCSA HDF Version 4.2 Release 15".
label-hdf4-library = Biblioteca
# Counts of the file's scientific datasets, raster images, groups (Vgroups) and tables (Vdatas).
label-hdf4-datasets = Conjuntos de datos
label-hdf4-images = Imágenes ráster
label-hdf4-vgroups = Grupos
label-hdf4-vdatas = Tablas
# HDF-EOS: NASA's Earth Observing System conventions on top of HDF4 (MODIS, ASTER, AIRS…).
group-hdf-eos = HDF-EOS
label-eos-version = Versión
# The grids, swaths and point structures the file defines.
label-eos-structures = Estructuras
eos-grids = { $count ->
    [one] { $count } rejilla ({ $names })
    [many] { $count } rejillas ({ $names })
   *[other] { $count } rejillas ({ $names })
}
eos-swaths = { $count ->
    [one] { $count } franja ({ $names })
    [many] { $count } franjas ({ $names })
   *[other] { $count } franjas ({ $names })
}
eos-points = { $count ->
    [one] { $count } estructura de puntos
    [many] { $count } estructuras de puntos
   *[other] { $count } estructuras de puntos
}
# The grid or swath the dataset shown belongs to.
label-eos-structure = Estructura
# The grid's map projection as its GCTP name, e.g. GCTP_SNSOID (sinusoidal).
label-eos-projection = Proyección
label-eos-grid-size = Tamaño de la rejilla
# Columns × rows.
eos-grid-size = { $width } × { $height }
# The grid's upper-left and lower-right corners.
label-eos-corners = Esquinas
# The product's short name from the inventory metadata, e.g. MOD09A1.
label-eos-product = Producto
# When the data was acquired.
label-eos-time = Fecha
eos-time = { $begin } a { $end }

## Imágenes planetarias (VICAR, PDS3, PDS4: datos de misiones de la NASA y la ESA)

# machine-drafted, needs native review
group-planetary = Imagen planetaria
# The body observed, e.g. MARS, Ganymede.
label-planetary-target = Objetivo
label-planetary-mission = Misión
# The spacecraft, rover or lander carrying the instrument.
label-planetary-spacecraft = Nave espacial
label-planetary-start = Hora de inicio
label-planetary-stop = Hora de fin
# The product's identifier in the archive.
label-planetary-product = ID de producto
# The map projection of a map-projected image, as the label names it.
label-planetary-projection = Proyección cartográfica
label-planetary-resolution = Resolución
planetary-metres-per-pixel = { $metres } m por píxel
planetary-pixels-per-degree = { $pixels } píxeles por grado
# Latitude and longitude of the map projection's centre.
label-planetary-centre = Centro de proyección
planetary-lat-lon = { $lat }°, { $lon }°
# The latitudes and longitudes the map covers.
label-planetary-coverage = Cobertura
planetary-coverage = latitud { $south }° a { $north }°, longitud { $west }° a { $east }°
# The body's equatorial radius the map uses.
label-planetary-radius = Radio del cuerpo
planetary-km = { $km } km

## OMERO (imágenes abiertas desde un servidor OMERO o el Image Data Resource)

# machine-drafted, needs native review
group-omero = OMERO
label-omero-server = Servidor
# $server the server's address; $user the user name signed in with.
omero-server-user = { $server } (como { $user })
omero-server-public = { $server } (público)
label-omero-image = Imagen
# $name the image's name; $id its number on the server.
omero-image = { $name } (ID { $id })
# The project and dataset the image is in, as "Project › Dataset".
label-omero-project = Proyecto
# The screen, plate and well, as "Screen › Plate › B4".
label-omero-screen = Cribado
label-omero-owner = Propietario
# $owner the image's owner; $group the OMERO group it belongs to.
omero-owner-group = { $owner } ({ $group })
label-omero-acquired = Adquisición
label-omero-dimensions = Dimensiones
omero-size = { $width } × { $height }
omero-z = { $count ->
    [one] { $count } plano Z
   *[other] { $count } planos Z
}
omero-c = { $count ->
    [one] { $count } canal
   *[other] { $count } canales
}
omero-t = { $count ->
    [one] { $count } punto temporal
   *[other] { $count } puntos temporales
}
# The physical size of a pixel along X, Y (and Z).
label-omero-physical = Tamaño de píxel
label-omero-channels = Canales
# A channel not shown in the server's rendering.
omero-channel-hidden = { $channel } (oculto)
# How the pixels are fetched from the server: rendered tiles (JPEG) or the whole plane (PNG).
label-omero-fetched = Obtenido como
omero-fetched-tiles = Teselas { $encoding } de { $width } × { $height }, { $levels ->
    [one] { $levels } nivel
   *[other] { $levels } niveles
}
omero-fetched-whole = Planos completos en { $encoding }

## Imágenes de transformación de reflectancia (PTM, RTI, Relight)

# machine-drafted, needs native review
group-rti = Transformación de reflectancia
# The reflectance model the coefficients belong to: PTM (polynomial), HSH (hemispherical harmonics), RBF…
label-rti-basis = Base
label-rti-order = Orden
# Coefficients per colour channel.
label-rti-terms = Coeficientes
# How colour is stored: LRGB (one colour, varying luminance), RGB, YCC…
label-rti-colour = Disposición del color
# Coefficient images stored in the file.
label-rti-planes = Planos
# The byte-to-coefficient mapping the file states, per coefficient.
label-rti-scale = Escala
label-rti-bias = Desplazamiento

## Nubes de puntos

# machine-drafted, needs native review
group-points = Nube de puntos
label-points = Puntos
# Points read of those in the file, when a very large cloud was evenly thinned: "32,000,000 of 120,000,000".
points-subsampled = { $loaded } de { $total }
# The LAS point data record format number (0–10).
label-point-format = Formato de punto
# The coordinate reference system.
label-crs = Sistema de coordenadas
# What each point carries besides its position.
label-attributes = Atributos
points-channel-rgb = RGB
points-channel-intensity = Intensidad
points-channel-classification = Clasificación
points-channel-returns = Retornos
# ASPRS classes present, with point counts: "Ground (1,234), Building (567)".
label-classes = Clases
# E57 scans, with point counts.
label-scans = Escaneos
point-class-0 = Nunca clasificado
point-class-1 = Sin clasificar
point-class-2 = Suelo
point-class-3 = Vegetación baja
point-class-4 = Vegetación media
point-class-5 = Vegetación alta
point-class-6 = Edificio
point-class-7 = Punto bajo (ruido)
point-class-8 = Punto clave del modelo
point-class-9 = Agua
point-class-10 = Ferrocarril
point-class-11 = Superficie de carretera
point-class-12 = Solape
point-class-13 = Cable de guarda
point-class-14 = Conductor
point-class-15 = Torre de transmisión
point-class-16 = Conector de cable
point-class-17 = Tablero de puente
point-class-18 = Ruido alto
point-class-19 = Estructura elevada
point-class-20 = Suelo ignorado
point-class-21 = Nieve
point-class-22 = Exclusión temporal
# A class without a standard name, by number.
point-class-other = Clase { $class }

## Gaussian splats

# machine-drafted, needs native review
group-splats = Splats gaussianos
label-splats = Splats
# Splats shown of those in the file, when a very large file was evenly thinned: "12,000,000 of 30,000,000".
splats-shown-of = { $shown } de { $total }
# The degree (1 to 3) of the spherical harmonics that give each splat its view-dependent colour.
label-splat-harmonics = Color dependiente de la vista
splats-harmonics-none = Ninguno
# $format: the file format, e.g. "SPZ".
splats-format-antialiased = { $format }, entrenado con antialiasing

## Advertencias

# machine-drafted, needs native review
warn-not-diagnostic = No apto para uso diagnóstico. Specular es un visor de imágenes general, no un producto sanitario: su representación no está calibrada para el diagnóstico.

warn-gps = Este archivo registra dónde se tomó la foto. Elimine la ubicación antes de compartirlo si debe permanecer privada.
warn-null-island = La posición GPS es exactamente 0°, 0°; probablemente la cámara no tenía señal, así que es probable que la ubicación sea errónea.
warn-date-future = La fecha de captura está en el futuro; probablemente el reloj de la cámara estaba mal configurado.
warn-icc-damaged = El perfil de color incrustado está dañado y no se ha podido leer.
warn-metadata-failed = No se pudieron leer los metadatos por un error de Specular. La imagen en sí está bien.
warn-orientation-conflict = EXIF y XMP no coinciden en la orientación (EXIF: { $exif }, XMP: { $xmp }). Se usa el valor EXIF.
warn-exif-damaged = No se han podido leer algunos datos EXIF ({ $error }); se muestra el resto.
warn-xmp-damaged = Los metadatos XMP están mal formados ({ $error }); solo se ha podido leer una parte.

## Ventana «Acerca de»

geonames-attribution = Nombres de lugares: GeoNames (www.geonames.org), con licencia Creative Commons Atribución 4.0 (CC BY 4.0, https://creativecommons.org/licenses/by/4.0/). Búsqueda sin conexión mediante el crate reverse_geocoder de Grant Miner (MIT o Apache-2.0). Los datos se han convertido en una lista de ciudades; se proporcionan tal cual, sin garantía.

## TIFF and DNG structure values (All fields)
# machine-drafted, needs native review

subfile-type = { $value ->
    [0] Imagen a resolución completa
    [1] Imagen a resolución reducida
    [2] Una página de una imagen de varias páginas
    [4] Máscara de transparencia
   *[other] Otro
}
predictor = { $value ->
    [1] Ninguno
    [2] Diferencia horizontal
    [3] Diferencia de coma flotante
   *[other] Otro
}
extra-samples = { $value ->
    [0] Sin especificar
    [1] Alfa premultiplicado
    [2] Alfa no premultiplicado
   *[other] Otro
}
sample-format = { $value ->
    [1] Entero sin signo
    [2] Entero con signo
    [3] Coma flotante
    [4] Indefinido
   *[other] Otro
}

## Datos geoespaciales (GeoTIFF, GeoJSON, Shapefile, GeoPackage, KML/KMZ, GPX)

# machine-drafted, needs native review
group-geospatial = Geoespacial
label-geo-crs = Sistema de coordenadas
label-geo-datum = Datum
label-geo-projection = Proyección
label-geo-unit = Unidades
label-geo-bounds = Extensión
label-geo-lonlat-bounds = Longitud y latitud
label-geo-pixel-size = Tamaño de píxel
label-geo-overviews = Vistas generales
label-geo-nodata = Valor sin datos
label-geo-raster-type = Tipo de ráster
label-geo-vertical = Referencia vertical
label-geo-citation = Cita
label-geo-features = Entidades
label-geo-vertices = Vértices
label-geo-layer = Capa
label-geo-layers = Capas
label-geo-tiles = Teselas
label-geo-encoding = Codificación de texto
label-geo-warnings = Problemas
# The companion files a georeference was read from ("scene.jgw, scene.prj").
label-geo-sidecars = Archivos complementarios
# machine-drafted, needs native review
# A projection Specular cannot convert to longitude and latitude: $method is its name.
geo-projection-unsupported = { $method } (coordenadas no admitidas)
# How longitude and latitude are brought to WGS 84 (what GPS and web maps use).
label-geo-to-wgs84 = A WGS 84
geo-to-wgs84-same = No hace falta
geo-to-wgs84-helmert = Cambio de datum (Helmert)
geo-to-wgs84-by-area = Cambio de datum (Helmert, elegido por zona)
geo-to-wgs84-unknown = Desconocido: las coordenadas están en el datum propio del archivo
# A coordinate system's name and its EPSG code: "WGS 84 / UTM zone 33N (EPSG:32633)".
geo-crs-epsg = { $name } (EPSG:{ $code })
geo-crs-unknown = No indicado
# Map bounds: the lower-left and upper-right corners.
geo-bounds = { $x0 }, { $y0 } a { $x1 }, { $y1 }
# Bounds in degrees: west to east longitude, south to north latitude.
geo-lonlat-bounds = { $west }° a { $east }° de longitud, { $south }° a { $north }° de latitud
geo-pixel-size = { $x } × { $y }
geo-overviews = { $n ->
    [one] { $n } vista general: { $sizes }
    [many] { $n } de vistas generales: { $sizes }
   *[other] { $n } vistas generales: { $sizes }
}
geo-pixel-is-point = Los píxeles son puntos
geo-layer = { $features ->
    [one] { $features } entidad
    [many] { $features } de entidades
   *[other] { $features } entidades
}, { $fields ->
    [one] { $fields } campo
    [many] { $fields } de campos
   *[other] { $fields } campos
}
# A GeoPackage tile pyramid: its table, the zoom level shown and its tile count.
geo-tiles = { $table }, zoom { $zoom }, { $tiles ->
    [one] { $tiles } tesela
    [many] { $tiles } de teselas
   *[other] { $tiles } teselas
}
fields-geo-geokeys = Claves GeoTIFF
fields-geo-gdal = Metadatos de GDAL
fields-geo-wkt = Sistema de coordenadas (WKT)
fields-geo-projjson = Sistema de coordenadas (PROJJSON)
# machine-drafted, needs native review
# What a web map tile set (MBTiles, PMTiles) says about itself: container, tile type, zoom
# levels, description and other metadata.
fields-geo-tileset = Conjunto de mosaicos
# The credit line a map's data asks to be shown with it ("© OpenStreetMap contributors").
label-geo-attribution = Créditos
# The header lines of a georeferenced raster format (ENVI, ASCII grid, DTED, ERDAS IMAGINE).
fields-geo-header = Cabecera
# machine-drafted, needs native review
# GIFTI files: the format version, what the file holds, the number of data arrays and the
# encoding of their values (e.g. "GZipBase64Binary").
label-gifti-version = Versión de GIFTI
label-gifti-role = Contiene
gifti-role-surface = Superficie
gifti-role-labels = Etiquetas por vértice
gifti-role-data = Valores por vértice
label-gifti-arrays = Matrices de datos
label-gifti-encoding = Codificación
label-gifti-vertices = Vértices
label-gifti-triangles = Triángulos
# The brain structure, e.g. "CortexLeft".
label-gifti-structure = Estructura
# A surface's geometric type, e.g. "Anatomical", "Inflated".
label-gifti-geometry = Tipo geométrico
# The coordinate transform of the vertices: $from and $to are coordinate spaces such as
# "scanner", "Talairach" or "MNI 152".
label-gifti-transform = Coordenadas
gifti-transform = { $from } (no hace falta transformar a { $to })
gifti-transform-applied = { $from } → { $to } (aplicada)
# The surface file a data file is drawn on, found beside it.
label-gifti-surface = Se muestra sobre
# The data array shown: its name, NIfTI intent (e.g. "NIFTI_INTENT_SHAPE") and data type.
label-gifti-array = Matriz
label-gifti-intent = Intent
label-gifti-data-type = Tipo de datos
# Entries of the label table (names and colours of regions).
label-gifti-labels = Tabla de etiquetas
# CIFTI-2 files: the version, NIfTI intent (e.g. "ConnDenseScalar") and matrix size.
label-cifti-version = Versión de CIFTI
label-cifti-intent = Intent
label-cifti-matrix = Matriz
cifti-matrix = { $rows } × { $columns }
# The maps (pages) of the file and what they are.
label-cifti-maps = Mapas
cifti-maps-scalars = { $count ->
        [one] { $count } mapa escalar
       *[other] { $count } mapas escalares
    }
cifti-maps-labels = { $count ->
        [one] { $count } mapa de etiquetas
       *[other] { $count } mapas de etiquetas
    }
cifti-maps-series = { $count ->
        [one] { $count } instante
       *[other] { $count } instantes
    }
cifti-maps-parcels = { $count ->
        [one] { $count } parcela
       *[other] { $count } parcelas
    }
cifti-maps-other = { $count }
# machine-drafted, needs native review
# A dense connectivity file (.dconn.nii): every brain place's connectivity to every other.
# $seeds: how many places can be picked; $size: the bytes read when one is picked (e.g. "357 KB").
label-cifti-dense = Conectividad
cifti-dense = { $seeds ->
        [one] { $seeds } semilla
       *[other] { $seeds } semillas
    }, { $size } leídos por semilla
# A time series' start and step; $unit is e.g. "second".
label-cifti-series = Serie
cifti-series = desde { $start }, cada { $step } ({ $unit })
label-cifti-models = Modelos cerebrales
label-cifti-parcels = Parcelas
label-cifti-structures = Estructuras
label-cifti-voxels = Vóxeles subcorticales
# A surface the file's values lie on: the brain structure (e.g. "CORTEX_LEFT"), its vertex
# count, and the surface file found beside the file for it, or that none was found.
label-cifti-surface = Superficie
cifti-surface-found = { $structure }: { $vertices ->
        [one] { $vertices } vértice
       *[other] { $vertices } vértices
    }, { $file }
cifti-surface-missing = { $structure }: necesita una superficie con { $vertices ->
        [one] { $vertices } vértice
       *[other] { $vertices } vértices
    } (no hay ninguna junto al archivo)
# How the file is shown: on the surfaces found beside it, or as its subcortical voxels.
label-cifti-shown = Se muestra como
cifti-shown-surfaces = Superficies coloreadas
cifti-shown-voxels = Volumen de los vóxeles subcorticales
# machine-drafted, needs native review
# Shown with a picked seed's row: click a surface vertex, or move the crosshair in MPR.
cifti-shown-connectivity = Conectividad de la semilla elegida
# Maps left out of the voxel volume, which would be too large with them.
label-cifti-left-out = Mapas omitidos
# Tractograms: format, number of streamlines (fibre tracts) and points.
label-tract-format = Formato
label-tract-streamlines = Líneas de corriente
# A large tractogram draws only some of its streamlines.
tract-streamlines-shown = { $count } ({ $shown } mostradas)
label-tract-points = Puntos
label-tract-mean-length = Longitud media
# A length in millimetres.
tract-mm = { $value } mm
# Values per point and per streamline the file stores, by name.
label-tract-scalars = Escalares por punto
label-tract-properties = Propiedades por línea de corriente
# TrackVis: the voxel grid the tracts were made in, its voxel size and voxel order (e.g. "LAS").
label-tract-grid = Rejilla
label-tract-voxel-size = Tamaño de vóxel
label-tract-voxel-order = Orden de vóxeles
# How the points were brought into scanner (RAS) millimetres.
label-tract-space = Coordenadas
tract-space-vox-to-ras = A partir de la matriz vóxel a RAS
tract-space-voxel-order = A partir del orden de vóxeles (sin matriz vóxel a RAS)
tract-space-scanner = Espacio del escáner
# MRtrix: the tracking step size.
label-tract-step = Tamaño de paso

## File headers (DPX, Cineon, XWD, MIFF, TIM, WSQ, JBIG)

# machine-drafted, needs native review
# What the file's own header records, for formats without Exif or XMP.
group-header = Cabecera del archivo
label-header-created = Creado
# The program or person that wrote the file.
label-header-creator = Autor
label-header-copyright = Copyright
label-header-project = Proyecto
label-header-comment = Comentario
# The scanner, camera or other device the image came from.
label-header-device = Dispositivo
# SMPTE time code of the frame: "01:00:00:12".
label-header-timecode = Código de tiempo
# A film frame's edge code (KeyKode).
label-header-edge-code = Código de borde
label-header-frame-rate = Velocidad de fotogramas
# Where the frame sits in its sequence.
label-header-frame = Fotograma
# Film stock and slate.
label-header-film = Película
# How code values relate to light: printing density, linear, BT.709…
label-header-transfer = Función de transferencia
# The colour primaries the file states.
label-header-colorimetric = Colorimetría
# The X window a screen dump was taken of.
label-header-window = Ventana
label-header-resolution = Resolución
label-header-compression = Compresión
# machine-drafted, needs native review
# JPEG XS: the profile and the level the codestream states ("Main 422.10"; "4k-1, sublevel 6 bpp").
label-header-profile = Perfil
# machine-drafted, needs native review
label-header-level = Nivel
# machine-drafted, needs native review
# JPEG XS: how many wavelet levels, horizontal and vertical ("5 horizontal, 1 vertical").
label-header-decomposition = Niveles wavelet
# machine-drafted, needs native review
# JPEG XS: the colour transform ("None", "Reversible colour transform (RCT)", "Star-Tetrix, RGGB mosaic").
label-header-colour-transform = Transformación de color
# machine-drafted, needs native review
# Photoshop: the artboards, by name and size ("Phone (375 × 667), Tablet (768 × 1024)").
label-header-artboards = Mesas de trabajo
# machine-drafted, needs native review
# Photoshop file without a merged image, shown from its layers: the features it leaves out,
# by Photoshop's names ("Color Lookup, Knockout").
label-header-not-drawn = No dibujado
# machine-drafted, needs native review
# The same, for the features drawn only approximately ("Bevel & Emboss, Satin").
label-header-approximated = Dibujado de forma aproximada

## Fonts (TTF, OTF, TTC/OTC, WOFF, WOFF2)

# machine-drafted, needs native review
group-font = Fuente
label-font-family = Familia
label-font-style = Estilo
# The face's place in a font collection: "2 of 4".
label-font-face = Cara
font-face-of = { $index } de { $count }
label-font-version = Versión
label-font-designer = Diseño
# The font's maker (the name table's manufacturer).
label-font-manufacturer = Fundición
label-font-glyphs = Glifos
label-font-outlines = Contornos
font-outlines-truetype = TrueType (cuadráticos)
font-outlines-cff = PostScript (CFF, cúbicos)
font-outlines-cff2 = PostScript (CFF2, cúbicos)
font-outlines-bitmap = Solo mapas de bits
font-outlines-none = Ninguno
# What the font's licence flags (OS/2 fsType) allow documents that embed it to do.
label-font-embedding = Incrustación
font-embedding-installable = Se puede incrustar e instalar.
font-embedding-restricted = No se puede incrustar.
font-embedding-preview-print = Se puede incrustar solo para ver e imprimir.
font-embedding-editable = Se puede incrustar en documentos que se editan.
font-embedding-no-subsetting = Solo completa, no como subconjunto.
font-embedding-bitmap-only = Solo como mapas de bits.
# Variation axes of a variable font: "Weight 100–900 (400)".
label-font-axes = Ejes de variación
font-axis = { $name } { $min }–{ $max } ({ $default })
label-font-instances = Estilos con nombre
label-font-colour = Glifos en color
font-colour-colr0 = Capas COLR
font-colour-colr1 = Degradados COLR v1
font-colour-sbix = Mapas de bits (sbix)
font-colour-cbdt = Mapas de bits (CBDT)
font-colour-svg = SVG
label-font-weight = Peso
label-font-units-per-em = Unidades por em
label-font-tables = Tablas
label-font-licence = Licencia
label-font-licence-url = URL de la licencia
label-font-copyright = Copyright
label-font-trademark = Marca registrada

## Office documents (Word, Excel, PowerPoint, iWork, OpenDocument)

# machine-drafted, needs native review
group-document = Documento
label-document-kind = Tipo
document-kind-word = Documento de Word
document-kind-excel = Libro de Excel
document-kind-powerpoint = Presentación de PowerPoint
document-kind-pages = Documento de Pages
document-kind-numbers = Hoja de cálculo de Numbers
document-kind-keynote = Presentación de Keynote
document-kind-odf-text = Texto OpenDocument
document-kind-odf-spreadsheet = Hoja de cálculo OpenDocument
document-kind-odf-presentation = Presentación OpenDocument
document-kind-odf-graphics = Dibujo OpenDocument
document-kind-odf-other = Archivo OpenDocument
# machine-drafted, needs native review
document-kind-playlist = Lista de reproducción
# machine-drafted, needs native review
label-playlist-items = Elementos
# Appended to the kind: "Word document, template, with macros".
document-template = plantilla
document-macros = con macros
document-legacy = formato Office 97–2003
label-document-preview = Vista previa
# The image format of the preview stored in the file, e.g. "JPEG image".
document-preview-image = Imagen { $format }
document-preview-none = Ninguna en este archivo
label-document-subject = Asunto
label-document-author = Autor
label-document-description = Descripción
label-document-category = Categoría
label-document-last-saved-by = Guardado por última vez por
label-document-created = Creado
label-document-modified = Modificado
label-document-printed = Última impresión
label-document-pages = Páginas
label-document-slides = Diapositivas
label-document-hidden-slides = Diapositivas ocultas
label-document-notes = Páginas de notas
label-document-sheets = Hojas
label-document-words = Palabras
label-document-characters = Caracteres
label-document-paragraphs = Párrafos
label-document-lines = Líneas
label-document-editing-time = Tiempo de edición
document-minutes = { $minutes } min
label-document-revision = Revisión
label-document-application = Guardado con
label-document-company = Compañía
label-document-template = Plantilla

## Books (EPUB)

# machine-drafted, needs native review
group-book = Libro
label-book-authors = Autores
label-book-publisher = Editorial
label-book-date = Publicado
label-book-language = Idioma
label-book-pages = Páginas
# Entries in the book's table of contents.
label-book-contents = Entradas del índice
label-book-layout = Maquetación
book-layout-reflowable = Adaptable
book-layout-fixed = Maquetación fija
book-right-to-left = de derecha a izquierda
label-book-version = Formato
book-version = EPUB { $version }
label-book-fonts = Fuentes incrustadas
book-fonts-obfuscated = { $count } ({ $obfuscated } ofuscadas)
label-book-subjects = Temas
label-book-identifier = Identificador
label-book-rights = Derechos

unit-symbol-mm = mm
unit-symbol-cm = cm
unit-symbol-m = m
unit-symbol-km = km
unit-symbol-mm2 = mm²
unit-symbol-cm2 = cm²
unit-symbol-m2 = m²
unit-symbol-km2 = km²
unit-symbol-ha = ha
unit-symbol-ft = ft
unit-symbol-ft2 = ft²
unit-symbol-mi = mi
unit-symbol-mi2 = mi²
unit-symbol-ac = ac

# machine-drafted, needs native review
volume-orient-parrec = Angulación y descentrado de Philips
volume-orient-ecat = Tamaños de vóxel y desplazamientos ECAT (no fiables)
label-volume-technique = Técnica
label-volume-version = Versión del formato
label-volume-isotope = Isótopo
label-volume-tracer = Trazador
label-volume-units = Unidades

# Text, tables and subtitles.
label-header-encoding = Juego de caracteres
label-header-lines = Líneas
label-header-line-ends = Finales de línea
label-header-words = Palabras
label-header-pages = Páginas
label-header-pictures = Imágenes
label-header-pictures-not-shown = Imágenes no mostradas
label-header-tables = Tablas
label-header-rows = Filas
label-header-columns = Columnas
label-header-delimiter = Delimitador
label-header-column-names = Nombres de columnas
label-header-cues = Subtítulos
label-header-cue-span = Subtítulos de – a
