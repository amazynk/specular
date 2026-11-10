# machine-drafted, needs native review
# Specular — واجهة العارض ولوحة الأوامر والأخطاء ونافذة «حول». العربية الفصحى.

-app-name = Specular

## تلميحات شريط الأدوات وتسميات إمكانية الوصول

toolbar-open = فتح صورة…
toolbar-prev = الصورة السابقة
toolbar-next = الصورة التالية
toolbar-back-gallery = العودة إلى المعرض
toolbar-zoom-out = تصغير
toolbar-zoom-in = تكبير
toolbar-zoom-level = مستوى التكبير والإعدادات المسبقة
toolbar-zoom-fit = ملاءمة للنافذة
toolbar-zoom-100 = الحجم الفعلي ({ PERCENT(100) })
toolbar-rotate-left = تدوير عكس اتجاه عقارب الساعة
toolbar-rotate-right = تدوير مع اتجاه عقارب الساعة
toolbar-fullscreen-enter = الدخول إلى وضع ملء الشاشة
toolbar-fullscreen-exit = الخروج من وضع ملء الشاشة
toolbar-info = معلومات الصورة
toolbar-settings = الإعدادات
toolbar-delete = نقل إلى سلة المهملات
toolbar-copy = نسخ الصورة
toolbar-palette = لوحة الأوامر
toolbar-go-to = الانتقال إلى صورة…
toolbar-go-to-range = 1–{ $count }
# machine-drafted, needs native review
toolbar-customise = تخصيص شريط الأدوات…
toolbar-more = المزيد
toolbar-announce-moved = { $item }: الموضع { $index } من { $count }
toolbar-announce-added = تمت إضافة { $item }: الموضع { $index } من { $count }
toolbar-announce-removed = تمت إزالة { $item } من شريط الأدوات
toolbar-announce-reset = أُعيد شريط الأدوات إلى الوضع الافتراضي
tooltip-with-shortcut = { $label } ({ $shortcut })

## المؤشرات

readout-position = { $index } / { $count }
readout-position-a11y = الصورة { $index } من { $count }
readout-zoom = { $zoom }
readout-zoom-a11y = التكبير { $zoom }
readout-zoom-fit = ملاءمة ({ $zoom })
zoom-preset-fit = ملاءمة للنافذة
zoom-preset-fill = ملء النافذة
zoom-preset-actual = الحجم الفعلي
# machine-drafted, needs native review
# The last entry of the zoom presets: the window fills the screen (ticked while it does).
zoom-preset-fullscreen = ملء الشاشة
zoom-preset-percent = { $zoom }
readout-dimensions = { $width } × { $height }
status-image-count =
    { $count ->
        [zero] لا توجد صور
        [one] صورة واحدة
        [two] صورتان
        [few] { $count } صور
        [many] { $count } صورة
       *[other] { $count } من الصور
    }

## لوحة الأوامر

palette-placeholder = اكتب أمرًا أو إعدادًا…
palette-no-results = لا توجد أوامر مطابقة
palette-set-setting = تعيين: { $setting } ← { $value }
palette-next = الصورة التالية
palette-prev = الصورة السابقة
palette-first = الصورة الأولى
palette-last = الصورة الأخيرة
palette-go-to = الانتقال إلى صورة…
palette-zoom-in = تكبير
palette-zoom-out = تصغير
palette-zoom-fit = تكبير للملاءمة
palette-zoom-fill = تكبير للملء
# machine-drafted, needs native review
palette-zoom-fit-width = ملاءمة العرض
palette-zoom-fit-height = ملاءمة الارتفاع
palette-zoom-100 = تكبير إلى { PERCENT(100) }
palette-zoom-200 = تكبير إلى { PERCENT(200) }
palette-zoom-300 = تكبير إلى { PERCENT(300) }
palette-zoom-400 = تكبير إلى { PERCENT(400) }
palette-zoom-rectangle = تكبير إلى التحديد
palette-rotate-left = تدوير عكس اتجاه عقارب الساعة
palette-rotate-right = تدوير مع اتجاه عقارب الساعة
palette-flip-horizontal = قلب أفقيًا
palette-flip-vertical = قلب عموديًا
palette-toggle-fullscreen = تبديل وضع ملء الشاشة
palette-navigator = إظهار المستكشف أو إخفاؤه
palette-loupe = العدسة المكبرة
palette-pixel-grid = إظهار شبكة البكسلات أو إخفاؤها
palette-info-panel = إظهار لوحة المعلومات أو إخفاؤها
palette-copy-image = نسخ الصورة
# machine-drafted, needs native review
palette-read-codes = قراءة الرموز
# machine-drafted, needs native review
palette-find-text = البحث عن النص
# machine-drafted, needs native review
palette-face-boxes = إظهار الوجوه
palette-copy-path = نسخ مسار الملف
palette-move-to-trash = نقل إلى سلة المهملات
palette-undo-trash = تراجع عن النقل إلى سلة المهملات
palette-open = فتح صورة…
# machine-drafted, needs native review
palette-open-address = فتح عنوان…
palette-settings = الإعدادات
# machine-drafted, needs native review
palette-customise-toolbar = تخصيص شريط الأدوات…
palette-cheat-sheet = اختصارات لوحة المفاتيح
palette-hud = إظهار HUD زمن الإطار أو إخفاؤه
palette-language = اللغة
palette-theme = المظهر
# machine-drafted, needs native review
palette-interface-larger = واجهة أكبر
palette-interface-smaller = واجهة أصغر
palette-interface-reset = الواجهة بنسبة { PERCENT(100) }
palette-quit = إنهاء { -app-name }

