# machine-drafted, needs native review
# Specular — Develop (non-destructive photo editing). Français.

# Shown once when an edit opens and some of its modules were made with a version that no
# longer renders: they were moved to the nearest version that does. $count: modules moved.
# machine-drafted, needs native review
toast-develop-edit-migrated = { $count ->
    [one] Un module de cette retouche a été mis à jour vers une version plus récente. Le rendu peut légèrement différer.
    [many] { $count } modules de cette retouche ont été mis à jour vers une version plus récente. Le rendu peut légèrement différer.
   *[other] { $count } modules de cette retouche ont été mis à jour vers une version plus récente. Le rendu peut légèrement différer.
}
# Shown when an edit holds modules this version of Specular cannot render (made with a newer
# Specular). They are kept and saved unchanged, but skipped. $count: modules skipped.
# machine-drafted, needs native review
toast-develop-edit-newer = { $count ->
    [one] Un module de cette retouche nécessite une version plus récente de Specular. Il est conservé mais pas affiché.
    [many] { $count } modules de cette retouche nécessitent une version plus récente de Specular. Ils sont conservés mais pas affichés.
   *[other] { $count } modules de cette retouche nécessitent une version plus récente de Specular. Ils sont conservés mais pas affichés.
}

## Develop's controls (sliders, curve, colour wheels). Numbers come formatted in the UI
## language's digits.

# machine-drafted, needs native review
# A slider's value in exposure values. $value: "+0.70".
develop-value-ev = { $value } EV
# machine-drafted, needs native review
# machine-drafted, needs native review
# An angle. $value: "2.5".
develop-value-degrees = { $value }°
# machine-drafted, needs native review
# A colour temperature. $value: "5,000".
develop-value-kelvin = { $value } K
# machine-drafted, needs native review
# The curve editor's selected point. $index, $count: point numbers; $input, $output: "25 %".
develop-curve-point = Point { $index } sur { $count } : entrée { $input }, sortie { $output }
# machine-drafted, needs native review
# The curve editor's keys, read by screen readers.
develop-curve-keys = Les flèches déplacent le point. Pg préc et Pg suiv en choisissent un autre. Inser ajoute un point, Suppr le retire.
# machine-drafted, needs native review
# A colour wheel with no colour.
develop-wheel-neutral = Neutre
# machine-drafted, needs native review
# A colour wheel's colour. $hue: degrees; $strength: "20 %".
develop-wheel-value = Teinte { $hue }°, intensité { $strength }
# machine-drafted, needs native review
# A colour wheel's keys, read by screen readers.
develop-wheel-keys = Les flèches déplacent la couleur. Suppr la réinitialise.

## Develop's panel.

# machine-drafted, needs native review
develop-title = Développement
# machine-drafted, needs native review
develop-undo = Annuler
# machine-drafted, needs native review
develop-redo = Rétablir
# machine-drafted, needs native review
develop-before-after = Avant et après
# machine-drafted, needs native review
develop-close = Quitter le développement
# machine-drafted, needs native review
develop-history = Historique
# machine-drafted, needs native review
# A card's on/off switch. $module: the module's name.
develop-module-on = { $module } activé
develop-card-menu = Options de { $module }
develop-card-changed = Modifié
# machine-drafted, needs native review
# The speed-edit readout over the image. $control: "Exposure"; $value: "+0.70 EV".
develop-speed-hud = { $control } : { $value }
# machine-drafted, needs native review
develop-filmstrip = Film fixe

## Develop's module cards and controls.

