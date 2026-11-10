# machine-drafted, needs native review
# Specular — Downloads: what is being fetched from a server (shown beside the toolbar, in
# the gallery's bottom bar, or as a small indicator) and the panel listing it. Українська.

## The panel

# The panel's title.
net-title = Завантаження
# The palette entry that opens the panel, and the indicator's tooltip.
net-show = Показати завантаження
# The panel when nothing is downloading any more.
net-empty = Нічого не завантажується.
# Cancels one download.
net-stop = Зупинити

## What is downloading (row titles). $host is a server's name ("tiles.example.org").

net-what-data = Дані з { $host }
# Tiles of a remote image (a cloud GeoTIFF, an OMERO image).
net-what-image-tiles = Плитки зображення з { $host }
net-what-point-cloud = Хмара точок з { $host }
net-what-map-tiles = Плитки мапи з { $host }
# A map service's capabilities, features or map images.
net-what-map-service = Сервіс мап на { $host }
net-what-dicom = Дані DICOM з { $host }
# $series is the series' description ("CT Abdomen 5 mm").
net-what-dicom-series = Серія DICOM { $series } з { $host }
# Studies being sent to a PACS; { $host } is its host and port.
# machine-drafted, needs native review
net-what-dicom-send = Надсилання досліджень на { $host }
net-what-omero = Дані OMERO з { $host }
net-what-zarr-chunks = Фрагменти Zarr з { $host }
net-what-zarr-metadata = Метадані Zarr з { $host }

## Remote Zarr stores ($address: the store's address, without any query or password).

zarr-open-failed = Не вдалося відкрити { $address }: { $reason }
# A Local AI model; $name is its name.
net-what-model = Модель { $name }
net-what-update = Перевірка оновлень

## Figures

# How much of how much: $done and $total are formatted sizes ("45 MB", "120 MB").
net-bytes-of = { $done } з { $total }
# A speed: $size is a formatted size ("2.1 MB").
net-speed = { $size }/s
# Many requests: $done and $total are formatted numbers; $count is $total, plain, for the
# plural.
net-requests = { $done } з { $total } { $count ->
    [one] запиту
    [few] запитів
    [many] запитів
   *[other] запиту
}
# A DICOM series retrieved image by image: $done and $total are formatted numbers;
# $count is $total, plain, for the plural.
# machine-drafted, needs native review
net-images = { $done } з { $total } { $count ->
    [one] зображення
    [few] зображень
    [many] зображень
   *[other] зображення
}
# Images the server could not send ($n formatted, $count plain).
# machine-drafted, needs native review
net-images-failed = не вдалося: { $n }
# Images sent with a warning.
# machine-drafted, needs native review
net-images-warning = { $n } { $count ->
    [one] попередження
    [few] попередження
    [many] попереджень
   *[other] попередження
}
# Two parts of a line joined ("45 MB of 120 MB · 2.1 MB/s").
net-join = { $first } · { $second }

## Status

# Running, the speed not measured yet.
net-starting = Запуск…
# Running, no time left known (many requests, or an unknown size).
net-running = Завантаження
net-done = Готово
# Done, but $count of its requests failed.
net-done-failed = Готово, невдалих: { $count }
# Cancelled with Stop.
net-stopped = Зупинено
# $reason is one of the reasons below.
net-failed = Не вдалося: { $reason }
net-reason-offline = немає з’єднання
net-reason-timeout = сервер не відповів вчасно
# $code is the HTTP status number.
net-reason-status = сервер відповів { $code }
net-reason-auth = вхід відхилено
net-reason-other = завантаження обірвалося

## The indicator

# More downloads than shown in place.
net-more = ще { $count }
# The indicator's name for screen readers: $status is the newest download and its figures.
net-chip-name = Завантаження: { $status }

## Announcements (screen readers)

# $what is a row title ("Model clip-vit-b32").
net-started = Завантаження: { $what }
net-finished = Завантажено: { $what }
net-failed-announce = Не вдалося завантажити: { $what }
net-stopped-announce = Завантаження зупинено: { $what }

## Без мережі (Налаштування › Приватність)

# machine-drafted, needs native review
# Shown where a network feature would be (update check, model downloads, servers, maps).
network-off = Мережу вимкнено в Налаштування › Приватність.
# The panel under the switch while the network is off.
privacy-off-title = Мережу вимкнено.
privacy-off-what = Перевірку оновлень, завантаження моделей, сервери DICOM, OMERO і Zarr, хмарні зображення й мапи вимкнено.
# What holds the network off when it is not the switch. $file is the policy file's path.
privacy-held-policy = Встановлено адміністратором у { $file }. Цей перемикач не може знову ввімкнути мережу.
# $what is --no-network or SPECULAR_NO_NETWORK.
privacy-held-run = Встановлено для цього запуску через { $what }. Цей перемикач не може знову ввімкнути мережу.

## Open address… (File)

# machine-drafted, needs native review
address-title = Відкрити адресу
# machine-drafted, needs native review
address-intro = Сховище Zarr, хмарний GeoTIFF (.tif), хмара точок COPC (.copc.laz), елемент або каталог STAC (.json) або сервер DICOMweb.
address-label = Адреса (http, https або s3)
address-recent = Нещодавні
address-open = Відкрити
address-network-off = Не вдається відкривати адреси: мережу вимкнено в Налаштування › Приватність.
address-problem-empty = Введіть або вставте адресу.
address-problem-scheme = Адреса має починатися з http://, https:// або s3://.
address-problem-host = В адресі немає імені сервера або є пробіли.
# machine-drafted, needs native review
address-problem-kind = { -app-name } не відкриває таку адресу. Він відкриває сховище .zarr, файл .tif, .copc.laz, файл .json STAC або сервер DICOMweb.
