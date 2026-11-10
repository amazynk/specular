# machine-drafted, needs native review
# Specular — Ayarlar penceresi. Türkçe.
#
# Kimlik şeması, crates/specular-settings içindeki ayar şemasından üretilir:
#   settings-section-<section>              bölüm başlığı
#   settings-<section>-<key>                ayar etiketi
#   settings-<section>-<key>-help           etiketin altındaki yardım metni
#   settings-<section>-<key>-<value>        bir seçim ayarının bir seçeneği
# Anahtarlar ve değerler, "_" yerine "-" yazılmış settings.toml adlarıdır.

## Pencere öğeleri

settings-title = Ayarlar
settings-search-placeholder = Ayarlarda ara
settings-search-no-results = Eşleşen ayar yok: { $query }
settings-reset = Sıfırla
settings-reset-section = Bölümü sıfırla
settings-modified = Değiştirildi
settings-default-value = Varsayılan: { $value }
settings-restart-required = Yeniden başlatınca etkinleşir
settings-restart-now = Şimdi yeniden başlat
settings-undo = Geri al
settings-redo = Yinele
settings-value-on = Açık
settings-value-off = Kapalı
settings-value-auto = Otomatik
settings-value-system = Sistemi izle
settings-value-system-language = Sistem varsayılanı
settings-value-follow-language = Arayüz dilini izle
settings-value-default-location = Varsayılan konum
settings-choose = Seç…
settings-clear = Temizle
settings-file-reloaded = settings.toml, { -app-name } dışında değiştirildi ve yeniden yüklendi.
settings-warning-unknown-key = settings.toml içinde bilinmeyen ayar: { $key }. Dosyada tutulur ancak yok sayılır.
settings-warning-invalid-value = Geçersiz değer ({ $key }): { $reason } Bunun yerine varsayılan kullanılır.
settings-warning-newer-version = settings.toml, { -app-name }'ın daha yeni bir sürümü tarafından yazılmış. Bu sürümün bilmediği ayarlar korunur.
settings-warning-syntax = settings.toml okunamıyor (satır { $line }): { $reason } Düzeltilene kadar varsayılanlar kullanılır.

## Doğrulama hataları (`specular --set` tarafından da yazdırılır)

settings-error-unknown-key = Böyle bir ayar yok: { $key }.
settings-error-unknown-key-suggestion = Böyle bir ayar yok: { $key }. Şunu mu demek istediniz: { $suggestion }?
settings-error-not-bool = Açık veya kapalı (true veya false) bekleniyordu.
settings-error-not-integer = Tam sayı bekleniyordu.
settings-error-not-number = Sayı bekleniyordu.
settings-error-out-of-range = { $min } ile { $max } arasında olmalı.
settings-error-out-of-range-auto = 0 (otomatik) veya { $min } ile { $max } arasında olmalı.
settings-error-unknown-option = Şunlardan biri bekleniyordu: { $options }.
settings-error-bad-colour = #1E1E1E veya #1E1E1E80 gibi bir renk bekleniyordu.
settings-error-bad-language = de, pt-BR veya zh-Hans gibi bir dil etiketi ya da “system” bekleniyordu.
settings-error-bad-path = Dosya veya klasör yolu bekleniyordu.
settings-error-not-settable = Bu girdi bir komuttur ve değeri yoktur.
settings-error-syntax = anahtar=değer bekleniyordu, örneğin viewing.wheel_behaviour=navigate.
# machine-drafted, needs native review
settings-error-bad-text = Sekme, satır sonu veya başka denetim karakterleri içeremez.
settings-error-invalid = { $reason }
settings-error-tile-url = {"{"}z{"}"}, {"{"}x{"}"} ve {"{"}y{"}"} içeren bir http veya https adresi gerekir.

## Birimler

unit-percent = { PERCENT($value) }
unit-milliseconds = { $value } ms
unit-seconds = { $value } sn
unit-mebibytes = { $value } MiB
# machine-drafted, needs native review
unit-luminance = { $value } cd/m²
unit-images = { $value } görüntü
unit-threads = { $value } iş parçacığı
unit-bytes = { $value } bayt
unit-kilobytes = { $value } kB
unit-megabytes = { $value } MB
unit-gigabytes = { $value } GB
unit-terabytes = { $value } TB
unit-kibibytes = { $value } KiB
unit-gibibytes = { $value } GiB
unit-tebibytes = { $value } TiB
unit-symbol-mm = mm
unit-symbol-cm = cm
unit-symbol-m = m
unit-symbol-km = km
unit-symbol-mm2 = mm²
unit-symbol-cm2 = cm²
unit-symbol-m2 = m²
unit-symbol-km2 = km²
unit-symbol-ha = ha
unit-symbol-ft = ft
unit-symbol-ft2 = ft²
unit-symbol-mi = mi
unit-symbol-mi2 = mi²
unit-symbol-ac = ac

## Bölümler

settings-section-general = Genel
settings-section-viewing = Görüntüleme
settings-section-browsing = Gezinme
settings-section-colour = Renk
settings-section-metadata = Meta veriler ve gizlilik
# machine-drafted, needs native review
settings-section-privacy = Gizlilik
settings-section-performance = Performans
settings-section-accessibility = Erişilebilirlik
settings-section-keys = Tuşlar
settings-section-advanced = Gelişmiş

## Genel

