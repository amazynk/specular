# machine-drafted, needs native review
# Specular — Maps tools: terrain (slope, aspect, contours, 3-D), time, wind and weather colours,
# online map services, photos and geotagging along GPS tracks, and map export.
# Türkçe.

## Cheat sheet groups of the Maps tab.
cheatsheet-map-terrain = Arazi
cheatsheet-map-time = Zaman ve hava
cheatsheet-map-online = Çevrim içi hizmetler
cheatsheet-map-photos = Fotoğraflar ve GPS
cheatsheet-map-output = Çıktı

## Palette entries and their one-line descriptions.
palette-map-slope = Eğim renkleri
palette-map-slope-help = Yükseklik modelini ne kadar dik olduğuna göre renklendirir.
palette-map-aspect = Bakı renkleri
palette-map-aspect-help = Yükseklik modelini yamaçların baktığı yöne göre renklendirir.
palette-map-contours = Eş yükselti eğrilerini göster veya gizle
palette-map-contours-help = Yükseklik modeli üzerinde eş yükselti eğrileri, birkaç eğride bir etiketli.
palette-map-terrain-3d = 3B arazi
palette-map-terrain-3d-help = Yükseklik modelini, renkleri üzerine serilmiş bir 3B yüzey olarak gösterir.
palette-map-time = Zaman kaydırıcısını göster veya gizle
palette-map-time-help = Bir ızgaranın zamanları ya da bir izin zamanları boyunca adım adım ilerler.
palette-map-play = Zamanı oynat veya duraklat
palette-map-play-help = Zaman kaydırıcısı kendi kendine, adım adım ilerler.
palette-map-wind = Rüzgâr oklarını göster veya gizle
palette-map-wind-help = Izgaranın u ve v bileşenlerinden rüzgâr hızı ve yönü okları.
palette-map-weather-colours = Sonraki hava durumu renk haritası
palette-map-weather-colours-help = Sıcaklık, yağış, rüzgâr ve basınç için yapılmış renk haritaları.
palette-map-services = Çevrim içi harita hizmetleri…
palette-map-services-help = WMS, WMTS, WFS, OGC API veya STAC sunucularından katman ekler.
palette-map-photos-on-track = İz üzerindeki fotoğrafları göster veya gizle
palette-map-photos-on-track-help = Klasördeki fotoğrafları çekim zamanlarına göre GPS izi boyunca yerleştirir.
palette-map-geotag = Fotoğraflara izden konum ekle…
palette-map-geotag-help = İzin konumlarını klasördeki fotoğrafların XMP yan dosyalarına yazar.
palette-map-export = Haritayı dışa aktar…
palette-map-export-help = Haritayı bir ölçekte, lejant, ölçek çubuğu ve kuzey okuyla PNG veya PDF olarak kaydeder.
palette-map-print = Haritayı ölçekli yazdır…
palette-map-print-help = Haritayı 1:{ NUMBER(25000) } gibi seçilen bir ölçekte yazdırır.
palette-map-copy-extent = Kapsamı kopyala
palette-map-copy-extent-help = Ekrandaki alanı sınırlayıcı kutu ya da WKT olarak kopyalar.

## Why a map command is unavailable.
disabled-no-time = Haritada zaman yok
disabled-no-wind = Haritada rüzgâr bileşenleri (u ve v) yok
disabled-no-track = Zamanlı GPS izi yok
disabled-not-grid = Harita bir değer ızgarası değil

## Settings › Maps and Metadata.
settings-maps-services = Çevrim içi harita hizmetleri
settings-maps-services-help = WMS, WMTS, WFS, OGC API ve STAC sunucularından katman ekler. Kapalı: hiçbir şey indirilmez.
settings-maps-services-allowed = İzin verilen harita sunucuları
settings-maps-services-allowed-help = Neyi gördükleri açıklandıktan sonra kabul ettiğiniz sunucular.
settings-metadata-basemap-service = Bir harita hizmeti katmanı
settings-metadata-basemap-service-help = Haritanın çevrim içi hizmetler panelinde seçilen katman.

