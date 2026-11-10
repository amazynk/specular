# machine-drafted, needs native review
# Specular — מסגרת המציג, לוח הפקודות, הודעות שגיאה ו"אודות". עברית.

-app-name = Specular

## Toolbar tooltips and accessibility labels

toolbar-open = פתיחת תמונה…
toolbar-prev = התמונה הקודמת
toolbar-next = התמונה הבאה
toolbar-back-gallery = חזרה לגלריה
toolbar-zoom-out = התרחקות
toolbar-zoom-in = התקרבות
toolbar-zoom-level = רמת זום והגדרות קבועות
toolbar-zoom-fit = התאמה לחלון
toolbar-zoom-100 = גודל אמיתי ({ PERCENT(100) })
toolbar-rotate-left = סיבוב נגד כיוון השעון
toolbar-rotate-right = סיבוב בכיוון השעון
toolbar-fullscreen-enter = מעבר למסך מלא
toolbar-fullscreen-exit = יציאה ממסך מלא
toolbar-info = מידע על התמונה
toolbar-settings = הגדרות
toolbar-delete = העברה לאשפה
toolbar-copy = העתקת התמונה
toolbar-palette = לוח פקודות
toolbar-go-to = מעבר לתמונה…
toolbar-go-to-range = 1–{ $count }
# machine-drafted, needs native review
toolbar-customise = התאמה אישית של סרגל הכלים…
toolbar-more = עוד
toolbar-announce-moved = { $item }: מיקום { $index } מתוך { $count }
toolbar-announce-added = { $item } נוסף: מיקום { $index } מתוך { $count }
toolbar-announce-removed = { $item } הוסר מסרגל הכלים
toolbar-announce-reset = סרגל הכלים אופס לברירת המחדל
tooltip-with-shortcut = { $label } ({ $shortcut })

## Readouts

readout-position = { $index } / { $count }
readout-position-a11y = תמונה { $index } מתוך { $count }
readout-zoom = { $zoom }
readout-zoom-a11y = זום { $zoom }
readout-zoom-fit = התאמה ({ $zoom })
zoom-preset-fit = התאמה לחלון
zoom-preset-fill = מילוי החלון
zoom-preset-actual = גודל אמיתי
# machine-drafted, needs native review
# The last entry of the zoom presets: the window fills the screen (ticked while it does).
zoom-preset-fullscreen = מסך מלא
zoom-preset-percent = { $zoom }
readout-dimensions = { $width } × { $height }
status-image-count =
    { $count ->
        [one] תמונה אחת
        [two] שתי תמונות
       *[other] { $count } תמונות
    }

## Command palette

palette-placeholder = הקלדת פקודה או הגדרה…
palette-no-results = אין פקודות תואמות
palette-set-setting = הגדרה: { $setting } ← { $value }
palette-next = התמונה הבאה
palette-prev = התמונה הקודמת
palette-first = התמונה הראשונה
palette-last = התמונה האחרונה
palette-go-to = מעבר לתמונה…
palette-zoom-in = התקרבות
palette-zoom-out = התרחקות
palette-zoom-fit = זום להתאמה
palette-zoom-fill = זום למילוי
# machine-drafted, needs native review
palette-zoom-fit-width = התאמה לרוחב
palette-zoom-fit-height = התאמה לגובה
palette-zoom-100 = זום ל־{ PERCENT(100) }
palette-zoom-200 = זום ל־{ PERCENT(200) }
palette-zoom-300 = זום ל־{ PERCENT(300) }
palette-zoom-400 = זום ל־{ PERCENT(400) }
palette-zoom-rectangle = זום לבחירה
palette-rotate-left = סיבוב נגד כיוון השעון
palette-rotate-right = סיבוב בכיוון השעון
palette-flip-horizontal = היפוך אופקי
palette-flip-vertical = היפוך אנכי
palette-toggle-fullscreen = מעבר למסך מלא או יציאה ממנו
palette-navigator = הצגה או הסתרה של הנווט
palette-loupe = זכוכית מגדלת
palette-pixel-grid = הצגה או הסתרה של רשת הפיקסלים
palette-info-panel = הצגה או הסתרה של חלונית המידע
palette-copy-image = העתקת התמונה
# machine-drafted, needs native review
palette-read-codes = קריאת קודים
# machine-drafted, needs native review
palette-find-text = חיפוש טקסט
# machine-drafted, needs native review
palette-face-boxes = הצגת פנים
palette-copy-path = העתקת נתיב הקובץ
palette-move-to-trash = העברה לאשפה
palette-undo-trash = ביטול ההעברה לאשפה
palette-open = פתיחת תמונה…
# machine-drafted, needs native review
palette-open-address = פתיחת כתובת…
palette-settings = הגדרות
# machine-drafted, needs native review
palette-customise-toolbar = התאמה אישית של סרגל הכלים…
palette-cheat-sheet = קיצורי מקלדת
palette-hud = הצגה או הסתרה של HUD זמני הפריימים
palette-language = שפה
palette-theme = ערכת נושא
# machine-drafted, needs native review
palette-interface-larger = ממשק גדול יותר
palette-interface-smaller = ממשק קטן יותר
palette-interface-reset = ממשק ב־{ PERCENT(100) }
palette-quit = יציאה מ־{ -app-name }

