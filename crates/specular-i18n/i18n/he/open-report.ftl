# machine-drafted, needs native review
# Specular — דוח פתיחה: מדוע פתיחת התמונה ארכה כפי שארכה (חלונית המידע, מקטע „ביצועים”;
# DESIGN.md §12.7).

open-report-title = ביצועים

## Headlines
open-report-fast = נפתחה תוך { $ms } ms
open-report-display-cache = הוצגה ממטמון התצוגה
open-report-slow-read = אחסון איטי: { $percent } מהזמן הלכו על קריאת הקובץ ({ $speed } MB/s)
# machine-drafted, needs native review
open-report-slow-read-no-speed = אחסון איטי: { $percent } מהזמן הלכו על קריאת הקובץ (קובץ קטן: בעיקר המתנה לתגובת האחסון)
open-report-archive-read = { $percent } מהזמן הלכו על פריסת הארכיון
open-report-startup = { $percent } מהזמן הלכו על הפעלת { -app-name }
open-report-checked = פוענחה פעמיים: התוצאה הראשונה של מפענח החומרה הושוותה לזו של ה־CPU ({ $percent } מהזמן)
open-report-hardware = פוענחה במפענח החומרה, שלקח { $percent } מהזמן
open-report-decode = תמונה גדולה: פענוח { $mpx } מגה־פיקסל לקח { $percent } מהזמן
open-report-upload = { $percent } מהזמן הלכו על שליחת התמונה ל־GPU
open-report-waiting = { $percent } מהזמן הלכו על המתנה לעבודה אחרת
open-report-stage = { $stage } לקח { $percent } מהזמן
open-report-memory-short = הזיכרון היה דל, ולכן ייתכן שהמערכת נאלצה להשתמש בהחלפה

## Why a decode ran on the processor although a hardware decoder can do it
open-report-sw-not-probed = פוענחה בתוכנה: מפענח החומרה עדיין זוהה
open-report-sw-no-device = פוענחה בתוכנה: { $api } אינו זמין במחשב זה
open-report-sw-no-codec = פוענחה בתוכנה: ל־{ $api } ב־GPU הזה אין מפענח { $codec }
open-report-sw-blocked-mismatch = פוענחה בתוכנה: מפענח החומרה הושבת כי התוצאה שלו לא תאמה את זו של ה־CPU
open-report-sw-blocked-error = פוענחה בתוכנה: מפענח החומרה הושבת לאחר שגיאה
open-report-sw-blocked-lost = פוענחה בתוכנה: מפענח החומרה הושבת כי הפסיק להגיב
open-report-sw-failed = פוענחה בתוכנה: מפענח החומרה נכשל בתמונה זו
open-report-sw-busy = פוענחה בתוכנה: מפענח החומרה היה עסוק
open-report-sw-unsupported = פוענחה בתוכנה: מפענח החומרה אינו יכול לטפל בתמונה זו
open-report-sw-unavailable = פוענחה בתוכנה: פענוח בחומרה כבוי עד ש־{ -app-name } יופעל מחדש
open-report-sw-cpu-only = פוענחה בתוכנה: העיבוד מוגדר ל„CPU בלבד”
open-report-sw-setting-off = פוענחה בתוכנה: פענוח בחומרה כבוי בהגדרות
open-report-sw-unchecked = פוענחה בתוכנה: מפענח החומרה עדיין לא נבדק (הוא נוסה תחילה על תמונה שמוכנה ברקע)
open-report-sw-lower-power = פוענחה בתוכנה: בעבודה על סוללה ה־CPU צורך לכך פחות חשמל
open-report-sw-faster = פוענחה בתוכנה: מהיר יותר במחשב זה (כ־{ $cpu } ms, לעומת { $hw } ms במפענח החומרה)
open-report-sw-scaled = פוענחה בתוכנה: מפענח החומרה מפענח תמונות רק בגודל מלא

