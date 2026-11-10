# Specular — Batch convert (DESIGN.md §6.5). machine-drafted, needs native review
# Every output is a new file; the sources are never changed.

palette-convert = Dönüştür…

## Settings → Convert
settings-section-convert = Dönüştürme
settings-convert-format = Biçim
settings-convert-format-help = Dönüştür… komutunun yazdığı biçim. PNG ve TIFF kayıpsızdır; WebP ve JPEG XL ikisi de olabilir.
settings-convert-format-jpeg = JPEG
settings-convert-format-png = PNG
settings-convert-format-webp = WebP
settings-convert-format-jxl = JPEG XL
settings-convert-format-tiff = TIFF
settings-convert-jpeg-quality = JPEG kalitesi
settings-convert-jpeg-quality-help = Daha yüksek değer, daha büyük dosyalarda daha fazla ayrıntı korur. 90'da özgün görüntüden ayırt etmek zordur.
settings-convert-jpeg-full-chroma = JPEG'de tam çözünürlüklü renk
settings-convert-jpeg-full-chroma-help = Rengi yarım (4:2:0) yerine tam çözünürlükte (4:4:4) tutar: daha keskin renk kenarları, daha büyük dosyalar.
settings-convert-png-effort = PNG sıkıştırması
settings-convert-png-effort-help = Daha yüksek değer daha küçük dosyalar üretir, ama daha yavaş. 1 en hızlısıdır.
settings-convert-jxl-effort = JPEG XL eforu
settings-convert-jxl-effort-help = Daha yüksek değer daha küçük dosyalar üretir, ama daha yavaş.
settings-convert-tiff-compression = TIFF sıkıştırması
settings-convert-tiff-compression-help = Deflate kayıpsızdır ve her TIFF okuyucu tarafından okunur.
settings-convert-tiff-compression-deflate = Deflate
settings-convert-tiff-compression-none = Yok
settings-convert-depth = Bit derinliği
settings-convert-depth-help = Otomatik, görüntüde 8'den fazla bit varsa ve biçim bunları taşıyabiliyorsa örnek başına 16 bit yazar (PNG, TIFF, JPEG XL).
settings-convert-depth-auto = Otomatik
settings-convert-depth-8 = 8 bit
settings-convert-depth-16 = 16 bit
settings-convert-resize = Yeniden boyutlandır
settings-convert-resize-help = Görüntüler doğrusal ışıkta yüksek kaliteli bir filtreyle küçültülür.
settings-convert-resize-none = Özgün boyut
settings-convert-resize-fit = İçine sığdır
settings-convert-resize-long-edge = Uzun kenar
settings-convert-resize-percent = Yüzde
settings-convert-width = Genişlik
settings-convert-width-help = İçine sığdır seçeneğinde bir görüntünün alabileceği en büyük genişlik.
settings-convert-height = Yükseklik
settings-convert-height-help = İçine sığdır seçeneğinde bir görüntünün alabileceği en büyük yükseklik.
settings-convert-long-edge = Uzun kenar
settings-convert-long-edge-help = Uzun kenar seçeneğinde görüntünün uzun kenarının uzunluğu.
settings-convert-percent = Yüzde
settings-convert-percent-help = Yüzde seçeneğinde özgün görüntüye göre boyut.
settings-convert-upscale = Küçük görüntüleri büyüt
settings-convert-upscale-help = Kapalıyken, istenen boyuttan zaten küçük olan görüntüler boyutlarını korur.
settings-convert-colour = Renk
settings-convert-colour-help = Koru, görüntünün renk profilini onunla birlikte saklar. sRGB'ye dönüştür, web ve renk yönetimi olmayan cihazlar için uygundur.
settings-convert-colour-keep = Profili koru
settings-convert-colour-srgb = sRGB'ye dönüştür
settings-convert-hdr = HDR görüntüler
settings-convert-hdr-help = PNG ve JPEG XL, HDR'yi olduğu gibi korur; diğer biçimler her zaman ton eşlemeyle SDR'ye dönüştürülür.
settings-convert-hdr-keep = Mümkünse koru
settings-convert-hdr-tone-map = SDR'ye ton eşle
settings-convert-metadata = Meta veriler
settings-convert-metadata-help = Kamera, çekim tarihi ve konum gibi Exif ve XMP bilgileri. Renk profili her zaman korunur.
settings-convert-metadata-all = Tümünü koru
settings-convert-metadata-no-location = Konum dışında tümü
settings-convert-metadata-strip = Kaldır
settings-convert-folder = Çıktı klasörü
settings-convert-folder-help = Dönüştürülen dosyaların gittiği yer. Ayarlanmamışsa her görüntünün yanındaki bir alt klasöre gider.
settings-convert-folder-unset = Görüntülerin yanında alt klasör
settings-convert-subfolder = Alt klasör adı
settings-convert-subfolder-help = Çıktı klasörü ayarlanmadığında kullanılan alt klasör. Boş: görüntülerin kendi klasörü.
settings-convert-pattern = Dosya adları
settings-convert-pattern-help = {"{"}name{"}"} özgün ad, {"{"}n:03{"}"} toplu işlemdeki sıra numarası, {"{"}date:%Y-%m-%d{"}"} çekim tarihi, {"{"}w{"}"} ve {"{"}h{"}"} yeni boyut, {"{"}ext{"}"} biçimin uzantısıdır.
settings-convert-conflicts = Bir ad kullanılıyorsa
settings-convert-conflicts-help = Var olan dosyalar asla değiştirilmez.
settings-convert-conflicts-suffix = Numara ekle
settings-convert-conflicts-skip = Görüntüyü atla

