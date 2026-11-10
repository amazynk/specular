# machine-drafted, needs native review
## Optional decoders: other vendors' free software that opens a few formats (Blackmagic RAW,
## DWG), used only when the user installed it. Specular never bundles or downloads it.
## $format is the format ("Blackmagic RAW", "DWG"), $decoder the decoder's own name
## ("Blackmagic RAW SDK", "ODA File Converter"), $vendor its maker ("Blackmagic Design",
## "Open Design Alliance"): names, left as they are.

## Settings › Optional decoders. The switches and paths are shown in the panel below
## "Look again", not as rows of their own.

settings-section-decoders = İsteğe bağlı kod çözücüler
settings-decoders-look-again = Yeniden ara
settings-decoders-look-again-help = O zamandan beri yüklenen kod çözücüleri arar.
    { -app-name }, penceresi her öne geldiğinde de arar.
settings-decoders-braw = Blackmagic RAW SDK kullan
settings-decoders-braw-help = Yüklüyse Blackmagic RAW klipleri onunla açılır.
settings-decoders-braw-path = Blackmagic RAW SDK
settings-decoders-braw-path-help = Yükleyicinin koyduğu yerde aranması için boş bırakın.
settings-decoders-dwg = ODA File Converter kullan
settings-decoders-dwg-help = Yüklüyse DWG çizimleri onunla açılır.
    Çizimin bir kopyası özel bir klasörde dönüştürülür, ardından bu klasör silinir.
settings-decoders-dwg-path = ODA File Converter programı
settings-decoders-dwg-path-help = Yükleyicinin koyduğu yerde aranması için boş bırakın.
settings-decoders-occt = OpenCASCADE kullan
settings-decoders-occt-help = Yüklüyse STEP ve IGES modelleri onunla açılır.
settings-decoders-occt-path = OpenCASCADE kitaplıkları
settings-decoders-occt-path-help = Sistemin kitaplıkları tuttuğu yerde aranmaları için boş bırakın.
# machine-drafted, needs native review
settings-decoders-czi = libCZI kullan
settings-decoders-czi-help = Zeiss CZI dosyaları, yüklüyse bununla açılır.
    { -app-name } uygulamasının yalıtılmış yardımcı işleminde, diğer dosyalarınıza erişmeden çalışır.
settings-decoders-czi-path = libCZI kitaplık klasörü
settings-decoders-czi-path-help = Sistemin kitaplıkları tuttuğu yerde aranması için boş bırakın.

## The panel: one block per decoder.

decoders-intro = Bazı biçimler, üreticilerinin ücretsiz yazılımıyla açılır.
    Bir kez yükleyin: { -app-name } onu bulur ve kullanır. Kendisi asla indirmez.
decoders-keywords = isteğe bağlı kod çözücüler üretici yazılım codec eklenti kod çözücü gerekir optional decoders vendor SDK codec plugin Blackmagic RAW BRAW RED R3D DWG ODA File Converter OpenCASCADE STEP IGES Zeiss CZI libCZI needs decoder
# A decoder's row: "Blackmagic RAW (.braw)". The tick box before it switches it on and off.
decoders-row = { $format } ({ $extensions })
# The row's value.
decoders-state-found = Bulundu
decoders-state-off = Kapalı
decoders-state-not-found = Yüklü değil
decoders-state-broken = Kullanılamıyor
decoders-state-refused = Mümkün değil
decoders-state-planned = Henüz yok
decoders-state-checking = Aranıyor…
# The row's lines.
decoders-uses = { $vendor } tarafından sunulan { $decoder } ile açılır.
decoders-not-found = Ücretsizdir. Yükleyin, ardından Yeniden ara düğmesine basın.
decoders-off = Kapalı: bu dosyalar açılmaz.
decoders-version = Sürüm { $version }.
decoders-r3d-why = RED’in lisansı, R3D dosyalarını yalnızca RED’in kendi koşullarıyla onun SDK’sı kullanılarak yapılmış programların okumasına izin verir.
    { -app-name } bu koşulları kabul edemez, bu yüzden R3D dosyaları açılmaz.
decoders-ecw-why = Hexagon lisansı ECW SDK’nın yalnızca onu birlikte dağıtan programlarca kullanılmasına izin verir, sizin kurduğunuz bir kopyaya değil.
    Bu yüzden ECW dosyaları açılmaz. ECW SDK ile oluşturulmuş JPEG 2000 dosyaları açılır.
decoders-mrsid-why = MrSID SDK lisansı onun yalnızca SDK’yı birlikte dağıtan programlarca kullanılmasına izin verir, sizin kurduğunuz bir kopyaya değil.
    Bu yüzden MrSID dosyaları açılmaz.
# machine-drafted, needs native review
# DJI radiometric JPEGs (drone thermal cameras): the photo and metadata open; the
# temperatures would need DJI's SDK.
decoders-dji-thermal-why = DJI’ın SDK lisansı onun yalnızca SDK’yı birlikte dağıtan programlarca kullanılmasına izin verir, sizin kurduğunuz bir kopyaya değil. Fotoğraf ve meta verileri yine de açılır.
decoders-occt-why = STEP ve IGES katıları OpenCASCADE gerektirir; { -app-name } uygulamasının bu sürümü onu kullanamaz.
# Rows with one button each, under a decoder.
decoders-get-row = Üreticinin indirme sayfası
decoders-get = Edin
decoders-locate-row = Başka bir yere mi yüklü?
decoders-locate = Konumu seç…
decoders-forget-row = Bunu elle seçtiniz.
decoders-forget = Unut…
# Asked before Forget… does anything.
decoders-forget-ask = Seçtiğiniz { $decoder } unutulsun mu?
decoders-forget-detail = { -app-name } onu yeniden kendisi arar. Diskten hiçbir şey silinmez.
decoders-forget-ok = Unut
# The title of the Locate… file dialog.
decoders-locate-title = { $decoder } konumunu seçin
# Toasts.
decoders-locate-wrong = Bu { $decoder } değil.
decoders-located = Seçtiğiniz { $decoder } kullanılıyor.
decoders-forgotten = { -app-name }, { $decoder } yazılımını yeniden kendisi arıyor.
decoders-still-missing = { $decoder } hâlâ bulunamadı.
decoders-now-found = { $decoder } bulundu.
decoders-looked = Kod çözücüler yeniden arandı.

## The card shown in place of a file that needs one. $file is the file's name.

decoders-card-title = { $format } için { $vendor } tarafından sunulan ücretsiz kod çözücü gerekir
decoders-card-what = { $file }, { $decoder } ile açılır. Ücretsizdir.
decoders-card-how = Yükleyin, sonra geri gelin: { -app-name } onu kendisi bulur.
decoders-card-get = Edin
decoders-card-installed = Yükledim
decoders-card-locate = Konumu seç…

## The gallery: the badge on such a file's thumbnail, and a line of its tooltip.

gallery-badge-needs-decoder = Kod çözücü gerekir
gallery-needs-decoder = { $vendor } tarafından sunulan ücretsiz { $decoder } ile açılır; bu yazılım yüklü değil.

## Why a file did not open, after "Can't open FILE:".

error-reason-needs-decoder = { $format } dosyaları { $vendor } tarafından sunulan ücretsiz { $decoder } gerektirir; bu yazılım yüklü değil.