## Hints
open-report-hint-copy-local = העתיקו את הקובץ לכונן מקומי כדי לפתוח אותו מהר יותר.
open-report-hint-extract = חלצו את הארכיון לתיקייה כדי לפתוח את דפיו מהר יותר.
open-report-hint-try-again = כדי לנסות שוב את מפענח החומרה: הגדרות ← ביצועים ← לנסות שוב.
open-report-hint-automatic = הגדירו את „היכן מעובדות התמונות” לאוטומטי בהגדרות ← ביצועים.
open-report-hint-turn-on = הגדירו את „פענוח בחומרה” לאוטומטי בהגדרות ← ביצועים.
open-report-hint-free-memory = סגרו תוכניות אחרות כדי לפנות זיכרון.
open-report-hint-once-only = רק התמונה הראשונה נבדקת כך.

## Rows
open-report-total = עד לחדות
open-report-first-pixels = פיקסלים ראשונים
# machine-drafted, needs native review
open-report-whole = התמונה כולה מוצגת
open-report-stage-startup = הפעלה
open-report-stage-read = קריאת הקובץ
open-report-stage-parse = קריאת הכותרת
open-report-stage-colour = הכנת ניהול צבע
open-report-stage-decode = פענוח
open-report-stage-upload = שליחה ל־GPU
open-report-stage-waiting = המתנה לעבודה אחרת
open-report-decoder = פוענח על ידי
open-report-decoder-checked = { $engine }, נבדק מול ה־CPU
open-report-decoder-cache = מטמון תצוגה
# $scale is the reduction: 4 for a level a quarter of the full width and height.
open-report-decoder-scaled = { $engine }, בגודל 1/{ $scale }
open-report-decoder-scaled-full = { $engine }, בגודל מלא, ואז הוקטנה ל־1/{ $scale }
open-report-decoder-scaled-no-index = { $engine }, בגודל 1/{ $scale }, ללא אינדקס ההתחלה המהירה
open-report-colour-conversion = הצבעים הומרו על ידי

## Advisor: a pattern across recent opens, what would help and by how much (rows)
# $count and $total are counts of opens, $format a file format ("HEIC"), $speed a formatted
# number (MB/s), $low and $high formatted numbers ("2.5").
open-report-advice-recurring = חוזר על עצמו
open-report-advice-change = מה יעזור
open-report-advice-gain = שיפור משוער
open-report-advice-software = { $count } מתוך { $total } התמונות האיטיות האחרונות בפורמט { $format } { $count ->
    [one] פוענחה
    [two] פוענחו
   *[other] פוענחו
} בתוכנה מסיבה זו
open-report-advice-slow-storage = { $count } מתוך { $total } התמונות האיטיות האחרונות { $count ->
    [one] התעכבה
    [two] התעכבו
   *[other] התעכבו
} בקריאת הקובץ (כ־{ $speed } MB/s)
# machine-drafted, needs native review
open-report-advice-slow-storage-no-speed = { $count } מתוך { $total } התמונות האיטיות האחרונות { $count ->
    [one] התעכבה
    [two] התעכבו
   *[other] התעכבו
} בקריאת הקובץ (קבצים קטנים: בעיקר המתנה לתגובת האחסון)
open-report-advice-archive = { $count } מתוך { $total } התמונות האיטיות האחרונות { $count ->
    [one] התעכבה
    [two] התעכבו
   *[other] התעכבו
} בפריסת הארכיון
open-report-advice-hardware-decoder = GPU עם מפענח חומרה ל־{ $codec }
open-report-advice-gain-range = מהיר פי { $low } עד { $high } בערך
open-report-advice-gain-one = מהיר פי { $low } בערך

