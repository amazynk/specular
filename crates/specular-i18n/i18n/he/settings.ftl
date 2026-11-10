# machine-drafted, needs native review
# Specular — חלון ההגדרות. עברית.

## Window chrome

settings-title = הגדרות
settings-search-placeholder = חיפוש בהגדרות
settings-search-no-results = אין הגדרות שתואמות ל־{ $query }
settings-reset = איפוס
settings-reset-section = איפוס המקטע
settings-modified = שונה
settings-default-value = ברירת מחדל: { $value }
settings-restart-required = ייכנס לתוקף לאחר הפעלה מחדש
settings-restart-now = הפעלה מחדש עכשיו
settings-undo = ביטול פעולה
settings-redo = ביצוע חוזר
settings-value-on = מופעל
settings-value-off = כבוי
settings-value-auto = אוטומטי
settings-value-system = לפי המערכת
settings-value-system-language = ברירת המחדל של המערכת
settings-value-follow-language = לפי שפת הממשק
settings-value-default-location = מיקום ברירת המחדל
settings-choose = בחירה…
settings-clear = ניקוי
settings-file-reloaded = הקובץ settings.toml שונה מחוץ ל־{ -app-name } ונטען מחדש.
settings-warning-unknown-key = הגדרה לא מוכרת { $key } בקובץ settings.toml. היא נשמרת בקובץ אך אינה בשימוש.
settings-warning-invalid-value = ערך לא חוקי עבור { $key }: { $reason } במקומו נעשה שימוש בברירת המחדל.
settings-warning-newer-version = הקובץ settings.toml נכתב על ידי גרסה חדשה יותר של { -app-name }. הגדרות שגרסה זו אינה מכירה נשמרות.
settings-warning-syntax = לא ניתן לקרוא את settings.toml (שורה { $line }): { $reason } ייעשה שימוש בברירות המחדל עד לתיקון הקובץ.

## Validation errors (also printed by `specular --set`)

settings-error-unknown-key = אין הגדרה בשם { $key }.
settings-error-unknown-key-suggestion = אין הגדרה בשם { $key }. האם התכוונת ל־{ $suggestion }?
settings-error-not-bool = נדרש מופעל או כבוי (true או false).
settings-error-not-integer = נדרש מספר שלם.
settings-error-not-number = נדרש מספר.
settings-error-out-of-range = הערך חייב להיות בין { $min } ל־{ $max }.
settings-error-out-of-range-auto = הערך חייב להיות 0 (אוטומטי) או בין { $min } ל־{ $max }.
settings-error-unknown-option = נדרש אחד מהערכים: { $options }.
settings-error-bad-colour = נדרש צבע כגון #1E1E1E או #1E1E1E80.
settings-error-bad-language = נדרש תג שפה כגון de, pt-BR או zh-Hans, או “system”.
settings-error-bad-path = נדרש נתיב של קובץ או תיקייה.
settings-error-not-settable = רשומה זו היא פקודה ואין לה ערך.
settings-error-syntax = נדרש key=value, לדוגמה viewing.wheel_behaviour=navigate.
# machine-drafted, needs native review
settings-error-bad-text = לא יכול להכיל טאבים, מעברי שורה או תווי בקרה אחרים.
settings-error-invalid = { $reason }
settings-error-tile-url = נדרשת כתובת http או https עם {"{"}z{"}"}, {"{"}x{"}"} ו־{"{"}y{"}"}.

## Units

unit-percent = { PERCENT($value) }
unit-milliseconds = { $value } אלפיות שנייה
unit-seconds = { $value } שניות
unit-mebibytes = { $value } MiB
# machine-drafted, needs native review
unit-luminance = { $value } cd/m²
unit-images =
    { $value ->
        [one] תמונה אחת
        [two] שתי תמונות
       *[other] { $value } תמונות
    }
unit-threads =
    { $value ->
        [one] תהליכון אחד
        [two] שני תהליכונים
       *[other] { $value } תהליכונים
    }
unit-bytes =
    { $value ->
        [one] בייט אחד
        [two] שני בייטים
       *[other] { $value } בייט
    }
unit-kilobytes = { $value } KB
unit-megabytes = { $value } MB
unit-gigabytes = { $value } GB
unit-terabytes = { $value } TB
unit-kibibytes = { $value } KiB
unit-gibibytes = { $value } GiB
unit-tebibytes = { $value } TiB
unit-symbol-mm = מ״מ
unit-symbol-cm = ס״מ
unit-symbol-m = מ׳
unit-symbol-km = ק״מ
unit-symbol-mm2 = ממ״ר
unit-symbol-cm2 = סמ״ר
unit-symbol-m2 = מ״ר
unit-symbol-km2 = קמ״ר
unit-symbol-ha = הקטר
unit-symbol-ft = רגל
unit-symbol-ft2 = רגל רבוע
unit-symbol-mi = מייל
unit-symbol-mi2 = מייל רבוע
unit-symbol-ac = אקר

## Sections

settings-section-general = כללי
settings-section-viewing = תצוגה
settings-section-browsing = דפדוף
settings-section-colour = צבע
settings-section-metadata = מטא־נתונים ופרטיות
# machine-drafted, needs native review
settings-section-privacy = פרטיות
settings-section-performance = ביצועים
settings-section-accessibility = נגישות
settings-section-keys = מקשים
settings-section-advanced = מתקדם

## General

