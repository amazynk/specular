# Specular — OMERO servers and the Image Data Resource (IDR), browsed read-only. Türkçe. machine-drafted, needs native review

palette-omero-servers = OMERO sunucuları…

omero-title = OMERO sunucuları
omero-servers-intro = Bir sunucudaki projeler, taramalar ve görüntüler yalnızca siz göz attığınızda listelenir. Öncesinde hiçbir şey gönderilmez.
omero-servers = Sunucular
omero-servers-empty = Henüz sunucu yok. OMERO.web adresiyle bir sunucu ekleyin.
omero-session-only = Parolalar yalnızca Specular kapanana kadar saklanır.
omero-add = Ekle…
omero-edit = Düzenle…
omero-remove = Kaldır
omero-close = Kapat
omero-browse = Göz at
omero-back = Geri
omero-save = Kaydet
omero-stop = Durdur
omero-open = Aç
# $server a server's name.
omero-removed = { $server } kaldırıldı
# Shown with servers reached without encryption (http://).
omero-not-encrypted = Şifrelenmemiş
# A server browsed without signing in.
omero-public = Herkese açık
# $user the user name a server is signed in with.
omero-signed-in-as = { $user } olarak

# The page that adds or edits a server.
omero-add-title = Sunucu ekle
omero-edit-title = Sunucu ayarları
omero-field-name = Ad
omero-hint-name = İsteğe bağlı
omero-field-url = Adres
omero-hint-url = https://omero.example.org
omero-field-user = Kullanıcı adı
omero-hint-user = Herkese açık veriler için boş
omero-field-password = Parola
omero-hint-password-kept = Verilen parolayı korumak için boş bırakın
omero-password-session = Parola yalnızca Specular kapanana kadar saklanır, asla diske yazılmaz.
omero-password-needed = Parolayı girin: yalnızca Specular kapanana kadar saklanır.
omero-http-warning = Bu adres şifrelenmemiş: parola açık metin olarak gönderilir.
omero-url-invalid = https:// veya http:// ile başlayan OMERO.web adresini girin.
# $reason why the list could not be saved.
omero-save-failed = Sunucu listesi kaydedilemedi: { $reason }

# The browse page.
omero-browse-title = Göz at
omero-top = Projeler ve taramalar
omero-datasets = Veri kümeleri
omero-plates = Plakalar
omero-wells = Kuyucuklar
omero-fields = Alanlar
omero-images = Görüntüler
omero-kind-project = Proje
omero-kind-dataset = Veri kümesi
omero-kind-image = Görüntü
omero-kind-screen = Tarama
omero-kind-plate = Plaka
omero-kind-well = Kuyucuk
# $well a well's row and column, as B7.
omero-well = Kuyucuk { $well }
omero-well-fields = { $count ->
    [one] Bir alan
   *[other] { $count } alan
}
omero-children = { $count ->
    [one] Bir öğe
   *[other] { $count } öğe
}
omero-load-more = Daha fazla yükle
omero-loading = Yükleniyor…
omero-empty = Burada bir şey yok
omero-stopped = Durduruldu
omero-found = { $count ->
    [one] Bir öğe
   *[other] { $count } öğe
}
# $reason why the image could not be opened.
omero-open-failed = Görüntü açılamadı: { $reason }
# The channel keys would hide the last channel shown.
omero-channel-last = En az bir kanal gösterilmeye devam eder
# $server a server's name.
omero-session-ended = { $server } üzerindeki oturum sona erdi. Devam etmek için parolayı girin.

# Errors.
# $server a server's name.
omero-error-offline = { $server } sunucusuna ulaşılamadı.
omero-error-sign-in = Bu sunucu oturum açmayı gerektiriyor.
omero-error-expired = Oturum sona erdi.
omero-error-login = Kullanıcı adı veya parola reddedildi.
omero-error-forbidden = Hesabınız bunu görme iznine sahip olmayabilir.
omero-error-not-found = Sunucuda bulunamadı (kaldırılmış olabilir).
# $code an HTTP status code.
omero-error-status = Sunucu { $code } hatasıyla yanıt verdi.
# $reason what was wrong with the answer.
omero-error-malformed = Sunucunun yanıtı okunamadı: { $reason }
