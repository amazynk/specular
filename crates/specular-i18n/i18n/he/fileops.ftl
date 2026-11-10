# machine-drafted, needs native review
# פעולות על קבצים: שינוי שם, שינוי שם לקבוצה, העתקה והעברה לתיקייה, סיבוב שנשמר בקובץ,
# ביטול וביצוע מחדש, הגדרה כרקע, שיתוף, שליחה בדוא״ל, ייצוא וייבוא של הגדרות.

## לוח הפקודות, דף העזר והתפריטים

palette-undo = ביטול פעולה
palette-undo-help = ביטול השינוי האחרון בקבצים: העברה לאשפה, שינוי שם, העתקה או העברה, סיבוב.
palette-redo = ביצוע מחדש
palette-redo-help = ביצוע מחדש של מה שהביטול האחרון החזיר.
palette-rename = שינוי שם…
palette-rename-help = מתן שם חדש לקובץ; קובצי הלוואי ובן הזוג RAW+JPEG משתנים איתו.
palette-batch-rename = שינוי שם לקבצים…
palette-batch-rename-help = שינוי שם לקבצים שנבחרו, או לכל התיקייה, לפי תבנית ועם תצוגה מקדימה.
palette-copy-to-folder = העתקה לתיקייה…
palette-copy-to-folder-help = העתקת הקובץ או הבחירה לתיקייה אחרת.
palette-move-to-folder = העברה לתיקייה…
palette-move-to-folder-help = העברת הקובץ או הבחירה לתיקייה אחרת.
palette-rotate-file-left = סיבוב הקובץ שמאלה
palette-rotate-file-left-help = סיבוב הקובץ עצמו ברבע סיבוב שמאלה, ללא אובדן כשהפורמט מאפשר.
palette-rotate-file-right = סיבוב הקובץ ימינה
palette-rotate-file-right-help = סיבוב הקובץ עצמו ברבע סיבוב ימינה, ללא אובדן כשהפורמט מאפשר.
palette-set-wallpaper-help = הפיכת התמונה לרקע שולחן העבודה.
palette-share = שיתוף…
palette-share-help = שליחת הקובץ ליישום אחר דרך תפריט השיתוף של המערכת.
palette-send-by-email = שליחה בדוא״ל…
palette-send-by-email-help = פתיחת הודעה חדשה ביישום הדואר עם הקובץ מצורף.
# machine-drafted, needs native review
palette-share-with-zynk = שיתוף עם Zynk…
palette-share-with-zynk-help = יצירת קישור אינטרנט לקובץ ביישום Zynk.
palette-send-with-zynk = שליחה עם Zynk…
palette-send-with-zynk-help = שליחת הקובץ לאיש קשר או לאחד ממכשיריך באמצעות Zynk.
palette-export-settings = ייצוא הגדרות…
palette-export-settings-help = שמירת כל ההגדרות וקיצורי המקשים לקובץ.
palette-import-settings = ייבוא הגדרות…
palette-import-settings-help = קריאת הגדרות מקובץ, אחרי הצגת מה שישתנה.

disabled-in-archive-file = עמוד זה נמצא בתוך ארכיון ואין לו קובץ משלו
disabled-not-rotatable = סוג קובץ זה לא יכול לשמור סיבוב
disabled-not-on-this-desktop = שולחן העבודה שלך לא מציע זאת

## הגדרות

settings-browsing-rotate-trim-edges = חיתוך שולי JPEG לסיבוב ללא אובדן
settings-browsing-rotate-trim-edges-help = קובץ JPEG שגודלו אינו כפולה של 8 או 16 פיקסלים ניתן לסובב ללא אובדן רק בחיתוך הפיקסלים האחרונים שלו. בלי זה משתנה במקום זאת תג הכיוון שלו, והחיתוך מוצע בכל פעם.
settings-browsing-rotate-keep-modified = שמירת תאריך השינוי בעת סיבוב
settings-browsing-rotate-keep-modified-help = קובץ שסובב שומר על תאריך השינוי האחרון שלו, כך שהוא נשאר במקומו במיון לפי תאריך.
settings-browsing-recent-folders = תיקיות אחרונות
settings-browsing-recent-folders-help = תיקיות ששימשו לאחרונה להעתקה ולהעברה.

## חלונות

