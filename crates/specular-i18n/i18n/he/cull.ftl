# machine-drafted, needs native review
# Specular — מצב מיון (DESIGN.md §6.3).

palette-cull = כניסה למצב מיון או יציאה ממנו
cheatsheet-cull = מצב מיון
settings-section-cull = מצב מיון
settings-cull-auto-enter = מיון תיקיות RAW
settings-cull-auto-enter-help = פתיחת תיקייה שרובה קובצי RAW ישירות במצב מיון.
settings-cull-auto-advance = התמונה הבאה אחרי סימון
settings-cull-auto-advance-help = מעבר הלאה אחרי בחירה, דחייה או דירוג בכוכבים. תוויות וביטול סימון נשארים בתמונה.
settings-cull-show = הצגה בזוגות RAW+JPEG
settings-cull-show-help = קובץ RAW וקובץ JPEG באותו שם ממוינים כתמונה אחת, והסימונים חלים על שניהם. ה־JPEG נפתח מהר יותר.
settings-cull-show-jpeg = JPEG
settings-cull-show-raw = RAW

# machine-drafted, needs native review
settings-cull-bursts = ערימת צילומי רצף
settings-cull-bursts-help = צילומים שצולמו ברצף מהיר במצלמה אחת מוצגים כערימה אחת. S פותח או סוגר ערימה, B שומר את התמונה המוצגת ודוחה את שאר הערימה.
settings-cull-burst-gap = מרווח רצף
settings-cull-burst-gap-help = ההפסקה הארוכה ביותר בין שני צילומים ברצף אחד.
settings-cull-watch = הצגת קבצים חדשים
settings-cull-watch-help = קבצים שמופיעים בתיקייה בזמן המיון (מצלמה מחוברת, ייבוא) נוספים בסוף לאחר שנכתבו במלואם.
settings-cull-follow-newest = מעקב אחר הקובץ החדש ביותר
settings-cull-follow-newest-help = מעבר לכל קובץ חדש כשהוא מגיע. T מחליף זאת בזמן המיון.
settings-cull-slot-1 = תיקייה 1
settings-cull-slot-1-help = Alt+1 מעתיק את התמונה (עם קובץ ה־RAW או ה־JPEG וקובצי הלוואי) לתיקייה זו, Alt+Shift+1 מעביר אותה לשם.
settings-cull-slot-1-unset = לא הוגדרה
settings-cull-slot-2 = תיקייה 2
settings-cull-slot-2-help = Alt+2 מעתיק את התמונה (עם קובץ ה־RAW או ה־JPEG וקובצי הלוואי) לתיקייה זו, Alt+Shift+2 מעביר אותה לשם.
settings-cull-slot-2-unset = לא הוגדרה
settings-cull-slot-3 = תיקייה 3
settings-cull-slot-3-help = Alt+3 מעתיק את התמונה (עם קובץ ה־RAW או ה־JPEG וקובצי הלוואי) לתיקייה זו, Alt+Shift+3 מעביר אותה לשם.
settings-cull-slot-3-unset = לא הוגדרה
settings-cull-collisions = כששם כבר תפוס
settings-cull-collisions-help = קבצים בתיקיות לעולם אינם מוחלפים.
settings-cull-collisions-suffix = הוספת מספר
settings-cull-collisions-skip = דילוג על התמונה
# machine-drafted, needs native review
# מצב מיון נבחר כשאין תיקייה או תמונות גלריה פתוחות.
cull-nothing = אין מה למיין — יש לפתוח קודם תיקיית תמונות
cull-slot-unset = לא הוגדרה תיקייה { $slot } — בחרו אחת בהגדרות ← מצב מיון
cull-slot-missing = תיקייה { $slot } אינה זמינה: { $reason }
cull-copied = הועתק אל { $folder }
cull-moved = הועבר אל { $folder }
cull-slot-renamed = נשמר בשם { $name } ב־{ $folder }
cull-slot-skipped = ב־{ $folder } כבר יש { $name } — דולג
cull-slot-same = התמונה כבר נמצאת ב־{ $folder }
cull-slot-failed = { $op ->
    [move] לא ניתן להעביר את { $file }
   *[copy] לא ניתן להעתיק את { $file }
}: { $reason }
cull-copy-kept = { $count ->
    [one] נשמר עותק אחד ששונה מאז
    [two] נשמרו { $count } עותקים ששונו מאז
   *[other] נשמרו { $count } עותקים ששונו מאז
}
cull-undo-failed = לא ניתן לבטל עבור { $file }: { $reason }
cull-burst-open = רצף { $index } מתוך { $count }
cull-burst-closed = רצף של { $count }
cull-burst-toggle = פתיחה או סגירה של הרצף
cull-burst-best = נשמרה 1, נדחו { $count }
cull-burst-none = אינה חלק מרצף
cull-follow-on = עוקב אחר קבצים חדשים
cull-follow-off = לא עוקב אחר קבצים חדשים
cull-new-files = { $count ->
    [one] קובץ חדש אחד
    [two] { $count } קבצים חדשים
   *[other] { $count } קבצים חדשים
}
cull-key-copy = העתקה לתיקייה 1–3
cull-key-move = העברה לתיקייה 1–3
cull-key-burst = פתיחה או סגירה של רצף
cull-key-best = שמירת זו ודחיית שאר הרצף
cull-key-follow = מעקב אחר קבצים חדשים
cull-key-subframes = הצגה או הסתרה של טבלת תת־החשיפות
cull-key-sort = מיון לפי העמודה הבאה
cull-key-sort-reverse = היפוך הסדר
cull-key-metric = המדד הבא בתרשים
cull-key-sigma-down = דחייה אוטומטית: הקטנת k
cull-key-sigma-up = דחייה אוטומטית: הגדלת k
cull-key-auto-reject = דחיית הפריימים המוצעים
cull-key-move-rejects = העברת הפריימים שנדחו לתת־תיקייה
cull-key-same-stretch = אותה מתיחה לכל הפריימים

