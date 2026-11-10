# machine-drafted, needs native review
# Specular — Develop's AI masks: subject, sky and object-at-click rows, and the
# Masks feature in Settings › Local AI.

develop-mask-kind-subject = Sujet
develop-mask-kind-sky = Ciel
develop-mask-kind-object = Objet au clic
# An AI mask row while its network runs. $kind: subject, sky, object.
develop-ai-computing = { $kind ->
        [subject] Recherche du sujet…
        [sky] Recherche du ciel…
       *[object] Recherche de l’objet…
    }
develop-ai-ready = Prêt
# An object row with its points ($n; a box counts as two).
develop-ai-object-clicks = Points : { $n }
develop-ai-click-help =
    Cliquez sur l’objet.
    Alt+clic sur ce qui n’en fait pas partie.
    Faites glisser pour un cadre.
# The row's model is not installed. $model is its name ("IS-Net").
develop-ai-needs =
    Nécessite le module { $model }.
    Ne masque rien d’ici là.
develop-ai-failed = Impossible de créer ce masque : { $error }
develop-ai-failed-worker = Le processus des masques IA s’est arrêté.
develop-ai-open-settings = Ouvrir l’IA locale…
develop-ai-clear = Effacer les points
develop-ai-too-many = Au plus { $n } points (un cadre compte pour deux).
develop-ai-box-set = Cadre défini
develop-ai-point-added = Point ajouté
develop-ai-point-removed = Point ajouté hors de l’objet
develop-ai-cleared = Points effacés
# Screen-reader names of the points on the picture. $n counts from 1.
develop-ai-point-on = Point { $n }, sur l’objet
develop-ai-point-off = Point { $n }, hors de l’objet

## Settings › Local AI: the Masks feature
settings-ai-masks = Masques IA
settings-ai-masks-help = Masques de sujet, de ciel et d’objet dans Développer. Le modèle d’objet est fourni avec cette fonction ; sujet et ciel sont des modules.
settings-ai-masks-size = Modèle
settings-ai-masks-size-help = EfficientSAM Ti est plus rapide. EfficientSAM S suit les bords de plus près.
settings-ai-masks-size-recommended = Par défaut
settings-ai-masks-size-small = EfficientSAM Ti
settings-ai-masks-size-standard = EfficientSAM S
settings-ai-masks-size-large = Grand
