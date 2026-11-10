# Specular — Local AI pack (DESIGN.md "Local AI pack", §10). Turkish. Machine-drafted, needs native review.

## Settings → Local AI
settings-section-ai = Yerel yapay zekâ
settings-ai-pack = Yerel yapay zekâ paketi
# machine-drafted, needs native review
settings-ai-pack-help = Yalnızca bu bilgisayarda çalışan isteğe bağlı özellikler. Buldukları hiçbir şey bilgisayardan çıkmaz. Her özellik için bir model seçin; siz seçene kadar hiçbir şey indirilmez.
# machine-drafted, needs native review
settings-ai-install-from-file = Dosyadan yükle…
settings-ai-install-from-file-help = İnterneti olmayan makineler için bu bilgisayara kopyaladığınız model dosyalarını yükleyin. Her dosya { -app-name } ile gelen listeye göre denetlenir.
settings-ai-install-from-folder = Klasörden yükle…
settings-ai-install-from-folder-help = Aynısı, bir klasördeki her model dosyası için.
settings-ai-search = Açıklamayla ara
settings-ai-search-help = Fotoğrafları “sahilde köpek” gibi tarif ederek bulun ve benzer kareleri daha iyi gruplayın.
settings-ai-text = Görüntülerdeki metin
settings-ai-text-help = Fotoğraf ve taramalardaki metni okur, böylece text: araması onları bulur.
settings-ai-culling = Ayıklama yardımı
# machine-drafted, needs native review
settings-ai-culling-help = Bir seri çekimin en iyi karesini seçmeye yardımcı olmak için kapalı gözleri, bulanıklığı ve kötü pozlamayı işaretler.
settings-ai-people = Kişiler
# machine-drafted, needs native review
settings-ai-people-help = Yüzleri bulur ve people: araması için kişilere göre gruplar. Gruplama, bir kitaplık için açana kadar kapalıdır. Yüz verileri tek bir kişi için veya hepsi için bir kerede silinebilir.

# The pack's and a feature's state. $percent is formatted ("25%"); $done and $total are
# formatted numbers.
ai-state = { $state ->
        [none] Bu sürümde kullanılabilir model yok.
        [installed] Yüklü
        [indexing] Yüklü · { $done }/{ $total } görüntü dizinlendi
        [downloading] İndiriliyor… { $percent }
        [failed] İndirme başarısız oldu
       *[available] Yüklü değil
    }
ai-removing = Kaldırılıyor…
# machine-drafted, needs native review
# Buttons of the Local AI page.
ai-button = { $action ->
        [download] İndir
        [cancel] İptal
        [remove] Kaldır
        [forget] Unut
        [faces] Tüm yüz verilerini sil
       *[pack] Paketi kaldır
    }
# The question before deleting.
ai-confirm = { $action ->
        [faces] Tüm yüz verileri silinsin mi? Bulunan tüm yüzler ve kişiler silinir. Fotoğraflarınız kalır.
       *[pack] Yerel yapay zekâ paketi, modelleri ve bulduğu her şey kaldırılsın mı? Fotoğraflarınız değiştirilmez.
    }

## Gallery search
gallery-query-needs-ai = { $field }: yerel yapay zekâ paketi gerekir (Ayarlar › Yerel yapay zekâ)
# machine-drafted, needs native review
gallery-query-ai-not-yet = { $field }: yerel yapay zekâ paketi bunu henüz arayamıyor

