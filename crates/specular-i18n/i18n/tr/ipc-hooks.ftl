# machine-drafted, needs native review
# Remote control and events (Settings → Integration), and hooks: programs settings.toml
# names to run when something happens in the viewer.

settings-integration-ipc = Uzaktan denetim ve olaylar
settings-integration-ipc-help = Bu bilgisayarda sizin adınıza çalışan başka programlar, özel bir soket (specular ipc) üzerinden { -app-name } uygulamasını yönetebilir ve içinde olanları duyabilir. Ağda hiçbir şey dinlemez.

## The hooks panel under "Remote control and events".
hooks-panel-title = Kancalar
hooks-panel-intro = Bir şey olduğunda settings.toml dosyasının çalıştırdığı programlar. Her biri ilk çalışmadan önce bir kez sorar.
hooks-panel-none = Kanca yok. settings.toml dosyasına [[hooks]] olarak ekleyin.
hooks-panel-keywords = kancalar hooks olaylar otomasyon betikler programlar
# $events: the event names, joined ("marked, exported").
hooks-panel-on = Olaylar: { $events }
hooks-panel-untrusted = İlk çalışmadan önce sorar
hooks-panel-allowed = İzin verildi
hooks-panel-remove = Kaldır…
hooks-panel-unused = Kullanılmıyor

## Asking before a hook's first run (and again when its program or arguments change).
# $name: the hook's name; $program: the program; $events: what sets it off (hook-event-*,
# joined with hook-or).
hook-consent-title = { $events } “{ $name }” ({ $program }) çalıştırılsın mı?
hook-consent-detail = Bu programı fotoğrafların adlarıyla başlatır: { $args }
hook-consent-trust = Yalnızca güvendiğiniz programlara izin verin. Kanca değişirse yeniden sorar.
hook-consent-allow = İzin ver
hook-consent-not-now = Şimdi değil
hook-or = { " " }veya{ " " }
hook-event-opened = bir fotoğraf açıldığında
hook-event-marked = fotoğraflar işaretlendiğinde
hook-event-keywords = anahtar sözcükler değiştiğinde
hook-event-exported = fotoğraflar dışa aktarıldığında
hook-event-imported = fotoğraflar içe aktarıldığında
hook-event-captured = bir kamera fotoğraf gönderdiğinde
hook-event-develop-saved = bir Geliştir düzenlemesi kaydedildiğinde
hook-event-any = herhangi bir şey olduğunda

## Removing a hook (Settings › Integration › Hooks › Remove…).
hook-remove-title = “{ $name }” kancası kaldırılsın mı?
hook-remove-detail = settings.toml dosyasından çıkarılır. Program ve dosyalarınız diskte kalır.
hook-remove-ok = Kaldır
hook-removed = “{ $name }” kancası kaldırıldı.
hook-settings-unwritable = settings.toml değiştirilemedi: { $reason }

## A hook that did not work (a quiet note; the log has the details).
hook-failed = “{ $name }” kancası çalışmadı: { $reason }
hook-failed-exit = { $code } koduyla sona erdi
hook-failed-timeout = { $seconds } sn sonra durduruldu
hook-failed-start = başlatılamadı ({ $reason })
hook-dropped = “{ $name }” kancası meşgul: { $count ->
   *[other] { $count } olay atlandı.
}

## An export started by another program (specular ipc export): nobody was asked.
export-recipes-remote-exists = { $count ->
   *[other] değiştirilecek { $count } dosya zaten var: hiçbir şey dışa aktarılmadı
}
export-recipes-remote-skipped = { $file } zaten var
