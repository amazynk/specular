# machine-drafted, needs native review
# Specular — Downloads: what is being fetched from a server (shown beside the toolbar, in
# the gallery's bottom bar, or as a small indicator) and the panel listing it. Dansk.

## The panel

# The panel's title.
net-title = Downloads
# The palette entry that opens the panel, and the indicator's tooltip.
net-show = Vis downloads
# The panel when nothing is downloading any more.
net-empty = Der hentes ikke noget.
# Cancels one download.
net-stop = Stop

## What is downloading (row titles). $host is a server's name ("tiles.example.org").

net-what-data = Data fra { $host }
# Tiles of a remote image (a cloud GeoTIFF, an OMERO image).
net-what-image-tiles = Billedfliser fra { $host }
net-what-point-cloud = Punktsky fra { $host }
net-what-map-tiles = Kortfliser fra { $host }
# A map service's capabilities, features or map images.
net-what-map-service = Korttjeneste på { $host }
net-what-dicom = DICOM-data fra { $host }
# $series is the series' description ("CT Abdomen 5 mm").
net-what-dicom-series = DICOM-serie { $series } fra { $host }
# Studies being sent to a PACS; { $host } is its host and port.
# machine-drafted, needs native review
net-what-dicom-send = Afsendelse af undersøgelser til { $host }
net-what-omero = OMERO-data fra { $host }
net-what-zarr-chunks = Zarr-blokke fra { $host }
net-what-zarr-metadata = Zarr-metadata fra { $host }

## Remote Zarr stores ($address: the store's address, without any query or password).

zarr-open-failed = Kan ikke åbne { $address }: { $reason }
# A Local AI model; $name is its name.
net-what-model = Model { $name }
net-what-update = Søgning efter opdateringer

## Figures

# How much of how much: $done and $total are formatted sizes ("45 MB", "120 MB").
net-bytes-of = { $done } af { $total }
# A speed: $size is a formatted size ("2.1 MB").
net-speed = { $size }/s
# Many requests: $done and $total are formatted numbers; $count is $total, plain, for the
# plural.
net-requests = { $done } af { $total } { $count ->
    [one] forespørgsel
   *[other] forespørgsler
}
# A DICOM series retrieved image by image: $done and $total are formatted numbers;
# $count is $total, plain, for the plural.
# machine-drafted, needs native review
net-images = { $done } af { $total } { $count ->
    [one] billede
   *[other] billeder
}
# Images the server could not send ($n formatted, $count plain).
# machine-drafted, needs native review
net-images-failed = { $n } mislykkedes
# Images sent with a warning.
# machine-drafted, needs native review
net-images-warning = { $n } { $count ->
    [one] advarsel
   *[other] advarsler
}
# Two parts of a line joined ("45 MB of 120 MB · 2.1 MB/s").
net-join = { $first } · { $second }

## Status

# Running, the speed not measured yet.
net-starting = Starter…
# Running, no time left known (many requests, or an unknown size).
net-running = Henter
net-done = Færdig
# Done, but $count of its requests failed.
net-done-failed = Færdig, { $count } mislykkedes
# Cancelled with Stop.
net-stopped = Stoppet
# $reason is one of the reasons below.
net-failed = Mislykkedes: { $reason }
net-reason-offline = ingen forbindelse
net-reason-timeout = serveren svarede ikke i tide
# $code is the HTTP status number.
net-reason-status = serveren svarede { $code }
net-reason-auth = login afvist
net-reason-other = download blev afbrudt

## The indicator

# More downloads than shown in place.
net-more = { $count } mere
# The indicator's name for screen readers: $status is the newest download and its figures.
net-chip-name = Downloads: { $status }

## Announcements (screen readers)

# $what is a row title ("Model clip-vit-b32").
net-started = Henter { $what }
net-finished = Hentet: { $what }
net-failed-announce = Download mislykkedes: { $what }
net-stopped-announce = Download stoppet: { $what }

## Intet netværk (Indstillinger › Privatliv)

# machine-drafted, needs native review
# Shown where a network feature would be (update check, model downloads, servers, maps).
network-off = Netværket er slået fra i Indstillinger › Privatliv.
# The panel under the switch while the network is off.
privacy-off-title = Netværket er slået fra.
privacy-off-what = Opdateringstjek, hentning af modeller, DICOM-, OMERO- og Zarr-servere, billeder i skyen og kort er slået fra.
# What holds the network off when it is not the switch. $file is the policy file's path.
privacy-held-policy = Angivet af din administrator i { $file }. Denne kontakt kan ikke slå netværket til igen.
# $what is --no-network or SPECULAR_NO_NETWORK.
privacy-held-run = Angivet for denne kørsel af { $what }. Denne kontakt kan ikke slå netværket til igen.

## Open address… (File)

# machine-drafted, needs native review
address-title = Åbn adresse
# machine-drafted, needs native review
address-intro = Et Zarr-lager, en cloud-GeoTIFF (.tif), en COPC-punktsky (.copc.laz), et STAC-item eller -katalog (.json) eller en DICOMweb-server.
address-label = Adresse (http, https eller s3)
address-recent = Seneste
address-open = Åbn
address-network-off = Kan ikke åbne adresser: netværket er slået fra i Indstillinger › Privatliv.
address-problem-empty = Skriv eller indsæt en adresse.
address-problem-scheme = Adressen skal begynde med http://, https:// eller s3://.
address-problem-host = Adressen har intet servernavn eller indeholder mellemrum.
# machine-drafted, needs native review
address-problem-kind = Det er ikke en adresse, { -app-name } åbner: et .zarr-lager, en .tif, en .copc.laz, en STAC-.json eller en DICOMweb-server.
