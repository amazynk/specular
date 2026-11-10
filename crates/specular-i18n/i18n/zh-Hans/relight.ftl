# Specular — reflectance transformation images (PTM, RTI, Relight): moving the light, shading modes, the light widget. machine-drafted, needs native review

keys-mode-relight = 重新打光
palette-relight-left = 将光源向左移动
palette-relight-right = 将光源向右移动
palette-relight-up = 将光源向上移动
palette-relight-down = 将光源向下移动
palette-relight-shading = 下一个打光模式
palette-relight-reset = 顶光
palette-relight-widget = 显示或隐藏光源控件
disabled-not-relightable = 仅适用于可重新打光的图像（PTM、RTI、Relight）
# Relighting modes: as a toast when they change and in the light widget's label.
relight-mode-default = 光照
relight-mode-specular = 镜面增强
relight-mode-normals = 表面法线
relight-mode-diffuse-gain = 漫反射增益
# The light widget (a hemisphere seen from above) for screen readers and as its tooltip.
# $azimuth: compass angle of the light in degrees (0 = from the top of the image, clockwise);
# $elevation: degrees above the image plane (90 = straight above). $mode: one of the modes above.
relight-light = 光源 { $azimuth }°，高度 { $elevation }°，{ $mode }
# Shown once when a relightable image opens.
relight-hint = Shift+拖动以移动光源
