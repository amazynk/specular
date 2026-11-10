# Specular — Medical analysis: segmentations, SUV, fusion, TMTV, time-intensity curves. עברית. machine-drafted, needs native review

palette-segments = הצגה או הסתרה של סגמנטציות
palette-segment-opacity = אטימות המילוי הבאה של הסגמנטציות
palette-fusion = מיזוג הסדרות של אותה מערכת ייחוס
palette-fusion-colormap = מפת הצבעים הבאה למיזוג
palette-fusion-blend = אטימות המיזוג הבאה
palette-fusion-nudge = הזזה ידנית של הסדרה הממוזגת
palette-tmtv = כלי TMTV (כדור, תיבה, כבוי)
palette-tmtv-threshold = סף TMTV (SUV { NUMBER(2.5) } או { PERCENT(41) } מ-SUVmax)
palette-time-curve = עקומת זמן-עוצמה של האזור האחרון
palette-export-curve = שמירת עקומת זמן-עוצמה כ-CSV

analysis-searching = עדיין מחפש סגמנטציות וסדרות למיזוג
segments-none = לא נמצאה סגמנטציה של מערכת ייחוס זו
segments-shown = הסגמנטציות מוצגות
segments-hidden = הסגמנטציות מוסתרות
segments-fill = מילוי סגמנטציות { PERCENT($percent) }
segments-failed = הסגמנטציה לא הוצגה: { $reason }
segment-entry = { $name } · { $ml } mL

suv-value = SUVbw { $value }
suv-factor = מקדם SUVbw { $factor }, { $minutes } min לאחר ההזרקה
suv-refused = אין SUV: { $reason }
suv-preset = SUV 0–{ $upper }
suv-refused-not-pet = אינה תמונת PET
suv-refused-units = יחידות הפיקסלים אינן Bq/mL
suv-refused-corrected = לא תוקנה להנחתה ולדעיכה
suv-refused-decay = תיקון דעיכה שאינו נתמך
suv-refused-weight = אין משקל מטופל
suv-refused-dose = אין מנה מוזרקת
suv-refused-half-life = אין זמן מחצית חיים של הרדיונוקליד
suv-refused-injection = אין שעת הזרקה
suv-refused-scan-time = אין שעת רכישה
suv-refused-delay = ההזרקה מאוחרת לסריקה
suv-refused-scale = אין מקדם קנה מידה SUV של Philips
suv-refused-height = אין גובה מטופל
suv-refused-sex = אין מין מטופל
suv-refused-lbm = מסת גוף רזה מחוץ לטווח

colormap-hot-iron = ברזל לוהט
colormap-rainbow = קשת
colormap-viridis = Viridis
fusion-none = אין סדרת PET,‏ SPECT,‏ CT או MR של מערכת ייחוס זו למיזוג
fusion-needed = יש למזג סדרה תחילה (Shift+U)
fusion-opening = ממזג { $modality }
fusion-off = המיזוג כבוי
fusion-refused = לא ניתן למזג: { $reason }
fusion-on = { $modality } ממוזג: { $colormap }, { PERCENT($blend) }
fusion-hidden = { $modality } הממוזג מוסתר
fusion-window-suv = חלון ממוזג SUV { $low }–{ $high }
fusion-value = { $modality } { $value }
fusion-value-suv = { $modality } SUVbw { $value }
fusion-blend-toast = אטימות מיזוג { PERCENT($percent) }
fusion-moved = הוזז ידנית: { $x }, { $y }, { $z } מ״מ, { $degrees }°
fusion-nudge-hint = החצים מזיזים את הסדרה הממוזגת ב-1 מ״מ,‏ Shift+Up/Down לאורך התצוגה, [ ו-] מסובבים אותה
fusion-nudge-off = הסדרה הממוזגת קובעה

tmtv-hint-sphere = TMTV: יש לגרור כדור ממרכזו
tmtv-hint-box = TMTV: יש לגרור תיבה
tmtv-off = TMTV כבוי
tmtv-threshold-suv = סף SUV { $suv }
tmtv-threshold-share = סף { PERCENT($share) } מ-SUVmax
tmtv-result = TMTV { $ml } mL { $count ->
        [one] באזור אחד
        [two] בשני אזורים
       *[other] ב-{ $count } אזורים
    }
tmtv-stats = SUVmax { $max }, SUVmean { $mean }, TLG { $tlg }
tmtv-outside = האזור מחוץ לנפח
tmtv-no-pet = TMTV דורש נפח PET עם SUV

analysis-mpr-only = מיזוג ו-TMTV פועלים ב-MPR (Shift+R)
curve-not-dynamic = אינה תמונה מרובת פריימים או סדרה דינמית
curve-not-mpr = עקומות זמן-עוצמה משורטטות על התמונה, מחוץ ל-MPR
curve-no-roi = יש לשרטט אליפסה או מלבן תחילה
curve-loading = { $count ->
        [one] קורא נקודת זמן אחת…
        [two] קורא שתי נקודות זמן…
       *[other] קורא { $count } נקודות זמן…
    }
curve-title = עקומת זמן-עוצמה
curve-mean = ממוצע
curve-max = מקסימום
curve-seconds = s
curve-saved = העקומה נשמרה
curve-none = אין עקומה לשמירה
curve-failed = העקומה לא נוצרה: { $reason }