## Terrain.
# A compass point, short: n, ne, e, se, s, sw, w, nw.
map-compass = { $point ->
    [ne] KD
    [e] D
    [se] GD
    [s] G
    [sw] GB
    [w] B
    [nw] KB
   *[n] K
}
map-slope-aspect = Eğim { $slope }° · { $direction } yönüne bakıyor ({ $aspect }°)
map-slope-flat = Eğim { $slope }° · düz
map-slope-on = Eğim renkleri
map-aspect-on = Bakı renkleri
map-relief-off = Yükseklik renkleri
map-contours-on = Eş yükselti eğrileri açık
map-contours-off = Eş yükselti eğrileri kapalı
map-legend-slope = Eğim
map-legend-aspect = Bakı: yamaçların baktığı yön
map-legend-degrees = { $degrees }°
map-legend-degrees-more = { $degrees }° ve üstü
map-terrain-3d-on = 3B arazi. Esc haritaya döner.
map-terrain-3d-on-exaggerated = 3B arazi, yükseklikler × { $times }. Esc haritaya döner.
map-terrain-exaggeration = Yükseklikler { $times } kat abartıldı.
map-terrain-drape = Harita renkleri

## Time, weather and wind.
map-time-grid = { $when } · { $steps } adımdan { $step }.
map-time-track = { $when } · başlangıçtan { $elapsed } sonra
map-time-speed = × { $times }
map-time-play = Oynat
map-time-pause = Duraklat
map-weather-map = Renkler: { $name }
map-weather-file = Dosyadaki renkler
map-wind-on = Rüzgâr okları açık
map-wind-off = Rüzgâr okları kapalı
map-wind-at = Rüzgâr { $speed } { $unit }, { $degrees }° yönünden ({ $direction })

## Copy extent.
map-extent-copied = Sınırlayıcı kutu kopyalandı. WKT için yeniden kopyalayın.
map-extent-copied-wkt = Kapsam WKT olarak kopyalandı

## Online map services.
map-services-title = Çevrim içi harita hizmetleri
map-service-off = Çevrim içi harita hizmetleri kapalı.
map-service-off-why = İnternetteki sunuculardan katman eklemek için açın.
map-service-turn-on = Aç
map-service-close = Kapat
map-service-back = Geri
map-service-cancel = İptal
map-service-connect = Bağlan
map-service-allow = İzin ver
map-service-remove = Kaldır
map-service-download = İndir
map-service-list-hint = Enter bir sunucuyu açar. Delete onu kaldırır.
map-service-add = Sunucu ekle…
map-service-add-sub = WMS, WMTS, WFS, OGC API veya STAC
map-service-add-hint = Sunucunun adresi. Enter bağlanır.
map-service-bad-address = Bir web adresi değil (http veya https, kullanıcı adı olmadan).
map-service-server-sub = { $kind } · { $host }
map-service-consent-title = Çevrim içi harita hizmeti
map-service-consent-server = Specular { $host } ile bağlantı kuracak.
map-service-consent-sees = Bu sunucu internet adresinizi ve istediğiniz harita alanını görür.
map-service-consent-private = Dosyalarınız ve adları gönderilmez.
map-service-asking = { $host } soruluyor…
map-service-downloading = { $host } adresinden indiriliyor…
map-service-layers-hint = { $kind } katmanları. Enter birini kullanır.
map-service-no-layers = Sunucu hiç katman listelemiyor.
map-service-offer-tiles = Döşemeler: fotoğraf haritasının altlık haritası olur
map-service-offer-features = Objeler: ekrandaki alan için bu haritaya eklenir
map-service-offer-search = Görüntüler: ekrandaki alanda ara
map-service-refused-mercator = Altlık haritanın gerektirdiği Web Mercator (EPSG:3857) ile sunulmuyor.
map-service-refused-format = PNG veya JPEG görüntü olarak sunulmuyor.
map-service-refused-geojson = Burada okunan biçim olan GeoJSON olarak sunulmuyor.
map-service-items-hint = { $count } görüntü bulundu. Enter birini indirir.
map-service-no-items = Burada görüntü bulunamadı.
map-service-item-cloud = { $when } · { PERCENT($cloud) } bulut
map-service-item-no-cog = Cloud-Optimized GeoTIFF yok
map-service-download-ask = { $host } adresinden { $size } indirilsin mi?
map-service-download-ask-unknown = Bu görüntü { $host } adresinden indirilsin mi? Boyutu bilinmiyor.
map-service-download-cache = Harita önbelleğinde tutulur ve açılır.
map-service-size-mb = { $size } MB
map-service-size-gb = { $size } GB
map-service-too-big = Görüntü { $size }, indirmek için çok büyük.
map-service-opened = İndirilen görüntü açılıyor
map-service-basemap = { $layer } artık fotoğraf haritasının altlık haritası. Fotoğraf haritası ilk indirmeden önce sorar.
map-service-status = Sunucu { $status } yanıtını verdi.
map-service-unknown = Bu sürümün okuduğu bir harita hizmeti değil.
map-service-exception = Sunucunun yanıtı: { $message }
map-service-malformed = Sunucunun yanıtı bozuk: { $reason }
map-service-remove-ask = “{ $server }” kayıtlı sunuculardan kaldırılsın mı?
map-service-remove-detail = Diskteki hiçbir şey kaybolmaz. Haritadaki katmanlar harita kapanana kadar kalır.
map-service-removed = { $server } kaldırıldı
map-service-saved = Kayıtlı harita sunucuları
map-service-saved-none = Henüz yok. Haritanın çevrim içi hizmetler panelinden ekleyin.
map-service-keywords = WMS WMTS WFS OGC API STAC sunucular harita hizmeti

