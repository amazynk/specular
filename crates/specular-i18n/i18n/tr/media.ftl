# machine-drafted, needs native review
# Animasyon oynatma ve okuma modu (arşivler, çizgi romanlar).

disabled-not-animated = Bu görüntü animasyonlu değil
disabled-in-archive = Arşivdeki sayfalar çöp kutusuna taşınamaz

## Komut paleti ve kısayol listesi

palette-play-pause = Animasyonu oynat veya duraklat
palette-next-frame = Sonraki kare
palette-prev-frame = Önceki kare
palette-faster = Daha hızlı oynat
palette-slower = Daha yavaş oynat
palette-normal-speed = Normal oynatma hızı
palette-loop-forever = Animasyonu sürekli döngüye al
palette-right-to-left = Sağdan sola sayfa sırası
# machine-drafted, needs native review
palette-contents = İçindekiler
palette-spreads = Çift sayfa görünümü

## Animasyon denetimleri (araç çubuğunun üstündeki çubuk)

# Oynatma denetimleri grubunun erişilebilirlik adı.
transport-region = Animasyon denetimleri
transport-play = Oynat
transport-pause = Duraklat
transport-prev-frame = Önceki kare
transport-next-frame = Sonraki kare
transport-slower = Daha yavaş
transport-faster = Daha hızlı
# $index gösterilen kare, $count kare sayısıdır.
transport-frame = Kare { $index } / { $count }
# Kare sayısı, animasyon bir kez oynatılana kadar bilinmez.
transport-frame-unknown = Kare { $index }
# $speed biçimlendirilmiş bir sayıdır, ör. 1,5.
transport-speed = { $speed }×
transport-speed-name = Oynatma hızı { $speed }×
# machine-drafted, needs native review
motion-badge = Hareketli
motion-badge-live = Live
motion-play = Hareketli klibi oynat
motion-play-live = Live Photo'yu oynat
motion-stop = Fotoğrafa dön
toast-motion-failed = Hareketli klip oynatılamıyor: { $reason }

## Bildirimler

toast-speed = Oynatma hızı { $speed }×
toast-loop-forever-on = Sürekli döngü
toast-loop-forever-off = Dosyadaki döngü ayarı
toast-right-to-left-on = Sayfalar sağdan sola okunuyor
toast-right-to-left-off = Sayfalar soldan sağa okunuyor
toast-spreads-on = Çift sayfa görünümü
toast-spreads-off = Tek seferde bir sayfa

## Arşivler

# Bir arşiv sayfasının pencere başlığı.
title-archive-page = { $page } — { $archive }
# İki sayfa gösterilirken konum göstergesi.
readout-position-spread = { $first }–{ $last } / { $count }
# $format RAR'dır.
archive-needs-sandbox = { $format } arşivleri yalnızca korumalı kod çözücü ana bilgisayarıyla açılabilir; bu sürümde henüz bulunmuyor.
archive-empty = Bu arşivde görüntü yok.
archive-failed = Arşiv açılamadı: { $error }
archive-encrypted = Bu arşiv şifreli. Parolasını girmek için yeniden açın.
archive-wrong-password = Bu arşivin parolası yanlış.
password-title = Parola gerekli
password-archive = “{ $archive }” şifreli.
password-label = Parola
password-show = Parolayı göster
password-wrong = Parola yanlış. Yeniden deneyin.
password-open = Aç
password-cancel = İptal

## Ayarlar

