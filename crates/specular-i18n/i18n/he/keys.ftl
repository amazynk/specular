# Specular — Settings → Keys, the key binding editor (DESIGN.md §11). machine-drafted, needs native review
# Key names inside messages ($keys) are never translated.

settings-keys-sequence-timeout = המתנה למקש הבא
settings-keys-sequence-timeout-help = כמה זמן רצף מקשים כמו G G ממתין למקש הבא. מקש שמתחיל גם רצף ארוך יותר מופעל אחרי ההשהיה הזו.
settings-keys-bindings = קיצורי מקשים
settings-keys-bindings-help = בחרו פעולה ולחצו על המקשים לשיוך. לכל מצב מקשים משלו; מצב מיון ותמונות רפואיות משתמשים אחרת במקשי המציג. כדי לשייך את Esc, הקישו עליו; החזקה מבטלת. הקיצורים נשמרים ב־keymap.toml.
palette-customise-keys = התאמת מקשים…
keys-mode-viewer = מציג
keys-mode-cull = מצב מיון
keys-mode-medical = תמונות רפואיות
keys-modes = מצב
keys-presets = הגדרה קבועה
keys-preset-default = ברירת מחדל
keys-preset-vim = Vim
keys-preset-culling = מיון בסגנון Lightroom
keys-preset-photo-mechanic = בסגנון Photo Mechanic
action-pan-left = הזזה שמאלה
action-pan-right = הזזה ימינה
action-pan-up = הזזה למעלה
action-pan-down = הזזה למטה
action-pan-page-left = הזזה במסך שמאלה
action-pan-page-right = הזזה במסך ימינה
action-pan-page-up = הזזה במסך למעלה
action-pan-page-down = הזזה במסך למטה
action-reload = טעינה מחדש של התמונה
keys-action-cull-pick = בחירה
keys-action-cull-reject = דחייה
keys-action-cull-unflag = ניקוי הסימון
keys-action-cull-rate = { $stars ->
    [0] ניקוי הדירוג
    [one] דירוג בכוכב אחד
    [two] דירוג ב־{ $stars } כוכבים
   *[other] דירוג ב־{ $stars } כוכבים
}
keys-action-cull-label = תווית צבע: { $label }
keys-action-cull-copy-to = העתקה לתיקייה { $slot }
keys-action-cull-move-to = העברה לתיקייה { $slot }
keys-import = ייבוא…
keys-export = ייצוא…
keys-export-mode = ייצוא { $mode }…
keys-reset-mode = איפוס { $mode }
keys-reset-all = איפוס כל המקשים
keys-add = הוספת מקש עבור { $action }
keys-change = שינוי { $keys }
keys-remove = הסרת { $keys }
keys-reset-action = איפוס { $action }
keys-none = אין מקשים
keys-changed = שונה
keys-press = לחצו על מקשים…
keys-recording-hint = החזיקו Esc לביטול
keys-cancel-recording = ביטול
keys-no-match = אין פעולה או מקש שמתאימים ל־„{ $query }”.
keys-list = פעולות והמקשים שלהן
keys-confirm-preset = להשתמש בהגדרה הקבועה { $preset }? היא מחליפה את כל הקיצורים שלך.
keys-confirm-reset-mode = לאפס את כל המקשים של { $mode } להגדרה הקבועה?
keys-confirm-reset-all = לאפס את כל המקשים של כל המצבים להגדרה הקבועה?
keys-confirm-ok = החלפה
keys-confirm-reset = איפוס
keys-confirm-cancel = ביטול
keys-conflict-same = { $keys } כבר מפעיל את „{ $other }”.
keys-conflict-prefix = { $keys } ו־{ $sequence } („{ $other }”) מתחילים אותו דבר: הקצר מופעל רק אחרי השהיה.
keys-conflict-shadow = { $keys } מפעיל את „{ $other }” ב{ $mode }.
keys-conflict-reserved = { $keys } עלול לא להגיע אף פעם ל־Specular: { $reason }
keys-choice-replace = החלפה
keys-choice-swap = החלפה הדדית
keys-choice-keep = שמירת שניהם
keys-choice-anyway = לשייך בכל זאת
keys-choice-cancel = ביטול
keys-reserved-switch = הוא מחליף חלונות.
keys-reserved-close = הוא סוגר את החלון.
keys-reserved-secure = המערכת תופסת אותו.
keys-reserved-desktop = קיצורי Super שייכים לשולחן העבודה.
keys-reserved-workspaces = הוא מחליף מסופים או סביבות עבודה.
keys-reserved-input = הוא מחליף את שיטת הקלט או את פריסת המקלדת.
keys-reserved-macos = macOS תופסת אותו.
keys-announce-recording = מקליט מקשים עבור { $action }. לחצו על המקשים או החזיקו Esc לביטול.
keys-announce-key = { $keys }
keys-announce-bound = { $keys } מפעיל כעת את { $action }.
keys-announce-already = { $keys } כבר מפעיל את { $action }.
keys-announce-cancelled = ההקלטה בוטלה.
keys-announce-removed = { $keys } הוסר מ־{ $action }.
keys-announce-reset-action = ל־{ $action } שוב יש את מקשי ההגדרה הקבועה.
keys-announce-reset-mode = המקשים של { $mode } אופסו.
keys-announce-reset-all = כל המקשים אופסו.
keys-announce-preset = נעשה כעת שימוש בהגדרה הקבועה { $preset }.
keys-toast-imported = הקיצורים יובאו
keys-toast-exported = הקיצורים יוצאו
keys-toast-reloaded = keymap.toml השתנה: הקיצורים נטענו מחדש
keys-toast-error = לא ניתן לקרוא את הקיצורים: { $error }
keys-toast-write-error = לא ניתן לשמור את keymap.toml: { $error }
keys-pending = { $keys } …  { $next }
keys-problem-syntax = לא ניתן לקרוא את keymap.toml: { $error }
keys-problem-key = המקש הלא מוכר „{ $key }” הושמט.
keys-problem-action = „{ $key }” מציין פעולה לא מוכרת „{ $action }”; הושמט.
keys-problem-mode = הטבלה [{ $mode }] אינה מצב; הושמטה.
keys-problem-duplicate = { $keys } מופיע פעמיים ב{ $mode }; השני בתוקף.
keys-problem-prefix = { $keys } מתחיל את { $longer } ב{ $mode }, ולכן מופעל רק אחרי השהיה.
keys-problem-preset = הגדרה קבועה לא מוכרת „{ $preset }”; מקשי ברירת המחדל בתוקף.

## keymap.toml editor (machine-drafted, needs native review)

keys-edit-file = עריכת keymap.toml
keys-problem-action-suggestion = „{ $key }” מציין פעולה לא מוכרת „{ $action }”; הושמט. האם התכוונת ל־„{ $suggestion }”?
keys-problem-mode-suggestion = הטבלה [{ $mode }] אינה מצב; הושמטה. האם התכוונת ל־[{ $suggestion }]?
keys-problem-preset-suggestion = הגדרה קבועה לא מוכרת „{ $preset }”; מקשי ברירת המחדל בתוקף. האם התכוונת ל־„{ $suggestion }”?