## Cheat sheet

cheatsheet-title = קיצורי מקלדת
cheatsheet-navigation = ניווט
cheatsheet-zoom-pan = זום והזזה
cheatsheet-view = תצוגה
cheatsheet-file = קובץ
cheatsheet-window = חלון
cheatsheet-help = עזרה
cheatsheet-hold = להחזיק את { $key }
cheatsheet-close-hint = יש להקיש { $key } או Esc כדי לסגור

## Errors

# machine-drafted, needs native review
viewer-no-preview = אין תצוגה מקדימה בקובץ זה.
error-cannot-open = לא ניתן לפתוח את { $file }
error-cannot-open-reason = לא ניתן לפתוח את { $file }: { $reason }
error-unsupported-format = הקובץ { $file } בפורמט ש־{ -app-name } עדיין לא יכול לקרוא.
error-file-truncated = הקובץ { $file } אינו שלם. מוצג החלק שניתן היה לקרוא.
error-file-too-large = הקובץ { $file } גדול מדי לפתיחה ({ $megapixels } מגה־פיקסלים; המגבלה היא { $limit } מגה־פיקסלים).
error-not-found = הקובץ { $file } כבר לא קיים.
error-permission-denied = אין לך הרשאה לקרוא את { $file }.
error-details = פרטים
crash-last-time = { -app-name } נסגר באופן בלתי צפוי בפעם הקודמת; דוח נשמר ב־{ $path }
error-trash-failed = לא ניתן היה להעביר את { $file } לאשפה: { $reason }
# machine-drafted, needs native review
# סיבות, שמוצגות אחרי "לא ניתן לפתוח את FILE:" ואחרי הודעות "לא ניתן …:" אחרות. הן אומרות
# מה קרה וכשזה עוזר, מה לעשות.
error-reason-not-found = הוא כבר לא שם. ייתכן שהועבר, ששמו שונה או שנמחק.
error-reason-permission = אין לך הרשאה לקרוא אותו. כדאי לבדוק את הבעלים וההרשאות שלו.
error-reason-disk-full = הדיסק מלא. יש לפנות מקום ולנסות שוב.
error-reason-read-only = הדיסק מוגן מפני כתיבה.
error-reason-truncated = הקובץ חלקי. ייתכן שהוא עדיין מועתק או מורד.
error-reason-is-folder = זו תיקייה, לא קובץ.
# A remote store (a Zarr image on a server or in an S3 bucket).
error-reason-remote-denied = השרת סירב לגישה. ייתכן שהנתונים פרטיים; עבור דלי S3 יש להגדיר תחילה פרטי גישה של AWS.
error-reason-remote-missing = אין נתוני Zarr בכתובת הזו.
# $status is an HTTP status code ("500").
error-reason-remote-status = השרת ענה בשגיאה { $status }.
error-reason-offline = לא ניתן להגיע לשרת. יש לבדוק את הכתובת ואת החיבור.
# machine-drafted, needs native review
error-reason-network-off = הרשת כבויה בהגדרות › פרטיות.
error-reason-timed-out = הכונן או השרת לא ענו בזמן. יש לבדוק את החיבור ולנסות שוב.
error-reason-memory = אין מספיק זיכרון. יש לסגור תמונות או יישומים אחרים ולנסות שוב.
error-reason-unsupported = הוא בפורמט ש־{ -app-name } עדיין לא יכול לקרוא.
# $detail הוא תיאור המפענח עצמו, טכני ובאנגלית ("12-bit lossless JPEG").
error-reason-unsupported-detail = הוא משתמש ביכולת ש־{ -app-name } עדיין לא יכול לקרוא ({ $detail }).
# machine-drafted, needs native review
# A file whose extension several formats share (".img") and whose content is none of them.
# $formats lists the format names, e.g. "ERDAS IMAGINE, ADSC SMV, Analyze 7.5".
error-reason-not-one-of = הוא אינו אף אחד מקובצי ה־.{ $ext } ש־{ -app-name } קורא ({ $formats }).
error-reason-damaged = הקובץ פגום או שאינו מה ששמו אומר ({ $detail }).
error-reason-too-large = הוא גדול מכדי להיפתח.
# machine-drafted, needs native review
error-reason-needs-memory = הוא צריך { $needed } של זיכרון RAM פנוי, אבל כרגע פנויים רק { $free }. יש לסגור כמה יישומים אחרים ולנסות שוב, או לפתוח אותו במחשב עם יותר זיכרון.
error-needs-memory = התמונה הזו צריכה { $needed } של זיכרון RAM פנוי, אבל כרגע פנויים רק { $free }. יש לסגור כמה יישומים אחרים ולנסות שוב, או לפתוח אותה במחשב עם יותר זיכרון.
error-needs-memory-short = דרושים { $needed } של RAM פנוי
error-reason-cancelled = הפתיחה שלו הופסקה.
error-reason-trash-access = הוא כבר לא שם, או שאין לך הרשאה לשנות אותו.
error-reason-restore-collision = כבר יש קובץ באותו שם בתיקייה שלו.
error-reason-trash-root = אי אפשר להעביר כונן שלם לאשפה.
error-reason-not-in-trash = הוא כבר לא באשפה.
error-reason-program-missing = התוכנה אינה מותקנת, או שהועברה. יש לבחור אותה שוב או להתקין אותה מחדש.
error-reason-program-not-allowed = לתוכנה אין הרשאה לרוץ. יש לוודא שהיא ניתנת להפעלה.
# machine-drafted, needs native review
error-reason-permission-write = אין לך הרשאה לשנות אותו. כדאי לבדוק את הבעלים וההרשאות שלו.
error-reason-file-has-errors = יש בו שגיאות. יש לתקן אותן קודם, כדי ששום דבר בו לא יאבד.
error-reason-not-dicom = הוא אינו קובץ DICOM.
error-reason-no-image = אין בו תמונה.
error-reason-needs-layout = זהו נפח ללא כותרת. כדי לפתוח אותו יש לדעת את גודלו, את סוג הדגימות ואת סדר הבתים.
error-reason-not-archive = הוא אינו ארכיון ש־{ -app-name } יכול לקרוא.
error-reason-needs-sandbox = סוג ארכיון זה נפתח רק בקורא המוגן של { -app-name }, שאינו זמין.
error-reason-encrypted = הוא מוצפן ונדרשת הסיסמה שלו.
error-reason-wrong-password = הסיסמה שגויה.
error-reason-checksum = הקובץ פגום: התוכן שלו אינו תואם לסכום הביקורת.
error-reason-missing-entry = קובץ שבתוכו חסר.
error-reason-busy = תוכנה אחרת משתמשת בו. יש לסגור אותו שם ולנסות שוב.
error-reason-catalogue-damaged = קובץ הקטלוג פגום.
error-reason-cannot-open = לא ניתן לפתוח את הקובץ שלו. יש לוודא שהתיקייה שלו קיימת ושיש לך הרשאת כתיבה אליה.
error-reason-newer-version = הוא נכתב בגרסה חדשה יותר של { -app-name }. יש לעדכן את { -app-name } כדי לשנות אותו.
error-reason-internal = משהו השתבש בתוך { -app-name }. הפרטים ביומן.
error-reason-volume-too-few = { $count ->
    [one] פרוסה אחת אינה מספיקה לבניית נפח.
    [two] { $count } פרוסות אינן מספיקות לבניית נפח.
   *[other] { $count } פרוסות אינן מספיקות לבניית נפח.
}
error-reason-volume-no-geometry = פרוסה אינה מציינת היכן היא ממוקמת או לאיזה כיוון היא פונה.
error-reason-volume-not-parallel = הפרוסות אינן מקבילות.
error-reason-volume-shared-position = כמה תמונות חולקות { $count ->
        [one] מיקום אחד
        [two] כל אחד משני מיקומים
       *[other] כל אחד מ־{ $count } מיקומים
    } (פאזות, הדים או סדרת זמן).