## Strip tooltips. machine-drafted, needs native review
cull-tip-burst-open = פתיחת הצילום הרציף
cull-tip-burst-close = סגירת הצילום הרציף
cull-tip-exit = יציאה ממצב מיון
cull-tip-label = תווית: { $label }

## Astronomy subframes. machine-drafted, needs native review
cull-astro-panel = תת־חשיפות
cull-astro-title = { $count ->
    [one] תת־חשיפה אחת
    [two] { $count } תת־חשיפות
   *[other] { $count } תת־חשיפות
}
cull-astro-pending = מודד { $count }
cull-astro-col-name = פריים
cull-astro-col-filter = מסנן
cull-astro-col-exposure = חשיפה
cull-astro-col-hfd = HFD
cull-astro-col-fwhm = FWHM
cull-astro-col-ecc = אקסצנטריות
cull-astro-col-stars = כוכבים
cull-astro-col-bg = רקע
cull-astro-col-snr = SNR
cull-astro-col-noise = רעש
cull-astro-same-stretch = אותה מתיחה
cull-astro-own-stretch = מתיחה נפרדת
cull-astro-suggest-high = { $metric } מעל { $value } (חציון + { $k }σ): { $count ->
    [one] פריים אחד
    [two] { $count } פריימים
   *[other] { $count } פריימים
}
cull-astro-suggest-low = { $metric } מתחת ל־{ $value } (חציון − { $k }σ): { $count ->
    [one] פריים אחד
    [two] { $count } פריימים
   *[other] { $count } פריימים
}
cull-astro-suggest-none = אין פריים מעבר ל־{ $k }σ
cull-astro-suggest-few = דחייה אוטומטית דורשת 3 פריימים שנמדדו
cull-astro-suggested = מוצע לדחייה
cull-astro-k = k { $k }
cull-astro-reject = דחיית { $count }
cull-astro-move = העברת הנדחים…
cull-astro-chart = { $metric } { $count ->
        [one] בפריים אחד
        [two] על פני שני פריימים
       *[other] על פני { $count } פריימים
    }, חציון { $median }.
cull-astro-sort-by = מיון לפי { $column }
cull-astro-ascending = בסדר עולה
cull-astro-descending = בסדר יורד
cull-astro-sorted = ממוין לפי { $column }
cull-astro-reversed = הסדר התהפך
cull-astro-metric = תרשים: { $metric }
cull-astro-k-set = דחייה אוטומטית ב־{ $k }σ
cull-astro-none = אין כאן תת־חשיפות
cull-astro-same-on = אותה מתיחה לכל הפריימים
cull-astro-same-off = כל פריים נמתח בנפרד
cull-astro-line-hfd = HFD { $value } px
cull-astro-line-fwhm = FWHM { $value } px
cull-astro-line-fwhm-arcsec = FWHM { $value } px ({ $arcsec }″)
cull-astro-line-ecc = אקסצנטריות { $value }
cull-astro-line-stars = { $count ->
    [one] כוכב אחד
    [two] { $count } כוכבים
   *[other] { $count } כוכבים
}
cull-astro-measuring = מודד כוכבים…
cull-astro-reject-confirm = { $count ->
    [one] לדחות את הפריים המוצע?
    [two] לדחות את { $count } הפריימים המוצעים?
   *[other] לדחות את { $count } הפריימים המוצעים?
}
cull-astro-reject-detail = { $rule }. ביטול מחזיר את הסימונים.
cull-astro-reject-ok = דחייה
cull-astro-nothing-to-reject = אין פריים לדחייה
cull-astro-rejected = { $count ->
    [one] נדחה פריים אחד
    [two] נדחו { $count } פריימים
   *[other] נדחו { $count } פריימים
}
cull-move-rejects-confirm = { $count ->
    [one] להעביר את הפריים שנדחה לתיקייה „{ $folder }”?
    [two] להעביר את { $count } הפריימים שנדחו לתיקייה „{ $folder }”?
   *[other] להעביר את { $count } הפריימים שנדחו לתיקייה „{ $folder }”?
}
cull-move-rejects-detail = הקבצים מועברים, לא נמחקים. ביטול מחזיר אותם.
cull-move-rejects-ok = העברה
cull-move-rejects-none = אין פריימים שנדחו להעברה
cull-moved-rejects = { $count ->
    [one] פריים אחד הועבר אל „{ $folder }”
    [two] { $count } פריימים הועברו אל „{ $folder }”
   *[other] { $count } פריימים הועברו אל „{ $folder }”
}
