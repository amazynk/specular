# Specular — reflectance transformation images (PTM, RTI, Relight): moving the light, shading modes, the light widget. machine-drafted, needs native review

keys-mode-relight = Reiluminación
palette-relight-left = Mover la luz a la izquierda
palette-relight-right = Mover la luz a la derecha
palette-relight-up = Mover la luz hacia arriba
palette-relight-down = Mover la luz hacia abajo
palette-relight-shading = Siguiente modo de iluminación
palette-relight-reset = Luz desde arriba
palette-relight-widget = Mostrar u ocultar el control de luz
disabled-not-relightable = Solo para imágenes reiluminables (PTM, RTI, Relight)
# Relighting modes: as a toast when they change and in the light widget's label.
relight-mode-default = Iluminado
relight-mode-specular = Realce especular
relight-mode-normals = Normales de superficie
relight-mode-diffuse-gain = Ganancia difusa
# The light widget (a hemisphere seen from above) for screen readers and as its tooltip.
# $azimuth: compass angle of the light in degrees (0 = from the top of the image, clockwise);
# $elevation: degrees above the image plane (90 = straight above). $mode: one of the modes above.
relight-light = Luz desde { $azimuth }°, a { $elevation }° de altura, { $mode }
# Shown once when a relightable image opens.
relight-hint = Shift+arrastrar para mover la luz
