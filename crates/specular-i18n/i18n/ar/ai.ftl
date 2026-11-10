# Specular — Local AI pack (DESIGN.md "Local AI pack", §10). Arabic. Machine-drafted, needs native review.

## Settings → Local AI
settings-section-ai = الذكاء الاصطناعي المحلي
settings-ai-pack = حزمة الذكاء الاصطناعي المحلي
# machine-drafted, needs native review
settings-ai-pack-help = ميزات اختيارية تعمل على هذا الحاسوب فقط. لا يغادره شيء مما تعثر عليه. اختر نموذجًا لكل ميزة؛ لا يُنزَّل شيء حتى تختاره.
# machine-drafted, needs native review
settings-ai-install-from-file = تثبيت من ملف…
settings-ai-install-from-file-help = ثبّت ملفات النماذج التي نسختها إلى هذا الحاسوب، للأجهزة التي لا تتصل بالإنترنت. يُتحقق من كل ملف مقابل القائمة المرفقة مع { -app-name }.
settings-ai-install-from-folder = تثبيت من مجلد…
settings-ai-install-from-folder-help = الأمر نفسه، لكل ملف نموذج في مجلد.
settings-ai-search = البحث بالوصف
settings-ai-search-help = اعثر على الصور بوصفها، مثل «كلب على الشاطئ»، وجمّع اللقطات المتشابهة على نحو أفضل.
settings-ai-text = النص في الصور
settings-ai-text-help = يقرأ النص في الصور والمستندات الممسوحة ضوئيًا ليعثر عليها البحث بـ text:.
settings-ai-culling = المساعدة في الفرز
# machine-drafted, needs native review
settings-ai-culling-help = يضع علامة على العيون المغمضة والضبابية وسوء التعريض للمساعدة في اختيار أفضل لقطة من تتابع.
settings-ai-people = الأشخاص
# machine-drafted, needs native review
settings-ai-people-help = يعثر على الوجوه ويجمّعها حسب الشخص، للبحث بـ people:. يبقى التجميع متوقفًا حتى تشغّله لمكتبة. يمكن حذف بيانات الوجوه لشخص واحد أو للجميع دفعة واحدة.

# The pack's and a feature's state. $percent is formatted ("25%"); $done and $total are
# formatted numbers.
ai-state = { $state ->
        [none] لا تتوفر نماذج في هذا الإصدار.
        [installed] مثبّتة
        [indexing] مثبّتة · فُهرست { $done } من { $total } صورة
        [downloading] جارٍ التنزيل… { $percent }
        [failed] فشل التنزيل
       *[available] غير مثبّتة
    }
ai-removing = جارٍ الإزالة…
# machine-drafted, needs native review
# Buttons of the Local AI page.
ai-button = { $action ->
        [download] تنزيل
        [cancel] إلغاء
        [remove] إزالة
        [forget] نسيان
        [faces] حذف كل بيانات الوجوه
       *[pack] إزالة الحزمة
    }
# The question before deleting.
ai-confirm = { $action ->
        [faces] هل تريد حذف كل بيانات الوجوه؟ ستُحذف كل الوجوه والأشخاص الذين عُثر عليهم. تبقى صورك كما هي.
       *[pack] هل تريد إزالة حزمة الذكاء الاصطناعي المحلي ونماذجها وكل ما عثرت عليه؟ لن تتغير صورك.
    }

## Gallery search
gallery-query-needs-ai = { $field }: يتطلب حزمة الذكاء الاصطناعي المحلي (الإعدادات › الذكاء الاصطناعي المحلي)
# machine-drafted, needs native review
gallery-query-ai-not-yet = { $field }: حزمة الذكاء الاصطناعي المحلي لا تستطيع البحث في هذا بعد

