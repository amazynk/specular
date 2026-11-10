# Linked views: several DICOM series side by side, scrolling together by position, with
# reference lines, a 3D cursor, window and zoom sync, and manual alignment of priors.

# machine-drafted, needs native review
palette-compare-series = مقارنة السلاسل (عروض مرتبطة)
# machine-drafted, needs native review
palette-compare-layout = تخطيط منافذ العرض التالي
# machine-drafted, needs native review
palette-next-viewport = منفذ العرض التالي
# machine-drafted, needs native review
palette-prev-viewport = منفذ العرض السابق
# machine-drafted, needs native review
palette-reference-lines = إظهار الخطوط المرجعية أو إخفاؤها
# machine-drafted, needs native review
palette-sync-window = مزامنة النافذة التالية
# machine-drafted, needs native review
palette-sync-zoom = التكبير والتحريك معًا حسب الحجم
# machine-drafted, needs native review
palette-link-series = ربط السلسلة التالي
# machine-drafted, needs native review
palette-align-series = محاذاة السلاسل الأخرى هنا
# machine-drafted, needs native review
palette-compare-series-help = عرض عدة سلاسل جنبًا إلى جنب مع التمرير معًا.
# machine-drafted, needs native review
palette-compare-layout-help = عرض عدد أكبر أو أقل من منافذ العرض.
# machine-drafted, needs native review
palette-next-viewport-help = جعل منفذ العرض التالي هو النشط.
# machine-drafted, needs native review
palette-prev-viewport-help = جعل منفذ العرض السابق هو النشط.
# machine-drafted, needs native review
palette-reference-lines-help = رسم موضع تقاطع المقطع النشط مع منافذ العرض الأخرى.
# machine-drafted, needs native review
palette-sync-window-help = اختيار منافذ العرض التي تشترك في تغيير النافذة/المستوى: متوقفة، أو النمط نفسه، أو الكل.
# machine-drafted, needs native review
palette-sync-zoom-help = عرض كل منافذ العرض بالحجم نفسه وتحريكها معًا.
# machine-drafted, needs native review
palette-link-series-help = اختيار كيفية متابعة هذه السلسلة للسلاسل الأخرى.
# machine-drafted, needs native review
palette-align-series-help = محاذاة سلسلة سابقة مع هذه السلسلة عند المقاطع المعروضة.
# machine-drafted, needs native review
disabled-not-comparing = قارن السلاسل أولًا

# The bar above the viewports. Toggles show their state by colour; the others name it.
# machine-drafted, needs native review
linked-layout = التخطيط: { $cols } × { $rows }
# machine-drafted, needs native review
linked-lines = الخطوط المرجعية
# machine-drafted, needs native review
linked-zoom = مزامنة التكبير
# machine-drafted, needs native review
linked-window-sync = مزامنة النافذة: { $mode ->
    [off] متوقفة
    [modality] النمط نفسه
   *[all] الكل
}
# machine-drafted, needs native review
linked-link = الربط: { $mode ->
    [frame] الإطار المرجعي
    [position] وضع المريض
   *[off] متوقف
}
# machine-drafted, needs native review
linked-align = حاذِ هنا

# One viewport. $active is "yes" for the active one.
# machine-drafted, needs native review
linked-viewport = { $active ->
    [yes] منفذ العرض { $n } من { $count }، نشط
   *[no] منفذ العرض { $n } من { $count }
}
# machine-drafted, needs native review
linked-empty = اسحب سلسلة إلى هنا
# machine-drafted, needs native review
linked-series = { $number } · { $modality } · { $description }
# machine-drafted, needs native review
linked-slice = المقطع { $n } من { $count } · { $position } مم
# machine-drafted, needs native review
linked-in-gap = داخل فجوة: أقرب مقطع على بعد { $distance } مم
# machine-drafted, needs native review
linked-outside = خارج هذه السلسلة ({ $distance } مم)
# machine-drafted, needs native review
linked-not-linked = غير مرتبطة
# machine-drafted, needs native review
linked-aligned = محاذاة يدويًا ({ $offset } مم)
# machine-drafted, needs native review
linked-cursor = المؤشر ثلاثي الأبعاد عند { $x }، { $y }، { $z } مم
# machine-drafted, needs native review
linked-chip = { $label }، { $count ->
        [zero] لا صور
        [one] صورة واحدة
        [two] صورتان
        [few] { $count } صور
        [many] { $count } صورة
       *[other] { $count } من الصور
    }

# Toasts and announcements.
# machine-drafted, needs native review
linked-opened = { $count ->
    [zero] لا توجد سلاسل للمقارنة
    [one] توجد سلسلة واحدة فقط هنا: اسحب أخرى من متصفح الدراسات
    [two] مقارنة سلسلتين
    [few] مقارنة { $count } سلاسل
    [many] مقارنة { $count } سلسلة
   *[other] مقارنة { $count } سلسلة
}
# machine-drafted, needs native review
linked-no-geometry = ليس لهذه السلسلة مواضع مقاطع: لا يمكن ربطها
# machine-drafted, needs native review
linked-active = منفذ العرض { $n }: { $series }
# machine-drafted, needs native review
linked-aligned-count = { $count ->
    [0] لا شيء للمحاذاة: السلاسل الأخرى تشترك في هذا الإطار المرجعي
    [zero] لا شيء للمحاذاة: السلاسل الأخرى تشترك في هذا الإطار المرجعي
    [one] تمت محاذاة سلسلة واحدة مع السلسلة النشطة، لهذه الجلسة
    [two] تمت محاذاة سلسلتين مع السلسلة النشطة، لهذه الجلسة
    [few] تمت محاذاة { $count } سلاسل مع السلسلة النشطة، لهذه الجلسة
    [many] تمت محاذاة { $count } سلسلة مع السلسلة النشطة، لهذه الجلسة
   *[other] تمت محاذاة { $count } سلسلة مع السلسلة النشطة، لهذه الجلسة
}
