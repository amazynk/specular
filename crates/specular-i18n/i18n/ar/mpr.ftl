# Multiplanar reformatting (MPR).

palette-mpr = إعادة البناء متعددة المستويات (MPR)
palette-mpr-slab = شريحة MPR التالية (رفيعة، MIP، MinIP، متوسط)
disabled-no-volume = هذا ليس حجمًا: افتح سلسلة أو ملفًا متعدد الإطارات أو حجمًا ثلاثي الأبعاد أو مكدس z
disabled-mpr-closed = افتح العروض متعددة المستويات أولًا
mpr-axial = محوري
mpr-coronal = إكليلي
mpr-sagittal = سهمي
mpr-oblique = مائل
mpr-xy = XY
mpr-xz = XZ
mpr-yz = YZ
mpr-3d = عرض ثلاثي الأبعاد
mpr-thin = رفيعة
mpr-mip = MIP
mpr-minip = MinIP
mpr-average = متوسط
mpr-slab = الشريحة: { $mode }، { $thickness } مم
# machine-drafted, needs native review
mpr-slab-angstrom = الشريحة: { $mode }، { $thickness } Å
mpr-slab-unit = الشريحة: { $mode }، { $thickness } { $unit }
mpr-slab-voxels = الشريحة: { $mode }، { $thickness } فوكسل
mpr-thicker = شريحة أسمك
mpr-thinner = شريحة أرفع
mpr-reset = إعادة تعيين العروض
mpr-close = إغلاق MPR
mpr-maximise = تكبير العرض
mpr-export-planes = تصدير المستويات
# machine-drafted, needs native review
# Streamlines (TrackVis .trk, MRtrix .tck) found beside a brain volume, drawn over each
# view within a slab about its plane; Alt+S steps the slab, then hides them. $files: the
# files' names; $thickness: the slab in millimetres.
mpr-tracts = خطوط انسياب { $files }: شريحة { $thickness } مم
mpr-tracts-off = خطوط انسياب { $files }: مخفية
mpr-tracts-none = لا توجد خطوط انسياب TrackVis أو MRtrix بجانب هذا الحجم
mpr-position = { $view } { $position } مم
# machine-drafted, needs native review
mpr-position-angstrom = { $view } { $position } Å
mpr-position-unit = { $view } { $position } { $unit }
mpr-position-voxels = { $view } { $position }
mpr-length-unit = { $value } { $unit }
mpr-length-voxels = { $value } فوكسل
mpr-axis-label = { $axis }: { $label }
mpr-axis-position = { $axis }: { $position } من { $count }
mpr-axis-channel = القناة
mpr-axis-unnamed = المحور { $axis }
mpr-one-channel = يعرض MPR قناة واحدة في كل مرة
# machine-drafted, needs native review
# $count: channels drawn together in their colours.
mpr-composite = القنوات الملوّنة: { $count }، كلٌّ بنطاق عرضه
# $reason: error-reason-needs-memory ("it needs 6 GB of free RAM, but only 4 GB is free right now. ...").
mpr-composite-memory = تُعرض قناة واحدة: { $reason }
mpr-isotropic = لا يوجد حجم فوكسل في الملف: تُعرض الفوكسلات كمكعبات
mpr-isotropic-z = لا يوجد تباعد شرائح في الملف: تُعرض الشرائح بفارق بكسل واحد
# A seismic volume's inline and crossline sections (the time slice reads "Time … ms").
mpr-seismic-inline = الخط الداخلي { $inline }
mpr-seismic-crossline = الخط العرضي { $crossline }
mpr-value = { $value } { $unit }
mpr-value-plain = { $value }
mpr-loading = جارٍ تحميل المقاطع: { $loaded } من { $count }
mpr-loaded = المقاطع: { $count }، { $width } × { $height }
mpr-downsampled = معروض بدقة 1/{ $factor } (حدود الذاكرة أو الرسومات)
mpr-converted = القيم معروضة كأعداد عشرية 32 بت
mpr-opening = جارٍ فتح الحجم…
mpr-gpu-memory = { $size } على بطاقة الرسومات
mpr-refused = تعذّر بناء حجم: { $reason }
mpr-failed = لم يُحمَّل الحجم: { $reason }
mpr-no-gpu = لا تتسع بطاقة الرسومات لهذا الحجم: { $reason }
mpr-planes-saved = { $count ->
    [zero] لم يُحفظ أي مستوى في { $folder }
    [one] حُفظ مستوى واحد في { $folder }
    [two] حُفظ مستويان في { $folder }
    [few] حُفظت { $count } مستويات في { $folder }
    [many] حُفظ { $count } مستوى في { $folder }
   *[other] حُفظ { $count } مستوى في { $folder }
}
mpr-planes-failed = تعذّر حفظ المستويات: { $reason }
mpr-view-a11y = عرض { $view }، { $position } مم
mpr-crosshair-a11y = التقاطع عند { $x }, { $y }, { $z } مم
# machine-drafted, needs native review
mpr-crosshair-a11y-angstrom = التقاطع عند { $x }, { $y }, { $z } Å
mpr-crosshair-a11y-unit = التقاطع عند { $x }, { $y }, { $z } { $unit }
mpr-crosshair-a11y-voxels = التقاطع عند الفوكسل { $x }, { $y }, { $z }
mpr-controls = عناصر تحكم MPR

