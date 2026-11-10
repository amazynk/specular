# machine-drafted, needs native review
# Specular — تقرير الفتح: لماذا استغرق فتح الصورة هذا الوقت (لوحة المعلومات، قسم «الأداء»؛
# DESIGN.md §12.7).

open-report-title = الأداء

## Headlines
open-report-fast = فُتحت في { $ms } ms
open-report-display-cache = عُرضت من ذاكرة العرض المؤقتة
open-report-slow-read = تخزين بطيء: ذهب { $percent } من الوقت في قراءة الملف ({ $speed } MB/s)
# machine-drafted, needs native review
open-report-slow-read-no-speed = تخزين بطيء: ذهب { $percent } من الوقت في قراءة الملف (ملف صغير: معظم الوقت في انتظار استجابة وحدة التخزين)
open-report-archive-read = ذهب { $percent } من الوقت في فك ضغط الأرشيف
open-report-startup = ذهب { $percent } من الوقت في تشغيل { -app-name }
open-report-checked = فُكّ ترميزها مرتين: قورنت النتيجة الأولى لمفكك الترميز بالعتاد بنتيجة CPU ({ $percent } من الوقت)
open-report-hardware = فُكّ ترميزها بمفكك الترميز بالعتاد، واستغرق ذلك { $percent } من الوقت
open-report-decode = صورة كبيرة: استغرق فك ترميز { $mpx } ميغابكسل { $percent } من الوقت
open-report-upload = ذهب { $percent } من الوقت في إرسال الصورة إلى GPU
open-report-waiting = ذهب { $percent } من الوقت في انتظار انتهاء أعمال أخرى
open-report-stage = استغرقت { $stage } { $percent } من الوقت
open-report-memory-short = كانت الذاكرة شحيحة، فربما اضطر النظام إلى استخدام التبديل

## Why a decode ran on the processor although a hardware decoder can do it
open-report-sw-not-probed = فُكّ الترميز برمجيًا: كان مفكك الترميز بالعتاد لا يزال قيد الاكتشاف
open-report-sw-no-device = فُكّ الترميز برمجيًا: { $api } غير متاح على هذا الحاسوب
open-report-sw-no-codec = فُكّ الترميز برمجيًا: لا يملك { $api } على وحدة GPU هذه مفكك ترميز { $codec }
open-report-sw-blocked-mismatch = فُكّ الترميز برمجيًا: أُوقف مفكك الترميز بالعتاد لأن نتيجته لم تطابق نتيجة CPU
open-report-sw-blocked-error = فُكّ الترميز برمجيًا: أُوقف مفكك الترميز بالعتاد بعد خطأ
open-report-sw-blocked-lost = فُكّ الترميز برمجيًا: أُوقف مفكك الترميز بالعتاد لأنه توقف عن الاستجابة
open-report-sw-failed = فُكّ الترميز برمجيًا: فشل مفكك الترميز بالعتاد في هذه الصورة
open-report-sw-busy = فُكّ الترميز برمجيًا: كان مفكك الترميز بالعتاد مشغولًا
open-report-sw-unsupported = فُكّ الترميز برمجيًا: لا يستطيع مفكك الترميز بالعتاد التعامل مع هذه الصورة
open-report-sw-unavailable = فُكّ الترميز برمجيًا: فك الترميز بالعتاد متوقف حتى يُعاد تشغيل { -app-name }
open-report-sw-cpu-only = فُكّ الترميز برمجيًا: المعالجة مضبوطة على «CPU فقط»
open-report-sw-setting-off = فُكّ الترميز برمجيًا: فك الترميز بالعتاد متوقف في الإعدادات
open-report-sw-unchecked = فُكّ الترميز برمجيًا: لم يُتحقق من مفكك الترميز بالعتاد بعد (يُجرَّب أولًا على صورة تُجهَّز في الخلفية)
open-report-sw-lower-power = فُكّ الترميز برمجيًا: عند العمل على البطارية تستهلك CPU طاقة أقل لهذا
open-report-sw-faster = فُكّ الترميز برمجيًا: أسرع على هذا الحاسوب (نحو { $cpu } ms مقابل { $hw } ms بمفكك الترميز بالعتاد)
open-report-sw-scaled = فُكّ الترميز برمجيًا: لا يفك مفكك الترميز بالعتاد ترميز الصور إلا بحجمها الكامل

