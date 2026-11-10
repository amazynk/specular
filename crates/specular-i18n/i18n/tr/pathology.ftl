# machine-drafted, needs native review
# Specular — Patoloji: boya ayrıştırma, açıklamaları dışa ve içe aktarma, hacimler üzerinde etiket hacimleri. Türkçe.

## Palet
palette-stain-separation = Boyaları ayrıştır
palette-stain-1 = Boya 1’i göster veya gizle
palette-stain-2 = Boya 2’yi göster veya gizle
palette-stain-3 = Boya 3’ü göster veya gizle
palette-stain-mode = Boyalar ışık veya optik yoğunluk olarak
palette-stain-estimate = Boyaları bölgeden tahmin et
palette-stain-normalise = Boya renklerini normalleştir
palette-stain-reference = Bölgeyi boya referansı olarak kullan
palette-stain-reset = Yayımlanmış boya vektörleri
palette-export-annotations = Açıklamaları dışa aktar…
palette-import-annotations = Açıklamaları içe aktar…
palette-label-volume = Etiket hacmini bindir…

## Devre dışı bırakılma nedenleri
disabled-not-colour-image = Renkli bir görüntü gerekir
disabled-stains-off = Önce boya ayrıştırmayı açın
disabled-no-region = Önce bir bölge çizin
disabled-no-slide-annotations = Dışa aktarılacak açıklama, bölge veya nokta yok
disabled-no-label-target = Çok düzlemli görünümlerde bir hacim veya bir tıbbi görüntü açın

## Boya adları
stain-hematoxylin = Hematoksilen
stain-eosin = Eozin
stain-dab = DAB
stain-residual = Kalıntı
stain-set-he = H&E
stain-set-hdab = H-DAB
stain-set-hedab = H&E-DAB

## Boya bildirimleri ve göstergesi
stain-off = Boya ayrıştırma kapalı
# { $set }: H&E, H-DAB veya H&E-DAB.
stain-on = Boyalar: { $set }
stain-shown = { $stain } gösteriliyor
stain-hidden = { $stain } gizlendi
stain-mode-light = Boyalar ışık olarak
stain-mode-od = Boyalar optik yoğunluk olarak
# Göstergenin başlığı: "H&E · ışık", "H-DAB · optik yoğunluk".
stain-legend-light = { $set } · ışık
stain-legend-od = { $set } · optik yoğunluk
stain-legend-estimated = Bölgeden tahmin edildi
stain-legend-normalised = Normalleştirildi
stain-legend-published = Yayımlanmış vektörler
# { $pixels }: kullanılan boyalı pikseller.
stain-estimated = Boyalar { $pixels } pikselden tahmin edildi
stain-estimate-failed = Boyalar tahmin edilemiyor: { $reason }
stain-estimate-background = bölge çoğunlukla arka plan
stain-estimate-one-colour = bölgede tek bir renk var
stain-estimate-three = tahmin için H&E veya H-DAB gerekir
stain-estimate-no-pixels = bölgenin pikselleri henüz yüklenmedi
stain-reference-set = Bölge boya referansı olarak ayarlandı
stain-normalise-on = Boya renkleri normalleştirildi
stain-normalise-off = Boya renkleri tarandığı gibi
stain-reset-done = Yayımlanmış boya vektörleri

## Açıklamaları dışa aktarma iletişim kutusu
annot-export-title = Açıklamaları dışa aktar
annot-export-format = Biçim
annot-export-format-coco = COCO JSON
annot-export-format-geojson = GeoJSON (QuPath)
annot-export-format-masks = Etiket maskeleri
annot-export-output = Maske biçimi
annot-export-output-png = PNG karoları
annot-export-output-tiff = TIFF karoları
annot-export-output-ome = OME-TIFF piramidi
annot-export-region = Bölge
annot-export-region-slide = Tüm lam
annot-export-region-view = Görünen alan
annot-export-region-drawn = Son çizilen bölge
annot-export-level = Düzey
# { $level }: tam çözünürlük için 0; { $width } × { $height } piksel cinsinden boyutu.
annot-export-level-value = { $level } · { $width } × { $height }
annot-export-tile = Karo boyutu
annot-export-empty = Boş karolar
annot-export-empty-skip = Atla
annot-export-empty-keep = Yaz
annot-export-what = { $count ->
    [one] 1 şekil
   *[other] { $count } şekil
}, { $classes ->
    [one] 1 sınıf
   *[other] { $classes } sınıf
}
annot-export-lines = Çizgiler COCO ve maskelere dahil edilmez.
annot-export-export = Dışa aktar…
annot-export-writing = Yazılıyor…
# { $files }: yazılan dosyalar.
annot-export-done = { $files ->
    [one] 1 dosya dışa aktarıldı
   *[other] { $files } dosya dışa aktarıldı
}
annot-export-failed = Dışa aktarma başarısız: { $reason }
annot-import-done = { $count ->
    [one] 1 açıklama içe aktarıldı
   *[other] { $count } açıklama içe aktarıldı
}
annot-import-failed = İçe aktarılamıyor: { $reason }
annot-import-skipped = { $count ->
    [one] 1 şeklin kullanılabilir bir dış hattı yoktu
   *[other] { $count } şeklin kullanılabilir bir dış hattı yoktu
}
annot-file-coco = COCO JSON
annot-file-geojson = GeoJSON

## Etiket hacimleri
label-volume-file = Etiket hacmi
label-volume-table = Renk tablosu
# { $file }: etiket hacminin dosya adı; { $count }: etiketleri.
label-volume-loaded = { $count ->
    [one] { $file }: 1 etiket
   *[other] { $file }: { $count } etiket
}
label-volume-failed = { $file } bindirilemiyor: { $reason }
label-volume-outside = { $file } bu hacimle örtüşmüyor
label-volume-not-whole = değerleri tam sayı değil
label-volume-colour = etiket değil renk içeriyor
label-volume-empty = hiç etiketi yok
label-volume-too-many = çok fazla etiketi var
label-volume-too-large = yeniden örneklemek için çok büyük
label-volume-degenerate = yönelimi geçersiz
# Renk tablosunda adı olmayan bir etiket.
label-volume-unnamed = Etiket { $value }