settings-viewing-animation-autoplay = Animasyonları otomatik oynat
settings-viewing-animation-autoplay-help = Animasyonlu görüntüleri (GIF, APNG, WebP…) açılır açılmaz başlat.
settings-viewing-animation-loop = Animasyon döngüsü
settings-viewing-animation-loop-help = Animasyonların ne sıklıkla tekrarlanacağı.
settings-viewing-animation-loop-file = Dosyadaki gibi
settings-viewing-animation-loop-forever = Sürekli
settings-viewing-animation-loop-once = Bir kez oynat
settings-viewing-animation-short-delays = Çok kısa kare gecikmeleri
settings-viewing-animation-short-delays-help = 10 ms veya daha kısa sürmesi gereken kareler: web tarayıcıları gibi 100 ms'ye yavaşlat veya kaydedildiği gibi oynat.
settings-viewing-animation-short-delays-browser = Web tarayıcıları gibi (100 ms)
settings-viewing-animation-short-delays-as-stored = Kaydedildiği gibi
settings-browsing-reading-order = Arşivlerde sayfa sırası
settings-browsing-reading-order-help = Çizgi roman arşivleri, ComicInfo.xml dosyası öyle diyorsa (manga) sağdan sola okunur; burada ayarlanmadıkça.
settings-browsing-reading-order-auto = Arşivden
settings-browsing-reading-order-left-to-right = Soldan sağa
settings-browsing-reading-order-right-to-left = Sağdan sola
settings-browsing-page-layout = Arşivlerde sayfalar
settings-browsing-page-layout-help = Tek sayfa veya yan yana iki sayfa göster; kapak her zaman tek başına gösterilir.
settings-browsing-page-layout-auto = Arşivden
settings-browsing-page-layout-single = Tek sayfa
settings-browsing-page-layout-spreads = Çift sayfa görünümü

## Pages of multi-image files (ICNS sizes, texture layers, faces and slices, TIFF pages) (machine-drafted, needs native review)
palette-next-page = Sonraki sayfa
palette-prev-page = Önceki sayfa
palette-first-page = İlk sayfa
palette-last-page = Son sayfa
disabled-one-page = Görüntünün yalnızca bir sayfası var
page-indicator = Sayfa { $index }/{ $count }
page-indicator-labelled = { $page }, { $label }
page-label-icon = { $width }×{ $height }
page-label-icon-density = { $width }×{ $height } @{ $density }x
page-label-layer = Katman { $index }
page-label-face = Yüz { $face }
page-label-sphere = Küre haritası
page-label-slice = Kesit { $index }
page-label-join = { $a } · { $b }
page-label-slide = Lam
page-label-slide-label = Etiket
page-label-slide-macro = Genel görünüm
page-label-slide-thumbnail = Küçük resim
# machine-drafted, needs native review
page-label-thermal = Termal görüntü
page-label-visible = Görünür fotoğraf
# machine-drafted, needs native review
page-label-left-view = Sol görünüm
# machine-drafted, needs native review
page-label-right-view = Sağ görünüm
# machine-drafted, needs native review
# An object of a CERN ROOT file in the page picker: its path in the file and its class, e.g.
# "run1/hits (TH2D)".
page-label-object = { $name } ({ $class })
# machine-drafted, needs native review
# A view of one step of a circuit board job ("panel · Top").
page-label-board-step = { $step } · { $view }
# machine-drafted, needs native review
# A focal plane and a time point of a remote microscopy image (OMERO), counted from 1.
page-label-z = Z { $index } / { $count }
page-label-t = T { $index } / { $count }
# machine-drafted, needs native review
# Views of a remote-sensing product (an ENVI cube, a Sentinel-2 product) in the page picker.
page-label-product-composite = Varsayılan bantlar
page-label-product-bands = Tüm bantlar
page-label-product-true-colour = Gerçek renk
page-label-product-false-colour = Yanlış renk (yakın kızılötesi)
page-label-product-swir = Kısa dalga kızılötesi
page-label-product-agriculture = Tarım
page-label-product-scene-class = Sahne sınıflandırması
page-label-product-cloud = Bulut olasılığı
# A product view at a pixel size, e.g. "True colour, 10 m".
page-label-product-at = { $view }, { $metres } m
title-file-page = { $file } — { $page }
info-page-group = Sayfa
info-page-position = Konum
info-page-contents = İçerik

# machine-drafted, needs native review
# OpenColorIO (Settings › Colour): why scene-linear images are shown without it.
ocio-failed = OCIO kullanılmıyor: { $reason }
ocio-builtin-config = $OCIO yerleşik bir yapılandırmayı (ocio://) gösteriyor; bunu dosya olarak kaydedin ve o dosyayı seçin.

## Page labels for icons and collections (machine-drafted, needs native review)
page-label-size = { $width }×{ $height }
page-label-size-bits = { $width }×{ $height } · { $bits } bit

## Page labels for Photoshop layers (machine-drafted, needs native review)
page-label-merged = Birleştirilmiş görüntü
page-label-from-layers = Katmanlardan oluşturuldu
page-label-layer-hidden = { $name } (gizli)
page-label-layer-unnamed = Adsız katman { $index }
