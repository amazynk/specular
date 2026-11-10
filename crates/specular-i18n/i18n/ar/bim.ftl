# machine-drafted, needs native review
## مراجعة النماذج ثلاثية الأبعاد: السير والطيران، مستويات المقطع وصندوق المقطع، العروض المحفوظة،
## مواضيع BCF (وضع ثلاثي الأبعاد). المفاتيح المذكورة هي المفاتيح الافتراضية.

cheatsheet-review = المراجعة

palette-mesh-walk = السير
palette-mesh-fly = الطيران
palette-mesh-views = العروض المحفوظة
palette-mesh-save-view = حفظ العرض
palette-mesh-section-plane = إضافة مستوى مقطع
palette-mesh-section-box = صندوق المقطع
palette-mesh-section-flip = قلب مستوى المقطع
palette-mesh-section-clear = إزالة كل المقاطع
palette-mesh-measure-cut = القياس على المقطع
palette-mesh-bcf-topics = مواضيع BCF
# machine-drafted, needs native review
palette-mesh-model-checks = فحوص النموذج
# machine-drafted, needs native review
palette-mesh-models = النماذج في المشهد
# machine-drafted, needs native review
palette-mesh-add-model = إضافة نموذج…
# machine-drafted, needs native review
palette-mesh-add-model-folder = إضافة نماذج من مجلد…

# السير والطيران: إشعارات وأسطر القراءة، حسب $what. $eye: ارتفاع العين فوق الأرضية مع
# وحدته (walk-line فقط).
mesh-nav = { $what ->
    [walk-toast] السير: W A S D للتحرك، والسحب للنظر، وEsc للتوقف
    [fly-toast] الطيران: W A S D للتحرك، وQ E للنزول والصعود، وEsc للتوقف
    [orbit] الدوران
    [walk-line] السير · العين على ارتفاع { $eye } فوق الأرضية
    [walk-keys] W A S D تحرك · السحب للنظر · Shift أسرع · Esc توقف
    [preparing] الجدران غير جاهزة بعد: ستعبر من خلالها
    [fly-keys] W A S D تحرك · Q E نزول، صعود · السحب للنظر · Shift أسرع · Esc توقف
   *[fly-line] الطيران
}

# نصوص قصيرة للمقاطع والقياس والعروض المحفوظة، حسب $what.
bim-note = { $what ->
    [plane-added] أُضيف مستوى مقطع: اسحب مقبضه لتحريكه
    [box-on] صندوق المقطع مفعّل: اسحب مقبض وجه لتحريكه
    [box-off] صندوق المقطع معطّل
    [cleared] أُزيلت المقاطع
    [measure-on] انقر نقطتين على المقطع
    [measure-needs-cut] أضف مستوى مقطع أو الصندوق أولًا
    [measure-off-cut] انقر على المقطع
    [measure-hint] القياس: انقر نقطتين على المقطع
    [ask-view-name] اسم العرض
    [ask-rename] الاسم الجديد
    [views-empty] لا عروض محفوظة. اضغط N لحفظ هذا العرض.
    [views-hints] Enter انتقال · N حفظ · F2 إعادة تسمية · Delete إزالة · P حفظ بصيغة PNG
    [delete-detail] يُزال العرض المحفوظ فقط. لا يتغير النموذج.
    [delete-ok] إزالة
    [not-over-model] اختر ملفًا آخر: هذا الملف سيحل محل النموذج.
   *[view-name] عرض
}

# نصوص BCF (BIM Collaboration Format) القصيرة، حسب $what.
bcf-note = { $what ->
    [title-none] مواضيع BCF
    [new-file] جديد
    [empty] لا مواضيع. اضغط O لفتح ملف BCF أو N لإضافة موضوع.
    [hints] Enter انتقال · ← → نقاط العرض · N موضوع · C تعليق · S الحالة · E العنوان · O فتح · Ctrl+S تصدير
    [ask-title] عنوان الموضوع
    [ask-comment] تعليق
    [untitled] بلا عنوان
    [topic-added] أُضيف الموضوع مع هذا العرض
   *[comment-added] أُضيف التعليق مع هذا العرض
}