settings-general-language = Arayüz dili
settings-general-language-help = Değişiklikler hemen etkinleşir. “Sistem varsayılanı” işletim sisteminizin dilini izler.
settings-general-region-formats = Bölge biçimleri
settings-general-region-formats-help = Tarih, saat, sayı ve boyutların nasıl yazıldığı. Varsayılan olarak arayüz dilini izler.
settings-general-theme = Tema
settings-general-theme-help = Pencereler ve denetimler için açık veya koyu görünüm. Görüntüler asla değiştirilmez.
settings-general-theme-system = Sistemi izle
settings-general-theme-light = Açık
settings-general-theme-dark = Koyu
# machine-drafted, needs native review
settings-general-on-start = { -app-name } dosyasız açıldığında
settings-general-on-start-help = { -app-name } tek başına açıldığında ne gösterileceği. Dosya yöneticisinden veya komut satırından açılan bir dosya her zaman görüntüleyicide gösterilir.
settings-general-on-start-start-screen = Başlangıç ekranı
settings-general-on-start-gallery = Son klasörün galerisi
settings-general-on-start-library = Kitaplık
settings-general-start-tips = Başlangıç ekranında “Deneyin” kartlarını göster
settings-general-start-tips-help = Yararlı özellikleri ve tuşlarını gösteren bir kart sırası. Komut paletindeki “Karşılamayı göster” onu geri getirir.
settings-general-tips = İpuçlarını göster
settings-general-tips-help = Bir özellik ilk kez işe yarayabileceğinde bir kez gösterilen kısa ipucu; örneğin bir klasörü galeri olarak gösteren tuş. Slayt gösterisi sırasında asla.
settings-general-tips-shown = Gösterilmiş ipuçları
settings-general-tips-shown-help = Zaten verilmiş tek seferlik ipuçları; yeniden gösterilmemeleri için.
# machine-drafted, needs native review
settings-general-your-name = Adınız
settings-general-your-name-help = Eklediğiniz damga ve işaretlemelerde gösterilir. Yalnızca oluşturduğunuz işaretleme dosyalarına kaydedilir.
settings-general-single-instance = Dosyaları mevcut pencerede aç
settings-general-single-instance-help = { -app-name } zaten çalışıyorsa, başka bir görüntü açmak yenisini başlatmak yerine onu o pencerede gösterir.
settings-general-file-associations = Dosya ilişkilendirmeleri…
settings-general-file-associations-help = { -app-name }'ı desteklediği görüntü biçimleri için varsayılan görüntüleyici yap.

## Görüntüleme

