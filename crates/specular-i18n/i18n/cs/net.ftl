# machine-drafted, needs native review
# Specular — Downloads: what is being fetched from a server (shown beside the toolbar, in
# the gallery's bottom bar, or as a small indicator) and the panel listing it. Čeština.

## The panel

# The panel's title.
net-title = Stahování
# The palette entry that opens the panel, and the indicator's tooltip.
net-show = Zobrazit stahování
# The panel when nothing is downloading any more.
net-empty = Nic se nestahuje.
# Cancels one download.
net-stop = Zastavit

## What is downloading (row titles). $host is a server's name ("tiles.example.org").

net-what-data = Data z { $host }
# Tiles of a remote image (a cloud GeoTIFF, an OMERO image).
net-what-image-tiles = Dlaždice obrazu z { $host }
net-what-point-cloud = Mračno bodů z { $host }
net-what-map-tiles = Dlaždice mapy z { $host }
# A map service's capabilities, features or map images.
net-what-map-service = Mapová služba na { $host }
net-what-dicom = Data DICOM z { $host }
# $series is the series' description ("CT Abdomen 5 mm").
net-what-dicom-series = Série DICOM { $series } z { $host }
# Studies being sent to a PACS; { $host } is its host and port.
# machine-drafted, needs native review
net-what-dicom-send = Odesílání studií na { $host }
net-what-omero = Data OMERO z { $host }
net-what-zarr-chunks = Bloky Zarr z { $host }
net-what-zarr-metadata = Metadata Zarr z { $host }

## Remote Zarr stores ($address: the store's address, without any query or password).

zarr-open-failed = Nelze otevřít { $address }: { $reason }
# A Local AI model; $name is its name.
net-what-model = Model { $name }
net-what-update = Kontrola aktualizací

## Figures

# How much of how much: $done and $total are formatted sizes ("45 MB", "120 MB").
net-bytes-of = { $done } z { $total }
# A speed: $size is a formatted size ("2.1 MB").
net-speed = { $size }/s
# Many requests: $done and $total are formatted numbers; $count is $total, plain, for the
# plural.
net-requests = { $done } z { $total } { $count ->
    [one] požadavku
    [few] požadavků
    [many] požadavku
   *[other] požadavků
}
# A DICOM series retrieved image by image: $done and $total are formatted numbers;
# $count is $total, plain, for the plural.
# machine-drafted, needs native review
net-images = { $done } z { $total } { $count ->
    [one] snímku
    [few] snímků
    [many] snímku
   *[other] snímků
}
# Images the server could not send ($n formatted, $count plain).
# machine-drafted, needs native review
net-images-failed = nezdařilo se: { $n }
# Images sent with a warning.
# machine-drafted, needs native review
net-images-warning = { $n } { $count ->
    [one] upozornění
    [few] upozornění
    [many] upozornění
   *[other] upozornění
}
# Two parts of a line joined ("45 MB of 120 MB · 2.1 MB/s").
net-join = { $first } · { $second }

## Status

# Running, the speed not measured yet.
net-starting = Spouštění…
# Running, no time left known (many requests, or an unknown size).
net-running = Stahování
net-done = Hotovo
# Done, but $count of its requests failed.
net-done-failed = Hotovo, nezdařilo se: { $count }
# Cancelled with Stop.
net-stopped = Zastaveno
# $reason is one of the reasons below.
net-failed = Nezdařilo se: { $reason }
net-reason-offline = bez připojení
net-reason-timeout = server neodpověděl včas
# $code is the HTTP status number.
net-reason-status = server odpověděl { $code }
net-reason-auth = přihlášení odmítnuto
net-reason-other = stahování se přerušilo

## The indicator

# More downloads than shown in place.
net-more = ještě { $count }
# The indicator's name for screen readers: $status is the newest download and its figures.
net-chip-name = Stahování: { $status }

## Announcements (screen readers)

# $what is a row title ("Model clip-vit-b32").
net-started = Stahuje se { $what }
net-finished = Staženo: { $what }
net-failed-announce = Stahování se nezdařilo: { $what }
net-stopped-announce = Stahování zastaveno: { $what }

## Bez sítě (Nastavení › Soukromí)

# machine-drafted, needs native review
# Shown where a network feature would be (update check, model downloads, servers, maps).
network-off = Síť je vypnutá v Nastavení › Soukromí.
# The panel under the switch while the network is off.
privacy-off-title = Síť je vypnutá.
privacy-off-what = Kontrola aktualizací, stahování modelů, servery DICOM, OMERO a Zarr, obrázky v cloudu a mapy jsou vypnuté.
# What holds the network off when it is not the switch. $file is the policy file's path.
privacy-held-policy = Nastavil váš správce v { $file }. Tento přepínač síť znovu nezapne.
# $what is --no-network or SPECULAR_NO_NETWORK.
privacy-held-run = Nastaveno pro toto spuštění pomocí { $what }. Tento přepínač síť znovu nezapne.

## Open address… (File)

# machine-drafted, needs native review
address-title = Otevřít adresu
# machine-drafted, needs native review
address-intro = Úložiště Zarr, cloudový GeoTIFF (.tif), mračno bodů COPC (.copc.laz), položka nebo katalog STAC (.json) nebo server DICOMweb.
address-label = Adresa (http, https nebo s3)
address-recent = Nedávné
address-open = Otevřít
address-network-off = Adresy nelze otevřít: síť je vypnutá v Nastavení › Soukromí.
address-problem-empty = Napište nebo vložte adresu.
address-problem-scheme = Adresa musí začínat http://, https:// nebo s3://.
address-problem-host = Adresa neobsahuje název serveru nebo obsahuje mezery.
# machine-drafted, needs native review
address-problem-kind = Tuto adresu { -app-name } neotevře. Otevírá úložiště .zarr, soubor .tif, .copc.laz, soubor .json STAC nebo server DICOMweb.