## ورقة الاختصارات

cheatsheet-title = اختصارات لوحة المفاتيح
cheatsheet-navigation = التنقل
cheatsheet-zoom-pan = التكبير والتحريك
cheatsheet-view = العرض
cheatsheet-file = ملف
cheatsheet-window = نافذة
cheatsheet-help = مساعدة
cheatsheet-hold = اضغط مطولًا على { $key }
cheatsheet-close-hint = اضغط { $key } أو Esc للإغلاق

## الأخطاء

# machine-drafted, needs native review
viewer-no-preview = لا توجد معاينة داخل هذا الملف.
error-cannot-open = تعذّر فتح { $file }
error-cannot-open-reason = تعذّر فتح { $file }: { $reason }
error-unsupported-format = الملف { $file } بتنسيق لا يستطيع { -app-name } قراءته بعد.
error-file-truncated = الملف { $file } غير مكتمل. يُعرض الجزء الذي أمكنت قراءته.
error-file-too-large = الملف { $file } أكبر من أن يُفتح ({ $megapixels } ميغابكسل؛ والحد الأقصى { $limit } ميغابكسل).
error-not-found = الملف { $file } لم يعد موجودًا.
error-permission-denied = ليس لديك إذن بقراءة { $file }.
error-details = التفاصيل
crash-last-time = أُغلق { -app-name } بشكل غير متوقع في المرة السابقة؛ حُفظ تقرير في { $path }
error-trash-failed = تعذّر نقل { $file } إلى سلة المهملات: { $reason }
# machine-drafted, needs native review
# الأسباب، تظهر بعد "تعذّر فتح FILE:" ورسائل "تعذّر …:" الأخرى. تذكر ما حدث، وعند
# الحاجة ما ينبغي فعله.
error-reason-not-found = لم يعد موجودًا. ربما نُقل أو أُعيدت تسميته أو حُذف.
error-reason-permission = ليس لديك إذن بقراءته. تحقق من مالكه وأذوناته.
error-reason-disk-full = القرص ممتلئ. حرّر بعض المساحة وحاول مجددًا.
error-reason-read-only = القرص للقراءة فقط.
error-reason-truncated = الملف غير مكتمل. ربما لا يزال قيد النسخ أو التنزيل.
error-reason-is-folder = إنه مجلد وليس ملفًا.
# A remote store (a Zarr image on a server or in an S3 bucket).
error-reason-remote-denied = رفض الخادم الوصول. قد تكون البيانات خاصة؛ لحاوية S3، اضبط بيانات اعتماد AWS أولًا.
error-reason-remote-missing = لا توجد بيانات Zarr في هذا العنوان.
# $status is an HTTP status code ("500").
error-reason-remote-status = أجاب الخادم بالخطأ { $status }.
error-reason-offline = تعذّر الوصول إلى الخادم. تحقق من العنوان والاتصال.
# machine-drafted, needs native review
error-reason-network-off = الشبكة مُطفأة في الإعدادات › الخصوصية.
error-reason-timed-out = لم يستجب محرك الأقراص أو الخادم في الوقت المحدد. تحقق من الاتصال وحاول مجددًا.
error-reason-memory = لا تتوفر ذاكرة كافية. أغلق صورًا أو تطبيقات أخرى وحاول مجددًا.
error-reason-unsupported = إنه بتنسيق لا يستطيع { -app-name } قراءته بعد.
# $detail هو وصف أداة فك الترميز نفسها، تقني وبالإنجليزية ("12-bit lossless JPEG").
error-reason-unsupported-detail = يستخدم ميزة لا يستطيع { -app-name } قراءتها بعد ({ $detail }).
# machine-drafted, needs native review
# A file whose extension several formats share (".img") and whose content is none of them.
# $formats lists the format names, e.g. "ERDAS IMAGINE, ADSC SMV, Analyze 7.5".
error-reason-not-one-of = إنه ليس أيًّا من ملفات .{ $ext } التي يقرؤها { -app-name } ({ $formats }).
error-reason-damaged = الملف تالف أو ليس ما يدل عليه اسمه ({ $detail }).
error-reason-too-large = إنه أكبر من أن يُفتح.
# machine-drafted, needs native review
error-reason-needs-memory = إنه يحتاج إلى { $needed } من ذاكرة RAM الحرة، لكن الحر منها الآن { $free } فقط. أغلق بعض التطبيقات الأخرى وحاول مجددًا، أو افتحه على حاسوب بذاكرة أكبر.
error-needs-memory = تحتاج هذه الصورة إلى { $needed } من ذاكرة RAM الحرة، لكن الحر منها الآن { $free } فقط. أغلق بعض التطبيقات الأخرى وحاول مجددًا، أو افتحها على حاسوب بذاكرة أكبر.
error-needs-memory-short = تحتاج إلى { $needed } من RAM الحرة
error-reason-cancelled = أُوقف فتحه.
error-reason-trash-access = لم يعد موجودًا، أو ليس لديك إذن بتغييره.
error-reason-restore-collision = يوجد بالفعل ملف بالاسم نفسه في مجلده.
error-reason-trash-root = لا يمكن نقل محرك أقراص كامل إلى سلة المهملات.
error-reason-not-in-trash = لم يعد في سلة المهملات.
error-reason-program-missing = البرنامج غير مثبّت، أو نُقل. اختره مجددًا أو أعد تثبيته.
error-reason-program-not-allowed = لا يُسمح للبرنامج بالتشغيل. تحقق من أنه قابل للتنفيذ.
# machine-drafted, needs native review
error-reason-permission-write = ليس لديك إذن بتغييره. تحقق من مالكه وأذوناته.
error-reason-file-has-errors = يحتوي على أخطاء. أصلحها أولًا حتى لا يُفقد شيء منه.
error-reason-not-dicom = ليس ملف DICOM.
error-reason-no-image = لا يحتوي على صورة.
error-reason-needs-layout = إنه حجم بلا ترويسة. يلزم معرفة حجمه ونوع عيّناته وترتيب بايتاته لفتحه.
error-reason-not-archive = ليس أرشيفًا يستطيع { -app-name } قراءته.
error-reason-needs-sandbox = لا يفتح هذا النوع من الأرشيفات إلا القارئ المحمي في { -app-name }، وهو غير متاح.
error-reason-encrypted = إنه مشفّر ويحتاج إلى كلمة مروره.
error-reason-wrong-password = كلمة المرور غير صحيحة.
error-reason-checksum = الملف تالف: محتوياته لا تطابق مجموعها الاختباري.
error-reason-missing-entry = ملف بداخله مفقود.
error-reason-busy = يستخدمه برنامج آخر. أغلقه هناك وحاول مجددًا.
error-reason-catalogue-damaged = ملف الفهرس تالف.
error-reason-cannot-open = تعذّر فتح ملفه. تحقق من وجود مجلده ومن أنه يمكنك الكتابة فيه.
error-reason-newer-version = كُتب بإصدار أحدث من { -app-name }. حدّث { -app-name } لتغييره.
error-reason-internal = حدث خطأ داخل { -app-name }. التفاصيل في السجل.
error-reason-volume-too-few = { $count ->
    [zero] لا توجد شرائح لبناء حجم.
    [one] شريحة واحدة لا تكفي لبناء حجم.
    [two] شريحتان لا تكفيان لبناء حجم.
    [few] { $count } شرائح لا تكفي لبناء حجم.
    [many] { $count } شريحة لا تكفي لبناء حجم.
   *[other] { $count } شريحة لا تكفي لبناء حجم.
}
error-reason-volume-no-geometry = إحدى الشرائح لا تحدد موضعها أو اتجاهها.
error-reason-volume-not-parallel = الشرائح غير متوازية.
error-reason-volume-shared-position = عدة صور تشترك في كل { $count ->
        [zero] موضع (أطوار أو أصداء أو سلسلة زمنية).
        [one] موضع من موضع واحد (أطوار أو أصداء أو سلسلة زمنية).
        [two] موضع من موضعين (أطوار أو أصداء أو سلسلة زمنية).
        [few] موضع من { $count } مواضع (أطوار أو أصداء أو سلسلة زمنية).
        [many] موضع من { $count } موضعًا (أطوار أو أصداء أو سلسلة زمنية).
       *[other] موضع من { $count } من المواضع (أطوار أو أصداء أو سلسلة زمنية).
    }
