# Multiplanar reformatting (MPR).

palette-mpr = שחזור רב-מישורי (MPR)
palette-mpr-slab = פרוסת MPR הבאה (דקה, MIP, MinIP, ממוצע)
disabled-no-volume = זה אינו נפח: יש לפתוח סדרה, קובץ מרובה פריימים, נפח תלת-ממדי או ערימת z
disabled-mpr-closed = יש לפתוח קודם את התצוגות הרב-מישוריות
mpr-axial = צירי
mpr-coronal = קורונלי
mpr-sagittal = סגיטלי
mpr-oblique = אלכסוני
mpr-xy = XY
mpr-xz = XZ
mpr-yz = YZ
mpr-3d = תצוגת תלת-ממד
mpr-thin = דקה
mpr-mip = MIP
mpr-minip = MinIP
mpr-average = ממוצע
mpr-slab = פרוסה: { $mode }, { $thickness } מ״מ
# machine-drafted, needs native review
mpr-slab-angstrom = פרוסה: { $mode }, { $thickness } Å
mpr-slab-unit = פרוסה: { $mode }, { $thickness } { $unit }
mpr-slab-voxels = פרוסה: { $mode }, { $thickness } ווקסלים
mpr-thicker = פרוסה עבה יותר
mpr-thinner = פרוסה דקה יותר
mpr-reset = איפוס תצוגות
mpr-close = סגירת MPR
mpr-maximise = הגדלת תצוגה
mpr-export-planes = ייצוא מישורים
# machine-drafted, needs native review
# Streamlines (TrackVis .trk, MRtrix .tck) found beside a brain volume, drawn over each
# view within a slab about its plane; Alt+S steps the slab, then hides them. $files: the
# files' names; $thickness: the slab in millimetres.
mpr-tracts = קווי זרימה מ-{ $files }: פרוסה { $thickness } מ״מ
mpr-tracts-off = קווי זרימה מ-{ $files }: מוסתרים
mpr-tracts-none = אין קווי זרימה של TrackVis או MRtrix לצד נפח זה
mpr-position = { $view } { $position } מ״מ
# machine-drafted, needs native review
mpr-position-angstrom = { $view } { $position } Å
mpr-position-unit = { $view } { $position } { $unit }
mpr-position-voxels = { $view } { $position }
mpr-length-unit = { $value } { $unit }
mpr-length-voxels = { $value } ווקסלים
mpr-axis-label = { $axis }: { $label }
mpr-axis-position = { $axis }: { $position } מתוך { $count }
mpr-axis-channel = ערוץ
mpr-axis-unnamed = ציר { $axis }
mpr-one-channel = MPR מציג ערוץ אחד בכל פעם
# machine-drafted, needs native review
# $count: channels drawn together in their colours.
mpr-composite = { $count } ערוצים בצבע, כל אחד בטווח התצוגה שלו
# $reason: error-reason-needs-memory ("it needs 6 GB of free RAM, but only 4 GB is free right now. ...").
mpr-composite-memory = מוצג ערוץ אחד: { $reason }
mpr-isotropic = אין גודל ווקסל בקובץ: הווקסלים מוצגים כקוביות
mpr-isotropic-z = אין מרווח בין פרוסות בקובץ: הפרוסות מוצגות במרחק פיקסל אחד
# A seismic volume's inline and crossline sections (the time slice reads "Time … ms").
mpr-seismic-inline = אינליין { $inline }
mpr-seismic-crossline = קרוסליין { $crossline }
mpr-value = { $value } { $unit }
mpr-value-plain = { $value }
mpr-loading = טוען פרוסות: { $loaded } מתוך { $count }
mpr-loaded = פרוסות: { $count }, { $width } × { $height }
mpr-downsampled = מוצג ברזולוציה 1/{ $factor } (מגבלות זיכרון או גרפיקה)
mpr-converted = הערכים מוצגים כמספרים צפים של 32 סיביות
mpr-opening = פותח את הנפח…
mpr-gpu-memory = { $size } בכרטיס הגרפי
mpr-refused = לא ניתן לבנות נפח: { $reason }
mpr-failed = הנפח לא נטען: { $reason }
mpr-no-gpu = הכרטיס הגרפי אינו יכול להכיל נפח זה: { $reason }
mpr-planes-saved = { $count ->
    [one] מישור אחד נשמר ב-{ $folder }
    [two] שני מישורים נשמרו ב-{ $folder }
   *[other] { $count } מישורים נשמרו ב-{ $folder }
}
mpr-planes-failed = לא ניתן היה לשמור את המישורים: { $reason }
mpr-view-a11y = תצוגת { $view }, { $position } מ״מ
mpr-crosshair-a11y = כוונת ב-{ $x }, { $y }, { $z } מ״מ
# machine-drafted, needs native review
mpr-crosshair-a11y-angstrom = כוונת ב-{ $x }, { $y }, { $z } Å
mpr-crosshair-a11y-unit = כוונת ב-{ $x }, { $y }, { $z } { $unit }
mpr-crosshair-a11y-voxels = כוונת בווקסל { $x }, { $y }, { $z }
mpr-controls = פקדי MPR

