# Specular — Medical images (DICOM). machine-drafted, needs native review

cheatsheet-medical = Tıbbi görüntüler
palette-window-level = Pencere/düzey aracı (sol sürükleme)
palette-window-default = Pencere: dosya varsayılanı
palette-window-lung = Pencere: akciğer (W 1500, L −600)
palette-window-mediastinum = Pencere: mediasten (W 350, L 50)
palette-window-bone = Pencere: kemik (W 1800, L 400)
palette-window-brain = Pencere: beyin (W 80, L 40)
palette-window-stroke = Pencere: inme (W 40, L 40)
palette-window-soft-tissue = Pencere: yumuşak doku (W 400, L 50)
palette-window-liver = Pencere: karaciğer (W 150, L 30)
palette-window-subdural = Pencere: subdural (W 200, L 75)
palette-window-full-range = Pencere: tam aralık
palette-invert-grey = Griyi ters çevir
palette-next-slice = Sonraki kare
palette-prev-slice = Önceki kare
disabled-not-medical = Yalnızca gri tonlamalı tıbbi görüntüler için
disabled-one-frame = Görüntünün tek karesi var
settings-section-medical = Tıbbi görüntüler
settings-medical-keep-window = Pencere/düzeyi sonraki seri için koru
settings-medical-keep-window-help = Başka bir seriye geçerken onun varsayılanından başlamak yerine ayarladığınız pencereyi korur. Bir dosyanın kareleri ve bir serinin kesitleri onu her zaman paylaşır.
medical-window = C { $center }  W { $width }
medical-inverted = ters çevrildi
medical-frame = Kare { $frame }/{ $count }
medical-tool = Pencere/düzey aracı: sürükleyin
medical-tool-on = Pencere/düzey aracı açık: ayarlamak için sürükleyin
medical-tool-off = Pencere/düzey aracı kapalı
medical-preset-not-ct = BT ön ayarları Hounsfield birimi cinsinden değerler gerektirir
medical-file-window-none = Bu dosyada { $index }. pencere yok
medical-file-window-unnamed = Dosya penceresi { $index }/{ $count }
medical-file-window = Dosya penceresi { $index }/{ $count }: { $name }
measure-templates-need-medical = Geometri şablonları Tıp profiliyle gelir
medical-off = Bu görüntü için tıbbi araç yok: { $reason }
medical-readout = Değer ve pencere

## Measurements, cine and overlays (machine-drafted, needs native review)

palette-cine = Kareleri oynat (sine)
palette-measure-length = Uzunluk ölç
palette-measure-angle = Açı ölç
palette-measure-ellipse = Eliptik bölge ölç
palette-measure-rectangle = Dikdörtgen bölge ölç
palette-clear-measurements = Ölçümleri temizle
palette-measure-templates = Geometri şablonları…
palette-overlays = Kaplamaları göster veya gizle
medical-cine = { $rate } kare/sn hızında oynatılıyor
medical-overlays-shown = Kaplamalar gösteriliyor
medical-overlays-hidden = Kaplamalar gizlendi
measure-hint-length = Uzunluk: sürükleyin veya iki uca tıklayın
measure-hint-angle = Açı: üç noktaya tıklayın, tepe noktası ikinci olsun
measure-hint-ellipse = Elips: bölgenin üzerinde sürükleyin
measure-hint-rectangle = Dikdörtgen: bölgenin üzerinde sürükleyin
measure-off = Ölçüm aracı kapalı
measure-cleared = Ölçümler temizlendi
measure-length-mm = { $value } mm
# machine-drafted, needs native review
measure-length-angstrom = { $value } Å
measure-length-px = { $value } px
measure-area-mm = Alan { $value } mm²
measure-area-px = Alan { $value } px²
measure-angle = { $value }°
measure-mean = Ortalama { $mean } ± { $sd } { $unit }
measure-range = En az { $min }  En çok { $max }  ({ $count } px)
measure-caveat-detector = Dedektörde: büyütme için düzeltilmedi
measure-caveat-magnification = Tahmini büyütme düzeltmesi
measure-caveat-uncalibrated = Piksel aralığı yok: piksel cinsinden
measure-caveat-outside-region = Kalibre edilmiş ultrason bölgesinin dışında: piksel cinsinden

## DICOM orientation labels, presentation states and wheel stepping (machine-drafted, needs native review)
settings-medical-orientation-labels = Yönelim etiketlerini göster
settings-medical-orientation-labels-help = Görüntü kenarlarında hastanın yönlerini gösteren harfler (R/L sağ/sol, A/P ön/arka, S/I baş/ayak); dosyadan alınır, döndürme ve çevirmeyi izler. L tuşu bunları oturum boyunca açıp kapatır.
settings-medical-wheel-steps-frames = Fare tekerleği kareler arasında geçer
settings-medical-wheel-steps-frames-help = Birden çok kare içeren bir dosya veya seride tekerlek, yakınlaştırmak ya da görüntü değiştirmek yerine kareler arasında geçer. Ctrl+tekerlek yine yakınlaştırır.
# machine-drafted, needs native review
settings-medical-display-function = Gri görüntüleme
settings-medical-display-function-help = Tıbbi görüntülerin gri tonlarının nasıl gösterileceği. DICOM GSDF (Gri Tonlama Standart Görüntüleme Fonksiyonu), bunları ekranın siyah ve beyaz parlaklığı arasında eşit görünürlük adımlarına dağıtır (DICOM PS3.14); kalibre edilmiş bir tanısal ekranda olduğu gibi. Ekranın renk profili yine uygulanır.
settings-medical-display-function-srgb = Standart (sRGB)
settings-medical-display-function-gsdf = DICOM GSDF
settings-medical-display-black = Ekranın siyah parlaklığı
settings-medical-display-black-help = Ekranın siyahının ortam ışığı dahil parlaklığı; ölçümden veya ekranın veri sayfasından. DICOM GSDF tarafından kullanılır.
settings-medical-display-white = Ekranın beyaz parlaklığı
settings-medical-display-white-help = Ekranın beyazının parlaklığı; ölçümden veya ekranın veri sayfasından. DICOM GSDF tarafından kullanılır.
palette-orientation-labels = Yönelim etiketlerini göster veya gizle
palette-presentation-state = Sonraki sunum durumu
disabled-no-presentation-state = Bu görüntüye başvuran bir sunum durumu yok
medical-labels-shown = Yönelim etiketleri gösteriliyor
medical-labels-hidden = Yönelim etiketleri gizlendi
medical-presentation = Sunum durumu: { $label } ({ $index }/{ $count })
medical-presentation-none = Sunum durumu kapalı
medical-presentation-readout = Sunum: { $label }
medical-preset-presentation = Gri düzeylerini sunum durumunun arama tablosu belirliyor

## Orientation labels for screen readers (machine-drafted, needs native review)
medical-orientation-a11y = Yön: sol { $left }, sağ { $right }, üst { $top }, alt { $bottom }
