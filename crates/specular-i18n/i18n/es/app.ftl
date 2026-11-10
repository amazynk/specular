# machine-drafted, needs native review
# Specular — interfaz del visor, paleta, errores y «Acerca de». Español (internacional neutro).

-app-name = Specular

## Descripciones emergentes y etiquetas de accesibilidad de la barra de herramientas

toolbar-open = Abrir imagen…
toolbar-prev = Imagen anterior
toolbar-next = Imagen siguiente
toolbar-back-gallery = Volver a la galería
toolbar-zoom-out = Alejar
toolbar-zoom-in = Acercar
toolbar-zoom-level = Nivel de zoom y valores predefinidos
toolbar-zoom-fit = Ajustar a la ventana
toolbar-zoom-100 = Tamaño real ({ PERCENT(100) })
toolbar-rotate-left = Girar a la izquierda
toolbar-rotate-right = Girar a la derecha
toolbar-fullscreen-enter = Activar pantalla completa
toolbar-fullscreen-exit = Salir de pantalla completa
toolbar-info = Información de la imagen
toolbar-settings = Configuración
toolbar-delete = Mover a la papelera
toolbar-copy = Copiar imagen
toolbar-palette = Paleta de comandos
toolbar-go-to = Ir a la imagen…
toolbar-go-to-range = 1–{ $count }
# machine-drafted, needs native review
toolbar-customise = Personalizar barra de herramientas…
toolbar-more = Más
toolbar-announce-moved = { $item }: posición { $index } de { $count }
toolbar-announce-added = { $item } añadido: posición { $index } de { $count }
toolbar-announce-removed = { $item } quitado de la barra de herramientas
toolbar-announce-reset = Barra de herramientas restablecida
tooltip-with-shortcut = { $label } ({ $shortcut })

## Indicadores

readout-position = { $index } / { $count }
readout-position-a11y = Imagen { $index } de { $count }
readout-zoom = { $zoom }
readout-zoom-a11y = Zoom { $zoom }
readout-zoom-fit = Ajustado ({ $zoom })
zoom-preset-fit = Ajustar a la ventana
zoom-preset-fill = Rellenar la ventana
zoom-preset-actual = Tamaño real
# machine-drafted, needs native review
# The last entry of the zoom presets: the window fills the screen (ticked while it does).
zoom-preset-fullscreen = Pantalla completa
zoom-preset-percent = { $zoom }
readout-dimensions = { $width } × { $height }
status-image-count =
    { $count ->
        [one] { $count } imagen
        [many] { $count } de imágenes
       *[other] { $count } imágenes
    }

## Paleta de comandos

palette-placeholder = Escriba un comando o un ajuste…
palette-no-results = No hay comandos coincidentes
palette-set-setting = Establecer: { $setting } → { $value }
palette-next = Imagen siguiente
palette-prev = Imagen anterior
palette-first = Primera imagen
palette-last = Última imagen
palette-go-to = Ir a la imagen…
palette-zoom-in = Acercar
palette-zoom-out = Alejar
palette-zoom-fit = Ajustar a la ventana
palette-zoom-fill = Rellenar la ventana
# machine-drafted, needs native review
palette-zoom-fit-width = Ajustar al ancho
palette-zoom-fit-height = Ajustar al alto
palette-zoom-100 = Zoom al { PERCENT(100) }
palette-zoom-200 = Zoom al { PERCENT(200) }
palette-zoom-300 = Zoom al { PERCENT(300) }
palette-zoom-400 = Zoom al { PERCENT(400) }
palette-zoom-rectangle = Zoom a la selección
palette-rotate-left = Girar a la izquierda
palette-rotate-right = Girar a la derecha
palette-flip-horizontal = Voltear horizontalmente
palette-flip-vertical = Voltear verticalmente
palette-toggle-fullscreen = Activar o desactivar pantalla completa
palette-navigator = Mostrar u ocultar el navegador
palette-loupe = Lupa
palette-pixel-grid = Mostrar u ocultar la cuadrícula de píxeles
palette-info-panel = Mostrar u ocultar el panel de información
palette-copy-image = Copiar imagen
# machine-drafted, needs native review
palette-read-codes = Leer códigos
# machine-drafted, needs native review
palette-find-text = Buscar texto
# machine-drafted, needs native review
palette-face-boxes = Mostrar caras
palette-copy-path = Copiar ruta del archivo
palette-move-to-trash = Mover a la papelera
palette-undo-trash = Deshacer mover a la papelera
palette-open = Abrir imagen…
# machine-drafted, needs native review
palette-open-address = Abrir dirección…
palette-settings = Configuración
# machine-drafted, needs native review
palette-customise-toolbar = Personalizar barra de herramientas…
palette-cheat-sheet = Atajos de teclado
palette-hud = Mostrar u ocultar el HUD de tiempo de fotograma
palette-language = Idioma
palette-theme = Tema
# machine-drafted, needs native review
palette-interface-larger = Interfaz más grande
palette-interface-smaller = Interfaz más pequeña
palette-interface-reset = Interfaz al { PERCENT(100) }
palette-quit = Salir de { -app-name }

