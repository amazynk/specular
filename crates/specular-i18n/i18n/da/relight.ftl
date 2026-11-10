# Specular — reflectance transformation images (PTM, RTI, Relight): moving the light, shading modes, the light widget. machine-drafted, needs native review

keys-mode-relight = Genbelysning
palette-relight-left = Flyt lyset til venstre
palette-relight-right = Flyt lyset til højre
palette-relight-up = Flyt lyset op
palette-relight-down = Flyt lyset ned
palette-relight-shading = Næste belysningstilstand
palette-relight-reset = Lys ovenfra
palette-relight-widget = Vis eller skjul lysvælgeren
disabled-not-relightable = Kun for billeder, der kan genbelyses (PTM, RTI, Relight)
# Relighting modes: as a toast when they change and in the light widget's label.
relight-mode-default = Belyst
relight-mode-specular = Glansforstærkning
relight-mode-normals = Overfladenormaler
relight-mode-diffuse-gain = Diffus forstærkning
# The light widget (a hemisphere seen from above) for screen readers and as its tooltip.
# $azimuth: compass angle of the light in degrees (0 = from the top of the image, clockwise);
# $elevation: degrees above the image plane (90 = straight above). $mode: one of the modes above.
relight-light = Lys fra { $azimuth }°, { $elevation }° højt, { $mode }
# Shown once when a relightable image opens.
relight-hint = Shift+træk flytter lyset
