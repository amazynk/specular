# machine-drafted, needs native review
## Optional decoders: other vendors' free software that opens a few formats (Blackmagic RAW,
## DWG), used only when the user installed it. Specular never bundles or downloads it.
## $format is the format ("Blackmagic RAW", "DWG"), $decoder the decoder's own name
## ("Blackmagic RAW SDK", "ODA File Converter"), $vendor its maker ("Blackmagic Design",
## "Open Design Alliance"): names, left as they are.

## Settings › Optional decoders. The switches and paths are shown in the panel below
## "Look again", not as rows of their own.

settings-section-decoders = مفككات الترميز الاختيارية
settings-decoders-look-again = البحث مجددًا
settings-decoders-look-again-help = البحث عن مفككات الترميز المثبّتة منذ ذلك الحين.
    يبحث { -app-name } أيضًا في كل مرة تنتقل فيها نافذته إلى المقدمة.
settings-decoders-braw = استخدام Blackmagic RAW SDK
settings-decoders-braw-help = تُفتح مقاطع Blackmagic RAW به عند تثبيته.
settings-decoders-braw-path = Blackmagic RAW SDK
settings-decoders-braw-path-help = اتركه فارغًا للبحث عنه حيث يضعه برنامج تثبيته.
settings-decoders-dwg = استخدام ODA File Converter
settings-decoders-dwg-help = تُفتح رسومات DWG به عند تثبيته.
    تُحوَّل نسخة من الرسم في مجلد خاص، ثم يُحذف المجلد.
settings-decoders-dwg-path = برنامج ODA File Converter
settings-decoders-dwg-path-help = اتركه فارغًا للبحث عنه حيث يضعه برنامج تثبيته.
settings-decoders-occt = استخدام OpenCASCADE
settings-decoders-occt-help = تُفتح نماذج STEP وIGES به عند تثبيته.
settings-decoders-occt-path = مكتبات OpenCASCADE
settings-decoders-occt-path-help = اتركه فارغًا للبحث عنها حيث يحفظ النظام المكتبات.
# machine-drafted, needs native review
settings-decoders-czi = استخدام libCZI
settings-decoders-czi-help = تُفتح ملفات Zeiss CZI به عند تثبيته.
    يعمل في المساعد المعزول لـ { -app-name }، دون الوصول إلى ملفاتك الأخرى.
settings-decoders-czi-path = مجلد مكتبة libCZI
settings-decoders-czi-path-help = اتركه فارغًا للبحث عنه حيث يحفظ النظام المكتبات.

## The panel: one block per decoder.

decoders-intro = تُفتح بعض الصيغ ببرامج مجانية من صانعيها.
    ثبّتها مرة واحدة: يعثر عليها { -app-name } ويستخدمها. ولا ينزّلها بنفسه أبدًا.
decoders-keywords = مفككات الترميز الاختيارية الصانع برنامج ترميز مكوّن إضافي يحتاج مفكك ترميز optional decoders vendor SDK codec plugin Blackmagic RAW BRAW RED R3D DWG ODA File Converter OpenCASCADE STEP IGES Zeiss CZI libCZI needs decoder
# A decoder's row: "Blackmagic RAW (.braw)". The tick box before it switches it on and off.
decoders-row = { $format } ({ $extensions })
# The row's value.
decoders-state-found = موجود
decoders-state-off = متوقف
decoders-state-not-found = غير مثبّت
decoders-state-broken = لا يمكن استخدامه
decoders-state-refused = غير ممكن
decoders-state-planned = غير متاح بعد
decoders-state-checking = جارٍ البحث…
# The row's lines.
decoders-uses = يُفتح باستخدام { $decoder } من { $vendor }.
decoders-not-found = إنه مجاني. ثبّته ثم اضغط «البحث مجددًا».
decoders-off = متوقف: لا تُفتح هذه الملفات.
decoders-version = الإصدار { $version }.
decoders-r3d-why = لا يسمح ترخيص RED بقراءة ملفات R3D إلا للبرامج المبنية بـ SDK الخاص بها وفق شروط RED نفسها.
    لا يستطيع { -app-name } قبول هذه الشروط، لذا لا تُفتح ملفات R3D.
decoders-ecw-why = ترخيص Hexagon يسمح باستخدام ECW SDK للبرامج التي تشحنه فقط، لا لنسخة ثبّتها بنفسك.
    لذلك لا تُفتح ملفات ECW. أما ملفات JPEG 2000 الناتجة عن ECW SDK فتُفتح.
decoders-mrsid-why = ترخيص MrSID SDK يسمح باستخدامه للبرامج التي تشحنه فقط، لا لنسخة ثبّتها بنفسك.
    لذلك لا تُفتح ملفات MrSID.
# machine-drafted, needs native review
# DJI radiometric JPEGs (drone thermal cameras): the photo and metadata open; the
# temperatures would need DJI's SDK.
decoders-dji-thermal-why = ترخيص SDK من DJI يسمح باستخدامه للبرامج التي تشحنه فقط، لا لنسخة ثبّتها بنفسك. ومع ذلك تُفتح الصورة وبياناتها الوصفية.
decoders-occt-why = تحتاج مجسّمات STEP وIGES إلى OpenCASCADE، ولا يستطيع هذا الإصدار من { -app-name } استخدامه.
# Rows with one button each, under a decoder.
decoders-get-row = صفحة التنزيل لدى الصانع
decoders-get = الحصول عليه
decoders-locate-row = مثبّت في مكان آخر؟
decoders-locate = تحديد الموقع…
decoders-forget-row = اخترته يدويًا.
decoders-forget = نسيان…
# Asked before Forget… does anything.
decoders-forget-ask = هل تريد نسيان { $decoder } الذي اخترته يدويًا؟
decoders-forget-detail = يعود { -app-name } إلى البحث عنه بنفسه. لا يُزال شيء من القرص.
decoders-forget-ok = نسيان
# The title of the Locate… file dialog.
decoders-locate-title = تحديد موقع { $decoder }
# Toasts.
decoders-locate-wrong = هذا ليس { $decoder }.
decoders-located = يُستخدم { $decoder } الذي اخترته.
decoders-forgotten = يبحث { -app-name } عن { $decoder } بنفسه مجددًا.
decoders-still-missing = لم يُعثر على { $decoder } بعد.
decoders-now-found = عُثر على { $decoder }.
decoders-looked = أُعيد البحث عن مفككات الترميز.

## The card shown in place of a file that needs one. $file is the file's name.

decoders-card-title = يحتاج { $format } إلى مفكك الترميز المجاني من { $vendor }
decoders-card-what = يُفتح { $file } باستخدام { $decoder }. إنه مجاني.
decoders-card-how = ثبّته ثم عُد: سيعثر عليه { -app-name } بنفسه.
decoders-card-get = الحصول عليه
decoders-card-installed = تم التثبيت
decoders-card-locate = تحديد الموقع…

## The gallery: the badge on such a file's thumbnail, and a line of its tooltip.

gallery-badge-needs-decoder = يحتاج مفكك ترميز
gallery-needs-decoder = يُفتح باستخدام { $decoder } المجاني من { $vendor }، وهو غير مثبّت.

## Why a file did not open, after "Can't open FILE:".

error-reason-needs-decoder = تحتاج ملفات { $format } إلى { $decoder } المجاني من { $vendor }، وهو غير مثبّت.