error-reason-volume-missing = { $count ->
        [zero] لا شرائح مفقودة.
        [one] نحو شريحة واحدة مفقودة.
        [two] نحو شريحتين مفقودتين.
        [few] نحو { $count } شرائح مفقودة.
        [many] نحو { $count } شريحة مفقودة.
       *[other] نحو { $count } من الشرائح مفقودة.
    }
error-reason-volume-mismatch = تختلف الشرائح في الحجم أو في تباعد البكسلات.
error-reason-volume-not-grey = الشرائح ملوّنة وليست بتدرجات الرمادي.
error-reason-volume-skewed = محاور الحجم ليست متعامدة.
error-reason-volume-too-large = يحتاج إلى { $need } MiB والمسموح به { $budget } MiB فقط.
error-reason-url-not-specular = ليس رابط { -app-name }.
error-reason-url-too-long = إنه طويل جدًا.
error-reason-url-damaged = إنه تالف. ربما اقتُطع أو تغيّر عند نسخه.
error-reason-url-unknown = يطلب شيئًا لا يعرفه هذا الإصدار من { -app-name }. قد يفيد تحديث { -app-name }.
error-reason-url-not-absolute = يجب أن تكون مسارات ملفاته مسارات كاملة.
error-reason-url-no-paths = لا يسمّي أي ملف.
error-reason-url-too-many = يسمّي ملفات كثيرة جدًا.

