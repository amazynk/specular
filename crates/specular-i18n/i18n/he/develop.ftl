# machine-drafted, needs native review
# Specular — Develop (non-destructive photo editing). עברית.

# Shown once when an edit opens and some of its modules were made with a version that no
# longer renders: they were moved to the nearest version that does. $count: modules moved.
# machine-drafted, needs native review
toast-develop-edit-migrated = { $count ->
    [one] מודול אחד בעריכה הזו עודכן לגרסה חדשה יותר. ייתכן שהתוצאה תיראה מעט שונה.
    [two] שני מודולים בעריכה הזו עודכנו לגרסה חדשה יותר. ייתכן שהתוצאה תיראה מעט שונה.
   *[other] { $count } מודולים בעריכה הזו עודכנו לגרסה חדשה יותר. ייתכן שהתוצאה תיראה מעט שונה.
}
# Shown when an edit holds modules this version of Specular cannot render (made with a newer
# Specular). They are kept and saved unchanged, but skipped. $count: modules skipped.
# machine-drafted, needs native review
toast-develop-edit-newer = { $count ->
    [one] מודול אחד בעריכה הזו דורש גרסת Specular חדשה יותר. הוא נשמר אך אינו מוצג.
    [two] שני מודולים בעריכה הזו דורשים גרסת Specular חדשה יותר. הם נשמרים אך אינם מוצגים.
   *[other] { $count } מודולים בעריכה הזו דורשים גרסת Specular חדשה יותר. הם נשמרים אך אינם מוצגים.
}

## Develop's controls (sliders, curve, colour wheels). Numbers come formatted in the UI
## language's digits.

# machine-drafted, needs native review
# A slider's value in exposure values. $value: "+0.70".
develop-value-ev = { $value } EV
# machine-drafted, needs native review
# machine-drafted, needs native review
# An angle. $value: "2.5".
develop-value-degrees = { $value }°
# machine-drafted, needs native review
# A colour temperature. $value: "5,000".
develop-value-kelvin = { $value } K
# machine-drafted, needs native review
# The curve editor's selected point. $index, $count: point numbers; $input, $output: "25 %".
develop-curve-point = נקודה { $index } מתוך { $count }: קלט { $input }, פלט { $output }
# machine-drafted, needs native review
# The curve editor's keys, read by screen readers.
develop-curve-keys = מקשי החצים מזיזים את הנקודה. Page Up ו־Page Down בוחרים נקודה אחרת. Insert מוסיף נקודה, Delete מסיר אותה.
# machine-drafted, needs native review
# A colour wheel with no colour.
develop-wheel-neutral = ניטרלי
# machine-drafted, needs native review
# A colour wheel's colour. $hue: degrees; $strength: "20 %".
develop-wheel-value = גוון { $hue }°, עוצמה { $strength }
# machine-drafted, needs native review
# A colour wheel's keys, read by screen readers.
develop-wheel-keys = מקשי החצים מזיזים את הצבע. Delete מאפס אותו.

## Develop's panel.

# machine-drafted, needs native review
develop-title = פיתוח
# machine-drafted, needs native review
develop-undo = ביטול
# machine-drafted, needs native review
develop-redo = ביצוע מחדש
# machine-drafted, needs native review
develop-before-after = לפני ואחרי
# machine-drafted, needs native review
develop-close = יציאה מפיתוח
# machine-drafted, needs native review
develop-history = היסטוריה
# machine-drafted, needs native review
# A card's on/off switch. $module: the module's name.
develop-module-on = { $module } פועל
develop-card-menu = אפשרויות { $module }
develop-card-changed = שונה
# machine-drafted, needs native review
# The speed-edit readout over the image. $control: "Exposure"; $value: "+0.70 EV".
develop-speed-hud = { $control }: { $value }
# machine-drafted, needs native review
develop-filmstrip = רצועת תמונות

## Develop's module cards and controls.

