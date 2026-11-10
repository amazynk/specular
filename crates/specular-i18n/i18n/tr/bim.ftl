# machine-drafted, needs native review
## 3B modelleri inceleme: yürüme ve uçma, kesit düzlemleri ve kesit kutusu, kayıtlı görünümler,
## BCF konuları (3B kipi). Anılan tuşlar varsayılan tuşlardır.

cheatsheet-review = İnceleme

palette-mesh-walk = Yürü
palette-mesh-fly = Uç
palette-mesh-views = Kayıtlı görünümler
palette-mesh-save-view = Görünümü kaydet
palette-mesh-section-plane = Kesit düzlemi ekle
palette-mesh-section-box = Kesit kutusu
palette-mesh-section-flip = Kesit düzlemini ters çevir
palette-mesh-section-clear = Tüm kesitleri kaldır
palette-mesh-measure-cut = Kesit üzerinde ölç
palette-mesh-bcf-topics = BCF konuları
# machine-drafted, needs native review
palette-mesh-model-checks = Model denetimleri
# machine-drafted, needs native review
palette-mesh-models = Sahnedeki modeller
# machine-drafted, needs native review
palette-mesh-add-model = Model ekle…
# machine-drafted, needs native review
palette-mesh-add-model-folder = Klasörden model ekle…

# Yürüme ve uçma: bildirimler ve gösterge satırları, $what değerine göre. $eye: gözün zeminden
# yüksekliği, birimiyle (yalnızca walk-line).
mesh-nav = { $what ->
    [walk-toast] Yürüme: W A S D ile hareket, sürükleyerek bakın, Esc ile durun
    [fly-toast] Uçma: W A S D ile hareket, Q E aşağı ve yukarı, Esc ile durun
    [orbit] Yörünge
    [walk-line] Yürüme · göz zeminden { $eye } yukarıda
    [walk-keys] W A S D hareket · sürükle bakar · Shift daha hızlı · Esc durdurur
    [preparing] Duvarlar henüz hazır değil: içlerinden geçersiniz
    [fly-keys] W A S D hareket · Q E aşağı, yukarı · sürükle bakar · Shift daha hızlı · Esc durdurur
   *[fly-line] Uçma
}

# Kesitler, ölçme ve kayıtlı görünümler için kısa metinler, $what değerine göre.
bim-note = { $what ->
    [plane-added] Kesit düzlemi eklendi: taşımak için tutamacını sürükleyin
    [box-on] Kesit kutusu açık: bir yüzü taşımak için tutamacını sürükleyin
    [box-off] Kesit kutusu kapalı
    [cleared] Kesitler kaldırıldı
    [measure-on] Kesit üzerinde iki noktaya tıklayın
    [measure-needs-cut] Önce bir kesit düzlemi veya kutu ekleyin
    [measure-off-cut] Kesitin üzerine tıklayın
    [measure-hint] Ölçme: kesit üzerinde iki noktaya tıklayın
    [ask-view-name] Görünümün adı
    [ask-rename] Yeni ad
    [views-empty] Kayıtlı görünüm yok. Bunu kaydetmek için N’ye basın.
    [views-hints] Enter git · N kaydet · F2 yeniden adlandır · Delete kaldır · P PNG olarak kaydet
    [delete-detail] Yalnızca kayıtlı görünüm kaldırılır. Model değişmez.
    [delete-ok] Kaldır
    [not-over-model] Başka bir dosya seçin: bu, modelin yerini alırdı.
   *[view-name] Görünüm
}

# BCF (BIM Collaboration Format) kısa metinleri, $what değerine göre.
bcf-note = { $what ->
    [title-none] BCF konuları
    [new-file] yeni
    [empty] Konu yok. BCF dosyası açmak için O’ya, konu eklemek için N’ye basın.
    [hints] Enter git · ← → bakış noktaları · N konu · C yorum · S durum · E başlık · O aç · Ctrl+S dışa aktar
    [ask-title] Konunun başlığı
    [ask-comment] Yorum
    [untitled] Başlıksız
    [topic-added] Konu bu görünümle eklendi
   *[comment-added] Yorum bu görünümle eklendi
}

