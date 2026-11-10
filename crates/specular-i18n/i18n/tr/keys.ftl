# Specular — Settings → Keys, the key binding editor (DESIGN.md §11). machine-drafted, needs native review
# Key names inside messages ($keys) are never translated.

settings-keys-sequence-timeout = Sonraki tuşu bekle
settings-keys-sequence-timeout-help = G G gibi bir tuş dizisinin sonraki tuşu ne kadar beklediği. Daha uzun bir diziyi de başlatan bir tuş bu duraklamadan sonra çalışır.
settings-keys-bindings = Tuş atamaları
settings-keys-bindings-help = Bir eylem seçin ve atanacak tuşlara basın. Her modun kendi tuşları vardır; Ayıklama modu ve tıbbi görüntüler aksi halde görüntüleyicinin tuşlarını kullanır. Esc atamak için ona dokunun; basılı tutmak iptal eder. Atamalar keymap.toml dosyasına kaydedilir.
palette-customise-keys = Tuşları özelleştir…
keys-mode-viewer = Görüntüleyici
keys-mode-cull = Ayıklama modu
keys-mode-medical = Tıbbi görüntüler
keys-modes = Mod
keys-presets = Hazır ayar
keys-preset-default = Varsayılan
keys-preset-vim = Vim
keys-preset-culling = Lightroom tarzı ayıklama
keys-preset-photo-mechanic = Photo Mechanic tarzı
action-pan-left = Sola kaydır
action-pan-right = Sağa kaydır
action-pan-up = Yukarı kaydır
action-pan-down = Aşağı kaydır
action-pan-page-left = Bir ekran sola kaydır
action-pan-page-right = Bir ekran sağa kaydır
action-pan-page-up = Bir ekran yukarı kaydır
action-pan-page-down = Bir ekran aşağı kaydır
action-reload = Görüntüyü yeniden yükle
keys-action-cull-pick = Seç
keys-action-cull-reject = Reddet
keys-action-cull-unflag = İşareti kaldır
keys-action-cull-rate = { $stars ->
    [0] Puanı kaldır
    [one] 1 yıldız ver
   *[other] { $stars } yıldız ver
}
keys-action-cull-label = Renk etiketi: { $label }
keys-action-cull-copy-to = { $slot }. klasöre kopyala
keys-action-cull-move-to = { $slot }. klasöre taşı
keys-import = İçe aktar…
keys-export = Dışa aktar…
keys-export-mode = { $mode } dışa aktar…
keys-reset-mode = { $mode } sıfırla
keys-reset-all = Tüm tuşları sıfırla
keys-add = { $action } için tuş ekle
keys-change = { $keys } değiştir
keys-remove = { $keys } kaldır
keys-reset-action = { $action } sıfırla
keys-none = Tuş yok
keys-changed = Değiştirildi
keys-press = Tuşlara basın…
keys-recording-hint = İptal için Esc'yi basılı tutun
keys-cancel-recording = İptal
keys-no-match = “{ $query }” ile eşleşen eylem veya tuş yok.
keys-list = Eylemler ve tuşları
keys-confirm-preset = { $preset } hazır ayarı kullanılsın mı? Tüm tuş atamalarınızın yerini alır.
keys-confirm-reset-mode = { $mode } modunun tüm tuşları hazır ayara sıfırlansın mı?
keys-confirm-reset-all = Tüm modların tüm tuşları hazır ayara sıfırlansın mı?
keys-confirm-ok = Değiştir
keys-confirm-reset = Sıfırla
keys-confirm-cancel = İptal
keys-conflict-same = { $keys } zaten “{ $other }” eylemini çalıştırıyor.
keys-conflict-prefix = { $keys } ve { $sequence } (“{ $other }”) aynı başlıyor: kısa olan yalnızca bir duraklamadan sonra çalışır.
keys-conflict-shadow = { $keys }, { $mode } modunda “{ $other }” eylemini çalıştırır.
keys-conflict-reserved = { $keys } Specular'a hiç ulaşmayabilir: { $reason }
keys-choice-replace = Değiştir
keys-choice-swap = Yer değiştir
keys-choice-keep = İkisini de tut
keys-choice-anyway = Yine de ata
keys-choice-cancel = İptal
keys-reserved-switch = pencereler arasında geçiş yapar.
keys-reserved-close = pencereyi kapatır.
keys-reserved-secure = sistem onu alır.
keys-reserved-desktop = Super kısayolları masaüstüne aittir.
keys-reserved-workspaces = terminaller veya çalışma alanları arasında geçiş yapar.
keys-reserved-input = giriş yöntemini veya klavye düzenini değiştirir.
keys-reserved-macos = macOS onu alır.
keys-announce-recording = { $action } için tuşlar kaydediliyor. Tuşlara basın veya iptal için Esc'yi basılı tutun.
keys-announce-key = { $keys }
keys-announce-bound = { $keys } artık { $action } eylemini çalıştırıyor.
keys-announce-already = { $keys } zaten { $action } eylemini çalıştırıyor.
keys-announce-cancelled = Kayıt iptal edildi.
keys-announce-removed = { $keys }, { $action } eyleminden kaldırıldı.
keys-announce-reset-action = { $action } yeniden hazır ayar tuşlarına sahip.
keys-announce-reset-mode = { $mode } tuşları sıfırlandı.
keys-announce-reset-all = Tüm tuşlar sıfırlandı.
keys-announce-preset = Artık { $preset } hazır ayarı kullanılıyor.
keys-toast-imported = Tuş atamaları içe aktarıldı
keys-toast-exported = Tuş atamaları dışa aktarıldı
keys-toast-reloaded = keymap.toml değişti: tuş atamaları yeniden yüklendi
keys-toast-error = Tuş atamaları okunamadı: { $error }
keys-toast-write-error = keymap.toml kaydedilemedi: { $error }
keys-pending = { $keys } …  { $next }
keys-problem-syntax = keymap.toml okunamadı: { $error }
keys-problem-key = Bilinmeyen “{ $key }” tuşu yok sayıldı.
keys-problem-action = “{ $key }” bilinmeyen “{ $action }” eylemini belirtiyor; yok sayıldı.
keys-problem-mode = [{ $mode }] tablosu bir mod değil; yok sayıldı.
keys-problem-duplicate = { $keys }, { $mode } içinde iki kez verilmiş; ikincisi kullanılır.
keys-problem-prefix = { $keys }, { $mode } içinde { $longer } dizisini başlatıyor; bu yüzden yalnızca bir duraklamadan sonra çalışır.
keys-problem-preset = Bilinmeyen “{ $preset }” hazır ayarı; varsayılan tuşlar geçerli.

## keymap.toml editor (machine-drafted, needs native review)

keys-edit-file = keymap.toml dosyasını düzenle
keys-problem-action-suggestion = “{ $key }” bilinmeyen “{ $action }” eylemini belirtiyor; yok sayıldı. Şunu mu demek istediniz: “{ $suggestion }”?
keys-problem-mode-suggestion = [{ $mode }] tablosu bir mod değil; yok sayıldı. Şunu mu demek istediniz: [{ $suggestion }]?
keys-problem-preset-suggestion = Bilinmeyen “{ $preset }” hazır ayarı; varsayılan tuşlar geçerli. Şunu mu demek istediniz: “{ $suggestion }”?
