# machine-drafted, needs native review

## Send to PACS (C-STORE, STOW-RS), DIMSE servers, teaching quiz and teaching case export.

## Servers: DIMSE fields and Verify.

dicomweb-field-kind = النوع
dicomweb-kind-web = DICOMweb
dicomweb-kind-dimse = DICOM (DIMSE)
dicomweb-field-host = المضيف
dicomweb-hint-host = pacs.example.org
dicomweb-field-port = المنفذ
dicomweb-hint-port = 104 أو 11112
# The application entity title the server answers to.
dicomweb-field-ae = ‏AE title الخاص بالخادم
dicomweb-hint-ae = PACS
# The application entity title Specular sends as.
dicomweb-field-our-ae = ‏AE title الخاص بنا
# machine-drafted, needs native review
dicomweb-field-tls = اتصال آمن (TLS)
dicomweb-tls-on = الاتصال الآمن مُفعّل
dicomweb-tls-off = الاتصال الآمن مُعطّل
dicomweb-problem-host = أدخل اسم مضيف الخادم أو عنوانه.
dicomweb-problem-port = المنفذ رقم من 1 إلى 65535.
dicomweb-problem-ae = يتكوّن AE title من 1 إلى 16 حرفًا، دون شرطة مائلة عكسية.
# machine-drafted, needs native review
dicomweb-dimse-note-open = يمكن البحث في خوادم DICOM (DIMSE) وفتح سلاسلها والإرسال إليها.
# How series are fetched: C-GET (over our own connection) or C-MOVE (the server connects back).
# machine-drafted, needs native review
pacs-field-retrieve = الاسترجاع عبر
# machine-drafted, needs native review
pacs-retrieve-get = C-GET
# machine-drafted, needs native review
pacs-retrieve-move = C-MOVE
# C-MOVE: the address of this computer and the port the server sends to.
# machine-drafted, needs native review
pacs-field-receive-address = عنوان الاستقبال
# machine-drafted, needs native review
pacs-hint-receive-address = عنوان هذا الحاسوب، مثل 192.168.1.20
# machine-drafted, needs native review
pacs-field-receive-port = منفذ الاستقبال
# machine-drafted, needs native review
pacs-hint-receive-port = مثل 11113
# machine-drafted, needs native review
pacs-move-note = لاستخدام C-MOVE يجب أن يعرف الخادم AE title الخاص بنا { $ae } مع هذا العنوان وهذا المنفذ.
    لا يستمع Specular هناك إلا أثناء وصول سلسلة.
# machine-drafted, needs native review
pacs-problem-receive-address = أدخل عنوان هذا الحاسوب (مثل 192.168.1.20) للاستقبال عليه.
# machine-drafted, needs native review
pacs-problem-receive-port = أدخل منفذًا للاستقبال. يحتاج C-MOVE إلى منفذ.
dicomweb-verify = تحقّق
dicomweb-verifying = جارٍ فحص الخادم…
dicomweb-verify-ok = استجاب { $server }.

## The Studies browser and the palette.

studies-send = إرسال إلى PACS…
studies-send-description = يرسل هذه السلسلة أو دراستها أو كل ما على القرص إلى خادم.
palette-send-to-pacs = إرسال إلى PACS…
palette-send-to-pacs-help = إرسال السلسلة أو الدراسة الحالية إلى خادم PACS.

## The Send page.

