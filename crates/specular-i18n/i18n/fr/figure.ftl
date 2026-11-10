# Figures, key images and teaching files.

## Command palette
# machine-drafted, needs native review
palette-export-figure = Exporter la figure…
# machine-drafted, needs native review
palette-export-figure-help = Enregistrer la vue sous forme de figure avec barre d’échelle, barre de couleurs et étiquettes.
# machine-drafted, needs native review
palette-toggle-key-image = Marquer comme image clé
# machine-drafted, needs native review
palette-toggle-key-image-help = Marquer cette image comme image clé, ou retirer la marque.
# machine-drafted, needs native review
palette-copy-key-images = Copier les images clés
# machine-drafted, needs native review
palette-copy-key-images-help = Copier les images clés en une seule image.
# machine-drafted, needs native review
palette-export-key-images = Exporter les images clés…
# machine-drafted, needs native review
palette-export-key-images-help = Enregistrer les images clés en diapositives ou en dossier de PNG.
# machine-drafted, needs native review
palette-export-teaching-file = Exporter un dossier d’enseignement…
# machine-drafted, needs native review
palette-export-teaching-file-help = Enregistrer les images clés sans les données du patient, pour l’enseignement.

## The figure dialog
# machine-drafted, needs native review
figure-dialog-title = Exporter la figure
# machine-drafted, needs native review
figure-format = Format
# machine-drafted, needs native review
figure-format-png = PNG, 16 bits
# machine-drafted, needs native review
figure-format-tiff = TIFF, 16 bits
# machine-drafted, needs native review
figure-region = Région
# machine-drafted, needs native review
figure-region-visible = Zone visible
# machine-drafted, needs native review
figure-region-whole = Image entière
# machine-drafted, needs native review
figure-size = Taille
# machine-drafted, needs native review
figure-size-pixels = Pixels de l’image (1:1)
# $width is a width in pixels.
# machine-drafted, needs native review
figure-size-width = { $width } px de large
# machine-drafted, needs native review
figure-scale-bar = Barre d’échelle
# machine-drafted, needs native review
figure-scale-bar-none = Aucun étalonnage
# machine-drafted, needs native review
figure-colour-bar = Barre de couleurs
# machine-drafted, needs native review
figure-colour-bar-none = Aucune palette ni fenêtre
# machine-drafted, needs native review
figure-inset = Encart
# machine-drafted, needs native review
figure-inset-rectangle = Dernier rectangle
# machine-drafted, needs native review
figure-inset-none = Dessinez d’abord un rectangle
# machine-drafted, needs native review
figure-channels = Canaux
# machine-drafted, needs native review
figure-channels-merged = Fusionnés
# machine-drafted, needs native review
figure-channels-split = Séparés et fusionnés
# machine-drafted, needs native review
figure-channels-one = Un canal
# machine-drafted, needs native review
figure-measurements = Mesures
# $count is the number of measurements.
# machine-drafted, needs native review
figure-measurements-on = { $count ->
    [one] Activées ({ $count })
    [many] Activées ({ $count })
   *[other] Activées ({ $count })
}
# machine-drafted, needs native review
figure-measurements-none = Aucune
# machine-drafted, needs native review
figure-title-row = Titre
# machine-drafted, needs native review
figure-provenance = Provenance
# machine-drafted, needs native review
figure-provenance-metadata = Dans le fichier
# machine-drafted, needs native review
figure-provenance-drawn = Dans le fichier et dessinée
# machine-drafted, needs native review
figure-file = Fichier
# machine-drafted, needs native review
figure-choose = Choisir…
# machine-drafted, needs native review
figure-on = Activé
# machine-drafted, needs native review
figure-off = Désactivé
# $source is where the value comes from (a calibration, a colour map).
# machine-drafted, needs native review
figure-on-from = Activé · { $source }
# machine-drafted, needs native review
figure-export = Exporter
# machine-drafted, needs native review
figure-working = Exportation…
# machine-drafted, needs native review
figure-not-full-res = Les pixels en pleine résolution ne sont pas chargés. Zoomez à { PERCENT(100) } ou choisissez une taille.
# machine-drafted, needs native review
figure-channel-red = Rouge
# machine-drafted, needs native review
figure-channel-green = Vert
# machine-drafted, needs native review
figure-channel-blue = Bleu
# $map is a colour map's name; $lo and $hi the values at its ends; $unit their unit.
# machine-drafted, needs native review
figure-legend-map = { $map }, de { $lo } à { $hi } { $unit }
# $lo and $hi are the window's ends; $unit their unit (HU).
# machine-drafted, needs native review
figure-legend-window = Fenêtre de { $lo } à { $hi } { $unit }
# $preset is a channel-colour preset's id.
# machine-drafted, needs native review
figure-display-channels = Couleurs des canaux { $preset }
# $channel is the channel shown (red, green, blue, alpha, luma).
# machine-drafted, needs native review
figure-display-channel = Canal { $channel }
# machine-drafted, needs native review
figure-display-hilo = Repères HiLo
# machine-drafted, needs native review
figure-display-as-stored = Tel qu’enregistré
# $value is the calibration (source and spacing).
# machine-drafted, needs native review
figure-caption-calibration = Étalonnage : { $value }
# $value is the display settings.
# machine-drafted, needs native review
figure-caption-display = Affichage : { $value }
# machine-drafted, needs native review
figure-not-image = Les figures sont créées à partir d’une image du visualiseur.
# machine-drafted, needs native review
figure-not-ready = L’image est encore en cours de chargement.
# $width and $height are in pixels.
# machine-drafted, needs native review
figure-too-large = Trop grande : { $width } × { $height } px. Choisissez la zone visible ou une taille.

