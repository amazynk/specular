# machine-drafted, needs native review
# Specular — la galería (modo de miniaturas, DESIGN §10) y Configuración → Galería. Español.

## Paleta y menús

palette-gallery = Galería
gallery-palette-view = Vista de la galería…
gallery-palette-layout = Diseño
gallery-palette-sort = Ordenar por
gallery-palette-group = Agrupar por
gallery-palette-filter = Filtrar…
gallery-palette-quick-filters = Filtros rápidos
gallery-palette-details = Vista de detalles
gallery-palette-info-overlay = Información en las miniaturas
gallery-palette-stacks = Apilar RAW+JPEG y ráfagas
gallery-palette-subfolders = Incluir subcarpetas
gallery-palette-compare = Comparar selección
gallery-palette-rate = Valorar…
gallery-palette-settings = Configuración de la galería…
gallery-view-menu = Ver
gallery-more-menu = Más
gallery-open = Abrir
gallery-new-window = Abrir en una ventana nueva
gallery-reveal = Mostrar en la carpeta
gallery-properties = Propiedades
gallery-copy-paths = Copiar rutas
gallery-select-all = Seleccionar todo
gallery-select-none = No seleccionar nada
gallery-select-invert = Invertir selección
gallery-next-unrated = Siguiente sin valorar
gallery-stack-toggle = Expandir o contraer la pila
gallery-run-on = { $name } ({ $count } archivos)
gallery-depth-off = Solo esta carpeta
gallery-depth-all = Todas las subcarpetas
gallery-depth-levels = { $depth ->
    [one] Un nivel de subcarpetas
    [many] { $depth } niveles de subcarpetas
   *[other] { $depth } niveles de subcarpetas
}

## Estado y vistas vacías

gallery-items = { $count ->
    [one] { $count } elemento
    [many] { $count } de elementos
   *[other] { $count } elementos
}
gallery-selected = { $count } de { $total } seleccionados
gallery-shown-of = { $count } de { $total } mostrados
gallery-listing = Leyendo la carpeta… { $count }
gallery-empty = Esta carpeta no contiene imágenes.
gallery-empty-filter = Ninguna imagen coincide con el filtro.
gallery-list-error = No se puede leer esta carpeta: { $reason }
gallery-no-folder = No hay ninguna carpeta abierta
gallery-kind-folder = Carpeta
gallery-drag-count = { $count ->
    [one] { $count } archivo
    [many] { $count } de archivos
   *[other] { $count } archivos
}
gallery-badge-pages = { $count } páginas

## Barra de filtro

# machine-drafted, needs native review
# The filter field takes words (matched against names) and field:value terms; the field
# names and words of the query (rating, camera, ext, is:pick, …) stay in English.
gallery-filter-query-placeholder = Nombre, o rating>=3 camera:X-T5 …
gallery-filter-clear = Borrar filtro
gallery-filter-rating = { $stars ->
    [one] ★ 1 o más
    [many] ★ { $stars } o más
   *[other] ★ { $stars } o más
}
gallery-filter-label = Etiqueta: { $label }
gallery-filter-picks = Seleccionadas
gallery-filter-hide-rejects = Ocultar rechazadas
gallery-filter-unrated = Sin valorar
gallery-filter-raw = RAW
gallery-filter-animated = Animadas
gallery-filter-hdr = HDR
gallery-filter-landscape = Horizontal
gallery-filter-portrait = Vertical

## Query errors and facets

