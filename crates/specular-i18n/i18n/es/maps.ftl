# machine-drafted, needs native review
# Specular — Maps (the Maps profile): the map view of georeferenced and vector map files, its
# layer panel, attribute table, coordinates, measuring, elevation profile and map furniture.
# English.

keys-mode-map = Mapas
# Cheat sheet groups of the Maps tab.
cheatsheet-map-layers = Capas
cheatsheet-map-coordinates = Coordenadas
cheatsheet-map-measure = Medición

## Palette entries and their one-line descriptions.
palette-map-layers = Mostrar u ocultar el panel de capas
palette-map-layers-help = Las capas del mapa: orden, opacidad, visibilidad y estilo.
palette-map-add-layer = Añadir una capa desde un archivo…
palette-map-add-layer-help = Superponer otro archivo de mapa, como un track GPX sobre un shapefile.
palette-map-zoom-to-layer = Zoom a la capa
palette-map-zoom-to-layer-help = Mostrar entera la capa elegida en el panel de capas.
palette-map-attribute-table = Mostrar u ocultar la tabla de atributos
palette-map-attribute-table-help = Las entidades de la capa elegida como tabla: ordenar, filtrar, buscar y seleccionar.
palette-map-coordinate-format = Siguiente formato de coordenadas
palette-map-coordinate-format-help = Grados decimales, grados minutos segundos, UTM o MGRS.
palette-map-units = Unidades métricas o imperiales
palette-map-units-help = Longitudes y áreas en metros, o en pies y millas.
palette-map-copy-coordinate = Copiar coordenada
palette-map-copy-coordinate-help = Copiar la posición bajo el puntero, o la del centro del mapa.
palette-map-go-to-coordinate = Ir a coordenada…
palette-map-go-to-coordinate-help = Escriba una posición en cualquier formato para centrar el mapa allí.
palette-map-measure-distance = Medir una distancia
palette-map-measure-distance-help = Haga clic en puntos del mapa: la longitud sobre la superficie terrestre y el rumbo.
palette-map-measure-area = Medir un área
palette-map-measure-area-help = Haga clic en las esquinas de un área: su tamaño y perímetro sobre la superficie terrestre.
palette-map-elevation-profile = Perfil de elevación
palette-map-elevation-profile-help = Trace una línea sobre un modelo de elevación para representar sus alturas.
palette-map-overview = Mostrar u ocultar el mapa general
palette-map-overview-help = Un mapa pequeño de toda la extensión, con la parte en pantalla marcada.

## Why a map command is unavailable.
disabled-not-map = Solo para mapas, con las herramientas de mapas activadas
disabled-no-lonlat = El mapa no tiene un sistema de coordenadas conocido
disabled-no-features = El mapa no tiene entidades
disabled-no-heights = El mapa no tiene alturas

## Settings › Maps.
settings-section-maps = Mapas
settings-maps-coordinate-format = Formato de coordenadas
settings-maps-coordinate-format-help = Cómo se muestran y copian las posiciones. Ir a coordenada lee todos los formatos.
settings-maps-coordinate-format-dd = Grados decimales
settings-maps-coordinate-format-dms = Grados, minutos, segundos
settings-maps-coordinate-format-utm = UTM
settings-maps-coordinate-format-mgrs = MGRS / USNG
settings-maps-units = Unidades
settings-maps-units-help = Longitudes y áreas en los mapas: la barra de escala y las mediciones. Automático sigue Metadatos › Unidades.
settings-maps-units-auto = Automático
settings-maps-units-metric = Métrico
settings-maps-units-imperial = Imperial

## The pointer readout. $position is written in the chosen format.
map-position = { $format ->
    [utm] UTM { $position }
    [mgrs] MGRS { $position }
    [dms] { $position }
   *[dd] Lat, lon { $position }
}
map-height = Altura { $height }
map-height-label = Altura
# A length or area with its unit symbol ("12.3 km", "4.5 ha").
map-length = { $value } { $unit }
map-scale-label = Barra de escala: { $length }
map-north-label = Flecha del norte: el norte está a { $degrees }° en sentido horario desde arriba
# The coordinate formats, for the toast after "Next coordinate format".
map-format-name = { $format ->
    [dms] grados, minutos, segundos
    [utm] UTM
    [mgrs] MGRS
   *[dd] grados decimales
}
map-format-toast = Coordenadas en { $format }
map-units-metric = Unidades: metros y kilómetros
map-units-imperial = Unidades: pies y millas
toast-copied-coordinate = Coordenada copiada

