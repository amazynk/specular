# machine-drafted, needs native review

## Send to PACS (C-STORE, STOW-RS), DIMSE servers, teaching quiz and teaching case export.

## Servers: DIMSE fields and Verify.

dicomweb-field-kind = סוג
dicomweb-kind-web = DICOMweb
dicomweb-kind-dimse = DICOM (DIMSE)
dicomweb-field-host = מארח
dicomweb-hint-host = pacs.example.org
dicomweb-field-port = פורט
dicomweb-hint-port = 104 או 11112
# The application entity title the server answers to.
dicomweb-field-ae = ה-AE title של השרת
dicomweb-hint-ae = PACS
# The application entity title Specular sends as.
dicomweb-field-our-ae = ה-AE title שלנו
# machine-drafted, needs native review
dicomweb-field-tls = חיבור מאובטח (TLS)
dicomweb-tls-on = חיבור מאובטח פועל
dicomweb-tls-off = חיבור מאובטח כבוי
dicomweb-problem-host = יש להזין את שם המארח או את הכתובת של השרת.
dicomweb-problem-port = הפורט הוא מספר מ־1 עד 65535.
dicomweb-problem-ae = ל-AE title יש 1 עד 16 תווים, ללא לוכסן הפוך.
# machine-drafted, needs native review
dicomweb-dimse-note-open = אפשר לחפש בשרתי DICOM (DIMSE), לפתוח מהם סדרות ולשלוח אליהם.
# How series are fetched: C-GET (over our own connection) or C-MOVE (the server connects back).
# machine-drafted, needs native review
pacs-field-retrieve = אחזור באמצעות
# machine-drafted, needs native review
pacs-retrieve-get = C-GET
# machine-drafted, needs native review
pacs-retrieve-move = C-MOVE
# C-MOVE: the address of this computer and the port the server sends to.
# machine-drafted, needs native review
pacs-field-receive-address = כתובת לקבלה
# machine-drafted, needs native review
pacs-hint-receive-address = הכתובת של המחשב הזה, למשל 192.168.1.20
# machine-drafted, needs native review
pacs-field-receive-port = פורט לקבלה
# machine-drafted, needs native review
pacs-hint-receive-port = למשל 11113
# machine-drafted, needs native review
pacs-move-note = עבור C-MOVE, השרת צריך להכיר את ה-AE title שלנו, { $ae }, עם הכתובת והפורט האלה.
    ‏Specular מאזין שם רק בזמן שסדרה מתקבלת.
# machine-drafted, needs native review
pacs-problem-receive-address = יש להזין את הכתובת של המחשב הזה (למשל 192.168.1.20) לקבלה.
# machine-drafted, needs native review
pacs-problem-receive-port = יש להזין פורט לקבלה. נדרש פורט עבור C-MOVE.
dicomweb-verify = אימות
dicomweb-verifying = בודק את השרת…
dicomweb-verify-ok = התקבלה תשובה מ־{ $server }.

## The Studies browser and the palette.

studies-send = שליחה ל-PACS…
studies-send-description = שולח לשרת את הסדרה הזו, את הבדיקה שלה או את כל מה שבתקליטור.
palette-send-to-pacs = שליחה ל-PACS…
palette-send-to-pacs-help = שליחת הסדרה או הבדיקה הנוכחית לשרת PACS.

## The Send page.