settings-general-language = שפת הממשק
settings-general-language-help = השינויים נכנסים לתוקף מיד. “ברירת המחדל של המערכת” פועלת לפי שפת מערכת ההפעלה.
settings-general-region-formats = תבניות אזוריות
settings-general-region-formats-help = אופן הכתיבה של תאריכים, שעות, מספרים וגדלים. כברירת מחדל הם פועלים לפי שפת הממשק.
settings-general-theme = ערכת נושא
settings-general-theme-help = מראה בהיר או כהה לחלונות ולפקדים. התמונות עצמן אינן משתנות לעולם.
settings-general-theme-system = לפי המערכת
settings-general-theme-light = בהיר
settings-general-theme-dark = כהה
# machine-drafted, needs native review
settings-general-on-start = כאשר { -app-name } נפתח ללא קובץ
settings-general-on-start-help = מה להציג כאשר { -app-name } נפתח בפני עצמו. קובץ שנפתח ממנהל הקבצים או משורת הפקודה מוצג תמיד במציג.
settings-general-on-start-start-screen = מסך הפתיחה
settings-general-on-start-gallery = הגלריה של התיקייה האחרונה
settings-general-on-start-library = ספרייה
settings-general-start-tips = הצגת „כדאי לנסות” במסך הפתיחה
settings-general-start-tips-help = שורת כרטיסים עם תכונות שימושיות והמקשים שלהן. „הצגת מסך הפתיחה” בלוח הפקודות מחזירה אותה.
settings-general-tips = הצגת עצות
settings-general-tips-help = עצה קצרה, פעם אחת, בפעם הראשונה שתכונה יכולה לעזור, למשל המקש שמציג תיקייה כגלריה. אף פעם לא במהלך מצגת.
settings-general-tips-shown = עצות שכבר הוצגו
settings-general-tips-shown-help = העצות החד־פעמיות שכבר ניתנו, כדי שלא יוצגו שוב.
# machine-drafted, needs native review
settings-general-your-name = השם שלך
settings-general-your-name-help = מוצג על חותמות וסימונים שנוספו על ידך. נשמר רק בקובצי הסימונים שיצרת.
settings-general-single-instance = פתיחת קבצים בחלון הקיים
settings-general-single-instance-help = כאשר { -app-name } כבר פועל, פתיחת תמונה נוספת מציגה אותה באותו חלון במקום להפעיל מופע חדש.
settings-general-file-associations = שיוכי קבצים…
settings-general-file-associations-help = הגדרת { -app-name } כמציג ברירת המחדל לפורמטי התמונה שהוא תומך בהם.

## Viewing

