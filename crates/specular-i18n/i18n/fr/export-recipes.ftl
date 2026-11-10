# Specular — Export recipes: saved ways to export, several run at once. machine-drafted, needs native review
# Every output is a new file (or replaces one only after asking); originals are never changed.

palette-export-recipes = Exporter avec des recettes…
palette-export-recipes-help = Exporter la sélection avec une ou plusieurs recettes enregistrées.
palette-develop-export-recipes = Exporter avec des recettes…
palette-develop-export-recipes-help = Exporter la photo retouchée, ou plusieurs, avec des recettes enregistrées.

export-recipes-title = Exporter avec des recettes
export-recipes-new = Nouvelle
export-recipes-edit = Modifier…
export-recipes-duplicate = Dupliquer
export-recipes-delete = Supprimer…
export-recipes-start = Exporter
export-recipes-save = Enregistrer
export-recipes-open-folder = Ouvrir le dossier
export-recipes-choose-image = Choisir l’image…
export-recipes-no-images = Aucune image à exporter.
export-recipes-none-checked = Cochez au moins une recette.
export-recipes-checked = { $name } : activée
export-recipes-unchecked = { $name } : désactivée

## The starters.
export-recipes-starter-web = Web, 2048 px
export-recipes-starter-print = Impression, taille réelle
export-recipes-starter-email = E-mail, petite taille
export-recipes-unnamed = Recette sans nom
export-recipes-new-name = Nouvelle recette
# { $name } is the recipe copied.
export-recipes-copy-name = { $name } (copie)

## The list's one-line summary.
export-recipes-summary-mark = filigrane
export-recipes-size-full = Taille réelle
export-recipes-size-long = { $px } px grand côté
export-recipes-size-short = { $px } px petit côté
export-recipes-size-mp = { $mp } MP
export-recipes-size-exact = { $width } × { $height } px
export-recipes-size-percent = { PERCENT($percent) }
export-recipes-folder-same = À côté de l’original
export-recipes-folder-sub = Dossier « { $name } »
export-recipes-folder-ask = Demander à chaque fois

