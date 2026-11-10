# Specular — Drawings (DXF): layers, paper, lineweights, model space and layouts. Français.

keys-mode-drawing = Dessins
cheatsheet-drawing = Dessins
palette-drawing-layers = Afficher ou masquer le panneau des calques
palette-drawing-all-layers = Afficher tous les calques, ou les calques tels qu’enregistrés
palette-drawing-paper = Inverser papier sombre et papier clair
palette-drawing-lineweights = Afficher ou masquer les épaisseurs de ligne
disabled-not-drawing = Uniquement pour les dessins (DXF)
# A drawing's page: model space (layouts are shown by their own name).
page-label-model = Modèle
# machine-drafted, needs native review
cad-pen-name = Plume { $n }
cad-page-name = Page { $n }
cad-layer-name = Calque { $n }
cad-map-name = Carte
# machine-drafted, needs native review
# A board's views (its Gerber files together): every layer stacked, and its top and bottom
# in board colours.
cad-board-layers-name = Couches
cad-board-top-name = Dessus
cad-board-bottom-name = Dessous
# machine-drafted, needs native review
# A plot's layers: its frame, a trace without a name (numbered from 1), and the contour
# lines of a 2-D spectrum above and below zero.
cad-chart-frame-name = Cadre
cad-trace-name = Tracé { $n }
cad-contours-positive-name = Contours positifs
cad-contours-negative-name = Contours négatifs
# machine-drafted, needs native review
# The page of a survey file (LandXML) showing its surfaces and alignments in 3D.
cad-model-3d-name = Modèle 3D
# The layers panel's title (and its name for screen readers).
drawing-layers-title = Calques · { $shown } sur { $total } affichés
# Toasts.
drawing-layers-all-toast = Tous les calques affichés
drawing-layers-saved-toast = Calques tels qu’enregistrés dans le dessin
drawing-paper-dark-toast = Papier sombre
drawing-paper-light-toast = Papier clair
drawing-lineweights-on = Épaisseurs de ligne affichées
drawing-lineweights-off = Épaisseurs de ligne désactivées : traits fins