settings-viewing-initial-zoom = זום התחלתי
settings-viewing-initial-zoom-help = באיזה גודל מוצגת תמונה שנפתחה זה עתה.
settings-viewing-initial-zoom-fit = התאמה לחלון
settings-viewing-initial-zoom-fit-if-larger = התאמת תמונות גדולות, הצגת קטנות ב־{ PERCENT(100) }
settings-viewing-initial-zoom-actual-size = גודל אמיתי ({ PERCENT(100) })
settings-viewing-initial-zoom-fill = מילוי החלון
settings-viewing-wheel-behaviour = גלגלת העכבר
settings-viewing-wheel-behaviour-help = מה קורה בגלילת הגלגלת. החזקת Ctrl מבצעת את הפעולה האחרת.
settings-viewing-wheel-behaviour-help-macos = מה קורה בגלילת הגלגלת. החזקת ⌘ מבצעת את הפעולה האחרת.
settings-viewing-wheel-behaviour-zoom = זום
settings-viewing-wheel-behaviour-navigate = התמונה הבאה והקודמת
settings-viewing-keep-zoom = שמירת הזום בין תמונות
settings-viewing-keep-zoom-help = כשהתצוגה מוגדלת, התמונה הבאה והקודמת נפתחות באותו זום ומציגות את אותו חלק של הפריים: שימושי לבדיקת מיקוד ברצף צילומים.
settings-viewing-zoom-animation = הנפשת זום
settings-viewing-zoom-animation-help = זום חלק במקום קפיצה. מושבת אוטומטית כאשר “הפחתת תנועה” מופעלת.
settings-viewing-zoom-animation-speed = מהירות הנפשת הזום
settings-viewing-zoom-animation-speed-help = באיזו מהירות מתייצב זום מונפש.
settings-viewing-zoom-animation-speed-relaxed = נינוחה
settings-viewing-zoom-animation-speed-normal = רגילה
settings-viewing-zoom-animation-speed-fast = מהירה
settings-viewing-background = רקע
settings-viewing-background-help = הצבע שמאחורי התמונה בחלון.
settings-viewing-background-theme = לפי ערכת הנושא
settings-viewing-background-black = שחור
settings-viewing-background-dark-grey = אפור כהה
settings-viewing-background-light-grey = אפור בהיר
settings-viewing-background-white = לבן
settings-viewing-background-custom = צבע מותאם אישית
settings-viewing-background-color = צבע רקע מותאם אישית
settings-viewing-background-color-help = בשימוש כאשר “רקע” מוגדר ל־“צבע מותאם אישית”.
settings-viewing-fullscreen-background = רקע במסך מלא
settings-viewing-fullscreen-background-help = הצבע שמאחורי התמונה במסך מלא.
settings-viewing-transparency = אזורים שקופים
settings-viewing-transparency-help = אופן הציור של חלקים שקופים בתמונה.
settings-viewing-transparency-checkerboard = לוח שחמט
settings-viewing-transparency-background = צבע הרקע
settings-viewing-transparency-black = שחור
settings-viewing-transparency-white = לבן
settings-viewing-pixel-grid-threshold = הצגת רשת פיקסלים החל מ־
settings-viewing-pixel-grid-threshold-help = ציור רשת בין הפיקסלים כאשר הזום הוא לפחות בערך זה.
settings-viewing-smoothing-high-zoom = פיקסלים מוגדלים
settings-viewing-smoothing-high-zoom-help = איך נראים הפיקסלים כאשר התמונה מוצגת בגודל של יותר מ־{ PERCENT(100) }.
settings-viewing-smoothing-high-zoom-auto = חלק, ומ־{ PERCENT(400) } ריבועים חדים
settings-viewing-smoothing-high-zoom-smooth = תמיד חלק
settings-viewing-smoothing-high-zoom-pixelated = תמיד ריבועים חדים
settings-viewing-double-click = לחיצה כפולה על התמונה
settings-viewing-double-click-help = לחיצה כפולה על הרקע הריק תמיד עוברת למסך מלא או יוצאת ממנו.
settings-viewing-double-click-toggle-zoom = מעבר בין התאמה ל־{ PERCENT(100) }
settings-viewing-double-click-fullscreen = מעבר למסך מלא או יציאה ממנו
settings-viewing-double-click-nothing = ללא פעולה
settings-viewing-fullscreen-on-open = פתיחת תמונות במסך מלא
settings-viewing-fullscreen-on-open-help = הפעלה במסך מלא כאשר תמונה נפתחת מחוץ ל־{ -app-name }.
settings-viewing-fullscreen-mode = סגנון מסך מלא
settings-viewing-fullscreen-mode-help = מסך מלא מקורי משתמש ב־Space משלו; מסך מלא מיידי מכסה את ה־Space הנוכחי ללא הנפשה.
settings-viewing-fullscreen-mode-native = מקורי (Space נפרד)
settings-viewing-fullscreen-mode-borderless = מיידי
settings-viewing-toolbar-autohide = הסתרה אוטומטית של סרגל הכלים
settings-viewing-toolbar-autohide-help = סרגל הכלים וחיצי הקצוות נעלמים בהדרגה כשהמצביע נייח, וחוזרים כשהוא זז.
settings-viewing-toolbar-autohide-delay = הסתרת סרגל הכלים לאחר
settings-viewing-toolbar-autohide-delay-help = כמה זמן המצביע צריך להיות נייח לפני שסרגל הכלים נעלם.
settings-viewing-toolbar-items = לחצני סרגל הכלים
settings-viewing-toolbar-items-help = אילו לחצנים מוצגים בסרגל הכלים, ובאיזה סדר.
settings-viewing-toolbar-drag = סידור סרגל הכלים מחדש בגרירה
settings-viewing-toolbar-drag-help = גררו לחצן לאורך סרגל הכלים כדי להזיז אותו. לחיצה קצרה עדיין מפעילה אותו; Esc מחזיר לחצן גרור למקומו.
settings-viewing-toolbar-items-open = פתיחה
settings-viewing-toolbar-items-prev = התמונה הקודמת
settings-viewing-toolbar-items-next = התמונה הבאה
settings-viewing-toolbar-items-zoom-out = התרחקות
settings-viewing-toolbar-items-zoom-level = רמת זום
settings-viewing-toolbar-items-zoom-in = התקרבות
settings-viewing-toolbar-items-fit = התאמה לחלון
settings-viewing-toolbar-items-rotate-left = סיבוב נגד כיוון השעון
settings-viewing-toolbar-items-rotate-right = סיבוב בכיוון השעון
settings-viewing-toolbar-items-info = מידע
settings-viewing-toolbar-items-copy = העתקת התמונה
settings-viewing-toolbar-items-delete = העברה לאשפה
settings-viewing-toolbar-items-palette = לוח פקודות
settings-viewing-toolbar-items-settings = הגדרות
settings-viewing-toolbar-items-fullscreen = מסך מלא
# machine-drafted, needs native review
settings-viewing-toolbar-items-spacer = רווח גמיש
settings-viewing-toolbar-items-separator = מפריד
settings-toolbar-preview = תצוגה מקדימה של סרגל הכלים
settings-toolbar-hidden = לחצנים מוסתרים
settings-toolbar-add-spacer = הוספת רווח גמיש
settings-toolbar-add-separator = הוספת מפריד
settings-toolbar-reset = איפוס לברירת המחדל
settings-toolbar-remove = הסרת { $item }
settings-toolbar-hint = גררו שורה כדי להזיז אותה, או בחרו בה והקישו Alt+↑ או Alt+↓. Tab עובר בין שדה החיפוש לרשימה.
settings-toolbar-move-up = הזזה למעלה
settings-toolbar-move-down = הזזה למטה
settings-toolbar-move-start = הזזה להתחלה
settings-toolbar-move-end = הזזה לסוף
settings-toolbar-menu-hide = הסתרה
settings-toolbar-menu-show = הצגה
settings-toolbar-menu-remove = הסרה
settings-toolbar-row-menu = פעולות נוספות עבור { $item }
settings-toolbar-position = מיקום { $index } מתוך { $count }
settings-toolbar-hidden-state = מוסתר

## Browsing

settings-browsing-sort-order = מיון תמונות לפי
settings-browsing-sort-order-help = הסדר שבו עוברים בין התמונות בתיקייה. מיון לפי שם מציב את “img2” לפני “img10” ופועל לפי האלפבית של השפה שלך.
settings-browsing-sort-order-natural = שם
settings-browsing-sort-order-date-taken = תאריך צילום
settings-browsing-sort-order-modified = תאריך שינוי
settings-browsing-sort-order-size = גודל הקובץ
settings-browsing-sort-order-type = סוג הקובץ
settings-browsing-sort-descending = סדר הפוך
settings-browsing-sort-descending-help = מיון מהאחרון לראשון.
settings-browsing-loop-folder = חזרה להתחלה בסוף התיקייה
settings-browsing-loop-folder-help = אחרי התמונה האחרונה ממשיכים לראשונה, וגם בכיוון ההפוך.
settings-browsing-include-subfolders = כולל תיקיות משנה
settings-browsing-include-subfolders-help = מעבר גם בין התמונות שבתיקיות המשנה.
settings-browsing-hold-acceleration = האצה בהחזקת הבא או הקודם
settings-browsing-hold-acceleration-help = באיזו מהירות הדפדוף מואץ בזמן שמקש או לחצן מוחזקים.
settings-browsing-hold-acceleration-off = ללא האצה
settings-browsing-hold-acceleration-gentle = עדינה
settings-browsing-hold-acceleration-normal = רגילה
settings-browsing-hold-acceleration-fast = מהירה
settings-browsing-confirm-trash = לשאול לפני העברה לאשפה
settings-browsing-confirm-trash-help = תמיד אפשר לבטל העברה לאשפה, ולכן כברירת מחדל { -app-name } אינו שואל.
settings-browsing-arrows-follow-reading-direction = מקשי החיצים לפי כיוון הקריאה
settings-browsing-arrows-follow-reading-direction-help = בשפות הנכתבות מימין לשמאל, מקש החץ השמאלי עובר לתמונה הבאה.

