# machine-drafted, needs native review
# Specular — Downloads: what is being fetched from a server (shown beside the toolbar, in
# the gallery's bottom bar, or as a small indicator) and the panel listing it. Türkçe.

## The panel

# The panel's title.
net-title = İndirmeler
# The palette entry that opens the panel, and the indicator's tooltip.
net-show = İndirmeleri göster
# The panel when nothing is downloading any more.
net-empty = İndirilen bir şey yok.
# Cancels one download.
net-stop = Durdur

## What is downloading (row titles). $host is a server's name ("tiles.example.org").

net-what-data = { $host } kaynağından veri
# Tiles of a remote image (a cloud GeoTIFF, an OMERO image).
net-what-image-tiles = { $host } kaynağından görüntü döşemeleri
net-what-point-cloud = { $host } kaynağından nokta bulutu
net-what-map-tiles = { $host } kaynağından harita döşemeleri
# A map service's capabilities, features or map images.
net-what-map-service = { $host } üzerindeki harita hizmeti
net-what-dicom = { $host } kaynağından DICOM verisi
# $series is the series' description ("CT Abdomen 5 mm").
net-what-dicom-series = { $host } kaynağından DICOM serisi { $series }
# Studies being sent to a PACS; { $host } is its host and port.
# machine-drafted, needs native review
net-what-dicom-send = { $host } sunucusuna tetkik gönderimi
net-what-omero = { $host } kaynağından OMERO verisi
net-what-zarr-chunks = { $host } kaynağından Zarr parçaları
net-what-zarr-metadata = { $host } kaynağından Zarr meta verileri

## Remote Zarr stores ($address: the store's address, without any query or password).

zarr-open-failed = Açılamıyor: { $address }: { $reason }
# A Local AI model; $name is its name.
net-what-model = Model { $name }
net-what-update = Güncelleme denetimi

## Figures

# How much of how much: $done and $total are formatted sizes ("45 MB", "120 MB").
net-bytes-of = { $done } / { $total }
# A speed: $size is a formatted size ("2.1 MB").
net-speed = { $size }/s
# Many requests: $done and $total are formatted numbers; $count is $total, plain, for the
# plural.
net-requests = { $done } / { $total } { $count ->
    [one] istek
   *[other] istek
}
# A DICOM series retrieved image by image: $done and $total are formatted numbers;
# $count is $total, plain, for the plural.
# machine-drafted, needs native review
net-images = { $done } / { $total } { $count ->
    [one] görüntü
   *[other] görüntü
}
# Images the server could not send ($n formatted, $count plain).
# machine-drafted, needs native review
net-images-failed = { $n } başarısız
# Images sent with a warning.
# machine-drafted, needs native review
net-images-warning = { $n } { $count ->
    [one] uyarı
   *[other] uyarı
}
# Two parts of a line joined ("45 MB of 120 MB · 2.1 MB/s").
net-join = { $first } · { $second }

## Status

# Running, the speed not measured yet.
net-starting = Başlatılıyor…
# Running, no time left known (many requests, or an unknown size).
net-running = İndiriliyor
net-done = Bitti
# Done, but $count of its requests failed.
net-done-failed = Bitti, { $count } başarısız
# Cancelled with Stop.
net-stopped = Durduruldu
# $reason is one of the reasons below.
net-failed = Başarısız: { $reason }
net-reason-offline = bağlantı yok
net-reason-timeout = sunucu zamanında yanıt vermedi
# $code is the HTTP status number.
net-reason-status = sunucu { $code } yanıtını verdi
net-reason-auth = oturum açma reddedildi
net-reason-other = indirme kesildi

## The indicator

# More downloads than shown in place.
net-more = { $count } tane daha
# The indicator's name for screen readers: $status is the newest download and its figures.
net-chip-name = İndirmeler: { $status }

## Announcements (screen readers)

# $what is a row title ("Model clip-vit-b32").
net-started = İndiriliyor: { $what }
net-finished = İndirildi: { $what }
net-failed-announce = İndirme başarısız: { $what }
net-stopped-announce = İndirme durduruldu: { $what }

## Ağ yok (Ayarlar › Gizlilik)

# machine-drafted, needs native review
# Shown where a network feature would be (update check, model downloads, servers, maps).
network-off = Ağ, Ayarlar › Gizlilik bölümünde kapalı.
# The panel under the switch while the network is off.
privacy-off-title = Ağ kapalı.
privacy-off-what = Güncelleme denetimleri, model indirmeleri, DICOM, OMERO ve Zarr sunucuları, bulut görüntüleri ve haritalar kapalı.
# What holds the network off when it is not the switch. $file is the policy file's path.
privacy-held-policy = Yöneticiniz tarafından { $file } içinde ayarlandı. Bu anahtar ağı yeniden açamaz.
# $what is --no-network or SPECULAR_NO_NETWORK.
privacy-held-run = Bu çalıştırma için { $what } ile ayarlandı. Bu anahtar ağı yeniden açamaz.

## Open address… (File)

# machine-drafted, needs native review
address-title = Adres aç
# machine-drafted, needs native review
address-intro = Bir Zarr deposu, buluttaki bir GeoTIFF (.tif), bir COPC nokta bulutu (.copc.laz), bir STAC öğesi veya kataloğu (.json) ya da bir DICOMweb sunucusu.
address-label = Adres (http, https veya s3)
address-recent = Son kullanılanlar
address-open = Aç
address-network-off = Adresler açılamıyor: ağ, Ayarlar › Gizlilik bölümünde kapalı.
address-problem-empty = Bir adres yazın veya yapıştırın.
address-problem-scheme = Adres http://, https:// veya s3:// ile başlamalıdır.
address-problem-host = Adreste sunucu adı yok veya boşluk var.
# machine-drafted, needs native review
address-problem-kind = Bu, { -app-name } uygulamasının açtığı bir adres değil: bir .zarr deposu, bir .tif, bir .copc.laz, bir STAC .json dosyası veya bir DICOMweb sunucusu.
