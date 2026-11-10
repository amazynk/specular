# Specular — Drawings (DXF): layers, paper, lineweights, model space and layouts. العربية.

keys-mode-drawing = الرسوم الهندسية
cheatsheet-drawing = الرسوم الهندسية
palette-drawing-layers = إظهار لوحة الطبقات أو إخفاؤها
palette-drawing-all-layers = إظهار كل الطبقات، أو الطبقات كما حُفظت
palette-drawing-paper = التبديل بين الورق الداكن والفاتح
palette-drawing-lineweights = إظهار سماكات الخطوط أو إخفاؤها
disabled-not-drawing = للرسوم الهندسية فقط (DXF)
# A drawing's page: model space (layouts are shown by their own name).
page-label-model = النموذج
# machine-drafted, needs native review
cad-pen-name = القلم { $n }
cad-page-name = الصفحة { $n }
cad-layer-name = الطبقة { $n }
cad-map-name = خريطة
# machine-drafted, needs native review
# A board's views (its Gerber files together): every layer stacked, and its top and bottom
# in board colours.
cad-board-layers-name = الطبقات
cad-board-top-name = الوجه العلوي
cad-board-bottom-name = الوجه السفلي
# machine-drafted, needs native review
# A plot's layers: its frame, a trace without a name (numbered from 1), and the contour
# lines of a 2-D spectrum above and below zero.
cad-chart-frame-name = الإطار
cad-trace-name = المنحنى { $n }
cad-contours-positive-name = الخطوط الكنتورية الموجبة
cad-contours-negative-name = الخطوط الكنتورية السالبة
# machine-drafted, needs native review
# The page of a survey file (LandXML) showing its surfaces and alignments in 3D.
cad-model-3d-name = نموذج ثلاثي الأبعاد
# The layers panel's title (and its name for screen readers).
drawing-layers-title = الطبقات · { $shown } من { $total } ظاهرة
# Toasts.
drawing-layers-all-toast = كل الطبقات ظاهرة
drawing-layers-saved-toast = الطبقات كما حُفظت في الرسم
drawing-paper-dark-toast = ورق داكن
drawing-paper-light-toast = ورق فاتح
drawing-lineweights-on = سماكات الخطوط ظاهرة
drawing-lineweights-off = سماكات الخطوط مطفأة: خطوط رفيعة

