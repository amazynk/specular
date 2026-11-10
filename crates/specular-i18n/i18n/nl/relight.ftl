# Specular — reflectance transformation images (PTM, RTI, Relight): moving the light, shading modes, the light widget. machine-drafted, needs native review

keys-mode-relight = Herbelichting
palette-relight-left = Licht naar links verplaatsen
palette-relight-right = Licht naar rechts verplaatsen
palette-relight-up = Licht omhoog verplaatsen
palette-relight-down = Licht omlaag verplaatsen
palette-relight-shading = Volgende belichtingsmodus
palette-relight-reset = Licht van boven
palette-relight-widget = Lichtregelaar tonen of verbergen
disabled-not-relightable = Alleen voor herbelichtbare afbeeldingen (PTM, RTI, Relight)
# Relighting modes: as a toast when they change and in the light widget's label.
relight-mode-default = Belicht
relight-mode-specular = Glansversterking
relight-mode-normals = Oppervlaktenormalen
relight-mode-diffuse-gain = Diffuse versterking
# The light widget (a hemisphere seen from above) for screen readers and as its tooltip.
# $azimuth: compass angle of the light in degrees (0 = from the top of the image, clockwise);
# $elevation: degrees above the image plane (90 = straight above). $mode: one of the modes above.
relight-light = Licht vanuit { $azimuth }°, { $elevation }° hoog, { $mode }
# Shown once when a relightable image opens.
relight-hint = Shift+slepen verplaatst het licht
