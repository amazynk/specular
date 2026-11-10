# machine-drafted, needs native review
# Specular — نافذة الإعدادات. العربية الفصحى.

## إطار النافذة

settings-title = الإعدادات
settings-search-placeholder = البحث في الإعدادات
settings-search-no-results = لا توجد إعدادات تطابق { $query }
settings-reset = إعادة تعيين
settings-reset-section = إعادة تعيين القسم
settings-modified = معدَّل
settings-default-value = الافتراضي: { $value }
settings-restart-required = يسري مفعوله بعد إعادة التشغيل
settings-restart-now = إعادة التشغيل الآن
settings-undo = تراجع
settings-redo = إعادة
settings-value-on = تشغيل
settings-value-off = إيقاف
settings-value-auto = تلقائي
settings-value-system = اتباع النظام
settings-value-system-language = لغة النظام الافتراضية
settings-value-follow-language = اتباع لغة الواجهة
settings-value-default-location = الموقع الافتراضي
settings-choose = اختيار…
settings-clear = مسح
settings-file-reloaded = تم تغيير settings.toml خارج { -app-name } وأُعيد تحميله.
settings-warning-unknown-key = إعداد غير معروف { $key } في settings.toml. سيُحتفظ به في الملف لكن سيتم تجاهله.
settings-warning-invalid-value = قيمة غير صالحة لـ { $key }: { $reason } ستُستخدم القيمة الافتراضية بدلًا منها.
settings-warning-newer-version = كُتب settings.toml بإصدار أحدث من { -app-name }. سيُحتفظ بالإعدادات التي لا يعرفها هذا الإصدار.
settings-warning-syntax = تتعذّر قراءة settings.toml (السطر { $line }): { $reason } ستُستخدم القيم الافتراضية حتى يتم إصلاحه.

## أخطاء التحقق (تُطبع أيضًا بواسطة specular --set)

settings-error-unknown-key = لا يوجد إعداد باسم { $key }.
settings-error-unknown-key-suggestion = لا يوجد إعداد باسم { $key }. هل تقصد { $suggestion }؟
settings-error-not-bool = المتوقع تشغيل أو إيقاف (true أو false).
settings-error-not-integer = المتوقع عدد صحيح.
settings-error-not-number = المتوقع رقم.
settings-error-out-of-range = يجب أن تكون القيمة بين { $min } و{ $max }.
settings-error-out-of-range-auto = يجب أن تكون القيمة 0 (تلقائي) أو بين { $min } و{ $max }.
settings-error-unknown-option = المتوقع واحد مما يلي: { $options }.
settings-error-bad-colour = المتوقع لون مثل #1E1E1E أو #1E1E1E80.
settings-error-bad-language = المتوقع وسم لغة مثل de أو pt-BR أو zh-Hans، أو “system”.
settings-error-bad-path = المتوقع مسار ملف أو مجلد.
settings-error-not-settable = هذا العنصر أمر وليست له قيمة.
settings-error-syntax = المتوقع key=value، على سبيل المثال viewing.wheel_behaviour=navigate.
# machine-drafted, needs native review
settings-error-bad-text = لا يمكن أن يحتوي على علامات جدولة أو فواصل أسطر أو أحرف تحكم أخرى.
settings-error-invalid = { $reason }
settings-error-tile-url = يحتاج إلى عنوان http أو https يتضمن {"{"}z{"}"} و{"{"}x{"}"} و{"{"}y{"}"}.

## الوحدات

unit-percent = { PERCENT($value) }
unit-milliseconds = { $value } ملي ثانية
unit-seconds = { $value } ثانية
unit-mebibytes = { $value } MiB
# machine-drafted, needs native review
unit-luminance = { $value } cd/m²
unit-images =
    { $value ->
        [zero] لا توجد صور
        [one] صورة واحدة
        [two] صورتان
        [few] { $value } صور
        [many] { $value } صورة
       *[other] { $value } من الصور
    }
unit-threads =
    { $value ->
        [zero] لا توجد خيوط معالجة
        [one] خيط معالجة واحد
        [two] خيطا معالجة
        [few] { $value } خيوط معالجة
        [many] { $value } خيطًا للمعالجة
       *[other] { $value } خيط معالجة
    }
unit-bytes =
    { $value ->
        [zero] صفر بايت
        [one] بايت واحد
        [two] بايتان
        [few] { $value } بايتات
        [many] { $value } بايتًا
       *[other] { $value } بايت
    }
unit-kilobytes = { $value } كيلوبايت
unit-megabytes = { $value } ميغابايت
unit-gigabytes = { $value } غيغابايت
unit-terabytes = { $value } تيرابايت
unit-kibibytes = { $value } KiB
unit-gibibytes = { $value } GiB
unit-tebibytes = { $value } TiB
unit-symbol-mm = مم
unit-symbol-cm = cm
unit-symbol-m = m
unit-symbol-km = km
unit-symbol-mm2 = مم²
unit-symbol-cm2 = cm²
unit-symbol-m2 = m²
unit-symbol-km2 = km²
unit-symbol-ha = ha
unit-symbol-ft = ft
unit-symbol-ft2 = ft²
unit-symbol-mi = mi
unit-symbol-mi2 = mi²
unit-symbol-ac = ac

## الأقسام

settings-section-general = عام
settings-section-viewing = العرض
settings-section-browsing = التصفح
settings-section-colour = الألوان
settings-section-metadata = البيانات الوصفية والخصوصية
# machine-drafted, needs native review
settings-section-privacy = الخصوصية
settings-section-performance = الأداء
settings-section-accessibility = إمكانية الوصول
settings-section-keys = المفاتيح
settings-section-advanced = متقدم

## عام