# machine-drafted, needs native review
develop-module-exposure = חשיפה
develop-module-light = תאורה
develop-look-profile = פרופיל
develop-look-standard = רגיל
develop-look-film = פילם
develop-look-natural = טבעי
develop-module-grading = דירוג צבע
develop-add-tool = הוספת כלי
# machine-drafted, needs native review
develop-module-white-balance = איזון לבן
# machine-drafted, needs native review
develop-module-tone = טונים
# machine-drafted, needs native review
develop-module-contrast = ניגודיות וטווח
# machine-drafted, needs native review
develop-module-colour = צבע
# machine-drafted, needs native review
develop-module-curve = עקומה
# machine-drafted, needs native review
develop-module-crop = חיתוך ויישור
# machine-drafted, needs native review
# A module this version has no card for.
develop-module-other = מודול אחר
# machine-drafted, needs native review
develop-exposure = חשיפה
# machine-drafted, needs native review
develop-black-level = רמת שחור
# machine-drafted, needs native review
develop-temperature = טמפרטורה
# machine-drafted, needs native review
develop-tint = גוון
# machine-drafted, needs native review
develop-highlights = הבהרות
# machine-drafted, needs native review
develop-shadows = צללים
# machine-drafted, needs native review
develop-contrast = ניגודיות
# machine-drafted, needs native review
develop-whites = לבנים
# machine-drafted, needs native review
develop-blacks = שחורים
# machine-drafted, needs native review
# The view transform: how scene brightness becomes screen brightness.
# machine-drafted, needs native review
# machine-drafted, needs native review
# machine-drafted, needs native review
# machine-drafted, needs native review
# HDR card: how far above SDR white the HDR picture may go, and which picture to show.
develop-hdr = HDR
# machine-drafted, needs native review
develop-hdr-limit = מגבלת HDR
# machine-drafted, needs native review
develop-hdr-preview = הצג
# machine-drafted, needs native review
develop-hdr-preview-hdr = HDR
# machine-drafted, needs native review
develop-hdr-preview-sdr = SDR
# machine-drafted, needs native review
develop-vibrance = חיוניות
# machine-drafted, needs native review
develop-saturation = רוויה
# machine-drafted, needs native review
develop-colour-wheels = גלגלי צבע
# machine-drafted, needs native review
develop-curve = עקומת טונים
# machine-drafted, needs native review
develop-module-local-contrast = ניגודיות מקומית
# machine-drafted, needs native review
# A switch: exposure set from the photo's own brightness.
develop-auto-exposure = חשיפה אוטומטית
# machine-drafted, needs native review
# How far the tone equalizer's highlights and shadows spread across an area.
develop-smoothing = החלקה
# machine-drafted, needs native review
# Local contrast: fine detail, and larger-scale contrast.
develop-texture = מרקם
# machine-drafted, needs native review
develop-clarity = צלילות
# machine-drafted, needs native review
# Levels on the curve card: the input that becomes black, the middle and white.
develop-levels-black = נקודת שחור
# machine-drafted, needs native review
develop-levels-mid = נקודת אמצע
# machine-drafted, needs native review
develop-levels-white = נקודת לבן
# machine-drafted, needs native review
develop-module-detail = פרטים
# machine-drafted, needs native review
develop-module-denoise = הפחתת רעש
# machine-drafted, needs native review
develop-module-sharpen = חידוד
# machine-drafted, needs native review
develop-noise-luminance = רעש בהירות
# machine-drafted, needs native review
develop-noise-detail = פרטי רעש
# machine-drafted, needs native review
develop-noise-colour = רעש צבע
# machine-drafted, needs native review
develop-sharpen-amount = עוצמת חידוד
# machine-drafted, needs native review
develop-sharpen-radius = רדיוס חידוד
# machine-drafted, needs native review
develop-sharpen-detail = פרטי חידוד
# machine-drafted, needs native review
develop-sharpen-masking = מיסוך
# machine-drafted, needs native review
develop-capture-amount = חידוד לכידה
# machine-drafted, needs native review
develop-capture-radius = רדיוס לכידה
# machine-drafted, needs native review
develop-straighten = יישור
# machine-drafted, needs native review
# History rows of the crop tool.
develop-crop = חיתוך
# machine-drafted, needs native review
develop-crop-turn = סיבוב
# machine-drafted, needs native review
develop-crop-flip = היפוך
# machine-drafted, needs native review
# The lens card: the camera's own corrections (its switch) and manual ones.
develop-module-lens = תיקוני עדשה
develop-module-optics = אופטיקה
# machine-drafted, needs native review
develop-lens-distortion = עיוות
# machine-drafted, needs native review
develop-lens-vignette = הצללת שוליים
# machine-drafted, needs native review
# Colour fringes at the edges of the frame (lateral chromatic aberration), per colour.
develop-lens-tca-red = שוליים אדומים
# machine-drafted, needs native review
develop-lens-tca-blue = שוליים כחולים
# machine-drafted, needs native review
# The Lensfun lens profile for raws without corrections of their own: a picker and a note.
develop-lensfun-profile = פרופיל עדשה
# machine-drafted, needs native review
# The picker's first option: the profile matched to the file's lens name.
develop-lensfun-auto = אוטומטי
# machine-drafted, needs native review
develop-lensfun-off = ללא
# machine-drafted, needs native review
# $lens: "Canon EF 24-70mm f/2.8L II USM". Lensfun is a name; keep it.
develop-lensfun-used = Lensfun: { $lens }
# machine-drafted, needs native review
develop-lensfun-none-chosen = אין פרופיל עדשה.
# machine-drafted, needs native review
# $lens: the lens name the file gives.
develop-lensfun-no-match = אין פרופיל Lensfun עבור { $lens }.
# machine-drafted, needs native review
develop-lensfun-no-profile = אין פרופיל Lensfun לעדשה הזו.
# machine-drafted, needs native review
develop-lensfun-no-name = הקובץ אינו מציין את שם העדשה. בחרו אותה למעלה.
# machine-drafted, needs native review
# $count: how many lenses match equally well (2 or more).
develop-lensfun-ambiguous = { $count } עדשות מתאימות. בחרו אחת למעלה.

## The crop tool over the photo (key R). Said to screen readers, or shown over the photo.

