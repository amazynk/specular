# machine-drafted, needs native review

## Model checks on IFC building models (mesh mode, Alt+Q): IDS, clashes, changes between
## versions, quantities. IFC class names (IfcWall), property set names and values are data
## and stay as the file has them. Keys named here are the defaults.

# The panel's tabs.
checks-tab = { $tab ->
    [ids] IDS
    [clashes] التعارضات
    [changes] التغييرات
   *[quantities] الكميات
}
# $tab: the tab's name.
checks-title = فحوص النموذج · { $tab }

# Short texts, by $what.
checks-note = { $what ->
    [not-ifc] تحتاج فحوص النموذج إلى نموذج IFC.
    [reading] جارٍ قراءة معلومات النموذج…
    [ids-empty] لا يوجد ملف IDS. اضغط O لفتح ملف.
    [ids-checking] جارٍ الفحص…
    [ids-no-failures] لا إخفاقات.
    [ids-hints] Enter الإخفاقات، انتقال · Backspace رجوع · O فتح · P الخصائص · Ctrl+S BCF · Ctrl+E CSV · Tab التالي
    [unnamed] مواصفة بلا اسم
    [clash-running] جارٍ البحث عن التعارضات…
    [clash-empty] اضغط R للبحث عن التعارضات.
    [clash-none] لا تعارضات.
    [clashes-hints] R تشغيل · A B المجموعات · M صلب أو خلوص · + − المسافة · Enter انتقال · S الحالة · Ctrl+S BCF · Ctrl+E CSV · Tab التالي
    [changes-reading] جارٍ قراءة الإصدار الآخر…
    [changes-empty] اضغط O لاختيار إصدار آخر من هذا النموذج.
    [changes-none] لا تغييرات.
    [changes-hints] O إصدار آخر · F تصفية · Enter انتقال · P الخصائص · Ctrl+S BCF · Ctrl+E CSV · Tab التالي
    [quantities-hints] G تجميع · Enter انتقال · P الخصائص · Ctrl+E CSV · Ctrl+S XLSX · Tab التالي
   *[not-drawn] لا توجد لهذا العنصر هندسة لعرضها.
}

# What a requirement is about. $name: an attribute name, an IFC class or a pattern.
checks-facet = { $facet ->
    [entity] الفئة
    [attribute] السمة { $name }
    [classification] التصنيف
    [material] المادة
   *[partof] جزء من { $name }
}

# Why an element fails a requirement. $what: what the requirement is about; $actual: what
# the element has.
checks-reason = { $why ->
    [class] الفئة هي { $actual }
    [predefined] النوع المعرّف مسبقًا هو { $actual }
    [missing] ‏{ $what } مفقود
    [empty] ‏{ $what } فارغ
    [value] ‏{ $what } هو { $actual }: غير مسموح
    [datatype] ‏{ $what }: نوع البيانات { $actual }
    [system] ‏{ $what }: النظام هو { $actual }
    [whole] ‏{ $what }: جزء من { $actual }
    [prohibited] يجب ألّا يوجد { $what }
   *[unsupported] لا يمكن فحص { $what }
}

# Shown for a missing value.
checks-none = —

# A fact about the chosen element. $value: its value.
checks-prop = { $what ->
    [storey] الطابق: { $value }
    [type] النوع: { $value }
    [material] المادة: { $value }
    [container] الحاوي: { $value }
   *[classification] التصنيف: { $value }
}

# $file: the IDS file; $ok: specifications that hold; $count: all of them.
checks-ids-summary = ‏{ $file }: ‏تتحقق { $ok } من { $count }

# A specification's result. $applicable: objects it applies to; $failed: those failing.
checks-spec = { $state ->
    [pass] ✓ ناجح: { $applicable }
    [fail] ✗ فاشل: { $failed } من { $applicable }
    [none] ✗ لا ينطبق على شيء
    [none-ok] ✓ لا ينطبق على شيء
    [prohibited] ✗ موجود ويجب ألّا يوجد: { $applicable }
   *[prohibited-ok] ✓ لا يوجد ما يجب ألّا يوجد
}

# A side of a clash test. $name: an IFC class or a storey's name.
checks-set = { $kind ->
    [all] كل العناصر
    [class] { $name }
   *[storey] الطابق { $name }
}
# $a, $b: the two sides; $distance: the tolerance or clearance with its unit.
checks-clash-setup = ‏A: { $a } · B: { $b } · { $mode ->
    [hard] صلب، أعمق من { $distance }
   *[clearance] أقرب من { $distance }
}
# $count: clashes found; $new: new ones; $resolved: earlier ones no longer found; $seconds: time taken.
checks-clash-count = { $count ->
    [zero] لا تعارضات
    [one] تعارض واحد
    [two] تعارضان
    [few] { $count } تعارضات
    [many] { $count } تعارضًا
   *[other] { $count } تعارض
}، الجديدة: { $new }، المحلولة: { $resolved } · { $seconds } ث
# One clash. $depth: the depth or distance with its unit; $status: its status.
checks-clash = { $kind ->
    [hard] صلب { $depth }
   *[clearance] خلوص { $depth }
} · { $status }
checks-clash-kind = { $kind ->
    [hard] صلب
   *[clearance] خلوص
}
checks-status = { $status ->
    [new] جديد
    [active] نشط
   *[resolved] محلول
}
# A BCF topic's title for a clash. $a, $b: the two elements.
checks-clash-topic = تعارض: { $a } × { $b }
checks-clash-done = { $count ->
    [0] لا تعارضات
    [zero] لا تعارضات
    [one] تعارض واحد
    [two] تعارضان
    [few] { $count } تعارضات
    [many] { $count } تعارضًا
   *[other] { $count } تعارض
}

