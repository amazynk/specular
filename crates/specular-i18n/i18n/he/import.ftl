# machine-drafted, needs native review
# Specular — ייבוא ממצלמות, מטלפונים ומכרטיסי זיכרון. עברית.
# הכרטיס רק נקרא; קבצים נמחקים ממנו רק לפי בקשה, אחרי העתקה שנבדקה.

palette-import = ייבוא תמונות…
disabled-no-import-source = לא מחובר כרטיס זיכרון, מצלמה או טלפון

## הגדרות → ייבוא
settings-section-import = ייבוא
settings-import-on-card = כשמכניסים כרטיס
settings-import-on-card-help = מה קורה כשמחברים כרטיס זיכרון, מצלמה או טלפון עם תמונות.
settings-import-on-card-ask = לשאול
settings-import-on-card-open = לפתוח ייבוא
settings-import-on-card-nothing = לא לעשות דבר
settings-import-destination = ייבוא אל
settings-import-destination-help = התיקייה שאליה נכנסים התמונות והסרטונים המיובאים. אם לא נקבעה, הם נכנסים לתיקיית התמונות.
settings-import-destination-unset = תיקיית התמונות
settings-import-folder-pattern = תיקיות
settings-import-folder-pattern-help = התיקיות שנוצרות בתוכה: {"{"}year{"}"}, {"{"}month{"}"}, {"{"}day{"}"}, {"{"}camera{"}"} ו־{"{"}event{"}"}, מופרדות ב־/.
settings-import-rename-pattern = שינוי שם
settings-import-rename-pattern-help = שמות קבצים חדשים, כמו בהמרה: {"{"}name{"}"}, {"{"}n:03{"}"}, {"{"}date:%Y%m%d{"}"}, {"{"}camera{"}"}, {"{"}event{"}"}. ריק שומר את שמות המצלמה.
settings-import-keep-pairs = לשמור RAW ו־JPEG יחד
settings-import-keep-pairs-help = קובץ RAW וה־JPEG שצולם איתו נכנסים לאותה תיקייה עם אותו שם, גם אחרי שינוי שם.
settings-import-low-res-videos = לייבא עותקי וידאו ברזולוציה נמוכה
settings-import-low-res-videos-help = חלק מהמצלמות מקליטות עותק תצוגה מקדימה קטן של כל סרטון (.LRV). כשכבוי, הם נשארים בכרטיס.
settings-import-eject = הוצאה בסיום
settings-import-eject-help = מוציא את הכרטיס אחרי שהכול הועתק, כדי שאפשר יהיה להוציא אותו.

## ההצעה כשמכניסים כרטיס
import-offer = לייבא מ־{ $name }?
import-offer-button = ייבוא

## תצוגת הייבוא
import-title = ייבוא
import-from = מ־{ $name }
import-source-next = המקור הבא
import-close = סגירה
import-scanning = מחפש תמונות חדשות… { $done } מתוך { $total }
import-scanning-start = מחפש תמונות חדשות…
import-none-new = כל מה שב־{ $name } כבר יובא בעבר.
import-empty = אין תמונות או סרטונים ב־{ $name }.
import-no-source = לא מחובר כרטיס זיכרון, מצלמה או טלפון.
import-summary = { $count ->
    [one] פריט חדש אחד
    [two] { $count } פריטים חדשים
   *[other] { $count } פריטים חדשים
} · { $size }
import-already = יובאו בעבר: { $count }
import-unreadable = { $count ->
    [one] לא ניתן היה לקרוא קובץ אחד
    [two] לא ניתן היה לקרוא { $count } קבצים
   *[other] לא ניתן היה לקרוא { $count } קבצים
}
import-selected = נבחרו { $count } מתוך { $total }
import-select-all = בחירת הכול
import-select-none = ביטול הבחירה
import-badge-pair = RAW+JPEG
import-badge-raw = RAW
import-badge-video = וידאו

import-destination = ייבוא אל
import-choose = בחירה…
import-folder-pattern = תיקיות
import-event = אירוע
import-event-hint = אופציונלי, עבור {"{"}event{"}"}
import-rename = שינוי שם
import-rename-hint = ריק שומר את שמות המצלמה
import-preview = הקובץ הראשון: { $path }
import-pattern-error = { $error }
import-keep-pairs = לשמור RAW ו־JPEG יחד
import-eject = הוצאה בסיום
import-delete = מחיקה מהכרטיס אחרי הייבוא
import-free = להעתקה { $size } · שטח פנוי { $free }
import-free-unknown = להעתקה { $size }
import-no-space = אין מספיק שטח פנוי ב־{ $folder }: הייבוא הזה צריך { $need } ופנויים רק { $free }. יש לפנות שטח או לבחור תיקייה אחרת.
import-start = { $count ->
    [one] ייבוא פריט אחד
    [two] ייבוא { $count } פריטים
   *[other] ייבוא { $count } פריטים
}
import-nothing-selected = בחרו את התמונות לייבוא

import-confirm-delete = { $count ->
    [one] אחרי ההעתקה והבדיקה, קובץ אחד יימחק מ־{ $name }. אי אפשר לבטל זאת.
    [two] אחרי ההעתקה והבדיקה, { $count } קבצים יימחקו מ־{ $name }. אי אפשר לבטל זאת.
   *[other] אחרי ההעתקה והבדיקה, { $count } קבצים יימחקו מ־{ $name }. אי אפשר לבטל זאת.
}
import-confirm-yes = ייבוא ומחיקה
import-confirm-no = חזרה

import-progress = מייבא { $done } מתוך { $total } · { $size }
import-progress-eta = מייבא { $done } מתוך { $total } · { $size } · { $time }
import-paused = מושהה · { $done } מתוך { $total }
import-pause = השהיה
import-resume = המשך
import-cancel = ביטול
import-hide = הסתרה
import-stopping = עוצר…
import-running = כבר מתבצע ייבוא.

import-done = { $count ->
    [one] קובץ אחד יובא
    [two] { $count } קבצים יובאו
   *[other] { $count } קבצים יובאו
} אל { $folder }
import-done-existed = כבר היו שם: { $count }
import-done-deleted = נמחקו מהכרטיס: { $count }
import-done-failed = { $count ->
    [one] לא ניתן היה לייבא קובץ אחד
    [two] לא ניתן היה לייבא { $count } קבצים
   *[other] לא ניתן היה לייבא { $count } קבצים
}
import-done-cancelled = הייבוא בוטל אחרי { $count ->
        [one] קובץ אחד
        [two] שני קבצים
       *[other] { $count } קבצים
    }. ייבוא חוזר ימשיך מהמקום שבו נעצר.
import-done-nothing = לא יובא שום דבר חדש.
import-ejected = אפשר להוציא את { $name }.
import-eject-failed = לא ניתן היה להוציא את { $name }: { $error }
import-failed-destination = אי אפשר להשתמש בתיקייה { $folder }: { $error }
import-ledger-failed = לא ניתן היה לקרוא את רשימת הקבצים שיובאו ({ $error }); ייתכן שקבצים שיובאו בעבר יוצעו שוב.

## קורא מסך
import-a11y-group = ייבוא
import-a11y-tile = { $name }, { $date }
import-a11y-tile-selected = { $name }, { $date }, נבחר