# machine-drafted, needs native review
develop-crop-open = חיתוך. יש לגרור את הקצוות, או Ctrl+גרירה לאורך קו שצריך להיות ישר. Enter שומר, Esc חוזר.
# machine-drafted, needs native review
develop-crop-kept = החיתוך נשמר
# machine-drafted, needs native review
develop-crop-cancelled = החיתוך בוטל
# machine-drafted, needs native review
develop-crop-reset = החיתוך והיישור אופסו
# machine-drafted, needs native review
# $angle: "-1.5".
develop-crop-straightened = יושר ל־{ $angle }°
# machine-drafted, needs native review
# $aspect: "3:2", or one of the two names below.
develop-crop-aspect = יחס גובה־רוחב { $aspect }
# machine-drafted, needs native review
develop-crop-aspect-free = חופשי
# machine-drafted, needs native review
develop-crop-aspect-original = מקורי
# machine-drafted, needs native review
develop-crop-turned-right = סובב ימינה
# machine-drafted, needs native review
develop-crop-turned-left = סובב שמאלה
# machine-drafted, needs native review
develop-crop-flipped-horizontal = הופך משמאל לימין
# machine-drafted, needs native review
develop-crop-flipped-vertical = הופך מלמעלה למטה
# machine-drafted, needs native review
# The label over the photo. $aspect: as above; $guide: one of the guide names below.
develop-crop-label = יחס גובה־רוחב { $aspect } · קווי עזר: { $guide }
# machine-drafted, needs native review
develop-crop-keys = A יחס · X החלפה · O קווי עזר · [ ] סיבוב · H V היפוך · Enter שמירה · Esc חזרה
# machine-drafted, needs native review
develop-crop-guide-thirds = שלישים
# machine-drafted, needs native review
develop-crop-guide-golden = יחס הזהב
# machine-drafted, needs native review
develop-crop-guide-diagonal = אלכסונים
# machine-drafted, needs native review
develop-crop-guide-grid = רשת

## Perspective: keystone and upright (its own card; guides are drawn in the crop tool).

# machine-drafted, needs native review
develop-module-perspective = פרספקטיבה
# machine-drafted, needs native review
# Upright: how the camera's tilt is found.
develop-persp-upright = זקיפה
# machine-drafted, needs native review
develop-persp-mode-off = כבוי
# machine-drafted, needs native review
develop-persp-mode-auto = אוטומטי
# machine-drafted, needs native review
develop-persp-mode-level = פילוס
# machine-drafted, needs native review
develop-persp-mode-vertical = אנכי
# machine-drafted, needs native review
develop-persp-mode-full = מלא
# machine-drafted, needs native review
develop-persp-mode-guided = מונחה
# machine-drafted, needs native review
develop-persp-vertical = אנכי
# machine-drafted, needs native review
develop-persp-horizontal = אופקי
# machine-drafted, needs native review
develop-persp-rotate = סיבוב
# machine-drafted, needs native review
develop-persp-aspect = יחס
# machine-drafted, needs native review
develop-persp-scale = קנה מידה
# machine-drafted, needs native review
develop-persp-offset-x = לצדדים
# machine-drafted, needs native review
develop-persp-offset-y = למעלה או למטה
# machine-drafted, needs native review
# Zoom so no empty edges show.
develop-persp-auto-scale = בלי קצוות ריקים
# machine-drafted, needs native review
develop-persp-guides = קווי עזר
# machine-drafted, needs native review
develop-persp-focal = אורך מוקד
# machine-drafted, needs native review
develop-persp-draw-guides = ציור קווי עזר
# machine-drafted, needs native review
# $n: guides drawn; $max: 4.
develop-persp-draw-guides-n = ציור קווי עזר ({ $n } מתוך { $max })
# machine-drafted, needs native review
develop-persp-reset = איפוס פרספקטיבה
# machine-drafted, needs native review
# $mode: one of the mode names above.
develop-persp-mode-set = זקיפה: { $mode }
# machine-drafted, needs native review
develop-persp-analysing = חיפוש קווים…
# machine-drafted, needs native review
# $n: the guides drawn.
develop-persp-reset-confirm = לאפס את הפרספקטיבה? { $n } קווי העזר שלך יוסרו. ביטול יחזיר אותם.
# machine-drafted, needs native review
develop-persp-reset-ok = איפוס
# machine-drafted, needs native review
develop-persp-reset-done = הפרספקטיבה אופסה
# machine-drafted, needs native review
# $n: the guide's number.
develop-persp-guide-remove-confirm = להסיר את קו העזר { $n }? ביטול יחזיר אותו.
# machine-drafted, needs native review
develop-persp-guide-remove-ok = הסרה
# machine-drafted, needs native review
develop-persp-guide-removed = קו העזר הוסר
# machine-drafted, needs native review
# $vertical, $horizontal: the slider values.
develop-persp-keystone-now = אנכי { $vertical }, אופקי { $horizontal }
# machine-drafted, needs native review
# $error: what went wrong.
develop-persp-failed = הזקיפה נכשלה: { $error }
# machine-drafted, needs native review
# $mode: a mode name; the angles in degrees ("2.5").
develop-persp-solved = { $mode }: הטיה { $vertical }°, פנייה { $horizontal }°, סיבוב { $rotate }°
# machine-drafted, needs native review
# $mode: a mode name.
develop-perspective-few-lines = מצב { $mode } צריך לפחות שני קווים ישרים שאפשר להבחין ביניהם. אפשר לנסות מצב אחר, או לצייר קווי עזר.
# machine-drafted, needs native review
# $n: 4.
develop-persp-guides-full = עד { $n } קווי עזר. גרירת קצה מזיזה קו.
# machine-drafted, needs native review
develop-persp-guides-on = קווי עזר. יש לגרור לאורך קו שצריך להיות אנכי או מאוזן.
# machine-drafted, needs native review
develop-persp-guides-off = חיתוך
# machine-drafted, needs native review
develop-persp-guide-moved = קו העזר הוזז
# machine-drafted, needs native review
# $n: how many guides now.
develop-persp-guide-added = קו עזר { $n } נוסף
# machine-drafted, needs native review
develop-persp-guides-keys = גרירה מוסיפה · גרירת קצה מזיזה · Delete מסיר · Shift+R חיתוך
# machine-drafted, needs native review
# $n: guides drawn; $max: 4.
develop-persp-guides-label = קווי עזר: { $n } מתוך { $max }
# machine-drafted, needs native review
# The three colour wheels.
develop-wheel-shadows = צללים
# machine-drafted, needs native review
develop-wheel-midtones = טוני ביניים
# machine-drafted, needs native review
develop-wheel-highlights = הבהרות
# machine-drafted, needs native review
# The luminance slider under each colour wheel (read by screen readers).
develop-luminance-shadows = בהירות הצללים
# machine-drafted, needs native review
develop-luminance-midtones = בהירות טוני הביניים
# machine-drafted, needs native review
develop-luminance-highlights = בהירות ההבהרות
# machine-drafted, needs native review
# The fourth colour wheel: the whole picture.
develop-wheel-global = כללי
# machine-drafted, needs native review
develop-luminance-global = בהירות כללית
# machine-drafted, needs native review
# Colour card: colourfulness changed evenly, and the lightness of colourful areas.
develop-chroma = עוצמת הצבע
# machine-drafted, needs native review
develop-brilliance = בהירות הצבעים