## The editor.
export-recipes-new-title = Nouvelle recette
export-recipes-edit-title = Modifier la recette
export-recipes-name = Nom de la recette
export-recipes-text = Texte personnalisé
export-recipes-pattern = Nom de fichier
export-recipes-mark-text-field = Texte du filigrane
export-recipes-subfolder = Nom du dossier
export-recipes-format = Format de fichier
export-recipes-quality = Qualité
export-recipes-lossless = Sans perte
export-recipes-bits = Profondeur de bits
export-recipes-bits-8 = 8 bits
export-recipes-bits-16 = 16 bits
export-recipes-space = Espace colorimétrique
export-recipes-space-srgb = sRGB
export-recipes-space-p3 = Display P3
export-recipes-space-adobe = Adobe RGB (1998)
export-recipes-space-prophoto = ProPhoto RGB
export-recipes-profile = Profil
export-recipes-profile-embed = Incorporer
export-recipes-profile-none = Omettre
export-recipes-folder = Dossier
export-recipes-folder-subfolder = Sous-dossier
export-recipes-collision = Nom déjà pris
export-recipes-collision-unique = Ajouter un numéro
export-recipes-collision-overwrite = Remplacer (demande)
export-recipes-collision-skip = Ignorer
export-recipes-after = Une fois fini
export-recipes-after-nothing = Rien
export-recipes-after-open = Ouvrir le dossier
export-recipes-resize = Taille
export-recipes-resize-long = Grand côté
export-recipes-resize-short = Petit côté
export-recipes-resize-mp = Mégapixels
export-recipes-resize-exact = Taille exacte
export-recipes-resize-percent = Pourcentage
export-recipes-edge = Côté
export-recipes-width = Largeur
export-recipes-height = Hauteur
export-recipes-megapixels = Mégapixels
export-recipes-percent = Pourcentage
export-recipes-px = { $px } px
export-recipes-mp = { $mp } MP
export-recipes-pct = { PERCENT($percent) }
export-recipes-enlarge = Agrandir
export-recipes-sharpen = Netteté pour
export-recipes-sharpen-off = Désactivée
export-recipes-sharpen-screen = Écran
export-recipes-sharpen-matte = Papier mat
export-recipes-sharpen-glossy = Papier brillant
export-recipes-amount = Intensité
export-recipes-amount-low = Faible
export-recipes-amount-standard = Normale
export-recipes-amount-high = Forte
export-recipes-metadata = Métadonnées
export-recipes-meta-all = Toutes
export-recipes-meta-copyright = Copyright seul
export-recipes-meta-contact = Copyright et contact
export-recipes-meta-none = Aucune
export-recipes-location = Lieu
export-recipes-people = Personnes visibles
export-recipes-keep = Garder
export-recipes-remove = Supprimer
export-recipes-mark = Filigrane
export-recipes-mark-none = Aucun
export-recipes-mark-text = Texte
export-recipes-mark-image = Image (à choisir)
export-recipes-position = Position
export-recipes-pos-top-left = En haut à gauche
export-recipes-pos-top = En haut
export-recipes-pos-top-right = En haut à droite
export-recipes-pos-left = À gauche
export-recipes-pos-centre = Au centre
export-recipes-pos-right = À droite
export-recipes-pos-bottom-left = En bas à gauche
export-recipes-pos-bottom = En bas
export-recipes-pos-bottom-right = En bas à droite
export-recipes-opacity = Opacité
export-recipes-scale = Largeur
export-recipes-yes = Oui
export-recipes-no = Non
export-recipes-error-name = Donnez un nom à la recette.
export-recipes-error-mark-text = Saisissez le texte du filigrane.
export-recipes-error-mark-image = Choisissez l’image du filigrane.
export-recipes-error-subfolder = Saisissez le nom du dossier.
export-recipes-saved = Recette « { $name } » enregistrée.

## Delete.
export-recipes-delete-title = Supprimer la recette « { $name } » ?
export-recipes-delete-detail = Les fichiers qu’elle a exportés restent sur le disque.
export-recipes-delete-confirm = Supprimer
export-recipes-delete-keep = Garder
export-recipes-deleted = Recette « { $name } » supprimée.

## The run.
# { $name } is the recipe whose folder is asked for.
export-recipes-ask-title = Dossier pour « { $name } »
export-recipes-asking = Choisissez le dossier de chaque recette qui le demande.
export-recipes-no-folder = Aucun dossier choisi. Rien n’a été exporté.
export-recipes-planning = Nommage des fichiers…
export-recipes-plan-pattern = Recette « { $name } » : { $reason }
export-recipes-replace-title = { $count ->
    [one] Remplacer 1 fichier ?
   *[other] Remplacer { $count } fichiers ?
}
export-recipes-replace-detail = Les originaux ne sont jamais remplacés.
export-recipes-replace-confirm = Remplacer
export-recipes-replace-kept = Rien n’a été exporté.
export-recipes-running = Exportation…
export-recipes-progress = { $done } fichiers sur { $total }
export-recipes-done = { $count ->
    [one] 1 fichier exporté.
   *[other] { $count } fichiers exportés.
}
export-recipes-stopped = { $count ->
    [one] Arrêté. 1 fichier exporté.
   *[other] Arrêté. { $count } fichiers exportés.
}
export-recipes-done-skipped = { $count ->
    [one] 1 ignoré : le nom était déjà pris.
   *[other] { $count } ignorés : les noms étaient déjà pris.
}
export-recipes-done-failed = { $count ->
    [one] 1 en échec :
   *[other] { $count } en échec :
}
# One failure: the source file, the recipe and why.
export-recipes-failure = { $file } ({ $recipe }) : { $reason }