# machine-drafted, needs native review
develop-module-exposure = Exposition
develop-module-light = Lumière
develop-look-profile = Profil
develop-look-standard = Standard
develop-look-film = Film
develop-look-natural = Naturel
develop-module-grading = Étalonnage
develop-add-tool = Ajouter un outil
# machine-drafted, needs native review
develop-module-white-balance = Balance des blancs
# machine-drafted, needs native review
develop-module-tone = Tons
# machine-drafted, needs native review
develop-module-contrast = Contraste et plage
# machine-drafted, needs native review
develop-module-colour = Couleur
# machine-drafted, needs native review
develop-module-curve = Courbe
# machine-drafted, needs native review
develop-module-crop = Recadrage et redressement
# machine-drafted, needs native review
# A module this version has no card for.
develop-module-other = Autre module
# machine-drafted, needs native review
develop-exposure = Exposition
# machine-drafted, needs native review
develop-black-level = Niveau de noir
# machine-drafted, needs native review
develop-temperature = Température
# machine-drafted, needs native review
develop-tint = Teinte
# machine-drafted, needs native review
develop-highlights = Hautes lumières
# machine-drafted, needs native review
develop-shadows = Ombres
# machine-drafted, needs native review
develop-contrast = Contraste
# machine-drafted, needs native review
develop-whites = Blancs
# machine-drafted, needs native review
develop-blacks = Noirs
# machine-drafted, needs native review
# The view transform: how scene brightness becomes screen brightness.
# machine-drafted, needs native review
# machine-drafted, needs native review
# machine-drafted, needs native review
# machine-drafted, needs native review
# HDR card: how far above SDR white the HDR picture may go, and which picture to show.
develop-hdr = HDR
# machine-drafted, needs native review
develop-hdr-limit = Limite HDR
# machine-drafted, needs native review
develop-hdr-preview = Afficher
# machine-drafted, needs native review
develop-hdr-preview-hdr = HDR
# machine-drafted, needs native review
develop-hdr-preview-sdr = SDR
# machine-drafted, needs native review
develop-vibrance = Vibrance
# machine-drafted, needs native review
develop-saturation = Saturation
# machine-drafted, needs native review
develop-colour-wheels = Roues chromatiques
# machine-drafted, needs native review
develop-curve = Courbe des tonalités
# machine-drafted, needs native review
develop-module-local-contrast = Contraste local
# machine-drafted, needs native review
# A switch: exposure set from the photo's own brightness.
develop-auto-exposure = Exposition automatique
# machine-drafted, needs native review
# How far the tone equalizer's highlights and shadows spread across an area.
develop-smoothing = Lissage
# machine-drafted, needs native review
# Local contrast: fine detail, and larger-scale contrast.
develop-texture = Texture
# machine-drafted, needs native review
develop-clarity = Clarté
# machine-drafted, needs native review
# Levels on the curve card: the input that becomes black, the middle and white.
develop-levels-black = Point noir
# machine-drafted, needs native review
develop-levels-mid = Point médian
# machine-drafted, needs native review
develop-levels-white = Point blanc
# machine-drafted, needs native review
develop-module-detail = Détail
# machine-drafted, needs native review
develop-module-denoise = Réduction du bruit
# machine-drafted, needs native review
develop-module-sharpen = Netteté
# machine-drafted, needs native review
develop-noise-luminance = Bruit de luminance
# machine-drafted, needs native review
develop-noise-detail = Détail du bruit
# machine-drafted, needs native review
develop-noise-colour = Bruit de couleur
# machine-drafted, needs native review
develop-sharpen-amount = Gain de netteté
# machine-drafted, needs native review
develop-sharpen-radius = Rayon de netteté
# machine-drafted, needs native review
develop-sharpen-detail = Détail de netteté
# machine-drafted, needs native review
develop-sharpen-masking = Masquage
# machine-drafted, needs native review
develop-capture-amount = Netteté de capture
# machine-drafted, needs native review
develop-capture-radius = Rayon de capture
# machine-drafted, needs native review
develop-straighten = Redresser
# machine-drafted, needs native review
# History rows of the crop tool.
develop-crop = Recadrer
# machine-drafted, needs native review
develop-crop-turn = Pivoter
# machine-drafted, needs native review
develop-crop-flip = Retourner
# machine-drafted, needs native review
# The lens card: the camera's own corrections (its switch) and manual ones.
develop-module-lens = Corrections de l’objectif
develop-module-optics = Optique
# machine-drafted, needs native review
develop-lens-distortion = Distorsion
# machine-drafted, needs native review
develop-lens-vignette = Vignetage
# machine-drafted, needs native review
# Colour fringes at the edges of the frame (lateral chromatic aberration), per colour.
develop-lens-tca-red = Frange rouge
# machine-drafted, needs native review
develop-lens-tca-blue = Frange bleue
# machine-drafted, needs native review
# The Lensfun lens profile for raws without corrections of their own: a picker and a note.
develop-lensfun-profile = Profil d’objectif
# machine-drafted, needs native review
# The picker's first option: the profile matched to the file's lens name.
develop-lensfun-auto = Automatique
# machine-drafted, needs native review
develop-lensfun-off = Aucun
# machine-drafted, needs native review
# $lens: "Canon EF 24-70mm f/2.8L II USM". Lensfun is a name; keep it.
develop-lensfun-used = Lensfun : { $lens }
# machine-drafted, needs native review
develop-lensfun-none-chosen = Aucun profil d’objectif.
# machine-drafted, needs native review
# $lens: the lens name the file gives.
develop-lensfun-no-match = Aucun profil Lensfun pour { $lens }.
# machine-drafted, needs native review
develop-lensfun-no-profile = Aucun profil Lensfun pour cet objectif.
# machine-drafted, needs native review
develop-lensfun-no-name = Le fichier n’indique pas l’objectif. Choisissez-le ci-dessus.
# machine-drafted, needs native review
# $count: how many lenses match equally well (2 or more).
develop-lensfun-ambiguous = { $count } objectifs correspondent. Choisissez-en un ci-dessus.

## The crop tool over the photo (key R). Said to screen readers, or shown over the photo.

