# machine-drafted, needs native review
# Specular — galeri (küçük resim modu, DESIGN §10) ve Ayarlar → Galeri.

## Palet ve menüler

palette-gallery = Galeri
gallery-palette-view = Galeri görünümü…
gallery-palette-layout = Düzen
gallery-palette-sort = Sırala
gallery-palette-group = Grupla
gallery-palette-filter = Filtrele…
gallery-palette-quick-filters = Hızlı filtreler
gallery-palette-details = Ayrıntılar görünümü
gallery-palette-info-overlay = Küçük resimlerde bilgi
gallery-palette-stacks = RAW+JPEG ve seri çekimleri yığınla
gallery-palette-subfolders = Alt klasörleri dahil et
gallery-palette-compare = Seçilenleri karşılaştır
gallery-palette-rate = Puanla…
gallery-palette-settings = Galeri ayarları…
gallery-view-menu = Görünüm
gallery-more-menu = Daha fazla
gallery-open = Aç
gallery-new-window = Yeni pencerede aç
gallery-reveal = Klasörde göster
gallery-properties = Özellikler
gallery-copy-paths = Yolları kopyala
gallery-select-all = Tümünü seç
gallery-select-none = Hiçbirini seçme
gallery-select-invert = Seçimi tersine çevir
gallery-next-unrated = Sonraki puansız
gallery-stack-toggle = Yığını genişlet veya daralt
gallery-run-on = { $name } ({ $count } dosya)
gallery-depth-off = Yalnızca bu klasör
gallery-depth-all = Tüm alt klasörler
gallery-depth-levels = { $depth } alt klasör düzeyi

## Durum ve boş durumlar

gallery-items = { $count } öğe
gallery-selected = { $count } / { $total } seçili
gallery-shown-of = { $count } / { $total } gösteriliyor
gallery-listing = Klasör okunuyor… { $count }
gallery-empty = Bu klasörde görüntü yok.
gallery-empty-filter = Filtreyle eşleşen görüntü yok.
gallery-list-error = Bu klasör okunamıyor: { $reason }
gallery-no-folder = Açık klasör yok
gallery-kind-folder = Klasör
gallery-drag-count = { $count } dosya
gallery-badge-pages = { $count } sayfa

## Filtre çubuğu

# machine-drafted, needs native review
# The filter field takes words (matched against names) and field:value terms; the field
# names and words of the query (rating, camera, ext, is:pick, …) stay in English.
gallery-filter-query-placeholder = Ad veya rating>=3 camera:X-T5 …
gallery-filter-clear = Filtreyi temizle
gallery-filter-rating = ★ { $stars } veya üzeri
gallery-filter-label = Etiket: { $label }
gallery-filter-picks = Seçilenler
gallery-filter-hide-rejects = Reddedilenleri gizle
gallery-filter-unrated = Puansız
gallery-filter-raw = RAW
gallery-filter-animated = Animasyonlu
gallery-filter-hdr = HDR
gallery-filter-landscape = Yatay
gallery-filter-portrait = Dikey

## Query errors and facets