## Colour

settings-colour-management = ניהול צבע
settings-colour-management-help = הצגת הצבעים כפי שנועדו להיראות, באמצעות פרופיל הצבע של התמונה והפרופיל של המסך.
settings-colour-display-profile = פרופיל המסך
settings-colour-display-profile-help = פרופיל הצבע של המסך. “אוטומטי” משתמש בפרופיל שמערכת ההפעלה מדווחת עליו.
settings-colour-rendering-intent = כוונת עיבוד
settings-colour-rendering-intent-help = האופן שבו צבעים שהמסך אינו יכול להציג מובאים לטווח שלו.
settings-colour-rendering-intent-perceptual = תפיסתית
settings-colour-rendering-intent-relative-colorimetric = קולורימטרית יחסית
settings-colour-rendering-intent-saturation = רוויה
settings-colour-rendering-intent-absolute-colorimetric = קולורימטרית מוחלטת
settings-colour-black-point-compensation = פיצוי נקודת שחור
settings-colour-black-point-compensation-help = שמירה על פרטים בצללים כאשר השחור של המסך בהיר מזה של התמונה.
settings-colour-untagged-images = תמונות ללא פרופיל צבע
settings-colour-untagged-images-help = איך להציג תמונות שאינן מציינות באילו צבעים הן משתמשות.
settings-colour-untagged-images-srgb = טיפול כ־sRGB
settings-colour-untagged-images-display = הצגה ללא שינוי
settings-colour-dithering = ריכוך (Dithering)
settings-colour-dithering-help = הוספת רעש כחול עדין בעת הפחתת עומק הצבע, כדי למנוע פסים בשמיים ובמעברי צבע.
settings-colour-hdr-output = פלט HDR
settings-colour-hdr-output-help = הצגת תמונות HDR בבהירות המלאה שלהן במסכים שתומכים בכך. במצב אוטומטי נעשה שימוש ב‑HDR כשהמסך מדווח על בהירות נוספת; אחרת האזורים הבהירים נדחסים בעדינות.
settings-colour-hdr-output-auto = אוטומטי
settings-colour-hdr-output-on = פועל
settings-colour-hdr-output-off = כבוי
# machine-drafted, needs native review
settings-colour-ocio-config = תצורת OCIO
settings-colour-ocio-config-help = תצורת OpenColorIO (.ocio) לתמונות EXR, Radiance HDR ו־PFM. כשהשדה ריק נעשה שימוש בתצורה ש־‎$OCIO מציין.
settings-colour-ocio-display = מסך OCIO
settings-colour-ocio-display-help = המסך (display) בתצורת ה־OCIO שמתאים לצג שלך.
settings-colour-ocio-display-default = הראשון בתצורה
settings-colour-ocio-view = תצוגת OCIO
settings-colour-ocio-view-help = האופן שבו תמונות scene-linear מוצגות במסך הזה, למשל תצוגה עם מיפוי טונים.
settings-colour-ocio-view-default = הראשונה של המסך
settings-colour-ocio-input = מרחב צבע הקלט של OCIO
settings-colour-ocio-input-help = מרחב הצבע של ערכי התמונות.
settings-colour-ocio-input-default = תפקיד scene-linear בתצורה
# machine-drafted, needs native review
settings-colour-proof-profile = פרופיל הגהה על המסך
settings-colour-proof-profile-help = פרופיל המדפסת שבו השתמשה ההגהה ב״פיתוח״ בפעם האחרונה.
settings-colour-proof-intent = כוונת עיבוד בהגהה
settings-colour-proof-intent-help = איך מקרבים צבעים שהמדפסת לא יכולה להדפיס.
settings-colour-proof-intent-perceptual = תפיסתית
settings-colour-proof-intent-relative = קולורימטרית יחסית
settings-colour-proof-bpc = פיצוי נקודת שחור בהגהה
settings-colour-proof-bpc-help = התאמת השחור לשחור של המדפסת.
settings-colour-proof-paper = לבן הנייר בהגהה
settings-colour-proof-paper-help = הצגת הצבע של הנייר עצמו.
settings-colour-proof-ink = שחור הדיו בהגהה
settings-colour-proof-ink-help = הצגת מידת הכהות של שחור הדיו.

## Metadata and privacy