## الإشعارات

# machine-drafted, needs native review
# A background task (a listing, an export, a file operation) stopped part way; $reason is
# error-reason-internal.
background-failed = تعذّر الإكمال: { $reason }
toast-moved-to-trash = نُقلت إلى سلة المهملات
toast-moved-to-trash-named = نُقل { $file } إلى سلة المهملات
toast-moved-to-trash-count =
    { $count ->
        [zero] لم تُنقل أي صورة إلى سلة المهملات
        [one] نُقلت صورة واحدة إلى سلة المهملات
        [two] نُقلت صورتان إلى سلة المهملات
        [few] نُقلت { $count } صور إلى سلة المهملات
        [many] نُقلت { $count } صورة إلى سلة المهملات
       *[other] نُقلت { $count } من الصور إلى سلة المهملات
    }
toast-undo = تراجع
toast-restored = تمت استعادة { $file }
toast-copied-image = تم نسخ الصورة
toast-copied-path = تم نسخ المسار
toast-copied-report = تم نسخ التقرير
toast-copied-value = تم نسخ القيمة
toast-loop-start = العودة إلى الصورة الأولى
toast-loop-end = الانتقال إلى الصورة الأخيرة
# machine-drafted, needs native review
toast-interface-size = حجم الواجهة { PERCENT($percent) }
toast-language-changed = تم تغيير اللغة إلى { $language }

## الحالات الفارغة

empty-drop = أفلت صورة هنا أو اضغط { $shortcut }
empty-open-button = فتح صورة…
empty-drop-release = أفلِت للفتح
empty-folder = لا يحتوي هذا المجلد على صور يستطيع { -app-name } فتحها.
# machine-drafted, needs native review
# تحت "فتح صورة…" في النافذة الفارغة: أين تجد كل شيء آخر.
empty-hint-keys = اضغط { $cheatsheet } لاختصارات لوحة المفاتيح، أو { $palette } للعثور على أي أمر.
empty-hint-cheatsheet = اضغط { $cheatsheet } لاختصارات لوحة المفاتيح.
empty-hint-palette = اضغط { $palette } للعثور على أي أمر.
# تحت مؤشر التحميل عندما يستغرق فتح ملف أكثر من لحظة.
viewer-opening = جارٍ فتح { $file }…
fullscreen-exit-hint = اضغط Esc للخروج من وضع ملء الشاشة

## منتقي اللغة

language-picker-title = اللغة
# machine-drafted, needs native review
language-picker-search = البحث في اللغات
language-completeness = مترجمة بنسبة { $percent }
language-system-default = لغة النظام الافتراضية ({ $language })
language-pseudo-accented = لغة تجريبية: أحرف مشكولة ونص أطول
language-pseudo-rtl = لغة تجريبية: معكوسة من اليمين إلى اليسار

## نافذة «حول»

