# machine-drafted, needs native review
# Specular — הגלריה (תצוגת תמונות ממוזערות, DESIGN §10) והגדרות ← גלריה. עברית.

## לוח הפקודות ותפריטים

palette-gallery = גלריה
gallery-palette-view = תצוגת הגלריה…
gallery-palette-layout = פריסה
gallery-palette-sort = מיון לפי
gallery-palette-group = קיבוץ לפי
gallery-palette-filter = סינון…
gallery-palette-quick-filters = מסננים מהירים
gallery-palette-details = תצוגת פרטים
gallery-palette-info-overlay = מידע על התמונות הממוזערות
gallery-palette-stacks = ערימות RAW+JPEG וצילומי רצף
gallery-palette-subfolders = כולל תיקיות משנה
gallery-palette-compare = השוואת הנבחרות
gallery-palette-rate = דירוג…
gallery-palette-settings = הגדרות הגלריה…
gallery-view-menu = תצוגה
gallery-more-menu = עוד
gallery-open = פתיחה
gallery-new-window = פתיחה בחלון חדש
gallery-reveal = הצגה בתיקייה
gallery-properties = מאפיינים
gallery-copy-paths = העתקת נתיבים
gallery-select-all = בחירת הכול
gallery-select-none = ביטול הבחירה
gallery-select-invert = היפוך הבחירה
gallery-next-unrated = הבאה ללא דירוג
gallery-stack-toggle = פתיחה או סגירה של ערימה
gallery-run-on = { $name } (קבצים: { $count })
gallery-depth-off = התיקייה הזאת בלבד
gallery-depth-all = כל תיקיות המשנה
gallery-depth-levels = רמות של תיקיות משנה: { $depth }

## מצב ומסכים ריקים

gallery-items = { $count ->
    [one] פריט אחד
    [two] שני פריטים
   *[other] { $count } פריטים
}
gallery-selected = { $count } מתוך { $total } נבחרו
gallery-shown-of = מוצגים { $count } מתוך { $total }
gallery-listing = קורא את התיקייה… { $count }
gallery-empty = אין תמונות בתיקייה הזאת.
gallery-empty-filter = אין תמונות שתואמות למסנן.
gallery-list-error = לא ניתן לקרוא את התיקייה: { $reason }
gallery-no-folder = לא נפתחה תיקייה
gallery-kind-folder = תיקייה
gallery-drag-count = קבצים: { $count }
gallery-badge-pages = עמודים: { $count }

## סרגל הסינון

# machine-drafted, needs native review
# The filter field takes words (matched against names) and field:value terms; the field
# names and words of the query (rating, camera, ext, is:pick, …) stay in English.
gallery-filter-query-placeholder = שם, או rating>=3 camera:X-T5 …
gallery-filter-clear = ניקוי המסנן
gallery-filter-rating = ★ { $stars } ומעלה
gallery-filter-label = תווית: { $label }
gallery-filter-picks = נבחרות
gallery-filter-hide-rejects = הסתרת הנדחות
gallery-filter-unrated = ללא דירוג
gallery-filter-raw = RAW
gallery-filter-animated = מונפשות
gallery-filter-hdr = HDR
gallery-filter-landscape = לרוחב
gallery-filter-portrait = לאורך

## Query errors and facets

