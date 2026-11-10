# machine-drafted, needs native review
# Specular — Develop's AI masks: subject, sky and object-at-click rows, and the
# Masks feature in Settings › Local AI.

develop-mask-kind-subject = Konu
develop-mask-kind-sky = Gökyüzü
develop-mask-kind-object = Tıklanan nesne
# An AI mask row while its network runs. $kind: subject, sky, object.
develop-ai-computing = { $kind ->
        [subject] Konu aranıyor…
        [sky] Gökyüzü aranıyor…
       *[object] Nesne aranıyor…
    }
develop-ai-ready = Hazır
# An object row with its points ($n; a box counts as two).
develop-ai-object-clicks = Noktalar: { $n }
develop-ai-click-help =
    Nesneye tıklayın.
    Ona ait olmayanı Alt+tıklayın.
    Çerçeve için sürükleyin.
# The row's model is not installed. $model is its name ("IS-Net").
develop-ai-needs =
    { $model } eklentisi gerekiyor.
    O zamana dek hiçbir şeyi maskelemez.
develop-ai-failed = Bu maske yapılamadı: { $error }
develop-ai-failed-worker = Yapay zekâ maskeleri işlemi durdu.
develop-ai-open-settings = Yerel yapay zekâyı aç…
develop-ai-clear = Noktaları temizle
develop-ai-too-many = En çok { $n } nokta (çerçeve iki sayılır).
develop-ai-box-set = Çerçeve ayarlandı
develop-ai-point-added = Nokta eklendi
develop-ai-point-removed = Nesnenin dışına nokta eklendi
develop-ai-cleared = Noktalar temizlendi
# Screen-reader names of the points on the picture. $n counts from 1.
develop-ai-point-on = Nokta { $n }, nesnenin üstünde
develop-ai-point-off = Nokta { $n }, nesnenin dışında

## Settings › Local AI: the Masks feature
settings-ai-masks = Yapay zekâ maskeleri
settings-ai-masks-help = Banyo’da konu, gökyüzü ve nesne maskeleri. Nesne modeli bu özellikle gelir; konu ve gökyüzü eklentidir.
settings-ai-masks-size = Model
settings-ai-masks-size-help = EfficientSAM Ti daha hızlıdır. EfficientSAM S kenarları daha yakından izler.
settings-ai-masks-size-recommended = Varsayılan
settings-ai-masks-size-small = EfficientSAM Ti
settings-ai-masks-size-standard = EfficientSAM S
settings-ai-masks-size-large = Büyük