settings-general-language = لغة الواجهة
settings-general-language-help = تسري التغييرات فورًا. يتبع خيار «لغة النظام الافتراضية» لغة نظام التشغيل لديك.
settings-general-region-formats = تنسيقات المنطقة
settings-general-region-formats-help = كيفية كتابة التواريخ والأوقات والأرقام والأحجام. تتبع لغة الواجهة افتراضيًا.
settings-general-theme = المظهر
settings-general-theme-help = مظهر فاتح أو داكن للنوافذ وعناصر التحكم. لا تتغير الصور أبدًا.
settings-general-theme-system = اتباع النظام
settings-general-theme-light = فاتح
settings-general-theme-dark = داكن
# machine-drafted, needs native review
settings-general-on-start = عند فتح { -app-name } دون ملف
settings-general-on-start-help = ما يُعرض عند فتح { -app-name } بمفرده. الملف المفتوح من مدير الملفات أو سطر الأوامر يظهر دائمًا في العارض.
settings-general-on-start-start-screen = شاشة البدء
settings-general-on-start-gallery = معرض آخر مجلد
settings-general-on-start-library = المكتبة
settings-general-start-tips = إظهار «جرّب هذه» في شاشة البدء
settings-general-start-tips-help = صف من البطاقات بميزات مفيدة ومفاتيحها. يعيده «إظهار الترحيب» في لوحة الأوامر.
settings-general-tips = إظهار التلميحات
settings-general-tips-help = تلميح قصير، مرة واحدة، في أول مرة قد تفيد فيها ميزة، مثل المفتاح الذي يعرض مجلدًا كمعرض. لا يظهر أبدًا أثناء عرض الشرائح.
settings-general-tips-shown = التلميحات التي ظهرت
settings-general-tips-shown-help = التلميحات لمرة واحدة التي ظهرت من قبل، كي لا تتكرر.
# machine-drafted, needs native review
settings-general-your-name = اسمك
settings-general-your-name-help = يظهر على الأختام والعلامات التي تضيفها. يُحفظ فقط في ملفات العلامات التي تنشئها.
settings-general-single-instance = فتح الملفات في النافذة الحالية
settings-general-single-instance-help = عندما يكون { -app-name } قيد التشغيل بالفعل، يؤدي فتح صورة أخرى إلى عرضها في تلك النافذة بدلًا من تشغيل نسخة جديدة.
settings-general-file-associations = اقترانات الملفات…
settings-general-file-associations-help = اجعل { -app-name } العارض الافتراضي لتنسيقات الصور التي يدعمها.

## العرض

