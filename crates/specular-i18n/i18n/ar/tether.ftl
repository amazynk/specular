# Specular — tethered capture: a camera on USB, its shots downloaded as they are taken. العربية. machine-drafted, needs native review

palette-tether = التصوير الموصول…
palette-tether-capture = التقاط صورة (موصول)

tether-title = التصوير الموصول
tether-close = إغلاق التصوير الموصول
tether-default-folder = التصوير الموصول

## Cameras
tether-none = لم يُعثر على كاميرا.
tether-none-hint = صِل الكاميرا بكبل USB وشغّلها. إن كان لها وضع USB فاختر PTP أو الاتصال بالحاسوب.
tether-look-again = البحث مجددًا
tether-connect = اتصال
tether-next-camera = الكاميرا التالية
tether-connecting = جارٍ الاتصال…
tether-connected = اتصلت { $camera }
tether-disconnect = قطع الاتصال
tether-try-again = إعادة المحاولة
tether-gone = انقطع اتصال الكاميرا.
tether-not-connected = لا توجد كاميرا متصلة. افتح التصوير الموصول أولًا.

## Why a camera can't be opened
tether-permission = لا يملك هذا المستخدم إذنًا بفتح جهاز USB الخاص بالكاميرا { $node }.
tether-permission-rule = تسمح بذلك قاعدة udev. بصفتك مديرًا، ضع هذا السطر في { $file }:
tether-permission-reload = ثم شغّل "sudo udevadm control --reload" وأعد توصيل الكاميرا.
tether-copy-rule = نسخ القاعدة
tether-busy-other = برنامج آخر يستخدم الكاميرا.
tether-busy-named = الكاميرا قيد الاستخدام من { $programs }.
tether-busy-hint = أغلق ذلك البرنامج أو افصل الكاميرا في مدير الملفات، ثم أعد المحاولة.
tether-driver = لا يسمح مشغّل النظام لهذه الكاميرا للبرامج الأخرى باستخدامها.

## The camera
tether-battery = البطارية { PERCENT($percent) }
tether-card = { $card }: { $free } متاحة
tether-card-shots = { $card }: { $free } متاحة، { $shots } لقطة
tether-no-card = لا توجد بطاقة ذاكرة.
tether-no-settings = لا تُبلغ الكاميرا عن إعدادات التعريض.
tether-no-remote = لا يمكن تشغيل هذه الكاميرا من هنا. تُنزَّل اللقطات الملتقطة بزر الغالق فيها.
tether-capture = التقاط
tether-step-down = { $setting }: خفض ({ $value })
tether-step-up = { $setting }: رفع ({ $value })

## Exposure settings
tether-prop-mode = الوضع
tether-prop-shutter = سرعة الغالق
tether-prop-aperture = فتحة العدسة
tether-prop-iso = ISO
tether-prop-bias = تعويض التعريض
tether-prop-white-balance = توازن اللون الأبيض
tether-auto = تلقائي
tether-bulb = تعريض طويل (B)
tether-camera-value = إعداد الكاميرا { $value }
tether-wb-manual = يدوي
tether-wb-one-push = قياس لمرة واحدة
tether-wb-daylight = ضوء النهار
tether-wb-fluorescent = فلورسنت
tether-wb-tungsten = تنغستن
tether-wb-flash = فلاش
tether-mode-manual = يدوي
tether-mode-program = برنامج
tether-mode-aperture = أولوية الفتحة
tether-mode-shutter = أولوية الغالق
tether-mode-creative = إبداعي
tether-mode-action = رياضة
tether-mode-portrait = بورتريه

## Where shots go
tether-folder = المجلد
tether-names = الأسماء
tether-names-camera = أسماء الكاميرا
tether-arrival = نمط Develop
tether-choose-folder = المجلد…
tether-choose-style = النمط…
tether-downloading = جارٍ التنزيل… { PERCENT($percent) }
tether-saved = حُفظت { $name }
tether-saved-count = { $count ->
    [zero] لم تُحفظ أي لقطة
    [one] حُفظت لقطة واحدة
    [two] حُفظت لقطتان
    [few] حُفظت { $count } لقطات
    [many] حُفظت { $count } لقطة
   *[other] حُفظت { $count } لقطة
}

## Failures
tether-busy = الكاميرا مشغولة. أعد المحاولة.
tether-card-full = بطاقة الذاكرة ممتلئة.
tether-set-refused = لم تقبل الكاميرا هذا الإعداد. تحقق من قرص الأوضاع.
tether-disk = تعذّر حفظ اللقطة: { $error }
tether-error = خطأ في الكاميرا: { $error }

## Settings › Cull mode
settings-cull-tether-folder = مجلد التصوير الموصول
settings-cull-tether-folder-help = مكان حفظ لقطات الكاميرا الموصولة بكبل.
settings-cull-tether-folder-unset = الصور › التصوير الموصول
settings-cull-tether-rename = أسماء ملفات التصوير الموصول
settings-cull-tether-rename-help = أسماء جديدة بالرموز نفسها المستخدمة في إعادة التسمية عند الاستيراد. يُبقي الحقل الفارغ أسماء الكاميرا.
