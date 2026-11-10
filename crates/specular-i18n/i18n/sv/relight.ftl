# Specular — reflectance transformation images (PTM, RTI, Relight): moving the light, shading modes, the light widget. machine-drafted, needs native review

keys-mode-relight = Ombelysning
palette-relight-left = Flytta ljuset åt vänster
palette-relight-right = Flytta ljuset åt höger
palette-relight-up = Flytta ljuset uppåt
palette-relight-down = Flytta ljuset nedåt
palette-relight-shading = Nästa belysningsläge
palette-relight-reset = Ljus ovanifrån
palette-relight-widget = Visa eller dölj ljusreglaget
disabled-not-relightable = Endast för bilder som kan ombelysas (PTM, RTI, Relight)
# Relighting modes: as a toast when they change and in the light widget's label.
relight-mode-default = Belyst
relight-mode-specular = Glansförstärkning
relight-mode-normals = Ytnormaler
relight-mode-diffuse-gain = Diffus förstärkning
# The light widget (a hemisphere seen from above) for screen readers and as its tooltip.
# $azimuth: compass angle of the light in degrees (0 = from the top of the image, clockwise);
# $elevation: degrees above the image plane (90 = straight above). $mode: one of the modes above.
relight-light = Ljus från { $azimuth }°, { $elevation }° högt, { $mode }
# Shown once when a relightable image opens.
relight-hint = Shift+dra flyttar ljuset