settings-metadata-show-info-on-open = הצגת חלונית המידע בפתיחה
settings-metadata-show-info-on-open-help = פתיחת החלונית עם פרטי המצלמה, התאריך והמקום בכל פעם שתמונה נפתחת.
settings-metadata-info-detail = חלונית המידע מציגה
settings-metadata-info-detail-help = הסיכום מסביר את הפרטים השימושיים ביותר בשפה פשוטה; “כל השדות” מציג את כל מה שנשמר בקובץ.
settings-metadata-info-detail-summary = סיכום
settings-metadata-info-detail-all = כל השדות
settings-metadata-reverse-geocoding = הצגת שמות מקומות למיקומי GPS
settings-metadata-reverse-geocoding-help = איתור היישוב הקרוב ביותר מנתונים מובנים. פועל ללא חיבור לרשת: המיקומים לעולם אינם יוצאים מהמחשב.
settings-metadata-hide-gps = הסתרת מיקומי GPS על המסך
settings-metadata-hide-gps-help = לא להציג קואורדינטות או מקומות, למשל בזמן שיתוף מסך. הקבצים אינם משתנים.
# machine-drafted, needs native review
settings-metadata-develop-sidecar = שמירת עריכות הפיתוח ליד התמונה
# machine-drafted, needs native review
settings-metadata-develop-sidecar-help = כותב IMG_1234.CR2.specular.xmp ליד הקובץ, כך שהעריכות עוברות איתו. כבוי: העריכות נשמרות בספרייה בלבד.
# machine-drafted, needs native review
settings-metadata-develop-import-lightroom = ייבוא הגדרות Lightroom בפיתוח
# machine-drafted, needs native review
settings-metadata-develop-import-lightroom-help = תמונה עם הגדרות Lightroom וללא עריכה נפתחת איתן, בקירוב. כבוי: הפיתוח מציע כפתור.
settings-metadata-units = יחידות
settings-metadata-units-help = יחידות לגובה, למרחקים ולמהירויות.
settings-metadata-units-auto = לפי התבניות האזוריות
settings-metadata-units-metric = מטריות
settings-metadata-units-imperial = אימפריאליות
# machine-drafted, needs native review
settings-metadata-basemap = מפת בסיס
settings-metadata-basemap-help = המפה שמתחת למיקומי התמונות. מפות מקוונות מורידות אריחים לאזור המוצג.
settings-metadata-basemap-off = מפה מובנית בלבד (לא מקוון)
settings-metadata-basemap-openstreetmap = OpenStreetMap (מקוון)
settings-metadata-basemap-custom = שרת אריחים אחר (מקוון)
settings-metadata-basemap-file = קובץ מפה משלי
settings-metadata-basemap-url = כתובת שרת האריחים
settings-metadata-basemap-url-help = כתובת עם {"{"}z{"}"}, {"{"}x{"}"} ו־{"{"}y{"}"}, למשל https://tiles.example.org/{"{"}z{"}"}/{"{"}x{"}"}/{"{"}y{"}"}.png
settings-metadata-basemap-path = קובץ מפה
settings-metadata-basemap-path-help = קובץ MBTiles או PMTiles. פועל ללא חיבור לרשת.
settings-metadata-basemap-cache = מטמון אריחי מפה
settings-metadata-basemap-cache-help = שטח דיסק לאריחים שהורדו. הישנים ביותר נמחקים ראשונים.
# machine-drafted, needs native review
settings-metadata-basemap-allowed = שרתי אריחים מורשים
settings-metadata-basemap-allowed-help = שרתי מפות שהסכמת להוריד מהם אריחים.

## Performance

settings-performance-ram-cache = מטמון זיכרון
settings-performance-ram-cache-help = זיכרון שמשמש לשמירת תמונות מפוענחות מוכנות. “אוטומטי” בוחר לפי כמות הזיכרון שבמחשב.
settings-performance-vram-cache = מטמון בזיכרון ה־GPU
settings-performance-vram-cache-help = זיכרון GPU שמשמש לתמונות ולאריחים. “אוטומטי” בוחר לפי ה־GPU.
settings-performance-prefetch-depth = תמונות להכנה מראש
settings-performance-prefetch-depth-help = כמה תמונות בכיוון התנועה מפוענחות מראש.
settings-performance-decoder-threads = תהליכוני פענוח
settings-performance-decoder-threads-help = תהליכוני CPU שמשמשים לפענוח תמונות. “אוטומטי” משתמש בכל ליבות הביצועים ומסתגל לקובץ.
settings-performance-gpu-adapter = GPU
settings-performance-gpu-adapter-help = באיזה GPU להשתמש כאשר יש במחשב יותר מאחד.
settings-performance-gpu-adapter-high-performance = הביצועים הגבוהים ביותר
settings-performance-gpu-adapter-low-power = צריכת החשמל הנמוכה ביותר
settings-performance-processing = היכן מעובדות התמונות
settings-performance-processing-help = חלק מהעבודה יכול לרוץ על ה־CPU, על ה־GPU או על מפענח חומרה. במצב אוטומטי המחשב נמדד ונבחרת הדרך המהירה ביותר לתמונה המוצגת והחסכונית ביותר לעבודה ברקע. מה נבחר במחשב הזה ולמה מוצג למטה.
settings-performance-processing-automatic = אוטומטי
settings-performance-processing-prefer-gpu = העדפת ה־GPU
settings-performance-processing-cpu-only = CPU בלבד
settings-performance-drag-preview = תצוגה מקדימה בזמן גרירה
settings-performance-drag-preview-help = איך הפיתוח מציג את התמונה בזמן גרירת מחוון. אוטומטי משתמש בתמונה קטנה יותר רק כשהמחשב לא עומד בקצב.
settings-performance-drag-preview-automatic = אוטומטי
settings-performance-drag-preview-sharp = תמיד חדה
settings-performance-drag-preview-fast = תמיד מהירה

