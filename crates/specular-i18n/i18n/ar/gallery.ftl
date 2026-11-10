# machine-drafted, needs native review
# Specular — المعرض (عرض الصور المصغّرة، DESIGN §10) والإعدادات ← المعرض. العربية الفصحى.

## لوحة الأوامر والقوائم

palette-gallery = المعرض
gallery-palette-view = عرض المعرض…
gallery-palette-layout = التخطيط
gallery-palette-sort = الترتيب حسب
gallery-palette-group = التجميع حسب
gallery-palette-filter = تصفية…
gallery-palette-quick-filters = عوامل تصفية سريعة
gallery-palette-details = عرض التفاصيل
gallery-palette-info-overlay = معلومات على الصور المصغّرة
gallery-palette-stacks = تكديس RAW+JPEG واللقطات المتتابعة
gallery-palette-subfolders = تضمين المجلدات الفرعية
gallery-palette-compare = مقارنة المحدد
gallery-palette-rate = التقييم…
gallery-palette-settings = إعدادات المعرض…
gallery-view-menu = عرض
gallery-more-menu = المزيد
gallery-open = فتح
gallery-new-window = فتح في نافذة جديدة
gallery-reveal = إظهار في المجلد
gallery-properties = الخصائص
gallery-copy-paths = نسخ المسارات
gallery-select-all = تحديد الكل
gallery-select-none = إلغاء التحديد
gallery-select-invert = عكس التحديد
gallery-next-unrated = التالية غير المقيّمة
gallery-stack-toggle = توسيع الكومة أو طيّها
gallery-run-on = { $name } (عدد الملفات: { $count })
gallery-depth-off = هذا المجلد فقط
gallery-depth-all = كل المجلدات الفرعية
gallery-depth-levels = مستويات المجلدات الفرعية: { $depth }

## الحالة والصفحات الفارغة

gallery-items = العناصر: { $count }
gallery-selected = المحدد: { $count } من { $total }
gallery-shown-of = المعروض: { $count } من { $total }
gallery-listing = جارٍ قراءة المجلد… { $count }
gallery-empty = لا توجد صور في هذا المجلد.
gallery-empty-filter = لا توجد صور تطابق عامل التصفية.
gallery-list-error = تتعذّر قراءة هذا المجلد: { $reason }
gallery-no-folder = لا يوجد مجلد مفتوح
gallery-kind-folder = مجلد
gallery-drag-count = عدد الملفات: { $count }
gallery-badge-pages = الصفحات: { $count }

## شريط التصفية

# machine-drafted, needs native review
# The filter field takes words (matched against names) and field:value terms; the field
# names and words of the query (rating, camera, ext, is:pick, …) stay in English.
gallery-filter-query-placeholder = الاسم، أو rating>=3 camera:X-T5 …
gallery-filter-clear = مسح عامل التصفية
gallery-filter-rating = ★ { $stars } فأكثر
gallery-filter-label = التسمية: { $label }
gallery-filter-picks = المختارة
gallery-filter-hide-rejects = إخفاء المرفوضة
gallery-filter-unrated = غير مقيّمة
gallery-filter-raw = RAW
gallery-filter-animated = متحركة
gallery-filter-hdr = HDR
gallery-filter-landscape = أفقية
gallery-filter-portrait = عمودية

## Query errors and facets