pacs-title = שליחה ל-PACS
# Under the title: what is being sent.
pacs-summary = { $studies ->
    [one] בדיקה אחת
    [two] שתי בדיקות
   *[other] { $studies } בדיקות
} · { $files ->
    [one] קובץ אחד
    [two] שני קבצים
   *[other] { $files } קבצים
}
pacs-what = מה לשלוח
pacs-scope-series = הסדרה הזו ({ $files ->
    [one] קובץ אחד
    [two] שני קבצים
   *[other] { $files } קבצים
})
pacs-scope-study = הבדיקה הזו ({ $files ->
    [one] קובץ אחד
    [two] שני קבצים
   *[other] { $files } קבצים
})
pacs-scope-all = כל מה שנקרא ({ $studies ->
    [one] בדיקה אחת
    [two] שתי בדיקות
   *[other] { $studies } בדיקות
}, { $files ->
    [one] קובץ אחד
    [two] שני קבצים
   *[other] { $files } קבצים
})
pacs-anonymise = הפיכה לאנונימי
pacs-anonymise-note = פרטי המטופל מוסרים מהעותקים שנשלחים. הקבצים שלך אינם משתנים.
pacs-identified-note = הקבצים נשלחים כמות שהם, עם פרטי המטופל.
# The list of servers to choose from.
pacs-to = אל
pacs-no-servers = אין עדיין שרתים. יש להוסיף שרת בשרתי DICOMweb.
pacs-send = שליחה…
pacs-go-back = חזרה
pacs-confirm-send = שליחה
pacs-confirm = לשלוח { $studies ->
    [one] בדיקה אחת
    [two] שתי בדיקות
   *[other] { $studies } בדיקות
} ({ $files ->
    [one] קובץ אחד
    [two] שני קבצים
   *[other] { $files } קבצים
}) אל { $server }?
pacs-busy = שליחה עדיין פועלת.
pacs-started = שולח.
pacs-stopping = עוצר…
pacs-sending = שולח { $done } מתוך { $total } אל { $server }…
pacs-sent-all = { $total ->
    [one] הקובץ נשלח אל { $server }.
    [two] שני הקבצים נשלחו אל { $server }.
   *[other] כל { $total } הקבצים נשלחו אל { $server }.
}
pacs-sent-some = { $sent } מתוך { $total } קבצים נשלחו אל { $server }.
pacs-stopped = נעצר אחרי { $sent } מתוך { $total ->
        [one] קובץ אחד
        [two] שני קבצים
       *[other] { $total } קבצים
    }.
pacs-stopped-because = נעצר אחרי { $sent } מתוך { $total ->
        [one] קובץ אחד
        [two] שני קבצים
       *[other] { $total } קבצים
    }: { $reason }
pacs-warnings = { $count ->
    [one] קובץ אחד נשמר עם אזהרה.
    [two] שני קבצים נשמרו עם אזהרה.
   *[other] { $count } קבצים נשמרו עם אזהרה.
}
# A file that was not sent: its file name and why.
pacs-failed-line = { $name }: { $reason }
pacs-failed-more = ועוד { $count }.

## Why one file was not sent.

pacs-reason-unreadable = לא ניתן היה לקרוא: { $reason }
pacs-reason-not-dicom = אינו קובץ DICOM
pacs-reason-burned-in = פרטי מטופל צרובים בתמונה, ולכן אינו נשלח כאנונימי
pacs-reason-document = מסמך שאי אפשר להפוך לאנונימי
pacs-reason-anonymise = לא ניתן היה להפוך לאנונימי: { $reason }
pacs-reason-not-accepted = השרת אינו מקבל { $class }
pacs-reason-convert = לא ניתן היה להמיר עבור השרת: { $reason }
pacs-reason-resources = לשרת אין מספיק מקום או משאבים
pacs-reason-mismatch = לדברי השרת הנתונים אינם תואמים לסוג שלהם
pacs-reason-understand = השרת לא הצליח לקרוא אותו
pacs-reason-class = השרת אינו תומך בסוג אובייקט זה
pacs-reason-not-authorised = השרת אינו מתיר זאת
pacs-reason-status = השרת דחה אותו (סטטוס { $code })
pacs-reason-refused = השרת דחה אותו
pacs-reason-too-large = גדול מדי עבור השרת
pacs-reason-http = השרת דחה אותו (שגיאה { $status })

## Why sending stopped, or Verify failed.

pacs-fatal-network-off = הרשת כבויה.
pacs-fatal-unknown-host = { $host } אינו שם מארח מוכר.
pacs-fatal-connect = אין תשובה ב־{ $host }, פורט { $port }.
pacs-fatal-unreachable = אין גישה אל { $host }.
pacs-fatal-timeout = { $host } לא השיב בזמן.
pacs-fatal-cancelled = נעצר.
pacs-fatal-called-ae = { $host } אינו עונה ל-AE title‏ { $ae }. יש לבדוק את ה-AE title של השרת.
pacs-fatal-calling-ae = { $host } אינו מכיר את ה-AE title שלנו, { $ae }. יש להוסיף אותו בשרת.
pacs-fatal-busy = { $host } עסוק. אפשר לנסות שוב מאוחר יותר.
pacs-fatal-rejected = { $host } דחה את החיבור.
pacs-fatal-aborted = { $host } סיים את החיבור.
pacs-fatal-nothing-accepted = { $host } אינו מקבל אף אחד מסוגי האובייקטים האלה.
pacs-fatal-protocol = { $host } השיב באופן ש-Specular אינו מבין.
pacs-fatal-auth = { $host } לא קיבל את הכניסה.
pacs-fatal-http = { $host } השיב בשגיאה { $status }.