## Faces: People view, face boxes and culling flags
# machine-drafted, needs native review
# "Show faces" on the picture.
faces-shown = الوجوه ظاهرة
# machine-drafted, needs native review
faces-hidden = الوجوه مخفية
# machine-drafted, needs native review
# Beside a face box whose eyes look closed.
faces-eyes-closed = العيون مغمضة
# machine-drafted, needs native review
# Culling flags on the cull mode strip.
cull-flag-eyes-closed = العيون مغمضة
# machine-drafted, needs native review
cull-flag-eyes-closed-some = العيون مغمضة: { $closed } من { $faces } من الوجوه
# machine-drafted, needs native review
cull-flag-blurred = ضبابية
# machine-drafted, needs native review
cull-flag-dark = معتمة جدًا
# machine-drafted, needs native review
cull-flag-bright = ساطعة جدًا
# machine-drafted, needs native review
cull-flag-clipped = إضاءات أو ظلال مقصوصة
# machine-drafted, needs native review
# The People view.
people-title = الأشخاص
# machine-drafted, needs native review
people-open = الأشخاص
# machine-drafted, needs native review
people-done = تم
# machine-drafted, needs native review
people-back = رجوع
# machine-drafted, needs native review
people-show-photos = عرض الصور
# machine-drafted, needs native review
people-name = تسمية…
# machine-drafted, needs native review
people-name-prompt = تسمية هذا الشخص
# machine-drafted, needs native review
people-faces-open = الوجوه
# machine-drafted, needs native review
people-faces-of = وجوه { $name }
# machine-drafted, needs native review
people-merge = دمج
# machine-drafted, needs native review
people-merged = الأشخاص المدمجون: { $count }
# machine-drafted, needs native review
people-merge-ask = هل تريد دمج { $from } في { $into }؟
# machine-drafted, needs native review
people-merge-ask-many = هل تريد دمج الأشخاص ({ $count }) في { $into }؟
# machine-drafted, needs native review
people-merge-ask-detail = تصبح وجوههم وجوه { $into }. لا تتغير الصور على القرص.
# machine-drafted, needs native review
people-not-this = ليس هذا الشخص
# machine-drafted, needs native review
people-face-removed = أُزيل الوجه من هذا الشخص. تبقى الصورة كما هي.
# machine-drafted, needs native review
# Someone not named yet.
people-unnamed = شخص #{ $id }
# machine-drafted, needs native review
people-face-count = { $count ->
        [zero] لا وجوه
        [one] وجه واحد
        [two] وجهان
        [few] { $count } وجوه
        [many] { $count } وجهًا
       *[other] { $count } وجه
    }
# machine-drafted, needs native review
people-count = { $count ->
        [zero] لا أشخاص
        [one] شخص واحد
        [two] شخصان
        [few] { $count } أشخاص
        [many] { $count } شخصًا
       *[other] { $count } شخص
    }
# machine-drafted, needs native review
people-face-a11y = الوجه { $index }
# machine-drafted, needs native review
people-loading = جارٍ التحميل…
# machine-drafted, needs native review
people-not-installed = ميزة الأشخاص غير مثبتة. نزّلها من الإعدادات › الذكاء الاصطناعي المحلي.
# machine-drafted, needs native review
people-grouping-off = تجميع الوجوه متوقف لهذه المكتبة. شغّل «تجميع الوجوه في أشخاص» من الإعدادات › الذكاء الاصطناعي المحلي.
# machine-drafted, needs native review
people-none = لم يُعثر على أشخاص بعد.

# machine-drafted, needs native review
# A feature's model: Default (the model Specular ships as the default, or the nearest one
# this computer can run) or a model by name, which stays that model on any computer. The
# page shows it only for a feature with more than one model; the dropdown labels Default
# with the model it means (ai-size-recommended-option) and names each model as the model
# list does. The option names below are the palette's and the command line's: model names
# are proper names, kept as they are.
settings-ai-search-size = النموذج
settings-ai-search-size-help = يعثر النموذج SigLIP 2 B/16 على الصور بأفضل دقة، وبلغات كثيرة. ويفهرس النموذج B/32 أسرع بنحو أربع مرات، لبطاقات الرسومات الأبطأ.
settings-ai-search-size-recommended = الافتراضي
# machine-drafted, needs native review
settings-ai-search-size-small = SigLIP 2 B/32
settings-ai-search-size-standard = PE Core S16
settings-ai-search-size-large = SigLIP 2 B/16
settings-ai-text-size = النموذج
settings-ai-text-size-help = النموذج PP-OCRv6 small أدق في القراءة، والنموذج tiny أسرع.
settings-ai-text-size-recommended = الافتراضي
settings-ai-text-size-small = PP-OCRv6 tiny
settings-ai-text-size-standard = PP-OCRv6 small
settings-ai-text-size-large = كبير
# machine-drafted, needs native review
settings-ai-culling-size = النموذج
settings-ai-culling-size-help = النموذج الذي يكتشف العيون المغمضة.
settings-ai-culling-size-recommended = الافتراضي
settings-ai-culling-size-small = صغير
# machine-drafted, needs native review
settings-ai-culling-size-standard = Face Landmarker
settings-ai-culling-size-large = كبير
# machine-drafted, needs native review
settings-ai-people-size = النموذج
settings-ai-people-size-help = النموذج SFace صغير. والنموذج AuraFace أكبر، ودُرّب على بيانات يقول ناشره إنها مرخّصة للاستخدام التجاري.
settings-ai-people-size-recommended = الافتراضي
settings-ai-people-size-small = صغير
# machine-drafted, needs native review
settings-ai-people-size-standard = BlazeFace + SFace
settings-ai-people-size-large = BlazeFace + AuraFace
# A model size by name.
ai-tier = { $tier ->
        [small] صغير
        [large] كبير
       *[standard] قياسي
    }
