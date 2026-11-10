# Specular — OMERO servers and the Image Data Resource (IDR), browsed read-only. العربية. machine-drafted, needs native review

palette-omero-servers = خوادم OMERO…

omero-title = خوادم OMERO
omero-servers-intro = لا تُعرض المشاريع والفحوص والصور على الخادم إلا عند تصفحه. لا يُرسل أي شيء قبل ذلك.
omero-servers = الخوادم
omero-servers-empty = لا توجد خوادم بعد. أضف خادمًا بعنوان OMERO.web الخاص به.
omero-session-only = تُحفظ كلمات المرور فقط حتى إغلاق Specular.
omero-add = إضافة…
omero-edit = تحرير…
omero-remove = إزالة
omero-close = إغلاق
omero-browse = تصفح
omero-back = رجوع
omero-save = حفظ
omero-stop = إيقاف
omero-open = فتح
# $server a server's name.
omero-removed = أُزيل { $server }
# Shown with servers reached without encryption (http://).
omero-not-encrypted = غير مشفر
# A server browsed without signing in.
omero-public = عام
# $user the user name a server is signed in with.
omero-signed-in-as = باسم { $user }

# The page that adds or edits a server.
omero-add-title = إضافة خادم
omero-edit-title = إعدادات الخادم
omero-field-name = الاسم
omero-hint-name = اختياري
omero-field-url = العنوان
omero-hint-url = https://omero.example.org
omero-field-user = اسم المستخدم
omero-hint-user = فارغ للبيانات العامة
omero-field-password = كلمة المرور
omero-hint-password-kept = اتركه فارغًا للاحتفاظ بكلمة المرور المُدخلة
omero-password-session = تُحفظ كلمة المرور فقط حتى إغلاق Specular، ولا تُكتب على القرص أبدًا.
omero-password-needed = أدخل كلمة المرور: تُحفظ فقط حتى إغلاق Specular.
omero-http-warning = هذا العنوان غير مشفر: سترسل كلمة المرور بنص صريح.
omero-url-invalid = أدخل عنوان OMERO.web، بدءًا بـ https:// أو http://.
# $reason why the list could not be saved.
omero-save-failed = تعذّر حفظ قائمة الخوادم: { $reason }

# The browse page.
omero-browse-title = تصفح
omero-top = المشاريع والفحوص
omero-datasets = مجموعات البيانات
omero-plates = الصفائح
omero-wells = الآبار
omero-fields = الحقول
omero-images = الصور
omero-kind-project = مشروع
omero-kind-dataset = مجموعة بيانات
omero-kind-image = صورة
omero-kind-screen = فحص
omero-kind-plate = صفيحة
omero-kind-well = بئر
# $well a well's row and column, as B7.
omero-well = البئر { $well }
omero-well-fields = { $count ->
    [zero] لا حقول
    [one] حقل واحد
    [two] حقلان
    [few] { $count } حقول
    [many] { $count } حقلًا
   *[other] { $count } حقل
}
omero-children = { $count ->
    [zero] لا عناصر
    [one] عنصر واحد
    [two] عنصران
    [few] { $count } عناصر
    [many] { $count } عنصرًا
   *[other] { $count } عنصر
}
omero-load-more = تحميل المزيد
omero-loading = جارٍ التحميل…
omero-empty = لا شيء هنا
omero-stopped = أُوقف
omero-found = { $count ->
    [zero] لا عناصر
    [one] عنصر واحد
    [two] عنصران
    [few] { $count } عناصر
    [many] { $count } عنصرًا
   *[other] { $count } عنصر
}
# $reason why the image could not be opened.
omero-open-failed = تعذّر فتح الصورة: { $reason }
# The channel keys would hide the last channel shown.
omero-channel-last = تبقى قناة واحدة على الأقل معروضة
# $server a server's name.
omero-session-ended = انتهت الجلسة على { $server }. أدخل كلمة المرور للمتابعة.

# Errors.
# $server a server's name.
omero-error-offline = تعذّر الوصول إلى { $server }.
omero-error-sign-in = يتطلب هذا الخادم تسجيل الدخول.
omero-error-expired = انتهت الجلسة.
omero-error-login = رُفض اسم المستخدم أو كلمة المرور.
omero-error-forbidden = قد لا يُسمح لحسابك برؤية هذا.
omero-error-not-found = غير موجود على الخادم (ربما أُزيل).
# $code an HTTP status code.
omero-error-status = ردّ الخادم بالخطأ { $code }.
# $reason what was wrong with the answer.
omero-error-malformed = تعذّرت قراءة ردّ الخادم: { $reason }
