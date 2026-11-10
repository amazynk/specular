# machine-drafted, needs native review
# Specular — وضع الفرز (DESIGN.md §6.3).

palette-cull = الدخول إلى وضع الفرز أو الخروج منه
cheatsheet-cull = وضع الفرز
settings-section-cull = وضع الفرز
settings-cull-auto-enter = فرز مجلدات RAW
settings-cull-auto-enter-help = فتح مجلد معظم ملفاته RAW مباشرة في وضع الفرز.
settings-cull-auto-advance = الصورة التالية بعد الوسم
settings-cull-auto-advance-help = الانتقال بعد الاختيار أو الرفض أو التقييم بالنجوم. تبقى التسميات وإزالة الوسم على الصورة.
settings-cull-show = العرض لأزواج RAW+JPEG
settings-cull-show-help = يُفرز ملف RAW وملف JPEG بالاسم نفسه كصورة واحدة، وتنطبق العلامات على كليهما. يُفتح JPEG أسرع.
settings-cull-show-jpeg = JPEG
settings-cull-show-raw = RAW

# machine-drafted, needs native review
settings-cull-bursts = تجميع اللقطات المتتابعة
settings-cull-bursts-help = تُعرض الصور الملتقطة بتتابع سريع بكاميرا واحدة كمجموعة واحدة. S يفتح المجموعة أو يغلقها، وB يُبقي الصورة المعروضة ويرفض بقية المجموعة.
settings-cull-burst-gap = فاصل التتابع
settings-cull-burst-gap-help = أطول توقف بين لقطتين في تتابع واحد.
settings-cull-watch = عرض الملفات الجديدة
settings-cull-watch-help = الملفات التي تظهر في المجلد أثناء الفرز (كاميرا موصولة، استيراد) تُضاف في النهاية بعد اكتمال كتابتها.
settings-cull-follow-newest = متابعة أحدث ملف
settings-cull-follow-newest-help = الانتقال إلى كل ملف جديد عند وصوله. T يبدّل ذلك أثناء الفرز.
settings-cull-slot-1 = المجلد 1
settings-cull-slot-1-help = Alt+1 ينسخ الصورة (مع ملف RAW أو JPEG والملفات المرافقة) إلى هذا المجلد، وAlt+Shift+1 ينقلها إليه.
settings-cull-slot-1-unset = غير محدد
settings-cull-slot-2 = المجلد 2
settings-cull-slot-2-help = Alt+2 ينسخ الصورة (مع ملف RAW أو JPEG والملفات المرافقة) إلى هذا المجلد، وAlt+Shift+2 ينقلها إليه.
settings-cull-slot-2-unset = غير محدد
settings-cull-slot-3 = المجلد 3
settings-cull-slot-3-help = Alt+3 ينسخ الصورة (مع ملف RAW أو JPEG والملفات المرافقة) إلى هذا المجلد، وAlt+Shift+3 ينقلها إليه.
settings-cull-slot-3-unset = غير محدد
settings-cull-collisions = عندما يكون الاسم مستخدمًا
settings-cull-collisions-help = لا تُستبدل الملفات في المجلدات أبدًا.
settings-cull-collisions-suffix = إضافة رقم
settings-cull-collisions-skip = تخطي الصورة
# machine-drafted, needs native review
# اختيار وضع الفرز دون فتح مجلد أو صور معرض.
cull-nothing = لا شيء للفرز — افتح مجلد صور أولًا
cull-slot-unset = لم يُحدَّد المجلد { $slot } — اختر واحدًا في الإعدادات ← وضع الفرز
cull-slot-missing = المجلد { $slot } غير متاح: { $reason }
cull-copied = نُسخ إلى { $folder }
cull-moved = نُقل إلى { $folder }
cull-slot-renamed = حُفظ باسم { $name } في { $folder }
cull-slot-skipped = يحتوي { $folder } على { $name } بالفعل — تم التخطي
cull-slot-same = الصورة موجودة بالفعل في { $folder }
cull-slot-failed = { $op ->
    [move] تعذّر نقل { $file }
   *[copy] تعذّر نسخ { $file }
}: { $reason }
cull-copy-kept = أُبقيت نسخ تغيّرت منذ ذلك الحين: { $count }
cull-undo-failed = تعذّر التراجع لـ { $file }: { $reason }
cull-burst-open = تتابع { $index } من { $count }
cull-burst-closed = تتابع من { $count }
cull-burst-toggle = فتح التتابع أو إغلاقه
cull-burst-best = أُبقيت 1، ورُفضت { $count }
cull-burst-none = ليست جزءًا من تتابع
cull-follow-on = متابعة الملفات الجديدة
cull-follow-off = دون متابعة الملفات الجديدة
cull-new-files = ملفات جديدة: { $count }
cull-key-copy = نسخ إلى المجلد 1–3
cull-key-move = نقل إلى المجلد 1–3
cull-key-burst = فتح تتابع أو إغلاقه
cull-key-best = إبقاء هذه ورفض بقية التتابع
cull-key-follow = متابعة الملفات الجديدة
cull-key-subframes = إظهار جدول الإطارات الفرعية أو إخفاؤه
cull-key-sort = الفرز حسب العمود التالي
cull-key-sort-reverse = عكس الترتيب
cull-key-metric = عرض المقياس التالي في المخطط
cull-key-sigma-down = الرفض التلقائي: خفض k
cull-key-sigma-up = الرفض التلقائي: رفع k
cull-key-auto-reject = رفض الإطارات المقترحة
cull-key-move-rejects = نقل الإطارات المرفوضة إلى مجلد فرعي
cull-key-same-stretch = التمديد نفسه لكل الإطارات

