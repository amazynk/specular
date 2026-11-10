# Multiplanar reformatting (MPR).

palette-mpr = Çok düzlemli yeniden yapılandırma (MPR)
palette-mpr-slab = Sonraki MPR kesiti (ince, MIP, MinIP, ortalama)
disabled-no-volume = Bu bir hacim değil: bir seri, çok kareli bir dosya, bir 3D hacim veya bir z yığını açın
disabled-mpr-closed = Önce çok düzlemli görünümleri açın
mpr-axial = Aksiyel
mpr-coronal = Koronal
mpr-sagittal = Sagital
mpr-oblique = Oblik
mpr-xy = XY
mpr-xz = XZ
mpr-yz = YZ
mpr-3d = 3D görünüm
mpr-thin = İnce
mpr-mip = MIP
mpr-minip = MinIP
mpr-average = Ortalama
mpr-slab = Kalınlık: { $mode }, { $thickness } mm
# machine-drafted, needs native review
mpr-slab-angstrom = Kalınlık: { $mode }, { $thickness } Å
mpr-slab-unit = Kalınlık: { $mode }, { $thickness } { $unit }
mpr-slab-voxels = Kalınlık: { $mode }, { $thickness } voksel
mpr-thicker = Daha kalın
mpr-thinner = Daha ince
mpr-reset = Görünümleri sıfırla
mpr-close = MPR’yi kapat
mpr-maximise = Görünümü büyüt
mpr-export-planes = Düzlemleri dışa aktar
# machine-drafted, needs native review
# Streamlines (TrackVis .trk, MRtrix .tck) found beside a brain volume, drawn over each
# view within a slab about its plane; Alt+S steps the slab, then hides them. $files: the
# files' names; $thickness: the slab in millimetres.
mpr-tracts = { $files } akım çizgileri: { $thickness } mm kalınlık
mpr-tracts-off = { $files } akım çizgileri: gizli
mpr-tracts-none = Bu hacmin yanında TrackVis veya MRtrix akım çizgisi yok
mpr-position = { $view } { $position } mm
# machine-drafted, needs native review
mpr-position-angstrom = { $view } { $position } Å
mpr-position-unit = { $view } { $position } { $unit }
mpr-position-voxels = { $view } { $position }
mpr-length-unit = { $value } { $unit }
mpr-length-voxels = { $value } voksel
mpr-axis-label = { $axis }: { $label }
mpr-axis-position = { $axis }: { $position }/{ $count }
mpr-axis-channel = Kanal
mpr-axis-unnamed = Eksen { $axis }
mpr-one-channel = MPR bir seferde tek kanal gösterir
# machine-drafted, needs native review
# $count: channels drawn together in their colours.
mpr-composite = { $count } kanal renkli, her biri kendi görüntüleme aralığında
# $reason: error-reason-needs-memory ("it needs 6 GB of free RAM, but only 4 GB is free right now. ...").
mpr-composite-memory = Tek kanal gösteriliyor: { $reason }
mpr-isotropic = Dosyada voksel boyutu yok: vokseller küp olarak gösteriliyor
mpr-isotropic-z = Dosyada kesit aralığı yok: kesitler bir piksel arayla gösteriliyor
# A seismic volume's inline and crossline sections (the time slice reads "Time … ms").
mpr-seismic-inline = Inline { $inline }
mpr-seismic-crossline = Crossline { $crossline }
mpr-value = { $value } { $unit }
mpr-value-plain = { $value }
mpr-loading = { $loaded }/{ $count } kesit yükleniyor
mpr-loaded = { $count } kesit, { $width } × { $height }
mpr-downsampled = 1/{ $factor } çözünürlükte gösteriliyor (bellek veya grafik sınırları)
mpr-converted = Değerler 32 bit kayan nokta olarak gösteriliyor
mpr-opening = Hacim açılıyor…
mpr-gpu-memory = Grafik kartında { $size }
mpr-refused = Hacim oluşturulamıyor: { $reason }
mpr-failed = Hacim yüklenmedi: { $reason }
mpr-no-gpu = Grafik kartı bu hacmi tutamıyor: { $reason }
mpr-planes-saved = { $count } düzlem { $folder } klasörüne kaydedildi
mpr-planes-failed = Düzlemler kaydedilemedi: { $reason }
mpr-view-a11y = { $view } görünümü, { $position } mm
mpr-crosshair-a11y = Artı imleç { $x }, { $y }, { $z } mm’de
# machine-drafted, needs native review
mpr-crosshair-a11y-angstrom = Artı imleç { $x }, { $y }, { $z } Å’de
mpr-crosshair-a11y-unit = Artı imleç { $x }, { $y }, { $z } { $unit } konumunda
mpr-crosshair-a11y-voxels = Artı imleç { $x }, { $y }, { $z } vokselinde
mpr-controls = MPR denetimleri