gallery-query-unknown-field = Campo desconocido «{ $field }»
gallery-query-unknown-field-suggest = Campo desconocido «{ $field }»: ¿quería decir «{ $suggestion }»?
gallery-query-missing-value = «{ $field }» necesita un valor
gallery-query-bad-number = «{ $value }» no es un número ({ $field })
gallery-query-bad-date = «{ $value }» no es una fecha ({ $field }): use 2024, 2024-05 o 2024-05-12
gallery-query-bad-word = «{ $value }» no vale para { $field }: use { $words }
# machine-drafted, needs native review
gallery-query-bad-exif-tag = «{ $value }» no es un nombre de etiqueta Exif
gallery-query-no-compare = { $field } no se puede comparar con < o > aquí
gallery-query-unterminated = Falta una comilla de cierre
gallery-query-unavailable = «{ $field }» necesita la Biblioteca (no disponible aquí)
gallery-facets = Facetas
gallery-facets-counting = Contando…
gallery-facets-empty = Nada que contar
gallery-facets-search = Buscar en facetas
gallery-facets-no-match = Ningún valor coincide
# The gallery's top bar button (its tooltip): back to the start screen.
gallery-home = Pantalla de inicio
gallery-facet-format = Formato
gallery-facet-camera = Cámara
gallery-facet-lens = Objetivo
gallery-facet-rating = Valoración
gallery-facet-year = Año
gallery-facet-orientation = Orientación
gallery-facet-label = Etiqueta
gallery-facet-properties = Propiedades
gallery-facet-square = Cuadrada
gallery-facet-alpha = Transparencia
gallery-facet-gps = Ubicación
gallery-facet-multipage = Varias páginas
# Screen readers: a facet value, e.g. “Camera: X-T5, 12 files”.
gallery-facet-a11y-value = { $facet }: { $value }, { $count ->
    [one] { $count } archivo
   *[other] { $count } archivos
}

## Grupos

# An event group's header: the first and the last day of a shooting session.
gallery-group-event = { $start } – { $end }
gallery-group-event-place = { $place } · { $dates }
gallery-group-this-folder = Esta carpeta
gallery-group-unknown = Desconocido
gallery-group-unrated = Sin valorar

## Selección de fotos

gallery-stars = { $stars ->
    [one] 1 estrella
    [many] { $stars } de estrellas
   *[other] { $stars } estrellas
}
gallery-rate-none = Sin valoración
gallery-pick = Seleccionar
gallery-reject = Rechazar
gallery-unflag = Sin marca
gallery-label-red = Rojo
gallery-label-yellow = Amarillo
gallery-label-green = Verde
gallery-label-blue = Azul
gallery-label-none = Sin etiqueta
gallery-compare-hint = Seleccione de 2 a 4 imágenes para compararlas.

## Archivos

gallery-trash-count = { $count ->
    [one] Mover { $count } archivo a la papelera
    [many] Mover { $count } de archivos a la papelera
   *[other] Mover { $count } archivos a la papelera
}
gallery-moved-to = { $count ->
    [one] Se ha movido { $count } archivo a { $folder }
    [many] Se han movido { $count } de archivos a { $folder }
   *[other] Se han movido { $count } archivos a { $folder }
}
gallery-copied-to = { $count ->
    [one] Se ha copiado { $count } archivo a { $folder }
    [many] Se han copiado { $count } de archivos a { $folder }
   *[other] Se han copiado { $count } archivos a { $folder }
}
gallery-move-failed = No se pudo mover { $file }: { $reason }
gallery-drop-move = Mover a { $folder }
gallery-drop-copy = Copiar a { $folder }
gallery-move-here = Mover la selección aquí
gallery-copy-here = Copiar la selección aquí
gallery-not-an-image = { $file } no es una imagen que { -app-name } pueda abrir.
gallery-cache-cleared = Se ha vaciado la caché de miniaturas
# machine-drafted, needs native review
gallery-thumb-failed = Sin miniatura: { $reason }
gallery-cache-clear-failed = No se pudo vaciar la caché de miniaturas: { $reason }

## Propiedades

gallery-prop-name = Nombre
gallery-prop-folder = Carpeta
gallery-prop-size = Tamaño del archivo
gallery-prop-dimensions = Dimensiones
gallery-prop-format = Formato
gallery-prop-date = Fecha
gallery-prop-camera = Cámara
gallery-prop-lens = Objetivo
gallery-prop-rating = Valoración
gallery-prop-label = Etiqueta
gallery-prop-items = Elementos

## Lectores de pantalla