gallery-query-unknown-field = שדה לא מוכר „{ $field }”
gallery-query-unknown-field-suggest = שדה לא מוכר „{ $field }”: האם התכוונת ל„{ $suggestion }”?
gallery-query-missing-value = „{ $field }” צריך ערך
gallery-query-bad-number = „{ $value }” אינו מספר ({ $field })
gallery-query-bad-date = „{ $value }” אינו תאריך ({ $field }): יש להשתמש ב־2024, 2024-05 או 2024-05-12
gallery-query-bad-word = „{ $value }” אינו מתאים ל־{ $field }: יש להשתמש ב־{ $words }
# machine-drafted, needs native review
gallery-query-bad-exif-tag = „{ $value }” אינו שם של תג Exif
gallery-query-no-compare = לא ניתן להשוות את { $field } כאן עם < או >
gallery-query-unterminated = חסר מירכאות סוגרות
gallery-query-unavailable = „{ $field }” דורש את הספרייה (לא זמין כאן)
gallery-facets = היבטים
gallery-facets-counting = סופר…
gallery-facets-empty = אין מה לספור
gallery-facets-search = חיפוש בהיבטים
gallery-facets-no-match = אין ערכים תואמים
# The gallery's top bar button (its tooltip): back to the start screen.
gallery-home = מסך הפתיחה
gallery-facet-format = תבנית
gallery-facet-camera = מצלמה
gallery-facet-lens = עדשה
gallery-facet-rating = דירוג
gallery-facet-year = שנה
gallery-facet-orientation = כיוון
gallery-facet-label = תווית
gallery-facet-properties = מאפיינים
gallery-facet-square = ריבועי
gallery-facet-alpha = שקיפות
gallery-facet-gps = מיקום
gallery-facet-multipage = עמודים מרובים
# Screen readers: a facet value, e.g. “Camera: X-T5, 12 files”.
gallery-facet-a11y-value = { $facet }: { $value }, { $count ->
    [one] קובץ אחד
    [two] שני קבצים
   *[other] { $count } קבצים
}

## קבוצות

# An event group's header: the first and the last day of a shooting session.
gallery-group-event = { $start } – { $end }
gallery-group-event-place = { $place } · { $dates }
gallery-group-this-folder = התיקייה הזאת
gallery-group-unknown = לא ידוע
gallery-group-unrated = ללא דירוג

## מיון תמונות

gallery-stars = { $stars ->
    [one] כוכב אחד
    [two] { $stars } כוכבים
   *[other] { $stars } כוכבים
}
gallery-rate-none = ללא דירוג
gallery-pick = בחירה
gallery-reject = דחייה
gallery-unflag = ללא סימון
gallery-label-red = אדום
gallery-label-yellow = צהוב
gallery-label-green = ירוק
gallery-label-blue = כחול
gallery-label-none = ללא תווית
gallery-compare-hint = יש לבחור 2 עד 4 תמונות כדי להשוות ביניהן.

## קבצים

gallery-trash-count = העברת קבצים לאשפה: { $count }
gallery-moved-to = קבצים שהועברו אל { $folder }: { $count }
gallery-copied-to = קבצים שהועתקו אל { $folder }: { $count }
gallery-move-failed = לא ניתן להעביר את { $file }: { $reason }
gallery-drop-move = העברה אל { $folder }
gallery-drop-copy = העתקה אל { $folder }
gallery-move-here = העברת הפריטים שנבחרו לכאן
gallery-copy-here = העתקת הפריטים שנבחרו לכאן
gallery-not-an-image = { $file } אינו תמונה ש־{ -app-name } יכולה לפתוח.
gallery-cache-cleared = מטמון התמונות הממוזערות נוקה
# machine-drafted, needs native review
gallery-thumb-failed = אין תמונה ממוזערת: { $reason }
gallery-cache-clear-failed = לא ניתן לנקות את מטמון התמונות הממוזערות: { $reason }

## מאפיינים

gallery-prop-name = שם
gallery-prop-folder = תיקייה
gallery-prop-size = גודל הקובץ
gallery-prop-dimensions = ממדים
gallery-prop-format = תבנית
gallery-prop-date = תאריך
gallery-prop-camera = מצלמה
gallery-prop-lens = עדשה
gallery-prop-rating = דירוג
gallery-prop-label = תווית
gallery-prop-items = פריטים

## קוראי מסך

gallery-a11y-list = { $folder }, פריטים: { $count }
gallery-a11y-dims = { $width } על { $height }
gallery-a11y-folder = תיקייה
gallery-a11y-up = למעלה אל { $folder }
gallery-a11y-archive = ארכיון
gallery-a11y-stack = ערימה של { $count }
gallery-a11y-filter-bar = מסנן
gallery-a11y-filter-field = סינון לפי שם או שדה
gallery-a11y-chip = תנאי סינון { $term }
gallery-a11y-chip-remove = יש ללחוץ כדי להסיר
gallery-a11y-results = { $count ->
    [0] אין פריטים תואמים
    [one] פריט אחד תואם
    [two] שני פריטים תואמים
   *[other] { $count } פריטים תואמים
}

