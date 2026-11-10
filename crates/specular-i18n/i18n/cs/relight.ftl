# Specular — reflectance transformation images (PTM, RTI, Relight): moving the light, shading modes, the light widget. machine-drafted, needs native review

keys-mode-relight = Změna osvětlení
palette-relight-left = Posunout světlo doleva
palette-relight-right = Posunout světlo doprava
palette-relight-up = Posunout světlo nahoru
palette-relight-down = Posunout světlo dolů
palette-relight-shading = Další režim osvětlení
palette-relight-reset = Světlo shora
palette-relight-widget = Zobrazit nebo skrýt ovladač světla
disabled-not-relightable = Pouze pro obrázky s proměnným osvětlením (PTM, RTI, Relight)
# Relighting modes: as a toast when they change and in the light widget's label.
relight-mode-default = Osvětleno
relight-mode-specular = Zvýraznění odlesků
relight-mode-normals = Normály povrchu
relight-mode-diffuse-gain = Difuzní zesílení
# The light widget (a hemisphere seen from above) for screen readers and as its tooltip.
# $azimuth: compass angle of the light in degrees (0 = from the top of the image, clockwise);
# $elevation: degrees above the image plane (90 = straight above). $mode: one of the modes above.
relight-light = Světlo z { $azimuth }°, výška { $elevation }°, { $mode }
# Shown once when a relightable image opens.
relight-hint = Shift+tažení posouvá světlo
