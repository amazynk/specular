# machine-drafted, needs native review
## Astronomy interop: DS9 regions, plate solving, SAMP, HiPS sky surveys.

palette-science-load-regions = DS9 bölgelerini bindir…
palette-science-load-regions-help = Bir DS9 bölge dosyasındaki (.reg) şekilleri görüntü üzerine çizer.
palette-science-export-regions = Bölgeleri DS9 olarak kaydet…
palette-science-export-regions-help = Ölçümleri ve bindirilen bölgeleri bir DS9 bölge dosyası olarak kaydeder.
palette-science-edit-regions = Bölgeleri ölçüm olarak düzenle
palette-science-edit-regions-help = Bindirilen bölgeleri taşıyıp değiştirebileceğiniz ölçümlere dönüştürür.
palette-science-clear-regions = Bindirilen bölgeleri kaldır
palette-science-clear-regions-help = DS9 bölgelerini görüntüden kaldırır.
palette-science-plate-solve = Astrometrik çözüm
palette-science-plate-solve-help = Görüntünün gök koordinatlarını ASTAP ya da astrometry.net ile bulur. Durdurmak için yeniden çalıştırın.
palette-science-save-plate-solution = Astrometrik çözümü kaydet
palette-science-save-plate-solution-help = Bulunan gök koordinatlarını görüntünün yanına .wcs dosyası olarak kaydeder.
palette-science-sky-survey = Arka planda gökyüzü taraması
palette-science-sky-survey-help = Görüntünün çevresinde sonraki gökyüzü taramasını gösterir ya da hiçbirini göstermez.
palette-science-samp-send-image = Görüntüyü SAMP uygulamalarına gönder
palette-science-samp-send-image-help = Bu görüntüyü DS9, Aladin ve SAMP merkezindeki diğer uygulamalarda açar.
palette-science-samp-send-position = Konumu SAMP uygulamalarına gönder
palette-science-samp-send-position-help = Diğer uygulamaları işaretçinin altındaki gök konumuna yöneltir.

disabled-samp-off = Önce bir SAMP merkezine bağlanın (Ayarlar › Tümleştirme).

settings-integration-samp = Bir SAMP merkezine bağlan
settings-integration-samp-help = DS9, TOPCAT, Aladin ve diğer astronomi uygulamalarıyla görüntü, tablo ve gök konumu alışverişi yapılır. Yalnızca bu bilgisayardaki bir merkez kullanılır.
settings-integration-astap-path = ASTAP programı
settings-integration-astap-path-help = Kendiliğinden bulunamadığında ASTAP astrometrik çözücüsü.
settings-integration-solve-field-path = astrometry.net programı
settings-integration-solve-field-path-help = Kendiliğinden bulunamadığında astrometry.net’in solve-field programı.

astro-regions-filter = DS9 bölgeleri
astro-regions-loaded = { $name }: { $count } bölge.
astro-regions-refused = { $count } satır okunmadı. İlki: satır { $line }, { $reason }.
astro-regions-approximate = { $count } panda iç ve dış kenarlarıyla çizildi.
astro-regions-none-here = { $name }: bu görüntüye hiçbir bölge düşmüyor.
astro-regions-failed = { $name } okunamadı: { $reason }
astro-regions-cleared = Bölgeler kaldırıldı.
astro-regions-nothing = Kaydedilecek bölge ya da ölçüm yok.
astro-regions-saved = { $name } kaydedildi.
astro-regions-skipped = Gök koordinatlarının dışındaki { $count } şekil atlandı.
astro-regions-save-failed = Bölgeler kaydedilemedi: { $reason }
astro-regions-edited = { $count } bölge artık ölçüm.
astro-regions-no-wcs = Gök bölgeleri için gök koordinatları olan bir görüntü gerekir.

astro-solve-no-solver = Astrometrik çözücü bulunamadı. ASTAP ya da astrometry.net kurun veya Ayarlar › Tümleştirme bölümünden birini seçin.
astro-solve-running = { $solver } ile çözülüyor… { $seconds } sn
astro-solve-stop = Durdur
astro-solve-stopped = Astrometrik çözüm durduruldu.
astro-solve-done = Çözüldü: { $ra }, { $dec }, piksel başına { $scale }.
astro-solve-failed = Astrometrik çözüm başarısız oldu: { $reason }
astro-solve-not-local = Yalnızca bu bilgisayardaki bir klasörde bulunan görüntüler çözülebilir.
astro-solve-none = Bu görüntü için henüz astrometrik çözüm yok.
astro-solve-saved = { $name } kaydedildi. Görüntünün kendisi değişmedi.
astro-solve-sidecar = Gök koordinatları { $name } dosyasından.

astro-samp-connected = SAMP merkezine bağlanıldı.
astro-samp-no-hub = SAMP: { $reason }. Bir merkez başladığında yeniden denenecek.
astro-samp-lost = SAMP merkezi durdu. Bir merkez başladığında yeniden denenecek.
astro-samp-sent = { $count } uygulamaya gönderildi.
astro-samp-nobody = Merkezdeki hiçbir uygulama bunu dinlemiyor.
astro-samp-failed = SAMP: { $reason }
astro-samp-refused = SAMP: { $url } bu bilgisayarda değil, bu yüzden açılmıyor.
astro-samp-pointed = { $ra }, { $dec } konumuna yöneltildi.
astro-samp-off-image = { $ra }, { $dec } bu görüntüde değil.
astro-samp-no-table = SAMP: o tablo burada bindirilmiş değil.
astro-samp-rows = { $count } katalog kaynağı vurgulandı.

astro-survey-off = Gökyüzü taraması kapalı.
astro-survey-on = Gökyüzü taraması: { $name }. { $credit }
astro-survey-failed = { $name } gösterilemiyor: { $reason }
astro-survey-credit = { $name }: { $credit }

net-what-sky-survey = { $host } kaynağından gökyüzü taraması karoları
