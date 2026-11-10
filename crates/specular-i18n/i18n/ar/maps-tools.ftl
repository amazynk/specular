# machine-drafted, needs native review
# Specular — Maps tools: terrain (slope, aspect, contours, 3-D), time, wind and weather colours,
# online map services, photos and geotagging along GPS tracks, and map export.
# العربية.
# Unit and ° come before the number so they show on its right in right-to-left text.

## Cheat sheet groups of the Maps tab.
cheatsheet-map-terrain = التضاريس
cheatsheet-map-time = الوقت والطقس
cheatsheet-map-online = الخدمات عبر الإنترنت
cheatsheet-map-photos = الصور وGPS
cheatsheet-map-output = الإخراج

## Palette entries and their one-line descriptions.
palette-map-slope = ألوان الانحدار
palette-map-slope-help = تلوين نموذج الارتفاعات حسب شدة انحداره.
palette-map-aspect = ألوان اتجاه المنحدر
palette-map-aspect-help = تلوين نموذج الارتفاعات حسب الاتجاه الذي تواجهه منحدراته.
palette-map-contours = إظهار خطوط الكنتور أو إخفاؤها
palette-map-contours-help = خطوط كنتور فوق نموذج الارتفاعات، مع تسمية كل بضعة خطوط.
palette-map-terrain-3d = التضاريس ثلاثية الأبعاد
palette-map-terrain-3d-help = عرض نموذج الارتفاعات سطحًا ثلاثي الأبعاد تكسوه ألوانه.
palette-map-time = إظهار شريط الوقت أو إخفاؤه
palette-map-time-help = التنقل بين أوقات الشبكة أو على طول أوقات المسار.
palette-map-play = تشغيل الوقت أو إيقافه مؤقتًا
palette-map-play-help = تشغيل شريط الوقت تلقائيًا، خطوة بخطوة.
palette-map-wind = إظهار أسهم الرياح أو إخفاؤها
palette-map-wind-help = أسهم لسرعة الرياح واتجاهها من مركّبتي u وv في الشبكة.
palette-map-weather-colours = خريطة ألوان الطقس التالية
palette-map-weather-colours-help = خرائط ألوان معدّة لدرجة الحرارة والمطر والرياح والضغط.
palette-map-services = خدمات الخرائط عبر الإنترنت…
palette-map-services-help = إضافة طبقات من خوادم WMS أو WMTS أو WFS أو OGC API أو STAC.
palette-map-photos-on-track = إظهار الصور على المسار أو إخفاؤها
palette-map-photos-on-track-help = وضع صور المجلد على طول مسار GPS حسب وقت التقاطها.
palette-map-geotag = وضع العلامات الجغرافية للصور من المسار…
palette-map-geotag-help = كتابة مواضع المسار في ملفات XMP مرافقة لصور المجلد.
palette-map-export = تصدير الخريطة…
palette-map-export-help = حفظ الخريطة بصيغة PNG أو PDF بمقياس محدد، مع مفتاح الخريطة وشريط المقياس وسهم الشمال.
palette-map-print = طباعة الخريطة بمقياس…
palette-map-print-help = طباعة الخريطة بمقياس مختار، مثل 1:{ NUMBER(25000) }.
palette-map-copy-extent = نسخ النطاق
palette-map-copy-extent-help = نسخ المنطقة المعروضة على الشاشة كمربع محيط، أو بصيغة WKT.

## Why a map command is unavailable.
disabled-no-time = لا توجد أوقات في الخريطة
disabled-no-wind = لا توجد مركّبات رياح في الخريطة (u وv)
disabled-no-track = لا يوجد مسار GPS بأوقات
disabled-not-grid = الخريطة ليست شبكة قيم

## Settings › Maps and Metadata.
settings-maps-services = خدمات الخرائط عبر الإنترنت
settings-maps-services-help = إضافة طبقات من خوادم WMS وWMTS وWFS وOGC API وSTAC. عند الإيقاف: لا يُجلب شيء.
settings-maps-services-allowed = خوادم الخرائط المسموح بها
settings-maps-services-allowed-help = خوادم وافقت عليها بعد شرح ما تراه.
settings-metadata-basemap-service = طبقة من خدمة خرائط
settings-metadata-basemap-service-help = الطبقة المختارة في لوحة الخدمات عبر الإنترنت في الخريطة.

