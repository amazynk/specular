# machine-drafted, needs native review
# Specular — Ayıklama modu (DESIGN.md §6.3).

palette-cull = Ayıklama moduna gir veya çık
cheatsheet-cull = Ayıklama modu
settings-section-cull = Ayıklama modu
settings-cull-auto-enter = RAW klasörlerini ayıkla
settings-cull-auto-enter-help = Çoğunlukla RAW dosyası içeren bir klasörü doğrudan ayıklama modunda aç.
settings-cull-auto-advance = İşaretledikten sonra sonraki görüntü
settings-cull-auto-advance-help = Seçme, reddetme veya yıldız puanından sonra ilerle. Etiketler ve işaret kaldırma görüntüde kalır.
settings-cull-show = RAW+JPEG çiftlerinde göster
settings-cull-show-help = Aynı adlı bir RAW ve bir JPEG tek görüntü olarak ayıklanır ve işaretler ikisine de uygulanır. JPEG daha hızlı açılır.
settings-cull-show-jpeg = JPEG
settings-cull-show-raw = RAW

# machine-drafted, needs native review
settings-cull-bursts = Seri çekimleri yığ
settings-cull-bursts-help = Bir fotoğraf makinesiyle art arda hızla çekilen kareler tek yığın olarak gösterilir. S bir yığını açar veya kapatır, B ekrandaki görüntüyü tutar ve yığının geri kalanını reddeder.
settings-cull-burst-gap = Seri aralığı
settings-cull-burst-gap-help = Bir serinin iki karesi arasındaki en uzun duraklama.
settings-cull-watch = Yeni dosyaları göster
settings-cull-watch-help = Ayıklama sırasında klasörde beliren dosyalar (bağlı fotoğraf makinesi, içe aktarma) tamamen yazıldıklarında sona eklenir.
settings-cull-follow-newest = En yeni dosyayı izle
settings-cull-follow-newest-help = Her yeni dosya geldiğinde ona atla. T ayıklama sırasında bunu açar/kapatır.
settings-cull-slot-1 = Klasör 1
settings-cull-slot-1-help = Alt+1 görüntüyü (RAW veya JPEG eşi ve yan dosyalarıyla) bu klasöre kopyalar, Alt+Shift+1 oraya taşır.
settings-cull-slot-1-unset = Ayarlanmadı
settings-cull-slot-2 = Klasör 2
settings-cull-slot-2-help = Alt+2 görüntüyü (RAW veya JPEG eşi ve yan dosyalarıyla) bu klasöre kopyalar, Alt+Shift+2 oraya taşır.
settings-cull-slot-2-unset = Ayarlanmadı
settings-cull-slot-3 = Klasör 3
settings-cull-slot-3-help = Alt+3 görüntüyü (RAW veya JPEG eşi ve yan dosyalarıyla) bu klasöre kopyalar, Alt+Shift+3 oraya taşır.
settings-cull-slot-3-unset = Ayarlanmadı
settings-cull-collisions = Bir ad kullanılıyorsa
settings-cull-collisions-help = Klasörlerdeki dosyalar asla değiştirilmez.
settings-cull-collisions-suffix = Numara ekle
settings-cull-collisions-skip = Görüntüyü atla
# machine-drafted, needs native review
# Hiçbir klasör veya galeri görüntüsü açık değilken ayıklama modu seçildi.
cull-nothing = Ayıklanacak bir şey yok — önce bir fotoğraf klasörü açın
cull-slot-unset = Klasör { $slot } ayarlanmadı — Ayarlar → Ayıklama modu'ndan birini seçin
cull-slot-missing = Klasör { $slot } kullanılamıyor: { $reason }
cull-copied = { $folder } klasörüne kopyalandı
cull-moved = { $folder } klasörüne taşındı
cull-slot-renamed = { $folder } içine { $name } olarak kaydedildi
cull-slot-skipped = { $folder } içinde zaten { $name } var — atlandı
cull-slot-same = Görüntü zaten { $folder } içinde
cull-slot-failed = { $op ->
    [move] { $file } taşınamadı
   *[copy] { $file } kopyalanamadı
}: { $reason }
cull-copy-kept = Sonradan değiştirilen { $count } kopya tutuldu
cull-undo-failed = { $file } için geri alınamadı: { $reason }
cull-burst-open = Seri { $index }/{ $count }
cull-burst-closed = { $count } karelik seri
cull-burst-toggle = Seriyi aç veya kapat
cull-burst-best = 1 tutuldu, { $count } reddedildi
cull-burst-none = Bir serinin parçası değil
cull-follow-on = Yeni dosyalar izleniyor
cull-follow-off = Yeni dosyalar izlenmiyor
cull-new-files = { $count } yeni dosya
cull-key-copy = Klasör 1–3'e kopyala
cull-key-move = Klasör 1–3'e taşı
cull-key-burst = Bir seriyi aç veya kapat
cull-key-best = Bunu tut, serinin geri kalanını reddet
cull-key-follow = Yeni dosyaları izle
cull-key-subframes = Alt kare tablosunu göster veya gizle
cull-key-sort = Sonraki sütuna göre sırala
cull-key-sort-reverse = Sırayı ters çevir
cull-key-metric = Grafikte sonraki ölçüt
cull-key-sigma-down = Otomatik ret: k'yi düşür
cull-key-sigma-up = Otomatik ret: k'yi artır
cull-key-auto-reject = Önerilen kareleri reddet
cull-key-move-rejects = Reddedilen kareleri bir alt klasöre taşı
cull-key-same-stretch = Tüm karelere aynı germe