## Teaching quiz.

palette-teaching-quiz = חידון הוראה
palette-teaching-quiz-help = מעבר בין תמונות המפתח או במקרה הוראה, כשהממצאים מוסתרים עד שחושפים אותם.
# "Case 2 of 5".
quiz-case = מקרה { $n } מתוך { $total }
quiz-hint-hidden = רווח חושף את הממצאים · → המקרה הבא · Esc מסיים
quiz-hint-revealed = רווח מסתיר אותם שוב · → המקרה הבא · Esc מסיים
quiz-hidden = הממצאים מוסתרים.
quiz-no-findings = לא נרשמו ממצאים.
quiz-notes = הערות למקרה: { $notes }
quiz-ended = החידון הסתיים.
quiz-empty = אין מקרים להצגה. יש לסמן תמונות מפתח ב-K או לבחור תיקיית מקרה הוראה.
quiz-not-a-case = תיקייה זו אינה מקרה הוראה שיוצא מ-Specular.
quiz-pick-title = בחירת תיקיית מקרה הוראה

## Teaching case folder (Export key images).

figure-keys-case = תיקיית מקרה הוראה
figure-case-notes = הערות למקרה
figure-case-findings = ממצאים
figure-case-none = לא נרשמו
figure-case-readme = מקרה הוראה שיוצא מ-Specular. פרטי המטופל הוסרו.
    ‏images: תמונות המפתח, עם הטקסט הצרוב מושחר.
    ‏DICOM: עותקים אנונימיים ומניפסט Key Object Selection‏ "For Teaching".
    ‏case.txt: ההערות למקרה והממצאים של כל תמונה. case.json: אותו תוכן עבור סקריפטים.
    עבור Radiopaedia: יש להעלות את תיקיית ה-DICOM או את התמונות ולהעתיק את ההערות מ-case.txt.

## Search on a DIMSE server (C-FIND).
# machine-drafted, needs native review
pacs-find-not-supported = { $host } אינו עונה לחיפושים (C-FIND).
pacs-find-status = { $host } דחה את החיפוש (סטטוס { $code }).

## Opening a series from a DIMSE server (C-GET, C-MOVE).
# machine-drafted, needs native review
pacs-retrieve-no-get = { $host } אינו שולח סדרות באמצעות C-GET.
    יש לבחור ב-C-MOVE בהגדרות השרת.
# machine-drafted, needs native review
pacs-retrieve-no-move = { $host } אינו מקבל בקשות C-MOVE.
# machine-drafted, needs native review
pacs-retrieve-unknown-destination = { $host } אינו מכיר את ה-AE title שלנו, { $ae }.
    יש להוסיף אותו בשרת עם הכתובת של המחשב הזה והפורט { $port }.
# machine-drafted, needs native review
pacs-retrieve-resources = ל-{ $host } אין די משאבים. אפשר לנסות שוב מאוחר יותר.
# machine-drafted, needs native review
pacs-retrieve-status = { $host } סירב לשלוח את הסדרה (סטטוס { $code }).
# machine-drafted, needs native review
pacs-retrieve-no-matches = ל-{ $host } אין תמונות לסדרה הזו.
# machine-drafted, needs native review
pacs-retrieve-roles = { $host } סירב לשלוח תמונות דרך החיבור הזה.
    יש לבחור ב-C-MOVE בהגדרות השרת.
# machine-drafted, needs native review
pacs-retrieve-move-nothing = { $host } לא הצליח לשלוח את התמונות למחשב הזה.
    יש לבדוק את הכתובת, את הפורט ואת חומת האש, ושהשרת מכיר את ה-AE title שלנו, { $ae }.
# machine-drafted, needs native review
pacs-retrieve-nothing = { $host } לא הצליח לשלוח { $count ->
    [one] את התמונה
    [two] אף אחת משתי התמונות
   *[other] אף אחת מ־{ $count } התמונות
} בצורה ש־Specular יכול לקרוא.
# machine-drafted, needs native review
pacs-retrieve-listen = אי אפשר לקבל ב־{ $address }, פורט { $port }: { $reason }

## DICOM over TLS.
# machine-drafted, needs native review
pacs-fatal-tls-certificate = התעודה של { $host } אינה מהימנה.
    המנפיק שלה חייב להופיע בין התעודות המהימנות של המערכת.
pacs-fatal-tls = אין חיבור מאובטח אל { $host }.
    יש לבדוק שפורט { $port } משתמש ב-DICOM על גבי TLS.