gallery-query-unknown-field = Bilinmeyen alan “{ $field }”
gallery-query-unknown-field-suggest = Bilinmeyen alan “{ $field }”: “{ $suggestion }” mi demek istediniz?
gallery-query-missing-value = “{ $field }” bir değer gerektirir
gallery-query-bad-number = “{ $value }” bir sayı değil ({ $field })
gallery-query-bad-date = “{ $value }” bir tarih değil ({ $field }): 2024, 2024-05 veya 2024-05-12 kullanın
gallery-query-bad-word = “{ $value }”, { $field } için uygun değil: { $words } kullanın
# machine-drafted, needs native review
gallery-query-bad-exif-tag = “{ $value }” bir Exif etiketi adı değil
gallery-query-no-compare = { $field } burada < veya > ile karşılaştırılamaz
gallery-query-unterminated = Kapanış tırnağı eksik
gallery-query-unavailable = “{ $field }” Kitaplık gerektirir (burada kullanılamaz)
gallery-facets = Özellikler
gallery-facets-counting = Sayılıyor…
gallery-facets-empty = Sayılacak bir şey yok
gallery-facets-search = Özelliklerde ara
gallery-facets-no-match = Eşleşen değer yok
# The gallery's top bar button (its tooltip): back to the start screen.
gallery-home = Başlangıç ekranı
gallery-facet-format = Biçim
gallery-facet-camera = Kamera
gallery-facet-lens = Objektif
gallery-facet-rating = Puan
gallery-facet-year = Yıl
gallery-facet-orientation = Yön
gallery-facet-label = Etiket
gallery-facet-properties = Nitelikler
gallery-facet-square = Kare
gallery-facet-alpha = Saydamlık
gallery-facet-gps = Konum
gallery-facet-multipage = Birden çok sayfa
# Screen readers: a facet value, e.g. “Camera: X-T5, 12 files”.
gallery-facet-a11y-value = { $facet }: { $value }, { $count ->
    [one] { $count } dosya
   *[other] { $count } dosya
}

## Gruplar

# An event group's header: the first and the last day of a shooting session.
gallery-group-event = { $start } – { $end }
gallery-group-event-place = { $place } · { $dates }
gallery-group-this-folder = Bu klasör
gallery-group-unknown = Bilinmiyor
gallery-group-unrated = Puansız

## Ayıklama

gallery-stars = { $stars } yıldız
gallery-rate-none = Puan yok
gallery-pick = Seç
gallery-reject = Reddet
gallery-unflag = İşaret yok
gallery-label-red = Kırmızı
gallery-label-yellow = Sarı
gallery-label-green = Yeşil
gallery-label-blue = Mavi
gallery-label-none = Etiket yok
gallery-compare-hint = Karşılaştırmak için 2 ile 4 arası görüntü seçin.

## Dosyalar

gallery-trash-count = { $count } dosyayı çöp kutusuna taşı
gallery-moved-to = { $count } dosya taşındı: { $folder }
gallery-copied-to = { $count } dosya kopyalandı: { $folder }
gallery-move-failed = Taşınamadı: { $file }: { $reason }
gallery-drop-move = Taşı: { $folder }
gallery-drop-copy = Kopyala: { $folder }
gallery-move-here = Seçilenleri buraya taşı
gallery-copy-here = Seçilenleri buraya kopyala
gallery-not-an-image = { $file }, { -app-name }'ın açabileceği bir görüntü değil.
gallery-cache-cleared = Küçük resim önbelleği temizlendi
# machine-drafted, needs native review
gallery-thumb-failed = Küçük resim yok: { $reason }
gallery-cache-clear-failed = Küçük resim önbelleği temizlenemedi: { $reason }

## Özellikler

gallery-prop-name = Ad
gallery-prop-folder = Klasör
gallery-prop-size = Dosya boyutu
gallery-prop-dimensions = Boyutlar
gallery-prop-format = Biçim
gallery-prop-date = Tarih
gallery-prop-camera = Kamera
gallery-prop-lens = Objektif
gallery-prop-rating = Puan
gallery-prop-label = Etiket
gallery-prop-items = Öğeler

## Ekran okuyucular

gallery-a11y-list = { $folder }, { $count } öğe
gallery-a11y-dims = { $width } × { $height }
gallery-a11y-folder = klasör
gallery-a11y-up = { $folder } klasörüne çıkıldı
gallery-a11y-archive = arşiv
gallery-a11y-stack = { $count } öğelik yığın
gallery-a11y-filter-bar = Filtre
gallery-a11y-filter-field = Ada veya alana göre filtrele
gallery-a11y-chip = Filtre ifadesi { $term }
gallery-a11y-chip-remove = Kaldırmak için basın
gallery-a11y-results = { $count ->
    [0] Eşleşen öğe yok
    [one] { $count } öğe eşleşiyor
   *[other] { $count } öğe eşleşiyor
}