# 3D view palette entries.
palette-mpr-preset = الإعداد المسبق التالي للعرض ثلاثي الأبعاد (CT عظام، أنسجة رخوة، رئة، أوعية؛ MR)
palette-mpr-render = طريقة العرض ثلاثي الأبعاد التالية (مظلل، Cinematic، MIP، MinIP)
palette-mpr-clip = القص ثلاثي الأبعاد التالي (بلا قص، صندوق العروض ثنائية الأبعاد، زاوية مقطوعة)
palette-mpr-export-stl = تصدير السطح ثلاثي الأبعاد بصيغة STL…
# machine-drafted, needs native review
palette-mpr-tract-slab = شريحة خطوط الانسياب التالية في MPR
palette-mpr-maximise = تكبير عرض MPR أو إظهار كل العروض
palette-mpr-export-planes = تصدير مستويات MPR كصور…

# The 3D view: transfer function presets, renderings, clipping; STL surface export.
vr-preset = { $preset ->
    [bone] CT عظام
    [soft] CT أنسجة رخوة
    [lung] CT رئة
    [angio] CT أوعية
   *[mr] MR
}
vr-mode = { $mode ->
    [cinematic] Cinematic
    [mip] MIP
    [minip] MinIP
   *[shaded] مظلل
}
vr-clip = { $clip ->
    [box] القص: صندوق العروض
    [cut] القص: زاوية مقطوعة
   *[off] بلا قص
}
vr-smooth = تنعيم السطح
vr-export = تصدير STL
vr-threshold = السطح عند { $value }
vr-stl-exporting = جارٍ بناء السطح…
vr-stl-saved = { $count ->
    [0] لا شيء عند هذه العتبة: لم يُحفظ أي سطح
   *[other] حُفظ في { $name }، المثلثات: { $count }
}
vr-stl-failed = تعذّر حفظ السطح: { $reason }
vr-view-a11y = { $preset }، { $mode }، { $clip }، السطح عند { $threshold }. Shift+الأسهم للدوران، وزائد وناقص للتكبير والتصغير.

## حجم خام: كيف تُرتَّب بيانات ملف .raw / .vol بلا ترويسة

