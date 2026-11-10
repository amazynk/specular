# machine-drafted, needs native review
# Specular — Maps (the Maps profile): the map view of georeferenced and vector map files, its
# layer panel, attribute table, coordinates, measuring, elevation profile and map furniture.
# Türkçe.

keys-mode-map = Haritalar
# Cheat sheet groups of the Maps tab.
cheatsheet-map-layers = Katmanlar
cheatsheet-map-coordinates = Koordinatlar
cheatsheet-map-measure = Ölçüm

## Palette entries and their one-line descriptions.
palette-map-layers = Katman panelini göster veya gizle
palette-map-layers-help = Haritanın katmanları: sıra, opaklık, görünürlük ve stil.
palette-map-add-layer = Dosyadan katman ekle…
palette-map-add-layer-help = Bunun üzerine başka bir harita dosyası çizin, ör. bir shapefile üzerine GPX izi.
palette-map-zoom-to-layer = Katmana yakınlaştır
palette-map-zoom-to-layer-help = Katman panelinde seçilen katmanın tamamını gösterir.
palette-map-attribute-table = Öznitelik tablosunu göster veya gizle
palette-map-attribute-table-help = Seçilen katmanın objeleri tablo olarak: sıralama, süzme, arama ve seçme.
palette-map-coordinate-format = Sonraki koordinat biçimi
palette-map-coordinate-format-help = Ondalık derece, derece dakika saniye, UTM veya MGRS.
palette-map-units = Metrik veya İngiliz birimleri
palette-map-units-help = Uzunluklar ve alanlar metre ya da fit ve mil olarak.
palette-map-copy-coordinate = Koordinatı kopyala
palette-map-copy-coordinate-help = İşaretçinin altındaki konumu ya da harita merkezini kopyalar.
palette-map-go-to-coordinate = Koordinata git…
palette-map-go-to-coordinate-help = Haritayı ortalamak için herhangi bir biçimde konum yazın.
palette-map-measure-distance = Mesafe ölç
palette-map-measure-distance-help = Haritada noktalara tıklayın: Dünya yüzeyindeki uzunluk ve yön açısı.
palette-map-measure-area = Alan ölç
palette-map-measure-area-help = Bir alanın köşelerine tıklayın: Dünya yüzeyindeki büyüklüğü ve çevresi.
palette-map-elevation-profile = Yükseklik profili
palette-map-elevation-profile-help = Yüksekliklerin grafiği için bir yükseklik modeli üzerine çizgi çizin.
palette-map-overview = Genel bakış haritasını göster veya gizle
palette-map-overview-help = Tüm kapsamın küçük bir haritası; ekrandaki kısım işaretli.

## Why a map command is unavailable.
disabled-not-map = Yalnızca haritalar için, Harita araçları açıkken
disabled-no-lonlat = Haritanın bilinen bir koordinat sistemi yok
disabled-no-features = Haritada obje yok
disabled-no-heights = Haritada yükseklik yok

## Settings › Maps.
settings-section-maps = Haritalar
settings-maps-coordinate-format = Koordinat biçimi
settings-maps-coordinate-format-help = Konumların nasıl gösterilip kopyalandığı. Koordinata git her biçimi okur.
settings-maps-coordinate-format-dd = Ondalık derece
settings-maps-coordinate-format-dms = Derece, dakika, saniye
settings-maps-coordinate-format-utm = UTM
settings-maps-coordinate-format-mgrs = MGRS / USNG
settings-maps-units = Birimler
settings-maps-units-help = Haritalardaki uzunluklar ve alanlar: ölçek çubuğu ve ölçümler. Otomatik, Meta veriler › Birimler ayarını izler.
settings-maps-units-auto = Otomatik
settings-maps-units-metric = Metrik
settings-maps-units-imperial = İngiliz

## The pointer readout. $position is written in the chosen format.
map-position = { $format ->
    [utm] UTM { $position }
    [mgrs] MGRS { $position }
    [dms] { $position }
   *[dd] Enlem, boylam { $position }
}
map-height = Yükseklik { $height }
map-height-label = Yükseklik
# A length or area with its unit symbol ("12.3 km", "4.5 ha").
map-length = { $value } { $unit }
map-scale-label = Ölçek çubuğu: { $length }
map-north-label = Kuzey oku: kuzey yukarıdan saat yönünde { $degrees }°
# The coordinate formats, for the toast after "Next coordinate format".
map-format-name = { $format ->
    [dms] derece, dakika, saniye
    [utm] UTM
    [mgrs] MGRS
   *[dd] ondalık derece
}
map-format-toast = Koordinatlar: { $format }
map-units-metric = Birimler: metre ve kilometre
map-units-imperial = Birimler: fit ve mil
toast-copied-coordinate = Koordinat kopyalandı