## Terrain.
# A compass point, short: n, ne, e, se, s, sw, w, nw.
map-compass = { $point ->
    [ne] الشمال الشرقي
    [e] الشرق
    [se] الجنوب الشرقي
    [s] الجنوب
    [sw] الجنوب الغربي
    [w] الغرب
    [nw] الشمال الغربي
   *[n] الشمال
}
map-slope-aspect = الانحدار °{ $slope } · يواجه { $direction } (°{ $aspect })
map-slope-flat = الانحدار °{ $slope } · مستوٍ
map-slope-on = ألوان الانحدار
map-aspect-on = ألوان اتجاه المنحدر
map-relief-off = ألوان الارتفاع
map-contours-on = خطوط الكنتور ظاهرة
map-contours-off = خطوط الكنتور مخفية
map-legend-slope = الانحدار
map-legend-aspect = اتجاه المنحدر: الجهة التي تواجهها المنحدرات
map-legend-degrees = °{ $degrees }
map-legend-degrees-more = °{ $degrees } فأكثر
map-terrain-3d-on = التضاريس ثلاثية الأبعاد. Esc يعود إلى الخريطة.
map-terrain-3d-on-exaggerated = التضاريس ثلاثية الأبعاد، الارتفاعات × { $times }. Esc يعود إلى الخريطة.
map-terrain-exaggeration = الارتفاعات مضخّمة { $times } مرة.
map-terrain-drape = ألوان الخريطة

## Time, weather and wind.
map-time-grid = { $when } · { $step } من { $steps }
map-time-track = { $when } · بعد { $elapsed }
map-time-speed = × { $times }
map-time-play = تشغيل
map-time-pause = إيقاف مؤقت
map-weather-map = الألوان: { $name }
map-weather-file = الألوان من الملف
map-wind-on = أسهم الرياح ظاهرة
map-wind-off = أسهم الرياح مخفية
map-wind-at = الرياح { $unit } { $speed } من °{ $degrees } ({ $direction })

## Copy extent.
map-extent-copied = نُسخ المربع المحيط. انسخ مرة أخرى للحصول على WKT.
map-extent-copied-wkt = نُسخ النطاق بصيغة WKT

## Online map services.
map-services-title = خدمات الخرائط عبر الإنترنت
map-service-off = خدمات الخرائط عبر الإنترنت متوقفة.
map-service-off-why = فعّلها لإضافة طبقات من خوادم على الإنترنت.
map-service-turn-on = تفعيل
map-service-close = إغلاق
map-service-back = رجوع
map-service-cancel = إلغاء
map-service-connect = اتصال
map-service-allow = سماح
map-service-remove = إزالة
map-service-download = تنزيل
map-service-list-hint = Enter يفتح خادمًا. Delete يزيله.
map-service-add = إضافة خادم…
map-service-add-sub = WMS أو WMTS أو WFS أو OGC API أو STAC
map-service-add-hint = عنوان الخادم. Enter يتصل.
map-service-bad-address = ليس عنوان ويب (http أو https، دون اسم مستخدم).
map-service-server-sub = { $kind } · { $host }
map-service-consent-title = خدمة خرائط عبر الإنترنت
map-service-consent-server = سيتصل Specular بـ { $host }.
map-service-consent-sees = يرى ذلك الخادم عنوانك على الإنترنت ومنطقة الخريطة التي تطلبها.
map-service-consent-private = لا تُرسل ملفاتك ولا أسماؤها.
map-service-asking = جارٍ سؤال { $host }…
map-service-downloading = جارٍ التنزيل من { $host }…
map-service-layers-hint = طبقات { $kind }. Enter يستخدم إحداها.
map-service-no-layers = لا يعرض الخادم أي طبقات.
map-service-offer-tiles = مربعات: تصبح الخريطة الأساسية لخريطة الصور
map-service-offer-features = معالم: تُضاف إلى هذه الخريطة للمنطقة المعروضة على الشاشة
map-service-offer-search = صور: البحث في المنطقة المعروضة على الشاشة
map-service-refused-mercator = غير متاح بإسقاط Web Mercator (EPSG:3857) الذي تحتاجه الخريطة الأساسية.
map-service-refused-format = غير متاح كصور PNG أو JPEG.
map-service-refused-geojson = غير متاح بصيغة GeoJSON، وهي الصيغة المقروءة هنا.
map-service-items-hint = { $count ->
        [zero] لم يُعثر على صور.
        [one] عُثر على صورة واحدة. Enter ينزّلها.
        [two] عُثر على صورتين. Enter ينزّل إحداهما.
        [few] عُثر على { $count } صور. Enter ينزّل إحداها.
        [many] عُثر على { $count } صورة. Enter ينزّل إحداها.
       *[other] عُثر على { $count } من الصور. Enter ينزّل إحداها.
    }