# machine-drafted, needs native review
# Default in the model dropdown, and the label of the "Back to default" row: with the model
# it means on this computer, "Default (SigLIP 2 B/16)". $size is the model's name.
ai-size-recommended-option = الافتراضي ({ $size })
# Under the model setting: the default, or the nearest model this computer can run. $gpu is
# the graphics card's name.
ai-recommended-for = الافتراضي لبطاقة { $gpu }: { $size }
ai-recommended = الافتراضي لهذا الحاسوب: { $size }
# Nothing fits: no size can run here.
ai-recommended-none = لا يوجد حجم يتسع في ذاكرة هذا الحاسوب.
# Under an installed feature: what its model runs on now. $gpu is the graphics card's name
# (used with "gpu" only). The CPU variants say why the graphics card is not used.
ai-runs-on = { $engine ->
        [gpu] يعمل على: وحدة معالجة الرسومات ({ $gpu })
        [gpu-unnamed] يعمل على: وحدة معالجة الرسومات
        [no-gpu] يعمل على: CPU (لم يُعثر على وحدة معالجة رسومات)
        [differed] يعمل على: CPU — اختلفت نتيجة وحدة معالجة الرسومات عن نتيجة CPU
        [failed] يعمل على: CPU — فشل النموذج على وحدة معالجة الرسومات
        [setting] يعمل على: CPU (المعالجة مضبوطة على «CPU فقط»)
        [checking] يعمل على: CPU — جارٍ التحقق من وحدة معالجة الرسومات مقابله
       *[cpu] يعمل على: CPU
    }
# A size's row: its files on disk ($place "disk"), or what it would download, the memory it
# needs, and " · needs the GPU" only for a size that cannot run without one. $bytes and
# $memory are formatted sizes.
ai-size-detail = { $place ->
        [disk] { $bytes } على القرص
       *[download] تنزيل { $bytes }
    } · ذاكرة { $memory }{ $gpu ->
        [yes] { " · يتطلب وحدة معالجة الرسومات" }
       *[no] { "" }
    }
# machine-drafted, needs native review
# Under a size that may not run on this computer (it is still shown, and choosing it asks
# first). "gpu": it needs a graphics card this computer does not have or cannot use;
# "memory": $need is the memory it needs, $have what this computer has for it (formatted
# sizes, "6 GB").
ai-size-unfit = { $why ->
        [gpu] يتطلب وحدة معالجة رسومات، ولا يوجد في هذا الحاسوب وحدة يمكنه استخدامها.
       *[memory] يتطلب { $need } من الذاكرة، أي أكثر من { $have } التي يسمح بها Specular للذكاء الاصطناعي على هذا الحاسوب.
    }
# Over the size setting while it names a size other than the one installed and in use
# ($current, $chosen: ai-tier). The rows below have the buttons to change it.
ai-size-not-in-use = الحجم { $current } قيد الاستخدام، وليس { $chosen }.
# machine-drafted, needs native review
# Buttons of a size's row and of the size question: "use" chooses that size (one not
# downloaded; a downloaded one says Switch, ai-size-button), "back" returns the setting to
# Recommended, "anyway" keeps a size that may not run on this computer (ai-unfit-question).
# The screen reader adds the size's name to each.
ai-size-action = { $action ->
        [back] العودة إلى الافتراضي
        [anyway] استخدام على أي حال
       *[use] استخدام هذا الحجم
    }
