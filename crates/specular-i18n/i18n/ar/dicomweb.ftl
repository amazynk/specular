# Specular — DICOMweb servers, study search and streamed series (DESIGN.md §12.8). العربية.

palette-dicom-servers = خوادم DICOMweb…

dicomweb-title = خوادم DICOMweb
dicomweb-servers-intro = لا يُبحث في الدراسات على الخادم ولا تُفتح إلا عندما تطلب ذلك. لا يُرسل شيء قبل ذلك.
dicomweb-servers = الخوادم
dicomweb-servers-empty = لا توجد خوادم بعد. أضف خادمًا بعنوان DICOMweb الخاص به.
dicomweb-session-only = تُحفظ كلمات المرور والرموز المميزة فقط حتى إغلاق Specular.
dicomweb-add = إضافة…
dicomweb-edit = تعديل…
dicomweb-remove = إزالة
dicomweb-close = إغلاق
dicomweb-browse = استعراض
dicomweb-back = رجوع
dicomweb-save = حفظ
dicomweb-search = بحث
dicomweb-stop = إيقاف
dicomweb-removed = تمت الإزالة: { $server }
# machine-drafted, needs native review
dicomweb-remove-ask = إزالة «{ $server }» من الخوادم المحفوظة؟
dicomweb-remove-detail = لا يُفقد شيء على القرص. تُنسى كلمة المرور أو الرمز الخاص به لهذه الجلسة.
# خوادم يُتصل بها دون تشفير (http://).
dicomweb-not-encrypted = غير مشفّر

# صفحة إضافة خادم أو تعديله.
dicomweb-add-title = إضافة خادم
dicomweb-edit-title = إعدادات الخادم
dicomweb-field-name = الاسم
dicomweb-hint-name = اختياري
dicomweb-field-url = العنوان
dicomweb-hint-url = https://pacs.example.org/dicom-web
dicomweb-field-auth = تسجيل الدخول
dicomweb-auth-none = دون تسجيل دخول
dicomweb-auth-basic = اسم المستخدم وكلمة المرور
dicomweb-auth-bearer = رمز الوصول
dicomweb-field-user = اسم المستخدم
dicomweb-field-password = كلمة المرور
dicomweb-field-token = رمز الوصول
dicomweb-hint-secret-kept = اتركه فارغًا للإبقاء على القيمة المُدخلة
dicomweb-secret-session = تُحفظ كلمة المرور أو الرمز فقط حتى إغلاق Specular، ولا تُحفظ على القرص أبدًا.
dicomweb-secret-needed = أدخل كلمة المرور أو الرمز مرة أخرى: تُحفظ فقط حتى إغلاق Specular.
dicomweb-http-warning = هذا العنوان غير مشفّر: ستُرسل كلمة المرور أو الرمز نصًا واضحًا.
dicomweb-url-empty = أدخل عنوان DICOMweb للخادم.
dicomweb-url-scheme = يجب أن يبدأ العنوان بـ https:// أو http://.
dicomweb-url-host = لا يحتوي العنوان على اسم خادم.
dicomweb-url-userinfo = ضع اسم المستخدم وكلمة المرور في حقول تسجيل الدخول، لا في العنوان.
dicomweb-save-failed = تعذّر حفظ قائمة الخوادم: { $reason }

# صفحة البحث.
dicomweb-search-title = البحث عن دراسات
dicomweb-field-patient-name = اسم المريض
dicomweb-field-patient-id = معرّف المريض
dicomweb-field-date-from = من
dicomweb-field-date-to = إلى
dicomweb-hint-date = YYYYMMDD
dicomweb-field-modality = الطريقة التصويرية
dicomweb-hint-modality = CT, MR…
dicomweb-field-accession = رقم الطلب
dicomweb-show-patient = إظهار بيانات المريض
dicomweb-switch-on = تشغيل
dicomweb-switch-off = إيقاف
dicomweb-patient-shown = بيانات المريض ظاهرة
dicomweb-patient-hidden = بيانات المريض مخفية
dicomweb-studies = الدراسات
dicomweb-search-hint = املأ أيًا من الحقول وابحث.
dicomweb-no-studies = لا توجد دراسات مطابقة.
dicomweb-searching = جارٍ البحث…
dicomweb-stopped = متوقف
dicomweb-load-more = تحميل المزيد
dicomweb-study-untitled = دراسة بلا وصف
dicomweb-series-untitled = سلسلة بلا وصف
dicomweb-study-counts = السلاسل: { $series }، الصور: { $images }
dicomweb-series-images = الصور: { $count }
dicomweb-patient-id = المعرّف { $id }
dicomweb-accession = الطلب { $number }
dicomweb-found = الدراسات التي عُثر عليها: { $count }
dicomweb-series-found = السلاسل: { $count }

# أخطاء الاتصال بالخادم.
dicomweb-error-offline = تعذّر الوصول إلى { $server }. تحقق من الشبكة والعنوان.
dicomweb-error-auth = رفض الخادم اسم المستخدم أو كلمة المرور أو الرمز.
dicomweb-error-not-found = لا توجد خدمة DICOMweb على هذا العنوان.
dicomweb-error-other = تعذّر على الخادم الرد: { $reason }

# سلسلة تُبث إلى العارض.
dicomweb-loading = جارٍ تحميل { $series }…
dicomweb-loading-progress = جارٍ تحميل { $series }: { $done } من { $total }
dicomweb-loaded = { $series }: الصور المحمّلة: { $total }
dicomweb-loaded-failed = الصور التي تعذّر تحميلها: { $failed } من { $total }
dicomweb-offline-cached = تعذّر الوصول إلى { $server }: تُعرض الصور المحفوظة سابقًا.
dicomweb-cache-failed = تعذّر فتح مجلد التنزيلات: { $reason }
