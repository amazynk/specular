# Specular — reflectance transformation images (PTM, RTI, Relight): moving the light, shading modes, the light widget. machine-drafted, needs native review

keys-mode-relight = リライティング
palette-relight-left = 光源を左へ移動
palette-relight-right = 光源を右へ移動
palette-relight-up = 光源を上へ移動
palette-relight-down = 光源を下へ移動
palette-relight-shading = 次のライティングモード
palette-relight-reset = 真上からの光
palette-relight-widget = 光源ウィジェットの表示/非表示
disabled-not-relightable = リライティング可能な画像 (PTM, RTI, Relight) のみ
# Relighting modes: as a toast when they change and in the light widget's label.
relight-mode-default = 照明
relight-mode-specular = 鏡面反射強調
relight-mode-normals = 表面法線
relight-mode-diffuse-gain = 拡散ゲイン
# The light widget (a hemisphere seen from above) for screen readers and as its tooltip.
# $azimuth: compass angle of the light in degrees (0 = from the top of the image, clockwise);
# $elevation: degrees above the image plane (90 = straight above). $mode: one of the modes above.
relight-light = 光源 { $azimuth }°、高さ { $elevation }°、{ $mode }
# Shown once when a relightable image opens.
relight-hint = Shift+ドラッグで光源を移動
