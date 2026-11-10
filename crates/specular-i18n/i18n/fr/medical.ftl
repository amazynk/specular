# Specular — Medical images (DICOM). machine-drafted, needs native review

cheatsheet-medical = Images médicales
palette-window-level = Outil de fenêtrage (glisser à gauche)
palette-window-default = Fenêtre: valeur du fichier
palette-window-lung = Fenêtre: poumon (W 1500, L −600)
palette-window-mediastinum = Fenêtre: médiastin (W 350, L 50)
palette-window-bone = Fenêtre: os (W 1800, L 400)
palette-window-brain = Fenêtre: cerveau (W 80, L 40)
palette-window-stroke = Fenêtre: AVC (W 40, L 40)
palette-window-soft-tissue = Fenêtre: tissus mous (W 400, L 50)
palette-window-liver = Fenêtre: foie (W 150, L 30)
palette-window-subdural = Fenêtre: sous-dural (W 200, L 75)
palette-window-full-range = Fenêtre: plage complète
palette-invert-grey = Inverser les gris
palette-next-slice = Image suivante
palette-prev-slice = Image précédente
disabled-not-medical = Uniquement pour les images médicales en niveaux de gris
disabled-one-frame = L’image n’a qu’une seule trame
settings-section-medical = Images médicales
settings-medical-keep-window = Garder le fenêtrage pour la série suivante
settings-medical-keep-window-help = Garder la fenêtre réglée en passant à une autre série, au lieu de partir de sa valeur par défaut. Les trames d’un même fichier et les coupes d’une même série la partagent toujours.
medical-window = C { $center }  W { $width }
medical-inverted = inversé
medical-frame = Trame { $frame } sur { $count }
medical-tool = Outil de fenêtrage : glisser
medical-tool-on = Outil de fenêtrage activé : glissez pour régler
medical-tool-off = Outil de fenêtrage désactivé
medical-preset-not-ct = Les préréglages CT exigent des valeurs en unités Hounsfield
medical-file-window-none = Pas de fenêtre { $index } dans ce fichier
medical-file-window-unnamed = Fenêtre du fichier { $index } sur { $count }
medical-file-window = Fenêtre du fichier { $index } sur { $count } : { $name }
measure-templates-need-medical = Les modèles géométriques viennent avec le profil Médical
medical-off = Pas d’outils médicaux pour cette image : { $reason }
medical-readout = Valeur et fenêtre

## Measurements, cine and overlays (machine-drafted, needs native review)

palette-cine = Lire les images (ciné)
palette-measure-length = Mesurer une longueur
palette-measure-angle = Mesurer un angle
palette-measure-ellipse = Mesurer une région elliptique
palette-measure-rectangle = Mesurer une région rectangulaire
palette-clear-measurements = Effacer les mesures
palette-measure-templates = Modèles géométriques…
palette-overlays = Afficher ou masquer les superpositions
medical-cine = Lecture à { $rate } images/s
medical-overlays-shown = Superpositions affichées
medical-overlays-hidden = Superpositions masquées
measure-hint-length = Longueur : faites glisser ou cliquez sur les deux extrémités
measure-hint-angle = Angle : cliquez sur trois points, le sommet en deuxième
measure-hint-ellipse = Ellipse : faites glisser sur la région
measure-hint-rectangle = Rectangle : faites glisser sur la région
measure-off = Outil de mesure désactivé
measure-cleared = Mesures effacées
measure-length-mm = { $value } mm
# machine-drafted, needs native review
measure-length-angstrom = { $value } Å
measure-length-px = { $value } px
measure-area-mm = Aire { $value } mm²
measure-area-px = Aire { $value } px²
measure-angle = { $value }°
measure-mean = Moyenne { $mean } ± { $sd } { $unit }
measure-range = Min { $min }  Max { $max }  ({ $count } px)
measure-caveat-detector = Au détecteur : non corrigé de l’agrandissement
measure-caveat-magnification = Correction d’agrandissement estimée
measure-caveat-uncalibrated = Pas d’espacement des pixels : en pixels
measure-caveat-outside-region = Hors d’une région échographique calibrée : en pixels

## DICOM orientation labels, presentation states and wheel stepping (machine-drafted, needs native review)
settings-medical-orientation-labels = Afficher les repères d’orientation
settings-medical-orientation-labels-help = Lettres aux bords de l’image indiquant les directions du patient (R/L droite/gauche, A/P antérieur/postérieur, S/I tête/pieds), lues dans le fichier et qui suivent rotations et retournements. L les affiche ou les masque pour la session.
settings-medical-wheel-steps-frames = La molette parcourt les trames
settings-medical-wheel-steps-frames-help = Dans un fichier ou une série à plusieurs trames, la molette passe d’une trame à l’autre au lieu de zoomer ou de changer d’image. Ctrl+molette zoome toujours.
# machine-drafted, needs native review
settings-medical-display-function = Affichage des gris
settings-medical-display-function-help = Façon dont les gris des images médicales sont affichés. DICOM GSDF (fonction d’affichage standard des niveaux de gris) les répartit en pas de visibilité égale entre les luminances du noir et du blanc de l’écran (DICOM PS3.14), comme sur un écran de diagnostic étalonné. Le profil colorimétrique de l’écran s’applique toujours.
settings-medical-display-function-srgb = Standard (sRGB)
settings-medical-display-function-gsdf = DICOM GSDF
settings-medical-display-black = Luminance du noir de l’écran
settings-medical-display-black-help = Luminance du noir de l’écran, lumière ambiante comprise, mesurée ou tirée de la fiche technique de l’écran. Utilisée par DICOM GSDF.
settings-medical-display-white = Luminance du blanc de l’écran
settings-medical-display-white-help = Luminance du blanc de l’écran, mesurée ou tirée de la fiche technique de l’écran. Utilisée par DICOM GSDF.
palette-orientation-labels = Afficher ou masquer les repères d’orientation
palette-presentation-state = État de présentation suivant
disabled-no-presentation-state = Aucun état de présentation ne fait référence à cette image
medical-labels-shown = Repères d’orientation affichés
medical-labels-hidden = Repères d’orientation masqués
medical-presentation = État de présentation : { $label } ({ $index } sur { $count })
medical-presentation-none = État de présentation désactivé
medical-presentation-readout = Présentation : { $label }
medical-preset-presentation = La table de correspondance de l’état de présentation définit les niveaux de gris

## Orientation labels for screen readers (machine-drafted, needs native review)
medical-orientation-a11y = Orientation : gauche { $left }, droite { $right }, haut { $top }, bas { $bottom }