# machine-drafted, needs native review
raw-volume-title = فتح حجم خام
raw-volume-file = لا يحتوي { $file } على ترويسة. حدّد كيف رُتّبت عيّناته.
raw-volume-guess-label = الترتيب المقترح
raw-volume-guess = { $index } من { $count }
raw-volume-custom = مخصّص
raw-volume-width = العرض (فوكسل)
raw-volume-height = الارتفاع (فوكسل)
raw-volume-slices = المقاطع
raw-volume-type = نوع العيّنة
raw-volume-type-unsigned = عدد صحيح بلا إشارة { $bits } بت
raw-volume-type-signed = عدد صحيح بإشارة { $bits } بت
raw-volume-type-float = فاصلة عائمة { $bits } بت
raw-volume-order = ترتيب البايتات
raw-volume-little = Little-endian
raw-volume-big = Big-endian
raw-volume-offset = إزاحة الترويسة
raw-volume-bytes = { $count ->
    [zero] صفر بايت
    [one] بايت واحد
    [two] بايتان
    [few] { $count } بايتات
    [many] { $count } بايتًا
   *[other] { $count } بايت
}
raw-volume-pixel = حجم البكسل
raw-volume-spacing = تباعد المقاطع
raw-volume-mm = { $value } مم
raw-volume-exact = يملأ الملف تمامًا ({ $size })
raw-volume-fits = يستخدم { $size } من { $file }؛ ويتبقى { $left }
raw-volume-too-big = يحتاج إلى { $size }، لكن حجم الملف { $file }
raw-volume-cancel = إلغاء
raw-volume-open = فتح
# machine-drafted, needs native review
# A dense connectivity file (CIFTI .dconn.nii) over voxels: the volume shown is one seed's
# row; letting go of the crosshair on another seed's voxel shows that one. $seed: the
# structure as the file names it and the voxel, such as "THALAMUS_LEFT (4, 6, 3)".
mpr-dconn-seed = اتصالية { $seed }؛ حرّك التقاطع إلى نقطة بذرة أخرى
# machine-drafted, needs native review
mpr-measure-hint-edit = اسحب طرفًا لتحريكه، أو إلى الطرف الآخر لإزالته
# machine-drafted, needs native review
mpr-measure-removed = أُزيل القياس
mpr-measure-lengths-only = يقيس MPR الأطوال فقط
mpr-more = المزيد…

## Curved MPR: a reformat along a curve clicked on a plane.
## machine-drafted, needs native review

palette-mpr-curve = MPR منحنٍ على طول منحنى مرسوم بالنقر
palette-mpr-curve-layout = MPR منحنٍ: مُقوَّم أو ممدود
palette-mpr-curve-save = حفظ المنحنى…
palette-mpr-curve-load = تحميل منحنى…
palette-mpr-export-segments = تصدير كل قطعة بصيغة STL…
palette-mpr-dental = تخطيط الأسنان (بانورامي ومقاطع عرضية)
palette-mpr-nerve = تتبّع القناة الفكية السفلية
cpr-kind = { $kind ->
    [stretched] ممدود
   *[straightened] مُقوَّم
}
cpr-button = منحنٍ
cpr-controls = عناصر تحكم MPR المنحني
cpr-at = عند { $position }
cpr-section = مقطع عرضي
cpr-hint-start = انقر نقاطًا على طول البنية في عرض { $view }
cpr-hint-more = انقر نقطة أخرى
cpr-add-points = إضافة نقاط
cpr-undo-point = التراجع عن النقطة
# machine-drafted, needs native review
cpr-mark = ضع علامة هنا
cpr-mark-set = تم وضع العلامة { $n }.
cpr-mark-removed = أُزيلت العلامة.
cpr-mark-here = العلامة { $n }
cpr-diameter = القطر { $value }
cpr-diameters = القطر من { $min } إلى { $max }
cpr-mark-span = العلامة { $from } إلى { $to }: { $length } على طول الخط المركزي
cpr-mark-total = كل العلامات: { $length }
cpr-clear = مسح المنحنى
cpr-save = حفظ المنحنى
cpr-load = تحميل المنحنى
cpr-done = تم
cpr-a11y = { $name } على طول { $length }، المؤشر عند { $position }. العجلة تحرّك المؤشر.
cpr-none = لا منحنى بعد: انقر نقاطًا أولًا
cpr-cleared = مُسح المنحنى
cpr-saved = حُفظ المنحنى: { $name }
cpr-save-failed = تعذّر حفظ المنحنى: { $reason }
cpr-loaded = حُمّل المنحنى
# machine-drafted, needs native review
# $count: centrelines read from the file.
cpr-loaded-count = { $count ->
    [zero] لم يُحمَّل أي خط مركزي
    [one] حُمّل خط مركزي واحد
    [two] حُمّل خطان مركزيان
    [few] حُمّلت { $count } خطوط مركزية
    [many] حُمّل { $count } خطًا مركزيًا
   *[other] حُمّل { $count } خط مركزي
}
# $count: centrelines found in files saved beside the volume.
cpr-sidecars-found = { $count ->
    [zero] لم يُعثر على أي خط مركزي محفوظ
    [one] عُثر على خط مركزي محفوظ واحد
    [two] عُثر على خطين مركزيين محفوظين
    [few] عُثر على { $count } خطوط مركزية محفوظة
    [many] عُثر على { $count } خطًا مركزيًا محفوظًا
   *[other] عُثر على { $count } خط مركزي محفوظ
}
# A centreline without a name, by its place in the list.
cpr-centreline = الخط المركزي { $number }
cpr-new = خط مركزي جديد
cpr-rename = إعادة التسمية
cpr-name = اسم الخط المركزي
cpr-delete = حذف
# $name: the centreline's name.
cpr-delete-title = حذف الخط المركزي «{ $name }»؟
cpr-delete-detail = تُحذف نقاطه. تبقى الملفات المحفوظة على القرص.
cpr-deleted = حُذف «{ $name }»
cpr-load-failed = تعذّر تحميل المنحنى: { $reason }
cpr-load-not-json = الملف ليس JSON
cpr-load-not-curves = الملف ليس منحنى Specular
cpr-load-empty = لا يحوي الملف منحنى من نقطتين أو أكثر
cpr-panoramic = بانورامي
cpr-panoramic-slab = بانورامي، شريحة { $thickness } مم
cpr-panoramic-thin = بانورامي، رفيع

