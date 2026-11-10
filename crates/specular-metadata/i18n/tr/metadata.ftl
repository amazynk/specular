# machine-drafted, needs native review
# Specular meta veri paneli — Türkçe.
#
# Kurallar
# - Sayılar yerel ayara göre (ICU4X) önceden biçimlendirilmiş dizeler olarak gelir, ör.
#   $num = "2.456.789". Çoğul biçim gerektiğinde ham sayı da $n (sayı) olarak
#   iletilir ve yalnızca seçici olarak kullanılır.
# - Tarih ve saatler ICU4X tarafından önceden biçimlendirilmiş gelir ($date, $time).
# - Araya eklenen her değer Fluent tarafından Unicode yalıtıcılarıyla sarılır.
# - İleti kimlikleri kalıcıdır; bir kimliği asla farklı bir anlam için yeniden kullanmayın.

## Özet grup başlıkları

group-camera = Kamera
group-exposure = Pozlama
group-lens = Objektif
group-date = Tarih ve saat
group-location = Konum
# machine-drafted, needs native review
# A DJI drone photo: the gimbal's and the aircraft's attitude, the height above take-off.
group-drone = Drone
label-gimbal = Gimbal
label-flight = Hava aracı
label-relative-altitude = Kalkış noktasına göre
drone-attitude = sapma { $yaw }, yunuslama { $pitch }, yatış { $roll }
# A DJI thermal camera's radiometric JPEG: its photo opens, its temperatures can't.
label-thermal-data = Sıcaklıklar
drone-thermal-refused = Gösterilmiyor: DJI’ın SDK lisansı buna izin vermiyor
drone-thermal-refused-detail = DJI sıcaklıklarını okumak DJI Thermal SDK’sını gerektirir; lisansı onun yalnızca SDK’yı birlikte dağıtan programlarca kullanılmasına izin verir. Görüntü ve meta verileri gösteriliyor.
group-image = Görüntü
group-file = Dosya
group-rights = Haklar ve açıklama
group-credentials = İçerik kimlik bilgileri
# machine-drafted, needs native review
group-medical = Tıbbi görüntü
# machine-drafted, needs native review
group-inspection = Parça ve muayene
group-nde-image = Muayene görüntüsü (DICONDE)
# machine-drafted, needs native review
# Industrial CT volumes (Zeiss Xradia, Amira, Bruker SkyScan, Nikon X-Tek, VGI, raw volumes).
group-industrial-image = Muayene görüntüsü
group-science = Bilimsel görüntü
group-em = Elektron mikroskobu
group-spm = Taramalı uç mikroskobu
group-slide = Lam
group-microscopy = Mikroskopi
# X-ray diffraction detector frames (CBF, EDF, SMV, Bruker, MarCCD, R-AXIS).
group-diffraction = Kırınım
group-surface = Yüzey topografisi
group-ebsd = Geri saçılmış elektron kırınımı
group-seismic = Sismik veri
# machine-drafted, needs native review
group-asdf = ASDF
# machine-drafted, needs native review
# CERN ROOT files (particle physics histograms and graphs).
group-root = ROOT
# machine-drafted, needs native review
# CASA images (radio interferometry: the .image, .residual, .model directories tclean writes).
group-casa = CASA görüntüsü
# machine-drafted, needs native review
# GIFTI files: brain surfaces and the values on their vertices (Human Connectome Project
# and FreeSurfer exports).
group-gifti = GIFTI
# CIFTI-2 files: maps over cortical surfaces and subcortical voxels ("grayordinates").
group-cifti = CIFTI
# White-matter fibre tracts (TrackVis, MRtrix).
group-tractography = Traktografi
# machine-drafted, needs native review
group-annotations = Açıklamalar
# Imaging mass spectrometry (imzML): a mass spectrum per pixel.
group-mass-spec = Kütle spektrometrisi görüntüleme
# machine-drafted, needs native review
group-thermal = Termal görüntü
# machine-drafted, needs native review
# Flow cytometry list-mode data (FCS): events of a cell sorter or analyser.
group-cytometry = Akış sitometrisi
# machine-drafted, needs native review
group-spectroscopy = Spektroskopi
# machine-drafted, needs native review
# Unstructured meshes of ocean, coastal and climate models (UGRID, ICON, MPAS in NetCDF).
group-ugrid = Yapılandırılmamış ağ
group-drawing = Çizim
# machine-drafted, needs native review
group-volume = Hacim

## Özet öğe etiketleri

label-camera = Kamera
label-software = Yazılım
label-application = Uygulama
label-shutter = Enstantane
label-aperture = Diyafram
label-iso = ISO
label-focal-length = Odak uzaklığı
label-exposure-bias = Pozlama telafisi
label-program = Mod
label-metering = Ölçüm
label-white-balance = Beyaz dengesi
label-flash = Flaş
label-lens = Objektif
label-lens-make = Objektif üreticisi
label-lens-range = Objektif aralığı
# machine-drafted, needs native review
label-focus-mode = Odak modu
label-stabilisation = Görüntü sabitleme
label-taken = Çekim
label-digitised = Sayısallaştırma
label-edited = Son düzenleme
label-place = Yer
label-coordinates = Koordinatlar
label-altitude = Yükseklik
label-direction = Yön
label-accuracy = Doğruluk
label-gps-time = GPS saati
label-dimensions = Boyutlar
label-encoding = Kodlama
# machine-drafted, needs native review
label-motion = Hareket
motion-kind-motion-photo = Hareketli fotoğraf
motion-kind-micro-video = Hareketli fotoğraf (MicroVideo)
motion-kind-samsung = Samsung hareketli fotoğraf
motion-kind-live-photo = Live Photo
motion-duration = { $seconds } sn
label-colour-profile = Renk profili
label-orientation = Yönlendirme
label-file-name = Ad
label-folder = Klasör
label-file-size = Boyut
label-format = Biçim
label-modified = Değiştirilme
label-title = Başlık
label-description = Açıklama
label-creator = Oluşturan
label-copyright = Telif hakkı
label-keywords = Anahtar sözcükler
label-rating = Puan
label-colour-label = Etiket
label-credentials = İçerik kimlik bilgileri

## "Tüm alanlar" grup başlıkları (etiket adları standartlardaki gibi kalır)

fields-ifd0 = IFD0 (ana görüntü)
fields-exif = Exif
fields-gps = GPS
fields-interop = Birlikte çalışabilirlik
fields-ifd1 = IFD1 (küçük resim)
fields-icc = ICC profili üst bilgisi
# $namespace: xmp-namespace iletisinden; $prefix: ör. "dc"
fields-xmp = XMP: { $namespace } ({ $prefix })
xmp-namespace = { $prefix ->
    [dc] Dublin Core
    [xmp] Temel
    [xmpRights] Haklar
    [xmpMM] Medya yönetimi
    [xmpDM] Dinamik medya
    [tiff] TIFF
    [exif] Exif
    [exifEX] Exif 2.3+
    [aux] Kamera aksesuarları
    [photoshop] Photoshop
    [crs] Camera Raw ayarları
    [lr] Lightroom
    [Iptc4xmpCore] IPTC Core
    [Iptc4xmpExt] IPTC Extension
    [plus] PLUS lisanslama
    [GPano] Fotoğraf küresi
    [GCamera] Google kamera
    [hdrgm] HDR kazanç haritası
    [mwg-rs] Bölgeler
    [pdf] PDF
    [stEvt] Kaynak olayları
    [stRef] Kaynak başvuruları
   *[other] Diğer
}
# $number: "0xA500"; $context: "Exif", "GPS", …
unknown-tag = Bilinmeyen etiket { $number } ({ $context })
blob-bytes = { $num ->
   *[other] ({ $num } bayt)
}
maker-note-bytes = ({ $num } bayt, üretici notu, kodu çözülmedi)
more-values = { $shown }, … ({ $total } değer)

## Genel bağlayıcılar

# Kodda oluşturulan bir listenin parçalarını ayırır (flaş açıklaması, yer alanları).
list-separator = {", "}
# Tek satırlık özette enstantane · diyafram · ISO · odak uzaklığını ayırır.
one-line-separator = {" · "}

## Pozlama

