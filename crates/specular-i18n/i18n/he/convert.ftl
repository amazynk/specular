# Specular — Batch convert (DESIGN.md §6.5). machine-drafted, needs native review
# Every output is a new file; the sources are never changed.

palette-convert = המרה…

## Settings → Convert
settings-section-convert = המרה
settings-convert-format = פורמט
settings-convert-format-help = הפורמט שבו “המרה…” כותבת. PNG ו־TIFF הם ללא אובדן; WebP ו־JPEG XL יכולים להיות עם אובדן או בלעדיו.
settings-convert-format-jpeg = JPEG
settings-convert-format-png = PNG
settings-convert-format-webp = WebP
settings-convert-format-jxl = JPEG XL
settings-convert-format-tiff = TIFF
settings-convert-jpeg-quality = איכות JPEG
settings-convert-jpeg-quality-help = ערך גבוה יותר שומר יותר פרטים בקבצים גדולים יותר. ב־90 קשה להבחין בהבדל מהמקור.
settings-convert-jpeg-full-chroma = צבע ברזולוציה מלאה ב־JPEG
settings-convert-jpeg-full-chroma-help = שומר את הצבע ברזולוציה מלאה (4:4:4) במקום חצי (4:2:0): קצוות צבע חדים יותר, קבצים גדולים יותר.
settings-convert-png-effort = דחיסת PNG
settings-convert-png-effort-help = ערך גבוה יותר יוצר קבצים קטנים יותר, לאט יותר. 1 הוא המהיר ביותר.
settings-convert-jxl-effort = מאמץ JPEG XL
settings-convert-jxl-effort-help = ערך גבוה יותר יוצר קבצים קטנים יותר, לאט יותר.
settings-convert-tiff-compression = דחיסת TIFF
settings-convert-tiff-compression-help = Deflate היא דחיסה ללא אובדן שכל קורא TIFF קורא.
settings-convert-tiff-compression-deflate = Deflate
settings-convert-tiff-compression-none = ללא
settings-convert-depth = עומק סיביות
settings-convert-depth-help = “אוטומטי” כותב 16 סיביות לדגימה כשלתמונה יש יותר מ־8 והפורמט תומך בהן (PNG,‏ TIFF,‏ JPEG XL).
settings-convert-depth-auto = אוטומטי
settings-convert-depth-8 = 8 סיביות
settings-convert-depth-16 = 16 סיביות
settings-convert-resize = שינוי גודל
settings-convert-resize-help = התמונות מוקטנות באור ליניארי בעזרת מסנן איכותי.
settings-convert-resize-none = גודל מקורי
settings-convert-resize-fit = התאמה לתוך
settings-convert-resize-long-edge = הצלע הארוכה
settings-convert-resize-percent = אחוזים
settings-convert-width = רוחב
settings-convert-width-help = הרוחב המרבי שתמונה יכולה לקבל עם “התאמה לתוך”.
settings-convert-height = גובה
settings-convert-height-help = הגובה המרבי שתמונה יכולה לקבל עם “התאמה לתוך”.
settings-convert-long-edge = הצלע הארוכה
settings-convert-long-edge-help = אורך הצלע הארוכה של התמונה עם “הצלע הארוכה”.
settings-convert-percent = אחוזים
settings-convert-percent-help = הגודל ביחס למקור עם “אחוזים”.
settings-convert-upscale = הגדלת תמונות קטנות
settings-convert-upscale-help = כשהאפשרות כבויה, תמונות שכבר קטנות מהגודל המבוקש שומרות על גודלן.
settings-convert-colour = צבע
settings-convert-colour-help = “שמירת הפרופיל” שומרת את פרופיל הצבע של התמונה יחד איתה. “המרה ל־sRGB” מתאימה לרשת ולמכשירים ללא ניהול צבע.
settings-convert-colour-keep = שמירת הפרופיל
settings-convert-colour-srgb = המרה ל־sRGB
settings-convert-hdr = תמונות HDR
settings-convert-hdr-help = ‏PNG ו־JPEG XL שומרים על HDR כמות שהוא; בפורמטים האחרים תמיד מתבצע מיפוי גוונים ל־SDR.
settings-convert-hdr-keep = שמירה כשאפשר
settings-convert-hdr-tone-map = מיפוי גוונים ל־SDR
settings-convert-metadata = מטא־נתונים
settings-convert-metadata-help = ‏Exif ו־XMP, כמו המצלמה, תאריך הצילום והמיקום. פרופיל הצבע נשמר תמיד.
settings-convert-metadata-all = שמירת הכול
settings-convert-metadata-no-location = הכול מלבד המיקום
settings-convert-metadata-strip = הסרה
settings-convert-folder = תיקיית פלט
settings-convert-folder-help = לאן הקבצים המומרים הולכים. אם לא הוגדרה, הם הולכים לתיקיית משנה ליד כל תמונה.
settings-convert-folder-unset = תיקיית משנה ליד התמונות
settings-convert-subfolder = שם תיקיית המשנה
settings-convert-subfolder-help = תיקיית המשנה שבה משתמשים כשלא הוגדרה תיקיית פלט. ריק: התיקייה של התמונות עצמן.
settings-convert-pattern = שמות קבצים
settings-convert-pattern-help = ‏{"{"}name{"}"} הוא השם המקורי, {"{"}n:03{"}"} המספר באצווה, {"{"}date:%Y-%m-%d{"}"} תאריך הצילום, {"{"}w{"}"} ו־{"{"}h{"}"} הגודל החדש, {"{"}ext{"}"} הסיומת של הפורמט.
settings-convert-conflicts = כששם כבר תפוס
settings-convert-conflicts-help = קבצים קיימים לעולם אינם מוחלפים.
settings-convert-conflicts-suffix = הוספת מספר
settings-convert-conflicts-skip = דילוג על התמונה

