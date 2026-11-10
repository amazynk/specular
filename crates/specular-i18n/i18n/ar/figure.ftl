# Figures, key images and teaching files.

## Command palette
# machine-drafted, needs native review
palette-export-figure = تصدير الشكل…
# machine-drafted, needs native review
palette-export-figure-help = حفظ العرض كشكل مع شريط المقياس وشريط الألوان والتسميات.
# machine-drafted, needs native review
palette-toggle-key-image = وضع علامة صورة مفتاحية
# machine-drafted, needs native review
palette-toggle-key-image-help = وضع علامة صورة مفتاحية على هذه الصورة، أو إزالتها.
# machine-drafted, needs native review
palette-copy-key-images = نسخ الصور المفتاحية
# machine-drafted, needs native review
palette-copy-key-images-help = نسخ الصور المفتاحية كصورة واحدة.
# machine-drafted, needs native review
palette-export-key-images = تصدير الصور المفتاحية…
# machine-drafted, needs native review
palette-export-key-images-help = حفظ الصور المفتاحية كشرائح أو كمجلد ملفات PNG.
# machine-drafted, needs native review
palette-export-teaching-file = تصدير ملف تعليمي…
# machine-drafted, needs native review
palette-export-teaching-file-help = حفظ الصور المفتاحية دون بيانات المريض، للتعليم.

## The figure dialog
# machine-drafted, needs native review
figure-dialog-title = تصدير الشكل
# machine-drafted, needs native review
figure-format = الصيغة
# machine-drafted, needs native review
figure-format-png = PNG، 16 بت
# machine-drafted, needs native review
figure-format-tiff = TIFF، 16 بت
# machine-drafted, needs native review
figure-region = المنطقة
# machine-drafted, needs native review
figure-region-visible = المنطقة المرئية
# machine-drafted, needs native review
figure-region-whole = الصورة كاملة
# machine-drafted, needs native review
figure-size = الحجم
# machine-drafted, needs native review
figure-size-pixels = وحدات بكسل الصورة (1:1)
# $width is a width in pixels.
# machine-drafted, needs native review
figure-size-width = العرض { $width } px
# machine-drafted, needs native review
figure-scale-bar = شريط المقياس
# machine-drafted, needs native review
figure-scale-bar-none = بلا معايرة
# machine-drafted, needs native review
figure-colour-bar = شريط الألوان
# machine-drafted, needs native review
figure-colour-bar-none = بلا خريطة ألوان أو نافذة
# machine-drafted, needs native review
figure-inset = مقطع مكبّر
# machine-drafted, needs native review
figure-inset-rectangle = آخر مستطيل
# machine-drafted, needs native review
figure-inset-none = ارسم مستطيلاً أولاً
# machine-drafted, needs native review
figure-channels = القنوات
# machine-drafted, needs native review
figure-channels-merged = مدمجة
# machine-drafted, needs native review
figure-channels-split = منفصلة ومدمجة
# machine-drafted, needs native review
figure-channels-one = قناة واحدة
# machine-drafted, needs native review
figure-measurements = القياسات
# $count is the number of measurements.
# machine-drafted, needs native review
figure-measurements-on = { $count ->
    [zero] مفعّلة ({ $count })
    [one] مفعّلة ({ $count })
    [two] مفعّلة ({ $count })
    [few] مفعّلة ({ $count })
    [many] مفعّلة ({ $count })
   *[other] مفعّلة ({ $count })
}
# machine-drafted, needs native review
figure-measurements-none = بلا
# machine-drafted, needs native review
figure-title-row = العنوان
# machine-drafted, needs native review
figure-provenance = المصدر
# machine-drafted, needs native review
figure-provenance-metadata = في الملف
# machine-drafted, needs native review
figure-provenance-drawn = في الملف ومرسومة
# machine-drafted, needs native review
figure-file = الملف
# machine-drafted, needs native review
figure-choose = اختيار…
# machine-drafted, needs native review
figure-on = مفعّل
# machine-drafted, needs native review
figure-off = معطّل
# $source is where the value comes from (a calibration, a colour map).
# machine-drafted, needs native review
figure-on-from = مفعّل · { $source }
# machine-drafted, needs native review
figure-export = تصدير
# machine-drafted, needs native review
figure-working = جارٍ التصدير…
# machine-drafted, needs native review
figure-not-full-res = لم تُحمَّل وحدات البكسل بدقتها الكاملة. كبّر إلى { PERCENT(100) } أو اختر حجماً.
# machine-drafted, needs native review
figure-channel-red = أحمر
# machine-drafted, needs native review
figure-channel-green = أخضر
# machine-drafted, needs native review
figure-channel-blue = أزرق
# $map is a colour map's name; $lo and $hi the values at its ends; $unit their unit.
# machine-drafted, needs native review
figure-legend-map = { $map }، من { $lo } إلى { $hi } { $unit }
# $lo and $hi are the window's ends; $unit their unit (HU).
# machine-drafted, needs native review
figure-legend-window = النافذة من { $lo } إلى { $hi } { $unit }
# $preset is a channel-colour preset's id.
# machine-drafted, needs native review
figure-display-channels = ألوان القنوات { $preset }
# $channel is the channel shown (red, green, blue, alpha, luma).
# machine-drafted, needs native review
figure-display-channel = القناة { $channel }
# machine-drafted, needs native review
figure-display-hilo = علامات HiLo
# machine-drafted, needs native review
figure-display-as-stored = كما حُفظت
# $value is the calibration (source and spacing).
# machine-drafted, needs native review
figure-caption-calibration = المعايرة: { $value }
# $value is the display settings.
# machine-drafted, needs native review
figure-caption-display = العرض: { $value }
# machine-drafted, needs native review
figure-not-image = تُنشأ الأشكال من صورة في العارض.
# machine-drafted, needs native review
figure-not-ready = ما زالت الصورة قيد التحميل.
# $width and $height are in pixels.
# machine-drafted, needs native review
figure-too-large = كبيرة جداً: { $width } × { $height } px. اختر المنطقة المرئية أو حجماً.