settings-viewing-initial-zoom = İlk yakınlaştırma
settings-viewing-initial-zoom-help = Yeni açılan bir görüntünün ne büyüklükte gösterileceği.
settings-viewing-initial-zoom-fit = Pencereye sığdır
settings-viewing-initial-zoom-fit-if-larger = Büyükleri sığdır, küçükleri { PERCENT(100) } göster
settings-viewing-initial-zoom-actual-size = Gerçek boyut ({ PERCENT(100) })
settings-viewing-initial-zoom-fill = Pencereyi doldur
settings-viewing-wheel-behaviour = Fare tekerleği
settings-viewing-wheel-behaviour-help = Tekerleği çevirmenin ne yaptığı. Diğer eylem için Ctrl tuşunu basılı tutun.
settings-viewing-wheel-behaviour-help-macos = Tekerleği çevirmenin ne yaptığı. Diğer eylem için ⌘ tuşunu basılı tutun.
settings-viewing-wheel-behaviour-zoom = Yakınlaştır
settings-viewing-wheel-behaviour-navigate = Sonraki ve önceki görüntü
settings-viewing-keep-zoom = Görüntüler arasında yakınlaştırmayı koru
settings-viewing-keep-zoom-help = Yakınlaştırılmışken sonraki ve önceki görüntü aynı yakınlaştırmayla açılır ve karenin aynı bölümünü gösterir: seri çekimlerde netliği kontrol etmek için kullanışlıdır.
settings-viewing-zoom-animation = Yakınlaştırmayı canlandır
settings-viewing-zoom-animation-help = Atlamak yerine yumuşakça yakınlaştır. “Hareketi azalt” açıkken otomatik olarak kapanır.
settings-viewing-zoom-animation-speed = Yakınlaştırma animasyonu hızı
settings-viewing-zoom-animation-speed-help = Canlandırılmış yakınlaştırmaların ne kadar çabuk oturduğu.
settings-viewing-zoom-animation-speed-relaxed = Sakin
settings-viewing-zoom-animation-speed-normal = Normal
settings-viewing-zoom-animation-speed-fast = Hızlı
settings-viewing-background = Arka plan
settings-viewing-background-help = Pencerede görüntünün arkasındaki renk.
settings-viewing-background-theme = Temaya uy
settings-viewing-background-black = Siyah
settings-viewing-background-dark-grey = Koyu gri
settings-viewing-background-light-grey = Açık gri
settings-viewing-background-white = Beyaz
settings-viewing-background-custom = Özel renk
settings-viewing-background-color = Özel arka plan rengi
settings-viewing-background-color-help = Arka plan “Özel renk” olarak ayarlandığında kullanılır.
settings-viewing-fullscreen-background = Tam ekran arka planı
settings-viewing-fullscreen-background-help = Tam ekranda görüntünün arkasındaki renk.
settings-viewing-transparency = Saydam alanlar
settings-viewing-transparency-help = Görüntünün saydam kısımlarının nasıl çizildiği.
settings-viewing-transparency-checkerboard = Dama tahtası
settings-viewing-transparency-background = Arka plan rengi
settings-viewing-transparency-black = Siyah
settings-viewing-transparency-white = Beyaz
settings-viewing-pixel-grid-threshold = Piksel ızgarasını göster
settings-viewing-pixel-grid-threshold-help = En az bu kadar yakınlaştırıldığında pikseller arasına ızgara çiz.
settings-viewing-smoothing-high-zoom = Büyütülmüş pikseller
settings-viewing-smoothing-high-zoom-help = Görüntü { PERCENT(100) }'den büyük gösterildiğinde piksellerin nasıl göründüğü.
settings-viewing-smoothing-high-zoom-auto = Yumuşak, { PERCENT(400) }'den itibaren keskin
settings-viewing-smoothing-high-zoom-smooth = Her zaman yumuşak
settings-viewing-smoothing-high-zoom-pixelated = Her zaman keskin kareler
settings-viewing-double-click = Görüntüye çift tıklama
settings-viewing-double-click-help = Boş arka plana çift tıklamak her zaman tam ekranı açar/kapatır.
settings-viewing-double-click-toggle-zoom = Sığdır ve { PERCENT(100) } arasında geçiş yap
settings-viewing-double-click-fullscreen = Tam ekranı aç/kapat
settings-viewing-double-click-nothing = Hiçbir şey yapma
settings-viewing-fullscreen-on-open = Görüntüleri tam ekranda aç
settings-viewing-fullscreen-on-open-help = Bir görüntü { -app-name } dışından açıldığında tam ekranda başla.
settings-viewing-fullscreen-mode = Tam ekran stili
settings-viewing-fullscreen-mode-help = Yerel tam ekran kendi Space alanını kullanır; anında tam ekran geçerli olanı animasyonsuz kaplar.
settings-viewing-fullscreen-mode-native = Yerel (kendi Space alanı)
settings-viewing-fullscreen-mode-borderless = Anında
settings-viewing-toolbar-autohide = Araç çubuğunu otomatik gizle
settings-viewing-toolbar-autohide-help = İşaretçi hareketsizken araç çubuğu ve kenar okları kaybolur, hareket edince geri gelir.
settings-viewing-toolbar-autohide-delay = Araç çubuğunu gizleme süresi
settings-viewing-toolbar-autohide-delay-help = Araç çubuğu kaybolmadan önce işaretçinin ne kadar süre hareketsiz kalması gerektiği.
settings-viewing-toolbar-items = Araç çubuğu düğmeleri
settings-viewing-toolbar-items-help = Araç çubuğunun hangi düğmeleri hangi sırayla gösterdiği.
settings-viewing-toolbar-drag = Araç çubuğunu sürükleyerek düzenle
settings-viewing-toolbar-drag-help = Bir düğmeyi taşımak için araç çubuğu boyunca sürükleyin. Kısa bir tıklama yine düğmeye basar; Esc sürüklenen düğmeyi yerine koyar.
settings-viewing-toolbar-items-open = Aç
settings-viewing-toolbar-items-prev = Önceki görüntü
settings-viewing-toolbar-items-next = Sonraki görüntü
settings-viewing-toolbar-items-zoom-out = Uzaklaştır
settings-viewing-toolbar-items-zoom-level = Yakınlaştırma düzeyi
settings-viewing-toolbar-items-zoom-in = Yakınlaştır
settings-viewing-toolbar-items-fit = Pencereye sığdır
settings-viewing-toolbar-items-rotate-left = Saat yönünün tersine döndür
settings-viewing-toolbar-items-rotate-right = Saat yönünde döndür
settings-viewing-toolbar-items-info = Bilgi
settings-viewing-toolbar-items-copy = Görüntüyü kopyala
settings-viewing-toolbar-items-delete = Çöp kutusuna taşı
settings-viewing-toolbar-items-palette = Komut paleti
settings-viewing-toolbar-items-settings = Ayarlar
settings-viewing-toolbar-items-fullscreen = Tam ekran
# machine-drafted, needs native review
settings-viewing-toolbar-items-spacer = Esnek boşluk
settings-viewing-toolbar-items-separator = Ayırıcı
settings-toolbar-preview = Araç çubuğu önizlemesi
settings-toolbar-hidden = Gizli düğmeler
settings-toolbar-add-spacer = Esnek boşluk ekle
settings-toolbar-add-separator = Ayırıcı ekle
settings-toolbar-reset = Varsayılana sıfırla
settings-toolbar-remove = Kaldır: { $item }
settings-toolbar-hint = Bir satırı taşımak için sürükleyin ya da seçip Alt+↑ veya Alt+↓ tuşuna basın. Sekme tuşu arama alanı ile liste arasında geçer.
settings-toolbar-move-up = Yukarı taşı
settings-toolbar-move-down = Aşağı taşı
settings-toolbar-move-start = Başa taşı
settings-toolbar-move-end = Sona taşı
settings-toolbar-menu-hide = Gizle
settings-toolbar-menu-show = Göster
settings-toolbar-menu-remove = Kaldır
settings-toolbar-row-menu = Diğer işlemler: { $item }
settings-toolbar-position = Konum { $index } / { $count }
settings-toolbar-hidden-state = Gizli

## Gezinme

settings-browsing-sort-order = Görüntüleri sırala
settings-browsing-sort-order-help = Bir klasörde ilerlerken kullanılan sıra. Ad sırası “img2”yi “img10”dan önce koyar ve dilinizin alfabesini izler.
settings-browsing-sort-order-natural = Ad
settings-browsing-sort-order-date-taken = Çekim tarihi
settings-browsing-sort-order-modified = Değiştirilme tarihi
settings-browsing-sort-order-size = Dosya boyutu
settings-browsing-sort-order-type = Dosya türü
settings-browsing-sort-descending = Ters sıra
settings-browsing-sort-descending-help = Sondan başa doğru sırala.
settings-browsing-loop-folder = Klasörün sonunda başa dön
settings-browsing-loop-folder-help = Son görüntüden sonra ilkiyle devam et; tersi de geçerli.
settings-browsing-include-subfolders = Alt klasörleri dahil et
settings-browsing-include-subfolders-help = Alt klasörlerdeki görüntülerde de gezin.
settings-browsing-hold-acceleration = Sonraki veya önceki basılı tutulunca hızlanma
settings-browsing-hold-acceleration-help = Bir tuş veya düğme basılı tutulurken geçişin ne kadar hızlandığı.
settings-browsing-hold-acceleration-off = Hızlanma yok
settings-browsing-hold-acceleration-gentle = Hafif
settings-browsing-hold-acceleration-normal = Normal
settings-browsing-hold-acceleration-fast = Hızlı
settings-browsing-confirm-trash = Çöp kutusuna taşımadan önce sor
settings-browsing-confirm-trash-help = Çöp kutusuna taşıma her zaman geri alınabilir; bu yüzden { -app-name } varsayılan olarak sormaz.
settings-browsing-arrows-follow-reading-direction = Ok tuşları okuma yönünü izler
settings-browsing-arrows-follow-reading-direction-help = Sağdan sola dillerde sol ok tuşu sonraki görüntüye gider.