settings-viewing-initial-zoom = التكبير الأولي
settings-viewing-initial-zoom-help = الحجم الذي تُعرض به الصورة المفتوحة حديثًا.
settings-viewing-initial-zoom-fit = ملاءمة للنافذة
settings-viewing-initial-zoom-fit-if-larger = ملاءمة الصور الكبيرة، وعرض الصغيرة بنسبة { PERCENT(100) }
settings-viewing-initial-zoom-actual-size = الحجم الفعلي ({ PERCENT(100) })
settings-viewing-initial-zoom-fill = ملء النافذة
settings-viewing-wheel-behaviour = عجلة الماوس
settings-viewing-wheel-behaviour-help = وظيفة تدوير العجلة. اضغط مطولًا على Ctrl لتنفيذ الإجراء الآخر.
settings-viewing-wheel-behaviour-help-macos = وظيفة تدوير العجلة. اضغط مطولًا على ⌘ لتنفيذ الإجراء الآخر.
settings-viewing-wheel-behaviour-zoom = تكبير
settings-viewing-wheel-behaviour-navigate = الصورة التالية والسابقة
settings-viewing-keep-zoom = الإبقاء على التكبير بين الصور
settings-viewing-keep-zoom-help = عند التكبير، تُفتح الصورة التالية والسابقة بالتكبير نفسه وتُظهر الجزء نفسه من الإطار: مفيد لفحص التركيز في سلسلة لقطات.
settings-viewing-zoom-animation = تحريك التكبير
settings-viewing-zoom-animation-help = تكبير سلس بدلًا من القفز. يُوقف تلقائيًا عند تشغيل «تقليل الحركة».
settings-viewing-zoom-animation-speed = سرعة حركة التكبير
settings-viewing-zoom-animation-speed-help = مدى سرعة استقرار التكبير المتحرك.
settings-viewing-zoom-animation-speed-relaxed = هادئة
settings-viewing-zoom-animation-speed-normal = عادية
settings-viewing-zoom-animation-speed-fast = سريعة
settings-viewing-background = الخلفية
settings-viewing-background-help = اللون خلف الصورة في النافذة.
settings-viewing-background-theme = مطابقة المظهر
settings-viewing-background-black = أسود
settings-viewing-background-dark-grey = رمادي داكن
settings-viewing-background-light-grey = رمادي فاتح
settings-viewing-background-white = أبيض
settings-viewing-background-custom = لون مخصص
settings-viewing-background-color = لون الخلفية المخصص
settings-viewing-background-color-help = يُستخدم عند تعيين الخلفية إلى «لون مخصص».
settings-viewing-fullscreen-background = خلفية وضع ملء الشاشة
settings-viewing-fullscreen-background-help = اللون خلف الصورة في وضع ملء الشاشة.
settings-viewing-transparency = المناطق الشفافة
settings-viewing-transparency-help = كيفية رسم الأجزاء الشفافة من الصورة.
settings-viewing-transparency-checkerboard = نمط رقعة الشطرنج
settings-viewing-transparency-background = لون الخلفية
settings-viewing-transparency-black = أسود
settings-viewing-transparency-white = أبيض
settings-viewing-pixel-grid-threshold = إظهار شبكة البكسلات بدءًا من
settings-viewing-pixel-grid-threshold-help = رسم شبكة بين البكسلات عند التكبير إلى هذا المستوى على الأقل.
settings-viewing-smoothing-high-zoom = البكسلات المكبّرة
settings-viewing-smoothing-high-zoom-help = مظهر البكسلات عند عرض الصورة بحجم أكبر من { PERCENT(100) }.
settings-viewing-smoothing-high-zoom-auto = ناعمة، ثم حادة بدءًا من { PERCENT(400) }
settings-viewing-smoothing-high-zoom-smooth = ناعمة دائمًا
settings-viewing-smoothing-high-zoom-pixelated = مربعات حادة دائمًا
settings-viewing-double-click = النقر المزدوج على الصورة
settings-viewing-double-click-help = يؤدي النقر المزدوج على الخلفية الفارغة دائمًا إلى تبديل وضع ملء الشاشة.
settings-viewing-double-click-toggle-zoom = التبديل بين الملاءمة و{ PERCENT(100) }
settings-viewing-double-click-fullscreen = تبديل وضع ملء الشاشة
settings-viewing-double-click-nothing = عدم القيام بأي شيء
settings-viewing-fullscreen-on-open = فتح الصور في وضع ملء الشاشة
settings-viewing-fullscreen-on-open-help = البدء في وضع ملء الشاشة عند فتح صورة من خارج { -app-name }.
settings-viewing-fullscreen-mode = نمط ملء الشاشة
settings-viewing-fullscreen-mode-help = يستخدم وضع ملء الشاشة الأصلي مساحة Space خاصة به؛ أما ملء الشاشة الفوري فيغطي المساحة الحالية دون حركة انتقالية.
settings-viewing-fullscreen-mode-native = أصلي (مساحة Space خاصة)
settings-viewing-fullscreen-mode-borderless = فوري
settings-viewing-toolbar-autohide = إخفاء شريط الأدوات تلقائيًا
settings-viewing-toolbar-autohide-help = يتلاشى شريط الأدوات وأسهم الحواف عندما يكون المؤشر ساكنًا ويعودان عند تحريكه.
settings-viewing-toolbar-autohide-delay = إخفاء شريط الأدوات بعد
settings-viewing-toolbar-autohide-delay-help = المدة التي يجب أن يبقى فيها المؤشر ساكنًا قبل أن يتلاشى شريط الأدوات.
settings-viewing-toolbar-items = أزرار شريط الأدوات
settings-viewing-toolbar-items-help = الأزرار التي يعرضها شريط الأدوات وترتيبها.
settings-viewing-toolbar-drag = إعادة ترتيب شريط الأدوات بالسحب
settings-viewing-toolbar-drag-help = اسحب زرًا على امتداد شريط الأدوات لنقله. النقرة القصيرة لا تزال تضغط الزر، ويعيد Esc الزر المسحوب إلى مكانه.
settings-viewing-toolbar-items-open = فتح
settings-viewing-toolbar-items-prev = الصورة السابقة
settings-viewing-toolbar-items-next = الصورة التالية
settings-viewing-toolbar-items-zoom-out = تصغير
settings-viewing-toolbar-items-zoom-level = مستوى التكبير
settings-viewing-toolbar-items-zoom-in = تكبير
settings-viewing-toolbar-items-fit = ملاءمة للنافذة
settings-viewing-toolbar-items-rotate-left = تدوير عكس اتجاه عقارب الساعة
settings-viewing-toolbar-items-rotate-right = تدوير مع اتجاه عقارب الساعة
settings-viewing-toolbar-items-info = المعلومات
settings-viewing-toolbar-items-copy = نسخ الصورة
settings-viewing-toolbar-items-delete = نقل إلى سلة المهملات
settings-viewing-toolbar-items-palette = لوحة الأوامر
settings-viewing-toolbar-items-settings = الإعدادات
settings-viewing-toolbar-items-fullscreen = ملء الشاشة
# machine-drafted, needs native review
settings-viewing-toolbar-items-spacer = مسافة مرنة
settings-viewing-toolbar-items-separator = فاصل
settings-toolbar-preview = معاينة شريط الأدوات
settings-toolbar-hidden = الأزرار المخفية
settings-toolbar-add-spacer = إضافة مسافة مرنة
settings-toolbar-add-separator = إضافة فاصل
settings-toolbar-reset = استعادة الافتراضي
settings-toolbar-remove = إزالة { $item }
settings-toolbar-hint = اسحب صفًا لنقله، أو حدده واضغط Alt+↑ أو Alt+↓. ينتقل Tab بين حقل البحث والقائمة.
settings-toolbar-move-up = نقل لأعلى
settings-toolbar-move-down = نقل لأسفل
settings-toolbar-move-start = نقل إلى البداية
settings-toolbar-move-end = نقل إلى النهاية
settings-toolbar-menu-hide = إخفاء
settings-toolbar-menu-show = إظهار
settings-toolbar-menu-remove = إزالة
settings-toolbar-row-menu = مزيد من الإجراءات لـ { $item }
settings-toolbar-position = الموضع { $index } من { $count }
settings-toolbar-hidden-state = مخفي

## التصفح

settings-browsing-sort-order = ترتيب الصور حسب
settings-browsing-sort-order-help = الترتيب المستخدم عند التنقل في مجلد. يضع الترتيب حسب الاسم «img2» قبل «img10» ويتبع الأبجدية في لغتك.
settings-browsing-sort-order-natural = الاسم
settings-browsing-sort-order-date-taken = تاريخ الالتقاط
settings-browsing-sort-order-modified = تاريخ التعديل
settings-browsing-sort-order-size = حجم الملف
settings-browsing-sort-order-type = نوع الملف
settings-browsing-sort-descending = عكس الترتيب
settings-browsing-sort-descending-help = الترتيب من الأخير إلى الأول.
settings-browsing-loop-folder = التكرار عند نهاية المجلد
settings-browsing-loop-folder-help = بعد الصورة الأخيرة، المتابعة من الأولى، والعكس بالعكس.
settings-browsing-include-subfolders = تضمين المجلدات الفرعية
settings-browsing-include-subfolders-help = التنقل عبر الصور في المجلدات الفرعية أيضًا.
settings-browsing-hold-acceleration = التسريع عند الضغط المطول على التالي أو السابق
settings-browsing-hold-acceleration-help = مدى سرعة تزايد التقليب أثناء الضغط المطول على مفتاح أو زر.
settings-browsing-hold-acceleration-off = بلا تسريع
settings-browsing-hold-acceleration-gentle = خفيف
settings-browsing-hold-acceleration-normal = عادي
settings-browsing-hold-acceleration-fast = سريع
settings-browsing-confirm-trash = السؤال قبل النقل إلى سلة المهملات
settings-browsing-confirm-trash-help = يمكن دائمًا التراجع عن النقل إلى سلة المهملات، لذا لا يسأل { -app-name } افتراضيًا.
settings-browsing-arrows-follow-reading-direction = مفاتيح الأسهم تتبع اتجاه القراءة
settings-browsing-arrows-follow-reading-direction-help = في اللغات التي تُكتب من اليمين إلى اليسار، يجعل مفتاح السهم الأيسر ينتقل إلى الصورة التالية.