## Key images
# machine-drafted, needs native review
figure-keys-title = تصدير الصور المفتاحية
# machine-drafted, needs native review
figure-teaching-title = تصدير ملف تعليمي
# machine-drafted, needs native review
figure-keys-images = الصور
# $count is the number of key images.
# machine-drafted, needs native review
figure-keys-count = { $count ->
    [zero] { $count } صورة مفتاحية
    [one] { $count } صورة مفتاحية
    [two] { $count } صورتان مفتاحيتان
    [few] { $count } صور مفتاحية
    [many] { $count } صورة مفتاحية
   *[other] { $count } صورة مفتاحية
}
# machine-drafted, needs native review
figure-keys-pptx = شرائح (PPTX)
# machine-drafted, needs native review
figure-keys-folder = مجلد ملفات PNG
# machine-drafted, needs native review
figure-keys-preset = إعداد مسبق
# machine-drafted, needs native review
figure-keys-standard = قياسي
# machine-drafted, needs native review
figure-keys-teaching = ملف تعليمي
# machine-drafted, needs native review
figure-keys-overlays-on = مرسومة وفي الملاحظات
# machine-drafted, needs native review
figure-keys-masks = نص مدمج في الصورة
# machine-drafted, needs native review
figure-keys-masks-auto = محجوب بالأسود
# machine-drafted, needs native review
figure-keys-notes = ملاحظات الحالة
# machine-drafted, needs native review
figure-keys-destination = الحفظ في
# machine-drafted, needs native review
figure-keys-empty = لا توجد صور مفتاحية بعد. ضع علامة صورة مفتاحية على صورة أولاً.
# machine-drafted, needs native review
figure-keys-teaching-note = تُحذف بيانات المريض ويُحجب النص المدمج في الصور بالأسود. راجع كل صورة قبل المشاركة.
# machine-drafted, needs native review
figure-keys-none-left = تعذّر تصدير أي صورة دون بيانات المريض.
# machine-drafted, needs native review
figure-keys-copied = نُسخت الصور المفتاحية
# $count is how many key images there are now.
# machine-drafted, needs native review
figure-key-marked = { $count ->
    [zero] وُضعت علامة صورة مفتاحية ({ $count })
    [one] وُضعت علامة صورة مفتاحية ({ $count })
    [two] وُضعت علامة صورة مفتاحية ({ $count })
    [few] وُضعت علامة صورة مفتاحية ({ $count })
    [many] وُضعت علامة صورة مفتاحية ({ $count })
   *[other] وُضعت علامة صورة مفتاحية ({ $count })
}
# $count is how many key images are left.
# machine-drafted, needs native review
figure-key-unmarked = { $count ->
    [zero] لم تعد صورة مفتاحية (المتبقي { $count })
    [one] لم تعد صورة مفتاحية (المتبقي { $count })
    [two] لم تعد صورة مفتاحية (المتبقي { $count })
    [few] لم تعد صورة مفتاحية (المتبقي { $count })
    [many] لم تعد صورة مفتاحية (المتبقي { $count })
   *[other] لم تعد صورة مفتاحية (المتبقي { $count })
}
# $n is a frame or page number.
# machine-drafted, needs native review
figure-key-frame = الصورة { $n }
# $n is the key image's number in the set.
# machine-drafted, needs native review
figure-key-n = الصورة المفتاحية { $n }

## Results
# $name is the file or folder written.
# machine-drafted, needs native review
figure-saved = تم حفظ { $name }
# $name is the file or folder written; $count how many images were left out.
# machine-drafted, needs native review
figure-saved-skipped = { $count ->
    [zero] تم حفظ { $name }. استُبعدت { $count } صورة: تعذّر العثور على النص المدمج فيها.
    [one] تم حفظ { $name }. استُبعدت { $count } صورة: تعذّر العثور على النص المدمج فيها.
    [two] تم حفظ { $name }. استُبعدت { $count } صورتان: تعذّر العثور على النص المدمج فيهما.
    [few] تم حفظ { $name }. استُبعدت { $count } صور: تعذّر العثور على النص المدمج فيها.
    [many] تم حفظ { $name }. استُبعدت { $count } صورة: تعذّر العثور على النص المدمج فيها.
   *[other] تم حفظ { $name }. استُبعدت { $count } صورة: تعذّر العثور على النص المدمج فيها.
}
# $reason says why.
# machine-drafted, needs native review
figure-failed = تعذّر التصدير: { $reason }
# $name is a file that is already in the folder.
# machine-drafted, needs native review
figure-folder-taken = { $name } موجود بالفعل في هذا المجلد. اختر مجلداً فارغاً.

## The viewer
# machine-drafted, needs native review
figure-key-badge = صورة مفتاحية
