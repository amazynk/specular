# machine-drafted, needs native review
## סקירת מודלים בתלת־ממד: הליכה וטיסה, מישורי חתך ותיבת חתך, מבטים שמורים, נושאי BCF
## (מצב תלת־ממד). המקשים המוזכרים הם ברירת המחדל.

cheatsheet-review = סקירה

palette-mesh-walk = הליכה
palette-mesh-fly = טיסה
palette-mesh-views = מבטים שמורים
palette-mesh-save-view = שמירת המבט
palette-mesh-section-plane = הוספת מישור חתך
palette-mesh-section-box = תיבת חתך
palette-mesh-section-flip = היפוך מישור החתך
palette-mesh-section-clear = הסרת כל החתכים
palette-mesh-measure-cut = מדידה על החתך
palette-mesh-bcf-topics = נושאי BCF
# machine-drafted, needs native review
palette-mesh-model-checks = בדיקות מודל
# machine-drafted, needs native review
palette-mesh-models = מודלים בסצנה
# machine-drafted, needs native review
palette-mesh-add-model = הוספת מודל…
# machine-drafted, needs native review
palette-mesh-add-model-folder = הוספת מודלים מתיקייה…

# הליכה וטיסה: הודעות ושורות תצוגה, לפי $what. $eye: גובה העין מעל הרצפה, עם היחידה
# (walk-line בלבד).
mesh-nav = { $what ->
    [walk-toast] הליכה: W A S D לתנועה, גרירה להסתכלות, Esc לעצירה
    [fly-toast] טיסה: W A S D לתנועה, Q E למטה ולמעלה, Esc לעצירה
    [orbit] סיבוב
    [walk-line] הליכה · העין { $eye } מעל הרצפה
    [walk-keys] W A S D תנועה · גרירה מסתכלת · Shift מהר יותר · Esc עוצר
    [preparing] הקירות עדיין לא מוכנים: עוברים דרכם
    [fly-keys] W A S D תנועה · Q E למטה, למעלה · גרירה מסתכלת · Shift מהר יותר · Esc עוצר
   *[fly-line] טיסה
}

# טקסטים קצרים של חתכים, מדידה ומבטים שמורים, לפי $what.
bim-note = { $what ->
    [plane-added] נוסף מישור חתך: גררו את הידית שלו כדי להזיז אותו
    [box-on] תיבת החתך פעילה: גררו את הידית של פאה כדי להזיז אותה
    [box-off] תיבת החתך כבויה
    [cleared] החתכים הוסרו
    [measure-on] לחצו על שתי נקודות בחתך
    [measure-needs-cut] יש להוסיף תחילה מישור חתך או את התיבה
    [measure-off-cut] לחצו על החתך
    [measure-hint] מדידה: לחצו על שתי נקודות בחתך
    [ask-view-name] שם המבט
    [ask-rename] שם חדש
    [views-empty] אין מבטים שמורים. הקישו N כדי לשמור את זה.
    [views-hints] Enter מעבר · N שמירה · F2 שינוי שם · Delete הסרה · P שמירה כ-PNG
    [delete-detail] רק המבט השמור יוסר. המודל לא משתנה.
    [delete-ok] הסרה
    [not-over-model] בחרו קובץ אחר: הקובץ הזה יחליף את המודל.
   *[view-name] מבט
}

# טקסטים קצרים של BCF‏ (BIM Collaboration Format), לפי $what.
bcf-note = { $what ->
    [title-none] נושאי BCF
    [new-file] חדש
    [empty] אין נושאים. הקישו O כדי לפתוח קובץ BCF או N כדי להוסיף נושא.
    [hints] Enter מעבר · ← → נקודות מבט · N נושא · C הערה · S מצב · E כותרת · O פתיחה · Ctrl+S ייצוא
    [ask-title] כותרת הנושא
    [ask-comment] הערה
    [untitled] ללא כותרת
    [topic-added] נושא נוסף עם המבט הזה
   *[comment-added] הערה נוספה עם המבט הזה
}

