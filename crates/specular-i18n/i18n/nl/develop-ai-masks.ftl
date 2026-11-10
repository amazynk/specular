# machine-drafted, needs native review
# Specular — Develop's AI masks: subject, sky and object-at-click rows, and the
# Masks feature in Settings › Local AI.

develop-mask-kind-subject = Onderwerp
develop-mask-kind-sky = Lucht
develop-mask-kind-object = Object bij klik
# An AI mask row while its network runs. $kind: subject, sky, object.
develop-ai-computing = { $kind ->
        [subject] Onderwerp zoeken…
        [sky] Lucht zoeken…
       *[object] Object zoeken…
    }
develop-ai-ready = Klaar
# An object row with its points ($n; a box counts as two).
develop-ai-object-clicks = Punten: { $n }
develop-ai-click-help =
    Klik op het object.
    Alt+klik op wat er niet bij hoort.
    Sleep voor een kader.
# The row's model is not installed. $model is its name ("IS-Net").
develop-ai-needs =
    Heeft de add-on { $model } nodig.
    Maskeert tot dan niets.
develop-ai-failed = Kon dit masker niet maken: { $error }
develop-ai-failed-worker = Het proces voor AI-maskers is gestopt.
develop-ai-open-settings = Lokale AI openen…
develop-ai-clear = Punten wissen
develop-ai-too-many = Hoogstens { $n } punten (een kader telt als twee).
develop-ai-box-set = Kader gezet
develop-ai-point-added = Punt toegevoegd
develop-ai-point-removed = Punt buiten het object toegevoegd
develop-ai-cleared = Punten gewist
# Screen-reader names of the points on the picture. $n counts from 1.
develop-ai-point-on = Punt { $n }, op het object
develop-ai-point-off = Punt { $n }, buiten het object

## Settings › Local AI: the Masks feature
settings-ai-masks = AI-maskers
settings-ai-masks-help = Maskers voor onderwerp, lucht en object in Ontwikkelen. Het objectmodel hoort bij deze functie; onderwerp en lucht zijn add-ons.
settings-ai-masks-size = Model
settings-ai-masks-size-help = EfficientSAM Ti is sneller. EfficientSAM S volgt randen nauwkeuriger.
settings-ai-masks-size-recommended = Standaard
settings-ai-masks-size-small = EfficientSAM Ti
settings-ai-masks-size-standard = EfficientSAM S
settings-ai-masks-size-large = Groot
