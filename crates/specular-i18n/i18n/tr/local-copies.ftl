# machine-drafted, needs native review
# Local copies for offline editing (the Library).

settings-library-local-copies-limit = Yerel kopya sınırı
settings-library-local-copies-limit-help = Yerel kopyaların Specular’ın veri klasöründe kaplayabileceği en çok alan.

local-keep-menu = Yerel kopya tut…
local-remove-menu = Yerel kopyayı kaldır…
local-keep-location-menu = Bu konumun yerel kopyasını tut…
local-remove-location-menu = Bu konumun yerel kopyasını kaldır…
local-keep-album-menu = Bu albümün yerel kopyasını tut…
local-remove-album-menu = Bu albümün yerel kopyasını kaldır…
local-remove-held-elsewhere = Bu kopyalar tutulan bir konuma ya da albüme ait. Onları orada kaldırın.

local-keep-ask = { $item } için yerel kopya tutulsun mu?
local-keep-detail = { $count ->
    [one] 1 fotoğraf.
   *[other] { $count } fotoğraf.
}
    Özgünler: dosyalar olduğu gibi.
    Proxy’ler: 2.560 piksel ham dosyalar, geliştirmeye yeter. Diğer dosyalar tümüyle kopyalanır.
    Kopyalar Specular’ın veri klasörüne gider. Özgünler değişmez.
local-keep-originals = Özgünleri tut
local-keep-proxies = Proxy’leri tut
local-keeping = { $item } kopyalanıyor…
local-kept = { $item }: { $count ->
    [one] 1 yerel kopya
   *[other] { $count } yerel kopya
}, { $size }.
local-kept-full = { $count ->
    [one] 1 fotoğraf boyut sınırına sığmadı.
   *[other] { $count } fotoğraf boyut sınırına sığmadı.
}
local-kept-unreachable = { $count ->
    [one] 1 fotoğraf okunamadı.
   *[other] { $count } fotoğraf okunamadı.
}

local-remove-ask = { $item } için yerel kopya kaldırılsın mı?
local-remove-detail = Specular’ın veri klasöründeki kopyalar silinir. Özgünlere dokunulmaz. Bekleyen değişiklikler korunur.
local-remove-ok = Yerel kopyayı kaldır
local-removed = { $item } yerel kopyası kaldırıldı, { $size } boşaldı.

local-banner-offline = { $location } çevrimdışı. Yerel kopyalar üzerinde çalışıyorsunuz.
local-banner-offline-many = { $count } konum çevrimdışı. Yerel kopyalar üzerinde çalışıyorsunuz.
local-banner-waiting = { $count ->
    [one] 1 değişiklik özgünü bekliyor.
   *[other] { $count } değişiklik özgünleri bekliyor.
}
local-banner-conflicts = { $count ->
    [one] Siz çevrimdışıyken 1 dosya değişti.
   *[other] Siz çevrimdışıyken { $count } dosya değişti.
}
local-review = Gözden geçir…
local-sidebar-originals = yerel kopya
local-sidebar-proxies = yerel proxy’ler

local-synced = { $count ->
    [one] 1 değişiklik özgününe yazıldı.
   *[other] { $count } değişiklik özgünlerine yazıldı.
}
local-sync-failed = { $count ->
    [one] 1 değişiklik yazılamadı. Beklemede kalıyor.
   *[other] { $count } değişiklik yazılamadı. Beklemede kalıyorlar.
}

local-conflict-title = Siz çevrimdışıyken { $file } değişti. Hangi sürüm tutulsun: { $what }?
local-conflict-yours = Sizinki: { $text }
local-conflict-theirs = Diskte: { $text }
local-conflict-later = İptal, kararı sonraya bırakır. Hiçbir şeyin üzerine yazılmaz.
local-keep-mine = Sizinkini tut
local-keep-theirs = Dosyanınkini tut
local-what-marks = puan
local-what-keywords = anahtar sözcükler
local-what-fields = IPTC alanları
local-what-edit = düzenleme
local-stars = { $count ->
    [0] Yıldız yok
    [one] 1 yıldız
   *[other] { $count } yıldız
}
local-label = { $label ->
    [1] Kırmızı
    [2] Sarı
    [3] Yeşil
   *[4] Mavi
}
local-pick = Seçildi
local-reject = Reddedildi
local-no-keywords = anahtar sözcük yok
local-changed-elsewhere = başka yerde değişti
local-photo-changed = fotoğrafın kendisi değişti
local-edit-here = burada düzenlendi, { $when }
local-edit-elsewhere = başka yerde düzenlendi, { $when }
local-edit-none = düzenleme yok

local-export-ask = { $count ->
    [one] 1 fotoğraf çevrimdışı. Proxy’sinden dışa aktarılsın mı?
   *[other] { $count } fotoğraf çevrimdışı. Proxy’lerinden dışa aktarılsın mı?
}
local-export-detail = Proxy’lerin uzun kenarı 2.560 pikseldir. Tam boyutu özgünler verir.
local-export-proxy = Proxy’lerden dışa aktar
local-export-wait = Özgünleri bekle
local-export-waiting = Dışa aktarma, özgünler geri gelince başlar.

local-panel-title = Yerel kopyalar
local-panel-used = { $limit } alanın { $used } kadarı kullanılıyor.
local-panel-originals = { $count ->
    [one] 1 özgün
   *[other] { $count } özgün
}
local-panel-proxies = { $count ->
    [one] 1 fotoğraf, ham dosyalar proxy olarak
   *[other] { $count } fotoğraf, ham dosyalar proxy olarak
}
local-panel-remove = Kaldır…
local-panel-footer = { $folder } içinde saklanır. Özgünler asla değiştirilmez.
local-panel-keywords = çevrimdışı yerel kopyalar proxy alan
