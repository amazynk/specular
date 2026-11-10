# Specular — Outil de mesure (longueurs, angles, régions et leurs statistiques sur toute image). Traduction automatique, à relire par un locuteur natif.

palette-measure = Mesurer
palette-measure-copy-csv = Copier les mesures en CSV
palette-measure-export = Exporter les mesures…
toast-copied-measurements = Mesures copiées en CSV

measure-tool-title = Mesurer
measure-tool-list = Mesures
measure-tool-count = { $count ->
    [0] Aucune mesure
    [one] { $count } mesure
   *[other] { $count } mesures
}
measure-tool-empty = Choisissez un outil et dessinez sur l’image.

measure-tool-point = Point
measure-tool-line = Ligne
measure-tool-polyline = Polyligne
measure-tool-angle = Angle
measure-tool-rect = Rectangle
measure-tool-ellipse = Ellipse
measure-tool-polygon = Polygone
measure-tool-freehand = Main levée
measure-tool-calibrate = Étalonner
# machine-drafted, needs native review
measure-tool-spot = Point

measure-tool-undo = Annuler
measure-tool-delete = Supprimer
measure-tool-copy = Copier en CSV
measure-tool-export = Exporter…
measure-tool-import = Importer…
measure-tool-done = Terminé
measure-tool-unit = Unité : { $unit }
measure-tool-apply = Appliquer

measure-tool-hint-line = Faites glisser ou cliquez sur deux points. Les flèches déplacent le réticule, Espace place un point.
measure-tool-hint-angle = Cliquez sur trois points ; le deuxième est le sommet.
measure-tool-hint-rect = Faites glisser ou cliquez sur deux coins.
measure-tool-hint-ellipse = Faites glisser ou cliquez sur deux coins de son cadre.
measure-tool-hint-polygon = Cliquez sur les coins ; Entrée ou un clic sur le premier coin le ferme.
measure-tool-hint-calibrate = Tracez une ligne sur un objet de longueur connue.
# machine-drafted, needs native review
measure-tool-hint-spot = Cliquez sur un point pour lire sa température.
measure-tool-known-length = Longueur connue : { $value } { $unit } (saisissez le nombre, U change l’unité, Entrée applique)
measure-tool-calibrated = Étalonné : un pixel vaut { $size } { $unit }
measure-tool-bad-length = Saisissez une longueur supérieure à zéro.

measure-tool-item = { $n }. { $shape }
measure-tool-area = Aire { $value }
measure-tool-mean = Moyenne { $mean } ± { $sd }
measure-tool-means = Moyenne { $values }
measure-tool-range = Plage { $min } – { $max } sur { $count } px
measure-tool-pixels = Sur { $count } px
measure-tool-stats-pending = Lecture des pixels…
measure-tool-stats-none = Aucune valeur de pixel ici
measure-tool-approximate = D’après une copie réduite (approximatif)

measure-tool-deleted = Mesure supprimée
measure-tool-undone = Annulé
measure-tool-nothing = Aucune mesure sur cette image
measure-tool-exported = Mesures enregistrées dans { $name }
measure-tool-export-failed = Impossible d’enregistrer les mesures : { $reason }
measure-tool-imported = { $count ->
    [one] { $count } mesure importée.
   *[other] { $count } mesures importées.
}
measure-tool-skipped = { $count ->
    [one] { $count } forme n’a pas pu être lue.
   *[other] { $count } formes n’ont pas pu être lues.
}
measure-tool-import-failed = Impossible de lire les mesures : { $reason }
measure-tool-too-large = Le fichier est trop volumineux.
# machine-drafted, needs native review
measure-tool-sr-not-dicom = DICOM SR nécessite une image DICOM. Enregistrez plutôt en CSV ou GeoJSON.

measure-source-uncalibrated = Pixels (non étalonné)
measure-source-dicom-pixel-spacing = Espacement des pixels DICOM
measure-source-dicom-calibrated = Espacement étalonné DICOM
measure-source-dicom-imager-pixel-spacing = Espacement DICOM au détecteur
measure-source-dicom-magnification = Espacement DICOM corrigé de l’agrandissement
measure-source-dicom-ultrasound-region = Région échographique DICOM
measure-source-tiff-resolution = Résolution TIFF
measure-source-exif-resolution = Résolution EXIF
measure-source-ome-physical-size = Taille physique OME
measure-source-fits-wcs = Coordonnées célestes FITS
measure-source-vendor-tag = Métadonnées du fabricant
measure-source-drawing-units = Unités du dessin
# machine-drafted, needs native review
measure-source-drawing-scale = D’après l’échelle du dessin
# machine-drafted, needs native review
measure-source-drawing-scale-ratio = D’après le dessin (échelle { $scale })
# machine-drafted, needs native review
measure-source-drawing-scale-viewport = D’après le dessin (échelle { $scale }, fenêtre { $viewport })
# machine-drafted, needs native review
measure-source-map-scale = Échelle de la carte au centre (approximative ; les outils cartographiques mesurent sur la Terre)
measure-source-user = Votre étalonnage