error-reason-volume-missing = { $count ->
        [one] חסרה כפרוסה אחת.
        [two] חסרות כשתי פרוסות.
       *[other] חסרות כ־{ $count } פרוסות.
    }
error-reason-volume-mismatch = הפרוסות שונות בגודל או במרווח הפיקסלים.
error-reason-volume-not-grey = הפרוסות צבעוניות, לא בגווני אפור.
error-reason-volume-skewed = צירי הנפח אינם ניצבים זה לזה.
error-reason-volume-too-large = הוא דורש { $need } MiB ומותרים רק { $budget } MiB.
error-reason-url-not-specular = הוא אינו קישור של { -app-name }.
error-reason-url-too-long = הוא ארוך מדי.
error-reason-url-damaged = הוא פגום. ייתכן שנקטע או שונה בעת ההעתקה.
error-reason-url-unknown = הוא מבקש משהו שגרסה זו של { -app-name } אינה מכירה. עדכון { -app-name } עשוי לעזור.
error-reason-url-not-absolute = נתיבי הקבצים שלו חייבים להיות נתיבים מלאים.
error-reason-url-no-paths = הוא אינו מציין אף קובץ.
error-reason-url-too-many = הוא מציין יותר מדי קבצים.

## Toasts

# machine-drafted, needs native review
# A background task (a listing, an export, a file operation) stopped part way; $reason is
# error-reason-internal.
background-failed = לא ניתן היה להשלים: { $reason }
toast-moved-to-trash = הועבר לאשפה
toast-moved-to-trash-named = { $file } הועבר לאשפה
toast-moved-to-trash-count =
    { $count ->
        [one] תמונה אחת הועברה לאשפה
        [two] שתי תמונות הועברו לאשפה
       *[other] { $count } תמונות הועברו לאשפה
    }
