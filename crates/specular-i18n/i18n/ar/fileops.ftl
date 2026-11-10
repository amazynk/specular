# machine-drafted, needs native review
# عمليات الملفات: إعادة التسمية، وإعادة تسمية عدة ملفات، والنسخ والنقل إلى مجلد، والتدوير
# المحفوظ في الملف، والتراجع والإعادة، والتعيين كخلفية، والمشاركة، والإرسال بالبريد،
# وتصدير الإعدادات واستيرادها.

## لوحة الأوامر وورقة الاختصارات والقوائم

palette-undo = تراجع
palette-undo-help = التراجع عن آخر تغيير في الملفات: النقل إلى سلة المهملات، أو إعادة التسمية، أو النسخ أو النقل، أو التدوير.
palette-redo = إعادة
palette-redo-help = إعادة ما ألغاه آخر تراجع.
palette-rename = إعادة التسمية…
palette-rename-help = إعطاء الملف اسمًا جديدًا؛ وتتبعه ملفاته المرافقة وشريكه في زوج RAW+JPEG.
palette-batch-rename = إعادة تسمية الملفات…
palette-batch-rename-help = إعادة تسمية الملفات المحددة، أو المجلد كله، وفق نمط مع معاينة.
palette-copy-to-folder = نسخ إلى مجلد…
palette-copy-to-folder-help = نسخ الملف أو العناصر المحددة إلى مجلد آخر.
palette-move-to-folder = نقل إلى مجلد…
palette-move-to-folder-help = نقل الملف أو العناصر المحددة إلى مجلد آخر.
palette-rotate-file-left = تدوير الملف لليسار
palette-rotate-file-left-help = تدوير الملف نفسه ربع دورة لليسار، دون فقد حيث يسمح التنسيق.
palette-rotate-file-right = تدوير الملف لليمين
palette-rotate-file-right-help = تدوير الملف نفسه ربع دورة لليمين، دون فقد حيث يسمح التنسيق.
palette-set-wallpaper-help = جعل الصورة خلفية سطح المكتب.
palette-share = مشاركة…
palette-share-help = إرسال الملف إلى تطبيق آخر عبر قائمة المشاركة في النظام.
palette-send-by-email = إرسال بالبريد الإلكتروني…
palette-send-by-email-help = بدء رسالة جديدة في تطبيق البريد مع إرفاق الملف.
# machine-drafted, needs native review
palette-share-with-zynk = مشاركة مع Zynk…
palette-share-with-zynk-help = إنشاء رابط ويب للملف في تطبيق Zynk.
palette-send-with-zynk = إرسال مع Zynk…
palette-send-with-zynk-help = إرسال الملف إلى جهة اتصال أو إلى أحد أجهزتك عبر Zynk.
palette-export-settings = تصدير الإعدادات…
palette-export-settings-help = حفظ كل الإعدادات واختصارات المفاتيح في ملف.
palette-import-settings = استيراد الإعدادات…
palette-import-settings-help = قراءة الإعدادات من ملف بعد عرض ما سيتغير.

disabled-in-archive-file = هذه الصفحة داخل أرشيف وليس لها ملف خاص بها
disabled-not-rotatable = هذا النوع من الملفات لا يمكنه تسجيل التدوير
disabled-not-on-this-desktop = سطح المكتب لديك لا يوفر هذا

## الإعدادات

settings-browsing-rotate-trim-edges = قص حواف JPEG للتدوير دون فقد
settings-browsing-rotate-trim-edges-help = لا يمكن تدوير ملف JPEG لا تكون أبعاده من مضاعفات 8 أو 16 بكسل دون فقد إلا بقص بضع بكسلات من طرفه. وبدون هذا الخيار يُغيَّر وسم الاتجاه بدلًا من ذلك، ويُعرض عليك القص في كل مرة.
settings-browsing-rotate-keep-modified = الإبقاء على تاريخ التعديل عند التدوير
settings-browsing-rotate-keep-modified-help = يحتفظ الملف المدوَّر بتاريخ آخر تعديل له، فيبقى في مكانه عند الفرز حسب التاريخ.
settings-browsing-recent-folders = المجلدات الأخيرة
settings-browsing-recent-folders-help = المجلدات المستخدمة مؤخرًا للنسخ والنقل.

## النوافذ