## Hints
open-report-hint-copy-local = انسخ الملف إلى قرص محلي ليُفتح أسرع.
open-report-hint-extract = استخرج الأرشيف إلى مجلد لتُفتح صفحاته أسرع.
open-report-hint-try-again = لتجربة مفكك الترميز بالعتاد مجددًا: الإعدادات ← الأداء ← إعادة المحاولة.
open-report-hint-automatic = اضبط «أين تُعالَج الصور» على تلقائي في الإعدادات ← الأداء.
open-report-hint-turn-on = اضبط «فك الترميز بالعتاد» على تلقائي في الإعدادات ← الأداء.
open-report-hint-free-memory = أغلق برامج أخرى لتحرير الذاكرة.
open-report-hint-once-only = تُفحص الصورة الأولى فقط بهذه الطريقة.

## Rows
open-report-total = حتى الوضوح
open-report-first-pixels = أول البكسلات
# machine-drafted, needs native review
open-report-whole = ظهرت الصورة كاملة
open-report-stage-startup = بدء التشغيل
open-report-stage-read = قراءة الملف
open-report-stage-parse = قراءة الترويسة
open-report-stage-colour = تحضير إدارة الألوان
open-report-stage-decode = فك الترميز
open-report-stage-upload = الإرسال إلى GPU
open-report-stage-waiting = انتظار أعمال أخرى
open-report-decoder = فُكّ الترميز بواسطة
open-report-decoder-checked = { $engine }، مع التحقق مقابل CPU
open-report-decoder-cache = ذاكرة العرض المؤقتة
# $scale is the reduction: 4 for a level a quarter of the full width and height.
open-report-decoder-scaled = { $engine }، بحجم 1/{ $scale }
open-report-decoder-scaled-full = { $engine }، بالحجم الكامل ثم صُغّرت إلى 1/{ $scale }
open-report-decoder-scaled-no-index = { $engine }، بحجم 1/{ $scale }، دون فهرس البدء السريع
open-report-colour-conversion = حُوّلت الألوان بواسطة

## Advisor: a pattern across recent opens, what would help and by how much (rows)
# $count and $total are counts of opens, $format a file format ("HEIC"), $speed a formatted
# number (MB/s), $low and $high formatted numbers ("2.5").
open-report-advice-recurring = متكرر
open-report-advice-change = ما قد يساعد
open-report-advice-gain = التحسّن المقدّر
open-report-advice-software = { $count } من آخر { $total } من صور { $format } البطيئة فُكّ ترميزها برمجيًا لهذا السبب
open-report-advice-slow-storage = { $count } من آخر { $total } من الصور البطيئة تأخرت بسبب قراءة الملف (نحو { $speed } MB/s)
# machine-drafted, needs native review
open-report-advice-slow-storage-no-speed = { $count } من آخر { $total } من الصور البطيئة تأخرت بسبب قراءة الملف (ملفات صغيرة: معظم الوقت في انتظار استجابة وحدة التخزين)
open-report-advice-archive = { $count } من آخر { $total } من الصور البطيئة تأخرت بسبب فك ضغط الأرشيف
open-report-advice-hardware-decoder = وحدة GPU فيها مفكك ترميز { $codec } بالعتاد
open-report-advice-gain-range = أسرع بنحو { $low } إلى { $high } مرة
open-report-advice-gain-one = أسرع بنحو { $low } مرة