## Faces: People view, face boxes and culling flags
# machine-drafted, needs native review
# "Show faces" on the picture.
faces-shown = Yüzler gösteriliyor
# machine-drafted, needs native review
faces-hidden = Yüzler gizlendi
# machine-drafted, needs native review
# Beside a face box whose eyes look closed.
faces-eyes-closed = Gözler kapalı
# machine-drafted, needs native review
# Culling flags on the cull mode strip.
cull-flag-eyes-closed = Gözler kapalı
# machine-drafted, needs native review
cull-flag-eyes-closed-some = Gözler kapalı: { $faces } yüzden { $closed } tanesi
# machine-drafted, needs native review
cull-flag-blurred = Bulanık
# machine-drafted, needs native review
cull-flag-dark = Çok karanlık
# machine-drafted, needs native review
cull-flag-bright = Çok parlak
# machine-drafted, needs native review
cull-flag-clipped = Kırpılmış açık tonlar veya gölgeler
# machine-drafted, needs native review
# The People view.
people-title = Kişiler
# machine-drafted, needs native review
people-open = Kişiler
# machine-drafted, needs native review
people-done = Bitti
# machine-drafted, needs native review
people-back = Geri
# machine-drafted, needs native review
people-show-photos = Fotoğrafları göster
# machine-drafted, needs native review
people-name = Adlandır…
# machine-drafted, needs native review
people-name-prompt = Bu kişiyi adlandır
# machine-drafted, needs native review
people-faces-open = Yüzler
# machine-drafted, needs native review
people-faces-of = Yüzler: { $name }
# machine-drafted, needs native review
people-merge = Birleştir
# machine-drafted, needs native review
people-merged = { $count } kişi birleştirildi
# machine-drafted, needs native review
people-merge-ask = { $from }, { $into } ile birleştirilsin mi?
# machine-drafted, needs native review
people-merge-ask-many = { $count } kişi { $into } ile birleştirilsin mi?
# machine-drafted, needs native review
people-merge-ask-detail = Yüzleri { $into } kişisine geçer. Diskteki fotoğraflar değişmez.
# machine-drafted, needs native review
people-not-this = Bu kişi değil
# machine-drafted, needs native review
people-face-removed = Yüz bu kişiden çıkarıldı. Fotoğraf yerinde kalır.
# machine-drafted, needs native review
# Someone not named yet.
people-unnamed = Kişi #{ $id }
# machine-drafted, needs native review
people-face-count = { $count ->
        [one] { $count } yüz
       *[other] { $count } yüz
    }
# machine-drafted, needs native review
people-count = { $count ->
        [one] { $count } kişi
       *[other] { $count } kişi
    }
# machine-drafted, needs native review
people-face-a11y = Yüz { $index }
# machine-drafted, needs native review
people-loading = Yükleniyor…
# machine-drafted, needs native review
people-not-installed = Kişiler yüklü değil. Ayarlar › Yerel yapay zekâ bölümünden indirin.
# machine-drafted, needs native review
people-grouping-off = Bu kitaplık için yüz gruplama kapalı. Ayarlar › Yerel yapay zekâ bölümünde “Yüzleri kişilere göre grupla” seçeneğini açın.
# machine-drafted, needs native review
people-none = Henüz kişi bulunamadı.

# machine-drafted, needs native review
# A feature's model: Default (the model Specular ships as the default, or the nearest one
# this computer can run) or a model by name, which stays that model on any computer. The
# page shows it only for a feature with more than one model; the dropdown labels Default
# with the model it means (ai-size-recommended-option) and names each model as the model
# list does. The option names below are the palette's and the command line's: model names
# are proper names, kept as they are.
settings-ai-search-size = Model
settings-ai-search-size-help = SigLIP 2 B/16 fotoğrafları en iyi bulur, birçok dilde. B/32 yaklaşık dört kat daha hızlı dizinler; daha yavaş ekran kartları içindir.
settings-ai-search-size-recommended = Varsayılan
# machine-drafted, needs native review
settings-ai-search-size-small = SigLIP 2 B/32
settings-ai-search-size-standard = PE Core S16
settings-ai-search-size-large = SigLIP 2 B/16
settings-ai-text-size = Model
settings-ai-text-size-help = PP-OCRv6 small daha doğru okur; tiny daha hızlıdır.
settings-ai-text-size-recommended = Varsayılan
settings-ai-text-size-small = PP-OCRv6 tiny
settings-ai-text-size-standard = PP-OCRv6 small
settings-ai-text-size-large = Büyük
# machine-drafted, needs native review
settings-ai-culling-size = Model
settings-ai-culling-size-help = Kapalı gözleri bulan model.
settings-ai-culling-size-recommended = Varsayılan
settings-ai-culling-size-small = Küçük
# machine-drafted, needs native review
settings-ai-culling-size-standard = Face Landmarker
settings-ai-culling-size-large = Büyük
# machine-drafted, needs native review
settings-ai-people-size = Model
settings-ai-people-size-help = SFace küçüktür. AuraFace daha büyüktür ve yayıncısının ticari kullanım için lisanslı dediği verilerle eğitilmiştir.
settings-ai-people-size-recommended = Varsayılan
settings-ai-people-size-small = Küçük
# machine-drafted, needs native review
settings-ai-people-size-standard = BlazeFace + SFace
settings-ai-people-size-large = BlazeFace + AuraFace
# A model size by name.
ai-tier = { $tier ->
        [small] Küçük
        [large] Büyük
       *[standard] Standart
    }
