# machine-drafted, needs native review
# Specular — Develop's AI masks: subject, sky and object-at-click rows, and the
# Masks feature in Settings › Local AI.

develop-mask-kind-subject = Obiekt główny
develop-mask-kind-sky = Niebo
develop-mask-kind-object = Obiekt po kliknięciu
# An AI mask row while its network runs. $kind: subject, sky, object.
develop-ai-computing = { $kind ->
        [subject] Szukanie obiektu głównego…
        [sky] Szukanie nieba…
       *[object] Szukanie obiektu…
    }
develop-ai-ready = Gotowe
# An object row with its points ($n; a box counts as two).
develop-ai-object-clicks = Punkty: { $n }
develop-ai-click-help =
    Kliknij obiekt.
    Alt+klik na tym, co do niego nie należy.
    Przeciągnij, aby zaznaczyć ramkę.
# The row's model is not installed. $model is its name ("IS-Net").
develop-ai-needs =
    Wymaga dodatku { $model }.
    Do tego czasu nic nie maskuje.
develop-ai-failed = Nie udało się utworzyć maski: { $error }
develop-ai-failed-worker = Proces masek AI zatrzymał się.
develop-ai-open-settings = Otwórz lokalną AI…
develop-ai-clear = Wyczyść punkty
develop-ai-too-many = Najwyżej { $n } punktów (ramka liczy się jako dwa).
develop-ai-box-set = Ramka ustawiona
develop-ai-point-added = Dodano punkt
develop-ai-point-removed = Dodano punkt poza obiektem
develop-ai-cleared = Wyczyszczono punkty
# Screen-reader names of the points on the picture. $n counts from 1.
develop-ai-point-on = Punkt { $n }, na obiekcie
develop-ai-point-off = Punkt { $n }, poza obiektem

## Settings › Local AI: the Masks feature
settings-ai-masks = Maski AI
settings-ai-masks-help = Maski obiektu głównego, nieba i obiektów w Wywołaniu. Model obiektów należy do tej funkcji; obiekt główny i niebo to dodatki.
settings-ai-masks-size = Model
settings-ai-masks-size-help = EfficientSAM Ti jest szybszy. EfficientSAM S dokładniej trzyma się krawędzi.
settings-ai-masks-size-recommended = Domyślny
settings-ai-masks-size-small = EfficientSAM Ti
settings-ai-masks-size-standard = EfficientSAM S
settings-ai-masks-size-large = Duży
