# machine-drafted, needs native review
# Specular — علم الأمراض: فصل الصبغات، تصدير التعليقات واستيرادها، أحجام التسميات فوق الأحجام. العربية.

## لوحة الأوامر
palette-stain-separation = فصل الصبغات
palette-stain-1 = إظهار الصبغة 1 أو إخفاؤها
palette-stain-2 = إظهار الصبغة 2 أو إخفاؤها
palette-stain-3 = إظهار الصبغة 3 أو إخفاؤها
palette-stain-mode = الصبغات كضوء أو ككثافة ضوئية
palette-stain-estimate = تقدير الصبغات من المنطقة
palette-stain-normalise = توحيد ألوان الصبغات
palette-stain-reference = استخدام المنطقة مرجعًا للصبغات
palette-stain-reset = متجهات الصبغات المنشورة
palette-export-annotations = تصدير التعليقات…
palette-import-annotations = استيراد التعليقات…
palette-label-volume = تراكب حجم التسميات…

## أسباب التعطيل
disabled-not-colour-image = يتطلب صورة ملونة
disabled-stains-off = شغّل فصل الصبغات أولًا
disabled-no-region = ارسم منطقة أولًا
disabled-no-slide-annotations = لا توجد تعليقات أو مناطق أو نقاط لتصديرها
disabled-no-label-target = افتح حجمًا في العروض متعددة المستويات، أو صورة طبية

## أسماء الصبغات
stain-hematoxylin = هيماتوكسيلين
stain-eosin = إيوزين
stain-dab = DAB
stain-residual = المتبقي
stain-set-he = H&E
stain-set-hdab = H-DAB
stain-set-hedab = H&E-DAB

## إشعارات الصبغات ومفتاحها
stain-off = فصل الصبغات متوقف
# { $set }: H&E أو H-DAB أو H&E-DAB.
stain-on = الصبغات: { $set }
stain-shown = { $stain } ظاهرة
stain-hidden = { $stain } مخفية
stain-mode-light = الصبغات كضوء
stain-mode-od = الصبغات ككثافة ضوئية
# عنوان المفتاح: "H&E · ضوء"، "H-DAB · كثافة ضوئية".
stain-legend-light = { $set } · ضوء
stain-legend-od = { $set } · كثافة ضوئية
stain-legend-estimated = مقدّرة من المنطقة
stain-legend-normalised = موحّدة
stain-legend-published = المتجهات المنشورة
# { $pixels }: البكسلات المصبوغة المستخدمة.
stain-estimated = قُدّرت الصبغات من { $pixels } بكسل
stain-estimate-failed = تعذّر تقدير الصبغات: { $reason }
stain-estimate-background = معظم المنطقة خلفية
stain-estimate-one-colour = المنطقة تحوي لونًا واحدًا
stain-estimate-three = التقدير يتطلب H&E أو H-DAB
stain-estimate-no-pixels = بكسلات المنطقة لم تُحمّل بعد
stain-reference-set = عُيّنت المنطقة مرجعًا للصبغات
stain-normalise-on = ألوان الصبغات موحّدة
stain-normalise-off = ألوان الصبغات كما مُسحت
stain-reset-done = المتجهات المنشورة للصبغات

## نافذة تصدير التعليقات
annot-export-title = تصدير التعليقات
annot-export-format = الصيغة
annot-export-format-coco = COCO JSON
annot-export-format-geojson = GeoJSON (QuPath)
annot-export-format-masks = أقنعة التسميات
annot-export-output = الأقنعة بصيغة
annot-export-output-png = مربعات PNG
annot-export-output-tiff = مربعات TIFF
annot-export-output-ome = هرم OME-TIFF
annot-export-region = المنطقة
annot-export-region-slide = الشريحة كاملة
annot-export-region-view = المنطقة الظاهرة
annot-export-region-drawn = آخر منطقة مرسومة
annot-export-level = المستوى
# { $level }: 0 للدقة الكاملة؛ { $width } × { $height } حجمه بالبكسل.
annot-export-level-value = { $level } · { $width } × { $height }
annot-export-tile = حجم المربع
annot-export-empty = المربعات الفارغة
annot-export-empty-skip = تخطٍّ
annot-export-empty-keep = كتابة
annot-export-what = { $count ->
    [zero] لا أشكال
    [one] شكل واحد
    [two] شكلان
    [few] { $count } أشكال
    [many] { $count } شكلًا
   *[other] { $count } شكل
}، { $classes ->
    [zero] لا فئات
    [one] فئة واحدة
    [two] فئتان
    [few] { $classes } فئات
    [many] { $classes } فئةً
   *[other] { $classes } فئة
}
annot-export-lines = تُستبعد الخطوط من COCO والأقنعة.
annot-export-export = تصدير…
annot-export-writing = جارٍ الكتابة…
# { $files }: الملفات المكتوبة.
annot-export-done = { $files ->
    [zero] لم يُصدَّر أي ملف
    [one] صُدِّر ملف واحد
    [two] صُدِّر ملفان
    [few] صُدِّرت { $files } ملفات
    [many] صُدِّر { $files } ملفًا
   *[other] صُدِّر { $files } ملف
}
annot-export-failed = فشل التصدير: { $reason }
annot-import-done = { $count ->
    [zero] لم يُستورد أي تعليق
    [one] استُورد تعليق واحد
    [two] استُورد تعليقان
    [few] استُوردت { $count } تعليقات
    [many] استُورد { $count } تعليقًا
   *[other] استُورد { $count } تعليق
}
annot-import-failed = تعذّر الاستيراد: { $reason }
annot-import-skipped = { $count ->
    [zero] لا أشكال بلا محيط صالح
    [one] شكل واحد بلا محيط صالح
    [two] شكلان بلا محيط صالح
    [few] { $count } أشكال بلا محيط صالح
    [many] { $count } شكلًا بلا محيط صالح
   *[other] { $count } شكل بلا محيط صالح
}
annot-file-coco = COCO JSON
annot-file-geojson = GeoJSON

## أحجام التسميات
label-volume-file = حجم التسميات
label-volume-table = جدول الألوان
# { $file }: اسم ملف حجم التسميات؛ { $count }: تسمياته.
label-volume-loaded = { $count ->
    [zero] { $file }: لا تسميات
    [one] { $file }: تسمية واحدة
    [two] { $file }: تسميتان
    [few] { $file }: { $count } تسميات
    [many] { $file }: { $count } تسميةً
   *[other] { $file }: { $count } تسمية
}
label-volume-failed = تعذّر تراكب { $file }: { $reason }
label-volume-outside = { $file } لا يتداخل مع هذا الحجم
label-volume-not-whole = قيمه ليست أعدادًا صحيحة
label-volume-colour = يحوي ألوانًا لا تسميات
label-volume-empty = لا يحوي تسميات
label-volume-too-many = يحوي تسميات كثيرة جدًا
label-volume-too-large = أكبر من أن يُعاد أخذ عيناته
label-volume-degenerate = اتجاهه غير صالح
# تسمية بلا اسم في جدول الألوان.
label-volume-unnamed = التسمية { $value }