gallery-query-unknown-field = حقل غير معروف «{ $field }»
gallery-query-unknown-field-suggest = حقل غير معروف «{ $field }»: هل تقصد «{ $suggestion }»؟
gallery-query-missing-value = «{ $field }» يحتاج إلى قيمة
gallery-query-bad-number = «{ $value }» ليس رقمًا ({ $field })
gallery-query-bad-date = «{ $value }» ليس تاريخًا ({ $field }): استخدم 2024 أو 2024-05 أو 2024-05-12
gallery-query-bad-word = «{ $value }» لا يناسب { $field }: استخدم { $words }
# machine-drafted, needs native review
gallery-query-bad-exif-tag = «{ $value }» ليس اسم وسم Exif
gallery-query-no-compare = لا يمكن مقارنة { $field } هنا باستخدام < أو >
gallery-query-unterminated = علامة الاقتباس الختامية مفقودة
gallery-query-unavailable = «{ $field }» يتطلب المكتبة (غير متاح هنا)
gallery-facets = الأوجه
gallery-facets-counting = جارٍ العد…
gallery-facets-empty = لا شيء للعد
gallery-facets-search = البحث في الأوجه
gallery-facets-no-match = لا توجد قيم مطابقة
# The gallery's top bar button (its tooltip): back to the start screen.
gallery-home = شاشة البدء
gallery-facet-format = التنسيق
gallery-facet-camera = الكاميرا
gallery-facet-lens = العدسة
gallery-facet-rating = التقييم
gallery-facet-year = السنة
gallery-facet-orientation = الاتجاه
gallery-facet-label = التسمية
gallery-facet-properties = الخصائص
gallery-facet-square = مربعة
gallery-facet-alpha = الشفافية
gallery-facet-gps = الموقع
gallery-facet-multipage = صفحات متعددة
# Screen readers: a facet value, e.g. “Camera: X-T5, 12 files”.
gallery-facet-a11y-value = { $facet }: { $value }, { $count ->
    [zero] لا ملفات
    [one] ملف واحد
    [two] ملفان
    [few] { $count } ملفات
    [many] { $count } ملفًا
   *[other] { $count } ملف
}

## المجموعات

# An event group's header: the first and the last day of a shooting session.
gallery-group-event = { $start } – { $end }
gallery-group-event-place = { $place } · { $dates }
gallery-group-this-folder = هذا المجلد
gallery-group-unknown = غير معروف
gallery-group-unrated = غير مقيّمة

## الفرز

gallery-stars = النجوم: { $stars }
gallery-rate-none = بلا تقييم
gallery-pick = اختيار
gallery-reject = رفض
gallery-unflag = بلا علامة
gallery-label-red = أحمر
gallery-label-yellow = أصفر
gallery-label-green = أخضر
gallery-label-blue = أزرق
gallery-label-none = بلا تسمية
gallery-compare-hint = حدّد من 2 إلى 4 صور لمقارنتها.

## الملفات

gallery-trash-count = نقل الملفات إلى سلة المهملات: { $count }
gallery-moved-to = الملفات المنقولة إلى { $folder }: { $count }
gallery-copied-to = الملفات المنسوخة إلى { $folder }: { $count }
gallery-move-failed = تعذّر نقل { $file }: { $reason }
gallery-drop-move = نقل إلى { $folder }
gallery-drop-copy = نسخ إلى { $folder }
gallery-move-here = نقل العناصر المحددة إلى هنا
gallery-copy-here = نسخ العناصر المحددة إلى هنا
gallery-not-an-image = ‏{ $file } ليس صورة يستطيع { -app-name } فتحها.
gallery-cache-cleared = مُسحت ذاكرة الصور المصغّرة المؤقتة
# machine-drafted, needs native review
gallery-thumb-failed = لا صورة مصغّرة: { $reason }
gallery-cache-clear-failed = تعذّر مسح ذاكرة الصور المصغّرة المؤقتة: { $reason }

## الخصائص

gallery-prop-name = الاسم
gallery-prop-folder = المجلد
gallery-prop-size = حجم الملف
gallery-prop-dimensions = الأبعاد
gallery-prop-format = الصيغة
gallery-prop-date = التاريخ
gallery-prop-camera = الكاميرا
gallery-prop-lens = العدسة
gallery-prop-rating = التقييم
gallery-prop-label = التسمية
gallery-prop-items = العناصر

## قارئات الشاشة

gallery-a11y-list = { $folder }، العناصر: { $count }
gallery-a11y-dims = { $width } في { $height }
gallery-a11y-folder = مجلد
gallery-a11y-up = الصعود إلى { $folder }
gallery-a11y-archive = أرشيف
gallery-a11y-stack = كومة من { $count }
gallery-a11y-filter-bar = التصفية
gallery-a11y-filter-field = التصفية حسب الاسم أو الحقل
gallery-a11y-chip = شرط التصفية { $term }
gallery-a11y-chip-remove = اضغط للإزالة
gallery-a11y-results = { $count ->
    [zero] لا توجد عناصر مطابقة
    [one] عنصر واحد مطابق
    [two] عنصران مطابقان
    [few] { $count } عناصر مطابقة
    [many] { $count } عنصرًا مطابقًا
   *[other] { $count } عنصر مطابق
}