## Ayarlar → Galeri

settings-section-gallery = Galeri
settings-gallery-layout = Düzen
settings-gallery-layout-help = Kareler küçük resimleri bir ızgaraya kırpar; hizalı satırlar her görüntünün şeklini korur; kırpılmamış, görüntülerin tamamını eşit hücrelere sığdırır.
settings-gallery-layout-squares = Kareler
settings-gallery-layout-justified = Hizalı satırlar
settings-gallery-layout-uncropped = Kırpılmamış
settings-gallery-layout-details = Ayrıntılar
settings-gallery-thumbnail-size = Küçük resim boyutu
settings-gallery-thumbnail-size-help = Galeride Ctrl+fare tekerleği, kıstırma veya Ctrl+artı ve eksi ile de değiştirilebilir.
settings-gallery-captions = Dosya adlarını göster
settings-gallery-captions-help = Her küçük resmin altında bir başlık.
settings-gallery-badges = Rozetleri göster
settings-gallery-badges-help = RAW, animasyonlu, HDR, çok sayfalı, puan ve etiket için küçük işaretler.
settings-gallery-sort-order = Sırala
settings-gallery-sort-order-help = Galerinin sırası. Klasörler her zaman önce gelir.
settings-gallery-sort-order-name = Ad
settings-gallery-sort-order-date-taken = Çekim tarihi
settings-gallery-sort-order-modified = Değiştirilme tarihi
settings-gallery-sort-order-size = Dosya boyutu
settings-gallery-sort-order-type = Dosya türü
settings-gallery-sort-order-dimensions = Boyutlar
settings-gallery-sort-descending = Ters sıra
settings-gallery-sort-descending-help = Sondan başa doğru sırala.
settings-gallery-group-by = Grupla
settings-gallery-group-by-help = Kaydırırken üstte kalan bölüm başlıkları.
settings-gallery-group-by-none = Hiçbiri
settings-gallery-group-by-day = Gün
settings-gallery-group-by-month = Ay
settings-gallery-group-by-year = Yıl
settings-gallery-group-by-event = Etkinlik
settings-gallery-group-by-folder = Klasör
settings-gallery-group-by-camera = Kamera
settings-gallery-group-by-format = Biçim
settings-gallery-group-by-rating = Puan
settings-gallery-group-by-target = Hedef
settings-gallery-group-by-filter = Filtre
settings-gallery-per-folder-view = Her klasör için görünümü hatırla
settings-gallery-per-folder-view-help = Düzen, boyut, sıralama ve gruplama klasör başına saklanır; aksi takdirde her yerde tek bir görünüm kullanılır.
settings-gallery-open-folders-in-gallery = Klasörleri galeride aç
settings-gallery-open-folders-in-gallery-help = Bir klasör açtığınızda ilk görüntü yerine küçük resimlerle başla.
settings-gallery-escape-opens-gallery = Esc galeriyi açar
settings-gallery-escape-opens-gallery-help = Kapatılacak başka bir şey olmadığında, görüntüleyicide Esc hiçbir şey yapmamak yerine galeriye gider.
settings-gallery-wrap-around = Ok tuşları başa döner
settings-gallery-wrap-around-help = Listenin sonunda ok tuşları başa, baştayken de sona gider. Basılı tutulan bir tuş yine sonda durur.
settings-gallery-show-hidden = Gizli dosyaları göster
settings-gallery-show-hidden-help = Adları nokta ile başlayan dosya ve klasörleri dahil et.
settings-gallery-show-other-files = Diğer dosyaları göster
settings-gallery-show-other-files-help = { -app-name }'ın açamadığı dosyaları genel bir simgeyle göster.
settings-gallery-show-archives = Arşivleri göster
settings-gallery-show-archives-help = Çizgi romanları ve diğer arşivleri (zip, cbz, rar, 7z…) döşeme olarak dahil et. Bir arşivi doğrudan açmak yine çalışır.
settings-gallery-subfolder-depth = Alt klasör düzeyleri
settings-gallery-subfolder-depth-help = Alt klasörlerdeki görüntüleri de bu kadar düzey derinliğe kadar göster (0: yalnızca bu klasör).
settings-gallery-stacks = İlişkili dosyaları yığınla
settings-gallery-stacks-help = RAW+JPEG çiftlerini ve seri çekimleri tek bir döşeme olarak göster. Bir yığını genişletmek için S tuşuna basın.
settings-gallery-info-overlay = Küçük resimlerde bilgi
settings-gallery-info-overlay-help = Her küçük resmin üzerinde boyutlar ve pozlama.
settings-gallery-prefer-embedded = Gömülü önizlemeleri tercih et
settings-gallery-prefer-embedded-help = Yeterince büyükse dosyada saklanan önizlemeyi kullan. Çok daha hızlı; biraz daha az kesin.
settings-gallery-sharper-raw-thumbnails = Daha net RAW küçük resimleri
settings-gallery-sharper-raw-thumbnails-help = Düzenlenen RAW fotoğrafların küçük resimleri 1024 px'te işlenir. Daha net, biraz daha yavaş. Kapalı: 512 px.
settings-gallery-rating-sidecars = Puanları XMP yan dosyalarına yaz
settings-gallery-rating-sidecars-help = Puanlar, etiketler ve işaretler, diğer fotoğraf uygulamalarının okuduğu, görüntünün yanındaki bir .xmp dosyasına da yazılır.
settings-gallery-cache-size = Küçük resim önbelleği boyutu
settings-gallery-cache-size-help = Önbellek dolduğunda en eski küçük resimler kaldırılır.
settings-gallery-cache-location = Küçük resim önbelleği konumu
settings-gallery-cache-location-help = Hızlı bir sürücüdeki klasör en iyi sonucu verir.
settings-gallery-clear-cache = Küçük resim önbelleğini temizle