# 3D view palette entries.
palette-mpr-preset = פריסט התלת-ממד הבא (CT עצם, רקמה רכה, ריאה, אנגיו; MR)
palette-mpr-render = עיבוד התלת-ממד הבא (מוצלל, Cinematic, MIP, MinIP)
palette-mpr-clip = חיתוך התלת-ממד הבא (ללא, תיבת תצוגות הדו-ממד, פינה חתוכה)
palette-mpr-export-stl = ייצוא משטח התלת-ממד כ-STL…
# machine-drafted, needs native review
palette-mpr-tract-slab = פרוסת קווי הזרימה הבאה ב-MPR
palette-mpr-maximise = הגדלת תצוגת ה-MPR או הצגת כל התצוגות
palette-mpr-export-planes = ייצוא מישורי ה-MPR כתמונות…

# The 3D view: transfer function presets, renderings, clipping; STL surface export.
vr-preset = { $preset ->
    [bone] CT עצם
    [soft] CT רקמה רכה
    [lung] CT ריאה
    [angio] CT אנגיו
   *[mr] MR
}
vr-mode = { $mode ->
    [cinematic] Cinematic
    [mip] MIP
    [minip] MinIP
   *[shaded] מוצלל
}
vr-clip = { $clip ->
    [box] חיתוך: תיבת התצוגות
    [cut] חיתוך: פינה חתוכה
   *[off] ללא חיתוך
}
vr-smooth = החלקת המשטח
vr-export = ייצוא STL
vr-threshold = משטח בסף { $value }
vr-stl-exporting = בונה את המשטח…
vr-stl-saved = { $count ->
    [0] אין דבר בסף זה: לא נשמר משטח
   *[other] נשמר ב-{ $name }, משולשים: { $count }
}
vr-stl-failed = לא ניתן לשמור את המשטח: { $reason }
vr-view-a11y = { $preset }, { $mode }, { $clip }, משטח בסף { $threshold }. Shift+חצים לסיבוב, פלוס ומינוס לזום.

## נפח גולמי: איך מסודר קובץ .raw / .vol ללא כותרת

# machine-drafted, needs native review
raw-volume-title = פתיחת נפח גולמי
raw-volume-file = ל-{ $file } אין כותרת. יש לציין איך מסודרות הדגימות שלו.
raw-volume-guess-label = סידור מוצע
raw-volume-guess = { $index } מתוך { $count }
raw-volume-custom = מותאם אישית
raw-volume-width = רוחב (ווקסלים)
raw-volume-height = גובה (ווקסלים)
raw-volume-slices = פרוסות
raw-volume-type = סוג דגימה
raw-volume-type-unsigned = ללא סימן, { $bits } סיביות
raw-volume-type-signed = עם סימן, { $bits } סיביות
raw-volume-type-float = נקודה צפה, { $bits } סיביות
raw-volume-order = סדר בתים
raw-volume-little = Little-endian
raw-volume-big = Big-endian
raw-volume-offset = היסט כותרת
raw-volume-bytes = { $count ->
    [one] בייט אחד
    [two] שני בייטים
   *[other] { $count } בייט
}
raw-volume-pixel = גודל פיקסל
raw-volume-spacing = מרווח בין פרוסות
raw-volume-mm = { $value } מ״מ
raw-volume-exact = ממלא את הקובץ בדיוק ({ $size })
raw-volume-fits = משתמש ב-{ $size } מתוך { $file }; נותרים { $left }
raw-volume-too-big = נדרשים { $size }, אבל גודל הקובץ { $file }
raw-volume-cancel = ביטול
raw-volume-open = פתיחה
# machine-drafted, needs native review
# A dense connectivity file (CIFTI .dconn.nii) over voxels: the volume shown is one seed's
# row; letting go of the crosshair on another seed's voxel shows that one. $seed: the
# structure as the file names it and the voxel, such as "THALAMUS_LEFT (4, 6, 3)".
mpr-dconn-seed = קישוריות של { $seed }; הזיזו את הכוונת לנקודת זרע אחרת
# machine-drafted, needs native review
mpr-measure-hint-edit = גררו קצה כדי להזיז אותו; אל הקצה השני כדי להסיר
# machine-drafted, needs native review
mpr-measure-removed = המדידה הוסרה
mpr-measure-lengths-only = MPR מודד אורכים בלבד
mpr-more = עוד…

## Curved MPR: a reformat along a curve clicked on a plane.
## machine-drafted, needs native review

