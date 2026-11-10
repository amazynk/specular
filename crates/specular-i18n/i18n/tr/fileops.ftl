# machine-drafted, needs native review
# Dosya işlemleri: yeniden adlandırma, toplu yeniden adlandırma, klasöre kopyalama ve taşıma,
# dosyaya kaydedilen döndürme, geri alma ve yineleme, duvar kâğıdı, paylaşma, e-postayla
# gönderme, ayarları dışa ve içe aktarma.

## Palet, kısayol listesi ve menüler

palette-undo = Geri al
palette-undo-help = Dosyalardaki son değişikliği geri alır: çöp kutusuna taşıma, yeniden adlandırma, kopyalama veya taşıma, döndürme.
palette-redo = Yinele
palette-redo-help = Son geri alınan işlemi yeniden yapar.
palette-rename = Yeniden adlandır…
palette-rename-help = Dosyaya yeni bir ad verir; yan dosyaları ve RAW+JPEG eşi de birlikte değişir.
palette-batch-rename = Dosyaları yeniden adlandır…
palette-batch-rename-help = Seçili dosyaları veya tüm klasörü bir kalıpla, önizleyerek yeniden adlandırır.
palette-copy-to-folder = Klasöre kopyala…
palette-copy-to-folder-help = Dosyayı veya seçimi başka bir klasöre kopyalar.
palette-move-to-folder = Klasöre taşı…
palette-move-to-folder-help = Dosyayı veya seçimi başka bir klasöre taşır.
palette-rotate-file-left = Dosyayı sola döndür
palette-rotate-file-left-help = Dosyanın kendisini çeyrek tur sola döndürür; biçim izin veriyorsa kayıpsız.
palette-rotate-file-right = Dosyayı sağa döndür
palette-rotate-file-right-help = Dosyanın kendisini çeyrek tur sağa döndürür; biçim izin veriyorsa kayıpsız.
palette-set-wallpaper-help = Görüntüyü masaüstü arka planı yapar.
palette-share = Paylaş…
palette-share-help = Dosyayı sistemin paylaşım menüsüyle başka bir uygulamaya gönderir.
palette-send-by-email = E-postayla gönder…
palette-send-by-email-help = E-posta uygulamanızda dosyanın ekli olduğu yeni bir e-posta başlatır.
# machine-drafted, needs native review
palette-share-with-zynk = Zynk ile paylaş…
palette-share-with-zynk-help = Zynk uygulamasında dosya için bir web bağlantısı oluşturur.
palette-send-with-zynk = Zynk ile gönder…
palette-send-with-zynk-help = Dosyayı Zynk ile bir kişiye veya cihazlarınızdan birine gönderir.
palette-export-settings = Ayarları dışa aktar…
palette-export-settings-help = Tüm ayarları ve tuş atamalarınızı bir dosyaya kaydeder.
palette-import-settings = Ayarları içe aktar…
palette-import-settings-help = Neyin değişeceğini gösterdikten sonra ayarları bir dosyadan okur.

disabled-in-archive-file = Bu sayfa bir arşivin içinde ve kendine ait bir dosyası yok
disabled-not-rotatable = Bu tür dosya bir döndürmeyi kaydedemez
disabled-not-on-this-desktop = Masaüstünüz bunu sunmuyor

## Ayarlar

settings-browsing-rotate-trim-edges = Kayıpsız döndürmek için JPEG kenarlarını kırp
settings-browsing-rotate-trim-edges-help = Boyutu 8 veya 16 pikselin katı olmayan bir JPEG, ancak son birkaç pikseli kesilerek kayıpsız döndürülebilir. Bu kapalıyken bunun yerine yönlendirme etiketi değiştirilir ve kırpma her seferinde önerilir.
settings-browsing-rotate-keep-modified = Döndürürken değiştirilme tarihini koru
settings-browsing-rotate-keep-modified-help = Döndürülen dosya son değiştirilme tarihini korur; böylece tarihe göre sıralamada yeri değişmez.
settings-browsing-recent-folders = Son klasörler
settings-browsing-recent-folders-help = Kopyalama ve taşıma için son kullanılan klasörler.

