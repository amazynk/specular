# تقارير DICOM المهيكلة، والأشكال الموجية، وجرعة العلاج الإشعاعي، والتصوير المقطعي للعين. machine-drafted, needs native review

## شجرة محتوى التقرير المهيكل (مستعرض الوسوم في وضع التقرير)
sr-tree-title = { $title } ({ $kind })
sr-col-item = البند
sr-col-concept = المفهوم
sr-col-type = النوع
sr-col-relationship = العلاقة
sr-col-value = القيمة
sr-show-image = عرض الصورة
sr-jump-missing = الصورة المشار إليها ليست في هذا المجلد
sr-jump-frame = لا توجد صورة في هذا الإطار المرجعي تحتوي على تلك النقطة

## الرسوم البيانية (مخططات الجرعة والحجم، أوراق تخطيط القلب)
plot-caliper-time = { $ms } ms · { $rate } /دقيقة
plot-caliper-time-zero = 0 ms
plot-caliper-hint = اسحب عبر رسم بياني للقياس
plot-adjust-hint = + و− يغيّران الكسب، و[ و] السرعة

## ورقة تخطيط القلب
ecg-paper = { $speed } mm/s · { $gain } mm/mV · { $rate } Hz
ecg-groups-more = { $count ->
    [zero] لا توجد مجموعات قنوات أخرى مخفية
    [one] مجموعة قنوات أخرى غير معروضة
    [two] مجموعتا قنوات أخريان غير معروضتين
    [few] { $count } مجموعات قنوات أخرى غير معروضة
    [many] { $count } مجموعة قنوات أخرى غير معروضة
   *[other] { $count } من مجموعات القنوات الأخرى غير معروضة
}

## جرعة العلاج الإشعاعي فوق MPR (خطوط تساوي الجرعة، خريطة الألوان، المفتاح، القراءة)
dose-value = الجرعة { $value } { $unit } ({ PERCENT($percent) })
dose-hidden = الجرعة مخفية (Shift+U يعرضها)
dose-on-rx = الجرعة: { $dose } { $unit } موصوفة · الحد الأقصى { $max } { $unit }
dose-on-max = الجرعة: نسبة مئوية من الحد الأقصى، { $max } { $unit }
dose-colour = الجرعة بألوان { $colormap } بنسبة { PERCENT($blend) }
dose-dvh-hint = { $count ->
    [zero] Shift+K: لا توجد مخططات جرعة-حجم
    [one] Shift+K: مخطط الجرعة-الحجم لبنية واحدة
    [two] Shift+K: مخططا الجرعة-الحجم لبنيتين
    [few] Shift+K: مخططات الجرعة-الحجم لـ { $count } بنى
    [many] Shift+K: مخططات الجرعة-الحجم لـ { $count } بنية
   *[other] Shift+K: مخططات الجرعة-الحجم لـ { $count } من البنى
}
dose-level = { PERCENT($percent) } · { $dose } { $unit }
dose-mode-lines = الجرعة كخطوط تساوي الجرعة
dose-mode-colour = الجرعة كخريطة ألوان

## مخططات الجرعة-الحجم (ورقة الرسم البياني)
dvh-title = مخططات الجرعة-الحجم (DVH)
dvh-col-roi = البنية
dvh-col-volume = الحجم
dvh-axis-dose = الجرعة ({ $unit })
dvh-axis-volume = الحجم (%)
dvh-prescription = الجرعة الموصوفة { $dose } Gy · تراكمي
dvh-max = الجرعة القصوى { $dose } { $unit } · تراكمي
dvh-none = لا توجد مجموعة بنى في هذا الإطار المرجعي لحساب مخطط الجرعة عليها

## التصوير المقطعي للعين (منزلق المسح B؛ صورة التحديد في الزاوية)
oct-bscan = مسح B
oct-depth = بعمق { $depth } µm
