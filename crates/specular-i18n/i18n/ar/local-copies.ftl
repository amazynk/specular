# machine-drafted, needs native review
# Local copies for offline editing (the Library).

settings-library-local-copies-limit = حدّ النسخ المحلية
settings-library-local-copies-limit-help = أقصى مساحة يمكن أن تشغلها النسخ المحلية في مجلد بيانات Specular.

local-keep-menu = الاحتفاظ بنسخة محلية…
local-remove-menu = إزالة النسخة المحلية…
local-keep-location-menu = الاحتفاظ بنسخة محلية من هذا الموقع…
local-remove-location-menu = إزالة النسخة المحلية من هذا الموقع…
local-keep-album-menu = الاحتفاظ بنسخة محلية من هذا الألبوم…
local-remove-album-menu = إزالة النسخة المحلية من هذا الألبوم…
local-remove-held-elsewhere = تتبع هذه النسخ موقعًا أو ألبومًا محتفظًا به. أزلها من هناك.

local-keep-ask = الاحتفاظ بنسخة محلية من { $item }؟
local-keep-detail = { $count ->
    [zero] لا صور.
    [one] صورة واحدة.
    [two] صورتان.
    [few] { $count } صور.
    [many] { $count } صورةً.
   *[other] { $count } صورة.
}
    الأصول: الملفات كما هي.
    النسخ الوسيطة: ملفات raw بدقة 2560 بكسل، تكفي للتحميض. تُنسخ الملفات الأخرى كاملة.
    تُحفظ النسخ في مجلد بيانات Specular. لا تتغير الأصول.
local-keep-originals = الاحتفاظ بالأصول
local-keep-proxies = الاحتفاظ بالنسخ الوسيطة
local-keeping = جارٍ نسخ { $item }…
local-kept = { $item }: { $count ->
    [zero] لا نسخ محلية
    [one] نسخة محلية واحدة
    [two] نسختان محليتان
    [few] { $count } نسخ محلية
    [many] { $count } نسخةً محلية
   *[other] { $count } نسخة محلية
}، { $size }.
local-kept-full = { $count ->
    [zero] لم تتجاوز أي صورة حدّ الحجم.
    [one] لم تتسع صورة واحدة ضمن حدّ الحجم.
    [two] لم تتسع صورتان ضمن حدّ الحجم.
    [few] لم تتسع { $count } صور ضمن حدّ الحجم.
    [many] لم تتسع { $count } صورةً ضمن حدّ الحجم.
   *[other] لم تتسع { $count } صورة ضمن حدّ الحجم.
}
local-kept-unreachable = { $count ->
    [zero] قُرئت كل الصور.
    [one] تعذّرت قراءة صورة واحدة.
    [two] تعذّرت قراءة صورتين.
    [few] تعذّرت قراءة { $count } صور.
    [many] تعذّرت قراءة { $count } صورةً.
   *[other] تعذّرت قراءة { $count } صورة.
}

local-remove-ask = إزالة النسخة المحلية من { $item }؟
local-remove-detail = تُحذف النسخ من مجلد بيانات Specular. لا تُمسّ الأصول. تبقى التغييرات المنتظرة.
local-remove-ok = إزالة النسخة المحلية
local-removed = أُزيلت النسخة المحلية من { $item }، وتحرّر { $size }.

local-banner-offline = { $location } غير متصل. أنت تعمل على النسخ المحلية.
local-banner-offline-many = مواقع غير متصلة: { $count }. أنت تعمل على النسخ المحلية.
local-banner-waiting = { $count ->
    [zero] لا تغييرات تنتظر الأصول.
    [one] تغيير واحد ينتظر الأصل.
    [two] تغييران ينتظران الأصول.
    [few] { $count } تغييرات تنتظر الأصول.
    [many] { $count } تغييرًا تنتظر الأصول.
   *[other] { $count } تغيير تنتظر الأصول.
}
local-banner-conflicts = { $count ->
    [zero] لم يتغير أي ملف وأنت غير متصل.
    [one] تغيّر ملف واحد وأنت غير متصل.
    [two] تغيّر ملفان وأنت غير متصل.
    [few] تغيّرت { $count } ملفات وأنت غير متصل.
    [many] تغيّر { $count } ملفًا وأنت غير متصل.
   *[other] تغيّر { $count } ملف وأنت غير متصل.
}
local-review = مراجعة…
local-sidebar-originals = نسخة محلية
local-sidebar-proxies = نسخ وسيطة محلية

