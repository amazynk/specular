# machine-drafted, needs native review
# Specular — görüntüleyici arayüzü, komut paleti, hatalar, Hakkında penceresi. Türkçe.

-app-name = Specular

## Araç çubuğu ipuçları ve erişilebilirlik etiketleri

toolbar-open = Görüntü aç…
toolbar-prev = Önceki görüntü
toolbar-next = Sonraki görüntü
toolbar-back-gallery = Galeriye dön
toolbar-zoom-out = Uzaklaştır
toolbar-zoom-in = Yakınlaştır
toolbar-zoom-level = Yakınlaştırma düzeyi ve hazır ayarlar
toolbar-zoom-fit = Pencereye sığdır
toolbar-zoom-100 = Gerçek boyut ({ PERCENT(100) })
toolbar-rotate-left = Saat yönünün tersine döndür
toolbar-rotate-right = Saat yönünde döndür
toolbar-fullscreen-enter = Tam ekrana geç
toolbar-fullscreen-exit = Tam ekrandan çık
toolbar-info = Görüntü bilgileri
toolbar-settings = Ayarlar
toolbar-delete = Çöp kutusuna taşı
toolbar-copy = Görüntüyü kopyala
toolbar-palette = Komut paleti
toolbar-go-to = Görüntüye git…
toolbar-go-to-range = 1–{ $count }
# machine-drafted, needs native review
toolbar-customise = Araç çubuğunu özelleştir…
toolbar-more = Diğer
toolbar-announce-moved = { $item }: konum { $index } / { $count }
toolbar-announce-added = { $item } eklendi: konum { $index } / { $count }
toolbar-announce-removed = { $item } araç çubuğundan kaldırıldı
toolbar-announce-reset = Araç çubuğu varsayılana sıfırlandı
# Bir ipucu etiketi ve ardından klavye kısayolu.
tooltip-with-shortcut = { $label } ({ $shortcut })
# Şu anda kullanılamayan bir denetimin ipucu; $reason, disabled-* iletilerinden biridir.
tooltip-unavailable = { $label } (kullanılamıyor: { $reason })
# Bir denetimin veya komutun neden kullanılamadığı (ipuçları, komut paleti, ekran okuyucular).
disabled-no-image = Açık görüntü yok
disabled-still-loading = Görüntü hâlâ yükleniyor
disabled-decode-failed = Bu görüntünün kodu çözülemedi
disabled-no-other-images = Bu klasörde başka görüntü yok

## Göstergeler

# Klasördeki konum, ör. "12 / 348".
readout-position = { $index } / { $count }
# Konum göstergesi için ekran okuyucuların seslendirdiği metin.
readout-position-a11y = Görüntü { $index } / { $count }
# $zoom önceden biçimlendirilmiş bir yüzdedir, ör. "%150".
readout-zoom = { $zoom }
readout-zoom-a11y = Yakınlaştırma { $zoom }
readout-zoom-fit = Sığdır ({ $zoom })
zoom-preset-fit = Pencereye sığdır
zoom-preset-fill = Pencereyi doldur
zoom-preset-actual = Gerçek boyut
# machine-drafted, needs native review
# The last entry of the zoom presets: the window fills the screen (ticked while it does).
zoom-preset-fullscreen = Tam ekran
# $zoom önceden biçimlendirilmiş bir yüzdedir.
zoom-preset-percent = { $zoom }
# Piksel cinsinden görüntü boyutları, ör. "6000 × 4000".
readout-dimensions = { $width } × { $height }
# Geçerli klasördeki görüntü sayısı.
status-image-count =
    { $count ->
        [one] { $count } görüntü
       *[other] { $count } görüntü
    }

## Komut paleti