## The dialog
convert-title = Dönüştür
convert-images = Görüntüler
convert-images-current = Bu görüntü
convert-images-selection = Seçim ({ $count })
convert-images-folder = Klasör ({ $count })
convert-images-picked = Seçilenler ({ $count })
convert-quality = Kalite
convert-chroma = Renk ayrıntısı
convert-chroma-half = 4:2:0 (daha küçük)
convert-chroma-full = 4:4:4 (daha keskin)
convert-effort = Efor
convert-on = Açık
convert-off = Kapalı
convert-pixels = { $value } piksel
convert-effort-fastest = { $value } (en hızlı)
convert-folder-subfolder = “{ $name }” alt klasörü
convert-folder-same = Görüntüyle aynı klasör
convert-choose-folder = Klasör seç…
convert-name = Ad
convert-name-preview = { $source } → { $output }
convert-name-error-unclosed = Bir {"{"} kapatılmamış.
convert-name-error-stray = Bir {"}"} için {"{"} yok. Süslü parantez için {"}}"} yazın.
convert-name-error-unknown = { $token } bir ad parçası değil. name, n, date, w, h veya ext kullanın.
convert-name-error-format = { $token } biçimi hatalı.
convert-name-error-empty = Bir ad kalıbı girin.
convert-start = Dönüştür
convert-stop = Durdur
convert-hide = Gizle
convert-close = Kapat
convert-progress = Dönüştürülüyor: { $done }/{ $total }
convert-progress-eta = Dönüştürülüyor: { $done }/{ $total } · yaklaşık { $time } kaldı
convert-eta-seconds = { $count ->
    [one] { $count } saniye
   *[other] { $count } saniye
}
convert-eta-minutes = { $count ->
    [one] { $count } dakika
   *[other] { $count } dakika
}
convert-done = { $count ->
    [one] { $count } görüntü dönüştürüldü
   *[other] { $count } görüntü dönüştürüldü
}
convert-done-skipped = { $count } atlandı: ad kullanılıyordu
convert-done-failed = { $count } başarısız
convert-stopped = { $count } görüntüden sonra durduruldu
convert-failure = { $file }: { $reason }
convert-failure-open = açılamıyor ({ $error })
convert-failure-decode = okunamıyor ({ $error })
convert-failure-encode = bu biçimde yazılamıyor ({ $error })
convert-failure-write = kaydedilemiyor ({ $error })
convert-failure-same = yeni dosya özgün dosyanın yerine geçerdi
convert-report-more = ve { $count } tane daha
convert-running = Zaten bir dönüştürme çalışıyor.

## Convert encoders (machine-drafted, needs native review)
settings-convert-format-avif = AVIF
settings-convert-webp-lossless = Kayıpsız WebP
settings-convert-webp-lossless-help = Her pikseli tam olarak korur. Kapalıyken WebP dosyaları WebP kalitesinde çok daha küçüktür.
settings-convert-webp-quality = WebP kalitesi
settings-convert-webp-quality-help = Daha yüksek değer, daha büyük dosyalarda daha fazla ayrıntı korur. WebP kayıpsız değilken kullanılır.
settings-convert-webp-effort = WebP eforu
settings-convert-webp-effort-help = Daha yüksek değer daha küçük dosyalar üretir, ama daha yavaş. WebP kayıpsız değilken kullanılır.
settings-convert-jxl-lossless = Kayıpsız JPEG XL
settings-convert-jxl-lossless-help = Her pikseli tam olarak korur. Kapalıyken JPEG XL dosyaları JPEG XL kalitesinde çok daha küçüktür.
settings-convert-jxl-quality = JPEG XL kalitesi
settings-convert-jxl-quality-help = Daha yüksek değer, daha büyük dosyalarda daha fazla ayrıntı korur. 90'da özgün görüntüden ayırt etmek zordur. JPEG XL kayıpsız değilken kullanılır.
settings-convert-avif-quality = AVIF kalitesi
settings-convert-avif-quality-help = Daha yüksek değer, daha büyük dosyalarda daha fazla ayrıntı korur.
settings-convert-avif-effort = AVIF eforu
settings-convert-avif-effort-help = Daha yüksek değer daha küçük dosyalar üretir, ama daha yavaş. 1 en hızlısıdır.
convert-lossless = Kayıpsız
