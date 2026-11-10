# machine-drafted, needs native review
# Specular — Maps tools: terrain (slope, aspect, contours, 3-D), time, wind and weather colours,
# online map services, photos and geotagging along GPS tracks, and map export.

## Cheat sheet groups of the Maps tab.
cheatsheet-map-terrain = Relieve
cheatsheet-map-time = Tiempo y meteorología
cheatsheet-map-online = Servicios en línea
cheatsheet-map-photos = Fotos y GPS
cheatsheet-map-output = Salida

## Palette entries and their one-line descriptions.
palette-map-slope = Colores de pendiente
palette-map-slope-help = Colorear un modelo de elevación según lo empinado que es.
palette-map-aspect = Colores de orientación
palette-map-aspect-help = Colorear un modelo de elevación según la dirección hacia la que miran sus laderas.
palette-map-contours = Mostrar u ocultar las curvas de nivel
palette-map-contours-help = Curvas de nivel sobre un modelo de elevación, rotuladas cada pocas curvas.
palette-map-terrain-3d = Relieve en 3D
palette-map-terrain-3d-help = Mostrar el modelo de elevación como una superficie 3D con sus colores encima.
palette-map-time = Mostrar u ocultar el control de tiempo
palette-map-time-help = Recorrer los tiempos de una cuadrícula o los de un track.
palette-map-play = Reproducir o pausar el tiempo
palette-map-play-help = Hacer avanzar el control de tiempo solo, paso a paso.
palette-map-wind = Mostrar u ocultar las flechas de viento
palette-map-wind-help = Flechas de velocidad y dirección del viento a partir de las componentes u y v de la cuadrícula.
palette-map-weather-colours = Siguiente mapa de color meteorológico
palette-map-weather-colours-help = Mapas de color para temperatura, lluvia, viento y presión.
palette-map-services = Servicios de mapas en línea…
palette-map-services-help = Añadir capas de servidores WMS, WMTS, WFS, OGC API o STAC.
palette-map-photos-on-track = Mostrar u ocultar las fotos en el track
palette-map-photos-on-track-help = Situar las fotos de la carpeta a lo largo del track GPS según la hora en que se tomaron.
palette-map-geotag = Geoetiquetar fotos con el track…
palette-map-geotag-help = Escribir las posiciones del track en archivos XMP adjuntos de las fotos de la carpeta.
palette-map-export = Exportar mapa…
palette-map-export-help = Guardar el mapa como PNG o PDF a una escala, con leyenda, barra de escala y flecha del norte.
palette-map-print = Imprimir mapa a escala…
palette-map-print-help = Imprimir el mapa a una escala elegida, como 1:{ NUMBER(25000) }.
palette-map-copy-extent = Copiar extensión
palette-map-copy-extent-help = Copiar la zona en pantalla como rectángulo envolvente, o como WKT.

## Why a map command is unavailable.
disabled-no-time = El mapa no tiene tiempos
disabled-no-wind = El mapa no tiene componentes de viento (u y v)
disabled-no-track = No hay track GPS con horas
disabled-not-grid = El mapa no es una cuadrícula de valores

## Settings › Maps and Metadata.
settings-maps-services = Servicios de mapas en línea
settings-maps-services-help = Añadir capas de servidores WMS, WMTS, WFS, OGC API y STAC. Desactivado: no se descarga nada.
settings-maps-services-allowed = Servidores de mapas permitidos
settings-maps-services-allowed-help = Servidores que aceptó tras la explicación de lo que ven.
settings-metadata-basemap-service = Una capa de un servicio de mapas
settings-metadata-basemap-service-help = La capa elegida en el panel de servicios en línea del mapa.