about-title = حول { -app-name }
about-version = الإصدار { $version }
about-tagline = عارض صور سريع ودقيق.
about-copyright = © { $year } مؤلفو { -app-name }
about-license = مرخّص بموجب MIT License / Apache License 2.0.
about-geonames = أسماء الأماكن من GeoNames (geonames.org)، مرخّصة بموجب Creative Commons Attribution 4.0.
about-unicode = بيانات اللغات والمناطق من Unicode CLDR عبر ICU4X. © Unicode, Inc.
# machine-drafted, needs native review
about-lensfun = ملفات تعريف العدسات من قاعدة بيانات Lensfun (lensfun.github.io)، مرخّصة بموجب Creative Commons Attribution-ShareAlike 3.0.
about-third-party = تراخيص الجهات الخارجية
about-copy-info = نسخ معلومات النظام
about-close = إغلاق

## واجهة العارض (المرحلة 1)
# machine-drafted, needs native review
app-close = إغلاق
app-truncated-badge = ملف غير مكتمل
app-info-summary = ملخص
app-info-all-fields = جميع الحقول
app-info-copy-value = نسخ القيمة
app-info-copy-field = نسخ قيمة { $field }
app-info-loading = جارٍ قراءة البيانات الوصفية…
# machine-drafted, needs native review
# كل الحقول، لملف لا يخزّن بيانات وصفية غير بكسلاته.
app-info-no-fields = لا يحتوي هذا الملف على حقول بيانات وصفية.
# Gain-map row of the info summary (machine-drafted, needs native review)
app-info-gain-map = خريطة كسب HDR
app-info-gain-map-headroom = حتى { $headroom }× من سطوع SDR
app-copy-failed = تعذّر النسخ: { $reason }
app-restore-failed = تعذّرت استعادة { $file }: { $reason }
app-open-dialog-title = فتح صورة
app-open-dialog-filter = الملفات المدعومة
app-open-dialog-all = كل الملفات
app-open-group-images = الصور
app-open-group-raw = ملفات RAW للكاميرا
app-open-group-video = الفيديو
app-open-group-sound = الصوت
app-open-group-documents = المستندات
app-open-group-drawings = الرسوم وCAD
app-open-group-models = النماذج ثلاثية الأبعاد
app-open-group-maps = الخرائط والمسارات
app-open-group-science = العلوم والطب
app-open-group-fonts = الخطوط
app-open-group-other = أخرى

## معلومات العارض (حول، الإعدادات › متقدم)

# machine-drafted, needs native review
renderer-title = معلومات العارض
renderer-section-adapter = محوّل الرسومات
renderer-section-limits = الحدود والميزات
renderer-section-display = الشاشة
renderer-section-memory = الذاكرة
renderer-section-decoders = مفكّكات ترميز اختيارية
renderer-unknown = غير معروف
# The PCI vendor and device IDs ("1002:747e").
renderer-ids = معرّف المورّد والجهاز
# Wayland, X11, Windows or macOS.
renderer-display-server = خادم العرض
renderer-features = الميزات المستخدمة
renderer-surface-format = تنسيق النافذة
renderer-surface-formats = التنسيقات المتاحة
renderer-present-mode = وضع العرض
renderer-present-modes = الأوضاع المتاحة
renderer-hdr = HDR
# $headroom is how much brighter than white the display goes ("4.0").
renderer-hdr-on = مُفعّل (هامش { $headroom }×)
renderer-hdr-off = مُعطّل (يمكن للشاشة عرض HDR)
renderer-hdr-unavailable = غير متاح على هذه الشاشة
renderer-decode-budget = يمكن أن يستخدم فك الترميز
renderer-decode-budget-detail = { PERCENT(80) } من الذاكرة الحرة، مشتركة بين كل الصور قيد الفتح.
renderer-display-cache = ذاكرة التخزين المؤقت للعرض

## HUD الأداء (F12)
# machine-drafted, needs native review
app-hud-scale = المقياس
app-hud-level = المستوى
app-hud-level-thumbnail = الصورة المصغّرة المضمّنة
app-hud-level-full = الدقة الكاملة
app-hud-level-scaled = 1/{ $denominator }
app-hud-open = الفتح
app-hud-decode = فك الترميز
app-hud-upload = الرفع
app-hud-frame = الإطار
app-hud-threads = خيوط المعالجة
app-hud-threads-value = { $threads } ({ $queued } في الانتظار)
app-hud-tiles = البلاطات
app-hud-tiles-value = { $tiles } من { $capacity }
app-hud-ram = الذاكرة
app-hud-vram = ذاكرة GPU
app-hud-gpu = GPU