## Settings → Performance → How { -app-name } runs on this computer (DESIGN.md §12.3.1)
# machine-drafted, needs native review
settings-paths-title = איך { -app-name } פועל במחשב הזה
settings-paths-intro = ‏{ -app-name } מודד את הזמן של כל דרך לבצע את המשימות האלה במחשב הזה ומשתמש בטובה ביותר. הזמנים הם לתמונה של { $mpx } מגה־פיקסל.
settings-paths-intro-measuring = המדידה עוד לא הסתיימה: { -app-name } מודד כשהמחשב פנוי.
settings-paths-intro-battery = בסוללה: הבחירות מעדיפות צריכת אנרגיה נמוכה יותר.
settings-paths-keywords = GPU CPU NPU כרטיס גרפי חומרה האצה מפענח מדד ביצועים מהירות
paths-engine-cpu = CPU
paths-engine-cpu-parallel = CPU, כל הליבות
paths-engine-gpu = GPU
paths-engine-hw-decoder = מפענח חומרה
paths-engine-npu = NPU
settings-paths-your-setting = { $engine } (ההגדרה שלך)
settings-paths-measured-faster = נמדד במחשב הזה: מהיר פי { $ratio } לעומת { $other }.
settings-paths-measured-similar = נמדד במחשב הזה: מהיר בערך כמו { $other }.
settings-paths-measured-power = בסוללה זו הדרך שצורכת הכי מעט אנרגיה (נמדד במחשב הזה).
settings-paths-not-measured = עוד לא נמדד: הבחירה הרגילה לסוג מחשב כזה.
settings-paths-set-by = נבחר לפי ההגדרה שלך "{ $setting }: { $option }".
settings-paths-blocked-check = { $engine } כובה ב־{ $date }: התוצאה שלו לא תאמה את זו של ה־CPU.
settings-paths-blocked-error = { $engine } כובה ב־{ $date } אחרי שגיאה.
settings-paths-blocked-lost = { $engine } כובה ב־{ $date } אחרי שהפסיק להגיב.
settings-paths-blocked-undated = { $engine } כובה אחרי שגיאה.
settings-paths-checking = { $engine } נבדק מול ה־CPU לפני שהוא משמש לתמונה המוצגת.
settings-paths-exploring = { $engine } נבדק ברקע עכשיו.
settings-paths-unavailable = אין במחשב הזה { $engine } למשימה הזאת.
settings-paths-background-cheaper = ברקע: { $engine }, שנמדד כצורך { $percent } פחות זמן CPU.
settings-paths-background-power = ברקע: { $engine }, כדי לחסוך באנרגיה.
settings-paths-background = ברקע: { $engine }.
settings-paths-detail-measured = { $engine }: { $time } ms, { $cpu } ms זמן CPU, { $runs ->
    [one] מדידה אחת
    [two] { $runs } מדידות
   *[other] { $runs } מדידות
} ({ $confidence })
settings-paths-detail-guess = { $engine }: בערך { $time } ms (הערכה)
settings-paths-confidence-low = ודאות נמוכה
settings-paths-confidence-medium = ודאות בינונית
settings-paths-confidence-high = ודאות גבוהה
settings-paths-machine = נמדד עבור { $gpu } · { $driver } · { $cpu }
settings-paths-retry = לנסות שוב
settings-paths-measure-again = למדוד מחדש
settings-paths-measure-again-question = לשכוח את מה שנמדד במחשב הזה, כולל הדרכים שכובו, ולמדוד מחדש כשהמחשב פנוי?
settings-paths-measure-again-confirm = לשכוח את המדידות

settings-performance-display-cache = שמירת מטמון תצוגה בדיסק
settings-performance-display-cache-help = שמירת עותקים של תמונות בגודל המסך, כך שתיקיות שכבר נצפו נפתחות מיד.
settings-performance-display-cache-size = גודל מטמון התצוגה
settings-performance-display-cache-size-help = הרשומות הישנות ביותר מוסרות כאשר המטמון מלא.
settings-performance-display-cache-location = מיקום מטמון התצוגה
settings-performance-display-cache-location-help = תיקייה בכונן מהיר היא הבחירה הטובה ביותר.
settings-performance-fast-simd = שימוש בהוראות SIMD
settings-performance-fast-simd-help = שימוש בהוראות הווקטוריות של ה־CPU לפענוח. יש לכבות רק לצורך אבחון בעיה.
settings-performance-fast-dct-scaling = פענוח בגודל המסך
settings-performance-fast-dct-scaling-help = פענוח קובצי JPEG גדולים ישירות בגודל שבו הם מוצגים. יש לכבות רק לצורך אבחון בעיה.
settings-performance-fast-parallel-decode = פענוח על כמה ליבות
settings-performance-fast-parallel-decode-help = פיצול תמונות גדולות בין ליבות ה־CPU. יש לכבות רק לצורך אבחון בעיה.
settings-performance-fast-restart-index = יצירת אינדקס לקובצי JPEG גדולים לצורך זום
settings-performance-fast-restart-index-help = שמירת המיקום שבו מתחיל כל חלק של קובץ JPEG, כך שאזורים מוגדלים מפוענחים במהירות. יש לכבות רק לצורך אבחון בעיה.
settings-performance-fast-yuv-upload = המרת צבעים ב־GPU
settings-performance-fast-yuv-upload-help = המרת צבעי JPEG ב־GPU או ב־CPU. במצב אוטומטי נבחר המהיר מביניהם במחשב זה. יש לכבות רק לצורך אבחון בעיה.
settings-performance-fast-yuv-upload-auto = אוטומטי
settings-performance-fast-yuv-upload-on = מופעל
settings-performance-fast-yuv-upload-off = כבוי
settings-performance-fast-async-io = קריאת קבצים אסינכרונית
settings-performance-fast-async-io-help = קריאה מראש באמצעות ממשק הקבצים המהיר ביותר של מערכת ההפעלה. יש לכבות רק לצורך אבחון בעיה.
settings-performance-fast-mmap = קבצים ממופים לזיכרון
settings-performance-fast-mmap-help = קריאת קבצים בלי להעתיק אותם. יש לכבות רק לצורך אבחון בעיה.
settings-performance-hardware-decode = פענוח בחומרה
settings-performance-hardware-decode-help = פתיחת תמונות JPEG באמצעות המפענח המובנה ב־GPU. במצב אוטומטי הוא משמש היכן שהוא מועיל, בדרך כלל לתמונות שמוכנות ברקע. תחילה הוא מושווה למפענח הרגיל, ומה שאינו נתמך נפתח כרגיל.
settings-performance-hardware-decode-auto = אוטומטי
settings-performance-hardware-decode-on = מופעל
settings-performance-hardware-decode-off = כבוי
settings-performance-frame-time-hud = HUD זמני פריימים
settings-performance-frame-time-hud-help = הצגת זמני הפענוח והפריימים, ואילו נתיבים מהירים פעילים.

## Accessibility

