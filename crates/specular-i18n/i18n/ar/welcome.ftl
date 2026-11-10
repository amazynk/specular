# machine-drafted, needs native review
# Specular — شاشة البدء (عند فتح Specular دون ملف وعند إغلاق آخر صورة)، وبطاقات «جرّب هذه»،
# والتلميحات التي تظهر مرة واحدة. العربية.

## شاشة البدء

welcome-title = { -app-name }
welcome-tagline = عارض سريع ودقيق للصور الفوتوغرافية وللصور التقنية والعلمية والطبية.
welcome-open = فتح ملف…
welcome-open-about = صورة أو مستند أو ملف بيانات.
welcome-browse = تصفح مجلد…
welcome-browse-about = اختر مجلدًا واعرضه كمعرض.
welcome-pictures = صورك
welcome-pictures-about = كل الصور في مكتبتك، حسب التاريخ والمكان.
welcome-pictures-setup-title = إعداد مكتبتك…
welcome-pictures-setup = بدءًا من مجلد { $folder }.
welcome-pictures-ask = هل تريد إضافة مجلد { $folder } إلى المكتبة؟
welcome-pictures-add = إضافة المجلد
welcome-drop = يمكنك أيضًا إفلات الملفات أو المجلدات في أي مكان من هذه النافذة.
welcome-recent = المجلدات الأخيرة
welcome-recent-name = { $name }، في { $parent }
welcome-tips = جرّب هذه
welcome-tips-hide = إخفاء هذه التلميحات
welcome-tips-hidden = أُخفيت التلميحات. يعيدها «إظهار الترحيب» في لوحة الأوامر.

## جرّب هذه: اسم وسطر واحد لكل بطاقة. المفتاح بجانب الاسم هو اختصار المستخدم نفسه.

welcome-tip-gallery = المعرض
welcome-tip-gallery-about = شاهد مجلدًا كاملًا دفعة واحدة.
welcome-tip-compare = المقارنة
welcome-tip-compare-about = حدد من صورتين إلى أربع صور في المعرض لعرضها جنبًا إلى جنب.
welcome-tip-cull = وضع الفرز
welcome-tip-cull-about = اختر صور جلسة وارفضها وقيّمها بمفتاح واحد لكل منها.
welcome-tip-slideshow = عرض الشرائح
welcome-tip-slideshow-about = اعرض مجلدًا بملء الشاشة، صورة تلو الأخرى.
welcome-tip-palette = لوحة الأوامر
welcome-tip-palette-about = اعثر على أي أمر أو إعداد باسمه.
welcome-tip-studies = الدراسات
welcome-tip-studies-about = المرضى والدراسات والسلاسل في قرص أو مجلد DICOM.
welcome-tip-dicom-servers = خوادم DICOMweb
welcome-tip-dicom-servers-about = تصفح الدراسات وافتحها من نظام PACS.
welcome-tip-dicom-tags = وسوم DICOM
welcome-tip-dicom-tags-about = كل وسوم الصورة، مع البحث.
welcome-tip-window-level = النافذة والمستوى
welcome-tip-window-level-about = اسحب لضبط تباين صورة DICOM.
welcome-tip-omero-servers = خوادم OMERO
welcome-tip-omero-servers-about = تصفح مشاريع الفحص المجهري على OMERO وIDR.
welcome-tip-stretch = التمديد
welcome-tip-stretch-about = أظهر التفاصيل الخافتة في البيانات الفلكية والعلمية.
welcome-tip-drawing-layers = طبقات الرسم
welcome-tip-drawing-layers-about = أظهر طبقات الرسم التقني أو أخفها.
welcome-tip-needs-image = افتح صورة أولًا، ثم اضغط { $key }.
welcome-tip-needs-gallery = في المعرض، حدد من صورتين إلى أربع صور، ثم اضغط { $key }.
welcome-tip-key = الاختصار: { $key }

## تلميحات لمرة واحدة: كل منها مرة، بهدوء، وواحد على الأكثر في كل تشغيل. الإعدادات › عام ›
## إظهار التلميحات يوقفها كلها.

hint-gallery = اضغط { $key } لعرض هذا المجلد كمعرض.
hint-gallery-first = حدد من صورتين إلى أربع صور واضغط { $compare } لمقارنتها، أو { $cull } لفرز المجلد.
hint-raw = مجلد صور خام: اضغط { $key } لفرزه (اختيار ورفض وتقييم).
hint-pages =
    { $count ->
        [zero] لا صفحات في هذا الملف. { $next } و{ $prev } يقلّبان الصفحات.
        [one] في هذا الملف صفحة واحدة. { $next } و{ $prev } يقلّبان الصفحات.
        [two] في هذا الملف صفحتان. { $next } و{ $prev } يقلّبان الصفحات.
        [few] في هذا الملف { $count } صفحات. { $next } و{ $prev } يقلّبان الصفحات.
        [many] في هذا الملف { $count } صفحة. { $next } و{ $prev } يقلّبان الصفحات.
       *[other] في هذا الملف { $count } من الصفحات. { $next } و{ $prev } يقلّبان الصفحات.
    }
hint-turn-off = إيقاف التلميحات
hint-turned-off = أُوقفت التلميحات. يعيد تشغيلها الإعدادات › عام › إظهار التلميحات.

## لوحة الأوامر

palette-show-welcome = إظهار الترحيب
palette-show-welcome-help = شاشة البدء، مع فتح وتصفح وصورك وأشياء لتجربتها.
palette-close = إغلاق
palette-close-help = إغلاق الصورة أو المجلد والعودة إلى شاشة البدء. في شاشة البدء: الخروج.

## التشغيل الأول (تحت سؤال الملف الشخصي)

profile-on-start = عند فتح { -app-name } دون ملف:
profile-skip = تخطٍّ
