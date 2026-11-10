# machine-drafted, needs native review
# Specular — Downloads: what is being fetched from a server (shown beside the toolbar, in
# the gallery's bottom bar, or as a small indicator) and the panel listing it. Svenska.

## The panel

# The panel's title.
net-title = Nedladdningar
# The palette entry that opens the panel, and the indicator's tooltip.
net-show = Visa nedladdningar
# The panel when nothing is downloading any more.
net-empty = Inget hämtas.
# Cancels one download.
net-stop = Stoppa

## What is downloading (row titles). $host is a server's name ("tiles.example.org").

net-what-data = Data från { $host }
# Tiles of a remote image (a cloud GeoTIFF, an OMERO image).
net-what-image-tiles = Bildrutor från { $host }
net-what-point-cloud = Punktmoln från { $host }
net-what-map-tiles = Kartrutor från { $host }
# A map service's capabilities, features or map images.
net-what-map-service = Karttjänst på { $host }
net-what-dicom = DICOM-data från { $host }
# $series is the series' description ("CT Abdomen 5 mm").
net-what-dicom-series = DICOM-serie { $series } från { $host }
# Studies being sent to a PACS; { $host } is its host and port.
# machine-drafted, needs native review
net-what-dicom-send = Skickar undersökningar till { $host }
net-what-omero = OMERO-data från { $host }
net-what-zarr-chunks = Zarr-block från { $host }
net-what-zarr-metadata = Zarr-metadata från { $host }

## Remote Zarr stores ($address: the store's address, without any query or password).

zarr-open-failed = Det går inte att öppna { $address }: { $reason }
# A Local AI model; $name is its name.
net-what-model = Modell { $name }
net-what-update = Sökning efter uppdateringar

## Figures

# How much of how much: $done and $total are formatted sizes ("45 MB", "120 MB").
net-bytes-of = { $done } av { $total }
# A speed: $size is a formatted size ("2.1 MB").
net-speed = { $size }/s
# Many requests: $done and $total are formatted numbers; $count is $total, plain, for the
# plural.
net-requests = { $done } av { $total } { $count ->
    [one] begäran
   *[other] begäranden
}
# A DICOM series retrieved image by image: $done and $total are formatted numbers;
# $count is $total, plain, for the plural.
# machine-drafted, needs native review
net-images = { $done } av { $total } { $count ->
    [one] bild
   *[other] bilder
}
# Images the server could not send ($n formatted, $count plain).
# machine-drafted, needs native review
net-images-failed = { $n } misslyckades
# Images sent with a warning.
# machine-drafted, needs native review
net-images-warning = { $n } { $count ->
    [one] varning
   *[other] varningar
}
# Two parts of a line joined ("45 MB of 120 MB · 2.1 MB/s").
net-join = { $first } · { $second }

## Status

# Running, the speed not measured yet.
net-starting = Startar…
# Running, no time left known (many requests, or an unknown size).
net-running = Hämtar
net-done = Klar
# Done, but $count of its requests failed.
net-done-failed = Klar, { $count } misslyckades
# Cancelled with Stop.
net-stopped = Stoppad
# $reason is one of the reasons below.
net-failed = Misslyckades: { $reason }
net-reason-offline = ingen anslutning
net-reason-timeout = servern svarade inte i tid
# $code is the HTTP status number.
net-reason-status = servern svarade { $code }
net-reason-auth = inloggningen nekades
net-reason-other = nedladdningen avbröts

## The indicator

# More downloads than shown in place.
net-more = { $count } till
# The indicator's name for screen readers: $status is the newest download and its figures.
net-chip-name = Nedladdningar: { $status }

## Announcements (screen readers)

# $what is a row title ("Model clip-vit-b32").
net-started = Hämtar { $what }
net-finished = Hämtat: { $what }
net-failed-announce = Nedladdningen misslyckades: { $what }
net-stopped-announce = Nedladdningen stoppades: { $what }

## Inget nätverk (Inställningar › Integritet)

# machine-drafted, needs native review
# Shown where a network feature would be (update check, model downloads, servers, maps).
network-off = Nätverket är avstängt i Inställningar › Integritet.
# The panel under the switch while the network is off.
privacy-off-title = Nätverket är avstängt.
privacy-off-what = Uppdateringskontroller, modellnedladdningar, DICOM-, OMERO- och Zarr-servrar, molnbilder och kartor är avstängda.
# What holds the network off when it is not the switch. $file is the policy file's path.
privacy-held-policy = Inställt av din administratör i { $file }. Den här knappen kan inte slå på nätverket igen.
# $what is --no-network or SPECULAR_NO_NETWORK.
privacy-held-run = Inställt för den här körningen av { $what }. Den här knappen kan inte slå på nätverket igen.

## Open address… (File)

# machine-drafted, needs native review
address-title = Öppna adress
# machine-drafted, needs native review
address-intro = Ett Zarr-lager, en GeoTIFF i molnet (.tif), ett COPC-punktmoln (.copc.laz), ett STAC-item eller en STAC-katalog (.json) eller en DICOMweb-server.
address-label = Adress (http, https eller s3)
address-recent = Senaste
address-open = Öppna
address-network-off = Kan inte öppna adresser: nätverket är avstängt i Inställningar › Integritet.
address-problem-empty = Skriv eller klistra in en adress.
address-problem-scheme = Adressen måste börja med http://, https:// eller s3://.
address-problem-host = Adressen saknar servernamn eller innehåller mellanslag.
# machine-drafted, needs native review
address-problem-kind = Det här är ingen adress som { -app-name } öppnar: ett .zarr-lager, en .tif, en .copc.laz, en STAC-.json eller en DICOMweb-server.
