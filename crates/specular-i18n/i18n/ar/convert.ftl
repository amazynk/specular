# Specular — Batch convert (DESIGN.md §6.5). machine-drafted, needs native review
# Every output is a new file; the sources are never changed.

palette-convert = تحويل…

## Settings → Convert
settings-section-convert = التحويل
settings-convert-format = الصيغة
settings-convert-format-help = الصيغة التي يكتب بها «تحويل…». تُكتب PNG وTIFF دون فقد؛ ويمكن كتابة WebP وJPEG XL بفقد أو دون فقد.
settings-convert-format-jpeg = JPEG
settings-convert-format-png = PNG
settings-convert-format-webp = WebP
settings-convert-format-jxl = JPEG XL
settings-convert-format-tiff = TIFF
settings-convert-jpeg-quality = جودة JPEG
settings-convert-jpeg-quality-help = القيمة الأعلى تحفظ تفاصيل أكثر في ملفات أكبر. عند 90 يصعب تمييز النتيجة عن الأصل.
settings-convert-jpeg-full-chroma = ألوان بدقة كاملة في JPEG
settings-convert-jpeg-full-chroma-help = تحتفظ بالألوان بدقة كاملة (4:4:4) بدلاً من النصف (4:2:0): حواف لونية أوضح وملفات أكبر.
settings-convert-png-effort = ضغط PNG
settings-convert-png-effort-help = القيمة الأعلى تنتج ملفات أصغر ببطء أكبر. القيمة 1 هي الأسرع.
settings-convert-jxl-effort = جهد JPEG XL
settings-convert-jxl-effort-help = القيمة الأعلى تنتج ملفات أصغر ببطء أكبر.
settings-convert-tiff-compression = ضغط TIFF
settings-convert-tiff-compression-help = Deflate ضغط دون فقد يقرؤه كل برنامج يقرأ TIFF.
settings-convert-tiff-compression-deflate = Deflate
settings-convert-tiff-compression-none = بلا
settings-convert-depth = عمق البت
settings-convert-depth-help = يكتب الخيار «تلقائي» 16 بت لكل عينة عندما يكون في الصورة أكثر من 8 وتستوعبها الصيغة (PNG وTIFF وJPEG XL).
settings-convert-depth-auto = تلقائي
settings-convert-depth-8 = 8 بت
settings-convert-depth-16 = 16 بت
settings-convert-resize = تغيير الحجم
settings-convert-resize-help = تُصغَّر الصور في الضوء الخطي بمرشّح عالي الجودة.
settings-convert-resize-none = الحجم الأصلي
settings-convert-resize-fit = احتواء ضمن
settings-convert-resize-long-edge = الضلع الأطول
settings-convert-resize-percent = نسبة مئوية
settings-convert-width = العرض
settings-convert-width-help = أقصى عرض يمكن أن تصل إليه الصورة مع «احتواء ضمن».
settings-convert-height = الارتفاع
settings-convert-height-help = أقصى ارتفاع يمكن أن تصل إليه الصورة مع «احتواء ضمن».
settings-convert-long-edge = الضلع الأطول
settings-convert-long-edge-help = طول الضلع الأطول للصورة مع «الضلع الأطول».
settings-convert-percent = نسبة مئوية
settings-convert-percent-help = الحجم نسبةً إلى الأصل مع «نسبة مئوية».
settings-convert-upscale = تكبير الصور الصغيرة
settings-convert-upscale-help = عند الإيقاف، تحتفظ الصور الأصغر من الحجم المطلوب بحجمها.
settings-convert-colour = الألوان
settings-convert-colour-help = يحفظ «الإبقاء على ملف التعريف» ملف تعريف ألوان الصورة معها. يناسب «التحويل إلى sRGB» الويب والأجهزة التي لا تدعم إدارة الألوان.
settings-convert-colour-keep = الإبقاء على ملف التعريف
settings-convert-colour-srgb = التحويل إلى sRGB
settings-convert-hdr = صور HDR
settings-convert-hdr-help = تحتفظ PNG وJPEG XL بـ HDR كما هو؛ أما الصيغ الأخرى فتُطبَّق عليها دائمًا مطابقة النغمات إلى SDR.
settings-convert-hdr-keep = الإبقاء عليه إن أمكن
settings-convert-hdr-tone-map = مطابقة النغمات إلى SDR
settings-convert-metadata = البيانات الوصفية
settings-convert-metadata-help = Exif وXMP، مثل الكاميرا وتاريخ الالتقاط والموقع. يُحتفظ بملف تعريف الألوان دائمًا.
settings-convert-metadata-all = الإبقاء على الكل
settings-convert-metadata-no-location = الكل عدا الموقع
settings-convert-metadata-strip = إزالة
settings-convert-folder = مجلد الإخراج
settings-convert-folder-help = مكان حفظ الملفات المحوَّلة. إن لم يُحدَّد، تُحفظ في مجلد فرعي بجوار كل صورة.
settings-convert-folder-unset = مجلد فرعي بجوار الصور
settings-convert-subfolder = اسم المجلد الفرعي
settings-convert-subfolder-help = المجلد الفرعي المستخدم عند عدم تحديد مجلد إخراج. فارغ: مجلد الصور نفسه.
settings-convert-pattern = أسماء الملفات
settings-convert-pattern-help = {"{"}name{"}"} هو الاسم الأصلي، و{"{"}n:03{"}"} الرقم في الدفعة، و{"{"}date:%Y-%m-%d{"}"} تاريخ الالتقاط، و{"{"}w{"}"} و{"{"}h{"}"} الحجم الجديد، و{"{"}ext{"}"} امتداد الصيغة.
settings-convert-conflicts = عندما يكون الاسم مستخدمًا
settings-convert-conflicts-help = لا تُستبدل الملفات الموجودة أبدًا.
settings-convert-conflicts-suffix = إضافة رقم
settings-convert-conflicts-skip = تخطي الصورة