## الألوان

settings-colour-management = إدارة الألوان
settings-colour-management-help = عرض الألوان كما يُفترض أن تبدو، باستخدام ملف تعريف ألوان الصورة وملف تعريف شاشتك.
settings-colour-display-profile = ملف تعريف الشاشة
settings-colour-display-profile-help = ملف تعريف ألوان شاشتك. يستخدم الخيار «تلقائي» الملف الذي يُبلغ عنه نظام التشغيل.
settings-colour-rendering-intent = هدف العرض
settings-colour-rendering-intent-help = كيفية إدخال الألوان التي لا تستطيع الشاشة عرضها ضمن نطاقها.
settings-colour-rendering-intent-perceptual = إدراكي
settings-colour-rendering-intent-relative-colorimetric = لوني نسبي
settings-colour-rendering-intent-saturation = التشبع
settings-colour-rendering-intent-absolute-colorimetric = لوني مطلق
settings-colour-black-point-compensation = تعويض النقطة السوداء
settings-colour-black-point-compensation-help = الحفاظ على تفاصيل الظلال عندما يكون اللون الأسود في الشاشة أفتح منه في الصورة.
settings-colour-untagged-images = الصور التي لا تحتوي على ملف تعريف ألوان
settings-colour-untagged-images-help = كيفية عرض الصور التي لا تحدد الألوان التي تستخدمها.
settings-colour-untagged-images-srgb = معاملتها على أنها sRGB
settings-colour-untagged-images-display = عرضها دون تغيير
settings-colour-dithering = التنقيط
settings-colour-dithering-help = إضافة ضوضاء زرقاء دقيقة عند تقليل عمق الألوان، مما يمنع ظهور التدرجات المتقطعة في السماء والتدرجات اللونية.
settings-colour-hdr-output = إخراج HDR
settings-colour-hdr-output-help = عرض صور HDR بكامل سطوعها على الشاشات التي تدعم ذلك. يستخدم الوضع التلقائي HDR عندما تُبلغ الشاشة عن سطوع إضافي، وإلا فتُضغط المناطق الساطعة بلطف.
settings-colour-hdr-output-auto = تلقائي
settings-colour-hdr-output-on = تشغيل
settings-colour-hdr-output-off = إيقاف
# machine-drafted, needs native review
settings-colour-ocio-config = إعداد OCIO
settings-colour-ocio-config-help = إعداد OpenColorIO (.ocio) لصور EXR وRadiance HDR وPFM. إذا تُرك فارغًا فسيُستخدم الإعداد الذي يحدده ‎$OCIO.
settings-colour-ocio-display = شاشة OCIO
settings-colour-ocio-display-help = الشاشة (display) في إعداد OCIO التي تطابق شاشتك.
settings-colour-ocio-display-default = الأولى في الإعداد
settings-colour-ocio-view = طريقة عرض OCIO
settings-colour-ocio-view-help = كيفية عرض الصور scene-linear على تلك الشاشة، مثل طريقة عرض بتعيين النغمات.
settings-colour-ocio-view-default = الأولى للشاشة
settings-colour-ocio-input = فضاء ألوان الإدخال في OCIO
settings-colour-ocio-input-help = فضاء الألوان لقيم الصور.
settings-colour-ocio-input-default = دور scene-linear في الإعداد
# machine-drafted, needs native review
settings-colour-proof-profile = ملف تعريف التدقيق على الشاشة
settings-colour-proof-profile-help = ملف تعريف الطابعة الذي استخدمه التدقيق في التحميض آخر مرة.
settings-colour-proof-intent = غرض التدقيق على الشاشة
settings-colour-proof-intent-help = كيف تُقرَّب الألوان التي لا تطبعها الطابعة.
settings-colour-proof-intent-perceptual = إدراكي
settings-colour-proof-intent-relative = لوني نسبي
settings-colour-proof-bpc = تعويض النقطة السوداء في التدقيق
settings-colour-proof-bpc-help = مطابقة الأسود مع أسود الطابعة.
settings-colour-proof-paper = أبيض الورق في التدقيق
settings-colour-proof-paper-help = إظهار لون الورق نفسه.
settings-colour-proof-ink = أسود الحبر في التدقيق
settings-colour-proof-ink-help = إظهار مدى قتامة أسود الحبر.

## البيانات الوصفية والخصوصية

