# Compare: split, signed difference, flicker, alignment, metrics and the pixel probe
# (DESIGN.md §6.3).

## Actions (palette, cheat sheet, remote control)
palette-compare-with = Şununla karşılaştır…
palette-compare-with-help = Bu görüntüyü seçtiğiniz başka bir görüntüyle karşılaştır.
palette-compare-side = Yan yana karşılaştır
palette-compare-side-help = Karşılaştırılan görüntüleri yan yana göster.
palette-compare-split = Bölücüyle karşılaştır
palette-compare-split-help = İkinci görüntüyü birincinin üzerinde, sürükleyebileceğiniz bir bölücüyle kesilmiş göster.
palette-compare-split-direction = Bölücüyü döndür
palette-compare-split-direction-help = Bölücüyü dikey yerine yatay koy, ya da geri al.
palette-compare-overlay = Bindirme olarak karşılaştır
palette-compare-overlay-help = İkinci görüntüyü birincinin üzerinde, karıştırarak göster.
palette-compare-difference = Farkı karşılaştır
palette-compare-difference-help = Görüntülerin nerede farklı olduğunu göster; sonraki tür için yeniden.
palette-compare-flicker = Karşılaştırılan görüntüler arasında titret
palette-compare-flicker-help = Diğer görüntüyü hemen göster.
palette-compare-auto-flicker = Otomatik titret
palette-compare-auto-flicker-help = İki görüntü arasında kendiliğinden geçiş yap, ya da durdur.
palette-compare-align = Karşılaştırılan görüntüleri hizala
palette-compare-align-help = Farklı boyuttaki görüntüleri hizala: sol üste, ortaya, ölçekleyerek ya da otomatik.
palette-compare-metric-space = Ölçüm uzayını değiştir
palette-compare-metric-space-help = Doğrusal ışıkta ya da kodlanmış sRGB’de ölç.
palette-compare-whole-image = Tüm görüntüyü ölç
palette-compare-whole-image-help = Görünen kısım yerine tüm görüntüyü ölç, ya da geri al.
palette-compare-probe = Karşılaştırma piksel sondası
palette-compare-probe-help = İşaretçinin altındaki iki görüntünün değerlerini göster.
palette-compare-link = Karşılaştırılan görüntüleri bağla ya da ayır
palette-compare-link-help = İki görüntünün her birini ayrı ayrı yakınlaştır ve kaydır, ya da yeniden birlikte.
palette-compare-close = Karşılaştırmayı kapat
palette-compare-close-help = Galeriye geri dön.
disabled-compare-closed = Önce karşılaştırmayı açın
disabled-compare-not-two = İki görüntüyle çalışır

## Cheat sheet
cheatsheet-compare-split = Bölücü veya yan yana
cheatsheet-compare-split-direction = Bölücü yatay veya dikey
cheatsheet-compare-auto-flicker = Otomatik titret
cheatsheet-compare-align = Sonraki hizalama
cheatsheet-compare-space = Ölçüm uzayı
cheatsheet-compare-whole = Tüm görüntü veya görünen kısım
cheatsheet-compare-probe = Piksel sondası
cheatsheet-compare-link = İki görüntüyü bağla veya ayır

## Settings
settings-gallery-compare-metric-space = Karşılaştırma ölçüm uzayı
settings-gallery-compare-metric-space-help = Karşılaştırmanın MAE, MSE, PSNR ve SSIM’i nerede ölçtüğü. Doğrusal ışık ekranın gösterdiğidir. sRGB, 8-bit dosyalarda diğer araçlarla eşleşir.
settings-gallery-compare-metric-space-linear = Doğrusal ışık
settings-gallery-compare-metric-space-srgb = Kodlanmış sRGB
settings-gallery-compare-flicker-limit = En hızlı otomatik titreşim
settings-gallery-compare-flicker-limit-help = Saniyede en fazla görüntü değişimi. 3’ün üzerinde yanıp sönme nöbetleri tetikleyebilir.

## Toolbar and captions
compare-split = Bölme
compare-split-down = Bölücü dikey
compare-split-across = Bölücü yatay
compare-link = Bağlı
compare-auto-flicker = Otomatik
compare-flicker-rate = Saniyede { $rate } değişim
compare-gain = Kazanç ×{ $gain }
compare-probe = Sonda
compare-kind-signed = İşaretli hata
compare-kind-on-image = Görüntü üzerinde hata
compare-align-top-left = Sol üst
compare-align-centre = Ortalanmış
compare-align-scaled = B, A’ya ölçeklendi
compare-align-auto = Otomatik hizalama
compare-space-linear = Doğrusal ışık
compare-space-srgb = sRGB
compare-space-help-linear = Ton eşlemeden sonra gösterildiği gibi doğrusal ışıkta ölçüldü. Buradaki PSNR, alışılmış 8-bit sRGB PSNR’ı değildir.
compare-space-help-srgb = Diğer araçların 8-bit dosyalarda yaptığı gibi kodlanmış sRGB değerleri üzerinde ölçüldü. SDR beyazının üzerindeki HDR beyaz sayılır.
compare-space-help-common = ΔE2000 her zaman CIELAB (D65), SDR beyazı L* 100, HDR kırpılmış. { $size } pikselden büyük görüntüler { $size } piksellik bir kopyadan ölçülür.
compare-region-view = Görünen kısım
compare-region-whole = Tüm görüntü
compare-ssim = SSIM { $ssim }
compare-ssim-small = SSIM için 11 piksel gerekir
compare-delta-e = ΔE { $mean } ({ PERCENT(95) }: { $p95 }, en çok { $max })
compare-delta-e-hdr = ΔE için HDR kırpıldı
compare-aligned-centre = Ortalandı, { $width } × { $height } karşılaştırıldı
compare-aligned-scaled = B { $scale }× ölçeklendi, { $width } × { $height } karşılaştırıldı
compare-aligned-auto = Hizalandı { $angle }° { $scale }×, { $width } × { $height } karşılaştırıldı
compare-aligning = Hizalanıyor…
compare-align-failed = Bu görüntüler hizalanamadı. Sol üste dönüldü.
compare-split-label = { $b }, { $a } üzerinde, bölücü { PERCENT($percent) } konumunda
compare-showing-which = Gösterilen { $which }: { $name }
compare-auto-flicker-on = Titriyor, saniyede { $rate } değişim
compare-auto-flicker-off = Titreşim durdu
compare-linked = Görüntüler bağlandı
compare-unlinked = Görüntüler ayrıldı: her biri kendi başına kayar
compare-with-title = Şununla karşılaştır
compare-probe-delta-e = ΔE2000
compare-probe-a11y = A: { $a }. B: { $b }. ΔE2000 { $delta }.
compare-a11y-divider = Bölme çizgisi
compare-a11y-space = Ölçüm uzayı
compare-a11y-align = Hizalama
