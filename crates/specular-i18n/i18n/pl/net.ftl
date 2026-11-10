# machine-drafted, needs native review
# Specular — Downloads: what is being fetched from a server (shown beside the toolbar, in
# the gallery's bottom bar, or as a small indicator) and the panel listing it. Polski.

## The panel

# The panel's title.
net-title = Pobieranie
# The palette entry that opens the panel, and the indicator's tooltip.
net-show = Pokaż pobieranie
# The panel when nothing is downloading any more.
net-empty = Nic nie jest pobierane.
# Cancels one download.
net-stop = Zatrzymaj

## What is downloading (row titles). $host is a server's name ("tiles.example.org").

net-what-data = Dane z { $host }
# Tiles of a remote image (a cloud GeoTIFF, an OMERO image).
net-what-image-tiles = Kafelki obrazu z { $host }
net-what-point-cloud = Chmura punktów z { $host }
net-what-map-tiles = Kafelki mapy z { $host }
# A map service's capabilities, features or map images.
net-what-map-service = Usługa map na { $host }
net-what-dicom = Dane DICOM z { $host }
# $series is the series' description ("CT Abdomen 5 mm").
net-what-dicom-series = Seria DICOM { $series } z { $host }
# Studies being sent to a PACS; { $host } is its host and port.
# machine-drafted, needs native review
net-what-dicom-send = Wysyłanie badań do { $host }
net-what-omero = Dane OMERO z { $host }
net-what-zarr-chunks = Fragmenty Zarr z { $host }
net-what-zarr-metadata = Metadane Zarr z { $host }

## Remote Zarr stores ($address: the store's address, without any query or password).

zarr-open-failed = Nie można otworzyć { $address }: { $reason }
# A Local AI model; $name is its name.
net-what-model = Model { $name }
net-what-update = Sprawdzanie aktualizacji

## Figures

# How much of how much: $done and $total are formatted sizes ("45 MB", "120 MB").
net-bytes-of = { $done } z { $total }
# A speed: $size is a formatted size ("2.1 MB").
net-speed = { $size }/s
# Many requests: $done and $total are formatted numbers; $count is $total, plain, for the
# plural.
net-requests = { $done } z { $total } { $count ->
    [one] żądania
    [few] żądań
    [many] żądań
   *[other] żądania
}
# A DICOM series retrieved image by image: $done and $total are formatted numbers;
# $count is $total, plain, for the plural.
# machine-drafted, needs native review
net-images = { $done } z { $total } { $count ->
    [one] obrazu
    [few] obrazów
    [many] obrazów
   *[other] obrazu
}
# Images the server could not send ($n formatted, $count plain).
# machine-drafted, needs native review
net-images-failed = nieudane: { $n }
# Images sent with a warning.
# machine-drafted, needs native review
net-images-warning = { $n } { $count ->
    [one] ostrzeżenie
    [few] ostrzeżenia
    [many] ostrzeżeń
   *[other] ostrzeżenia
}
# Two parts of a line joined ("45 MB of 120 MB · 2.1 MB/s").
net-join = { $first } · { $second }

## Status

# Running, the speed not measured yet.
net-starting = Uruchamianie…
# Running, no time left known (many requests, or an unknown size).
net-running = Pobieranie
net-done = Gotowe
# Done, but $count of its requests failed.
net-done-failed = Gotowe, nieudane: { $count }
# Cancelled with Stop.
net-stopped = Zatrzymano
# $reason is one of the reasons below.
net-failed = Niepowodzenie: { $reason }
net-reason-offline = brak połączenia
net-reason-timeout = serwer nie odpowiedział na czas
# $code is the HTTP status number.
net-reason-status = serwer odpowiedział { $code }
net-reason-auth = logowanie odrzucone
net-reason-other = pobieranie zostało przerwane

## The indicator

# More downloads than shown in place.
net-more = jeszcze { $count }
# The indicator's name for screen readers: $status is the newest download and its figures.
net-chip-name = Pobieranie: { $status }

## Announcements (screen readers)

# $what is a row title ("Model clip-vit-b32").
net-started = Pobieranie: { $what }
net-finished = Pobrano: { $what }
net-failed-announce = Pobieranie nieudane: { $what }
net-stopped-announce = Pobieranie zatrzymane: { $what }

## Bez sieci (Ustawienia › Prywatność)

# machine-drafted, needs native review
# Shown where a network feature would be (update check, model downloads, servers, maps).
network-off = Sieć jest wyłączona w Ustawienia › Prywatność.
# The panel under the switch while the network is off.
privacy-off-title = Sieć jest wyłączona.
privacy-off-what = Sprawdzanie aktualizacji, pobieranie modeli, serwery DICOM, OMERO i Zarr, obrazy w chmurze i mapy są wyłączone.
# What holds the network off when it is not the switch. $file is the policy file's path.
privacy-held-policy = Ustawione przez administratora w { $file }. Ten przełącznik nie może ponownie włączyć sieci.
# $what is --no-network or SPECULAR_NO_NETWORK.
privacy-held-run = Ustawione dla tego uruchomienia przez { $what }. Ten przełącznik nie może ponownie włączyć sieci.

## Open address… (File)

# machine-drafted, needs native review
address-title = Otwórz adres
# machine-drafted, needs native review
address-intro = Magazyn Zarr, GeoTIFF w chmurze (.tif), chmura punktów COPC (.copc.laz), element lub katalog STAC (.json) lub serwer DICOMweb.
address-label = Adres (http, https lub s3)
address-recent = Ostatnie
address-open = Otwórz
address-network-off = Nie można otwierać adresów: sieć jest wyłączona w Ustawienia › Prywatność.
address-problem-empty = Wpisz lub wklej adres.
address-problem-scheme = Adres musi zaczynać się od http://, https:// lub s3://.
address-problem-host = Adres nie zawiera nazwy serwera lub zawiera spacje.
# machine-drafted, needs native review
address-problem-kind = { -app-name } nie otwiera takiego adresu. Otwiera magazyn .zarr, plik .tif, .copc.laz, plik .json STAC lub serwer DICOMweb.
