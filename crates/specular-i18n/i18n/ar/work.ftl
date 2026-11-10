# machine-drafted, needs native review
# Specular — الأعمال في الخلفية: المؤشر الذي يظهر أثناء تشغيل شيء في الخلفية، واللوحة التي يفتحها. العربية.

## اللوحة

work-title = الأعمال في الخلفية
work-show = عرض الأعمال في الخلفية
work-empty = لا شيء قيد التشغيل في الخلفية.

## الأعمال

work-kind-library = مكتبة الصور
work-kind-ai-search = البحث بالذكاء الاصطناعي
work-kind-faces = الأشخاص
work-kind-text = النص في الصور
work-kind-culling = درجات الفرز
work-kind-download = تنزيل نموذج الذكاء الاصطناعي
work-kind-similar = لقطات متشابهة

## الخطوات

work-step-finding = جارٍ العثور على الصور
work-step-reading = جارٍ قراءة تفاصيل الصور
work-step-signatures = جارٍ مقارنة الصور
work-step-grouping = جارٍ تجميع اللقطات المتشابهة
work-step-text-index = جارٍ إنشاء فهرس البحث
work-step-embedding = جارٍ فهرسة الصور للبحث بالذكاء الاصطناعي
work-step-switching = جارٍ إعادة الفهرسة لحجم النموذج الجديد
work-step-faces = جارٍ العثور على الوجوه
work-step-ocr = جارٍ قراءة النص في الصور
work-step-scoring = جارٍ تقييم الحدة والتعريض
work-step-downloading = جارٍ التنزيل
work-step-focus = جارٍ التحقق من الحدة
work-step-item = { $step } · { $item }

## التقدم

work-count-photos = { $done } من { $total } { $count ->
    [zero] صورة
    [one] صورة
    [two] صورة
    [few] صور
    [many] صورة
   *[other] صورة
}
work-count = { $done } من { $total }
work-count-left = { $count } · يتبقى { $left }
work-so-far-photos = { $done } { $count ->
    [zero] صورة
    [one] صورة
    [two] صورة
    [few] صور
    [many] صورة
   *[other] صورة
} حتى الآن
work-so-far = { $done } حتى الآن

## الوقت المتبقي

work-eta-estimating = جارٍ التقدير…
work-eta-seconds = الوقت المتبقي: { $n ->
    [zero] { $n } ثانية
    [one] ثانية واحدة
    [two] ثانيتان
    [few] { $n } ثوانٍ
    [many] { $n } ثانية
   *[other] { $n } ثانية
} تقريبًا
work-eta-minutes = الوقت المتبقي: { $n ->
    [zero] { $n } دقيقة
    [one] دقيقة واحدة
    [two] دقيقتان
    [few] { $n } دقائق
    [many] { $n } دقيقة
   *[other] { $n } دقيقة
} تقريبًا
work-eta-hours = الوقت المتبقي: { $h ->
    [zero] { $h } ساعة
    [one] ساعة واحدة
    [two] ساعتان
    [few] { $h } ساعات
    [many] { $h } ساعة
   *[other] { $h } ساعة
} تقريبًا
work-eta-hours-minutes = الوقت المتبقي: { $h ->
    [zero] { $h } ساعة
    [one] ساعة واحدة
    [two] ساعتان
    [few] { $h } ساعات
    [many] { $h } ساعة
   *[other] { $h } ساعة
} و{ $m ->
    [zero] { $m } دقيقة
    [one] دقيقة واحدة
    [two] دقيقتان
    [few] { $m } دقائق
    [many] { $m } دقيقة
   *[other] { $m } دقيقة
} تقريبًا

## سبب انتظار العمل

work-paused-browsing = متوقف مؤقتًا أثناء التصفح
work-paused-battery = متوقف مؤقتًا أثناء العمل على البطارية
work-paused-low-power = متوقف مؤقتًا في وضع توفير الطاقة
work-paused-heat = متوقف مؤقتًا ما دام الحاسوب ساخنًا
work-paused-memory = متوقف مؤقتًا ما دامت الذاكرة منخفضة
work-paused-user = متوقف مؤقتًا

## ما يلي

work-then = ثم: { $steps }

## الأزرار

work-pause = إيقاف مؤقت
work-resume = استئناف
work-settings = الإعدادات…

## المؤشر

work-chip = { $job } · { $done } من { $total }
work-chip-so-far = { $job } · { $done }
work-chip-paused = { $job } · { $reason }
work-chip-more = { $chip } (+{ $more })
work-chip-name = الأعمال في الخلفية: { $status }

## الإعلانات

work-started = بدأ { $job } في الخلفية
work-finished = اكتمل { $job }