exposure-time-fraction = 1/{ $denominator } sn
exposure-time-seconds = { $seconds } sn
aperture-value = f/{ $f }
iso-value = ISO { $iso }
focal-length-value = { $mm } mm
focal-length-with-equiv = { $mm } mm ({ $equiv } mm eşd.)
focal-length-equiv-only = { $equiv } mm eşd.
# $ev büyüklüktür; işaret seçiciden gelir.
exposure-bias-value = { $sign ->
    [plus] +{ $ev } EV
    [minus] −{ $ev } EV
   *[zero] 0 EV
}
exposure-program = { $value ->
    [1] Manuel
    [2] Program (otomatik)
    [3] Diyafram önceliği
    [4] Enstantane önceliği
    [5] Yaratıcı (alan derinliği)
    [6] Hareket (hızlı enstantane)
    [7] Portre
    [8] Manzara
   *[other] Bilinmiyor
}
metering-mode = { $value ->
    [1] Ortalama
    [2] Merkez ağırlıklı ortalama
    [3] Nokta
    [4] Çoklu nokta
    [5] Matris (değerlendirmeli)
    [6] Kısmi
   *[other] Diğer
}
# machine-drafted, needs native review
# Canon maker note CameraSettings FocusMode; 256–258 are the live-view variants of 0–2.
canon-focus-mode = { $value ->
    [0] One-Shot AF
    [1] AI Servo AF
    [2] AI Focus AF
    [3] Manuel odak
    [4] Tek
    [5] Sürekli
    [6] Manuel odak
    [16] Pan focus
    [256] One-Shot AF (Canlı Görünüm)
    [257] AI Servo AF (Canlı Görünüm)
    [258] AI Focus AF (Canlı Görünüm)
    [512] Movie Snap odak
    [519] Movie Servo AF
   *[other] Diğer
}
# Canon maker note CameraSettings ImageStabilization; 256–260 repeat 0–4.
canon-stabilisation = { $value ->
    [0] Kapalı
    [1] Açık
    [2] Yalnızca çekimde
    [3] Kaydırma
    [4] Dinamik
    [256] Kapalı
    [257] Açık
    [258] Yalnızca çekimde
    [259] Kaydırma
    [260] Dinamik
   *[other] Diğer
}
# machine-drafted, needs native review
panasonic-focus-mode = { $value ->
    [1] Otomatik
    [2] Manuel odak
    [4] Otomatik (odak düğmesi)
    [5] Otomatik (sürekli)
    [6] AF-S
    [7] AF-C
    [8] AF-F
   *[other] Diğer
}
# machine-drafted, needs native review
panasonic-stabilisation = { $value ->
    [2] Açık (optik)
    [3] Kapalı
    [4] Açık (mod 2)
    [5] Açık (optik, kaydırma)
    [6] Açık (yalnızca gövde)
    [7] Açık (yalnızca gövde, kaydırma)
    [9] Dual IS
    [10] Dual IS (kaydırma)
    [11] Dual IS 2
    [12] Dual IS 2 (kaydırma)
   *[other] Diğer
}
label-comment = Yorum
fields-tags = { $kind } etiketleri
group-picture = Görüntü ayarları
fields-fujifilm = Fujifilm üretici verileri
label-film-simulation = Film simülasyonu
label-dynamic-range = Dinamik aralık
label-highlight-tone = Parlak tonlar
label-shadow-tone = Gölgeler
label-colour-setting = Renk
label-sharpness = Keskinlik
label-noise-reduction = Gürültü azaltma
label-clarity = Netlik
label-grain-effect = Gren efekti
label-colour-chrome = Renk krom
label-colour-chrome-blue = Renk krom mavi
label-shutter-type = Deklanşör türü
label-drive-mode = Çekim modu
fuji-step = { $sign ->
    [plus] +{ $n }
    [minus] −{ $n }
   *[zero] 0
}
fuji-film-mode = { $value ->
    [0] Provia (standart)
    [256] Stüdyo portresi
    [272] Stüdyo portresi (daha doygun)
    [288] Astia (yumuşak)
    [304] Stüdyo portresi (daha keskin)
    [512] Velvia (canlı)
    [768] Stüdyo portresi Ex
    [1024] Velvia
    [1280] Pro Neg. Std
    [1281] Pro Neg. Hi
    [1536] Classic Chrome
    [1792] Eterna
    [2048] Classic Neg.
    [2304] Eterna Bleach Bypass
    [2560] Nostalgic Neg.
    [2816] Reala Ace
   *[other] Diğer
}
fuji-monochrome = { $value ->
    [768] Tek renkli
    [769] Tek renkli, kırmızı filtre
    [770] Tek renkli, sarı filtre
    [771] Tek renkli, yeşil filtre
    [784] Sepya
    [1280] Acros
    [1281] Acros, kırmızı filtre
    [1282] Acros, sarı filtre
    [1283] Acros, yeşil filtre
   *[other] Tek renkli
}
fuji-white-balance = { $value ->
    [0] Otomatik
    [1] Otomatik (beyaz önceliği)
    [2] Otomatik (ortam önceliği)
    [256] Gün ışığı
    [512] Bulutlu
    [768] Gün ışığı floresan
    [769] Gün beyazı floresan
    [770] Beyaz floresan
    [771] Sıcak beyaz floresan
    [772] Oturma odası sıcak beyaz floresan
    [1024] Akkor
    [1280] Flaş
    [1536] Su altı
    [3840] Özel 1
    [3841] Özel 2
    [3842] Özel 3
    [3843] Özel 4
    [3844] Özel 5
    [4080] Renk sıcaklığı
   *[other] Diğer
}
fuji-white-balance-kelvin = { $kelvin } K
fuji-dynamic-range = DR{ $percent }
fuji-dynamic-range-auto = Otomatik (DR{ $percent })
fuji-dynamic-range-setting = { $value ->
    [0] Otomatik
    [256] Standart (DR100)
    [512] Geniş 1 (DR230)
    [513] Geniş 2 (DR400)
   *[other] Diğer
}
fuji-effect = { $value ->
    [0] Kapalı
    [32] Zayıf
    [64] Güçlü
   *[other] Diğer
}
fuji-grain-size = { $value ->
    [16] küçük
    [32] büyük
   *[other] diğer
}
fuji-grain = { $strength }, { $size }
fuji-shutter-type = { $value ->
    [0] Mekanik
    [1] Elektronik
    [2] Elektronik (uzun pozlamalar)
    [3] Elektronik ön perde
   *[other] Diğer
}
fuji-drive-mode = { $value ->
    [0] Tek kare
    [1] Seri, düşük hız
    [2] Seri, yüksek hız
   *[other] Diğer
}
fuji-focus-mode = { $value ->
    [0] Manuel
    [1] AF-S
    [2] AF-C
    [100] Otomatik
    [101] Manuel
   *[other] Diğer
}
fuji-stabilisation = { $kind } ({ $mode })
fuji-stabilisation-kind = { $value ->
    [0] Yok
    [1] Optik
    [2] Sensör kaydırma
    [3] Objektif
    [258] Sensör kaydırma ve dijital
    [512] Dijital
   *[other] Diğer
}
fuji-stabilisation-mode = { $value ->
    [0] Kapalı
    [1] açık, sürekli
    [2] açık, çekim sırasında
   *[other] Diğer
}
light-source = { $value ->
    [1] Gün ışığı
    [2] Floresan
    [3] Tungsten
    [4] Flaş
    [9] Açık hava
    [10] Bulutlu
    [11] Gölge
    [12] Gün ışığı floresan
    [13] Gün beyazı floresan
    [14] Soğuk beyaz floresan
    [15] Beyaz floresan
    [16] Sıcak beyaz floresan
    [17] Standart ışık A
    [18] Standart ışık B
    [19] Standart ışık C
    [20] D55
    [21] D65
    [22] D75
    [23] D50
    [24] Stüdyo tungsten
   *[other] Diğer
}
white-balance-auto = Otomatik
white-balance-manual = Manuel
white-balance-manual-with-light = Manuel ({ $light })
# Flaş açıklaması bu parçalardan oluşturulur ve list-separator ile birleştirilir.
flash-none = Flaş işlevi yok
flash-fired = Flaş patladı
flash-not-fired = Flaş patlamadı
flash-forced-on = zorla açık
flash-forced-off = zorla kapalı
flash-auto = otomatik mod
flash-return-not-detected = dönüş ışığı algılanmadı
flash-return-detected = dönüş ışığı algılandı
flash-red-eye = kırmızı göz azaltma

## Objektif

lens-focal-single = { $mm } mm
lens-focal-range = { $min }–{ $max } mm
lens-aperture-single = f/{ $f }
lens-aperture-range = f/{ $min }–{ $max }
lens-range = { $focal } { $aperture }

## Tarih ve saat

# $date: "14 Mayıs 2024 Salı"; $time: "18:32"
# $weekday: "Salı" (ICU); $date: "14 Mayıs 2024" (ICU)
date-with-weekday = { $date } { $weekday }
date-time = { $date }, { $time }
date-time-zone = { $datetime } ({ $zone })
zone-utc = UTC
# $offset: "+1", "+5:30", "−3"
zone-offset = UTC{ $offset }
gps-date-time = { $date }, { $time } UTC
detail-zone-from-gps = Saat dilimi GPS saatinden hesaplandı

## Konum

# $lat ve $lon işaretsizdir; yarım küre seçiciden gelir.
coordinates = { $lat }° { $latRef ->
    [south] G
   *[north] K
}, { $lon }° { $lonRef ->
    [west] B
   *[east] D
}
altitude = { $ref ->
    [above] Deniz seviyesinin { $m } m üstünde
    [below] Deniz seviyesinin { $m } m altında
   *[sea] Deniz seviyesinde
}
compass-point = { $point ->
    [n] kuzey
    [nne] kuzey-kuzeydoğu
    [ne] kuzeydoğu
    [ene] doğu-kuzeydoğu
    [e] doğu
    [ese] doğu-güneydoğu
    [se] güneydoğu
    [sse] güney-güneydoğu
    [s] güney
    [ssw] güney-güneybatı
    [sw] güneybatı
    [wsw] batı-güneybatı
    [w] batı
    [wnw] batı-kuzeybatı
    [nw] kuzeybatı
   *[nnw] kuzey-kuzeybatı
}
# $point: compass-point iletisinden; $degrees: "287"
facing = Yön: { $point } ({ $degrees }°)
detail-true-north = Coğrafi kuzeye göre
detail-magnetic-north = Manyetik kuzeye göre
accuracy = ±{ $m } m
distance-km = { $km } km
place = { $city }, { $country }
place-city-only = { $city }
# Eşleşen yerleşim yerinden 30 km'den uzak.
place-near = { $place } yakınında
place-detail = { $areas } · { $city } merkezine { $distance }
place-detail-distance = { $city } merkezine { $distance }

## Görüntü