# machine-drafted, needs native review
# Default in the model dropdown, and the label of the "Back to default" row: with the model
# it means on this computer, "Default (SigLIP 2 B/16)". $size is the model's name.
ai-size-recommended-option = Varsayılan ({ $size })
# Under the model setting: the default, or the nearest model this computer can run. $gpu is
# the graphics card's name.
ai-recommended-for = { $gpu } için varsayılan: { $size }
ai-recommended = Bu bilgisayar için varsayılan: { $size }
# Nothing fits: no size can run here.
ai-recommended-none = Bu bilgisayarın belleğine sığan bir boyut yok.
# Under an installed feature: what its model runs on now. $gpu is the graphics card's name
# (used with "gpu" only). The CPU variants say why the graphics card is not used.
ai-runs-on = { $engine ->
        [gpu] Çalıştığı yer: GPU ({ $gpu })
        [gpu-unnamed] Çalıştığı yer: GPU
        [no-gpu] Çalıştığı yer: CPU (GPU bulunamadı)
        [differed] Çalıştığı yer: CPU — GPU sonucu CPU’nunkinden farklıydı
        [failed] Çalıştığı yer: CPU — model GPU’da başarısız oldu
        [setting] Çalıştığı yer: CPU (işleme “Yalnızca CPU” olarak ayarlı)
        [checking] Çalıştığı yer: CPU — GPU buna göre denetleniyor
       *[cpu] Çalıştığı yer: CPU
    }
# A size's row: its files on disk ($place "disk"), or what it would download, the memory it
# needs, and " · needs the GPU" only for a size that cannot run without one. $bytes and
# $memory are formatted sizes.
ai-size-detail = { $place ->
        [disk] diskte { $bytes }
       *[download] { $bytes } indirme
    } · { $memory } bellek{ $gpu ->
        [yes] { " · GPU gerektirir" }
       *[no] { "" }
    }
# machine-drafted, needs native review
# Under a size that may not run on this computer (it is still shown, and choosing it asks
# first). "gpu": it needs a graphics card this computer does not have or cannot use;
# "memory": $need is the memory it needs, $have what this computer has for it (formatted
# sizes, "6 GB").
ai-size-unfit = { $why ->
        [gpu] GPU gerektirir ve bu bilgisayarda kullanılabilecek bir GPU yok.
       *[memory] { $need } bellek gerektirir; bu, Specular'ın bu bilgisayarda yapay zekâ için izin verdiği { $have } miktarından fazla.
    }
# Over the size setting while it names a size other than the one installed and in use
# ($current, $chosen: ai-tier). The rows below have the buttons to change it.
ai-size-not-in-use = Kullanımda olan { $current }, { $chosen } değil.
# machine-drafted, needs native review
# Buttons of a size's row and of the size question: "use" chooses that size (one not
# downloaded; a downloaded one says Switch, ai-size-button), "back" returns the setting to
# Recommended, "anyway" keeps a size that may not run on this computer (ai-unfit-question).
# The screen reader adds the size's name to each.
ai-size-action = { $action ->
        [back] Varsayılana dön
        [anyway] Yine de kullan
       *[use] Bu boyutu kullan
    }