## Photos and geotagging.
map-geotag-title = Fotoğraflara izden konum ekle
map-photos-on = İz üzerinde fotoğraflar
map-photos-off = İz üzerindeki fotoğraflar gizli
map-geotag-offset = Kamera saati: GPS saatine göre { $offset }. ← → bir dakika değiştirir, Shift ile bir saat, Alt ile bir saniye.
map-geotag-reading = Fotoğraflar okunuyor…
map-geotag-summary = { $total } fotoğraftan { $count } tanesi konum alıyor.
map-geotag-sidecars = Konumlar fotoğrafların yanındaki XMP yan dosyalarına yazılır. Fotoğraf dosyaları değişmez.
map-geotag-writing = Yazılıyor…
map-geotag-write = Yan dosyaları yaz…
map-geotag-guess = Farkı tahmin et
map-geotag-will-write = { $position } alır
map-geotag-has-position = Zaten konumu var: olduğu gibi bırakıldı
map-geotag-no-time = Çekim zamanı yok: olduğu gibi bırakıldı
map-geotag-outside = İzin { $offset } dışında çekilmiş: olduğu gibi bırakıldı
map-geotag-gap = İzdeki bir boşlukta (bir konumdan { $seconds } sn): olduğu gibi bırakıldı
map-geotag-nothing = Bu farkta hiçbir fotoğraf konum almıyor.
map-geotag-confirm = Konumlar { $count ->
    [one] { $count } XMP yan dosyasına
   *[other] { $count } XMP yan dosyasına
} yazılsın mı? Fotoğrafların kendisi değişmez.
map-geotag-confirm-ok = Yan dosyaları yaz
map-geotag-done = { $count ->
    [one] { $count } yan dosya yazıldı
   *[other] { $count } yan dosya yazıldı
}
map-geotag-done-failed = { $count } yan dosya yazıldı; { $failed } başarısız: { $reason }
map-geotag-not-sidecar = Bir yan dosya adı değil.

## Export and printing.
map-export-title = Haritayı dışa aktar
map-print-title = Haritayı ölçekli yazdır
map-export-keys = ↑ ↓ seçer, ← → değiştirir, Enter başlatır.
map-export-format = Biçim
map-export-scale = Ölçek
map-export-scale-value = 1:{ $n }
map-export-resolution = Çözünürlük
map-export-dpi = { $dpi } dpi
map-export-paper = Kâğıt
map-export-size = { $width } × { $height } piksel · arazide { $across } × { $down } km
map-export-save = Kaydet…
map-export-print = Yazdır…
map-export-too-large = Bu ölçekte çok büyük: { $width } × { $height } piksel. Daha küçük bir ölçek veya çözünürlük seçin.
map-export-no-scale = Bu haritanın arazide bir ölçeği yok.
map-export-no-gpu = Dışa aktarma için grafik işlemcisi gerekir.
map-export-subtitle = Ölçek { $scale } · { $crs }
map-export-data = Veri: { $sources }
map-export-saved = { $name } kaydedildi
map-export-printing = Yazdırılıyor…
map-export-printed = Yazıcıya gönderildi
map-export-print-failed = Yazdırılamadı: { $reason }
