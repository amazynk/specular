# machine-drafted, needs native review
# Specular — Maps (the Maps profile): the map view of georeferenced and vector map files, its
# layer panel, attribute table, coordinates, measuring, elevation profile and map furniture.
# English.

keys-mode-map = מפות
# Cheat sheet groups of the Maps tab.
cheatsheet-map-layers = שכבות
cheatsheet-map-coordinates = קואורדינטות
cheatsheet-map-measure = מדידה

## Palette entries and their one-line descriptions.
palette-map-layers = הצגה או הסתרה של חלונית השכבות
palette-map-layers-help = שכבות המפה: סדר, אטימות, נראות וסגנון.
palette-map-add-layer = הוספת שכבה מקובץ…
palette-map-add-layer-help = ציור קובץ מפה נוסף מעל זה, למשל מסלול GPX מעל Shapefile.
palette-map-zoom-to-layer = התקרבות לשכבה
palette-map-zoom-to-layer-help = הצגת כל השכבה שנבחרה בחלונית השכבות.
palette-map-attribute-table = הצגה או הסתרה של טבלת המאפיינים
palette-map-attribute-table-help = הישויות של השכבה שנבחרה כטבלה: מיון, סינון, חיפוש ובחירה.
palette-map-coordinate-format = תבנית הקואורדינטות הבאה
palette-map-coordinate-format-help = מעלות עשרוניות, מעלות דקות שניות, UTM או MGRS.
palette-map-units = יחידות מטריות או אימפריאליות
palette-map-units-help = אורכים ושטחים במטרים, או ברגל ובמיילים.
palette-map-copy-coordinate = העתקת קואורדינטה
palette-map-copy-coordinate-help = העתקת המיקום שמתחת לסמן, או של מרכז המפה.
palette-map-go-to-coordinate = מעבר לקואורדינטה…
palette-map-go-to-coordinate-help = יש להקליד מיקום בכל תבנית כדי למרכז שם את המפה.
palette-map-measure-distance = מדידת מרחק
palette-map-measure-distance-help = יש ללחוץ על נקודות במפה: האורך על פני כדור הארץ והכיוון.
palette-map-measure-area = מדידת שטח
palette-map-measure-area-help = יש ללחוץ על פינות של אזור: גודלו והיקפו על פני כדור הארץ.
palette-map-elevation-profile = חתך גבהים
palette-map-elevation-profile-help = יש לצייר קו מעל מודל גבהים כדי להציג את גבהיו בתרשים.
palette-map-overview = הצגה או הסתרה של מפת הסקירה
palette-map-overview-help = מפה קטנה של כל התחום, עם החלק שעל המסך מסומן.

## Why a map command is unavailable.
disabled-not-map = רק למפות, כשכלי המפות פעילים
disabled-no-lonlat = למפה אין מערכת קואורדינטות ידועה
disabled-no-features = למפה אין ישויות
disabled-no-heights = למפה אין גבהים

## Settings › Maps.
settings-section-maps = מפות
settings-maps-coordinate-format = תבנית קואורדינטות
settings-maps-coordinate-format-help = איך מיקומים מוצגים ומועתקים. מעבר לקואורדינטה קורא כל תבנית.
settings-maps-coordinate-format-dd = מעלות עשרוניות
settings-maps-coordinate-format-dms = מעלות, דקות, שניות
settings-maps-coordinate-format-utm = UTM
settings-maps-coordinate-format-mgrs = MGRS / USNG
settings-maps-units = יחידות
settings-maps-units-help = אורכים ושטחים במפות: סרגל קנה המידה והמדידות. אוטומטי עוקב אחר מטא־נתונים › יחידות.
settings-maps-units-auto = אוטומטי
settings-maps-units-metric = מטריות
settings-maps-units-imperial = אימפריאליות

## The pointer readout. $position is written in the chosen format.
map-position = { $format ->
    [utm] UTM { $position }
    [mgrs] MGRS { $position }
    [dms] { $position }
   *[dd] רוחב, אורך { $position }
}
map-height = גובה { $height }
map-height-label = גובה
# A length or area with its unit symbol ("12.3 km", "4.5 ha").
map-length = { $value } { $unit }
map-scale-label = סרגל קנה מידה: { $length }
map-north-label = חץ צפון: הצפון ב־{ $degrees }° עם כיוון השעון מלמעלה
# The coordinate formats, for the toast after "Next coordinate format".
map-format-name = { $format ->
    [dms] מעלות, דקות, שניות
    [utm] UTM
    [mgrs] MGRS
   *[dd] מעלות עשרוניות
}
map-format-toast = קואורדינטות ב־{ $format }
map-units-metric = יחידות: מטרים וקילומטרים
map-units-imperial = יחידות: רגל ומיילים
toast-copied-coordinate = הקואורדינטה הועתקה

