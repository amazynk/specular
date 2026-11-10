# machine-drafted, needs native review
# Specular — Maps tools: terrain (slope, aspect, contours, 3-D), time, wind and weather colours,
# online map services, photos and geotagging along GPS tracks, and map export.
# עברית.
# Unit and ° come before the number so they show on its right in right-to-left text.

## Cheat sheet groups of the Maps tab.
cheatsheet-map-terrain = פני השטח
cheatsheet-map-time = זמן ומזג אוויר
cheatsheet-map-online = שירותים מקוונים
cheatsheet-map-photos = תמונות ו־GPS
cheatsheet-map-output = פלט

## Palette entries and their one-line descriptions.
palette-map-slope = צבעי שיפוע
palette-map-slope-help = צביעת מודל גבהים לפי התלילות.
palette-map-aspect = צבעי מפנה
palette-map-aspect-help = צביעת מודל גבהים לפי הכיוון שאליו פונים המדרונות.
palette-map-contours = הצגה או הסתרה של קווי גובה
palette-map-contours-help = קווי גובה מעל מודל גבהים, עם תווית כל כמה קווים.
palette-map-terrain-3d = פני השטח בתלת־ממד
palette-map-terrain-3d-help = הצגת מודל הגבהים כמשטח תלת־ממדי עם הצבעים שלו פרושים עליו.
palette-map-time = הצגה או הסתרה של מחוון הזמן
palette-map-time-help = מעבר בין הזמנים של רשת או לאורך הזמנים של מסלול.
palette-map-play = הפעלה או השהיה של הזמן
palette-map-play-help = הרצת מחוון הזמן מעצמו, צעד אחר צעד.
palette-map-wind = הצגה או הסתרה של חיצי רוח
palette-map-wind-help = חיצים של מהירות וכיוון הרוח מרכיבי u ו־v של הרשת.
palette-map-weather-colours = מפת צבעי מזג האוויר הבאה
palette-map-weather-colours-help = מפות צבעים לטמפרטורה, גשם, רוח ולחץ.
palette-map-services = שירותי מפות מקוונים…
palette-map-services-help = הוספת שכבות משרתי WMS,‏ WMTS,‏ WFS,‏ OGC API או STAC.
palette-map-photos-on-track = הצגה או הסתרה של תמונות על המסלול
palette-map-photos-on-track-help = מיקום תמונות התיקייה לאורך מסלול ה־GPS לפי זמן הצילום.
palette-map-geotag = תיוג גאוגרפי של תמונות מהמסלול…
palette-map-geotag-help = כתיבת מיקומי המסלול לקובצי XMP נלווים של תמונות התיקייה.
palette-map-export = ייצוא המפה…
palette-map-export-help = שמירת המפה כ־PNG או PDF בקנה מידה, עם מקרא, סרגל קנה מידה וחץ צפון.
palette-map-print = הדפסת המפה בקנה מידה…
palette-map-print-help = הדפסת המפה בקנה מידה נבחר, כמו 1:{ NUMBER(25000) }.
palette-map-copy-extent = העתקת התחום
palette-map-copy-extent-help = העתקת האזור שעל המסך כתיבה תוחמת, או כ־WKT.

## Why a map command is unavailable.
disabled-no-time = למפה אין זמנים
disabled-no-wind = למפה אין רכיבי רוח (u ו־v)
disabled-no-track = אין מסלול GPS עם זמנים
disabled-not-grid = המפה אינה רשת של ערכים

## Settings › Maps and Metadata.
settings-maps-services = שירותי מפות מקוונים
settings-maps-services-help = הוספת שכבות משרתי WMS,‏ WMTS,‏ WFS,‏ OGC API ו־STAC. כבוי: שום דבר לא מורד.
settings-maps-services-allowed = שרתי מפות מורשים
settings-maps-services-allowed-help = שרתים שהסכמת להם אחרי ההסבר על מה שהם רואים.
settings-metadata-basemap-service = שכבה משירות מפות
settings-metadata-basemap-service-help = השכבה שנבחרה בחלונית השירותים המקוונים של המפה.