## Performance insights: the indicator in the corner and the panel it opens
# machine-drafted, needs native review
# $total is a formatted duration ("240 ms"), $format a file format ("HEIC").
open-report-insights-title = תובנות ביצועים
open-report-insights-indicator = למה זה איטי?
open-report-insights-announce = תובנת ביצועים: תמונות נפתחות שוב ושוב לאט מאותה סיבה. פתחו את „למה זה איטי?” כדי לראות מה יעזור.
open-report-insights-hide = הסתרה
open-report-insights-last = התמונה האיטית האחרונה: { $total } עד לחדות
open-report-insights-now = מה אפשר לעשות עכשיו
open-report-insights-hardware = חומרה שתעזור
open-report-insights-none = אין האטה חוזרת
open-report-insights-none-detail = תובנות מופיעות כשכמה תמונות נפתחות לאט מאותה סיבה. כדי למצוא אותן, { -app-name } שומר שבוע של פתיחות איטיות במחשב זה; שום דבר לא נשלח לשום מקום.
open-report-insights-software = תמונות { $format } מפוענחות ב־CPU
open-report-insights-storage = קריאת הקבצים מעכבת את התמונות
open-report-insights-archive = פריסת הארכיונים מעכבת את התמונות
open-report-insights-nothing-now = אף הגדרה ב־{ -app-name } לא משנה זאת.
open-report-insights-display-cache = הפעילו את מטמון התצוגה: תמונות שכבר פתחתם ייפתחו אז מעותק מהיר במחשב זה.
open-report-insights-prefetch = החזיקו יותר תמונות סמוכות מוכנות, כך שהבאות יוכנו בזמן שאתם צופים.
open-report-insights-hw-ssd = כונן פנימי מהיר (NVMe SSD) לקבצים אלה
open-report-insights-action-turn-on = הפעלה
open-report-insights-action-try-again = לנסות שוב
open-report-insights-action-automatic = הגדרה לאוטומטי
open-report-insights-dismiss-storage = לא להציג שוב עבור אחסון
open-report-insights-dismiss-decoder = לא להציג שוב עבור פענוח
open-report-insights-off = כיבוי התובנות
open-report-insights-on = הפעלת התובנות
open-report-insights-restore = הצגת התובנות המוסתרות שוב
open-report-insights-power = המחשב הזה חוסך בחשמל (עובד על סוללה או במצב חיסכון בחשמל), וזה מאט את הפענוח ב־CPU. כשהוא מחובר לחשמל, תמונות עשויות להיפתח מהר יותר.

## Performance insights in the gallery: the same, about making thumbnails
# machine-drafted, needs native review
# $count and $total are counts of thumbnails, $total in -last-thumbnail a formatted duration
# ("240 ms"), $format a file format ("HEIC"), $speed a formatted number (MB/s).
open-report-insights-announce-thumbnails = תובנת ביצועים: תמונות ממוזערות מופיעות שוב ושוב לאט מאותה סיבה. פתחו את „למה זה איטי?” כדי לראות מה יעזור.
open-report-insights-last-thumbnail = התמונה הממוזערת האיטית האחרונה: { $total } ליצירה
open-report-insights-thumbnails-software = תמונות ממוזערות של תמונות { $format } מפוענחות ב־CPU
open-report-insights-thumbnails-storage = קריאת הקבצים מעכבת את התמונות הממוזערות
open-report-insights-thumbnails-archive = פריסת הארכיונים מעכבת את התמונות הממוזערות
open-report-insights-thumbnail-cache = לא ניתן לשמור תמונות ממוזערות במחשב זה (מטמון התמונות הממוזערות אינו זמין), ולכן הן נוצרות מחדש בכל פעם שהתיקייה מוצגת.
open-report-advice-thumbnails-software = { $count } מתוך { $total } התמונות הממוזערות האיטיות האחרונות בפורמט { $format } { $count ->
    [one] פוענחה
    [two] פוענחו
   *[other] פוענחו
} בתוכנה מסיבה זו
open-report-advice-thumbnails-slow-storage = { $count } מתוך { $total } התמונות הממוזערות האיטיות האחרונות { $count ->
    [one] התעכבה
    [two] התעכבו
   *[other] התעכבו
} בקריאת הקובץ (כ־{ $speed } MB/s)
# machine-drafted, needs native review
open-report-advice-thumbnails-slow-storage-no-speed = { $count } מתוך { $total } התמונות הממוזערות האיטיות האחרונות { $count ->
    [one] התעכבה
    [two] התעכבו
   *[other] התעכבו
} בקריאת הקובץ (קבצים קטנים: בעיקר המתנה לתגובת האחסון)
open-report-advice-thumbnails-archive = { $count } מתוך { $total } התמונות הממוזערות האיטיות האחרונות { $count ->
    [one] התעכבה
    [two] התעכבו
   *[other] התעכבו
} בפריסת הארכיון
