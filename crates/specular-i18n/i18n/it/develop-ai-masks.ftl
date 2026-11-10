# machine-drafted, needs native review
# Specular — Develop's AI masks: subject, sky and object-at-click rows, and the
# Masks feature in Settings › Local AI.

develop-mask-kind-subject = Soggetto
develop-mask-kind-sky = Cielo
develop-mask-kind-object = Oggetto al clic
# An AI mask row while its network runs. $kind: subject, sky, object.
develop-ai-computing = { $kind ->
        [subject] Ricerca del soggetto…
        [sky] Ricerca del cielo…
       *[object] Ricerca dell’oggetto…
    }
develop-ai-ready = Pronto
# An object row with its points ($n; a box counts as two).
develop-ai-object-clicks = Punti: { $n }
develop-ai-click-help =
    Fai clic sull’oggetto.
    Alt+clic su ciò che non ne fa parte.
    Trascina per un riquadro.
# The row's model is not installed. $model is its name ("IS-Net").
develop-ai-needs =
    Richiede il componente { $model }.
    Fino ad allora non maschera nulla.
develop-ai-failed = Impossibile creare questa maschera: { $error }
develop-ai-failed-worker = Il processo delle maschere IA si è fermato.
develop-ai-open-settings = Apri IA locale…
develop-ai-clear = Cancella punti
develop-ai-too-many = Al massimo { $n } punti (un riquadro conta come due).
develop-ai-box-set = Riquadro impostato
develop-ai-point-added = Punto aggiunto
develop-ai-point-removed = Punto aggiunto fuori dall’oggetto
develop-ai-cleared = Punti cancellati
# Screen-reader names of the points on the picture. $n counts from 1.
develop-ai-point-on = Punto { $n }, sull’oggetto
develop-ai-point-off = Punto { $n }, fuori dall’oggetto

## Settings › Local AI: the Masks feature
settings-ai-masks = Maschere IA
settings-ai-masks-help = Maschere di soggetto, cielo e oggetto in Sviluppo. Il modello per gli oggetti è incluso; soggetto e cielo sono componenti aggiuntivi.
settings-ai-masks-size = Modello
settings-ai-masks-size-help = EfficientSAM Ti è più veloce. EfficientSAM S segue i bordi con più precisione.
settings-ai-masks-size-recommended = Predefinito
settings-ai-masks-size-small = EfficientSAM Ti
settings-ai-masks-size-standard = EfficientSAM S
settings-ai-masks-size-large = Grande
