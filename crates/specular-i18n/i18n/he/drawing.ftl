# Specular — Drawings (DXF): layers, paper, lineweights, model space and layouts. עברית.

keys-mode-drawing = שרטוטים
cheatsheet-drawing = שרטוטים
palette-drawing-layers = הצגה או הסתרה של חלונית השכבות
palette-drawing-all-layers = הצגת כל השכבות, או השכבות כפי שנשמרו
palette-drawing-paper = החלפה בין נייר כהה לבהיר
palette-drawing-lineweights = הצגה או הסתרה של עובי הקווים
disabled-not-drawing = רק לשרטוטים (DXF)
# A drawing's page: model space (layouts are shown by their own name).
page-label-model = מודל
# machine-drafted, needs native review
cad-pen-name = עט { $n }
cad-page-name = עמוד { $n }
cad-layer-name = שכבה { $n }
cad-map-name = מפה
# machine-drafted, needs native review
# A board's views (its Gerber files together): every layer stacked, and its top and bottom
# in board colours.
cad-board-layers-name = שכבות
cad-board-top-name = צד עליון
cad-board-bottom-name = צד תחתון
# machine-drafted, needs native review
# A plot's layers: its frame, a trace without a name (numbered from 1), and the contour
# lines of a 2-D spectrum above and below zero.
cad-chart-frame-name = מסגרת
cad-trace-name = עקומה { $n }
cad-contours-positive-name = קווי מתאר חיוביים
cad-contours-negative-name = קווי מתאר שליליים
# machine-drafted, needs native review
# The page of a survey file (LandXML) showing its surfaces and alignments in 3D.
cad-model-3d-name = מודל תלת־ממדי
# The layers panel's title (and its name for screen readers).
drawing-layers-title = שכבות · { $shown } מתוך { $total } מוצגות
# Toasts.
drawing-layers-all-toast = כל השכבות מוצגות
drawing-layers-saved-toast = השכבות כפי שנשמרו בשרטוט
drawing-paper-dark-toast = נייר כהה
drawing-paper-light-toast = נייר בהיר
drawing-lineweights-on = עובי הקווים מוצג
drawing-lineweights-off = עובי הקווים כבוי: קווים דקים