fileop-cancel = ביטול
rename-title = שינוי שם
rename-field-label = שם
rename-button = שינוי שם
rename-sidecars-follow = גם שונה שמם: { $files }
rename-extension-changed = שינוי הסיומת לא ממיר את הקובץ; ייתכן שיישומים אחרים לא יפתחו אותו.
rename-problem-empty = יש להקליד שם.
rename-problem-invalid-char = שמות לא יכולים להכיל ”{ $char }“.
rename-problem-reserved = ”{ $name }“ הוא שם ש־Windows שומרת להתקנים.
rename-problem-too-long = השם ארוך מדי; שמות יכולים להכיל עד 255 בתים.
rename-problem-dot = אי אפשר להשתמש ב־”.“ וב־”..“ כשמות.
rename-problem-trailing = שמות לא יכולים להסתיים ברווח או בנקודה.
rename-problem-exists = לקובץ אחר כבר יש שם זה.
rename-problem-read-only = אי אפשר לשנות תיקייה זו.
rename-problem-duplicate = שני קבצים יקבלו שם זה.

batch-rename-title =
    { $count ->
        [one] שינוי שם לקובץ אחד
        [two] שינוי שם ל־{ $count } קבצים
       *[other] שינוי שם ל־{ $count } קבצים
    }
batch-rename-field-label = תבנית
batch-rename-tokens = אפשר להשתמש ב־{ "{" }name{ "}" }, { "{" }n:3{ "}" } (001, 002, …), { "{" }date:%Y-%m-%d{ "}" }, { "{" }camera{ "}" }, { "{" }w{ "}" }×{ "{" }h{ "}" } ו־{ "{" }ext{ "}" }.
batch-rename-summary =
    { $count ->
        [one] שם של קובץ אחד ישתנה
        [two] שמות של { $count } קבצים ישתנו
       *[other] שמות של { $count } קבצים ישתנו
    }{ $problems ->
        [0] .
        [one] ; לאחד אי אפשר, והוא נשאר כמו שהוא.
        [two] ; ל־{ $problems } אי אפשר, והם נשארים כמו שהם.
       *[other] ; ל־{ $problems } אי אפשר, והם נשארים כמו שהם.
    }
batch-rename-reading = קורא תאריכים ומצלמות…
batch-rename-preview = תצוגה מקדימה
batch-rename-unchanged = שומר על שמו
batch-rename-more =
    { $count ->
        [one] עוד אחד למטה
        [two] עוד { $count } למטה
       *[other] עוד { $count } למטה
    }

transfer-copy-title-one = העתקת ”{ $file }“ לתיקייה
transfer-copy-title =
    { $count ->
        [one] העתקת קובץ אחד לתיקייה
        [two] העתקת { $count } קבצים לתיקייה
       *[other] העתקת { $count } קבצים לתיקייה
    }
transfer-move-title-one = העברת ”{ $file }“ לתיקייה
transfer-move-title =
    { $count ->
        [one] העברת קובץ אחד לתיקייה
        [two] העברת { $count } קבצים לתיקייה
       *[other] העברת { $count } קבצים לתיקייה
    }
transfer-recent = תיקיות אחרונות
transfer-folder = תיקייה
transfer-choose-folder = בחירת תיקייה…
transfer-copy-dialog = העתקה לתיקייה
transfer-move-dialog = העברה לתיקייה
transfer-sidecars =
    { $count ->
        [one] קובץ לוואי אחד עובר יחד איתו.
        [two] { $count } קובצי לוואי עוברים יחד איתם.
       *[other] { $count } קובצי לוואי עוברים יחד איתם.
    }
transfer-taken =
    { $count ->
        [one] שם אחד כבר בשימוש ב־{ $folder }.
        [two] { $count } שמות כבר בשימוש ב־{ $folder }.
       *[other] { $count } שמות כבר בשימוש ב־{ $folder }.
    }
transfer-when-taken = כששם כבר בשימוש
transfer-skip = דילוג
transfer-keep-both = שמירת שניהם
transfer-replace-option = החלפה
transfer-copy = העתקה
transfer-move = העברה
transfer-replace-title =
    { $count ->
        [one] להחליף קובץ אחד ב־{ $folder }?
        [two] להחליף { $count } קבצים ב־{ $folder }?
       *[other] להחליף { $count } קבצים ב־{ $folder }?
    }
transfer-replace-note = הקבצים שנמצאים שם עכשיו יעברו לאשפה. ביטול מחזיר אותם.
transfer-replace = החלפה
transfer-back = חזרה
transfer-stop = עצירה

