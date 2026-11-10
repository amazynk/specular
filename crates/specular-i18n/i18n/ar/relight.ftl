# Specular — reflectance transformation images (PTM, RTI, Relight): moving the light, shading modes, the light widget. machine-drafted, needs native review

keys-mode-relight = إعادة الإضاءة
palette-relight-left = تحريك الضوء إلى اليسار
palette-relight-right = تحريك الضوء إلى اليمين
palette-relight-up = تحريك الضوء إلى الأعلى
palette-relight-down = تحريك الضوء إلى الأسفل
palette-relight-shading = وضع الإضاءة التالي
palette-relight-reset = الضوء من الأعلى
palette-relight-widget = إظهار أداة الضوء أو إخفاؤها
disabled-not-relightable = للصور القابلة لإعادة الإضاءة فقط (PTM وRTI وRelight)
# Relighting modes: as a toast when they change and in the light widget's label.
relight-mode-default = مُضاء
relight-mode-specular = تعزيز الانعكاس المرآوي
relight-mode-normals = نواظم السطح
relight-mode-diffuse-gain = تضخيم الانتشار
# The light widget (a hemisphere seen from above) for screen readers and as its tooltip.
# $azimuth: compass angle of the light in degrees (0 = from the top of the image, clockwise);
# $elevation: degrees above the image plane (90 = straight above). $mode: one of the modes above.
relight-light = الضوء من { $azimuth }°، بارتفاع { $elevation }°، { $mode }
# Shown once when a relightable image opens.
relight-hint = Shift+سحب لتحريك الضوء
