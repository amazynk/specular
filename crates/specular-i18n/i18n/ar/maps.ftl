# machine-drafted, needs native review
# Specular — Maps (the Maps profile): the map view of georeferenced and vector map files, its
# layer panel, attribute table, coordinates, measuring, elevation profile and map furniture.
# English.

keys-mode-map = الخرائط
# Cheat sheet groups of the Maps tab.
cheatsheet-map-layers = الطبقات
cheatsheet-map-coordinates = الإحداثيات
cheatsheet-map-measure = القياس

## Palette entries and their one-line descriptions.
palette-map-layers = إظهار لوحة الطبقات أو إخفاؤها
palette-map-layers-help = طبقات الخريطة: الترتيب والعتامة والظهور والنمط.
palette-map-add-layer = إضافة طبقة من ملف…
palette-map-add-layer-help = رسم ملف خريطة آخر فوق هذه الخريطة، مثل مسار GPX فوق ملف Shapefile.
palette-map-zoom-to-layer = التكبير إلى الطبقة
palette-map-zoom-to-layer-help = عرض الطبقة المختارة في لوحة الطبقات كاملةً.
palette-map-attribute-table = إظهار جدول السمات أو إخفاؤه
palette-map-attribute-table-help = معالم الطبقة المختارة في جدول: فرز وتصفية وبحث وتحديد.
palette-map-coordinate-format = تنسيق الإحداثيات التالي
palette-map-coordinate-format-help = درجات عشرية، أو درجات ودقائق وثوانٍ، أو UTM أو MGRS.
palette-map-units = وحدات مترية أو إمبراطورية
palette-map-units-help = الأطوال والمساحات بالأمتار أو بالأقدام والأميال.
palette-map-copy-coordinate = نسخ الإحداثي
palette-map-copy-coordinate-help = نسخ الموضع تحت المؤشر، أو موضع مركز الخريطة.
palette-map-go-to-coordinate = الانتقال إلى إحداثي…
palette-map-go-to-coordinate-help = اكتب موضعًا بأي تنسيق لتوسيط الخريطة عنده.
palette-map-measure-distance = قياس مسافة
palette-map-measure-distance-help = انقر على نقاط في الخريطة: الطول على سطح الأرض والاتجاه.
palette-map-measure-area = قياس مساحة
palette-map-measure-area-help = انقر على زوايا منطقة: مساحتها ومحيطها على سطح الأرض.
palette-map-elevation-profile = مقطع الارتفاع
palette-map-elevation-profile-help = ارسم خطًا فوق نموذج ارتفاعات لعرض ارتفاعاته في مخطط.
palette-map-overview = إظهار الخريطة العامة أو إخفاؤها
palette-map-overview-help = خريطة صغيرة للنطاق كله، مع تمييز الجزء المعروض على الشاشة.

## Why a map command is unavailable.
disabled-not-map = للخرائط فقط، مع تفعيل أدوات الخرائط
disabled-no-lonlat = ليس للخريطة نظام إحداثيات معروف
disabled-no-features = لا توجد معالم في الخريطة
disabled-no-heights = لا توجد ارتفاعات في الخريطة

## Settings › Maps.
settings-section-maps = الخرائط
settings-maps-coordinate-format = تنسيق الإحداثيات
settings-maps-coordinate-format-help = كيفية عرض المواضع ونسخها. يقرأ «الانتقال إلى إحداثي» كل التنسيقات.
settings-maps-coordinate-format-dd = درجات عشرية
settings-maps-coordinate-format-dms = درجات ودقائق وثوانٍ
settings-maps-coordinate-format-utm = UTM
settings-maps-coordinate-format-mgrs = MGRS / USNG
settings-maps-units = الوحدات
settings-maps-units-help = الأطوال والمساحات على الخرائط: شريط المقياس والقياسات. «تلقائي» يتبع البيانات الوصفية › الوحدات.
settings-maps-units-auto = تلقائي
settings-maps-units-metric = متري
settings-maps-units-imperial = إمبراطوري

## The pointer readout. $position is written in the chosen format.
map-position = { $format ->
    [utm] UTM { $position }
    [mgrs] MGRS { $position }
    [dms] { $position }
   *[dd] العرض، الطول { $position }
}
map-height = الارتفاع { $height }
map-height-label = الارتفاع
# A length or area with its unit symbol ("12.3 km", "4.5 ha").
map-length = { $value } { $unit }
map-scale-label = شريط المقياس: { $length }
map-north-label = سهم الشمال: الشمال على { $degrees }° باتجاه عقارب الساعة من الأعلى
# The coordinate formats, for the toast after "Next coordinate format".
map-format-name = { $format ->
    [dms] درجات ودقائق وثوانٍ
    [utm] UTM
    [mgrs] MGRS
   *[dd] درجات عشرية
}
map-format-toast = الإحداثيات بتنسيق { $format }
map-units-metric = الوحدات: أمتار وكيلومترات
map-units-imperial = الوحدات: أقدام وأميال
toast-copied-coordinate = نُسخ الإحداثي

