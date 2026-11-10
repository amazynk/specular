# machine-drafted, needs native review
# Specular — Pathologie : séparation des colorants, export et import d’annotations, volumes d’étiquettes sur les volumes. Français.

## Palette
palette-stain-separation = Séparer les colorants
palette-stain-1 = Afficher ou masquer le colorant 1
palette-stain-2 = Afficher ou masquer le colorant 2
palette-stain-3 = Afficher ou masquer le colorant 3
palette-stain-mode = Colorants en lumière ou en densité optique
palette-stain-estimate = Estimer les colorants depuis la région
palette-stain-normalise = Normaliser les couleurs des colorants
palette-stain-reference = Utiliser la région comme référence de coloration
palette-stain-reset = Vecteurs de coloration publiés
palette-export-annotations = Exporter les annotations…
palette-import-annotations = Importer des annotations…
palette-label-volume = Superposer un volume d’étiquettes…

## Raisons de désactivation
disabled-not-colour-image = Nécessite une image en couleur
disabled-stains-off = Activez d’abord la séparation des colorants
disabled-no-region = Tracez d’abord une région
disabled-no-slide-annotations = Aucune annotation, région ou point à exporter
disabled-no-label-target = Ouvrez un volume en vues multiplanaires, ou une image médicale

## Noms des colorants
stain-hematoxylin = Hématoxyline
stain-eosin = Éosine
stain-dab = DAB
stain-residual = Résidu
stain-set-he = H&E
stain-set-hdab = H-DAB
stain-set-hedab = H&E-DAB

## Notifications et légende des colorants
stain-off = Séparation des colorants désactivée
# { $set } : H&E, H-DAB ou H&E-DAB.
stain-on = Colorants : { $set }
stain-shown = { $stain } visible
stain-hidden = { $stain } non visible
stain-mode-light = Colorants en lumière
stain-mode-od = Colorants en densité optique
# Titre de la légende : « H&E · lumière », « H-DAB · densité optique ».
stain-legend-light = { $set } · lumière
stain-legend-od = { $set } · densité optique
stain-legend-estimated = Estimés depuis la région
stain-legend-normalised = Normalisés
stain-legend-published = Vecteurs publiés
# { $pixels } : pixels colorés utilisés.
stain-estimated = Colorants estimés sur { $pixels } pixels
stain-estimate-failed = Impossible d’estimer les colorants : { $reason }
stain-estimate-background = la région est surtout du fond
stain-estimate-one-colour = la région ne contient qu’une couleur
stain-estimate-three = l’estimation nécessite H&E ou H-DAB
stain-estimate-no-pixels = les pixels de la région ne sont pas encore chargés
stain-reference-set = Région définie comme référence de coloration
stain-normalise-on = Couleurs des colorants normalisées
stain-normalise-off = Couleurs des colorants telles que numérisées
stain-reset-done = Vecteurs de coloration publiés

## Boîte de dialogue d’export des annotations
annot-export-title = Exporter les annotations
annot-export-format = Format
annot-export-format-coco = COCO JSON
annot-export-format-geojson = GeoJSON (QuPath)
annot-export-format-masks = Masques d’étiquettes
annot-export-output = Masques en
annot-export-output-png = Tuiles PNG
annot-export-output-tiff = Tuiles TIFF
annot-export-output-ome = Pyramide OME-TIFF
annot-export-region = Région
annot-export-region-slide = Lame entière
annot-export-region-view = Zone visible
annot-export-region-drawn = Dernière région tracée
annot-export-level = Niveau
# { $level } : 0 pour la pleine résolution ; { $width } × { $height } sa taille en pixels.
annot-export-level-value = { $level } · { $width } × { $height }
annot-export-tile = Taille des tuiles
annot-export-empty = Tuiles vides
annot-export-empty-skip = Ignorer
annot-export-empty-keep = Écrire
annot-export-what = { $count ->
    [one] { $count } forme
    [many] { $count } de formes
   *[other] { $count } formes
}, { $classes ->
    [one] { $classes } classe
    [many] { $classes } de classes
   *[other] { $classes } classes
}
annot-export-lines = Les lignes sont exclues de COCO et des masques.
annot-export-export = Exporter…
annot-export-writing = Écriture…
# { $files } : fichiers écrits.
annot-export-done = { $files ->
    [one] { $files } fichier exporté
    [many] { $files } de fichiers exportés
   *[other] { $files } fichiers exportés
}
annot-export-failed = Échec de l’export : { $reason }
annot-import-done = { $count ->
    [one] { $count } annotation importée
    [many] { $count } d’annotations importées
   *[other] { $count } annotations importées
}
annot-import-failed = Import impossible : { $reason }
annot-import-skipped = { $count ->
    [one] { $count } forme n’avait pas de contour utilisable
    [many] { $count } de formes n’avaient pas de contour utilisable
   *[other] { $count } formes n’avaient pas de contour utilisable
}
annot-file-coco = COCO JSON
annot-file-geojson = GeoJSON

## Volumes d’étiquettes
label-volume-file = Volume d’étiquettes
label-volume-table = Table des couleurs
# { $file } : nom du fichier du volume d’étiquettes ; { $count } : ses étiquettes.
label-volume-loaded = { $count ->
    [one] { $file } : { $count } étiquette
    [many] { $file } : { $count } d’étiquettes
   *[other] { $file } : { $count } étiquettes
}
label-volume-failed = Impossible de superposer { $file } : { $reason }
label-volume-outside = { $file } ne recouvre pas ce volume
label-volume-not-whole = ses valeurs ne sont pas des nombres entiers
label-volume-colour = il contient des couleurs, pas des étiquettes
label-volume-empty = il n’a aucune étiquette
label-volume-too-many = il a trop d’étiquettes
label-volume-too-large = il est trop grand pour être rééchantillonné
label-volume-degenerate = son orientation n’est pas valide
# Une étiquette sans nom dans la table des couleurs.
label-volume-unnamed = Étiquette { $value }
