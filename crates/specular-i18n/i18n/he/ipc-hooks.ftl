# machine-drafted, needs native review
# Remote control and events (Settings → Integration), and hooks: programs settings.toml
# names to run when something happens in the viewer.

settings-integration-ipc = שליטה מרחוק ואירועים
settings-integration-ipc-help = תוכניות אחרות במחשב הזה, שרצות תחת המשתמש שלך, יכולות לשלוט ב־{ -app-name } ולשמוע מה קורה בו דרך שקע פרטי (specular ipc). שום דבר לא מאזין ברשת.

## The hooks panel under "Remote control and events".
hooks-panel-title = הוקים
hooks-panel-intro = תוכניות ש־settings.toml מפעיל כשמשהו קורה. כל אחת שואלת פעם אחת לפני ההפעלה הראשונה.
hooks-panel-none = אין הוקים. אפשר להוסיף אותם כ־[[hooks]] ב־settings.toml.
hooks-panel-keywords = הוקים hooks אירועים אוטומציה סקריפטים תוכניות
# $events: the event names, joined ("marked, exported").
hooks-panel-on = כאשר { $events }
hooks-panel-untrusted = שואל לפני ההפעלה הראשונה
hooks-panel-allowed = מותר
hooks-panel-remove = הסרה…
hooks-panel-unused = לא בשימוש

## Asking before a hook's first run (and again when its program or arguments change).
# $name: the hook's name; $program: the program; $events: what sets it off (hook-event-*,
# joined with hook-or).
hook-consent-title = להפעיל את „{ $name }” ({ $program }) כאשר { $events }?
hook-consent-detail = התוכנית תופעל עם שמות התמונות: { $args }
hook-consent-trust = יש לאשר רק תוכניות שסומכים עליהן. השאלה תחזור אם ההוק ישתנה.
hook-consent-allow = אישור
hook-consent-not-now = לא עכשיו
hook-or = { " " }או{ " " }
hook-event-opened = תמונה נפתחת
hook-event-marked = תמונות מסומנות
hook-event-keywords = מילות מפתח משתנות
hook-event-exported = תמונות מיוצאות
hook-event-imported = תמונות מיובאות
hook-event-captured = מצלמה שולחת תמונה
hook-event-develop-saved = עריכה בפיתוח נשמרת
hook-event-any = משהו קורה

## Removing a hook (Settings › Integration › Hooks › Remove…).
hook-remove-title = להסיר את ההוק „{ $name }”?
hook-remove-detail = הוא יוצא מ־settings.toml. התוכנית והקבצים שלך נשארים בדיסק.
hook-remove-ok = הסרה
hook-removed = ההוק „{ $name }” הוסר.
hook-settings-unwritable = לא ניתן לשנות את settings.toml: { $reason }

## A hook that did not work (a quiet note; the log has the details).
hook-failed = ההוק „{ $name }” לא עבד: { $reason }
hook-failed-exit = הוא הסתיים עם קוד { $code }
hook-failed-timeout = הוא נעצר אחרי { $seconds } שנ׳
hook-failed-start = לא ניתן היה להפעיל אותו ({ $reason })
hook-dropped = ההוק „{ $name }” עסוק: { $count ->
    [one] אירוע אחד דולג.
    [two] שני אירועים דולגו.
   *[other] { $count } אירועים דולגו.
}

## An export started by another program (specular ipc export): nobody was asked.
export-recipes-remote-exists = { $count ->
    [one] קיים קובץ שהוא היה מחליף: לא יוצא דבר
    [two] קיימים שני קבצים שהוא היה מחליף: לא יוצא דבר
   *[other] קיימים { $count } קבצים שהוא היה מחליף: לא יוצא דבר
}
export-recipes-remote-skipped = { $file } קיים
