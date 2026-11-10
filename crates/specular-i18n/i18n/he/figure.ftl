# Figures, key images and teaching files.

## Command palette
# machine-drafted, needs native review
palette-export-figure = ייצוא איור…
# machine-drafted, needs native review
palette-export-figure-help = שמירת התצוגה כאיור עם סרגל קנה מידה, סרגל צבעים ותוויות.
# machine-drafted, needs native review
palette-toggle-key-image = סימון כתמונת מפתח
# machine-drafted, needs native review
palette-toggle-key-image-help = סימון התמונה הזו כתמונת מפתח, או הסרת הסימון.
# machine-drafted, needs native review
palette-copy-key-images = העתקת תמונות מפתח
# machine-drafted, needs native review
palette-copy-key-images-help = העתקת תמונות המפתח כתמונה אחת.
# machine-drafted, needs native review
palette-export-key-images = ייצוא תמונות מפתח…
# machine-drafted, needs native review
palette-export-key-images-help = שמירת תמונות המפתח כשקופיות או כתיקייה של קובצי PNG.
# machine-drafted, needs native review
palette-export-teaching-file = ייצוא קובץ הוראה…
# machine-drafted, needs native review
palette-export-teaching-file-help = שמירת תמונות המפתח ללא פרטי המטופל, לצורכי הוראה.

## The figure dialog
# machine-drafted, needs native review
figure-dialog-title = ייצוא איור
# machine-drafted, needs native review
figure-format = פורמט
# machine-drafted, needs native review
figure-format-png = PNG, ‏16 סיביות
# machine-drafted, needs native review
figure-format-tiff = TIFF, ‏16 סיביות
# machine-drafted, needs native review
figure-region = אזור
# machine-drafted, needs native review
figure-region-visible = האזור הנראה
# machine-drafted, needs native review
figure-region-whole = התמונה כולה
# machine-drafted, needs native review
figure-size = גודל
# machine-drafted, needs native review
figure-size-pixels = פיקסלים של התמונה (1:1)
# $width is a width in pixels.
# machine-drafted, needs native review
figure-size-width = רוחב { $width } px
# machine-drafted, needs native review
figure-scale-bar = סרגל קנה מידה
# machine-drafted, needs native review
figure-scale-bar-none = ללא כיול
# machine-drafted, needs native review
figure-colour-bar = סרגל צבעים
# machine-drafted, needs native review
figure-colour-bar-none = ללא מפת צבעים או חלון
# machine-drafted, needs native review
figure-inset = תקריב משובץ
# machine-drafted, needs native review
figure-inset-rectangle = המלבן האחרון
# machine-drafted, needs native review
figure-inset-none = קודם יש לצייר מלבן
# machine-drafted, needs native review
figure-channels = ערוצים
# machine-drafted, needs native review
figure-channels-merged = ממוזגים
# machine-drafted, needs native review
figure-channels-split = נפרדים וממוזגים
# machine-drafted, needs native review
figure-channels-one = ערוץ אחד
# machine-drafted, needs native review
figure-measurements = מדידות
# $count is the number of measurements.
# machine-drafted, needs native review
figure-measurements-on = { $count ->
    [one] פועל ({ $count })
    [two] פועל ({ $count })
   *[other] פועל ({ $count })
}
# machine-drafted, needs native review
figure-measurements-none = ללא
# machine-drafted, needs native review
figure-title-row = כותרת
# machine-drafted, needs native review
figure-provenance = מקור
# machine-drafted, needs native review
figure-provenance-metadata = בקובץ
# machine-drafted, needs native review
figure-provenance-drawn = בקובץ ומצוירת
# machine-drafted, needs native review
figure-file = קובץ
# machine-drafted, needs native review
figure-choose = בחירה…
# machine-drafted, needs native review
figure-on = פועל
# machine-drafted, needs native review
figure-off = כבוי
# $source is where the value comes from (a calibration, a colour map).
# machine-drafted, needs native review
figure-on-from = פועל · { $source }
# machine-drafted, needs native review
figure-export = ייצוא
# machine-drafted, needs native review
figure-working = מייצא…
# machine-drafted, needs native review
figure-not-full-res = הפיקסלים ברזולוציה מלאה לא נטענו. יש להתקרב ל-{ PERCENT(100) } או לבחור גודל.
# machine-drafted, needs native review
figure-channel-red = אדום
# machine-drafted, needs native review
figure-channel-green = ירוק
# machine-drafted, needs native review
figure-channel-blue = כחול
# $map is a colour map's name; $lo and $hi the values at its ends; $unit their unit.
# machine-drafted, needs native review
figure-legend-map = { $map }, מ-{ $lo } עד { $hi } { $unit }
# $lo and $hi are the window's ends; $unit their unit (HU).
# machine-drafted, needs native review
figure-legend-window = חלון מ-{ $lo } עד { $hi } { $unit }
# $preset is a channel-colour preset's id.
# machine-drafted, needs native review
figure-display-channels = צבעי ערוצים { $preset }
# $channel is the channel shown (red, green, blue, alpha, luma).
# machine-drafted, needs native review
figure-display-channel = ערוץ { $channel }
# machine-drafted, needs native review
figure-display-hilo = סימוני HiLo
# machine-drafted, needs native review
figure-display-as-stored = כפי שנשמר
# $value is the calibration (source and spacing).
# machine-drafted, needs native review
figure-caption-calibration = כיול: { $value }
# $value is the display settings.
# machine-drafted, needs native review
figure-caption-display = תצוגה: { $value }
# machine-drafted, needs native review
figure-not-image = איורים נוצרים מתמונה בתוך הצפייה.
# machine-drafted, needs native review
figure-not-ready = התמונה עדיין נטענת.
# $width and $height are in pixels.
# machine-drafted, needs native review
figure-too-large = גדול מדי: { $width } × { $height } px. יש לבחור את האזור הנראה או גודל.

