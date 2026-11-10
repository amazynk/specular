# Specular — reflectance transformation images (PTM, RTI, Relight): moving the light, shading modes, the light widget. English.

keys-mode-relight = Relighting
palette-relight-left = Move the light left
palette-relight-right = Move the light right
palette-relight-up = Move the light up
palette-relight-down = Move the light down
palette-relight-shading = Next relighting mode
palette-relight-reset = Light from above
palette-relight-widget = Show or hide the light widget
disabled-not-relightable = Only for relightable images (PTM, RTI, Relight)
# Relighting modes: as a toast when they change and in the light widget's label.
relight-mode-default = Lit
relight-mode-specular = Specular enhancement
relight-mode-normals = Surface normals
relight-mode-diffuse-gain = Diffuse gain
# The light widget (a hemisphere seen from above) for screen readers and as its tooltip.
# $azimuth: compass angle of the light in degrees (0 = from the top of the image, clockwise);
# $elevation: degrees above the image plane (90 = straight above). $mode: one of the modes above.
relight-light = Light from { $azimuth }°, { $elevation }° high, { $mode }
# Shown once when a relightable image opens.
relight-hint = Shift+drag to move the light
