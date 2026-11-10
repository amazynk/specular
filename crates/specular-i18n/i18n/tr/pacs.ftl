# machine-drafted, needs native review

## Send to PACS (C-STORE, STOW-RS), DIMSE servers, teaching quiz and teaching case export.

## Servers: DIMSE fields and Verify.

dicomweb-field-kind = Tür
dicomweb-kind-web = DICOMweb
dicomweb-kind-dimse = DICOM (DIMSE)
dicomweb-field-host = Ana makine
dicomweb-hint-host = pacs.example.org
dicomweb-field-port = Bağlantı noktası
dicomweb-hint-port = 104 veya 11112
# The application entity title the server answers to.
dicomweb-field-ae = Sunucunun AE title değeri
dicomweb-hint-ae = PACS
# The application entity title Specular sends as.
dicomweb-field-our-ae = Bizim AE title
# machine-drafted, needs native review
dicomweb-field-tls = Güvenli bağlantı (TLS)
dicomweb-tls-on = Güvenli bağlantı açık
dicomweb-tls-off = Güvenli bağlantı kapalı
dicomweb-problem-host = Sunucunun ana makine adını veya adresini girin.
dicomweb-problem-port = Bağlantı noktası 1 ile 65535 arasında bir sayıdır.
dicomweb-problem-ae = AE title 1 ile 16 karakter arasındadır ve ters eğik çizgi içermez.
# machine-drafted, needs native review
dicomweb-dimse-note-open = DICOM (DIMSE) sunucularında arama yapılabilir, seriler açılabilir ve bu sunuculara gönderim yapılabilir.
# How series are fetched: C-GET (over our own connection) or C-MOVE (the server connects back).
# machine-drafted, needs native review
pacs-field-retrieve = Alma yöntemi
# machine-drafted, needs native review
pacs-retrieve-get = C-GET
# machine-drafted, needs native review
pacs-retrieve-move = C-MOVE
# C-MOVE: the address of this computer and the port the server sends to.
# machine-drafted, needs native review
pacs-field-receive-address = Alım adresi
# machine-drafted, needs native review
pacs-hint-receive-address = Bu bilgisayarın adresi, örneğin 192.168.1.20
# machine-drafted, needs native review
pacs-field-receive-port = Alım bağlantı noktası
# machine-drafted, needs native review
pacs-hint-receive-port = Örneğin 11113
# machine-drafted, needs native review
pacs-move-note = C-MOVE için sunucunun bizim { $ae } AE title değerimizi bu adres ve bağlantı noktasıyla tanıması gerekir.
    Specular orada yalnızca bir seri gelirken dinler.
# machine-drafted, needs native review
pacs-problem-receive-address = Alım için bu bilgisayarın adresini (örneğin 192.168.1.20) girin.
# machine-drafted, needs native review
pacs-problem-receive-port = Alım için bir bağlantı noktası girin. C-MOVE buna ihtiyaç duyar.
dicomweb-verify = Doğrula
dicomweb-verifying = Sunucu denetleniyor…
dicomweb-verify-ok = { $server } yanıt verdi.

## The Studies browser and the palette.

studies-send = PACS’e gönder…
studies-send-description = Bu seriyi, tetkikini veya diskteki her şeyi bir sunucuya gönderir.
palette-send-to-pacs = PACS’e gönder…
palette-send-to-pacs-help = Geçerli seriyi veya tetkiki bir PACS sunucusuna gönderir.

## The Send page.

