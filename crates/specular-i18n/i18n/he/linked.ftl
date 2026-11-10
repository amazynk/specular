# Linked views: several DICOM series side by side, scrolling together by position, with
# reference lines, a 3D cursor, window and zoom sync, and manual alignment of priors.

# machine-drafted, needs native review
palette-compare-series = השוואת סדרות (תצוגות מקושרות)
# machine-drafted, needs native review
palette-compare-layout = פריסת התצוגות הבאה
# machine-drafted, needs native review
palette-next-viewport = התצוגה הבאה
# machine-drafted, needs native review
palette-prev-viewport = התצוגה הקודמת
# machine-drafted, needs native review
palette-reference-lines = הצגה או הסתרה של קווי ייחוס
# machine-drafted, needs native review
palette-sync-window = סנכרון החלון הבא
# machine-drafted, needs native review
palette-sync-zoom = זום והזזה משותפים לפי גודל
# machine-drafted, needs native review
palette-link-series = קישור הסדרה הבא
# machine-drafted, needs native review
palette-align-series = יישור הסדרות האחרות לכאן
# machine-drafted, needs native review
palette-compare-series-help = הצגת כמה סדרות זו לצד זו, עם גלילה משותפת.
# machine-drafted, needs native review
palette-compare-layout-help = הצגת יותר או פחות תצוגות.
# machine-drafted, needs native review
palette-next-viewport-help = הפיכת התצוגה הבאה לפעילה.
# machine-drafted, needs native review
palette-prev-viewport-help = הפיכת התצוגה הקודמת לפעילה.
# machine-drafted, needs native review
palette-reference-lines-help = סימון המקום שבו הפרוסה הפעילה חותכת את התצוגות האחרות.
# machine-drafted, needs native review
palette-sync-window-help = בחירת התצוגות שחולקות שינוי של חלון/רמה: כבוי, אותה שיטת הדמיה או הכול.
# machine-drafted, needs native review
palette-sync-zoom-help = הצגת כל התצוגות באותו גודל והזזתן יחד.
# machine-drafted, needs native review
palette-link-series-help = בחירה כיצד סדרה זו עוקבת אחרי האחרות.
# machine-drafted, needs native review
palette-align-series-help = יישור סדרה קודמת לסדרה זו בפרוסות המוצגות.
# machine-drafted, needs native review
disabled-not-comparing = יש להשוות סדרות קודם

# The bar above the viewports. Toggles show their state by colour; the others name it.
# machine-drafted, needs native review
linked-layout = פריסה: { $cols } × { $rows }
# machine-drafted, needs native review
linked-lines = קווי ייחוס
# machine-drafted, needs native review
linked-zoom = סנכרון זום
# machine-drafted, needs native review
linked-window-sync = סנכרון חלון: { $mode ->
    [off] כבוי
    [modality] אותה שיטת הדמיה
   *[all] הכול
}
# machine-drafted, needs native review
linked-link = קישור: { $mode ->
    [frame] מערכת ייחוס
    [position] מיקום המטופל
   *[off] כבוי
}
# machine-drafted, needs native review
linked-align = יישור לכאן

# One viewport. $active is "yes" for the active one.
# machine-drafted, needs native review
linked-viewport = { $active ->
    [yes] תצוגה { $n } מתוך { $count }, פעילה
   *[no] תצוגה { $n } מתוך { $count }
}
# machine-drafted, needs native review
linked-empty = יש לגרור לכאן סדרה
# machine-drafted, needs native review
linked-series = { $number } · { $modality } · { $description }
# machine-drafted, needs native review
linked-slice = פרוסה { $n } מתוך { $count } · { $position } מ״מ
# machine-drafted, needs native review
linked-in-gap = בתוך מרווח: הפרוסה הקרובה ביותר במרחק { $distance } מ״מ
# machine-drafted, needs native review
linked-outside = מחוץ לסדרה זו ({ $distance } מ״מ)
# machine-drafted, needs native review
linked-not-linked = לא מקושרת
# machine-drafted, needs native review
linked-aligned = מיושרת ידנית ({ $offset } מ״מ)
# machine-drafted, needs native review
linked-cursor = סמן תלת-ממד ב-{ $x }, { $y }, { $z } מ״מ
# machine-drafted, needs native review
linked-chip = { $label }, { $count ->
        [one] תמונה אחת
        [two] שתי תמונות
       *[other] { $count } תמונות
    }

# Toasts and announcements.
# machine-drafted, needs native review
linked-opened = { $count ->
    [one] יש כאן סדרה אחת בלבד: יש לגרור סדרה נוספת מדפדפן הבדיקות
    [two] משווים שתי סדרות
   *[other] משווים { $count } סדרות
}
# machine-drafted, needs native review
linked-no-geometry = לסדרה זו אין מיקומי פרוסות: לא ניתן לקשר אותה
# machine-drafted, needs native review
linked-active = תצוגה { $n }: { $series }
# machine-drafted, needs native review
linked-aligned-count = { $count ->
    [0] אין מה ליישר: הסדרות האחרות חולקות מערכת ייחוס זו
    [one] סדרה אחת יושרה לפעילה, בהפעלה זו
    [two] שתי סדרות יושרו לפעילה, בהפעלה זו
   *[other] { $count } סדרות יושרו לפעילה, בהפעלה זו
}