palette-placeholder = Bir komut veya ayar yazın…
palette-no-results = Eşleşen komut yok
# Paletten bir ayarı değiştirme, ör. "Ayarla: Fare tekerleği → Sonraki görüntü".
palette-set-setting = Ayarla: { $setting } → { $value }
palette-next = Sonraki görüntü
palette-prev = Önceki görüntü
palette-first = İlk görüntü
palette-last = Son görüntü
palette-go-to = Görüntüye git…
palette-zoom-in = Yakınlaştır
palette-zoom-out = Uzaklaştır
palette-zoom-fit = Sığacak şekilde yakınlaştır
palette-zoom-fill = Dolduracak şekilde yakınlaştır
# machine-drafted, needs native review
palette-zoom-fit-width = Genişliğe sığdır
palette-zoom-fit-height = Yüksekliğe sığdır
palette-zoom-100 = { PERCENT(100) } yakınlaştır
palette-zoom-200 = { PERCENT(200) } yakınlaştır
palette-zoom-300 = { PERCENT(300) } yakınlaştır
palette-zoom-400 = { PERCENT(400) } yakınlaştır
palette-zoom-rectangle = Seçime yakınlaştır
palette-rotate-left = Saat yönünün tersine döndür
palette-rotate-right = Saat yönünde döndür
palette-flip-horizontal = Yatay çevir
palette-flip-vertical = Dikey çevir
palette-toggle-fullscreen = Tam ekranı aç/kapat
palette-navigator = Gezgini göster veya gizle
palette-loupe = Büyüteç
palette-pixel-grid = Piksel ızgarasını göster veya gizle
palette-info-panel = Bilgi panelini göster veya gizle
palette-copy-image = Görüntüyü kopyala
# machine-drafted, needs native review
palette-read-codes = Kodları oku
# machine-drafted, needs native review
palette-find-text = Metin bul
# machine-drafted, needs native review
palette-face-boxes = Yüzleri göster
palette-copy-path = Dosya yolunu kopyala
palette-move-to-trash = Çöp kutusuna taşı
palette-undo-trash = Çöp kutusuna taşımayı geri al
palette-open = Görüntü aç…
# machine-drafted, needs native review
palette-open-address = Adres aç…
palette-settings = Ayarlar
# machine-drafted, needs native review
palette-customise-toolbar = Araç çubuğunu özelleştir…
palette-cheat-sheet = Klavye kısayolları
palette-hud = Kare süresi göstergesini göster veya gizle
palette-language = Dil
palette-theme = Tema
# machine-drafted, needs native review
palette-interface-larger = Daha büyük arayüz
palette-interface-smaller = Daha küçük arayüz
palette-interface-reset = Arayüz { PERCENT(100) }
palette-quit = { -app-name }'dan çık

## Kısayol listesi

cheatsheet-title = Klavye kısayolları
cheatsheet-navigation = Gezinme
cheatsheet-zoom-pan = Yakınlaştırma ve kaydırma
cheatsheet-view = Görünüm
cheatsheet-file = Dosya
cheatsheet-window = Pencere
cheatsheet-help = Yardım
cheatsheet-hold = Basılı tut: { $key }
cheatsheet-close-hint = Kapatmak için { $key } veya Esc tuşuna basın

## Hatalar

