# Specular — reflectance transformation images (PTM, RTI, Relight): moving the light, shading modes, the light widget. machine-drafted, needs native review

keys-mode-relight = Reilluminazione
palette-relight-left = Sposta la luce a sinistra
palette-relight-right = Sposta la luce a destra
palette-relight-up = Sposta la luce in alto
palette-relight-down = Sposta la luce in basso
palette-relight-shading = Modalità di illuminazione successiva
palette-relight-reset = Luce dall’alto
palette-relight-widget = Mostra o nascondi il controllo della luce
disabled-not-relightable = Solo per immagini reilluminabili (PTM, RTI, Relight)
# Relighting modes: as a toast when they change and in the light widget's label.
relight-mode-default = Illuminato
relight-mode-specular = Esaltazione speculare
relight-mode-normals = Normali di superficie
relight-mode-diffuse-gain = Guadagno diffuso
# The light widget (a hemisphere seen from above) for screen readers and as its tooltip.
# $azimuth: compass angle of the light in degrees (0 = from the top of the image, clockwise);
# $elevation: degrees above the image plane (90 = straight above). $mode: one of the modes above.
relight-light = Luce da { $azimuth }°, a { $elevation }° di altezza, { $mode }
# Shown once when a relightable image opens.
relight-hint = Shift+trascina per spostare la luce