# machine-drafted, needs native review
develop-crop-open = Recadrer. Faites glisser les bords, ou Ctrl+glisser le long d’une ligne qui doit être horizontale. Entrée valide, Échap revient en arrière.
# machine-drafted, needs native review
develop-crop-kept = Recadrage conservé
# machine-drafted, needs native review
develop-crop-cancelled = Recadrage annulé
# machine-drafted, needs native review
develop-crop-reset = Recadrage et redressement réinitialisés
# machine-drafted, needs native review
# $angle: "-1.5".
develop-crop-straightened = Redressé de { $angle }°
# machine-drafted, needs native review
# $aspect: "3:2", or one of the two names below.
develop-crop-aspect = Format { $aspect }
# machine-drafted, needs native review
develop-crop-aspect-free = libre
# machine-drafted, needs native review
develop-crop-aspect-original = d’origine
# machine-drafted, needs native review
develop-crop-turned-right = Pivoté à droite
# machine-drafted, needs native review
develop-crop-turned-left = Pivoté à gauche
# machine-drafted, needs native review
develop-crop-flipped-horizontal = Retourné de gauche à droite
# machine-drafted, needs native review
develop-crop-flipped-vertical = Retourné de haut en bas
# machine-drafted, needs native review
# The label over the photo. $aspect: as above; $guide: one of the guide names below.
develop-crop-label = Format { $aspect } · Repère : { $guide }
# machine-drafted, needs native review
develop-crop-keys = A format · X permuter · O repère · [ ] pivoter · H V retourner · Entrée valider · Échap retour
# machine-drafted, needs native review
develop-crop-guide-thirds = tiers
# machine-drafted, needs native review
develop-crop-guide-golden = nombre d’or
# machine-drafted, needs native review
develop-crop-guide-diagonal = diagonales
# machine-drafted, needs native review
develop-crop-guide-grid = grille

## Perspective: keystone and upright (its own card; guides are drawn in the crop tool).

# machine-drafted, needs native review
develop-module-perspective = Perspective
# machine-drafted, needs native review
# Upright: how the camera's tilt is found.
develop-persp-upright = Aplomb
# machine-drafted, needs native review
develop-persp-mode-off = Désactivé
# machine-drafted, needs native review
develop-persp-mode-auto = Auto
# machine-drafted, needs native review
develop-persp-mode-level = Niveau
# machine-drafted, needs native review
develop-persp-mode-vertical = Verticales
# machine-drafted, needs native review
develop-persp-mode-full = Complet
# machine-drafted, needs native review
develop-persp-mode-guided = Guidé
# machine-drafted, needs native review
develop-persp-vertical = Verticale
# machine-drafted, needs native review
develop-persp-horizontal = Horizontale
# machine-drafted, needs native review
develop-persp-rotate = Rotation
# machine-drafted, needs native review
develop-persp-aspect = Proportions
# machine-drafted, needs native review
develop-persp-scale = Échelle
# machine-drafted, needs native review
develop-persp-offset-x = Latéral
# machine-drafted, needs native review
develop-persp-offset-y = Haut ou bas
# machine-drafted, needs native review
# Zoom so no empty edges show.
develop-persp-auto-scale = Éviter les bords vides
# machine-drafted, needs native review
develop-persp-guides = Repères
# machine-drafted, needs native review
develop-persp-focal = Focale
# machine-drafted, needs native review
develop-persp-draw-guides = Tracer des repères
# machine-drafted, needs native review
# $n: guides drawn; $max: 4.
develop-persp-draw-guides-n = Tracer des repères ({ $n } sur { $max })
# machine-drafted, needs native review
develop-persp-reset = Réinitialiser la perspective
# machine-drafted, needs native review
# $mode: one of the mode names above.
develop-persp-mode-set = Aplomb : { $mode }
# machine-drafted, needs native review
develop-persp-analysing = Recherche des lignes…
# machine-drafted, needs native review
# $n: the guides drawn.
develop-persp-reset-confirm = Réinitialiser la perspective ? Vos { $n } repères sont supprimés. Annuler les rétablit.
# machine-drafted, needs native review
develop-persp-reset-ok = Réinitialiser
# machine-drafted, needs native review
develop-persp-reset-done = Perspective réinitialisée
# machine-drafted, needs native review
# $n: the guide's number.
develop-persp-guide-remove-confirm = Supprimer le repère { $n } ? Annuler le rétablit.
# machine-drafted, needs native review
develop-persp-guide-remove-ok = Supprimer
# machine-drafted, needs native review
develop-persp-guide-removed = Repère supprimé
# machine-drafted, needs native review
# $vertical, $horizontal: the slider values.
develop-persp-keystone-now = Verticale { $vertical }, horizontale { $horizontal }
# machine-drafted, needs native review
# $error: what went wrong.
develop-persp-failed = Échec de l’aplomb : { $error }
# machine-drafted, needs native review
# $mode: a mode name; the angles in degrees ("2.5").
develop-persp-solved = { $mode } : inclinaison { $vertical }°, pivot { $horizontal }°, rotation { $rotate }°
# machine-drafted, needs native review
# $mode: a mode name.
develop-perspective-few-lines = { $mode } a besoin d’au moins deux lignes droites bien distinctes. Essayez un autre mode, ou tracez des repères.
# machine-drafted, needs native review
# $n: 4.
develop-persp-guides-full = { $n } repères au maximum. Faites glisser une extrémité pour en déplacer un.
# machine-drafted, needs native review
develop-persp-guides-on = Repères. Faites glisser le long d’une ligne qui doit être verticale ou horizontale.
# machine-drafted, needs native review
develop-persp-guides-off = Recadrer
# machine-drafted, needs native review
develop-persp-guide-moved = Repère déplacé
# machine-drafted, needs native review
# $n: how many guides now.
develop-persp-guide-added = Repère { $n } ajouté
# machine-drafted, needs native review
develop-persp-guides-keys = Glisser ajoute · Glisser une extrémité déplace · Suppr supprime · Shift+R recadrer
# machine-drafted, needs native review
# $n: guides drawn; $max: 4.
develop-persp-guides-label = Repères : { $n } sur { $max }
# machine-drafted, needs native review
# The three colour wheels.
develop-wheel-shadows = Ombres
# machine-drafted, needs native review
develop-wheel-midtones = Tons moyens
# machine-drafted, needs native review
develop-wheel-highlights = Hautes lumières
# machine-drafted, needs native review
# The luminance slider under each colour wheel (read by screen readers).
develop-luminance-shadows = Luminance des ombres
# machine-drafted, needs native review
develop-luminance-midtones = Luminance des tons moyens
# machine-drafted, needs native review
develop-luminance-highlights = Luminance des hautes lumières
# machine-drafted, needs native review
# The fourth colour wheel: the whole picture.
develop-wheel-global = Global
# machine-drafted, needs native review
develop-luminance-global = Luminance globale
# machine-drafted, needs native review
# Colour card: colourfulness changed evenly, and the lightness of colourful areas.
develop-chroma = Intensité des couleurs
# machine-drafted, needs native review
develop-brilliance = Luminosité des couleurs