## Terrain.
# A compass point, short: n, ne, e, se, s, sw, w, nw.
map-compass = { $point ->
    [ne] צפון־מזרח
    [e] מזרח
    [se] דרום־מזרח
    [s] דרום
    [sw] דרום־מערב
    [w] מערב
    [nw] צפון־מערב
   *[n] צפון
}
map-slope-aspect = שיפוע °{ $slope } · פונה ל{ $direction } (°{ $aspect })
map-slope-flat = שיפוע °{ $slope } · שטוח
map-slope-on = צבעי שיפוע
map-aspect-on = צבעי מפנה
map-relief-off = צבעי גובה
map-contours-on = קווי גובה מוצגים
map-contours-off = קווי גובה מוסתרים
map-legend-slope = שיפוע
map-legend-aspect = מפנה: הכיוון שאליו פונים המדרונות
map-legend-degrees = °{ $degrees }
map-legend-degrees-more = °{ $degrees } ומעלה
map-terrain-3d-on = פני השטח בתלת־ממד. Esc חוזר למפה.
map-terrain-3d-on-exaggerated = פני השטח בתלת־ממד, גבהים × { $times }. Esc חוזר למפה.
map-terrain-exaggeration = הגבהים מוגדלים פי { $times }.
map-terrain-drape = צבעי המפה

## Time, weather and wind.
map-time-grid = { $when } · { $step } מתוך { $steps }
map-time-track = { $when } · { $elapsed } מההתחלה
map-time-speed = × { $times }
map-time-play = הפעלה
map-time-pause = השהיה
map-weather-map = צבעים: { $name }
map-weather-file = צבעים מהקובץ
map-wind-on = חיצי רוח מוצגים
map-wind-off = חיצי רוח מוסתרים
map-wind-at = רוח { $unit } { $speed } מכיוון °{ $degrees } ({ $direction })

## Copy extent.
map-extent-copied = התיבה התוחמת הועתקה. העתקה נוספת נותנת WKT.
map-extent-copied-wkt = התחום הועתק כ־WKT

## Online map services.
map-services-title = שירותי מפות מקוונים
map-service-off = שירותי המפות המקוונים כבויים.
map-service-off-why = יש להפעיל אותם כדי להוסיף שכבות משרתים באינטרנט.
map-service-turn-on = הפעלה
map-service-close = סגירה
map-service-back = חזרה
map-service-cancel = ביטול
map-service-connect = התחברות
map-service-allow = אישור
map-service-remove = הסרה
map-service-download = הורדה
map-service-list-hint = Enter פותח שרת. Delete מסיר אותו.
map-service-add = הוספת שרת…
map-service-add-sub = WMS,‏ WMTS,‏ WFS,‏ OGC API או STAC
map-service-add-hint = כתובת השרת. Enter מתחבר.
map-service-bad-address = זו לא כתובת רשת (http או https, ללא שם משתמש).
map-service-server-sub = { $kind } · { $host }
map-service-consent-title = שירות מפות מקוון
map-service-consent-server = Specular ייצור קשר עם { $host }.
map-service-consent-sees = השרת הזה רואה את כתובת האינטרנט שלך ואת אזור המפה שביקשת.
map-service-consent-private = הקבצים שלך ושמותיהם אינם נשלחים.
map-service-asking = פונה אל { $host }…
map-service-downloading = מוריד מ־{ $host }…
map-service-layers-hint = שכבות { $kind }. Enter משתמש באחת.
map-service-no-layers = השרת אינו מציג שכבות.
map-service-offer-tiles = אריחים: הופכים למפת הבסיס של מפת התמונות
map-service-offer-features = ישויות: נוספות למפה הזו עבור האזור שעל המסך
map-service-offer-search = תמונות: חיפוש באזור שעל המסך
map-service-refused-mercator = לא מוצע ב־Web Mercator (EPSG:3857), שמפת הבסיס צריכה.
map-service-refused-format = לא מוצע כתמונות PNG או JPEG.
map-service-refused-geojson = לא מוצע כ־GeoJSON, התבנית שנקראת כאן.
map-service-items-hint = { $count ->
        [one] נמצאה תמונה אחת. Enter מוריד אותה.
        [two] נמצאו שתי תמונות. Enter מוריד אחת.
       *[other] נמצאו { $count } תמונות. Enter מוריד אחת.
    }