# Trash confirmation (machine-drafted, needs native review)
app-confirm-trash-title = هل تريد نقل { $file } إلى سلة المهملات؟
# machine-drafted, needs native review
app-confirm-trash-sidecars = تذهب ملفاته المرافقة إلى سلة المهملات أيضًا: { $files }
app-cancel = إلغاء

# Stage 1 merge (machine-drafted, needs native review)
disabled-no-image = لا توجد صورة مفتوحة
disabled-still-loading = لا تزال الصورة قيد التحميل
disabled-decode-failed = تعذّر فك ترميز هذه الصورة
disabled-no-other-images = لا توجد صور أخرى في هذا المجلد
tooltip-unavailable = { $label } (غير متاح: { $reason })
error-decoder-stopped = توقف مفكك الترميز دون إنتاج صورة
# machine-drafted, needs native review
# Shown after "Can’t open FILE:" for brain data (GIFTI .func/.shape/.label, CIFTI) that is
# drawn on a surface: no .surf.gii with that many vertices was found in the same folder.
error-needs-surface = يلزمه سطح دماغ (ملف ‎.surf.gii بعدد { $vertices } رأسًا) في المجلد نفسه
fullscreen-back-hint = اضغط Esc لإعادة ضبط التكبير أو الخروج من ملء الشاشة
action-back = إغلاق اللوحة (في مربع حوار، الرجوع خطوة أولًا)، إعادة ضبط التكبير، الخروج من ملء الشاشة؛ وفي المعرض، الصعود مجلدًا واحدًا
about-third-party-file = يتضمن { -app-name } برامج وبيانات من جهات خارجية تخضع لتراخيصها الخاصة؛ راجع الملف THIRD-PARTY.md المثبت مع { -app-name }.
toast-low-memory = قد تحتاج هذه الصورة إلى نحو { $need } من الذاكرة، لكن المتاح { $available } فقط. قد يكون تحميلها بطيئًا أو قد يفشل.

hw-report-title = تقرير العتاد
hw-report-copy = نسخ التقرير
hw-report-version = الإصدار
hw-report-cpu = CPU
hw-report-cores = الأنوية
hw-report-cores-value = { $logical } منطقية، { $physical } فعلية
hw-report-performance-cores = أنوية الأداء
hw-report-cpu-features = تسريعات CPU
hw-report-none = لا شيء
hw-report-memory = الذاكرة
hw-report-memory-value = { $total } إجمالًا، { $available } متاحة
hw-report-graphics-api = واجهة الرسوميات
hw-report-driver = التعريف
hw-report-max-texture = أكبر نسيج
hw-report-pixels = { $width } × { $height } بكسل
hw-report-decode-threads = خيوط فك الترميز
hw-report-ram-cache = ذاكرة الصور المؤقتة
hw-report-vram-cache = ذاكرة GPU المؤقتة
hw-report-processing = المعالجة
hw-report-path-jpeg-decode = فك ترميز JPEG
hw-report-path-ycbcr-to-rgb = تحويل الألوان
hw-report-path-png-unfilter = فك ترميز PNG
hw-report-path-heic-decode = فك ترميز HEIC
hw-report-path-inference = تحليل الصور
hw-report-engine-cpu = CPU
hw-report-engine-cpu-parallel = CPU، بالتوازي
hw-report-engine-gpu = GPU
hw-report-engine-hw-decoder = مفكّك الترميز بالعتاد
hw-report-engine-npu = NPU
# machine-drafted, needs native review
hw-report-build-debug = إصدار تنقيح
hw-report-build-release = إصدار نهائي

## Accessibility (screen readers)
# The image as a screen reader announces it: file name and size in pixels.
a11y-image = { $name }، { $width } × { $height } بكسل
# The region where short notifications (toasts) appear.
a11y-notifications = الإشعارات
# machine-drafted, needs native review
a11y-transport-playback = التشغيل
a11y-transport-pages = الصفحات
a11y-settings-sections = الأقسام