## Colour mixer: hue, saturation and brightness per colour.

# machine-drafted, needs native review
develop-module-colour-mixer = Mélangeur de couleurs
# machine-drafted, needs native review
develop-colour-red = Rouge
# machine-drafted, needs native review
develop-colour-orange = Orange
# machine-drafted, needs native review
develop-colour-yellow = Jaune
# machine-drafted, needs native review
develop-colour-green = Vert
# machine-drafted, needs native review
develop-colour-aqua = Turquoise
# machine-drafted, needs native review
develop-colour-blue = Bleu
# machine-drafted, needs native review
develop-colour-purple = Violet
# machine-drafted, needs native review
develop-colour-magenta = Magenta
# machine-drafted, needs native review
# $colour: one of the colour names above.
develop-mixer-hue = Teinte – { $colour }
# machine-drafted, needs native review
develop-mixer-saturation = Saturation – { $colour }
# machine-drafted, needs native review
develop-mixer-brightness = Luminance – { $colour }

## Point colour: pick a colour in the photo, then shift it and its neighbours.

# machine-drafted, needs native review
develop-module-point-colour = Couleur ponctuelle
# machine-drafted, needs native review
develop-point-pick = Choisir une couleur
# machine-drafted, needs native review
# On the pick button once a colour is picked.
develop-point-pick-again = Choisir une autre couleur
# machine-drafted, needs native review
# Said and shown while picking.
develop-point-picking = Cliquez sur la couleur à modifier. Échap annule.
# machine-drafted, needs native review
develop-point-picked = Couleur choisie
# machine-drafted, needs native review
develop-point-cancelled = Choix annulé
# machine-drafted, needs native review
# How far from the picked colour changes reach: overall, then by hue, saturation and lightness.
develop-point-range = Plage
# machine-drafted, needs native review
develop-point-hue-range = Plage de teinte
# machine-drafted, needs native review
develop-point-saturation-range = Étendue de saturation
# machine-drafted, needs native review
develop-point-lightness-range = Étendue de luminosité
# machine-drafted, needs native review
develop-point-hue = Décalage de teinte
# machine-drafted, needs native review
develop-point-saturation = Décalage de saturation
# machine-drafted, needs native review
develop-point-lightness = Décalage de luminosité

## Black and white.

# machine-drafted, needs native review
develop-module-black-white = Noir et blanc
# machine-drafted, needs native review
# $colour: a colour name; how light that colour turns in grey.
develop-bw-mix = Luminance – { $colour }

## LUT: a colour look-up table file.