## Hoja de referencia

cheatsheet-title = Atajos de teclado
cheatsheet-navigation = Navegación
cheatsheet-zoom-pan = Zoom y desplazamiento
cheatsheet-view = Visualización
cheatsheet-file = Archivo
cheatsheet-window = Ventana
cheatsheet-help = Ayuda
cheatsheet-hold = Mantener { $key }
cheatsheet-close-hint = Pulse { $key } o Esc para cerrar

## Errores

# machine-drafted, needs native review
viewer-no-preview = Este archivo no contiene vista previa.
error-cannot-open = No se puede abrir { $file }
error-cannot-open-reason = No se puede abrir { $file }: { $reason }
error-unsupported-format = { $file } está en un formato que { -app-name } aún no puede leer.
error-file-truncated = { $file } está incompleto. Se muestra la parte que se pudo leer.
error-file-too-large = { $file } es demasiado grande para abrirlo ({ $megapixels } megapíxeles; el límite es de { $limit } megapíxeles).
error-not-found = { $file } ya no existe.
error-permission-denied = No tiene permiso para leer { $file }.
error-details = Detalles
crash-last-time = { -app-name } se cerró inesperadamente la última vez; se guardó un informe en { $path }
error-trash-failed = No se pudo mover { $file } a la papelera: { $reason }
# machine-drafted, needs native review
# Motivos, mostrados tras «No se puede abrir ARCHIVO:» y otros mensajes «No se pudo …:», por eso
# empiezan en minúscula. Dicen qué pasó y, cuando ayuda, qué hacer.
error-reason-not-found = ya no está. Puede que se haya movido, cambiado de nombre o eliminado.
error-reason-permission = no tienes permiso para leerlo. Comprueba su propietario y sus permisos.
error-reason-disk-full = el disco está lleno. Libera espacio y vuelve a intentarlo.
error-reason-read-only = el disco es de solo lectura.
error-reason-truncated = el archivo está incompleto. Puede que aún se esté copiando o descargando.
error-reason-is-folder = es una carpeta, no un archivo.
# A remote store (a Zarr image on a server or in an S3 bucket).
error-reason-remote-denied = el servidor denegó el acceso. Puede que los datos sean privados; para un bucket de S3, configura primero las credenciales de AWS.
error-reason-remote-missing = no hay datos Zarr en esa dirección.
# $status is an HTTP status code ("500").
error-reason-remote-status = el servidor respondió con el error { $status }.
error-reason-offline = no se puede acceder al servidor. Comprueba la dirección y la conexión.
# machine-drafted, needs native review
error-reason-network-off = la red está desactivada en Configuración › Privacidad.
error-reason-timed-out = la unidad o el servidor no respondió a tiempo. Comprueba la conexión y vuelve a intentarlo.
error-reason-memory = no hay memoria suficiente. Cierra otras imágenes o aplicaciones y vuelve a intentarlo.
error-reason-unsupported = está en un formato que { -app-name } aún no puede leer.
# $detail es el texto del propio decodificador, técnico y en inglés («12-bit lossless JPEG»).
error-reason-unsupported-detail = usa una función que { -app-name } aún no puede leer ({ $detail }).
# machine-drafted, needs native review
# A file whose extension several formats share (".img") and whose content is none of them.
# $formats lists the format names, e.g. "ERDAS IMAGINE, ADSC SMV, Analyze 7.5".
error-reason-not-one-of = no es ninguno de los archivos .{ $ext } que { -app-name } lee ({ $formats }).
error-reason-damaged = el archivo está dañado o no es lo que indica su nombre ({ $detail }).
error-reason-too-large = es demasiado grande para abrirlo.
# machine-drafted, needs native review
error-reason-needs-memory = necesita { $needed } de RAM libre, pero ahora solo hay { $free } libres. Cierra otras aplicaciones y vuelve a intentarlo, o ábrelo en un ordenador con más memoria.
error-needs-memory = Esta imagen necesita { $needed } de RAM libre, pero ahora solo hay { $free } libres. Cierra otras aplicaciones y vuelve a intentarlo, o ábrela en un ordenador con más memoria.
error-needs-memory-short = Necesita { $needed } de RAM libre
error-reason-cancelled = se detuvo su apertura.
error-reason-trash-access = ya no está, o no tienes permiso para modificarlo.
error-reason-restore-collision = ya hay un archivo con el mismo nombre en su carpeta.
error-reason-trash-root = no se puede mover una unidad entera a la papelera.
error-reason-not-in-trash = ya no está en la papelera.
error-reason-program-missing = el programa no está instalado o se ha movido. Vuelve a elegirlo o reinstálalo.
error-reason-program-not-allowed = el programa no tiene permiso para ejecutarse. Comprueba que sea ejecutable.
# machine-drafted, needs native review
error-reason-permission-write = no tienes permiso para modificarlo. Comprueba su propietario y sus permisos.
error-reason-file-has-errors = tiene errores. Corrígelos primero para que no se pierda nada de su contenido.
error-reason-not-dicom = no es un archivo DICOM.
error-reason-no-image = no contiene ninguna imagen.
error-reason-needs-layout = es un volumen sin cabecera. Para abrirlo se necesitan su tamaño, su tipo de muestra y su orden de bytes.
error-reason-not-archive = no es un archivo comprimido que { -app-name } pueda leer.
error-reason-needs-sandbox = este tipo de archivo comprimido solo lo abre el lector protegido de { -app-name }, que no está disponible.
error-reason-encrypted = está cifrado y necesita su contraseña.
error-reason-wrong-password = la contraseña es incorrecta.
error-reason-checksum = el archivo está dañado: su contenido no coincide con su suma de comprobación.
error-reason-missing-entry = falta un archivo que contiene.
error-reason-busy = otro programa lo está usando. Ciérralo allí y vuelve a intentarlo.
error-reason-catalogue-damaged = el archivo del catálogo está dañado.
error-reason-cannot-open = no se puede abrir su archivo. Comprueba que su carpeta existe y que puedes escribir en ella.
error-reason-newer-version = lo escribió una versión más reciente de { -app-name }. Actualiza { -app-name } para modificarlo.
error-reason-internal = algo falló dentro de { -app-name }. El registro tiene los detalles.
error-reason-volume-too-few = { $count ->
    [one] un corte es muy poco para construir un volumen.
    [many] { $count } de cortes son muy pocos para construir un volumen.
   *[other] { $count } cortes son muy pocos para construir un volumen.
}
error-reason-volume-no-geometry = un corte no indica dónde se sitúa ni hacia dónde está orientado.
error-reason-volume-not-parallel = los cortes no son paralelos.
error-reason-volume-shared-position = varias imágenes comparten cada una de { $count ->
        [one] { $count } posición
       *[other] { $count } posiciones
    } (fases, ecos o una serie temporal).