## Colour mixer: hue, saturation and brightness per colour.

# machine-drafted, needs native review
develop-module-colour-mixer = מערבל צבעים
# machine-drafted, needs native review
develop-colour-red = אדום
# machine-drafted, needs native review
develop-colour-orange = כתום
# machine-drafted, needs native review
develop-colour-yellow = צהוב
# machine-drafted, needs native review
develop-colour-green = ירוק
# machine-drafted, needs native review
develop-colour-aqua = טורקיז
# machine-drafted, needs native review
develop-colour-blue = כחול
# machine-drafted, needs native review
develop-colour-purple = סגול
# machine-drafted, needs native review
develop-colour-magenta = מג׳נטה
# machine-drafted, needs native review
# $colour: one of the colour names above.
develop-mixer-hue = גוון – { $colour }
# machine-drafted, needs native review
develop-mixer-saturation = רוויה – { $colour }
# machine-drafted, needs native review
develop-mixer-brightness = בהירות – { $colour }

## Point colour: pick a colour in the photo, then shift it and its neighbours.

# machine-drafted, needs native review
develop-module-point-colour = צבע נקודתי
# machine-drafted, needs native review
develop-point-pick = בחירת צבע
# machine-drafted, needs native review
# On the pick button once a colour is picked.
develop-point-pick-again = בחירת צבע אחר
# machine-drafted, needs native review
# Said and shown while picking.
develop-point-picking = יש ללחוץ על הצבע לשינוי. Esc מבטל.
# machine-drafted, needs native review
develop-point-picked = הצבע נבחר
# machine-drafted, needs native review
develop-point-cancelled = הבחירה בוטלה
# machine-drafted, needs native review
# How far from the picked colour changes reach: overall, then by hue, saturation and lightness.
develop-point-range = טווח
# machine-drafted, needs native review
develop-point-hue-range = טווח גוון
# machine-drafted, needs native review
develop-point-saturation-range = טווח רוויה
# machine-drafted, needs native review
develop-point-lightness-range = טווח בהירות
# machine-drafted, needs native review
develop-point-hue = הזזת גוון
# machine-drafted, needs native review
develop-point-saturation = הזזת רוויה
# machine-drafted, needs native review
develop-point-lightness = הזזת בהירות

## Black and white.

# machine-drafted, needs native review
develop-module-black-white = שחור-לבן
# machine-drafted, needs native review
# $colour: a colour name; how light that colour turns in grey.
develop-bw-mix = בהירות – { $colour }

## LUT: a colour look-up table file.