toast-undo = ביטול פעולה
toast-restored = { $file } שוחזר
toast-copied-image = התמונה הועתקה
toast-copied-path = הנתיב הועתק
toast-copied-report = הדוח הועתק
toast-copied-value = הערך הועתק
toast-loop-start = חזרה לתמונה הראשונה
toast-loop-end = מעבר לתמונה האחרונה
# machine-drafted, needs native review
toast-interface-size = גודל הממשק { PERCENT($percent) }
toast-language-changed = השפה שונתה ל־{ $language }

## Empty states

empty-drop = אפשר לגרור תמונה לכאן או להקיש { $shortcut }
empty-open-button = פתיחת תמונה…
empty-drop-release = יש לשחרר כדי לפתוח
empty-folder = בתיקייה זו אין תמונות ש־{ -app-name } יכול לפתוח.
# machine-drafted, needs native review
# מתחת ל"פתיחת תמונה…" בחלון הריק: איפה למצוא את כל השאר.
empty-hint-keys = אפשר להקיש { $cheatsheet } לקיצורי המקלדת, או { $palette } כדי למצוא כל פקודה.
empty-hint-cheatsheet = אפשר להקיש { $cheatsheet } לקיצורי המקלדת.
empty-hint-palette = אפשר להקיש { $palette } כדי למצוא כל פקודה.
# מתחת לסמל הטעינה כשפתיחת קובץ לוקחת יותר מרגע.
viewer-opening = פותח את { $file }…
fullscreen-exit-hint = יש להקיש Esc כדי לצאת ממסך מלא

## Language picker

language-picker-title = שפה
# machine-drafted, needs native review
language-picker-search = חיפוש שפות
language-completeness = { $percent } תורגמו
language-system-default = ברירת המחדל של המערכת ({ $language })
language-pseudo-accented = שפת בדיקה: עם סימנים ומוארכת
language-pseudo-rtl = שפת בדיקה: משוקפת מימין לשמאל

## About box

about-title = אודות { -app-name }
about-version = גרסה { $version }
about-tagline = מציג תמונות מהיר ונאמן למקור.
about-copyright = © { $year } המחברים של { -app-name }
about-license = מופץ ברישיון MIT License / Apache License 2.0.
about-geonames = שמות מקומות מ־GeoNames (geonames.org), ברישיון Creative Commons Attribution 4.0.
about-unicode = נתוני אזור מ־Unicode CLDR באמצעות ICU4X. © Unicode, Inc.
# machine-drafted, needs native review
about-lensfun = פרופילי עדשות ממסד הנתונים של Lensfun (lensfun.github.io), ברישיון Creative Commons Attribution-ShareAlike 3.0.
about-third-party = רישיונות של צד שלישי
about-copy-info = העתקת מידע המערכת
about-close = סגירה

