# Specular — Drawings (DXF): layers, paper, lineweights, model space and layouts. Türkçe.

keys-mode-drawing = Çizimler
cheatsheet-drawing = Çizimler
palette-drawing-layers = Katman panelini göster veya gizle
palette-drawing-all-layers = Tüm katmanları ya da kaydedildiği hâliyle katmanları göster
palette-drawing-paper = Koyu ve açık kâğıt arasında geçiş yap
palette-drawing-lineweights = Çizgi kalınlıklarını göster veya gizle
disabled-not-drawing = Yalnızca çizimler için (DXF)
# A drawing's page: model space (layouts are shown by their own name).
page-label-model = Model
# machine-drafted, needs native review
cad-pen-name = Kalem { $n }
cad-page-name = Sayfa { $n }
cad-layer-name = Katman { $n }
cad-map-name = Harita
# machine-drafted, needs native review
# A board's views (its Gerber files together): every layer stacked, and its top and bottom
# in board colours.
cad-board-layers-name = Katmanlar
cad-board-top-name = Üst yüz
cad-board-bottom-name = Alt yüz
# machine-drafted, needs native review
# A plot's layers: its frame, a trace without a name (numbered from 1), and the contour
# lines of a 2-D spectrum above and below zero.
cad-chart-frame-name = Çerçeve
cad-trace-name = Eğri { $n }
cad-contours-positive-name = Pozitif konturlar
cad-contours-negative-name = Negatif konturlar
# machine-drafted, needs native review
# The page of a survey file (LandXML) showing its surfaces and alignments in 3D.
cad-model-3d-name = 3B model
# The layers panel's title (and its name for screen readers).
drawing-layers-title = Katmanlar · { $total } katmandan { $shown } tanesi gösteriliyor
# Toasts.
drawing-layers-all-toast = Tüm katmanlar gösteriliyor
drawing-layers-saved-toast = Katmanlar çizimde kaydedildiği hâliyle
drawing-paper-dark-toast = Koyu kâğıt
drawing-paper-light-toast = Açık kâğıt
drawing-lineweights-on = Çizgi kalınlıkları gösteriliyor
drawing-lineweights-off = Çizgi kalınlıkları kapalı: ince çizgiler