error-reason-volume-missing = { $count ->
        [one] falta alrededor de { $count } corte.
       *[other] faltan unos { $count } cortes.
    }
error-reason-volume-mismatch = los cortes difieren en tamaño o en espaciado de píxeles.
error-reason-volume-not-grey = los cortes están en color, no en escala de grises.
error-reason-volume-skewed = los ejes del volumen no forman ángulos rectos.
error-reason-volume-too-large = necesita { $need } MiB y solo se permiten { $budget } MiB.
error-reason-url-not-specular = no es un enlace de { -app-name }.
error-reason-url-too-long = es demasiado largo.
error-reason-url-damaged = está dañado. Puede que se haya cortado o modificado al copiarlo.
error-reason-url-unknown = pide algo que esta versión de { -app-name } no conoce. Actualizar { -app-name } puede ayudar.
error-reason-url-not-absolute = sus rutas de archivo deben ser rutas completas.
error-reason-url-no-paths = no indica ningún archivo.
error-reason-url-too-many = indica demasiados archivos.

## Notificaciones

# machine-drafted, needs native review
# A background task (a listing, an export, a file operation) stopped part way; $reason is
# error-reason-internal.
background-failed = No se pudo terminar: { $reason }
toast-moved-to-trash = Movido a la papelera
toast-moved-to-trash-named = { $file } se ha movido a la papelera
toast-moved-to-trash-count =
    { $count ->
        [one] Se ha movido { $count } imagen a la papelera
        [many] Se han movido { $count } de imágenes a la papelera
       *[other] Se han movido { $count } imágenes a la papelera
    }