## ממשק המציג (שלב 1)
# machine-drafted, needs native review
app-close = סגירה
app-truncated-badge = קובץ חלקי
app-info-summary = סיכום
app-info-all-fields = כל השדות
app-info-copy-value = העתקת הערך
app-info-copy-field = העתקת הערך של { $field }
app-info-loading = קריאת מטא־נתונים…
# machine-drafted, needs native review
# כל השדות, עבור קובץ שאינו שומר מטא־נתונים מעבר לפיקסלים שלו.
app-info-no-fields = לקובץ זה אין שדות מטא־נתונים.
# Gain-map row of the info summary (machine-drafted, needs native review)
app-info-gain-map = מפת הגבר HDR
app-info-gain-map-headroom = עד { $headroom }× מבהירות SDR
app-copy-failed = לא ניתן היה להעתיק: { $reason }
app-restore-failed = לא ניתן היה לשחזר את { $file }: { $reason }
app-open-dialog-title = פתיחת תמונה
app-open-dialog-filter = קבצים נתמכים
app-open-dialog-all = כל הקבצים
app-open-group-images = תמונות
app-open-group-raw = קובצי RAW של מצלמה
app-open-group-video = וידאו
app-open-group-sound = שמע
app-open-group-documents = מסמכים
app-open-group-drawings = שרטוטים ו-CAD
app-open-group-models = מודלים בתלת־ממד
app-open-group-maps = מפות ומסלולים
app-open-group-science = מדע ורפואה
app-open-group-fonts = גופנים
app-open-group-other = אחר

## מידע על הרינדור (אודות, הגדרות › מתקדם)

# machine-drafted, needs native review
renderer-title = מידע על הרינדור
renderer-section-adapter = מתאם גרפי
renderer-section-limits = מגבלות ותכונות
renderer-section-display = תצוגה
renderer-section-memory = זיכרון
renderer-section-decoders = מפענחים אופציונליים
renderer-unknown = לא ידוע
# The PCI vendor and device IDs ("1002:747e").
renderer-ids = מזהה יצרן והתקן
# Wayland, X11, Windows or macOS.
renderer-display-server = שרת תצוגה
renderer-features = תכונות בשימוש
renderer-surface-format = תבנית החלון
renderer-surface-formats = תבניות זמינות
renderer-present-mode = מצב הצגה
renderer-present-modes = מצבים זמינים
renderer-hdr = HDR
# $headroom is how much brighter than white the display goes ("4.0").
renderer-hdr-on = פעיל (מרווח { $headroom }×)
renderer-hdr-off = כבוי (התצוגה יכולה להציג HDR)
renderer-hdr-unavailable = לא זמין בתצוגה הזו
renderer-decode-budget = הפענוח רשאי להשתמש ב־
renderer-decode-budget-detail = { PERCENT(80) } מהזיכרון הפנוי, משותף לכל התמונות שנפתחות.
renderer-display-cache = מטמון תצוגה

## HUD ביצועים (F12)
# machine-drafted, needs native review
app-hud-scale = קנה מידה
app-hud-level = רמה
app-hud-level-thumbnail = תמונה ממוזערת מוטמעת
app-hud-level-full = רזולוציה מלאה
app-hud-level-scaled = 1/{ $denominator }
app-hud-open = פתיחה
app-hud-decode = פענוח
app-hud-upload = העלאה
app-hud-frame = פריים
app-hud-threads = תהליכונים
app-hud-threads-value = { $threads } ({ $queued } בתור)
app-hud-tiles = אריחים
app-hud-tiles-value = { $tiles } מתוך { $capacity }
app-hud-ram = זיכרון
app-hud-vram = זיכרון GPU
app-hud-gpu = GPU

# Trash confirmation (machine-drafted, needs native review)
app-confirm-trash-title = להעביר את { $file } לאשפה?
# machine-drafted, needs native review
app-confirm-trash-sidecars = גם הקבצים הנלווים יועברו לאשפה: { $files }
app-cancel = ביטול