dimensions = { $width } × { $height }
dimensions-megapixels = { $width } × { $height } ({ $mp } MP)
encoding-bit-depth = { $bits } bit
# $format: "8 bit YCbCr 4:2:0"
encoding-progressive = { $format }, aşamalı
encoding-baseline = { $format }, temel
encoding-hierarchical = { $format }, hiyerarşik
# machine-drafted, needs native review
encoding-alpha = { $format }, { $alpha ->
    [premultiplied] önceden çarpılmış alfa
    [transparency] saydamlık
   *[alpha] alfa
}
# [Lab], [Palette] and the variants after them: machine-drafted, needs native review
colour-model = { $model ->
    [YCbCr] YCbCr
    [RGB] RGB
    [Greyscale] Gri tonlamalı
    [CMYK] CMYK
    [YCCK] YCCK
    [Lab] L*a*b*
    [Palette] Dizinli renk
    [Multichannel] Çok kanallı
    [Duotone] Çift ton
    [YCgCo] YCgCo
    [XYB] XYB
    [YC] Parlaklık/renklilik (YC)
    [Bayer] Bayer RAW
    [X-Trans] X-Trans RAW
    [CFA] Renk filtresi dizili RAW
    [LinearRaw] Doğrusal RAW
    [RGBE] RGBE (ortak üs)
    [XYZE] XYZE (ortak üs)
    [R] Tek kanal (R)
    [RG] İki kanal (RG)
   *[other] { $model }
}
adobe-rgb-from-exif = Adobe RGB (EXIF etiketinden; gömülü profil yok)
orientation = { $value ->
    [1] Dik (döndürme gerekmez)
    [2] Yatay aynalanmış (uygulandı)
    [3] 180° döndürülmüş (uygulandı)
    [4] Dikey aynalanmış (uygulandı)
    [5] Yatay aynalanmış ve saat yönünün tersine 90° döndürülmüş (uygulandı)
    [6] Saat yönünde 90° döndürülmüş (uygulandı)
    [7] Yatay aynalanmış ve saat yönünde 90° döndürülmüş (uygulandı)
    [8] Saat yönünün tersine 90° döndürülmüş (uygulandı)
   *[other] Bilinmiyor ({ $value })
}
# Uyarılar için kısa biçim.
orientation-short = { $value ->
    [1] Dik
    [2] Yatay aynalanmış
    [3] 180° döndürülmüş
    [4] Dikey aynalanmış
    [5] Yatay aynalanmış ve saat yönünün tersine 90° döndürülmüş
    [6] Saat yönünde 90° döndürülmüş
    [7] Yatay aynalanmış ve saat yönünde 90° döndürülmüş
    [8] Saat yönünün tersine 90° döndürülmüş
   *[other] değer { $value }
}

## Dosya

file-size-bytes = { $num ->
   *[other] { $num } bayt
}
# Ondalık birimler (1 KB = 1000 bayt).
file-size-unit = { $num } { $unit ->
    [kb] KB
    [mb] MB
    [gb] GB
    [tb] TB
   *[pb] PB
}

## Haklar

rating-rejected = Reddedildi
# $stars: "★★★★☆"
rating-stars = { $stars } (5 üzerinden { $n })

## İçerik kimlik bilgileri

c2pa-present = Mevcut (doğrulanmadı)
c2pa-detail = Bu dosya bir C2PA bildirimi taşıyor; imzası denetlenmedi.

## ICC üst bilgi değerleri

icc-class = { $class ->
    [scnr] Giriş aygıtı
    [mntr] Görüntüleme aygıtı
    [prtr] Çıkış aygıtı
    [link] Aygıt bağlantısı
    [spac] Renk uzayı
    [abst] Soyut
    [nmcl] Adlandırılmış renk
   *[other] Bilinmiyor
}
icc-intent = { $intent ->
    [0] Algısal
    [1] Göreli kolorimetrik
    [2] Doygunluk
    [3] Mutlak kolorimetrik
   *[other] Bilinmiyor
}
icc-platform = { $platform ->
    [APPL] Apple
    [MSFT] Microsoft
    [SGI] Silicon Graphics
    [SUNW] Sun Microsystems
    [none] Belirtilmemiş
   *[other] Bilinmiyor
}
icc-coded = { $name } ({ $code })

# machine-drafted, needs native review
label-modality = Modalite
label-body-part = Vücut bölgesi
label-study = Tetkik
label-series = Seri
label-slice-thickness = Kesit kalınlığı
label-pixel-spacing = Piksel aralığı
label-imager-pixel-spacing = Dedektörde piksel aralığı
label-frames = Kare
label-transfer-syntax = Aktarım sözdizimi
label-document = Belge
label-document-type = Belge türü
medical-spacing = { $row } × { $column } mm
medical-mm = { $mm } mm
medical-lossy = { $name } (kayıplı sıkıştırma)
detail-pixel-spacing = Hastada piksel merkezleri arasındaki mesafe (satır × sütun).
# machine-drafted, needs native review
# Pixel spacing of an industrial inspection image (DICONDE, CT of an object).
detail-pixel-spacing-object = Nesnede piksel merkezleri arasındaki mesafe (satır × sütun).
detail-imager-pixel-spacing = Dedektörde ölçülür, büyütme için düzeltilmemiştir: hastadaki mesafeler genellikle daha küçüktür.

## Endüstriyel muayene (DICONDE)

# machine-drafted, needs native review
label-nde-component-shape = Parça şekli
label-nde-component-manufacturer = Parça üreticisi
label-nde-material = Malzeme
label-nde-thickness = Malzeme kalınlığı
label-nde-pipe-diameter = Boru çapı
label-nde-source = Kaynak
label-nde-environment = Ortam
label-nde-iqi = Görüntü kalitesi göstergesi
label-nde-indications = Belirtiler
label-nde-standard = Standart

## Araştırma hacimleri (NIfTI, Analyze, NRRD, MetaImage)

# machine-drafted, needs native review
label-voxel-size = Voksel boyutu
label-data-type = Veri türü
label-time-points = Zaman noktaları
label-orientation-source = Yönelim kaynağı
label-intent = Değerler
label-compression = Sıkıştırma
volume-voxel-size = { $x } × { $y } × { $z } mm
volume-orient-sform = NIfTI sform
volume-orient-qform = NIfTI qform
volume-orient-analyze = Analyze başlığı (güvenilir değil)
volume-orient-space = NRRD uzay yönleri
volume-orient-transform = MetaImage dönüşümü
volume-orient-spacings = Yalnızca voksel aralığı
volume-orient-none = Belirtilmemiş
volume-orient-vox2ras = FreeSurfer vox2ras
volume-orient-mgh-default = FreeSurfer varsayılanı (güvenilir değil)
volume-orient-minc = MINC boyut yön kosinüsleri
volume-orient-object = Nesne koordinatları (hasta yönelimi yok)
volume-content-projections = Projeksiyonlar (ham tarama)
volume-content-reconstruction = Yeniden yapılandırılmış hacim
volume-coords-rectilinear = Doğrusal (düzensiz aralık, düzenli gösterilir)
# Scan parameters and tags a FreeSurfer MGH file keeps after its voxels.
label-volume-tr = Tekrarlama zamanı
label-volume-te = Eko zamanı
label-volume-ti = İnversiyon zamanı
label-volume-flip = Sapma açısı
label-volume-fov = Görüntüleme alanı
label-volume-field = Alan şiddeti
label-volume-pedir = Faz kodlama
label-volume-cmdline = Komut satırı
label-volume-xform = Talairach dönüşümü
label-volume-content = İçerik
label-volume-projections = Projeksiyonlar
label-volume-angles = Dönüş aralığı
label-volume-angle-step = Dönüş adımı
label-volume-kv = Tüp gerilimi
label-volume-current = Tüp akımı
label-volume-power = Kaynak gücü
label-volume-exposure = Pozlama
label-volume-sod = Kaynak–nesne
label-volume-odd = Nesne–dedektör
label-volume-sdd = Kaynak–dedektör
label-volume-magnification = Optik büyütme
label-volume-binning = Binning
label-volume-objective = Objektif
label-volume-filter = Kaynak filtresi
label-volume-camera = Dedektör
label-volume-instrument = Cihaz
label-volume-software = Yazılım
label-volume-date = Çekim tarihi
label-volume-ring = Halka artefaktı düzeltmesi
label-volume-smoothing = Yumuşatma
label-volume-coords = Izgara
# machine-drafted, needs native review
# An eye scan (OCT): which eye.
label-volume-eye = Göz
volume-eye-right = Sağ (OD)
volume-eye-left = Sol (OS)
detail-volume-unreliable = Dosya, eksenlerinin hastada nereyi gösterdiğini belirtmiyor: sol ve sağ yer değiştirmiş olabilir, bu yüzden yönelim etiketleri gösterilmez.
# machine-drafted, needs native review
# An industrial CT volume: its axes are the scanner's, there is no patient.
detail-volume-object = Eksenler bir hastanın değil tarayıcının eksenleridir, bu yüzden yönelim etiketleri gösterilmez.

## Bilimsel görüntüler (FITS, XISF)