fileop-cancel = إلغاء
rename-title = إعادة التسمية
rename-field-label = الاسم
rename-button = إعادة التسمية
rename-sidecars-follow = أُعيدت تسميته أيضًا: { $files }
rename-extension-changed = تغيير الامتداد لا يحوّل الملف؛ وقد لا تفتحه تطبيقات أخرى.
rename-problem-empty = اكتب اسمًا.
rename-problem-invalid-char = لا يمكن أن تحتوي الأسماء على «{ $char }».
rename-problem-reserved = «{ $name }» اسم يحجزه Windows للأجهزة.
rename-problem-too-long = الاسم طويل جدًا؛ الحد الأقصى 255 بايت.
rename-problem-dot = لا يمكن استخدام «.» و«..» كأسماء.
rename-problem-trailing = لا يمكن أن ينتهي الاسم بمسافة أو نقطة.
rename-problem-exists = يوجد ملف آخر بهذا الاسم.
rename-problem-read-only = لا يمكن تغيير هذا المجلد.
rename-problem-duplicate = سيحصل ملفان على هذا الاسم.

batch-rename-title =
    { $count ->
        [zero] إعادة تسمية { $count } ملف
        [one] إعادة تسمية ملف واحد
        [two] إعادة تسمية ملفين
        [few] إعادة تسمية { $count } ملفات
        [many] إعادة تسمية { $count } ملفًا
       *[other] إعادة تسمية { $count } ملف
    }
batch-rename-field-label = النمط
batch-rename-tokens = استخدم { "{" }name{ "}" } و{ "{" }n:3{ "}" } (001، 002، …) و{ "{" }date:%Y-%m-%d{ "}" } و{ "{" }camera{ "}" } و{ "{" }w{ "}" }×{ "{" }h{ "}" } و{ "{" }ext{ "}" }.
batch-rename-summary =
    { $count ->
        [zero] لن يُعاد تسمية أي ملف
        [one] ستُعاد تسمية ملف واحد
        [two] ستُعاد تسمية ملفين
        [few] ستُعاد تسمية { $count } ملفات
        [many] ستُعاد تسمية { $count } ملفًا
       *[other] ستُعاد تسمية { $count } ملف
    }{ $problems ->
        [0] .
        [zero] .
        [one] ؛ وملف واحد لا يمكن تسميته فيبقى كما هو.
        [two] ؛ وملفان لا يمكن تسميتهما فيبقيان كما هما.
        [few] ؛ و{ $problems } ملفات لا يمكن تسميتها فتبقى كما هي.
        [many] ؛ و{ $problems } ملفًا لا يمكن تسميتها فتبقى كما هي.
       *[other] ؛ و{ $problems } ملف لا يمكن تسميتها فتبقى كما هي.
    }
batch-rename-reading = جارٍ قراءة التواريخ والكاميرات…
batch-rename-preview = معاينة
batch-rename-unchanged = يبقى اسمه
batch-rename-more =
    { $count ->
        [zero] لا مزيد أدناه
        [one] واحد آخر أدناه
        [two] اثنان آخران أدناه
        [few] { $count } أخرى أدناه
        [many] { $count } أخرى أدناه
       *[other] { $count } أخرى أدناه
    }

transfer-copy-title-one = نسخ «{ $file }» إلى مجلد
transfer-copy-title =
    { $count ->
        [zero] نسخ { $count } ملف إلى مجلد
        [one] نسخ ملف واحد إلى مجلد
        [two] نسخ ملفين إلى مجلد
        [few] نسخ { $count } ملفات إلى مجلد
        [many] نسخ { $count } ملفًا إلى مجلد
       *[other] نسخ { $count } ملف إلى مجلد
    }
transfer-move-title-one = نقل «{ $file }» إلى مجلد
transfer-move-title =
    { $count ->
        [zero] نقل { $count } ملف إلى مجلد
        [one] نقل ملف واحد إلى مجلد
        [two] نقل ملفين إلى مجلد
        [few] نقل { $count } ملفات إلى مجلد
        [many] نقل { $count } ملفًا إلى مجلد
       *[other] نقل { $count } ملف إلى مجلد
    }
transfer-recent = المجلدات الأخيرة
transfer-folder = المجلد
transfer-choose-folder = اختيار مجلد…
transfer-copy-dialog = نسخ إلى مجلد
transfer-move-dialog = نقل إلى مجلد
transfer-sidecars =
    { $count ->
        [zero] لا ملفات مرافقة.
        [one] ينتقل معه ملف مرافق واحد.
        [two] ينتقل معه ملفان مرافقان.
        [few] تنتقل معها { $count } ملفات مرافقة.
        [many] ينتقل معها { $count } ملفًا مرافقًا.
       *[other] ينتقل معها { $count } ملف مرافق.
    }