## Pencereler

fileop-cancel = İptal
rename-title = Yeniden adlandır
rename-field-label = Ad
rename-button = Yeniden adlandır
rename-sidecars-follow = Bunlar da yeniden adlandırılır: { $files }
rename-extension-changed = Uzantıyı değiştirmek dosyayı dönüştürmez; diğer uygulamalar açamayabilir.
rename-problem-empty = Bir ad yazın.
rename-problem-invalid-char = Adlar “{ $char }” içeremez.
rename-problem-reserved = “{ $name }”, Windows’un aygıtlar için ayırdığı bir addır.
rename-problem-too-long = Bu ad çok uzun; adlar en fazla 255 bayt olabilir.
rename-problem-dot = “.” ve “..” ad olarak kullanılamaz.
rename-problem-trailing = Adlar boşluk veya noktayla bitemez.
rename-problem-exists = Bu ad başka bir dosyada zaten var.
rename-problem-read-only = Bu klasör değiştirilemez.
rename-problem-duplicate = İki dosya bu adı alırdı.

batch-rename-title =
    { $count ->
        [one] { $count } dosyayı yeniden adlandır
       *[other] { $count } dosyayı yeniden adlandır
    }
batch-rename-field-label = Kalıp
batch-rename-tokens = { "{" }name{ "}" }, { "{" }n:3{ "}" } (001, 002, …), { "{" }date:%Y-%m-%d{ "}" }, { "{" }camera{ "}" }, { "{" }w{ "}" }×{ "{" }h{ "}" } ve { "{" }ext{ "}" } kullanın.
batch-rename-summary =
    { $count ->
        [one] { $count } dosya yeniden adlandırılacak
       *[other] { $count } dosya yeniden adlandırılacak
    }{ $problems ->
        [0] .
        [one] ; { $problems } dosya adlandırılamıyor ve olduğu gibi kalıyor.
       *[other] ; { $problems } dosya adlandırılamıyor ve olduğu gibi kalıyor.
    }
batch-rename-reading = Tarihler ve kameralar okunuyor…
batch-rename-preview = Önizleme
batch-rename-unchanged = adı değişmez
batch-rename-more =
    { $count ->
        [one] aşağıda { $count } tane daha
       *[other] aşağıda { $count } tane daha
    }

transfer-copy-title-one = “{ $file }” dosyasını bir klasöre kopyala
transfer-copy-title =
    { $count ->
        [one] { $count } dosyayı bir klasöre kopyala
       *[other] { $count } dosyayı bir klasöre kopyala
    }
transfer-move-title-one = “{ $file }” dosyasını bir klasöre taşı
transfer-move-title =
    { $count ->
        [one] { $count } dosyayı bir klasöre taşı
       *[other] { $count } dosyayı bir klasöre taşı
    }
transfer-recent = Son klasörler
transfer-folder = Klasör
transfer-choose-folder = Klasör seç…
transfer-copy-dialog = Klasöre kopyala
transfer-move-dialog = Klasöre taşı
transfer-sidecars =
    { $count ->
        [one] { $count } yan dosya da birlikte gider.
       *[other] { $count } yan dosya da birlikte gider.
    }
transfer-taken =
    { $count ->
        [one] { $folder } içinde { $count } ad zaten kullanılıyor.
       *[other] { $folder } içinde { $count } ad zaten kullanılıyor.
    }
transfer-when-taken = Bir ad zaten kullanılıyorsa
transfer-skip = Atla
transfer-keep-both = İkisini de sakla
transfer-replace-option = Değiştir
transfer-copy = Kopyala
transfer-move = Taşı
transfer-replace-title =
    { $count ->
        [one] { $folder } içindeki { $count } dosya değiştirilsin mi?
       *[other] { $folder } içindeki { $count } dosya değiştirilsin mi?
    }