## Go to coordinate.
map-goto-hint = Grados decimales, DMS, UTM o MGRS. Intro lleva allí.
map-goto-empty = Escriba una posición.
map-goto-unrecognised = No es una posición en un formato conocido.
map-goto-range = Latitud o longitud fuera de rango.
map-goto-no-way = Este mapa no puede situar una posición.
map-goto-outside = Esa posición está fuera del mapa.
map-goto-toast = Centrado en { $position }

## The layer panel.
map-layers-title = Capas: { $shown } de { $total } visibles
# Under a layer's name: its kind, features and opacity.
map-layer-sub = { $kind } · { $features } · { PERCENT($opacity) }
# A layer's row for screen readers.
map-layer-row = { $name }: { $kind }, { $features ->
    [one] { $features } entidad
    [many] { $features } de entidades
   *[other] { $features } entidades
}, opacidad { PERCENT($opacity) }
map-kind-area = Áreas
map-kind-line = Líneas
map-kind-point = Puntos
map-kind-empty = Sin entidades
map-layer-visible = Mostrar u ocultar la capa
map-layer-opacity-down = Menos opaca
map-layer-opacity-up = Más opaca
map-layer-colour = Siguiente color
map-layer-width = Siguiente grosor de línea
map-layer-point = Siguiente tamaño de punto
map-layer-up = Subir
map-layer-down = Bajar
map-layer-remove = Quitar
map-layer-keep = Conservar
map-layer-remove-ask = ¿Quitar { $name } del mapa? El archivo se queda en el disco.
map-layer-adding = Añadiendo una capa…
map-removed = { $name } quitado del mapa
map-added = { $layers ->
    [one] { $name } añadido
    [many] { $name } añadido: { $layers } de capas
   *[other] { $name } añadido: { $layers } capas
}
map-add-failed = No se puede añadir { $name }: { $reason }
map-add-filter = Archivos de mapa
map-add-not-map = no es un mapa.
map-add-not-vector = no tiene entidades.
map-add-no-crs = no tiene archivo de proyección (.prj).

## Identify.
map-identify-more = y { $count } más en la tabla de atributos
map-cell = Celda { $x }, { $y }
map-band = Banda { $n }
map-no-data = Sin datos

## The attribute table.
map-table-title = { $layer }: { $shown } de { $total } entidades, { $selected } seleccionadas
map-table-search = Buscar, o campo = valor
map-table-selected-only = Solo seleccionadas
map-table-empty = Ninguna entidad coincide.
map-table-many = muchas
map-table-stats-number = { $field }: { $count ->
        [one] { $count } valor
       *[other] { $count } valores
    }, { $empty } vacíos · mín. { $min } · máx. { $max } · media { $mean } · suma { $sum } · desv. típ. { $sd }
map-table-stats-text = { $field }: { $count ->
        [one] { $count } valor
       *[other] { $count } valores
    }, { $empty } vacíos · { $distinct } distintos
map-table-stats-text-top = { $field }: { $count ->
        [one] { $count } valor
       *[other] { $count } valores
    }, { $empty } vacíos · { $distinct } distintos · más frecuente { $top } ({ $times }×)

## Measuring and the elevation profile.
map-measure-hint-distance = Haga clic en puntos a lo largo del camino.
map-measure-hint-area = Haga clic en las esquinas del área.
map-measure-hint-profile = Haga clic en puntos a lo largo de la línea.
map-measure-keys = Intro termina. Retroceso quita el último punto. Esc cierra.
map-measure-distance = Distancia { $length }
map-measure-bearing = Rumbo { $bearing }
map-measure-segment = Último tramo { $length }, rumbo { $bearing }
map-measure-area = Área { $area }
map-measure-perimeter = Perímetro { $length }
map-bearing-value = { $degrees }°
map-profile-summary = Longitud { $length } · mínima { $min } · máxima { $max } · subida { $climb } · bajada { $descent }
map-profile-no-heights = Longitud { $length }. Sin alturas a lo largo de la línea.
map-profile-at = { $distance }: { $height }
map-profile-at-none = { $distance }: sin altura
