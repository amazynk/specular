# machine-drafted, needs native review

## Model checks on IFC building models (mesh mode, Alt+Q): IDS, clashes, changes between
## versions, quantities. IFC class names (IfcWall), property set names and values are data
## and stay as the file has them. Keys named here are the defaults.

# The panel's tabs.
checks-tab = { $tab ->
    [ids] IDS
    [clashes] התנגשויות
    [changes] שינויים
   *[quantities] כמויות
}
# $tab: the tab's name.
checks-title = בדיקות מודל · { $tab }

# Short texts, by $what.
checks-note = { $what ->
    [not-ifc] בדיקות מודל דורשות מודל IFC.
    [reading] קורא את המידע של המודל…
    [ids-empty] אין קובץ IDS. הקישו O כדי לפתוח קובץ.
    [ids-checking] בודק…
    [ids-no-failures] אין כשלים.
    [ids-hints] Enter כשלים, מעבר · Backspace חזרה · O פתיחה · P מאפיינים · Ctrl+S BCF · Ctrl+E CSV · Tab הבא
    [unnamed] מפרט ללא שם
    [clash-running] מחפש התנגשויות…
    [clash-empty] הקישו R כדי לחפש התנגשויות.
    [clash-none] אין התנגשויות.
    [clashes-hints] R הרצה · A B קבוצות · M קשה או מרווח · + − מרחק · Enter מעבר · S מצב · Ctrl+S BCF · Ctrl+E CSV · Tab הבא
    [changes-reading] קורא את הגרסה האחרת…
    [changes-empty] הקישו O כדי לבחור גרסה אחרת של המודל הזה.
    [changes-none] אין שינויים.
    [changes-hints] O גרסה אחרת · F סינון · Enter מעבר · P מאפיינים · Ctrl+S BCF · Ctrl+E CSV · Tab הבא
    [quantities-hints] G קיבוץ · Enter מעבר · P מאפיינים · Ctrl+E CSV · Ctrl+S XLSX · Tab הבא
   *[not-drawn] לרכיב הזה אין גאומטריה להצגה.
}

# What a requirement is about. $name: an attribute name, an IFC class or a pattern.
checks-facet = { $facet ->
    [entity] מחלקה
    [attribute] תכונה { $name }
    [classification] סיווג
    [material] חומר
   *[partof] חלק מ־{ $name }
}

# Why an element fails a requirement. $what: what the requirement is about; $actual: what
# the element has.
checks-reason = { $why ->
    [class] המחלקה היא { $actual }
    [predefined] הסוג המוגדר מראש הוא { $actual }
    [missing] ‏{ $what } חסר
    [empty] ‏{ $what } ריק
    [value] ‏{ $what } הוא { $actual }: לא מותר
    [datatype] ‏{ $what }: סוג הנתונים הוא { $actual }
    [system] ‏{ $what }: המערכת היא { $actual }
    [whole] ‏{ $what }: חלק מ־{ $actual }
    [prohibited] ‏{ $what } אסור שיופיע
   *[unsupported] לא ניתן לבדוק את { $what }
}

# Shown for a missing value.
checks-none = —

# A fact about the chosen element. $value: its value.
checks-prop = { $what ->
    [storey] קומה: { $value }
    [type] סוג: { $value }
    [material] חומר: { $value }
    [container] מכיל: { $value }
   *[classification] סיווג: { $value }
}

# $file: the IDS file; $ok: specifications that hold; $count: all of them.
checks-ids-summary = ‏{ $file }: ‏{ $ok } מתוך { $count } מתקיימים

# A specification's result. $applicable: objects it applies to; $failed: those failing.
checks-spec = { $state ->
    [pass] ✓ { $applicable } עוברים
    [fail] ✗ { $failed } מתוך { $applicable } נכשלים
    [none] ✗ לא חל על דבר
    [none-ok] ✓ לא חל על דבר
    [prohibited] ✗ { $applicable } שאסור שיהיו
   *[prohibited-ok] ✓ אין כאלה שאסור שיהיו
}

# A side of a clash test. $name: an IFC class or a storey's name.
checks-set = { $kind ->
    [all] כל הרכיבים
    [class] { $name }
   *[storey] קומה { $name }
}
# $a, $b: the two sides; $distance: the tolerance or clearance with its unit.
checks-clash-setup = ‏A: { $a } · B: { $b } · { $mode ->
    [hard] קשה, עמוק מ־{ $distance }
   *[clearance] קרוב מ־{ $distance }
}
# $count: clashes found; $new: new ones; $resolved: earlier ones no longer found; $seconds: time taken.
checks-clash-count = { $count ->
    [one] התנגשות אחת
    [two] שתי התנגשויות
   *[other] { $count } התנגשויות
}, { $new } חדשות, { $resolved } נפתרו · { $seconds } שנ׳
# One clash. $depth: the depth or distance with its unit; $status: its status.
checks-clash = { $kind ->
    [hard] קשה { $depth }
   *[clearance] מרווח { $depth }
} · { $status }
checks-clash-kind = { $kind ->
    [hard] קשה
   *[clearance] מרווח
}
checks-status = { $status ->
    [new] חדשה
    [active] פעילה
   *[resolved] נפתרה
}
# A BCF topic's title for a clash. $a, $b: the two elements.
checks-clash-topic = התנגשות: { $a } × { $b }
checks-clash-done = { $count ->
    [0] אין התנגשויות
    [one] התנגשות אחת
    [two] שתי התנגשויות
   *[other] { $count } התנגשויות
}

