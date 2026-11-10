# Specular — معارض العملاء: مجلد HTML ثابت من الصور، وقراءة اختيارات العميل مرة أخرى.

# machine-drafted, needs native review

palette-client-gallery = معرض للعميل…
palette-client-gallery-help = كتابة الصور كمجلد صفحة ويب لعميل، مع اختيارات يمكنه إرسالها مرة أخرى.
palette-import-client-picks = استيراد اختيارات العميل…
palette-import-client-picks-help = وسم الصور التي اختارها العميل في معرضه.

webgallery-heading = معرض للعميل
webgallery-subtitle = { $count ->
    [zero] لا صور
    [one] صورة واحدة
    [two] صورتان
    [few] { $count } صور
    [many] { $count } صورة
   *[other] { $count } صورة
}
webgallery-title-label = العنوان
webgallery-title-default = صور تجريبية
webgallery-watermark-label = نص العلامة المائية (فارغ: بلا علامة)
webgallery-source = الصور
webgallery-source-current = هذه الصورة
webgallery-source-selection = التحديد
webgallery-source-album = الألبوم ”{ $name }“
webgallery-source-search = نتائج البحث
webgallery-source-folder = هذا المجلد
webgallery-source-picks = المختارة
webgallery-source-value = { $what } ({ $count })
webgallery-theme = السمة
webgallery-theme-light = فاتحة
webgallery-theme-dark = داكنة
webgallery-theme-minimal = بسيطة
webgallery-size = الضلع الأطول
webgallery-size-value = { $px } بكسل
webgallery-format = الصيغة
webgallery-captions = أسفل كل صورة
webgallery-captions-off = لا شيء
webgallery-captions-name = اسم الملف
webgallery-captions-title = العنوان
webgallery-captions-all = اسم الملف والعنوان والتعليق
webgallery-exif = إظهار الكاميرا والتعريض
webgallery-location = الإبقاء على الموقع (GPS) في الصور
webgallery-note-private = لا يمكن حماية معرض ثابت بكلمة مرور.
webgallery-note-share = كل من لديه المجلد أو عنوانه يرى كل الصور. شارك الرابط مع عميلك فقط، أو استخدم التحكم في الوصول لدى مستضيف موقعك.
webgallery-note-picks = يختار العملاء بقلب. تبقى الاختيارات في متصفحهم حتى يرسلوا إليك القائمة.
webgallery-write = كتابة المعرض…
webgallery-busy = جارٍ كتابة { $done } من { $total }…
webgallery-stop = إيقاف
webgallery-done = { $count ->
    [zero] كُتب المعرض: لا صور في { $folder }
    [one] كُتب المعرض: صورة واحدة في { $folder }
    [two] كُتب المعرض: صورتان في { $folder }
    [few] كُتب المعرض: { $count } صور في { $folder }
    [many] كُتب المعرض: { $count } صورة في { $folder }
   *[other] كُتب المعرض: { $count } صورة في { $folder }
}
webgallery-done-failed = { $failed ->
    [zero] لم تتعذر كتابة أي صورة.
    [one] تعذرت كتابة صورة واحدة.
    [two] تعذرت كتابة صورتين.
    [few] تعذرت كتابة { $failed } صور.
    [many] تعذرت كتابة { $failed } صورة.
   *[other] تعذرت كتابة { $failed } صورة.
}
webgallery-stopped = أُوقف المعرض. لم يُحتفظ بشيء.
webgallery-failed = لم يُكتب المعرض: { $reason }
webgallery-nothing = تعذرت كتابة أي صورة.

webgallery-page-count = { $count ->
    [zero] لا صور
    [one] صورة واحدة
    [two] صورتان
    [few] { $count } صور
    [many] { $count } صورة
   *[other] { $count } صورة
}
webgallery-page-hint = المس القلب على الصور التي تريدها. تبقى اختياراتك في هذا المتصفح حتى ترسلها.
webgallery-page-picked = المختارة: {"{"}n{"}"}
webgallery-page-pick = اختيار
webgallery-page-copy = نسخ اختياراتي
webgallery-page-copied = نُسخت. الصق القائمة في رسالة.
webgallery-page-copy-by-hand = انسخ القائمة المحددة والصقها في رسالة.
webgallery-page-download-txt = تنزيل picks.txt
webgallery-page-download-json = تنزيل picks.json
webgallery-page-clear = مسح الاختيارات
webgallery-page-clear-ask = هل تريد مسح كل اختياراتك في هذا المتصفح؟
webgallery-page-no-storage = لن يحتفظ هذا المتصفح باختياراتك. أرسلها قبل إغلاق الصفحة.
webgallery-page-noscript = يتطلب الاختيار JavaScript. ما زالت الصور تُفتح.
webgallery-page-previous = السابقة
webgallery-page-next = التالية
webgallery-page-close = إغلاق
webgallery-page-position = {"{"}i{"}"} من {"{"}n{"}"}
webgallery-page-viewer = عارض الصور
webgallery-page-picks = اختياراتك

clientpicks-heading = استيراد اختيارات العميل
clientpicks-subtitle = { $count ->
    [zero] مطابقة مع لا صور معروضة
    [one] مطابقة مع صورة واحدة معروضة
    [two] مطابقة مع الصورتين المعروضتين
    [few] مطابقة مع { $count } صور معروضة
    [many] مطابقة مع { $count } صورة معروضة
   *[other] مطابقة مع { $count } صورة معروضة
}
clientpicks-paste-label = الأسماء (الصق قائمة العميل)
clientpicks-choose = اختيار ملف الاختيارات…
clientpicks-from-file = من { $name }
clientpicks-apply-as = وسمها
clientpicks-as-pick = كمختارة
clientpicks-as-stars = { $stars ->
    [zero] بلا نجوم
    [one] نجمة واحدة
    [two] نجمتان
    [few] { $stars } نجوم
    [many] { $stars } نجمة
   *[other] { $stars } نجمة
}
clientpicks-as-label = التسمية { $label }
clientpicks-empty = الصق القائمة التي أرسلها العميل، أو اختر ملف picks.txt أو picks.json الخاص به.
clientpicks-matched = { $matched } من { $total } أسماء تطابق صورًا هنا.
clientpicks-missing = غير موجودة: { $names }
clientpicks-ambiguous = في أكثر من مجلد، تُركت كما هي: { $names }
clientpicks-more = { $names } و{ $more } أخرى
clientpicks-apply = تطبيق
clientpicks-applied = { $count ->
    [zero] لم تُوسم أي صورة.
    [one] وُسمت صورة واحدة.
    [two] وُسمت صورتان.
    [few] وُسمت { $count } صور.
    [many] وُسمت { $count } صورة.
   *[other] وُسمت { $count } صورة.
}
clientpicks-unchanged = كانت تلك الصور موسومة من قبل.
clientpicks-read-failed = تتعذر قراءة { $name }: { $reason }