# $planes: عدد مستويات المقطع.
bim-sections-line = { $planes ->
    [0] لا مستويات مقطع
    [zero] لا مستويات مقطع
    [one] مستوى مقطع واحد
    [two] مستويا مقطع
    [few] { $planes } مستويات مقطع
    [many] { $planes } مستوى مقطع
   *[other] { $planes } مستوى مقطع
}
# $size: حجم صندوق المقطع مع وحدته.
bim-box-line = الصندوق { $size }
# $count: أقصى عدد ممكن من المستويات.
bim-planes-full = { $count } مستويات مقطع على الأكثر
# مقبض سحب، لقارئات الشاشة. $n: رقمه.
bim-handle = { $kind ->
    [box] وجه صندوق المقطع { $n }
   *[plane] مستوى المقطع { $n }
}
# $length: الطول مع وحدته.
bim-measure-line = الطول على المقطع: { $length }
bim-views-title = { $count ->
    [zero] لا عروض محفوظة
    [one] عرض محفوظ واحد
    [two] عرضان محفوظان
    [few] { $count } عروض محفوظة
    [many] { $count } عرضًا محفوظًا
   *[other] { $count } عرض محفوظ
}
# $name: اسم عرض.
bim-view-saved = تم حفظ «{ $name }»
bim-view-delete-title = إزالة العرض «{ $name }»؟
# $file: ملف مكتوب.
bim-saved-file = تم حفظ { $file }
# $reason: سبب الفشل.
bim-failed = { $what ->
    [open] تعذّر فتح ملف BCF: { $reason }
    [views] العروض محفوظة لهذه الجلسة فقط: { $reason }
   *[save] تعذّر الحفظ: { $reason }
}
# $count: المواضيع؛ $file: اسم ملف الأرشيف.
bcf-title = { $count ->
    [zero] لا مواضيع
    [one] موضوع واحد
    [two] موضوعان
    [few] { $count } مواضيع
    [many] { $count } موضوعًا
   *[other] { $count } موضوع
} · { $file }
bcf-comments = { $count ->
    [zero] لا تعليقات
    [one] تعليق واحد
    [two] تعليقان
    [few] { $count } تعليقات
    [many] { $count } تعليقًا
   *[other] { $count } تعليق
}
# $at: نقطة العرض المعروضة؛ $count: نقاط عرض الموضوع.
bcf-viewpoint-of = نقطة العرض { $at } من { $count }
bcf-opened = { $count ->
    [zero] لم يُفتح أي موضوع
    [one] فُتح موضوع واحد
    [two] فُتح موضوعان
    [few] فُتحت { $count } مواضيع
    [many] فُتح { $count } موضوعًا
   *[other] فُتح { $count } موضوع
}

## Several IFC models in one scene (the Models list, Alt+Shift+M).

# The list's title. $count: models in the scene.
# machine-drafted, needs native review
fed-title = { $count ->
    [zero] النماذج: 0
    [one] النماذج: 1
    [two] النماذج: 2
    [few] النماذج: { $count }
    [many] النماذج: { $count }
   *[other] النماذج: { $count }
}
# machine-drafted, needs native review
fed-note = { $what ->
    [not-ifc] هذا الملف ليس نموذج IFC.
    [not-ifc-open] افتح نموذج IFC أولًا. لا يمكن دمج إلا نماذج IFC.
    [this-model] هذا النموذج
    [hints] A إضافة نموذج · Shift+A إضافة مجلد · Space إظهار أو إخفاء · C التلوين حسب النموذج · Enter انتقال · Delete إزالة
    [origin-ok] الوضع عند نقطة الأصل
    [first-stays] يبقى النموذج الذي فتحته. افتح ملفًا آخر لتغييره.
    [remove-detail] يخرج من المشهد. يبقى الملف على القرص.
   *[remove-ok] إزالة
}
# A model's row. $basis: how it was placed; $elements: its elements; $shown: shown or hidden.
# machine-drafted, needs native review
fed-row = { $basis } · { $elements ->
    [zero] لا عناصر
    [one] عنصر واحد
    [two] عنصران
    [few] { $elements } عناصر
    [many] { $elements } عنصرًا
   *[other] { $elements } عنصر
} · { $shown ->
    [hidden] مخفي
   *[shown] ظاهر
}
# How a model was placed against the first. $crs: the coordinate system's name.
# machine-drafted, needs native review
fed-basis = { $basis ->
    [reference] النموذج المفتوح
    [map-crs] حسب إحداثيات الخريطة ({ $crs })
    [map] حسب إحداثيات الخريطة
    [site] حسب خط عرض الموقع وخط طوله (تقريبي)
    [project] حسب إحداثيات المشروع المشتركة
   *[origin] عند نقطة الأصل (غير محاذى)
}
# Why a model cannot be aligned. $first, $this: coordinate system names.
# machine-drafted, needs native review
fed-unaligned = { $why ->
    [crs] تستخدم النماذج أنظمة إحداثيات مختلفة: { $first } و{ $this }.
    [only-first] النموذج المفتوح مرجعي جغرافيًا، وهذا النموذج ليس كذلك.
    [only-this] هذا النموذج مرجعي جغرافيًا، والنموذج المفتوح ليس كذلك.
   *[broken] لا يمكن استخدام تحويل الخريطة لهذا النموذج (مقياسه صفر).
}
# The chosen model's placement. $distance: how far it moved; $angle: degrees turned.
# machine-drafted, needs native review
fed-placed = نُقل { $distance }، ودُوّر { $angle }°
# machine-drafted, needs native review
fed-adding = { $count ->
    [zero] قراءة { $count } نموذج…
    [one] قراءة نموذج واحد…
    [two] قراءة نموذجين…
    [few] قراءة { $count } نماذج…
    [many] قراءة { $count } نموذجًا…
   *[other] قراءة { $count } نموذج…
}
# machine-drafted, needs native review
fed-ask-origin = تعذّرت محاذاة { $file }. أتضعه عند نقطة الأصل؟
# machine-drafted, needs native review
fed-no-files = لا توجد ملفات IFC في { $folder }.
# machine-drafted, needs native review
fed-not-added = لم يُضف { $file }: { $reason }
# machine-drafted, needs native review
fed-added = أُضيف { $file }. عدد النماذج في المشهد: { $count }.
# machine-drafted, needs native review
fed-remove-title = إزالة { $file } من المشهد؟
# machine-drafted, needs native review
fed-removed = أُزيل { $file } من المشهد.
# machine-drafted, needs native review
fed-shown = { $file } { $shown ->
    [hidden] مخفي
   *[shown] ظاهر
}
# machine-drafted, needs native review
fed-colours = { $on ->
    [on] التلوين حسب النموذج مفعّل
   *[off] التلوين حسب النموذج متوقف
}