# 3D view palette entries.
palette-mpr-preset = Sonraki 3D ön ayarı (BT kemik, yumuşak doku, akciğer, anjiyo; MR)
palette-mpr-render = Sonraki 3D işleme (gölgeli, Cinematic, MIP, MinIP)
palette-mpr-clip = Sonraki 3D kırpma (yok, 2D görünümlerin kutusu, kesilmiş köşe)
palette-mpr-export-stl = 3D yüzeyi STL olarak dışa aktar…
# machine-drafted, needs native review
palette-mpr-tract-slab = MPR’de sonraki akım çizgisi kalınlığı
palette-mpr-maximise = MPR görünümünü büyüt veya tüm görünümleri göster
palette-mpr-export-planes = MPR düzlemlerini görüntü olarak dışa aktar…

# The 3D view: transfer function presets, renderings, clipping; STL surface export.
vr-preset = { $preset ->
    [bone] BT kemik
    [soft] BT yumuşak doku
    [lung] BT akciğer
    [angio] BT anjiyo
   *[mr] MR
}
vr-mode = { $mode ->
    [cinematic] Cinematic
    [mip] MIP
    [minip] MinIP
   *[shaded] Gölgeli
}
vr-clip = { $clip ->
    [box] Kırpma: görünümlerin kutusu
    [cut] Kırpma: kesilmiş köşe
   *[off] Kırpma yok
}
vr-smooth = Yüzeyi yumuşat
vr-export = STL dışa aktar
vr-threshold = Yüzey eşiği: { $value }
vr-stl-exporting = Yüzey oluşturuluyor…
vr-stl-saved = { $count ->
    [0] Bu eşikte hiçbir şey yok: yüzey kaydedilmedi
   *[other] { $count } üçgen { $name } dosyasına kaydedildi
}
vr-stl-failed = Yüzey kaydedilemedi: { $reason }
vr-view-a11y = { $preset }, { $mode }, { $clip }, yüzey eşiği { $threshold }. Shift+oklar döndürür, artı ve eksi yakınlaştırır.

## Ham hacim: başlıksız bir .raw / .vol dosyasının düzeni

# machine-drafted, needs native review
raw-volume-title = Ham hacmi aç
raw-volume-file = { $file } dosyasının başlığı yok. Örneklerin nasıl dizildiğini belirtin.
raw-volume-guess-label = Önerilen düzen
raw-volume-guess = { $index } / { $count }
raw-volume-custom = Özel
raw-volume-width = Genişlik (voksel)
raw-volume-height = Yükseklik (voksel)
raw-volume-slices = Kesitler
raw-volume-type = Örnek türü
raw-volume-type-unsigned = İşaretsiz { $bits } bit
raw-volume-type-signed = İşaretli { $bits } bit
raw-volume-type-float = Kayan noktalı { $bits } bit
raw-volume-order = Bayt sırası
raw-volume-little = Little-endian
raw-volume-big = Big-endian
raw-volume-offset = Başlık ofseti
raw-volume-bytes = { $count ->
   *[other] { $count } bayt
}
raw-volume-pixel = Piksel boyutu
raw-volume-spacing = Kesit aralığı
raw-volume-mm = { $value } mm
raw-volume-exact = Dosyayı tam dolduruyor ({ $size })
raw-volume-fits = { $file } dosyasının { $size } kadarını kullanıyor; { $left } artıyor
raw-volume-too-big = { $size } gerekiyor, ancak dosya { $file }
raw-volume-cancel = İptal
raw-volume-open = Aç
# machine-drafted, needs native review
# A dense connectivity file (CIFTI .dconn.nii) over voxels: the volume shown is one seed's
# row; letting go of the crosshair on another seed's voxel shows that one. $seed: the
# structure as the file names it and the voxel, such as "THALAMUS_LEFT (4, 6, 3)".
mpr-dconn-seed = { $seed } bağlantısallığı; artı imlecini başka bir tohuma taşıyın
# machine-drafted, needs native review
mpr-measure-hint-edit = Taşımak için bir ucu sürükleyin; kaldırmak için diğer ucun üzerine bırakın
# machine-drafted, needs native review
mpr-measure-removed = Ölçüm kaldırıldı
mpr-measure-lengths-only = MPR yalnızca uzunluk ölçer
mpr-more = Daha fazla…

## Curved MPR: a reformat along a curve clicked on a plane.
## machine-drafted, needs native review

