# Specular — Ölçüm aracı (her görüntüde uzunluklar, açılar, bölgeler ve istatistikleri). Makine çevirisi, ana dili konuşan biri tarafından gözden geçirilmeli.

palette-measure = Ölç
palette-measure-copy-csv = Ölçümleri CSV olarak kopyala
palette-measure-export = Ölçümleri dışa aktar…
toast-copied-measurements = Ölçümler CSV olarak kopyalandı

measure-tool-title = Ölç
measure-tool-list = Ölçümler
measure-tool-count = { $count ->
    [0] Ölçüm yok
    [one] Bir ölçüm
   *[other] { $count } ölçüm
}
measure-tool-empty = Bir araç seçin ve görüntü üzerine çizin.

measure-tool-point = Nokta
measure-tool-line = Çizgi
measure-tool-polyline = Çoklu çizgi
measure-tool-angle = Açı
measure-tool-rect = Dikdörtgen
measure-tool-ellipse = Elips
measure-tool-polygon = Çokgen
measure-tool-freehand = Serbest el
measure-tool-calibrate = Kalibre et
# machine-drafted, needs native review
measure-tool-spot = Nokta

measure-tool-undo = Geri al
measure-tool-delete = Sil
measure-tool-copy = CSV kopyala
measure-tool-export = Dışa aktar…
measure-tool-import = İçe aktar…
measure-tool-done = Bitti
measure-tool-unit = Birim: { $unit }
measure-tool-apply = Uygula

measure-tool-hint-line = Sürükleyin veya iki noktaya tıklayın. Oklar artı imlecini taşır, Boşluk bir nokta koyar.
measure-tool-hint-angle = Üç noktaya tıklayın; ikincisi tepe noktasıdır.
measure-tool-hint-rect = Sürükleyin veya iki köşeye tıklayın.
measure-tool-hint-ellipse = Sürükleyin veya çerçevesinin iki köşesine tıklayın.
measure-tool-hint-polygon = Köşelere tıklayın; Enter veya ilk köşeye tıklamak şekli kapatır.
measure-tool-hint-calibrate = Uzunluğu bilinen bir şeyin üzerine çizgi çizin.
# machine-drafted, needs native review
measure-tool-hint-spot = Sıcaklığını okumak için bir noktaya tıklayın.
measure-tool-known-length = Bilinen uzunluk: { $value } { $unit } (sayıyı yazın, U birimi değiştirir, Enter uygular)
measure-tool-calibrated = Kalibre edildi: bir piksel { $size } { $unit }
measure-tool-bad-length = Sıfırdan büyük bir uzunluk yazın.

measure-tool-item = { $n }. { $shape }
measure-tool-area = Alan { $value }
measure-tool-mean = Ortalama { $mean } ± { $sd }
measure-tool-means = Ortalama { $values }
measure-tool-range = Aralık { $min } – { $max }, { $count } px üzerinde
measure-tool-pixels = { $count } px üzerinde
measure-tool-stats-pending = Pikseller okunuyor…
measure-tool-stats-none = Burada piksel değeri yok
measure-tool-approximate = Küçültülmüş bir kopyadan (yaklaşık)

measure-tool-deleted = Ölçüm silindi
measure-tool-undone = Geri alındı
measure-tool-nothing = Bu görüntüde ölçüm yok
measure-tool-exported = Ölçümler { $name } dosyasına kaydedildi
measure-tool-export-failed = Ölçümler kaydedilemedi: { $reason }
measure-tool-imported = { $count ->
    [one] Bir ölçüm içe aktarıldı.
   *[other] { $count } ölçüm içe aktarıldı.
}
measure-tool-skipped = { $count ->
    [one] Bir şekil okunamadı.
   *[other] { $count } şekil okunamadı.
}
measure-tool-import-failed = Ölçümler okunamadı: { $reason }
measure-tool-too-large = Dosya çok büyük.
# machine-drafted, needs native review
measure-tool-sr-not-dicom = DICOM SR için DICOM görüntüsü gerekir. Bunun yerine CSV veya GeoJSON olarak kaydedin.

measure-source-uncalibrated = Piksel (kalibre edilmemiş)
measure-source-dicom-pixel-spacing = DICOM piksel aralığı
measure-source-dicom-calibrated = DICOM kalibre edilmiş aralık
measure-source-dicom-imager-pixel-spacing = DICOM dedektör aralığı (dedektörde)
measure-source-dicom-magnification = Büyütme için düzeltilmiş DICOM aralığı
measure-source-dicom-ultrasound-region = DICOM ultrason bölgesi
measure-source-tiff-resolution = TIFF çözünürlüğü
measure-source-exif-resolution = EXIF çözünürlüğü
measure-source-ome-physical-size = OME fiziksel boyutu
measure-source-fits-wcs = FITS dünya koordinatları
measure-source-vendor-tag = Üretici meta verileri
measure-source-drawing-units = Çizim birimleri
# machine-drafted, needs native review
measure-source-drawing-scale = Çizim ölçeğinden
# machine-drafted, needs native review
measure-source-drawing-scale-ratio = Çizimden (ölçek { $scale })
# machine-drafted, needs native review
measure-source-drawing-scale-viewport = Çizimden (ölçek { $scale }, görünüm penceresi { $viewport })
# machine-drafted, needs native review
measure-source-map-scale = Merkezdeki harita ölçeği (yaklaşık; Haritalar Dünya üzerinde ölçer)
measure-source-user = Kalibrasyonunuz