## Renk

settings-colour-management = Renk yönetimi
settings-colour-management-help = Görüntünün ve ekranınızın renk profillerini kullanarak renkleri amaçlandığı gibi göster.
settings-colour-display-profile = Ekran profili
settings-colour-display-profile-help = Ekranınızın renk profili. Otomatik, işletim sisteminizin bildirdiğini kullanır.
settings-colour-rendering-intent = İşleme amacı
settings-colour-rendering-intent-help = Ekranın gösteremediği renklerin aralığına nasıl getirildiği.
settings-colour-rendering-intent-perceptual = Algısal
settings-colour-rendering-intent-relative-colorimetric = Göreli kolorimetrik
settings-colour-rendering-intent-saturation = Doygunluk
settings-colour-rendering-intent-absolute-colorimetric = Mutlak kolorimetrik
settings-colour-black-point-compensation = Siyah nokta dengelemesi
settings-colour-black-point-compensation-help = Ekranın siyahı görüntününkinden açıksa gölge ayrıntılarını koru.
settings-colour-untagged-images = Renk profili olmayan görüntüler
settings-colour-untagged-images-help = Hangi renkleri kullandığını belirtmeyen görüntülerin nasıl gösterileceği.
settings-colour-untagged-images-srgb = sRGB olarak işle
settings-colour-untagged-images-display = Değiştirmeden göster
settings-colour-dithering = Titreklik (dithering)
settings-colour-dithering-help = Renk derinliğini azaltırken ince mavi gürültü ekle; gökyüzü ve geçişlerde bantlaşmayı önler.
settings-colour-hdr-output = HDR çıkışı
settings-colour-hdr-output-help = HDR görüntüleri destekleyen ekranlarda tam parlaklıklarıyla göster. Otomatik, ekran ek parlaklık bildirdiğinde HDR kullanır; aksi takdirde parlak alanlar sığması için yumuşakça sıkıştırılır.
settings-colour-hdr-output-auto = Otomatik
settings-colour-hdr-output-on = Açık
settings-colour-hdr-output-off = Kapalı
# machine-drafted, needs native review
settings-colour-ocio-config = OCIO yapılandırması
settings-colour-ocio-config-help = EXR, Radiance HDR ve PFM görüntüleri için bir OpenColorIO yapılandırması (.ocio). Boş bırakılırsa $OCIO tarafından belirtilen kullanılır.
settings-colour-ocio-display = OCIO ekranı
settings-colour-ocio-display-help = OCIO yapılandırmasında monitörünüze karşılık gelen ekran.
settings-colour-ocio-display-default = Yapılandırmadaki ilk ekran
settings-colour-ocio-view = OCIO görünümü
settings-colour-ocio-view-help = Sahne doğrusal (scene-linear) görüntülerin o ekranda nasıl gösterileceği; örneğin ton eşlemeli bir görünüm.
settings-colour-ocio-view-default = Ekranın ilk görünümü
settings-colour-ocio-input = OCIO giriş renk uzayı
settings-colour-ocio-input-help = Görüntü değerlerinin renk uzayı.
settings-colour-ocio-input-default = Yapılandırmanın scene-linear rolü
# machine-drafted, needs native review
settings-colour-proof-profile = Ekran provası profili
settings-colour-proof-profile-help = Banyo’nun ekran provasının en son kullandığı yazıcı profili.
settings-colour-proof-intent = Ekran provası oluşturma amacı
settings-colour-proof-intent-help = Yazıcının basamadığı renklerin nasıl uyarlanacağı.
settings-colour-proof-intent-perceptual = Algısal
settings-colour-proof-intent-relative = Göreli kolorimetrik
settings-colour-proof-bpc = Ekran provasında siyah nokta telafisi
settings-colour-proof-bpc-help = Siyahı yazıcının siyahına eşler.
settings-colour-proof-paper = Ekran provasında kâğıt beyazı
settings-colour-proof-paper-help = Kâğıdın kendi rengini gösterir.
settings-colour-proof-ink = Ekran provasında mürekkep siyahı
settings-colour-proof-ink-help = Mürekkep siyahının ne kadar koyu olduğunu gösterir.

## Meta veriler ve gizlilik