## الإعدادات ← المعرض

settings-section-gallery = المعرض
settings-gallery-layout = التخطيط
settings-gallery-layout-help = المربعات تقصّ الصور المصغّرة في شبكة؛ الصفوف المضبوطة تحافظ على شكل كل صورة؛ بلا قصّ يضع الصور كاملة في خلايا متساوية.
settings-gallery-layout-squares = مربعات
settings-gallery-layout-justified = صفوف مضبوطة
settings-gallery-layout-uncropped = بلا قصّ
settings-gallery-layout-details = التفاصيل
settings-gallery-thumbnail-size = حجم الصور المصغّرة
settings-gallery-thumbnail-size-help = يمكن أيضًا في المعرض استخدام Ctrl+عجلة الفأرة أو القرص أو Ctrl+زائد وناقص.
settings-gallery-captions = إظهار أسماء الملفات
settings-gallery-captions-help = تسمية تحت كل صورة مصغّرة.
settings-gallery-badges = إظهار الشارات
settings-gallery-badges-help = علامات صغيرة لـ RAW والحركة وHDR وتعدد الصفحات والتقييم والتسمية.
settings-gallery-sort-order = الترتيب حسب
settings-gallery-sort-order-help = ترتيب المعرض. تأتي المجلدات أولًا دائمًا.
settings-gallery-sort-order-name = الاسم
settings-gallery-sort-order-date-taken = تاريخ الالتقاط
settings-gallery-sort-order-modified = تاريخ التعديل
settings-gallery-sort-order-size = حجم الملف
settings-gallery-sort-order-type = نوع الملف
settings-gallery-sort-order-dimensions = الأبعاد
settings-gallery-sort-descending = عكس الترتيب
settings-gallery-sort-descending-help = الترتيب من الأخير إلى الأول.
settings-gallery-group-by = التجميع حسب
settings-gallery-group-by-help = عناوين أقسام تبقى في الأعلى أثناء التمرير.
settings-gallery-group-by-none = لا شيء
settings-gallery-group-by-day = اليوم
settings-gallery-group-by-month = الشهر
settings-gallery-group-by-year = السنة
settings-gallery-group-by-event = الحدث
settings-gallery-group-by-folder = المجلد
settings-gallery-group-by-camera = الكاميرا
settings-gallery-group-by-format = الصيغة
settings-gallery-group-by-rating = التقييم
settings-gallery-group-by-target = الهدف
settings-gallery-group-by-filter = المرشّح
settings-gallery-per-folder-view = تذكّر العرض لكل مجلد
settings-gallery-per-folder-view-help = يُحفظ التخطيط والحجم والترتيب والتجميع لكل مجلد؛ وإلا يُستخدم عرض واحد في كل مكان.
settings-gallery-open-folders-in-gallery = فتح المجلدات في المعرض
settings-gallery-open-folders-in-gallery-help = عند فتح مجلد، ابدأ بصوره المصغّرة بدلًا من الصورة الأولى.
settings-gallery-escape-opens-gallery = مفتاح Escape يفتح المعرض
settings-gallery-escape-opens-gallery-help = عندما لا يبقى ما يُغلق، ينتقل Escape في العارض إلى المعرض.
settings-gallery-wrap-around = مفاتيح الأسهم تلتف حول القائمة
settings-gallery-wrap-around-help = عند نهاية القائمة تنتقل مفاتيح الأسهم إلى بدايتها، ومن البداية إلى النهاية. يبقى الضغط المستمر على المفتاح متوقفًا عند النهاية.
settings-gallery-show-hidden = إظهار الملفات المخفية
settings-gallery-show-hidden-help = تضمين الملفات والمجلدات التي تبدأ أسماؤها بنقطة.
settings-gallery-show-other-files = إظهار الملفات الأخرى
settings-gallery-show-other-files-help = تضمين الملفات التي لا يستطيع { -app-name } فتحها، بأيقونة عامة.
settings-gallery-show-archives = إظهار الأرشيفات
settings-gallery-show-archives-help = تضمين القصص المصورة والأرشيفات الأخرى (zip وcbz وrar و7z…) كمربعات. فتح الأرشيف مباشرةً ما زال يعمل.
settings-gallery-subfolder-depth = مستويات المجلدات الفرعية
settings-gallery-subfolder-depth-help = إظهار صور المجلدات الفرعية أيضًا حتى هذا العدد من المستويات (0: هذا المجلد فقط).
settings-gallery-stacks = تكديس الملفات المرتبطة
settings-gallery-stacks-help = عرض أزواج RAW+JPEG واللقطات المتتابعة كمربع واحد. اضغط S لتوسيع الكومة.
settings-gallery-info-overlay = معلومات على الصور المصغّرة
settings-gallery-info-overlay-help = الأبعاد والتعريض فوق كل صورة مصغّرة.
settings-gallery-prefer-embedded = تفضيل المعاينات المضمّنة
settings-gallery-prefer-embedded-help = استخدام المعاينة المخزّنة في الملف إن كانت كبيرة بما يكفي. أسرع بكثير وأقل دقة بقليل.
settings-gallery-sharper-raw-thumbnails = صور مصغّرة أوضح لملفات RAW
settings-gallery-sharper-raw-thumbnails-help = تُعالَج صور RAW المعدّلة بدقة 1024 بكسل لصورها المصغّرة. أوضح وأبطأ قليلًا. عند الإيقاف: 512 بكسل.
settings-gallery-rating-sidecars = كتابة التقييمات في ملفات XMP مرافقة
settings-gallery-rating-sidecars-help = تُكتب التقييمات والتسميات والعلامات أيضًا في ملف ‎.xmp بجوار الصورة تقرؤه تطبيقات الصور الأخرى.
settings-gallery-cache-size = حجم ذاكرة الصور المصغّرة المؤقتة
settings-gallery-cache-size-help = تُحذف أقدم الصور المصغّرة عندما تمتلئ الذاكرة المؤقتة.
settings-gallery-cache-location = موقع ذاكرة الصور المصغّرة المؤقتة
settings-gallery-cache-location-help = يُفضَّل مجلد على قرص سريع.
settings-gallery-clear-cache = مسح ذاكرة الصور المصغّرة المؤقتة
settings-gallery-clear-cache-help = تُحذف الصور المصغّرة المحفوظة، وتُنشأ من جديد أثناء التصفح.

