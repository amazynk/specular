# Specular — Scientific images (FITS, XISF): stretch, limits, planes, channels, coordinates. Türkçe.

keys-mode-science = Bilimsel görüntüler
cheatsheet-science = Bilimsel görüntüler
cheatsheet-science-look = Germe ve sınırlar
cheatsheet-science-planes = Düzlemler ve kanallar
# machine-drafted, needs native review
cheatsheet-science-channels-1-9 = Kanal 1–9 arasını göster veya gizle
cheatsheet-science-channels-10-16 = Kanal 10–16 arasını göster veya gizle
palette-science-stretch = Sonraki germe
palette-science-limits = Sonraki sınırlar
palette-science-next-plane = Sonraki düzlem
palette-science-prev-plane = Önceki düzlem
palette-science-next-axis = Başka bir eksende ilerle
palette-science-composite = Kanalları renkli birleştir
palette-science-channel-1 = Kanal 1'i göster veya gizle
palette-science-channel-2 = Kanal 2'yi göster veya gizle
palette-science-channel-3 = Kanal 3'ü göster veya gizle
palette-science-channel-4 = Kanal 4'ü göster veya gizle
palette-science-channel-5 = Kanal 5'i göster veya gizle
palette-science-channel-6 = Kanal 6'yı göster veya gizle
palette-science-channel-7 = Kanal 7'yi göster veya gizle
palette-science-channel-8 = Kanal 8'i göster veya gizle
palette-science-channel-9 = Kanal 9'u göster veya gizle
# machine-drafted, needs native review
palette-science-channel-10 = Kanal 10'u göster veya gizle
palette-science-channel-11 = Kanal 11'i göster veya gizle
palette-science-channel-12 = Kanal 12'yi göster veya gizle
palette-science-channel-13 = Kanal 13'ü göster veya gizle
palette-science-channel-14 = Kanal 14'ü göster veya gizle
palette-science-channel-15 = Kanal 15'i göster veya gizle
palette-science-channel-16 = Kanal 16'yı göster veya gizle
palette-science-grid = Koordinat ızgarasını göster veya gizle
palette-science-reset = Otomatik görünüm
palette-science-level = Yükseklik haritalarını düzelt (ortalama düzlemi çıkar)
palette-science-wiggle = Renklerin üzerinde değişken alanlı sismik wiggle izleri
# machine-drafted, needs native review
palette-science-wireframe = Renklerin üzerinde yapılandırılmamış ağ kenarları (tel kafes)
palette-science-import-lut = Dosyadan renk tablosu (ImageJ .lut)…
palette-science-hillshade = Yükseklik modellerinin rölyefini gölgelendir (tepe gölgesi)
palette-science-fft = Düzlemin güç spektrumu (FFT)
palette-science-catalogue = Kaynak kataloğu bindir (VOTable, FITS tablosu, CSV)…
palette-science-clear-catalogues = Bindirilen katalogları kaldır
# A catalogue overlaid on a sky image: its name (the table's or the file's), how many of its
# sources fall on the image, and how many of those are drawn (at most a set number).
science-catalogue-toast = { $name }: { $count ->
        [one] görüntüde bir kaynak
       *[other] görüntüde { $count } kaynak
    }
