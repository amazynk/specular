# The metadata editor: IPTC fields of the selected photos (gallery, Library, cull mode and
# the viewer).

palette-edit-metadata = تحرير البيانات الوصفية
palette-edit-metadata-help = تحرير العنوان والتعليق والمنشئ والحقوق والمواقع للصور المحددة.
metaedit-menu = تحرير البيانات الوصفية…
metaedit-title = البيانات الوصفية
metaedit-close = إغلاق البيانات الوصفية
metaedit-photos = { $count ->
    [zero] { $count } صورة
    [one] صورة واحدة
    [two] صورتان
    [few] { $count } صور
    [many] { $count } صورةً
   *[other] { $count } صورة
}
metaedit-none-selected = حدد صورًا لتحرير بياناتها الوصفية.
metaedit-reading = جارٍ قراءة البيانات الوصفية…
metaedit-mixed = مختلفة: اكتب لتعيينها على الكل
metaedit-clearing = مُسحت من كل الصور. اضغط Esc للإبقاء عليها.
metaedit-keywords-others = لبعضها المزيد
metaedit-saved = حُفظ { $field }.
metaedit-cleared = مُسح { $field }.
metaedit-write-failed = { $count ->
    [zero] لم تُحفظ البيانات الوصفية لـ{ $count } صورة: { $error }
    [one] لم تُحفظ البيانات الوصفية لصورة واحدة: { $error }
    [two] لم تُحفظ البيانات الوصفية لصورتين: { $error }
    [few] لم تُحفظ البيانات الوصفية لـ{ $count } صور: { $error }
    [many] لم تُحفظ البيانات الوصفية لـ{ $count } صورةً: { $error }
   *[other] لم تُحفظ البيانات الوصفية لـ{ $count } صورة: { $error }
}
metaedit-writes-sidecars = تُحفظ في ملفات XMP مرافقة.
metaedit-writes-files = تُحفظ في ملفات JPEG وTIFF وDNG، وتُنشأ للملفات الأخرى ملفات XMP مرافقة.

metaedit-group-templates = القوالب
metaedit-group-description = الوصف
metaedit-group-creator = المنشئ
metaedit-group-rights = الحقوق
metaedit-group-shown = الموقع الظاهر
metaedit-group-created = موقع الإنشاء
metaedit-group-workflow = سير العمل
metaedit-templates-none = لا قوالب بعد. يملأ القالب الحقول التي تستخدمها كثيرًا.

metaedit-field-title = العنوان
metaedit-field-headline = العنوان الرئيسي
metaedit-field-description = التعليق
metaedit-field-keywords = الكلمات المفتاحية (مفصولة بـ ;)
metaedit-field-persons-shown = الأشخاص الظاهرون (مفصولون بـ ;)
metaedit-field-event = الحدث
metaedit-field-creator = المنشئون (مفصولون بـ ;)
metaedit-field-creator-job-title = المسمى الوظيفي للمنشئ
metaedit-field-contact-address = العنوان البريدي
metaedit-field-contact-city = المدينة
metaedit-field-contact-region = الولاية أو المقاطعة
metaedit-field-contact-postal-code = الرمز البريدي
metaedit-field-contact-country = البلد
metaedit-field-contact-phone = الهاتف
metaedit-field-contact-email = البريد الإلكتروني
metaedit-field-contact-url = الموقع الإلكتروني
metaedit-field-copyright = إشعار حقوق النشر
metaedit-field-credit = سطر الإسناد
metaedit-field-source = المصدر
metaedit-field-usage-terms = شروط الاستخدام
metaedit-field-shown-sublocation = الموقع الفرعي
metaedit-field-shown-city = المدينة
metaedit-field-shown-state = الولاية أو المقاطعة
metaedit-field-shown-country = البلد
metaedit-field-shown-country-code = رمز البلد
metaedit-field-created-sublocation = الموقع الفرعي
metaedit-field-created-city = المدينة
metaedit-field-created-state = الولاية أو المقاطعة
metaedit-field-created-country = البلد
metaedit-field-created-country-code = رمز البلد
metaedit-field-instructions = التعليمات
metaedit-field-job-id = معرّف المهمة

metaedit-template-new = قالب جديد
metaedit-template-edit = تحرير
metaedit-template-remove = إزالة
metaedit-template-done = تم
metaedit-template-name = اسم القالب
metaedit-template-default-name = القالب { $n }
metaedit-template-title = القالب
metaedit-template-hint = تُترك الحقول الفارغة كما هي.
metaedit-template-replace-keywords = استبدال الكلمات المفتاحية للصور
metaedit-template-applied = { $count ->
    [zero] طُبّق «{ $name }» على { $count } صورة.
    [one] طُبّق «{ $name }» على صورة واحدة.
    [two] طُبّق «{ $name }» على صورتين.
    [few] طُبّق «{ $name }» على { $count } صور.
    [many] طُبّق «{ $name }» على { $count } صورةً.
   *[other] طُبّق «{ $name }» على { $count } صورة.
}
metaedit-template-remove-ask = إزالة القالب «{ $name }»؟
metaedit-template-remove-keep = تحتفظ الصور ببياناتها الوصفية.
metaedit-template-remove-yes = إزالة
metaedit-template-removed = أُزيل القالب «{ $name }».

app-info-edit-metadata = تحرير البيانات الوصفية…

settings-gallery-metadata-into-files = كتابة البيانات الوصفية في ملفات الصور
settings-gallery-metadata-into-files-help = يكتب محرر البيانات الوصفية أيضًا في ملفات JPEG وTIFF وDNG دون تغيير أي شيء آخر فيها. عند الإيقاف: ملفات XMP مرافقة فقط.
