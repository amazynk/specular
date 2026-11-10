# تشغيل الصور المتحركة ووضع القراءة (الأرشيفات والقصص المصورة).

disabled-not-animated = هذه الصورة ليست متحركة
disabled-in-archive = لا يمكن نقل الصفحات الموجودة داخل أرشيف إلى سلة المهملات

## لوحة الأوامر وقائمة الاختصارات

palette-play-pause = تشغيل الصورة المتحركة أو إيقافها مؤقتًا
palette-next-frame = الإطار التالي
palette-prev-frame = الإطار السابق
palette-faster = تشغيل أسرع
palette-slower = تشغيل أبطأ
palette-normal-speed = سرعة التشغيل العادية
palette-loop-forever = تكرار الصورة المتحركة بلا نهاية
palette-right-to-left = ترتيب الصفحات من اليمين إلى اليسار
# machine-drafted, needs native review
palette-contents = جدول المحتويات
palette-spreads = صفحتان متقابلتان

## عناصر التحكم في الصورة المتحركة

transport-region = عناصر التحكم في الصورة المتحركة
transport-play = تشغيل
transport-pause = إيقاف مؤقت
transport-prev-frame = الإطار السابق
transport-next-frame = الإطار التالي
transport-slower = أبطأ
transport-faster = أسرع
transport-frame = الإطار { $index } / { $count }
transport-frame-unknown = الإطار { $index }
transport-speed = { $speed }×
transport-speed-name = سرعة التشغيل { $speed }×
# machine-drafted, needs native review
motion-badge = متحركة
motion-badge-live = Live
motion-play = تشغيل المقطع المتحرك
motion-play-live = تشغيل Live Photo
motion-stop = العودة إلى الصورة الثابتة
toast-motion-failed = تعذّر تشغيل المقطع المتحرك: { $reason }

## الإشعارات

toast-speed = سرعة التشغيل { $speed }×
toast-loop-forever-on = تكرار بلا نهاية
toast-loop-forever-off = تكرار حسب الملف
toast-right-to-left-on = الصفحات تُقرأ من اليمين إلى اليسار
toast-right-to-left-off = الصفحات تُقرأ من اليسار إلى اليمين
toast-spreads-on = صفحتان متقابلتان
toast-spreads-off = صفحة واحدة في كل مرة

## الأرشيفات

title-archive-page = { $page } — { $archive }
readout-position-spread = { $first }–{ $last } / { $count }
archive-needs-sandbox = لا يمكن فتح أرشيفات { $format } إلا بواسطة مفكك الترميز المعزول، وهو غير مضمَّن في هذا الإصدار بعد.
archive-empty = لا يحتوي هذا الأرشيف على صور.
archive-failed = تعذّر فتح الأرشيف: { $error }
archive-encrypted = هذا الأرشيف مشفّر. افتحه مرة أخرى لإدخال كلمة المرور.
archive-wrong-password = كلمة المرور لهذا الأرشيف غير صحيحة.
password-title = كلمة المرور مطلوبة
password-archive = الأرشيف «{ $archive }» مشفّر.
password-label = كلمة المرور
password-show = إظهار كلمة المرور
password-wrong = كلمة المرور غير صحيحة. حاول مرة أخرى.
password-open = فتح
password-cancel = إلغاء

## الإعدادات

settings-viewing-animation-autoplay = تشغيل الصور المتحركة تلقائيًا
settings-viewing-animation-autoplay-help = بدء الصور المتحركة (GIF وAPNG وWebP…) فور فتحها.
settings-viewing-animation-loop = تكرار الصور المتحركة
settings-viewing-animation-loop-help = عدد مرات تكرار الصور المتحركة.
settings-viewing-animation-loop-file = حسب الملف
settings-viewing-animation-loop-forever = بلا نهاية
settings-viewing-animation-loop-once = مرة واحدة
settings-viewing-animation-short-delays = فواصل الإطارات القصيرة جدًا
settings-viewing-animation-short-delays-help = الإطارات المحددة بـ 10 ملي ثانية أو أقل: إبطاؤها إلى 100 ملي ثانية كما تفعل متصفحات الويب، أو تشغيلها كما خُزّنت.
settings-viewing-animation-short-delays-browser = كمتصفحات الويب (100 ملي ثانية)
settings-viewing-animation-short-delays-as-stored = كما خُزّنت
settings-browsing-reading-order = ترتيب الصفحات في الأرشيفات
settings-browsing-reading-order-help = تُقرأ أرشيفات القصص المصورة من اليمين إلى اليسار عندما يذكر ذلك ملف ComicInfo.xml (المانغا)، ما لم يُضبط غير ذلك هنا.
settings-browsing-reading-order-auto = حسب الأرشيف
settings-browsing-reading-order-left-to-right = من اليسار إلى اليمين
settings-browsing-reading-order-right-to-left = من اليمين إلى اليسار
settings-browsing-page-layout = الصفحات في الأرشيفات
settings-browsing-page-layout-help = عرض صفحة واحدة أو صفحتين متجاورتين؛ يُعرض الغلاف وحده دائمًا.
settings-browsing-page-layout-auto = حسب الأرشيف
settings-browsing-page-layout-single = صفحة واحدة
settings-browsing-page-layout-spreads = صفحتان متقابلتان