palette-mpr-curve = MPR מעוקל לאורך עקומה שסומנה בלחיצות
palette-mpr-curve-layout = MPR מעוקל: מיושר או מתוח
palette-mpr-curve-save = שמירת העקומה…
palette-mpr-curve-load = טעינת עקומה…
palette-mpr-export-segments = ייצוא כל סגמנט כ-STL…
palette-mpr-dental = פריסה דנטלית (פנורמה וחתכי רוחב)
palette-mpr-nerve = סימון התעלה המנדיבולרית
cpr-kind = { $kind ->
    [stretched] מתוח
   *[straightened] מיושר
}
cpr-button = מעוקל
cpr-controls = פקדי MPR מעוקל
cpr-at = ב-{ $position }
cpr-section = חתך רוחב
cpr-hint-start = יש ללחוץ על נקודות לאורך המבנה בתצוגת { $view }
cpr-hint-more = יש ללחוץ על נקודה נוספת
cpr-add-points = הוספת נקודות
cpr-undo-point = ביטול נקודה
# machine-drafted, needs native review
cpr-mark = סימון כאן
cpr-mark-set = סימון { $n } נקבע.
cpr-mark-removed = הסימון הוסר.
cpr-mark-here = סימון { $n }
cpr-diameter = קוטר { $value }
cpr-diameters = קוטר { $min } עד { $max }
cpr-mark-span = סימון { $from } עד { $to }: { $length } לאורך קו המרכז
cpr-mark-total = כל הסימונים: { $length }
cpr-clear = ניקוי העקומה
cpr-save = שמירת העקומה
cpr-load = טעינת עקומה
cpr-done = סיום
cpr-a11y = { $name } לאורך { $length }, הסמן ב-{ $position }. הגלגלת מזיזה את הסמן.
cpr-none = אין עדיין עקומה: יש ללחוץ קודם על נקודות
cpr-cleared = העקומה נוקתה
cpr-saved = העקומה נשמרה: { $name }
cpr-save-failed = לא ניתן לשמור את העקומה: { $reason }
cpr-loaded = העקומה נטענה
# machine-drafted, needs native review
# $count: centrelines read from the file.
cpr-loaded-count = { $count ->
    [one] נטען קו מרכז אחד
    [two] נטענו שני קווי מרכז
   *[other] נטענו { $count } קווי מרכז
}
# $count: centrelines found in files saved beside the volume.
cpr-sidecars-found = { $count ->
    [one] נמצא קו מרכז שמור אחד
    [two] נמצאו שני קווי מרכז שמורים
   *[other] נמצאו { $count } קווי מרכז שמורים
}
# A centreline without a name, by its place in the list.
cpr-centreline = קו מרכז { $number }
cpr-new = קו מרכז חדש
cpr-rename = שינוי שם
cpr-name = שם קו המרכז
cpr-delete = מחיקה
# $name: the centreline's name.
cpr-delete-title = למחוק את קו המרכז “{ $name }”?
cpr-delete-detail = הנקודות שלו יימחקו. קבצים שנשמרו בדיסק נשארים.
cpr-deleted = “{ $name }” נמחק
cpr-load-failed = לא ניתן לטעון את העקומה: { $reason }
cpr-load-not-json = הקובץ אינו JSON
cpr-load-not-curves = הקובץ אינו עקומה של Specular
cpr-load-empty = הקובץ אינו מכיל עקומה של שתי נקודות או יותר
cpr-panoramic = פנורמה
cpr-panoramic-slab = פנורמה, פרוסה של { $thickness } מ״מ
cpr-panoramic-thin = פנורמה, דקה

## The dental layout (Medical).
## machine-drafted, needs native review

dental-button = דנטלי
dental-not-for-diagnosis = לא מיועד לאבחון או לתכנון טיפול
dental-nerve = התעלה המנדיבולרית
dental-nerve-hint = יש ללחוץ על נקודות לאורך התעלה המנדיבולרית בפנורמה או בחתך רוחב
dental-nerve-done = סימון התעלה המנדיבולרית כבוי
dental-spacing = כל { $spacing } מ״מ
dental-section = חתך { $number }
dental-off = יש לפתוח קודם את הפריסה הדנטלית
dental-suggesting = מחפש את קשת השיניים…
dental-suggested = הוצעה קשת: יש לגרור את נקודותיה כדי לכוונן
dental-no-arch = לא נמצאה קשת: יש ללחוץ על נקודות לאורכה בתצוגה הצירית
dental-vendor = CBCT דנטלי של { $vendor }

## Each segment as its own STL.
## machine-drafted, needs native review

seg-stl-export-button = ייצוא סגמנטים
seg-stl-detail = סגמנטים: { $count ->
        [one] משולש אחד
        [two] שני משולשים
       *[other] { $count } משולשים
    }
seg-stl-detail-full = סגמנטים: פירוט מלא
seg-stl-none = אין סגמנטים לייצוא
seg-stl-not-labels = נפח זה אינו מפת תוויות
seg-stl-exporting = בונה את משטחי הסגמנטים…
seg-stl-saved = { $count ->
    [one] סגמנט אחד נשמר ב-{ $folder }
    [two] שני סגמנטים נשמרו ב-{ $folder }
   *[other] { $count } סגמנטים נשמרו ב-{ $folder }
}
seg-stl-open = לא סגורים: { $count }.
seg-stl-empty = הסגמנטים ריקים: לא נשמר דבר
seg-stl-failed = לא ניתן לשמור את הסגמנטים: { $reason }
# machine-drafted, needs native review
toolbar-mpr = שלושה מישורים (MPR)
toolbar-mpr-close = סגירת שלושת המישורים (MPR)
settings-viewing-toolbar-items-mpr = שלושה מישורים (MPR)