settings-metadata-show-info-on-open = Açılışta bilgi panelini göster
settings-metadata-show-info-on-open-help = Bir görüntü her açıldığında kamera, tarih ve yer ayrıntılarını içeren paneli aç.
settings-metadata-info-detail = Bilgi panelinin gösterdiği
settings-metadata-info-detail-help = Özet en yararlı ayrıntıları sade bir dille açıklar; tüm alanlar dosyada saklanan her şeyi listeler.
settings-metadata-info-detail-summary = Özet
settings-metadata-info-detail-all = Tüm alanlar
settings-metadata-reverse-geocoding = GPS konumları için yer adlarını göster
settings-metadata-reverse-geocoding-help = En yakın yerleşim yerini yerleşik verilerden bul. Çevrimdışı çalışır: konumlar bilgisayarınızdan asla çıkmaz.
settings-metadata-hide-gps = GPS konumlarını ekranda gizle
settings-metadata-hide-gps-help = Örneğin ekranınızı paylaşırken koordinatları veya yerleri gösterme. Dosyalar değiştirilmez.
# machine-drafted, needs native review
settings-metadata-develop-sidecar = Geliştirme düzenlemelerini fotoğrafın yanına kaydet
# machine-drafted, needs native review
settings-metadata-develop-sidecar-help = Dosyanın yanına IMG_1234.CR2.specular.xmp yazar, böylece düzenlemeler dosyayla birlikte taşınır. Kapalı: düzenlemeler yalnızca kitaplıkta kalır.
# machine-drafted, needs native review
settings-metadata-develop-import-lightroom = Banyo’da Lightroom ayarlarını içe aktar
# machine-drafted, needs native review
settings-metadata-develop-import-lightroom-help = Lightroom ayarları olan ve düzenlemesi olmayan bir fotoğraf bunlarla, yaklaşık olarak açılır. Kapalı: Banyo bir düğme sunar.
settings-metadata-units = Birimler
settings-metadata-units-help = Yükseklik, mesafe ve hız birimleri.
settings-metadata-units-auto = Bölge biçimlerini izle
settings-metadata-units-metric = Metrik
settings-metadata-units-imperial = İngiliz
# machine-drafted, needs native review
settings-metadata-basemap = Altlık harita
settings-metadata-basemap-help = Fotoğraf konumlarının altındaki harita. Çevrimiçi haritalar baktığınız bölgenin döşemelerini indirir.
settings-metadata-basemap-off = Yalnızca yerleşik harita (çevrimdışı)
settings-metadata-basemap-openstreetmap = OpenStreetMap (çevrimiçi)
settings-metadata-basemap-custom = Başka bir döşeme sunucusu (çevrimiçi)
settings-metadata-basemap-file = Kendi harita dosyam
settings-metadata-basemap-url = Döşeme sunucusu adresi
settings-metadata-basemap-url-help = {"{"}z{"}"}, {"{"}x{"}"} ve {"{"}y{"}"} içeren bir adres, örneğin https://tiles.example.org/{"{"}z{"}"}/{"{"}x{"}"}/{"{"}y{"}"}.png
settings-metadata-basemap-path = Harita dosyası
settings-metadata-basemap-path-help = Bir MBTiles veya PMTiles dosyası. Çevrimdışı çalışır.
settings-metadata-basemap-cache = Harita döşemesi önbelleği
settings-metadata-basemap-cache-help = İndirilen döşemeler için disk alanı. Önce en eskiler silinir.
# machine-drafted, needs native review
settings-metadata-basemap-allowed = İzin verilen döşeme sunucuları
settings-metadata-basemap-allowed-help = Döşeme indirmeyi kabul ettiğiniz harita sunucuları.

## Performans

settings-performance-ram-cache = Bellek önbelleği
settings-performance-ram-cache-help = Kodu çözülmüş görüntüleri hazır tutmak için kullanılan bellek. Otomatik, bilgisayarınızın bellek miktarına göre seçer.
settings-performance-vram-cache = GPU bellek önbelleği
settings-performance-vram-cache-help = Görüntüler ve döşemeler için kullanılan GPU belleği. Otomatik, GPU'nuza göre seçer.
settings-performance-prefetch-depth = Önceden hazırlanacak görüntüler
settings-performance-prefetch-depth-help = İlerlediğiniz yöndeki kaç görüntünün kodunun önceden çözüleceği.
settings-performance-decoder-threads = Kod çözücü iş parçacıkları
settings-performance-decoder-threads-help = Görüntülerin kodunu çözmek için kullanılan CPU iş parçacıkları. Otomatik, tüm performans çekirdeklerini kullanır ve dosyaya uyum sağlar.
settings-performance-gpu-adapter = GPU
settings-performance-gpu-adapter-help = Bilgisayarda birden fazla GPU varsa hangisinin kullanılacağı.
settings-performance-gpu-adapter-high-performance = En yüksek performans
settings-performance-gpu-adapter-low-power = En düşük güç tüketimi
settings-performance-processing = Görüntülerin işlendiği yer
settings-performance-processing-help = Bazı işler CPU'da, GPU'da veya bir donanım kod çözücüsünde çalışabilir. Otomatik bu bilgisayarı ölçer; baktığınız görüntü için en hızlı, arka plandaki işler için en tasarruflu yolu seçer. Bu bilgisayarda neyi neden seçtiği aşağıda gösterilir.
settings-performance-processing-automatic = Otomatik
settings-performance-processing-prefer-gpu = GPU'yu tercih et
settings-performance-processing-cpu-only = Yalnızca CPU
settings-performance-drag-preview = Sürüklerken önizleme
settings-performance-drag-preview-help = Bir kaydırıcıyı sürüklerken Geliştirme’nin fotoğrafı nasıl gösterdiği. Otomatik, yalnızca bilgisayar yetişemediğinde daha küçük bir resim kullanır.
settings-performance-drag-preview-automatic = Otomatik
settings-performance-drag-preview-sharp = Her zaman keskin
settings-performance-drag-preview-fast = Her zaman hızlı

