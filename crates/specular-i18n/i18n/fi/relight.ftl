# Specular — reflectance transformation images (PTM, RTI, Relight): moving the light, shading modes, the light widget. machine-drafted, needs native review

keys-mode-relight = Uudelleenvalaisu
palette-relight-left = Siirrä valoa vasemmalle
palette-relight-right = Siirrä valoa oikealle
palette-relight-up = Siirrä valoa ylös
palette-relight-down = Siirrä valoa alas
palette-relight-shading = Seuraava valaisutila
palette-relight-reset = Valo ylhäältä
palette-relight-widget = Näytä tai piilota valon säädin
disabled-not-relightable = Vain uudelleenvalaistaville kuville (PTM, RTI, Relight)
# Relighting modes: as a toast when they change and in the light widget's label.
relight-mode-default = Valaistu
relight-mode-specular = Kiiltojen korostus
relight-mode-normals = Pinnan normaalit
relight-mode-diffuse-gain = Diffuusi vahvistus
# The light widget (a hemisphere seen from above) for screen readers and as its tooltip.
# $azimuth: compass angle of the light in degrees (0 = from the top of the image, clockwise);
# $elevation: degrees above the image plane (90 = straight above). $mode: one of the modes above.
relight-light = Valo suunnasta { $azimuth }°, korkeus { $elevation }°, { $mode }
# Shown once when a relightable image opens.
relight-hint = Shift+vedä siirtää valoa