gallery-a11y-list = { $folder }, { $count ->
    [one] { $count } elemento
    [many] { $count } de elementos
   *[other] { $count } elementos
}
gallery-a11y-dims = { $width } por { $height }
gallery-a11y-folder = carpeta
gallery-a11y-up = Subiste a { $folder }
gallery-a11y-archive = archivo comprimido
gallery-a11y-stack = pila de { $count }
gallery-a11y-filter-bar = Filtro
gallery-a11y-filter-field = Filtrar por nombre o campo
gallery-a11y-chip = Criterio de filtro { $term }
gallery-a11y-chip-remove = Pulse para quitar
gallery-a11y-results = { $count ->
    [0] Ningún elemento coincide
    [one] { $count } elemento coincide
    [many] { $count } de elementos coinciden
   *[other] { $count } elementos coinciden
}

## Configuración → Galería

settings-section-gallery = Galería
settings-gallery-layout = Diseño
settings-gallery-layout-help = Cuadrados recorta las miniaturas a una cuadrícula; filas justificadas mantiene la forma de cada imagen; sin recortar encaja las imágenes completas en celdas iguales.
settings-gallery-layout-squares = Cuadrados
settings-gallery-layout-justified = Filas justificadas
settings-gallery-layout-uncropped = Sin recortar
settings-gallery-layout-details = Detalles
settings-gallery-thumbnail-size = Tamaño de las miniaturas
settings-gallery-thumbnail-size-help = También con Ctrl+rueda del ratón, con el gesto de pellizcar o con Ctrl+más y menos en la galería.
settings-gallery-captions = Mostrar nombres de archivo
settings-gallery-captions-help = Un rótulo debajo de cada miniatura.
settings-gallery-badges = Mostrar distintivos
settings-gallery-badges-help = Pequeñas marcas para RAW, animada, HDR, varias páginas, valoración y etiqueta.
settings-gallery-sort-order = Ordenar por
settings-gallery-sort-order-help = El orden de la galería. Las carpetas siempre van primero.
settings-gallery-sort-order-name = Nombre
settings-gallery-sort-order-date-taken = Fecha de captura
settings-gallery-sort-order-modified = Fecha de modificación
settings-gallery-sort-order-size = Tamaño del archivo
settings-gallery-sort-order-type = Tipo de archivo
settings-gallery-sort-order-dimensions = Dimensiones
settings-gallery-sort-descending = Orden inverso
settings-gallery-sort-descending-help = Ordenar del último al primero.
settings-gallery-group-by = Agrupar por
settings-gallery-group-by-help = Encabezados de sección que permanecen arriba al desplazarse.
settings-gallery-group-by-none = Nada
settings-gallery-group-by-day = Día
settings-gallery-group-by-month = Mes
settings-gallery-group-by-year = Año
settings-gallery-group-by-event = Evento
settings-gallery-group-by-folder = Carpeta
settings-gallery-group-by-camera = Cámara
settings-gallery-group-by-format = Formato
settings-gallery-group-by-rating = Valoración
settings-gallery-group-by-target = Objeto
settings-gallery-group-by-filter = Filtro
settings-gallery-per-folder-view = Recordar la vista de cada carpeta
settings-gallery-per-folder-view-help = El diseño, el tamaño, el orden y la agrupación se guardan por carpeta; de lo contrario, se usa una sola vista en todas partes.
settings-gallery-open-folders-in-gallery = Abrir carpetas en la galería
settings-gallery-open-folders-in-gallery-help = Al abrir una carpeta, empezar por sus miniaturas en lugar de por la primera imagen.
settings-gallery-escape-opens-gallery = Esc abre la galería
settings-gallery-escape-opens-gallery-help = Cuando no hay nada más que cerrar, Esc en el visor pasa a la galería en lugar de no hacer nada.
settings-gallery-wrap-around = Las flechas dan la vuelta
settings-gallery-wrap-around-help = Al final de la lista, las flechas pasan al principio, y del principio al final. Si mantienes pulsada una tecla, se sigue deteniendo al final.
settings-gallery-show-hidden = Mostrar archivos ocultos
settings-gallery-show-hidden-help = Incluir archivos y carpetas cuyo nombre empieza por un punto.
settings-gallery-show-other-files = Mostrar otros archivos
settings-gallery-show-other-files-help = Incluir archivos que { -app-name } no puede abrir, mostrados con un icono genérico.
settings-gallery-show-archives = Mostrar archivos comprimidos
settings-gallery-show-archives-help = Incluir cómics y otros archivos comprimidos (zip, cbz, rar, 7z…) como miniaturas. Abrir uno directamente sigue funcionando.
settings-gallery-subfolder-depth = Niveles de subcarpetas
settings-gallery-subfolder-depth-help = Mostrar también las imágenes de las subcarpetas, hasta este número de niveles (0: solo esta carpeta).
settings-gallery-stacks = Apilar archivos relacionados
settings-gallery-stacks-help = Mostrar los pares RAW+JPEG y las ráfagas como una sola miniatura. Pulse S para expandir una pila.
settings-gallery-info-overlay = Información en las miniaturas
settings-gallery-info-overlay-help = Dimensiones y exposición sobre cada miniatura.
settings-gallery-prefer-embedded = Preferir vistas previas incrustadas
settings-gallery-prefer-embedded-help = Usar la vista previa guardada en el archivo cuando sea lo bastante grande. Mucho más rápido; un poco menos exacto.
settings-gallery-sharper-raw-thumbnails = Miniaturas RAW más nítidas
settings-gallery-sharper-raw-thumbnails-help = Las fotos RAW editadas se revelan a 1024 px para sus miniaturas. Más nítidas, algo más lentas. Desactivado: 512 px.
settings-gallery-rating-sidecars = Guardar valoraciones en archivos XMP adjuntos
settings-gallery-rating-sidecars-help = Las valoraciones, etiquetas y marcas también se guardan en un archivo .xmp junto a la imagen, que otras aplicaciones de fotografía leen.
settings-gallery-cache-size = Tamaño de la caché de miniaturas
settings-gallery-cache-size-help = Las miniaturas más antiguas se eliminan cuando la caché está llena.
settings-gallery-cache-location = Ubicación de la caché de miniaturas
settings-gallery-cache-location-help = Lo ideal es una carpeta en una unidad rápida.
settings-gallery-clear-cache = Vaciar la caché de miniaturas