# machine-drafted, needs native review
viewer-no-preview = Bu dosyada önizleme yok.
error-cannot-open = Açılamıyor: { $file }
error-cannot-open-reason = Açılamıyor: { $file }: { $reason }
error-unsupported-format = { $file } dosyasının biçimi henüz { -app-name } tarafından okunamıyor.
error-file-truncated = { $file } dosyası eksik. Okunabilen kısım gösteriliyor.
error-file-too-large = { $file } dosyası açılamayacak kadar büyük ({ $megapixels } megapiksel; sınır { $limit } megapiksel).
error-not-found = { $file } artık mevcut değil.
error-decoder-stopped = kod çözücü bir görüntü üretmeden durdu
# machine-drafted, needs native review
# Shown after "Can’t open FILE:" for brain data (GIFTI .func/.shape/.label, CIFTI) that is
# drawn on a surface: no .surf.gii with that many vertices was found in the same folder.
error-needs-surface = aynı klasörde bir beyin yüzeyi ({ $vertices } köşeli bir .surf.gii dosyası) gerekiyor
error-permission-denied = { $file } dosyasını okuma izniniz yok.
error-details = Ayrıntılar
crash-last-time = { -app-name } geçen sefer beklenmedik şekilde kapandı; bir rapor { $path } konumuna kaydedildi
error-trash-failed = Çöp kutusuna taşınamadı: { $file }: { $reason }
# machine-drafted, needs native review
# "Açılamıyor: FILE:" ve diğer "…amadı:" iletilerinden sonra gösterilen nedenler; bu yüzden küçük harfle başlar.
# Ne olduğunu ve yardımcı olacaksa ne yapılacağını söyler.
error-reason-not-found = artık orada değil. Taşınmış, yeniden adlandırılmış veya silinmiş olabilir.
error-reason-permission = okuma izniniz yok. Sahibini ve izinlerini kontrol edin.
error-reason-disk-full = disk dolu. Biraz yer açıp yeniden deneyin.
error-reason-read-only = disk salt okunur.
error-reason-truncated = dosya eksik. Hâlâ kopyalanıyor veya indiriliyor olabilir.
error-reason-is-folder = bu bir dosya değil, klasör.
# A remote store (a Zarr image on a server or in an S3 bucket).
error-reason-remote-denied = sunucu erişimi reddetti. Veriler özel olabilir; S3 kovası için önce AWS kimlik bilgilerini ayarlayın.
error-reason-remote-missing = bu adreste Zarr verisi yok.
# $status is an HTTP status code ("500").
error-reason-remote-status = sunucu { $status } hatasıyla yanıt verdi.
error-reason-offline = sunucuya ulaşılamıyor. Adresi ve bağlantıyı kontrol edin.
# machine-drafted, needs native review
error-reason-network-off = ağ, Ayarlar › Gizlilik bölümünde kapalı.
error-reason-timed-out = sürücü veya sunucu zamanında yanıt vermedi. Bağlantıyı kontrol edip yeniden deneyin.
error-reason-memory = yeterli bellek yok. Diğer görüntüleri veya uygulamaları kapatıp yeniden deneyin.
error-reason-unsupported = biçimi henüz { -app-name } tarafından okunamıyor.
# $detail kod çözücünün kendi teknik, İngilizce ifadesidir ("12-bit lossless JPEG").
error-reason-unsupported-detail = { -app-name } uygulamasının henüz okuyamadığı bir özellik kullanıyor ({ $detail }).
# machine-drafted, needs native review
# A file whose extension several formats share (".img") and whose content is none of them.
# $formats lists the format names, e.g. "ERDAS IMAGINE, ADSC SMV, Analyze 7.5".
error-reason-not-one-of = { -app-name } uygulamasının okuduğu .{ $ext } dosyalarından ({ $formats }) hiçbiri değil.
error-reason-damaged = dosya hasarlı veya adının belirttiği türde değil ({ $detail }).
error-reason-too-large = açılamayacak kadar büyük.
# machine-drafted, needs native review
error-reason-needs-memory = { $needed } boş RAM gerekiyor ancak şu anda yalnızca { $free } boş. Diğer bazı uygulamaları kapatıp yeniden deneyin veya daha fazla belleği olan bir bilgisayarda açın.
error-needs-memory = Bu görüntü { $needed } boş RAM gerektiriyor ancak şu anda yalnızca { $free } boş. Diğer bazı uygulamaları kapatıp yeniden deneyin veya daha fazla belleği olan bir bilgisayarda açın.
error-needs-memory-short = { $needed } boş RAM gerekiyor
error-reason-cancelled = açma işlemi durduruldu.
error-reason-trash-access = artık orada değil veya değiştirme izniniz yok.
error-reason-restore-collision = klasöründe aynı ada sahip bir dosya zaten var.
error-reason-trash-root = bütün bir sürücü çöp kutusuna taşınamaz.
error-reason-not-in-trash = artık çöp kutusunda değil.
error-reason-program-missing = program yüklü değil veya taşınmış. Yeniden seçin veya yeniden yükleyin.
error-reason-program-not-allowed = programın çalışmasına izin verilmiyor. Yürütülebilir olduğunu kontrol edin.
# machine-drafted, needs native review
error-reason-permission-write = değiştirme izniniz yok. Sahibini ve izinlerini kontrol edin.
error-reason-file-has-errors = hatalar içeriyor. İçindeki hiçbir şey kaybolmasın diye önce bunları düzeltin.
error-reason-not-dicom = bir DICOM dosyası değil.
error-reason-no-image = görüntü içermiyor.
error-reason-needs-layout = başlıksız bir hacim. Açmak için boyutu, örnek türü ve bayt sırası gerekiyor.
error-reason-not-archive = { -app-name } uygulamasının okuyabileceği bir arşiv değil.
error-reason-needs-sandbox = bu tür arşivler yalnızca { -app-name } uygulamasının korumalı okuyucusuyla açılır ve bu okuyucu kullanılamıyor.
error-reason-encrypted = şifrelenmiş ve parolası gerekiyor.
error-reason-wrong-password = parola yanlış.
error-reason-checksum = dosya hasarlı: içeriği sağlama toplamıyla eşleşmiyor.
error-reason-missing-entry = içindeki bir dosya eksik.
error-reason-busy = başka bir program kullanıyor. Orada kapatıp yeniden deneyin.
error-reason-catalogue-damaged = katalog dosyası hasarlı.
error-reason-cannot-open = dosyası açılamıyor. Klasörünün var olduğunu ve içine yazabildiğinizi kontrol edin.
error-reason-newer-version = { -app-name } uygulamasının daha yeni bir sürümüyle yazılmış. Değiştirmek için { -app-name } uygulamasını güncelleyin.
error-reason-internal = { -app-name } içinde bir şeyler ters gitti. Ayrıntılar günlükte.
error-reason-volume-too-few = { $count ->
    [one] bir hacim oluşturmak için bir kesit çok az.
   *[other] bir hacim oluşturmak için { $count } kesit çok az.
}
error-reason-volume-no-geometry = bir kesit nerede durduğunu veya hangi yöne baktığını belirtmiyor.
error-reason-volume-not-parallel = kesitler paralel değil.
error-reason-volume-shared-position = { $count } konumun her birini birden çok görüntü paylaşıyor (fazlar, ekolar veya bir zaman serisi).
error-reason-volume-missing = yaklaşık { $count } kesit eksik.
error-reason-volume-mismatch = kesitlerin boyutu veya piksel aralığı farklı.
error-reason-volume-not-grey = kesitler gri değerlerde değil, renkli.
error-reason-volume-skewed = hacmin eksenleri dik açılı değil.
error-reason-volume-too-large = { $need } MiB gerekiyor, ancak yalnızca { $budget } MiB'a izin veriliyor.
error-reason-url-not-specular = bir { -app-name } bağlantısı değil.
error-reason-url-too-long = çok uzun.
error-reason-url-damaged = hasarlı. Kopyalanırken kesilmiş veya değiştirilmiş olabilir.
error-reason-url-unknown = { -app-name } uygulamasının bu sürümünün bilmediği bir şey istiyor. { -app-name } uygulamasını güncellemek yardımcı olabilir.
error-reason-url-not-absolute = dosya yolları tam yollar olmalıdır.
error-reason-url-no-paths = hiçbir dosya belirtmiyor.
error-reason-url-too-many = çok fazla dosya belirtiyor.

