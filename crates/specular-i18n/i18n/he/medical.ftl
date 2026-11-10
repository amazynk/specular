# Specular — Medical images (DICOM). machine-drafted, needs native review

cheatsheet-medical = תמונות רפואיות
palette-window-level = כלי חלון/רמה (גרירה בשמאלי)
palette-window-default = חלון: ברירת המחדל של הקובץ
palette-window-lung = חלון: ריאות (W 1500, L −600)
palette-window-mediastinum = חלון: מיצר (W 350, L 50)
palette-window-bone = חלון: עצם (W 1800, L 400)
palette-window-brain = חלון: מוח (W 80, L 40)
palette-window-stroke = חלון: שבץ (W 40, L 40)
palette-window-soft-tissue = חלון: רקמות רכות (W 400, L 50)
palette-window-liver = חלון: כבד (W 150, L 30)
palette-window-subdural = חלון: תת־קשיתי (W 200, L 75)
palette-window-full-range = חלון: הטווח המלא
palette-invert-grey = היפוך גווני אפור
palette-next-slice = הפריים הבא
palette-prev-slice = הפריים הקודם
disabled-not-medical = רק לתמונות רפואיות בגווני אפור
disabled-one-frame = בתמונה יש פריים אחד
settings-section-medical = תמונות רפואיות
settings-medical-keep-window = שמירת חלון/רמה לסדרה הבאה
settings-medical-keep-window-help = שמירת החלון שהגדרתם במעבר לסדרה אחרת, במקום להתחיל מברירת המחדל שלה. פריימים של קובץ אחד ופרוסות של סדרה אחת תמיד חולקים אותו.
medical-window = C { $center }  W { $width }
medical-inverted = הפוך
medical-frame = פריים { $frame } מתוך { $count }
medical-tool = כלי חלון/רמה: גררו
medical-tool-on = כלי חלון/רמה פעיל: גררו כדי לכוונן
medical-tool-off = כלי חלון/רמה כבוי
medical-preset-not-ct = הגדרות CT דורשות ערכים ביחידות הונספילד
medical-file-window-none = אין חלון { $index } בקובץ זה
medical-file-window-unnamed = חלון הקובץ { $index } מתוך { $count }
medical-file-window = חלון הקובץ { $index } מתוך { $count }: { $name }
measure-templates-need-medical = תבניות גאומטריה זמינות עם הפרופיל הרפואי
medical-off = אין כלים רפואיים לתמונה זו: { $reason }
medical-readout = ערך וחלון

## Measurements, cine and overlays (machine-drafted, needs native review)

palette-cine = הפעלת תמונות (סינה)
palette-measure-length = מדידת אורך
palette-measure-angle = מדידת זווית
palette-measure-ellipse = מדידת אזור אליפטי
palette-measure-rectangle = מדידת אזור מלבני
palette-clear-measurements = ניקוי המדידות
palette-measure-templates = תבניות גאומטריה…
palette-overlays = הצגה או הסתרה של שכבות־על
medical-cine = מתנגן ב־{ $rate } תמונות לשנייה
medical-overlays-shown = שכבות־העל מוצגות
medical-overlays-hidden = שכבות־העל מוסתרות
measure-hint-length = אורך: יש לגרור או ללחוץ על שני הקצוות
measure-hint-angle = זווית: יש ללחוץ על שלוש נקודות, הקודקוד שני
measure-hint-ellipse = אליפסה: יש לגרור לאורך האזור
measure-hint-rectangle = מלבן: יש לגרור לאורך האזור
measure-off = כלי המדידה כבוי
measure-cleared = המדידות נוקו
measure-length-mm = { $value } מ״מ
# machine-drafted, needs native review
measure-length-angstrom = { $value } Å
measure-length-px = { $value } px
measure-area-mm = שטח { $value } ממ״ר
measure-area-px = שטח { $value } px²
measure-angle = { $value }°
measure-mean = ממוצע { $mean } ± { $sd } { $unit }
measure-range = מינ׳ { $min }  מקס׳ { $max }  ({ $count } px)
measure-caveat-detector = בגלאי: ללא תיקון הגדלה
measure-caveat-magnification = תיקון הגדלה משוער
measure-caveat-uncalibrated = אין מרווח פיקסלים: בפיקסלים
measure-caveat-outside-region = מחוץ לאזור אולטרסאונד מכויל: בפיקסלים

## DICOM orientation labels, presentation states and wheel stepping (machine-drafted, needs native review)
settings-medical-orientation-labels = הצגת תוויות כיוון
settings-medical-orientation-labels-help = אותיות בשולי התמונה לכיווני המטופל (R/L ימין/שמאל, A/P קדמי/אחורי, S/I ראש/רגליים), הנלקחות מהקובץ ועוקבות אחר סיבוב והיפוך. L מציג או מסתיר אותן לאורך ההפעלה.
settings-medical-wheel-steps-frames = גלגלת העכבר עוברת בין פריימים
settings-medical-wheel-steps-frames-help = בקובץ או בסדרה עם כמה פריימים הגלגלת עוברת בין הפריימים במקום לבצע זום או להחליף תמונה. Ctrl+גלגלת עדיין מבצע זום.
# machine-drafted, needs native review
settings-medical-display-function = תצוגת אפור
settings-medical-display-function-help = האופן שבו מוצגים גווני האפור של תמונות רפואיות. DICOM GSDF (פונקציית תצוגה סטנדרטית לגווני אפור) מפזרת אותם בצעדים שווים בנראוּת בין עוצמת ההארה של השחור והלבן של המסך (DICOM PS3.14), כמו במסך אבחוני מכויל. פרופיל הצבע של המסך עדיין חל.
settings-medical-display-function-srgb = רגיל (sRGB)
settings-medical-display-function-gsdf = DICOM GSDF
settings-medical-display-black = עוצמת הארה של השחור במסך
settings-medical-display-black-help = עוצמת ההארה של השחור במסך, כולל אור הסביבה, ממדידה או מגיליון הנתונים של המסך. משמשת את DICOM GSDF.
settings-medical-display-white = עוצמת הארה של הלבן במסך
settings-medical-display-white-help = עוצמת ההארה של הלבן במסך, ממדידה או מגיליון הנתונים של המסך. משמשת את DICOM GSDF.
palette-orientation-labels = הצגה או הסתרה של תוויות הכיוון
palette-presentation-state = מצב התצוגה הבא
disabled-no-presentation-state = אין מצב תצוגה שמפנה לתמונה זו
medical-labels-shown = תוויות הכיוון מוצגות
medical-labels-hidden = תוויות הכיוון מוסתרות
medical-presentation = מצב תצוגה: { $label } ({ $index } מתוך { $count })
medical-presentation-none = מצב תצוגה כבוי
medical-presentation-readout = תצוגה: { $label }
medical-preset-presentation = טבלת החיפוש של מצב התצוגה קובעת את רמות האפור

## Orientation labels for screen readers (machine-drafted, needs native review)
medical-orientation-a11y = כיוון: שמאל { $left }, ימין { $right }, למעלה { $top }, למטה { $bottom }
