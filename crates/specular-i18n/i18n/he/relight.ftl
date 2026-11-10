# Specular — reflectance transformation images (PTM, RTI, Relight): moving the light, shading modes, the light widget. machine-drafted, needs native review

keys-mode-relight = תאורה מחדש
palette-relight-left = הזזת האור שמאלה
palette-relight-right = הזזת האור ימינה
palette-relight-up = הזזת האור למעלה
palette-relight-down = הזזת האור למטה
palette-relight-shading = מצב התאורה הבא
palette-relight-reset = אור מלמעלה
palette-relight-widget = הצגה או הסתרה של בקר האור
disabled-not-relightable = רק לתמונות הניתנות לתאורה מחדש (PTM, RTI, Relight)
# Relighting modes: as a toast when they change and in the light widget's label.
relight-mode-default = מואר
relight-mode-specular = הדגשת ברק
relight-mode-normals = נורמלים של פני השטח
relight-mode-diffuse-gain = הגבר דיפוזי
# The light widget (a hemisphere seen from above) for screen readers and as its tooltip.
# $azimuth: compass angle of the light in degrees (0 = from the top of the image, clockwise);
# $elevation: degrees above the image plane (90 = straight above). $mode: one of the modes above.
relight-light = אור מ־{ $azimuth }°, בגובה { $elevation }°, { $mode }
# Shown once when a relightable image opens.
relight-hint = Shift+גרירה מזיזה את האור