# machine-drafted, needs native review
develop-module-lut = LUT
# machine-drafted, needs native review
develop-lut-choose = Choisir un fichier LUT…
# machine-drafted, needs native review
# The file chosen. $name: its file name.
develop-lut-file = Fichier : { $name }
# machine-drafted, needs native review
# The file can't be used. $name: its file name.
develop-lut-unreadable = Impossible de lire { $name }
# machine-drafted, needs native review
# The file dialog.
develop-lut-dialog = Choisir un fichier LUT
# machine-drafted, needs native review
develop-lut-kind = Fichiers LUT
# machine-drafted, needs native review
# The encoding the LUT was made for.
develop-lut-space = Conçue pour
# machine-drafted, needs native review
develop-lut-space-linear = Linéaire (Rec.2020)
# machine-drafted, needs native review
develop-lut-space-acescg = ACES linéaire (ACEScg)
# machine-drafted, needs native review
develop-lut-space-acescct = ACES log (ACEScct)
# machine-drafted, needs native review
develop-lut-space-srgb = Photo (sRGB)
# machine-drafted, needs native review
develop-lut-space-rec709 = Vidéo (Rec.709)
# machine-drafted, needs native review
develop-lut-interpolation = Méthode
# machine-drafted, needs native review
develop-lut-tetrahedral = Précise
# machine-drafted, needs native review
develop-lut-trilinear = Simple
# machine-drafted, needs native review
develop-lut-strength = Intensité

## Develop's history and status.

# machine-drafted, needs native review
develop-history-original = Original
# machine-drafted, needs native review
# A module switched on or off. $module: its name.
develop-history-on = { $module } activé
# machine-drafted, needs native review
develop-history-off = { $module } désactivé
# machine-drafted, needs native review
# Said to screen readers after a click on a history row. $step: the row's name.
develop-at-original = Retour à l’original
# machine-drafted, needs native review
develop-at-step = Retour à : { $step }
# machine-drafted, needs native review
develop-undone = Annulé : { $step }
# machine-drafted, needs native review
develop-redone = Rétabli : { $step }
# machine-drafted, needs native review
develop-entered = Développement. Les retouches ne modifient jamais le fichier.
# machine-drafted, needs native review
develop-left = Développement quitté
# machine-drafted, needs native review
develop-split-on = Avant et après côte à côte
# machine-drafted, needs native review
develop-split-off = Retouche seule
# machine-drafted, needs native review
develop-showing-before = Affichage de l’original
# machine-drafted, needs native review
develop-showing-after = Affichage de la retouche
# machine-drafted, needs native review
# Labels over the image.
develop-before = Avant
# machine-drafted, needs native review
develop-after = Après
# machine-drafted, needs native review
develop-working = Développement…
# machine-drafted, needs native review
develop-preview-failed = Impossible d’afficher la retouche de ce fichier

## Develop's commands (palette, cheat sheet, Settings → Keys) and why they are unavailable.