label-object = Nesne
label-telescope = Teleskop
label-instrument = Alet
label-filter = Filtre
label-observed = Gözlem
label-exposure-time = Pozlama
label-sample = Örnekler
label-unit = Birim
label-hdu = Uzantı
label-centre = Merkez
label-centre-deg = Merkez (derece)
label-frame = Koordinat sistemi
# machine-drafted, needs native review
science-frame-galactic = Galaktik
science-frame-ecliptic = Ekliptik
label-projection = İzdüşüm
label-pixel-scale = Piksel ölçeği
science-seconds = { $seconds } sn
science-arcsec-px = Piksel başına { $arcsec }″
label-distortion = Bozulma
label-wcs = Gök koordinatları
science-wcs-unsupported = Desteklenmiyor: { $reason }
# machine-drafted, needs native review
science-wcs-approximate = Yaklaşık (yalnızca yönelim, bozulma yok)
label-observer = Gözlemci
label-bayer = Bayer deseni
label-capture-start = Kayıt başlangıcı (UTC)
label-capture-end = Kayıt sonu (UTC)
label-frame-rate = Kare hızı
science-fps = { $fps } kare/sn
science-frames-cut = { $frames } / { $declared }
# The HDF5 dataset or NetCDF variable shown.
label-dataset = Veri kümesi
label-pixel-size = Piksel boyutu
# "0.25 nm per pixel"; $size may be "0.25 × 0.5".
science-pixel-size = Piksel başına { $size } { $unit }
# Microscopy (DigitalMicrograph, MRC, Gwyddion, Igor).
label-microscope = Mikroskop
label-voltage = Hızlandırma gerilimi
label-magnification = Büyütme
label-imaging-mode = Mod
label-detector = Dedektör
label-acquired = Edinim
label-field-of-view = Görüş alanı
science-kilovolts = { $kv } kV
science-magnification = { $value }×
label-vendor = Üretici
label-scanner = Tarayıcı
label-objective = Objektif
label-pyramid-levels = Düzeyler
# machine-drafted, needs native review
label-emissivity = Yayınım
label-object-distance = Nesne uzaklığı
label-reflected-temperature = Yansıyan sıcaklık
label-atmospheric-temperature = Atmosfer sıcaklığı
label-humidity = Bağıl nem
label-raw-range = Ham aralık
label-temperature-range = Sıcaklık aralığı
thermal-temperature = { $value } { $unit }
thermal-metres = { $value } m
thermal-percent = { PERCENT($value) }
thermal-range = { $low } – { $high }
slide-levels = { $count }: { $sizes }
# X-ray diffraction: the experiment geometry from a detector frame's header.
label-wavelength = Dalga boyu
# Ångström, e.g. "0.97625 Å".
xtal-angstrom = { $value } Å
label-photon-energy = Foton enerjisi
xtal-kev = { $value } keV
# Sample to detector distance.
label-detector-distance = Dedektör mesafesi
xtal-mm = { $value } mm
# Where the direct beam meets the detector, in pixels from the image's top-left corner.
label-beam-centre = Işın merkezi
xtal-beam-centre = x { $x }, y { $y } px
# The detector arm's swing angle (2θ).
label-detector-swing = Dedektör 2θ
xtal-degrees = { $value }°
# Goniometer angles (Greek letters are the field's usual names).
label-phi = Fi (φ)
label-omega = Omega (ω)
label-chi = Ki (χ)
label-kappa = Kappa (κ)
# The sample's rotation during the exposure: start angle plus width, e.g. "12.5° + 0.1°".
label-oscillation = Salınım
xtal-oscillation = { $start }° + { $width }°
# $axis is the axis name from the file, e.g. "phi" or "omega".
xtal-oscillation-axis = { $axis } etrafında { $start }° + { $width }°
# The count at which a pixel saturates.
label-overload = Doyma değeri
# The finest lattice spacing (d) recorded at the detector's edge and in its corners.
label-resolution = Çözünürlük
xtal-resolution = Kenarda { $edge } Å, köşelerde { $corner } Å
label-geometry = Geometri
xtal-curved = Eğri görüntüleme plakası: q, d veya çözünürlük halkası yok
xtal-incomplete = Eksik başlık: q, d veya çözünürlük halkası yok
# All-fields group title: "FITS header".
fields-science-header = { $format } başlığı
# machine-drafted, needs native review
fields-asdf-tree = ASDF ağacı
label-asdf-model = Veri modeli
label-asdf-version = Dosya sürümü
label-asdf-standard = ASDF Standardı
label-asdf-library = Yazan
# machine-drafted, needs native review
# All-fields group title for a CERN ROOT file's list of keys (objects).
fields-root-keys = ROOT anahtarları
# CERN ROOT files: the ROOT version that wrote the file (e.g. "6.28/04"), its compression,
# the number of keys (stored objects), and the object shown: class (e.g. "TH2F"), its path in
# the file, title, number of entries and sum of weights.
label-root-version = ROOT sürümü
label-root-compression = Sıkıştırma
label-root-keys = Anahtarlar
label-root-class = Sınıf
label-root-name = Ad
label-root-title = Başlık
label-root-entries = Girdiler
label-root-sum-weights = Ağırlıklar toplamı
# A graph's number of points.
label-root-points = Noktalar
# $axis is "x", "y" or "z".
label-root-axis = Eksen { $axis }
# $quantity is the axis title with its unit, e.g. "Energy [GeV]"; $min and $max the axis
# limits.
root-axis-fixed = { $quantity }: { $bins } bin, { $min } ile { $max } arası
root-axis-variable = { $quantity }: { $bins } değişken genişlikli bin, { $min } ile { $max } arası
label-root-mean-rms = Ortalama ve RMS { $axis }
root-mean-rms = { $mean } ± { $rms }
# Trees (event data) are listed only, with their entry counts.
label-root-trees = Ağaçlar
root-tree = { $name } ({ $entries } girdi)
# machine-drafted, needs native review
# CASA images: what the image is (from its directory name), its restoring beam (major and
# minor axis, position angle), the beams of its planes when they differ, the rest frequency,
# spectral reference frame (e.g. "LSRK") and velocity definition of the frequency labels,
# the default mask (e.g. "mask0") and the casacore storage manager with its byte order.
label-casa-type = Görüntü türü
casa-type-image = Onarılmış görüntü
casa-type-residual = Artık
casa-type-model = Model
casa-type-pb = Birincil hüzme
casa-type-mask = Temizleme maskesi
casa-type-psf = Nokta yayılım fonksiyonu
casa-type-sumwt = Ağırlık toplamı
casa-type-weight = Ağırlıklar
label-casa-beam = Onarım hüzmesi
casa-beam = { $major }″ × { $minor }″, KA { $pa }°
label-casa-plane-beams = Düzlem başına hüzmeler
casa-plane-beams = { $count ->
        [one] { $count } hüzme
       *[other] { $count } hüzme
    }, büyük eksen { $min }″ ile { $max }″ arası
label-casa-rest-frequency = Durgun frekans
casa-ghz = { $ghz } GHz
label-casa-spectral-frame = Spektral referans sistemi
label-casa-velocity = Hız tanımı
casa-velocity-radio = Radyo
casa-velocity-optical = Optik
casa-velocity-relativistic = Göreli
label-casa-mask = Varsayılan maske
label-casa-storage = Depolama
casa-storage-big = { $manager }, big-endian
casa-storage-little = { $manager }, little-endian
# machine-drafted, needs native review
# Unstructured meshes: the mesh's name in the file, the convention describing it (UGRID,
# ICON, MPAS), its counts, face shapes, coordinates and where the values sit.
label-ugrid-name = Ağ
label-ugrid-convention = Kural
label-ugrid-nodes = Düğümler
label-ugrid-faces = Yüzler
label-ugrid-edges = Kenarlar
label-ugrid-shapes = Yüz şekilleri
label-ugrid-coordinates = Koordinatlar
label-ugrid-values = Değerlerin konumu
label-ugrid-split = Çizim için bölünmüş
# $n is the formatted count, $count the number (for the plural).
ugrid-triangles = { $count ->
    [one] { $count } üçgen
   *[other] { $count } üçgen
}
ugrid-quads = { $count ->
    [one] { $count } dörtgen
   *[other] { $count } dörtgen
}
# Faces of five or more nodes; $max is the most nodes of one face.
ugrid-polygons = { $count ->
    [one] { $max } düğümlü { $count } çokgen
   *[other] en çok { $max } düğümlü { $count } çokgen
}
ugrid-lonlat = Boylam ve enlem
ugrid-projected = İzdüşümlü x ve y
ugrid-projected-unit = İzdüşümlü x ve y ({ $unit })
ugrid-on-faces = Yüzler (düz renkli çizilir)
ugrid-on-nodes = Düğümler (yüzler boyunca ara değerlenir)
# A mesh file without data variables: the image shows how many nodes each face has.
ugrid-grid-only = Yok: görüntü yüz başına düğüm sayısını gösterir
# Faces cut at the antimeridian or closed to a pole so they draw on a flat map.
ugrid-split = { $count ->
    [one] antimeridyende veya bir kutupta { $count } yüz
   *[other] antimeridyende veya bir kutupta { $count } yüz
}
# machine-drafted, needs native review
# Slide annotations (NDP.view, Aperio ImageScope, ASAP sidecar files).
label-annotation-file = Dosya
# "slide.xml (Aperio ImageScope)"
annotation-source = { $file } ({ $format })
label-annotation-shapes = Şekiller
label-annotation-groups = Gruplar
# Shapes or points over the reader's limits, left out.
label-annotation-skipped = Atlanan
annotation-polygon = Çokgen
annotation-polyline = Çizgi
annotation-rectangle = Dikdörtgen
annotation-ellipse = Elips
annotation-point = Nokta
annotation-points = Noktalar
annotation-ruler = Cetvel
annotation-arrow = Ok
# The list of annotations is cut short: "and 1,200 more".
annotation-more = ve { $count } tane daha
# All-fields group title: "Annotations in slide.xml".
fields-annotations-header = { $file } içindeki açıklamalar
label-drawing-version = Sürüm
label-drawing-units = Birimler
label-drawing-extents = Kapsam
label-drawing-sheet = Pafta
label-drawing-layouts = Düzenler
label-drawing-layers = Katmanlar
label-drawing-entities = Nesneler
label-drawing-codepage = Kod sayfası
label-drawing-missing-fonts = Yerine konan yazı tipleri
# machine-drafted, needs native review
# A font file the drawing names that was not found: the styles naming it, and the font drawing its text instead.
drawing-missing-font = { $file } (stiller { $styles }), yerine { $substitute } kullanıldı
# A Big Font (double-byte text) that was not found; its characters come from other fonts.
drawing-missing-bigfont = { $file } (stiller { $styles }), yerine yedek yazı tipleri kullanıldı
# Survey data (LandXML).
label-survey-crs = Koordinat sistemi
survey-crs-none = Belirtilmemiş
label-survey-project = Proje adı
label-survey-application = Yazan
label-survey-surface = Arazi modeli
# A TIN surface: its name, counts, plan and sloped areas, the volume above its lowest point, its elevations and the contour interval drawn; unit is "m" or "ft" (areas in square units, volume in cubic units).
survey-surface = { $name }: { $points } nokta, { $triangles } üçgen; plan alanı { $area } { $unit }², yüzey alanı { $area3d } { $unit }²; en alçak noktanın üstünde { $volume } { $unit }³; kotlar { $zmin }–{ $zmax } { $unit }; eş yükselti eğrileri { $interval } { $unit } aralıkla
label-survey-alignment = Aks
# An alignment: its name, length and first and last stations ("1+234.50").
survey-alignment = { $name }: { $length } { $unit }, km { $start } ile { $end } arası
# SHX shape fonts found and used for the drawing's text.
label-drawing-shx-fonts = SHX yazı tipleri
label-drawing-warnings = Sorunlar
label-plot-paper = Kâğıt
# "2 of 5": the page shown and how many the plot has.
plot-page-of = { $page }/{ $pages }
label-plot-page = Sayfa
label-plot-pens = Kullanılan kalemler
# The printer language a plot came wrapped in, e.g. "PCL 5".
label-plot-job = Yazdırma işi
# What a plot draws: strokes, fills and labels.
label-plot-items = Çizilen öğeler
# Width × height of what is drawn, in drawing units: "1,250 × 800 mm".
drawing-extents = { $width } × { $height } { $unit }
drawing-extents-unitless = { $width } × { $height }
drawing-model-space = Model
# "24, 3 off or frozen"
drawing-layers-off = { $num }, { $off } tanesi kapalı veya dondurulmuş
# DXF $INSUNITS.
drawing-units = { $code ->
    [1] İnç
    [2] Fit
    [3] Mil
    [4] Milimetre
    [5] Santimetre
    [6] Metre
    [7] Kilometre
    [8] Mikroinç
    [9] Binde bir inç
    [10] Yarda
    [11] Ångström
    [12] Nanometre
    [13] Mikron
    [14] Desimetre
    [15] Dekametre
    [16] Hektometre
    [17] Gigametre
    [18] Astronomik birim
    [19] Işık yılı
    [20] Parsek
    [21] ABD ölçme fiti
   *[other] Birimsiz
}
# All-fields group title: "DXF entities".
fields-drawing-entities = { $format } nesneleri

