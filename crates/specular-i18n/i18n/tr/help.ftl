# machine-drafted, needs native review
## Kılavuz paneli, çökme raporları, tanılama paketi, güncelleme denetimi ve Hakkında. Türkçe.
## Üslup: kısa, açık, dolgusuz; her cümlede tek fikir.

## Palet

palette-help-manual = Yardım
palette-help-manual-help = Kılavuzu ekrandakiyle ilgili sayfada aç.
palette-save-diagnostics = Tanılama paketini kaydet…
palette-save-diagnostics-help = Çökme raporlarını, günlüğü ve sistem ayrıntılarını bir zip dosyasına kaydet. Hiçbir şey gönderilmez.
# Kılavuzun bir bölümünü açan palet girdisi; { $title } bir sayfa ya da "Sayfa › Bölüm".
palette-help-topic = Yardım: { $title }
palette-help-topic-help = Kılavuzun bu bölümünü aç.
# Paletteki kullanılamayan bir komutun altında.
palette-disabled-help = Şu an kullanılamıyor. Nedenini okumak için Enter'a basın.

## Yardım paneli

help-title = Yardım
help-search-placeholder = Kılavuzda ara
help-contents = İçindekiler
help-back = Geri
help-forward = İleri
help-no-results = “{ $query }” için bir şey bulunamadı.
help-results = { $count ->
    [one] 1 bölüm bulundu
   *[other] { $count } bölüm bulundu
}
help-section-start = Başlarken
help-section-modes = Görüntüleme ve modlar
help-section-tasks = Nasıl yaparım…
help-section-reference = Başvuru
help-keys-unbound = Atanmadı
# Kılavuzda geçen ve tuşu olmayan bir komut; paletten çalışır.
help-key-unbound = { $name } (palet)
help-keys-command = Komut
help-keys-keys = Tuşlar
help-commands = Her komut komut paletindedir ({ $key }). Adının bir kısmını yazın.
help-commands-no-key = Her komut komut paletindedir. Adının bir kısmını yazın.
help-link = Git: { $title }
help-open = Yardım
# Panelin ekran okuyucu adı; { $page } gösterilen sayfadır.
help-label = Yardım: { $page }
help-disabled-why = Neden?

## Sonraki açılışta sunulan çökme raporu

crash-title = { -app-name } beklenmedik şekilde kapandı
crash-body = Bu bilgisayara bir rapor kaydedildi. Hiçbir şey gönderilmedi.
crash-view = Raporu göster
crash-hide = Raporu gizle
crash-copy = Kopyala
crash-copied = Rapor kopyalandı
crash-save-bundle = Tanılama paketini kaydet…
crash-dismiss = Kapat

## Tanılama paketi

diag-title = Tanılama paketini kaydet
diag-intro = Hata raporuna eklemek için bir zip dosyası. Bu bilgisayara kaydedilir. Hiçbir şey gönderilmez.
diag-contents = İçerik
diag-include-names = Dosya adlarını ekle
diag-include-names-help = Kapalı: adlar file-1.jpg, dir-1 vb. ile değiştirilir.
diag-privacy = Ev klasörünüz ~, kullanıcı adınız <user> olarak görünür. Parolalar, sunucu adresleri ve görüntüler asla eklenmez.
diag-where = Sorun bildir: { $issues } · { $email }
diag-save = Kaydet…
diag-cancel = İptal
diag-saved = Tanılama paketi kaydedildi: { $path }
diag-failed = Tanılama paketi kaydedilemedi: { $reason }
diag-entry-summary = Sürüm, sistem, GPU ve sürücü
diag-entry-crash = Son çökme raporu
diag-entry-crashes = Önceki çökme raporları ({ $count })
diag-entry-log = Son { $count } günlük satırı
diag-entry-settings = Parolalar ve adresler olmadan ayarlar
diag-entry-gpu = GPU ve ekran ayrıntıları
diag-entry-readme = Bu paket nedir
diag-size = { $size }
# Paketin içindeki README.txt.
diag-readme =
    { -app-name } tanılama paketi.

    Bir sorunu bulmaya yardımcı olanları içerir: sürüm ve sistem ayrıntıları, çökme raporları, günlüğün sonu ve ayarlar.

    Ev klasörünüz ~, kullanıcı adınız <user> olarak görünür. Eklemeyi seçmediyseniz dosya ve klasör adları file-1, dir-1 vb. ile değiştirilir. Parolalar ve sunucu adresleri kaldırılır.

    Hiçbir şey gönderilmedi. Bu dosyayı paylaşıp paylaşmamaya siz karar verirsiniz.

## Güncellemeler (Ayarlar)

settings-section-updates = Güncellemeler
settings-updates-check = Güncellemeleri denetle
settings-updates-check-help = Günde bir kez yeni sürüm olup olmadığını sor. İstek yalnızca sürüm numarasını taşır: tanımlayıcı yok, çerez yok. Kapalı: { -app-name } güncellemeler için asla bağlanmaz.
settings-updates-channel = Kanal
settings-updates-channel-help = Kararlı: yalnızca sürümler. Beta: test sürümleri de.
settings-updates-channel-stable = Kararlı
settings-updates-channel-beta = Beta
settings-updates-check-now = Şimdi denetle
settings-updates-check-now-help = Şimdi bir kez denetle.
update-available = { $version } sürümü kullanılabilir
update-notes = Sürüm notları
update-download = İndir
update-none = { -app-name } güncel.
update-checking = Güncellemeler denetleniyor…
update-failed = Güncellemeler denetlenemedi: { $reason }
update-last-checked = Son denetim: { $when }

## Hakkında

about-build = Derleme { $commit } · { $profile } · { $target }
about-build-no-commit = Derleme { $profile } · { $target }
about-help = Yardım
about-diagnostics = Tanılama paketini kaydet…
# machine-drafted, needs native review
about-renderer = Oluşturucu bilgileri…
about-licences-count = { $count ->
    [one] 1 üçüncü taraf bileşen
   *[other] { $count } üçüncü taraf bileşen
}
about-licence-group = { $licence } ({ $count })
about-contact = Sorunlar: { $issues } · Destek: { $email }
