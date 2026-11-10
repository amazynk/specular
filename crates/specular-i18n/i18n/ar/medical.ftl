# Specular — Medical images (DICOM). machine-drafted, needs native review

cheatsheet-medical = الصور الطبية
palette-window-level = أداة النافذة/المستوى (سحب بالزر الأيسر)
palette-window-default = النافذة: افتراضي الملف
palette-window-lung = النافذة: الرئة (W 1500, L −600)
palette-window-mediastinum = النافذة: المنصف (W 350, L 50)
palette-window-bone = النافذة: العظم (W 1800, L 400)
palette-window-brain = النافذة: الدماغ (W 80, L 40)
palette-window-stroke = النافذة: السكتة (W 40, L 40)
palette-window-soft-tissue = النافذة: الأنسجة الرخوة (W 400, L 50)
palette-window-liver = النافذة: الكبد (W 150, L 30)
palette-window-subdural = النافذة: تحت الجافية (W 200, L 75)
palette-window-full-range = النافذة: النطاق الكامل
palette-invert-grey = عكس التدرج الرمادي
palette-next-slice = الإطار التالي
palette-prev-slice = الإطار السابق
disabled-not-medical = للصور الطبية الرمادية فقط
disabled-one-frame = الصورة ذات إطار واحد
settings-section-medical = الصور الطبية
settings-medical-keep-window = الإبقاء على النافذة/المستوى للسلسلة التالية
settings-medical-keep-window-help = الإبقاء على النافذة التي ضبطتها عند الانتقال إلى سلسلة أخرى بدلًا من البدء بقيمتها الافتراضية. إطارات الملف الواحد وشرائح السلسلة الواحدة تتشاركها دائمًا.
medical-window = C { $center }  W { $width }
medical-inverted = معكوس
medical-frame = الإطار { $frame } من { $count }
medical-tool = أداة النافذة/المستوى: اسحب
medical-tool-on = أداة النافذة/المستوى مفعّلة: اسحب للضبط
medical-tool-off = أداة النافذة/المستوى معطّلة
medical-preset-not-ct = تتطلب إعدادات التصوير المقطعي قيمًا بوحدات هاونسفيلد
medical-file-window-none = لا توجد نافذة { $index } في هذا الملف
medical-file-window-unnamed = نافذة الملف { $index } من { $count }
medical-file-window = نافذة الملف { $index } من { $count }: { $name }
measure-templates-need-medical = تتوفر القوالب الهندسية مع الملف الطبي
medical-off = لا أدوات طبية لهذه الصورة: { $reason }
medical-readout = القيمة والنافذة

## Measurements, cine and overlays (machine-drafted, needs native review)

palette-cine = تشغيل الإطارات (سينما)
palette-measure-length = قياس الطول
palette-measure-angle = قياس الزاوية
palette-measure-ellipse = قياس منطقة إهليلجية
palette-measure-rectangle = قياس منطقة مستطيلة
palette-clear-measurements = مسح القياسات
palette-measure-templates = قوالب هندسية…
palette-overlays = إظهار التراكبات أو إخفاؤها
medical-cine = التشغيل بمعدل { $rate } إطار/ث
medical-overlays-shown = التراكبات ظاهرة
medical-overlays-hidden = التراكبات مخفية
measure-hint-length = الطول: اسحب أو انقر على الطرفين
measure-hint-angle = الزاوية: انقر على ثلاث نقاط، والرأس ثانيًا
measure-hint-ellipse = القطع الناقص: اسحب عبر المنطقة
measure-hint-rectangle = المستطيل: اسحب عبر المنطقة
measure-off = أداة القياس متوقفة
measure-cleared = تم مسح القياسات
measure-length-mm = { $value } مم
# machine-drafted, needs native review
measure-length-angstrom = { $value } Å
measure-length-px = { $value } px
measure-area-mm = المساحة { $value } مم²
measure-area-px = المساحة { $value } px²
measure-angle = { $value }°
measure-mean = المتوسط { $mean } ± { $sd } { $unit }
measure-range = الأدنى { $min }  الأقصى { $max }  ({ $count } px)
measure-caveat-detector = عند الكاشف: دون تصحيح التكبير
measure-caveat-magnification = تصحيح تكبير تقديري
measure-caveat-uncalibrated = لا توجد مسافة بين البكسلات: بالبكسل
measure-caveat-outside-region = خارج منطقة موجات فوق صوتية معايَرة: بالبكسل

## DICOM orientation labels, presentation states and wheel stepping (machine-drafted, needs native review)
settings-medical-orientation-labels = إظهار تسميات الاتجاه
settings-medical-orientation-labels-help = أحرف عند حواف الصورة تدل على اتجاهات المريض (R/L يمين/يسار، A/P أمامي/خلفي، S/I الرأس/القدمان)، مأخوذة من الملف وتتبع التدوير والقلب. يبدّلها المفتاح L طوال الجلسة.
settings-medical-wheel-steps-frames = عجلة الماوس تتنقل بين الإطارات
settings-medical-wheel-steps-frames-help = في ملف أو سلسلة بعدة إطارات تنتقل العجلة بين الإطارات بدلًا من التكبير أو تغيير الصورة. يظل Ctrl+العجلة يكبّر.
# machine-drafted, needs native review
settings-medical-display-function = عرض الرمادي
settings-medical-display-function-help = طريقة عرض درجات الرمادي في الصور الطبية. توزّع DICOM GSDF (دالة العرض القياسية للتدرج الرمادي) هذه الدرجات بخطوات متساوية في وضوحها بين نصوع الأسود والأبيض في الشاشة (DICOM PS3.14)، كما في شاشة تشخيصية معايَرة. يظل ملف تعريف ألوان الشاشة مطبّقًا.
settings-medical-display-function-srgb = قياسي (sRGB)
settings-medical-display-function-gsdf = DICOM GSDF
settings-medical-display-black = نصوع الأسود في الشاشة
settings-medical-display-black-help = نصوع الأسود في الشاشة، بما في ذلك الضوء المحيط، من قياس أو من ورقة بيانات الشاشة. تستخدمه DICOM GSDF.
settings-medical-display-white = نصوع الأبيض في الشاشة
settings-medical-display-white-help = نصوع الأبيض في الشاشة، من قياس أو من ورقة بيانات الشاشة. تستخدمه DICOM GSDF.
palette-orientation-labels = إظهار تسميات الاتجاه أو إخفاؤها
palette-presentation-state = حالة العرض التالية
disabled-no-presentation-state = لا توجد حالة عرض تشير إلى هذه الصورة
medical-labels-shown = تسميات الاتجاه ظاهرة
medical-labels-hidden = تسميات الاتجاه مخفية
medical-presentation = حالة العرض: { $label } ({ $index } من { $count })
medical-presentation-none = حالة العرض معطّلة
medical-presentation-readout = العرض: { $label }
medical-preset-presentation = جدول البحث في حالة العرض يحدد مستويات الرمادي

## Orientation labels for screen readers (machine-drafted, needs native review)
medical-orientation-a11y = الاتجاه: اليسار { $left }، اليمين { $right }، الأعلى { $top }، الأسفل { $bottom }
