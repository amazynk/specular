# machine-drafted, needs native review
# Specular — פתולוגיה: הפרדת צבענים, ייצוא וייבוא הערות, נפחי תוויות מעל נפחים. עברית.

## לוח הפקודות
palette-stain-separation = הפרדת צבענים
palette-stain-1 = הצגה או הסתרה של צבען 1
palette-stain-2 = הצגה או הסתרה של צבען 2
palette-stain-3 = הצגה או הסתרה של צבען 3
palette-stain-mode = צבענים כאור או כצפיפות אופטית
palette-stain-estimate = הערכת הצבענים מהאזור
palette-stain-normalise = נרמול צבעי הצבענים
palette-stain-reference = שימוש באזור כייחוס לצבענים
palette-stain-reset = וקטורי הצבענים שפורסמו
palette-export-annotations = ייצוא הערות…
palette-import-annotations = ייבוא הערות…
palette-label-volume = שכבת על של נפח תוויות…

## סיבות להשבתה
disabled-not-colour-image = נדרשת תמונה צבעונית
disabled-stains-off = יש להפעיל קודם את הפרדת הצבענים
disabled-no-region = יש לשרטט קודם אזור
disabled-no-slide-annotations = אין הערות, אזורים או נקודות לייצוא
disabled-no-label-target = יש לפתוח נפח בתצוגות הרב-מישוריות, או תמונה רפואית

## שמות הצבענים
stain-hematoxylin = המטוקסילין
stain-eosin = אאוזין
stain-dab = DAB
stain-residual = שארית
stain-set-he = H&E
stain-set-hdab = H-DAB
stain-set-hedab = H&E-DAB

## הודעות ומקרא של הצבענים
stain-off = הפרדת הצבענים כבויה
# { $set }: H&E,‏ H-DAB או H&E-DAB.
stain-on = צבענים: { $set }
stain-shown = { $stain } מוצג
stain-hidden = { $stain } מוסתר
stain-mode-light = צבענים כאור
stain-mode-od = צבענים כצפיפות אופטית
# כותרת המקרא: "H&E · אור", "H-DAB · צפיפות אופטית".
stain-legend-light = { $set } · אור
stain-legend-od = { $set } · צפיפות אופטית
stain-legend-estimated = הוערך מהאזור
stain-legend-normalised = מנורמל
stain-legend-published = וקטורים שפורסמו
# { $pixels }: הפיקסלים הצבועים שבהם נעשה שימוש.
stain-estimated = הצבענים הוערכו מ-{ $pixels } פיקסלים
stain-estimate-failed = לא ניתן להעריך את הצבענים: { $reason }
stain-estimate-background = רוב האזור הוא רקע
stain-estimate-one-colour = באזור יש צבע אחד בלבד
stain-estimate-three = ההערכה דורשת H&E או H-DAB
stain-estimate-no-pixels = הפיקסלים של האזור עדיין לא נטענו
stain-reference-set = האזור נקבע כייחוס לצבענים
stain-normalise-on = צבעי הצבענים מנורמלים
stain-normalise-off = צבעי הצבענים כפי שנסרקו
stain-reset-done = וקטורי הצבענים שפורסמו

## חלון ייצוא ההערות
annot-export-title = ייצוא הערות
annot-export-format = תבנית
annot-export-format-coco = COCO JSON
annot-export-format-geojson = GeoJSON (QuPath)
annot-export-format-masks = מסכות תוויות
annot-export-output = מסכות בתור
annot-export-output-png = אריחי PNG
annot-export-output-tiff = אריחי TIFF
annot-export-output-ome = פירמידת OME-TIFF
annot-export-region = אזור
annot-export-region-slide = השקופית כולה
annot-export-region-view = האזור הנראה
annot-export-region-drawn = האזור האחרון ששורטט
annot-export-level = רמה
# { $level }: 0 לרזולוציה מלאה; { $width } × { $height } גודלה בפיקסלים.
annot-export-level-value = { $level } · { $width } × { $height }
annot-export-tile = גודל אריח
annot-export-empty = אריחים ריקים
annot-export-empty-skip = דילוג
annot-export-empty-keep = כתיבה
annot-export-what = { $count ->
    [one] צורה אחת
    [two] שתי צורות
   *[other] { $count } צורות
}, { $classes ->
    [one] מחלקה אחת
    [two] שתי מחלקות
   *[other] { $classes } מחלקות
}
annot-export-lines = קווים אינם נכללים ב-COCO ובמסכות.
annot-export-export = ייצוא…
annot-export-writing = כותב…
# { $files }: הקבצים שנכתבו.
annot-export-done = { $files ->
    [one] יוצא קובץ אחד
    [two] יוצאו שני קבצים
   *[other] יוצאו { $files } קבצים
}
annot-export-failed = הייצוא נכשל: { $reason }
annot-import-done = { $count ->
    [one] יובאה הערה אחת
    [two] יובאו שתי הערות
   *[other] יובאו { $count } הערות
}
annot-import-failed = לא ניתן לייבא: { $reason }
annot-import-skipped = { $count ->
    [one] לצורה אחת לא היה קו מתאר שמיש
    [two] לשתי צורות לא היה קו מתאר שמיש
   *[other] ל-{ $count } צורות לא היה קו מתאר שמיש
}
annot-file-coco = COCO JSON
annot-file-geojson = GeoJSON

## נפחי תוויות
label-volume-file = נפח תוויות
label-volume-table = טבלת צבעים
# { $file }: שם הקובץ של נפח התוויות; { $count }: התוויות שבו.
label-volume-loaded = { $count ->
    [one] { $file }: תווית אחת
    [two] { $file }: שתי תוויות
   *[other] { $file }: { $count } תוויות
}
label-volume-failed = לא ניתן להציג את { $file } כשכבת על: { $reason }
label-volume-outside = { $file } אינו חופף לנפח הזה
label-volume-not-whole = הערכים בו אינם מספרים שלמים
label-volume-colour = יש בו צבע, לא תוויות
label-volume-empty = אין בו תוויות
label-volume-too-many = יש בו יותר מדי תוויות
label-volume-too-large = הוא גדול מדי לדגימה מחדש
label-volume-degenerate = הכיוון שלו אינו תקין
# תווית ללא שם בטבלת הצבעים.
label-volume-unnamed = תווית { $value }
