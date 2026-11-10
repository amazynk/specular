# Specular — DICOMweb servers, study search and streamed series (DESIGN.md §12.8). עברית.

palette-dicom-servers = שרתי DICOMweb…

dicomweb-title = שרתי DICOMweb
dicomweb-servers-intro = בדיקות בשרת מחופשות ונפתחות רק כשמבקשים זאת. דבר אינו נשלח לפני כן.
dicomweb-servers = שרתים
dicomweb-servers-empty = אין עדיין שרתים. יש להוסיף שרת לפי כתובת ה-DICOMweb שלו.
dicomweb-session-only = סיסמאות ואסימונים נשמרים רק עד לסגירת Specular.
dicomweb-add = הוספה…
dicomweb-edit = עריכה…
dicomweb-remove = הסרה
dicomweb-close = סגירה
dicomweb-browse = עיון
dicomweb-back = חזרה
dicomweb-save = שמירה
dicomweb-search = חיפוש
dicomweb-stop = עצירה
dicomweb-removed = הוסר: { $server }
# machine-drafted, needs native review
dicomweb-remove-ask = להסיר את “{ $server }” מהשרתים השמורים?
dicomweb-remove-detail = שום דבר בדיסק לא אובד. הסיסמה או האסימון שלו לסשן זה נשכחים.
# שרתים שהגישה אליהם אינה מוצפנת (http://).
dicomweb-not-encrypted = לא מוצפן

# הדף להוספה או עריכה של שרת.
dicomweb-add-title = הוספת שרת
dicomweb-edit-title = הגדרות שרת
dicomweb-field-name = שם
dicomweb-hint-name = אופציונלי
dicomweb-field-url = כתובת
dicomweb-hint-url = https://pacs.example.org/dicom-web
dicomweb-field-auth = כניסה
dicomweb-auth-none = ללא כניסה
dicomweb-auth-basic = שם משתמש וסיסמה
dicomweb-auth-bearer = אסימון גישה
dicomweb-field-user = שם משתמש
dicomweb-field-password = סיסמה
dicomweb-field-token = אסימון גישה
dicomweb-hint-secret-kept = יש להשאיר ריק כדי לשמור את הערך שהוזן
dicomweb-secret-session = הסיסמה או האסימון נשמרים רק עד לסגירת Specular, אף פעם לא בדיסק.
dicomweb-secret-needed = יש להזין שוב את הסיסמה או האסימון: הם נשמרים רק עד לסגירת Specular.
dicomweb-http-warning = כתובת זו אינה מוצפנת: הסיסמה או האסימון יישלחו כטקסט גלוי.
dicomweb-url-empty = יש להזין את כתובת ה-DICOMweb של השרת.
dicomweb-url-scheme = הכתובת חייבת להתחיל ב-https:// או ב-http://.
dicomweb-url-host = אין בכתובת שם שרת.
dicomweb-url-userinfo = יש להזין את שם המשתמש והסיסמה בשדות הכניסה, לא בכתובת.
dicomweb-save-failed = לא ניתן היה לשמור את רשימת השרתים: { $reason }

# דף החיפוש.
dicomweb-search-title = חיפוש בדיקות
dicomweb-field-patient-name = שם המטופל
dicomweb-field-patient-id = מזהה מטופל
dicomweb-field-date-from = מתאריך
dicomweb-field-date-to = עד תאריך
dicomweb-hint-date = YYYYMMDD
dicomweb-field-modality = מודליות
dicomweb-hint-modality = CT, MR…
dicomweb-field-accession = מספר הפניה
dicomweb-show-patient = הצגת נתוני מטופל
dicomweb-switch-on = פועל
dicomweb-switch-off = כבוי
dicomweb-patient-shown = נתוני המטופל מוצגים
dicomweb-patient-hidden = נתוני המטופל מוסתרים
dicomweb-studies = בדיקות
dicomweb-search-hint = יש למלא שדה כלשהו ולחפש.
dicomweb-no-studies = אין בדיקות תואמות.
dicomweb-searching = מחפש…
dicomweb-stopped = נעצר
dicomweb-load-more = טעינת עוד
dicomweb-study-untitled = בדיקה ללא תיאור
dicomweb-series-untitled = סדרה ללא תיאור
dicomweb-study-counts = סדרות: { $series }, תמונות: { $images }
dicomweb-series-images = תמונות: { $count }
dicomweb-patient-id = מזהה { $id }
dicomweb-accession = הפניה { $number }
dicomweb-found = בדיקות שנמצאו: { $count }
dicomweb-series-found = סדרות: { $count }

# שגיאות בתקשורת עם שרת.
dicomweb-error-offline = אין גישה אל { $server }. יש לבדוק את הרשת ואת הכתובת.
dicomweb-error-auth = השרת דחה את שם המשתמש, הסיסמה או האסימון.
dicomweb-error-not-found = אין שירות DICOMweb בכתובת זו.
dicomweb-error-other = השרת לא הצליח להשיב: { $reason }

# סדרה המוזרמת למציג.
dicomweb-loading = טוען את { $series }…
dicomweb-loading-progress = טוען את { $series }: { $done } מתוך { $total }
dicomweb-loaded = { $series }: תמונות שנטענו: { $total }
dicomweb-loaded-failed = תמונות שלא ניתן היה לטעון: { $failed } מתוך { $total }
dicomweb-offline-cached = אין גישה אל { $server }: מוצגות התמונות שנשמרו קודם לכן.
dicomweb-cache-failed = לא ניתן היה לפתוח את תיקיית ההורדות: { $reason }