settings-metadata-show-info-on-open = إظهار لوحة المعلومات عند الفتح
settings-metadata-show-info-on-open-help = فتح اللوحة التي تعرض تفاصيل الكاميرا والتاريخ والمكان كلما فُتحت صورة.
settings-metadata-info-detail = تعرض لوحة المعلومات
settings-metadata-info-detail-help = يشرح الملخص أهم التفاصيل بلغة بسيطة؛ أما «كل الحقول» فيسرد كل ما هو مخزن في الملف.
settings-metadata-info-detail-summary = ملخص
settings-metadata-info-detail-all = كل الحقول
settings-metadata-reverse-geocoding = إظهار أسماء الأماكن لمواقع GPS
settings-metadata-reverse-geocoding-help = البحث عن أقرب بلدة من بيانات مضمّنة. يعمل دون اتصال: لا تغادر المواقع حاسوبك أبدًا.
settings-metadata-hide-gps = إخفاء مواقع GPS على الشاشة
settings-metadata-hide-gps-help = عدم إظهار الإحداثيات أو الأماكن، على سبيل المثال أثناء مشاركة شاشتك. لا تتغير الملفات.
# machine-drafted, needs native review
settings-metadata-develop-sidecar = حفظ تعديلات التحميض بجانب الصورة
# machine-drafted, needs native review
settings-metadata-develop-sidecar-help = يكتب IMG_1234.CR2.specular.xmp بجانب الملف، فتنتقل التعديلات معه. عند الإيقاف: تبقى التعديلات في المكتبة فقط.
# machine-drafted, needs native review
settings-metadata-develop-import-lightroom = استورد إعدادات Lightroom في التحميض
# machine-drafted, needs native review
settings-metadata-develop-import-lightroom-help = تُفتح الصورة التي فيها إعدادات Lightroom بلا تعديل بها، تقريبيًا. عند الإيقاف: يعرض التحميض زرًا.
settings-metadata-units = الوحدات
settings-metadata-units-help = وحدات الارتفاع والمسافات والسرعات.
settings-metadata-units-auto = اتباع تنسيقات المنطقة
settings-metadata-units-metric = متري
settings-metadata-units-imperial = إمبراطوري
# machine-drafted, needs native review
settings-metadata-basemap = الخريطة الأساسية
settings-metadata-basemap-help = الخريطة أسفل مواقع الصور. تنزّل الخرائط عبر الإنترنت مربعات المنطقة التي تعرضها.
settings-metadata-basemap-off = الخريطة المضمّنة فقط (دون اتصال)
settings-metadata-basemap-openstreetmap = OpenStreetMap (عبر الإنترنت)
settings-metadata-basemap-custom = خادم مربعات آخر (عبر الإنترنت)
settings-metadata-basemap-file = ملف خريطة خاص بي
settings-metadata-basemap-url = عنوان خادم المربعات
settings-metadata-basemap-url-help = عنوان يحتوي على {"{"}z{"}"} و{"{"}x{"}"} و{"{"}y{"}"}، مثل https://tiles.example.org/{"{"}z{"}"}/{"{"}x{"}"}/{"{"}y{"}"}.png
settings-metadata-basemap-path = ملف الخريطة
settings-metadata-basemap-path-help = ملف MBTiles أو PMTiles. يعمل دون اتصال.
settings-metadata-basemap-cache = ذاكرة التخزين المؤقت لمربعات الخريطة
settings-metadata-basemap-cache-help = مساحة القرص للمربعات المنزّلة. تُزال الأقدم أولًا.
# machine-drafted, needs native review
settings-metadata-basemap-allowed = خوادم المربعات المسموح بها
settings-metadata-basemap-allowed-help = خوادم الخرائط التي وافقت على تنزيل المربعات منها.

## الأداء

settings-performance-ram-cache = ذاكرة التخزين المؤقت
settings-performance-ram-cache-help = الذاكرة المستخدمة لإبقاء الصور المفكوكة جاهزة. يختار الخيار «تلقائي» بناءً على حجم ذاكرة حاسوبك.
settings-performance-vram-cache = ذاكرة التخزين المؤقت في ذاكرة GPU
settings-performance-vram-cache-help = ذاكرة GPU المستخدمة للصور والمربعات. يختار الخيار «تلقائي» بناءً على وحدة GPU لديك.
settings-performance-prefetch-depth = الصور المُعدّة مسبقًا
settings-performance-prefetch-depth-help = عدد الصور التي يتم فك ترميزها مسبقًا في اتجاه تنقلك.
settings-performance-decoder-threads = خيوط فك الترميز
settings-performance-decoder-threads-help = خيوط CPU المستخدمة لفك ترميز الصور. يستخدم الخيار «تلقائي» جميع أنوية الأداء ويتكيف مع الملف.
settings-performance-gpu-adapter = GPU
settings-performance-gpu-adapter-help = وحدة GPU التي تُستخدم عندما يحتوي الحاسوب على أكثر من وحدة واحدة.
settings-performance-gpu-adapter-high-performance = أعلى أداء
settings-performance-gpu-adapter-low-power = أقل استهلاك للطاقة
settings-performance-processing = أين تُعالَج الصور
settings-performance-processing-help = يمكن تنفيذ بعض الأعمال على CPU أو GPU أو مفكّك ترميز بالعتاد. يقيس الوضع التلقائي هذا الحاسوب ويختار أسرع طريقة للصورة المعروضة وأوفر طريقة للعمل في الخلفية. يظهر أدناه ما اختير على هذا الحاسوب وسببه.
settings-performance-processing-automatic = تلقائي
settings-performance-processing-prefer-gpu = تفضيل GPU
settings-performance-processing-cpu-only = CPU فقط
settings-performance-drag-preview = المعاينة أثناء السحب
settings-performance-drag-preview-help = كيف يعرض التحميض الصورة أثناء سحب شريط التمرير. التلقائي يستخدم صورة أصغر فقط عندما لا يستطيع هذا الحاسوب المواكبة.
settings-performance-drag-preview-automatic = تلقائي
settings-performance-drag-preview-sharp = حادة دائمًا
settings-performance-drag-preview-fast = سريعة دائمًا

