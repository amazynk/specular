# Specular — reflectance transformation images (PTM, RTI, Relight): moving the light, shading modes, the light widget. machine-drafted, needs native review

keys-mode-relight = Reiluminação
palette-relight-left = Mover a luz para a esquerda
palette-relight-right = Mover a luz para a direita
palette-relight-up = Mover a luz para cima
palette-relight-down = Mover a luz para baixo
palette-relight-shading = Modo de iluminação seguinte
palette-relight-reset = Luz de cima
palette-relight-widget = Mostrar ou ocultar o controlo da luz
disabled-not-relightable = Apenas para imagens reilumináveis (PTM, RTI, Relight)
# Relighting modes: as a toast when they change and in the light widget's label.
relight-mode-default = Iluminado
relight-mode-specular = Realce especular
relight-mode-normals = Normais da superfície
relight-mode-diffuse-gain = Ganho difuso
# The light widget (a hemisphere seen from above) for screen readers and as its tooltip.
# $azimuth: compass angle of the light in degrees (0 = from the top of the image, clockwise);
# $elevation: degrees above the image plane (90 = straight above). $mode: one of the modes above.
relight-light = Luz de { $azimuth }°, a { $elevation }° de altura, { $mode }
# Shown once when a relightable image opens.
relight-hint = Shift+arrastar move a luz
