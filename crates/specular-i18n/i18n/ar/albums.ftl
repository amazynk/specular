# machine-drafted, needs native review
# Specular — Library albums, smart albums and the date timeline.

## Sidebar
albums-title = الألبومات
albums-new = ألبوم جديد…
albums-new-smart = ألبوم ذكي جديد…
albums-save-search = حفظ البحث كألبوم ذكي…
albums-smart = ألبوم ذكي
# The album the photos go to when added without choosing.
albums-target = تذهب الإضافات إلى هنا
albums-group-count = { $count ->
    [zero] { $count } ألبوم
    [one] ألبوم واحد
    [two] ألبومان
    [few] { $count } ألبومات
    [many] { $count } ألبومًا
   *[other] { $count } ألبوم
}
albums-need-library = الألبومات موجودة في المكتبة. أضف هذا المجلد إلى المكتبة أولًا.

## Screen readers: the buttons of album and group rows
albums-a11y-rename = إعادة تسمية { $album }
albums-a11y-edit = تحرير { $album }
albums-a11y-delete = حذف الألبوم { $album }
albums-a11y-rename-group = إعادة تسمية المجموعة { $group }
albums-a11y-delete-group = حذف المجموعة { $group }

## Sheets
albums-name = الاسم
albums-unnamed = بلا اسم بعد
albums-new-title = ألبوم جديد
albums-new-smart-title = ألبوم ذكي جديد
albums-edit-smart-title = تحرير الألبوم الذكي
albums-rename-title = إعادة تسمية الألبوم
albums-rename-group-title = إعادة تسمية المجموعة
albums-create = إنشاء
albums-rename = إعادة التسمية
albums-save = حفظ
albums-groups-note = استخدم / لوضعه في مجموعة: العميل X / المختارة.
albums-name-empty = اكتب اسمًا.
albums-name-taken = { $album } موجود بالفعل.
albums-name-clash = { $album } ألبوم، لذا لا يمكن أن يكون مجموعة أيضًا.

## Smart album rules
albums-rules = القواعد
albums-rules-note = تُطبَّق القواعد من الأعلى إلى الأسفل. تُتجاهل القواعد الفارغة.
albums-rules-none = أضف قاعدة واحدة على الأقل.
albums-rule-label = القاعدة { $number }
albums-rule-empty = قاعدة فارغة
albums-rule-add = إضافة قاعدة
albums-rule-combine = الدمج مع القواعد أعلاه
albums-rule-first = الصور التي تطابق
albums-rule-and = و
albums-rule-or = أو
albums-rule-except = باستثناء

## Adding and removing photos
albums-add-menu = إضافة إلى ألبوم…
albums-remove-menu = إزالة من الألبوم
albums-add-title = { $count ->
    [zero] إضافة { $count } صورة إلى ألبوم
    [one] إضافة صورة واحدة إلى ألبوم
    [two] إضافة صورتين إلى ألبوم
    [few] إضافة { $count } صور إلى ألبوم
    [many] إضافة { $count } صورةً إلى ألبوم
   *[other] إضافة { $count } صورة إلى ألبوم
}
albums-add = إضافة
albums-add-new = ألبوم جديد
albums-add-list = الألبومات
albums-add-none = لا ألبومات بعد. اكتب اسمًا لإنشاء ألبوم.
albums-add-note = أو اكتب اسمًا لألبوم جديد.
albums-added = { $count ->
    [zero] أُضيفت { $count } صورة إلى { $album }.
    [one] أُضيفت صورة واحدة إلى { $album }.
    [two] أُضيفت صورتان إلى { $album }.
    [few] أُضيفت { $count } صور إلى { $album }.
    [many] أُضيفت { $count } صورةً إلى { $album }.
   *[other] أُضيفت { $count } صورة إلى { $album }.
}
albums-added-missing = { $count ->
    [zero] { $count } صورة ليست في المكتبة، لذا لم تُضَف.
    [one] صورة واحدة ليست في المكتبة، لذا لم تُضَف.
    [two] صورتان ليستا في المكتبة، لذا لم تُضافا.
    [few] { $count } صور ليست في المكتبة، لذا لم تُضَف.
    [many] { $count } صورةً ليست في المكتبة، لذا لم تُضَف.
   *[other] { $count } صورة ليست في المكتبة، لذا لم تُضَف.
}
albums-created = أُنشئ { $album }.
albums-removed = { $count ->
    [zero] أُزيلت { $count } صورة من { $album }.
    [one] أُزيلت صورة واحدة من { $album }.
    [two] أُزيلت صورتان من { $album }.
    [few] أُزيلت { $count } صور من { $album }.
    [many] أُزيلت { $count } صورةً من { $album }.
   *[other] أُزيلت { $count } صورة من { $album }.
}
albums-remove = إزالة
albums-remove-confirm = { $count ->
    [zero] إزالة { $count } صورة من { $album }؟
    [one] إزالة صورة واحدة من { $album }؟
    [two] إزالة صورتين من { $album }؟
    [few] إزالة { $count } صور من { $album }؟
    [many] إزالة { $count } صورةً من { $album }؟
   *[other] إزالة { $count } صورة من { $album }؟
}
albums-remove-confirm-detail = تبقى الملفات على القرص وفي المكتبة.
albums-delete = حذف
albums-delete-confirm = حذف الألبوم { $album }؟
albums-delete-group-confirm = { $count ->
    [zero] حذف المجموعة { $group } و{ $count } ألبوم فيها؟
    [one] حذف المجموعة { $group } وألبومها الوحيد؟
    [two] حذف المجموعة { $group } وألبوميها؟
    [few] حذف المجموعة { $group } و{ $count } ألبومات فيها؟
    [many] حذف المجموعة { $group } و{ $count } ألبومًا فيها؟
   *[other] حذف المجموعة { $group } و{ $count } ألبوم فيها؟
}
albums-delete-confirm-detail = تبقى الصور على القرص وفي المكتبة.
albums-deleted = { $count ->
    [zero] حُذف { $count } ألبوم.
    [one] حُذف الألبوم.
    [two] حُذف ألبومان.
    [few] حُذفت { $count } ألبومات.
    [many] حُذف { $count } ألبومًا.
   *[other] حُذف { $count } ألبوم.
}
albums-failed = تعذّر تغيير الألبومات: { $reason }

## Date timeline
albums-timeline = الخط الزمني
albums-timeline-description = اختر فترة لعرض صورها فقط. Shift+نقرة يختار عدة فترات متتالية.
albums-timeline-picked = يُعرض { $period }
albums-timeline-cleared = تُعرض كل التواريخ