## IC yerleşimleri (GDSII)

# machine-drafted, needs native review
label-layout-library = Kütüphane
label-layout-units = Birimler
# The database unit (the grid coordinates are stored in) and the user unit, with SI symbols:
# "1 nm database unit, 1 µm user unit".
layout-units = { $db } veritabanı birimi, { $user } kullanıcı birimi
# The top cell shown (each top cell is a page).
label-layout-cell = Hücre
label-layout-cells = Hücreler
# Cells no other cell places.
label-layout-top-cells = Üst hücreler
# "TOP, TEST, PAD and 12 more"
layout-top-cells-more = { $names } ve { $more } tane daha
# Layer/datatype pairs.
label-layout-layers = Katmanlar
# Polygons, paths and boxes, as stored (each cell once).
label-layout-shapes = Şekiller
label-layout-texts = Etiketler
# Cell references and the placements they make: "1,204 (88,320 placements)".
label-layout-references = Referanslar
layout-references = { $refs } ({ $placements } { $placements ->
    [one] yerleşim
   *[other] yerleşim
})
# Shapes with every cell placement expanded.
label-layout-flat-shapes = Düzleştirilmiş şekiller
label-layout-bounds = Sınırlar
# Corners of what the cell draws, micrometres: "-12.5, 0 to 2,900, 3,500 µm".
layout-bounds = { $x0 }, { $y0 } – { $x1 }, { $y1 } µm

## Devre kartları ve şemalar (Gerber, Excellon, KiCad)

# machine-drafted, needs native review
group-board = Devre kartı
group-schematic = Şema
# The board view shown (its layers, its top or bottom), or the schematic sheet.
label-pcb-view = Görünüm
label-pcb-project = Proje
label-pcb-revision = Revizyon
label-pcb-date = Tarih
# The CAD program that wrote the file.
label-pcb-generator = Oluşturan
# Width × height of the board outline: "100 × 80 mm".
label-pcb-size = Kart boyutu
label-pcb-thickness = Kalınlık
label-pcb-copper-layers = Bakır katmanlar
label-pcb-sheets = Sayfalar
# The board's layers or files, each with its role: "board-F_Cu.gbr (Top copper)".
label-pcb-layers = Katmanlar
pcb-layer-role = { $name } ({ $role })
label-pcb-components = Ayak izleri
label-pcb-symbols = Semboller
label-pcb-nets = Netler
label-pcb-tracks = Yollar
label-pcb-wires = Teller
label-pcb-vias = Via’lar
# Drilled holes and how many drill sizes: "1,204 in 9 sizes".
label-pcb-holes = Delikler
pcb-holes = { $holes }, { $sizes } { $sizes ->
    [one] boyut
   *[other] boyut
}
label-pcb-apertures = Açıklıklar
# What a board layer or file is, shown after its name.
pcb-role-inner-copper = İç bakır { $n }
pcb-role-top-copper = Üst bakır
pcb-role-bottom-copper = Alt bakır
pcb-role-top-solder-mask = Üst lehim maskesi
pcb-role-bottom-solder-mask = Alt lehim maskesi
pcb-role-top-silkscreen = Üst serigrafi
pcb-role-bottom-silkscreen = Alt serigrafi
pcb-role-top-paste = Üst pasta
pcb-role-bottom-paste = Alt pasta
pcb-role-top-solder-paste = Üst lehim pastası
pcb-role-bottom-solder-paste = Alt lehim pastası
pcb-role-top-adhesive = Üst yapıştırıcı
pcb-role-bottom-adhesive = Alt yapıştırıcı
pcb-role-top-courtyard = Üst yerleşim alanı
pcb-role-bottom-courtyard = Alt yerleşim alanı
pcb-role-top-fabrication = Üst üretim
pcb-role-bottom-fabrication = Alt üretim
pcb-role-board-outline = Kart dış hattı
pcb-role-plated-drill = Kaplamalı delme
pcb-role-non-plated-drill = Kaplamasız delme
pcb-role-drill = Delme
pcb-role-drill-holes = Delme delikleri
pcb-role-mechanical = Mekanik
pcb-role-margin = Kenar boşluğu
pcb-role-drawings = Çizimler
pcb-role-comments = Yorumlar
pcb-role-engineering-change-1 = Mühendislik değişikliği 1
pcb-role-engineering-change-2 = Mühendislik değişikliği 2
pcb-role-vias = Via’lar
pcb-role-other = Diğer
pcb-role-unknown = Bilinmiyor
label-pcb-macros = Açıklık makroları

## Spektrumlar (JCAMP-DX, SPC, mzML, mzXML, Bruker NMR)

# machine-drafted, needs native review
group-spectrum = Spektrum
label-spectrum-format = Biçim
label-spectrum-technique = Teknik
label-spectrum-title = Başlık
# The page shown: "3 of 1,204: Scan 1532".
label-spectrum-page = Sayfa
spectrum-page-of = { $page } / { $pages }: { $name }
# An axis with its unit: "Wavenumber (cm⁻¹)".
spectrum-axis = { $title } ({ $unit })
label-spectrum-x = X ekseni
label-spectrum-y = Y ekseni
label-spectrum-nucleus = Çekirdek
label-spectrum-frequency = Spektrometre frekansı
label-spectrum-solvent = Çözücü
label-spectrum-pulse-program = Puls programı
label-spectrum-spectra = Spektrumlar
label-spectrum-points = Noktalar
label-spectrum-instrument = Alet
# Who or what lab measured it.
label-spectrum-origin = Kaynak
label-spectrum-date = Ölçüm tarihi
# How Specular made the page from the raw data (an FID transformed, phased automatically).
label-spectrum-processing = İşleme
spectrum-technique-ir = Kızılötesi
spectrum-technique-raman = Raman
spectrum-technique-uv-vis = UV-Vis
spectrum-technique-fluorescence = Floresans
spectrum-technique-nmr = NMR
spectrum-technique-ms = Kütle spektrumu
spectrum-technique-chromatogram = Kromatogram
spectrum-technique-other = Spektrum

## 3D modeller (STL, OBJ, PLY, glTF, 3MF)

# machine-drafted, needs native review
group-mesh = 3B model
label-triangles = Üçgenler
label-vertices = Köşeler
label-materials = Malzemeler
label-textures = Dokular
label-up-axis = Yukarı ekseni
# Textures the model uses, some of which could not be read: "3 (1 missing)".
mesh-textures-missing = { $count } ({ $missing } eksik)

## Molekül yapıları (PDB, mmCIF, CIF, SDF, XYZ)