## Read codes (QR codes in the picture) (machine-drafted, needs native review)
codes-placeholder = الرموز في هذه الصورة
codes-looking = جارٍ البحث عن الرموز…
codes-none = لم يُعثر على رموز في هذه الصورة.
codes-failed = تعذّرت قراءة الصورة: { $reason }
codes-copy = نسخ «{ $text }»
# $kind is "QR", $version a number.
codes-kind = { $kind } { $version }
# Data Matrix: its size in modules, e.g. "Data Matrix 12×12".
# machine-drafted, needs native review
codes-kind-size = { $kind } { $rows }×{ $cols }
codes-open-row = فتح الرابط على { $host }…
codes-network-off = لا توجد شبكة مُشغَّلة، لذا لا تُفتح الروابط.
codes-open-title = فتح رابط إلى { $host }؟
codes-open-detail = يُفتح الرابط في متصفح الويب. لا تفتح إلا الروابط التي تثق بها.
codes-open = فتح في المتصفح
# machine-drafted, needs native review
# Find text: the text in the picture shown, read by the Local AI pack's Text models.
ocr-placeholder = النص في هذه الصورة
# machine-drafted, needs native review
ocr-looking = جارٍ قراءة النص…
# machine-drafted, needs native review
ocr-none = لم يُعثر على نص في هذه الصورة.
# machine-drafted, needs native review
ocr-failed = تعذّرت قراءة النص: { $reason }
# machine-drafted, needs native review
ocr-needs-pack = تحتاج ميزة البحث عن النص إلى نماذج النص في حزمة الذكاء الاصطناعي المحلي (الإعدادات › الذكاء الاصطناعي المحلي).
# machine-drafted, needs native review
ocr-loading = جارٍ تحميل نماذج النص…
# machine-drafted, needs native review
# A row: choosing it copies the line. $text is the line (one line, shortened).
ocr-copy = نسخ «{ $text }»
# machine-drafted, needs native review
ocr-copy-all = نسخ كل النص
# machine-drafted, needs native review
# A row's note: how sure the reading is, and the add-on that read it. $percent is formatted.
ocr-confidence = الثقة: { $percent }

## Hardware report panel (machine-drafted, needs native review)
hw-report-section-system = النظام
hw-report-section-gpu = GPU
hw-report-section-decoding = فك الترميز
hw-report-operating-system = نظام التشغيل
hw-report-adapter = المحوّل
hw-report-gpu-type = النوع
hw-report-gpu-init = زمن البدء
hw-report-max-layers = طبقات النسيج
hw-report-compression = ضغط النسيج
hw-report-unified-memory = ذاكرة مشتركة مع CPU
hw-report-display = الشاشة
hw-report-hw-decode = فك الترميز بالعتاد
hw-report-core-cluster = مجموعة الأنوية
hw-report-power = الطاقة
hw-report-calibration = قياسات السرعة
hw-report-codec-others = أخرى
hw-report-performance-cores-value = { $count } من { $total }
hw-report-cluster-value = { $name }: { $physical } فعلية، { $logical } منطقية
hw-report-thermal-nominal = حرارة عادية
hw-report-thermal-fair = دافئ
hw-report-thermal-serious = ساخن، يُبطَّأ الأداء
hw-report-thermal-critical = سخونة مفرطة
hw-report-power-battery = على البطارية ({ $percent })
hw-report-power-battery-unknown = على البطارية
hw-report-power-mains = موصول بالكهرباء
hw-report-power-mains-percent = موصول بالكهرباء ({ $percent })
hw-report-power-saving = توفير الطاقة مُفعّل
hw-report-gpu-discrete = مخصّص
hw-report-gpu-integrated = مدمج
hw-report-gpu-virtual = افتراضي
hw-report-gpu-software = برمجي، على CPU
hw-report-gpu-other = آخر
hw-report-gpu-unavailable = غير متاح
hw-report-ms = { $ms } ms
hw-report-yes = نعم
hw-report-no = لا
hw-report-unified-platform-differs = يفيد النظام بخلاف ذلك
hw-report-display-value = { $fps } Hz، هامش HDR الآن { $now }، حتى { $max }
hw-report-hw-decode-value = { $api } على { $device }
hw-report-hw-decode-checking = لا يزال الفحص جاريًا…
hw-report-codec-up-to = حتى { $width } × { $height }
hw-report-codec-none = غير متاح
hw-report-codec-h264-encode = ترميز H.264
hw-report-supported = مدعوم: { $list }
hw-report-unsupported = غير مدعوم: { $list }
hw-report-calibration-done = مقيسة على هذا الحاسوب
hw-report-calibration-pending = لم تُقَس بعد
hw-report-paths-power-saving = توفير الطاقة: تُفضَّل الطرق الأقل استهلاكًا للطاقة
hw-report-paths-caption = الطرق لصورة بدقة { $mpx } ميغابكسل
hw-report-col-task = المهمة
hw-report-col-path = الطريقة
hw-report-col-time = الزمن
hw-report-col-cpu = وقت CPU (كل الخيوط)
hw-report-col-runs = القياسات
hw-report-col-use = الاستخدام
hw-report-runs-untested = لم يُختبر
hw-report-use-viewing = العرض ({ $reason })
hw-report-use-background = الخلفية ({ $reason })
hw-report-reason-only = الخيار الوحيد
hw-report-reason-faster = الأسرع
hw-report-reason-cheaper = أقل وقت CPU
hw-report-reason-lower-power = أقل طاقة
hw-report-reason-explore = قيد التجربة
hw-report-reason-forced = مضبوط في الإعدادات
hw-report-reason-prefer-gpu = تفضيل GPU
hw-report-reason-cpu-only = CPU فقط
hw-report-reason-calibrate = قيد القياس
hw-report-reason-scaled = حجم مصغّر
hw-report-trust-unverified = لم يُفحص بعد
hw-report-trust-blocked = أُوقف بعد خطأ
hw-report-trust-absent = غير موجود في هذا الحاسوب

