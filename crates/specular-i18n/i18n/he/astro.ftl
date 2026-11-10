# machine-drafted, needs native review
## Astronomy interop: DS9 regions, plate solving, SAMP, HiPS sky surveys.

palette-science-load-regions = הצגת אזורי DS9‏ מעל התמונה…
palette-science-load-regions-help = ציור הצורות מקובץ אזורים של DS9‏ (.reg) על התמונה.
palette-science-export-regions = שמירת האזורים כ־DS9‏…
palette-science-export-regions-help = שמירת המדידות והאזורים המוצגים כקובץ אזורים של DS9‏.
palette-science-edit-regions = עריכת האזורים כמדידות
palette-science-edit-regions-help = הפיכת האזורים המוצגים למדידות שאפשר להזיז ולשנות.
palette-science-clear-regions = הסרת האזורים המוצגים מעל התמונה
palette-science-clear-regions-help = הסרת אזורי DS9‏ מהתמונה.
palette-science-plate-solve = פתרון אסטרומטרי
palette-science-plate-solve-help = מציאת קואורדינטות השמיים של התמונה בעזרת ASTAP או astrometry.net‏. הפעלה נוספת עוצרת.
palette-science-save-plate-solution = שמירת הפתרון האסטרומטרי
palette-science-save-plate-solution-help = שמירת קואורדינטות השמיים שנמצאו לצד התמונה, כקובץ .wcs‏.
palette-science-sky-survey = סקר שמיים ברקע
palette-science-sky-survey-help = הצגת סקר השמיים הבא סביב התמונה, או אף אחד.
palette-science-samp-send-image = שליחת התמונה ליישומי SAMP
palette-science-samp-send-image-help = פתיחת התמונה הזו ב־DS9‏, ב־Aladin וביישומים האחרים במרכזת SAMP‏.
palette-science-samp-send-position = שליחת המיקום ליישומי SAMP
palette-science-samp-send-position-help = הפניית היישומים האחרים למיקום בשמיים שמתחת לסמן.

disabled-samp-off = יש להתחבר קודם למרכזת SAMP‏ (הגדרות › שילוב).

settings-integration-samp = התחברות למרכזת SAMP
settings-integration-samp-help = החלפת תמונות, טבלאות ומיקומים בשמיים עם DS9‏, TOPCAT‏, Aladin ויישומי אסטרונומיה אחרים. נעשה שימוש רק במרכזת שבמחשב הזה.
settings-integration-astap-path = תוכנת ASTAP
settings-integration-astap-path-help = הפותר האסטרומטרי ASTAP‏, כשהוא לא נמצא מעצמו.
settings-integration-solve-field-path = תוכנת astrometry.net
settings-integration-solve-field-path-help = תוכנת solve-field של astrometry.net‏, כשהיא לא נמצאת מעצמה.

astro-regions-filter = אזורי DS9
astro-regions-loaded = { $name }: { $count ->
        [one] אזור אחד.
        [two] שני אזורים.
       *[other] { $count } אזורים.
    }
astro-regions-refused = { $count ->
        [one] שורה אחת לא נקראה.
        [two] שתי שורות לא נקראו.
       *[other] { $count } שורות לא נקראו.
    } הראשונה: שורה { $line }, { $reason }.
astro-regions-approximate = { $count ->
        [one] צורת panda אחת צוירה לפי השוליים הפנימיים והחיצוניים שלה.
        [two] שתי צורות panda צוירו לפי השוליים הפנימיים והחיצוניים שלהן.
       *[other] { $count } צורות panda צוירו לפי השוליים הפנימיים והחיצוניים שלהן.
    }
astro-regions-none-here = { $name }: אף אזור לא נופל על התמונה הזו.
astro-regions-failed = לא ניתן לקרוא את { $name }: { $reason }
astro-regions-cleared = האזורים הוסרו.
astro-regions-nothing = אין אזורים או מדידות לשמירה.
astro-regions-saved = { $name } נשמר.
astro-regions-skipped = { $count ->
        [one] צורה אחת מחוץ לקואורדינטות השמיים הושמטה.
        [two] שתי צורות מחוץ לקואורדינטות השמיים הושמטו.
       *[other] { $count } צורות מחוץ לקואורדינטות השמיים הושמטו.
    }
astro-regions-save-failed = לא ניתן לשמור את האזורים: { $reason }
astro-regions-edited = { $count ->
        [one] אזור אחד הוא עכשיו מדידה.
        [two] שני אזורים הם עכשיו מדידות.
       *[other] { $count } אזורים הם עכשיו מדידות.
    }
astro-regions-no-wcs = אזורים בשמיים דורשים תמונה עם קואורדינטות שמיים.

astro-solve-no-solver = לא נמצא פותר אסטרומטרי. יש להתקין ASTAP או astrometry.net‏, או לבחור אחד בהגדרות › שילוב.
astro-solve-running = פותר בעזרת { $solver }… { $seconds } שנ׳
astro-solve-stop = עצירה
astro-solve-stopped = הפתרון האסטרומטרי נעצר.
astro-solve-done = נפתר: { $ra }, { $dec }, { $scale } לפיקסל.
astro-solve-failed = הפתרון האסטרומטרי נכשל: { $reason }
astro-solve-not-local = אפשר לפתור רק תמונות שבתיקייה במחשב הזה.
astro-solve-none = אין עדיין פתרון אסטרומטרי לתמונה הזו.
astro-solve-saved = { $name } נשמר. התמונה עצמה לא שונתה.
astro-solve-sidecar = קואורדינטות שמיים מתוך { $name }.

astro-samp-connected = מחובר למרכזת SAMP‏.
astro-samp-no-hub = ‏SAMP: { $reason }. ניסיון נוסף כשמרכזת תופעל.
astro-samp-lost = מרכזת SAMP נעצרה. ניסיון נוסף כשתופעל מרכזת.
astro-samp-sent = { $count ->
        [one] נשלח אל יישום אחד.
        [two] נשלח אל שני יישומים.
       *[other] נשלח אל { $count } יישומים.
    }
astro-samp-nobody = אף יישום במרכזת לא מאזין להודעה הזו.
astro-samp-failed = ‏SAMP: { $reason }
astro-samp-refused = ‏SAMP: { $url } לא נמצא במחשב הזה, ולכן לא נפתח.
astro-samp-pointed = הופנה אל { $ra }, { $dec }.
astro-samp-off-image = { $ra }, { $dec } לא נמצא בתמונה הזו.
astro-samp-no-table = ‏SAMP: הטבלה הזו לא מוצגת כאן מעל התמונה.
astro-samp-rows = { $count ->
        [one] מקור אחד מהקטלוג הודגש.
        [two] שני מקורות מהקטלוג הודגשו.
       *[other] { $count } מקורות מהקטלוג הודגשו.
    }

astro-survey-off = סקר השמיים כבוי.
astro-survey-on = סקר שמיים: { $name }. { $credit }
astro-survey-failed = לא ניתן להציג את { $name }: { $reason }
astro-survey-credit = { $name }: { $credit }

net-what-sky-survey = אריחי סקר שמיים מ־{ $host }