settings-file-import-title = ייבוא הגדרות
settings-file-import-dialog = ייבוא הגדרות
settings-file-export-dialog = ייצוא הגדרות
settings-file-import-changes = שינויים
settings-file-import-change = { $before } ← { $after }
settings-file-import-no-key = אין מקש
settings-file-import-unknown = לא הגדרה של גרסה זו; הושמט.
settings-file-import-keymap = קיצורי מקשים
settings-file-import-nothing = שום דבר לא ישתנה: ההגדרות האלה כבר בתוקף.
settings-file-import-summary =
    { $count ->
        [one] שינוי אחד. אפשר לבטל את הייבוא בהגדרות.
        [two] { $count } שינויים. אפשר לבטל את הייבוא בהגדרות.
       *[other] { $count } שינויים. אפשר לבטל את הייבוא בהגדרות.
    }
settings-file-import-secrets =
    { $count ->
        [one] רשומה אחת נראית כמו סיסמה או מפתח ולעולם לא מיובאת.
        [two] { $count } רשומות נראות כמו סיסמאות או מפתחות ולעולם לא מיובאות.
       *[other] { $count } רשומות נראות כמו סיסמאות או מפתחות ולעולם לא מיובאות.
    }
settings-file-import-commands =
    { $count ->
        [one] פקודת משתמש אחת לא יובאה: פקודות מפעילות תוכניות. אם היא אמינה בעיניך, העתק אותה לקובץ ההגדרות בעצמך.
        [two] { $count } פקודות משתמש לא יובאו: פקודות מפעילות תוכניות. אם הן אמינות בעיניך, העתק אותן לקובץ ההגדרות בעצמך.
       *[other] { $count } פקודות משתמש לא יובאו: פקודות מפעילות תוכניות. אם הן אמינות בעיניך, העתק אותן לקובץ ההגדרות בעצמך.
    }
settings-file-import-button = ייבוא

## תוצאות

fileop-renamed = השם שונה ל־{ $file }
fileop-renamed-count =
    { $count ->
        [one] שונה שם של קובץ אחד
        [two] שונו שמות של { $count } קבצים
       *[other] שונו שמות של { $count } קבצים
    }
fileop-copied =
    { $count ->
        [one] קובץ אחד הועתק אל { $folder }
        [two] { $count } קבצים הועתקו אל { $folder }
       *[other] { $count } קבצים הועתקו אל { $folder }
    }
fileop-moved =
    { $count ->
        [one] קובץ אחד הועבר אל { $folder }
        [two] { $count } קבצים הועברו אל { $folder }
       *[other] { $count } קבצים הועברו אל { $folder }
    }
fileop-skipped =
    { $count ->
        [one] דולג על אחד: שמו כבר בשימוש שם.
        [two] דולג על { $count }: שמותיהם כבר בשימוש שם.
       *[other] דולג על { $count }: שמותיהם כבר בשימוש שם.
    }
fileop-stopped = נעצר; מה שבוצע נשאר.
fileop-failed-file =
    { $count ->
        [one] לא ניתן להעתיק או להעביר את { $file }: { $reason }
        [two] לא ניתן להעתיק או להעביר { $count } קבצים; הראשון, { $file }: { $reason }
       *[other] לא ניתן להעתיק או להעביר { $count } קבצים; הראשון, { $file }: { $reason }
    }
fileop-progress-copy =
    { $count ->
        [one] מעתיק קובץ אחד… { PERCENT($percent) }
        [two] מעתיק { $count } קבצים… { PERCENT($percent) }
       *[other] מעתיק { $count } קבצים… { PERCENT($percent) }
    }
fileop-progress-move =
    { $count ->
        [one] מעביר קובץ אחד… { PERCENT($percent) }
        [two] מעביר { $count } קבצים… { PERCENT($percent) }
       *[other] מעביר { $count } קבצים… { PERCENT($percent) }
    }
fileop-done-status = הסתיים
fileop-busy = השינוי האחרון בקבצים עדיין מתבצע; נסו שוב בעוד רגע.
fileop-nothing-to-redo = אין מה לבצע מחדש
fileop-undone = בוטל: { $what }
fileop-redone = בוצע מחדש: { $what }
fileop-kind-rename = שינוי שם
fileop-kind-batch-rename = שינוי שם לכמה קבצים
fileop-kind-move = העברה לתיקייה
fileop-kind-copy = העתקה לתיקייה
fileop-kind-rotate = סיבוב
rename-failed = לא ניתן לשנות שם: { $reason }