# machine-drafted, needs native review
keys-mode-develop = Développement
# machine-drafted, needs native review
# The cheat sheet's heading over the speed-edit keys.
cheatsheet-develop-speed = Retouche rapide : maintenir, puis faire défiler
# machine-drafted, needs native review
palette-develop = Développement
# machine-drafted, needs native review
palette-develop-help = Retoucher cette photo sans modifier le fichier. Relancer pour quitter.
# machine-drafted, needs native review
palette-develop-before-after = Avant et après
# machine-drafted, needs native review
palette-develop-before-after-help = Afficher l’original, ou de nouveau la retouche.
# machine-drafted, needs native review
palette-develop-split = Avant et après côte à côte
# machine-drafted, needs native review
palette-develop-split-help = Afficher l’original et la retouche côte à côte.
# machine-drafted, needs native review
palette-develop-panel = Aller au panneau Développement
# machine-drafted, needs native review
palette-develop-panel-help = Placer le clavier dans le panneau. Échap revient.
# machine-drafted, needs native review
palette-develop-speed-exposure = Retouche rapide de l’exposition
# machine-drafted, needs native review
palette-develop-speed-exposure-help = Maintenir, puis faire défiler ou appuyer sur les flèches pour changer l’exposition.
# machine-drafted, needs native review
palette-develop-speed-contrast = Retouche rapide du contraste
# machine-drafted, needs native review
palette-develop-speed-contrast-help = Maintenir, puis faire défiler ou appuyer sur les flèches pour changer le contraste.
# machine-drafted, needs native review
palette-develop-speed-highlights = Retouche rapide des hautes lumières
# machine-drafted, needs native review
palette-develop-speed-highlights-help = Maintenir, puis faire défiler ou appuyer sur les flèches pour changer les hautes lumières.
# machine-drafted, needs native review
palette-develop-speed-shadows = Retouche rapide des ombres
# machine-drafted, needs native review
palette-develop-speed-shadows-help = Maintenir, puis faire défiler ou appuyer sur les flèches pour changer les ombres.
# machine-drafted, needs native review
palette-develop-speed-whites = Retouche rapide des blancs
# machine-drafted, needs native review
palette-develop-speed-whites-help = Maintenir, puis faire défiler ou appuyer sur les flèches pour changer les blancs.
# machine-drafted, needs native review
palette-develop-speed-blacks = Retouche rapide des noirs
# machine-drafted, needs native review
palette-develop-speed-blacks-help = Maintenir, puis faire défiler ou appuyer sur les flèches pour changer les noirs.
# machine-drafted, needs native review
palette-develop-speed-temperature = Retouche rapide de la température
# machine-drafted, needs native review
palette-develop-speed-temperature-help = Maintenir, puis faire défiler ou appuyer sur les flèches pour changer la température de la balance des blancs.
# machine-drafted, needs native review
palette-develop-speed-vibrance = Retouche rapide de la vibrance
# machine-drafted, needs native review
palette-develop-speed-vibrance-help = Maintenir, puis faire défiler ou appuyer sur les flèches pour changer la vibrance.
# machine-drafted, needs native review
palette-develop-crop = Recadrer et redresser
# machine-drafted, needs native review
palette-develop-crop-help = Recadrer, redresser, pivoter ou retourner la photo. Entrée valide, Échap revient en arrière.
# machine-drafted, needs native review
disabled-not-photo = Uniquement pour les photos
# machine-drafted, needs native review
disabled-develop-off = Le développement est désactivé dans les profils Médical et Science
# machine-drafted, needs native review
develop-module-corrections = Corrections
# machine-drafted, needs native review
develop-module-raw = Raw
# machine-drafted, needs native review
develop-off = Désactivé
# machine-drafted, needs native review
develop-on = Activé
# machine-drafted, needs native review
develop-highlight-method = Hautes lumières écrêtées
# machine-drafted, needs native review
develop-highlight-opposed = Reconstruire la couleur
# machine-drafted, needs native review
develop-highlight-rebuild = Reconstruire les grandes zones
# machine-drafted, needs native review
develop-highlight-clip = Écrêter en blanc
# machine-drafted, needs native review
develop-highlight-threshold = Seuil d’écrêtage
# machine-drafted, needs native review
develop-module-hot-pixels = Pixels chauds
# machine-drafted, needs native review
develop-hot-threshold = Seuil des pixels chauds
# machine-drafted, needs native review
develop-module-ca = Aberration chromatique
# machine-drafted, needs native review
develop-ca-mode = Franges
# machine-drafted, needs native review
develop-ca-auto = Automatique
# machine-drafted, needs native review
develop-ca-manual = Manuel
# machine-drafted, needs native review
develop-ca-red = Décalage rouge
# machine-drafted, needs native review
develop-ca-blue = Décalage bleu
# machine-drafted, needs native review
develop-module-defringe = Suppression des franges
# machine-drafted, needs native review
develop-defringe = Suppression des franges
# machine-drafted, needs native review
develop-defringe-purple = Teinte violette
# machine-drafted, needs native review
develop-defringe-green = Teinte verte
# machine-drafted, needs native review
develop-defringe-range = Plage de teinte
# machine-drafted, needs native review
develop-defringe-edge = Contraste des bords
# machine-drafted, needs native review
develop-module-flat-field = Images de calibrage
# machine-drafted, needs native review
develop-calibration = Calibrage
# machine-drafted, needs native review
develop-calibration-auto = Depuis le dossier
# machine-drafted, needs native review
develop-calibration-chosen = Fichiers choisis
# machine-drafted, needs native review
develop-flat-choose = Choisir un flat…
# machine-drafted, needs native review
develop-dark-choose = Choisir un dark…
# machine-drafted, needs native review
develop-flat-file = Image flat : { $name }
# machine-drafted, needs native review
develop-dark-file = Image dark : { $name }
# machine-drafted, needs native review
develop-flat-use = Image flat
# machine-drafted, needs native review
develop-flat-use-full = Vignetage et poussières
# machine-drafted, needs native review
develop-flat-use-vignetting = Vignetage seulement
# machine-drafted, needs native review
develop-flat-dialog = Choisir un flat
# machine-drafted, needs native review
develop-dark-dialog = Choisir un dark
# machine-drafted, needs native review
develop-raw-kind = Fichiers raw
# machine-drafted, needs native review
develop-flat-frame = Image flat
# machine-drafted, needs native review
develop-dark-frame = Image dark