# $planes: kaç kesit düzlemi.
bim-sections-line = { $planes ->
    [0] Kesit düzlemi yok
    [one] 1 kesit düzlemi
   *[other] { $planes } kesit düzlemi
}
# $size: kesit kutusunun boyutu, birimiyle.
bim-box-line = kutu { $size }
# $count: olabilecek en fazla düzlem sayısı.
bim-planes-full = En fazla { $count } kesit düzlemi
# Ekran okuyucular için bir sürükleme tutamacı. $n: numarası.
bim-handle = { $kind ->
    [box] Kesit kutusu yüzü { $n }
   *[plane] Kesit düzlemi { $n }
}
# $length: birimiyle uzunluk.
bim-measure-line = Kesit üzerindeki uzunluk: { $length }
bim-views-title = { $count ->
    [one] 1 kayıtlı görünüm
   *[other] { $count } kayıtlı görünüm
}
# $name: bir görünümün adı.
bim-view-saved = “{ $name }” kaydedildi
bim-view-delete-title = “{ $name }” görünümü kaldırılsın mı?
# $file: yazılan bir dosya.
bim-saved-file = { $file } kaydedildi
# $reason: neden başarısız olduğu.
bim-failed = { $what ->
    [open] BCF dosyası açılamadı: { $reason }
    [views] Görünümler yalnızca bu oturum için tutuldu: { $reason }
   *[save] Kaydedilemedi: { $reason }
}
# $count: konular; $file: arşivin dosya adı.
bcf-title = { $count ->
    [one] 1 konu
   *[other] { $count } konu
} · { $file }
bcf-comments = { $count ->
    [one] 1 yorum
   *[other] { $count } yorum
}
# $at: gösterilen bakış noktası; $count: konunun bakış noktaları.
bcf-viewpoint-of = Bakış noktası { $at } / { $count }
bcf-opened = { $count ->
    [one] 1 konu açıldı
   *[other] { $count } konu açıldı
}

## Several IFC models in one scene (the Models list, Alt+Shift+M).

# The list's title. $count: models in the scene.
# machine-drafted, needs native review
fed-title = { $count ->
   *[other] Modeller: { $count }
}
# machine-drafted, needs native review
fed-note = { $what ->
    [not-ifc] Bu dosya bir IFC modeli değil.
    [not-ifc-open] Önce bir IFC modeli açın. Yalnızca IFC modelleri birleştirilebilir.
    [this-model] Bu model
    [hints] A model ekle · Shift+A klasör ekle · Space göster veya gizle · C modele göre renklendir · Enter git · Delete kaldır
    [origin-ok] Orijine yerleştir
    [first-stays] Açtığınız model kalır. Değiştirmek için başka bir dosya açın.
    [remove-detail] Sahneden çıkar. Diskteki dosya yerinde kalır.
   *[remove-ok] Kaldır
}
# A model's row. $basis: how it was placed; $elements: its elements; $shown: shown or hidden.
# machine-drafted, needs native review
fed-row = { $basis } · { $elements ->
   *[other] { $elements } eleman
} · { $shown ->
    [hidden] gizli
   *[shown] görünür
}
# How a model was placed against the first. $crs: the coordinate system's name.
# machine-drafted, needs native review
fed-basis = { $basis ->
    [reference] açılan model
    [map-crs] harita koordinatlarına göre ({ $crs })
    [map] harita koordinatlarına göre
    [site] arsanın enlem ve boylamına göre (yaklaşık)
    [project] ortak proje koordinatlarına göre
   *[origin] orijinde (hizalanmamış)
}
# Why a model cannot be aligned. $first, $this: coordinate system names.
# machine-drafted, needs native review
fed-unaligned = { $why ->
    [crs] Modeller farklı koordinat sistemleri kullanıyor: { $first } ve { $this }.
    [only-first] Açılan model coğrafi referanslı, bu model değil.
    [only-this] Bu model coğrafi referanslı, açılan model değil.
   *[broken] Bu modelin harita dönüşümü kullanılamıyor (ölçeği sıfır).
}
# The chosen model's placement. $distance: how far it moved; $angle: degrees turned.
# machine-drafted, needs native review
fed-placed = { $distance } taşındı, { $angle }° döndürüldü
# machine-drafted, needs native review
fed-adding = { $count ->
   *[other] { $count } model okunuyor…
}
# machine-drafted, needs native review
fed-ask-origin = { $file } hizalanamıyor. Orijine yerleştirilsin mi?
# machine-drafted, needs native review
fed-no-files = { $folder } içinde IFC dosyası yok.
# machine-drafted, needs native review
fed-not-added = { $file } eklenmedi: { $reason }
# machine-drafted, needs native review
fed-added = { $file } eklendi. Sahnedeki model sayısı: { $count }.
# machine-drafted, needs native review
fed-remove-title = { $file } sahneden kaldırılsın mı?
# machine-drafted, needs native review
fed-removed = { $file } sahneden kaldırıldı.
# machine-drafted, needs native review
fed-shown = { $file } { $shown ->
    [hidden] gizlendi
   *[shown] gösteriliyor
}
# machine-drafted, needs native review
fed-colours = { $on ->
    [on] Modele göre renklendirme açık
   *[off] Modele göre renklendirme kapalı
}