## Go to coordinate.
map-goto-hint = Ondalık derece, DMS, UTM veya MGRS. Enter oraya gider.
map-goto-empty = Bir konum yazın.
map-goto-unrecognised = Bilinen bir biçimde konum değil.
map-goto-range = Enlem veya boylam aralık dışında.
map-goto-no-way = Bu harita bir konumu yerleştiremiyor.
map-goto-outside = Bu konum haritanın dışında.
map-goto-toast = { $position } ortalandı

## The layer panel.
map-layers-title = Katmanlar: { $total } katmandan { $shown } tanesi gösteriliyor
# Under a layer's name: its kind, features and opacity.
map-layer-sub = { $kind } · { $features } · { PERCENT($opacity) }
# A layer's row for screen readers.
map-layer-row = { $name }: { $kind }, { $features ->
   *[other] { $features } obje
}, { PERCENT($opacity) } opak
map-kind-area = Alanlar
map-kind-line = Çizgiler
map-kind-point = Noktalar
map-kind-empty = Obje yok
map-layer-visible = Katmanı göster veya gizle
map-layer-opacity-down = Daha az opak
map-layer-opacity-up = Daha opak
map-layer-colour = Sonraki renk
map-layer-width = Sonraki çizgi kalınlığı
map-layer-point = Sonraki nokta boyutu
map-layer-up = Yukarı taşı
map-layer-down = Aşağı taşı
map-layer-remove = Kaldır
map-layer-keep = Tut
map-layer-remove-ask = { $name } haritadan kaldırılsın mı? Dosya diskte kalır.
map-layer-adding = Katman ekleniyor…
map-removed = { $name } haritadan kaldırıldı
map-added = { $layers ->
   *[other] { $name } eklendi: { $layers } katman
}
map-add-failed = { $name } eklenemiyor: { $reason }
map-add-filter = Harita dosyaları
map-add-not-map = bir harita değil.
map-add-not-vector = objesi yok.
map-add-no-crs = projeksiyon dosyası (.prj) yok.

## Identify.
map-identify-more = ve öznitelik tablosunda { $count } tane daha
map-cell = Hücre { $x }, { $y }
map-band = Bant { $n }
map-no-data = Veri yok

## The attribute table.
map-table-title = { $layer }: { $total } objeden { $shown } tanesi, { $selected } seçili
map-table-search = Ara veya alan = değer
map-table-selected-only = Yalnızca seçililer
map-table-empty = Eşleşen obje yok.
map-table-many = çok
map-table-stats-number = { $field }: { $count } değer, { $empty } boş · en az { $min } · en çok { $max } · ortalama { $mean } · toplam { $sum } · std. sapma { $sd }
map-table-stats-text = { $field }: { $count } değer, { $empty } boş · { $distinct } farklı
map-table-stats-text-top = { $field }: { $count } değer, { $empty } boş · { $distinct } farklı · en sık { $top } ({ $times }×)

## Measuring and the elevation profile.
map-measure-hint-distance = Yol boyunca noktalara tıklayın.
map-measure-hint-area = Alanın köşelerine tıklayın.
map-measure-hint-profile = Çizgi boyunca noktalara tıklayın.
map-measure-keys = Enter bitirir. Backspace son noktayı siler. Esc kapatır.
map-measure-distance = Mesafe { $length }
map-measure-bearing = Yön açısı { $bearing }
map-measure-segment = Son parça { $length }, yön açısı { $bearing }
map-measure-area = Alan { $area }
map-measure-perimeter = Çevre { $length }
map-bearing-value = { $degrees }°
map-profile-summary = Uzunluk { $length } · en düşük { $min } · en yüksek { $max } · tırmanış { $climb } · iniş { $descent }
map-profile-no-heights = Uzunluk { $length }. Çizgi boyunca yükseklik yok.
map-profile-at = { $distance }: { $height }
map-profile-at-none = { $distance }: yükseklik yok