## Bildirimler

# machine-drafted, needs native review
# A background task (a listing, an export, a file operation) stopped part way; $reason is
# error-reason-internal.
background-failed = Tamamlanamadı: { $reason }
toast-moved-to-trash = Çöp kutusuna taşındı
toast-moved-to-trash-named = Çöp kutusuna taşındı: { $file }
toast-moved-to-trash-count =
    { $count ->
        [one] { $count } görüntü çöp kutusuna taşındı
       *[other] { $count } görüntü çöp kutusuna taşındı
    }
toast-undo = Geri al
toast-restored = Geri yüklendi: { $file }
toast-copied-image = Görüntü kopyalandı
toast-copied-path = Yol kopyalandı
toast-copied-report = Rapor kopyalandı
toast-copied-value = Değer kopyalandı
toast-loop-start = İlk görüntüye dönüldü
toast-loop-end = Son görüntüye geçildi
# machine-drafted, needs native review
toast-interface-size = Arayüz boyutu { PERCENT($percent) }
toast-language-changed = Dil değiştirildi: { $language }

## Boş durumlar

empty-drop = Buraya bir görüntü bırakın veya { $shortcut } tuşlarına basın
empty-open-button = Görüntü aç…
empty-drop-release = Açmak için bırakın
empty-folder = Bu klasörde { -app-name } ile açılabilecek görüntü yok.
# machine-drafted, needs native review
# Boş pencerede "Görüntü aç…" altında: diğer her şeyin nerede bulunacağı.
empty-hint-keys = Klavye kısayolları için { $cheatsheet }, herhangi bir komutu bulmak için { $palette } tuşlarına basın.
empty-hint-cheatsheet = Klavye kısayolları için { $cheatsheet } tuşlarına basın.
empty-hint-palette = Herhangi bir komutu bulmak için { $palette } tuşlarına basın.
# Bir dosyanın açılması biraz zaman aldığında dönen simgenin altında.
viewer-opening = { $file } açılıyor…
fullscreen-exit-hint = Tam ekrandan çıkmak için Esc tuşuna basın
# Tam ekrana girerken gösterilir; yakınlaştırılmışsa Esc önce yakınlaştırmayı sıfırlar.
fullscreen-back-hint = Yakınlaştırmayı sıfırlamak veya tam ekrandan çıkmak için Esc tuşuna basın
# Kısayol listesi ve komut paleti: Esc'nin yaptığı, her basışta bir adım.
action-back = Paneli kapat (bir iletişim kutusunda önce bir adım geri), yakınlaştırmayı sıfırla, tam ekrandan çık; galeride bir klasör yukarı