## Settings → Performance → How { -app-name } runs on this computer (DESIGN.md §12.3.1)
# machine-drafted, needs native review
settings-paths-title = كيف يعمل { -app-name } على هذا الحاسوب
settings-paths-intro = يقيس { -app-name } زمن كل طريقة لأداء هذه الأعمال على هذا الحاسوب ويستخدم أفضلها. الأزمنة لصورة بدقة { $mpx } ميغابكسل.
settings-paths-intro-measuring = لم يكتمل القياس بعد: يقيس { -app-name } حين يكون الحاسوب خاملًا.
settings-paths-intro-battery = على البطارية: تُفضَّل الخيارات الأقل استهلاكًا للطاقة.
settings-paths-keywords = GPU CPU NPU بطاقة الرسومات العتاد تسريع مفكك الترميز قياس الأداء السرعة
paths-engine-cpu = CPU
paths-engine-cpu-parallel = CPU، كل الأنوية
paths-engine-gpu = GPU
paths-engine-hw-decoder = مفكك الترميز بالعتاد
paths-engine-npu = NPU
settings-paths-your-setting = { $engine } (إعدادك)
settings-paths-measured-faster = قيس على هذا الحاسوب: أسرع من { $other } بـ{ $ratio } مرة.
settings-paths-measured-similar = قيس على هذا الحاسوب: بسرعة { $other } تقريبًا.
settings-paths-measured-power = على البطارية هذه الطريقة الأقل استهلاكًا للطاقة (قيس على هذا الحاسوب).
settings-paths-not-measured = لم يُقَس بعد: الاختيار المعتاد لهذا النوع من الحواسيب.
settings-paths-set-by = اختاره إعدادك «{ $setting }: { $option }».
settings-paths-blocked-check = أُوقف { $engine } في { $date }: لم تطابق نتيجته نتيجة CPU.
settings-paths-blocked-error = أُوقف { $engine } في { $date } بعد خطأ.
settings-paths-blocked-lost = أُوقف { $engine } في { $date } بعد أن توقف عن الاستجابة.
settings-paths-blocked-undated = أُوقف { $engine } بعد خطأ.
settings-paths-checking = يُقارَن { $engine } مع CPU قبل استخدامه للصورة المعروضة.
settings-paths-exploring = تجري تجربة { $engine } في الخلفية الآن.
settings-paths-unavailable = لا يوجد في هذا الحاسوب { $engine } لهذا العمل.
settings-paths-background-cheaper = في الخلفية: { $engine }، وقد قيس أنه يستهلك وقت CPU أقل بنسبة { $percent }.
settings-paths-background-power = في الخلفية: { $engine }، لتوفير الطاقة.
settings-paths-background = في الخلفية: { $engine }.
settings-paths-detail-measured = { $engine }: { $time } ms، ووقت CPU { $cpu } ms، { $runs ->
    [zero] بلا قياسات
    [one] قياس واحد
    [two] قياسان
    [few] { $runs } قياسات
    [many] { $runs } قياسًا
   *[other] { $runs } قياس
} ({ $confidence })
settings-paths-detail-guess = { $engine }: نحو { $time } ms (تقدير)
settings-paths-confidence-low = ثقة منخفضة
settings-paths-confidence-medium = ثقة متوسطة
settings-paths-confidence-high = ثقة عالية
settings-paths-machine = قيس لـ { $gpu } · { $driver } · { $cpu }
settings-paths-retry = إعادة المحاولة
settings-paths-measure-again = إعادة القياس
settings-paths-measure-again-question = هل تريد نسيان ما قيس على هذا الحاسوب، بما في ذلك الطرق الموقَفة، وإعادة القياس حين يكون الحاسوب خاملًا؟
settings-paths-measure-again-confirm = نسيان القياسات

settings-performance-display-cache = الاحتفاظ بذاكرة تخزين مؤقت للعرض على القرص
settings-performance-display-cache-help = تخزين نسخ من الصور بحجم الشاشة لكي تُفتح المجلدات التي شاهدتها فورًا.
settings-performance-display-cache-size = حجم ذاكرة التخزين المؤقت للعرض
settings-performance-display-cache-size-help = تُزال أقدم الإدخالات عندما تمتلئ ذاكرة التخزين المؤقت.
settings-performance-display-cache-location = موقع ذاكرة التخزين المؤقت للعرض
settings-performance-display-cache-location-help = يُفضل استخدام مجلد على محرك أقراص سريع.
settings-performance-fast-simd = استخدام تعليمات SIMD
settings-performance-fast-simd-help = استخدام تعليمات المتجهات في CPU لفك الترميز. لا توقفه إلا لتشخيص مشكلة.
settings-performance-fast-dct-scaling = فك الترميز بحجم الشاشة
settings-performance-fast-dct-scaling-help = فك ترميز ملفات JPEG الكبيرة مباشرة بالحجم الذي تُعرض به. لا توقفه إلا لتشخيص مشكلة.
settings-performance-fast-parallel-decode = فك الترميز على عدة أنوية
settings-performance-fast-parallel-decode-help = تقسيم الصور الكبيرة على أنوية CPU. لا توقفه إلا لتشخيص مشكلة.
settings-performance-fast-restart-index = فهرسة ملفات JPEG الكبيرة للتكبير
settings-performance-fast-restart-index-help = تذكّر موضع بداية كل جزء من ملف JPEG، لكي يتم فك ترميز المناطق المكبّرة بسرعة. لا توقفه إلا لتشخيص مشكلة.
settings-performance-fast-yuv-upload = تحويل الألوان على GPU
settings-performance-fast-yuv-upload-help = تحويل ألوان JPEG على GPU أو CPU. يختار الوضع التلقائي الأسرع على هذا الحاسوب. لا توقفه إلا لتشخيص مشكلة.
settings-performance-fast-yuv-upload-auto = تلقائي
settings-performance-fast-yuv-upload-on = تشغيل
settings-performance-fast-yuv-upload-off = إيقاف
settings-performance-fast-async-io = قراءة الملفات غير المتزامنة
settings-performance-fast-async-io-help = القراءة المسبقة باستخدام أسرع واجهة ملفات في نظام التشغيل. لا توقفه إلا لتشخيص مشكلة.
settings-performance-fast-mmap = الملفات المعيّنة في الذاكرة
settings-performance-fast-mmap-help = قراءة الملفات دون نسخها. لا توقفه إلا لتشخيص مشكلة.
settings-performance-hardware-decode = فك الترميز بالعتاد
settings-performance-hardware-decode-help = فتح صور JPEG بمفكّك الترميز المدمج في GPU. يستخدمه الوضع التلقائي حيث يفيد، عادةً للصور المُجهّزة في الخلفية. يُقارن أولًا بمفكّك الترميز العادي، وما لا يدعمه يُفتح بالطريقة المعتادة.
settings-performance-hardware-decode-auto = تلقائي
settings-performance-hardware-decode-on = تشغيل
settings-performance-hardware-decode-off = إيقاف
settings-performance-frame-time-hud = HUD زمن الإطار
settings-performance-frame-time-hud-help = إظهار أزمنة فك الترميز والإطارات والمسارات السريعة المستخدمة.

## إمكانية الوصول