undo-problem-changed = אי אפשר לבטל: { $file } השתנה מאז, והביטול יאבד את השינוי הזה.
undo-problem-missing = אי אפשר לבטל: { $file } כבר לא קיים.
undo-problem-taken = אי אפשר לבטל: לקובץ אחר יש עכשיו את השם { $file }.
undo-problem-io = אי אפשר לבטל את השינוי ב־{ $file }: { $reason }

rotate-done-lossless = { $file } סובב ללא כל אובדן.
rotate-done-lossless-trimmed = { $file } סובב ללא כל אובדן; השוליים נחתכו ל־{ $width } × { $height }.
rotate-done-tag = { $file } סובב באמצעות שינוי תג הכיוון שלו.
rotate-done-sidecar = הסיבוב של { $file } נשמר ב־{ $sidecar }; קובץ ה־RAW עצמו לעולם לא משתנה.
rotate-tag-unreadable = לא ניתן היה לקרוא מספיק מנתוני התמונה כדי לסובב אותה.
rotate-tag-multi-picture = הוא מכיל יותר מתמונה אחת, וסיבוב הראשונה ישבור את האחרות.
rotate-tag-trailer = אחרי התמונה יש נתונים שהסיבוב יזיז.
rotate-tag-not-aligned = סיבוב הפיקסלים ללא אובדן יחתוך אותו ל־{ $width } × { $height }.
rotate-trim-offer = חיתוך וסיבוב
rotate-problem-unsupported = { $file } הוא סוג קובץ שלא יכול לשמור סיבוב.
rotate-problem-too-large = { $file } גדול מדי לסיבוב.
rotate-problem-damaged = { $file } פגום במקום שבו נשמר הכיוון: { $reason }
rotate-problem-io = לא ניתן לסובב את { $file }: { $reason }

share-failed = לא ניתן לשתף: { $reason }
email-failed = לא ניתן להפעיל את יישום הדואר: { $reason }
# machine-drafted, needs native review
zynk-opening = פותח את Zynk…
zynk-failed = לא ניתן להתחבר ל-Zynk: { $reason }
zynk-get-title-send = ”שליחה עם Zynk“ דורשת את היישום Zynk
zynk-get-title-share = ”שיתוף עם Zynk“ דורש את היישום Zynk
zynk-get-about = היישום Zynk הוא יישום חינמי ל-Mac, ל-Windows, ל-Linux, ל-iPhone ול-Android. הוא שולח תמונות, סרטונים ותיקיות שלמות ישירות למכשירים האחרים שלך ולאנשים אחרים, בכל גודל. ההעברות מוצפנות מקצה לקצה וממשיכות מהמקום שבו נעצרו אם החיבור מתנתק. לנמען שאין לו Zynk, הוא יוצר קישור אינטרנט שממנו אפשר להוריד את הקבצים בדפדפן.
zynk-get-macos = אפשר להוריד אותו מ-Mac App Store. אחרי ההתקנה, { -app-name } מעביר אליו את הקבצים שלך ישירות.
zynk-get-windows = אפשר להוריד אותו מ-Microsoft Store. אחרי ההתקנה, { -app-name } מעביר אליו את הקבצים שלך ישירות.
zynk-get-linux = הגרסה של Zynk ל-Linux היא AppImage, בלי שום התקנה: יש להוריד אותה, לשים אותה ב-⁦~/Applications⁩ ולהפוך אותה לניתנת להרצה (במאפיינים שלה, או באמצעות ⁦chmod +x⁩). { -app-name } מוצא אותה שם, ובכל מקום אחר כל עוד Zynk פועל.
zynk-get = הורדת Zynk
zynk-send-old-windows = ”שליחה עם Zynk“ דורשת Windows 10 גרסה 2004 ואילך. ”שיתוף עם Zynk“ עובד בגרסה הזו של Windows.
settings-file-export-done = ההגדרות נשמרו ב־{ $file }
settings-file-export-failed = לא ניתן לשמור את ההגדרות: { $reason }
settings-file-import-unreadable = לא ניתן לקרוא את { $file }: { $reason }
settings-file-import-done =
    { $count ->
        [one] יובא שינוי אחד
        [two] יובאו { $count } שינויים
       *[other] יובאו { $count } שינויים
    }
