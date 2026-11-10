# machine-drafted, needs native review
# Local copies for offline editing (the Library).

settings-library-local-copies-limit = מגבלת עותקים מקומיים
settings-library-local-copies-limit-help = המקום המרבי שעותקים מקומיים יכולים לתפוס בתיקיית הנתונים של Specular.

local-keep-menu = שמירת עותק מקומי…
local-remove-menu = הסרת העותק המקומי…
local-keep-location-menu = שמירת עותק מקומי של מיקום זה…
local-remove-location-menu = הסרת העותק המקומי של מיקום זה…
local-keep-album-menu = שמירת עותק מקומי של אלבום זה…
local-remove-album-menu = הסרת העותק המקומי של אלבום זה…
local-remove-held-elsewhere = העותקים האלה שייכים למיקום או לאלבום שנשמרו. יש להסיר אותם שם.

local-keep-ask = לשמור עותק מקומי של { $item }?
local-keep-detail = { $count ->
    [one] תמונה אחת.
    [two] שתי תמונות.
   *[other] { $count } תמונות.
}
    מקור: הקבצים כמו שהם.
    פרוקסי: קובצי raw ב־2,560 פיקסלים, מספיק לפיתוח. קבצים אחרים מועתקים במלואם.
    העותקים נשמרים בתיקיית הנתונים של Specular. קובצי המקור לא משתנים.
local-keep-originals = שמירת קובצי המקור
local-keep-proxies = שמירת פרוקסי
local-keeping = מעתיק את { $item }…
local-kept = { $item }: { $count ->
    [one] עותק מקומי אחד
    [two] שני עותקים מקומיים
   *[other] { $count } עותקים מקומיים
}, { $size }.
local-kept-full = { $count ->
    [one] תמונה אחת לא נכנסה במגבלת הגודל.
    [two] שתי תמונות לא נכנסו במגבלת הגודל.
   *[other] { $count } תמונות לא נכנסו במגבלת הגודל.
}
local-kept-unreachable = { $count ->
    [one] לא ניתן היה לקרוא תמונה אחת.
    [two] לא ניתן היה לקרוא שתי תמונות.
   *[other] לא ניתן היה לקרוא { $count } תמונות.
}

local-remove-ask = להסיר את העותק המקומי של { $item }?
local-remove-detail = העותקים בתיקיית הנתונים של Specular נמחקים. לא נוגעים בקובצי המקור. שינויים שממתינים נשמרים.
local-remove-ok = הסרת העותק המקומי
local-removed = העותק המקומי של { $item } הוסר, { $size } התפנו.

local-banner-offline = { $location } לא מקוון. עובדים על עותקים מקומיים.
local-banner-offline-many = { $count } מיקומים לא מקוונים. עובדים על עותקים מקומיים.
local-banner-waiting = { $count ->
    [one] שינוי אחד ממתין לקובץ המקור.
    [two] שני שינויים ממתינים לקובצי המקור.
   *[other] { $count } שינויים ממתינים לקובצי המקור.
}
local-banner-conflicts = { $count ->
    [one] קובץ אחד השתנה בזמן שהיית לא מקוון.
    [two] שני קבצים השתנו בזמן שהיית לא מקוון.
   *[other] { $count } קבצים השתנו בזמן שהיית לא מקוון.
}
local-review = סקירה…
local-sidebar-originals = עותק מקומי
local-sidebar-proxies = פרוקסי מקומי

local-synced = { $count ->
    [one] שינוי אחד נכתב לקובץ המקור שלו.
    [two] שני שינויים נכתבו לקובצי המקור שלהם.
   *[other] { $count } שינויים נכתבו לקובצי המקור שלהם.
}
local-sync-failed = { $count ->
    [one] לא ניתן היה לכתוב שינוי אחד. הוא ממשיך להמתין.
    [two] לא ניתן היה לכתוב שני שינויים. הם ממשיכים להמתין.
   *[other] לא ניתן היה לכתוב { $count } שינויים. הם ממשיכים להמתין.
}

local-conflict-title = { $file } השתנה בזמן שהיית לא מקוון. איזו גרסה לשמור: { $what }?
local-conflict-yours = שלך: { $text }
local-conflict-theirs = בדיסק: { $text }
local-conflict-later = ביטול משאיר את ההחלטה למועד מאוחר יותר. שום דבר לא נדרס.
local-keep-mine = שמירת שלך
local-keep-theirs = שמירת זו של הקובץ
local-what-marks = דירוג
local-what-keywords = מילות מפתח
local-what-fields = שדות IPTC
local-what-edit = עריכה
local-stars = { $count ->
    [0] ללא כוכבים
    [one] כוכב אחד
    [two] שני כוכבים
   *[other] { $count } כוכבים
}
local-label = { $label ->
    [1] אדום
    [2] צהוב
    [3] ירוק
   *[4] כחול
}
local-pick = נבחרה
local-reject = נדחתה
local-no-keywords = אין מילות מפתח
local-changed-elsewhere = השתנה במקום אחר
local-photo-changed = התמונה עצמה השתנתה
local-edit-here = נערכה כאן, { $when }
local-edit-elsewhere = נערכה במקום אחר, { $when }
local-edit-none = ללא עריכה

local-export-ask = { $count ->
    [one] תמונה אחת לא מקוונת. לייצא אותה מהפרוקסי שלה?
    [two] שתי תמונות לא מקוונות. לייצא אותן מהפרוקסי שלהן?
   *[other] { $count } תמונות לא מקוונות. לייצא אותן מהפרוקסי שלהן?
}
local-export-detail = הפרוקסי הם 2,560 פיקסלים בצלע הארוכה. קובצי המקור נותנים את הגודל המלא.
local-export-proxy = ייצוא מפרוקסי
local-export-wait = המתנה לקובצי המקור
local-export-waiting = הייצוא יתחיל כשקובצי המקור יחזרו.

local-panel-title = עותקים מקומיים
local-panel-used = בשימוש { $used } מתוך { $limit }.
local-panel-originals = { $count ->
    [one] קובץ מקור אחד
    [two] שני קובצי מקור
   *[other] { $count } קובצי מקור
}
local-panel-proxies = { $count ->
    [one] תמונה אחת, raw כפרוקסי
    [two] שתי תמונות, raw כפרוקסי
   *[other] { $count } תמונות, raw כפרוקסי
}
local-panel-remove = הסרה…
local-panel-footer = מאוחסן ב־{ $folder }. קובצי המקור לעולם לא משתנים.
local-panel-keywords = לא מקוון עותקים מקומיים פרוקסי מקום