## Ayarlar → Performans → { -app-name } bu bilgisayarda nasıl çalışıyor (yol seçici,
## DESIGN.md §12.3.1). $engine ve $other, hw-report-engine-* adlarıdır (küçük harfli, cümle
## içinde kullanılır); paths-engine-* bağımsız adlardır.
settings-paths-title = { -app-name } bu bilgisayarda nasıl çalışıyor
settings-paths-intro = { -app-name }, bu işleri yapmanın her yolunu bu bilgisayarda ölçer ve en iyisini kullanır. Süreler { $mpx } megapiksellik bir fotoğraf içindir.
settings-paths-intro-measuring = Ölçüm henüz bitmedi: { -app-name } bilgisayar boştayken ölçer.
settings-paths-intro-battery = Pil gücünde: seçimler daha düşük enerji kullanımını gözetir.
settings-paths-keywords = GPU CPU NPU grafik kartı donanım hızlandırma kod çözücü karşılaştırma hız ekran kartı işlemci
paths-engine-cpu = CPU
paths-engine-cpu-parallel = CPU, tüm çekirdekler
paths-engine-gpu = GPU
paths-engine-hw-decoder = Donanım kod çözücü
paths-engine-npu = NPU
settings-paths-your-setting = { $engine } (sizin ayarınız)
settings-paths-measured-faster = Bu bilgisayarda ölçüldü: { $other } ile karşılaştırıldığında { $ratio } kat hızlı.
settings-paths-measured-similar = Bu bilgisayarda ölçüldü: { $other } ile yaklaşık aynı hızda.
settings-paths-measured-power = Pil gücünde en az enerjiyi bu kullanır (bu bilgisayarda ölçüldü).
settings-paths-not-measured = Henüz ölçülmedi: bu tür bilgisayarlar için olağan seçim.
settings-paths-set-by = Ayarınıza göre seçildi: “{ $setting }: { $option }”.
settings-paths-blocked-check = Kapatıldı: { $engine } ({ $date }); sonucu CPU'nunkiyle eşleşmedi.
settings-paths-blocked-error = Bir hatadan sonra kapatıldı: { $engine } ({ $date }).
settings-paths-blocked-lost = Yanıt vermeyi bıraktıktan sonra kapatıldı: { $engine } ({ $date }).
settings-paths-blocked-undated = Bir hatadan sonra kapatıldı: { $engine }.
settings-paths-checking = Baktığınız görüntü için kullanılmadan önce CPU ile karşılaştırılarak denetlenir: { $engine }.
settings-paths-exploring = Şu anda arka planda deneniyor: { $engine }.
settings-paths-unavailable = Bu bilgisayarda bunun için yok: { $engine }.
settings-paths-background-cheaper = Arka planda: { $engine }; ölçümlere göre { $percent } daha az CPU süresi kullanıyor.
settings-paths-background-power = Arka planda: { $engine }, enerji tasarrufu için.
settings-paths-background = Arka planda: { $engine }.
settings-paths-detail-measured = { $engine }: { $time } ms, { $cpu } ms CPU süresi, { $runs } ölçüm ({ $confidence })
settings-paths-detail-guess = { $engine }: yaklaşık { $time } ms (tahmin)
settings-paths-confidence-low = düşük güven
settings-paths-confidence-medium = orta güven
settings-paths-confidence-high = yüksek güven
settings-paths-machine = Ölçüm yapılan donanım: { $gpu } · { $driver } · { $cpu }
settings-paths-retry = Yeniden dene
settings-paths-measure-again = Yeniden ölç
settings-paths-measure-again-question = Kapatılan yollar dahil bu bilgisayarda ölçülenler unutulsun ve bilgisayar boştayken yeniden ölçülsün mü?
settings-paths-measure-again-confirm = Ölçümleri unut

settings-performance-display-cache = Diskte ekran önbelleği tut
settings-performance-display-cache-help = Görüntülerin ekran boyutlu kopyalarını sakla; böylece daha önce gördüğünüz klasörler anında açılır.
settings-performance-display-cache-size = Ekran önbelleği boyutu
settings-performance-display-cache-size-help = Önbellek dolduğunda en eski girdiler kaldırılır.
settings-performance-display-cache-location = Ekran önbelleği konumu
settings-performance-display-cache-location-help = Hızlı bir sürücüdeki klasör en iyi sonucu verir.
settings-performance-fast-simd = SIMD komutlarını kullan
settings-performance-fast-simd-help = Kod çözme için CPU'nun vektör komutlarını kullan. Yalnızca bir sorunu teşhis etmek için kapatın.
settings-performance-fast-dct-scaling = Ekran boyutunda kod çöz
settings-performance-fast-dct-scaling-help = Büyük JPEG'lerin kodunu doğrudan gösterildikleri boyutta çöz. Yalnızca bir sorunu teşhis etmek için kapatın.
settings-performance-fast-parallel-decode = Birden çok çekirdekte kod çöz
settings-performance-fast-parallel-decode-help = Büyük görüntüleri CPU çekirdeklerine böl. Yalnızca bir sorunu teşhis etmek için kapatın.
settings-performance-fast-restart-index = Yakınlaştırma için büyük JPEG'leri dizinle
settings-performance-fast-restart-index-help = Bir JPEG'in her bölümünün nerede başladığını hatırla; böylece yakınlaştırılmış bölgelerin kodu hızla çözülür. Yalnızca bir sorunu teşhis etmek için kapatın.
settings-performance-fast-yuv-upload = Renkleri GPU'da dönüştür
settings-performance-fast-yuv-upload-help = JPEG renklerini GPU'da veya CPU'da dönüştür. Otomatik, bu bilgisayarda hangisi daha hızlıysa onu seçer. Yalnızca bir sorunu teşhis etmek için kapatın.
settings-performance-fast-yuv-upload-auto = Otomatik
settings-performance-fast-yuv-upload-on = Açık
settings-performance-fast-yuv-upload-off = Kapalı
settings-performance-fast-async-io = Eşzamansız dosya okuma
settings-performance-fast-async-io-help = İşletim sisteminin en hızlı dosya arayüzüyle önceden oku. Yalnızca bir sorunu teşhis etmek için kapatın.
settings-performance-fast-mmap = Belleğe eşlenmiş dosyalar
settings-performance-fast-mmap-help = Dosyaları kopyalamadan oku. Yalnızca bir sorunu teşhis etmek için kapatın.
settings-performance-hardware-decode = Donanımla kod çözme
settings-performance-hardware-decode-help = JPEG fotoğrafları GPU'nun yerleşik kod çözücüsü açsın. Otomatik, genellikle arka planda hazırlanan görüntüler için, yararlı olduğu yerde kullanır. Önce normal kod çözücüyle karşılaştırılarak denetlenir; işleyemediği her şey normal şekilde açılır.
settings-performance-hardware-decode-auto = Otomatik
settings-performance-hardware-decode-on = Açık
settings-performance-hardware-decode-off = Kapalı
settings-performance-frame-time-hud = Kare süresi göstergesi
settings-performance-frame-time-hud-help = Kod çözme ve kare sürelerini ve hangi hızlı yolların kullanıldığını göster.
settings-performance-stall-warning = Takılan yüklemeleri bildirme süresi
settings-performance-stall-warning-help = Bir görüntü bu süreden sonra hâlâ yükleniyorsa, yükleme durumunu ve bekleyen işleri günlüğe yaz (standart hata veya SPECULAR_LOG_FILE).
# machine-drafted, needs native review
settings-performance-insights = Performans önerileri
settings-performance-insights-help = Görüntüler aynı nedenle sürekli yavaş açıldığında köşede neyin yardımcı olacağıyla birlikte “Bu neden yavaş?” göster. Öğrenilenler bu bilgisayarda kalır.
settings-performance-insights-storage = Yavaş depolamayla ilgili öneriler
settings-performance-insights-storage-help = Dosyaların okunması (bir ağ paylaşımından, yavaş bir sürücüden veya arşivlerden) görüntülerin açılmasını sürekli geciktirdiğinde öneri göster.
settings-performance-insights-decoder = Kod çözmeyle ilgili öneriler
settings-performance-insights-decoder-help = Bir donanım kod çözücü yardımcı olabilecekken görüntülerin kodu sürekli CPU'da çözüldüğünde öneri göster.
# machine-drafted, needs native review
settings-performance-insights-restore = Gizlenen önerileri yeniden göster
settings-performance-insights-restore-help = “Bu neden yavaş?” üzerinde × ile gizlediğiniz önerileri hem görüntü açma hem küçük resimler için geri getirir. Gizlenmiş öneri yokken kullanılamaz.