# $file: the other version; counts of added, removed and changed elements; $filter: what is listed.
checks-changes-summary = ‏{ $file }: ‏المضاف { $added } · المُزال { $removed } · المتغيّر { $changed }{ $filter ->
    [added] {" "}· يُعرض المضاف
    [removed] {" "}· يُعرض المُزال
    [changed] {" "}· يُعرض المتغيّر
   *[all] {""}
}
checks-change = { $kind ->
    [added] مضاف
    [removed] مُزال
   *[geometry] الهندسة
}
checks-change-fields = { $count ->
    [zero] لا حقول
    [one] حقل واحد
    [two] حقلان
    [few] { $count } حقول
    [many] { $count } حقلًا
   *[other] { $count } حقل
}
# A field that changed. $name: the field; $old, $new: its values.
checks-field = { $name }: { $old } → { $new }
# $distance: how far it moved, with its unit.
checks-moved = نُقل { $distance }

# $by: the grouping; $count: elements; $area, $volume: totals with units.
checks-qto-summary = العناصر: { $count } · { $area } · { $volume } · { $by ->
    [element] حسب العنصر
    [class] حسب الفئة
    [storey] حسب الطابق
   *[type] حسب النوع
}
# Where quantities came from.
checks-source = { $source ->
    [file] من الملف
    [computed] محسوبة
   *[mixed] بعضها محسوب
}
# A sheet of the quantities workbook.
checks-group = { $by ->
    [element] العناصر
    [class] حسب الفئة
    [storey] حسب الطابق
   *[type] حسب النوع
}

# Column headers of exported tables.
checks-col = { $col ->
    [spec] المواصفة
    [guid] GlobalId
    [class] الفئة
    [name] الاسم
    [reason] السبب
    [a] A GlobalId
    [a-name] A
    [b] B GlobalId
    [b-name] B
    [kind] نوع التعارض
    [depth] العمق أو المسافة (m)
    [status] الحالة
    [x] X (m)
    [y] Y (m)
    [z] Z (m)
    [change] التغيير
    [field] الحقل
    [old] القديم
    [new] الجديد
    [type] النوع
    [container] الحاوي
    [material] المادة
    [classification] التصنيف
    [storey] الطابق
    [count] العدد
    [length] الطول (m)
    [area] المساحة (m²)
    [volume] الحجم (m³)
   *[source] المصدر
}

# $reason: why, in plain words.
checks-error = تعذّر ذلك: { $reason }
# A results summary for screen readers. $failed: specifications that do not hold.
checks-ids-done = { $failed ->
    [0] تم الفحص: كل المواصفات متحققة
    [zero] تم الفحص: كل المواصفات متحققة
    [one] تم الفحص: مواصفة واحدة غير متحققة
    [two] تم الفحص: مواصفتان غير متحققتين
    [few] تم الفحص: { $failed } مواصفات غير متحققة
    [many] تم الفحص: { $failed } مواصفةً غير متحققة
   *[other] تم الفحص: { $failed } مواصفة غير متحققة
}
# $file: the file written.
checks-saved = تم حفظ { $file }

# Several models in one scene. An element or clash set and the model file it is from.
# machine-drafted, needs native review
checks-in-model = { $element } ({ $model })
# A clash set taken from every model of the scene.
# machine-drafted, needs native review
checks-every-model = كل النماذج
# The model IDS, changes and quantities are about. $model: its file name.
# machine-drafted, needs native review
checks-model = النموذج: { $model }
# A tab's keys in a scene of several models, by $tab (ids, clashes, changes, quantities).
# machine-drafted, needs native review
checks-hints-models = { $tab ->
    [ids] M النموذج · Enter الإخفاقات، انتقال · Backspace رجوع · O فتح · P الخصائص · Ctrl+S BCF · Ctrl+E CSV · Tab التالي
    [clashes] R تشغيل · A B المجموعات · G H نماذجها · M صلب أو خلوص · + − المسافة · Enter انتقال · S الحالة · Ctrl+S BCF · Ctrl+E CSV · Tab التالي
    [changes] M النموذج · O إصدار آخر · F تصفية · Enter انتقال · P الخصائص · Ctrl+S BCF · Ctrl+E CSV · Tab التالي
   *[quantities] M النموذج · G تجميع · Enter انتقال · P الخصائص · Ctrl+E CSV · Ctrl+S XLSX · Tab التالي
}