## Compare
# machine-drafted, needs native review

compare-side = جنبًا إلى جنب
compare-overlay = تراكب
compare-difference = الفرق
compare-flicker = تبديل سريع
compare-blend = مزج
compare-kind-absolute = الخطأ المطلق
compare-kind-squared = الخطأ التربيعي
compare-kind-relative = الخطأ النسبي
compare-scale-down = حساسية أقل
compare-scale-up = حساسية أكبر
compare-scale = المقياس الكامل { $value }
compare-space = ضوء خطي، كما يُعرض
compare-space-help = تُقاس الأخطاء والمقاييس بالضوء الخطي كما تُعرض الصورة، بعد تعيين الدرجات اللونية، لكل قناة بما فيها ألفا.
compare-metrics = ‏MAE { $mae }  ·  MSE { $mse }  ·  PSNR { $psnr } dB
compare-sampled = عُيّنت بدقة ‎{ $width } × { $height }
compare-aligned = محاذاة إلى أعلى اليسار، قورن ‎{ $width } × { $height }
compare-vs = مقارنة { $b } مع { $a }
compare-overlay-label = تراكب { $b } فوق { $a } بنسبة { PERCENT($percent) }
compare-pending = جارٍ القياس…
# machine-drafted, needs native review
compare-diff-failed = تعذّر عرض الفرق: { $reason }
compare-no-overlap = لا شيء للمقارنة في العرض
compare-zoom-fit = ملاءمة
compare-zoom = ‏{ PERCENT($percent) }
compare-showing = يُعرض { $name }
compare-a11y-group = مقارنة
compare-a11y-blend = المزج بين الصور
compare-a11y-zoom = التكبير
gallery-palette-compare-side = المقارنة جنبًا إلى جنب
gallery-palette-compare-overlay = المقارنة بالتراكب
gallery-palette-compare-difference = مقارنة الفرق: { $kind }
gallery-palette-compare-flicker = التبديل السريع بين الصور المقارنة
gallery-palette-compare-actual = المقارنة بنسبة { PERCENT(100) }