# machine-drafted, needs native review
develop-module-lut = LUT
# machine-drafted, needs native review
develop-lut-choose = בחירת קובץ LUT…
# machine-drafted, needs native review
# The file chosen. $name: its file name.
develop-lut-file = קובץ: { $name }
# machine-drafted, needs native review
# The file can't be used. $name: its file name.
develop-lut-unreadable = לא ניתן לקרוא את { $name }
# machine-drafted, needs native review
# The file dialog.
develop-lut-dialog = בחירת קובץ LUT
# machine-drafted, needs native review
develop-lut-kind = קובצי LUT
# machine-drafted, needs native review
# The encoding the LUT was made for.
develop-lut-space = מיועד עבור
# machine-drafted, needs native review
develop-lut-space-linear = ליניארי (Rec.2020)
# machine-drafted, needs native review
develop-lut-space-acescg = ACES ליניארי (ACEScg)
# machine-drafted, needs native review
develop-lut-space-acescct = ACES לוגריתמי (ACEScct)
# machine-drafted, needs native review
develop-lut-space-srgb = תמונה (sRGB)
# machine-drafted, needs native review
develop-lut-space-rec709 = וידאו (Rec.709)
# machine-drafted, needs native review
develop-lut-interpolation = שיטה
# machine-drafted, needs native review
develop-lut-tetrahedral = מדויקת
# machine-drafted, needs native review
develop-lut-trilinear = פשוטה
# machine-drafted, needs native review
develop-lut-strength = עוצמה

## Develop's history and status.

# machine-drafted, needs native review
develop-history-original = מקור
# machine-drafted, needs native review
# A module switched on or off. $module: its name.
develop-history-on = { $module } פועל
# machine-drafted, needs native review
develop-history-off = { $module } כבוי
# machine-drafted, needs native review
# Said to screen readers after a click on a history row. $step: the row's name.
develop-at-original = חזרה למקור
# machine-drafted, needs native review
develop-at-step = חזרה אל: { $step }
# machine-drafted, needs native review
develop-undone = בוטל: { $step }
# machine-drafted, needs native review
develop-redone = בוצע מחדש: { $step }
# machine-drafted, needs native review
develop-entered = פיתוח. עריכות אף פעם לא משנות את הקובץ.
# machine-drafted, needs native review
develop-left = יצאת מפיתוח
# machine-drafted, needs native review
develop-split-on = לפני ואחרי זה לצד זה
# machine-drafted, needs native review
develop-split-off = העריכה בלבד
# machine-drafted, needs native review
develop-showing-before = מוצג המקור
# machine-drafted, needs native review
develop-showing-after = מוצגת העריכה
# machine-drafted, needs native review
# Labels over the image.
develop-before = לפני
# machine-drafted, needs native review
develop-after = אחרי
# machine-drafted, needs native review
develop-working = מתבצע פיתוח…
# machine-drafted, needs native review
develop-preview-failed = לא ניתן להציג את העריכה של הקובץ הזה

## Develop's commands (palette, cheat sheet, Settings → Keys) and why they are unavailable.

