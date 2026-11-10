# machine-drafted, needs native review
# Tümleştirme: birlikte aç, kullanıcı komutları, yapıştır (Aşama 2).
palette-open-with = Birlikte aç…
palette-paste = Yapıştır
open-with-placeholder = Birlikte aç…
open-with-loading = Uygulamalar aranıyor…
open-with-default = { $name } (varsayılan)
open-with-choose-other = Başka bir uygulama seç…
open-with-choose-title = Bir uygulama seçin
open-with-failed = Açılamadı (uygulama: { $name }): { $reason }
command-missing = Bulunamadı: { $program }
command-failed = Başlatılamadı: { $name }: { $reason }
command-exited = Hatayla sonlandı: { $name }: { $status }
paste-nothing = Panoda açılacak bir şey yok
paste-failed = Görüntü yapıştırılamadı: { $reason }
drag-failed = Dosya sürüklenemedi: { $reason }
about-storage = Ayarlar: { $settings } · Önbellek: { $cache }
about-storage-portable = Taşınabilir mod: ayarlar ve önbellek: { $dir }

## Scripting and recipes

palette-copy-view-command = Görünümü kabuk komutu olarak kopyala
palette-copy-view-python = Görünümü Python olarak kopyala
toast-copied-view-command = Komut kopyalandı
script-copy-no-image = Tanımlanacak görüntü yok
recipe-confirm-title = “{ $name }” tarifi ilk kez çalıştırılsın mı? Çalıştırdığı: { $command }
recipe-confirm-run = Çalıştır
recipe-no-file = Tarif bir görüntü dosyası gerektirir
recipe-running = { $name } çalışıyor…
recipe-done = { $name }: { $shapes ->
   *[other] { $shapes } şekil
} eklendi
recipe-failed = { $name } başarısız: { $reason }
recipe-timeout = { $seconds } sn sonra durduruldu
recipe-no-output = { $name } { $file } yazmadı
