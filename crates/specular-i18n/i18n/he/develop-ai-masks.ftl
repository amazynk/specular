# machine-drafted, needs native review
# Specular — Develop's AI masks: subject, sky and object-at-click rows, and the
# Masks feature in Settings › Local AI.

develop-mask-kind-subject = נושא
develop-mask-kind-sky = שמיים
develop-mask-kind-object = עצם בלחיצה
# An AI mask row while its network runs. $kind: subject, sky, object.
develop-ai-computing = { $kind ->
        [subject] מחפש את הנושא…
        [sky] מחפש את השמיים…
       *[object] מחפש את העצם…
    }
develop-ai-ready = מוכן
# An object row with its points ($n; a box counts as two).
develop-ai-object-clicks = נקודות: { $n }
develop-ai-click-help =
    לחצו על העצם.
    לחיצה עם Alt על מה שאינו חלק ממנו.
    גרירה מציירת מסגרת.
# The row's model is not installed. $model is its name ("IS-Net").
develop-ai-needs =
    דרושה התוספת { $model }.
    עד אז לא מסוכן דבר.
develop-ai-failed = לא ניתן ליצור את המסכה: { $error }
develop-ai-failed-worker = תהליך מסכות הבינה המלאכותית נעצר.
develop-ai-open-settings = פתיחת בינה מלאכותית מקומית…
develop-ai-clear = ניקוי נקודות
develop-ai-too-many = לכל היותר { $n } נקודות (מסגרת נחשבת לשתיים).
develop-ai-box-set = המסגרת נקבעה
develop-ai-point-added = נקודה נוספה
develop-ai-point-removed = נקודה נוספה מחוץ לעצם
develop-ai-cleared = הנקודות נוקו
# Screen-reader names of the points on the picture. $n counts from 1.
develop-ai-point-on = נקודה { $n }, על העצם
develop-ai-point-off = נקודה { $n }, מחוץ לעצם

## Settings › Local AI: the Masks feature
settings-ai-masks = מסכות בינה מלאכותית
settings-ai-masks-help = מסכות נושא, שמיים ועצם בפיתוח. מודל העצמים מגיע עם התכונה; נושא ושמיים הם תוספות.
settings-ai-masks-size = מודל
settings-ai-masks-size-help = EfficientSAM Ti מהיר יותר. EfficientSAM S עוקב אחר קצוות במדויק יותר.
settings-ai-masks-size-recommended = ברירת מחדל
settings-ai-masks-size-small = EfficientSAM Ti
settings-ai-masks-size-standard = EfficientSAM S
settings-ai-masks-size-large = גדול