# machine-drafted, needs native review
# Markup on drawings and images (Alt+M): kept in a file next to the drawing, never in it.
palette-markup = Annoter
palette-markup-help = Nuages, texte, flèches, tampons, mesures et repères photo, enregistrés à côté du fichier.
palette-compare-sheets = Comparer des planches
palette-compare-sheets-help = Superposer un autre indice de cette planche, aligné, avec les modifications entourées.
markup-title = Annotations
markup-list = Annotations de cette page
markup-count = { $count ->
    [0] Aucune annotation sur cette page
    [one] { $count } annotation sur cette page
   *[other] { $count } annotations sur cette page
}
markup-empty = Choisissez un outil et cliquez sur la planche.
markup-item = { $n }. { $kind }
markup-tool-select = Sélection
markup-tool-cloud = Nuage
markup-tool-text = Texte
markup-tool-arrow = Flèche
markup-tool-stamp = Tampon
markup-tool-length = Longueur
markup-tool-area = Aire
markup-tool-pin = Repère photo
markup-hint-select = Cliquez sur un élément pour le sélectionner. Faites-le glisser pour le déplacer. Enter modifie son texte.
markup-hint-cloud = Tracez un rectangle, ou cliquez sur les coins et appuyez sur Enter.
markup-hint-text = Cliquez à l’endroit du texte.
markup-hint-arrow = Faites glisser de l’étiquette vers le point, ou cliquez sur les deux.
markup-hint-stamp = Cliquez à l’endroit du tampon.
markup-hint-length = Faites glisser, ou cliquez sur les deux extrémités.
markup-hint-area = Cliquez sur les coins, puis appuyez sur Enter.
markup-hint-pin = Cliquez sur un point, puis choisissez une photo.
markup-kind-cloud = Nuage
markup-kind-text = Texte
markup-kind-arrow = Flèche
markup-kind-stamp = Tampon
markup-kind-length = Longueur
markup-kind-area = Aire
markup-kind-pin = Photo
markup-colour = Couleur : { $colour }
markup-colour-red = Rouge
markup-colour-blue = Bleu
markup-colour-green = Vert
markup-colour-orange = Orange
markup-colour-magenta = Magenta
markup-colour-black = Noir
markup-width = Trait : { $width }
markup-width-thin = Fin
markup-width-medium = Moyen
markup-width-thick = Épais
markup-stamp = Tampon : { $words }
markup-stamp-approved = Approuvé
markup-stamp-rejected = Refusé
markup-stamp-reviewed = Vérifié
markup-stamp-custom = Texte libre
markup-undo = Annuler
markup-redo = Rétablir
markup-delete = Supprimer
markup-open-photo = Ouvrir la photo
markup-from-measurements = Ajouter les mesures
markup-from-measurements-done = { $count ->
    [one] { $count } mesure ajoutée aux annotations
   *[other] { $count } mesures ajoutées aux annotations
}
markup-export-pdf = Exporter en PDF…
markup-export-png = Exporter en PNG…
markup-done = Terminé
markup-ask-text = Texte (Enter pour placer, Esc pour annuler)
markup-ask-label = Étiquette (facultative ; Enter pour placer)
markup-ask-stamp = Texte du tampon (Enter pour placer)
markup-ask-edit = Nouveau texte (Enter pour enregistrer)
markup-added = Ajouté : { $kind }
markup-moved = Déplacé
markup-pin-hint = Cliquez à nouveau sur le repère ou appuyez sur Enter pour ouvrir sa photo.
markup-deleted = Supprimé
markup-undone = Annulé
markup-redone = Rétabli
markup-delete-ask = Supprimer { $name } ?
markup-delete-detail = L’élément est retiré du fichier d’annotations. Le dessin lui-même n’est pas modifié.
markup-delete-ok = Supprimer
markup-photo-missing = { $name } n’est plus là.
markup-load-failed = Annotations non lues : { $reason }
markup-save-failed = Annotations non enregistrées : { $reason }
markup-sidecar-too-large = le fichier d’annotations est trop volumineux
markup-pdf-unreadable = le PDF n’a pas pu être lu
markup-exported = { $name } enregistré
markup-export-failed = Non exporté : { $reason }
markup-export-same-file = choisissez un autre nom ; l’original n’est jamais écrasé
markup-export-encrypted = le PDF est chiffré. Exportez plutôt en PNG.
markup-export-damaged = la structure du PDF est endommagée. Exportez plutôt en PNG.
sheets-title = Comparer des planches
sheets-other = Autre planche…
sheets-other-named = Autre : { $name }
sheets-page = Page { $page } sur { $count }
sheets-auto = Aligner
sheets-pick = Choisir 2 points
sheets-overlay = Superposition
sheets-difference = Différence
sheets-blend = Fondu
sheets-blink = Clignotement
sheets-slider = Autre planche { PERCENT($percent) }
sheets-clouds = { $count ->
    [0] Aucune modification
    [one] Entourer { $count } modification d’un nuage
   *[other] Entourer { $count } modifications de nuages
}
sheets-done = Terminé
sheets-choose = Choisissez l’autre planche.
sheets-reading = Lecture de la planche…
sheets-aligning = Alignement…
sheets-aligned = Aligné : échelle { $scale }, rotation { $angle }°, écart max. { $error } px. { $changes ->
    [0] Aucune modification trouvée.
    [one] { $changes } modification.
   *[other] { $changes } modifications.
}
sheets-align-failed = Alignement automatique impossible. Choisissez 2 points.
sheets-pick-this = Point { $n } : cliquez dessus sur cette planche.
sheets-pick-other = Point { $n } : cliquez sur le même point sur l’autre planche.
sheets-pick-same = Les deux points sont identiques. Choisissez à nouveau 2 points.
sheets-read-failed = Planche non lue : { $reason }
sheets-failed = Cette étape a échoué. Réessayez ou choisissez une autre planche.
sheets-legend-this = Cette planche seulement
sheets-legend-other = Autre planche seulement
sheets-legend-both = Les deux
sheets-cloud-text = Modifié
sheets-clouds-added = { $count ->
    [0] Aucun nuage ajouté
    [one] { $count } nuage ajouté aux annotations
   *[other] { $count } nuages ajoutés aux annotations
}
sheets-opened = Comparer des planches
sheets-closed = Comparaison fermée
markup-ask-new-line = Shift+Enter : nouvelle ligne

## Settings › Drawings

# machine-drafted, needs native review
settings-section-drawings = Dessins
settings-drawings-shx-folders = Dossiers de polices SHX
settings-drawings-shx-folders-help = Dossiers contenant des polices SHX pour le texte des fichiers DXF et DWG, séparés par ;. Le dossier du dessin lui-même est consulté en premier.
# A folder in the SHX font folders setting that does not exist or is not a folder.
settings-error-shx-folder = Pas un dossier : { $folder }