pacs-title = PACS’e gönder
# Under the title: what is being sent.
pacs-summary = { $studies ->
    [one] { $studies } tetkik
   *[other] { $studies } tetkik
} · { $files ->
    [one] { $files } dosya
   *[other] { $files } dosya
}
pacs-what = Gönderilecek
pacs-scope-series = Bu seri ({ $files ->
    [one] { $files } dosya
   *[other] { $files } dosya
})
pacs-scope-study = Bu tetkik ({ $files ->
    [one] { $files } dosya
   *[other] { $files } dosya
})
pacs-scope-all = Okunan her şey ({ $studies ->
    [one] { $studies } tetkik
   *[other] { $studies } tetkik
}, { $files ->
    [one] { $files } dosya
   *[other] { $files } dosya
})
pacs-anonymise = Anonimleştir
pacs-anonymise-note = Gönderilen kopyalardan hasta bilgileri kaldırılır. Dosyalarınız değişmez.
pacs-identified-note = Dosyalar hasta bilgileriyle birlikte olduğu gibi gönderilir.
# The list of servers to choose from.
pacs-to = Alıcı
pacs-no-servers = Henüz sunucu yok. DICOMweb sunucuları bölümünden bir sunucu ekleyin.
pacs-send = Gönder…
pacs-go-back = Geri dön
pacs-confirm-send = Gönder
pacs-confirm = { $studies ->
    [one] { $studies } tetkik
   *[other] { $studies } tetkik
} ({ $files ->
    [one] { $files } dosya
   *[other] { $files } dosya
}) { $server } sunucusuna gönderilsin mi?
pacs-busy = Bir gönderim hâlâ sürüyor.
pacs-started = Gönderiliyor.
pacs-stopping = Durduruluyor…
pacs-sending = { $server } sunucusuna { $done } / { $total } gönderiliyor…
pacs-sent-all = { $total ->
    [one] { $total } dosya { $server } sunucusuna gönderildi.
   *[other] { $total } dosyanın tümü { $server } sunucusuna gönderildi.
}
pacs-sent-some = { $total } dosyadan { $sent } tanesi { $server } sunucusuna gönderildi.
pacs-stopped = { $total } dosyadan { $sent } tanesinden sonra durduruldu.
pacs-stopped-because = { $total } dosyadan { $sent } tanesinden sonra durduruldu: { $reason }
pacs-warnings = { $count ->
    [one] { $count } dosya bir uyarıyla depolandı.
   *[other] { $count } dosya bir uyarıyla depolandı.
}
# A file that was not sent: its file name and why.
pacs-failed-line = { $name }: { $reason }
pacs-failed-more = Ve { $count } tane daha.

## Why one file was not sent.

pacs-reason-unreadable = Okunamadı: { $reason }
pacs-reason-not-dicom = DICOM dosyası değil
pacs-reason-burned-in = Görüntüye gömülü hasta bilgileri içerdiği için anonimleştirilmiş olarak gönderilmez
pacs-reason-document = Anonimleştirilemeyen bir belge
pacs-reason-anonymise = Anonimleştirilemedi: { $reason }
pacs-reason-not-accepted = Sunucu { $class } kabul etmiyor
pacs-reason-convert = Sunucu için dönüştürülemedi: { $reason }
pacs-reason-resources = Sunucunun alanı veya kaynakları tükendi
pacs-reason-mismatch = Sunucuya göre veriler türleriyle uyuşmuyor
pacs-reason-understand = Sunucu bunu okuyamadı
pacs-reason-class = Sunucu bu tür nesneyi desteklemiyor
pacs-reason-not-authorised = Sunucu buna izin vermiyor
pacs-reason-status = Sunucu reddetti (durum { $code })
pacs-reason-refused = Sunucu reddetti
pacs-reason-too-large = Sunucu için çok büyük
pacs-reason-http = Sunucu reddetti (hata { $status })

## Why sending stopped, or Verify failed.

pacs-fatal-network-off = Ağ kapalı.
pacs-fatal-unknown-host = { $host } bilinen bir ana makine adı değil.
pacs-fatal-connect = { $host }, bağlantı noktası { $port } üzerinde yanıt veren yok.
pacs-fatal-unreachable = { $host } adresine ulaşılamıyor.
pacs-fatal-timeout = { $host } zamanında yanıt vermedi.
pacs-fatal-cancelled = Durduruldu.
pacs-fatal-called-ae = { $host }, { $ae } AE title değerine yanıt vermiyor. Sunucunun AE title değerini denetleyin.
pacs-fatal-calling-ae = { $host }, bizim { $ae } AE title değerimizi tanımıyor. Sunucuya ekleyin.
pacs-fatal-busy = { $host } meşgul. Daha sonra yeniden deneyin.
pacs-fatal-rejected = { $host } bağlantıyı reddetti.
pacs-fatal-aborted = { $host } bağlantıyı sonlandırdı.
pacs-fatal-nothing-accepted = { $host } bu nesne türlerinin hiçbirini kabul etmiyor.
pacs-fatal-protocol = { $host }, Specular’ın anlamadığı bir biçimde yanıt verdi.
pacs-fatal-auth = { $host } oturum açmayı kabul etmedi.
pacs-fatal-http = { $host } { $status } hatasıyla yanıt verdi.

## Teaching quiz.