pacs-title = إرسال إلى PACS
# Under the title: what is being sent.
pacs-summary = { $studies ->
    [zero] { $studies } دراسة
    [one] دراسة واحدة
    [two] دراستان
    [few] { $studies } دراسات
    [many] { $studies } دراسة
   *[other] { $studies } دراسة
} · { $files ->
    [zero] { $files } ملف
    [one] ملف واحد
    [two] ملفان
    [few] { $files } ملفات
    [many] { $files } ملفًا
   *[other] { $files } ملف
}
pacs-what = ما يُرسل
pacs-scope-series = هذه السلسلة ({ $files ->
    [zero] { $files } ملف
    [one] ملف واحد
    [two] ملفان
    [few] { $files } ملفات
    [many] { $files } ملفًا
   *[other] { $files } ملف
})
pacs-scope-study = هذه الدراسة ({ $files ->
    [zero] { $files } ملف
    [one] ملف واحد
    [two] ملفان
    [few] { $files } ملفات
    [many] { $files } ملفًا
   *[other] { $files } ملف
})
pacs-scope-all = كل ما قُرئ ({ $studies ->
    [zero] { $studies } دراسة
    [one] دراسة واحدة
    [two] دراستان
    [few] { $studies } دراسات
    [many] { $studies } دراسة
   *[other] { $studies } دراسة
}، { $files ->
    [zero] { $files } ملف
    [one] ملف واحد
    [two] ملفان
    [few] { $files } ملفات
    [many] { $files } ملفًا
   *[other] { $files } ملف
})
pacs-anonymise = إخفاء الهوية
pacs-anonymise-note = تُزال بيانات المريض من النسخ المرسلة. لا تتغير ملفاتك.
pacs-identified-note = تُرسل الملفات كما هي، مع بيانات المريض.
# The list of servers to choose from.
pacs-to = إلى
pacs-no-servers = لا توجد خوادم بعد. أضف خادمًا في خوادم DICOMweb.
pacs-send = إرسال…
pacs-go-back = رجوع
pacs-confirm-send = إرسال
pacs-confirm = إرسال { $studies ->
    [zero] { $studies } دراسة
    [one] دراسة واحدة
    [two] دراستين
    [few] { $studies } دراسات
    [many] { $studies } دراسة
   *[other] { $studies } دراسة
} ({ $files ->
    [zero] { $files } ملف
    [one] ملف واحد
    [two] ملفين
    [few] { $files } ملفات
    [many] { $files } ملفًا
   *[other] { $files } ملف
}) إلى { $server }؟
pacs-busy = لا يزال إرسال قيد التشغيل.
pacs-started = جارٍ الإرسال.
pacs-stopping = جارٍ الإيقاف…
pacs-sending = جارٍ إرسال { $done } من { $total } إلى { $server }…
pacs-sent-all = { $total ->
    [zero] لم يكن هناك ملفات لإرسالها إلى { $server }.
    [one] أُرسل الملف إلى { $server }.
    [two] أُرسل الملفان إلى { $server }.
    [few] أُرسلت الملفات الـ { $total } كلها إلى { $server }.
    [many] أُرسلت الملفات الـ { $total } كلها إلى { $server }.
   *[other] أُرسلت الملفات الـ { $total } كلها إلى { $server }.
}
pacs-sent-some = أُرسل { $sent } من { $total } ملفات إلى { $server }.
pacs-stopped = توقف بعد { $sent } من { $total ->
        [zero] { $total } ملفات
        [one] ملف واحد
        [two] ملفين
        [few] { $total } ملفات
        [many] { $total } ملفًا
       *[other] { $total } من الملفات
    }.
pacs-stopped-because = توقف بعد { $sent } من { $total ->
        [zero] { $total } ملفات
        [one] ملف واحد
        [two] ملفين
        [few] { $total } ملفات
        [many] { $total } ملفًا
       *[other] { $total } من الملفات
    }: { $reason }
pacs-warnings = { $count ->
    [zero] لم يُخزَّن أي ملف مع تحذير.
    [one] خُزّن ملف واحد مع تحذير.
    [two] خُزّن ملفان مع تحذير.
    [few] خُزّنت { $count } ملفات مع تحذير.
    [many] خُزّن { $count } ملفًا مع تحذير.
   *[other] خُزّن { $count } ملف مع تحذير.
}
# A file that was not sent: its file name and why.
pacs-failed-line = { $name }: { $reason }
pacs-failed-more = و{ $count } غيرها.

## Why one file was not sent.

pacs-reason-unreadable = تعذّرت القراءة: { $reason }
pacs-reason-not-dicom = ليس ملف DICOM
pacs-reason-burned-in = فيه بيانات مريض مدمجة في الصورة، لذا لا يُرسل مع إخفاء الهوية
pacs-reason-document = مستند لا يمكن إخفاء الهوية فيه
pacs-reason-anonymise = تعذّر إخفاء الهوية: { $reason }
pacs-reason-not-accepted = لا يقبل الخادم { $class }
pacs-reason-convert = تعذّر التحويل للخادم: { $reason }
pacs-reason-resources = نفدت مساحة الخادم أو موارده
pacs-reason-mismatch = يقول الخادم إن البيانات لا تطابق نوعها
pacs-reason-understand = تعذّر على الخادم قراءته
pacs-reason-class = لا يدعم الخادم هذا النوع من الكائنات
pacs-reason-not-authorised = لا يسمح الخادم بذلك
pacs-reason-status = رفضه الخادم (الحالة { $code })
pacs-reason-refused = رفضه الخادم
pacs-reason-too-large = أكبر من أن يقبله الخادم
pacs-reason-http = رفضه الخادم (الخطأ { $status })

## Why sending stopped, or Verify failed.

pacs-fatal-network-off = الشبكة مُطفأة.
pacs-fatal-unknown-host = { $host } ليس اسم مضيف معروفًا.
pacs-fatal-connect = لا شيء يستجيب على { $host }، المنفذ { $port }.
pacs-fatal-unreachable = تعذّر الوصول إلى { $host }.
pacs-fatal-timeout = لم يستجب { $host } في الوقت المحدد.
pacs-fatal-cancelled = توقف.
pacs-fatal-called-ae = لا يستجيب { $host } لـ AE title‏ { $ae }. تحقق من AE title الخاص بالخادم.
pacs-fatal-calling-ae = لا يعرف { $host } AE title الخاص بنا { $ae }. أضفه على الخادم.
pacs-fatal-busy = { $host } مشغول. أعد المحاولة لاحقًا.
pacs-fatal-rejected = رفض { $host } الاتصال.
pacs-fatal-aborted = أنهى { $host } الاتصال.
pacs-fatal-nothing-accepted = لا يقبل { $host } أيًّا من هذه الأنواع من الكائنات.
pacs-fatal-protocol = استجاب { $host } بطريقة لا يفهمها Specular.
pacs-fatal-auth = لم يقبل { $host } تسجيل الدخول.
pacs-fatal-http = استجاب { $host } بالخطأ { $status }.