## Key images
# machine-drafted, needs native review
figure-keys-title = ייצוא תמונות מפתח
# machine-drafted, needs native review
figure-teaching-title = ייצוא קובץ הוראה
# machine-drafted, needs native review
figure-keys-images = תמונות
# $count is the number of key images.
# machine-drafted, needs native review
figure-keys-count = { $count ->
    [one] { $count } תמונת מפתח
    [two] { $count } תמונות מפתח
   *[other] { $count } תמונות מפתח
}
# machine-drafted, needs native review
figure-keys-pptx = שקופיות (PPTX)
# machine-drafted, needs native review
figure-keys-folder = תיקייה של קובצי PNG
# machine-drafted, needs native review
figure-keys-preset = הגדרה מוכנה
# machine-drafted, needs native review
figure-keys-standard = רגיל
# machine-drafted, needs native review
figure-keys-teaching = קובץ הוראה
# machine-drafted, needs native review
figure-keys-overlays-on = מצוירות ובהערות
# machine-drafted, needs native review
figure-keys-masks = טקסט צרוב
# machine-drafted, needs native review
figure-keys-masks-auto = מושחר
# machine-drafted, needs native review
figure-keys-notes = הערות למקרה
# machine-drafted, needs native review
figure-keys-destination = שמירה אל
# machine-drafted, needs native review
figure-keys-empty = עדיין אין תמונות מפתח. קודם יש לסמן תמונה כתמונת מפתח.
# machine-drafted, needs native review
figure-keys-teaching-note = פרטי המטופל מושמטים והטקסט הצרוב מושחר. יש לבדוק כל תמונה לפני השיתוף.
# machine-drafted, needs native review
figure-keys-none-left = לא ניתן היה לייצא אף תמונה ללא פרטי מטופל.
# machine-drafted, needs native review
figure-keys-copied = תמונות המפתח הועתקו
# $count is how many key images there are now.
# machine-drafted, needs native review
figure-key-marked = { $count ->
    [one] סומנה כתמונת מפתח ({ $count })
    [two] סומנה כתמונת מפתח ({ $count })
   *[other] סומנה כתמונת מפתח ({ $count })
}
# $count is how many key images are left.
# machine-drafted, needs native review
figure-key-unmarked = { $count ->
    [one] כבר אינה תמונת מפתח (נותרו { $count })
    [two] כבר אינה תמונת מפתח (נותרו { $count })
   *[other] כבר אינה תמונת מפתח (נותרו { $count })
}
# $n is a frame or page number.
# machine-drafted, needs native review
figure-key-frame = תמונה { $n }
# $n is the key image's number in the set.
# machine-drafted, needs native review
figure-key-n = תמונת מפתח { $n }

## Results
# $name is the file or folder written.
# machine-drafted, needs native review
figure-saved = { $name } נשמר
# $name is the file or folder written; $count how many images were left out.
# machine-drafted, needs native review
figure-saved-skipped = { $count ->
    [one] { $name } נשמר. { $count } תמונה הושמטה: הטקסט הצרוב בה לא נמצא.
    [two] { $name } נשמר. { $count } תמונות הושמטו: הטקסט הצרוב בהן לא נמצא.
   *[other] { $name } נשמר. { $count } תמונות הושמטו: הטקסט הצרוב בהן לא נמצא.
}
# $reason says why.
# machine-drafted, needs native review
figure-failed = לא ניתן לייצא: { $reason }
# $name is a file that is already in the folder.
# machine-drafted, needs native review
figure-folder-taken = { $name } כבר נמצא בתיקייה הזו. יש לבחור תיקייה ריקה.

## The viewer
# machine-drafted, needs native review
figure-key-badge = תמונת מפתח