## Compare
# machine-drafted, needs native review

compare-side = En paralelo
compare-overlay = Superposición
compare-difference = Diferencia
compare-flicker = Parpadeo
compare-blend = Fundido
compare-kind-absolute = Error absoluto
compare-kind-squared = Error cuadrático
compare-kind-relative = Error relativo
compare-scale-down = Menos sensible
compare-scale-up = Más sensible
compare-scale = Escala completa { $value }
compare-space = Luz lineal, tal como se muestra
compare-space-help = Los errores y las métricas se miden en luz lineal tal como se muestra la imagen, tras el mapeo de tonos, por canal incluido el alfa.
compare-metrics = MAE { $mae }  ·  MSE { $mse }  ·  PSNR { $psnr } dB
compare-sampled = muestreado a { $width } × { $height }
compare-aligned = Alineado arriba a la izquierda, { $width } × { $height } comparados
compare-vs = { $b } frente a { $a }
compare-overlay-label = { $b } sobre { $a } al { PERCENT($percent) }
compare-pending = Midiendo…
# machine-drafted, needs native review
compare-diff-failed = No se puede mostrar la diferencia: { $reason }
compare-no-overlap = Nada que comparar en la vista
compare-zoom-fit = Ajustar
compare-zoom = { PERCENT($percent) }
compare-showing = Mostrando { $name }
compare-a11y-group = Comparar
compare-a11y-blend = Fundido entre las imágenes
compare-a11y-zoom = Zoom
gallery-palette-compare-side = Comparar en paralelo
gallery-palette-compare-overlay = Comparar como superposición
gallery-palette-compare-difference = Comparar diferencia: { $kind }
gallery-palette-compare-flicker = Alternar entre las imágenes comparadas
gallery-palette-compare-actual = Comparar al { PERCENT(100) }