# The size the setting chose is not installed while another one is ($current, ai-tier).
ai-size-chosen-missing = Seçildi ancak henüz indirilmedi: { $current } kullanılmaya devam ediyor.
# What a size is now: from what is installed, never from the setting. "downloaded": its files
# are here but another size is in use; "missing": the chosen size, not downloaded.
ai-size-state = { $state ->
        [current] Kullanımda
        [downloaded] İndirildi, kullanımda değil
        [missing] İndirilmedi
        [previous] Yeni boyut kitaplığınızı dizinleyene kadar kullanımda
        [target] İndiriliyor…
        [failed] İndirme başarısız oldu
       *[other] { "" }
    }
# A licence row's button (opens the licence's page).
ai-licence = Lisansı görüntüle
# Asked before a size change that downloads and re-indexes. $feature is the feature's name,
# $size the new size (ai-tier), $download a formatted size.
ai-switch-question = { $download ->
        [none] { $feature }: { $size } boyutuna geçilsin mi? Kitaplığınız yeniden dizinlenir; o zamana kadar geçerli boyut kullanılmaya devam eder.
       *[other] { $feature }: { $size } boyutuna geçilsin mi? Bu işlem { $download } indirir ve kitaplığınızı yeniden dizinler; o zamana kadar geçerli boyut kullanılmaya devam eder.
    }
# machine-drafted, needs native review
# A size that may not run on this computer, chosen for a feature not installed yet: asked
# before the setting keeps it (ai-size-unfit says why, on the next line). $size is ai-tier,
# $feature the feature's name.
ai-unfit-question = { $feature }: { $size } yine de kullanılsın mı? Bu bilgisayarda çalışmayabilir ya da yalnızca çok yavaş çalışabilir.
# The recommendation moved: offered, never applied silently.
ai-suggest-question = { $feature }: { $size } boyutu artık bu bilgisayara daha uygun. Geçilsin mi? { $download ->
        [none] Kitaplığınız yeniden dizinlenir.
       *[other] Bu işlem { $download } indirir ve kitaplığınızı yeniden dizinler.
    }
ai-suggest-toast = Yerel yapay zekâ: başka bir model boyutu bu bilgisayara daha uygun (Ayarlar › Yerel yapay zekâ).
# A feature switching size: the new size ($size, ai-tier) indexes the library while the old one
# stays in use. $done and $total are formatted numbers.
ai-switching = { $size } boyutuna geçiliyor · { $done }/{ $total } görüntü dizinlendi

## The model manager: what each model says besides its size and licence

# machine-drafted, needs native review
# A model or feature not installable yet.
ai-coming = Sonraki bir sürümde gelecek
# machine-drafted, needs native review
# What a model reads. $languages: multilingual, english, or a script id (OCR packs).
ai-choice-languages = { $languages ->
        [multilingual] Birçok dil
        [english] Yalnızca İngilizce
        [cjk-latin] Çince, Japonca ve Latin alfabesi
        [zh-latin] Çince ve Latin alfabesi
        [arabic] Arapça
        [hebrew] İbranice
        [korean] Korece
        [cyrillic] Kiril alfabesi
        [greek] Yunanca
        [thai] Tayca
        [devanagari] Devanagari
        [tamil] Tamilce
        [telugu] Telugu dili
        [eslav] Rusça, Ukraynaca ve Belarusça
        [latin] Latin alfabesi (birçok Avrupa dili)
       *[other] { $languages }
    }
# A hint about a model. $hint: faster, closed-eyes, small-faces.
ai-choice-hint = { $hint ->
        [faster] Daha hızlı, daha yavaş ekran kartları için
        [closed-eyes] Kapalı gözleri bulur
        [small-faces] Küçük yüzlerde daha iyi
        [no-translate] İngilizceye çeviremez
       *[other] { "" }
    }