toast-undo = Deshacer
toast-restored = { $file } se ha restaurado
toast-copied-image = Imagen copiada
toast-copied-path = Ruta copiada
toast-copied-report = Informe copiado
toast-copied-value = Valor copiado
toast-loop-start = De vuelta a la primera imagen
toast-loop-end = Se ha saltado a la última imagen
# machine-drafted, needs native review
toast-interface-size = Tamaño de la interfaz { PERCENT($percent) }
toast-language-changed = Idioma cambiado a { $language }

## Estados vacíos

empty-drop = Arrastre una imagen aquí o pulse { $shortcut }
empty-open-button = Abrir imagen…
empty-drop-release = Suelte para abrir
empty-folder = Esta carpeta no contiene imágenes que { -app-name } pueda abrir.
# machine-drafted, needs native review
# Bajo «Abrir imagen…» en la ventana vacía: dónde encontrar todo lo demás.
empty-hint-keys = Pulsa { $cheatsheet } para ver los atajos de teclado, o { $palette } para buscar cualquier comando.
empty-hint-cheatsheet = Pulsa { $cheatsheet } para ver los atajos de teclado.
empty-hint-palette = Pulsa { $palette } para buscar cualquier comando.
# Bajo el indicador de carga cuando un archivo tarda en abrirse.
viewer-opening = Abriendo { $file }…
fullscreen-exit-hint = Pulse Esc para salir de pantalla completa

## Selector de idioma

language-picker-title = Idioma
# machine-drafted, needs native review
language-picker-search = Buscar idiomas
language-completeness = { $percent } traducido
language-system-default = Predeterminado del sistema ({ $language })
language-pseudo-accented = Pseudoidioma: con acentos y más largo
language-pseudo-rtl = Pseudoidioma: reflejado de derecha a izquierda

## Acerca de

about-title = Acerca de { -app-name }
about-version = Versión { $version }
about-tagline = Un visor de imágenes rápido y fiel.
about-copyright = © { $year } Los autores de { -app-name }
about-license = Distribuido bajo la MIT License / Apache License 2.0.
about-geonames = Nombres de lugares de GeoNames (geonames.org), con licencia Creative Commons Attribution 4.0.
about-unicode = Datos regionales del CLDR de Unicode mediante ICU4X. © Unicode, Inc.
# machine-drafted, needs native review
about-lensfun = Perfiles de lente de la base de datos Lensfun (lensfun.github.io), con licencia Creative Commons Attribution-ShareAlike 3.0.
about-third-party = Licencias de terceros
about-copy-info = Copiar información del sistema
about-close = Cerrar