## הגדרות ← גלריה

settings-section-gallery = גלריה
settings-gallery-layout = פריסה
settings-gallery-layout-help = ריבועים חותכים את התמונות לרשת; שורות מיושרות שומרות על צורת כל תמונה; ללא חיתוך מציג תמונות שלמות בתאים שווים.
settings-gallery-layout-squares = ריבועים
settings-gallery-layout-justified = שורות מיושרות
settings-gallery-layout-uncropped = ללא חיתוך
settings-gallery-layout-details = פרטים
settings-gallery-thumbnail-size = גודל התמונות הממוזערות
settings-gallery-thumbnail-size-help = בגלריה אפשר גם עם Ctrl+גלגלת העכבר, צביטה או Ctrl+פלוס ומינוס.
settings-gallery-captions = הצגת שמות קבצים
settings-gallery-captions-help = כיתוב מתחת לכל תמונה ממוזערת.
settings-gallery-badges = הצגת תגים
settings-gallery-badges-help = סימנים קטנים ל־RAW, הנפשה, HDR, ריבוי עמודים, דירוג ותווית.
settings-gallery-sort-order = מיון לפי
settings-gallery-sort-order-help = סדר הגלריה. תיקיות תמיד מופיעות ראשונות.
settings-gallery-sort-order-name = שם
settings-gallery-sort-order-date-taken = תאריך צילום
settings-gallery-sort-order-modified = תאריך שינוי
settings-gallery-sort-order-size = גודל הקובץ
settings-gallery-sort-order-type = סוג הקובץ
settings-gallery-sort-order-dimensions = ממדים
settings-gallery-sort-descending = סדר הפוך
settings-gallery-sort-descending-help = מיון מהאחרון לראשון.
settings-gallery-group-by = קיבוץ לפי
settings-gallery-group-by-help = כותרות מקטעים שנשארות למעלה בזמן הגלילה.
settings-gallery-group-by-none = ללא
settings-gallery-group-by-day = יום
settings-gallery-group-by-month = חודש
settings-gallery-group-by-year = שנה
settings-gallery-group-by-event = אירוע
settings-gallery-group-by-folder = תיקייה
settings-gallery-group-by-camera = מצלמה
settings-gallery-group-by-format = תבנית
settings-gallery-group-by-rating = דירוג
settings-gallery-group-by-target = יעד
settings-gallery-group-by-filter = מסנן
settings-gallery-per-folder-view = זכירת התצוגה לכל תיקייה
settings-gallery-per-folder-view-help = הפריסה, הגודל, המיון והקיבוץ נשמרים לכל תיקייה; אחרת תצוגה אחת משמשת בכל מקום.
settings-gallery-open-folders-in-gallery = פתיחת תיקיות בגלריה
settings-gallery-open-folders-in-gallery-help = בפתיחת תיקייה, להתחיל בתמונות הממוזערות במקום בתמונה הראשונה.
settings-gallery-escape-opens-gallery = Escape פותח את הגלריה
settings-gallery-escape-opens-gallery-help = כשאין מה לסגור, Escape במציג עובר לגלריה.
settings-gallery-wrap-around = מקשי החיצים חוזרים להתחלה
settings-gallery-wrap-around-help = בסוף הרשימה מקשי החיצים עוברים להתחלה, ומההתחלה לסוף. החזקת מקש עדיין עוצרת בסוף.
settings-gallery-show-hidden = הצגת קבצים מוסתרים
settings-gallery-show-hidden-help = כולל קבצים ותיקיות ששמם מתחיל בנקודה.
settings-gallery-show-other-files = הצגת קבצים אחרים
settings-gallery-show-other-files-help = כולל קבצים ש־{ -app-name } לא יכולה לפתוח, עם סמל כללי.
settings-gallery-show-archives = הצגת ארכיונים
settings-gallery-show-archives-help = כולל קומיקס וארכיונים אחרים (zip,‏ cbz,‏ rar,‏ 7z…) כאריחים. פתיחת ארכיון ישירות עדיין עובדת.
settings-gallery-subfolder-depth = רמות תיקיות משנה
settings-gallery-subfolder-depth-help = הצגת תמונות גם מתיקיות משנה, עד מספר הרמות הזה (0: התיקייה הזאת בלבד).
settings-gallery-stacks = ערימת קבצים קשורים
settings-gallery-stacks-help = הצגת זוגות RAW+JPEG וצילומי רצף כאריח אחד. S פותח ערימה.
settings-gallery-info-overlay = מידע על התמונות הממוזערות
settings-gallery-info-overlay-help = ממדים וחשיפה מעל כל תמונה ממוזערת.
settings-gallery-prefer-embedded = העדפת תצוגות מקדימות מוטמעות
settings-gallery-prefer-embedded-help = שימוש בתצוגה המקדימה שבקובץ כשהיא גדולה מספיק. מהיר בהרבה, מעט פחות מדויק.
settings-gallery-sharper-raw-thumbnails = תמונות ממוזערות חדות יותר ל־RAW
settings-gallery-sharper-raw-thumbnails-help = תמונות RAW ערוכות מפותחות ב־1024 פיקסלים עבור התמונות הממוזערות שלהן. חד יותר, מעט איטי יותר. כבוי: 512 פיקסלים.
settings-gallery-rating-sidecars = כתיבת דירוגים לקובצי XMP נלווים
settings-gallery-rating-sidecars-help = דירוגים, תוויות וסימונים נכתבים גם לקובץ ‎.xmp ליד התמונה, שיישומי צילום אחרים קוראים.
settings-gallery-cache-size = גודל מטמון התמונות הממוזערות
settings-gallery-cache-size-help = התמונות הממוזערות הישנות ביותר נמחקות כשהמטמון מלא.
settings-gallery-cache-location = מיקום מטמון התמונות הממוזערות
settings-gallery-cache-location-help = תיקייה בכונן מהיר עדיפה.
settings-gallery-clear-cache = ניקוי מטמון התמונות הממוזערות
settings-gallery-clear-cache-help = מוחק את התמונות הממוזערות השמורות. הן נוצרות מחדש בזמן העיון.