## Strip tooltips. machine-drafted, needs native review
cull-tip-burst-open = فتح المجموعة المتتابعة
cull-tip-burst-close = إغلاق المجموعة المتتابعة
cull-tip-exit = مغادرة وضع الفرز
cull-tip-label = التسمية: { $label }

## Astronomy subframes. machine-drafted, needs native review
cull-astro-panel = الإطارات الفرعية
cull-astro-title = { $count ->
    [zero] { $count } إطار فرعي
    [one] إطار فرعي واحد
    [two] إطاران فرعيان
    [few] { $count } إطارات فرعية
    [many] { $count } إطارًا فرعيًا
   *[other] { $count } إطار فرعي
}
cull-astro-pending = جارٍ قياس { $count }
cull-astro-col-name = الإطار
cull-astro-col-filter = المرشّح
cull-astro-col-exposure = التعريض
cull-astro-col-hfd = HFD
cull-astro-col-fwhm = FWHM
cull-astro-col-ecc = اللامركزية
cull-astro-col-stars = النجوم
cull-astro-col-bg = الخلفية
cull-astro-col-snr = SNR
cull-astro-col-noise = الضجيج
cull-astro-same-stretch = التمديد نفسه
cull-astro-own-stretch = تمديد مستقل
cull-astro-suggest-high = { $metric } أعلى من { $value } (الوسيط + { $k }σ): { $count ->
    [zero] { $count } إطار
    [one] إطار واحد
    [two] إطاران
    [few] { $count } إطارات
    [many] { $count } إطارًا
   *[other] { $count } إطار
}
cull-astro-suggest-low = { $metric } أدنى من { $value } (الوسيط − { $k }σ): { $count ->
    [zero] { $count } إطار
    [one] إطار واحد
    [two] إطاران
    [few] { $count } إطارات
    [many] { $count } إطارًا
   *[other] { $count } إطار
}
cull-astro-suggest-none = لا إطار يتجاوز { $k }σ
cull-astro-suggest-few = يحتاج الرفض التلقائي إلى 3 إطارات مقيسة
cull-astro-suggested = مقترح للرفض
cull-astro-k = k { $k }
cull-astro-reject = رفض { $count }
cull-astro-move = نقل المرفوضة…
cull-astro-chart = { $metric } عبر { $count ->
        [zero] لا إطارات
        [one] إطار واحد
        [two] إطارين
        [few] { $count } إطارات
        [many] { $count } إطارًا
       *[other] { $count } من الإطارات
    }، الوسيط { $median }.
cull-astro-sort-by = الفرز حسب { $column }
cull-astro-ascending = تصاعديًا
cull-astro-descending = تنازليًا
cull-astro-sorted = مرتّبة حسب { $column }
cull-astro-reversed = عُكس الترتيب
cull-astro-metric = المخطط: { $metric }
cull-astro-k-set = الرفض التلقائي عند { $k }σ
cull-astro-none = لا إطارات فرعية هنا
cull-astro-same-on = التمديد نفسه لكل الإطارات
cull-astro-same-off = كل إطار يُمدَّد على حدة
cull-astro-line-hfd = HFD { $value } px
cull-astro-line-fwhm = FWHM { $value } px
cull-astro-line-fwhm-arcsec = FWHM { $value } px ({ $arcsec }″)
cull-astro-line-ecc = اللامركزية { $value }
cull-astro-line-stars = { $count ->
    [zero] { $count } نجم
    [one] نجم واحد
    [two] نجمان
    [few] { $count } نجوم
    [many] { $count } نجمًا
   *[other] { $count } نجم
}
cull-astro-measuring = جارٍ قياس النجوم…
cull-astro-reject-confirm = { $count ->
    [zero] رفض { $count } إطار مقترح؟
    [one] رفض الإطار المقترح؟
    [two] رفض الإطارين المقترحين؟
    [few] رفض { $count } إطارات مقترحة؟
    [many] رفض { $count } إطارًا مقترحًا؟
   *[other] رفض { $count } إطار مقترح؟
}
cull-astro-reject-detail = { $rule }. التراجع يعيد العلامات.
cull-astro-reject-ok = رفض
cull-astro-nothing-to-reject = لا إطار للرفض
cull-astro-rejected = { $count ->
    [zero] رُفض { $count } إطار
    [one] رُفض إطار واحد
    [two] رُفض إطاران
    [few] رُفضت { $count } إطارات
    [many] رُفض { $count } إطارًا
   *[other] رُفض { $count } إطار
}
cull-move-rejects-confirm = { $count ->
    [zero] نقل { $count } إطار مرفوض إلى المجلد «{ $folder }»؟
    [one] نقل الإطار المرفوض إلى المجلد «{ $folder }»؟
    [two] نقل الإطارين المرفوضين إلى المجلد «{ $folder }»؟
    [few] نقل { $count } إطارات مرفوضة إلى المجلد «{ $folder }»؟
    [many] نقل { $count } إطارًا مرفوضًا إلى المجلد «{ $folder }»؟
   *[other] نقل { $count } إطار مرفوض إلى المجلد «{ $folder }»؟
}
cull-move-rejects-detail = تُنقل الملفات ولا تُحذف. التراجع يعيدها.
cull-move-rejects-ok = نقل
cull-move-rejects-none = لا إطارات مرفوضة للنقل
cull-moved-rejects = { $count ->
    [zero] نُقل { $count } إطار إلى «{ $folder }»
    [one] نُقل إطار واحد إلى «{ $folder }»
    [two] نُقل إطاران إلى «{ $folder }»
    [few] نُقلت { $count } إطارات إلى «{ $folder }»
    [many] نُقل { $count } إطارًا إلى «{ $folder }»
   *[other] نُقل { $count } إطار إلى «{ $folder }»
}