## Interfaz del visor (fase 1)
# machine-drafted, needs native review
app-close = Cerrar
app-truncated-badge = Archivo incompleto
app-info-summary = Resumen
app-info-all-fields = Todos los campos
app-info-copy-value = Copiar valor
app-info-copy-field = Copiar el valor de { $field }
app-info-loading = Leyendo metadatos…
# machine-drafted, needs native review
# Todos los campos, para un archivo sin metadatos aparte de sus píxeles.
app-info-no-fields = Este archivo no tiene campos de metadatos.
# Gain-map row of the info summary (machine-drafted, needs native review)
app-info-gain-map = Mapa de ganancia HDR
app-info-gain-map-headroom = Hasta { $headroom }× el brillo SDR
app-copy-failed = No se pudo copiar: { $reason }
app-restore-failed = No se pudo restaurar { $file }: { $reason }
app-open-dialog-title = Abrir imagen
app-open-dialog-filter = Archivos compatibles
app-open-dialog-all = Todos los archivos
app-open-group-images = Imágenes
app-open-group-raw = RAW de cámara
app-open-group-video = Vídeo
app-open-group-sound = Sonido
app-open-group-documents = Documentos
app-open-group-drawings = Dibujos y CAD
app-open-group-models = Modelos 3D
app-open-group-maps = Mapas y rutas
app-open-group-science = Ciencia y medicina
app-open-group-fonts = Fuentes
app-open-group-other = Otros

## Información del renderizador (Acerca de, Configuración › Avanzado)

# machine-drafted, needs native review
renderer-title = Información del renderizador
renderer-section-adapter = Adaptador gráfico
renderer-section-limits = Límites y funciones
renderer-section-display = Pantalla
renderer-section-memory = Memoria
renderer-section-decoders = Decodificadores opcionales
renderer-unknown = Desconocido
# The PCI vendor and device IDs ("1002:747e").
renderer-ids = ID de fabricante y dispositivo
# Wayland, X11, Windows or macOS.
renderer-display-server = Servidor gráfico
renderer-features = Funciones usadas
renderer-surface-format = Formato de la ventana
renderer-surface-formats = Formatos ofrecidos
renderer-present-mode = Modo de presentación
renderer-present-modes = Modos ofrecidos
renderer-hdr = HDR
# $headroom is how much brighter than white the display goes ("4.0").
renderer-hdr-on = Activado (margen de { $headroom }×)
renderer-hdr-off = Desactivado (la pantalla puede mostrar HDR)
renderer-hdr-unavailable = No disponible en esta pantalla
renderer-decode-budget = La decodificación puede usar
renderer-decode-budget-detail = El { PERCENT(80) } de la memoria libre, compartido por todas las imágenes que se están abriendo.
renderer-display-cache = Caché de visualización

## HUD de rendimiento (F12)
# machine-drafted, needs native review
app-hud-scale = Escala
app-hud-level = Nivel
app-hud-level-thumbnail = Miniatura incrustada
app-hud-level-full = Resolución completa
app-hud-level-scaled = 1/{ $denominator }
app-hud-open = Apertura
app-hud-decode = Decodificación
app-hud-upload = Subida a la GPU
app-hud-frame = Fotograma
app-hud-threads = Hilos
app-hud-threads-value = { $threads } ({ $queued } en cola)
app-hud-tiles = Mosaicos
app-hud-tiles-value = { $tiles } de { $capacity }
app-hud-ram = Memoria
app-hud-vram = Memoria de la GPU
app-hud-gpu = GPU

# Trash confirmation (machine-drafted, needs native review)
app-confirm-trash-title = ¿Mover { $file } a la papelera?
# machine-drafted, needs native review
app-confirm-trash-sidecars = Sus archivos adjuntos también van a la papelera: { $files }
app-cancel = Cancelar

