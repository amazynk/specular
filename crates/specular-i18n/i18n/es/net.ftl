# machine-drafted, needs native review
# Specular — Downloads: what is being fetched from a server (shown beside the toolbar, in
# the gallery's bottom bar, or as a small indicator) and the panel listing it. Español.

## The panel

# The panel's title.
net-title = Descargas
# The palette entry that opens the panel, and the indicator's tooltip.
net-show = Mostrar descargas
# The panel when nothing is downloading any more.
net-empty = No se está descargando nada.
# Cancels one download.
net-stop = Detener

## What is downloading (row titles). $host is a server's name ("tiles.example.org").

net-what-data = Datos de { $host }
# Tiles of a remote image (a cloud GeoTIFF, an OMERO image).
net-what-image-tiles = Mosaicos de imagen de { $host }
net-what-point-cloud = Nube de puntos de { $host }
net-what-map-tiles = Mosaicos de mapa de { $host }
# A map service's capabilities, features or map images.
net-what-map-service = Servicio de mapas en { $host }
net-what-dicom = Datos DICOM de { $host }
# $series is the series' description ("CT Abdomen 5 mm").
net-what-dicom-series = Serie DICOM { $series } de { $host }
# Studies being sent to a PACS; { $host } is its host and port.
# machine-drafted, needs native review
net-what-dicom-send = Envío de estudios a { $host }
net-what-omero = Datos OMERO de { $host }
net-what-zarr-chunks = Fragmentos Zarr de { $host }
net-what-zarr-metadata = Metadatos Zarr de { $host }

## Remote Zarr stores ($address: the store's address, without any query or password).

zarr-open-failed = No se puede abrir { $address }: { $reason }
# A Local AI model; $name is its name.
net-what-model = Modelo { $name }
net-what-update = Búsqueda de actualizaciones

## Figures

# How much of how much: $done and $total are formatted sizes ("45 MB", "120 MB").
net-bytes-of = { $done } de { $total }
# A speed: $size is a formatted size ("2.1 MB").
net-speed = { $size }/s
# Many requests: $done and $total are formatted numbers; $count is $total, plain, for the
# plural.
net-requests = { $done } de { $total } { $count ->
    [one] solicitud
    [many] solicitudes
   *[other] solicitudes
}
# A DICOM series retrieved image by image: $done and $total are formatted numbers;
# $count is $total, plain, for the plural.
# machine-drafted, needs native review
net-images = { $done } de { $total } { $count ->
    [one] imagen
    [many] imágenes
   *[other] imágenes
}
# Images the server could not send ($n formatted, $count plain).
# machine-drafted, needs native review
net-images-failed = { $n } con error
# Images sent with a warning.
# machine-drafted, needs native review
net-images-warning = { $n } { $count ->
    [one] advertencia
    [many] advertencias
   *[other] advertencias
}
# Two parts of a line joined ("45 MB of 120 MB · 2.1 MB/s").
net-join = { $first } · { $second }

## Status

# Running, the speed not measured yet.
net-starting = Iniciando…
# Running, no time left known (many requests, or an unknown size).
net-running = Descargando
net-done = Hecho
# Done, but $count of its requests failed.
net-done-failed = Hecho, { $count } con error
# Cancelled with Stop.
net-stopped = Detenido
# $reason is one of the reasons below.
net-failed = Error: { $reason }
net-reason-offline = sin conexión
net-reason-timeout = el servidor no respondió a tiempo
# $code is the HTTP status number.
net-reason-status = el servidor respondió { $code }
net-reason-auth = inicio de sesión rechazado
net-reason-other = la descarga se interrumpió

## The indicator

# More downloads than shown in place.
net-more = { $count } más
# The indicator's name for screen readers: $status is the newest download and its figures.
net-chip-name = Descargas: { $status }

## Announcements (screen readers)

# $what is a row title ("Model clip-vit-b32").
net-started = Descargando { $what }
net-finished = Descargado: { $what }
net-failed-announce = Error en la descarga: { $what }
net-stopped-announce = Descarga detenida: { $what }

## Sin red (Configuración › Privacidad)

# machine-drafted, needs native review
# Shown where a network feature would be (update check, model downloads, servers, maps).
network-off = La red está desactivada en Configuración › Privacidad.
# The panel under the switch while the network is off.
privacy-off-title = La red está desactivada.
privacy-off-what = Las búsquedas de actualizaciones, las descargas de modelos, los servidores DICOM, OMERO y Zarr, las imágenes en la nube y los mapas están desactivados.
# What holds the network off when it is not the switch. $file is the policy file's path.
privacy-held-policy = Lo ha establecido tu administrador en { $file }. Este interruptor no puede volver a activar la red.
# $what is --no-network or SPECULAR_NO_NETWORK.
privacy-held-run = Establecido para esta ejecución por { $what }. Este interruptor no puede volver a activar la red.

## Open address… (File)

# machine-drafted, needs native review
address-title = Abrir dirección
# machine-drafted, needs native review
address-intro = Un almacén Zarr, un GeoTIFF en la nube (.tif), una nube de puntos COPC (.copc.laz), un ítem o catálogo STAC (.json) o un servidor DICOMweb.
address-label = Dirección (http, https o s3)
address-recent = Recientes
address-open = Abrir
address-network-off = No se pueden abrir direcciones: la red está desactivada en Configuración › Privacidad.
address-problem-empty = Escriba o pegue una dirección.
address-problem-scheme = La dirección debe empezar por http://, https:// o s3://.
address-problem-host = La dirección no tiene nombre de servidor o contiene espacios.
# machine-drafted, needs native review
address-problem-kind = No es una dirección que { -app-name } abra: un almacén .zarr, un .tif, un .copc.laz, un .json STAC o un servidor DICOMweb.