# machine-drafted, needs native review
# Markup on drawings and images (Alt+M): kept in a file next to the drawing, never in it.
palette-markup = İşaretle
palette-markup-help = Bulutlar, metin, oklar, damgalar, ölçümler ve fotoğraf iğneleri; dosyanın yanına kaydedilir.
palette-compare-sheets = Paftaları karşılaştır
palette-compare-sheets-help = Bu paftanın başka bir revizyonunu hizalayarak bindirir, değişiklikleri çevreler.
markup-title = İşaretlemeler
markup-list = Bu sayfadaki işaretlemeler
markup-count = { $count ->
    [0] Bu sayfada işaretleme yok
    [one] Bu sayfada 1 öğe
   *[other] Bu sayfada { $count } öğe
}
markup-empty = Bir araç seçin ve paftaya tıklayın.
markup-item = { $n }. { $kind }
markup-tool-select = Seç
markup-tool-cloud = Bulut
markup-tool-text = Metin
markup-tool-arrow = Ok
markup-tool-stamp = Damga
markup-tool-length = Uzunluk
markup-tool-area = Alan
markup-tool-pin = Fotoğraf iğnesi
markup-hint-select = Seçmek için bir öğeye tıklayın. Taşımak için sürükleyin. Enter metnini düzenler.
markup-hint-cloud = Bir dikdörtgen sürükleyin veya köşelere tıklayıp Enter'a basın.
markup-hint-text = Metnin olacağı yere tıklayın.
markup-hint-arrow = Etiketten noktaya sürükleyin veya ikisine de tıklayın.
markup-hint-stamp = Damganın olacağı yere tıklayın.
markup-hint-length = Sürükleyin veya iki uca tıklayın.
markup-hint-area = Köşelere tıklayın, ardından Enter'a basın.
markup-hint-pin = Bir noktaya tıklayın, ardından bir fotoğraf seçin.
markup-kind-cloud = Bulut
markup-kind-text = Metin
markup-kind-arrow = Ok
markup-kind-stamp = Damga
markup-kind-length = Uzunluk
markup-kind-area = Alan
markup-kind-pin = Fotoğraf
markup-colour = Renk: { $colour }
markup-colour-red = Kırmızı
markup-colour-blue = Mavi
markup-colour-green = Yeşil
markup-colour-orange = Turuncu
markup-colour-magenta = Eflatun
markup-colour-black = Siyah
markup-width = Çizgi: { $width }
markup-width-thin = İnce
markup-width-medium = Orta
markup-width-thick = Kalın
markup-stamp = Damga: { $words }
markup-stamp-approved = Onaylandı
markup-stamp-rejected = Reddedildi
markup-stamp-reviewed = İncelendi
markup-stamp-custom = Kendi metniniz
markup-undo = Geri al
markup-redo = Yinele
markup-delete = Sil
markup-open-photo = Fotoğrafı aç
markup-from-measurements = Ölçümleri ekle
markup-from-measurements-done = { $count ->
    [one] 1 ölçüm işaretleme olarak eklendi
   *[other] { $count } ölçüm işaretleme olarak eklendi
}
markup-export-pdf = PDF olarak dışa aktar…
markup-export-png = PNG olarak dışa aktar…
markup-done = Bitti
markup-ask-text = Metin (yerleştirmek için Enter, iptal için Esc)
markup-ask-label = Etiket (isteğe bağlı; yerleştirmek için Enter)
markup-ask-stamp = Damga metni (yerleştirmek için Enter)
markup-ask-edit = Yeni metin (kaydetmek için Enter)
markup-added = Eklendi: { $kind }
markup-moved = Taşındı
markup-pin-hint = Fotoğrafını açmak için iğneye yeniden tıklayın veya Enter'a basın.
markup-deleted = Silindi
markup-undone = Geri alındı
markup-redone = Yinelendi
markup-delete-ask = { $name } silinsin mi?
markup-delete-detail = İşaretleme dosyasından kaldırılır. Çizimin kendisi değişmez.
markup-delete-ok = Sil
markup-photo-missing = { $name } artık yok.
markup-load-failed = İşaretlemeler okunamadı: { $reason }
markup-save-failed = İşaretlemeler kaydedilemedi: { $reason }
markup-sidecar-too-large = işaretleme dosyası çok büyük
markup-pdf-unreadable = PDF okunamadı
markup-exported = { $name } kaydedildi
markup-export-failed = Dışa aktarılamadı: { $reason }
markup-export-same-file = başka bir ad seçin; orijinalin üzerine asla yazılmaz
markup-export-encrypted = PDF şifreli. Bunun yerine PNG olarak dışa aktarın.
markup-export-damaged = PDF'nin yapısı bozuk. Bunun yerine PNG olarak dışa aktarın.
sheets-title = Paftaları karşılaştır
sheets-other = Diğer pafta…
sheets-other-named = Diğer: { $name }
sheets-page = Sayfa { $page }/{ $count }
sheets-auto = Hizala
sheets-pick = 2 nokta seç
sheets-overlay = Bindirme
sheets-difference = Fark
sheets-blend = Geçiş
sheets-blink = Yanıp sönme
sheets-slider = Diğer pafta { PERCENT($percent) }
sheets-clouds = { $count ->
    [0] Değişiklik yok
    [one] 1 değişikliği bulutla
   *[other] { $count } değişikliği bulutla
}
sheets-done = Bitti
sheets-choose = Diğer paftayı seçin.
sheets-reading = Pafta okunuyor…
sheets-aligning = Hizalanıyor…
sheets-aligned = Hizalandı: ölçek { $scale }, dönüş { $angle }°, sapma en fazla { $error } px. { $changes ->
    [0] Değişiklik bulunamadı.
    [one] 1 değişiklik.
   *[other] { $changes } değişiklik.
}
sheets-align-failed = Otomatik hizalanamadı. 2 nokta seçin.
sheets-pick-this = Nokta { $n }: bu paftada ona tıklayın.
sheets-pick-other = Nokta { $n }: diğer paftada aynı noktaya tıklayın.
sheets-pick-same = İki nokta aynı. Yeniden 2 nokta seçin.
sheets-read-failed = Pafta okunamadı: { $reason }
sheets-failed = Bu adım başarısız oldu. Yeniden deneyin veya başka bir pafta seçin.
sheets-legend-this = Yalnızca bu pafta
sheets-legend-other = Yalnızca diğer pafta
sheets-legend-both = Her ikisi
sheets-cloud-text = Değişti
sheets-clouds-added = { $count ->
    [0] Bulut eklenmedi
    [one] İşaretlemeye 1 bulut eklendi
   *[other] İşaretlemeye { $count } bulut eklendi
}
sheets-opened = Paftaları karşılaştır
sheets-closed = Karşılaştırma kapatıldı
markup-ask-new-line = Shift+Enter: yeni satır

## Settings › Drawings

# machine-drafted, needs native review
settings-section-drawings = Çizimler
settings-drawings-shx-folders = SHX yazı tipi klasörleri
settings-drawings-shx-folders-help = DXF ve DWG metinleri için SHX yazı tipleri içeren klasörler, ; ile ayrılmış. Önce çizimin kendi klasörüne bakılır.
# A folder in the SHX font folders setting that does not exist or is not a folder.
settings-error-shx-folder = Klasör değil: { $folder }