# Stage 1 merge (machine-drafted, needs native review)
disabled-no-image = אין תמונה פתוחה
disabled-still-loading = התמונה עדיין נטענת
disabled-decode-failed = לא ניתן לפענח את התמונה הזו
disabled-no-other-images = אין תמונות נוספות בתיקייה זו
tooltip-unavailable = { $label } (לא זמין: { $reason })
error-decoder-stopped = המפענח נעצר בלי להפיק תמונה
# machine-drafted, needs native review
# Shown after "Can’t open FILE:" for brain data (GIFTI .func/.shape/.label, CIFTI) that is
# drawn on a surface: no .surf.gii with that many vertices was found in the same folder.
error-needs-surface = נדרש משטח מוח (קובץ ‎.surf.gii עם { $vertices } קודקודים) באותה תיקייה
fullscreen-back-hint = יש ללחוץ Esc כדי לאפס את הזום או לצאת ממסך מלא
action-back = סגירת החלונית (בתיבת דו־שיח, קודם צעד אחד אחורה), איפוס התקריב, יציאה ממסך מלא; בגלריה, תיקייה אחת למעלה
about-third-party-file = ‏{ -app-name } כולל תוכנה ונתונים של צד שלישי בכפוף לרישיונות שלהם; ראו THIRD-PARTY.md, המותקן יחד עם { -app-name }.
toast-low-memory = ייתכן שתמונה זו תצטרך כ־{ $need } זיכרון, אך זמינים רק { $available }. ייתכן שהטעינה תהיה איטית או תיכשל.

hw-report-title = דוח חומרה
hw-report-copy = העתקת הדוח
hw-report-version = גרסה
hw-report-cpu = CPU
hw-report-cores = ליבות
hw-report-cores-value = { $logical } לוגיות, { $physical } פיזיות
hw-report-performance-cores = ליבות ביצועים
hw-report-cpu-features = האצות CPU
hw-report-none = אין
hw-report-memory = זיכרון
hw-report-memory-value = { $total } בסך הכול, { $available } פנויים
hw-report-graphics-api = ממשק גרפי
hw-report-driver = מנהל התקן
hw-report-max-texture = מרקם מרבי
hw-report-pixels = { $width } × { $height } פיקסלים
hw-report-decode-threads = תהליכוני פענוח
hw-report-ram-cache = מטמון תמונות בזיכרון
hw-report-vram-cache = מטמון זיכרון GPU
hw-report-processing = עיבוד
hw-report-path-jpeg-decode = פענוח JPEG
hw-report-path-ycbcr-to-rgb = המרת צבעים
hw-report-path-png-unfilter = פענוח PNG
hw-report-path-heic-decode = פענוח HEIC
hw-report-path-inference = ניתוח תמונה
hw-report-engine-cpu = CPU
hw-report-engine-cpu-parallel = CPU, במקביל
hw-report-engine-gpu = GPU
hw-report-engine-hw-decoder = מפענח חומרה
hw-report-engine-npu = NPU
# machine-drafted, needs native review
hw-report-build-debug = גרסת ניפוי באגים
hw-report-build-release = גרסת הפצה

## Accessibility (screen readers)
# The image as a screen reader announces it: file name and size in pixels.
a11y-image = { $name }, { $width } × { $height } פיקסלים
# The region where short notifications (toasts) appear.
a11y-notifications = התראות
# machine-drafted, needs native review
a11y-transport-playback = ניגון
a11y-transport-pages = עמודים
a11y-settings-sections = מקטעים