map-service-no-items = لم يُعثر على صور هنا.
map-service-item-cloud = { $when } · غيوم { PERCENT($cloud) }
map-service-item-no-cog = لا يوجد Cloud-Optimized GeoTIFF
map-service-download-ask = تنزيل { $size } من { $host }؟
map-service-download-ask-unknown = تنزيل هذه الصورة من { $host }؟ حجمها غير معروف.
map-service-download-cache = تُحفظ في ذاكرة التخزين المؤقت للخرائط وتُفتح.
map-service-size-mb = MB { $size }
map-service-size-gb = GB { $size }
map-service-too-big = حجم الصورة { $size }، وهي أكبر من أن تُنزَّل.
map-service-opened = جارٍ فتح الصورة المنزّلة
map-service-basemap = أصبحت { $layer } الخريطة الأساسية لخريطة الصور. تسأل خريطة الصور قبل أول تنزيل لها.
map-service-status = أجاب الخادم بـ { $status }.
map-service-unknown = ليست خدمة خرائط يقرؤها هذا الإصدار.
map-service-exception = قال الخادم: { $message }
map-service-malformed = إجابة الخادم معطوبة: { $reason }
map-service-remove-ask = إزالة «{ $server }» من الخوادم المحفوظة؟
map-service-remove-detail = لا يُفقد شيء على القرص. تبقى الطبقات الموجودة على الخريطة حتى إغلاقها.
map-service-removed = أُزيل { $server }
map-service-saved = خوادم الخرائط المحفوظة
map-service-saved-none = لا شيء بعد. أضف خادمًا من لوحة الخدمات عبر الإنترنت في الخريطة.
map-service-keywords = WMS WMTS WFS OGC API STAC خوادم

## Photos and geotagging.
map-geotag-title = وضع العلامات الجغرافية للصور من المسار
map-photos-on = الصور على المسار
map-photos-off = الصور على المسار مخفية
map-geotag-offset = ساعة الكاميرا: { $offset } عن توقيت GPS. ← → يغيّرانها دقيقة، ومع Shift ساعة، ومع Alt ثانية.
map-geotag-reading = جارٍ قراءة الصور…
map-geotag-summary = تحصل { $count } من { $total } صورة على موضع.
map-geotag-sidecars = تُكتب المواضع في ملفات XMP مرافقة بجوار الصور. لا تُغيَّر ملفات الصور.
map-geotag-writing = جارٍ الكتابة…
map-geotag-write = كتابة الملفات المرافقة…
map-geotag-guess = تخمين الفارق
map-geotag-will-write = تحصل على { $position }
map-geotag-has-position = لها موضع مسبقًا: تُترك كما هي
map-geotag-no-time = لا وقت التقاط: تُترك كما هي
map-geotag-outside = التُقطت على بُعد { $offset } خارج المسار: تُترك كما هي
map-geotag-gap = في فجوة من المسار ({ $seconds } ث من نقطة تحديد موقع): تُترك كما هي
map-geotag-nothing = لا تحصل أي صورة على موضع بهذا الفارق.
map-geotag-confirm = كتابة المواضع في { $count ->
    [zero] { $count } ملف XMP مرافق
    [one] ملف XMP مرافق واحد
    [two] ملفَّي XMP مرافقين
    [few] { $count } ملفات XMP مرافقة
    [many] { $count } ملف XMP مرافقًا
   *[other] { $count } ملف XMP مرافق
}؟ لا تُغيَّر الصور نفسها.
map-geotag-confirm-ok = كتابة الملفات المرافقة
map-geotag-done = { $count ->
    [zero] كُتب { $count } ملف مرافق
    [one] كُتب ملف مرافق واحد
    [two] كُتب ملفان مرافقان
    [few] كُتبت { $count } ملفات مرافقة
    [many] كُتب { $count } ملفًا مرافقًا
   *[other] كُتب { $count } ملف مرافق
}
map-geotag-done-failed = كُتبت الملفات المرافقة: { $count }؛ فشل { $failed }: { $reason }
map-geotag-not-sidecar = ليس اسم ملف مرافق.

## Export and printing.
map-export-title = تصدير الخريطة
map-print-title = طباعة الخريطة بمقياس
map-export-keys = ↑ ↓ للاختيار، ← → للتغيير، Enter للتنفيذ.
map-export-format = الصيغة
map-export-scale = المقياس
map-export-scale-value = 1:{ $n }
map-export-resolution = الدقة
map-export-dpi = { $dpi } نقطة/بوصة
map-export-paper = الورق
map-export-size = { $width } × { $height } بكسل · { $across } × { $down } كم على الأرض
map-export-save = حفظ…
map-export-print = طباعة…
map-export-too-large = كبيرة جدًا بهذا المقياس: { $width } × { $height } بكسل. اختر مقياسًا أو دقة أصغر.
map-export-no-scale = ليس لهذه الخريطة مقياس على الأرض.
map-export-no-gpu = يتطلب التصدير وحدة معالجة الرسومات.
map-export-subtitle = المقياس { $scale } · { $crs }
map-export-data = البيانات: { $sources }
map-export-saved = حُفظ { $name }
map-export-printing = جارٍ الطباعة…
map-export-printed = أُرسلت إلى الطابعة
map-export-print-failed = لم تُطبع: { $reason }
