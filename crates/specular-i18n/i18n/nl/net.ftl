# machine-drafted, needs native review
# Specular — Downloads: what is being fetched from a server (shown beside the toolbar, in
# the gallery's bottom bar, or as a small indicator) and the panel listing it. Nederlands.

## The panel

# The panel's title.
net-title = Downloads
# The palette entry that opens the panel, and the indicator's tooltip.
net-show = Downloads tonen
# The panel when nothing is downloading any more.
net-empty = Er wordt niets gedownload.
# Cancels one download.
net-stop = Stoppen

## What is downloading (row titles). $host is a server's name ("tiles.example.org").

net-what-data = Gegevens van { $host }
# Tiles of a remote image (a cloud GeoTIFF, an OMERO image).
net-what-image-tiles = Beeldtegels van { $host }
net-what-point-cloud = Puntenwolk van { $host }
net-what-map-tiles = Kaarttegels van { $host }
# A map service's capabilities, features or map images.
net-what-map-service = Kaartdienst op { $host }
net-what-dicom = DICOM-gegevens van { $host }
# $series is the series' description ("CT Abdomen 5 mm").
net-what-dicom-series = DICOM-serie { $series } van { $host }
# Studies being sent to a PACS; { $host } is its host and port.
# machine-drafted, needs native review
net-what-dicom-send = Onderzoeken versturen naar { $host }
net-what-omero = OMERO-gegevens van { $host }
net-what-zarr-chunks = Zarr-chunks van { $host }
net-what-zarr-metadata = Zarr-metagegevens van { $host }

## Remote Zarr stores ($address: the store's address, without any query or password).

zarr-open-failed = Kan { $address } niet openen: { $reason }
# A Local AI model; $name is its name.
net-what-model = Model { $name }
net-what-update = Zoeken naar updates

## Figures

# How much of how much: $done and $total are formatted sizes ("45 MB", "120 MB").
net-bytes-of = { $done } van { $total }
# A speed: $size is a formatted size ("2.1 MB").
net-speed = { $size }/s
# Many requests: $done and $total are formatted numbers; $count is $total, plain, for the
# plural.
net-requests = { $done } van { $total } { $count ->
    [one] verzoek
   *[other] verzoeken
}
# A DICOM series retrieved image by image: $done and $total are formatted numbers;
# $count is $total, plain, for the plural.
# machine-drafted, needs native review
net-images = { $done } van { $total } { $count ->
    [one] beeld
   *[other] beelden
}
# Images the server could not send ($n formatted, $count plain).
# machine-drafted, needs native review
net-images-failed = { $n } mislukt
# Images sent with a warning.
# machine-drafted, needs native review
net-images-warning = { $n } { $count ->
    [one] waarschuwing
   *[other] waarschuwingen
}
# Two parts of a line joined ("45 MB of 120 MB · 2.1 MB/s").
net-join = { $first } · { $second }

## Status

# Running, the speed not measured yet.
net-starting = Starten…
# Running, no time left known (many requests, or an unknown size).
net-running = Downloaden
net-done = Klaar
# Done, but $count of its requests failed.
net-done-failed = Klaar, { $count } mislukt
# Cancelled with Stop.
net-stopped = Gestopt
# $reason is one of the reasons below.
net-failed = Mislukt: { $reason }
net-reason-offline = geen verbinding
net-reason-timeout = de server antwoordde niet op tijd
# $code is the HTTP status number.
net-reason-status = de server antwoordde { $code }
net-reason-auth = aanmelding geweigerd
net-reason-other = de download brak af

## The indicator

# More downloads than shown in place.
net-more = nog { $count }
# The indicator's name for screen readers: $status is the newest download and its figures.
net-chip-name = Downloads: { $status }

## Announcements (screen readers)

# $what is a row title ("Model clip-vit-b32").
net-started = { $what } downloaden
net-finished = { $what } gedownload
net-failed-announce = Download mislukt: { $what }
net-stopped-announce = Download gestopt: { $what }

## Geen netwerk (Instellingen › Privacy)

# machine-drafted, needs native review
# Shown where a network feature would be (update check, model downloads, servers, maps).
network-off = Het netwerk staat uit in Instellingen › Privacy.
# The panel under the switch while the network is off.
privacy-off-title = Het netwerk staat uit.
privacy-off-what = Updatecontroles, modeldownloads, DICOM-, OMERO- en Zarr-servers, cloudafbeeldingen en kaarten staan uit.
# What holds the network off when it is not the switch. $file is the policy file's path.
privacy-held-policy = Ingesteld door uw beheerder in { $file }. Deze schakelaar kan het netwerk niet weer aanzetten.
# $what is --no-network or SPECULAR_NO_NETWORK.
privacy-held-run = Voor deze sessie ingesteld door { $what }. Deze schakelaar kan het netwerk niet weer aanzetten.

## Open address… (File)

# machine-drafted, needs native review
address-title = Adres openen
# machine-drafted, needs native review
address-intro = Een Zarr-store, een cloud-GeoTIFF (.tif), een COPC-puntenwolk (.copc.laz), een STAC-item of -catalogus (.json) of een DICOMweb-server.
address-label = Adres (http, https of s3)
address-recent = Recent
address-open = Openen
address-network-off = Kan geen adressen openen: het netwerk staat uit in Instellingen › Privacy.
address-problem-empty = Typ of plak een adres.
address-problem-scheme = Het adres moet beginnen met http://, https:// of s3://.
address-problem-host = Het adres bevat geen servernaam, of bevat spaties.
# machine-drafted, needs native review
address-problem-kind = Dit is geen adres dat { -app-name } opent: een .zarr-store, een .tif, een .copc.laz, een STAC-.json of een DICOMweb-server.