# machine-drafted, needs native review
# Markup on drawings and images (Alt+M): kept in a file next to the drawing, never in it.
palette-markup = إضافة علامات المراجعة
palette-markup-help = سحب ونص وأسهم وأختام وقياسات ودبابيس صور، تُحفظ بجانب الملف.
palette-compare-sheets = مقارنة اللوحات
palette-compare-sheets-help = تراكب إصدار آخر من هذه اللوحة، بعد محاذاته، مع تحديد التغييرات.
markup-title = علامات المراجعة
markup-list = العلامات في هذه الصفحة
markup-count = { $count ->
    [0] لا توجد علامات في هذه الصفحة
    [zero] لا توجد علامات في هذه الصفحة
    [one] عنصر واحد في هذه الصفحة
    [two] عنصران في هذه الصفحة
    [few] { $count } عناصر في هذه الصفحة
    [many] { $count } عنصرًا في هذه الصفحة
   *[other] { $count } عنصر في هذه الصفحة
}
markup-empty = اختر أداة وانقر على اللوحة.
markup-item = { $n }. { $kind }
markup-tool-select = تحديد
markup-tool-cloud = سحابة
markup-tool-text = نص
markup-tool-arrow = سهم
markup-tool-stamp = ختم
markup-tool-length = الطول
markup-tool-area = المساحة
markup-tool-pin = دبوس صورة
markup-hint-select = انقر على عنصر لتحديده. اسحبه لنقله. يحرّر Enter نصه.
markup-hint-cloud = اسحب مستطيلًا، أو انقر على الزوايا واضغط Enter.
markup-hint-text = انقر حيث يوضع النص.
markup-hint-arrow = اسحب من التسمية إلى النقطة، أو انقر على كلتيهما.
markup-hint-stamp = انقر حيث يوضع الختم.
markup-hint-length = اسحب أو انقر على الطرفين.
markup-hint-area = انقر على الزوايا ثم اضغط Enter.
markup-hint-pin = انقر على نقطة ثم اختر صورة.
markup-kind-cloud = سحابة
markup-kind-text = نص
markup-kind-arrow = سهم
markup-kind-stamp = ختم
markup-kind-length = طول
markup-kind-area = مساحة
markup-kind-pin = صورة
markup-colour = اللون: { $colour }
markup-colour-red = أحمر
markup-colour-blue = أزرق
markup-colour-green = أخضر
markup-colour-orange = برتقالي
markup-colour-magenta = أرجواني
markup-colour-black = أسود
markup-width = الخط: { $width }
markup-width-thin = رفيع
markup-width-medium = متوسط
markup-width-thick = سميك
markup-stamp = الختم: { $words }
markup-stamp-approved = معتمد
markup-stamp-rejected = مرفوض
markup-stamp-reviewed = تمت المراجعة
markup-stamp-custom = نص من اختيارك
markup-undo = تراجع
markup-redo = إعادة
markup-delete = حذف
markup-open-photo = فتح الصورة
markup-from-measurements = إضافة القياسات
markup-from-measurements-done = { $count ->
    [zero] لم يُضف أي قياس كعلامة
    [one] أُضيف قياس واحد كعلامة
    [two] أُضيف قياسان كعلامتين
    [few] أُضيفت { $count } قياسات كعلامات
    [many] أُضيف { $count } قياسًا كعلامات
   *[other] أُضيف { $count } قياس كعلامات
}
markup-export-pdf = تصدير PDF…
markup-export-png = تصدير PNG…
markup-done = تم
markup-ask-text = النص (Enter للوضع، Esc للإلغاء)
markup-ask-label = التسمية (اختيارية؛ Enter للوضع)
markup-ask-stamp = نص الختم (Enter للوضع)
markup-ask-edit = النص الجديد (Enter للحفظ)
markup-added = أُضيف: { $kind }
markup-moved = نُقل
markup-pin-hint = انقر على الدبوس مرة أخرى أو اضغط Enter لفتح صورته.
markup-deleted = حُذف
markup-undone = تم التراجع
markup-redone = أُعيد
markup-delete-ask = حذف { $name }؟
markup-delete-detail = يُزال من ملف العلامات. لا يتغير الرسم نفسه.
markup-delete-ok = حذف
markup-photo-missing = لم يعد { $name } موجودًا.
markup-load-failed = لم تُقرأ العلامات: { $reason }
markup-save-failed = لم تُحفظ العلامات: { $reason }
markup-sidecar-too-large = ملف العلامات كبير جدًا
markup-pdf-unreadable = تعذّرت قراءة ملف PDF
markup-exported = حُفظ { $name }
markup-export-failed = لم يُصدَّر: { $reason }
markup-export-same-file = اختر اسمًا آخر؛ لا يُستبدل الأصل أبدًا
markup-export-encrypted = ملف PDF مشفّر. صدّر PNG بدلًا منه.
markup-export-damaged = بنية ملف PDF تالفة. صدّر PNG بدلًا منه.
sheets-title = مقارنة اللوحات
sheets-other = لوحة أخرى…
sheets-other-named = الأخرى: { $name }
sheets-page = الصفحة { $page } من { $count }
sheets-auto = محاذاة
sheets-pick = تحديد نقطتين
sheets-overlay = تراكب
sheets-difference = الفرق
sheets-blend = تلاشٍ
sheets-blink = وميض
sheets-slider = اللوحة الأخرى { PERCENT($percent) }
sheets-clouds = { $count ->
    [0] لا توجد تغييرات
    [zero] لا توجد تغييرات
    [one] إحاطة تغيير واحد بسحابة
    [two] إحاطة تغييرين بسحب
    [few] إحاطة { $count } تغييرات بسحب
    [many] إحاطة { $count } تغييرًا بسحب
   *[other] إحاطة { $count } تغيير بسحب
}
sheets-done = تم
sheets-choose = اختر اللوحة الأخرى.
sheets-reading = جارٍ قراءة اللوحة…
sheets-aligning = جارٍ المحاذاة…
sheets-aligned = تمت المحاذاة: المقياس { $scale }، الدوران { $angle }°، الانحراف ضمن { $error } px. { $changes ->
    [0] لم يُعثر على تغييرات.
    [zero] لم يُعثر على تغييرات.
    [one] تغيير واحد.
    [two] تغييران.
    [few] { $changes } تغييرات.
    [many] { $changes } تغييرًا.
   *[other] { $changes } تغيير.
}
sheets-align-failed = تعذّرت المحاذاة تلقائيًا. حدّد نقطتين.
sheets-pick-this = النقطة { $n }: انقر عليها في هذه اللوحة.
sheets-pick-other = النقطة { $n }: انقر على النقطة نفسها في اللوحة الأخرى.
sheets-pick-same = النقطتان متطابقتان. حدّد نقطتين من جديد.
sheets-read-failed = لم تُقرأ اللوحة: { $reason }
sheets-failed = فشلت هذه الخطوة. حاول مرة أخرى أو اختر لوحة أخرى.
sheets-legend-this = هذه اللوحة فقط
sheets-legend-other = اللوحة الأخرى فقط
sheets-legend-both = كلتاهما
sheets-cloud-text = تغيّر
sheets-clouds-added = { $count ->
    [0] لم تُضف أي سحب
    [zero] لم تُضف أي سحب
    [one] أُضيفت سحابة واحدة إلى العلامات
    [two] أُضيفت سحابتان إلى العلامات
    [few] أُضيفت { $count } سحب إلى العلامات
    [many] أُضيفت { $count } سحابةً إلى العلامات
   *[other] أُضيفت { $count } سحابة إلى العلامات
}
sheets-opened = مقارنة اللوحات
sheets-closed = أُغلقت المقارنة
markup-ask-new-line = Shift+Enter: سطر جديد

## Settings › Drawings

# machine-drafted, needs native review
settings-section-drawings = الرسوم الهندسية
settings-drawings-shx-folders = مجلدات خطوط SHX
settings-drawings-shx-folders-help = مجلدات تحتوي خطوط SHX لنصوص DXF وDWG، مفصولة بالرمز ;. يُبحث في مجلد الرسم نفسه أولًا.
# A folder in the SHX font folders setting that does not exist or is not a folder.
settings-error-shx-folder = ليس مجلدًا: { $folder }
