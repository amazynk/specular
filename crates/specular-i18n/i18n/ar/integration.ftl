# Integration: open with, user commands, paste (Stage 2).
palette-open-with = فتح باستخدام…
palette-paste = لصق
open-with-placeholder = فتح باستخدام…
open-with-loading = جارٍ البحث عن التطبيقات…
open-with-default = { $name } (الافتراضي)
open-with-choose-other = اختيار تطبيق آخر…
open-with-choose-title = اختيار تطبيق
open-with-failed = تعذّر الفتح باستخدام { $name }: { $reason }
command-missing = لم يُعثر على { $program }
command-failed = تعذّر تشغيل { $name }: { $reason }
command-exited = انتهى { $name } بخطأ: { $status }
paste-nothing = لا يوجد في الحافظة ما يمكن فتحه
paste-failed = تعذّر لصق الصورة: { $reason }
drag-failed = تعذّر سحب الملف: { $reason }
about-storage = الإعدادات: { $settings } · ذاكرة التخزين المؤقت: { $cache }
about-storage-portable = الوضع المحمول: الإعدادات وذاكرة التخزين المؤقت في { $dir }

## Scripting and recipes

palette-copy-view-command = نسخ العرض كأمر صدفة
palette-copy-view-python = نسخ العرض بلغة Python
toast-copied-view-command = نُسخ الأمر
script-copy-no-image = لا توجد صورة لوصفها
recipe-confirm-title = هل تريد تشغيل الوصفة «{ $name }» لأول مرة؟ إنها تشغّل: { $command }
recipe-confirm-run = تشغيل
recipe-no-file = تحتاج الوصفة إلى ملف صورة
recipe-running = جارٍ تشغيل { $name }…
recipe-done = { $name }: { $shapes ->
    [zero] لم يُضف أي شكل
    [one] أُضيف شكل واحد
    [two] أُضيف شكلان
    [few] أُضيفت { $shapes } أشكال
    [many] أُضيف { $shapes } شكلًا
   *[other] أُضيف { $shapes } شكل
}
recipe-failed = فشل { $name }: { $reason }
recipe-timeout = أُوقف بعد { $seconds } ث
recipe-no-output = لم يكتب { $name } الملف { $file }