## Strip tooltips. machine-drafted, needs native review
cull-tip-burst-open = Seriyi aç
cull-tip-burst-close = Seriyi kapat
cull-tip-exit = Ayıklama modundan çık
cull-tip-label = Etiket: { $label }

## Astronomy subframes. machine-drafted, needs native review
cull-astro-panel = Alt kareler
cull-astro-title = { $count } alt kare
cull-astro-pending = { $count } ölçülüyor
cull-astro-col-name = Kare
cull-astro-col-filter = Filtre
cull-astro-col-exposure = Poz
cull-astro-col-hfd = HFD
cull-astro-col-fwhm = FWHM
cull-astro-col-ecc = Eksant.
cull-astro-col-stars = Yıldızlar
cull-astro-col-bg = Arka plan
cull-astro-col-snr = SNR
cull-astro-col-noise = Gürültü
cull-astro-same-stretch = Aynı germe
cull-astro-own-stretch = Ayrı germe
cull-astro-suggest-high = { $metric } { $value } üzerinde (medyan + { $k }σ): { $count } kare
cull-astro-suggest-low = { $metric } { $value } altında (medyan − { $k }σ): { $count } kare
cull-astro-suggest-none = { $k }σ ötesinde kare yok
cull-astro-suggest-few = Otomatik ret için 3 ölçülmüş kare gerekir
cull-astro-suggested = ret için önerildi
cull-astro-k = k { $k }
cull-astro-reject = { $count } kareyi reddet
cull-astro-move = Reddedilenleri taşı…
cull-astro-chart = { $count } karede { $metric }, medyan { $median }.
cull-astro-sort-by = { $column } sütununa göre sırala
cull-astro-ascending = artan
cull-astro-descending = azalan
cull-astro-sorted = { $column } sütununa göre sıralandı
cull-astro-reversed = Sıra ters çevrildi
cull-astro-metric = Grafik: { $metric }
cull-astro-k-set = Otomatik ret: { $k }σ
cull-astro-none = Burada alt kare yok
cull-astro-same-on = Tüm karelere aynı germe
cull-astro-same-off = Her kare ayrı geriliyor
cull-astro-line-hfd = HFD { $value } px
cull-astro-line-fwhm = FWHM { $value } px
cull-astro-line-fwhm-arcsec = FWHM { $value } px ({ $arcsec }″)
cull-astro-line-ecc = eksantriklik { $value }
cull-astro-line-stars = { $count } yıldız
cull-astro-measuring = yıldızlar ölçülüyor…
cull-astro-reject-confirm = { $count ->
    [one] Önerilen kare reddedilsin mi?
   *[other] Önerilen { $count } kare reddedilsin mi?
}
cull-astro-reject-detail = { $rule }. Geri al işaretleri geri getirir.
cull-astro-reject-ok = Reddet
cull-astro-nothing-to-reject = Reddedilecek kare yok
cull-astro-rejected = { $count } kare reddedildi
cull-move-rejects-confirm = { $count ->
    [one] Reddedilen kare “{ $folder }” klasörüne taşınsın mı?
   *[other] Reddedilen { $count } kare “{ $folder }” klasörüne taşınsın mı?
}
cull-move-rejects-detail = Dosyalar silinmez, taşınır. Geri al onları geri getirir.
cull-move-rejects-ok = Taşı
cull-move-rejects-none = Taşınacak reddedilmiş kare yok
cull-moved-rejects = { $count } kare “{ $folder }” klasörüne taşındı
