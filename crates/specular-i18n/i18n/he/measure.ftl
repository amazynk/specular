# Specular — כלי המדידה (אורכים, זוויות, אזורים והסטטיסטיקה שלהם בכל תמונה). טיוטת מכונה, נדרשת בדיקה של דובר שפת אם.

palette-measure = מדידה
palette-measure-copy-csv = העתקת המדידות כ-CSV
palette-measure-export = ייצוא המדידות…
toast-copied-measurements = המדידות הועתקו כ-CSV

measure-tool-title = מדידה
measure-tool-list = מדידות
measure-tool-count = { $count ->
    [0] אין מדידות
    [one] מדידה אחת
    [two] שתי מדידות
   *[other] { $count } מדידות
}
measure-tool-empty = יש לבחור כלי ולצייר על התמונה.

measure-tool-point = נקודה
measure-tool-line = קו
measure-tool-polyline = קו שבור
measure-tool-angle = זווית
measure-tool-rect = מלבן
measure-tool-ellipse = אליפסה
measure-tool-polygon = מצולע
measure-tool-freehand = יד חופשית
measure-tool-calibrate = כיול
# machine-drafted, needs native review
measure-tool-spot = נקודה

measure-tool-undo = ביטול פעולה
measure-tool-delete = מחיקה
measure-tool-copy = העתקת CSV
measure-tool-export = ייצוא…
measure-tool-import = ייבוא…
measure-tool-done = סיום
measure-tool-unit = יחידה: { $unit }
measure-tool-apply = החלה

measure-tool-hint-line = יש לגרור או ללחוץ על שתי נקודות. החצים מזיזים את הכוונת, רווח מציב נקודה.
measure-tool-hint-angle = יש ללחוץ על שלוש נקודות; השנייה היא הקודקוד.
measure-tool-hint-rect = יש לגרור או ללחוץ על שתי פינות.
measure-tool-hint-ellipse = יש לגרור או ללחוץ על שתי פינות של המסגרת שלה.
measure-tool-hint-polygon = יש ללחוץ על הפינות; Enter או לחיצה על הפינה הראשונה סוגרים את הצורה.
measure-tool-hint-calibrate = יש לצייר קו לאורך משהו שאורכו ידוע.
# machine-drafted, needs native review
measure-tool-hint-spot = יש ללחוץ על נקודה כדי לקרוא את הטמפרטורה שלה.
measure-tool-known-length = אורך ידוע: { $value } { $unit } (יש להקליד את המספר, U מחליף יחידה, Enter מחיל)
measure-tool-calibrated = כויל: פיקסל אחד הוא { $size } { $unit }
measure-tool-bad-length = יש להקליד אורך גדול מאפס.

measure-tool-item = { $n }. { $shape }
measure-tool-area = שטח { $value }
measure-tool-mean = ממוצע { $mean } ± { $sd }
measure-tool-means = ממוצע { $values }
measure-tool-range = טווח { $min } – { $max } על פני { $count } px
measure-tool-pixels = על פני { $count } px
measure-tool-stats-pending = קורא פיקסלים…
measure-tool-stats-none = אין כאן ערכי פיקסלים
measure-tool-approximate = מעותק מוקטן (בקירוב)

measure-tool-deleted = המדידה נמחקה
measure-tool-undone = בוטל
measure-tool-nothing = אין מדידות בתמונה זו
measure-tool-exported = המדידות נשמרו ב-{ $name }
measure-tool-export-failed = לא ניתן היה לשמור את המדידות: { $reason }
measure-tool-imported = { $count ->
    [one] יובאה מדידה אחת.
    [two] יובאו שתי מדידות.
   *[other] יובאו { $count } מדידות.
}
measure-tool-skipped = { $count ->
    [one] לא ניתן היה לקרוא צורה אחת.
    [two] לא ניתן היה לקרוא שתי צורות.
   *[other] לא ניתן היה לקרוא { $count } צורות.
}
measure-tool-import-failed = לא ניתן היה לקרוא את המדידות: { $reason }
measure-tool-too-large = הקובץ גדול מדי.
# machine-drafted, needs native review
measure-tool-sr-not-dicom = לשמירה כ-DICOM SR נדרשת תמונת DICOM. יש לשמור כ-CSV או כ-GeoJSON במקום זאת.

measure-source-uncalibrated = פיקסלים (לא מכויל)
measure-source-dicom-pixel-spacing = מרווח פיקסלים DICOM
measure-source-dicom-calibrated = מרווח מכויל DICOM
measure-source-dicom-imager-pixel-spacing = מרווח DICOM בגלאי
measure-source-dicom-magnification = מרווח DICOM מתוקן להגדלה
measure-source-dicom-ultrasound-region = אזור אולטרסאונד DICOM
measure-source-tiff-resolution = רזולוציית TIFF
measure-source-exif-resolution = רזולוציית EXIF
measure-source-ome-physical-size = גודל פיזי OME
measure-source-fits-wcs = קואורדינטות עולם FITS
measure-source-vendor-tag = מטא-נתוני היצרן
measure-source-drawing-units = יחידות השרטוט
# machine-drafted, needs native review
measure-source-drawing-scale = מקנה המידה של השרטוט
# machine-drafted, needs native review
measure-source-drawing-scale-ratio = מהשרטוט (קנה מידה { $scale })
# machine-drafted, needs native review
measure-source-drawing-scale-viewport = מהשרטוט (קנה מידה { $scale }, חלון תצוגה { $viewport })
# machine-drafted, needs native review
measure-source-map-scale = קנה המידה של המפה במרכז (בקירוב; כלי המפות מודדים על פני כדור הארץ)
measure-source-user = הכיול שלך
