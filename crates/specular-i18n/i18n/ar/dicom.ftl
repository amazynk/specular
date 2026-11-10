# DICOM series and the tag browser. machine-drafted, needs native review

palette-dicom-tags = عرض وسوم DICOM
palette-next-series = السلسلة التالية
palette-prev-series = السلسلة السابقة
disabled-not-dicom = الصورة ليست ملف DICOM
disabled-no-other-series = لا توجد سلسلة أخرى في هذا المجلد
dicom-tags-title = وسوم DICOM
dicom-tags-title-file = وسوم DICOM: { $file }
dicom-tags-filter = تصفية الوسوم
dicom-tags-show-patient = إظهار بيانات المريض
dicom-tags-col-tag = الوسم
dicom-tags-col-keyword = الكلمة المفتاحية
dicom-tags-col-vr = VR
dicom-tags-col-length = الطول
dicom-tags-col-value = القيمة
dicom-tags-hidden = مخفي
dicom-tags-hidden-count = قيم المريض المخفية: { $count }
dicom-tags-no-match = لا وسوم مطابقة
dicom-tags-reading = جارٍ قراءة الوسوم…
dicom-tags-error = تعذّرت قراءة الوسوم: { $reason }
dicom-tags-items = العناصر: { $count }
dicom-tags-fragments = الأجزاء: { $count }
dicom-tags-item = عنصر { $index }
dicom-tags-bytes = البايتات: { $count }
dicom-tags-empty = (فارغ)
dicom-tags-omitted = عناصر أخرى غير مدرجة: { $count }
dicom-series-group = السلسلة
dicom-series-slice = المقطع
dicom-series-slice-value = { $index } من { $count }
dicom-series-frames = الإطارات: { $frames }
dicom-series-location = موضع المقطع
dicom-series-mm = { $mm } مم
dicom-series-spacing = تباعد المقاطع
dicom-series-which = السلسلة
dicom-series-which-value = { $index } من { $count }
dicom-series-order = الترتيب
dicom-series-order-position = حسب الموضع
dicom-series-order-instance = حسب رقم المثيل
dicom-series-order-time = حسب وقت الاكتساب
dicom-series-order-name = حسب اسم الملف
dicom-series-gaps = مقاطع مفقودة
dicom-series-gaps-value = المقاطع المفقودة: { $count }
dicom-series-gaps-where = بعد المقطع { $slices }
dicom-series-irregular = التباعد غير منتظم
dicom-series-tilt = ميل الجسر
dicom-series-tilt-value = { $degrees }°
dicom-series-duplicates = تم تخطي المكررات
dicom-series-toast = السلسلة { $index } من { $series } – المقاطع: { $count }
dicom-series-capped = قُرئت أول ملفات المجلد فقط: { $count }

studies-title = الدراسات
studies-title-medium = الدراسات: { $medium }
studies-show-patient = إظهار بيانات المريض
studies-patient-hidden = مخفي
studies-patient-hidden-note = أسماء المرضى وتواريخ ميلادهم مخفية
studies-none = لم يُعثر على دراسات
studies-reading = جارٍ قراءة القرص…
studies-reading-count = جارٍ قراءة القرص: { $done } من { $total ->
        [zero] { $total } ملفات
        [one] ملف واحد
        [two] ملفين
        [few] { $total } ملفات
        [many] { $total } ملفًا
       *[other] { $total } من الملفات
    }
studies-opening = جارٍ فتح السلسلة…
studies-error = تعذّرت قراءة القرص: { $reason }
studies-missing = ملفات مدرجة على القرص مفقودة: { $count }
studies-capped = قُرئت أول الملفات فقط: { $count }
# machine-drafted, needs native review
studies-stopped = توقفت القراءة بعد { $done } من { $total ->
        [zero] { $total } ملفات
        [one] ملف واحد
        [two] ملفين
        [few] { $total } ملفات
        [many] { $total } ملفًا
       *[other] { $total } من الملفات
    }: القائمة غير مكتملة
studies-stopped-early = توقفت القراءة: القائمة غير مكتملة
studies-cancel-description = يوقف قراءة القرص. يبقى ما قُرئ حتى الآن مدرجًا.
studies-patient-n = المريض { $index }
studies-patient-unknown = مريض غير معروف
studies-born = تاريخ الميلاد: { $date }
studies-no-description = بلا وصف
studies-series = السلسلة { $number }: { $description }
studies-series-number = السلسلة { $number }
studies-images = الصور: { $count }
studies-images-missing = المفقودة: { $count }
studies-others = كائنات أخرى: { $count }
studies-no-images = لا تحتوي هذه السلسلة على صور على القرص
studies-medium-inserted = أُدخل قرص يحتوي على دراسات طبية
studies-medium-show = إظهار الدراسات
palette-studies = إظهار الدراسات على هذا القرص
