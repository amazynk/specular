# machine-drafted, needs native review
# Specular — Fotoğraf makinelerinden, telefonlardan ve bellek kartlarından içe aktarma. Türkçe.
# Kart yalnızca okunur; dosyalar ondan yalnızca istendiğinde, doğrulanmış bir kopyadan sonra silinir.

palette-import = Fotoğrafları içe aktar…
disabled-no-import-source = Bağlı bellek kartı, fotoğraf makinesi veya telefon yok

## Ayarlar → İçe aktarma
settings-section-import = İçe aktarma
settings-import-on-card = Kart takıldığında
settings-import-on-card-help = Fotoğraf içeren bir bellek kartı, fotoğraf makinesi veya telefon bağlandığında ne olacağı.
settings-import-on-card-ask = Sor
settings-import-on-card-open = İçe aktarmayı aç
settings-import-on-card-nothing = Hiçbir şey yapma
settings-import-destination = Hedef
settings-import-destination-help = İçe aktarılan fotoğrafların ve videoların gittiği klasör. Ayarlanmazsa Resimler klasörünüze gider.
settings-import-destination-unset = Resimler klasörü
settings-import-folder-pattern = Klasörler
settings-import-folder-pattern-help = İçinde oluşturulan klasörler: {"{"}year{"}"}, {"{"}month{"}"}, {"{"}day{"}"}, {"{"}camera{"}"} ve {"{"}event{"}"}, / ile ayrılır.
settings-import-rename-pattern = Yeniden adlandır
settings-import-rename-pattern-help = Dönüştür'deki gibi yeni dosya adları: {"{"}name{"}"}, {"{"}n:03{"}"}, {"{"}date:%Y%m%d{"}"}, {"{"}camera{"}"}, {"{"}event{"}"}. Boş bırakılırsa fotoğraf makinesinin adları korunur.
settings-import-keep-pairs = RAW ve JPEG'i bir arada tut
settings-import-keep-pairs-help = Bir RAW dosyası ve onunla çekilen JPEG, yeniden adlandırılsalar bile aynı klasöre aynı adla gider.
settings-import-low-res-videos = Düşük çözünürlüklü video kopyalarını içe aktar
settings-import-low-res-videos-help = Bazı fotoğraf makineleri her videonun küçük bir önizleme kopyasını kaydeder (.LRV). Kapalıyken kartta kalırlar.
settings-import-eject = Bitince çıkar
settings-import-eject-help = Her şey kopyalanınca kartı çıkarır, böylece kart yerinden alınabilir.

## Kart takıldığındaki öneri
import-offer = { $name } içinden içe aktarılsın mı?
import-offer-button = İçe aktar

## İçe aktarma görünümü
import-title = İçe aktar
import-from = kaynak: { $name }
import-source-next = Sonraki kaynak
import-close = Kapat
import-scanning = Yeni fotoğraflar aranıyor… { $done } / { $total }
import-scanning-start = Yeni fotoğraflar aranıyor…
import-none-new = { $name } üzerindeki her şey daha önce içe aktarıldı.
import-empty = { $name } üzerinde fotoğraf veya video yok.
import-no-source = Bağlı bellek kartı, fotoğraf makinesi veya telefon yok.
import-summary = { $count ->
    [one] { $count } yeni öğe
   *[other] { $count } yeni öğe
} · { $size }
import-already = { $count } tanesi daha önce içe aktarıldı
import-unreadable = { $count ->
    [one] { $count } dosya okunamadı
   *[other] { $count } dosya okunamadı
}
import-selected = { $count } / { $total } seçili
import-select-all = Tümünü seç
import-select-none = Seçimi kaldır
import-badge-pair = RAW+JPEG
import-badge-raw = RAW
import-badge-video = Video

import-destination = Hedef
import-choose = Seç…
import-folder-pattern = Klasörler
import-event = Etkinlik
import-event-hint = İsteğe bağlı, {"{"}event{"}"} için
import-rename = Yeniden adlandır
import-rename-hint = Boş bırakılırsa fotoğraf makinesinin adları korunur
import-preview = İlk dosya: { $path }
import-pattern-error = { $error }
import-keep-pairs = RAW ve JPEG'i bir arada tut
import-eject = Bitince çıkar
import-delete = İçe aktardıktan sonra karttan sil
import-free = Kopyalanacak: { $size } · boş alan: { $free }
import-free-unknown = Kopyalanacak: { $size }
import-no-space = { $folder } içinde yeterli boş alan yok: bu içe aktarma { $need } gerektiriyor ve yalnızca { $free } boş. Yer açın veya başka bir klasör seçin.
import-start = { $count ->
    [one] { $count } öğeyi içe aktar
   *[other] { $count } öğeyi içe aktar
}
import-nothing-selected = İçe aktarılacak fotoğrafları seçin

import-confirm-delete = { $count ->
    [one] Kopyalanıp doğrulandıktan sonra { $count } dosya { $name } üzerinden silinecek. Bu geri alınamaz.
   *[other] Kopyalanıp doğrulandıktan sonra { $count } dosya { $name } üzerinden silinecek. Bu geri alınamaz.
}
import-confirm-yes = İçe aktar ve sil
import-confirm-no = Geri dön

import-progress = İçe aktarılıyor { $done } / { $total } · { $size }
import-progress-eta = İçe aktarılıyor { $done } / { $total } · { $size } · { $time }
import-paused = Duraklatıldı · { $done } / { $total }
import-pause = Duraklat
import-resume = Sürdür
import-cancel = İptal
import-hide = Gizle
import-stopping = Durduruluyor…
import-running = Zaten bir içe aktarma sürüyor.

import-done = { $count ->
    [one] { $count } dosya
   *[other] { $count } dosya
} { $folder } klasörüne aktarıldı
import-done-existed = { $count } tanesi zaten vardı
import-done-deleted = { $count } tanesi karttan silindi
import-done-failed = { $count ->
    [one] { $count } dosya içe aktarılamadı
   *[other] { $count } dosya içe aktarılamadı
}
import-done-cancelled = İçe aktarma { $count } dosyadan sonra iptal edildi. Yeniden içe aktarmak kaldığı yerden devam eder.
import-done-nothing = Yeni bir şey içe aktarılmadı.
import-ejected = { $name } çıkarılabilir.
import-eject-failed = { $name } çıkarılamadı: { $error }
import-failed-destination = { $folder } klasörü kullanılamıyor: { $error }
import-ledger-failed = İçe aktarılan dosyaların listesi okunamadı ({ $error }); daha önce içe aktarılan dosyalar yeniden önerilebilir.

## Ekran okuyucu
import-a11y-group = İçe aktar
import-a11y-tile = { $name }, { $date }
import-a11y-tile-selected = { $name }, { $date }, seçili