## Dil seçici

language-picker-title = Dil
# machine-drafted, needs native review
language-picker-search = Dil ara
# $percent önceden biçimlendirilmiş bir yüzdedir, ör. "%87".
language-completeness = { $percent } çevrildi
language-system-default = Sistem varsayılanı ({ $language })
language-pseudo-accented = Sözde yerel ayar: aksanlı ve daha uzun
language-pseudo-rtl = Sözde yerel ayar: sağdan sola yansıtılmış

## Hakkında penceresi

about-title = { -app-name } hakkında
about-version = Sürüm { $version }
about-tagline = Hızlı ve aslına sadık bir görüntü görüntüleyici.
about-copyright = © { $year } { -app-name } yazarları
about-license = MIT License / Apache License 2.0 ile lisanslanmıştır.
about-geonames = Yer adları GeoNames'ten (geonames.org), Creative Commons Attribution 4.0 ile lisanslanmıştır.
about-unicode = Yerel ayar verileri ICU4X aracılığıyla Unicode CLDR'den. © Unicode, Inc.
# machine-drafted, needs native review
about-lensfun = Lens profilleri Lensfun veritabanından (lensfun.github.io), Creative Commons Attribution-ShareAlike 3.0 lisansıyla.
about-third-party = Üçüncü taraf lisansları
about-third-party-file = { -app-name }, kendi lisanslarına tabi üçüncü taraf yazılım ve veriler içerir; { -app-name } ile birlikte kurulan THIRD-PARTY.md dosyasına bakın.
about-copy-info = Sistem bilgilerini kopyala
about-close = Kapat

## Görüntüleyici arayüzü (Aşama 1 uygulaması)

app-close = Kapat
# Dosyası erken biten bir görüntüde gösterilen rozet.
app-truncated-badge = Eksik dosya
app-info-summary = Özet
app-info-all-fields = Tüm alanlar
app-info-copy-value = Değeri kopyala
app-info-copy-field = { $field } alanının değerini kopyala
app-info-loading = Meta veriler okunuyor…
# machine-drafted, needs native review
# Tüm alanlar; pikselleri dışında meta veri saklamayan bir dosya için.
app-info-no-fields = Bu dosyada meta veri alanı yok.
# Gain-map row of the info summary (machine-drafted, needs native review)
app-info-gain-map = HDR kazanç haritası
app-info-gain-map-headroom = SDR parlaklığının { $headroom } katına kadar
app-copy-failed = Kopyalanamadı: { $reason }
app-restore-failed = Geri yüklenemedi: { $file }: { $reason }
app-open-dialog-title = Görüntü aç
app-open-dialog-filter = Desteklenen dosyalar
app-open-dialog-all = Tüm dosyalar
app-open-group-images = Görüntüler
app-open-group-raw = Kamera RAW
app-open-group-video = Video
app-open-group-sound = Ses
app-open-group-documents = Belgeler
app-open-group-drawings = Çizimler ve CAD
app-open-group-models = 3B modeller
app-open-group-maps = Haritalar ve rotalar
app-open-group-science = Bilim ve tıp
app-open-group-fonts = Yazı tipleri
app-open-group-other = Diğer

## Oluşturucu bilgileri (Hakkında, Ayarlar › Gelişmiş)

# machine-drafted, needs native review
renderer-title = Oluşturucu bilgileri
renderer-section-adapter = Grafik bağdaştırıcısı
renderer-section-limits = Sınırlar ve özellikler
renderer-section-display = Ekran
renderer-section-memory = Bellek
renderer-section-decoders = İsteğe bağlı kod çözücüler
renderer-unknown = Bilinmiyor
# The PCI vendor and device IDs ("1002:747e").
renderer-ids = Üretici ve aygıt kimliği
# Wayland, X11, Windows or macOS.
renderer-display-server = Görüntü sunucusu
renderer-features = Kullanılan özellikler
renderer-surface-format = Pencere biçimi
renderer-surface-formats = Sunulan biçimler
renderer-present-mode = Sunum kipi
renderer-present-modes = Sunulan kipler
renderer-hdr = HDR
# $headroom is how much brighter than white the display goes ("4.0").
renderer-hdr-on = Açık ({ $headroom }× pay)
renderer-hdr-off = Kapalı (ekran HDR gösterebilir)
renderer-hdr-unavailable = Bu ekranda kullanılamıyor
renderer-decode-budget = Kod çözme şunu kullanabilir
renderer-decode-budget-detail = Boş belleğin { PERCENT(80) }’i, açılmakta olan tüm görüntüler arasında paylaşılır.
renderer-display-cache = Görüntü önbelleği

