# Develop: tone zones, batch conflicts, RAW+JPEG pairs, CMYK sources.
# machine-drafted, needs native review

develop-tone-zones = مناطق الدرجات
develop-tone-zone = { $ev } EV
develop-tone-zones-adjust = ضبط المناطق على الصورة
develop-tone-zones-done = إنهاء ضبط المناطق
develop-tone-zones-adjusting = اسحب لأعلى أو لأسفل على الصورة لرفع تلك الدرجة أو خفضها. Esc للإنهاء.
develop-tone-zone-picked = المنطقة { $zone }
develop-batch-conflict-title = { $count ->
    [zero] لم تتغير أي صورة خارج Specular
    [one] تغيرت صورة واحدة خارج Specular
    [two] تغيرت صورتان خارج Specular
    [few] تغيرت { $count } صور خارج Specular
    [many] تغيرت { $count } صورة خارج Specular
   *[other] تغيرت { $count } صورة خارج Specular
}
develop-batch-conflict-detail = ملفاتها الجانبية أحدث من النسخ في المكتبة.
develop-batch-conflict-keep = إبقاء: يُوضع اللصق فوق تغييرات الملف.
develop-batch-conflict-replace = استبدال: يُوضع اللصق على نسخة المكتبة. تضيع تغييرات الملف.
develop-batch-conflict-skip-detail = تخطي: تبقى تلك الصور كما هي.
develop-batch-conflict-file = إبقاء تغييرات الملف
develop-batch-conflict-library = استبدالها
develop-batch-conflict-skip = تخطي تلك الصور
develop-pair-raw = يجري تحميض { $file }، ملف RAW لهذا الزوج
develop-source-cmyk = تُحرَّر صورة CMYK هذه بنظام RGB. حُوّلت باستخدام ملف تعريفها.
