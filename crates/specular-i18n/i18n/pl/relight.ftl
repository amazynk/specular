# Specular — reflectance transformation images (PTM, RTI, Relight): moving the light, shading modes, the light widget. machine-drafted, needs native review

keys-mode-relight = Zmiana oświetlenia
palette-relight-left = Przesuń światło w lewo
palette-relight-right = Przesuń światło w prawo
palette-relight-up = Przesuń światło w górę
palette-relight-down = Przesuń światło w dół
palette-relight-shading = Następny tryb oświetlenia
palette-relight-reset = Światło z góry
palette-relight-widget = Pokaż lub ukryj kontrolkę światła
disabled-not-relightable = Tylko dla obrazów z możliwością zmiany oświetlenia (PTM, RTI, Relight)
# Relighting modes: as a toast when they change and in the light widget's label.
relight-mode-default = Oświetlony
relight-mode-specular = Wzmocnienie odblasków
relight-mode-normals = Normalne powierzchni
relight-mode-diffuse-gain = Wzmocnienie rozproszenia
# The light widget (a hemisphere seen from above) for screen readers and as its tooltip.
# $azimuth: compass angle of the light in degrees (0 = from the top of the image, clockwise);
# $elevation: degrees above the image plane (90 = straight above). $mode: one of the modes above.
relight-light = Światło z { $azimuth }°, wysokość { $elevation }°, { $mode }
# Shown once when a relightable image opens.
relight-hint = Shift+przeciągnij, aby przesunąć światło