## Compare
# machine-drafted, needs native review

compare-side = זו לצד זו
compare-overlay = שכבת־על
compare-difference = הפרש
compare-flicker = הבהוב
compare-blend = מיזוג
compare-kind-absolute = שגיאה מוחלטת
compare-kind-squared = שגיאה ריבועית
compare-kind-relative = שגיאה יחסית
compare-scale-down = פחות רגיש
compare-scale-up = רגיש יותר
compare-scale = סקלה מלאה { $value }
compare-space = אור ליניארי, כפי שמוצג
compare-space-help = השגיאות והמדדים נמדדים באור ליניארי כפי שהתמונה מוצגת, אחרי מיפוי גוונים, לכל ערוץ כולל אלפא.
compare-metrics = ‏MAE { $mae }  ·  MSE { $mse }  ·  PSNR { $psnr } dB
compare-sampled = נדגם ב־‎{ $width } × { $height }
compare-aligned = מיושר לפינה העליונה השמאלית, הושוו ‎{ $width } × { $height }
compare-vs = השוואת { $b } מול { $a }
compare-overlay-label = ‏{ $b } מעל { $a } ב־{ PERCENT($percent) }
compare-pending = מודד…
# machine-drafted, needs native review
compare-diff-failed = לא ניתן להציג את ההפרש: { $reason }
compare-no-overlap = אין מה להשוות בתצוגה
compare-zoom-fit = התאמה
compare-zoom = ‏{ PERCENT($percent) }
compare-showing = מוצג { $name }
compare-a11y-group = השוואה
compare-a11y-blend = מיזוג בין התמונות
compare-a11y-zoom = זום
gallery-palette-compare-side = השוואה זו לצד זו
gallery-palette-compare-overlay = השוואה כשכבת־על
gallery-palette-compare-difference = השוואת הפרש: { $kind }
gallery-palette-compare-flicker = הבהוב בין התמונות המושוות
gallery-palette-compare-actual = השוואה ב־{ PERCENT(100) }

