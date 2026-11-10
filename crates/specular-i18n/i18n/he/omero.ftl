# Specular — OMERO servers and the Image Data Resource (IDR), browsed read-only. עברית. machine-drafted, needs native review

palette-omero-servers = שרתי OMERO…

omero-title = שרתי OMERO
omero-servers-intro = פרויקטים, סינונים ותמונות בשרת מוצגים רק כשמעיינים בו. דבר אינו נשלח לפני כן.
omero-servers = שרתים
omero-servers-empty = אין עדיין שרתים. אפשר להוסיף שרת לפי כתובת ה־OMERO.web שלו.
omero-session-only = הסיסמאות נשמרות רק עד לסגירת Specular.
omero-add = הוספה…
omero-edit = עריכה…
omero-remove = הסרה
omero-close = סגירה
omero-browse = עיון
omero-back = חזרה
omero-save = שמירה
omero-stop = עצירה
omero-open = פתיחה
# $server a server's name.
omero-removed = { $server } הוסר
# Shown with servers reached without encryption (http://).
omero-not-encrypted = לא מוצפן
# A server browsed without signing in.
omero-public = ציבורי
# $user the user name a server is signed in with.
omero-signed-in-as = בתור { $user }

# The page that adds or edits a server.
omero-add-title = הוספת שרת
omero-edit-title = הגדרות שרת
omero-field-name = שם
omero-hint-name = רשות
omero-field-url = כתובת
omero-hint-url = https://omero.example.org
omero-field-user = שם משתמש
omero-hint-user = ריק עבור נתונים ציבוריים
omero-field-password = סיסמה
omero-hint-password-kept = להשאיר ריק כדי לשמור על הסיסמה שניתנה
omero-password-session = הסיסמה נשמרת רק עד לסגירת Specular, ולעולם לא בדיסק.
omero-password-needed = נא להזין את הסיסמה: היא נשמרת רק עד לסגירת Specular.
omero-http-warning = כתובת זו אינה מוצפנת: הסיסמה תישלח כטקסט גלוי.
omero-url-invalid = נא להזין את כתובת ה־OMERO.web, שמתחילה ב־https:// או ב־http://.
# $reason why the list could not be saved.
omero-save-failed = לא ניתן היה לשמור את רשימת השרתים: { $reason }

# The browse page.
omero-browse-title = עיון
omero-top = פרויקטים וסינונים
omero-datasets = מערכי נתונים
omero-plates = לוחות
omero-wells = בארות
omero-fields = שדות
omero-images = תמונות
omero-kind-project = פרויקט
omero-kind-dataset = מערך נתונים
omero-kind-image = תמונה
omero-kind-screen = סינון
omero-kind-plate = לוח
omero-kind-well = באר
# $well a well's row and column, as B7.
omero-well = באר { $well }
omero-well-fields = { $count ->
    [one] שדה אחד
    [two] שני שדות
   *[other] { $count } שדות
}
omero-children = { $count ->
    [one] פריט אחד
    [two] שני פריטים
   *[other] { $count } פריטים
}
omero-load-more = טעינת עוד
omero-loading = בטעינה…
omero-empty = אין כאן דבר
omero-stopped = נעצר
omero-found = { $count ->
    [one] פריט אחד
    [two] שני פריטים
   *[other] { $count } פריטים
}
# $reason why the image could not be opened.
omero-open-failed = לא ניתן היה לפתוח את התמונה: { $reason }
# The channel keys would hide the last channel shown.
omero-channel-last = לפחות ערוץ אחד נשאר מוצג
# $server a server's name.
omero-session-ended = ההפעלה ב־{ $server } הסתיימה. נא להזין את הסיסמה כדי להמשיך.

# Errors.
# $server a server's name.
omero-error-offline = לא ניתן היה להתחבר אל { $server }.
omero-error-sign-in = שרת זה דורש כניסה.
omero-error-expired = ההפעלה הסתיימה.
omero-error-login = שם המשתמש או הסיסמה נדחו.
omero-error-forbidden = ייתכן שלחשבון שלך אין הרשאה לראות זאת.
omero-error-not-found = לא נמצא בשרת (ייתכן שהוסר).
# $code an HTTP status code.
omero-error-status = השרת השיב בשגיאה { $code }.
# $reason what was wrong with the answer.
omero-error-malformed = לא ניתן היה לקרוא את תשובת השרת: { $reason }