## Pages of multi-image files (ICNS sizes, texture layers, faces and slices, TIFF pages) (machine-drafted, needs native review)
palette-next-page = الصفحة التالية
palette-prev-page = الصفحة السابقة
palette-first-page = الصفحة الأولى
palette-last-page = الصفحة الأخيرة
disabled-one-page = لا تحتوي الصورة إلا على صفحة واحدة
page-indicator = الصفحة { $index } من { $count }
page-indicator-labelled = { $page }، { $label }
page-label-icon = { $width }×{ $height }
page-label-icon-density = { $width }×{ $height } @{ $density }x
page-label-layer = الطبقة { $index }
page-label-face = الوجه { $face }
page-label-sphere = خريطة كروية
page-label-slice = المقطع { $index }
page-label-join = { $a } · { $b }
page-label-slide = الشريحة
page-label-slide-label = الملصق
page-label-slide-macro = نظرة عامة
page-label-slide-thumbnail = صورة مصغّرة
# machine-drafted, needs native review
page-label-thermal = صورة حرارية
page-label-visible = صورة مرئية
# machine-drafted, needs native review
page-label-left-view = المنظر الأيسر
# machine-drafted, needs native review
page-label-right-view = المنظر الأيمن
# machine-drafted, needs native review
# An object of a CERN ROOT file in the page picker: its path in the file and its class, e.g.
# "run1/hits (TH2D)".
page-label-object = { $name } ({ $class })
# machine-drafted, needs native review
# A view of one step of a circuit board job ("panel · Top").
page-label-board-step = { $step } · { $view }
# machine-drafted, needs native review
# A focal plane and a time point of a remote microscopy image (OMERO), counted from 1.
page-label-z = Z { $index } من { $count }
page-label-t = T { $index } من { $count }
# machine-drafted, needs native review
# Views of a remote-sensing product (an ENVI cube, a Sentinel-2 product) in the page picker.
page-label-product-composite = النطاقات الافتراضية
page-label-product-bands = جميع النطاقات
page-label-product-true-colour = الألوان الحقيقية
page-label-product-false-colour = الألوان الزائفة (الأشعة تحت الحمراء القريبة)
page-label-product-swir = الأشعة تحت الحمراء قصيرة الموجة
page-label-product-agriculture = الزراعة
page-label-product-scene-class = تصنيف المشهد
page-label-product-cloud = احتمالية السحب
# A product view at a pixel size, e.g. "True colour, 10 m".
page-label-product-at = { $view }، { $metres } m
title-file-page = { $file } — { $page }
info-page-group = الصفحة
info-page-position = الموضع
info-page-contents = المحتوى

# machine-drafted, needs native review
# OpenColorIO (Settings › Colour): why scene-linear images are shown without it.
ocio-failed = لا يُستخدم OCIO: { $reason }
ocio-builtin-config = يشير ‎$OCIO إلى إعداد مضمّن (ocio://)؛ احفظه في ملف ثم اختر ذلك الملف.

## Page labels for icons and collections (machine-drafted, needs native review)
page-label-size = { $width }×{ $height }
page-label-size-bits = { $width }×{ $height } · { $bits } بت

## Page labels for Photoshop layers (machine-drafted, needs native review)
page-label-merged = الصورة المدمجة
page-label-from-layers = مبنية من الطبقات
page-label-layer-hidden = { $name } (مخفية)
page-label-layer-unnamed = طبقة بلا اسم { $index }
