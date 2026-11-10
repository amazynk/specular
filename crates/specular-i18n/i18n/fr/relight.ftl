# Specular — reflectance transformation images (PTM, RTI, Relight): moving the light, shading modes, the light widget. machine-drafted, needs native review

keys-mode-relight = Rééclairage
palette-relight-left = Déplacer la lumière vers la gauche
palette-relight-right = Déplacer la lumière vers la droite
palette-relight-up = Déplacer la lumière vers le haut
palette-relight-down = Déplacer la lumière vers le bas
palette-relight-shading = Mode d’éclairage suivant
palette-relight-reset = Lumière d’en haut
palette-relight-widget = Afficher ou masquer le contrôle de la lumière
disabled-not-relightable = Uniquement pour les images rééclairables (PTM, RTI, Relight)
# Relighting modes: as a toast when they change and in the light widget's label.
relight-mode-default = Éclairé
relight-mode-specular = Rehaussement spéculaire
relight-mode-normals = Normales de surface
relight-mode-diffuse-gain = Gain diffus
# The light widget (a hemisphere seen from above) for screen readers and as its tooltip.
# $azimuth: compass angle of the light in degrees (0 = from the top of the image, clockwise);
# $elevation: degrees above the image plane (90 = straight above). $mode: one of the modes above.
relight-light = Lumière à { $azimuth }°, { $elevation }° de hauteur, { $mode }
# Shown once when a relightable image opens.
relight-hint = Shift+glisser pour déplacer la lumière