transfer-replace-note = Oradaki mevcut dosyalar çöp kutusuna gider. Geri al onları geri getirir.
transfer-replace = Değiştir
transfer-back = Geri
transfer-stop = Durdur

settings-file-import-title = Ayarları içe aktar
settings-file-import-dialog = Ayarları içe aktar
settings-file-export-dialog = Ayarları dışa aktar
settings-file-import-changes = Değişiklikler
settings-file-import-change = { $before } → { $after }
settings-file-import-no-key = tuş yok
settings-file-import-unknown = Bu sürümde böyle bir ayar yok; atlandı.
settings-file-import-keymap = Tuş atamaları
settings-file-import-nothing = Hiçbir şey değişmez: bu ayarlar zaten geçerli.
settings-file-import-summary =
    { $count ->
        [one] { $count } değişiklik. İçe aktarma Ayarlar’da geri alınabilir.
       *[other] { $count } değişiklik. İçe aktarma Ayarlar’da geri alınabilir.
    }
settings-file-import-secrets =
    { $count ->
        [one] { $count } girdi bir parola veya anahtara benziyor ve asla içe aktarılmaz.
       *[other] { $count } girdi parola veya anahtara benziyor ve asla içe aktarılmaz.
    }
settings-file-import-commands =
    { $count ->
        [one] { $count } kullanıcı komutu içe aktarılmadı: komutlar program başlatır. Güveniyorsanız ayar dosyanıza kendiniz kopyalayın.
       *[other] { $count } kullanıcı komutu içe aktarılmadı: komutlar program başlatır. Güveniyorsanız ayar dosyanıza kendiniz kopyalayın.
    }
settings-file-import-button = İçe aktar

## Sonuçlar

fileop-renamed = Yeni ad: { $file }
fileop-renamed-count =
    { $count ->
        [one] { $count } dosya yeniden adlandırıldı
       *[other] { $count } dosya yeniden adlandırıldı
    }
fileop-copied =
    { $count ->
        [one] { $count } dosya { $folder } klasörüne kopyalandı
       *[other] { $count } dosya { $folder } klasörüne kopyalandı
    }
fileop-moved =
    { $count ->
        [one] { $count } dosya { $folder } klasörüne taşındı
       *[other] { $count } dosya { $folder } klasörüne taşındı
    }
fileop-skipped =
    { $count ->
        [one] { $count } dosya atlandı: adı orada zaten kullanılıyor.
       *[other] { $count } dosya atlandı: adları orada zaten kullanılıyor.
    }
fileop-stopped = Durduruldu; yapılanlar yapılmış olarak kalır.
fileop-failed-file =
    { $count ->
        [one] { $file } kopyalanamadı veya taşınamadı: { $reason }
       *[other] { $count } dosya kopyalanamadı veya taşınamadı; ilki, { $file }: { $reason }
    }
fileop-progress-copy =
    { $count ->
        [one] { $count } dosya kopyalanıyor… { PERCENT($percent) }
       *[other] { $count } dosya kopyalanıyor… { PERCENT($percent) }
    }
fileop-progress-move =
    { $count ->
        [one] { $count } dosya taşınıyor… { PERCENT($percent) }
       *[other] { $count } dosya taşınıyor… { PERCENT($percent) }
    }
fileop-done-status = Bitti
fileop-busy = Son dosya değişikliği hâlâ sürüyor; biraz sonra yeniden deneyin.
fileop-nothing-to-redo = Yinelenecek bir şey yok
fileop-undone = Geri alındı: { $what }
fileop-redone = Yinelendi: { $what }
fileop-kind-rename = yeniden adlandırma
fileop-kind-batch-rename = birkaç dosyanın yeniden adlandırılması
fileop-kind-move = klasöre taşıma
fileop-kind-copy = klasöre kopyalama
fileop-kind-rotate = döndürme
rename-failed = Yeniden adlandırılamadı: { $reason }