## Teaching quiz.

palette-teaching-quiz = اختبار تعليمي
palette-teaching-quiz-help = التنقل بين الصور المفتاحية أو حالة تعليمية، مع إخفاء النتائج حتى تكشفها.
# "Case 2 of 5".
quiz-case = الحالة { $n } من { $total }
quiz-hint-hidden = المسافة تكشف النتائج · → الحالة التالية · Esc للإنهاء
quiz-hint-revealed = المسافة تخفيها مجددًا · → الحالة التالية · Esc للإنهاء
quiz-hidden = النتائج مخفية.
quiz-no-findings = لا توجد نتائج مسجّلة.
quiz-notes = ملاحظات الحالة: { $notes }
quiz-ended = انتهى الاختبار.
quiz-empty = لا توجد حالات لعرضها. علّم الصور المفتاحية بالمفتاح K، أو اختر مجلد حالة تعليمية.
quiz-not-a-case = هذا المجلد ليس حالة تعليمية صدّرها Specular.
quiz-pick-title = اختيار مجلد حالة تعليمية

## Teaching case folder (Export key images).

figure-keys-case = مجلد حالة تعليمية
figure-case-notes = ملاحظات الحالة
figure-case-findings = النتائج
figure-case-none = لا شيء مسجّل
figure-case-readme = حالة تعليمية صدّرها Specular. أُزيلت بيانات المريض.
    ‏images: الصور المفتاحية، مع حجب النص المدمج فيها بالأسود.
    ‏DICOM: نسخ بهوية مخفية وبيان Key Object Selection‏ "For Teaching".
    ‏case.txt: ملاحظات الحالة ونتائج كل صورة. case.json: المحتوى نفسه للبرامج النصية.
    لـ Radiopaedia: ارفع مجلد DICOM أو الصور، وانسخ الملاحظات من case.txt.

## Search on a DIMSE server (C-FIND).
# machine-drafted, needs native review
pacs-find-not-supported = لا يستجيب { $host } لعمليات البحث (C-FIND).
pacs-find-status = رفض { $host } البحث (الحالة { $code }).

## Opening a series from a DIMSE server (C-GET, C-MOVE).
# machine-drafted, needs native review
pacs-retrieve-no-get = لا يرسل { $host } السلاسل عبر C-GET.
    اختر C-MOVE في إعدادات الخادم.
# machine-drafted, needs native review
pacs-retrieve-no-move = لا يقبل { $host } طلبات C-MOVE.
# machine-drafted, needs native review
pacs-retrieve-unknown-destination = لا يعرف { $host } AE title الخاص بنا { $ae }.
    أضفه على الخادم مع عنوان هذا الحاسوب والمنفذ { $port }.
# machine-drafted, needs native review
pacs-retrieve-resources = نفدت موارد { $host }. أعد المحاولة لاحقًا.
# machine-drafted, needs native review
pacs-retrieve-status = رفض { $host } إرسال السلسلة (الحالة { $code }).
# machine-drafted, needs native review
pacs-retrieve-no-matches = لا توجد لدى { $host } صور لهذه السلسلة.
# machine-drafted, needs native review
pacs-retrieve-roles = رفض { $host } إرسال الصور عبر هذا الاتصال.
    اختر C-MOVE في إعدادات الخادم.
# machine-drafted, needs native review
pacs-retrieve-move-nothing = تعذّر على { $host } إرسال الصور إلى هذا الحاسوب.
    تحقق من العنوان والمنفذ والجدار الناري، ومن أن الخادم يعرف AE title الخاص بنا { $ae }.
# machine-drafted, needs native review
pacs-retrieve-nothing = تعذّر على { $host } إرسال { $count ->
    [zero] أي صورة
    [one] الصورة
    [two] أيٍّ من الصورتين
    [few] أيٍّ من الصور الـ{ $count }
    [many] أيٍّ من الصور الـ{ $count }
   *[other] أيٍّ من الصور الـ{ $count }
} بصيغة يقرؤها Specular.
# machine-drafted, needs native review
pacs-retrieve-listen = يتعذّر الاستقبال على { $address }، المنفذ { $port }: { $reason }

## DICOM over TLS.
# machine-drafted, needs native review
pacs-fatal-tls-certificate = شهادة { $host } غير موثوقة.
    يجب أن تكون الجهة المُصدِرة لها ضمن الشهادات الموثوقة في النظام.
pacs-fatal-tls = لا يوجد اتصال آمن بـ { $host }.
    تحقق من أن المنفذ { $port } يستخدم DICOM عبر TLS.
