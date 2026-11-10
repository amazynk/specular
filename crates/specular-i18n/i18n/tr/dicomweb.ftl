# Specular — DICOMweb servers, study search and streamed series (DESIGN.md §12.8). Türkçe.

palette-dicom-servers = DICOMweb sunucuları…

dicomweb-title = DICOMweb sunucuları
dicomweb-servers-intro = Bir sunucudaki tetkikler yalnızca siz istediğinizde aranır ve açılır. Öncesinde hiçbir şey gönderilmez.
dicomweb-servers = Sunucular
dicomweb-servers-empty = Henüz sunucu yok. DICOMweb adresiyle bir sunucu ekleyin.
dicomweb-session-only = Parolalar ve belirteçler yalnızca Specular kapanana kadar saklanır.
dicomweb-add = Ekle…
dicomweb-edit = Düzenle…
dicomweb-remove = Kaldır
dicomweb-close = Kapat
dicomweb-browse = Gözat
dicomweb-back = Geri
dicomweb-save = Kaydet
dicomweb-search = Ara
dicomweb-stop = Durdur
dicomweb-removed = { $server } kaldırıldı
# machine-drafted, needs native review
dicomweb-remove-ask = “{ $server }” kayıtlı sunuculardan kaldırılsın mı?
dicomweb-remove-detail = Diskteki hiçbir şey kaybolmaz. Bu oturum için parolası veya erişim belirteci unutulur.
# Şifrelemesiz (http://) erişilen sunucularda gösterilir.
dicomweb-not-encrypted = Şifrelenmemiş

# Sunucu ekleme veya düzenleme sayfası.
dicomweb-add-title = Sunucu ekle
dicomweb-edit-title = Sunucu ayarları
dicomweb-field-name = Ad
dicomweb-hint-name = İsteğe bağlı
dicomweb-field-url = Adres
dicomweb-hint-url = https://pacs.example.org/dicom-web
dicomweb-field-auth = Oturum açma
dicomweb-auth-none = Oturum açma yok
dicomweb-auth-basic = Kullanıcı adı ve parola
dicomweb-auth-bearer = Erişim belirteci
dicomweb-field-user = Kullanıcı adı
dicomweb-field-password = Parola
dicomweb-field-token = Erişim belirteci
dicomweb-hint-secret-kept = Girilen değeri korumak için boş bırakın
dicomweb-secret-session = Parola veya belirteç yalnızca Specular kapanana kadar saklanır, asla diske yazılmaz.
dicomweb-secret-needed = Parolayı veya belirteci yeniden girin: yalnızca Specular kapanana kadar saklanır.
dicomweb-http-warning = Bu adres şifrelenmemiş: parola veya belirteç açık metin olarak gönderilir.
dicomweb-url-empty = Sunucunun DICOMweb adresini girin.
dicomweb-url-scheme = Adres https:// veya http:// ile başlamalıdır.
dicomweb-url-host = Adreste sunucu adı yok.
dicomweb-url-userinfo = Kullanıcı adını ve parolayı adrese değil, oturum açma alanlarına yazın.
dicomweb-save-failed = Sunucu listesi kaydedilemedi: { $reason }

# Arama sayfası.
dicomweb-search-title = Tetkik bul
dicomweb-field-patient-name = Hasta adı
dicomweb-field-patient-id = Hasta kimliği
dicomweb-field-date-from = Başlangıç
dicomweb-field-date-to = Bitiş
dicomweb-hint-date = YYYYAAGG
dicomweb-field-modality = Modalite
dicomweb-hint-modality = CT, MR…
dicomweb-field-accession = Erişim numarası
dicomweb-show-patient = Hasta verilerini göster
dicomweb-switch-on = Açık
dicomweb-switch-off = Kapalı
dicomweb-patient-shown = Hasta verileri gösteriliyor
dicomweb-patient-hidden = Hasta verileri gizli
dicomweb-studies = Tetkikler
dicomweb-search-hint = Alanlardan herhangi birini doldurup arayın.
dicomweb-no-studies = Eşleşen tetkik yok.
dicomweb-searching = Aranıyor…
dicomweb-stopped = Durduruldu
dicomweb-load-more = Daha fazla yükle
dicomweb-study-untitled = Açıklamasız tetkik
dicomweb-series-untitled = Açıklamasız seri
dicomweb-study-counts = { $series } seri, { $images } görüntü
dicomweb-series-images = { $count } görüntü
dicomweb-patient-id = Kimlik { $id }
dicomweb-accession = Erişim { $number }
dicomweb-found = { $count } tetkik bulundu
dicomweb-series-found = { $count } seri

# Sunucuyla iletişim hataları.
dicomweb-error-offline = { $server } sunucusuna ulaşılamıyor. Ağı ve adresi denetleyin.
dicomweb-error-auth = Sunucu kullanıcı adını, parolayı veya belirteci reddetti.
dicomweb-error-not-found = Bu adreste DICOMweb hizmeti yok.
dicomweb-error-other = Sunucu yanıt veremedi: { $reason }

# Görüntüleyiciye akışla aktarılan seri.
dicomweb-loading = { $series } yükleniyor…
dicomweb-loading-progress = { $series } yükleniyor: { $done } / { $total }
dicomweb-loaded = { $series }: { $total } görüntü yüklendi
dicomweb-loaded-failed = { $total } görüntüden { $failed } tanesi yüklenemedi.
dicomweb-offline-cached = { $server } sunucusuna ulaşılamıyor: daha önce kaydedilen görüntüler gösteriliyor.
dicomweb-cache-failed = İndirme klasörü açılamadı: { $reason }