## Compare
# machine-drafted, needs native review

compare-side = Yan yana
compare-overlay = Bindirme
compare-difference = Fark
compare-flicker = Titreşim
compare-blend = Karıştır
compare-kind-absolute = Mutlak hata
compare-kind-squared = Karesel hata
compare-kind-relative = Göreli hata
compare-scale-down = Daha az hassas
compare-scale-up = Daha hassas
compare-scale = Tam ölçek { $value }
compare-space = Doğrusal ışık, gösterildiği gibi
compare-space-help = Hatalar ve ölçümler, görüntü gösterildiği gibi doğrusal ışıkta, ton eşlemeden sonra, alfa dahil kanal başına ölçülür.
compare-metrics = MAE { $mae }  ·  MSE { $mse }  ·  PSNR { $psnr } dB
compare-sampled = { $width } × { $height } ile örneklendi
compare-aligned = Sol üste hizalandı, { $width } × { $height } karşılaştırıldı
compare-vs = { $b } ile { $a } karşılaştırması
compare-overlay-label = { $b }, { $a } üzerinde, { PERCENT($percent) }
compare-pending = Ölçülüyor…
# machine-drafted, needs native review
compare-diff-failed = Fark gösterilemiyor: { $reason }
compare-no-overlap = Görünümde karşılaştırılacak bir şey yok
compare-zoom-fit = Sığdır
compare-zoom = { PERCENT($percent) }
compare-showing = Gösterilen: { $name }
compare-a11y-group = Karşılaştır
compare-a11y-blend = Görüntüler arasında karıştır
compare-a11y-zoom = Yakınlaştırma
gallery-palette-compare-side = Yan yana karşılaştır
gallery-palette-compare-overlay = Bindirme olarak karşılaştır
gallery-palette-compare-difference = Farkı karşılaştır: { $kind }
gallery-palette-compare-flicker = Karşılaştırılan görüntüler arasında titret
gallery-palette-compare-actual = { PERCENT(100) }'de karşılaştır

