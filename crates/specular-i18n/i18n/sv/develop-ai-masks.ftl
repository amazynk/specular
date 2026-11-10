# machine-drafted, needs native review
# Specular — Develop's AI masks: subject, sky and object-at-click rows, and the
# Masks feature in Settings › Local AI.

develop-mask-kind-subject = Motiv
develop-mask-kind-sky = Himmel
develop-mask-kind-object = Objekt vid klick
# An AI mask row while its network runs. $kind: subject, sky, object.
develop-ai-computing = { $kind ->
        [subject] Söker motivet…
        [sky] Söker himlen…
       *[object] Söker objektet…
    }
develop-ai-ready = Klar
# An object row with its points ($n; a box counts as two).
develop-ai-object-clicks = Punkter: { $n }
develop-ai-click-help =
    Klicka på objektet.
    Alt+klicka på det som inte hör till.
    Dra för en ram.
# The row's model is not installed. $model is its name ("IS-Net").
develop-ai-needs =
    Behöver tillägget { $model }.
    Maskerar inget tills dess.
develop-ai-failed = Kunde inte göra masken: { $error }
develop-ai-failed-worker = Processen för AI-masker stoppade.
develop-ai-open-settings = Öppna lokal AI…
develop-ai-clear = Rensa punkter
develop-ai-too-many = Högst { $n } punkter (en ram räknas som två).
develop-ai-box-set = Ram satt
develop-ai-point-added = Punkt tillagd
develop-ai-point-removed = Punkt tillagd utanför objektet
develop-ai-cleared = Punkter rensade
# Screen-reader names of the points on the picture. $n counts from 1.
develop-ai-point-on = Punkt { $n }, på objektet
develop-ai-point-off = Punkt { $n }, utanför objektet

## Settings › Local AI: the Masks feature
settings-ai-masks = AI-masker
settings-ai-masks-help = Motiv-, himmel- och objektmasker i Framkalla. Objektmodellen ingår i funktionen; motiv och himmel är tillägg.
settings-ai-masks-size = Modell
settings-ai-masks-size-help = EfficientSAM Ti är snabbare. EfficientSAM S följer kanter närmare.
settings-ai-masks-size-recommended = Standard
settings-ai-masks-size-small = EfficientSAM Ti
settings-ai-masks-size-standard = EfficientSAM S
settings-ai-masks-size-large = Stor