# Stage 1 merge (machine-drafted, needs native review)
disabled-no-image = No hay ninguna imagen abierta
disabled-still-loading = La imagen todavía se está cargando
disabled-decode-failed = No se pudo decodificar esta imagen
disabled-no-other-images = No hay más imágenes en esta carpeta
tooltip-unavailable = { $label } (no disponible: { $reason })
error-decoder-stopped = el decodificador se detuvo sin producir una imagen
# machine-drafted, needs native review
# Shown after "Can’t open FILE:" for brain data (GIFTI .func/.shape/.label, CIFTI) that is
# drawn on a surface: no .surf.gii with that many vertices was found in the same folder.
error-needs-surface = necesita una superficie cerebral (un archivo .surf.gii con { $vertices } vértices) en la misma carpeta
fullscreen-back-hint = Pulsa Esc para restablecer el zoom o salir de la pantalla completa
action-back = Cerrar el panel (en un diálogo, primero retroceder un paso), restablecer el zoom, salir de pantalla completa; en la galería, subir una carpeta
about-third-party-file = { -app-name } incluye software y datos de terceros bajo sus propias licencias; consulta THIRD-PARTY.md, instalado con { -app-name }.
toast-low-memory = Esta imagen puede necesitar unos { $need } de memoria, pero solo hay { $available } disponibles. Puede cargarse despacio o fallar.

hw-report-title = Informe de hardware
hw-report-copy = Copiar informe
hw-report-version = Versión
hw-report-cpu = CPU
hw-report-cores = Núcleos
hw-report-cores-value = { $logical } lógicos, { $physical } físicos
hw-report-performance-cores = Núcleos de rendimiento
hw-report-cpu-features = Aceleración de CPU
hw-report-none = Ninguna
hw-report-memory = Memoria
hw-report-memory-value = { $total } en total, { $available } disponibles
hw-report-graphics-api = API gráfica
hw-report-driver = Controlador
hw-report-max-texture = Textura máxima
hw-report-pixels = { $width } × { $height } px
hw-report-decode-threads = Hilos de decodificación
hw-report-ram-cache = Caché de imágenes en memoria
hw-report-vram-cache = Caché de memoria de la GPU
hw-report-processing = Procesamiento
hw-report-path-jpeg-decode = Decodificación JPEG
hw-report-path-ycbcr-to-rgb = Conversión de color
hw-report-path-png-unfilter = Decodificación PNG
hw-report-path-heic-decode = Decodificación HEIC
hw-report-path-inference = Análisis de imágenes
hw-report-engine-cpu = CPU
hw-report-engine-cpu-parallel = CPU, en paralelo
hw-report-engine-gpu = GPU
hw-report-engine-hw-decoder = decodificador por hardware
hw-report-engine-npu = NPU
# machine-drafted, needs native review
hw-report-build-debug = compilación de depuración
hw-report-build-release = compilación de publicación

## Accessibility (screen readers)
# The image as a screen reader announces it: file name and size in pixels.
a11y-image = { $name }, { $width } × { $height } píxeles
# The region where short notifications (toasts) appear.
a11y-notifications = Notificaciones
# machine-drafted, needs native review
a11y-transport-playback = Reproducción
a11y-transport-pages = Páginas
a11y-settings-sections = Secciones

## Read codes (QR codes in the picture) (machine-drafted, needs native review)
codes-placeholder = Códigos en esta imagen
codes-looking = Buscando códigos…
codes-none = No se encontraron códigos en esta imagen.
codes-failed = No se pudo leer la imagen: { $reason }
codes-copy = Copiar «{ $text }»
# $kind is "QR", $version a number.
codes-kind = { $kind } { $version }
# Data Matrix: its size in modules, e.g. "Data Matrix 12×12".
# machine-drafted, needs native review
codes-kind-size = { $kind } { $rows }×{ $cols }
codes-open-row = Abrir enlace en { $host }…
codes-network-off = No hay ninguna red activada, así que los enlaces no se abren.
codes-open-title = ¿Abrir un enlace a { $host }?
codes-open-detail = Se abre en el navegador web. Abra solo enlaces de confianza.
codes-open = Abrir en el navegador
# machine-drafted, needs native review
# Find text: the text in the picture shown, read by the Local AI pack's Text models.
ocr-placeholder = Texto en esta imagen
# machine-drafted, needs native review
ocr-looking = Leyendo el texto…
# machine-drafted, needs native review
ocr-none = No se encontró texto en esta imagen.
# machine-drafted, needs native review
ocr-failed = No se pudo leer el texto: { $reason }
# machine-drafted, needs native review
ocr-needs-pack = Buscar texto necesita los modelos de texto del paquete de IA local (Configuración › IA local).
# machine-drafted, needs native review
ocr-loading = Cargando los modelos de texto…
# machine-drafted, needs native review
# A row: choosing it copies the line. $text is the line (one line, shortened).
ocr-copy = Copiar «{ $text }»
# machine-drafted, needs native review
ocr-copy-all = Copiar todo el texto
# machine-drafted, needs native review
# A row's note: how sure the reading is, and the add-on that read it. $percent is formatted.
ocr-confidence = Confianza: { $percent }