## The dental layout (Medical).
## machine-drafted, needs native review

dental-button = الأسنان
dental-not-for-diagnosis = غير مخصص للتشخيص أو لتخطيط العلاج
dental-nerve = القناة الفكية السفلية
dental-nerve-hint = انقر نقاطًا على طول القناة الفكية السفلية في الصورة البانورامية أو في مقطع عرضي
dental-nerve-done = أُوقف تتبّع القناة الفكية السفلية
dental-spacing = كل { $spacing } مم
dental-section = المقطع { $number }
dental-off = افتح تخطيط الأسنان أولًا
dental-suggesting = جارٍ البحث عن القوس السنية…
dental-suggested = اقتُرحت قوس: اسحب نقاطها لضبطها
dental-no-arch = لم يُعثر على قوس: انقر نقاطًا على طولها في العرض المحوري
dental-vendor = CBCT أسنان من { $vendor }

## Each segment as its own STL.
## machine-drafted, needs native review

seg-stl-export-button = تصدير القطع
seg-stl-detail = القطع: عدد المثلثات { $count }
seg-stl-detail-full = القطع: التفاصيل كاملة
seg-stl-none = لا قطع للتصدير
seg-stl-not-labels = هذا الحجم ليس خريطة تسميات
seg-stl-exporting = جارٍ بناء أسطح القطع…
seg-stl-saved = { $count ->
    [zero] لم تُحفظ أي قطعة في { $folder }
    [one] حُفظت قطعة واحدة في { $folder }
    [two] حُفظت قطعتان في { $folder }
    [few] حُفظت { $count } قطع في { $folder }
    [many] حُفظت { $count } قطعة في { $folder }
   *[other] حُفظت { $count } قطعة في { $folder }
}
seg-stl-open = غير مغلقة: { $count }.
seg-stl-empty = القطع فارغة: لم يُحفظ شيء
seg-stl-failed = تعذّر حفظ القطع: { $reason }
# machine-drafted, needs native review
toolbar-mpr = ثلاثة مستويات (MPR)
toolbar-mpr-close = إغلاق المستويات الثلاثة (MPR)
settings-viewing-toolbar-items-mpr = ثلاثة مستويات (MPR)