science-catalogue-capped = { $name }: görüntüdeki { $count } kaynaktan { $shown } tanesi gösteriliyor
science-catalogue-none = { $name }: görüntüde kaynak yok
science-catalogue-failed = Gök konumları içeren bir katalog değil: { $name }
science-catalogues-cleared = Kataloglar kaldırıldı
# The file dialog's filter name for catalogue files.
science-catalogue-filter = Kaynak katalogları
# A GW sky map's credible region: the smallest area holding this share of the probability,
# drawn as a contour labelled e.g. "90 %".
science-credible = { PERCENT($percent) }
palette-science-ion-image = Bir m/z değerinde iyon görüntüsü (kütle spektrometrisi)…
disabled-not-science = Yalnızca bilimsel görüntüler için (astronomi, elektron ve taramalı uç mikroskobu)
disabled-one-plane = Görüntünün tek düzlemi var
disabled-one-axis = Görüntüde ilerlenecek başka eksen yok
disabled-no-channels = Görüntüde birleştirilecek böyle bir kanal yok
disabled-no-coordinates = Görüntüde gök koordinatları yok
disabled-not-height-map = Yalnızca yükseklik haritaları için (Gwyddion, Igor)
disabled-not-elevation = Yalnızca tek bantlı coğrafi referanslı yükseklik modelleri için
disabled-not-seismic = Yalnızca sismik veriler için (SEG-Y, Seismic Unix)
disabled-not-mass-spec = Yalnızca kütle spektrometrisi görüntüleri için (imzML)
# machine-drafted, needs native review
disabled-not-ugrid = Yalnızca yapılandırılmamış ağlar için (UGRID, ICON ve MPAS NetCDF)
disabled-not-bayer = Yalnızca renkli kamera ham kareleri için (Bayer desenli FITS, SER video)
palette-debayer = Renkli kamera karelerini debayer edilmiş veya ham göster
palette-bayer-pattern = Bu dosya için sonraki Bayer deseni
science-debayer-on = Debayer edildi ({ $pattern })
science-debayer-off = Ham renk filtresi dizisi
science-bayer-pattern = Bayer deseni { $pattern }
# machine-drafted, needs native review
palette-annotations = Açıklamaları göster veya gizle
disabled-no-annotations = Görüntünün yanında açıklama yok (NDPA, Aperio veya ASAP XML)
science-annotations-on = Açıklamalar gösteriliyor
science-annotations-off = Açıklamalar gizlendi
science-stretch-auto = Otomatik (her renk)
# Stretches: how values between the limits map to grey levels.
science-stretch-linear = Doğrusal
science-stretch-sqrt = Karekök
science-stretch-log = Logaritmik
science-stretch-asinh = Asinh
science-stretch-histeq = Histogram eşitleme
# Limits: which values show black and white. $percent is e.g. 99.5.
science-limits-zscale = Zscale
science-limits-minmax = Min/maks
science-limits-percentile = { PERCENT($percent) }
# Seismic gain: limits symmetric about zero at the $percent percentile of the amplitudes.
science-limits-symmetric = ±{ PERCENT($percent) }
# The display range the file stores (ImageJ's min and max), used until other limits are chosen.
science-limits-file = Dosya aralığı
# Toasts when the look changes.
science-stretch-toast = Germe: { $stretch }
science-limits-toast = Sınırlar: { $limits }
science-reset-toast = Otomatik görünüm
# An imported colour table: $name is its file name.
science-lut-toast = Renk tablosu { $name }
science-lut-failed = Renk tablosu değil: { $name }
# The file dialog's filter name for .lut files.
science-lut-filter = ImageJ renk tabloları
science-composite-on = Kanallar renkli birleştirildi
science-composite-off = Gri tonlamalı tek kanal
science-channel-on = Gösteriliyor: { $channel }
science-channel-off = Gizlendi: { $channel }
# machine-drafted, needs native review
# A channel without a name in the file, in the shown/hidden toasts.
science-channel-unnamed = Kanal { $number }
science-grid-on = Koordinat ızgarası gösteriliyor
science-grid-off = Koordinat ızgarası gizlendi
# X-ray diffraction frames: the same action shows circles of equal resolution (d-spacing).
science-rings-on = Çözünürlük halkaları gösteriliyor
science-rings-off = Çözünürlük halkaları gizlendi
# A resolution ring's label: its d-spacing in ångström, e.g. "2.5 Å".
science-ring = { $d } Å
# Seismic wiggle traces: drawn once the traces are at least 4 screen pixels apart.
science-wiggle-on = Wiggle izleri açık (görmek için yakınlaştırın)
science-wiggle-off = Wiggle izleri kapalı
# machine-drafted, needs native review
# Unstructured meshes: the edges drawn over the faces; too many to draw in view until the
# user zooms in.
science-wireframe-on = Ağ kenarları açık
science-wireframe-off = Ağ kenarları kapalı
science-wireframe-dense = Ağ kenarları açık (görmek için yakınlaştırın)
# Plane levelling (scanning probe height maps): the best-fit plane is subtracted for display.
science-level-on = Düzlem düzeltme açık
science-level-off = Düzlem düzeltme kapalı
# Relief shading of an elevation model. $azimuth is the sun's direction in degrees clockwise
# from north, $altitude its height above the horizon in degrees.
science-hillshade-on = Tepe gölgesi açık: güneş { $azimuth }° yönünde, { $altitude }° yükseklikte
science-hillshade-off = Tepe gölgesi kapalı
# The power spectrum view: the plane's centred 2-D Fourier power, log scaled.
science-fft-on = Güç spektrumu gösteriliyor
science-fft-off = Güç spektrumu gizlendi
science-axis-toast = İlerlenen eksen: { $axis }
# Readout lines (bottom corner).
# The value under the pointer, with the values' unit when the file states one.
science-value = Değer { $value }
science-value-unit = Değer { $value } { $unit }
# machine-drafted, needs native review
science-value-flags = Değer { $value } (bitler { $bits })
# machine-drafted, needs native review
# A class value under the pointer and its name, e.g. "Value 4 · Vegetation".
science-value-class = Değer { $value } · { $class }
# machine-drafted, needs native review
# Sentinel-2 scene classes (L2A SCL, ESA's names) and L1C cloud masks.
s2-class-no-data = Veri yok
s2-class-defective = Doygun veya kusurlu
s2-class-dark = Karanlık alan pikselleri
s2-class-cloud-shadow = Bulut gölgeleri
s2-class-vegetation = Bitki örtüsü
s2-class-not-vegetated = Bitki örtüsüz
s2-class-water = Su
s2-class-unclassified = Sınıflandırılmamış
s2-class-cloud-medium = Orta olasılıklı bulut
s2-class-cloud-high = Yüksek olasılıklı bulut
s2-class-cirrus = İnce sirrus
s2-class-snow = Kar veya buz
s2-class-clear = Açık
s2-class-opaque-cloud = Opak bulutlar
# Several channels under the pointer: "R 12.5  G 3.1  B 0.4".
science-values = { $values }
# Sky position under the pointer, sexagesimal then degrees.
science-radec = α { $ra }  δ { $dec }
science-radec-deg = { $ra }°  { $dec }°
# Galactic or ecliptic longitude and latitude.
science-lonlat = l { $lon }°  b { $lat }°
science-wcs-unsupported = WCS desteklenmiyor: { $reason }
# machine-drafted, needs native review
# A B1950 (FK4) image: what is converted between its frame and another. Its own position
# stays B1950 in the readout. ICRS, FK4 and SAMP are names and stay as they are.
science-fk4-markers = Katalog işaretleri: ICRS → FK4 dönüştürüldü
science-fk4-markers-galactic = Katalog işaretleri: galaktik → FK4 dönüştürüldü
science-fk4-samp = SAMP: FK4 → ICRS dönüştürüldü
# The look: stretch and limits.
science-look = { $stretch } · { $limits }
# Shown under the look while plane levelling is on.
science-levelled = Düzlem düzeltildi
# Shown under the look while the hillshade is on.
science-hillshaded = Tepe gölgesi: güneş { $azimuth }° yönünde, { $altitude }° yükseklikte
# Shown in the readout while the power spectrum is shown.
science-fft = Güç spektrumu (log₁₀)
# The spectrum's value under the pointer: log₁₀ of the power.
science-fft-power = log₁₀ güç { $value }
# Spatial frequency under the pointer along x and y, and its unit ("1/µm", "1/px").
science-fft-frequency = Frekans u { $u }  v { $v } { $unit }
# The period of that frequency: "Period 2.5 nm".
science-fft-period = Periyot { $period }
# Crystal orientation under the pointer (EBSD): Bunge Euler angles in degrees.
science-euler = φ₁ { $phi1 }  Φ { $phi }  φ₂ { $phi2 }
# The crystal phase under the pointer and its symmetry ("Iron fcc (m-3m)").
science-phase = Faz { $phase } ({ $symmetry })
# Title of the spectrum plot of the pixel under the pointer (an EDS map).
science-spectrum-title = { $x }, { $y } noktasında spektrum
# Physical position under the pointer (microscopy): "x 1.25 µm  y 3.5 µm".
science-position = x { $x }  y { $y }
# machine-drafted, needs native review
# CERN ROOT histograms: the bin under the pointer on one axis. $axis is the axis title
# ("p_{T}", or "x"), $range the bin's label or its edges ("20 GeV – 25 GeV"), $bin its
# number counted from 1.
science-root-bin = { $axis } { $range } (bin { $bin })
# A ROOT graph's point under the pointer.
science-root-point = x { $x }  y { $y }
# Under- and overflow bins (fills outside the axis ranges) hold something but are not shown.
science-root-flow = { $count } alt/üst taşma bini gizlendi (toplam { $sum })
# Bins of different widths are drawn on an even grid of pixels.
science-root-resampled = Değişken genişlikli binler düzgün bir ızgaraya yeniden örneklendi
# machine-drafted, needs native review
# A flow cytometry plot (FCS) under the pointer: each axis's parameter and its value.
science-cytometry-at = { $x } { $xv }  { $y } { $yv }
# A cytometry histogram under the pointer: its parameter and the value.
science-cytometry-at-x = { $x } { $xv }
# Events in the plot's bin (or the histogram's bar) under the pointer.
science-cytometry-events = { $count ->
    [one] { $count } olay
   *[other] { $count } olay
}
# The plot's axis scales ($x, $y: Linear, Log, Logicle or Arcsinh).
science-cytometry-scales = x { $x }, y { $y }
science-cytometry-scale-x = x { $x }
# Spillover compensation is applied, from the file's keyword ($keyword, e.g. $SPILLOVER).
science-cytometry-compensated = Kompanse edildi ({ $keyword })
science-cytometry-uncompensated = Kompanse edilmedi
# X-ray diffraction frames: what lies under the pointer. q is the momentum transfer
# (per ångström), d the lattice spacing (ångström), 2θ the scattering angle. The symbols
# stay as they are: "q 0.512 Å⁻¹  d 12.27 Å  2θ 4.56°".
science-scatter = q { $q } Å⁻¹  d { $d } Å  2θ { $tth }°
# Seismic readout: the trace under the pointer in a 3-D volume ($inline, $crossline the
# header numbers) or a 2-D line; $trace counts from 1 in file order, $cdp is the CDP number.
science-seismic-cell = Inline { $inline }  Crossline { $crossline }
science-seismic-trace = İz { $trace }  CDP { $cdp }
# A cell of a 3-D volume with no trace in the file.
science-seismic-gap = İz yok
# Two-way time (ms) or depth of the row under the pointer.
science-seismic-time = Zaman { $time } ms
science-seismic-depth = Derinlik { $depth } { $unit }
# The trace's CDP (or source) X and Y with the coordinate scalar applied.
science-seismic-xy = X { $x }  Y { $y }
# machine-drafted, needs native review
# Unstructured mesh readout: the face (cell) under the pointer, counting from 0 as in the
# file's connectivity, and the node nearest to the pointer when the values sit on nodes.
science-mesh-face = Yüz { $face }
science-mesh-face-node = Yüz { $face }  Düğüm { $node }
# The pointer lies between the mesh's faces (land in an ocean mesh, outside its boundary).
science-mesh-outside = Ağın dışında
# Position on an axis past the image: "Plane 3 of 40", or with its world value.
science-plane = { $axis } { $index }/{ $count }
science-plane-world = { $axis } { $index }/{ $count } ({ $world })
science-axis-unnamed = Düzlem
science-axis-extension = Görüntü
science-axis-channel = Kanal
# A column of a HEALPix sky map (probability, distance…) shown as the image.
science-axis-field = Alan
# X-ray event lists binned into an image: the bin size, in the event list's sky pixels
# ("Bin 2 of 4 (8 px)"), and the energy band of the events counted.
science-axis-bin = Bin
science-bin-size = { $size } px
science-axis-band = Enerji
science-band-all = tüm enerjiler
# machine-drafted, needs native review
# Complex-valued arrays (NumPy, MATLAB, Zarr, TDMS): which part of each value is shown.
science-axis-part = Bileşen
science-part-magnitude = genlik
science-part-real = gerçel kısım
science-part-imaginary = sanal kısım
science-part-phase = faz (radyan)
# machine-drafted, needs native review
# Complex radar (SAR) images: intensity in decibels.
science-part-db = yoğunluk (dB)
# machine-drafted, needs native review
# The frames of a thermal sequence (FLIR SEQ/CSQ), in time.
science-axis-frame = Kare
science-readout = Değer ve koordinatlar
# Accessible names of the plane sliders.
science-sliders = Düzlemler
# The scale bar's length.
science-scale-arcsec = { $value }″
science-scale-arcmin = { $value }′
science-scale-deg = { $value }°
# A physical length, e.g. "20 nm" (electron microscopy images).
science-scale-length = { $value } { $unit }
# machine-drafted, needs native review
# A length along a named axis of a plot, e.g. "20 GeV (p_{T})".
science-scale-axis = { $length } ({ $axis })
science-scale-a11y = Ölçek çubuğu: { $length }
# Mass spectrometry images (imzML). The image is the total ion current (each spot's summed
# intensities) until an m/z is typed; then that ion's image, its intensities summed over
# m/z ± the tolerance. $window is e.g. "760.5851 ± 5 ppm".
science-ion-tic = Toplam iyon akımı
science-ion-window = İyon görüntüsü m/z { $window }
# The m/z prompt: $text is what is typed so far (opens on the base peak under the pointer).
science-ion-prompt = m/z değerinde iyon görüntüsü: { $text }▏
# $tolerance is e.g. "10 ppm" or "0.1 Da"; a tolerance can also be typed after the m/z
# ("760.585 5 ppm"). Tab steps the tolerance; Enter on an empty prompt shows the total ion
# current again.
science-ion-prompt-hint = ± { $tolerance } · Tab tolerans · Enter göster · Esc iptal
science-ion-bad = Geçerli bir m/z değil: { $text }
# The spot under the pointer as the file numbers it (from 1).
science-ms-spot = Nokta x { $x }  y { $y }
science-ms-spot-z = Nokta x { $x }  y { $y }  z { $z }
science-ms-no-spectrum = Bu noktada spektrum yok
# machine-drafted, needs native review
palette-science-temperature-unit = Sıcaklık birimi (°C, °F, K)
palette-science-isotherm = İşaretçinin altındaki sıcaklığın üstünde veya altında izoterm
palette-science-thermal-parameters = Yayınım ve yansıyan sıcaklık…
palette-science-export-temperatures = Sıcaklıkları dışa aktar (kayan noktalı TIFF veya CSV)…
palette-thermal-visible = Termal görüntünün görünür fotoğrafı
disabled-not-thermal = Yalnızca termal görüntüler için
disabled-not-radiometric = Dosya sıcaklıkları saklıyor: parametreleri değiştirilemez
disabled-no-visible-photo = Termal görüntünün görünür fotoğrafı yok
science-temperature-unit-toast = Sıcaklıklar { $unit } cinsinden
science-isotherm-above = { $level } üstünde izoterm
science-isotherm-below = { $level } altında izoterm
science-isotherm-off = İzoterm kapalı
science-thermal-emissivity-prompt = Yayınım: { $text }
science-thermal-reflected-prompt = Yansıyan sıcaklık ({ $unit }): { $text }
science-thermal-prompt-hint = Tab sonraki alan · Enter yeniden hesapla · Esc iptal
science-thermal-bad = Geçerli bir değer değil: { $text }
science-thermal-applied = Yayınım { $emissivity }, yansıyan { $reflected } için yeniden hesaplandı
science-thermal-conditions = Yayınım { $emissivity } · yansıyan { $reflected }
science-thermal-export-tiff = 32 bit kayan noktalı TIFF
science-thermal-export-csv = CSV
science-thermal-export-done = Sıcaklıklar { $name } dosyasına kaydedildi
science-thermal-export-failed = Sıcaklıklar kaydedilemedi: { $reason }
# machine-drafted, needs native review
science-values-failed = Değerler gösterilemedi: { $reason }
# machine-drafted, needs native review
# Thermal sequences (FLIR SEQ/CSQ): playback and each measurement over time.
palette-science-thermal-play = Termal diziyi oynat veya duraklat
palette-science-thermal-series-csv = Zaman içindeki sıcaklıkları kaydet (CSV)…
disabled-not-thermal-sequence = Yalnızca termal diziler için
thermal-seq-title = Zaman içindeki sıcaklıklar
# $time is seconds from the first frame, e.g. "0.12 s".
thermal-seq-frame = Kare { $frame } / { $frames } · { $time }
thermal-seq-play = Oynat
thermal-seq-pause = Duraklat
thermal-seq-playing = Oynatılıyor
thermal-seq-paused = Duraklatıldı
thermal-seq-save-csv = CSV kaydet…
thermal-seq-hint = Zaman içindeki grafiğini çizmek için bir nokta, çizgi veya alan (M) ekleyin.
thermal-seq-reading = Kare okunuyor: { $done } / { $total }…
thermal-seq-failed = Kareler okunamadı: { $reason }
thermal-seq-no-series = Henüz kaydedilecek bir şey yok: bir nokta, çizgi veya alan ekleyin.
# For screen readers: each measurement's value at the frame shown, e.g.
# "1 Spot: 36.4 °C, 2 Rectangle: 30.1 °C".
thermal-seq-chart = Grafik: { $values }
# machine-drafted, needs native review
# The report (PDF) of the image shown: a thermal report for a thermal image, an inspection report for any other.
palette-thermal-report = Rapor (PDF)…
thermal-report-heading = Termal rapor
thermal-report-default-title = Termografik inceleme raporu
thermal-report-title-label = Başlık
thermal-report-notes-label = Notlar
thermal-report-photo = Görünür fotoğrafı ekle
thermal-report-folder = Bu klasördeki diğer termal görüntüleri ekle
thermal-report-save = PDF kaydet…
thermal-report-busy = Rapor yazılıyor…
thermal-report-nothing = Sıcaklıklar henüz gösterilmiyor.
thermal-report-saved = Rapor { $name } dosyasına kaydedildi ({ $count ->
    [one] { $count } görüntü
   *[other] { $count } görüntü
})
thermal-report-failed = Rapor kaydedilemedi: { $reason }
# In the PDF.
thermal-report-notes = Notlar
thermal-report-measurements = Ölçümler
thermal-report-no-measurements = Ölçüm yok.
thermal-report-thermal-caption = Termal görüntü
thermal-report-visible-caption = Görünür fotoğraf
thermal-report-col-measurement = Ölçüm
thermal-report-col-min = Min.
thermal-report-col-max = Maks.
thermal-report-col-mean = Ortalama
thermal-report-col-emissivity = Yayınım
thermal-report-whole-image = Tüm görüntü
thermal-report-camera = Kamera
thermal-report-lens = Objektif
thermal-report-date = Çekim
thermal-report-place = Yer
thermal-report-frame = Kare
thermal-report-emissivity = Yayınım
thermal-report-reflected = Yansıyan sıcaklık
thermal-report-distance = Mesafe
thermal-report-humidity = Nem
thermal-report-range = Aralık
thermal-report-format = Biçim
thermal-report-page = Sayfa { $page } / { $pages }
