# machine-drafted, needs native review
# Specular — Açılış raporu: bir görüntünün açılması neden bu kadar sürdü (bilgi paneli,
# “Performans” bölümü; DESIGN.md §12.7).
# Turkish suffixes after placeables are avoided; native review should check word order.

open-report-title = Performans

## Headlines
open-report-fast = Açılma süresi: { $ms } ms
open-report-display-cache = Görüntüleme önbelleğinden gösterildi
open-report-slow-read = Yavaş depolama: sürenin { $percent } kadarı dosyayı okumaya gitti ({ $speed } MB/s)
# machine-drafted, needs native review
open-report-slow-read-no-speed = Yavaş depolama: sürenin { $percent } kadarı dosyayı okumaya gitti (küçük dosya: çoğunlukla depolamanın yanıt vermesini bekleme)
open-report-archive-read = Sürenin { $percent } kadarı arşivi açmaya gitti
open-report-startup = Sürenin { $percent } kadarı { -app-name } uygulamasını başlatmaya gitti
open-report-checked = İki kez çözüldü: donanım kod çözücünün ilk sonucu CPU'nunkiyle karşılaştırıldı (sürenin { $percent } kadarı)
open-report-hardware = Donanım kod çözücüyle çözüldü; sürenin { $percent } kadarını aldı
open-report-decode = Büyük görüntü: { $mpx } megapikselin kodunu çözmek sürenin { $percent } kadarını aldı
open-report-upload = Sürenin { $percent } kadarı görüntüyü GPU'ya göndermeye gitti
open-report-waiting = Sürenin { $percent } kadarı başka işlerin bitmesini beklemeye gitti
open-report-stage = { $stage }: sürenin { $percent } kadarı
open-report-memory-short = Bellek azdı, sistem takas alanı kullanmak zorunda kalmış olabilir

## Why a decode ran on the processor although a hardware decoder can do it
open-report-sw-not-probed = Yazılımla çözüldü: donanım kod çözücü hâlâ algılanıyordu
open-report-sw-no-device = Yazılımla çözüldü: bu bilgisayarda { $api } kullanılamıyor
open-report-sw-no-codec = Yazılımla çözüldü: bu GPU'da { $api } için { $codec } kod çözücü yok
open-report-sw-blocked-mismatch = Yazılımla çözüldü: donanım kod çözücünün sonucu CPU'nunkiyle eşleşmediği için kapatıldı
open-report-sw-blocked-error = Yazılımla çözüldü: donanım kod çözücü bir hatadan sonra kapatıldı
open-report-sw-blocked-lost = Yazılımla çözüldü: donanım kod çözücü yanıt vermeyi bıraktığı için kapatıldı
open-report-sw-failed = Yazılımla çözüldü: donanım kod çözücü bu görüntüde başarısız oldu
open-report-sw-busy = Yazılımla çözüldü: donanım kod çözücü meşguldü
open-report-sw-unsupported = Yazılımla çözüldü: donanım kod çözücü bu görüntüyü işleyemiyor
open-report-sw-unavailable = Yazılımla çözüldü: donanımla kod çözme, { -app-name } yeniden başlatılana kadar kapalı
open-report-sw-cpu-only = Yazılımla çözüldü: işleme “Yalnızca CPU” olarak ayarlı
open-report-sw-setting-off = Yazılımla çözüldü: donanımla kod çözme ayarlarda kapalı
open-report-sw-unchecked = Yazılımla çözüldü: donanım kod çözücü henüz denetlenmedi (önce arka planda hazırlanan bir görüntüde denenir)
open-report-sw-lower-power = Yazılımla çözüldü: pil gücünde CPU bunun için daha az enerji harcar
open-report-sw-faster = Yazılımla çözüldü: bu bilgisayarda daha hızlı (yaklaşık { $cpu } ms; donanım kod çözücüde { $hw } ms)
open-report-sw-scaled = Yazılımla çözüldü: donanım kod çözücü görüntüleri yalnızca tam boyutta çözebiliyor