## The dialog
convert-title = המרה
convert-images = תמונות
convert-images-current = התמונה הזו
convert-images-selection = בחירה ({ $count })
convert-images-folder = תיקייה ({ $count })
convert-images-picked = נבחרות ({ $count })
convert-quality = איכות
convert-chroma = פירוט צבע
convert-chroma-half = 4:2:0 (קטן יותר)
convert-chroma-full = 4:4:4 (חד יותר)
convert-effort = מאמץ
convert-on = מופעל
convert-off = כבוי
convert-pixels = { $value } פיקסלים
convert-effort-fastest = { $value } (המהיר ביותר)
convert-folder-subfolder = תיקיית המשנה „{ $name }”
convert-folder-same = אותה תיקייה כמו התמונה
convert-choose-folder = בחירת תיקייה…
convert-name = שם
convert-name-preview = { $source } → { $output }
convert-name-error-unclosed = יש {"{"} שלא נסגר.
convert-name-error-stray = ל־{"}"} אין {"{"} תואם. כתבו {"}}"} כדי לקבל סוגר מסולסל.
convert-name-error-unknown = { $token } אינו חלק משם. השתמשו ב־name,‏ n,‏ date,‏ w,‏ h או ext.
convert-name-error-format = ל־{ $token } יש עיצוב שגוי.
convert-name-error-empty = הזינו תבנית שם.
convert-start = המרה
convert-stop = עצירה
convert-hide = הסתרה
convert-close = סגירה
convert-progress = ממיר { $done } מתוך { $total }
convert-progress-eta = ממיר { $done } מתוך { $total } · נותרו כ־{ $time }
convert-eta-seconds = { $count ->
    [one] שנייה אחת
    [two] שתי שניות
   *[other] { $count } שניות
}
convert-eta-minutes = { $count ->
    [one] דקה אחת
    [two] שתי דקות
   *[other] { $count } דקות
}
convert-done = { $count ->
    [one] תמונה אחת הומרה
    [two] שתי תמונות הומרו
   *[other] { $count } תמונות הומרו
}
convert-done-skipped = דולגו כי השם היה תפוס: { $count }
convert-done-failed = נכשלו: { $count }
convert-stopped = נעצר אחרי { $count }
convert-failure = { $file }: { $reason }
convert-failure-open = לא ניתן לפתוח ({ $error })
convert-failure-decode = לא ניתן לקרוא ({ $error })
convert-failure-encode = לא ניתן לכתוב בפורמט זה ({ $error })
convert-failure-write = לא ניתן לשמור ({ $error })
convert-failure-same = הקובץ החדש היה מחליף את המקור
convert-report-more = ועוד { $count }
convert-running = המרה כבר מתבצעת.

## Convert encoders (machine-drafted, needs native review)
settings-convert-format-avif = AVIF
settings-convert-webp-lossless = ‏WebP ללא אובדן
settings-convert-webp-lossless-help = שומר על כל פיקסל בדיוק. כשהאפשרות כבויה, קובצי WebP קטנים בהרבה באיכות WebP שנבחרה.
settings-convert-webp-quality = איכות WebP
settings-convert-webp-quality-help = ערך גבוה יותר שומר יותר פרטים בקבצים גדולים יותר. בשימוש כאשר WebP אינו ללא אובדן.
settings-convert-webp-effort = מאמץ WebP
settings-convert-webp-effort-help = ערך גבוה יותר יוצר קבצים קטנים יותר, לאט יותר. בשימוש כאשר WebP אינו ללא אובדן.
settings-convert-jxl-lossless = ‏JPEG XL ללא אובדן
settings-convert-jxl-lossless-help = שומר על כל פיקסל בדיוק. כשהאפשרות כבויה, קובצי JPEG XL קטנים בהרבה באיכות JPEG XL שנבחרה.
settings-convert-jxl-quality = איכות JPEG XL
settings-convert-jxl-quality-help = ערך גבוה יותר שומר יותר פרטים בקבצים גדולים יותר. ב־90 קשה להבחין בהבדל מהמקור. בשימוש כאשר JPEG XL אינו ללא אובדן.
settings-convert-avif-quality = איכות AVIF
settings-convert-avif-quality-help = ערך גבוה יותר שומר יותר פרטים בקבצים גדולים יותר.
settings-convert-avif-effort = מאמץ AVIF
settings-convert-avif-effort-help = ערך גבוה יותר יוצר קבצים קטנים יותר, לאט יותר. 1 הוא המהיר ביותר.
convert-lossless = ללא אובדן