palette-mpr-curve = Tıklanan eğri boyunca kavisli MPR
palette-mpr-curve-layout = Kavisli MPR: düzleştirilmiş veya gerilmiş
palette-mpr-curve-save = Eğriyi kaydet…
palette-mpr-curve-load = Eğri yükle…
palette-mpr-export-segments = Her segmenti STL olarak dışa aktar…
palette-mpr-dental = Dental düzen (panoramik ve kesitler)
palette-mpr-nerve = Mandibular kanalı izle
cpr-kind = { $kind ->
    [stretched] Gerilmiş
   *[straightened] Düzleştirilmiş
}
cpr-button = Kavisli
cpr-controls = Kavisli MPR denetimleri
cpr-at = Konum: { $position }
cpr-section = Kesit
cpr-hint-start = { $view } görünümünde yapı boyunca noktalara tıklayın
cpr-hint-more = Bir nokta daha tıklayın
cpr-add-points = Nokta ekle
cpr-undo-point = Noktayı geri al
# machine-drafted, needs native review
cpr-mark = Buraya işaret koy
cpr-mark-set = { $n }. işaret kondu.
cpr-mark-removed = İşaret kaldırıldı.
cpr-mark-here = İşaret { $n }
cpr-diameter = Çap { $value }
cpr-diameters = Çap { $min } ile { $max } arası
cpr-mark-span = İşaret { $from } – { $to }: merkez çizgisi boyunca { $length }
cpr-mark-total = Tüm işaretler: { $length }
cpr-clear = Eğriyi temizle
cpr-save = Eğriyi kaydet
cpr-load = Eğri yükle
cpr-done = Bitti
cpr-a11y = { $name }, uzunluk { $length }, imleç { $position } konumunda. Tekerlek imleci taşır.
cpr-none = Henüz eğri yok: önce noktalara tıklayın
cpr-cleared = Eğri temizlendi
cpr-saved = Eğri kaydedildi: { $name }
cpr-save-failed = Eğri kaydedilemedi: { $reason }
cpr-loaded = Eğri yüklendi
# machine-drafted, needs native review
# $count: centrelines read from the file.
cpr-loaded-count = { $count ->
    [one] 1 merkez çizgisi yüklendi
   *[other] { $count } merkez çizgisi yüklendi
}
# $count: centrelines found in files saved beside the volume.
cpr-sidecars-found = { $count ->
    [one] Kayıtlı 1 merkez çizgisi bulundu
   *[other] Kayıtlı { $count } merkez çizgisi bulundu
}
# A centreline without a name, by its place in the list.
cpr-centreline = Merkez çizgisi { $number }
cpr-new = Yeni merkez çizgisi
cpr-rename = Yeniden adlandır
cpr-name = Merkez çizgisi adı
cpr-delete = Sil
# $name: the centreline's name.
cpr-delete-title = “{ $name }” merkez çizgisi silinsin mi?
cpr-delete-detail = Noktaları silinir. Diske kaydedilen dosyalar kalır.
cpr-deleted = “{ $name }” silindi
cpr-load-failed = Eğri yüklenemedi: { $reason }
cpr-load-not-json = dosya JSON değil
cpr-load-not-curves = dosya bir Specular eğrisi değil
cpr-load-empty = dosyada iki veya daha fazla noktalı bir eğri yok
cpr-panoramic = Panoramik
cpr-panoramic-slab = Panoramik, { $thickness } mm kalınlık
cpr-panoramic-thin = Panoramik, ince

## The dental layout (Medical).
## machine-drafted, needs native review

dental-button = Dental
dental-not-for-diagnosis = Tanı veya tedavi planlaması için değildir
dental-nerve = Mandibular kanal
dental-nerve-hint = Panoramik görüntüde veya bir kesitte mandibular kanal boyunca noktalara tıklayın
dental-nerve-done = Mandibular kanal izleme kapalı
dental-spacing = Her { $spacing } mm’de bir
dental-section = Kesit { $number }
dental-off = Önce dental düzeni açın
dental-suggesting = Diş arkı aranıyor…
dental-suggested = Ark önerildi: ayarlamak için noktalarını sürükleyin
dental-no-arch = Ark bulunamadı: aksiyel görünümde ark boyunca noktalara tıklayın
dental-vendor = { $vendor } dental KIBT

## Each segment as its own STL.
## machine-drafted, needs native review

seg-stl-export-button = Segmentleri dışa aktar
seg-stl-detail = Segmentler: { $count } üçgen
seg-stl-detail-full = Segmentler: tam ayrıntı
seg-stl-none = Dışa aktarılacak segment yok
seg-stl-not-labels = bu hacim bir etiket haritası değil
seg-stl-exporting = Segmentlerin yüzeyleri oluşturuluyor…
seg-stl-saved = { $count ->
    [one] { $count } segment { $folder } klasörüne kaydedildi
   *[other] { $count } segment { $folder } klasörüne kaydedildi
}
seg-stl-open = Kapalı olmayan: { $count }.
seg-stl-empty = Segmentler boş: hiçbir şey kaydedilmedi
seg-stl-failed = Segmentler kaydedilemedi: { $reason }
# machine-drafted, needs native review
toolbar-mpr = Üç düzlem (MPR)
toolbar-mpr-close = Üç düzlemi kapat (MPR)
settings-viewing-toolbar-items-mpr = Üç düzlem (MPR)