## Hints
open-report-hint-copy-local = Daha hızlı açılması için dosyayı yerel bir diske kopyalayın.
open-report-hint-extract = Sayfalarının daha hızlı açılması için arşivi bir klasöre çıkarın.
open-report-hint-try-again = Donanım kod çözücüyü yeniden denemek için: Ayarlar → Performans → Yeniden dene.
open-report-hint-automatic = Ayarlar → Performans içinde “Görüntülerin işlendiği yer” seçeneğini Otomatik yapın.
open-report-hint-turn-on = Ayarlar → Performans içinde “Donanımla kod çözme” seçeneğini Otomatik yapın.
open-report-hint-free-memory = Bellek açmak için diğer programları kapatın.
open-report-hint-once-only = Yalnızca ilk görüntü bu şekilde denetlenir.

## Rows
open-report-total = Netleşene kadar
open-report-first-pixels = İlk pikseller
# machine-drafted, needs native review
open-report-whole = Görüntünün tamamı gösterildi
open-report-stage-startup = Başlatma
open-report-stage-read = Dosyayı okuma
open-report-stage-parse = Üst bilgiyi okuma
open-report-stage-colour = Renk yönetimini hazırlama
open-report-stage-decode = Kod çözme
open-report-stage-upload = GPU'ya gönderme
open-report-stage-waiting = Başka işleri bekleme
open-report-decoder = Kod çözen
open-report-decoder-checked = { $engine }, CPU ile denetlendi
open-report-decoder-cache = Görüntüleme önbelleği
# $scale is the reduction: 4 for a level a quarter of the full width and height.
open-report-decoder-scaled = { $engine }, 1/{ $scale } boyutta
open-report-decoder-scaled-full = { $engine }, tam boyutta, ardından 1/{ $scale } boyuta küçültüldü
open-report-decoder-scaled-no-index = { $engine }, 1/{ $scale } boyutta, hızlı başlatma dizini olmadan
open-report-colour-conversion = Renkleri dönüştüren

## Advisor: a pattern across recent opens, what would help and by how much (rows)
# $count and $total are counts of opens, $format a file format ("HEIC"), $speed a formatted
# number (MB/s), $low and $high formatted numbers ("2.5").
open-report-advice-recurring = Yinelenen
open-report-advice-change = Yardımcı olur
open-report-advice-gain = Tahmini kazanç
open-report-advice-software = Son { $total } yavaş { $format } görüntüsünden { $count } tanesi bu nedenle yazılımla çözüldü
open-report-advice-slow-storage = Son { $total } yavaş görüntüden { $count } tanesi dosya okunurken gecikti (yaklaşık { $speed } MB/s)
# machine-drafted, needs native review
open-report-advice-slow-storage-no-speed = Son { $total } yavaş görüntüden { $count } tanesi dosya okunurken gecikti (küçük dosyalar: çoğunlukla depolamanın yanıt vermesini bekleme)
open-report-advice-archive = Son { $total } yavaş görüntüden { $count } tanesi arşiv açılırken gecikti
open-report-advice-hardware-decoder = { $codec } donanım kod çözücüsü olan bir GPU
open-report-advice-gain-range = Yaklaşık { $low } ila { $high } kat daha hızlı
open-report-advice-gain-one = Yaklaşık { $low } kat daha hızlı