## Go to coordinate.
map-goto-hint = درجات عشرية أو DMS أو UTM أو MGRS. Enter ينتقل إليه.
map-goto-empty = اكتب موضعًا.
map-goto-unrecognised = ليس موضعًا بتنسيق معروف.
map-goto-range = خط العرض أو خط الطول خارج النطاق.
map-goto-no-way = لا تستطيع هذه الخريطة تحديد موضع.
map-goto-outside = هذا الموضع خارج الخريطة.
map-goto-toast = تم التوسيط على { $position }

## The layer panel.
map-layers-title = الطبقات: { $shown } من { $total } معروضة
# Under a layer's name: its kind, features and opacity.
map-layer-sub = { $kind } · { $features } · { PERCENT($opacity) }
# A layer's row for screen readers.
map-layer-row = { $name }: { $kind }، { $features ->
    [zero] { $features } معلم
    [one] معلم واحد
    [two] معلمان
    [few] { $features } معالم
    [many] { $features } معلمًا
   *[other] { $features } معلم
}، العتامة { PERCENT($opacity) }
map-kind-area = مساحات
map-kind-line = خطوط
map-kind-point = نقاط
map-kind-empty = لا معالم
map-layer-visible = إظهار الطبقة أو إخفاؤها
map-layer-opacity-down = عتامة أقل
map-layer-opacity-up = عتامة أكثر
map-layer-colour = اللون التالي
map-layer-width = عرض الخط التالي
map-layer-point = حجم النقطة التالي
map-layer-up = نقل لأعلى
map-layer-down = نقل لأسفل
map-layer-remove = إزالة
map-layer-keep = إبقاء
map-layer-remove-ask = إزالة { $name } من الخريطة؟ يبقى الملف على القرص.
map-layer-adding = جارٍ إضافة طبقة…
map-removed = أُزيل { $name } من الخريطة
map-added = { $layers ->
    [zero] أُضيف { $name }: { $layers } طبقة
    [one] أُضيف { $name }
    [two] أُضيف { $name }: طبقتان
    [few] أُضيف { $name }: { $layers } طبقات
    [many] أُضيف { $name }: { $layers } طبقة
   *[other] أُضيف { $name }: { $layers } طبقة
}
map-add-failed = تتعذّر إضافة { $name }: { $reason }
map-add-filter = ملفات الخرائط
map-add-not-map = ليس خريطة.
map-add-not-vector = لا يحتوي على معالم.
map-add-no-crs = لا يحتوي على ملف إسقاط (.prj).

## Identify.
map-identify-more = و{ $count } أخرى في جدول السمات
map-cell = الخلية { $x }، { $y }
map-band = النطاق { $n }
map-no-data = لا بيانات

## The attribute table.
map-table-title = { $layer }: { $shown } من { $total } معلم، { $selected } محدد
map-table-search = بحث، أو حقل = قيمة
map-table-selected-only = المحدد فقط
map-table-empty = لا توجد معالم مطابقة.
map-table-many = كثير
map-table-stats-number = { $field }: القيم { $count }، الفارغة { $empty } · الأدنى { $min } · الأعلى { $max } · المتوسط { $mean } · المجموع { $sum } · الانحراف المعياري { $sd }
map-table-stats-text = { $field }: القيم { $count }، الفارغة { $empty } · المختلفة { $distinct }
map-table-stats-text-top = { $field }: القيم { $count }، الفارغة { $empty } · المختلفة { $distinct } · الأكثر تكرارًا { $top } ({ $times }×)

## Measuring and the elevation profile.
map-measure-hint-distance = انقر على نقاط على طول الطريق.
map-measure-hint-area = انقر على زوايا المنطقة.
map-measure-hint-profile = انقر على نقاط على طول الخط.
map-measure-keys = Enter ينهي. Backspace يزيل آخر نقطة. Esc يغلق.
map-measure-distance = المسافة { $length }
map-measure-bearing = الاتجاه { $bearing }
map-measure-segment = الجزء الأخير { $length }، الاتجاه { $bearing }
map-measure-area = المساحة { $area }
map-measure-perimeter = المحيط { $length }
map-bearing-value = °{ $degrees }
map-profile-summary = الطول { $length } · الأدنى { $min } · الأعلى { $max } · الصعود { $climb } · الهبوط { $descent }
map-profile-no-heights = الطول { $length }. لا ارتفاعات على طول الخط.
map-profile-at = { $distance }: { $height }
map-profile-at-none = { $distance }: لا ارتفاع