settings-accessibility-reduce-motion = تقليل الحركة
settings-accessibility-reduce-motion-help = استبدال حركات التكبير والانزلاق والتلاشي بتغييرات فورية.
settings-accessibility-reduce-motion-system = اتباع النظام
settings-accessibility-reduce-motion-on = تشغيل
settings-accessibility-reduce-motion-off = إيقاف
settings-accessibility-increase-contrast = زيادة التباين
settings-accessibility-increase-contrast-help = حدود ونصوص أقوى، وشفافية أقل.
settings-accessibility-increase-contrast-system = اتباع النظام
settings-accessibility-increase-contrast-on = تشغيل
settings-accessibility-increase-contrast-off = إيقاف
settings-accessibility-text-scale = حجم النص
settings-accessibility-text-scale-help = حجم النص بالنسبة إلى الحجم القياسي.
settings-accessibility-ui-scale = مقياس الواجهة
settings-accessibility-ui-scale-help = حجم الواجهة بأكملها، بالإضافة إلى تحجيم الشاشة المعيّن في نظام التشغيل لديك.
settings-accessibility-focus-ring-always = إظهار حلقة التركيز دائمًا
settings-accessibility-focus-ring-always-help = إظهار عنصر التحكم الذي عليه تركيز لوحة المفاتيح حتى عند استخدام الماوس.
settings-accessibility-screen-reader-verbosity = تفاصيل قارئ الشاشة
# machine-drafted, needs native review
settings-accessibility-screen-reader-verbosity-help = مقدار ما يعلنه قارئ الشاشة عند التنقل بين الصور، ومقدار ما يقرأه من قراءة القيمة تحت المؤشر.
settings-accessibility-screen-reader-verbosity-concise = موجز
settings-accessibility-screen-reader-verbosity-normal = عادي
settings-accessibility-screen-reader-verbosity-verbose = مفصّل
# machine-drafted, needs native review
settings-accessibility-announce-cursor = قراءة القيمة تحت المؤشر
settings-accessibility-announce-cursor-help = يقرأ قارئ الشاشة قيمة البكسل أو HU أو درجة الحرارة أو الموضع تحت المؤشر أو مؤشر التقاطع بعد أن يتوقف عن الحركة.

## الخصوصية

# machine-drafted, needs native review
settings-privacy-no-network = بلا شبكة
# machine-drafted, needs native review
settings-privacy-no-network-help = عند الإيقاف: لا يتصل إلا ما يلي، وكلٌّ منها بعد أن تشغّله أو تفتح به شيئًا: التحقق من التحديثات، وتنزيل نماذج الذكاء الاصطناعي المحلي، والخرائط عبر الإنترنت، والصور من عنوان ويب أو تخزين سحابي، والخوادم التي تضيفها (DICOMweb وOMERO والخرائط). عند التشغيل: لا يتصل شيء.

## متقدم

settings-advanced-portable-mode = الوضع المحمول
settings-advanced-portable-mode-help = الاحتفاظ بالإعدادات وذاكرات التخزين المؤقت في المجلد المجاور للبرنامج بدلًا من ملف تعريف المستخدم.
settings-advanced-log-level = تفاصيل السجل
settings-advanced-log-level-help = مقدار ما يكتبه { -app-name } في ملف السجل الخاص به.
settings-advanced-log-level-error = الأخطاء فقط
settings-advanced-log-level-warn = التحذيرات
settings-advanced-log-level-info = المعلومات
settings-advanced-log-level-debug = تصحيح الأخطاء
settings-advanced-log-level-trace = كل شيء
settings-advanced-developer-mode = وضع المطوّر
settings-advanced-developer-mode-help = يعرض أدوات لتطوير { -app-name } واختباره، مثل اللغات الزائفة في قائمة اللغات.
# machine-drafted, needs native review
settings-advanced-audit-log = سجل تدقيق محلي
# machine-drafted, needs native review
settings-advanced-audit-log-help = يسجّل الصور الطبية المفتوحة والتصديرات وإخفاء الهوية والطباعة والإرسال إلى PACS في audit.jsonl داخل مجلد البيانات. يبقى على هذا الحاسوب.
settings-advanced-hw-report = تقرير الأجهزة…
settings-advanced-hw-report-help = إظهار ميزات CPU وGPU التي اكتشفها { -app-name } والمسارات السريعة التي يستخدمها.
# machine-drafted, needs native review
settings-advanced-renderer-info = معلومات العارض…
settings-advanced-renderer-info-help = إظهار محوّل الرسومات والتعريف وتنسيقات العرض وحدود الذاكرة التي يستخدمها { -app-name }، جاهزة للنسخ إلى تقرير خطأ.
settings-advanced-edit-settings-file = تحرير settings.toml
settings-advanced-edit-settings-file-help = حرّر ملف الإعدادات في محرر { -app-name } المدمج. يفحص الملف أثناء الكتابة ولا يحفظه إلا عندما يخلو من الأخطاء.
settings-advanced-import = استيراد الإعدادات…
settings-advanced-import-help = تحميل الإعدادات من ملف مُصدَّر من { -app-name }.
settings-advanced-export = تصدير الإعدادات…
settings-advanced-export-help = حفظ جميع الإعدادات في ملف واحد، للنسخ الاحتياطي أو لاستخدامها على حاسوب آخر.
settings-advanced-reset-all = إعادة تعيين جميع الإعدادات…
settings-advanced-reset-all-help = إعادة كل إعداد إلى قيمته الافتراضية. يمكن التراجع عن ذلك.

# Stage 1 merge (machine-drafted, needs native review)
settings-performance-stall-warning = الإبلاغ عن التحميل المتوقف بعد
settings-performance-stall-warning-help = إذا ظلت الصورة قيد التحميل بعد هذه المدة، تُكتب حالة تحميلها والمهام المعلقة في السجل (الخطأ القياسي أو SPECULAR_LOG_FILE).
# machine-drafted, needs native review
settings-performance-insights = تلميحات الأداء
settings-performance-insights-help = عندما تُفتح الصور ببطء مرارًا للسبب نفسه، يُعرض «لماذا هذا بطيء؟» في الزاوية مع ما قد يساعد. ما يتعلّمه يبقى على هذا الحاسوب.
settings-performance-insights-storage = تلميحات عن بطء التخزين
settings-performance-insights-storage-help = عرض التلميحات عندما تؤخّر قراءة الملفات (من مشاركة شبكية أو قرص بطيء أو أرشيفات) فتح الصور مرارًا.
settings-performance-insights-decoder = تلميحات عن فك الترميز
settings-performance-insights-decoder-help = عرض التلميحات عندما يتكرر فك ترميز الصور على CPU مع أن مفكك ترميز بالعتاد قد يساعد.
# machine-drafted, needs native review
settings-performance-insights-restore = إظهار التلميحات المخفية مجددًا
settings-performance-insights-restore-help = يعيد التلميحات التي أخفيتها بـ × في «لماذا هذا بطيء؟»، لفتح الصور وللصور المصغّرة معًا. غير متاح ما دام لا شيء مخفيًا.