## Search value list (machine-drafted, needs native review)
gallery-picker-hint = المسافة أو Ctrl+Enter لتحديد عدة قيم

## Search value picker, part 2 (machine-drafted, needs native review)
gallery-facet-profile = ملف تعريف ألوان
gallery-facet-date = تاريخ الالتقاط
gallery-facet-picked = المختارة
gallery-facet-rejected = المرفوضة
gallery-facet-flagged = المختارة أو المرفوضة
gallery-facet-rated = المقيَّمة
gallery-facet-labelled = ذات تسمية

## Search value picker, part 3 (machine-drafted, needs native review)

gallery-facet-bits = عمق البت
gallery-facet-pages = الصفحات
gallery-facet-width = العرض
gallery-facet-height = الارتفاع
gallery-facet-megapixels = ميغابكسل
gallery-picker-bits = { $bits } بت
gallery-picker-pages = { $pages ->
    [zero] لا صفحات
    [one] صفحة واحدة
    [two] صفحتان
    [few] { $pages } صفحات
    [many] { $pages } صفحة
   *[other] { $pages } صفحة
}
gallery-picker-pixels = { $px } بكسل
gallery-picker-megapixels = { $mp } ميغابكسل
gallery-picker-group-any = أي ملف في مجموعة
gallery-picker-group-keep = مقترح للإبقاء
gallery-picker-group-discard = مقترح للاستبعاد
gallery-picker-ops = مقارنة
gallery-picker-op-eq = يساوي
gallery-picker-op-ge = على الأقل
gallery-picker-op-le = على الأكثر
gallery-picker-ge = { $value } على الأقل
gallery-picker-gt = أكثر من { $value }
gallery-picker-le = { $value } على الأكثر
gallery-picker-lt = أقل من { $value }

# Map view.
gallery-facet-place = المكان
gallery-palette-map = الخريطة
map-title = الخريطة
map-summary = صور ذات موقع: { $count }
map-photos = الصور: { $count }
map-zoom-in = تكبير
map-zoom-out = تصغير
map-show-all = عرض الكل
map-done = تم
map-credit = بيانات الخريطة: Natural Earth
# machine-drafted, needs native review
# With a base map's tiles on screen: $credit is the tile source's credit ("© OpenStreetMap
# contributors").
map-credit-tiles = الخريطة: { $credit } · Natural Earth
# The one-time explanation before Specular downloads map tiles from a server; one idea per
# line. $host is the server's name, $size the cache size ("256 MB").
map-basemap-consent-title = مربعات الخريطة عبر الإنترنت
map-basemap-consent-server = تأتي مربعات الخريطة من { $host }.
map-basemap-consent-sees = يرى ذلك الخادم عنوانك على الإنترنت والمناطق التي تعرضها.
map-basemap-consent-private = لا تُرسَل صورك ولا أسماؤها ولا مواقعها.
map-basemap-consent-cache = تُحفظ المربعات على هذا الحاسوب، حتى { $size }.
map-basemap-allow = استخدام الخريطة عبر الإنترنت
map-basemap-decline = الإبقاء على الخريطة دون اتصال
# Why the base map is not shown; $reason is one of the lines below or a file error.
map-basemap-error = الخريطة الأساسية غير معروضة: { $reason }
map-basemap-bad-address = يجب أن يحتوي عنوان خادم المربعات على {"{"}z{"}"} و{"{"}x{"}"} و{"{"}y{"}"}.
map-basemap-unknown-tiles = مربعات ملف الخريطة من نوع غير معروف.
map-loading = جارٍ قراءة مواقع الصور…
map-empty = لا توجد صورة من هذه الصور ذات موقع
map-hidden = مواقع الصور مخفية (الإعدادات › البيانات الوصفية والخصوصية)
map-a11y-group = خريطة مواقع الصور
map-a11y-status = الأماكن المعروضة: { $count }، مستوى التكبير { $zoom }