# The size the setting chose is not installed while another one is ($current, ai-tier).
ai-size-chosen-missing = مُختار، لكنه لم يُنزَّل بعد: يبقى { $current } قيد الاستخدام.
# What a size is now: from what is installed, never from the setting. "downloaded": its files
# are here but another size is in use; "missing": the chosen size, not downloaded.
ai-size-state = { $state ->
        [current] قيد الاستخدام
        [downloaded] مُنزَّل، غير مستخدم
        [missing] غير مُنزَّل
        [previous] قيد الاستخدام حتى يفهرس الحجم الجديد مكتبتك
        [target] جارٍ التنزيل…
        [failed] فشل التنزيل
       *[other] { "" }
    }
# A licence row's button (opens the licence's page).
ai-licence = عرض الترخيص
# Asked before a size change that downloads and re-indexes. $feature is the feature's name,
# $size the new size (ai-tier), $download a formatted size.
ai-switch-question = { $download ->
        [none] { $feature }: هل تريد التبديل إلى الحجم { $size }؟ ستُفهرس مكتبتك من جديد، ويبقى الحجم الحالي قيد الاستخدام حتى ذلك الحين.
       *[other] { $feature }: هل تريد التبديل إلى الحجم { $size }؟ سيُنزَّل { $download } وتُفهرس مكتبتك من جديد، ويبقى الحجم الحالي قيد الاستخدام حتى ذلك الحين.
    }
# machine-drafted, needs native review
# A size that may not run on this computer, chosen for a feature not installed yet: asked
# before the setting keeps it (ai-size-unfit says why, on the next line). $size is ai-tier,
# $feature the feature's name.
ai-unfit-question = { $feature }: هل تريد استخدام الحجم { $size } على أي حال؟ قد لا يعمل على هذا الحاسوب، أو قد يعمل ببطء شديد.
# The recommendation moved: offered, never applied silently.
ai-suggest-question = { $feature }: الحجم { $size } أنسب الآن لهذا الحاسوب. هل تريد التبديل؟ { $download ->
        [none] ستُفهرس مكتبتك من جديد.
       *[other] سيُنزَّل { $download } وتُفهرس مكتبتك من جديد.
    }
ai-suggest-toast = الذكاء الاصطناعي المحلي: حجم نموذج آخر أنسب لهذا الحاسوب (الإعدادات › الذكاء الاصطناعي المحلي).
# A feature switching size: the new size ($size, ai-tier) indexes the library while the old one
# stays in use. $done and $total are formatted numbers.
ai-switching = جارٍ التبديل إلى الحجم { $size } · فُهرست { $done } من { $total } صورة

## The model manager: what each model says besides its size and licence

# machine-drafted, needs native review
# A model or feature not installable yet.
ai-coming = سيتوفر في إصدار لاحق
# machine-drafted, needs native review
# What a model reads. $languages: multilingual, english, or a script id (OCR packs).
ai-choice-languages = { $languages ->
        [multilingual] لغات كثيرة
        [english] الإنجليزية فقط
        [cjk-latin] الصينية واليابانية والحروف اللاتينية
        [zh-latin] الصينية والحروف اللاتينية
        [arabic] العربية
        [hebrew] العبرية
        [korean] الكورية
        [cyrillic] السيريلية
        [greek] اليونانية
        [thai] التايلاندية
        [devanagari] الديفاناغارية
        [tamil] التاميلية
        [telugu] التيلوغوية
        [eslav] الروسية والأوكرانية والبيلاروسية
        [latin] الأبجدية اللاتينية (لغات أوروبية كثيرة)
       *[other] { $languages }
    }
# A hint about a model. $hint: faster, closed-eyes, small-faces.
ai-choice-hint = { $hint ->
        [faster] أسرع، لبطاقات الرسومات الأبطأ
        [closed-eyes] يكتشف العيون المغمضة
        [small-faces] أفضل مع الوجوه الصغيرة
        [no-translate] لا يترجم إلى الإنجليزية
       *[other] { "" }
    }
