# Specular — reflectance transformation images (PTM, RTI, Relight): moving the light, shading modes, the light widget. machine-drafted, needs native review

keys-mode-relight = Neubeleuchtung
palette-relight-left = Licht nach links bewegen
palette-relight-right = Licht nach rechts bewegen
palette-relight-up = Licht nach oben bewegen
palette-relight-down = Licht nach unten bewegen
palette-relight-shading = Nächster Beleuchtungsmodus
palette-relight-reset = Licht von oben
palette-relight-widget = Lichtsteuerung ein- oder ausblenden
disabled-not-relightable = Nur für neu beleuchtbare Bilder (PTM, RTI, Relight)
# Relighting modes: as a toast when they change and in the light widget's label.
relight-mode-default = Beleuchtet
relight-mode-specular = Glanzverstärkung
relight-mode-normals = Oberflächennormalen
relight-mode-diffuse-gain = Diffuse Verstärkung
# The light widget (a hemisphere seen from above) for screen readers and as its tooltip.
# $azimuth: compass angle of the light in degrees (0 = from the top of the image, clockwise);
# $elevation: degrees above the image plane (90 = straight above). $mode: one of the modes above.
relight-light = Licht aus { $azimuth }°, { $elevation }° hoch, { $mode }
# Shown once when a relightable image opens.
relight-hint = Shift+Ziehen bewegt das Licht