## Search value list (machine-drafted, needs native review)
gallery-picker-hint = רווח או Ctrl+Enter לבחירת כמה ערכים

## Search value picker, part 2 (machine-drafted, needs native review)
gallery-facet-profile = פרופיל צבע
gallery-facet-date = תאריך צילום
gallery-facet-picked = נבחרות
gallery-facet-rejected = נדחות
gallery-facet-flagged = נבחרות או נדחות
gallery-facet-rated = מדורגות
gallery-facet-labelled = עם תווית

## Search value picker, part 3 (machine-drafted, needs native review)

gallery-facet-bits = עומק סיביות
gallery-facet-pages = עמודים
gallery-facet-width = רוחב
gallery-facet-height = גובה
gallery-facet-megapixels = מגה-פיקסלים
gallery-picker-bits = { $bits } סיביות
gallery-picker-pages = { $pages ->
    [one] עמוד אחד
    [two] שני עמודים
   *[other] { $pages } עמודים
}
gallery-picker-pixels = { $px } פיקסלים
gallery-picker-megapixels = { $mp } MP
gallery-picker-group-any = כל קובץ בקבוצה
gallery-picker-group-keep = הצעה: שמירה
gallery-picker-group-discard = הצעה: השלכה
gallery-picker-ops = השוואה
gallery-picker-op-eq = שווה ל־
gallery-picker-op-ge = לפחות
gallery-picker-op-le = לכל היותר
gallery-picker-ge = לפחות { $value }
gallery-picker-gt = יותר מ־{ $value }
gallery-picker-le = לכל היותר { $value }
gallery-picker-lt = פחות מ־{ $value }

# Map view.
gallery-facet-place = מקום
gallery-palette-map = מפה
map-title = מפה
map-summary = תמונות עם מיקום: { $count }
map-photos = תמונות: { $count }
map-zoom-in = התקרבות
map-zoom-out = התרחקות
map-show-all = הצגת הכול
map-done = סיום
map-credit = נתוני מפה: Natural Earth
# machine-drafted, needs native review
# With a base map's tiles on screen: $credit is the tile source's credit ("© OpenStreetMap
# contributors").
map-credit-tiles = מפה: { $credit } · Natural Earth
# The one-time explanation before Specular downloads map tiles from a server; one idea per
# line. $host is the server's name, $size the cache size ("256 MB").
map-basemap-consent-title = אריחי מפה מקוונים
map-basemap-consent-server = אריחי המפה מגיעים מ־{ $host }.
map-basemap-consent-sees = השרת הזה רואה את כתובת האינטרנט שלך ואת האזורים שבהם צופים.
map-basemap-consent-private = התמונות שלך, השמות שלהן והמיקומים שלהן אינם נשלחים.
map-basemap-consent-cache = האריחים נשמרים במחשב הזה, עד { $size }.
map-basemap-allow = שימוש במפה מקוונת
map-basemap-decline = להישאר עם מפה לא מקוונת
# Why the base map is not shown; $reason is one of the lines below or a file error.
map-basemap-error = מפת הבסיס לא מוצגת: { $reason }
map-basemap-bad-address = כתובת שרת האריחים חייבת לכלול {"{"}z{"}"}, {"{"}x{"}"} ו־{"{"}y{"}"}.
map-basemap-unknown-tiles = סוג האריחים בקובץ המפה אינו ידוע.
map-loading = קורא את מיקומי התמונות…
map-empty = לאף אחת מהתמונות האלה אין מיקום
map-hidden = מיקומי התמונות מוסתרים (הגדרות › מטא־נתונים ופרטיות)
map-a11y-group = מפת מיקומי התמונות
map-a11y-status = מקומות מוצגים: { $count }, רמת זום { $zoom }