## Erişilebilirlik

settings-accessibility-reduce-motion = Hareketi azalt
settings-accessibility-reduce-motion-help = Yakınlaştırma, kaydırma ve solma animasyonlarını anında değişimlerle değiştir.
settings-accessibility-reduce-motion-system = Sistemi izle
settings-accessibility-reduce-motion-on = Açık
settings-accessibility-reduce-motion-off = Kapalı
settings-accessibility-increase-contrast = Karşıtlığı artır
settings-accessibility-increase-contrast-help = Daha belirgin kenarlıklar ve metin, daha az saydamlık.
settings-accessibility-increase-contrast-system = Sistemi izle
settings-accessibility-increase-contrast-on = Açık
settings-accessibility-increase-contrast-off = Kapalı
settings-accessibility-text-scale = Metin boyutu
settings-accessibility-text-scale-help = Standart boyuta göre metin boyutu.
settings-accessibility-ui-scale = Arayüz ölçeği
settings-accessibility-ui-scale-help = İşletim sisteminizde ayarlanan ekran ölçeklemesine ek olarak tüm arayüzün boyutu.
settings-accessibility-focus-ring-always = Odak halkasını her zaman göster
settings-accessibility-focus-ring-always-help = Fare kullanırken bile klavye odağının hangi denetimde olduğunu göster.
settings-accessibility-screen-reader-verbosity = Ekran okuyucu ayrıntısı
# machine-drafted, needs native review
settings-accessibility-screen-reader-verbosity-help = Görüntüler arasında geçerken ekran okuyucunun ne kadar seslendirdiği ve imlecin altındaki göstergenin ne kadarını okuduğu.
settings-accessibility-screen-reader-verbosity-concise = Kısa
settings-accessibility-screen-reader-verbosity-normal = Normal
settings-accessibility-screen-reader-verbosity-verbose = Ayrıntılı
# machine-drafted, needs native review
settings-accessibility-announce-cursor = İmlecin altındaki değeri oku
settings-accessibility-announce-cursor-help = Ekran okuyucu, imleç veya klavye artı imleci hareket etmeyi bırakınca altındaki piksel değerini, HU’yu, sıcaklığı veya konumu okur.

## Gizlilik

# machine-drafted, needs native review
settings-privacy-no-network = Ağ yok
# machine-drafted, needs native review
settings-privacy-no-network-help = Kapalı: yalnızca şunlar bağlanır, her biri ancak onu açtığınızda veya onunla bir şey açtığınızda: güncelleme denetimi, yerel yapay zekâ modeli indirmeleri, çevrimiçi haritalar, bir web adresinden veya bulut depolamadan görüntüler ve eklediğiniz sunucular (DICOMweb, OMERO, haritalar). Açık: hiçbir şey bağlanmaz.

## Gelişmiş

settings-advanced-portable-mode = Taşınabilir mod
settings-advanced-portable-mode-help = Ayarları ve önbellekleri kullanıcı profiliniz yerine programın yanındaki klasörde tut.
settings-advanced-log-level = Günlük ayrıntısı
settings-advanced-log-level-help = { -app-name }'ın günlük dosyasına ne kadar yazdığı.
settings-advanced-log-level-error = Yalnızca hatalar
settings-advanced-log-level-warn = Uyarılar
settings-advanced-log-level-info = Bilgi
settings-advanced-log-level-debug = Hata ayıklama
settings-advanced-log-level-trace = Her şey
settings-advanced-developer-mode = Geliştirici modu
settings-advanced-developer-mode-help = Dil listesindeki sözde diller gibi { -app-name } geliştirme ve test araçlarını göster.
# machine-drafted, needs native review
settings-advanced-audit-log = Yerel denetim günlüğü
# machine-drafted, needs native review
settings-advanced-audit-log-help = Açılan tıbbi görüntüleri, dışa aktarmaları, anonimleştirmeleri, yazdırmaları ve PACS’e gönderimleri veri klasöründeki audit.jsonl dosyasına kaydet. Bu bilgisayarda kalır.
settings-advanced-hw-report = Donanım raporu…
settings-advanced-hw-report-help = { -app-name }'ın hangi CPU ve GPU özelliklerini algıladığını ve hangi hızlı yolları kullandığını göster.
# machine-drafted, needs native review
settings-advanced-renderer-info = Oluşturucu bilgileri…
settings-advanced-renderer-info-help = { -app-name } uygulamasının kullandığı grafik bağdaştırıcısını, sürücüyü, ekran biçimlerini ve bellek sınırlarını, bir hata raporuna kopyalanmaya hazır olarak gösterir.
settings-advanced-edit-settings-file = settings.toml dosyasını düzenle
settings-advanced-edit-settings-file-help = Ayarlar dosyasını { -app-name } uygulamasının kendi düzenleyicisinde düzenleyin. Yazarken dosyayı denetler ve yalnızca hata yoksa kaydeder.
settings-advanced-import = Ayarları içe aktar…
settings-advanced-import-help = { -app-name }'dan dışa aktarılmış bir dosyadan ayarları yükle.
settings-advanced-export = Ayarları dışa aktar…
settings-advanced-export-help = Yedekleme veya başka bir bilgisayar için tüm ayarları tek bir dosyaya kaydet.
settings-advanced-reset-all = Tüm ayarları sıfırla…
settings-advanced-reset-all-help = Her ayarı varsayılanına döndür. Bu işlem geri alınabilir.