## Terrain.
# A compass point, short: n, ne, e, se, s, sw, w, nw.
map-compass = { $point ->
    [ne] NE
    [e] E
    [se] SE
    [s] S
    [sw] SO
    [w] O
    [nw] NO
   *[n] N
}
map-slope-aspect = Pendiente { $slope }° · orientada al { $direction } ({ $aspect }°)
map-slope-flat = Pendiente { $slope }° · llano
map-slope-on = Colores de pendiente
map-aspect-on = Colores de orientación
map-relief-off = Colores de altura
map-contours-on = Curvas de nivel activadas
map-contours-off = Curvas de nivel desactivadas
map-legend-slope = Pendiente
map-legend-aspect = Orientación: hacia dónde miran las laderas
map-legend-degrees = { $degrees }°
map-legend-degrees-more = { $degrees }° o más
map-terrain-3d-on = Relieve en 3D. Esc vuelve al mapa.
map-terrain-3d-on-exaggerated = Relieve en 3D, alturas × { $times }. Esc vuelve al mapa.
map-terrain-exaggeration = Alturas exageradas { $times } veces.
map-terrain-drape = Colores del mapa

## Time, weather and wind.
map-time-grid = { $when } · { $step } de { $steps }
map-time-track = { $when } · a los { $elapsed }
map-time-speed = × { $times }
map-time-play = Reproducir
map-time-pause = Pausa
map-weather-map = Colores: { $name }
map-weather-file = Colores del archivo
map-wind-on = Flechas de viento activadas
map-wind-off = Flechas de viento desactivadas
map-wind-at = Viento { $speed } { $unit } del { $degrees }° ({ $direction })

## Copy extent.
map-extent-copied = Rectángulo envolvente copiado. Copie de nuevo para WKT.
map-extent-copied-wkt = Extensión copiada como WKT

## Online map services.
map-services-title = Servicios de mapas en línea
map-service-off = Los servicios de mapas en línea están desactivados.
map-service-off-why = Actívelos para añadir capas de servidores de internet.
map-service-turn-on = Activar
map-service-close = Cerrar
map-service-back = Atrás
map-service-cancel = Cancelar
map-service-connect = Conectar
map-service-allow = Permitir
map-service-remove = Quitar
map-service-download = Descargar
map-service-list-hint = Intro abre un servidor. Supr lo quita.
map-service-add = Añadir un servidor…
map-service-add-sub = WMS, WMTS, WFS, OGC API o STAC
map-service-add-hint = La dirección del servidor. Intro conecta.
map-service-bad-address = No es una dirección web (http o https, sin nombre de usuario).
map-service-server-sub = { $kind } · { $host }
map-service-consent-title = Servicio de mapas en línea
map-service-consent-server = Specular contactará con { $host }.
map-service-consent-sees = Ese servidor ve su dirección de internet y la zona del mapa que pide.
map-service-consent-private = Sus archivos y sus nombres no se envían.
map-service-asking = Consultando { $host }…
map-service-downloading = Descargando de { $host }…
map-service-layers-hint = Capas { $kind }. Intro usa una.
map-service-no-layers = El servidor no lista ninguna capa.
map-service-offer-tiles = Mosaicos: pasa a ser el mapa base del mapa de fotos
map-service-offer-features = Entidades: se añaden a este mapa para la zona en pantalla
map-service-offer-search = Imágenes: buscar en la zona en pantalla
map-service-refused-mercator = No se ofrece en Web Mercator (EPSG:3857), que necesita el mapa base.
map-service-refused-format = No se ofrece como imágenes PNG o JPEG.
map-service-refused-geojson = No se ofrece como GeoJSON, el formato que se lee aquí.
map-service-items-hint = { $count ->
        [one] { $count } imagen encontrada. Intro la descarga.
       *[other] { $count } imágenes encontradas. Intro descarga una.
    }