# A model's training data. $source: the data as its publisher names it ($named is "no" when
# it is not named). $terms: whether the publisher states the data's terms ("stated"), does
# not ("unstated"), or the data is licensed for research only ("research").
ai-choice-data = { $terms ->
        [stated] بيانات التدريب: { $source }
        [research] بيانات التدريب: { $source } (مرخّصة للبحث فقط)
       *[unstated] { $named ->
            [no] بيانات التدريب: غير مسمّاة، والشروط غير مذكورة
           *[yes] بيانات التدريب: { $source }، والشروط غير مذكورة
        }
    }
# Where a model downloads from. $host is a host name ("huggingface.co").
ai-choice-host = يُنزَّل من { $host }
# A model still coming, in a feature's row. $size is formatted, $licence an SPDX name.
ai-choice-coming = { $name }: { $size }, { $licence }
# People: grouping faces into people, per library.
ai-grouping = تجميع الوجوه في أشخاص
ai-grouping-help = متوقف حتى تشغّله لهذه المكتبة. إيقافه يوقف التجميع؛ وتبقى الوجوه المجمّعة حتى تحذفها.
ai-grouping-state = { $on ->
        [yes] مشغّل في هذه المكتبة
       *[no] متوقف
    }
ai-grouping-button = { $on ->
        [yes] إيقاف
       *[no] تشغيل
    }
# machine-drafted, needs native review
# An add-on (a script's text recogniser) before its feature is downloaded. $feature is the
# feature's name ("Text in photos").
ai-addon-first = نزّل { $feature } أولًا.
# machine-drafted, needs native review
# Asked before an add-on is removed. $name is the model's name ("PP-OCRv5 Arabic").
ai-addon-remove-title = هل تريد إزالة { $name }؟
# machine-drafted, needs native review
ai-addon-remove-detail = تُحذف ملفات النموذج. تبقى صورك والنص الذي عُثر عليه سابقًا.
# machine-drafted, needs native review
# Text: reading the text in this library's photos, per library.
ai-text-indexing = قراءة النص في صور هذه المكتبة
# machine-drafted, needs native review
ai-text-indexing-help = متوقف حتى تشغّله لهذه المكتبة. بعدها يعثر البحث بـ text: في المكتبة على الصور حسب الكلمات التي فيها.
# machine-drafted, needs native review
ai-text-indexing-state = { $on ->
        [yes] مشغّل في هذه المكتبة
       *[no] متوقف
    }

## التثبيت من ملف

# machine-drafted, needs native review
# After model files were copied in.
ai-sideload-ready = { $count ->
        [zero] لم يُنسخ أي ملف نموذج.
        [one] نُسخ ملف نموذج واحد. اضغط تنزيل لتثبيته.
        [two] نُسخ ملفا نموذج. اضغط تنزيل لتثبيتهما.
        [few] نُسخت { $count } ملفات نماذج. اضغط تنزيل لتثبيتها.
        [many] نُسخ { $count } ملف نموذج. اضغط تنزيل لتثبيتها.
       *[other] نُسخ { $count } ملف نموذج. اضغط تنزيل لتثبيتها.
    }
ai-sideload-none = لم يُعثر على ملفات نماذج.
# $file is a file name, or several separated by commas.
ai-sideload-unknown = { $file } ليس نموذجاً يعرفه { -app-name }. لم يُنسخ.
ai-sideload-failed = تعذّر نسخ { $file }: { $reason }
# Asked before using a file whose SHA-256 isn't in the shipped list. $model is the model it
# would be used as ("clip-vit-b32-1").
ai-unverified-title = هل تريد استخدام نموذج غير متحقق منه؟
ai-unverified-detail = { $file } ليس في قائمة النماذج المرفقة مع { -app-name }، لذا لا يمكن التحقق منه. سيُستخدم بوصفه { $model }.
ai-unverified-use = استخدام النموذج غير المتحقق منه

# machine-drafted, needs native review
ai-failed-internal = حدث خطأ ما داخل { -app-name }. التفاصيل في السجل.
# The buttons under a model-size question: switch, or keep the size in use.
ai-size-button = { $action ->
        [switch] تبديل
       *[keep] الإبقاء على الحجم الحالي
    }
