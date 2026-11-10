# machine-drafted, needs native review
# Specular — Develop's AI masks: subject, sky and object-at-click rows, and the
# Masks feature in Settings › Local AI.

develop-mask-kind-subject = Motiv
develop-mask-kind-sky = Himmel
develop-mask-kind-object = Objekt per Klick
# An AI mask row while its network runs. $kind: subject, sky, object.
develop-ai-computing = { $kind ->
        [subject] Motiv wird gesucht…
        [sky] Himmel wird gesucht…
       *[object] Objekt wird gesucht…
    }
develop-ai-ready = Fertig
# An object row with its points ($n; a box counts as two).
develop-ai-object-clicks = Punkte: { $n }
develop-ai-click-help =
    Klicken Sie auf das Objekt.
    Alt+Klick auf das, was nicht dazugehört.
    Ziehen zeichnet einen Rahmen.
# The row's model is not installed. $model is its name ("IS-Net").
develop-ai-needs =
    Benötigt das Add-on { $model }.
    Maskiert bis dahin nichts.
develop-ai-failed = Diese Maske konnte nicht erstellt werden: { $error }
develop-ai-failed-worker = Der KI-Masken-Prozess wurde beendet.
develop-ai-open-settings = Lokale KI öffnen…
develop-ai-clear = Punkte entfernen
develop-ai-too-many = Höchstens { $n } Punkte (ein Rahmen zählt als zwei).
develop-ai-box-set = Rahmen gesetzt
develop-ai-point-added = Punkt hinzugefügt
develop-ai-point-removed = Punkt außerhalb des Objekts hinzugefügt
develop-ai-cleared = Punkte entfernt
# Screen-reader names of the points on the picture. $n counts from 1.
develop-ai-point-on = Punkt { $n }, auf dem Objekt
develop-ai-point-off = Punkt { $n }, außerhalb des Objekts

## Settings › Local AI: the Masks feature
settings-ai-masks = KI-Masken
settings-ai-masks-help = Motiv-, Himmel- und Objektmasken in Entwickeln. Das Objektmodell gehört zu dieser Funktion; Motiv und Himmel sind Add-ons.
settings-ai-masks-size = Modell
settings-ai-masks-size-help = EfficientSAM Ti ist schneller. EfficientSAM S folgt Kanten genauer.
settings-ai-masks-size-recommended = Standard
settings-ai-masks-size-small = EfficientSAM Ti
settings-ai-masks-size-standard = EfficientSAM S
settings-ai-masks-size-large = Groß
