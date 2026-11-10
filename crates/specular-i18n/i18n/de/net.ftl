# machine-drafted, needs native review
# Specular — Downloads: what is being fetched from a server (shown beside the toolbar, in
# the gallery's bottom bar, or as a small indicator) and the panel listing it. Deutsch.

## The panel

# The panel's title.
net-title = Downloads
# The palette entry that opens the panel, and the indicator's tooltip.
net-show = Downloads zeigen
# The panel when nothing is downloading any more.
net-empty = Es wird nichts heruntergeladen.
# Cancels one download.
net-stop = Stopp

## What is downloading (row titles). $host is a server's name ("tiles.example.org").

net-what-data = Daten von { $host }
# Tiles of a remote image (a cloud GeoTIFF, an OMERO image).
net-what-image-tiles = Bildkacheln von { $host }
net-what-point-cloud = Punktwolke von { $host }
net-what-map-tiles = Kartenkacheln von { $host }
# A map service's capabilities, features or map images.
net-what-map-service = Kartendienst auf { $host }
net-what-dicom = DICOM-Daten von { $host }
# $series is the series' description ("CT Abdomen 5 mm").
net-what-dicom-series = DICOM-Serie { $series } von { $host }
# Studies being sent to a PACS; { $host } is its host and port.
# machine-drafted, needs native review
net-what-dicom-send = Senden von Studien an { $host }
net-what-omero = OMERO-Daten von { $host }
net-what-zarr-chunks = Zarr-Chunks von { $host }
net-what-zarr-metadata = Zarr-Metadaten von { $host }

## Remote Zarr stores ($address: the store's address, without any query or password).

zarr-open-failed = { $address } kann nicht geöffnet werden: { $reason }
# A Local AI model; $name is its name.
net-what-model = Modell { $name }
net-what-update = Suche nach Updates

## Figures

# How much of how much: $done and $total are formatted sizes ("45 MB", "120 MB").
net-bytes-of = { $done } von { $total }
# A speed: $size is a formatted size ("2.1 MB").
net-speed = { $size }/s
# Many requests: $done and $total are formatted numbers; $count is $total, plain, for the
# plural.
net-requests = { $done } von { $total } { $count ->
    [one] Anfrage
   *[other] Anfragen
}
# A DICOM series retrieved image by image: $done and $total are formatted numbers;
# $count is $total, plain, for the plural.
# machine-drafted, needs native review
net-images = { $done } von { $total } { $count ->
    [one] Bild
   *[other] Bildern
}
# Images the server could not send ($n formatted, $count plain).
# machine-drafted, needs native review
net-images-failed = { $n } fehlgeschlagen
# Images sent with a warning.
# machine-drafted, needs native review
net-images-warning = { $n } { $count ->
    [one] Warnung
   *[other] Warnungen
}
# Two parts of a line joined ("45 MB of 120 MB · 2.1 MB/s").
net-join = { $first } · { $second }

## Status

# Running, the speed not measured yet.
net-starting = Wird gestartet…
# Running, no time left known (many requests, or an unknown size).
net-running = Wird heruntergeladen
net-done = Fertig
# Done, but $count of its requests failed.
net-done-failed = Fertig, { $count } fehlgeschlagen
# Cancelled with Stop.
net-stopped = Gestoppt
# $reason is one of the reasons below.
net-failed = Fehlgeschlagen: { $reason }
net-reason-offline = keine Verbindung
net-reason-timeout = der Server hat nicht rechtzeitig geantwortet
# $code is the HTTP status number.
net-reason-status = der Server antwortete mit { $code }
net-reason-auth = Anmeldung abgelehnt
net-reason-other = der Download brach ab

## The indicator

# More downloads than shown in place.
net-more = { $count } weitere
# The indicator's name for screen readers: $status is the newest download and its figures.
net-chip-name = Downloads: { $status }

## Announcements (screen readers)

# $what is a row title ("Model clip-vit-b32").
net-started = { $what } wird heruntergeladen
net-finished = { $what } heruntergeladen
net-failed-announce = Download fehlgeschlagen: { $what }
net-stopped-announce = Download gestoppt: { $what }

## Kein Netzwerk (Einstellungen › Datenschutz)

# machine-drafted, needs native review
# Shown where a network feature would be (update check, model downloads, servers, maps).
network-off = Das Netzwerk ist in Einstellungen › Datenschutz ausgeschaltet.
# The panel under the switch while the network is off.
privacy-off-title = Das Netzwerk ist aus.
privacy-off-what = Updateprüfungen, Modell-Downloads, DICOM-, OMERO- und Zarr-Server, Cloud-Bilder und Karten sind aus.
# What holds the network off when it is not the switch. $file is the policy file's path.
privacy-held-policy = Von Ihrem Administrator in { $file } festgelegt. Dieser Schalter kann das Netzwerk nicht wieder einschalten.
# $what is --no-network or SPECULAR_NO_NETWORK.
privacy-held-run = Für diesen Start durch { $what } festgelegt. Dieser Schalter kann das Netzwerk nicht wieder einschalten.

## Open address… (File)

# machine-drafted, needs native review
address-title = Adresse öffnen
# machine-drafted, needs native review
address-intro = Ein Zarr-Store, ein Cloud-GeoTIFF (.tif), eine COPC-Punktwolke (.copc.laz), ein STAC-Item oder -Katalog (.json) oder ein DICOMweb-Server.
address-label = Adresse (http, https oder s3)
address-recent = Zuletzt verwendet
address-open = Öffnen
address-network-off = Adressen können nicht geöffnet werden: das Netzwerk ist in Einstellungen › Datenschutz ausgeschaltet.
address-problem-empty = Geben Sie eine Adresse ein oder fügen Sie sie ein.
address-problem-scheme = Die Adresse muss mit http://, https:// oder s3:// beginnen.
address-problem-host = Die Adresse hat keinen Servernamen oder enthält Leerzeichen.
# machine-drafted, needs native review
address-problem-kind = Keine Adresse, die { -app-name } öffnet: ein .zarr-Store, eine .tif, eine .copc.laz, eine STAC-.json oder ein DICOMweb-Server.