# machine-drafted, needs native review
group-structure = Yapı
# The entry's identifier: a PDB ID such as 1CRN, or the CIF data block's name.
label-structure-id = Kayıt
label-structure-classification = Sınıflandırma
# How the structure was determined: X-ray diffraction, electron microscopy, solution NMR…
label-structure-method = Yöntem
# The crystallographic or cryo-EM resolution, in ångströms.
label-structure-resolution = Çözünürlük
structure-resolution = { $value } Å
label-structure-deposited = Depolanma
label-structure-formula = Formül
label-structure-chains = Zincirler
# Residues (amino acids, nucleotides) in the polymer chains.
label-structure-residues = Kalıntılar
label-structure-atoms = Atomlar
# Atoms after the unit cell was filled by symmetry, and those the file lists: "96 (12 in the asymmetric unit)".
structure-atoms-cell = { $count } (asimetrik birimde { $asymmetric })
label-structure-bonds = Bağlar
# Models in the file (an NMR ensemble), of which the first is shown.
label-structure-models = Modeller
# Molecules (SDF records) or data blocks in the file, of which the first is shown.
label-structure-records = Kayıtlar
structure-first-of = { $count } (ilki gösteriliyor)
# Small molecules bound to the polymer, by their three-letter codes.
label-structure-ligands = Ligandlar
# A list cut short: "HEM, NAG, SO4 and 9 more".
structure-ligands-more = { $list } ve { $more } tane daha
label-structure-cell = Birim hücre
# Unit cell edge lengths (Å) and angles (degrees).
structure-cell = a = { $a } Å, b = { $b } Å, c = { $c } Å, α = { $alpha }°, β = { $beta }°, γ = { $gamma }°
label-structure-space-group = Uzay grubu
label-structure-secondary = İkincil yapı
structure-ss-file = Dosyadan
structure-ss-computed = Omurgadan hesaplandı

## HDF4 ve HDF-EOS2

# machine-drafted, needs native review
group-hdf4 = HDF4
# The HDF library version that wrote the file, e.g. "NCSA HDF Version 4.2 Release 15".
label-hdf4-library = Kitaplık
# Counts of the file's scientific datasets, raster images, groups (Vgroups) and tables (Vdatas).
label-hdf4-datasets = Veri kümeleri
label-hdf4-images = Raster görüntüler
label-hdf4-vgroups = Gruplar
label-hdf4-vdatas = Tablolar
# HDF-EOS: NASA's Earth Observing System conventions on top of HDF4 (MODIS, ASTER, AIRS…).
group-hdf-eos = HDF-EOS
label-eos-version = Sürüm
# The grids, swaths and point structures the file defines.
label-eos-structures = Yapılar
eos-grids = { $count ->
    [one] { $count } ızgara ({ $names })
   *[other] { $count } ızgara ({ $names })
}
eos-swaths = { $count ->
    [one] { $count } tarama şeridi ({ $names })
   *[other] { $count } tarama şeridi ({ $names })
}
eos-points = { $count ->
    [one] { $count } nokta yapısı
   *[other] { $count } nokta yapısı
}
# The grid or swath the dataset shown belongs to.
label-eos-structure = Yapı
# The grid's map projection as its GCTP name, e.g. GCTP_SNSOID (sinusoidal).
label-eos-projection = Projeksiyon
label-eos-grid-size = Izgara boyutu
# Columns × rows.
eos-grid-size = { $width } × { $height }
# The grid's upper-left and lower-right corners.
label-eos-corners = Köşeler
# The product's short name from the inventory metadata, e.g. MOD09A1.
label-eos-product = Ürün
# When the data was acquired.
label-eos-time = Zaman
eos-time = { $begin } – { $end }

## Gezegen görüntüleri (VICAR, PDS3, PDS4: NASA ve ESA görev verileri)

# machine-drafted, needs native review
group-planetary = Gezegen görüntüsü
# The body observed, e.g. MARS, Ganymede.
label-planetary-target = Hedef
label-planetary-mission = Görev
# The spacecraft, rover or lander carrying the instrument.
label-planetary-spacecraft = Uzay aracı
label-planetary-start = Başlangıç zamanı
label-planetary-stop = Bitiş zamanı
# The product's identifier in the archive.
label-planetary-product = Ürün kimliği
# The map projection of a map-projected image, as the label names it.
label-planetary-projection = Harita izdüşümü
label-planetary-resolution = Çözünürlük
planetary-metres-per-pixel = Piksel başına { $metres } m
planetary-pixels-per-degree = Derece başına { $pixels } piksel
# Latitude and longitude of the map projection's centre.
label-planetary-centre = İzdüşüm merkezi
planetary-lat-lon = { $lat }°, { $lon }°
# The latitudes and longitudes the map covers.
label-planetary-coverage = Kapsam
planetary-coverage = enlem { $south }° – { $north }°, boylam { $west }° – { $east }°
# The body's equatorial radius the map uses.
label-planetary-radius = Gök cisminin yarıçapı
planetary-km = { $km } km

## OMERO (bir OMERO sunucusundan veya Image Data Resource'tan açılan görüntüler)

# machine-drafted, needs native review
group-omero = OMERO
label-omero-server = Sunucu
# $server the server's address; $user the user name signed in with.
omero-server-user = { $server } ({ $user } olarak)
omero-server-public = { $server } (herkese açık)
label-omero-image = Görüntü
# $name the image's name; $id its number on the server.
omero-image = { $name } (ID { $id })
# The project and dataset the image is in, as "Project › Dataset".
label-omero-project = Proje
# The screen, plate and well, as "Screen › Plate › B4".
label-omero-screen = Tarama
label-omero-owner = Sahip
# $owner the image's owner; $group the OMERO group it belongs to.
omero-owner-group = { $owner } ({ $group })
label-omero-acquired = Edinim
label-omero-dimensions = Boyutlar
omero-size = { $width } × { $height }
omero-z = { $count ->
    [one] { $count } Z düzlemi
   *[other] { $count } Z düzlemi
}
omero-c = { $count ->
    [one] { $count } kanal
   *[other] { $count } kanal
}
omero-t = { $count ->
    [one] { $count } zaman noktası
   *[other] { $count } zaman noktası
}
# The physical size of a pixel along X, Y (and Z).
label-omero-physical = Piksel boyutu
label-omero-channels = Kanallar
# A channel not shown in the server's rendering.
omero-channel-hidden = { $channel } (gizli)
# How the pixels are fetched from the server: rendered tiles (JPEG) or the whole plane (PNG).
label-omero-fetched = Alınma biçimi
omero-fetched-tiles = { $width } × { $height } boyutunda { $encoding } döşemeler, { $levels ->
    [one] { $levels } düzey
   *[other] { $levels } düzey
}
omero-fetched-whole = { $encoding } olarak tüm düzlemler

## Yansıma dönüşümü görüntüleri (PTM, RTI, Relight)

# machine-drafted, needs native review
group-rti = Yansıma dönüşümü
# The reflectance model the coefficients belong to: PTM (polynomial), HSH (hemispherical harmonics), RBF…
label-rti-basis = Taban
label-rti-order = Derece
# Coefficients per colour channel.
label-rti-terms = Katsayılar
# How colour is stored: LRGB (one colour, varying luminance), RGB, YCC…
label-rti-colour = Renk düzeni
# Coefficient images stored in the file.
label-rti-planes = Düzlemler
# The byte-to-coefficient mapping the file states, per coefficient.
label-rti-scale = Ölçek
label-rti-bias = Sapma

## Nokta bulutları

# machine-drafted, needs native review
group-points = Nokta bulutu
label-points = Noktalar
# Points read of those in the file, when a very large cloud was evenly thinned: "32,000,000 of 120,000,000".
points-subsampled = { $loaded } / { $total }
# The LAS point data record format number (0–10).
label-point-format = Nokta biçimi
# The coordinate reference system.
label-crs = Koordinat sistemi
# What each point carries besides its position.
label-attributes = Öznitelikler
points-channel-rgb = RGB
points-channel-intensity = Yoğunluk
points-channel-classification = Sınıflandırma
points-channel-returns = Dönüşler
# ASPRS classes present, with point counts: "Ground (1,234), Building (567)".
label-classes = Sınıflar
# E57 scans, with point counts.
label-scans = Taramalar
point-class-0 = Hiç sınıflandırılmamış
point-class-1 = Sınıflandırılmamış
point-class-2 = Zemin
point-class-3 = Alçak bitki örtüsü
point-class-4 = Orta bitki örtüsü
point-class-5 = Yüksek bitki örtüsü
point-class-6 = Bina
point-class-7 = Alçak nokta (gürültü)
point-class-8 = Model anahtar noktası
point-class-9 = Su
point-class-10 = Demiryolu
point-class-11 = Yol yüzeyi
point-class-12 = Örtüşme
point-class-13 = Koruma teli
point-class-14 = İletken tel
point-class-15 = İletim kulesi
point-class-16 = Tel bağlantısı
point-class-17 = Köprü tabliyesi
point-class-18 = Yüksek gürültü
point-class-19 = Havai yapı
point-class-20 = Yok sayılan zemin
point-class-21 = Kar
point-class-22 = Zamansal hariç tutma
# A class without a standard name, by number.
point-class-other = Sınıf { $class }

## Gaussian splats

# machine-drafted, needs native review
group-splats = Gauss splat'leri
label-splats = Splat'ler
# Splats shown of those in the file, when a very large file was evenly thinned: "12,000,000 of 30,000,000".
splats-shown-of = { $shown } / { $total }
# The degree (1 to 3) of the spherical harmonics that give each splat its view-dependent colour.
label-splat-harmonics = Görüş açısına bağlı renk
splats-harmonics-none = Yok
# $format: the file format, e.g. "SPZ".
splats-format-antialiased = { $format }, kenar yumuşatmayla eğitilmiş

