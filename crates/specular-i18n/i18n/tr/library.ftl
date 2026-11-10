# machine-drafted, needs native review
# Specular — Library (DESIGN §10).

palette-library = Kitaplık

## Top bar
library-title = Kitaplık
library-locations-button = Konumlar

## Locations sidebar
library-locations = Konumlar
library-all = Tüm konumlar
library-add = Konum ekle…
library-remove = { $location } konumunu kitaplıktan kaldır
library-offline = Çevrimdışı
library-location-pending = Dizinlenecek: { $count }
library-a11y-location = { $location }, { $count ->
   *[other] { $count } görüntü
}
library-a11y-location-offline = { $location }, Çevrimdışı, { $count ->
   *[other] { $count } görüntü
}

## Indexing progress
library-walking = Bulunan görüntüler: { $count } · { $location }
# machine-drafted, needs native review
library-walking-count = Bulunan görüntüler: { $count }
library-reading = Dizinleniyor… { $done }/{ $total }
library-held = Siz gezinirken dizinleme duraklar
library-up-to-date = Güncel
# machine-drafted, needs native review
library-stopped = Dizinleme durduruldu
library-stopped-pending = Dizinleme durduruldu · Dizinlenecek: { $count }
library-resume = Sürdür
# machine-drafted, needs native review
# The indexer stopped by itself (a database error, a bug) or stopped moving; $reason says why.
library-halted = Dizinleme durdu: { $reason }
library-reason-stalled = { $minutes ->
    [one] bir dakikadır
   *[other] { $minutes } dakikadır
} hiçbir şey ilerlemedi. Bir dosya veya sürücü yanıt vermiyor olabilir.
library-retry = Yeniden dene
library-retry-description = Dizinlemeyi yeniden başlatır. Dizinlenenler aranabilir kalır.
library-save-diagnostics = Tanılama paketini kaydet…
library-stop-description = Dizinlemeyi durdurur. O ana kadar dizinlenenler aranabilir kalır; Sürdür veya Yeniden tara (F5) devam eder.
library-resume-description = Tüm konumları yeniden gözden geçirir ve kalanları dizinler.
library-opening = Kitaplık açılıyor…
library-searching = Aranıyor…

## Empty and error states
library-empty = Henüz konum yok. Bir klasör ekleyin: onun ve alt klasörlerinin görüntüleri burada tek bir ızgarada görünür.
library-empty-offline = Bu konumlar çevrimdışı. Görüntüleri, konumlar geri geldiğinde yeniden görünür.
library-indexing = Görüntüler aranıyor…
# machine-drafted, needs native review
library-remove-confirm = “{ $location }” kitaplıktan kaldırılsın mı?
library-remove-confirm-detail = Fotoğraflarınız puanları, etiketleri ve anahtar sözcükleriyle birlikte diskte kalır. Kitaplık orada dizinlediklerini unutur; klasörü yeniden eklerseniz yeniden dizinlenir.
library-remove-confirm-ok = Kaldır
library-indexing-in = { $location } içinde görüntüler aranıyor… şimdiye kadar { $count } bulundu
# machine-drafted, needs native review
library-empty-stopped = Dizinleme, bu görüntüler okunmadan önce durduruldu. Sürdür oradan devam eder.
library-unavailable = Kitaplık açılamadı: { $reason }
library-read-only = Bu kitaplık daha yeni bir Specular tarafından yazıldı: aranabilir ama güncellenemez.

## Screen readers
library-a11y-list = Kitaplık, { $count ->
   *[other] { $count } görüntü
}
library-a11y-added = { $location } kitaplığa eklendi
library-a11y-removed = { $location } kitaplıktan kaldırıldı

## Adding the home folder
# machine-drafted, needs native review
library-home-confirm = “{ $folder }” içindeki her şey eklensin mi?
library-home-confirm-detail = Yalnızca fotoğraflar değil, program dosyaları, indirilenler ve önbellekler de burada. Dizinleme daha uzun sürer ve kitaplık simgeler ile başka görüntülerle dolar.
library-home-confirm-pictures = Resimler klasörünüzde yalnızca resimleriniz var.
library-home-confirm-use-pictures = Resimler’i ekle
library-home-confirm-keep = Ana klasörü ekle

## Folder watches
# machine-drafted, needs native review
library-unwatched = Bazı klasörler izlenmiyor
library-unwatched-detail = { $minutes ->
   *[other] Değişiklikler { $minutes } min içinde görünür
}
library-unwatched-description = Specular’ın klasör izleme hakkı bitti. Sınırın nasıl artırılacağını anlatan yardımı açar.

## Settings → Library
# machine-drafted, needs native review
settings-section-library = Kitaplık
settings-library-skip-folders = Şu adlı klasörleri atla
settings-library-skip-folders-help = Kitaplık, bu adlara sahip klasörlere bakmaz; adları virgülle ayırın. Gizli klasörler her zaman atlanır.
settings-library-skip-marked = Şunları içeren klasörleri atla
settings-library-skip-marked-help = Bunlardan birini içeren klasörlere de bakılmaz: kod depoları için .git, Python ortamları için pyvenv.cfg, önbellekler için CACHEDIR.TAG. Bir konumun kendi klasörü hiçbir zaman atlanmaz.
settings-library-show-timeline = Zaman çizelgesi
settings-library-show-timeline-help = Kitaplık ızgarasının üstünde her yıl, ay ya da günde kaç fotoğraf olduğunu gösterir.