# machine-drafted, needs native review
# Markup on drawings and images (Alt+M): kept in a file next to the drawing, never in it.
palette-markup = סימון
palette-markup-help = עננים, טקסט, חצים, חותמות, מדידות ונעצי תמונה, נשמרים ליד הקובץ.
palette-compare-sheets = השוואת גיליונות
palette-compare-sheets-help = הצגת גרסה אחרת של הגיליון הזה מעליו, מיושרת, עם השינויים מסומנים.
markup-title = סימונים
markup-list = סימונים בעמוד הזה
markup-count = { $count ->
    [0] אין סימונים בעמוד הזה
    [one] פריט אחד בעמוד הזה
    [two] שני פריטים בעמוד הזה
   *[other] { $count } פריטים בעמוד הזה
}
markup-empty = יש לבחור כלי וללחוץ על הגיליון.
markup-item = { $n }. { $kind }
markup-tool-select = בחירה
markup-tool-cloud = ענן
markup-tool-text = טקסט
markup-tool-arrow = חץ
markup-tool-stamp = חותמת
markup-tool-length = אורך
markup-tool-area = שטח
markup-tool-pin = נעץ תמונה
markup-hint-select = לחיצה על פריט בוחרת בו. גרירה מזיזה אותו. Enter עורך את הטקסט שלו.
markup-hint-cloud = יש לגרור מלבן, או ללחוץ על הפינות ולהקיש Enter.
markup-hint-text = יש ללחוץ במקום שבו יופיע הטקסט.
markup-hint-arrow = יש לגרור מהתווית אל הנקודה, או ללחוץ על שתיהן.
markup-hint-stamp = יש ללחוץ במקום שבו תופיע החותמת.
markup-hint-length = יש לגרור, או ללחוץ על שני הקצוות.
markup-hint-area = יש ללחוץ על הפינות ואז להקיש Enter.
markup-hint-pin = יש ללחוץ על נקודה ואז לבחור תמונה.
markup-kind-cloud = ענן
markup-kind-text = טקסט
markup-kind-arrow = חץ
markup-kind-stamp = חותמת
markup-kind-length = אורך
markup-kind-area = שטח
markup-kind-pin = תמונה
markup-colour = צבע: { $colour }
markup-colour-red = אדום
markup-colour-blue = כחול
markup-colour-green = ירוק
markup-colour-orange = כתום
markup-colour-magenta = מג׳נטה
markup-colour-black = שחור
markup-width = קו: { $width }
markup-width-thin = דק
markup-width-medium = בינוני
markup-width-thick = עבה
markup-stamp = חותמת: { $words }
markup-stamp-approved = מאושר
markup-stamp-rejected = נדחה
markup-stamp-reviewed = נבדק
markup-stamp-custom = טקסט משלך
markup-undo = ביטול פעולה
markup-redo = ביצוע מחדש
markup-delete = מחיקה
markup-open-photo = פתיחת התמונה
markup-from-measurements = הוספת מדידות
markup-from-measurements-done = { $count ->
    [one] מדידה אחת נוספה כסימון
    [two] שתי מדידות נוספו כסימונים
   *[other] { $count } מדידות נוספו כסימונים
}
markup-export-pdf = ייצוא PDF…
markup-export-png = ייצוא PNG…
markup-done = סיום
markup-ask-text = טקסט (Enter למיקום, Esc לביטול)
markup-ask-label = תווית (רשות; Enter למיקום)
markup-ask-stamp = נוסח החותמת (Enter למיקום)
markup-ask-edit = טקסט חדש (Enter לשמירה)
markup-added = נוסף: { $kind }
markup-moved = הוזז
markup-pin-hint = לחיצה נוספת על הנעץ או הקשה על Enter פותחת את התמונה שלו.
markup-deleted = נמחק
markup-undone = בוטל
markup-redone = בוצע מחדש
markup-delete-ask = למחוק את { $name }?
markup-delete-detail = הפריט יוסר מקובץ הסימונים. השרטוט עצמו לא משתנה.
markup-delete-ok = מחיקה
markup-photo-missing = { $name } כבר לא נמצא שם.
markup-load-failed = הסימונים לא נקראו: { $reason }
markup-save-failed = הסימונים לא נשמרו: { $reason }
markup-sidecar-too-large = קובץ הסימונים גדול מדי
markup-pdf-unreadable = לא ניתן היה לקרוא את ה־PDF
markup-exported = { $name } נשמר
markup-export-failed = לא יוצא: { $reason }
markup-export-same-file = יש לבחור שם אחר; המקור אף פעם לא נדרס
markup-export-encrypted = ה־PDF מוצפן. כדאי לייצא PNG במקום.
markup-export-damaged = המבנה של ה־PDF פגום. כדאי לייצא PNG במקום.
sheets-title = השוואת גיליונות
sheets-other = גיליון אחר…
sheets-other-named = אחר: { $name }
sheets-page = עמוד { $page } מתוך { $count }
sheets-auto = יישור
sheets-pick = בחירת 2 נקודות
sheets-overlay = שכבת־על
sheets-difference = הפרש
sheets-blend = עמעום
sheets-blink = הבהוב
sheets-slider = גיליון אחר { PERCENT($percent) }
sheets-clouds = { $count ->
    [0] אין שינויים
    [one] סימון שינוי אחד בענן
    [two] סימון שני שינויים בעננים
   *[other] סימון { $count } שינויים בעננים
}
sheets-done = סיום
sheets-choose = יש לבחור את הגיליון האחר.
sheets-reading = הגיליון נקרא…
sheets-aligning = מתבצע יישור…
sheets-aligned = יושר: קנה מידה { $scale }, סיבוב { $angle }°, סטייה עד { $error } px. { $changes ->
    [0] לא נמצאו שינויים.
    [one] שינוי אחד.
    [two] שני שינויים.
   *[other] { $changes } שינויים.
}
sheets-align-failed = לא ניתן ליישר אוטומטית. יש לבחור 2 נקודות.
sheets-pick-this = נקודה { $n }: יש ללחוץ עליה בגיליון הזה.
sheets-pick-other = נקודה { $n }: יש ללחוץ על אותה נקודה בגיליון האחר.
sheets-pick-same = שתי הנקודות זהות. יש לבחור שוב 2 נקודות.
sheets-read-failed = הגיליון לא נקרא: { $reason }
sheets-failed = השלב הזה נכשל. אפשר לנסות שוב או לבחור גיליון אחר.
sheets-legend-this = רק הגיליון הזה
sheets-legend-other = רק הגיליון האחר
sheets-legend-both = שניהם
sheets-cloud-text = שונה
sheets-clouds-added = { $count ->
    [0] לא נוספו עננים
    [one] ענן אחד נוסף לסימונים
    [two] שני עננים נוספו לסימונים
   *[other] { $count } עננים נוספו לסימונים
}
sheets-opened = השוואת גיליונות
sheets-closed = ההשוואה נסגרה
markup-ask-new-line = Shift+Enter: שורה חדשה

## Settings › Drawings

# machine-drafted, needs native review
settings-section-drawings = שרטוטים
settings-drawings-shx-folders = תיקיות גופני SHX
settings-drawings-shx-folders-help = תיקיות עם גופני SHX לטקסט ב־DXF וב־DWG, מופרדות בתו ;. התיקייה של השרטוט עצמו נסרקת ראשונה.
# A folder in the SHX font folders setting that does not exist or is not a folder.
settings-error-shx-folder = זו לא תיקייה: { $folder }
