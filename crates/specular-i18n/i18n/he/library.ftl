# machine-drafted, needs native review
# Specular — Library (DESIGN §10).

palette-library = ספרייה

## Top bar
library-title = ספרייה
library-locations-button = מיקומים

## Locations sidebar
library-locations = מיקומים
library-all = כל המיקומים
library-add = הוספת מיקום…
library-remove = הסרת { $location } מהספרייה
library-offline = לא מקוון
library-location-pending = ממתינים לאינדוקס: { $count }
library-a11y-location = { $location }, { $count ->
    [one] תמונה אחת
    [two] שתי תמונות
   *[other] { $count } תמונות
}
library-a11y-location-offline = { $location }, לא מקוון, { $count ->
    [one] תמונה אחת
    [two] שתי תמונות
   *[other] { $count } תמונות
}

## Indexing progress
library-walking = תמונות שנמצאו: { $count } · { $location }
# machine-drafted, needs native review
library-walking-count = תמונות שנמצאו: { $count }
library-reading = מאנדקס… { $done } מתוך { $total }
library-held = האינדוקס נעצר בזמן הגלישה
library-up-to-date = מעודכן
# machine-drafted, needs native review
library-stopped = האינדוקס נעצר
library-stopped-pending = האינדוקס נעצר · ממתינים לאינדוקס: { $count }
library-resume = המשך
# machine-drafted, needs native review
# The indexer stopped by itself (a database error, a bug) or stopped moving; $reason says why.
library-halted = האינדוקס נעצר: { $reason }
library-reason-stalled = שום דבר לא זז { $minutes ->
    [one] במשך דקה
    [two] במשך שתי דקות
   *[other] במשך { $minutes } דקות
}. ייתכן שקובץ או כונן אינם מגיבים.
library-retry = לנסות שוב
library-retry-description = מתחיל את האינדוקס מחדש. מה שאונדקס נשאר זמין לחיפוש.
library-save-diagnostics = שמירת חבילת אבחון…
library-stop-description = עוצר את האינדוקס. מה שאונדקס עד כה נשאר זמין לחיפוש; „המשך” או „סריקה מחדש” (F5) ממשיכים מאותה נקודה.
library-resume-description = עובר שוב על כל המיקומים ומאנדקס את מה שנותר.
library-opening = הספרייה נפתחת…
library-searching = מחפש…

## Empty and error states
library-empty = אין עדיין מיקומים. הוסיפו תיקייה: התמונות שבה ובתיקיות המשנה שלה יוצגו כאן ברשת אחת.
library-empty-offline = המיקומים האלה אינם מקוונים. התמונות יוצגו שוב כשיחזרו.
library-indexing = מחפש תמונות…
# machine-drafted, needs native review
library-remove-confirm = להסיר את „{ $location }” מהספרייה?
library-remove-confirm-detail = התמונות שלך נשארות בדיסק, עם הדירוגים, התוויות ומילות המפתח שלהן. הספרייה שוכחת את מה שאינדקסה שם; הוספת התיקייה מחדש תאנדקס אותה שוב.
library-remove-confirm-ok = הסרה
library-indexing-in = מחפש תמונות ב־{ $location }… נמצאו עד כה { $count }
# machine-drafted, needs native review
library-empty-stopped = האינדוקס נעצר לפני שהתמונות האלה נקראו. „המשך” ימשיך משם.
library-unavailable = לא ניתן לפתוח את הספרייה: { $reason }
library-read-only = הספרייה הזו נכתבה בגרסה חדשה יותר של Specular: אפשר לחפש בה אך לא לעדכן אותה.

## Screen readers
library-a11y-list = ספרייה, { $count ->
    [one] תמונה אחת
    [two] שתי תמונות
   *[other] { $count } תמונות
}
library-a11y-added = { $location } נוסף לספרייה
library-a11y-removed = { $location } הוסר מהספרייה

## Adding the home folder
# machine-drafted, needs native review
library-home-confirm = להוסיף את כל התוכן של „{ $folder }”?
library-home-confirm-detail = יש בה קובצי תוכנות, הורדות ומטמונים, לא רק תמונות. האינדוקס ארוך יותר, והספרייה מתמלאת בסמלים ובתמונות אחרות.
library-home-confirm-pictures = בתיקיית התמונות שלך יש רק את התמונות שלך.
library-home-confirm-use-pictures = הוספת התמונות
library-home-confirm-keep = הוספת תיקיית הבית

## Folder watches
# machine-drafted, needs native review
library-unwatched = חלק מהתיקיות אינן במעקב
library-unwatched-detail = { $minutes ->
    [one] שינויים מופיעים תוך דקה
    [two] שינויים מופיעים תוך שתי דקות
   *[other] שינויים מופיעים תוך { $minutes } min
}
library-unwatched-description = ל-Specular נגמרו המעקבים אחר תיקיות. פותח את העזרה על הגדלת המגבלה.

## Settings → Library
# machine-drafted, needs native review
settings-section-library = ספרייה
settings-library-skip-folders = דילוג על תיקיות בשם
settings-library-skip-folders-help = הספרייה אינה מחפשת בתיקיות בשמות האלה, מופרדים בפסיקים. תיקיות מוסתרות תמיד מדולגות.
settings-library-skip-marked = דילוג על תיקיות המכילות
settings-library-skip-marked-help = וגם לא בתיקיות המכילות אחד מאלה: ‎.git‎ לעותקי קוד, ‎pyvenv.cfg‎ לסביבות Python, ‎CACHEDIR.TAG‎ למטמונים. תיקיית המיקום עצמה אינה מדולגת לעולם.
settings-library-show-timeline = ציר זמן
settings-library-show-timeline-help = מציג מעל רשת הספרייה כמה תמונות יש בכל שנה, חודש או יום.