## Read codes (QR codes in the picture) (machine-drafted, needs native review)
codes-placeholder = קודים בתמונה זו
codes-looking = מתבצע חיפוש קודים…
codes-none = לא נמצאו קודים בתמונה זו.
codes-failed = לא ניתן היה לקרוא את התמונה: { $reason }
codes-copy = העתקת „{ $text }”
# $kind is "QR", $version a number.
codes-kind = { $kind } { $version }
# Data Matrix: its size in modules, e.g. "Data Matrix 12×12".
# machine-drafted, needs native review
codes-kind-size = { $kind } { $rows }×{ $cols }
codes-open-row = פתיחת קישור ב־{ $host }…
codes-network-off = אין רשת פעילה, ולכן קישורים לא נפתחים.
codes-open-title = לפתוח קישור אל { $host }?
codes-open-detail = הקישור ייפתח בדפדפן האינטרנט. כדאי לפתוח רק קישורים מהימנים.
codes-open = פתיחה בדפדפן
# machine-drafted, needs native review
# Find text: the text in the picture shown, read by the Local AI pack's Text models.
ocr-placeholder = טקסט בתמונה זו
# machine-drafted, needs native review
ocr-looking = הטקסט נקרא…
# machine-drafted, needs native review
ocr-none = לא נמצא טקסט בתמונה זו.
# machine-drafted, needs native review
ocr-failed = לא ניתן היה לקרוא את הטקסט: { $reason }
# machine-drafted, needs native review
ocr-needs-pack = חיפוש טקסט דורש את מודלי הטקסט של חבילת הבינה המלאכותית המקומית (הגדרות › בינה מלאכותית מקומית).
# machine-drafted, needs native review
ocr-loading = מודלי הטקסט נטענים…
# machine-drafted, needs native review
# A row: choosing it copies the line. $text is the line (one line, shortened).
ocr-copy = העתקת „{ $text }”
# machine-drafted, needs native review
ocr-copy-all = העתקת כל הטקסט
# machine-drafted, needs native review
# A row's note: how sure the reading is, and the add-on that read it. $percent is formatted.
ocr-confidence = ודאות: { $percent }

## Hardware report panel (machine-drafted, needs native review)
hw-report-section-system = מערכת
hw-report-section-gpu = GPU
hw-report-section-decoding = פענוח
hw-report-operating-system = מערכת הפעלה
hw-report-adapter = מתאם
hw-report-gpu-type = סוג
hw-report-gpu-init = זמן הפעלה
hw-report-max-layers = שכבות מרקם
hw-report-compression = דחיסת מרקמים
hw-report-unified-memory = זיכרון משותף עם ה־CPU
hw-report-display = צג
hw-report-hw-decode = פענוח בחומרה
hw-report-core-cluster = קבוצת ליבות
hw-report-power = אספקת חשמל
hw-report-calibration = מדידות מהירות
hw-report-codec-others = אחרים
hw-report-performance-cores-value = { $count } מתוך { $total }
hw-report-cluster-value = { $name }: { $physical } פיזיות, { $logical } לוגיות
hw-report-thermal-nominal = טמפרטורה רגילה
hw-report-thermal-fair = חם
hw-report-thermal-serious = חם מאוד, מאט
hw-report-thermal-critical = התחממות יתר
hw-report-power-battery = בסוללה ({ $percent })
hw-report-power-battery-unknown = בסוללה
hw-report-power-mains = מחובר לחשמל
hw-report-power-mains-percent = מחובר לחשמל ({ $percent })
hw-report-power-saving = חיסכון בחשמל מופעל
hw-report-gpu-discrete = ייעודי
hw-report-gpu-integrated = משולב
hw-report-gpu-virtual = וירטואלי
hw-report-gpu-software = תוכנה, על ה־CPU
hw-report-gpu-other = אחר
hw-report-gpu-unavailable = לא זמין
hw-report-ms = { $ms } ms
hw-report-yes = כן
hw-report-no = לא
hw-report-unified-platform-differs = המערכת מדווחת אחרת
hw-report-display-value = { $fps } Hz, מרווח HDR כעת { $now }, עד { $max }
hw-report-hw-decode-value = { $api } על { $device }
hw-report-hw-decode-checking = עדיין בבדיקה…
hw-report-codec-up-to = עד { $width } × { $height }
hw-report-codec-none = לא זמין
hw-report-codec-h264-encode = קידוד H.264
hw-report-supported = נתמך: { $list }
hw-report-unsupported = לא נתמך: { $list }
hw-report-calibration-done = נמדד במחשב זה
hw-report-calibration-pending = טרם נמדד
hw-report-paths-power-saving = חיסכון בחשמל: מועדפות דרכים שצורכות פחות אנרגיה
hw-report-paths-caption = דרכים לתמונה של { $mpx } מגה־פיקסל
hw-report-col-task = משימה
hw-report-col-path = דרך
hw-report-col-time = זמן
hw-report-col-cpu = זמן CPU (כל התהליכונים)
hw-report-col-runs = מדידות
hw-report-col-use = שימוש
hw-report-runs-untested = לא נבדק
hw-report-use-viewing = צפייה ({ $reason })
hw-report-use-background = רקע ({ $reason })
hw-report-reason-only = האפשרות היחידה
hw-report-reason-faster = המהירה ביותר
hw-report-reason-cheaper = הכי מעט זמן CPU
hw-report-reason-lower-power = הכי מעט אנרגיה
hw-report-reason-explore = בניסיון
hw-report-reason-forced = נקבע בהגדרות
hw-report-reason-prefer-gpu = העדפת ה־GPU
hw-report-reason-cpu-only = CPU בלבד
hw-report-reason-calibrate = נמדד כעת
hw-report-reason-scaled = גודל מוקטן
hw-report-trust-unverified = טרם נבדק
hw-report-trust-blocked = כובה לאחר שגיאה
hw-report-trust-absent = לא קיים במחשב זה

