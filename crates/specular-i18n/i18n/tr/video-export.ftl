# Specular — Export slideshow as video (DESIGN.md §6.5). Turkish. Machine-drafted, needs native review.

palette-export-slideshow = Slayt gösterisini video olarak dışa aktar…

video-export-title = Slayt gösterisini video olarak dışa aktar
video-export-resolution = Çözünürlük
# $width and $height in pixels.
video-export-resolution-window = Pencere ({ $width } × { $height })
video-export-fps = Kare hızı
# $fps frames per second, a number.
video-export-fps-value = { $fps } fps
video-export-quality = Kalite
video-export-quality-high = Yüksek
video-export-quality-standard = Standart
video-export-quality-compact = Kompakt
video-export-images = { $count ->
    [one] Bir görüntü; slayt gösterisinin sırası, geçişi ve altyazılarıyla
   *[other] { $count } görüntü; slayt gösterisinin sırası, geçişi ve altyazılarıyla
}
# $length a duration ("2:40"), $size a file size ("38 MB").
video-export-summary = Yaklaşık { $length } uzunluğunda, yaklaşık { $size }
video-export-export = Dışa aktar…
video-export-replace = Değiştir
# $file a file name.
video-export-replace-question = { $file } zaten var. Değiştirilsin mi?
# $percent a formatted percentage ("42%").
video-export-progress = Dışa aktarılıyor… { $percent }
video-export-cancelling = İptal ediliyor…
# $file a file name, $size a file size ("38 MB").
video-export-saved = { $file } kaydedildi ({ $size })
video-export-skipped = { $count ->
    [one] Bir görüntü okunamadı ve dışarıda bırakıldı.
   *[other] { $count } görüntü okunamadı ve dışarıda bırakıldı.
}
video-export-failed = Video dışa aktarılamadı: { $reason }
video-export-cancelled = Dışa aktarma iptal edildi
video-export-no-images = Görüntülerin hiçbiri okunamadı.
video-export-show-in-folder = Klasörde göster

## Toolbar item and compatibility (machine-drafted, needs native review)
settings-viewing-toolbar-items-export-slideshow = Slayt gösterisini video olarak dışa aktar
video-export-compatibility = Uyumluluk
video-export-compatibility-current = Güncel oynatıcılar
video-export-compatibility-all = Tüm oynatıcılar