## Uyarılar

# machine-drafted, needs native review
warn-not-diagnostic = Tanı amaçlı değildir. Specular genel bir görüntü görüntüleyicisidir, tıbbi cihaz değildir: görüntülemesi tanı için kalibre edilmemiştir.

warn-gps = Bu dosya nerede çekildiğini kaydediyor. Gizli kalması gerekiyorsa paylaşmadan önce konumu kaldırın.
warn-null-island = GPS konumu tam olarak 0°, 0°; kamera büyük olasılıkla konum belirleyemedi, bu yüzden konum muhtemelen yanlış.
warn-date-future = Çekim tarihi gelecekte; kamera saati büyük olasılıkla yanlış ayarlanmış.
warn-icc-damaged = Gömülü renk profili bozuk ve okunamadı.
warn-metadata-failed = Specular’daki bir hata nedeniyle meta veriler okunamadı. Görüntünün kendisi sağlam.
warn-orientation-conflict = EXIF ve XMP yönlendirme konusunda uyuşmuyor (EXIF: { $exif }, XMP: { $xmp }). EXIF değeri kullanılır.
warn-exif-damaged = Bazı EXIF verileri okunamadı ({ $error }); geri kalanı gösteriliyor.
warn-xmp-damaged = XMP meta verileri hatalı biçimlendirilmiş ({ $error }); yalnızca bir kısmı okunabildi.

## Hakkında penceresi