## Performance insights: the indicator in the corner and the panel it opens
# machine-drafted, needs native review
# $total is a formatted duration ("240 ms"), $format a file format ("HEIC").
open-report-insights-title = تلميحات الأداء
open-report-insights-indicator = لماذا هذا بطيء؟
open-report-insights-announce = تلميح أداء: الصور تُفتح ببطء مرارًا للسبب نفسه. افتح «لماذا هذا بطيء؟» لترى ما قد يساعد.
open-report-insights-hide = إخفاء
open-report-insights-last = آخر صورة بطيئة: { $total } حتى الوضوح
open-report-insights-now = ما يمكنك فعله الآن
open-report-insights-hardware = عتاد قد يساعد
open-report-insights-none = لا يوجد بطء متكرر
open-report-insights-none-detail = تظهر التلميحات عندما تُفتح عدة صور ببطء للسبب نفسه. للعثور عليها يحتفظ { -app-name } بسجل أسبوع من عمليات الفتح البطيئة على هذا الحاسوب؛ ولا يُرسَل أي شيء إلى أي مكان.
open-report-insights-software = صور { $format } يُفك ترميزها على CPU
open-report-insights-storage = قراءة الملفات تؤخّر الصور
open-report-insights-archive = فك ضغط الأرشيفات يؤخّر الصور
open-report-insights-nothing-now = لا يغيّر أي إعداد في { -app-name } هذا الأمر.
open-report-insights-display-cache = شغّل ذاكرة العرض المؤقتة: ستُفتح الصور التي فتحتها من قبل من نسخة سريعة على هذا الحاسوب.
open-report-insights-prefetch = أبقِ مزيدًا من الصور المجاورة جاهزة، لتُجهَّز الصور التالية بينما تشاهد.
open-report-insights-hw-ssd = قرص داخلي سريع (NVMe SSD) لهذه الملفات
open-report-insights-action-turn-on = تشغيل
open-report-insights-action-try-again = إعادة المحاولة
open-report-insights-action-automatic = الضبط على تلقائي
open-report-insights-dismiss-storage = عدم العرض مجددًا للتخزين
open-report-insights-dismiss-decoder = عدم العرض مجددًا لفك الترميز
open-report-insights-off = إيقاف التلميحات
open-report-insights-on = تشغيل التلميحات
open-report-insights-restore = إظهار التلميحات المخفية مجددًا
open-report-insights-power = هذا الحاسوب يوفّر الطاقة (يعمل على البطارية أو في وضع توفير الطاقة)، مما يبطئ فك الترميز على CPU. عند توصيله بالكهرباء قد تُفتح الصور أسرع.

## Performance insights in the gallery: the same, about making thumbnails
# machine-drafted, needs native review
# $count and $total are counts of thumbnails, $total in -last-thumbnail a formatted duration
# ("240 ms"), $format a file format ("HEIC"), $speed a formatted number (MB/s).
open-report-insights-announce-thumbnails = تلميح أداء: الصور المصغّرة تظهر ببطء مرارًا للسبب نفسه. افتح «لماذا هذا بطيء؟» لترى ما قد يساعد.
open-report-insights-last-thumbnail = آخر صورة مصغّرة بطيئة: { $total } لإنشائها
open-report-insights-thumbnails-software = الصور المصغّرة لصور { $format } يُفك ترميزها على CPU
open-report-insights-thumbnails-storage = قراءة الملفات تؤخّر الصور المصغّرة
open-report-insights-thumbnails-archive = فك ضغط الأرشيفات يؤخّر الصور المصغّرة
open-report-insights-thumbnail-cache = لا يمكن الاحتفاظ بالصور المصغّرة على هذا الحاسوب (ذاكرة الصور المصغّرة المؤقتة غير متاحة)، لذا تُنشأ من جديد في كل مرة يُعرض فيها المجلد.
open-report-advice-thumbnails-software = { $count } من آخر { $total } من الصور المصغّرة البطيئة لصور { $format } فُكّ ترميزها برمجيًا لهذا السبب
open-report-advice-thumbnails-slow-storage = { $count } من آخر { $total } من الصور المصغّرة البطيئة تأخرت بسبب قراءة الملف (نحو { $speed } MB/s)
# machine-drafted, needs native review
open-report-advice-thumbnails-slow-storage-no-speed = { $count } من آخر { $total } من الصور المصغّرة البطيئة تأخرت بسبب قراءة الملف (ملفات صغيرة: معظم الوقت في انتظار استجابة وحدة التخزين)
open-report-advice-thumbnails-archive = { $count } من آخر { $total } من الصور المصغّرة البطيئة تأخرت بسبب فك ضغط الأرشيف