## Key images
# machine-drafted, needs native review
figure-keys-title = Exporter les images clés
# machine-drafted, needs native review
figure-teaching-title = Exporter un dossier d’enseignement
# machine-drafted, needs native review
figure-keys-images = Images
# $count is the number of key images.
# machine-drafted, needs native review
figure-keys-count = { $count ->
    [one] { $count } image clé
    [many] { $count } images clés
   *[other] { $count } images clés
}
# machine-drafted, needs native review
figure-keys-pptx = Diapositives (PPTX)
# machine-drafted, needs native review
figure-keys-folder = Dossier de PNG
# machine-drafted, needs native review
figure-keys-preset = Préréglage
# machine-drafted, needs native review
figure-keys-standard = Standard
# machine-drafted, needs native review
figure-keys-teaching = Dossier d’enseignement
# machine-drafted, needs native review
figure-keys-overlays-on = Dessinés et dans les notes
# machine-drafted, needs native review
figure-keys-masks = Texte incrusté
# machine-drafted, needs native review
figure-keys-masks-auto = Masqué en noir
# machine-drafted, needs native review
figure-keys-notes = Notes de cas
# machine-drafted, needs native review
figure-keys-destination = Enregistrer dans
# machine-drafted, needs native review
figure-keys-empty = Aucune image clé pour l’instant. Marquez d’abord une image comme image clé.
# machine-drafted, needs native review
figure-keys-teaching-note = Les données du patient sont omises et le texte incrusté est masqué en noir. Vérifiez chaque image avant de la partager.
# machine-drafted, needs native review
figure-keys-none-left = Aucune image n’a pu être exportée sans données du patient.
# machine-drafted, needs native review
figure-keys-copied = Images clés copiées
# $count is how many key images there are now.
# machine-drafted, needs native review
figure-key-marked = { $count ->
    [one] Marquée comme image clé ({ $count })
    [many] Marquée comme image clé ({ $count })
   *[other] Marquée comme image clé ({ $count })
}
# $count is how many key images are left.
# machine-drafted, needs native review
figure-key-unmarked = { $count ->
    [one] N’est plus une image clé ({ $count } restantes)
    [many] N’est plus une image clé ({ $count } restantes)
   *[other] N’est plus une image clé ({ $count } restantes)
}
# $n is a frame or page number.
# machine-drafted, needs native review
figure-key-frame = Image { $n }
# $n is the key image's number in the set.
# machine-drafted, needs native review
figure-key-n = Image clé { $n }

## Results
# $name is the file or folder written.
# machine-drafted, needs native review
figure-saved = { $name } enregistré
# $name is the file or folder written; $count how many images were left out.
# machine-drafted, needs native review
figure-saved-skipped = { $count ->
    [one] { $name } enregistré. { $count } image a été écartée : son texte incrusté est introuvable.
    [many] { $name } enregistré. { $count } images ont été écartées : leur texte incrusté est introuvable.
   *[other] { $name } enregistré. { $count } images ont été écartées : leur texte incrusté est introuvable.
}
# $reason says why.
# machine-drafted, needs native review
figure-failed = Exportation impossible : { $reason }
# $name is a file that is already in the folder.
# machine-drafted, needs native review
figure-folder-taken = { $name } se trouve déjà dans ce dossier. Choisissez un dossier vide.

## The viewer
# machine-drafted, needs native review
figure-key-badge = Image clé
