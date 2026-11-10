# machine-drafted, needs native review
# Specular — Library (DESIGN §10).

palette-library = المكتبة

## Top bar
library-title = المكتبة
library-locations-button = المواقع

## Locations sidebar
library-locations = المواقع
library-all = كل المواقع
library-add = إضافة موقع…
library-remove = إزالة { $location } من المكتبة
library-offline = غير متصل
library-location-pending = بانتظار الفهرسة: { $count }
library-a11y-location = { $location }, { $count ->
    [zero] لا صور
    [one] صورة واحدة
    [two] صورتان
    [few] { $count } صور
    [many] { $count } صورة
   *[other] { $count } صورة
}
library-a11y-location-offline = { $location }, غير متصل, { $count ->
    [zero] لا صور
    [one] صورة واحدة
    [two] صورتان
    [few] { $count } صور
    [many] { $count } صورة
   *[other] { $count } صورة
}

## Indexing progress
library-walking = الصور التي عُثر عليها: { $count } · { $location }
# machine-drafted, needs native review
library-walking-count = الصور التي عُثر عليها: { $count }
library-reading = جارٍ الفهرسة… { $done } من { $total }
library-held = تتوقف الفهرسة مؤقتًا أثناء التصفح
library-up-to-date = محدَّثة
# machine-drafted, needs native review
library-stopped = توقفت الفهرسة
library-stopped-pending = توقفت الفهرسة · بانتظار الفهرسة: { $count }
library-resume = استئناف
# machine-drafted, needs native review
# The indexer stopped by itself (a database error, a bug) or stopped moving; $reason says why.
library-halted = توقفت الفهرسة: { $reason }
library-reason-stalled = لم يتحرك شيء منذ { $minutes ->
    [zero] { $minutes } دقيقة
    [one] دقيقة واحدة
    [two] دقيقتين
    [few] { $minutes } دقائق
    [many] { $minutes } دقيقة
   *[other] { $minutes } دقيقة
}. قد لا يستجيب ملف أو محرك أقراص.
library-retry = إعادة المحاولة
library-retry-description = يبدأ الفهرسة من جديد. يبقى ما فُهرس قابلًا للبحث.
library-save-diagnostics = حفظ حزمة التشخيص…
library-stop-description = يوقف الفهرسة. يبقى ما فُهرس حتى الآن قابلًا للبحث؛ يتابع «استئناف» أو «إعادة الفحص» (F5) من حيث توقفت.
library-resume-description = يبحث في كل المواقع مجددًا ويفهرس ما تبقى.
library-opening = جارٍ فتح المكتبة…
library-searching = جارٍ البحث…

## Empty and error states
library-empty = لا توجد مواقع بعد. أضف مجلدًا: تظهر صوره وصور مجلداته الفرعية هنا في شبكة واحدة.
library-empty-offline = هذه المواقع غير متصلة. تظهر صورها مجددًا عند عودتها.
library-indexing = جارٍ البحث عن الصور…
# machine-drafted, needs native review
library-remove-confirm = إزالة «{ $location }» من المكتبة؟
library-remove-confirm-detail = تبقى صورك على القرص مع تقييماتها وتسمياتها وكلماتها المفتاحية. تنسى المكتبة ما فهرسته هناك؛ وإضافة المجلد مجددًا تعيد فهرسته.
library-remove-confirm-ok = إزالة
library-indexing-in = جارٍ البحث عن الصور في { $location }… عُثر على { $count } حتى الآن
# machine-drafted, needs native review
library-empty-stopped = أُوقفت الفهرسة قبل قراءة هذه الصور. يتابع «استئناف» من هناك.
library-unavailable = تعذّر فتح المكتبة: { $reason }
library-read-only = كُتبت هذه المكتبة بإصدار أحدث من Specular: يمكن البحث فيها لكن لا يمكن تحديثها.

## Screen readers
library-a11y-list = المكتبة, { $count ->
    [zero] لا صور
    [one] صورة واحدة
    [two] صورتان
    [few] { $count } صور
    [many] { $count } صورة
   *[other] { $count } صورة
}
library-a11y-added = أُضيف { $location } إلى المكتبة
library-a11y-removed = أُزيل { $location } من المكتبة

## Adding the home folder
# machine-drafted, needs native review
library-home-confirm = إضافة كل محتويات «{ $folder }»؟
library-home-confirm-detail = يحتوي على ملفات البرامج والتنزيلات وذاكرات التخزين المؤقت، لا الصور وحدها. تستغرق الفهرسة وقتًا أطول، وتمتلئ المكتبة بالأيقونات وصور أخرى.
library-home-confirm-pictures = مجلد الصور لديك يحتوي على صورك فقط.
library-home-confirm-use-pictures = إضافة الصور
library-home-confirm-keep = إضافة المجلد الرئيسي

## Folder watches
# machine-drafted, needs native review
library-unwatched = بعض المجلدات غير مراقَبة
library-unwatched-detail = { $minutes ->
    [zero] تظهر التغييرات خلال أقل من دقيقة
    [one] تظهر التغييرات خلال دقيقة واحدة
    [two] تظهر التغييرات خلال دقيقتين
    [few] تظهر التغييرات خلال { $minutes } min
    [many] تظهر التغييرات خلال { $minutes } min
   *[other] تظهر التغييرات خلال { $minutes } min
}
library-unwatched-description = نفدت عمليات مراقبة المجلدات في Specular. يفتح المساعدة حول رفع الحد.

## Settings → Library
# machine-drafted, needs native review
settings-section-library = المكتبة
settings-library-skip-folders = تخطي المجلدات المسماة
settings-library-skip-folders-help = لا تبحث المكتبة في المجلدات بهذه الأسماء، وتُفصل بفواصل. تُتخطى المجلدات المخفية دائمًا.
settings-library-skip-marked = تخطي المجلدات التي تحتوي على
settings-library-skip-marked-help = ولا في المجلدات التي تحتوي على أحد هذه: ‎.git‎ لنسخ الشيفرة، و‎pyvenv.cfg‎ لبيئات Python، و‎CACHEDIR.TAG‎ لذاكرات التخزين المؤقت. لا يُتخطى مجلد الموقع نفسه أبدًا.
settings-library-show-timeline = الخط الزمني للتواريخ
settings-library-show-timeline-help = يعرض عدد الصور في كل سنة أو شهر أو يوم فوق شبكة المكتبة.
