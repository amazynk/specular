# Integration: open with, user commands, paste (Stage 2).
palette-open-with = פתיחה באמצעות…
palette-paste = הדבקה
open-with-placeholder = פתיחה באמצעות…
open-with-loading = מחפש יישומים…
open-with-default = { $name } (ברירת מחדל)
open-with-choose-other = בחירת יישום אחר…
open-with-choose-title = בחירת יישום
open-with-failed = לא ניתן לפתוח באמצעות { $name }: { $reason }
command-missing = { $program } לא נמצא
command-failed = לא ניתן להפעיל את { $name }: { $reason }
command-exited = { $name } הסתיים עם שגיאה: { $status }
paste-nothing = אין בלוח הגזירים דבר לפתיחה
paste-failed = לא ניתן להדביק את התמונה: { $reason }
drag-failed = לא ניתן לגרור את הקובץ: { $reason }
about-storage = הגדרות: { $settings } · מטמון: { $cache }
about-storage-portable = מצב נייד: ההגדרות והמטמון ב־{ $dir }

## Scripting and recipes

palette-copy-view-command = העתקת התצוגה כפקודת מעטפת
palette-copy-view-python = העתקת התצוגה כ־Python
toast-copied-view-command = הפקודה הועתקה
script-copy-no-image = אין תמונה לתיאור
recipe-confirm-title = להריץ את המתכון „{ $name }” בפעם הראשונה? הוא מריץ: { $command }
recipe-confirm-run = הרצה
recipe-no-file = המתכון צריך קובץ תמונה
recipe-running = { $name } רץ…
recipe-done = { $name }: { $shapes ->
    [one] נוספה צורה אחת
    [two] נוספו { $shapes } צורות
   *[other] נוספו { $shapes } צורות
}
recipe-failed = { $name } נכשל: { $reason }
recipe-timeout = נעצר אחרי { $seconds } שנ׳
recipe-no-output = { $name } לא כתב { $file }