# machine-drafted, needs native review
# machine-drafted, needs native review
develop-mask-add = Ajouter le masque
# machine-drafted, needs native review
develop-mask-kind-ellipse = Ellipse
# machine-drafted, needs native review
develop-mask-kind-rectangle = Rectangle
# machine-drafted, needs native review
develop-mask-kind-polygon = Polygone
# machine-drafted, needs native review
develop-mask-kind-linear = Dégradé linéaire
# machine-drafted, needs native review
develop-mask-kind-radial = Dégradé radial
# machine-drafted, needs native review
develop-mask-kind-brush = Pinceau
# machine-drafted, needs native review
develop-mask-kind-luminance = Plage de luminance
# machine-drafted, needs native review
develop-mask-kind-hue = Plage de teinte
# machine-drafted, needs native review
develop-mask-kind-chroma = Plage de saturation
# machine-drafted, needs native review
develop-mask-kind-colour = Plage de couleur
# machine-drafted, needs native review
develop-mask-kind-other = Autre masque
# machine-drafted, needs native review
develop-mask-row = { $kind } { $n }
# machine-drafted, needs native review
develop-mask-mode = Fusion
# machine-drafted, needs native review
develop-blend-normal = Normal
# machine-drafted, needs native review
develop-blend-multiply = Produit
# machine-drafted, needs native review
develop-blend-screen = Superposition claire
# machine-drafted, needs native review
develop-blend-overlay = Incrustation
# machine-drafted, needs native review
develop-blend-soft-light = Lumière douce
# machine-drafted, needs native review
develop-blend-lightness = Luminosité seule
# machine-drafted, needs native review
develop-blend-colour = Couleur seule
# machine-drafted, needs native review
develop-mask-amount = Force du masque
# machine-drafted, needs native review
develop-mask-invert = Inverser le masque
# machine-drafted, needs native review
develop-mask-off = Désactivé
# machine-drafted, needs native review
develop-mask-on = Activé
# machine-drafted, needs native review
develop-mask-refine = Affiner les bords
# machine-drafted, needs native review
develop-mask-show = Afficher le masque
# machine-drafted, needs native review
develop-mask-show-red = Rouge
# machine-drafted, needs native review
develop-mask-show-grey = Gris
# machine-drafted, needs native review
develop-mask-remove = Supprimer le masque
# machine-drafted, needs native review
develop-mask-combine-add = Ajouter
# machine-drafted, needs native review
develop-mask-combine-subtract = Soustraire
# machine-drafted, needs native review
develop-mask-combine-intersect = Intersection
# machine-drafted, needs native review
develop-mask-row-opacity = Opacité
# machine-drafted, needs native review
develop-mask-row-invert = Inverser
# machine-drafted, needs native review
develop-mask-feather = Contour progressif
# machine-drafted, needs native review
develop-mask-roundness = Arrondi
# machine-drafted, needs native review
develop-mask-brush-size = Taille du pinceau
# machine-drafted, needs native review
develop-mask-brush-feather = Contour du pinceau
# machine-drafted, needs native review
develop-mask-brush-flow = Flux
# machine-drafted, needs native review
develop-mask-brush-mode = Pinceau
# machine-drafted, needs native review
develop-mask-brush-paint = Peindre
# machine-drafted, needs native review
develop-mask-brush-erase = Effacer
# machine-drafted, needs native review
develop-mask-range-from = De
# machine-drafted, needs native review
develop-mask-range-full-from = Plein dès
# machine-drafted, needs native review
develop-mask-range-full-to = Plein jusqu’à
# machine-drafted, needs native review
develop-mask-range-to = À
# machine-drafted, needs native review
develop-mask-range-test = Tester
# machine-drafted, needs native review
develop-mask-range-input = Entrée
# machine-drafted, needs native review
develop-mask-range-output = Sortie
# machine-drafted, needs native review
develop-mask-colour-pick = Prélever une couleur
# machine-drafted, needs native review
develop-mask-colour-pick-more = Prélever une autre couleur
# machine-drafted, needs native review
develop-mask-colour-range = Plage
# machine-drafted, needs native review
develop-mask-colour-falloff = Atténuation
# machine-drafted, needs native review
develop-mask-edit = Modifier sur l’image
# machine-drafted, needs native review
develop-mask-row-remove = Supprimer { $name }
# machine-drafted, needs native review
develop-mask-remove-confirm = Supprimer le masque de { $module } ? Ses formes disparaissent. La photo sur le disque reste.
# machine-drafted, needs native review
develop-mask-row-remove-confirm = Supprimer { $name } de { $module } ? Les autres masques et la photo sur le disque restent.
# machine-drafted, needs native review
develop-mask-remove-ok = Supprimer
# machine-drafted, needs native review
develop-mask-removed = Masque supprimé.
# machine-drafted, needs native review
develop-mask-added = { $name } ajouté.
# machine-drafted, needs native review
develop-mask-full = Ce masque a déjà autant de parties qu’il peut en contenir.
# machine-drafted, needs native review
develop-mask-editing = Modification de { $name }. Faites glisser les poignées, ou utilisez Tab et les flèches.
# machine-drafted, needs native review
develop-mask-painting = Peinture de { $name }. Faites glisser sur l’image.
# machine-drafted, needs native review
develop-mask-picking = Cliquez sur l’image pour prélever une couleur.
# machine-drafted, needs native review
develop-mask-picked = Couleur prélevée.
# machine-drafted, needs native review
develop-mask-done = Modification du masque terminée.
# machine-drafted, needs native review
develop-mask-handles = Poignées du masque
# machine-drafted, needs native review
develop-handle-centre = Centre
# machine-drafted, needs native review
develop-handle-width = Largeur
# machine-drafted, needs native review
develop-handle-height = Hauteur
# machine-drafted, needs native review
develop-handle-rotate = Rotation
# machine-drafted, needs native review
develop-handle-feather = Contour progressif
# machine-drafted, needs native review
develop-handle-inner = Bord intérieur
# machine-drafted, needs native review
develop-handle-start = Début
# machine-drafted, needs native review
develop-handle-end = Fin
# machine-drafted, needs native review
develop-handle-point = Point { $n }
# machine-drafted, needs native review
develop-instance = Calque
# machine-drafted, needs native review
develop-instance-n = { $n }
# machine-drafted, needs native review
develop-instance-chosen = Calque { $n }.
# machine-drafted, needs native review
develop-instance-new = Nouveau calque
# machine-drafted, needs native review
develop-instance-duplicate = Dupliquer le calque
# machine-drafted, needs native review
develop-instance-delete = Supprimer le calque
# machine-drafted, needs native review
develop-instance-earlier = Avancer
# machine-drafted, needs native review
develop-instance-later = Reculer
# machine-drafted, needs native review
develop-instance-delete-confirm = Supprimer le calque { $n } de { $module } ? Ses réglages et masques disparaissent. La photo sur le disque reste.
# machine-drafted, needs native review
develop-instance-delete-ok = Supprimer
# machine-drafted, needs native review
develop-instance-deleted = Calque supprimé.
# machine-drafted, needs native review
develop-history-mask = Masque : { $module }
# machine-drafted, needs native review
develop-history-mask-shapes = Formes du masque : { $module }
# machine-drafted, needs native review
develop-history-instance-new = Nouveau : { $module }
# machine-drafted, needs native review
develop-history-instance-delete = Supprimé : { $module }
# machine-drafted, needs native review
develop-history-instance-move = Ordre : { $module }
# Other tools' edits and sidecar conflicts.
# machine-drafted, needs native review
develop-others-title = Autres logiciels
# machine-drafted, needs native review
develop-darktable-edited = Modifié dans darktable :
# machine-drafted, needs native review
develop-darktable-open = Ouvrir dans darktable
# machine-drafted, needs native review
develop-darktable-opening = Ouverture dans darktable
# machine-drafted, needs native review
develop-darktable-failed = darktable n’a pas démarré : { $reason }
# machine-drafted, needs native review
develop-lightroom-import = Importer depuis Lightroom
# machine-drafted, needs native review
develop-lightroom-imported = { $count ->
    [one] 1 réglage Lightroom importé. Le résultat est approximatif.
    [many] { $count } réglages Lightroom importés. Le résultat est approximatif.
   *[other] { $count } réglages Lightroom importés. Le résultat est approximatif.
}
# machine-drafted, needs native review
develop-lightroom-nothing = Rien à importer : ces réglages Lightroom ne changent rien ici.
# machine-drafted, needs native review
develop-history-lightroom = Importé de Lightroom (approximatif)
# machine-drafted, needs native review
develop-conflict-title = La retouche de { $file } a changé hors de Specular
# machine-drafted, needs native review
develop-conflict-file = Fichier annexe : enregistré { $time }
# machine-drafted, needs native review
develop-conflict-library = Bibliothèque : enregistrée { $time }
# machine-drafted, needs native review
develop-conflict-writer = Le fichier annexe a été écrit par { $writer }.
# machine-drafted, needs native review
develop-conflict-detail = Gardez-en une. L’autre est remplacée.
# machine-drafted, needs native review
develop-conflict-use-file = Utiliser celle du fichier
# machine-drafted, needs native review
develop-conflict-keep-library = Garder celle de la bibliothèque
# machine-drafted, needs native review
develop-conflict-used-file = Retouche du fichier annexe utilisée
# machine-drafted, needs native review
develop-conflict-kept-library = Retouche de la bibliothèque conservée
# machine-drafted, needs native review
toast-develop-edit-newer-process = Cette retouche vient d’un Specular plus récent. Elle peut paraître différente ici.
# machine-drafted, needs native review
info-darktable-edited = Modifié dans darktable
# machine-drafted, needs native review
info-darktable-no-modules = Aucun module actif
# machine-drafted, needs native review
info-darktable-group = Fichier
# machine-drafted, needs native review
toast-develop-edit-partial = Cette retouche vient d’un Specular plus récent. Les parties que cette version ne sait pas afficher sont omises. Les changements ici ne sont pas enregistrés.
# machine-drafted, needs native review
toast-develop-sidecar-unreadable = Le fichier de retouche à côté de cette photo est illisible. Il est gardé tel quel. Les changements ici ne sont pas enregistrés.
# machine-drafted, needs native review
develop-demosaic = Détails raw
develop-demosaic-directional = Standard
develop-demosaic-dual = Zones unies lissées
develop-demosaic-fast = Rapide
develop-dual-threshold = Contraste des zones unies
