# machine-drafted, needs native review
# Specular — Downloads: what is being fetched from a server (shown beside the toolbar, in
# the gallery's bottom bar, or as a small indicator) and the panel listing it. Italiano.

## The panel

# The panel's title.
net-title = Download
# The palette entry that opens the panel, and the indicator's tooltip.
net-show = Mostra download
# The panel when nothing is downloading any more.
net-empty = Nessun download in corso.
# Cancels one download.
net-stop = Interrompi

## What is downloading (row titles). $host is a server's name ("tiles.example.org").

net-what-data = Dati da { $host }
# Tiles of a remote image (a cloud GeoTIFF, an OMERO image).
net-what-image-tiles = Riquadri d’immagine da { $host }
net-what-point-cloud = Nuvola di punti da { $host }
net-what-map-tiles = Riquadri di mappa da { $host }
# A map service's capabilities, features or map images.
net-what-map-service = Servizio di mappe su { $host }
net-what-dicom = Dati DICOM da { $host }
# $series is the series' description ("CT Abdomen 5 mm").
net-what-dicom-series = Serie DICOM { $series } da { $host }
# Studies being sent to a PACS; { $host } is its host and port.
# machine-drafted, needs native review
net-what-dicom-send = Invio di studi a { $host }
net-what-omero = Dati OMERO da { $host }
net-what-zarr-chunks = Blocchi Zarr da { $host }
net-what-zarr-metadata = Metadati Zarr da { $host }

## Remote Zarr stores ($address: the store's address, without any query or password).

zarr-open-failed = Impossibile aprire { $address }: { $reason }
# A Local AI model; $name is its name.
net-what-model = Modello { $name }
net-what-update = Ricerca aggiornamenti

## Figures

# How much of how much: $done and $total are formatted sizes ("45 MB", "120 MB").
net-bytes-of = { $done } di { $total }
# A speed: $size is a formatted size ("2.1 MB").
net-speed = { $size }/s
# Many requests: $done and $total are formatted numbers; $count is $total, plain, for the
# plural.
net-requests = { $done } di { $total } { $count ->
    [one] richiesta
    [many] richieste
   *[other] richieste
}
# A DICOM series retrieved image by image: $done and $total are formatted numbers;
# $count is $total, plain, for the plural.
# machine-drafted, needs native review
net-images = { $done } di { $total } { $count ->
    [one] immagine
    [many] immagini
   *[other] immagini
}
# Images the server could not send ($n formatted, $count plain).
# machine-drafted, needs native review
net-images-failed = { $n } non riuscite
# Images sent with a warning.
# machine-drafted, needs native review
net-images-warning = { $n } { $count ->
    [one] avviso
    [many] avvisi
   *[other] avvisi
}
# Two parts of a line joined ("45 MB of 120 MB · 2.1 MB/s").
net-join = { $first } · { $second }

## Status

# Running, the speed not measured yet.
net-starting = Avvio…
# Running, no time left known (many requests, or an unknown size).
net-running = Download in corso
net-done = Fatto
# Done, but $count of its requests failed.
net-done-failed = Fatto, { $count } non riusciti
# Cancelled with Stop.
net-stopped = Interrotto
# $reason is one of the reasons below.
net-failed = Non riuscito: { $reason }
net-reason-offline = nessuna connessione
net-reason-timeout = il server non ha risposto in tempo
# $code is the HTTP status number.
net-reason-status = il server ha risposto { $code }
net-reason-auth = accesso rifiutato
net-reason-other = il download si è interrotto

## The indicator

# More downloads than shown in place.
net-more = altri { $count }
# The indicator's name for screen readers: $status is the newest download and its figures.
net-chip-name = Download: { $status }

## Announcements (screen readers)

# $what is a row title ("Model clip-vit-b32").
net-started = Download di { $what }
net-finished = Scaricato: { $what }
net-failed-announce = Download non riuscito: { $what }
net-stopped-announce = Download interrotto: { $what }

## Nessuna rete (Impostazioni › Privacy)

# machine-drafted, needs native review
# Shown where a network feature would be (update check, model downloads, servers, maps).
network-off = La rete è disattivata in Impostazioni › Privacy.
# The panel under the switch while the network is off.
privacy-off-title = La rete è disattivata.
privacy-off-what = Controllo degli aggiornamenti, download dei modelli, server DICOM, OMERO e Zarr, immagini nel cloud e mappe sono disattivati.
# What holds the network off when it is not the switch. $file is the policy file's path.
privacy-held-policy = Impostato dall’amministratore in { $file }. Questo interruttore non può riattivare la rete.
# $what is --no-network or SPECULAR_NO_NETWORK.
privacy-held-run = Impostato per questa esecuzione da { $what }. Questo interruttore non può riattivare la rete.

## Open address… (File)

# machine-drafted, needs native review
address-title = Apri indirizzo
# machine-drafted, needs native review
address-intro = Un archivio Zarr, un GeoTIFF nel cloud (.tif), una nuvola di punti COPC (.copc.laz), un item o catalogo STAC (.json) o un server DICOMweb.
address-label = Indirizzo (http, https o s3)
address-recent = Recenti
address-open = Apri
address-network-off = Impossibile aprire indirizzi: la rete è disattivata in Impostazioni › Privacy.
address-problem-empty = Digita o incolla un indirizzo.
address-problem-scheme = L’indirizzo deve iniziare con http://, https:// o s3://.
address-problem-host = L’indirizzo non contiene il nome del server, oppure contiene spazi.
# machine-drafted, needs native review
address-problem-kind = { -app-name } non apre questo indirizzo. Apre un archivio .zarr, un .tif, un .copc.laz, un .json STAC o un server DICOMweb.
