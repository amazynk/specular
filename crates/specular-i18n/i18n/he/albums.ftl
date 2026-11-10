# machine-drafted, needs native review
# Specular — Library albums, smart albums and the date timeline.

## Sidebar
albums-title = אלבומים
albums-new = אלבום חדש…
albums-new-smart = אלבום חכם חדש…
albums-save-search = שמירת החיפוש כאלבום חכם…
albums-smart = אלבום חכם
# The album the photos go to when added without choosing.
albums-target = לכאן מתווספות התמונות
albums-group-count = { $count ->
    [one] אלבום אחד
    [two] שני אלבומים
   *[other] { $count } אלבומים
}
albums-need-library = האלבומים נמצאים בספרייה. יש להוסיף קודם את התיקייה הזו לספרייה.

## Screen readers: the buttons of album and group rows
albums-a11y-rename = שינוי השם של { $album }
albums-a11y-edit = עריכת { $album }
albums-a11y-delete = מחיקת האלבום { $album }
albums-a11y-rename-group = שינוי השם של הקבוצה { $group }
albums-a11y-delete-group = מחיקת הקבוצה { $group }

## Sheets
albums-name = שם
albums-unnamed = עדיין אין שם
albums-new-title = אלבום חדש
albums-new-smart-title = אלבום חכם חדש
albums-edit-smart-title = עריכת אלבום חכם
albums-rename-title = שינוי שם האלבום
albums-rename-group-title = שינוי שם הקבוצה
albums-create = יצירה
albums-rename = שינוי שם
albums-save = שמירה
albums-groups-note = אפשר להשתמש ב־/ כדי לשים אותו בקבוצה: לקוח X / נבחרות.
albums-name-empty = יש להקליד שם.
albums-name-taken = { $album } כבר קיים.
albums-name-clash = { $album } הוא אלבום, ולכן הוא לא יכול להיות גם קבוצה.

## Smart album rules
albums-rules = כללים
albums-rules-note = הכללים חלים מלמעלה למטה. כללים ריקים לא נלקחים בחשבון.
albums-rules-none = יש להוסיף לפחות כלל אחד.
albums-rule-label = כלל { $number }
albums-rule-empty = כלל ריק
albums-rule-add = הוספת כלל
albums-rule-combine = שילוב עם הכללים שלמעלה
albums-rule-first = תמונות שתואמות
albums-rule-and = וגם
albums-rule-or = או
albums-rule-except = חוץ מ

## Adding and removing photos
albums-add-menu = הוספה לאלבום…
albums-remove-menu = הסרה מהאלבום
albums-add-title = { $count ->
    [one] הוספת תמונה אחת לאלבום
    [two] הוספת שתי תמונות לאלבום
   *[other] הוספת { $count } תמונות לאלבום
}
albums-add = הוספה
albums-add-new = אלבום חדש
albums-add-list = אלבומים
albums-add-none = עדיין אין אלבומים. אפשר להקליד שם כדי ליצור אחד.
albums-add-note = או להקליד שם לאלבום חדש.
albums-added = { $count ->
    [one] נוספה תמונה אחת ל־{ $album }.
    [two] נוספו שתי תמונות ל־{ $album }.
   *[other] נוספו { $count } תמונות ל־{ $album }.
}
albums-added-missing = { $count ->
    [one] תמונה אחת לא נמצאת בספרייה, ולכן לא נוספה.
    [two] שתי תמונות לא נמצאות בספרייה, ולכן לא נוספו.
   *[other] { $count } תמונות לא נמצאות בספרייה, ולכן לא נוספו.
}
albums-created = { $album } נוצר.
albums-removed = { $count ->
    [one] הוסרה תמונה אחת מ־{ $album }.
    [two] הוסרו שתי תמונות מ־{ $album }.
   *[other] הוסרו { $count } תמונות מ־{ $album }.
}
albums-remove = הסרה
albums-remove-confirm = { $count ->
    [one] להסיר תמונה אחת מ־{ $album }?
    [two] להסיר שתי תמונות מ־{ $album }?
   *[other] להסיר { $count } תמונות מ־{ $album }?
}
albums-remove-confirm-detail = הקבצים נשארים בדיסק ובספרייה.
albums-delete = מחיקה
albums-delete-confirm = למחוק את האלבום { $album }?
albums-delete-group-confirm = { $count ->
    [one] למחוק את הקבוצה { $group } ואת האלבום שבה?
    [two] למחוק את הקבוצה { $group } ואת שני האלבומים שבה?
   *[other] למחוק את הקבוצה { $group } ואת { $count } האלבומים שבה?
}
albums-delete-confirm-detail = התמונות נשארות בדיסק ובספרייה.
albums-deleted = { $count ->
    [one] האלבום נמחק.
    [two] שני אלבומים נמחקו.
   *[other] { $count } אלבומים נמחקו.
}
albums-failed = לא ניתן היה לשנות את האלבומים: { $reason }

## Date timeline
albums-timeline = ציר זמן
albums-timeline-description = בחירת תקופה מציגה רק את התמונות שלה. Shift+לחיצה בוחרת רצף של תקופות.
albums-timeline-picked = מוצג: { $period }
albums-timeline-cleared = מוצגים כל התאריכים