palette-teaching-quiz = Eğitim sınavı
palette-teaching-quiz-help = Anahtar görüntülerde veya bir eğitim vakasında ilerler; bulgular siz gösterene kadar gizli kalır.
# "Case 2 of 5".
quiz-case = Vaka { $n } / { $total }
quiz-hint-hidden = Boşluk bulguları gösterir · → sonraki vaka · Esc bitirir
quiz-hint-revealed = Boşluk yeniden gizler · → sonraki vaka · Esc bitirir
quiz-hidden = Bulgular gizli.
quiz-no-findings = Kayıtlı bulgu yok.
quiz-notes = Vaka notları: { $notes }
quiz-ended = Sınav bitti.
quiz-empty = Gösterilecek vaka yok. Anahtar görüntüleri K ile işaretleyin veya bir eğitim vakası klasörü seçin.
quiz-not-a-case = Bu klasör, Specular tarafından dışa aktarılmış bir eğitim vakası değil.
quiz-pick-title = Eğitim vakası klasörü seçin

## Teaching case folder (Export key images).

figure-keys-case = Eğitim vakası klasörü
figure-case-notes = Vaka notları
figure-case-findings = Bulgular
figure-case-none = Kayıt yok
figure-case-readme = Specular tarafından dışa aktarılmış eğitim vakası. Hasta bilgileri kaldırıldı.
    images: anahtar görüntüler, gömülü metin karartılmış.
    DICOM: anonimleştirilmiş kopyalar ve "For Teaching" başlıklı bir Key Object Selection bildirimi.
    case.txt: vaka notları ve her görüntünün bulguları. case.json: betikler için aynısı.
    Radiopaedia için: DICOM klasörünü veya görüntüleri yükleyin ve notları case.txt dosyasından kopyalayın.

## Search on a DIMSE server (C-FIND).
# machine-drafted, needs native review
pacs-find-not-supported = { $host } aramalara (C-FIND) yanıt vermiyor.
pacs-find-status = { $host } aramayı reddetti (durum { $code }).

## Opening a series from a DIMSE server (C-GET, C-MOVE).
# machine-drafted, needs native review
pacs-retrieve-no-get = { $host }, serileri C-GET ile göndermiyor.
    Sunucunun ayarlarında C-MOVE seçin.
# machine-drafted, needs native review
pacs-retrieve-no-move = { $host }, C-MOVE isteklerini kabul etmiyor.
# machine-drafted, needs native review
pacs-retrieve-unknown-destination = { $host }, bizim { $ae } AE title değerimizi tanımıyor.
    Bunu sunucuya bu bilgisayarın adresi ve { $port } bağlantı noktasıyla ekleyin.
# machine-drafted, needs native review
pacs-retrieve-resources = { $host } kaynaklarını tüketti. Daha sonra yeniden deneyin.
# machine-drafted, needs native review
pacs-retrieve-status = { $host } seriyi göndermeyi reddetti (durum { $code }).
# machine-drafted, needs native review
pacs-retrieve-no-matches = { $host } sunucusunda bu seri için görüntü yok.
# machine-drafted, needs native review
pacs-retrieve-roles = { $host }, bu bağlantı üzerinden görüntü göndermeyi reddetti.
    Sunucunun ayarlarında C-MOVE seçin.
# machine-drafted, needs native review
pacs-retrieve-move-nothing = { $host } görüntüleri bu bilgisayara gönderemedi.
    Adresi, bağlantı noktasını ve güvenlik duvarını, ayrıca sunucunun bizim { $ae } AE title değerimizi tanıyıp tanımadığını denetleyin.
# machine-drafted, needs native review
pacs-retrieve-nothing = { $host }, { $count ->
    [one] görüntüyü
   *[other] { $count } görüntünün hiçbirini
} Specular’ın okuyabileceği bir biçimde gönderemedi.
# machine-drafted, needs native review
pacs-retrieve-listen = { $address } adresinde, { $port } bağlantı noktasında alım yapılamıyor: { $reason }

## DICOM over TLS.
# machine-drafted, needs native review
pacs-fatal-tls-certificate = { $host } sertifikasına güvenilmiyor.
    Sertifikayı verenin sistemin güvenilir sertifikaları arasında olması gerekir.
pacs-fatal-tls = { $host } ile güvenli bağlantı yok.
    { $port } bağlantı noktasının TLS üzerinden DICOM kullandığını denetleyin.
