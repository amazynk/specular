# Specular — أداة القياس (الأطوال والزوايا والمناطق وإحصاءاتها على أي صورة). مسودة آلية، تحتاج إلى مراجعة من متحدث أصلي.

palette-measure = قياس
palette-measure-copy-csv = نسخ القياسات بصيغة CSV
palette-measure-export = تصدير القياسات…
toast-copied-measurements = تم نسخ القياسات بصيغة CSV

measure-tool-title = قياس
measure-tool-list = القياسات
measure-tool-count = { $count ->
    [0] لا توجد قياسات
    [zero] لا توجد قياسات
    [one] قياس واحد
    [two] قياسان
    [few] { $count } قياسات
    [many] { $count } قياسًا
   *[other] { $count } قياس
}
measure-tool-empty = اختر أداة وارسم على الصورة.

measure-tool-point = نقطة
measure-tool-line = خط
measure-tool-polyline = خط متعدد
measure-tool-angle = زاوية
measure-tool-rect = مستطيل
measure-tool-ellipse = قطع ناقص
measure-tool-polygon = مضلع
measure-tool-freehand = رسم حر
measure-tool-calibrate = معايرة
# machine-drafted, needs native review
measure-tool-spot = نقطة

measure-tool-undo = تراجع
measure-tool-delete = حذف
measure-tool-copy = نسخ CSV
measure-tool-export = تصدير…
measure-tool-import = استيراد…
measure-tool-done = تم
measure-tool-unit = الوحدة: { $unit }
measure-tool-apply = تطبيق

measure-tool-hint-line = اسحب أو انقر على نقطتين. الأسهم تحرّك علامة التصويب، والمسافة تضع نقطة.
measure-tool-hint-angle = انقر على ثلاث نقاط؛ الثانية هي الرأس.
measure-tool-hint-rect = اسحب أو انقر على زاويتين.
measure-tool-hint-ellipse = اسحب أو انقر على زاويتين من إطاره.
measure-tool-hint-polygon = انقر على الزوايا؛ Enter أو النقر على الزاوية الأولى يغلق الشكل.
measure-tool-hint-calibrate = ارسم خطًا فوق شيء معلوم الطول.
# machine-drafted, needs native review
measure-tool-hint-spot = انقر على نقطة لقراءة درجة حرارتها.
measure-tool-known-length = الطول المعلوم: { $value } { $unit } (اكتب الرقم، U يغيّر الوحدة، Enter يطبّق)
measure-tool-calibrated = تمت المعايرة: البكسل الواحد يساوي { $size } { $unit }
measure-tool-bad-length = اكتب طولًا أكبر من الصفر.

measure-tool-item = { $n }. { $shape }
measure-tool-area = المساحة { $value }
measure-tool-mean = المتوسط { $mean } ± { $sd }
measure-tool-means = المتوسط { $values }
measure-tool-range = النطاق { $min } – { $max } على { $count } px
measure-tool-pixels = على { $count } px
measure-tool-stats-pending = جارٍ قراءة البكسلات…
measure-tool-stats-none = لا توجد قيم بكسل هنا
measure-tool-approximate = من نسخة مصغّرة (تقريبي)

measure-tool-deleted = تم حذف القياس
measure-tool-undone = تم التراجع
measure-tool-nothing = لا توجد قياسات على هذه الصورة
measure-tool-exported = حُفظت القياسات في { $name }
measure-tool-export-failed = تعذّر حفظ القياسات: { $reason }
measure-tool-imported = { $count ->
    [zero] لم يُستورد أي قياس.
    [one] تم استيراد قياس واحد.
    [two] تم استيراد قياسين.
    [few] تم استيراد { $count } قياسات.
    [many] تم استيراد { $count } قياسًا.
   *[other] تم استيراد { $count } قياس.
}
measure-tool-skipped = { $count ->
    [zero] لم يتعذّر قراءة أي شكل.
    [one] تعذّرت قراءة شكل واحد.
    [two] تعذّرت قراءة شكلين.
    [few] تعذّرت قراءة { $count } أشكال.
    [many] تعذّرت قراءة { $count } شكلًا.
   *[other] تعذّرت قراءة { $count } شكل.
}
measure-tool-import-failed = تعذّرت قراءة القياسات: { $reason }
measure-tool-too-large = الملف كبير جدًا.
# machine-drafted, needs native review
measure-tool-sr-not-dicom = يتطلب DICOM SR صورة DICOM. احفظ بصيغة CSV أو GeoJSON بدلًا من ذلك.

measure-source-uncalibrated = بكسلات (غير معايَرة)
measure-source-dicom-pixel-spacing = تباعد البكسل في DICOM
measure-source-dicom-calibrated = تباعد DICOM المعايَر
measure-source-dicom-imager-pixel-spacing = تباعد DICOM عند الكاشف
measure-source-dicom-magnification = تباعد DICOM مصحَّح للتكبير
measure-source-dicom-ultrasound-region = منطقة الموجات فوق الصوتية DICOM
measure-source-tiff-resolution = دقة TIFF
measure-source-exif-resolution = دقة EXIF
measure-source-ome-physical-size = الحجم الفعلي OME
measure-source-fits-wcs = إحداثيات العالم FITS
measure-source-vendor-tag = بيانات الشركة المصنّعة الوصفية
measure-source-drawing-units = وحدات الرسم
# machine-drafted, needs native review
measure-source-drawing-scale = من مقياس الرسم
# machine-drafted, needs native review
measure-source-drawing-scale-ratio = من الرسم (المقياس { $scale })
# machine-drafted, needs native review
measure-source-drawing-scale-viewport = من الرسم (المقياس { $scale }، إطار العرض { $viewport })
# machine-drafted, needs native review
measure-source-map-scale = مقياس الخريطة في المركز (تقريبي؛ أدوات الخرائط تقيس على الأرض)
measure-source-user = معايرتك