## Hardware report panel (machine-drafted, needs native review)
hw-report-section-system = Sistema
hw-report-section-gpu = GPU
hw-report-section-decoding = Decodificación
hw-report-operating-system = Sistema operativo
hw-report-adapter = Adaptador
hw-report-gpu-type = Tipo
hw-report-gpu-init = Tiempo de inicio
hw-report-max-layers = Capas de textura
hw-report-compression = Compresión de texturas
hw-report-unified-memory = Memoria compartida con la CPU
hw-report-display = Pantalla
hw-report-hw-decode = Decodificación por hardware
hw-report-core-cluster = Grupo de núcleos
hw-report-power = Alimentación
hw-report-calibration = Mediciones de velocidad
hw-report-codec-others = Otros
hw-report-performance-cores-value = { $count } de { $total }
hw-report-cluster-value = { $name }: { $physical } físicos, { $logical } lógicos
hw-report-thermal-nominal = Temperatura normal
hw-report-thermal-fair = Algo caliente
hw-report-thermal-serious = Caliente, rendimiento reducido
hw-report-thermal-critical = Sobrecalentado
hw-report-power-battery = Con batería ({ $percent })
hw-report-power-battery-unknown = Con batería
hw-report-power-mains = Conectado a la corriente
hw-report-power-mains-percent = Conectado a la corriente ({ $percent })
hw-report-power-saving = Ahorro de energía activado
hw-report-gpu-discrete = Dedicado
hw-report-gpu-integrated = Integrado
hw-report-gpu-virtual = Virtual
hw-report-gpu-software = Software, en la CPU
hw-report-gpu-other = Otro
hw-report-gpu-unavailable = No disponible
hw-report-ms = { $ms } ms
hw-report-yes = Sí
hw-report-no = No
hw-report-unified-platform-differs = El sistema indica lo contrario
hw-report-display-value = { $fps } Hz, margen HDR { $now } ahora, hasta { $max }
hw-report-hw-decode-value = { $api } en { $device }
hw-report-hw-decode-checking = Aún comprobando…
hw-report-codec-up-to = Hasta { $width } × { $height }
hw-report-codec-none = No disponible
hw-report-codec-h264-encode = Codificación H.264
hw-report-supported = compatible: { $list }
hw-report-unsupported = no compatible: { $list }
hw-report-calibration-done = Medido en este equipo
hw-report-calibration-pending = Aún sin medir
hw-report-paths-power-saving = Ahorro de energía: se prefieren las vías que consumen menos
hw-report-paths-caption = Vías para una imagen de { $mpx } megapíxeles
hw-report-col-task = Tarea
hw-report-col-path = Vía
hw-report-col-time = Tiempo
hw-report-col-cpu = Tiempo de CPU (todos los hilos)
hw-report-col-runs = Mediciones
hw-report-col-use = Se usa para
hw-report-runs-untested = Sin probar
hw-report-use-viewing = Visualización ({ $reason })
hw-report-use-background = Segundo plano ({ $reason })
hw-report-reason-only = la única opción
hw-report-reason-faster = la más rápida
hw-report-reason-cheaper = menos tiempo de CPU
hw-report-reason-lower-power = menos energía
hw-report-reason-explore = en prueba
hw-report-reason-forced = fijado en Configuración
hw-report-reason-prefer-gpu = Preferir la GPU
hw-report-reason-cpu-only = Solo la CPU
hw-report-reason-calibrate = midiéndose
hw-report-reason-scaled = tamaño reducido
hw-report-trust-unverified = Aún sin comprobar
hw-report-trust-blocked = Desactivado tras un error
hw-report-trust-absent = No está en este equipo

