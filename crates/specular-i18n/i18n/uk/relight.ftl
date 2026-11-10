# Specular — reflectance transformation images (PTM, RTI, Relight): moving the light, shading modes, the light widget. machine-drafted, needs native review

keys-mode-relight = Зміна освітлення
palette-relight-left = Посунути світло ліворуч
palette-relight-right = Посунути світло праворуч
palette-relight-up = Посунути світло вгору
palette-relight-down = Посунути світло вниз
palette-relight-shading = Наступний режим освітлення
palette-relight-reset = Світло згори
palette-relight-widget = Показати або сховати регулятор світла
disabled-not-relightable = Лише для зображень зі змінним освітленням (PTM, RTI, Relight)
# Relighting modes: as a toast when they change and in the light widget's label.
relight-mode-default = Освітлено
relight-mode-specular = Підсилення відблисків
relight-mode-normals = Нормалі поверхні
relight-mode-diffuse-gain = Дифузне підсилення
# The light widget (a hemisphere seen from above) for screen readers and as its tooltip.
# $azimuth: compass angle of the light in degrees (0 = from the top of the image, clockwise);
# $elevation: degrees above the image plane (90 = straight above). $mode: one of the modes above.
relight-light = Світло з { $azimuth }°, висота { $elevation }°, { $mode }
# Shown once when a relightable image opens.
relight-hint = Shift+перетягування переміщує світло