## settings.toml editor (machine-drafted, needs native review)

config-editor-title = settings.toml
config-editor-save = حفظ
config-editor-revert = استرجاع
config-editor-close = إغلاق
config-editor-modified = معدّل
config-editor-text = ملف الإعدادات
config-editor-position = السطر { $line }، العمود { $column }
config-editor-saved = تم حفظ settings.toml وتطبيقه.
config-editor-cant-save = تعذّر الحفظ. عدد المشكلات التي يجب إصلاحها: { $count }.
config-editor-save-failed = تعذّر حفظ settings.toml: { $reason }
config-editor-conflict = تم تغيير settings.toml في مكان آخر.
config-editor-reload = إعادة التحميل
config-editor-keep-mine = الاحتفاظ بنسختي
config-editor-discard-question = تجاهل التغييرات غير المحفوظة في settings.toml؟
config-editor-discard = تجاهل
config-editor-keep-editing = متابعة التحرير
config-editor-find-placeholder = البحث في الملف
config-editor-find-count = { $current } من { $total }
config-editor-find-none = لا توجد نتائج
config-editor-find-previous = النتيجة السابقة
config-editor-find-next = النتيجة التالية
config-editor-find-close = إغلاق البحث
config-editor-problems = المشكلات
config-editor-problems-count =
    { $count ->
        [0] لا توجد مشكلات
       *[other] المشكلات: { $count }
    }
config-editor-no-problems = لا توجد مشكلات. يمكن حفظ الملف.
config-editor-problem-at = السطر { $line }: { $message }
config-editor-severity-error = خطأ
config-editor-severity-warning = تحذير
config-editor-severity-info = ملاحظة
config-editor-problem-syntax = TOML غير صالح: { $reason }
config-editor-problem-duplicate = تم تعيين { $key } مرتين في الجدول نفسه. احذف أحدهما.
config-editor-problem-unknown-section = لا يوجد قسم إعدادات [{ $key }].
config-editor-problem-unknown-section-suggestion = لا يوجد قسم إعدادات [{ $key }]. هل تقصد [{ $suggestion }]؟
config-editor-problem-unknown-key = لا يوجد إعداد { $key }.
config-editor-problem-unknown-key-suggestion = لا يوجد إعداد { $key }. هل تقصد { $suggestion }؟
config-editor-problem-invalid = قيمة غير صالحة لـ { $key }: { $reason }
config-editor-problem-migrated = كُتب { $key } كما كانت تكتبه نسخة أقدم من { -app-name }. يتم تحويله عند القراءة.
config-editor-problem-newer-version = كتبت هذا الملف نسخة أحدث من { -app-name } (التنسيق { $found }؛ هذه النسخة تقرأ { $supported }). يتم الاحتفاظ بالإعدادات غير المعروفة.
config-editor-problem-command = الأمر { $index } في [[commands]]: { $reason }
config-editor-problem-hook = الخطاف { $index } في [[hooks]]: { $reason }

## keymap.toml in the same editor (machine-drafted, needs native review)

config-editor-keymap-title = keymap.toml
config-editor-keymap-text = ملف اختصارات المفاتيح
config-editor-keymap-saved = تم حفظ keymap.toml وتطبيقه.
config-editor-keymap-save-failed = تعذّر حفظ keymap.toml: { $reason }
config-editor-keymap-conflict = تغيّر keymap.toml في مكان آخر.
config-editor-keymap-discard-question = هل تريد تجاهل تغييراتك غير المحفوظة على keymap.toml؟
config-editor-new-file = ملف جديد، يُنشأ عند الحفظ
config-editor-no-folder = لا يوجد مجلد إعدادات لحفظه فيه.

## Reset all settings confirmation (machine-drafted, needs native review)

settings-reset-all-confirm = هل تريد إعادة كل إعداد إلى قيمته الافتراضية؟
settings-reset-all-confirm-detail = يؤثر هذا في كل أقسام الإعدادات. يمكن التراجع عنه لاحقًا.
settings-reset-all-confirm-ok = إعادة تعيين الكل
settings-reset-all-done = أُعيدت كل الإعدادات إلى قيمها الافتراضية.

## Hardware video encoding (machine-drafted, needs native review)
paths-engine-hw-encoder = مرمّز بالعتاد

## Path timing units (machine-drafted, needs native review)
settings-paths-checking-before-use = يُقارَن { $engine } مع CPU قبل استخدامه.
settings-paths-unit-thumbnail = الأزمنة لصورة مصغّرة واحدة بحجم { $size }
settings-paths-unit-frame = الأزمنة لكل إطار فيديو بحجم { $size }
# machine-drafted, needs native review
settings-metadata-develop-calibration = مجلد إطارات المعايرة
# machine-drafted, needs native review
settings-metadata-develop-calibration-help = إطارات flat وdark للتحميض. تحتوي أسماء الملفات أو المجلدات على "flat" أو "dark".

## Colour swatches under a colour setting's field (machine-drafted, needs native review)
settings-colour-white = أبيض
settings-colour-light-grey = رمادي فاتح
settings-colour-grey = رمادي
settings-colour-dark-grey = رمادي داكن
settings-colour-black = أسود
settings-colour-yellow = أصفر
settings-colour-orange = برتقالي
settings-colour-red = أحمر
settings-colour-magenta = أرجواني
settings-colour-blue = أزرق
settings-colour-cyan = سماوي
settings-colour-green = أخضر
