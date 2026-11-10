# DICOM yapılandırılmış raporlar, dalga formları, RT dozu ve oftalmik tomografi. machine-drafted, needs native review

## Yapılandırılmış rapor içerik ağacı (rapor modunda etiket tarayıcısı)
sr-tree-title = { $title } ({ $kind })
sr-col-item = Öğe
sr-col-concept = Kavram
sr-col-type = Tür
sr-col-relationship = İlişki
sr-col-value = Değer
sr-show-image = Görüntüyü göster
sr-jump-missing = Başvurulan görüntü bu klasörde değil
sr-jump-frame = Bu referans çerçevesindeki hiçbir görüntü o noktayı içermiyor

## Grafikler (doz hacim histogramları, EKG kâğıtları)
plot-caliper-time = { $ms } ms · { $rate } /dk
plot-caliper-time-zero = 0 ms
plot-caliper-hint = Ölçmek için bir grafik üzerinde sürükleyin
plot-adjust-hint = + ve − kazancı, [ ve ] hızı değiştirir

## EKG kâğıdı
ecg-paper = { $speed } mm/s · { $gain } mm/mV · { $rate } Hz
ecg-groups-more = { $count ->
    [one] Bir kanal grubu daha gösterilmiyor
   *[other] { $count } kanal grubu daha gösterilmiyor
}

## MPR üzerinde RT dozu (izodoz çizgileri, renk haritası, gösterge, okuma)
dose-value = Doz { $value } { $unit } ({ PERCENT($percent) })
dose-hidden = Doz gizli (Shift+U gösterir)
dose-on-rx = Doz: { $dose } { $unit } reçete edilen · maks. { $max } { $unit }
dose-on-max = Doz: maksimumun yüzdesi, { $max } { $unit }
dose-colour = Doz { $colormap } ile, { PERCENT($blend) }
dose-dvh-hint = { $count ->
    [one] Shift+K: bir yapının doz hacim histogramı
   *[other] Shift+K: { $count } yapının doz hacim histogramları
}
dose-level = { PERCENT($percent) } · { $dose } { $unit }
dose-mode-lines = Doz izodoz çizgileri olarak
dose-mode-colour = Doz renk haritası olarak

## Doz hacim histogramları (grafik sayfası)
dvh-title = Doz hacim histogramları
dvh-col-roi = Yapı
dvh-col-volume = Hacim
dvh-axis-dose = Doz ({ $unit })
dvh-axis-volume = Hacim (%)
dvh-prescription = Reçete { $dose } Gy · kümülatif
dvh-max = Maksimum doz { $dose } { $unit } · kümülatif
dvh-none = Bu referans çerçevesinde dozun histogramının çıkarılacağı bir yapı seti yok

## Oftalmik tomografi (B-tarama kaydırıcısı; köşedeki konum belirleme görüntüsü)
oct-bscan = B-tarama
oct-depth = { $depth } µm derinlikte