settings-accessibility-reduce-motion = הפחתת תנועה
settings-accessibility-reduce-motion-help = החלפת הנפשות של זום, החלקה ודהייה בשינויים מיידיים.
settings-accessibility-reduce-motion-system = לפי המערכת
settings-accessibility-reduce-motion-on = מופעל
settings-accessibility-reduce-motion-off = כבוי
settings-accessibility-increase-contrast = הגברת ניגודיות
settings-accessibility-increase-contrast-help = גבולות וטקסט בולטים יותר, ופחות שקיפות.
settings-accessibility-increase-contrast-system = לפי המערכת
settings-accessibility-increase-contrast-on = מופעל
settings-accessibility-increase-contrast-off = כבוי
settings-accessibility-text-scale = גודל טקסט
settings-accessibility-text-scale-help = גודל הטקסט ביחס לגודל הרגיל.
settings-accessibility-ui-scale = קנה מידה של הממשק
settings-accessibility-ui-scale-help = גודל הממשק כולו, בנוסף לקנה המידה של התצוגה שהוגדר במערכת ההפעלה.
settings-accessibility-focus-ring-always = הצגת מסגרת המיקוד תמיד
settings-accessibility-focus-ring-always-help = הצגת הפקד שבמיקוד המקלדת גם בעת שימוש בעכבר.
settings-accessibility-screen-reader-verbosity = רמת פירוט של קורא המסך
# machine-drafted, needs native review
settings-accessibility-screen-reader-verbosity-help = כמה מידע קורא המסך מכריז במעבר בין תמונות, וכמה מהקריאה שמתחת לסמן הוא מקריא.
settings-accessibility-screen-reader-verbosity-concise = תמציתית
settings-accessibility-screen-reader-verbosity-normal = רגילה
settings-accessibility-screen-reader-verbosity-verbose = מפורטת
# machine-drafted, needs native review
settings-accessibility-announce-cursor = הקראת הערך שמתחת לסמן
settings-accessibility-announce-cursor-help = קורא המסך מקריא את ערך הפיקסל, HU, הטמפרטורה או המיקום שמתחת לסמן או לכוונת המקלדת כשהם מפסיקים לנוע.

## פרטיות

# machine-drafted, needs native review
settings-privacy-no-network = ללא רשת
# machine-drafted, needs native review
settings-privacy-no-network-help = כבוי: רק אלה מתחברים, כל אחד אחרי שהפעלת אותו או פתחת בו משהו: בדיקת עדכונים, הורדת מודלים של בינה מלאכותית מקומית, מפות מקוונות, תמונות מכתובת אינטרנט או מאחסון בענן, ושרתים שהוספת (DICOMweb,‏ OMERO, מפות). מופעל: שום דבר לא מתחבר.

## Advanced

settings-advanced-portable-mode = מצב נייד
settings-advanced-portable-mode-help = שמירת ההגדרות והמטמונים בתיקייה שליד התוכנה במקום בפרופיל המשתמש.
settings-advanced-log-level = רמת פירוט היומן
settings-advanced-log-level-help = כמה מידע { -app-name } כותב לקובץ היומן שלו.
settings-advanced-log-level-error = שגיאות בלבד
settings-advanced-log-level-warn = אזהרות
settings-advanced-log-level-info = מידע
settings-advanced-log-level-debug = ניפוי באגים
settings-advanced-log-level-trace = הכול
settings-advanced-developer-mode = מצב מפתחים
settings-advanced-developer-mode-help = מציג כלים לפיתוח ובדיקה של { -app-name }, כמו שפות הדמה ברשימת השפות.
# machine-drafted, needs native review
settings-advanced-audit-log = יומן ביקורת מקומי
# machine-drafted, needs native review
settings-advanced-audit-log-help = רושם תמונות רפואיות שנפתחו, ייצואים, התממה, הדפסות ושליחות ל-PACS בקובץ audit.jsonl בתיקיית הנתונים. היומן נשאר במחשב זה.
settings-advanced-hw-report = דוח חומרה…
settings-advanced-hw-report-help = הצגת תכונות ה־CPU וה־GPU ש־{ -app-name } זיהה, ואילו נתיבים מהירים הוא משתמש בהם.
# machine-drafted, needs native review
settings-advanced-renderer-info = מידע על הרינדור…
settings-advanced-renderer-info-help = הצגת המתאם הגרפי, מנהל ההתקן, תבניות התצוגה ומגבלות הזיכרון ש־{ -app-name } משתמש בהם, מוכנים להעתקה לדוח באג.
settings-advanced-edit-settings-file = עריכת settings.toml
settings-advanced-edit-settings-file-help = עריכת קובץ ההגדרות בעורך המובנה של { -app-name }. העורך בודק את הקובץ בזמן ההקלדה ושומר אותו רק כשאין בו שגיאות.
settings-advanced-import = ייבוא הגדרות…
settings-advanced-import-help = טעינת הגדרות מקובץ שיוצא מ־{ -app-name }.
settings-advanced-export = ייצוא הגדרות…
settings-advanced-export-help = שמירת כל ההגדרות בקובץ אחד, לגיבוי או למחשב אחר.
settings-advanced-reset-all = איפוס כל ההגדרות…
settings-advanced-reset-all-help = החזרת כל הגדרה לערך ברירת המחדל שלה. ניתן לבטל פעולה זו.

# Stage 1 merge (machine-drafted, needs native review)
settings-performance-stall-warning = דיווח על טעינה תקועה אחרי
settings-performance-stall-warning-help = אם תמונה עדיין נטענת אחרי פרק זמן זה, מצב הטעינה שלה והמשימות הממתינות נכתבים ליומן (פלט השגיאה התקני או SPECULAR_LOG_FILE).
# machine-drafted, needs native review
settings-performance-insights = תובנות ביצועים
settings-performance-insights-help = כשתמונות נפתחות שוב ושוב לאט מאותה סיבה, יוצג בפינה „למה זה איטי?” עם מה שיעזור. מה שנלמד נשאר במחשב זה.
settings-performance-insights-storage = תובנות על אחסון איטי
settings-performance-insights-storage-help = הצגת תובנות כשקריאת קבצים (משיתוף רשת, מכונן איטי או מארכיונים) מעכבת שוב ושוב את פתיחת התמונות.
settings-performance-insights-decoder = תובנות על פענוח
settings-performance-insights-decoder-help = הצגת תובנות כשתמונות מפוענחות שוב ושוב ב־CPU אף שמפענח חומרה יכול לעזור.
# machine-drafted, needs native review
settings-performance-insights-restore = הצגת התובנות המוסתרות שוב
settings-performance-insights-restore-help = מחזיר את התובנות שהסתרת עם × ב„למה זה איטי?”, גם לפתיחת תמונות וגם לתמונות ממוזערות. לא זמין כל עוד אין תובנות מוסתרות.

