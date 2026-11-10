# machine-drafted, needs native review
## חלונית המדריך, דוחות קריסה, חבילת האבחון, בדיקת עדכונים ואודות. עברית.
## סגנון: קצר, ברור, בלי מילוי; רעיון אחד בכל משפט.

## לוח

palette-help-manual = עזרה
palette-help-manual-help = פתיחת המדריך בעמוד של מה שמוצג במסך.
palette-save-diagnostics = שמירת חבילת אבחון…
palette-save-diagnostics-help = שמירת דוחות קריסה, היומן ופרטי המערכת בקובץ zip. שום דבר לא נשלח.
# פריט בלוח שפותח סעיף במדריך; { $title } הוא עמוד או ”עמוד › סעיף“.
palette-help-topic = עזרה: { $title }
palette-help-topic-help = פתיחת הסעיף הזה במדריך.
# מתחת לפקודה לא זמינה בלוח.
palette-disabled-help = לא זמין כרגע. יש ללחוץ Enter כדי לקרוא למה.

## חלונית העזרה

help-title = עזרה
help-search-placeholder = חיפוש במדריך
help-contents = תוכן העניינים
help-back = אחורה
help-forward = קדימה
help-no-results = לא נמצא דבר עבור ”{ $query }“.
help-results = { $count ->
    [one] נמצא סעיף אחד
    [two] נמצאו { $count } סעיפים
   *[other] נמצאו { $count } סעיפים
}
help-section-start = צעדים ראשונים
help-section-modes = תצוגה ומצבים
help-section-tasks = איך…
help-section-reference = עיון
help-keys-unbound = לא מוגדר
# פקודה שמוזכרת במדריך ואין לה מקש; היא מופעלת מהלוח.
help-key-unbound = { $name } (לוח)
help-keys-command = פקודה
help-keys-keys = מקשים
help-commands = כל פקודה נמצאת בלוח הפקודות ({ $key }). יש להקליד חלק מהשם.
help-commands-no-key = כל פקודה נמצאת בלוח הפקודות. יש להקליד חלק מהשם.
help-link = מעבר אל { $title }
help-open = עזרה
# שם החלונית לקוראי מסך; { $page } הוא העמוד המוצג.
help-label = עזרה: { $page }
help-disabled-why = למה?

## דוח קריסה שמוצע בהפעלה הבאה

crash-title = { -app-name } נסגר באופן לא צפוי
crash-body = דוח נשמר במחשב הזה. שום דבר לא נשלח.
crash-view = הצגת הדוח
crash-hide = הסתרת הדוח
crash-copy = העתקה
crash-copied = הדוח הועתק
crash-save-bundle = שמירת חבילת אבחון…
crash-dismiss = סגירה

## חבילת אבחון

diag-title = שמירת חבילת אבחון
diag-intro = קובץ zip לצירוף לדיווח על באג. הוא נשמר במחשב הזה. שום דבר לא נשלח.
diag-contents = תוכן
diag-include-names = הכללת שמות קבצים
diag-include-names-help = כבוי: השמות מוחלפים ב־file-1.jpg, dir-1 וכן הלאה.
diag-privacy = תיקיית הבית שלך מופיעה כ־~ ושם המשתמש שלך כ־<user>. סיסמאות, כתובות שרתים ותמונות אף פעם לא נכללים.
diag-where = דיווח על בעיה: { $issues } · { $email }
diag-save = שמירה…
diag-cancel = ביטול
diag-saved = חבילת האבחון נשמרה: { $path }
diag-failed = לא ניתן היה לשמור את חבילת האבחון: { $reason }
diag-entry-summary = גרסה, מערכת, GPU ומנהל התקן
diag-entry-crash = דוח הקריסה האחרון
diag-entry-crashes = דוחות קריסה קודמים ({ $count })
diag-entry-log = { $count } שורות אחרונות מהיומן
diag-entry-settings = הגדרות, בלי סיסמאות או כתובות
diag-entry-gpu = פרטי GPU ותצוגה
diag-entry-readme = מה החבילה הזאת
diag-size = { $size }
# קובץ README.txt שבתוך החבילה.
diag-readme =
    חבילת אבחון של { -app-name }.

    היא מכילה את מה שעוזר למצוא בעיה: פרטי גרסה ומערכת, דוחות קריסה, סוף היומן וההגדרות.

    תיקיית הבית שלך מופיעה כ־~ ושם המשתמש שלך כ־<user>. אלא אם בחרת לכלול אותם, שמות קבצים ותיקיות מוחלפים ב־file-1, dir-1 וכן הלאה. סיסמאות וכתובות שרתים מוסרות.

    שום דבר לא נשלח. ההחלטה אם לשתף את הקובץ הזה היא שלך.

## עדכונים (הגדרות)

settings-section-updates = עדכונים
settings-updates-check = בדיקת עדכונים
settings-updates-check-help = פעם ביום, לשאול אם יש גרסה חדשה. הבקשה כוללת רק את מספר הגרסה: בלי מזהה, בלי עוגייה. כבוי: { -app-name } אף פעם לא מתחבר לצורך עדכונים.
settings-updates-channel = ערוץ
settings-updates-channel-help = יציב: גרסאות רשמיות בלבד. בטא: גם גרסאות ניסיון.
settings-updates-channel-stable = יציב
settings-updates-channel-beta = בטא
settings-updates-check-now = בדיקה עכשיו
settings-updates-check-now-help = בדיקה אחת עכשיו.
update-available = גרסה { $version } זמינה
update-notes = הערות גרסה
update-download = הורדה
update-none = { -app-name } מעודכן.
update-checking = מתבצעת בדיקת עדכונים…
update-failed = לא ניתן היה לבדוק עדכונים: { $reason }
update-last-checked = נבדק לאחרונה { $when }

## אודות

about-build = בנייה { $commit } · { $profile } · { $target }
about-build-no-commit = בנייה { $profile } · { $target }
about-help = עזרה
about-diagnostics = שמירת חבילת אבחון…
# machine-drafted, needs native review
about-renderer = מידע על הרינדור…
about-licences-count = { $count ->
    [one] רכיב צד שלישי אחד
    [two] { $count } רכיבי צד שלישי
   *[other] { $count } רכיבי צד שלישי
}
about-licence-group = { $licence } ({ $count })
about-contact = בעיות: { $issues } · תמיכה: { $email }
