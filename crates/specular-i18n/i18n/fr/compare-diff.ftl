# Compare: split, signed difference, flicker, alignment, metrics and the pixel probe
# (DESIGN.md §6.3).

## Actions (palette, cheat sheet, remote control)
palette-compare-with = Comparer avec…
palette-compare-with-help = Comparer cette image avec une autre de votre choix.
palette-compare-side = Comparer côte à côte
palette-compare-side-help = Afficher les images comparées côte à côte.
palette-compare-split = Comparer avec un séparateur
palette-compare-split-help = Afficher la deuxième image sur la première, coupée par un séparateur déplaçable.
palette-compare-split-direction = Tourner le séparateur
palette-compare-split-direction-help = Placer le séparateur à l’horizontale plutôt qu’à la verticale, ou revenir.
palette-compare-overlay = Comparer en superposition
palette-compare-overlay-help = Afficher la deuxième image sur la première, en fondu.
palette-compare-difference = Comparer la différence
palette-compare-difference-help = Montrer où les images diffèrent ; à nouveau pour le type suivant.
palette-compare-flicker = Faire clignoter les images comparées
palette-compare-flicker-help = Afficher l’autre image immédiatement.
palette-compare-auto-flicker = Clignoter automatiquement
palette-compare-auto-flicker-help = Alterner seul entre les deux images, ou arrêter.
palette-compare-align = Aligner les images comparées
palette-compare-align-help = Aligner des images de tailles différentes : en haut à gauche, centrées, mises à l’échelle ou trouvées automatiquement.
palette-compare-metric-space = Changer d’espace de mesure
palette-compare-metric-space-help = Mesurer en lumière linéaire ou en sRGB encodé.
palette-compare-whole-image = Mesurer toute l’image
palette-compare-whole-image-help = Mesurer toute l’image au lieu de la partie visible, ou revenir.
palette-compare-probe = Sonde de pixel de la comparaison
palette-compare-probe-help = Afficher les valeurs des deux images sous le pointeur.
palette-compare-link = Lier ou délier les images comparées
palette-compare-link-help = Zoomer et déplacer chacune des deux images séparément, ou de nouveau ensemble.
palette-compare-close = Fermer la comparaison
palette-compare-close-help = Revenir à la galerie.
disabled-compare-closed = Ouvrez d’abord la comparaison
disabled-compare-not-two = Fonctionne avec deux images

## Cheat sheet
cheatsheet-compare-split = Séparateur ou côte à côte
cheatsheet-compare-split-direction = Séparateur horizontal ou vertical
cheatsheet-compare-auto-flicker = Clignoter automatiquement
cheatsheet-compare-align = Alignement suivant
cheatsheet-compare-space = Espace de mesure
cheatsheet-compare-whole = Toute l’image ou partie visible
cheatsheet-compare-probe = Sonde de pixel
cheatsheet-compare-link = Lier ou délier deux images

## Settings
settings-gallery-compare-metric-space = Espace de mesure de la comparaison
settings-gallery-compare-metric-space-help = Où la comparaison mesure MAE, MSE, PSNR et SSIM. La lumière linéaire correspond à l’affichage. sRGB concorde avec les autres outils sur les fichiers 8-bit.
settings-gallery-compare-metric-space-linear = Lumière linéaire
settings-gallery-compare-metric-space-srgb = sRGB encodé
settings-gallery-compare-flicker-limit = Clignotement automatique le plus rapide
settings-gallery-compare-flicker-limit-help = Changements d’image par seconde au plus. Au-delà de 3, le clignotement peut déclencher des crises d’épilepsie.

## Toolbar and captions
compare-split = Séparateur
compare-split-down = Séparateur vertical
compare-split-across = Séparateur horizontal
compare-link = Liées
compare-auto-flicker = Auto
compare-flicker-rate = { $rate } changements par seconde
compare-gain = Gain ×{ $gain }
compare-probe = Sonde
compare-kind-signed = Erreur signée
compare-kind-on-image = Erreur sur l’image
compare-align-top-left = En haut à gauche
compare-align-centre = Centré
compare-align-scaled = B mis à l’échelle de A
compare-align-auto = Alignement auto
compare-space-linear = Lumière linéaire
compare-space-srgb = sRGB
compare-space-help-linear = Mesuré en lumière linéaire, tel qu’affiché après le mappage des tons. Ici, le PSNR n’est pas le PSNR sRGB 8-bit habituel.
compare-space-help-srgb = Mesuré sur les valeurs sRGB encodées, comme le font d’autres outils pour les fichiers 8-bit. Le HDR au-delà du blanc SDR compte comme du blanc.
compare-space-help-common = ΔE2000 est toujours en CIELAB (D65), blanc SDR à L* 100, HDR écrêté. Les images de plus de { $size } pixels sont mesurées sur une copie de { $size } pixels.
compare-region-view = Partie visible
compare-region-whole = Toute l’image
compare-ssim = SSIM { $ssim }
compare-ssim-small = SSIM nécessite 11 pixels
compare-delta-e = ΔE { $mean } ({ PERCENT(95) } : { $p95 }, max { $max })
compare-delta-e-hdr = HDR écrêté pour ΔE
compare-aligned-centre = Centré, { $width } × { $height } comparés
compare-aligned-scaled = B mis à l’échelle { $scale }×, { $width } × { $height } comparés
compare-aligned-auto = Aligné { $angle }° { $scale }×, { $width } × { $height } comparés
compare-aligning = Alignement…
compare-align-failed = Impossible d’aligner ces images. Retour en haut à gauche.
compare-split-label = { $b } sur { $a }, séparateur à { PERCENT($percent) }
compare-showing-which = Affiche { $which } : { $name }
compare-auto-flicker-on = Clignotement, { $rate } changements par seconde
compare-auto-flicker-off = Clignotement arrêté
compare-linked = Images liées
compare-unlinked = Images déliées : chacune se déplace seule
compare-with-title = Comparer avec
compare-probe-delta-e = ΔE2000
compare-probe-a11y = A : { $a }. B : { $b }. ΔE2000 { $delta }.
compare-a11y-divider = Séparateur
compare-a11y-space = Espace de mesure
compare-a11y-align = Alignement
