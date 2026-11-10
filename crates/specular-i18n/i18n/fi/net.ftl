# machine-drafted, needs native review
# Specular — Downloads: what is being fetched from a server (shown beside the toolbar, in
# the gallery's bottom bar, or as a small indicator) and the panel listing it. Suomi.

## The panel

# The panel's title.
net-title = Lataukset
# The palette entry that opens the panel, and the indicator's tooltip.
net-show = Näytä lataukset
# The panel when nothing is downloading any more.
net-empty = Mitään ei ladata.
# Cancels one download.
net-stop = Pysäytä

## What is downloading (row titles). $host is a server's name ("tiles.example.org").

net-what-data = Tietoja palvelimelta { $host }
# Tiles of a remote image (a cloud GeoTIFF, an OMERO image).
net-what-image-tiles = Kuvan ruutuja palvelimelta { $host }
net-what-point-cloud = Pistepilvi palvelimelta { $host }
net-what-map-tiles = Kartan ruutuja palvelimelta { $host }
# A map service's capabilities, features or map images.
net-what-map-service = Karttapalvelu palvelimella { $host }
net-what-dicom = DICOM-tietoja palvelimelta { $host }
# $series is the series' description ("CT Abdomen 5 mm").
net-what-dicom-series = DICOM-sarja { $series } palvelimelta { $host }
# Studies being sent to a PACS; { $host } is its host and port.
# machine-drafted, needs native review
net-what-dicom-send = Tutkimusten lähetys palvelimelle { $host }
net-what-omero = OMERO-tietoja palvelimelta { $host }
net-what-zarr-chunks = Zarr-lohkoja palvelimelta { $host }
net-what-zarr-metadata = Zarr-metatietoja palvelimelta { $host }

## Remote Zarr stores ($address: the store's address, without any query or password).

zarr-open-failed = Kohdetta { $address } ei voi avata: { $reason }
# A Local AI model; $name is its name.
net-what-model = Malli { $name }
net-what-update = Päivitysten tarkistus

## Figures

# How much of how much: $done and $total are formatted sizes ("45 MB", "120 MB").
net-bytes-of = { $done }/{ $total }
# A speed: $size is a formatted size ("2.1 MB").
net-speed = { $size }/s
# Many requests: $done and $total are formatted numbers; $count is $total, plain, for the
# plural.
net-requests = { $done }/{ $total } { $count ->
    [one] pyyntö
   *[other] pyyntöä
}
# A DICOM series retrieved image by image: $done and $total are formatted numbers;
# $count is $total, plain, for the plural.
# machine-drafted, needs native review
net-images = { $done }/{ $total } { $count ->
    [one] kuva
   *[other] kuvaa
}
# Images the server could not send ($n formatted, $count plain).
# machine-drafted, needs native review
net-images-failed = { $n } epäonnistui
# Images sent with a warning.
# machine-drafted, needs native review
net-images-warning = { $n } { $count ->
    [one] varoitus
   *[other] varoitusta
}
# Two parts of a line joined ("45 MB of 120 MB · 2.1 MB/s").
net-join = { $first } · { $second }

## Status

# Running, the speed not measured yet.
net-starting = Aloitetaan…
# Running, no time left known (many requests, or an unknown size).
net-running = Ladataan
net-done = Valmis
# Done, but $count of its requests failed.
net-done-failed = Valmis, { $count } epäonnistui
# Cancelled with Stop.
net-stopped = Pysäytetty
# $reason is one of the reasons below.
net-failed = Epäonnistui: { $reason }
net-reason-offline = ei yhteyttä
net-reason-timeout = palvelin ei vastannut ajoissa
# $code is the HTTP status number.
net-reason-status = palvelin vastasi { $code }
net-reason-auth = kirjautuminen hylättiin
net-reason-other = lataus katkesi

## The indicator

# More downloads than shown in place.
net-more = { $count } lisää
# The indicator's name for screen readers: $status is the newest download and its figures.
net-chip-name = Lataukset: { $status }

## Announcements (screen readers)

# $what is a row title ("Model clip-vit-b32").
net-started = Ladataan: { $what }
net-finished = Ladattu: { $what }
net-failed-announce = Lataus epäonnistui: { $what }
net-stopped-announce = Lataus pysäytetty: { $what }

## Ei verkkoa (Asetukset › Yksityisyys)

# machine-drafted, needs native review
# Shown where a network feature would be (update check, model downloads, servers, maps).
network-off = Verkko on pois päältä kohdassa Asetukset › Yksityisyys.
# The panel under the switch while the network is off.
privacy-off-title = Verkko on pois päältä.
privacy-off-what = Päivitystarkistukset, mallien lataukset, DICOM-, OMERO- ja Zarr-palvelimet, pilvikuvat ja kartat ovat pois päältä.
# What holds the network off when it is not the switch. $file is the policy file's path.
privacy-held-policy = Ylläpitäjäsi on asettanut tämän tiedostossa { $file }. Tämä kytkin ei voi kytkeä verkkoa takaisin päälle.
# $what is --no-network or SPECULAR_NO_NETWORK.
privacy-held-run = Asetettu tälle ajokerralle: { $what }. Tämä kytkin ei voi kytkeä verkkoa takaisin päälle.

## Open address… (File)

# machine-drafted, needs native review
address-title = Avaa osoite
# machine-drafted, needs native review
address-intro = Zarr-säilö, pilvessä oleva GeoTIFF (.tif), COPC-pistepilvi (.copc.laz), STAC-kohde tai -luettelo (.json) tai DICOMweb-palvelin.
address-label = Osoite (http, https tai s3)
address-recent = Viimeaikaiset
address-open = Avaa
address-network-off = Osoitteita ei voi avata: verkko on pois päältä kohdassa Asetukset › Yksityisyys.
address-problem-empty = Kirjoita tai liitä osoite.
address-problem-scheme = Osoitteen on alettava http://, https:// tai s3://.
address-problem-host = Osoitteessa ei ole palvelimen nimeä, tai siinä on välilyöntejä.
# machine-drafted, needs native review
address-problem-kind = { -app-name } ei avaa tätä osoitetta. Se avaa .zarr-säilön, .tif-tiedoston, .copc.laz-tiedoston, STAC-.json-tiedoston tai DICOMweb-palvelimen.
