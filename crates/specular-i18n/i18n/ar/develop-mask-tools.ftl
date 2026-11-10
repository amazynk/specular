# machine-drafted, needs native review
# Specular — أقنعة التحميض. العربية.

develop-mask-kind-details = التفاصيل
develop-mask-kind-borrowed = قناع وحدة أخرى
develop-mask-kind-group = قناع منسوخ
develop-mask-row-from = قناع { $module } { $n }
develop-mask-from = استخدام القناع من
develop-mask-from-option = { $module } { $n }
develop-mask-used-by =
    { $count ->
        [zero] تستخدم { $modules } هذا القناع. ستحتفظ كل منها بنسخة منه.
        [one] تستخدم { $modules } هذا القناع. ستحتفظ بنسخة منه.
        [two] تستخدم { $modules } هذا القناع. سيحتفظ كل منهما بنسخة منه.
        [few] تستخدم { $modules } هذا القناع. ستحتفظ كل منها بنسخة منه.
        [many] تستخدم { $modules } هذا القناع. ستحتفظ كل منها بنسخة منه.
       *[other] تستخدم { $modules } هذا القناع. ستحتفظ كل منها بنسخة منه.
    }

develop-mask-refine-radius = نصف قطر الحافة
develop-mask-grow = توسيع الحافة
develop-mask-blur = تمويه القناع
develop-mask-details-keep = التفاصيل فقط (+) أو المساحات المستوية فقط (−)
develop-mask-details-threshold = العتبة
develop-mask-details-softness = النعومة

develop-mask-point-feather = تنعيم النقطة { $n }
develop-mask-point-added = أُضيفت النقطة { $n }.
develop-mask-point-removed = أُزيلت النقطة { $n }.
develop-mask-point-last = يحتاج المضلع إلى ثلاث نقاط.
develop-mask-point-choose = اختر نقطة أولًا: انقر عليها أو اضغط Tab.

develop-mask-brush-pressure = ضغط القلم
develop-mask-brush-pressure-flow = التدفق
develop-mask-brush-pressure-size = الحجم
develop-mask-brush-pressure-both = كلاهما
develop-mask-brush-smoothing = التنعيم
develop-brush-size-now = حجم الفرشاة { $percent }
develop-brush-feather-now = حافة الفرشاة { $percent }
develop-brush-erasing = الفرشاة تمسح.
develop-brush-painting = الفرشاة ترسم.

develop-mask-overlay-on = يُعرض قناع { $module }.
develop-mask-overlay-off = أُخفي القناع.
develop-mask-overlay-none = لا يوجد قناع لعرضه.

develop-mask-preset = إعداد قناع مسبق
develop-mask-preset-save = حفظ القناع كإعداد مسبق…
develop-mask-preset-save-title = حفظ القناع كإعداد مسبق
develop-mask-preset-default-name = قناع { $number }
develop-mask-preset-saved = حُفظ إعداد القناع { $name }.
develop-mask-preset-applied = طُبّق إعداد القناع { $name }.
develop-mask-preset-delete = حذف الإعداد { $name }
develop-mask-preset-delete-confirm = حذف إعداد القناع { $name }؟ تحتفظ الصور التي طُبّق عليها بأقنعتها.
develop-mask-preset-delete-ok = حذف
develop-mask-preset-deleted = حُذف إعداد القناع { $name }.

palette-develop-brush-smaller = فرشاة أصغر
palette-develop-brush-smaller-help = تصغير فرشاة القناع.
palette-develop-brush-larger = فرشاة أكبر
palette-develop-brush-larger-help = تكبير فرشاة القناع.
palette-develop-brush-softer = فرشاة أنعم
palette-develop-brush-softer-help = منح فرشاة القناع حافة أنعم.
palette-develop-brush-harder = فرشاة أصلب
palette-develop-brush-harder-help = منح فرشاة القناع حافة أصلب.
palette-develop-brush-erase = رسم أو مسح
palette-develop-brush-erase-help = تبديل فرشاة القناع بين الرسم والمسح.
palette-develop-mask-overlay = إظهار القناع
palette-develop-mask-overlay-help = إظهار قناع الوحدة قيد التحرير أو إخفاؤه.