## The dialog
convert-title = تحويل
convert-images = الصور
convert-images-current = هذه الصورة
convert-images-selection = التحديد ({ $count })
convert-images-folder = المجلد ({ $count })
convert-images-picked = المختارة ({ $count })
convert-quality = الجودة
convert-chroma = تفاصيل الألوان
convert-chroma-half = 4:2:0 (أصغر)
convert-chroma-full = 4:4:4 (أوضح)
convert-effort = الجهد
convert-on = تشغيل
convert-off = إيقاف
convert-pixels = { $value } بكسل
convert-effort-fastest = { $value } (الأسرع)
convert-folder-subfolder = المجلد الفرعي «{ $name }»
convert-folder-same = مجلد الصورة نفسه
convert-choose-folder = اختيار مجلد…
convert-name = الاسم
convert-name-preview = { $source } → { $output }
convert-name-error-unclosed = يوجد {"{"} غير مغلق.
convert-name-error-stray = يوجد {"}"} بلا {"{"}. اكتب {"}}"} لإدراج قوس معقوف.
convert-name-error-unknown = { $token } ليس جزءًا من الاسم. استخدم name أو n أو date أو w أو h أو ext.
convert-name-error-format = تنسيق { $token } غير صالح.
convert-name-error-empty = أدخل نمطًا للاسم.
convert-start = تحويل
convert-stop = إيقاف
convert-hide = إخفاء
convert-close = إغلاق
convert-progress = جارٍ تحويل { $done } من { $total }
convert-progress-eta = جارٍ تحويل { $done } من { $total } · يتبقى نحو { $time }
convert-eta-seconds = { $count ->
    [zero] { $count } ثانية
    [one] ثانية واحدة
    [two] ثانيتان
    [few] { $count } ثوانٍ
    [many] { $count } ثانية
   *[other] { $count } ثانية
}
convert-eta-minutes = { $count ->
    [zero] { $count } دقيقة
    [one] دقيقة واحدة
    [two] دقيقتان
    [few] { $count } دقائق
    [many] { $count } دقيقة
   *[other] { $count } دقيقة
}
convert-done = { $count ->
    [zero] لم تُحوَّل أي صورة
    [one] حُوِّلت صورة واحدة
    [two] حُوِّلت صورتان
    [few] حُوِّلت { $count } صور
    [many] حُوِّلت { $count } صورة
   *[other] حُوِّلت { $count } من الصور
}
convert-done-skipped = المتخطاة لأن الاسم مستخدم: { $count }
convert-done-failed = الفاشلة: { $count }
convert-stopped = توقف بعد { $count }
convert-failure = { $file }: { $reason }
convert-failure-open = يتعذّر فتحه ({ $error })
convert-failure-decode = يتعذّر قراءته ({ $error })
convert-failure-encode = يتعذّر كتابته بهذه الصيغة ({ $error })
convert-failure-write = يتعذّر حفظه ({ $error })
convert-failure-same = سيحل الملف الجديد محل الأصل
convert-report-more = و{ $count } غيرها
convert-running = يوجد تحويل قيد التشغيل بالفعل.

## Convert encoders (machine-drafted, needs native review)
settings-convert-format-avif = AVIF
settings-convert-webp-lossless = WebP دون فقد
settings-convert-webp-lossless-help = يحفظ كل بكسل بدقة تامة. عند إيقافه تكون ملفات WebP أصغر بكثير بجودة WebP المحددة.
settings-convert-webp-quality = جودة WebP
settings-convert-webp-quality-help = القيمة الأعلى تحفظ تفاصيل أكثر في ملفات أكبر. يُستخدم عندما لا يكون WebP دون فقد.
settings-convert-webp-effort = جهد WebP
settings-convert-webp-effort-help = القيمة الأعلى تنتج ملفات أصغر ببطء أكبر. يُستخدم عندما لا يكون WebP دون فقد.
settings-convert-jxl-lossless = JPEG XL دون فقد
settings-convert-jxl-lossless-help = يحفظ كل بكسل بدقة تامة. عند إيقافه تكون ملفات JPEG XL أصغر بكثير بجودة JPEG XL المحددة.
settings-convert-jxl-quality = جودة JPEG XL
settings-convert-jxl-quality-help = القيمة الأعلى تحفظ تفاصيل أكثر في ملفات أكبر. عند 90 يصعب تمييز النتيجة عن الأصل. يُستخدم عندما لا يكون JPEG XL دون فقد.
settings-convert-avif-quality = جودة AVIF
settings-convert-avif-quality-help = القيمة الأعلى تحفظ تفاصيل أكثر في ملفات أكبر.
settings-convert-avif-effort = جهد AVIF
settings-convert-avif-effort-help = القيمة الأعلى تنتج ملفات أصغر ببطء أكبر. القيمة 1 هي الأسرع.
convert-lossless = دون فقد