local-synced = { $count ->
    [zero] لم يُكتب أي تغيير.
    [one] كُتب تغيير واحد إلى أصله.
    [two] كُتب تغييران إلى أصولهما.
    [few] كُتبت { $count } تغييرات إلى أصولها.
    [many] كُتب { $count } تغييرًا إلى أصولها.
   *[other] كُتب { $count } تغيير إلى أصولها.
}
local-sync-failed = { $count ->
    [zero] لا تغييرات فاشلة.
    [one] تعذّرت كتابة تغيير واحد. يبقى منتظرًا.
    [two] تعذّرت كتابة تغييرين. يبقيان منتظرين.
    [few] تعذّرت كتابة { $count } تغييرات. تبقى منتظرة.
    [many] تعذّرت كتابة { $count } تغييرًا. تبقى منتظرة.
   *[other] تعذّرت كتابة { $count } تغيير. تبقى منتظرة.
}

local-conflict-title = تغيّر { $file } وأنت غير متصل. أي نسخة تحتفظ بها: { $what }؟
local-conflict-yours = نسختك: { $text }
local-conflict-theirs = على القرص: { $text }
local-conflict-later = الإلغاء يؤجل القرار. لا يُستبدل شيء.
local-keep-mine = الاحتفاظ بنسختك
local-keep-theirs = الاحتفاظ بنسخة الملف
local-what-marks = التقييم
local-what-keywords = الكلمات المفتاحية
local-what-fields = حقول IPTC
local-what-edit = التعديل
local-stars = { $count ->
    [0] بلا نجوم
    [zero] بلا نجوم
    [one] نجمة واحدة
    [two] نجمتان
    [few] { $count } نجوم
    [many] { $count } نجمةً
   *[other] { $count } نجمة
}
local-label = { $label ->
    [1] أحمر
    [2] أصفر
    [3] أخضر
   *[4] أزرق
}
local-pick = مختارة
local-reject = مرفوضة
local-no-keywords = بلا كلمات مفتاحية
local-changed-elsewhere = تغيّر في مكان آخر
local-photo-changed = تغيّرت الصورة نفسها
local-edit-here = عُدّلت هنا، { $when }
local-edit-elsewhere = عُدّلت في مكان آخر، { $when }
local-edit-none = بلا تعديل

local-export-ask = { $count ->
    [zero] لا صور غير متصلة. التصدير من النسخ الوسيطة؟
    [one] صورة واحدة غير متصلة. تصديرها من نسختها الوسيطة؟
    [two] صورتان غير متصلتين. تصديرهما من نسختيهما الوسيطتين؟
    [few] { $count } صور غير متصلة. تصديرها من نسخها الوسيطة؟
    [many] { $count } صورةً غير متصلة. تصديرها من نسخها الوسيطة؟
   *[other] { $count } صورة غير متصلة. تصديرها من نسخها الوسيطة؟
}
local-export-detail = النسخ الوسيطة بطول 2560 بكسل على الضلع الأطول. الأصول تعطي الحجم الكامل.
local-export-proxy = التصدير من النسخ الوسيطة
local-export-wait = انتظار الأصول
local-export-waiting = يبدأ التصدير عند عودة الأصول.

local-panel-title = النسخ المحلية
local-panel-used = المستخدم { $used } من { $limit }.
local-panel-originals = { $count ->
    [zero] لا أصول
    [one] أصل واحد
    [two] أصلان
    [few] { $count } أصول
    [many] { $count } أصلًا
   *[other] { $count } أصل
}
local-panel-proxies = { $count ->
    [zero] لا صور
    [one] صورة واحدة، وملفات raw كنسخ وسيطة
    [two] صورتان، وملفات raw كنسخ وسيطة
    [few] { $count } صور، وملفات raw كنسخ وسيطة
    [many] { $count } صورةً، وملفات raw كنسخ وسيطة
   *[other] { $count } صورة، وملفات raw كنسخ وسيطة
}
local-panel-remove = إزالة…
local-panel-footer = محفوظة في { $folder }. لا تتغير الأصول أبدًا.
local-panel-keywords = غير متصل نسخ محلية نسخ وسيطة مساحة
