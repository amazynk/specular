# machine-drafted, needs native review
# Specular — Develop's AI masks: subject, sky and object-at-click rows, and the
# Masks feature in Settings › Local AI.

develop-mask-kind-subject = Motiv
develop-mask-kind-sky = Himmel
develop-mask-kind-object = Objekt ved klik
# An AI mask row while its network runs. $kind: subject, sky, object.
develop-ai-computing = { $kind ->
        [subject] Finder motivet…
        [sky] Finder himlen…
       *[object] Finder objektet…
    }
develop-ai-ready = Klar
# An object row with its points ($n; a box counts as two).
develop-ai-object-clicks = Punkter: { $n }
develop-ai-click-help =
    Klik på objektet.
    Alt+klik på det, der ikke hører til.
    Træk for en ramme.
# The row's model is not installed. $model is its name ("IS-Net").
develop-ai-needs =
    Kræver tilføjelsen { $model }.
    Maskerer intet indtil da.
develop-ai-failed = Kunne ikke lave masken: { $error }
develop-ai-failed-worker = Processen for AI-masker stoppede.
develop-ai-open-settings = Åbn lokal AI…
develop-ai-clear = Ryd punkter
develop-ai-too-many = Højst { $n } punkter (en ramme tæller som to).
develop-ai-box-set = Ramme sat
develop-ai-point-added = Punkt tilføjet
develop-ai-point-removed = Punkt tilføjet uden for objektet
develop-ai-cleared = Punkter ryddet
# Screen-reader names of the points on the picture. $n counts from 1.
develop-ai-point-on = Punkt { $n }, på objektet
develop-ai-point-off = Punkt { $n }, uden for objektet

## Settings › Local AI: the Masks feature
settings-ai-masks = AI-masker
settings-ai-masks-help = Motiv-, himmel- og objektmasker i Fremkald. Objektmodellen følger med funktionen; motiv og himmel er tilføjelser.
settings-ai-masks-size = Model
settings-ai-masks-size-help = EfficientSAM Ti er hurtigere. EfficientSAM S følger kanter tættere.
settings-ai-masks-size-recommended = Standard
settings-ai-masks-size-small = EfficientSAM Ti
settings-ai-masks-size-standard = EfficientSAM S
settings-ai-masks-size-large = Stor
