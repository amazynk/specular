# Specular — reflectance transformation images (PTM, RTI, Relight): moving the light, shading modes, the light widget. machine-drafted, needs native review

keys-mode-relight = Yeniden aydınlatma
palette-relight-left = Işığı sola taşı
palette-relight-right = Işığı sağa taşı
palette-relight-up = Işığı yukarı taşı
palette-relight-down = Işığı aşağı taşı
palette-relight-shading = Sonraki aydınlatma modu
palette-relight-reset = Işık yukarıdan
palette-relight-widget = Işık denetimini göster veya gizle
disabled-not-relightable = Yalnızca yeniden aydınlatılabilir görüntüler için (PTM, RTI, Relight)
# Relighting modes: as a toast when they change and in the light widget's label.
relight-mode-default = Aydınlatılmış
relight-mode-specular = Speküler vurgulama
relight-mode-normals = Yüzey normalleri
relight-mode-diffuse-gain = Dağınık kazanç
# The light widget (a hemisphere seen from above) for screen readers and as its tooltip.
# $azimuth: compass angle of the light in degrees (0 = from the top of the image, clockwise);
# $elevation: degrees above the image plane (90 = straight above). $mode: one of the modes above.
relight-light = Işık { $azimuth }° yönünden, { $elevation }° yükseklikte, { $mode }
# Shown once when a relightable image opens.
relight-hint = Işığı taşımak için Shift+sürükleyin
