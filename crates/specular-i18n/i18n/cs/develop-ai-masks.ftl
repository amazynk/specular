# machine-drafted, needs native review
# Specular — Develop's AI masks: subject, sky and object-at-click rows, and the
# Masks feature in Settings › Local AI.

develop-mask-kind-subject = Hlavní objekt
develop-mask-kind-sky = Obloha
develop-mask-kind-object = Objekt kliknutím
# An AI mask row while its network runs. $kind: subject, sky, object.
develop-ai-computing = { $kind ->
        [subject] Hledá se hlavní objekt…
        [sky] Hledá se obloha…
       *[object] Hledá se objekt…
    }
develop-ai-ready = Hotovo
# An object row with its points ($n; a box counts as two).
develop-ai-object-clicks = Body: { $n }
develop-ai-click-help =
    Klepněte na objekt.
    Alt+klepněte na to, co k němu nepatří.
    Tažením nakreslíte rámeček.
# The row's model is not installed. $model is its name ("IS-Net").
develop-ai-needs =
    Potřebuje doplněk { $model }.
    Do té doby nic nemaskuje.
develop-ai-failed = Masku nelze vytvořit: { $error }
develop-ai-failed-worker = Proces masek AI se zastavil.
develop-ai-open-settings = Otevřít místní AI…
develop-ai-clear = Smazat body
develop-ai-too-many = Nejvýše { $n } bodů (rámeček se počítá za dva).
develop-ai-box-set = Rámeček nastaven
develop-ai-point-added = Bod přidán
develop-ai-point-removed = Bod přidán mimo objekt
develop-ai-cleared = Body smazány
# Screen-reader names of the points on the picture. $n counts from 1.
develop-ai-point-on = Bod { $n }, na objektu
develop-ai-point-off = Bod { $n }, mimo objekt

## Settings › Local AI: the Masks feature
settings-ai-masks = Masky AI
settings-ai-masks-help = Masky hlavního objektu, oblohy a objektů ve Vyvolání. Model objektů je součástí funkce; hlavní objekt a obloha jsou doplňky.
settings-ai-masks-size = Model
settings-ai-masks-size-help = EfficientSAM Ti je rychlejší. EfficientSAM S přesněji sleduje hrany.
settings-ai-masks-size-recommended = Výchozí
settings-ai-masks-size-small = EfficientSAM Ti
settings-ai-masks-size-standard = EfficientSAM S
settings-ai-masks-size-large = Velký