## Search value list (machine-drafted, needs native review)
gallery-picker-hint = Espacio o Ctrl+Intro selecciona varios

## Search value picker, part 2 (machine-drafted, needs native review)
gallery-facet-profile = Perfil de color
gallery-facet-date = Fecha de captura
gallery-facet-picked = Seleccionadas
gallery-facet-rejected = Rechazadas
gallery-facet-flagged = Seleccionadas o rechazadas
gallery-facet-rated = Valoradas
gallery-facet-labelled = Con etiqueta

## Search value picker, part 3 (machine-drafted, needs native review)

gallery-facet-bits = Profundidad de bits
gallery-facet-pages = Páginas
gallery-facet-width = Ancho
gallery-facet-height = Alto
gallery-facet-megapixels = Megapíxeles
gallery-picker-bits = { $bits } bits
gallery-picker-pages = { $pages ->
    [one] { $pages } página
   *[other] { $pages } páginas
}
gallery-picker-pixels = { $px } px
gallery-picker-megapixels = { $mp } MP
gallery-picker-group-any = Cualquiera de un grupo
gallery-picker-group-keep = Sugerencia: conservar
gallery-picker-group-discard = Sugerencia: descartar
gallery-picker-ops = Comparar
gallery-picker-op-eq = Igual a
gallery-picker-op-ge = Al menos
gallery-picker-op-le = Como máximo
gallery-picker-ge = al menos { $value }
gallery-picker-gt = más de { $value }
gallery-picker-le = como máximo { $value }
gallery-picker-lt = menos de { $value }

# Map view.
gallery-facet-place = Lugar
gallery-palette-map = Mapa
map-title = Mapa
map-summary = { $count ->
    [one] { $count } foto con ubicación
   *[other] { $count } fotos con ubicación
}
map-photos = { $count ->
    [one] { $count } foto
   *[other] { $count } fotos
}
map-zoom-in = Acercar
map-zoom-out = Alejar
map-show-all = Mostrar todo
map-done = Listo
map-credit = Datos del mapa: Natural Earth
# machine-drafted, needs native review
# With a base map's tiles on screen: $credit is the tile source's credit ("© OpenStreetMap
# contributors").
map-credit-tiles = Mapa: { $credit } · Natural Earth
# The one-time explanation before Specular downloads map tiles from a server; one idea per
# line. $host is the server's name, $size the cache size ("256 MB").
map-basemap-consent-title = Mosaicos de mapa en línea
map-basemap-consent-server = Los mosaicos del mapa vienen de { $host }.
map-basemap-consent-sees = Ese servidor ve su dirección de internet y las zonas que consulta.
map-basemap-consent-private = Sus fotos, sus nombres y sus ubicaciones no se envían.
map-basemap-consent-cache = Los mosaicos se guardan en este equipo, hasta { $size }.
map-basemap-allow = Usar mapa en línea
map-basemap-decline = Mantener mapa sin conexión
# Why the base map is not shown; $reason is one of the lines below or a file error.
map-basemap-error = Mapa base no mostrado: { $reason }
map-basemap-bad-address = la dirección del servidor de mosaicos necesita {"{"}z{"}"}, {"{"}x{"}"} y {"{"}y{"}"}.
map-basemap-unknown-tiles = los mosaicos del archivo de mapa son de un tipo desconocido.
map-loading = Leyendo las ubicaciones de las fotos…
map-empty = Ninguna de estas fotos tiene ubicación
map-hidden = Las ubicaciones de las fotos están ocultas (Configuración › Metadatos y privacidad)
map-a11y-group = Mapa de ubicaciones de fotos
map-a11y-status = { $count ->
    [one] { $count } lugar mostrado, nivel de zoom { $zoom }
   *[other] { $count } lugares mostrados, nivel de zoom { $zoom }
}