## PMI (product manufacturing information) of a CAD model: dimensions, geometric tolerances,
## datums and the file's saved views (mesh mode).

# machine-drafted, needs native review
palette-mesh-pmi = PMI والعروض المحفوظة
# $count: annotations in the model.
# machine-drafted, needs native review
pmi-title = { $count ->
    [zero] لا تعليقات توضيحية
    [one] تعليق توضيحي واحد
    [two] تعليقان توضيحيان
    [few] { $count } تعليقات توضيحية
    [many] { $count } تعليقًا توضيحيًا
   *[other] { $count } تعليق توضيحي
}
# machine-drafted, needs native review
pmi-view = عرض محفوظ
# machine-drafted, needs native review
pmi-view-all = يُظهر كل التعليقات التوضيحية
# $count: annotations the saved view shows.
# machine-drafted, needs native review
pmi-view-shows = { $count ->
    [zero] لا يُظهر أي تعليق توضيحي
    [one] يُظهر تعليقًا توضيحيًا واحدًا
    [two] يُظهر تعليقين توضيحيين
    [few] يُظهر { $count } تعليقات توضيحية
    [many] يُظهر { $count } تعليقًا توضيحيًا
   *[other] يُظهر { $count } تعليق توضيحي
}
# $name: the saved view's name.
# machine-drafted, needs native review
pmi-view-shown = يُعرض العرض «{ $name }». A يُظهر الكل.
# machine-drafted, needs native review
pmi-has-faces = الأوجه مُبرزة
# Short texts, by $what.
# machine-drafted, needs native review
pmi-note = { $what ->
    [none] لا توجد PMI في هذا النموذج.
    [hints] ↑ ↓ اختيار · Enter انتقال · A الكل · D كما رُسمت · Shift+A إخفاء
    [hidden] التعليقات التوضيحية مخفية
    [shown] التعليقات التوضيحية ظاهرة
    [as-drawn] التعليقات التوضيحية كما يرسمها الملف
    [as-labels] التعليقات التوضيحية كتسميات
   *[all-shown] كل التعليقات التوضيحية ظاهرة
}
# What an annotation is, by $what.
# machine-drafted, needs native review
pmi-kind = { $what ->
    [dimension] بُعد
    [tolerance] تفاوت هندسي
    [datum] مرجع
    [target] هدف مرجعي
   *[graphic] تعليق توضيحي مرسوم
}
# machine-drafted, needs native review
pmi-dimension = { $what ->
    [linear] مسافة
    [angular] زاوية
    [diameter] قطر
    [radius] نصف قطر
    [length] طول
    [thickness] سماكة
   *[other] بُعد
}
# Geometric characteristics (ISO 1101), by $what.
# machine-drafted, needs native review
pmi-characteristic = { $what ->
    [straightness] الاستقامة
    [flatness] الاستواء
    [circularity] الاستدارة
    [cylindricity] الأسطوانية
    [line-profile] شكل الخط
    [surface-profile] شكل السطح
    [angularity] الميل
    [perpendicularity] التعامد
    [parallelism] التوازي
    [position] الموضع
    [concentricity] التمركز
    [coaxiality] اتحاد المحور
    [symmetry] التماثل
    [circular-runout] الخفقان الدائري
   *[total-runout] الخفقان الكلي
}