# machine-drafted, needs native review
keys-mode-develop = פיתוח
# machine-drafted, needs native review
# The cheat sheet's heading over the speed-edit keys.
cheatsheet-develop-speed = עריכה מהירה: להחזיק ואז לגלול
# machine-drafted, needs native review
palette-develop = פיתוח
# machine-drafted, needs native review
palette-develop-help = עריכת התמונה בלי לשנות את הקובץ. הפעלה נוספת יוצאת.
# machine-drafted, needs native review
palette-develop-before-after = לפני ואחרי
# machine-drafted, needs native review
palette-develop-before-after-help = הצגת המקור, או שוב את העריכה.
# machine-drafted, needs native review
palette-develop-split = לפני ואחרי מפוצל
# machine-drafted, needs native review
palette-develop-split-help = הצגת המקור והעריכה זה לצד זה.
# machine-drafted, needs native review
palette-develop-panel = מעבר ללוח הפיתוח
# machine-drafted, needs native review
palette-develop-panel-help = העברת המקלדת אל הלוח. Esc מחזיר.
# machine-drafted, needs native review
palette-develop-speed-exposure = עריכה מהירה של החשיפה
# machine-drafted, needs native review
palette-develop-speed-exposure-help = להחזיק ואז לגלול או להקיש על החצים כדי לשנות את החשיפה.
# machine-drafted, needs native review
palette-develop-speed-contrast = עריכה מהירה של הניגודיות
# machine-drafted, needs native review
palette-develop-speed-contrast-help = להחזיק ואז לגלול או להקיש על החצים כדי לשנות את הניגודיות.
# machine-drafted, needs native review
palette-develop-speed-highlights = עריכה מהירה של ההבהרות
# machine-drafted, needs native review
palette-develop-speed-highlights-help = להחזיק ואז לגלול או להקיש על החצים כדי לשנות את ההבהרות.
# machine-drafted, needs native review
palette-develop-speed-shadows = עריכה מהירה של הצללים
# machine-drafted, needs native review
palette-develop-speed-shadows-help = להחזיק ואז לגלול או להקיש על החצים כדי לשנות את הצללים.
# machine-drafted, needs native review
palette-develop-speed-whites = עריכה מהירה של הלבנים
# machine-drafted, needs native review
palette-develop-speed-whites-help = להחזיק ואז לגלול או להקיש על החצים כדי לשנות את הלבנים.
# machine-drafted, needs native review
palette-develop-speed-blacks = עריכה מהירה של השחורים
# machine-drafted, needs native review
palette-develop-speed-blacks-help = להחזיק ואז לגלול או להקיש על החצים כדי לשנות את השחורים.
# machine-drafted, needs native review
palette-develop-speed-temperature = עריכה מהירה של הטמפרטורה
# machine-drafted, needs native review
palette-develop-speed-temperature-help = להחזיק ואז לגלול או להקיש על החצים כדי לשנות את טמפרטורת איזון הלבן.
# machine-drafted, needs native review
palette-develop-speed-vibrance = עריכה מהירה של החיוניות
# machine-drafted, needs native review
palette-develop-speed-vibrance-help = להחזיק ואז לגלול או להקיש על החצים כדי לשנות את החיוניות.
# machine-drafted, needs native review
palette-develop-crop = חיתוך ויישור
# machine-drafted, needs native review
palette-develop-crop-help = חיתוך, יישור, סיבוב או היפוך של התמונה. Enter שומר, Esc חוזר.
# machine-drafted, needs native review
disabled-not-photo = לתמונות סטילס בלבד
# machine-drafted, needs native review
disabled-develop-off = הפיתוח כבוי בפרופילים רפואה ומדע
# machine-drafted, needs native review
develop-module-corrections = תיקונים
# machine-drafted, needs native review
develop-module-raw = Raw
# machine-drafted, needs native review
develop-off = כבוי
# machine-drafted, needs native review
develop-on = פועל
# machine-drafted, needs native review
develop-highlight-method = אזורים שרופים
# machine-drafted, needs native review
develop-highlight-opposed = שחזור צבע
# machine-drafted, needs native review
develop-highlight-rebuild = שחזור אזורים גדולים
# machine-drafted, needs native review
develop-highlight-clip = חיתוך ללבן
# machine-drafted, needs native review
develop-highlight-threshold = רמת חיתוך
# machine-drafted, needs native review
develop-module-hot-pixels = פיקסלים חמים
# machine-drafted, needs native review
develop-hot-threshold = סף פיקסלים חמים
# machine-drafted, needs native review
develop-module-ca = סטייה כרומטית
# machine-drafted, needs native review
develop-ca-mode = שוליים
# machine-drafted, needs native review
develop-ca-auto = אוטומטי
# machine-drafted, needs native review
develop-ca-manual = ידני
# machine-drafted, needs native review
develop-ca-red = הסטת אדום
# machine-drafted, needs native review
develop-ca-blue = הסטת כחול
# machine-drafted, needs native review
develop-module-defringe = הסרת שוליים צבעוניים
# machine-drafted, needs native review
develop-defringe = הסרת שוליים צבעוניים
# machine-drafted, needs native review
develop-defringe-purple = גוון סגול
# machine-drafted, needs native review
develop-defringe-green = גוון ירוק
# machine-drafted, needs native review
develop-defringe-range = טווח גוון
# machine-drafted, needs native review
develop-defringe-edge = ניגודיות קצוות
# machine-drafted, needs native review
develop-module-flat-field = תמונות כיול
# machine-drafted, needs native review
develop-calibration = כיול
# machine-drafted, needs native review
develop-calibration-auto = מתיקייה
# machine-drafted, needs native review
develop-calibration-chosen = קבצים שנבחרו
# machine-drafted, needs native review
develop-flat-choose = בחירת תמונת flat…
# machine-drafted, needs native review
develop-dark-choose = בחירת תמונת dark…
# machine-drafted, needs native review
develop-flat-file = תמונת flat: { $name }
# machine-drafted, needs native review
develop-dark-file = תמונת dark: { $name }
# machine-drafted, needs native review
develop-flat-use = תמונת flat
# machine-drafted, needs native review
develop-flat-use-full = הצללת שוליים ואבק
# machine-drafted, needs native review
develop-flat-use-vignetting = הצללת שוליים בלבד
# machine-drafted, needs native review
develop-flat-dialog = בחירת תמונת flat
# machine-drafted, needs native review
develop-dark-dialog = בחירת תמונת dark
# machine-drafted, needs native review
develop-raw-kind = קובצי Raw
# machine-drafted, needs native review
develop-flat-frame = תמונת flat
# machine-drafted, needs native review
develop-dark-frame = תמונת dark