# $planes: כמה מישורי חתך.
bim-sections-line = { $planes ->
    [0] אין מישורי חתך
    [one] מישור חתך אחד
    [two] { $planes } מישורי חתך
   *[other] { $planes } מישורי חתך
}
# $size: גודל תיבת החתך עם היחידה.
bim-box-line = תיבה { $size }
# $count: המספר המרבי של מישורים.
bim-planes-full = לכל היותר { $count } מישורי חתך
# ידית גרירה, לקוראי מסך. $n: המספר שלה.
bim-handle = { $kind ->
    [box] פאה { $n } של תיבת החתך
   *[plane] מישור חתך { $n }
}
# $length: האורך עם היחידה.
bim-measure-line = אורך על החתך: { $length }
bim-views-title = { $count ->
    [one] מבט שמור אחד
    [two] שני מבטים שמורים
   *[other] { $count } מבטים שמורים
}
# $name: שם של מבט.
bim-view-saved = „{ $name }” נשמר
bim-view-delete-title = להסיר את המבט „{ $name }”?
# $file: קובץ שנכתב.
bim-saved-file = { $file } נשמר
# $reason: מדוע זה נכשל.
bim-failed = { $what ->
    [open] לא ניתן לפתוח את קובץ ה-BCF:‏ { $reason }
    [views] המבטים נשמרים להפעלה הזו בלבד: { $reason }
   *[save] לא ניתן לשמור: { $reason }
}
# $count: נושאים; $file: שם קובץ הארכיון.
bcf-title = { $count ->
    [one] נושא אחד
    [two] שני נושאים
   *[other] { $count } נושאים
} · { $file }
bcf-comments = { $count ->
    [one] הערה אחת
    [two] שתי הערות
   *[other] { $count } הערות
}
# $at: נקודת המבט המוצגת; $count: נקודות המבט של הנושא.
bcf-viewpoint-of = נקודת מבט { $at } מתוך { $count }
bcf-opened = { $count ->
    [one] נפתח נושא אחד
    [two] נפתחו שני נושאים
   *[other] נפתחו { $count } נושאים
}

## Several IFC models in one scene (the Models list, Alt+Shift+M).

# The list's title. $count: models in the scene.
# machine-drafted, needs native review
fed-title = { $count ->
    [one] מודלים: 1
    [two] מודלים: 2
   *[other] מודלים: { $count }
}
# machine-drafted, needs native review
fed-note = { $what ->
    [not-ifc] הקובץ הזה אינו מודל IFC.
    [not-ifc-open] פתחו קודם מודל IFC. אפשר לשלב רק מודלי IFC.
    [this-model] המודל הזה
    [hints] A הוספת מודל · Shift+A הוספת תיקייה · Space הצגה או הסתרה · C צביעה לפי מודל · Enter מעבר · Delete הסרה
    [origin-ok] מיקום בראשית הצירים
    [first-stays] המודל שפתחתם נשאר. כדי להחליף אותו, פתחו קובץ אחר.
    [remove-detail] הוא יוצא מהסצנה. הקובץ בדיסק נשאר.
   *[remove-ok] הסרה
}
# A model's row. $basis: how it was placed; $elements: its elements; $shown: shown or hidden.
# machine-drafted, needs native review
fed-row = { $basis } · { $elements ->
    [one] רכיב אחד
    [two] שני רכיבים
   *[other] { $elements } רכיבים
} · { $shown ->
    [hidden] מוסתר
   *[shown] מוצג
}
# How a model was placed against the first. $crs: the coordinate system's name.
# machine-drafted, needs native review
fed-basis = { $basis ->
    [reference] המודל שנפתח
    [map-crs] לפי קואורדינטות מפה ({ $crs })
    [map] לפי קואורדינטות מפה
    [site] לפי קו הרוחב וקו האורך של האתר (בקירוב)
    [project] לפי קואורדינטות פרויקט משותפות
   *[origin] בראשית הצירים (לא מיושר)
}
# Why a model cannot be aligned. $first, $this: coordinate system names.
# machine-drafted, needs native review
fed-unaligned = { $why ->
    [crs] המודלים משתמשים במערכות קואורדינטות שונות: { $first } ו-{ $this }.
    [only-first] למודל שנפתח יש ייחוס גאוגרפי, ולמודל הזה אין.
    [only-this] למודל הזה יש ייחוס גאוגרפי, ולמודל שנפתח אין.
   *[broken] אי אפשר להשתמש בהמרת המפה של המודל הזה (קנה המידה שלה הוא אפס).
}
# The chosen model's placement. $distance: how far it moved; $angle: degrees turned.
# machine-drafted, needs native review
fed-placed = הוזז { $distance }, סובב { $angle }°
# machine-drafted, needs native review
fed-adding = { $count ->
    [one] קריאת מודל אחד…
    [two] קריאת שני מודלים…
   *[other] קריאת { $count } מודלים…
}
# machine-drafted, needs native review
fed-ask-origin = אי אפשר ליישר את { $file }. למקם אותו בראשית הצירים?
# machine-drafted, needs native review
fed-no-files = אין קובצי IFC בתיקייה { $folder }.
# machine-drafted, needs native review
fed-not-added = { $file } לא נוסף: { $reason }
# machine-drafted, needs native review
fed-added = { $file } נוסף. מודלים בסצנה: { $count }.
# machine-drafted, needs native review
fed-remove-title = להסיר את { $file } מהסצנה?
# machine-drafted, needs native review
fed-removed = { $file } הוסר מהסצנה.
# machine-drafted, needs native review
fed-shown = { $file } { $shown ->
    [hidden] מוסתר
   *[shown] מוצג
}
# machine-drafted, needs native review
fed-colours = { $on ->
    [on] צביעה לפי מודל פועלת
   *[off] צביעה לפי מודל כבויה
}