## Keyboard cheat sheet tabs and sections (machine-drafted, needs native review)
cheatsheet-mode-gallery = גלריה
cheatsheet-mode-compare = השוואה
cheatsheet-gallery-browse = עיון
cheatsheet-gallery-select = בחירה
cheatsheet-gallery-mark = דירוג וסימון
cheatsheet-gallery-move = מעבר בין תמונות
cheatsheet-gallery-page = עמוד למעלה או למטה
cheatsheet-gallery-ends = התמונה הראשונה או האחרונה
cheatsheet-gallery-up = למעלה אל התיקייה המכילה
cheatsheet-gallery-history-back = חזרה לתיקייה הקודמת
cheatsheet-gallery-history-forward = קדימה לתיקייה הבאה
cheatsheet-gallery-back = סגירת התצוגה העליונה, ואז חזרה
cheatsheet-gallery-viewer = חזרה למציג
cheatsheet-gallery-album-add = הוספה לאלבום האחרון
cheatsheet-gallery-select-one = בחירה או ביטול בחירה
cheatsheet-gallery-quick-look = מבט מהיר
cheatsheet-gallery-filter = סינון
cheatsheet-gallery-bigger = תמונות ממוזערות גדולות יותר
cheatsheet-gallery-smaller = תמונות ממוזערות קטנות יותר
cheatsheet-gallery-default-size = גודל רגיל לתמונות ממוזערות
cheatsheet-gallery-refresh = רענון התיקייה
cheatsheet-gallery-menu = תפריט הקשר
cheatsheet-compare-views = השוואה
cheatsheet-compare-overlay = שכבת־על או זו לצד זו
cheatsheet-compare-difference = הפרש (שוב: הסוג הבא)
cheatsheet-compare-step-down = פחות מיזוג או הגברת הפרש
cheatsheet-compare-step-up = יותר מיזוג או הגברת הפרש
cheatsheet-compare-pick = התמונה שעליה פועלים המקשים
cheatsheet-compare-close = סגירת ההשוואה
cheatsheet-channels = ערוצים
cheatsheet-medical-window = חלון ורמה
cheatsheet-medical-measure = מדידה והערות
cheatsheet-medical-frames = פריימים וסדרות
# machine-drafted, needs native review
cheatsheet-medical-3d = תלת־ממד ושחזור מחדש
cheatsheet-medical-analysis = סגמנטציה, מיזוג ו־PET
# machine-drafted, needs native review
cheatsheet-medical-studies = בדיקות ושיתוף
cheatsheet-palette-only-title = בלוח הפקודות ({ $key })
cheatsheet-palette-only-title-nokey = בלוח הפקודות
cheatsheet-palette-only-tag = לוח
cheatsheet-palette-only-line = { $action }: אין מקש; בלוח הפקודות ({ $key })
cheatsheet-palette-only-line-nokey = { $action }: אין מקש; בלוח הפקודות
# Esc בדף הקיצורים (התיאור הארוך שלו בלוח הפקודות לא נכנס).
cheatsheet-back = סגירה או חזרה

## Hardware video encoding (machine-drafted, needs native review)
hw-report-path-video-encode = ייצוא וידאו
hw-report-engine-hw-encoder = מקודד חומרה

## Path timing units (machine-drafted, needs native review)
hw-report-task-thumbnail = { $task } (לכל תמונה ממוזערת בגודל { $size })
hw-report-task-frame = { $task } (לכל פריים בגודל { $size })

## Hardware report, software-only formats (machine-drafted, needs native review)
hw-report-codec-software = בתוכנה בלבד

about-natural-earth = נתוני מפה מ-Natural Earth ‏(naturalearthdata.com), נחלת הכלל.
app-info-show-on-map = הצגה במפה
