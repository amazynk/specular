# machine-drafted, needs native review
# Specular — Arka plan işleri: arka planda bir şey çalışırken gösterilen gösterge ve açtığı panel. Türkçe.

## Panel

work-title = Arka plan işleri
work-show = Arka plan işlerini göster
work-empty = Arka planda çalışan bir şey yok.

## İşler

work-kind-library = Fotoğraf kitaplığı
work-kind-ai-search = Yapay zekâ araması
work-kind-faces = Kişiler
work-kind-text = Fotoğraflardaki metin
work-kind-culling = Ayıklama puanları
work-kind-download = Yapay zekâ modeli indirme
work-kind-similar = Benzer çekimler

## Adımlar

work-step-finding = Fotoğraflar bulunuyor
work-step-reading = Fotoğraf ayrıntıları okunuyor
work-step-signatures = Fotoğraflar karşılaştırılıyor
work-step-grouping = Benzer çekimler gruplanıyor
work-step-text-index = Arama dizini oluşturuluyor
work-step-embedding = Fotoğraflar yapay zekâ araması için dizinleniyor
work-step-switching = Yeni model boyutu için yeniden dizinleniyor
work-step-faces = Yüzler bulunuyor
work-step-ocr = Fotoğraflardaki metin okunuyor
work-step-scoring = Netlik ve pozlama puanlanıyor
work-step-downloading = İndiriliyor
work-step-focus = Netlik denetleniyor
work-step-item = { $step } · { $item }

## İlerleme

work-count-photos = { $done } / { $total } { $count ->
    [one] fotoğraf
   *[other] fotoğraf
}
work-count = { $done } / { $total }
work-count-left = { $count } · { $left } kaldı
work-so-far-photos = Şu ana kadar { $done } { $count ->
    [one] fotoğraf
   *[other] fotoğraf
}
work-so-far = Şu ana kadar { $done }

## Kalan süre

work-eta-estimating = Tahmin ediliyor…
work-eta-seconds = Yaklaşık { $n ->
    [one] { $n } saniye
   *[other] { $n } saniye
} kaldı
work-eta-minutes = Yaklaşık { $n ->
    [one] { $n } dakika
   *[other] { $n } dakika
} kaldı
work-eta-hours = Yaklaşık { $h ->
    [one] { $h } saat
   *[other] { $h } saat
} kaldı
work-eta-hours-minutes = Yaklaşık { $h ->
    [one] { $h } saat
   *[other] { $h } saat
} { $m ->
    [one] { $m } dakika
   *[other] { $m } dakika
} kaldı

## Bir iş neden bekliyor

work-paused-browsing = Siz göz atarken duraklatıldı
work-paused-battery = Pil gücündeyken duraklatıldı
work-paused-low-power = Güç tasarrufu modunda duraklatıldı
work-paused-heat = Bilgisayar ısınmışken duraklatıldı
work-paused-memory = Bellek azken duraklatıldı
work-paused-user = Duraklatıldı

## Sırada ne var

work-then = Ardından: { $steps }

## Düğmeler

work-pause = Duraklat
work-resume = Sürdür
work-settings = Ayarlar…

## Gösterge

work-chip = { $job } · { $done } / { $total }
work-chip-so-far = { $job } · { $done }
work-chip-paused = { $job } · { $reason }
work-chip-more = { $chip } (+{ $more })
work-chip-name = Arka plan işleri: { $status }

## Duyurular

work-started = { $job } arka planda başladı
work-finished = { $job } tamamlandı