map-service-no-items = No hay imágenes aquí.
map-service-item-cloud = { $when } · { PERCENT($cloud) } de nubes
map-service-item-no-cog = Sin Cloud-Optimized GeoTIFF
map-service-download-ask = ¿Descargar { $size } de { $host }?
map-service-download-ask-unknown = ¿Descargar esta imagen de { $host }? No se sabe su tamaño.
map-service-download-cache = Se guarda en la caché de mapas y se abre.
map-service-size-mb = { $size } MB
map-service-size-gb = { $size } GB
map-service-too-big = La imagen ocupa { $size }, demasiado para descargarla.
map-service-opened = Abriendo la imagen descargada
map-service-basemap = { $layer } es ahora el mapa base del mapa de fotos. El mapa de fotos pregunta antes de su primera descarga.
map-service-status = El servidor respondió { $status }.
map-service-unknown = No es un servicio de mapas que esta versión lea.
map-service-exception = El servidor dijo: { $message }
map-service-malformed = La respuesta del servidor está dañada: { $reason }
map-service-remove-ask = ¿Quitar «{ $server }» de los servidores guardados?
map-service-remove-detail = No se pierde nada del disco. Las capas que ya están en el mapa se quedan hasta que se cierre.
map-service-removed = { $server } quitado
map-service-saved = Servidores de mapas guardados
map-service-saved-none = Ninguno todavía. Añada uno en el panel de servicios en línea del mapa.
map-service-keywords = WMS WMTS WFS OGC API STAC servidores servicios mapas

## Photos and geotagging.
map-geotag-title = Geoetiquetar fotos con el track
map-photos-on = Fotos en el track
map-photos-off = Fotos en el track ocultas
map-geotag-offset = Reloj de la cámara: { $offset } respecto a la hora GPS. ← → lo cambian un minuto, con Mayús una hora, con Alt un segundo.
map-geotag-reading = Leyendo las fotos…
map-geotag-summary = { $count } de { $total } fotos reciben una posición.
map-geotag-sidecars = Las posiciones van a archivos XMP adjuntos junto a las fotos. Los archivos de las fotos no se modifican.
map-geotag-writing = Escribiendo…
map-geotag-write = Escribir archivos adjuntos…
map-geotag-guess = Estimar desfase
map-geotag-will-write = Recibe { $position }
map-geotag-has-position = Ya tiene posición: se deja como está
map-geotag-no-time = Sin hora de captura: se deja como está
map-geotag-outside = Tomada { $offset } fuera del track: se deja como está
map-geotag-gap = En un hueco del track (a { $seconds } s de un punto): se deja como está
map-geotag-nothing = Ninguna foto recibe posición con este desfase.
map-geotag-confirm = ¿Escribir posiciones en { $count ->
    [one] { $count } archivo XMP adjunto
   *[other] { $count } archivos XMP adjuntos
}? Las fotos en sí no se modifican.
map-geotag-confirm-ok = Escribir archivos adjuntos
map-geotag-done = { $count ->
    [one] { $count } archivo adjunto escrito
   *[other] { $count } archivos adjuntos escritos
}
map-geotag-done-failed = { $count ->
        [one] { $count } archivo adjunto escrito
       *[other] { $count } archivos adjuntos escritos
    }; { $failed } fallaron: { $reason }
map-geotag-not-sidecar = No es un nombre de archivo adjunto.

## Export and printing.
map-export-title = Exportar mapa
map-print-title = Imprimir mapa a escala
map-export-keys = ↑ ↓ eligen, ← → cambian, Intro ejecuta.
map-export-format = Formato
map-export-scale = Escala
map-export-scale-value = 1:{ $n }
map-export-resolution = Resolución
map-export-dpi = { $dpi } ppp
map-export-paper = Papel
map-export-size = { $width } × { $height } px · { $across } × { $down } km sobre el terreno
map-export-save = Guardar…
map-export-print = Imprimir…
map-export-too-large = Demasiado grande a esta escala: { $width } × { $height } px. Elija una escala o resolución menor.
map-export-no-scale = Este mapa no tiene escala sobre el terreno.
map-export-no-gpu = La exportación necesita el procesador gráfico.
map-export-subtitle = Escala { $scale } · { $crs }
map-export-data = Datos: { $sources }
map-export-saved = { $name } guardado
map-export-printing = Imprimiendo…
map-export-printed = Enviado a la impresora
map-export-print-failed = No impreso: { $reason }