# machine-drafted, needs native review
# machine-drafted, needs native review
develop-mask-add = הוספת מסכה
# machine-drafted, needs native review
develop-mask-kind-ellipse = אליפסה
# machine-drafted, needs native review
develop-mask-kind-rectangle = מלבן
# machine-drafted, needs native review
develop-mask-kind-polygon = מצולע
# machine-drafted, needs native review
develop-mask-kind-linear = מעבר קווי
# machine-drafted, needs native review
develop-mask-kind-radial = מעבר מעגלי
# machine-drafted, needs native review
develop-mask-kind-brush = מברשת
# machine-drafted, needs native review
develop-mask-kind-luminance = טווח בהירות
# machine-drafted, needs native review
develop-mask-kind-hue = טווח גוון
# machine-drafted, needs native review
develop-mask-kind-chroma = טווח רוויה
# machine-drafted, needs native review
develop-mask-kind-colour = טווח צבע
# machine-drafted, needs native review
develop-mask-kind-other = מסכה אחרת
# machine-drafted, needs native review
develop-mask-row = { $kind } { $n }
# machine-drafted, needs native review
develop-mask-mode = מיזוג
# machine-drafted, needs native review
develop-blend-normal = רגיל
# machine-drafted, needs native review
develop-blend-multiply = הכפלה
# machine-drafted, needs native review
develop-blend-screen = מסך
# machine-drafted, needs native review
develop-blend-overlay = שכבת־על
# machine-drafted, needs native review
develop-blend-soft-light = אור רך
# machine-drafted, needs native review
develop-blend-lightness = בהירות בלבד
# machine-drafted, needs native review
develop-blend-colour = צבע בלבד
# machine-drafted, needs native review
develop-mask-amount = עוצמת המסכה
# machine-drafted, needs native review
develop-mask-invert = היפוך המסכה
# machine-drafted, needs native review
develop-mask-off = כבוי
# machine-drafted, needs native review
develop-mask-on = מופעל
# machine-drafted, needs native review
develop-mask-refine = שיפור קצוות
# machine-drafted, needs native review
develop-mask-show = הצגת המסכה
# machine-drafted, needs native review
develop-mask-show-red = אדום
# machine-drafted, needs native review
develop-mask-show-grey = אפור
# machine-drafted, needs native review
develop-mask-remove = הסרת המסכה
# machine-drafted, needs native review
develop-mask-combine-add = הוספה
# machine-drafted, needs native review
develop-mask-combine-subtract = החסרה
# machine-drafted, needs native review
develop-mask-combine-intersect = חיתוך
# machine-drafted, needs native review
develop-mask-row-opacity = אטימות
# machine-drafted, needs native review
develop-mask-row-invert = היפוך
# machine-drafted, needs native review
develop-mask-feather = ריכוך
# machine-drafted, needs native review
develop-mask-roundness = עיגול פינות
# machine-drafted, needs native review
develop-mask-brush-size = גודל מברשת
# machine-drafted, needs native review
develop-mask-brush-feather = ריכוך מברשת
# machine-drafted, needs native review
develop-mask-brush-flow = זרימה
# machine-drafted, needs native review
develop-mask-brush-mode = מברשת
# machine-drafted, needs native review
develop-mask-brush-paint = ציור
# machine-drafted, needs native review
develop-mask-brush-erase = מחיקה
# machine-drafted, needs native review
develop-mask-range-from = מ־
# machine-drafted, needs native review
develop-mask-range-full-from = מלא מ־
# machine-drafted, needs native review
develop-mask-range-full-to = מלא עד
# machine-drafted, needs native review
develop-mask-range-to = עד
# machine-drafted, needs native review
develop-mask-range-test = בדיקה
# machine-drafted, needs native review
develop-mask-range-input = קלט
# machine-drafted, needs native review
develop-mask-range-output = פלט
# machine-drafted, needs native review
develop-mask-colour-pick = בחירת צבע
# machine-drafted, needs native review
develop-mask-colour-pick-more = בחירת צבע נוסף
# machine-drafted, needs native review
develop-mask-colour-range = טווח
# machine-drafted, needs native review
develop-mask-colour-falloff = דעיכה
# machine-drafted, needs native review
develop-mask-edit = עריכה על התמונה
# machine-drafted, needs native review
develop-mask-row-remove = הסרת { $name }
# machine-drafted, needs native review
develop-mask-remove-confirm = להסיר את המסכה של { $module }? הצורות שלה יימחקו. התמונה בדיסק נשארת.
# machine-drafted, needs native review
develop-mask-row-remove-confirm = להסיר את { $name } מ־{ $module }? שאר המסכות והתמונה בדיסק נשארות.
# machine-drafted, needs native review
develop-mask-remove-ok = הסרה
# machine-drafted, needs native review
develop-mask-removed = המסכה הוסרה.
# machine-drafted, needs native review
develop-mask-added = { $name } נוספה.
# machine-drafted, needs native review
develop-mask-full = למסכה הזו כבר יש כל החלקים שהיא יכולה להכיל.
# machine-drafted, needs native review
develop-mask-editing = עריכת { $name }. גררו את הידיות, או הקישו Tab והחצים.
# machine-drafted, needs native review
develop-mask-painting = ציור { $name }. גררו על התמונה.
# machine-drafted, needs native review
develop-mask-picking = לחצו על התמונה כדי לבחור צבע.
# machine-drafted, needs native review
develop-mask-picked = הצבע נבחר.
# machine-drafted, needs native review
develop-mask-done = עריכת המסכה הסתיימה.
# machine-drafted, needs native review
develop-mask-handles = ידיות המסכה
# machine-drafted, needs native review
develop-handle-centre = מרכז
# machine-drafted, needs native review
develop-handle-width = רוחב
# machine-drafted, needs native review
develop-handle-height = גובה
# machine-drafted, needs native review
develop-handle-rotate = סיבוב
# machine-drafted, needs native review
develop-handle-feather = ריכוך
# machine-drafted, needs native review
develop-handle-inner = קצה פנימי
# machine-drafted, needs native review
develop-handle-start = התחלה
# machine-drafted, needs native review
develop-handle-end = סוף
# machine-drafted, needs native review
develop-handle-point = נקודה { $n }
# machine-drafted, needs native review
develop-instance = שכבה
# machine-drafted, needs native review
develop-instance-n = { $n }
# machine-drafted, needs native review
develop-instance-chosen = שכבה { $n }.
# machine-drafted, needs native review
develop-instance-new = שכבה חדשה
# machine-drafted, needs native review
develop-instance-duplicate = שכפול שכבה
# machine-drafted, needs native review
develop-instance-delete = מחיקת שכבה
# machine-drafted, needs native review
develop-instance-earlier = הזזה מוקדם יותר
# machine-drafted, needs native review
develop-instance-later = הזזה מאוחר יותר
# machine-drafted, needs native review
develop-instance-delete-confirm = למחוק את שכבה { $n } של { $module }? ההגדרות והמסכות שלה יימחקו. התמונה בדיסק נשארת.
# machine-drafted, needs native review
develop-instance-delete-ok = מחיקה
# machine-drafted, needs native review
develop-instance-deleted = השכבה נמחקה.
# machine-drafted, needs native review
develop-history-mask = מסכה: { $module }
# machine-drafted, needs native review
develop-history-mask-shapes = צורות מסכה: { $module }
# machine-drafted, needs native review
develop-history-instance-new = חדש: { $module }
# machine-drafted, needs native review
develop-history-instance-delete = נמחק: { $module }
# machine-drafted, needs native review
develop-history-instance-move = סדר: { $module }
# Other tools' edits and sidecar conflicts.
# machine-drafted, needs native review
develop-others-title = תוכנות אחרות
# machine-drafted, needs native review
develop-darktable-edited = נערכה ב-darktable:
# machine-drafted, needs native review
develop-darktable-open = פתיחה ב-darktable
# machine-drafted, needs native review
develop-darktable-opening = נפתחת ב-darktable
# machine-drafted, needs native review
develop-darktable-failed = darktable לא נפתחה: { $reason }
# machine-drafted, needs native review
develop-lightroom-import = ייבוא מ-Lightroom
# machine-drafted, needs native review
develop-lightroom-imported = { $count ->
    [one] יובאה הגדרת Lightroom אחת. התוצאה מקורבת.
    [two] יובאו { $count } הגדרות Lightroom. התוצאה מקורבת.
   *[other] יובאו { $count } הגדרות Lightroom. התוצאה מקורבת.
}
# machine-drafted, needs native review
develop-lightroom-nothing = אין מה לייבא: ההגדרות האלה לא משנות כאן דבר.
# machine-drafted, needs native review
develop-history-lightroom = יובא מ-Lightroom (בקירוב)
# machine-drafted, needs native review
develop-conflict-title = העריכה של { $file } השתנתה מחוץ ל-Specular
# machine-drafted, needs native review
develop-conflict-file = קובץ נלווה: נשמר { $time }
# machine-drafted, needs native review
develop-conflict-library = ספרייה: נשמרה { $time }
# machine-drafted, needs native review
develop-conflict-writer = את הקובץ הנלווה כתבה { $writer }.
# machine-drafted, needs native review
develop-conflict-detail = יש לשמור אחת. השנייה תוחלף.
# machine-drafted, needs native review
develop-conflict-use-file = להשתמש בזו שבקובץ
# machine-drafted, needs native review
develop-conflict-keep-library = לשמור את זו שבספרייה
# machine-drafted, needs native review
develop-conflict-used-file = נעשה שימוש בעריכה מהקובץ הנלווה
# machine-drafted, needs native review
develop-conflict-kept-library = נשמרה העריכה שבספרייה
# machine-drafted, needs native review
toast-develop-edit-newer-process = העריכה הזו נעשתה בגרסת Specular חדשה יותר. ייתכן שתיראה כאן אחרת.
# machine-drafted, needs native review
info-darktable-edited = נערכה ב-darktable
# machine-drafted, needs native review
info-darktable-no-modules = אין מודולים פעילים
# machine-drafted, needs native review
info-darktable-group = קובץ
# machine-drafted, needs native review
toast-develop-edit-partial = העריכה הזו נעשתה בגרסת Specular חדשה יותר. חלקים שהגרסה הזו לא יודעת להציג הושמטו. שינויים כאן לא נשמרים.
# machine-drafted, needs native review
toast-develop-sidecar-unreadable = לא ניתן לקרוא את קובץ העריכה שליד התמונה. הוא נשאר כמו שהוא. שינויים כאן לא נשמרים.
# machine-drafted, needs native review
develop-demosaic = פרטי Raw
develop-demosaic-directional = רגיל
develop-demosaic-dual = החלקת אזורים אחידים
develop-demosaic-fast = מהיר
develop-dual-threshold = ניגודיות אזורים אחידים