## Performans göstergesi (F12)

app-hud-scale = Ölçek
app-hud-level = Düzey
app-hud-level-thumbnail = Gömülü küçük resim
app-hud-level-full = Tam çözünürlük
# $denominator 2, 4 veya 8'dir: görüntünün kodu 1/2, 1/4 veya 1/8 boyutunda çözülmüştür.
app-hud-level-scaled = 1/{ $denominator }
app-hud-open = Açma
app-hud-decode = Kod çözme
app-hud-upload = Yükleme
app-hud-frame = Kare
app-hud-threads = İş parçacıkları
app-hud-threads-value = { $threads } ({ $queued } kuyrukta)
app-hud-tiles = Döşemeler
app-hud-tiles-value = { $tiles } / { $capacity }
app-hud-ram = Bellek
app-hud-vram = GPU belleği
app-hud-gpu = GPU

# Çöp kutusu onayı
app-confirm-trash-title = { $file } çöp kutusuna taşınsın mı?
# machine-drafted, needs native review
app-confirm-trash-sidecars = Yan dosyaları da çöpe gider: { $files }
app-cancel = İptal
# Çok büyük bir görüntünün kodu çözülmeden önce gösterilen uyarı. $need ve $available bellek
# boyutlarıdır ("3,2 GiB").
toast-low-memory = Bu görüntü yaklaşık { $need } bellek gerektirebilir, ancak yalnızca { $available } kullanılabilir. Yavaş yüklenebilir veya hiç yüklenmeyebilir.

# Donanım raporu (Ayarlar → Gelişmiş). Değerleri uygulama doldurur.
hw-report-title = Donanım raporu
hw-report-copy = Raporu kopyala
hw-report-version = Sürüm
hw-report-cpu = CPU
hw-report-cores = Çekirdekler
hw-report-cores-value = { $logical } mantıksal, { $physical } fiziksel
hw-report-performance-cores = Performans çekirdekleri
hw-report-cpu-features = CPU hızlı yolları
hw-report-none = Yok
hw-report-memory = Bellek
hw-report-memory-value = { $total } toplam, { $available } kullanılabilir
hw-report-graphics-api = Grafik API'si
hw-report-driver = Sürücü
hw-report-max-texture = En büyük doku
hw-report-pixels = { $width } × { $height } piksel
hw-report-decode-threads = Kod çözme iş parçacıkları
hw-report-ram-cache = Görüntü bellek önbelleği
hw-report-vram-cache = GPU bellek önbelleği
hw-report-processing = İşleme
hw-report-path-jpeg-decode = JPEG kod çözme
hw-report-path-ycbcr-to-rgb = Renk dönüştürme
hw-report-path-png-unfilter = PNG kod çözme
hw-report-path-heic-decode = HEIC kod çözme
hw-report-path-inference = Görüntü analizi
hw-report-engine-cpu = CPU
hw-report-engine-cpu-parallel = CPU, paralel
hw-report-engine-gpu = GPU
hw-report-engine-hw-decoder = donanım kod çözücü
hw-report-engine-npu = NPU
hw-report-build-debug = hata ayıklama derlemesi
hw-report-build-release = sürüm derlemesi

## Erişilebilirlik (ekran okuyucular)
# Ekran okuyucunun seslendirdiği şekliyle görüntü: dosya adı ve piksel boyutu.
a11y-image = { $name }, { $width } × { $height } piksel
# Kısa bildirimlerin göründüğü bölge.
a11y-notifications = Bildirimler
# machine-drafted, needs native review
a11y-transport-playback = Oynatma
a11y-transport-pages = Sayfalar
a11y-settings-sections = Bölümler

