# machine-drafted, needs native review
## لوحة الدليل، وتقارير الأعطال، وحزمة التشخيص، والتحقق من التحديثات، وحول. العربية.
## الأسلوب: قصير وواضح وبلا حشو؛ فكرة واحدة في كل جملة.

## لوحة الأوامر

palette-help-manual = مساعدة
palette-help-manual-help = فتح الدليل على صفحة ما يظهر على الشاشة.
palette-save-diagnostics = حفظ حزمة التشخيص…
palette-save-diagnostics-help = حفظ تقارير الأعطال والسجل وتفاصيل النظام في ملف zip. لا يُرسَل شيء.
# عنصر في اللوحة يفتح قسمًا من الدليل؛ { $title } صفحة أو «صفحة › قسم».
palette-help-topic = مساعدة: { $title }
palette-help-topic-help = فتح هذا القسم من الدليل.
# تحت أمر غير متاح في اللوحة.
palette-disabled-help = غير متاح الآن. اضغط Enter لمعرفة السبب.

## لوحة المساعدة

help-title = مساعدة
help-search-placeholder = البحث في الدليل
help-contents = المحتويات
help-back = رجوع
help-forward = تقدّم
help-no-results = لم يُعثر على شيء لـ «{ $query }».
help-results = { $count ->
    [zero] لم يُعثر على أي قسم
    [one] عُثر على قسم واحد
    [two] عُثر على قسمين
    [few] عُثر على { $count } أقسام
    [many] عُثر على { $count } قسمًا
   *[other] عُثر على { $count } قسم
}
help-section-start = البدء
help-section-modes = العرض والأوضاع
help-section-tasks = كيف…
help-section-reference = مرجع
help-keys-unbound = غير معيّن
# أمر مذكور في الدليل بلا مفتاح؛ يُشغَّل من اللوحة.
help-key-unbound = { $name } (اللوحة)
help-keys-command = الأمر
help-keys-keys = المفاتيح
help-commands = كل أمر موجود في لوحة الأوامر ({ $key }). اكتب جزءًا من اسمه.
help-commands-no-key = كل أمر موجود في لوحة الأوامر. اكتب جزءًا من اسمه.
help-link = انتقل إلى { $title }
help-open = مساعدة
# اسم اللوحة لقارئات الشاشة؛ { $page } الصفحة المعروضة.
help-label = مساعدة: { $page }
help-disabled-why = لماذا؟

## تقرير العطل المعروض عند التشغيل التالي

crash-title = أُغلق { -app-name } بشكل غير متوقع
crash-body = حُفظ تقرير على هذا الحاسوب. لم يُرسَل شيء.
crash-view = عرض التقرير
crash-hide = إخفاء التقرير
crash-copy = نسخ
crash-copied = نُسخ التقرير
crash-save-bundle = حفظ حزمة التشخيص…
crash-dismiss = تجاهل

## حزمة التشخيص

diag-title = حفظ حزمة التشخيص
diag-intro = ملف zip لإرفاقه ببلاغ خطأ. يُحفظ على هذا الحاسوب. لا يُرسَل شيء.
diag-contents = المحتويات
diag-include-names = تضمين أسماء الملفات
diag-include-names-help = عند الإيقاف: تُستبدل الأسماء بـ file-1.jpg وdir-1 وهكذا.
diag-privacy = يظهر مجلد المنزل على هيئة ~ واسم المستخدم على هيئة <user>. لا تُضمَّن كلمات المرور وعناوين الخوادم والصور أبدًا.
diag-where = الإبلاغ عن مشكلة: { $issues } · { $email }
diag-save = حفظ…
diag-cancel = إلغاء
diag-saved = حُفظت حزمة التشخيص: { $path }
diag-failed = تعذّر حفظ حزمة التشخيص: { $reason }
diag-entry-summary = الإصدار والنظام ووحدة GPU والتعريف
diag-entry-crash = آخر تقرير عطل
diag-entry-crashes = تقارير أعطال سابقة ({ $count })
diag-entry-log = آخر أسطر السجل: { $count }
diag-entry-settings = الإعدادات، بلا كلمات مرور أو عناوين
diag-entry-gpu = تفاصيل GPU والشاشة
diag-entry-readme = ما هذه الحزمة
diag-size = { $size }
# ملف README.txt داخل الحزمة.
diag-readme =
    حزمة تشخيص { -app-name }.

    تحتوي ما يساعد على إيجاد المشكلة: الإصدار وتفاصيل النظام، وتقارير الأعطال، ونهاية السجل، والإعدادات.

    يظهر مجلد المنزل على هيئة ~ واسم المستخدم على هيئة <user>. ما لم تختر تضمينها، تُستبدل أسماء الملفات والمجلدات بـ file-1 وdir-1 وهكذا. تُزال كلمات المرور وعناوين الخوادم.

    لم يُرسَل شيء. أنت من يقرّر مشاركة هذا الملف أو لا.

## التحديثات (الإعدادات)

settings-section-updates = التحديثات
settings-updates-check = التحقق من التحديثات
settings-updates-check-help = مرة في اليوم، السؤال عن وجود إصدار جديد. لا يحمل الطلب إلا رقم الإصدار: بلا معرّف وبلا ملف تعريف ارتباط. عند الإيقاف: لا يتصل { -app-name } أبدًا من أجل التحديثات.
settings-updates-channel = القناة
settings-updates-channel-help = مستقر: الإصدارات فقط. تجريبي: والإصدارات التجريبية أيضًا.
settings-updates-channel-stable = مستقر
settings-updates-channel-beta = تجريبي
settings-updates-check-now = تحقّق الآن
settings-updates-check-now-help = التحقق مرة واحدة الآن.
update-available = الإصدار { $version } متاح
update-notes = ملاحظات الإصدار
update-download = تنزيل
update-none = { -app-name } محدَّث.
update-checking = جارٍ التحقق من التحديثات…
update-failed = تعذّر التحقق من التحديثات: { $reason }
update-last-checked = آخر تحقق { $when }

## حول

about-build = البناء { $commit } · { $profile } · { $target }
about-build-no-commit = البناء { $profile } · { $target }
about-help = مساعدة
about-diagnostics = حفظ حزمة التشخيص…
# machine-drafted, needs native review
about-renderer = معلومات العارض…
about-licences-count = { $count ->
    [zero] لا مكونات من جهات خارجية
    [one] مكوّن واحد من جهة خارجية
    [two] مكوّنان من جهات خارجية
    [few] { $count } مكونات من جهات خارجية
    [many] { $count } مكوّنًا من جهات خارجية
   *[other] { $count } مكوّن من جهات خارجية
}
about-licence-group = { $licence } ({ $count })
about-contact = المشكلات: { $issues } · الدعم: { $email }