map-service-no-items = לא נמצאו כאן תמונות.
map-service-item-cloud = { $when } · עננות { PERCENT($cloud) }
map-service-item-no-cog = אין Cloud-Optimized GeoTIFF
map-service-download-ask = להוריד { $size } מ־{ $host }?
map-service-download-ask-unknown = להוריד את התמונה הזו מ־{ $host }? הגודל שלה אינו ידוע.
map-service-download-cache = היא נשמרת במטמון המפות ונפתחת.
map-service-size-mb = MB { $size }
map-service-size-gb = GB { $size }
map-service-too-big = גודל התמונה { $size }, גדולה מדי להורדה.
map-service-opened = פותח את התמונה שהורדה
map-service-basemap = { $layer } היא עכשיו מפת הבסיס של מפת התמונות. מפת התמונות שואלת לפני ההורדה הראשונה שלה.
map-service-status = השרת ענה { $status }.
map-service-unknown = זה לא שירות מפות שגרסה זו קוראת.
map-service-exception = השרת אמר: { $message }
map-service-malformed = התשובה של השרת פגומה: { $reason }
map-service-remove-ask = להסיר את “{ $server }” מהשרתים השמורים?
map-service-remove-detail = שום דבר בדיסק לא אובד. שכבות שכבר במפה נשארות עד שהיא נסגרת.
map-service-removed = { $server } הוסר
map-service-saved = שרתי מפות שמורים
map-service-saved-none = אין עדיין. אפשר להוסיף שרת בחלונית השירותים המקוונים של המפה.
map-service-keywords = WMS WMTS WFS OGC API STAC שרתים

## Photos and geotagging.
map-geotag-title = תיוג גאוגרפי של תמונות מהמסלול
map-photos-on = תמונות על המסלול
map-photos-off = התמונות על המסלול מוסתרות
map-geotag-offset = שעון המצלמה: { $offset } מזמן ה־GPS. ← → משנים בדקה, עם Shift בשעה, עם Alt בשנייה.
map-geotag-reading = קורא את התמונות…
map-geotag-summary = { $count } מתוך { $total } תמונות מקבלות מיקום.
map-geotag-sidecars = המיקומים נכתבים לקובצי XMP נלווים ליד התמונות. קובצי התמונות אינם משתנים.
map-geotag-writing = כותב…
map-geotag-write = כתיבת קבצים נלווים…
map-geotag-guess = ניחוש ההפרש
map-geotag-will-write = מקבלת { $position }
map-geotag-has-position = כבר יש מיקום: נשארת כמו שהיא
map-geotag-no-time = אין זמן צילום: נשארת כמו שהיא
map-geotag-outside = צולמה { $offset } מחוץ למסלול: נשארת כמו שהיא
map-geotag-gap = בפער במסלול ({ $seconds } שנ׳ מנקודת מיקום): נשארת כמו שהיא
map-geotag-nothing = אף תמונה לא מקבלת מיקום בהפרש הזה.
map-geotag-confirm = לכתוב מיקומים ל{ $count ->
    [one] קובץ XMP נלווה אחד
    [two] { $count } קובצי XMP נלווים
   *[other] { $count } קובצי XMP נלווים
}? התמונות עצמן אינן משתנות.
map-geotag-confirm-ok = כתיבת קבצים נלווים
map-geotag-done = { $count ->
    [one] נכתב קובץ נלווה אחד
    [two] נכתבו { $count } קבצים נלווים
   *[other] נכתבו { $count } קבצים נלווים
}
map-geotag-done-failed = { $count ->
        [one] נכתב קובץ נלווה אחד
        [two] נכתבו שני קבצים נלווים
       *[other] נכתבו { $count } קבצים נלווים
    }; { $failed } נכשלו: { $reason }
map-geotag-not-sidecar = זה לא שם של קובץ נלווה.

## Export and printing.
map-export-title = ייצוא המפה
map-print-title = הדפסת המפה בקנה מידה
map-export-keys = ↑ ↓ בחירה, ← → שינוי, Enter מבצע.
map-export-format = תבנית
map-export-scale = קנה מידה
map-export-scale-value = 1:{ $n }
map-export-resolution = רזולוציה
map-export-dpi = dpi { $dpi }
map-export-paper = נייר
map-export-size = { $width } × { $height } פיקסלים · { $across } × { $down } ק״מ על הקרקע
map-export-save = שמירה…
map-export-print = הדפסה…
map-export-too-large = גדול מדי בקנה מידה זה: { $width } × { $height } פיקסלים. יש לבחור קנה מידה או רזולוציה קטנים יותר.
map-export-no-scale = למפה זו אין קנה מידה על הקרקע.
map-export-no-gpu = הייצוא דורש את המעבד הגרפי.
map-export-subtitle = קנה מידה { $scale } · { $crs }
map-export-data = נתונים: { $sources }
map-export-saved = { $name } נשמר
map-export-printing = מדפיס…
map-export-printed = נשלח למדפסת
map-export-print-failed = לא הודפס: { $reason }