geonames-attribution = Yer adları: GeoNames (www.geonames.org), Creative Commons Attribution 4.0 (CC BY 4.0, https://creativecommons.org/licenses/by/4.0/) ile lisanslanmıştır. Çevrimdışı arama, Grant Miner'ın reverse_geocoder crate'i ile yapılır (MIT veya Apache-2.0). Veriler bir şehir listesine dönüştürülmüştür; hiçbir garanti olmaksızın olduğu gibi sağlanır.

## TIFF and DNG structure values (All fields)
# machine-drafted, needs native review

subfile-type = { $value ->
    [0] Tam çözünürlüklü görüntü
    [1] Düşük çözünürlüklü görüntü
    [2] Çok sayfalı bir görüntünün bir sayfası
    [4] Saydamlık maskesi
   *[other] Diğer
}
predictor = { $value ->
    [1] Yok
    [2] Yatay fark
    [3] Kayan nokta farkı
   *[other] Diğer
}
extra-samples = { $value ->
    [0] Belirtilmemiş
    [1] Önceden çarpılmış alfa
    [2] Düz alfa
   *[other] Diğer
}
sample-format = { $value ->
    [1] İşaretsiz tamsayı
    [2] İşaretli tamsayı
    [3] Kayan nokta
    [4] Tanımsız
   *[other] Diğer
}

## Coğrafi veriler (GeoTIFF, GeoJSON, Shapefile, GeoPackage, KML/KMZ, GPX)

# machine-drafted, needs native review
group-geospatial = Coğrafi veri
label-geo-crs = Koordinat sistemi
label-geo-datum = Datum
label-geo-projection = Projeksiyon
label-geo-unit = Birimler
label-geo-bounds = Kapsam
label-geo-lonlat-bounds = Boylam ve enlem
label-geo-pixel-size = Piksel boyutu
label-geo-overviews = Genel bakışlar
label-geo-nodata = Veri yok değeri
label-geo-raster-type = Raster türü
label-geo-vertical = Düşey referans
label-geo-citation = Kaynak
label-geo-features = Objeler
label-geo-vertices = Köşeler
label-geo-layer = Katman
label-geo-layers = Katmanlar
label-geo-tiles = Döşemeler
label-geo-encoding = Metin kodlaması
label-geo-warnings = Sorunlar
# The companion files a georeference was read from ("scene.jgw, scene.prj").
label-geo-sidecars = Eşlik eden dosyalar
# machine-drafted, needs native review
# A projection Specular cannot convert to longitude and latitude: $method is its name.
geo-projection-unsupported = { $method } (koordinatlar desteklenmiyor)
# How longitude and latitude are brought to WGS 84 (what GPS and web maps use).
label-geo-to-wgs84 = WGS 84'e
geo-to-wgs84-same = Gerekmiyor
geo-to-wgs84-helmert = Datum dönüşümü (Helmert)
geo-to-wgs84-by-area = Datum dönüşümü (Helmert, bölgeye göre seçildi)
geo-to-wgs84-unknown = Bilinmiyor: koordinatlar dosyanın kendi datumunda
# A coordinate system's name and its EPSG code: "WGS 84 / UTM zone 33N (EPSG:32633)".
geo-crs-epsg = { $name } (EPSG:{ $code })
geo-crs-unknown = Belirtilmemiş
# Map bounds: the lower-left and upper-right corners.
geo-bounds = { $x0 }, { $y0 } ile { $x1 }, { $y1 } arası
# Bounds in degrees: west to east longitude, south to north latitude.
geo-lonlat-bounds = { $west }° ile { $east }° boylam, { $south }° ile { $north }° enlem arası
geo-pixel-size = { $x } × { $y }
geo-overviews = { $n ->
   *[other] { $n } genel bakış: { $sizes }
}
geo-pixel-is-point = Pikseller noktadır
geo-layer = { $features ->
   *[other] { $features } obje
}, { $fields ->
   *[other] { $fields } alan
}
# A GeoPackage tile pyramid: its table, the zoom level shown and its tile count.
geo-tiles = { $table }, yakınlaştırma { $zoom }, { $tiles ->
   *[other] { $tiles } döşeme
}
fields-geo-geokeys = GeoTIFF anahtarları
fields-geo-gdal = GDAL meta verileri
fields-geo-wkt = Koordinat sistemi (WKT)
fields-geo-projjson = Koordinat sistemi (PROJJSON)
# machine-drafted, needs native review
# What a web map tile set (MBTiles, PMTiles) says about itself: container, tile type, zoom
# levels, description and other metadata.
fields-geo-tileset = Döşeme kümesi
# The credit line a map's data asks to be shown with it ("© OpenStreetMap contributors").
label-geo-attribution = Kaynak
# The header lines of a georeferenced raster format (ENVI, ASCII grid, DTED, ERDAS IMAGINE).
fields-geo-header = Başlık
# machine-drafted, needs native review
# GIFTI files: the format version, what the file holds, the number of data arrays and the
# encoding of their values (e.g. "GZipBase64Binary").
label-gifti-version = GIFTI sürümü
label-gifti-role = İçerik
gifti-role-surface = Yüzey
gifti-role-labels = Köşe başına etiketler
gifti-role-data = Köşe başına değerler
label-gifti-arrays = Veri dizileri
label-gifti-encoding = Kodlama
label-gifti-vertices = Köşeler
label-gifti-triangles = Üçgenler
# The brain structure, e.g. "CortexLeft".
label-gifti-structure = Yapı
# A surface's geometric type, e.g. "Anatomical", "Inflated".
label-gifti-geometry = Geometrik tür
# The coordinate transform of the vertices: $from and $to are coordinate spaces such as
# "scanner", "Talairach" or "MNI 152".
label-gifti-transform = Koordinatlar
gifti-transform = { $from } ({ $to } dönüşümü gerekmiyor)
gifti-transform-applied = { $from } → { $to } (uygulandı)
# The surface file a data file is drawn on, found beside it.
label-gifti-surface = Gösterildiği yüzey
# The data array shown: its name, NIfTI intent (e.g. "NIFTI_INTENT_SHAPE") and data type.
label-gifti-array = Dizi
label-gifti-intent = Intent
label-gifti-data-type = Veri türü
# Entries of the label table (names and colours of regions).
label-gifti-labels = Etiket tablosu
# CIFTI-2 files: the version, NIfTI intent (e.g. "ConnDenseScalar") and matrix size.
label-cifti-version = CIFTI sürümü
label-cifti-intent = Intent
label-cifti-matrix = Matris
cifti-matrix = { $rows } × { $columns }
# The maps (pages) of the file and what they are.
label-cifti-maps = Haritalar
cifti-maps-scalars = { $count } skaler harita
cifti-maps-labels = { $count } etiket haritası
cifti-maps-series = { $count } zaman noktası
cifti-maps-parcels = { $count } parsel
cifti-maps-other = { $count }
# machine-drafted, needs native review
# A dense connectivity file (.dconn.nii): every brain place's connectivity to every other.
# $seeds: how many places can be picked; $size: the bytes read when one is picked (e.g. "357 KB").
label-cifti-dense = Bağlantısallık
cifti-dense = { $seeds } tohum, tohum başına { $size } okunur
# A time series' start and step; $unit is e.g. "second".
label-cifti-series = Seri
cifti-series = { $start } başlangıçlı, { $step } aralıklı ({ $unit })
label-cifti-models = Beyin modelleri
label-cifti-parcels = Parseller
label-cifti-structures = Yapılar
label-cifti-voxels = Subkortikal vokseller
# A surface the file's values lie on: the brain structure (e.g. "CORTEX_LEFT"), its vertex
# count, and the surface file found beside the file for it, or that none was found.
label-cifti-surface = Yüzey
cifti-surface-found = { $structure }: { $vertices } köşe, { $file }
cifti-surface-missing = { $structure }: { $vertices } köşeli bir yüzey gerekiyor (dosyanın yanında yok)
# How the file is shown: on the surfaces found beside it, or as its subcortical voxels.
label-cifti-shown = Gösterim
cifti-shown-surfaces = Renklendirilmiş yüzeyler
cifti-shown-voxels = Subkortikal voksellerin hacmi
# machine-drafted, needs native review
# Shown with a picked seed's row: click a surface vertex, or move the crosshair in MPR.
cifti-shown-connectivity = Seçilen tohumun bağlantısallığı
# Maps left out of the voxel volume, which would be too large with them.
label-cifti-left-out = Dışarıda bırakılan haritalar
# Tractograms: format, number of streamlines (fibre tracts) and points.
label-tract-format = Biçim
label-tract-streamlines = Akım çizgileri
# A large tractogram draws only some of its streamlines.
tract-streamlines-shown = { $count } ({ $shown } gösteriliyor)
label-tract-points = Noktalar
label-tract-mean-length = Ortalama uzunluk
# A length in millimetres.
tract-mm = { $value } mm
# Values per point and per streamline the file stores, by name.
label-tract-scalars = Nokta başına skalerler
label-tract-properties = Akım çizgisi başına özellikler
# TrackVis: the voxel grid the tracts were made in, its voxel size and voxel order (e.g. "LAS").
label-tract-grid = Izgara
label-tract-voxel-size = Voksel boyutu
label-tract-voxel-order = Voksel sırası
# How the points were brought into scanner (RAS) millimetres.
label-tract-space = Koordinatlar
tract-space-vox-to-ras = Vokselden RAS’a matristen
tract-space-voxel-order = Voksel sırasından (vokselden RAS’a matris yok)
tract-space-scanner = Tarayıcı uzayı
# MRtrix: the tracking step size.
label-tract-step = Adım boyu

## File headers (DPX, Cineon, XWD, MIFF, TIM, WSQ, JBIG)

# machine-drafted, needs native review
# What the file's own header records, for formats without Exif or XMP.
group-header = Dosya üstbilgisi
label-header-created = Oluşturulma
# The program or person that wrote the file.
label-header-creator = Oluşturan
label-header-copyright = Telif hakkı
label-header-project = Proje
label-header-comment = Yorum
# The scanner, camera or other device the image came from.
label-header-device = Aygıt
# SMPTE time code of the frame: "01:00:00:12".
label-header-timecode = Zaman kodu
# A film frame's edge code (KeyKode).
label-header-edge-code = Kenar kodu
label-header-frame-rate = Kare hızı
# Where the frame sits in its sequence.
label-header-frame = Kare
# Film stock and slate.
label-header-film = Film
# How code values relate to light: printing density, linear, BT.709…
label-header-transfer = Aktarım işlevi
# The colour primaries the file states.
label-header-colorimetric = Kolorimetri
# The X window a screen dump was taken of.
label-header-window = Pencere
label-header-resolution = Çözünürlük
label-header-compression = Sıkıştırma
# machine-drafted, needs native review
# JPEG XS: the profile and the level the codestream states ("Main 422.10"; "4k-1, sublevel 6 bpp").
label-header-profile = Profil
# machine-drafted, needs native review
label-header-level = Düzey
# machine-drafted, needs native review
# JPEG XS: how many wavelet levels, horizontal and vertical ("5 horizontal, 1 vertical").
label-header-decomposition = Dalgacık düzeyleri
# machine-drafted, needs native review
# JPEG XS: the colour transform ("None", "Reversible colour transform (RCT)", "Star-Tetrix, RGGB mosaic").
label-header-colour-transform = Renk dönüşümü
# machine-drafted, needs native review
# Photoshop: the artboards, by name and size ("Phone (375 × 667), Tablet (768 × 1024)").
label-header-artboards = Çalışma yüzeyleri
# machine-drafted, needs native review
# Photoshop file without a merged image, shown from its layers: the features it leaves out,
# by Photoshop's names ("Color Lookup, Knockout").
label-header-not-drawn = Çizilmeyen
# machine-drafted, needs native review
# The same, for the features drawn only approximately ("Bevel & Emboss, Satin").
label-header-approximated = Yaklaşık çizilen

## Fonts (TTF, OTF, TTC/OTC, WOFF, WOFF2)

# machine-drafted, needs native review
group-font = Yazı tipi
label-font-family = Aile
label-font-style = Stil
# The face's place in a font collection: "2 of 4".
label-font-face = Yüz
font-face-of = { $index } / { $count }
label-font-version = Sürüm
label-font-designer = Tasarımcı
# The font's maker (the name table's manufacturer).
label-font-manufacturer = Dökümhane
label-font-glyphs = Glifler
label-font-outlines = Anahatlar
font-outlines-truetype = TrueType (karesel)
font-outlines-cff = PostScript (CFF, kübik)
font-outlines-cff2 = PostScript (CFF2, kübik)
font-outlines-bitmap = Yalnızca bit eşlemler
font-outlines-none = Yok
# What the font's licence flags (OS/2 fsType) allow documents that embed it to do.
label-font-embedding = Gömme
font-embedding-installable = Gömülebilir ve kurulabilir.
font-embedding-restricted = Gömülemez.
font-embedding-preview-print = Yalnızca görüntüleme ve yazdırma için gömülebilir.
font-embedding-editable = Düzenlenen belgelere gömülebilir.
font-embedding-no-subsetting = Yalnızca tamamı, alt küme olarak değil.
font-embedding-bitmap-only = Yalnızca bit eşlem olarak.
# Variation axes of a variable font: "Weight 100–900 (400)".
label-font-axes = Değişim eksenleri
font-axis = { $name } { $min }–{ $max } ({ $default })
label-font-instances = Adlandırılmış stiller
label-font-colour = Renkli glifler
font-colour-colr0 = COLR katmanları
font-colour-colr1 = COLR v1 geçişleri
font-colour-sbix = Bit eşlemler (sbix)
font-colour-cbdt = Bit eşlemler (CBDT)
font-colour-svg = SVG
label-font-weight = Kalınlık
label-font-units-per-em = Em başına birim
label-font-tables = Tablolar
label-font-licence = Lisans
label-font-licence-url = Lisans URL'si
label-font-copyright = Telif hakkı
label-font-trademark = Ticari marka

## Office documents (Word, Excel, PowerPoint, iWork, OpenDocument)

# machine-drafted, needs native review
group-document = Belge
label-document-kind = Tür
document-kind-word = Word belgesi
document-kind-excel = Excel çalışma kitabı
document-kind-powerpoint = PowerPoint sunusu
document-kind-pages = Pages belgesi
document-kind-numbers = Numbers elektronik tablosu
document-kind-keynote = Keynote sunusu
document-kind-odf-text = OpenDocument metni
document-kind-odf-spreadsheet = OpenDocument elektronik tablosu
document-kind-odf-presentation = OpenDocument sunusu
document-kind-odf-graphics = OpenDocument çizimi
document-kind-odf-other = OpenDocument dosyası
# machine-drafted, needs native review
document-kind-playlist = Çalma listesi
# machine-drafted, needs native review
label-playlist-items = Öğeler
# Appended to the kind: "Word document, template, with macros".
document-template = şablon
document-macros = makrolu
document-legacy = Office 97–2003 biçimi
label-document-preview = Önizleme
# The image format of the preview stored in the file, e.g. "JPEG image".
document-preview-image = { $format } görüntüsü
document-preview-none = Bu dosyada yok
label-document-subject = Konu
label-document-author = Yazar
label-document-description = Açıklama
label-document-category = Kategori
label-document-last-saved-by = Son kaydeden
label-document-created = Oluşturulma
label-document-modified = Değiştirilme
label-document-printed = Son yazdırma
label-document-pages = Sayfalar
label-document-slides = Slaytlar
label-document-hidden-slides = Gizli slaytlar
label-document-notes = Not sayfaları
label-document-sheets = Çalışma sayfaları
label-document-words = Sözcükler
label-document-characters = Karakterler
label-document-paragraphs = Paragraflar
label-document-lines = Satırlar
label-document-editing-time = Düzenleme süresi
document-minutes = { $minutes } dk
label-document-revision = Düzeltme
label-document-application = Kaydeden uygulama
label-document-company = Şirket
label-document-template = Şablon

## Books (EPUB)

# machine-drafted, needs native review
group-book = Kitap
label-book-authors = Yazarlar
label-book-publisher = Yayıncı
label-book-date = Yayımlanma
label-book-language = Dil
label-book-pages = Sayfalar
# Entries in the book's table of contents.
label-book-contents = İçindekiler girdileri
label-book-layout = Düzen
book-layout-reflowable = Akışkan
book-layout-fixed = Sabit düzen
book-right-to-left = sağdan sola
label-book-version = Biçim
book-version = EPUB { $version }
label-book-fonts = Gömülü yazı tipleri
book-fonts-obfuscated = { $count } ({ $obfuscated } gizlenmiş)
label-book-subjects = Konular
label-book-identifier = Tanımlayıcı
label-book-rights = Haklar

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

# machine-drafted, needs native review
volume-orient-parrec = Philips açılandırma ve merkez kayması
volume-orient-ecat = ECAT voksel boyutları ve kaymaları (güvenilir değil)
label-volume-technique = Teknik
label-volume-version = Biçim sürümü
label-volume-isotope = İzotop
label-volume-tracer = İzleyici
label-volume-units = Birimler

# Text, tables and subtitles.
label-header-encoding = Karakter kümesi
label-header-lines = Satırlar
label-header-line-ends = Satır sonları
label-header-words = Sözcükler
label-header-pages = Sayfalar
label-header-pictures = Resimler
label-header-pictures-not-shown = Gösterilmeyen resimler
label-header-tables = Tablolar
label-header-rows = Tablo satırları
label-header-columns = Sütunlar
label-header-delimiter = Ayırıcı
label-header-column-names = Sütun adları
label-header-cues = Altyazılar
label-header-cue-span = Altyazılar: başlangıç – bitiş