## Search value list (machine-drafted, needs native review)
gallery-picker-hint = Boşluk veya Ctrl+Enter birden çok seçer

## Search value picker, part 2 (machine-drafted, needs native review)
gallery-facet-profile = Renk profili
gallery-facet-date = Çekim tarihi
gallery-facet-picked = Seçilenler
gallery-facet-rejected = Reddedilenler
gallery-facet-flagged = Seçilen veya reddedilenler
gallery-facet-rated = Puanlananlar
gallery-facet-labelled = Etiketliler

## Search value picker, part 3 (machine-drafted, needs native review)

gallery-facet-bits = Bit derinliği
gallery-facet-pages = Sayfalar
gallery-facet-width = Genişlik
gallery-facet-height = Yükseklik
gallery-facet-megapixels = Megapiksel
gallery-picker-bits = { $bits } bit
gallery-picker-pages = { $pages ->
   *[other] { $pages } sayfa
}
gallery-picker-pixels = { $px } piksel
gallery-picker-megapixels = { $mp } MP
gallery-picker-group-any = Bir gruptaki herhangi biri
gallery-picker-group-keep = Öneri: tut
gallery-picker-group-discard = Öneri: at
gallery-picker-ops = Karşılaştır
gallery-picker-op-eq = Eşit
gallery-picker-op-ge = En az
gallery-picker-op-le = En çok
gallery-picker-ge = en az { $value }
gallery-picker-gt = { $value } üzeri
gallery-picker-le = en çok { $value }
gallery-picker-lt = { $value } altı

# Map view.
gallery-facet-place = Yer
gallery-palette-map = Harita
map-title = Harita
map-summary = Konumu olan { $count } fotoğraf
map-photos = { $count } fotoğraf
map-zoom-in = Yakınlaştır
map-zoom-out = Uzaklaştır
map-show-all = Tümünü göster
map-done = Bitti
map-credit = Harita verileri: Natural Earth
# machine-drafted, needs native review
# With a base map's tiles on screen: $credit is the tile source's credit ("© OpenStreetMap
# contributors").
map-credit-tiles = Harita: { $credit } · Natural Earth
# The one-time explanation before Specular downloads map tiles from a server; one idea per
# line. $host is the server's name, $size the cache size ("256 MB").
map-basemap-consent-title = Çevrimiçi harita döşemeleri
map-basemap-consent-server = Harita döşemeleri { $host } sunucusundan gelir.
map-basemap-consent-sees = Bu sunucu internet adresinizi ve baktığınız bölgeleri görür.
map-basemap-consent-private = Fotoğraflarınız, adları ve konumları gönderilmez.
map-basemap-consent-cache = Döşemeler bu bilgisayarda { $size } boyutuna kadar saklanır.
map-basemap-allow = Çevrimiçi haritayı kullan
map-basemap-decline = Çevrimdışı haritada kal
# Why the base map is not shown; $reason is one of the lines below or a file error.
map-basemap-error = Altlık harita gösterilmiyor: { $reason }
map-basemap-bad-address = döşeme sunucusu adresinde {"{"}z{"}"}, {"{"}x{"}"} ve {"{"}y{"}"} olmalı.
map-basemap-unknown-tiles = harita dosyasının döşemeleri bilinmeyen bir türde.
map-loading = Fotoğraf konumları okunuyor…
map-empty = Bu fotoğrafların hiçbirinde konum yok
map-hidden = Fotoğraf konumları gizli (Ayarlar › Meta veriler ve gizlilik)
map-a11y-group = Fotoğraf konumları haritası
map-a11y-status = { $count } yer gösteriliyor, yakınlaştırma düzeyi { $zoom }