undo-problem-changed = Geri alınamıyor: { $file } o zamandan beri değişti ve geri almak bu değişikliği kaybettirir.
undo-problem-missing = Geri alınamıyor: { $file } artık yok.
undo-problem-taken = Geri alınamıyor: artık başka bir dosyanın adı { $file }.
undo-problem-io = { $file } üzerindeki değişiklik geri alınamıyor: { $reason }

rotate-done-lossless = { $file } hiç kayıp olmadan döndürüldü.
rotate-done-lossless-trimmed = { $file } hiç kayıp olmadan döndürüldü; kenarı { $width } × { $height } olacak şekilde kırpıldı.
rotate-done-tag = { $file }, yönlendirme etiketi değiştirilerek döndürüldü.
rotate-done-sidecar = { $file } dosyasının döndürmesi { $sidecar } içine kaydedildi; RAW dosyasının kendisi asla değiştirilmez.
rotate-tag-unreadable = Görüntü verisi döndürmeye yetecek kadar okunamadı.
rotate-tag-multi-picture = Birden fazla görüntü içeriyor; ilkini döndürmek diğerlerini bozar.
rotate-tag-trailer = Görüntünün ardından veri geliyor; döndürmek bu veriyi kaydırır.
rotate-tag-not-aligned = Pikselleri kayıpsız döndürmek onu { $width } × { $height } boyutuna kırpar.
rotate-trim-offer = Kırp ve döndür
rotate-problem-unsupported = { $file } döndürme kaydedemeyen türde bir dosya.
rotate-problem-too-large = { $file } döndürmek için çok büyük.
rotate-problem-damaged = { $file } yönlendirmenin tutulduğu yerde hasarlı: { $reason }
rotate-problem-io = { $file } döndürülemedi: { $reason }

share-failed = Paylaşılamadı: { $reason }
email-failed = E-posta uygulamanız başlatılamadı: { $reason }
# machine-drafted, needs native review
zynk-opening = Zynk açılıyor…
zynk-failed = Zynk’e ulaşılamadı: { $reason }
zynk-get-title-send = “Zynk ile gönder” için Zynk uygulaması gerekir
zynk-get-title-share = “Zynk ile paylaş” için Zynk uygulaması gerekir
zynk-get-about = Zynk; Mac, Windows, Linux, iPhone ve Android için ücretsiz bir uygulamadır. Fotoğrafları, videoları ve klasörlerin tamamını, boyutu ne olursa olsun, doğrudan diğer cihazlarınıza ve başka kişilere gönderir. Aktarımlar uçtan uca şifrelenir ve bağlantı koparsa kaldığı yerden devam eder. Zynk’i olmayan bir alıcı için, dosyaları tarayıcıdan indirebileceği bir web bağlantısı oluşturur.
zynk-get-macos = Mac App Store’dan edinin. Kurulduktan sonra { -app-name } dosyalarınızı doğrudan ona aktarır.
zynk-get-windows = Microsoft Store’dan edinin. Kurulduktan sonra { -app-name } dosyalarınızı doğrudan ona aktarır.
zynk-get-linux = Linux için Zynk bir AppImage’dır, kurulum gerektirmez: indirin, ~/Applications klasörüne koyun ve çalıştırılabilir yapın (dosyanın Özellikler’inden ya da chmod +x ile). { -app-name } onu orada bulur; Zynk çalışırken başka bir yerde olsa da bulur.
zynk-get = Zynk’i edinin
zynk-send-old-windows = “Zynk ile gönder” için Windows 10 sürüm 2004 veya üstü gerekir. “Zynk ile paylaş” bu Windows’ta çalışır.
settings-file-export-done = Ayarlar { $file } dosyasına kaydedildi
settings-file-export-failed = Ayarlar kaydedilemedi: { $reason }
settings-file-import-unreadable = { $file } okunamadı: { $reason }
settings-file-import-done =
    { $count ->
        [one] { $count } değişiklik içe aktarıldı
       *[other] { $count } değişiklik içe aktarıldı
    }