## Read codes (QR codes in the picture) (machine-drafted, needs native review)
codes-placeholder = Bu görüntüdeki kodlar
codes-looking = Kodlar aranıyor…
codes-none = Bu görüntüde kod bulunamadı.
codes-failed = Görüntü okunamadı: { $reason }
codes-copy = “{ $text }” metnini kopyala
# $kind is "QR", $version a number.
codes-kind = { $kind } { $version }
# Data Matrix: its size in modules, e.g. "Data Matrix 12×12".
# machine-drafted, needs native review
codes-kind-size = { $kind } { $rows }×{ $cols }
codes-open-row = { $host } bağlantısını aç…
codes-network-off = Açık bir ağ yok, bu nedenle bağlantılar açılmaz.
codes-open-title = { $host } adresine giden bağlantı açılsın mı?
codes-open-detail = Web tarayıcınızda açılır. Yalnızca güvendiğiniz bağlantıları açın.
codes-open = Tarayıcıda aç
# machine-drafted, needs native review
# Find text: the text in the picture shown, read by the Local AI pack's Text models.
ocr-placeholder = Bu görüntüdeki metin
# machine-drafted, needs native review
ocr-looking = Metin okunuyor…
# machine-drafted, needs native review
ocr-none = Bu görüntüde metin bulunamadı.
# machine-drafted, needs native review
ocr-failed = Metin okunamadı: { $reason }
# machine-drafted, needs native review
ocr-needs-pack = Metin bulma, yerel yapay zekâ paketinin metin modellerini gerektirir (Ayarlar › Yerel yapay zekâ).
# machine-drafted, needs native review
ocr-loading = Metin modelleri yükleniyor…
# machine-drafted, needs native review
# A row: choosing it copies the line. $text is the line (one line, shortened).
ocr-copy = “{ $text }” metnini kopyala
# machine-drafted, needs native review
ocr-copy-all = Tüm metni kopyala
# machine-drafted, needs native review
# A row's note: how sure the reading is, and the add-on that read it. $percent is formatted.
ocr-confidence = Güven: { $percent }

## Hardware report panel (machine-drafted, needs native review)
hw-report-section-system = Sistem
hw-report-section-gpu = GPU
hw-report-section-decoding = Kod çözme
hw-report-operating-system = İşletim sistemi
hw-report-adapter = Bağdaştırıcı
hw-report-gpu-type = Tür
hw-report-gpu-init = Başlatma süresi
hw-report-max-layers = Doku katmanları
hw-report-compression = Doku sıkıştırma
hw-report-unified-memory = CPU ile paylaşılan bellek
hw-report-display = Ekran
hw-report-hw-decode = Donanımla kod çözme
hw-report-core-cluster = Çekirdek grubu
hw-report-power = Güç
hw-report-calibration = Hız ölçümleri
hw-report-codec-others = Diğerleri
hw-report-performance-cores-value = { $count } / { $total }
hw-report-cluster-value = { $name }: { $physical } fiziksel, { $logical } mantıksal
hw-report-thermal-nominal = Normal sıcaklık
hw-report-thermal-fair = Ilık
hw-report-thermal-serious = Sıcak, yavaşlatılıyor
hw-report-thermal-critical = Aşırı ısınma
hw-report-power-battery = Pil gücünde ({ $percent })
hw-report-power-battery-unknown = Pil gücünde
hw-report-power-mains = Prize takılı
hw-report-power-mains-percent = Prize takılı ({ $percent })
hw-report-power-saving = Güç tasarrufu açık
hw-report-gpu-discrete = Ayrı
hw-report-gpu-integrated = Tümleşik
hw-report-gpu-virtual = Sanal
hw-report-gpu-software = Yazılım, CPU'da
hw-report-gpu-other = Diğer
hw-report-gpu-unavailable = Kullanılamıyor
hw-report-ms = { $ms } ms
hw-report-yes = Evet
hw-report-no = Hayır
hw-report-unified-platform-differs = Sistem farklı bildiriyor
hw-report-display-value = { $fps } Hz, HDR payı şu an { $now }, en çok { $max }
hw-report-hw-decode-value = { $device } üzerinde { $api }
hw-report-hw-decode-checking = Hâlâ denetleniyor…
hw-report-codec-up-to = En çok { $width } × { $height }
hw-report-codec-none = Kullanılamıyor
hw-report-codec-h264-encode = H.264 kodlama
hw-report-supported = desteklenen: { $list }
hw-report-unsupported = desteklenmeyen: { $list }
hw-report-calibration-done = Bu bilgisayarda ölçüldü
hw-report-calibration-pending = Henüz ölçülmedi
hw-report-paths-power-saving = Güç tasarrufu: daha az enerji kullanan yollar tercih edilir
hw-report-paths-caption = { $mpx } megapiksellik görüntü için yollar
hw-report-col-task = Görev
hw-report-col-path = Yol
hw-report-col-time = Süre
hw-report-col-cpu = CPU süresi (tüm iş parçacıkları)
hw-report-col-runs = Ölçümler
hw-report-col-use = Kullanım
hw-report-runs-untested = Test edilmedi
hw-report-use-viewing = Görüntüleme ({ $reason })
hw-report-use-background = Arka plan ({ $reason })
hw-report-reason-only = tek seçenek
hw-report-reason-faster = en hızlı
hw-report-reason-cheaper = en az CPU süresi
hw-report-reason-lower-power = en az enerji
hw-report-reason-explore = deneniyor
hw-report-reason-forced = Ayarlar'da belirlendi
hw-report-reason-prefer-gpu = GPU'yu tercih et
hw-report-reason-cpu-only = Yalnızca CPU
hw-report-reason-calibrate = ölçülüyor
hw-report-reason-scaled = küçültülmüş boyut
hw-report-trust-unverified = Henüz denetlenmedi
hw-report-trust-blocked = Bir hatadan sonra kapatıldı
hw-report-trust-absent = Bu bilgisayarda yok