transfer-taken =
    { $count ->
        [zero] لا أسماء مستخدمة في { $folder }.
        [one] اسم واحد مستخدم من قبل في { $folder }.
        [two] اسمان مستخدمان من قبل في { $folder }.
        [few] { $count } أسماء مستخدمة من قبل في { $folder }.
        [many] { $count } اسمًا مستخدمًا من قبل في { $folder }.
       *[other] { $count } اسم مستخدم من قبل في { $folder }.
    }
transfer-when-taken = عندما يكون الاسم مستخدمًا
transfer-skip = تخطٍّ
transfer-keep-both = الاحتفاظ بالاثنين
transfer-replace-option = استبدال
transfer-copy = نسخ
transfer-move = نقل
transfer-replace-title =
    { $count ->
        [zero] استبدال { $count } ملف في { $folder }؟
        [one] استبدال ملف واحد في { $folder }؟
        [two] استبدال ملفين في { $folder }؟
        [few] استبدال { $count } ملفات في { $folder }؟
        [many] استبدال { $count } ملفًا في { $folder }؟
       *[other] استبدال { $count } ملف في { $folder }؟
    }
transfer-replace-note = تذهب الملفات الموجودة هناك إلى سلة المهملات. والتراجع يعيدها.
transfer-replace = استبدال
transfer-back = رجوع
transfer-stop = إيقاف

settings-file-import-title = استيراد الإعدادات
settings-file-import-dialog = استيراد الإعدادات
settings-file-export-dialog = تصدير الإعدادات
settings-file-import-changes = التغييرات
settings-file-import-change = { $before } ← { $after }
settings-file-import-no-key = بلا مفتاح
settings-file-import-unknown = ليس إعدادًا في هذا الإصدار؛ تُرك.
settings-file-import-keymap = اختصارات المفاتيح
settings-file-import-nothing = لن يتغير شيء: هذه الإعدادات سارية بالفعل.
settings-file-import-summary =
    { $count ->
        [zero] لا تغييرات. يمكن التراجع عن الاستيراد في الإعدادات.
        [one] تغيير واحد. يمكن التراجع عن الاستيراد في الإعدادات.
        [two] تغييران. يمكن التراجع عن الاستيراد في الإعدادات.
        [few] { $count } تغييرات. يمكن التراجع عن الاستيراد في الإعدادات.
        [many] { $count } تغييرًا. يمكن التراجع عن الاستيراد في الإعدادات.
       *[other] { $count } تغيير. يمكن التراجع عن الاستيراد في الإعدادات.
    }
settings-file-import-secrets =
    { $count ->
        [zero] لا إدخالات تشبه كلمة مرور أو مفتاحًا.
        [one] إدخال واحد يشبه كلمة مرور أو مفتاحًا ولا يُستورد أبدًا.
        [two] إدخالان يشبهان كلمات مرور أو مفاتيح ولا يُستوردان أبدًا.
        [few] { $count } إدخالات تشبه كلمات مرور أو مفاتيح ولا تُستورد أبدًا.
        [many] { $count } إدخالًا تشبه كلمات مرور أو مفاتيح ولا تُستورد أبدًا.
       *[other] { $count } إدخال تشبه كلمات مرور أو مفاتيح ولا تُستورد أبدًا.
    }
settings-file-import-commands =
    { $count ->
        [zero] لا أوامر مستخدم مستبعدة.
        [one] لم يُستورد أمر مستخدم واحد: الأوامر تشغّل برامج. انسخه بنفسك إلى ملف الإعدادات إن كنت تثق به.
        [two] لم يُستورد أمرا مستخدم: الأوامر تشغّل برامج. انسخهما بنفسك إلى ملف الإعدادات إن كنت تثق بهما.
        [few] لم تُستورد { $count } أوامر مستخدم: الأوامر تشغّل برامج. انسخها بنفسك إلى ملف الإعدادات إن كنت تثق بها.
        [many] لم يُستورد { $count } أمرًا للمستخدم: الأوامر تشغّل برامج. انسخها بنفسك إلى ملف الإعدادات إن كنت تثق بها.
       *[other] لم يُستورد { $count } أمر مستخدم: الأوامر تشغّل برامج. انسخها بنفسك إلى ملف الإعدادات إن كنت تثق بها.
    }