## Go to coordinate.
map-goto-hint = מעלות עשרוניות, DMS, UTM או MGRS. Enter עובר לשם.
map-goto-empty = יש להקליד מיקום.
map-goto-unrecognised = זה לא מיקום בתבנית מוכרת.
map-goto-range = קו הרוחב או קו האורך מחוץ לטווח.
map-goto-no-way = מפה זו לא יכולה למקם מיקום.
map-goto-outside = המיקום הזה מחוץ למפה.
map-goto-toast = מורכז על { $position }

## The layer panel.
map-layers-title = שכבות: { $shown } מתוך { $total } מוצגות
# Under a layer's name: its kind, features and opacity.
map-layer-sub = { $kind } · { $features } · { PERCENT($opacity) }
# A layer's row for screen readers.
map-layer-row = { $name }: { $kind }, { $features ->
    [one] ישות אחת
    [two] { $features } ישויות
   *[other] { $features } ישויות
}, אטימות { PERCENT($opacity) }
map-kind-area = שטחים
map-kind-line = קווים
map-kind-point = נקודות
map-kind-empty = אין ישויות
map-layer-visible = הצגה או הסתרה של השכבה
map-layer-opacity-down = פחות אטום
map-layer-opacity-up = יותר אטום
map-layer-colour = הצבע הבא
map-layer-width = עובי הקו הבא
map-layer-point = גודל הנקודה הבא
map-layer-up = הזזה למעלה
map-layer-down = הזזה למטה
map-layer-remove = הסרה
map-layer-keep = השארה
map-layer-remove-ask = להסיר את { $name } מהמפה? הקובץ נשאר בדיסק.
map-layer-adding = מתבצעת הוספת שכבה…
map-removed = { $name } הוסר מהמפה
map-added = { $layers ->
    [one] { $name } נוסף
    [two] { $name } נוסף: { $layers } שכבות
   *[other] { $name } נוסף: { $layers } שכבות
}
map-add-failed = לא ניתן להוסיף את { $name }: { $reason }
map-add-filter = קובצי מפה
map-add-not-map = זו לא מפה.
map-add-not-vector = אין בו ישויות.
map-add-no-crs = אין לו קובץ היטל (.prj).

## Identify.
map-identify-more = ועוד { $count } בטבלת המאפיינים
map-cell = תא { $x }, { $y }
map-band = ערוץ { $n }
map-no-data = אין נתונים

## The attribute table.
map-table-title = { $layer }: { $shown } מתוך { $total } ישויות, { $selected } נבחרו
map-table-search = חיפוש, או שדה = ערך
map-table-selected-only = רק הנבחרות
map-table-empty = אין ישויות תואמות.
map-table-many = הרבה
map-table-stats-number = { $field }: { $count ->
        [one] ערך אחד
        [two] שני ערכים
       *[other] { $count } ערכים
    }, { $empty } ריקים · מינ׳ { $min } · מקס׳ { $max } · ממוצע { $mean } · סכום { $sum } · סטיית תקן { $sd }
map-table-stats-text = { $field }: { $count ->
        [one] ערך אחד
        [two] שני ערכים
       *[other] { $count } ערכים
    }, { $empty } ריקים · { $distinct } שונים
map-table-stats-text-top = { $field }: { $count ->
        [one] ערך אחד
        [two] שני ערכים
       *[other] { $count } ערכים
    }, { $empty } ריקים · { $distinct } שונים · הנפוץ ביותר { $top } ({ $times }×)

## Measuring and the elevation profile.
map-measure-hint-distance = יש ללחוץ על נקודות לאורך הדרך.
map-measure-hint-area = יש ללחוץ על פינות האזור.
map-measure-hint-profile = יש ללחוץ על נקודות לאורך הקו.
map-measure-keys = Enter מסיים. Backspace מסיר את הנקודה האחרונה. Esc סוגר.
map-measure-distance = מרחק { $length }
map-measure-bearing = כיוון { $bearing }
map-measure-segment = קטע אחרון { $length }, כיוון { $bearing }
map-measure-area = שטח { $area }
map-measure-perimeter = היקף { $length }
map-bearing-value = °{ $degrees }
map-profile-summary = אורך { $length } · הנמוך ביותר { $min } · הגבוה ביותר { $max } · עלייה { $climb } · ירידה { $descent }
map-profile-no-heights = אורך { $length }. אין גבהים לאורך הקו.
map-profile-at = { $distance }: { $height }
map-profile-at-none = { $distance }: אין גובה