## Keyboard cheat sheet tabs and sections (machine-drafted, needs native review)
cheatsheet-mode-gallery = Galeri
cheatsheet-mode-compare = Karşılaştırma
cheatsheet-gallery-browse = Gözatma
cheatsheet-gallery-select = Seçim
cheatsheet-gallery-mark = Puanla ve işaretle
cheatsheet-gallery-move = Görüntüler arasında geç
cheatsheet-gallery-page = Sayfa yukarı veya aşağı
cheatsheet-gallery-ends = İlk veya son görüntü
cheatsheet-gallery-up = Üst klasöre çık
cheatsheet-gallery-history-back = Önceki klasöre geri dön
cheatsheet-gallery-history-forward = Sonraki klasöre ilerle
cheatsheet-gallery-back = Üstteki görünümü kapat, sonra geri dön
cheatsheet-gallery-viewer = Görüntüleyiciye dön
cheatsheet-gallery-album-add = Son albüme ekle
cheatsheet-gallery-select-one = Seç veya seçimi kaldır
cheatsheet-gallery-quick-look = Hızlı bakış
cheatsheet-gallery-filter = Filtrele
cheatsheet-gallery-bigger = Küçük resimleri büyüt
cheatsheet-gallery-smaller = Küçük resimleri küçült
cheatsheet-gallery-default-size = Standart küçük resim boyutu
cheatsheet-gallery-refresh = Klasörü yenile
cheatsheet-gallery-menu = Bağlam menüsü
cheatsheet-compare-views = Karşılaştırma
cheatsheet-compare-overlay = Bindirme veya yan yana
cheatsheet-compare-difference = Fark (yeniden: sonraki tür)
cheatsheet-compare-step-down = Daha az karıştırma veya fark kazancı
cheatsheet-compare-step-up = Daha fazla karıştırma veya fark kazancı
cheatsheet-compare-pick = Tuşların etki ettiği görüntü
cheatsheet-compare-close = Karşılaştırmayı kapat
cheatsheet-channels = Kanallar
cheatsheet-medical-window = Pencere ve düzey
cheatsheet-medical-measure = Ölç ve not ekle
cheatsheet-medical-frames = Kareler ve seriler
# machine-drafted, needs native review
cheatsheet-medical-3d = 3D ve yeniden biçimlendirme
cheatsheet-medical-analysis = Segmentasyon, füzyon ve PET
# machine-drafted, needs native review
cheatsheet-medical-studies = Tetkikler ve paylaşım
cheatsheet-palette-only-title = Komut paletinde ({ $key })
cheatsheet-palette-only-title-nokey = Komut paletinde
cheatsheet-palette-only-tag = Palet
cheatsheet-palette-only-line = { $action }: tuş yok; komut paletinde ({ $key })
cheatsheet-palette-only-line-nokey = { $action }: tuş yok; komut paletinde
# Kısayol listesinde Esc (paletteki uzun açıklaması sığmıyor).
cheatsheet-back = Kapat veya geri git

## Hardware video encoding (machine-drafted, needs native review)
hw-report-path-video-encode = Video dışa aktarma
hw-report-engine-hw-encoder = donanım kodlayıcı

## Path timing units (machine-drafted, needs native review)
hw-report-task-thumbnail = { $task } ({ $size } küçük resim başına)
hw-report-task-frame = { $task } ({ $size } kare başına)

## Hardware report, software-only formats (machine-drafted, needs native review)
hw-report-codec-software = Yalnızca yazılım

about-natural-earth = Harita verileri Natural Earth’ten (naturalearthdata.com), kamu malı.
app-info-show-on-map = Haritada göster