## settings.toml editor (machine-drafted, needs native review)

config-editor-title = settings.toml
config-editor-save = Kaydet
config-editor-revert = Geri al
config-editor-close = Kapat
config-editor-modified = Değiştirildi
config-editor-text = Ayarlar dosyası
config-editor-position = Satır { $line }, sütun { $column }
config-editor-saved = settings.toml kaydedildi ve uygulandı.
config-editor-cant-save =
    { $count ->
        [one] Kaydedilemiyor: 1 sorun var. Önce düzeltin.
       *[other] Kaydedilemiyor: { $count } sorun var. Önce düzeltin.
    }
config-editor-save-failed = settings.toml kaydedilemedi: { $reason }
config-editor-conflict = settings.toml başka bir yerde değiştirildi.
config-editor-reload = Yeniden yükle
config-editor-keep-mine = Benimkini koru
config-editor-discard-question = settings.toml dosyasındaki kaydedilmemiş değişiklikler atılsın mı?
config-editor-discard = At
config-editor-keep-editing = Düzenlemeye devam et
config-editor-find-placeholder = Dosyada bul
config-editor-find-count = { $current } / { $total }
config-editor-find-none = Eşleşme yok
config-editor-find-previous = Önceki eşleşme
config-editor-find-next = Sonraki eşleşme
config-editor-find-close = Aramayı kapat
config-editor-problems = Sorunlar
config-editor-problems-count =
    { $count ->
        [0] Sorun yok
        [one] 1 sorun
       *[other] { $count } sorun
    }
config-editor-no-problems = Sorun yok. Dosya kaydedilebilir.
config-editor-problem-at = Satır { $line }: { $message }
config-editor-severity-error = Hata
config-editor-severity-warning = Uyarı
config-editor-severity-info = Not
config-editor-problem-syntax = Geçerli TOML değil: { $reason }
config-editor-problem-duplicate = { $key } aynı tabloda iki kez ayarlanmış. Birini kaldırın.
config-editor-problem-unknown-section = [{ $key }] adlı bir ayar bölümü yok.
config-editor-problem-unknown-section-suggestion = [{ $key }] adlı bir ayar bölümü yok. [{ $suggestion }] mi demek istediniz?
config-editor-problem-unknown-key = { $key } adlı bir ayar yok.
config-editor-problem-unknown-key-suggestion = { $key } adlı bir ayar yok. { $suggestion } mi demek istediniz?
config-editor-problem-invalid = { $key } için geçersiz değer: { $reason }
config-editor-problem-migrated = { $key }, { -app-name } uygulamasının eski bir sürümünün yazdığı biçimde. Okunurken dönüştürülür.
config-editor-problem-newer-version = Bu dosya { -app-name } uygulamasının daha yeni bir sürümü tarafından yazıldı (biçim { $found }; bu sürüm { $supported } okur). Bilinmeyen ayarlar korunur.
config-editor-problem-command = [[commands]] içindeki { $index }. komut: { $reason }
config-editor-problem-hook = [[hooks]] içindeki { $index }. kanca: { $reason }

## keymap.toml in the same editor (machine-drafted, needs native review)

config-editor-keymap-title = keymap.toml
config-editor-keymap-text = Tuş atamaları dosyası
config-editor-keymap-saved = keymap.toml kaydedildi ve uygulandı.
config-editor-keymap-save-failed = keymap.toml kaydedilemiyor: { $reason }
config-editor-keymap-conflict = keymap.toml başka bir yerde değişti.
config-editor-keymap-discard-question = keymap.toml dosyasındaki kaydedilmemiş değişiklikleriniz atılsın mı?
config-editor-new-file = yeni dosya, kaydedilince oluşturulur
config-editor-no-folder = kaydedilecek bir ayarlar klasörü yok.

## Reset all settings confirmation (machine-drafted, needs native review)

settings-reset-all-confirm = Tüm ayarlar varsayılanlarına sıfırlansın mı?
settings-reset-all-confirm-detail = Bu, Ayarlar’ın tüm bölümlerini etkiler. Daha sonra geri alınabilir.
settings-reset-all-confirm-ok = Tümünü sıfırla
settings-reset-all-done = Tüm ayarlar varsayılanlarına sıfırlandı.

## Hardware video encoding (machine-drafted, needs native review)
paths-engine-hw-encoder = Donanım kodlayıcı

## Path timing units (machine-drafted, needs native review)
settings-paths-checking-before-use = Kullanılmadan önce CPU ile karşılaştırılarak denetlenir: { $engine }.
settings-paths-unit-thumbnail = Süreler { $size } boyutunda bir küçük resim içindir
settings-paths-unit-frame = Süreler { $size } boyutunda video karesi başınadır
# machine-drafted, needs native review
settings-metadata-develop-calibration = Kalibrasyon kareleri klasörü
# machine-drafted, needs native review
settings-metadata-develop-calibration-help = Geliştirme için flat ve dark kareler. Dosya veya klasör adları "flat" ya da "dark" içerir.

## Colour swatches under a colour setting's field (machine-drafted, needs native review)
settings-colour-white = Beyaz
settings-colour-light-grey = Açık gri
settings-colour-grey = Gri
settings-colour-dark-grey = Koyu gri
settings-colour-black = Siyah
settings-colour-yellow = Sarı
settings-colour-orange = Turuncu
settings-colour-red = Kırmızı
settings-colour-magenta = Macenta
settings-colour-blue = Mavi
settings-colour-cyan = Camgöbeği
settings-colour-green = Yeşil
