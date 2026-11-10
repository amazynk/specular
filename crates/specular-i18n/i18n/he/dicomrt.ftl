# דוחות מובנים של DICOM, צורות גל, מנת קרינה בטיפול קרינתי וטומוגרפיה של העין. machine-drafted, needs native review

## עץ התוכן של הדוח המובנה (דפדפן התגיות במצב דוח)
sr-tree-title = { $title } ({ $kind })
sr-col-item = פריט
sr-col-concept = מושג
sr-col-type = סוג
sr-col-relationship = קשר
sr-col-value = ערך
sr-show-image = הצגת התמונה
sr-jump-missing = התמונה שאליה יש הפניה אינה בתיקייה זו
sr-jump-frame = אין תמונה של מערכת ייחוס זו שמכילה את הנקודה הזו

## גרפים (היסטוגרמות מנה-נפח, גיליונות א.ק.ג)
plot-caliper-time = { $ms } ms · { $rate } /דקה
plot-caliper-time-zero = 0 ms
plot-caliper-hint = גרירה על פני גרף למדידה
plot-adjust-hint = + ו־− משנים את ההגבר, [ ו־] את המהירות

## גיליון א.ק.ג
ecg-paper = { $speed } mm/s · { $gain } mm/mV · { $rate } Hz
ecg-groups-more = { $count ->
    [one] קבוצת ערוצים נוספת אחת אינה מוצגת
    [two] שתי קבוצות ערוצים נוספות אינן מוצגות
   *[other] { $count } קבוצות ערוצים נוספות אינן מוצגות
}

## מנת קרינה מעל ה־MPR (קווי איזודוזה, מפת צבעים, מקרא, קריאה)
dose-value = מנה { $value } { $unit } ({ PERCENT($percent) })
dose-hidden = המנה מוסתרת (Shift+U מציג אותה)
dose-on-rx = מנה: { $dose } { $unit } במרשם · מרבית { $max } { $unit }
dose-on-max = מנה: אחוז מהמרבית, { $max } { $unit }
dose-colour = מנה ב־{ $colormap } ב־{ PERCENT($blend) }
dose-dvh-hint = { $count ->
    [one] Shift+K: היסטוגרמת מנה-נפח של מבנה אחד
    [two] Shift+K: היסטוגרמות מנה-נפח של שני מבנים
   *[other] Shift+K: היסטוגרמות מנה-נפח של { $count } מבנים
}
dose-level = { PERCENT($percent) } · { $dose } { $unit }
dose-mode-lines = מנה כקווי איזודוזה
dose-mode-colour = מנה כמפת צבעים

## היסטוגרמות מנה-נפח (גיליון הגרף)
dvh-title = היסטוגרמות מנה-נפח (DVH)
dvh-col-roi = מבנה
dvh-col-volume = נפח
dvh-axis-dose = מנה ({ $unit })
dvh-axis-volume = נפח (%)
dvh-prescription = מנה במרשם { $dose } Gy · מצטבר
dvh-max = מנה מרבית { $dose } { $unit } · מצטבר
dvh-none = אין ערכת מבנים של מערכת ייחוס זו שעליה ניתן לחשב את היסטוגרמת המנה

## טומוגרפיה של העין (מחוון סריקות B; תמונת המיקום בפינה)
oct-bscan = סריקת B
oct-depth = בעומק { $depth } µm