# $file: the other version; counts of added, removed and changed elements; $filter: what is listed.
checks-changes-summary = ‏{ $file }: ‏{ $added } נוספו · { $removed } הוסרו · { $changed } שונו{ $filter ->
    [added] {" "}· מוצגים שנוספו
    [removed] {" "}· מוצגים שהוסרו
    [changed] {" "}· מוצגים ששונו
   *[all] {""}
}
checks-change = { $kind ->
    [added] נוסף
    [removed] הוסר
   *[geometry] גאומטריה
}
checks-change-fields = { $count ->
    [one] שדה אחד
    [two] שני שדות
   *[other] { $count } שדות
}
# A field that changed. $name: the field; $old, $new: its values.
checks-field = { $name }: { $old } → { $new }
# $distance: how far it moved, with its unit.
checks-moved = הוזז { $distance }

# $by: the grouping; $count: elements; $area, $volume: totals with units.
checks-qto-summary = { $count ->
        [one] רכיב אחד
        [two] שני רכיבים
       *[other] { $count } רכיבים
    } · { $area } · { $volume } · { $by ->
    [element] לפי רכיב
    [class] לפי מחלקה
    [storey] לפי קומה
   *[type] לפי סוג
}
# Where quantities came from.
checks-source = { $source ->
    [file] מהקובץ
    [computed] מחושב
   *[mixed] חלקן מחושבות
}
# A sheet of the quantities workbook.
checks-group = { $by ->
    [element] רכיבים
    [class] לפי מחלקה
    [storey] לפי קומה
   *[type] לפי סוג
}

# Column headers of exported tables.
checks-col = { $col ->
    [spec] מפרט
    [guid] GlobalId
    [class] מחלקה
    [name] שם
    [reason] סיבה
    [a] A GlobalId
    [a-name] A
    [b] B GlobalId
    [b-name] B
    [kind] סוג התנגשות
    [depth] עומק או מרחק (m)
    [status] מצב
    [x] X (m)
    [y] Y (m)
    [z] Z (m)
    [change] שינוי
    [field] שדה
    [old] ישן
    [new] חדש
    [type] סוג
    [container] מכיל
    [material] חומר
    [classification] סיווג
    [storey] קומה
    [count] מספר
    [length] אורך (m)
    [area] שטח (m²)
    [volume] נפח (m³)
   *[source] מקור
}

# $reason: why, in plain words.
checks-error = לא ניתן לעשות זאת: { $reason }
# A results summary for screen readers. $failed: specifications that do not hold.
checks-ids-done = { $failed ->
    [0] נבדק: כל המפרטים מתקיימים
    [one] נבדק: מפרט אחד אינו מתקיים
    [two] נבדק: שני מפרטים אינם מתקיימים
   *[other] נבדק: { $failed } מפרטים אינם מתקיימים
}
# $file: the file written.
checks-saved = { $file } נשמר

# Several models in one scene. An element or clash set and the model file it is from.
# machine-drafted, needs native review
checks-in-model = { $element } ({ $model })
# A clash set taken from every model of the scene.
# machine-drafted, needs native review
checks-every-model = כל המודלים
# The model IDS, changes and quantities are about. $model: its file name.
# machine-drafted, needs native review
checks-model = מודל: { $model }
# A tab's keys in a scene of several models, by $tab (ids, clashes, changes, quantities).
# machine-drafted, needs native review
checks-hints-models = { $tab ->
    [ids] M מודל · Enter כשלים, מעבר · Backspace חזרה · O פתיחה · P מאפיינים · Ctrl+S BCF · Ctrl+E CSV · Tab הבא
    [clashes] R הרצה · A B קבוצות · G H המודלים שלהן · M קשה או מרווח · + − מרחק · Enter מעבר · S מצב · Ctrl+S BCF · Ctrl+E CSV · Tab הבא
    [changes] M מודל · O גרסה אחרת · F סינון · Enter מעבר · P מאפיינים · Ctrl+S BCF · Ctrl+E CSV · Tab הבא
   *[quantities] M מודל · G קיבוץ · Enter מעבר · P מאפיינים · Ctrl+E CSV · Ctrl+S XLSX · Tab הבא
}