settings-file-import-button = استيراد

## النتائج

fileop-renamed = أُعيدت التسمية إلى { $file }
fileop-renamed-count =
    { $count ->
        [zero] لم يُعَد تسمية أي ملف
        [one] أُعيدت تسمية ملف واحد
        [two] أُعيدت تسمية ملفين
        [few] أُعيدت تسمية { $count } ملفات
        [many] أُعيدت تسمية { $count } ملفًا
       *[other] أُعيدت تسمية { $count } ملف
    }
fileop-copied =
    { $count ->
        [zero] لم يُنسخ أي ملف إلى { $folder }
        [one] نُسخ ملف واحد إلى { $folder }
        [two] نُسخ ملفان إلى { $folder }
        [few] نُسخت { $count } ملفات إلى { $folder }
        [many] نُسخ { $count } ملفًا إلى { $folder }
       *[other] نُسخ { $count } ملف إلى { $folder }
    }
fileop-moved =
    { $count ->
        [zero] لم يُنقل أي ملف إلى { $folder }
        [one] نُقل ملف واحد إلى { $folder }
        [two] نُقل ملفان إلى { $folder }
        [few] نُقلت { $count } ملفات إلى { $folder }
        [many] نُقل { $count } ملفًا إلى { $folder }
       *[other] نُقل { $count } ملف إلى { $folder }
    }
fileop-skipped =
    { $count ->
        [zero] لم يُتخطَّ أي ملف.
        [one] تُخطي ملف واحد: اسمه مستخدم هناك.
        [two] تُخطي ملفان: اسماهما مستخدمان هناك.
        [few] تُخطيت { $count } ملفات: أسماؤها مستخدمة هناك.
        [many] تُخطي { $count } ملفًا: أسماؤها مستخدمة هناك.
       *[other] تُخطي { $count } ملف: أسماؤها مستخدمة هناك.
    }
fileop-stopped = توقف؛ ما أُنجز يبقى منجزًا.
fileop-failed-file =
    { $count ->
        [zero] تعذر نسخ { $file } أو نقله: { $reason }
        [one] تعذر نسخ { $file } أو نقله: { $reason }
        [two] تعذر نسخ ملفين أو نقلهما؛ أولهما { $file }: { $reason }
        [few] تعذر نسخ { $count } ملفات أو نقلها؛ أولها { $file }: { $reason }
        [many] تعذر نسخ { $count } ملفًا أو نقلها؛ أولها { $file }: { $reason }
       *[other] تعذر نسخ { $count } ملف أو نقلها؛ أولها { $file }: { $reason }
    }
fileop-progress-copy =
    { $count ->
        [zero] جارٍ النسخ… { PERCENT($percent) }
        [one] جارٍ نسخ ملف واحد… { PERCENT($percent) }
        [two] جارٍ نسخ ملفين… { PERCENT($percent) }
        [few] جارٍ نسخ { $count } ملفات… { PERCENT($percent) }
        [many] جارٍ نسخ { $count } ملفًا… { PERCENT($percent) }
       *[other] جارٍ نسخ { $count } ملف… { PERCENT($percent) }
    }
fileop-progress-move =
    { $count ->
        [zero] جارٍ النقل… { PERCENT($percent) }
        [one] جارٍ نقل ملف واحد… { PERCENT($percent) }
        [two] جارٍ نقل ملفين… { PERCENT($percent) }
        [few] جارٍ نقل { $count } ملفات… { PERCENT($percent) }
        [many] جارٍ نقل { $count } ملفًا… { PERCENT($percent) }
       *[other] جارٍ نقل { $count } ملف… { PERCENT($percent) }
    }
fileop-done-status = تم
fileop-busy = ما زال آخر تغيير في الملفات قيد التنفيذ؛ حاول بعد لحظة.
fileop-nothing-to-redo = لا شيء لإعادته
fileop-undone = تم التراجع: { $what }
fileop-redone = أُعيد: { $what }
fileop-kind-rename = إعادة التسمية
fileop-kind-batch-rename = إعادة تسمية عدة ملفات
fileop-kind-move = النقل إلى مجلد
fileop-kind-copy = النسخ إلى مجلد
fileop-kind-rotate = التدوير
rename-failed = تعذرت إعادة التسمية: { $reason }

