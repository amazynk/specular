# The play queue: what plays next, repeat and shuffle, playlists (M3U, PLS, XSPF), the
# queue panel and the sound transport's buttons.

# machine-drafted, needs native review
cheatsheet-video-queue = Kuyruk, tekrar ve karıştırma
# machine-drafted, needs native review
palette-next-track = Sonraki öğe
# machine-drafted, needs native review
palette-next-track-help = Kuyruğun sonraki öğesini, kuyruk yoksa klasördeki aynı türden sonraki dosyayı çal.
# machine-drafted, needs native review
palette-prev-track = Önceki öğe
# machine-drafted, needs native review
palette-prev-track-help = Önceki öğeyi çal. 3 saniyeden sonra bu öğe baştan başlar.
# machine-drafted, needs native review
palette-cycle-repeat = Tekrarla
# machine-drafted, needs native review
palette-cycle-repeat-help = Tekrar kapalı, tümü veya bir.
# machine-drafted, needs native review
palette-shuffle = Karıştır
# machine-drafted, needs native review
palette-shuffle-help = Rastgele sırayla çal, her öğe turda bir kez.
# machine-drafted, needs native review
palette-play-queue = Çalma kuyruğu
# machine-drafted, needs native review
palette-play-queue-help = Sırada ne çalacağını göster: yeniden sırala, kaldır, kaydet.
# machine-drafted, needs native review
palette-queue-play-next = Sonra çal
# machine-drafted, needs native review
palette-queue-play-next-help = Dosyayı (galeride seçimi) çalan öğeden sonra çal.
# machine-drafted, needs native review
palette-queue-add = Kuyruğa ekle
# machine-drafted, needs native review
palette-queue-add-help = Dosyayı (galeride seçimi) kuyruğun sonuna ekle.
# machine-drafted, needs native review
palette-save-queue = Kuyruğu çalma listesi olarak kaydet…
# machine-drafted, needs native review
palette-save-queue-help = Kuyruğu M3U8 veya XSPF çalma listesi olarak kaydet.
# machine-drafted, needs native review
transport-prev-track = Önceki öğe
# machine-drafted, needs native review
transport-next-track = Sonraki öğe
# machine-drafted, needs native review
queue-title = Çalma kuyruğu
# machine-drafted, needs native review
queue-summary = { $count ->
    [one] 1 öğe
   *[other] { $count } öğe
}
# machine-drafted, needs native review
queue-summary-time = { $count ->
    [one] 1 öğe
   *[other] { $count } öğe
} · { $time }
# machine-drafted, needs native review
queue-empty = Kuyruk boş.
    Ses dosyalarını ve videoları Sonra çal veya Kuyruğa ekle ile ekleyin ya da bir çalma listesi açın.
# machine-drafted, needs native review
queue-empty-short = Kuyruk boş.
# machine-drafted, needs native review
queue-playing = Çalıyor
# machine-drafted, needs native review
queue-repeat = Tekrarla
# machine-drafted, needs native review
queue-repeat-off = Tekrar: kapalı
# machine-drafted, needs native review
queue-repeat-all = Tekrar: tümü
# machine-drafted, needs native review
queue-repeat-one = Tekrar: bir
# machine-drafted, needs native review
queue-shuffle = Karıştır
# machine-drafted, needs native review
queue-shuffle-on = Karıştır: açık
# machine-drafted, needs native review
queue-shuffle-off = Karıştır: kapalı
# machine-drafted, needs native review
queue-save = Çalma listesi olarak kaydet…
# machine-drafted, needs native review
queue-save-tip = Kuyruğu M3U8 veya XSPF çalma listesi olarak kaydet
# machine-drafted, needs native review
queue-clear = Temizle…
# machine-drafted, needs native review
queue-clear-tip = Kuyruğu temizle. Dosyalar diskte kalır.
# machine-drafted, needs native review
queue-remove = Kuyruktan kaldır (dosya diskte kalır)
# machine-drafted, needs native review
queue-network-item = Ağ öğesi, çalınmaz: { $address }
# machine-drafted, needs native review
queue-artist-title = { $artist } – { $title }
# machine-drafted, needs native review
queue-at-end = Sonraki öğe yok.
# machine-drafted, needs native review
queue-at-start = Önceki öğe yok.
# machine-drafted, needs native review
queue-missing = { $name } artık yok. Atlandı.
# machine-drafted, needs native review
queue-network-not-played = Ağ öğeleri henüz çalınmıyor.
# machine-drafted, needs native review
queue-nothing-to-add = Eklenecek bir şey yok. Kuyruğa yalnızca ses dosyaları, videolar ve çalma listeleri girer.
# machine-drafted, needs native review
queue-added = { $count ->
    [one] Kuyruğa 1 öğe eklendi.
   *[other] Kuyruğa { $count } öğe eklendi.
}
# machine-drafted, needs native review
queue-added-next = { $count ->
    [one] 1 öğe sonra çalacak.
   *[other] { $count } öğe sonra çalacak.
}
# machine-drafted, needs native review
queue-playlist-opened = { $name }: { $count ->
    [one] kuyrukta 1 öğe.
   *[other] kuyrukta { $count } öğe.
}
# machine-drafted, needs native review
queue-playlist-nothing = { $name } içinde burada çalınabilecek bir şey yok. Kuyruk içeriğini gösteriyor.
# machine-drafted, needs native review
queue-playlist-failed = { $name } açılamıyor: { $reason }
# machine-drafted, needs native review
queue-playlist-unreadable = dosya okunamıyor.
# machine-drafted, needs native review
queue-playlist-damaged = Specular’ın okuyabildiği bir çalma listesi değil.
# machine-drafted, needs native review
queue-clear-ask = { $count ->
    [one] Kuyruk temizlensin mi (1 öğe)?
   *[other] Kuyruk temizlensin mi ({ $count } öğe)?
}
# machine-drafted, needs native review
queue-clear-detail = Yalnızca liste temizlenir. Dosyalar diskte kalır.
# machine-drafted, needs native review
queue-clear-ok = Kuyruğu temizle
# machine-drafted, needs native review
queue-cleared = Kuyruk temizlendi.
# machine-drafted, needs native review
queue-save-title = Kuyruğu çalma listesi olarak kaydet
# machine-drafted, needs native review
queue-default-name = Kuyruk
# machine-drafted, needs native review
queue-format-m3u8 = M3U8 çalma listesi
# machine-drafted, needs native review
queue-format-xspf = XSPF çalma listesi
# machine-drafted, needs native review
queue-saved = { $name } kaydedildi.
# machine-drafted, needs native review
queue-save-failed = { $name } kaydedilemiyor: { $reason }
# machine-drafted, needs native review
settings-viewing-sound-play-next = Sonraki ses dosyasını çal
# machine-drafted, needs native review
settings-viewing-sound-play-next-help = Bir ses dosyası bitince klasördeki sonraki ses dosyası çalar.
# machine-drafted, needs native review
settings-viewing-play-repeat = Tekrarla
# machine-drafted, needs native review
settings-viewing-play-repeat-help = Ses dosyaları, videolar ve kuyruk için.
# machine-drafted, needs native review
settings-viewing-play-repeat-off = Kapalı
# machine-drafted, needs native review
settings-viewing-play-repeat-all = Tümü
# machine-drafted, needs native review
settings-viewing-play-repeat-one = Bir
# machine-drafted, needs native review
settings-viewing-play-shuffle = Karıştır
# machine-drafted, needs native review
settings-viewing-play-shuffle-help = Rastgele sırayla çal, her öğe turda bir kez.