## PMI (product manufacturing information) of a CAD model: dimensions, geometric tolerances,
## datums and the file's saved views (mesh mode).

# machine-drafted, needs native review
palette-mesh-pmi = PMI ve kayıtlı görünümler
# $count: annotations in the model.
# machine-drafted, needs native review
pmi-title = { $count ->
   *[other] { $count } açıklama
}
# machine-drafted, needs native review
pmi-view = Kayıtlı görünüm
# machine-drafted, needs native review
pmi-view-all = tüm açıklamaları gösterir
# $count: annotations the saved view shows.
# machine-drafted, needs native review
pmi-view-shows = { $count ->
   *[other] { $count } açıklama gösterir
}
# $name: the saved view's name.
# machine-drafted, needs native review
pmi-view-shown = “{ $name }” görünümü gösteriliyor. A tümünü gösterir.
# machine-drafted, needs native review
pmi-has-faces = yüzler vurgulandı
# Short texts, by $what.
# machine-drafted, needs native review
pmi-note = { $what ->
    [none] Bu modelde PMI yok.
    [hints] ↑ ↓ seç · Enter git · A tümü · D çizildiği gibi · Shift+A gizle
    [hidden] Açıklamalar gizlendi
    [shown] Açıklamalar gösteriliyor
    [as-drawn] Açıklamalar dosyanın çizdiği gibi
    [as-labels] Açıklamalar etiket olarak
   *[all-shown] Tüm açıklamalar gösteriliyor
}
# What an annotation is, by $what.
# machine-drafted, needs native review
pmi-kind = { $what ->
    [dimension] Ölçü
    [tolerance] Geometrik tolerans
    [datum] Referans
    [target] Referans hedefi
   *[graphic] Çizilmiş açıklama
}
# machine-drafted, needs native review
pmi-dimension = { $what ->
    [linear] Mesafe
    [angular] Açı
    [diameter] Çap
    [radius] Yarıçap
    [length] Uzunluk
    [thickness] Kalınlık
   *[other] Ölçü
}
# Geometric characteristics (ISO 1101), by $what.
# machine-drafted, needs native review
pmi-characteristic = { $what ->
    [straightness] Doğrusallık
    [flatness] Düzlemsellik
    [circularity] Dairesellik
    [cylindricity] Silindiriklik
    [line-profile] Çizgi profili
    [surface-profile] Yüzey profili
    [angularity] Açısallık
    [perpendicularity] Diklik
    [parallelism] Paralellik
    [position] Konum
    [concentricity] Eş merkezlilik
    [coaxiality] Eş eksenlilik
    [symmetry] Simetri
    [circular-runout] Dairesel salgı
   *[total-runout] Toplam salgı
}