## Performance insights: the indicator in the corner and the panel it opens
# machine-drafted, needs native review
# $total is a formatted duration ("240 ms"), $format a file format ("HEIC").
open-report-insights-title = Performans önerileri
open-report-insights-indicator = Bu neden yavaş?
open-report-insights-announce = Performans önerisi: görüntüler aynı nedenle sürekli yavaş açılıyor. Neyin yardımcı olacağını görmek için “Bu neden yavaş?” öğesini açın.
open-report-insights-hide = Gizle
open-report-insights-last = Son yavaş görüntü: netleşene kadar { $total }
open-report-insights-now = Şimdi yapabilecekleriniz
open-report-insights-hardware = Yardımcı olacak donanım
open-report-insights-none = Tekrarlayan yavaşlama yok
open-report-insights-none-detail = Öneriler, birkaç görüntü aynı nedenle yavaş açıldığında görünür. { -app-name } bunları bulmak için bu bilgisayarda bir haftalık yavaş açılışları saklar; hiçbir yere hiçbir şey gönderilmez.
open-report-insights-software = { $format } görüntülerinin kodu CPU'da çözülüyor
open-report-insights-storage = Dosyaların okunması görüntüleri geciktiriyor
open-report-insights-archive = Arşivlerin açılması görüntüleri geciktiriyor
open-report-insights-nothing-now = { -app-name } ayarlarından hiçbiri bunu değiştirmez.
open-report-insights-display-cache = Görüntüleme önbelleğini açın: daha önce açtığınız görüntüler bu bilgisayardaki hızlı bir kopyadan açılır.
open-report-insights-prefetch = Daha fazla komşu görüntüyü hazır tutun; böylece siz bakarken sonrakiler hazırlanır.
open-report-insights-hw-ssd = Bu dosyalar için hızlı bir dahili sürücü (NVMe SSD)
open-report-insights-action-turn-on = Aç
open-report-insights-action-try-again = Yeniden dene
open-report-insights-action-automatic = Otomatik yap
open-report-insights-dismiss-storage = Depolama için bir daha gösterme
open-report-insights-dismiss-decoder = Kod çözme için bir daha gösterme
open-report-insights-off = Önerileri kapat
open-report-insights-on = Önerileri aç
open-report-insights-restore = Gizlenen önerileri yeniden göster
open-report-insights-power = Bu bilgisayar güç tasarrufu yapıyor (pil gücünde ya da bir güç tasarrufu modunda), bu da CPU'da kod çözmeyi yavaşlatıyor. Prize takılıyken görüntüler daha hızlı açılabilir.

## Performance insights in the gallery: the same, about making thumbnails
# machine-drafted, needs native review
# $count and $total are counts of thumbnails, $total in -last-thumbnail a formatted duration
# ("240 ms"), $format a file format ("HEIC"), $speed a formatted number (MB/s).
open-report-insights-announce-thumbnails = Performans önerisi: küçük resimler aynı nedenle sürekli yavaş görünüyor. Neyin yardımcı olacağını görmek için “Bu neden yavaş?” öğesini açın.
open-report-insights-last-thumbnail = Son yavaş küçük resim: oluşturulması { $total }
open-report-insights-thumbnails-software = { $format } görüntülerinin küçük resimlerinin kodu CPU'da çözülüyor
open-report-insights-thumbnails-storage = Dosyaların okunması küçük resimleri geciktiriyor
open-report-insights-thumbnails-archive = Arşivlerin açılması küçük resimleri geciktiriyor
open-report-insights-thumbnail-cache = Küçük resimler bu bilgisayarda saklanamıyor (küçük resim önbelleği kullanılamıyor), bu yüzden klasör her gösterildiğinde yeniden oluşturuluyor.
open-report-advice-thumbnails-software = Son { $total } yavaş { $format } küçük resminden { $count } tanesi bu nedenle yazılımla çözüldü
open-report-advice-thumbnails-slow-storage = Son { $total } yavaş küçük resimden { $count } tanesi dosya okunurken gecikti (yaklaşık { $speed } MB/s)
# machine-drafted, needs native review
open-report-advice-thumbnails-slow-storage-no-speed = Son { $total } yavaş küçük resimden { $count } tanesi dosya okunurken gecikti (küçük dosyalar: çoğunlukla depolamanın yanıt vermesini bekleme)
open-report-advice-thumbnails-archive = Son { $total } yavaş küçük resimden { $count } tanesi arşiv açılırken gecikti