## Keyboard cheat sheet tabs and sections (machine-drafted, needs native review)
cheatsheet-mode-gallery = المعرض
cheatsheet-mode-compare = المقارنة
cheatsheet-gallery-browse = التصفح
cheatsheet-gallery-select = التحديد
cheatsheet-gallery-mark = التقييم ووضع العلامات
cheatsheet-gallery-move = التنقل بين الصور
cheatsheet-gallery-page = صفحة لأعلى أو لأسفل
cheatsheet-gallery-ends = الصورة الأولى أو الأخيرة
cheatsheet-gallery-up = الصعود إلى المجلد الأعلى
cheatsheet-gallery-history-back = الرجوع إلى المجلد السابق
cheatsheet-gallery-history-forward = التقدم إلى المجلد التالي
cheatsheet-gallery-back = إغلاق العرض العلوي ثم الرجوع
cheatsheet-gallery-viewer = العودة إلى العارض
cheatsheet-gallery-album-add = إضافة إلى آخر ألبوم
cheatsheet-gallery-select-one = تحديد أو إلغاء التحديد
cheatsheet-gallery-quick-look = نظرة سريعة
cheatsheet-gallery-filter = تصفية
cheatsheet-gallery-bigger = صور مصغّرة أكبر
cheatsheet-gallery-smaller = صور مصغّرة أصغر
cheatsheet-gallery-default-size = الحجم القياسي للصور المصغّرة
cheatsheet-gallery-refresh = تحديث المجلد
cheatsheet-gallery-menu = قائمة السياق
cheatsheet-compare-views = المقارنة
cheatsheet-compare-overlay = تراكب أو جنبًا إلى جنب
cheatsheet-compare-difference = الفرق (مرة أخرى: النوع التالي)
cheatsheet-compare-step-down = مزج أقل أو تضخيم أقل للفرق
cheatsheet-compare-step-up = مزج أكثر أو تضخيم أكبر للفرق
cheatsheet-compare-pick = الصورة التي تعمل عليها المفاتيح
cheatsheet-compare-close = إغلاق المقارنة
cheatsheet-channels = القنوات
cheatsheet-medical-window = النافذة والمستوى
cheatsheet-medical-measure = القياس والتعليقات التوضيحية
cheatsheet-medical-frames = الإطارات والسلاسل
# machine-drafted, needs native review
cheatsheet-medical-3d = الأبعاد الثلاثية وإعادة التشكيل
cheatsheet-medical-analysis = التقسيم والدمج وPET
# machine-drafted, needs native review
cheatsheet-medical-studies = الدراسات والمشاركة
cheatsheet-palette-only-title = في لوحة الأوامر ({ $key })
cheatsheet-palette-only-title-nokey = في لوحة الأوامر
cheatsheet-palette-only-tag = اللوحة
cheatsheet-palette-only-line = { $action }: بلا مفتاح؛ في لوحة الأوامر ({ $key })
cheatsheet-palette-only-line-nokey = { $action }: بلا مفتاح؛ في لوحة الأوامر
# Esc في ورقة الاختصارات (لا يتسع الوصف الطويل له في لوحة الأوامر).
cheatsheet-back = إغلاق أو رجوع

## Hardware video encoding (machine-drafted, needs native review)
hw-report-path-video-encode = تصدير الفيديو
hw-report-engine-hw-encoder = مرمّز بالعتاد

## Path timing units (machine-drafted, needs native review)
hw-report-task-thumbnail = { $task } (لكل صورة مصغّرة بحجم { $size })
hw-report-task-frame = { $task } (لكل إطار بحجم { $size })

## Hardware report, software-only formats (machine-drafted, needs native review)
hw-report-codec-software = برمجيًا فقط

about-natural-earth = بيانات الخريطة من Natural Earth (naturalearthdata.com)، ملكية عامة.
app-info-show-on-map = عرض على الخريطة