## settings.toml editor (machine-drafted, needs native review)

config-editor-title = settings.toml
config-editor-save = שמירה
config-editor-revert = שחזור
config-editor-close = סגירה
config-editor-modified = שונה
config-editor-text = קובץ הגדרות
config-editor-position = שורה { $line }, עמודה { $column }
config-editor-saved = settings.toml נשמר והוחל.
config-editor-cant-save =
    { $count ->
        [one] אי אפשר לשמור: בעיה אחת. יש לתקן אותה קודם.
        [two] אי אפשר לשמור: { $count } בעיות. יש לתקן אותן קודם.
       *[other] אי אפשר לשמור: { $count } בעיות. יש לתקן אותן קודם.
    }
config-editor-save-failed = לא ניתן לשמור את settings.toml: { $reason }
config-editor-conflict = settings.toml שונה במקום אחר.
config-editor-reload = טעינה מחדש
config-editor-keep-mine = שמירת הגרסה שלי
config-editor-discard-question = לבטל את השינויים שלא נשמרו ב־settings.toml?
config-editor-discard = ביטול השינויים
config-editor-keep-editing = המשך עריכה
config-editor-find-placeholder = חיפוש בקובץ
config-editor-find-count = { $current } מתוך { $total }
config-editor-find-none = אין התאמות
config-editor-find-previous = ההתאמה הקודמת
config-editor-find-next = ההתאמה הבאה
config-editor-find-close = סגירת החיפוש
config-editor-problems = בעיות
config-editor-problems-count =
    { $count ->
        [0] אין בעיות
        [one] בעיה אחת
        [two] { $count } בעיות
       *[other] { $count } בעיות
    }
config-editor-no-problems = אין בעיות. אפשר לשמור את הקובץ.
config-editor-problem-at = שורה { $line }: { $message }
config-editor-severity-error = שגיאה
config-editor-severity-warning = אזהרה
config-editor-severity-info = הערה
config-editor-problem-syntax = TOML לא תקין: { $reason }
config-editor-problem-duplicate = { $key } מוגדר פעמיים באותה טבלה. יש להסיר אחד מהם.
config-editor-problem-unknown-section = אין מקטע הגדרות [{ $key }].
config-editor-problem-unknown-section-suggestion = אין מקטע הגדרות [{ $key }]. האם התכוונת ל־[{ $suggestion }]?
config-editor-problem-unknown-key = אין הגדרה { $key }.
config-editor-problem-unknown-key-suggestion = אין הגדרה { $key }. האם התכוונת ל־{ $suggestion }?
config-editor-problem-invalid = ערך לא תקין עבור { $key }: { $reason }
config-editor-problem-migrated = { $key } כתוב כפי שגרסה ישנה של { -app-name } כתבה אותו. הוא מומר בעת הקריאה.
config-editor-problem-newer-version = הקובץ נכתב בגרסה חדשה יותר של { -app-name } (תבנית { $found }; גרסה זו קוראת { $supported }). הגדרות לא מוכרות נשמרות.
config-editor-problem-command = פקודה { $index } ב־[[commands]]: { $reason }
config-editor-problem-hook = הוק { $index } ב־[[hooks]]: { $reason }

## keymap.toml in the same editor (machine-drafted, needs native review)

config-editor-keymap-title = keymap.toml
config-editor-keymap-text = קובץ קיצורי המקלדת
config-editor-keymap-saved = keymap.toml נשמר והוחל.
config-editor-keymap-save-failed = לא ניתן לשמור את keymap.toml: { $reason }
config-editor-keymap-conflict = keymap.toml השתנה במקום אחר.
config-editor-keymap-discard-question = לבטל את השינויים שלא נשמרו ב־keymap.toml?
config-editor-new-file = קובץ חדש, ייווצר בעת השמירה
config-editor-no-folder = אין תיקיית הגדרות לשמור אותו בה.

## Reset all settings confirmation (machine-drafted, needs native review)

settings-reset-all-confirm = לאפס כל הגדרה לברירת המחדל שלה?
settings-reset-all-confirm-detail = הפעולה משפיעה על כל אזורי ההגדרות. אפשר לבטל אותה אחר כך.
settings-reset-all-confirm-ok = איפוס הכול
settings-reset-all-done = כל ההגדרות אופסו לברירות המחדל.

## Hardware video encoding (machine-drafted, needs native review)
paths-engine-hw-encoder = מקודד חומרה

## Path timing units (machine-drafted, needs native review)
settings-paths-checking-before-use = לפני השימוש, { $engine } נבדק מול ה־CPU.
settings-paths-unit-thumbnail = הזמנים הם לתמונה ממוזערת אחת בגודל { $size }
settings-paths-unit-frame = הזמנים הם לכל פריים וידאו בגודל { $size }
# machine-drafted, needs native review
settings-metadata-develop-calibration = תיקיית תמונות כיול
# machine-drafted, needs native review
settings-metadata-develop-calibration-help = תמונות flat ו-dark לפיתוח. שמות הקבצים או התיקיות מכילים "flat" או "dark".

## Colour swatches under a colour setting's field (machine-drafted, needs native review)
settings-colour-white = לבן
settings-colour-light-grey = אפור בהיר
settings-colour-grey = אפור
settings-colour-dark-grey = אפור כהה
settings-colour-black = שחור
settings-colour-yellow = צהוב
settings-colour-orange = כתום
settings-colour-red = אדום
settings-colour-magenta = מג׳נטה
settings-colour-blue = כחול
settings-colour-cyan = ציאן
settings-colour-green = ירוק