## Keyboard cheat sheet tabs and sections (machine-drafted, needs native review)
cheatsheet-mode-gallery = Galería
cheatsheet-mode-compare = Comparar
cheatsheet-gallery-browse = Explorar
cheatsheet-gallery-select = Seleccionar
cheatsheet-gallery-mark = Valorar y marcar
cheatsheet-gallery-move = Moverse entre imágenes
cheatsheet-gallery-page = Página arriba o abajo
cheatsheet-gallery-ends = Primera o última imagen
cheatsheet-gallery-up = Subir a la carpeta que la contiene
cheatsheet-gallery-history-back = Volver a la carpeta anterior
cheatsheet-gallery-history-forward = Avanzar a la carpeta siguiente
cheatsheet-gallery-back = Cerrar la vista superior y luego volver
cheatsheet-gallery-viewer = Volver al visor
cheatsheet-gallery-album-add = Añadir al último álbum
cheatsheet-gallery-select-one = Seleccionar o deseleccionar
cheatsheet-gallery-quick-look = Vista rápida
cheatsheet-gallery-filter = Filtrar
cheatsheet-gallery-bigger = Miniaturas más grandes
cheatsheet-gallery-smaller = Miniaturas más pequeñas
cheatsheet-gallery-default-size = Tamaño estándar de las miniaturas
cheatsheet-gallery-refresh = Actualizar la carpeta
cheatsheet-gallery-menu = Menú contextual
cheatsheet-compare-views = Comparar
cheatsheet-compare-overlay = Superposición o en paralelo
cheatsheet-compare-difference = Diferencia (otra vez: siguiente tipo)
cheatsheet-compare-step-down = Menos fundido o ganancia de la diferencia
cheatsheet-compare-step-up = Más fundido o ganancia de la diferencia
cheatsheet-compare-pick = Imagen sobre la que actúan las teclas
cheatsheet-compare-close = Cerrar la comparación
cheatsheet-channels = Canales
cheatsheet-medical-window = Ventana y nivel
cheatsheet-medical-measure = Medir y anotar
cheatsheet-medical-frames = Fotogramas y series
# machine-drafted, needs native review
cheatsheet-medical-3d = 3D y reconstrucción
cheatsheet-medical-analysis = Segmentación, fusión y PET
# machine-drafted, needs native review
cheatsheet-medical-studies = Estudios y compartir
cheatsheet-palette-only-title = En la paleta de comandos ({ $key })
cheatsheet-palette-only-title-nokey = En la paleta de comandos
cheatsheet-palette-only-tag = Paleta
cheatsheet-palette-only-line = { $action }: sin tecla; en la paleta de comandos ({ $key })
cheatsheet-palette-only-line-nokey = { $action }: sin tecla; en la paleta de comandos
# Esc en la hoja de atajos (la descripción larga de la paleta no cabe).
cheatsheet-back = Cerrar o volver atrás

## Hardware video encoding (machine-drafted, needs native review)
hw-report-path-video-encode = Exportación de vídeo
hw-report-engine-hw-encoder = codificador por hardware

## Path timing units (machine-drafted, needs native review)
hw-report-task-thumbnail = { $task } (por miniatura de { $size })
hw-report-task-frame = { $task } (por fotograma de { $size })

## Hardware report, software-only formats (machine-drafted, needs native review)
hw-report-codec-software = Solo por software

about-natural-earth = Datos del mapa de Natural Earth (naturalearthdata.com), dominio público.
app-info-show-on-map = Mostrar en el mapa