## PMI (product manufacturing information) of a CAD model: dimensions, geometric tolerances,
## datums and the file's saved views (mesh mode).

# machine-drafted, needs native review
palette-mesh-pmi = PMI ומבטים שמורים
# $count: annotations in the model.
# machine-drafted, needs native review
pmi-title = { $count ->
    [one] הערה אחת
    [two] שתי הערות
   *[other] { $count } הערות
}
# machine-drafted, needs native review
pmi-view = מבט שמור
# machine-drafted, needs native review
pmi-view-all = מציג את כל ההערות
# $count: annotations the saved view shows.
# machine-drafted, needs native review
pmi-view-shows = { $count ->
    [one] מציג הערה אחת
    [two] מציג שתי הערות
   *[other] מציג { $count } הערות
}
# $name: the saved view's name.
# machine-drafted, needs native review
pmi-view-shown = מוצג המבט „{ $name }”. A מציג הכול.
# machine-drafted, needs native review
pmi-has-faces = פאות מודגשות
# Short texts, by $what.
# machine-drafted, needs native review
pmi-note = { $what ->
    [none] אין PMI במודל הזה.
    [hints] ↑ ↓ בחירה · Enter מעבר · A הכול · D כפי שצויר · Shift+A הסתרה
    [hidden] ההערות מוסתרות
    [shown] ההערות מוצגות
    [as-drawn] ההערות כפי שהקובץ מצייר אותן
    [as-labels] ההערות כתוויות
   *[all-shown] כל ההערות מוצגות
}
# What an annotation is, by $what.
# machine-drafted, needs native review
pmi-kind = { $what ->
    [dimension] מידה
    [tolerance] טולרנס גאומטרי
    [datum] ייחוס
    [target] מטרת ייחוס
   *[graphic] הערה מצוירת
}
# machine-drafted, needs native review
pmi-dimension = { $what ->
    [linear] מרחק
    [angular] זווית
    [diameter] קוטר
    [radius] רדיוס
    [length] אורך
    [thickness] עובי
   *[other] מידה
}
# Geometric characteristics (ISO 1101), by $what.
# machine-drafted, needs native review
pmi-characteristic = { $what ->
    [straightness] ישרות
    [flatness] מישוריות
    [circularity] עגילות
    [cylindricity] גליליות
    [line-profile] פרופיל קו
    [surface-profile] פרופיל משטח
    [angularity] זוויתיות
    [perpendicularity] ניצבות
    [parallelism] מקבילות
    [position] מיקום
    [concentricity] קונצנטריות
    [coaxiality] קואקסיאליות
    [symmetry] סימטריה
    [circular-runout] זריקה מעגלית
   *[total-runout] זריקה כוללת
}