# A model's training data. $source: the data as its publisher names it ($named is "no" when
# it is not named). $terms: whether the publisher states the data's terms ("stated"), does
# not ("unstated"), or the data is licensed for research only ("research").
ai-choice-data = { $terms ->
        [stated] Eğitim verisi: { $source }
        [research] Eğitim verisi: { $source } (yalnızca araştırma için lisanslı)
       *[unstated] { $named ->
            [no] Eğitim verisi: adı verilmemiş, koşullar belirtilmemiş
           *[yes] Eğitim verisi: { $source }, koşullar belirtilmemiş
        }
    }
# Where a model downloads from. $host is a host name ("huggingface.co").
ai-choice-host = { $host } adresinden indirilir
# A model still coming, in a feature's row. $size is formatted, $licence an SPDX name.
ai-choice-coming = { $name }: { $size }, { $licence }
# People: grouping faces into people, per library.
ai-grouping = Yüzleri kişilere göre grupla
ai-grouping-help = Bu kitaplık için açana kadar kapalıdır. Kapatmak gruplamayı durdurur; zaten gruplanmış yüzler siz silene kadar kalır.
ai-grouping-state = { $on ->
        [yes] Bu kitaplıkta açık
       *[no] Kapalı
    }
ai-grouping-button = { $on ->
        [yes] Kapat
       *[no] Aç
    }
# machine-drafted, needs native review
# An add-on (a script's text recogniser) before its feature is downloaded. $feature is the
# feature's name ("Text in photos").
ai-addon-first = Önce { $feature } özelliğini indirin.
# machine-drafted, needs native review
# Asked before an add-on is removed. $name is the model's name ("PP-OCRv5 Arabic").
ai-addon-remove-title = { $name } kaldırılsın mı?
# machine-drafted, needs native review
ai-addon-remove-detail = Model dosyaları silinir. Fotoğraflarınız ve zaten bulunan metin kalır.
# machine-drafted, needs native review
# Text: reading the text in this library's photos, per library.
ai-text-indexing = Bu kitaplıktaki fotoğraflarda metni oku
# machine-drafted, needs native review
ai-text-indexing-help = Bu kitaplık için açana kadar kapalıdır. Sonra Kitaplık aramasındaki text:, fotoğrafları içlerindeki sözcüklere göre bulur.
# machine-drafted, needs native review
ai-text-indexing-state = { $on ->
        [yes] Bu kitaplıkta açık
       *[no] Kapalı
    }

## Dosyadan yükleme

# machine-drafted, needs native review
# After model files were copied in.
ai-sideload-ready = { $count ->
        [one] Bir model dosyası kopyalandı. Yüklemek için İndir’e basın.
       *[other] { $count } model dosyası kopyalandı. Yüklemek için İndir’e basın.
    }
ai-sideload-none = Model dosyası bulunamadı.
# $file is a file name, or several separated by commas.
ai-sideload-unknown = { $file }, { -app-name } uygulamasının tanıdığı bir model değil. Kopyalanmadı.
ai-sideload-failed = { $file } kopyalanamadı: { $reason }
# Asked before using a file whose SHA-256 isn't in the shipped list. $model is the model it
# would be used as ("clip-vit-b32-1").
ai-unverified-title = Doğrulanmamış bir model kullanılsın mı?
ai-unverified-detail = { $file }, { -app-name } ile gelen modeller listesinde yok, bu yüzden denetlenemiyor. { $model } olarak kullanılacak.
ai-unverified-use = Doğrulanmamış modeli kullan

# machine-drafted, needs native review
ai-failed-internal = { -app-name } içinde bir şeyler ters gitti. Ayrıntılar günlükte.
# The buttons under a model-size question: switch, or keep the size in use.
ai-size-button = { $action ->
        [switch] Geç
       *[keep] Geçerli boyutu koru
    }
