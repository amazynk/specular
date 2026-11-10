# Linked views: several DICOM series side by side, scrolling together by position, with
# reference lines, a 3D cursor, window and zoom sync, and manual alignment of priors.

# machine-drafted, needs native review
palette-compare-series = Serileri karşılaştır (bağlı görünümler)
# machine-drafted, needs native review
palette-compare-layout = Sonraki görünüm düzeni
# machine-drafted, needs native review
palette-next-viewport = Sonraki görünüm
# machine-drafted, needs native review
palette-prev-viewport = Önceki görünüm
# machine-drafted, needs native review
palette-reference-lines = Referans çizgilerini göster veya gizle
# machine-drafted, needs native review
palette-sync-window = Sonraki pencere senkronu
# machine-drafted, needs native review
palette-sync-zoom = Boyuta göre birlikte yakınlaştır ve kaydır
# machine-drafted, needs native review
palette-link-series = Sonraki seri bağlantısı
# machine-drafted, needs native review
palette-align-series = Diğer serileri buraya hizala
# machine-drafted, needs native review
palette-compare-series-help = Birkaç seriyi yan yana, birlikte kayarak göster.
# machine-drafted, needs native review
palette-compare-layout-help = Daha fazla veya daha az görünüm göster.
# machine-drafted, needs native review
palette-next-viewport-help = Sonraki görünümü etkin yap.
# machine-drafted, needs native review
palette-prev-viewport-help = Önceki görünümü etkin yap.
# machine-drafted, needs native review
palette-reference-lines-help = Etkin kesitin diğer görünümleri nerede kestiğini çiz.
# machine-drafted, needs native review
palette-sync-window-help = Hangi görünümlerin pencere/seviye değişikliğini paylaşacağını seç: kapalı, aynı modalite veya tümü.
# machine-drafted, needs native review
palette-sync-zoom-help = Tüm görünümleri aynı boyutta göster ve birlikte kaydır.
# machine-drafted, needs native review
palette-link-series-help = Bu serinin diğerlerini nasıl izleyeceğini seç.
# machine-drafted, needs native review
palette-align-series-help = Önceki bir seriyi gösterilen kesitlerde bununla hizala.
# machine-drafted, needs native review
disabled-not-comparing = Önce serileri karşılaştırın

# The bar above the viewports. Toggles show their state by colour; the others name it.
# machine-drafted, needs native review
linked-layout = Düzen: { $cols } × { $rows }
# machine-drafted, needs native review
linked-lines = Referans çizgileri
# machine-drafted, needs native review
linked-zoom = Yakınlaştırma eşzamanlaması
# machine-drafted, needs native review
linked-window-sync = Pencere eşzamanlaması: { $mode ->
    [off] kapalı
    [modality] aynı modalite
   *[all] tümü
}
# machine-drafted, needs native review
linked-link = Bağlantı: { $mode ->
    [frame] referans çerçevesi
    [position] hasta pozisyonu
   *[off] kapalı
}
# machine-drafted, needs native review
linked-align = Buraya hizala

# One viewport. $active is "yes" for the active one.
# machine-drafted, needs native review
linked-viewport = { $active ->
    [yes] Görünüm { $n }/{ $count }, etkin
   *[no] Görünüm { $n }/{ $count }
}
# machine-drafted, needs native review
linked-empty = Buraya bir seri sürükleyin
# machine-drafted, needs native review
linked-series = { $number } · { $modality } · { $description }
# machine-drafted, needs native review
linked-slice = Kesit { $n }/{ $count } · { $position } mm
# machine-drafted, needs native review
linked-in-gap = Boşlukta: en yakın kesit { $distance } mm uzakta
# machine-drafted, needs native review
linked-outside = Bu serinin dışında ({ $distance } mm)
# machine-drafted, needs native review
linked-not-linked = Bağlı değil
# machine-drafted, needs native review
linked-aligned = Elle hizalandı ({ $offset } mm)
# machine-drafted, needs native review
linked-cursor = 3D imleç { $x }, { $y }, { $z } mm konumunda
# machine-drafted, needs native review
linked-chip = { $label }, { $count } görüntü

# Toasts and announcements.
# machine-drafted, needs native review
linked-opened = { $count ->
    [one] Burada yalnızca bir seri var: Tetkik tarayıcısından bir tane daha sürükleyin
   *[other] { $count } seri karşılaştırılıyor
}
# machine-drafted, needs native review
linked-no-geometry = Bu serinin kesit konumları yok: bağlanamaz
# machine-drafted, needs native review
linked-active = Görünüm { $n }: { $series }
# machine-drafted, needs native review
linked-aligned-count = { $count ->
    [0] Hizalanacak bir şey yok: diğer seriler bu referans çerçevesini paylaşıyor
    [one] 1 seri etkin seriyle hizalandı, bu oturum için
   *[other] { $count } seri etkin seriyle hizalandı, bu oturum için
}