undo-problem-changed = لا يمكن التراجع: تغيّر { $file } منذ ذلك الحين، والتراجع سيُضيع هذا التغيير.
undo-problem-missing = لا يمكن التراجع: { $file } لم يعد موجودًا.
undo-problem-taken = لا يمكن التراجع: ملف آخر يحمل الآن الاسم { $file }.
undo-problem-io = لا يمكن التراجع عن تغيير { $file }: { $reason }

rotate-done-lossless = دُوِّر { $file } دون أي فقد.
rotate-done-lossless-trimmed = دُوِّر { $file } دون أي فقد؛ وقُصت حافته إلى { $width } × { $height }.
rotate-done-tag = دُوِّر { $file } بتغيير وسم الاتجاه.
rotate-done-sidecar = حُفظ تدوير { $file } في { $sidecar }؛ ملف RAW نفسه لا يُغيَّر أبدًا.
rotate-tag-unreadable = تعذرت قراءة بيانات الصورة بالقدر الكافي لتدويرها.
rotate-tag-multi-picture = يحتوي على أكثر من صورة، وتدوير الأولى سيُفسد الأخرى.
rotate-tag-trailer = تلي الصورة بيانات سيزيحها التدوير.
rotate-tag-not-aligned = تدوير البكسلات دون فقد سيقصها إلى { $width } × { $height }.
rotate-trim-offer = قص وتدوير
rotate-problem-unsupported = { $file } من نوع ملفات لا يمكنه تسجيل التدوير.
rotate-problem-too-large = { $file } أكبر من أن يُدوَّر.
rotate-problem-damaged = { $file } تالف في موضع حفظ الاتجاه: { $reason }
rotate-problem-io = تعذر تدوير { $file }: { $reason }

share-failed = تعذرت المشاركة: { $reason }
email-failed = تعذر تشغيل تطبيق البريد: { $reason }
# machine-drafted, needs native review
zynk-opening = جارٍ فتح Zynk…
zynk-failed = تعذر الوصول إلى Zynk: { $reason }
zynk-get-title-send = «إرسال مع Zynk» يحتاج إلى تطبيق Zynk
zynk-get-title-share = «مشاركة مع Zynk» يحتاج إلى تطبيق Zynk
zynk-get-about = إن Zynk تطبيق مجاني لأجهزة Mac وWindows وLinux وiPhone وAndroid. يرسل الصور ومقاطع الفيديو والمجلدات الكاملة مباشرةً إلى أجهزتك الأخرى وإلى أشخاص آخرين، مهما كان حجمها. عمليات النقل مشفّرة من طرف إلى طرف، وتُستأنف من حيث توقفت إذا انقطع الاتصال. وللمستلم الذي لا يملك Zynk، ينشئ رابط ويب يمكنه تنزيل الملفات منه في المتصفح.
zynk-get-macos = احصل عليه من Mac App Store. بعد تثبيته، يسلّم { -app-name } ملفاتك إليه مباشرةً.
zynk-get-windows = احصل عليه من Microsoft Store. بعد تثبيته، يسلّم { -app-name } ملفاتك إليه مباشرةً.
zynk-get-linux = يأتي Zynk لنظام Linux كملف AppImage لا يحتاج إلى تثبيت: نزّله، وضعه في ⁦~/Applications⁩، واجعله قابلاً للتنفيذ (من خصائصه، أو بالأمر ⁦chmod +x⁩). يجده { -app-name } هناك، وفي أي مكان آخر ما دام Zynk يعمل.
zynk-get = احصل على Zynk
zynk-send-old-windows = «إرسال مع Zynk» يحتاج إلى Windows 10 الإصدار 2004 أو أحدث. «مشاركة مع Zynk» يعمل على هذا الإصدار من Windows.
settings-file-export-done = حُفظت الإعدادات في { $file }
settings-file-export-failed = تعذر حفظ الإعدادات: { $reason }
settings-file-import-unreadable = تعذرت قراءة { $file }: { $reason }
settings-file-import-done =
    { $count ->
        [zero] لم يُستورد أي تغيير
        [one] استُورد تغيير واحد
        [two] استُورد تغييران
        [few] استُوردت { $count } تغييرات
        [many] استُورد { $count } تغييرًا
       *[other] استُورد { $count } تغيير
    }
