# machine-drafted, needs native review
# لوحة البيانات الوصفية في Specular — العربية الفصحى.

## عناوين مجموعات الملخص

group-camera = الكاميرا
group-exposure = التعريض
group-lens = العدسة
group-date = التاريخ والوقت
group-location = الموقع
# machine-drafted, needs native review
# A DJI drone photo: the gimbal's and the aircraft's attitude, the height above take-off.
group-drone = الطائرة المسيّرة
label-gimbal = الجيمبال
label-flight = الطائرة
label-relative-altitude = فوق نقطة الإقلاع
drone-attitude = الانعراج { $yaw }، الميل { $pitch }، الدحرجة { $roll }
# A DJI thermal camera's radiometric JPEG: its photo opens, its temperatures can't.
label-thermal-data = درجات الحرارة
drone-thermal-refused = غير معروضة: ترخيص SDK من DJI لا يسمح بذلك
drone-thermal-refused-detail = قراءة درجات حرارة DJI تحتاج إلى Thermal SDK من DJI، وترخيصه يسمح باستخدامه للبرامج التي تشحنه فقط. تُعرض الصورة وبياناتها الوصفية.
group-image = الصورة
group-file = الملف
group-rights = الحقوق والوصف
group-credentials = بيانات اعتماد المحتوى
# machine-drafted, needs native review
group-medical = صورة طبية
# machine-drafted, needs native review
group-inspection = المكوّن والفحص
group-nde-image = صورة الفحص (DICONDE)
# machine-drafted, needs native review
# Industrial CT volumes (Zeiss Xradia, Amira, Bruker SkyScan, Nikon X-Tek, VGI, raw volumes).
group-industrial-image = صورة الفحص
group-science = صورة علمية
group-em = المجهر الإلكتروني
group-spm = مجهر المسبار الماسح
group-slide = الشريحة
group-microscopy = المجهرية
# X-ray diffraction detector frames (CBF, EDF, SMV, Bruker, MarCCD, R-AXIS).
group-diffraction = الحيود
group-surface = طبوغرافيا السطح
group-ebsd = حيود الإلكترونات المرتدة
group-seismic = البيانات السيزمية
# machine-drafted, needs native review
group-asdf = ASDF
# machine-drafted, needs native review
# CERN ROOT files (particle physics histograms and graphs).
group-root = ROOT
# machine-drafted, needs native review
# CASA images (radio interferometry: the .image, .residual, .model directories tclean writes).
group-casa = صورة CASA
# machine-drafted, needs native review
# GIFTI files: brain surfaces and the values on their vertices (Human Connectome Project
# and FreeSurfer exports).
group-gifti = GIFTI
# CIFTI-2 files: maps over cortical surfaces and subcortical voxels ("grayordinates").
group-cifti = CIFTI
# White-matter fibre tracts (TrackVis, MRtrix).
group-tractography = تصوير المسالك العصبية
# machine-drafted, needs native review
group-annotations = التعليقات التوضيحية
# Imaging mass spectrometry (imzML): a mass spectrum per pixel.
group-mass-spec = التصوير بمطيافية الكتلة
# machine-drafted, needs native review
group-thermal = صورة حرارية
# machine-drafted, needs native review
# Flow cytometry list-mode data (FCS): events of a cell sorter or analyser.
group-cytometry = قياس الخلايا التدفقي
# machine-drafted, needs native review
group-spectroscopy = التحليل الطيفي
# machine-drafted, needs native review
# Unstructured meshes of ocean, coastal and climate models (UGRID, ICON, MPAS in NetCDF).
group-ugrid = شبكة غير منتظمة
group-drawing = رسم هندسي
# machine-drafted, needs native review
group-volume = حجم

## تسميات عناصر الملخص

label-camera = الكاميرا
label-software = البرنامج
label-application = التطبيق
label-shutter = سرعة الغالق
label-aperture = فتحة العدسة
label-iso = ISO
label-focal-length = البعد البؤري
label-exposure-bias = تعويض التعريض
label-program = الوضع
label-metering = وضع القياس
label-white-balance = توازن الأبيض
label-flash = الفلاش
label-lens = العدسة
label-lens-make = الشركة المصنّعة للعدسة
label-lens-range = نطاق العدسة
# machine-drafted, needs native review
label-focus-mode = وضع التركيز البؤري
label-stabilisation = تثبيت الصورة
label-taken = وقت الالتقاط
label-digitised = وقت الرقمنة
label-edited = آخر تحرير
label-place = المكان
label-coordinates = الإحداثيات
label-altitude = الارتفاع
label-direction = اتجاه التصوير
label-accuracy = الدقة
label-gps-time = وقت GPS
label-dimensions = الأبعاد
label-encoding = الترميز
# machine-drafted, needs native review
label-motion = الحركة
motion-kind-motion-photo = صورة متحركة
motion-kind-micro-video = صورة متحركة (MicroVideo)
motion-kind-samsung = صورة Samsung المتحركة
motion-kind-live-photo = Live Photo
motion-duration = ‏{ $seconds } ث
label-colour-profile = ملف تعريف الألوان
label-orientation = اتجاه الصورة
label-file-name = الاسم
label-folder = المجلد
label-file-size = الحجم
label-format = التنسيق
label-modified = تاريخ التعديل
label-title = العنوان
label-description = الوصف
label-creator = المنشئ
label-copyright = حقوق النشر
label-keywords = الكلمات المفتاحية
label-rating = التقييم
label-colour-label = التسمية اللونية
label-credentials = بيانات اعتماد المحتوى

## عناوين مجموعات «كل الحقول» (تبقى أسماء الوسوم كما في المعايير)

fields-ifd0 = IFD0 (الصورة الرئيسية)
fields-exif = Exif
fields-gps = GPS
fields-interop = التشغيل البيني
fields-ifd1 = IFD1 (الصورة المصغّرة)
fields-icc = ترويسة ملف تعريف ICC
fields-xmp = XMP: { $namespace } ({ $prefix })
xmp-namespace = { $prefix ->
    [dc] Dublin Core
    [xmp] أساسي
    [xmpRights] الحقوق
    [xmpMM] إدارة الوسائط
    [xmpDM] الوسائط الديناميكية
    [tiff] TIFF
    [exif] Exif
    [exifEX] Exif 2.3+
    [aux] ملحقات الكاميرا
    [photoshop] Photoshop
    [crs] إعدادات Camera Raw
    [lr] Lightroom
    [Iptc4xmpCore] IPTC Core
    [Iptc4xmpExt] IPTC Extension
    [plus] ترخيص PLUS
    [GPano] صورة كروية
    [GCamera] كاميرا Google
    [hdrgm] خريطة كسب HDR
    [mwg-rs] المناطق
    [pdf] PDF
    [stEvt] أحداث المورد
    [stRef] مراجع المورد
   *[other] غير ذلك
}
unknown-tag = وسم غير معروف { $number } ({ $context })
blob-bytes = { $num ->
    [zero] ({ $num } بايت)
    [one] ({ $num } بايت)
    [two] ({ $num } بايت)
    [few] ({ $num } بايتات)
    [many] ({ $num } بايتًا)
   *[other] ({ $num } بايت)
}
maker-note-bytes = (الحجم: { $num ->
        [zero] { $num } بايت
        [one] { $num } بايت
        [two] { $num } بايت
        [few] { $num } بايتات
        [many] { $num } بايتًا
       *[other] { $num } بايت
    }، بيانات الشركة المصنّعة، لم تُفكّ شفرتها)
more-values = { $shown }، … (إجمالي القيم: { $total })

## فواصل عامة

list-separator = {"، "}
one-line-separator = {" · "}

## التعريض

exposure-time-fraction = 1/{ $denominator } ث
exposure-time-seconds = { $seconds } ث
aperture-value = f/{ $f }
iso-value = ISO { $iso }
focal-length-value = { $mm } مم
focal-length-with-equiv = { $mm } مم (ما يعادل { $equiv } مم)
focal-length-equiv-only = ما يعادل { $equiv } مم
exposure-bias-value = { $sign ->
    [plus] +{ $ev } EV
    [minus] −{ $ev } EV
   *[zero] 0 EV
}
exposure-program = { $value ->
    [1] يدوي
    [2] برنامج (تلقائي)
    [3] أولوية فتحة العدسة
    [4] أولوية الغالق
    [5] إبداعي (عمق الميدان)
    [6] حركة (غالق سريع)
    [7] صورة شخصية
    [8] منظر طبيعي
   *[other] غير معروف
}
metering-mode = { $value ->
    [1] متوسط
    [2] متوسط مرجّح للمركز
    [3] نقطي
    [4] متعدد النقاط
    [5] مصفوفي (تقييمي)
    [6] جزئي
   *[other] غير ذلك
}
# machine-drafted, needs native review
# Canon maker note CameraSettings FocusMode; 256–258 are the live-view variants of 0–2.
canon-focus-mode = { $value ->
    [0] One-Shot AF
    [1] AI Servo AF
    [2] AI Focus AF
    [3] تركيز يدوي
    [4] فردي
    [5] مستمر
    [6] تركيز يدوي
    [16] Pan focus
    [256] One-Shot AF (العرض المباشر)
    [257] AI Servo AF (العرض المباشر)
    [258] AI Focus AF (العرض المباشر)
    [512] تركيز Movie Snap
    [519] Movie Servo AF
   *[other] غير ذلك
}
# Canon maker note CameraSettings ImageStabilization; 256–260 repeat 0–4.
canon-stabilisation = { $value ->
    [0] إيقاف
    [1] تشغيل
    [2] عند التصوير فقط
    [3] تتبّع أفقي
    [4] ديناميكي
    [256] إيقاف
    [257] تشغيل
    [258] عند التصوير فقط
    [259] تتبّع أفقي
    [260] ديناميكي
   *[other] غير ذلك
}
# machine-drafted, needs native review
panasonic-focus-mode = { $value ->
    [1] تلقائي
    [2] تركيز يدوي
    [4] تلقائي (زر التركيز)
    [5] تلقائي (مستمر)
    [6] AF-S
    [7] AF-C
    [8] AF-F
   *[other] أخرى
}
# machine-drafted, needs native review
panasonic-stabilisation = { $value ->
    [2] تشغيل (بصري)
    [3] إيقاف
    [4] تشغيل (الوضع 2)
    [5] تشغيل (بصري, تتبع)
    [6] تشغيل (الهيكل فقط)
    [7] تشغيل (الهيكل فقط, تتبع)
    [9] Dual IS
    [10] Dual IS (تتبع)
    [11] Dual IS 2
    [12] Dual IS 2 (تتبع)
   *[other] أخرى
}
label-comment = تعليق
fields-tags = وسوم { $kind }
group-picture = إعدادات الصورة
fields-fujifilm = ملاحظة الصانع من Fujifilm
label-film-simulation = محاكاة الفيلم
label-dynamic-range = النطاق الديناميكي
label-highlight-tone = الإضاءات
label-shadow-tone = الظلال
label-colour-setting = اللون
label-sharpness = الحدة
label-noise-reduction = تقليل الضوضاء
label-clarity = الوضوح
label-grain-effect = تأثير الحبيبات
label-colour-chrome = تأثير الكروم اللوني
label-colour-chrome-blue = الكروم اللوني الأزرق
label-shutter-type = نوع الغالق
label-drive-mode = وضع التصوير
fuji-step = { $sign ->
    [plus] +{ $n }
    [minus] −{ $n }
   *[zero] 0
}
fuji-film-mode = { $value ->
    [0] Provia (قياسي)
    [256] صورة شخصية في الاستوديو
    [272] صورة شخصية في الاستوديو (تشبع أكبر)
    [288] Astia (ناعم)
    [304] صورة شخصية في الاستوديو (أحدّ)
    [512] Velvia (حيوي)
    [768] صورة شخصية في الاستوديو Ex
    [1024] Velvia
    [1280] Pro Neg. Std
    [1281] Pro Neg. Hi
    [1536] Classic Chrome
    [1792] Eterna
    [2048] Classic Neg.
    [2304] Eterna Bleach Bypass
    [2560] Nostalgic Neg.
    [2816] Reala Ace
   *[other] أخرى
}
fuji-monochrome = { $value ->
    [768] أحادي اللون
    [769] أحادي اللون، مرشح أحمر
    [770] أحادي اللون، مرشح أصفر
    [771] أحادي اللون، مرشح أخضر
    [784] بني داكن
    [1280] Acros
    [1281] Acros، مرشح أحمر
    [1282] Acros، مرشح أصفر
    [1283] Acros، مرشح أخضر
   *[other] أحادي اللون
}
fuji-white-balance = { $value ->
    [0] تلقائي
    [1] تلقائي (أولوية الأبيض)
    [2] تلقائي (أولوية الأجواء)
    [256] ضوء النهار
    [512] غائم
    [768] فلورسنت ضوء النهار
    [769] فلورسنت أبيض نهاري
    [770] فلورسنت أبيض
    [771] فلورسنت أبيض دافئ
    [772] فلورسنت أبيض دافئ للمنازل
    [1024] مصباح متوهج
    [1280] فلاش
    [1536] تحت الماء
    [3840] مخصص 1
    [3841] مخصص 2
    [3842] مخصص 3
    [3843] مخصص 4
    [3844] مخصص 5
    [4080] درجة حرارة اللون
   *[other] أخرى
}
fuji-white-balance-kelvin = { $kelvin } K
fuji-dynamic-range = DR{ $percent }
fuji-dynamic-range-auto = تلقائي (DR{ $percent })
fuji-dynamic-range-setting = { $value ->
    [0] تلقائي
    [256] قياسي (DR100)
    [512] واسع 1 (DR230)
    [513] واسع 2 (DR400)
   *[other] أخرى
}
fuji-effect = { $value ->
    [0] إيقاف
    [32] ضعيف
    [64] قوي
   *[other] أخرى
}
fuji-grain-size = { $value ->
    [16] صغيرة
    [32] كبيرة
   *[other] أخرى
}
fuji-grain = { $strength }، { $size }
fuji-shutter-type = { $value ->
    [0] ميكانيكي
    [1] إلكتروني
    [2] إلكتروني (تعريض طويل)
    [3] الستارة الأمامية الإلكترونية
   *[other] أخرى
}
fuji-drive-mode = { $value ->
    [0] لقطة واحدة
    [1] متتابع، سرعة منخفضة
    [2] متتابع، سرعة عالية
   *[other] أخرى
}
fuji-focus-mode = { $value ->
    [0] يدوي
    [1] AF-S
    [2] AF-C
    [100] تلقائي
    [101] يدوي
   *[other] أخرى
}
fuji-stabilisation = { $kind } ({ $mode })
fuji-stabilisation-kind = { $value ->
    [0] بلا
    [1] بصري
    [2] إزاحة المستشعر
    [3] العدسة
    [258] إزاحة المستشعر ورقمي
    [512] رقمي
   *[other] أخرى
}
fuji-stabilisation-mode = { $value ->
    [0] إيقاف
    [1] تشغيل، مستمر
    [2] تشغيل، أثناء التصوير
   *[other] أخرى
}
light-source = { $value ->
    [1] ضوء النهار
    [2] فلورسنت
    [3] تنغستن
    [4] فلاش
    [9] طقس صافٍ
    [10] غائم
    [11] ظل
    [12] فلورسنت بلون ضوء النهار
    [13] فلورسنت أبيض نهاري
    [14] فلورسنت أبيض بارد
    [15] فلورسنت أبيض
    [16] فلورسنت أبيض دافئ
    [17] ضوء قياسي A
    [18] ضوء قياسي B
    [19] ضوء قياسي C
    [20] D55
    [21] D65
    [22] D75
    [23] D50
    [24] تنغستن الاستوديو
   *[other] غير ذلك
}
white-balance-auto = تلقائي
white-balance-manual = يدوي
white-balance-manual-with-light = يدوي ({ $light })
flash-none = لا توجد وظيفة فلاش
flash-fired = انطلق الفلاش
flash-not-fired = لم ينطلق الفلاش
flash-forced-on = تشغيل إجباري
flash-forced-off = إيقاف إجباري
flash-auto = وضع تلقائي
flash-return-not-detected = لم يُرصد الضوء المرتد
flash-return-detected = رُصد الضوء المرتد
flash-red-eye = تقليل احمرار العين

## العدسة

lens-focal-single = { $mm } مم
lens-focal-range = { $min }–{ $max } مم
lens-aperture-single = f/{ $f }
lens-aperture-range = f/{ $min }–{ $max }
lens-range = { $focal } { $aperture }

## التاريخ والوقت

date-with-weekday = { $weekday }، { $date }
date-time = { $date }، { $time }
date-time-zone = { $datetime } ({ $zone })
zone-utc = UTC
zone-offset = UTC{ $offset }
gps-date-time = { $date }، { $time } UTC
detail-zone-from-gps = استُنتجت المنطقة الزمنية من ساعة GPS

## الموقع

coordinates = { $lat }° { $latRef ->
    [south] جنوبًا
   *[north] شمالًا
}، { $lon }° { $lonRef ->
    [west] غربًا
   *[east] شرقًا
}
altitude = { $ref ->
    [above] { $m } م فوق مستوى سطح البحر
    [below] { $m } م تحت مستوى سطح البحر
   *[sea] عند مستوى سطح البحر
}
compass-point = { $point ->
    [n] الشمال
    [nne] شمال الشمال الشرقي
    [ne] الشمال الشرقي
    [ene] شرق الشمال الشرقي
    [e] الشرق
    [ese] شرق الجنوب الشرقي
    [se] الجنوب الشرقي
    [sse] جنوب الجنوب الشرقي
    [s] الجنوب
    [ssw] جنوب الجنوب الغربي
    [sw] الجنوب الغربي
    [wsw] غرب الجنوب الغربي
    [w] الغرب
    [wnw] غرب الشمال الغربي
    [nw] الشمال الغربي
   *[nnw] شمال الشمال الغربي
}
facing = باتجاه { $point } ({ $degrees }°)
detail-true-north = بالنسبة إلى الشمال الجغرافي
detail-magnetic-north = بالنسبة إلى الشمال المغناطيسي
accuracy = ±{ $m } م
distance-km = { $km } كم
place = { $city }، { $country }
place-city-only = { $city }
place-near = بالقرب من { $place }
place-detail = { $areas } · على بُعد { $distance } من { $city }
place-detail-distance = على بُعد { $distance } من { $city }

## الصورة

dimensions = { $width } × { $height }
dimensions-megapixels = { $width } × { $height } ({ $mp } ميغابكسل)
encoding-bit-depth = { $bits } بت
encoding-progressive = { $format }، تدريجي
encoding-baseline = { $format }، أساسي
encoding-hierarchical = { $format }، هرمي
# machine-drafted, needs native review
encoding-alpha = { $format } { $alpha ->
    [premultiplied] مع ألفا مضروبة مسبقًا
    [transparency] مع شفافية
   *[alpha] مع ألفا
}
# [Lab], [Palette] and the variants after them: machine-drafted, needs native review
colour-model = { $model ->
    [YCbCr] YCbCr
    [RGB] RGB
    [Greyscale] تدرّج رمادي
    [CMYK] CMYK
    [YCCK] YCCK
    [Lab] L*a*b*
    [Palette] ألوان مفهرسة
    [Multichannel] متعدد القنوات
    [Duotone] ثنائي اللون
    [YCgCo] YCgCo
    [XYB] XYB
    [YC] نصوع/تلوين (YC)
    [Bayer] RAW بنمط Bayer
    [X-Trans] RAW بنمط X-Trans
    [CFA] RAW بمصفوفة مرشحات ألوان
    [LinearRaw] RAW خطي
    [RGBE] RGBE (أُس مشترك)
    [XYZE] XYZE (أُس مشترك)
    [R] قناة واحدة (R)
    [RG] قناتان (RG)
   *[other] { $model }
}
adobe-rgb-from-exif = Adobe RGB (من وسم EXIF؛ لا يوجد ملف تعريف مضمّن)
orientation = { $value ->
    [1] قائمة (لا يلزم تدوير)
    [2] انعكاس أفقي (تم التطبيق)
    [3] تدوير 180° (تم التطبيق)
    [4] انعكاس رأسي (تم التطبيق)
    [5] انعكاس أفقي وتدوير 90° عكس اتجاه عقارب الساعة (تم التطبيق)
    [6] تدوير 90° مع اتجاه عقارب الساعة (تم التطبيق)
    [7] انعكاس أفقي وتدوير 90° مع اتجاه عقارب الساعة (تم التطبيق)
    [8] تدوير 90° عكس اتجاه عقارب الساعة (تم التطبيق)
   *[other] غير معروف ({ $value })
}
orientation-short = { $value ->
    [1] قائمة
    [2] انعكاس أفقي
    [3] تدوير 180°
    [4] انعكاس رأسي
    [5] انعكاس أفقي وتدوير 90° عكس اتجاه عقارب الساعة
    [6] تدوير 90° مع اتجاه عقارب الساعة
    [7] انعكاس أفقي وتدوير 90° مع اتجاه عقارب الساعة
    [8] تدوير 90° عكس اتجاه عقارب الساعة
   *[other] القيمة { $value }
}

## الملف

file-size-bytes = { $num ->
    [zero] { $num } بايت
    [one] { $num } بايت
    [two] { $num } بايت
    [few] { $num } بايتات
    [many] { $num } بايتًا
   *[other] { $num } بايت
}
file-size-unit = { $num } { $unit ->
    [kb] KB
    [mb] MB
    [gb] GB
    [tb] TB
   *[pb] PB
}

## الحقوق

rating-rejected = مرفوض
rating-stars = { $stars } ({ $n } من 5)

## بيانات اعتماد المحتوى

c2pa-present = موجودة (لم يُتحقَّق منها)
c2pa-detail = يحمل هذا الملف بيان C2PA؛ لم يُتحقَّق من توقيعه.

## قيم ترويسة ICC

icc-class = { $class ->
    [scnr] جهاز إدخال
    [mntr] جهاز عرض
    [prtr] جهاز إخراج
    [link] ربط أجهزة
    [spac] فضاء لوني
    [abst] مجرّد
    [nmcl] لون مُسمّى
   *[other] غير معروف
}
icc-intent = { $intent ->
    [0] إدراكي
    [1] قياس لوني نسبي
    [2] تشبّع
    [3] قياس لوني مطلق
   *[other] غير معروف
}
icc-platform = { $platform ->
    [APPL] Apple
    [MSFT] Microsoft
    [SGI] Silicon Graphics
    [SUNW] Sun Microsystems
    [none] غير محدد
   *[other] غير معروف
}
icc-coded = { $name } ({ $code })

# machine-drafted, needs native review
label-modality = الطريقة التصويرية
label-body-part = جزء الجسم
label-study = الدراسة
label-series = السلسلة
label-slice-thickness = سماكة الشريحة
label-pixel-spacing = تباعد البكسلات
label-imager-pixel-spacing = تباعد البكسلات عند الكاشف
label-frames = الإطارات
label-transfer-syntax = صيغة النقل
label-document = مستند
label-document-type = نوع المستند
medical-spacing = { $row } × { $column } مم
medical-mm = { $mm } مم
medical-lossy = { $name } (ضغط مع فقدان)
detail-pixel-spacing = المسافة بين مراكز البكسلات داخل جسم المريض (صفوف × أعمدة).
# machine-drafted, needs native review
# Pixel spacing of an industrial inspection image (DICONDE, CT of an object).
detail-pixel-spacing-object = المسافة بين مراكز البكسلات داخل الجسم المفحوص (صفوف × أعمدة).
detail-imager-pixel-spacing = مقيسة عند الكاشف دون تصحيح التكبير: المسافات داخل جسم المريض عادةً أصغر.

## الفحص الصناعي (DICONDE)

# machine-drafted, needs native review
label-nde-component-shape = شكل المكوّن
label-nde-component-manufacturer = الشركة المصنّعة للمكوّن
label-nde-material = المادة
label-nde-thickness = سماكة المادة
label-nde-pipe-diameter = قطر الأنبوب
label-nde-source = المصدر
label-nde-environment = البيئة
label-nde-iqi = مؤشر جودة الصورة
label-nde-indications = المؤشرات
label-nde-standard = المعيار

## أحجام بحثية (NIfTI وAnalyze وNRRD وMetaImage)

# machine-drafted, needs native review
label-voxel-size = حجم الفوكسل
label-data-type = نوع البيانات
label-time-points = النقاط الزمنية
label-orientation-source = الاتجاه من
label-intent = القيم
label-compression = الضغط
volume-voxel-size = { $x } × { $y } × { $z } مم
volume-orient-sform = NIfTI sform
volume-orient-qform = NIfTI qform
volume-orient-analyze = ترويسة Analyze (غير موثوقة)
volume-orient-space = اتجاهات الفضاء في NRRD
volume-orient-transform = تحويل MetaImage
volume-orient-spacings = تباعد الفوكسل فقط
volume-orient-none = غير مذكور
volume-orient-vox2ras = FreeSurfer vox2ras
volume-orient-mgh-default = الافتراضي في FreeSurfer (غير موثوق)
volume-orient-minc = جيوب تمام أبعاد MINC
volume-orient-object = إحداثيات الجسم (بلا اتجاه للمريض)
volume-content-projections = إسقاطات (مسح خام)
volume-content-reconstruction = حجم مُعاد بناؤه
volume-coords-rectilinear = مستقيمة (تباعد غير منتظم، معروض بانتظام)
# Scan parameters and tags a FreeSurfer MGH file keeps after its voxels.
label-volume-tr = زمن التكرار
label-volume-te = زمن الصدى
label-volume-ti = زمن الانقلاب
label-volume-flip = زاوية الانقلاب
label-volume-fov = مجال الرؤية
label-volume-field = شدة المجال
label-volume-pedir = ترميز الطور
label-volume-cmdline = سطر الأوامر
label-volume-xform = تحويل Talairach
label-volume-content = المحتوى
label-volume-projections = الإسقاطات
label-volume-angles = مدى الدوران
label-volume-angle-step = خطوة الدوران
label-volume-kv = جهد الأنبوب
label-volume-current = تيار الأنبوب
label-volume-power = قدرة المصدر
label-volume-exposure = التعريض
label-volume-sod = من المصدر إلى الجسم
label-volume-odd = من الجسم إلى الكاشف
label-volume-sdd = من المصدر إلى الكاشف
label-volume-magnification = التكبير البصري
label-volume-binning = التجميع (Binning)
label-volume-objective = العدسة الشيئية
label-volume-filter = مرشّح المصدر
label-volume-camera = الكاشف
label-volume-instrument = الجهاز
label-volume-software = البرنامج
label-volume-date = تاريخ الالتقاط
label-volume-ring = تصحيح الأثر الحلقي
label-volume-smoothing = التنعيم
label-volume-coords = الشبكة
# machine-drafted, needs native review
# An eye scan (OCT): which eye.
label-volume-eye = العين
volume-eye-right = اليمنى (OD)
volume-eye-left = اليسرى (OS)
detail-volume-unreliable = لا يذكر الملف اتجاه محاوره في جسم المريض: قد يكون اليسار واليمين معكوسين، لذا لا تُعرض تسميات الاتجاه.
# machine-drafted, needs native review
# An industrial CT volume: its axes are the scanner's, there is no patient.
detail-volume-object = المحاور هي محاور الماسح الضوئي لا محاور مريض، لذا لا تُعرض تسميات الاتجاه.

## الصور العلمية (FITS، XISF)

label-object = الجرم
label-telescope = التلسكوب
label-instrument = الأداة
label-filter = المرشح
label-observed = وقت الرصد
label-exposure-time = التعريض
label-sample = العينات
label-unit = الوحدة
label-hdu = الامتداد
label-centre = المركز
label-centre-deg = المركز (بالدرجات)
label-frame = نظام الإحداثيات
# machine-drafted, needs native review
science-frame-galactic = مجري
science-frame-ecliptic = كسوفي
label-projection = الإسقاط
label-pixel-scale = مقياس البكسل
science-seconds = { $seconds } ث
science-arcsec-px = { $arcsec }″ لكل بكسل
label-distortion = التشوه
label-wcs = الإحداثيات السماوية
science-wcs-unsupported = غير مدعوم: { $reason }
# machine-drafted, needs native review
science-wcs-approximate = تقريبي (من التوجيه فقط، دون تشوّه)
label-observer = الراصد
label-bayer = نمط باير
label-capture-start = بدء التسجيل (UTC)
label-capture-end = نهاية التسجيل (UTC)
label-frame-rate = معدل الإطارات
science-fps = { $fps } إطار/ث
science-frames-cut = { $frames } من { $declared }
# The HDF5 dataset or NetCDF variable shown.
label-dataset = مجموعة البيانات
label-pixel-size = حجم البكسل
# "0.25 nm per pixel"; $size may be "0.25 × 0.5".
science-pixel-size = { $size } { $unit } لكل بكسل
# Microscopy (DigitalMicrograph, MRC, Gwyddion, Igor).
label-microscope = المجهر
label-voltage = جهد التسريع
label-magnification = التكبير
label-imaging-mode = الوضع
label-detector = الكاشف
label-acquired = وقت الالتقاط
label-field-of-view = مجال الرؤية
science-kilovolts = { $kv } kV
science-magnification = { $value }×
label-vendor = الشركة المصنّعة
label-scanner = الماسح الضوئي
label-objective = العدسة الشيئية
label-pyramid-levels = المستويات
# machine-drafted, needs native review
label-emissivity = الابتعاثية
label-object-distance = مسافة الجسم
label-reflected-temperature = درجة الحرارة المنعكسة
label-atmospheric-temperature = درجة حرارة الجو
label-humidity = الرطوبة النسبية
label-raw-range = النطاق الخام
label-temperature-range = نطاق درجات الحرارة
thermal-temperature = { $value } { $unit }
thermal-metres = { $value } m
thermal-percent = { PERCENT($value) }
thermal-range = { $low } – { $high }
slide-levels = { $count }: { $sizes }
# X-ray diffraction: the experiment geometry from a detector frame's header.
label-wavelength = الطول الموجي
# Ångström, e.g. "0.97625 Å".
xtal-angstrom = { $value } Å
label-photon-energy = طاقة الفوتون
xtal-kev = { $value } keV
# Sample to detector distance.
label-detector-distance = المسافة إلى الكاشف
xtal-mm = { $value } مم
# Where the direct beam meets the detector, in pixels from the image's top-left corner.
label-beam-centre = مركز الحزمة
xtal-beam-centre = x { $x }, y { $y } px
# The detector arm's swing angle (2θ).
label-detector-swing = زاوية الكاشف 2θ
xtal-degrees = { $value }°
# Goniometer angles (Greek letters are the field's usual names).
label-phi = فاي (φ)
label-omega = أوميغا (ω)
label-chi = كاي (χ)
label-kappa = كابا (κ)
# The sample's rotation during the exposure: start angle plus width, e.g. "12.5° + 0.1°".
label-oscillation = التذبذب
xtal-oscillation = { $start }° + { $width }°
# $axis is the axis name from the file, e.g. "phi" or "omega".
xtal-oscillation-axis = { $start }° + { $width }° حول { $axis }
# The count at which a pixel saturates.
label-overload = حد التشبع
# The finest lattice spacing (d) recorded at the detector's edge and in its corners.
label-resolution = الدقة
xtal-resolution = { $edge } Å عند الحافة، { $corner } Å في الزوايا
label-geometry = الهندسة
xtal-curved = لوح تصوير منحنٍ: لا q ولا d ولا حلقات دقة
xtal-incomplete = ترويسة غير مكتملة: لا q ولا d ولا حلقات دقة
# All-fields group title: "FITS header".
fields-science-header = ترويسة { $format }
# machine-drafted, needs native review
fields-asdf-tree = شجرة ASDF
label-asdf-model = نموذج البيانات
label-asdf-version = إصدار الملف
label-asdf-standard = معيار ASDF
label-asdf-library = كُتب بواسطة
# machine-drafted, needs native review
# All-fields group title for a CERN ROOT file's list of keys (objects).
fields-root-keys = مفاتيح ROOT
# CERN ROOT files: the ROOT version that wrote the file (e.g. "6.28/04"), its compression,
# the number of keys (stored objects), and the object shown: class (e.g. "TH2F"), its path in
# the file, title, number of entries and sum of weights.
label-root-version = إصدار ROOT
label-root-compression = الضغط
label-root-keys = المفاتيح
label-root-class = الصنف
label-root-name = الاسم
label-root-title = العنوان
label-root-entries = المُدخلات
label-root-sum-weights = مجموع الأوزان
# A graph's number of points.
label-root-points = النقاط
# $axis is "x", "y" or "z".
label-root-axis = المحور { $axis }
# $quantity is the axis title with its unit, e.g. "Energy [GeV]"; $min and $max the axis
# limits.
root-axis-fixed = { $quantity }: { $bins ->
        [zero] لا خانات
        [one] خانة واحدة
        [two] خانتان
        [few] { $bins } خانات
        [many] { $bins } خانة
       *[other] { $bins } خانة
    }، من { $min } إلى { $max }
root-axis-variable = { $quantity }: { $bins ->
        [zero] لا خانات
        [one] خانة واحدة
        [two] خانتان
        [few] { $bins } خانات
        [many] { $bins } خانة
       *[other] { $bins } خانة
    } بعرض متغير، من { $min } إلى { $max }
label-root-mean-rms = المتوسط وRMS { $axis }
root-mean-rms = { $mean } ± { $rms }
# Trees (event data) are listed only, with their entry counts.
label-root-trees = الأشجار
root-tree = { $name } ({ $entries ->
        [zero] لا مُدخلات
        [one] مُدخل واحد
        [two] مُدخلان
        [few] { $entries } مُدخلات
        [many] { $entries } مُدخلًا
       *[other] { $entries } مُدخل
    })
# machine-drafted, needs native review
# CASA images: what the image is (from its directory name), its restoring beam (major and
# minor axis, position angle), the beams of its planes when they differ, the rest frequency,
# spectral reference frame (e.g. "LSRK") and velocity definition of the frequency labels,
# the default mask (e.g. "mask0") and the casacore storage manager with its byte order.
label-casa-type = نوع الصورة
casa-type-image = الصورة المستعادة
casa-type-residual = البواقي
casa-type-model = النموذج
casa-type-pb = الحزمة الأولية
casa-type-mask = قناع التنظيف
casa-type-psf = دالة انتشار النقطة
casa-type-sumwt = مجموع الأوزان
casa-type-weight = الأوزان
label-casa-beam = حزمة الاستعادة
casa-beam = { $major }″ × { $minor }″, زاوية الموضع { $pa }°
label-casa-plane-beams = الحزم لكل مستوى
casa-plane-beams = { $count ->
        [zero] { $count } حزمة
        [one] { $count } حزمة واحدة
        [two] { $count } حزمتان
        [few] { $count } حزم
        [many] { $count } حزمة
       *[other] { $count } حزمة
    }، المحور الأكبر من { $min }″ إلى { $max }″
label-casa-rest-frequency = تردد السكون
casa-ghz = { $ghz } GHz
label-casa-spectral-frame = الإطار المرجعي الطيفي
label-casa-velocity = تعريف السرعة
casa-velocity-radio = راديوي
casa-velocity-optical = بصري
casa-velocity-relativistic = نسبوي
label-casa-mask = القناع الافتراضي
label-casa-storage = التخزين
casa-storage-big = { $manager }، big-endian
casa-storage-little = { $manager }، little-endian
# machine-drafted, needs native review
# Unstructured meshes: the mesh's name in the file, the convention describing it (UGRID,
# ICON, MPAS), its counts, face shapes, coordinates and where the values sit.
label-ugrid-name = الشبكة
label-ugrid-convention = الاصطلاح
label-ugrid-nodes = العُقد
label-ugrid-faces = الأوجه
label-ugrid-edges = الحواف
label-ugrid-shapes = أشكال الأوجه
label-ugrid-coordinates = الإحداثيات
label-ugrid-values = موضع القيم
label-ugrid-split = مقسّمة للرسم
# $n is the formatted count, $count the number (for the plural).
ugrid-triangles = { $count ->
    [zero] { $count } مثلث
    [one] { $count } مثلث
    [two] { $count } مثلثان
    [few] { $count } مثلثات
    [many] { $count } مثلثًا
   *[other] { $count } مثلث
}
ugrid-quads = { $count ->
    [zero] { $count } شكل رباعي
    [one] { $count } شكل رباعي
    [two] { $count } شكلان رباعيان
    [few] { $count } أشكال رباعية
    [many] { $count } شكلًا رباعيًا
   *[other] { $count } شكل رباعي
}
# Faces of five or more nodes; $max is the most nodes of one face.
ugrid-polygons = { $count ->
    [zero] { $count } مضلع بحد أقصى { $max } عقدة
    [one] { $count } مضلع من { $max } عقدة
    [two] { $count } مضلعان بحد أقصى { $max } عقدة
    [few] { $count } مضلعات بحد أقصى { $max } عقدة
    [many] { $count } مضلعًا بحد أقصى { $max } عقدة
   *[other] { $count } مضلع بحد أقصى { $max } عقدة
}
ugrid-lonlat = خط الطول وخط العرض
ugrid-projected = x وy المُسقَطان
ugrid-projected-unit = x وy المُسقَطان ({ $unit })
ugrid-on-faces = الأوجه (مرسومة بلون موحّد)
ugrid-on-nodes = العُقد (مُستكمَلة عبر الأوجه)
# A mesh file without data variables: the image shows how many nodes each face has.
ugrid-grid-only = لا شيء: تعرض الصورة عدد العُقد في كل وجه
# Faces cut at the antimeridian or closed to a pole so they draw on a flat map.
ugrid-split = { $count ->
    [zero] { $count } وجه عند خط الزوال المقابل أو أحد القطبين
    [one] { $count } وجه عند خط الزوال المقابل أو أحد القطبين
    [two] { $count } وجهان عند خط الزوال المقابل أو أحد القطبين
    [few] { $count } أوجه عند خط الزوال المقابل أو أحد القطبين
    [many] { $count } وجهًا عند خط الزوال المقابل أو أحد القطبين
   *[other] { $count } وجه عند خط الزوال المقابل أو أحد القطبين
}
# machine-drafted, needs native review
# Slide annotations (NDP.view, Aperio ImageScope, ASAP sidecar files).
label-annotation-file = الملف
# "slide.xml (Aperio ImageScope)"
annotation-source = { $file } ({ $format })
label-annotation-shapes = الأشكال
label-annotation-groups = المجموعات
# Shapes or points over the reader's limits, left out.
label-annotation-skipped = المحذوفة
annotation-polygon = مضلع
annotation-polyline = خط
annotation-rectangle = مستطيل
annotation-ellipse = قطع ناقص
annotation-point = نقطة
annotation-points = نقاط
annotation-ruler = مسطرة
annotation-arrow = سهم
# The list of annotations is cut short: "and 1,200 more".
annotation-more = و{ $count } أخرى
# All-fields group title: "Annotations in slide.xml".
fields-annotations-header = التعليقات التوضيحية في { $file }
label-drawing-version = الإصدار
label-drawing-units = الوحدات
label-drawing-extents = الامتداد
label-drawing-sheet = الورقة
label-drawing-layouts = التخطيطات
label-drawing-layers = الطبقات
label-drawing-entities = الكيانات
label-drawing-codepage = صفحة الترميز
label-drawing-missing-fonts = الخطوط البديلة
# machine-drafted, needs native review
# A font file the drawing names that was not found: the styles naming it, and the font drawing its text instead.
drawing-missing-font = { $file } (الأنماط { $styles })، رُسم بخط { $substitute }
# A Big Font (double-byte text) that was not found; its characters come from other fonts.
drawing-missing-bigfont = { $file } (الأنماط { $styles })، رُسم بخطوط احتياطية
# Survey data (LandXML).
label-survey-crs = نظام الإحداثيات
survey-crs-none = غير مذكور
label-survey-project = اسم المشروع
label-survey-application = كُتب بواسطة
label-survey-surface = نموذج سطح الأرض
# A TIN surface: its name, counts, plan and sloped areas, the volume above its lowest point, its elevations and the contour interval drawn; unit is "m" or "ft" (areas in square units, volume in cubic units).
survey-surface = { $name }: عدد النقاط { $points }، عدد المثلثات { $triangles }؛ المساحة الأفقية { $area } { $unit }²، المساحة السطحية { $area3d } { $unit }²؛ { $volume } { $unit }³ فوق أدنى نقطة؛ المناسيب { $zmin }–{ $zmax } { $unit }؛ خطوط الكنتور كل { $interval } { $unit }
label-survey-alignment = المحور
# An alignment: its name, length and first and last stations ("1+234.50").
survey-alignment = { $name }: { $length } { $unit }، المحطات من { $start } إلى { $end }
# SHX shape fonts found and used for the drawing's text.
label-drawing-shx-fonts = خطوط SHX
label-drawing-warnings = المشكلات
label-plot-paper = الورق
# "2 of 5": the page shown and how many the plot has.
plot-page-of = { $page } من { $pages }
label-plot-page = الصفحة
label-plot-pens = الأقلام المستخدمة
# The printer language a plot came wrapped in, e.g. "PCL 5".
label-plot-job = مهمة الطابعة
# What a plot draws: strokes, fills and labels.
label-plot-items = العناصر المرسومة
# Width × height of what is drawn, in drawing units: "1,250 × 800 mm".
drawing-extents = { $width } × { $height } { $unit }
drawing-extents-unitless = { $width } × { $height }
drawing-model-space = النموذج
# "24, 3 off or frozen"
drawing-layers-off = { $num }، منها { $off } مطفأة أو مجمّدة
# DXF $INSUNITS.
drawing-units = { $code ->
    [1] بوصات
    [2] أقدام
    [3] أميال
    [4] مليمترات
    [5] سنتيمترات
    [6] أمتار
    [7] كيلومترات
    [8] ميكروبوصات
    [9] أجزاء من الألف من البوصة
    [10] ياردات
    [11] أنغسترومات
    [12] نانومترات
    [13] ميكرونات
    [14] ديسيمترات
    [15] ديكامترات
    [16] هكتومترات
    [17] جيجامترات
    [18] وحدات فلكية
    [19] سنوات ضوئية
    [20] فراسخ فلكية
    [21] أقدام المسح الأمريكية
   *[other] بلا وحدة
}
# All-fields group title: "DXF entities".
fields-drawing-entities = كيانات { $format }

## تخطيطات الدوائر المتكاملة (GDSII)

# machine-drafted, needs native review
label-layout-library = المكتبة
label-layout-units = الوحدات
# The database unit (the grid coordinates are stored in) and the user unit, with SI symbols:
# "1 nm database unit, 1 µm user unit".
layout-units = { $db } وحدة قاعدة البيانات، { $user } وحدة المستخدم
# The top cell shown (each top cell is a page).
label-layout-cell = الخلية
label-layout-cells = الخلايا
# Cells no other cell places.
label-layout-top-cells = الخلايا العليا
# "TOP, TEST, PAD and 12 more"
layout-top-cells-more = { $names } و{ $more } أخرى
# Layer/datatype pairs.
label-layout-layers = الطبقات
# Polygons, paths and boxes, as stored (each cell once).
label-layout-shapes = الأشكال
label-layout-texts = التسميات
# Cell references and the placements they make: "1,204 (88,320 placements)".
label-layout-references = المراجع
layout-references = { $refs } ({ $placements } { $placements ->
    [zero] موضع
    [one] موضع
    [two] موضع
    [few] مواضع
    [many] موضعًا
   *[other] موضع
})
# Shapes with every cell placement expanded.
label-layout-flat-shapes = الأشكال بعد التسطيح
label-layout-bounds = الحدود
# Corners of what the cell draws, micrometres: "-12.5, 0 to 2,900, 3,500 µm".
layout-bounds = { $x0 }، { $y0 } إلى { $x1 }، { $y1 } µm

## اللوحات الإلكترونية والمخططات (Gerber, Excellon, KiCad)

# machine-drafted, needs native review
group-board = اللوحة الإلكترونية
group-schematic = المخطط
# The board view shown (its layers, its top or bottom), or the schematic sheet.
label-pcb-view = العرض
label-pcb-project = المشروع
label-pcb-revision = المراجعة
label-pcb-date = التاريخ
# The CAD program that wrote the file.
label-pcb-generator = أُنشئ باستخدام
# Width × height of the board outline: "100 × 80 mm".
label-pcb-size = حجم اللوحة
label-pcb-thickness = السماكة
label-pcb-copper-layers = طبقات النحاس
label-pcb-sheets = الأوراق
# The board's layers or files, each with its role: "board-F_Cu.gbr (Top copper)".
label-pcb-layers = الطبقات
pcb-layer-role = { $name } ({ $role })
label-pcb-components = البصمات
label-pcb-symbols = الرموز
label-pcb-nets = الشبكات
label-pcb-tracks = المسارات
label-pcb-wires = الأسلاك
label-pcb-vias = الثقوب الموصلة
# Drilled holes and how many drill sizes: "1,204 in 9 sizes".
label-pcb-holes = الثقوب
pcb-holes = { $holes } بـ { $sizes } { $sizes ->
    [zero] مقاس
    [one] مقاس
    [two] مقاس
    [few] مقاسات
    [many] مقاسًا
   *[other] مقاس
}
label-pcb-apertures = الفتحات
# What a board layer or file is, shown after its name.
pcb-role-inner-copper = نحاس داخلي { $n }
pcb-role-top-copper = النحاس العلوي
pcb-role-bottom-copper = النحاس السفلي
pcb-role-top-solder-mask = قناع اللحام العلوي
pcb-role-bottom-solder-mask = قناع اللحام السفلي
pcb-role-top-silkscreen = الطباعة الحريرية العلوية
pcb-role-bottom-silkscreen = الطباعة الحريرية السفلية
pcb-role-top-paste = المعجون العلوي
pcb-role-bottom-paste = المعجون السفلي
pcb-role-top-solder-paste = معجون اللحام العلوي
pcb-role-bottom-solder-paste = معجون اللحام السفلي
pcb-role-top-adhesive = اللاصق العلوي
pcb-role-bottom-adhesive = اللاصق السفلي
pcb-role-top-courtyard = حيّز المكوّنات العلوي
pcb-role-bottom-courtyard = حيّز المكوّنات السفلي
pcb-role-top-fabrication = التصنيع العلوي
pcb-role-bottom-fabrication = التصنيع السفلي
pcb-role-board-outline = حدود اللوحة
pcb-role-plated-drill = ثقب مطلي
pcb-role-non-plated-drill = ثقب غير مطلي
pcb-role-drill = الثقب
pcb-role-drill-holes = ثقوب الحفر
pcb-role-mechanical = ميكانيكية
pcb-role-margin = الهامش
pcb-role-drawings = الرسومات
pcb-role-comments = التعليقات
pcb-role-engineering-change-1 = تغيير هندسي 1
pcb-role-engineering-change-2 = تغيير هندسي 2
pcb-role-vias = الثقوب الموصلة
pcb-role-other = أخرى
pcb-role-unknown = غير معروفة
label-pcb-macros = وحدات ماكرو الفتحات

## الأطياف (JCAMP-DX, SPC, mzML, mzXML, Bruker NMR)

# machine-drafted, needs native review
group-spectrum = الطيف
label-spectrum-format = التنسيق
label-spectrum-technique = التقنية
label-spectrum-title = العنوان
# The page shown: "3 of 1,204: Scan 1532".
label-spectrum-page = الصفحة
spectrum-page-of = { $page } من { $pages }: { $name }
# An axis with its unit: "Wavenumber (cm⁻¹)".
spectrum-axis = { $title } ({ $unit })
label-spectrum-x = المحور X
label-spectrum-y = المحور Y
label-spectrum-nucleus = النواة
label-spectrum-frequency = تردد المطياف
label-spectrum-solvent = المذيب
label-spectrum-pulse-program = برنامج النبضات
label-spectrum-spectra = الأطياف
label-spectrum-points = النقاط
label-spectrum-instrument = الأداة
# Who or what lab measured it.
label-spectrum-origin = المصدر
label-spectrum-date = تاريخ القياس
# How Specular made the page from the raw data (an FID transformed, phased automatically).
label-spectrum-processing = المعالجة
spectrum-technique-ir = الأشعة تحت الحمراء
spectrum-technique-raman = رامان
spectrum-technique-uv-vis = UV-Vis
spectrum-technique-fluorescence = التفلور
spectrum-technique-nmr = الرنين المغناطيسي النووي
spectrum-technique-ms = طيف الكتلة
spectrum-technique-chromatogram = كروماتوغرام
spectrum-technique-other = الطيف

## النماذج ثلاثية الأبعاد (STL, OBJ, PLY, glTF, 3MF)

# machine-drafted, needs native review
group-mesh = نموذج ثلاثي الأبعاد
label-triangles = المثلثات
label-vertices = الرؤوس
label-materials = المواد
label-textures = الخامات
label-up-axis = المحور العلوي
# Textures the model uses, some of which could not be read: "3 (1 missing)".
mesh-textures-missing = { $count } (مفقود: { $missing })

## البنى الجزيئية (PDB, mmCIF, CIF, SDF, XYZ)

# machine-drafted, needs native review
group-structure = البنية
# The entry's identifier: a PDB ID such as 1CRN, or the CIF data block's name.
label-structure-id = المُدخل
label-structure-classification = التصنيف
# How the structure was determined: X-ray diffraction, electron microscopy, solution NMR…
label-structure-method = الطريقة
# The crystallographic or cryo-EM resolution, in ångströms.
label-structure-resolution = الدقة
structure-resolution = { $value } Å
label-structure-deposited = الإيداع
label-structure-formula = الصيغة
label-structure-chains = السلاسل
# Residues (amino acids, nucleotides) in the polymer chains.
label-structure-residues = البقايا
label-structure-atoms = الذرات
# Atoms after the unit cell was filled by symmetry, and those the file lists: "96 (12 in the asymmetric unit)".
structure-atoms-cell = { $count } ({ $asymmetric } في الوحدة غير المتناظرة)
label-structure-bonds = الروابط
# Models in the file (an NMR ensemble), of which the first is shown.
label-structure-models = النماذج
# Molecules (SDF records) or data blocks in the file, of which the first is shown.
label-structure-records = السجلات
structure-first-of = { $count } (يُعرض الأول)
# Small molecules bound to the polymer, by their three-letter codes.
label-structure-ligands = الربائط
# A list cut short: "HEM, NAG, SO4 and 9 more".
structure-ligands-more = { $list } و{ $more } أخرى
label-structure-cell = خلية الوحدة
# Unit cell edge lengths (Å) and angles (degrees).
structure-cell = a = { $a } Å، b = { $b } Å، c = { $c } Å، α = { $alpha }°، β = { $beta }°، γ = { $gamma }°
label-structure-space-group = زمرة الفراغ
label-structure-secondary = البنية الثانوية
structure-ss-file = من الملف
structure-ss-computed = محسوبة من السلسلة الرئيسية

## HDF4 وHDF-EOS2

# machine-drafted, needs native review
group-hdf4 = HDF4
# The HDF library version that wrote the file, e.g. "NCSA HDF Version 4.2 Release 15".
label-hdf4-library = المكتبة
# Counts of the file's scientific datasets, raster images, groups (Vgroups) and tables (Vdatas).
label-hdf4-datasets = مجموعات البيانات
label-hdf4-images = الصور النقطية
label-hdf4-vgroups = المجموعات
label-hdf4-vdatas = الجداول
# HDF-EOS: NASA's Earth Observing System conventions on top of HDF4 (MODIS, ASTER, AIRS…).
group-hdf-eos = HDF-EOS
label-eos-version = الإصدار
# The grids, swaths and point structures the file defines.
label-eos-structures = البنى
eos-grids = { $count ->
    [zero] { $count } شبكة ({ $names })
    [one] { $count } شبكة ({ $names })
    [two] { $count } شبكتان ({ $names })
    [few] { $count } شبكات ({ $names })
    [many] { $count } شبكة ({ $names })
   *[other] { $count } شبكة ({ $names })
}
eos-swaths = { $count ->
    [zero] { $count } شريط مسح ({ $names })
    [one] { $count } شريط مسح ({ $names })
    [two] { $count } شريطا مسح ({ $names })
    [few] { $count } أشرطة مسح ({ $names })
    [many] { $count } شريط مسح ({ $names })
   *[other] { $count } شريط مسح ({ $names })
}
eos-points = { $count ->
    [zero] { $count } بنية نقطية
    [one] { $count } بنية نقطية
    [two] { $count } بنيتان نقطيتان
    [few] { $count } بنى نقطية
    [many] { $count } بنية نقطية
   *[other] { $count } بنية نقطية
}
# The grid or swath the dataset shown belongs to.
label-eos-structure = البنية
# The grid's map projection as its GCTP name, e.g. GCTP_SNSOID (sinusoidal).
label-eos-projection = الإسقاط
label-eos-grid-size = حجم الشبكة
# Columns × rows.
eos-grid-size = { $width } × { $height }
# The grid's upper-left and lower-right corners.
label-eos-corners = الزوايا
# The product's short name from the inventory metadata, e.g. MOD09A1.
label-eos-product = المنتج
# When the data was acquired.
label-eos-time = الوقت
eos-time = من { $begin } إلى { $end }

## صور كوكبية (VICAR وPDS3 وPDS4: بيانات بعثات NASA وESA)

# machine-drafted, needs native review
group-planetary = صورة كوكبية
# The body observed, e.g. MARS, Ganymede.
label-planetary-target = الهدف
label-planetary-mission = البعثة
# The spacecraft, rover or lander carrying the instrument.
label-planetary-spacecraft = المركبة الفضائية
label-planetary-start = وقت البدء
label-planetary-stop = وقت الانتهاء
# The product's identifier in the archive.
label-planetary-product = معرّف المنتج
# The map projection of a map-projected image, as the label names it.
label-planetary-projection = إسقاط الخريطة
label-planetary-resolution = الدقة
planetary-metres-per-pixel = { $metres } م لكل بكسل
planetary-pixels-per-degree = { $pixels } بكسل لكل درجة
# Latitude and longitude of the map projection's centre.
label-planetary-centre = مركز الإسقاط
planetary-lat-lon = °{ $lat }، °{ $lon }
# The latitudes and longitudes the map covers.
label-planetary-coverage = التغطية
planetary-coverage = خط العرض من °{ $south } إلى °{ $north }، خط الطول من °{ $west } إلى °{ $east }
# The body's equatorial radius the map uses.
label-planetary-radius = نصف قطر الجرم
planetary-km = { $km } كم

## OMERO (صور مفتوحة من خادم OMERO أو من Image Data Resource)

# machine-drafted, needs native review
group-omero = OMERO
label-omero-server = الخادم
# $server the server's address; $user the user name signed in with.
omero-server-user = { $server } (باسم { $user })
omero-server-public = { $server } (عام)
label-omero-image = الصورة
# $name the image's name; $id its number on the server.
omero-image = { $name } (المعرّف { $id })
# The project and dataset the image is in, as "Project › Dataset".
label-omero-project = المشروع
# The screen, plate and well, as "Screen › Plate › B4".
label-omero-screen = الفحص
label-omero-owner = المالك
# $owner the image's owner; $group the OMERO group it belongs to.
omero-owner-group = { $owner } ({ $group })
label-omero-acquired = تاريخ الالتقاط
label-omero-dimensions = الأبعاد
omero-size = { $width } × { $height }
omero-z = { $count ->
    [zero] { $count } مستوى Z
    [one] مستوى Z واحد
    [two] مستويا Z
    [few] { $count } مستويات Z
    [many] { $count } مستوى Z
   *[other] { $count } مستوى Z
}
omero-c = { $count ->
    [zero] { $count } قناة
    [one] قناة واحدة
    [two] قناتان
    [few] { $count } قنوات
    [many] { $count } قناة
   *[other] { $count } قناة
}
omero-t = { $count ->
    [zero] { $count } نقطة زمنية
    [one] نقطة زمنية واحدة
    [two] نقطتان زمنيتان
    [few] { $count } نقاط زمنية
    [many] { $count } نقطة زمنية
   *[other] { $count } نقطة زمنية
}
# The physical size of a pixel along X, Y (and Z).
label-omero-physical = حجم البكسل
label-omero-channels = القنوات
# A channel not shown in the server's rendering.
omero-channel-hidden = { $channel } (مخفية)
# How the pixels are fetched from the server: rendered tiles (JPEG) or the whole plane (PNG).
label-omero-fetched = طريقة الجلب
omero-fetched-tiles = بلاطات { $encoding } بحجم { $width } × { $height }، { $levels ->
    [zero] { $levels } مستوى
    [one] مستوى واحد
    [two] مستويان
    [few] { $levels } مستويات
    [many] { $levels } مستوى
   *[other] { $levels } مستوى
}
omero-fetched-whole = مستويات كاملة بصيغة { $encoding }

## صور تحويل الانعكاسية (PTM, RTI, Relight)

# machine-drafted, needs native review
group-rti = تحويل الانعكاسية
# The reflectance model the coefficients belong to: PTM (polynomial), HSH (hemispherical harmonics), RBF…
label-rti-basis = الأساس
label-rti-order = الرتبة
# Coefficients per colour channel.
label-rti-terms = المعاملات
# How colour is stored: LRGB (one colour, varying luminance), RGB, YCC…
label-rti-colour = ترتيب الألوان
# Coefficient images stored in the file.
label-rti-planes = المستويات
# The byte-to-coefficient mapping the file states, per coefficient.
label-rti-scale = المقياس
label-rti-bias = الإزاحة

## السحب النقطية

# machine-drafted, needs native review
group-points = سحابة نقطية
label-points = النقاط
# Points read of those in the file, when a very large cloud was evenly thinned: "32,000,000 of 120,000,000".
points-subsampled = { $loaded } من { $total }
# The LAS point data record format number (0–10).
label-point-format = تنسيق النقاط
# The coordinate reference system.
label-crs = نظام الإحداثيات
# What each point carries besides its position.
label-attributes = السمات
points-channel-rgb = RGB
points-channel-intensity = الشدة
points-channel-classification = التصنيف
points-channel-returns = الارتدادات
# ASPRS classes present, with point counts: "Ground (1,234), Building (567)".
label-classes = الفئات
# E57 scans, with point counts.
label-scans = عمليات المسح
point-class-0 = لم تُصنَّف قط
point-class-1 = غير مصنفة
point-class-2 = أرض
point-class-3 = نباتات منخفضة
point-class-4 = نباتات متوسطة
point-class-5 = نباتات عالية
point-class-6 = مبنى
point-class-7 = نقطة منخفضة (ضوضاء)
point-class-8 = نقطة مفتاحية للنموذج
point-class-9 = ماء
point-class-10 = سكة حديد
point-class-11 = سطح طريق
point-class-12 = تداخل
point-class-13 = سلك حماية
point-class-14 = سلك موصل
point-class-15 = برج نقل الكهرباء
point-class-16 = موصل أسلاك
point-class-17 = سطح جسر
point-class-18 = ضوضاء عالية
point-class-19 = هيكل علوي
point-class-20 = أرض مُتجاهَلة
point-class-21 = ثلج
point-class-22 = استبعاد زمني
# A class without a standard name, by number.
point-class-other = الفئة { $class }

## Gaussian splats

# machine-drafted, needs native review
group-splats = لطخات غاوسية (Gaussian splats)
label-splats = لطخات
# Splats shown of those in the file, when a very large file was evenly thinned: "12,000,000 of 30,000,000".
splats-shown-of = { $shown } من { $total }
# The degree (1 to 3) of the spherical harmonics that give each splat its view-dependent colour.
label-splat-harmonics = لون يعتمد على زاوية الرؤية
splats-harmonics-none = لا شيء
# $format: the file format, e.g. "SPZ".
splats-format-antialiased = { $format }، مُدرَّب مع تنعيم الحواف

## التحذيرات

# machine-drafted, needs native review
warn-not-diagnostic = ليس للاستخدام التشخيصي. Specular عارض صور عام وليس جهازًا طبيًا: عرضه غير معاير للتشخيص.

warn-gps = يسجّل هذا الملف مكان التقاط الصورة. أزِل الموقع قبل المشاركة إن كنت تريد إبقاءه خاصًا.
warn-null-island = موقع GPS هو 0°، 0° تمامًا؛ على الأرجح لم تكن الكاميرا قد حدّدت موقعها، لذا فالموقع غالبًا غير صحيح.
warn-date-future = تاريخ الالتقاط في المستقبل؛ على الأرجح ضُبطت ساعة الكاميرا بشكل خاطئ.
warn-icc-damaged = ملف تعريف الألوان المضمّن تالف وتعذّرت قراءته.
warn-metadata-failed = تعذّرت قراءة البيانات الوصفية بسبب خطأ في Specular. الصورة نفسها سليمة.
warn-orientation-conflict = يختلف EXIF عن XMP في اتجاه الصورة (EXIF: { $exif }، XMP: { $xmp }). تُستخدم قيمة EXIF.
warn-exif-damaged = تعذّرت قراءة بعض بيانات EXIF ({ $error })؛ يُعرض الباقي.
warn-xmp-damaged = بيانات XMP الوصفية مشوّهة ({ $error })؛ لم يُقرأ سوى جزء منها.

## نافذة «حول»

geonames-attribution = أسماء الأماكن: GeoNames (www.geonames.org)، مرخّصة بموجب رخصة المشاع الإبداعي نَسب المُصنَّف 4.0 (CC BY 4.0، https://creativecommons.org/licenses/by/4.0/). البحث دون اتصال بواسطة حزمة reverse_geocoder من تأليف Grant Miner (MIT أو Apache-2.0). حُوّلت البيانات إلى قائمة مدن؛ وهي مقدّمة كما هي دون أي ضمان.

## TIFF and DNG structure values (All fields)
# machine-drafted, needs native review

subfile-type = { $value ->
    [0] صورة بالدقة الكاملة
    [1] صورة بدقة مخفّضة
    [2] صفحة واحدة من صورة متعددة الصفحات
    [4] قناع الشفافية
   *[other] أخرى
}
predictor = { $value ->
    [1] لا شيء
    [2] فرق أفقي
    [3] فرق بفاصلة عائمة
   *[other] أخرى
}
extra-samples = { $value ->
    [0] غير محدد
    [1] ألفا مضروبة مسبقًا
    [2] ألفا مستقيمة
   *[other] أخرى
}
sample-format = { $value ->
    [1] عدد صحيح بلا إشارة
    [2] عدد صحيح بإشارة
    [3] فاصلة عائمة
    [4] غير معرّف
   *[other] أخرى
}

## البيانات الجغرافية المكانية (GeoTIFF, GeoJSON, Shapefile, GeoPackage, KML/KMZ, GPX)

# machine-drafted, needs native review
group-geospatial = بيانات جغرافية مكانية
label-geo-crs = نظام الإحداثيات
label-geo-datum = المرجع الجيوديسي
label-geo-projection = الإسقاط
label-geo-unit = الوحدات
label-geo-bounds = النطاق
label-geo-lonlat-bounds = خطا الطول والعرض
label-geo-pixel-size = حجم البكسل
label-geo-overviews = النظرات العامة
label-geo-nodata = قيمة انعدام البيانات
label-geo-raster-type = نوع البيانات النقطية
label-geo-vertical = المرجع الرأسي
label-geo-citation = الاستشهاد
label-geo-features = المعالم
label-geo-vertices = الرؤوس
label-geo-layer = الطبقة
label-geo-layers = الطبقات
label-geo-tiles = البلاطات
label-geo-encoding = ترميز النص
label-geo-warnings = المشكلات
# The companion files a georeference was read from ("scene.jgw, scene.prj").
label-geo-sidecars = الملفات المرافقة
# machine-drafted, needs native review
# A projection Specular cannot convert to longitude and latitude: $method is its name.
geo-projection-unsupported = { $method } (الإحداثيات غير مدعومة)
# How longitude and latitude are brought to WGS 84 (what GPS and web maps use).
label-geo-to-wgs84 = إلى WGS 84
geo-to-wgs84-same = لا حاجة
geo-to-wgs84-helmert = تحويل المرجع الجيوديسي (Helmert)
geo-to-wgs84-by-area = تحويل المرجع الجيوديسي (Helmert، حسب المنطقة)
geo-to-wgs84-unknown = غير معروف: الإحداثيات على المرجع الجيوديسي الخاص بالملف
# A coordinate system's name and its EPSG code: "WGS 84 / UTM zone 33N (EPSG:32633)".
geo-crs-epsg = { $name } (EPSG:{ $code })
geo-crs-unknown = غير مذكور
# Map bounds: the lower-left and upper-right corners.
geo-bounds = من { $x0 }، { $y0 } إلى { $x1 }، { $y1 }
# Bounds in degrees: west to east longitude, south to north latitude.
geo-lonlat-bounds = خط الطول من { $west }° إلى { $east }°، خط العرض من { $south }° إلى { $north }°
geo-pixel-size = { $x } × { $y }
geo-overviews = { $n ->
    [zero] { $n } نظرة عامة: { $sizes }
    [one] نظرة عامة واحدة: { $sizes }
    [two] نظرتان عامتان: { $sizes }
    [few] { $n } نظرات عامة: { $sizes }
    [many] { $n } نظرة عامة: { $sizes }
   *[other] { $n } نظرة عامة: { $sizes }
}
geo-pixel-is-point = البكسلات نقاط
geo-layer = { $features ->
    [zero] { $features } معلم
    [one] معلم واحد
    [two] معلمان
    [few] { $features } معالم
    [many] { $features } معلمًا
   *[other] { $features } معلم
}، { $fields ->
    [zero] { $fields } حقل
    [one] حقل واحد
    [two] حقلان
    [few] { $fields } حقول
    [many] { $fields } حقلًا
   *[other] { $fields } حقل
}
# A GeoPackage tile pyramid: its table, the zoom level shown and its tile count.
geo-tiles = { $table }، مستوى التكبير { $zoom }، { $tiles ->
    [zero] { $tiles } بلاطة
    [one] بلاطة واحدة
    [two] بلاطتان
    [few] { $tiles } بلاطات
    [many] { $tiles } بلاطة
   *[other] { $tiles } بلاطة
}
fields-geo-geokeys = مفاتيح GeoTIFF
fields-geo-gdal = بيانات GDAL الوصفية
fields-geo-wkt = نظام الإحداثيات (WKT)
fields-geo-projjson = نظام الإحداثيات (PROJJSON)
# machine-drafted, needs native review
# What a web map tile set (MBTiles, PMTiles) says about itself: container, tile type, zoom
# levels, description and other metadata.
fields-geo-tileset = مجموعة مربعات
# The credit line a map's data asks to be shown with it ("© OpenStreetMap contributors").
label-geo-attribution = الإسناد
# The header lines of a georeferenced raster format (ENVI, ASCII grid, DTED, ERDAS IMAGINE).
fields-geo-header = الترويسة
# machine-drafted, needs native review
# GIFTI files: the format version, what the file holds, the number of data arrays and the
# encoding of their values (e.g. "GZipBase64Binary").
label-gifti-version = إصدار GIFTI
label-gifti-role = المحتوى
gifti-role-surface = سطح
gifti-role-labels = تسميات لكل رأس
gifti-role-data = قيم لكل رأس
label-gifti-arrays = مصفوفات البيانات
label-gifti-encoding = الترميز
label-gifti-vertices = الرؤوس
label-gifti-triangles = المثلثات
# The brain structure, e.g. "CortexLeft".
label-gifti-structure = البنية
# A surface's geometric type, e.g. "Anatomical", "Inflated".
label-gifti-geometry = النوع الهندسي
# The coordinate transform of the vertices: $from and $to are coordinate spaces such as
# "scanner", "Talairach" or "MNI 152".
label-gifti-transform = الإحداثيات
gifti-transform = { $from } (لا حاجة إلى التحويل إلى { $to })
gifti-transform-applied = { $from } ← { $to } (مطبّق)
# The surface file a data file is drawn on, found beside it.
label-gifti-surface = معروض على
# The data array shown: its name, NIfTI intent (e.g. "NIFTI_INTENT_SHAPE") and data type.
label-gifti-array = المصفوفة
label-gifti-intent = Intent
label-gifti-data-type = نوع البيانات
# Entries of the label table (names and colours of regions).
label-gifti-labels = جدول التسميات
# CIFTI-2 files: the version, NIfTI intent (e.g. "ConnDenseScalar") and matrix size.
label-cifti-version = إصدار CIFTI
label-cifti-intent = Intent
label-cifti-matrix = المصفوفة
cifti-matrix = { $rows } × { $columns }
# The maps (pages) of the file and what they are.
label-cifti-maps = الخرائط
cifti-maps-scalars = خرائط قياسية: { $count }
cifti-maps-labels = خرائط تسميات: { $count }
cifti-maps-series = نقاط زمنية: { $count }
cifti-maps-parcels = قطع: { $count }
cifti-maps-other = { $count }
# machine-drafted, needs native review
# A dense connectivity file (.dconn.nii): every brain place's connectivity to every other.
# $seeds: how many places can be picked; $size: the bytes read when one is picked (e.g. "357 KB").
label-cifti-dense = الاتصالية
cifti-dense = نقاط البذرة: { $seeds }، يُقرأ { $size } لكل نقطة
# A time series' start and step; $unit is e.g. "second".
label-cifti-series = السلسلة
cifti-series = من { $start }، كل { $step } ({ $unit })
label-cifti-models = نماذج الدماغ
label-cifti-parcels = القطع
label-cifti-structures = البنى
label-cifti-voxels = الفوكسلات تحت القشرية
# A surface the file's values lie on: the brain structure (e.g. "CORTEX_LEFT"), its vertex
# count, and the surface file found beside the file for it, or that none was found.
label-cifti-surface = السطح
cifti-surface-found = { $structure }: الرؤوس: { $vertices }، { $file }
cifti-surface-missing = { $structure }: يلزم سطح { $vertices ->
        [zero] بلا رؤوس
        [one] برأس واحد
        [two] برأسين
        [few] بعدد { $vertices } رؤوس
        [many] بعدد { $vertices } رأسًا
       *[other] بعدد { $vertices } رأس
    } (لا يوجد بجانب الملف)
# How the file is shown: on the surfaces found beside it, or as its subcortical voxels.
label-cifti-shown = معروض كـ
cifti-shown-surfaces = أسطح ملوّنة
cifti-shown-voxels = حجم الفوكسلات تحت القشرية
# machine-drafted, needs native review
# Shown with a picked seed's row: click a surface vertex, or move the crosshair in MPR.
cifti-shown-connectivity = اتصالية نقطة البذرة المختارة
# Maps left out of the voxel volume, which would be too large with them.
label-cifti-left-out = الخرائط المستبعدة
# Tractograms: format, number of streamlines (fibre tracts) and points.
label-tract-format = الصيغة
label-tract-streamlines = خطوط الانسياب
# A large tractogram draws only some of its streamlines.
tract-streamlines-shown = { $count } (المعروض { $shown })
label-tract-points = النقاط
label-tract-mean-length = متوسط الطول
# A length in millimetres.
tract-mm = { $value } مم
# Values per point and per streamline the file stores, by name.
label-tract-scalars = القيم القياسية لكل نقطة
label-tract-properties = الخصائص لكل خط انسياب
# TrackVis: the voxel grid the tracts were made in, its voxel size and voxel order (e.g. "LAS").
label-tract-grid = الشبكة
label-tract-voxel-size = حجم الفوكسل
label-tract-voxel-order = ترتيب الفوكسلات
# How the points were brought into scanner (RAS) millimetres.
label-tract-space = الإحداثيات
tract-space-vox-to-ras = من مصفوفة الفوكسل إلى RAS
tract-space-voxel-order = من ترتيب الفوكسلات (لا توجد مصفوفة الفوكسل إلى RAS)
tract-space-scanner = فضاء الماسح
# MRtrix: the tracking step size.
label-tract-step = حجم الخطوة

## File headers (DPX, Cineon, XWD, MIFF, TIM, WSQ, JBIG)

# machine-drafted, needs native review
# What the file's own header records, for formats without Exif or XMP.
group-header = ترويسة الملف
label-header-created = تاريخ الإنشاء
# The program or person that wrote the file.
label-header-creator = المنشئ
label-header-copyright = حقوق النشر
label-header-project = المشروع
label-header-comment = تعليق
# The scanner, camera or other device the image came from.
label-header-device = الجهاز
# SMPTE time code of the frame: "01:00:00:12".
label-header-timecode = الرمز الزمني
# A film frame's edge code (KeyKode).
label-header-edge-code = رمز الحافة
label-header-frame-rate = معدل الإطارات
# Where the frame sits in its sequence.
label-header-frame = الإطار
# Film stock and slate.
label-header-film = الفيلم
# How code values relate to light: printing density, linear, BT.709…
label-header-transfer = دالة النقل
# The colour primaries the file states.
label-header-colorimetric = القياس اللوني
# The X window a screen dump was taken of.
label-header-window = النافذة
label-header-resolution = الدقة
label-header-compression = الضغط
# machine-drafted, needs native review
# JPEG XS: the profile and the level the codestream states ("Main 422.10"; "4k-1, sublevel 6 bpp").
label-header-profile = الملف التعريفي
# machine-drafted, needs native review
label-header-level = المستوى
# machine-drafted, needs native review
# JPEG XS: how many wavelet levels, horizontal and vertical ("5 horizontal, 1 vertical").
label-header-decomposition = مستويات المويجات
# machine-drafted, needs native review
# JPEG XS: the colour transform ("None", "Reversible colour transform (RCT)", "Star-Tetrix, RGGB mosaic").
label-header-colour-transform = تحويل الألوان
# machine-drafted, needs native review
# Photoshop: the artboards, by name and size ("Phone (375 × 667), Tablet (768 × 1024)").
label-header-artboards = لوحات الرسم
# machine-drafted, needs native review
# Photoshop file without a merged image, shown from its layers: the features it leaves out,
# by Photoshop's names ("Color Lookup, Knockout").
label-header-not-drawn = غير مرسوم
# machine-drafted, needs native review
# The same, for the features drawn only approximately ("Bevel & Emboss, Satin").
label-header-approximated = مرسوم تقريبيًا

## Fonts (TTF, OTF, TTC/OTC, WOFF, WOFF2)

# machine-drafted, needs native review
group-font = الخط
label-font-family = العائلة
label-font-style = النمط
# The face's place in a font collection: "2 of 4".
label-font-face = الوجه
font-face-of = { $index } من { $count }
label-font-version = الإصدار
label-font-designer = المصمم
# The font's maker (the name table's manufacturer).
label-font-manufacturer = المسبك
label-font-glyphs = الحروف الرسومية
label-font-outlines = الخطوط الخارجية
font-outlines-truetype = TrueType (تربيعية)
font-outlines-cff = PostScript (CFF، تكعيبية)
font-outlines-cff2 = PostScript (CFF2، تكعيبية)
font-outlines-bitmap = صور نقطية فقط
font-outlines-none = لا شيء
# What the font's licence flags (OS/2 fsType) allow documents that embed it to do.
label-font-embedding = التضمين
font-embedding-installable = يجوز تضمينه وتثبيته.
font-embedding-restricted = لا يجوز تضمينه.
font-embedding-preview-print = يجوز تضمينه للعرض والطباعة فقط.
font-embedding-editable = يجوز تضمينه في مستندات تُحرَّر.
font-embedding-no-subsetting = كاملًا فقط، لا كمجموعة جزئية.
font-embedding-bitmap-only = كصور نقطية فقط.
# Variation axes of a variable font: "Weight 100–900 (400)".
label-font-axes = محاور التغيير
font-axis = { $name } { $min }–{ $max } ({ $default })
label-font-instances = الأنماط المسماة
label-font-colour = حروف رسومية ملونة
font-colour-colr0 = طبقات COLR
font-colour-colr1 = تدرجات COLR v1
font-colour-sbix = صور نقطية (sbix)
font-colour-cbdt = صور نقطية (CBDT)
font-colour-svg = SVG
label-font-weight = السماكة
label-font-units-per-em = الوحدات لكل em
label-font-tables = الجداول
label-font-licence = الترخيص
label-font-licence-url = رابط الترخيص
label-font-copyright = حقوق النشر
label-font-trademark = العلامة التجارية

## Office documents (Word, Excel, PowerPoint, iWork, OpenDocument)

# machine-drafted, needs native review
group-document = المستند
label-document-kind = النوع
document-kind-word = مستند Word
document-kind-excel = مصنف Excel
document-kind-powerpoint = عرض تقديمي PowerPoint
document-kind-pages = مستند Pages
document-kind-numbers = جدول بيانات Numbers
document-kind-keynote = عرض تقديمي Keynote
document-kind-odf-text = نص OpenDocument
document-kind-odf-spreadsheet = جدول بيانات OpenDocument
document-kind-odf-presentation = عرض تقديمي OpenDocument
document-kind-odf-graphics = رسم OpenDocument
document-kind-odf-other = ملف OpenDocument
# machine-drafted, needs native review
document-kind-playlist = قائمة تشغيل
# machine-drafted, needs native review
label-playlist-items = العناصر
# Appended to the kind: "Word document, template, with macros".
document-template = قالب
document-macros = مع وحدات ماكرو
document-legacy = تنسيق Office 97–2003
label-document-preview = المعاينة
# The image format of the preview stored in the file, e.g. "JPEG image".
document-preview-image = صورة { $format }
document-preview-none = لا توجد في هذا الملف
label-document-subject = الموضوع
label-document-author = المؤلف
label-document-description = الوصف
label-document-category = الفئة
label-document-last-saved-by = آخر من حفظ
label-document-created = تاريخ الإنشاء
label-document-modified = تاريخ التعديل
label-document-printed = آخر طباعة
label-document-pages = الصفحات
label-document-slides = الشرائح
label-document-hidden-slides = الشرائح المخفية
label-document-notes = صفحات الملاحظات
label-document-sheets = الأوراق
label-document-words = الكلمات
label-document-characters = الأحرف
label-document-paragraphs = الفقرات
label-document-lines = الأسطر
label-document-editing-time = وقت التحرير
document-minutes = { $minutes } دقيقة
label-document-revision = المراجعة
label-document-application = حُفظ باستخدام
label-document-company = الشركة
label-document-template = القالب

## Books (EPUB)

# machine-drafted, needs native review
group-book = الكتاب
label-book-authors = المؤلفون
label-book-publisher = الناشر
label-book-date = تاريخ النشر
label-book-language = اللغة
label-book-pages = الصفحات
# Entries in the book's table of contents.
label-book-contents = عناصر جدول المحتويات
label-book-layout = التخطيط
book-layout-reflowable = قابل لإعادة التدفق
book-layout-fixed = تخطيط ثابت
book-right-to-left = من اليمين إلى اليسار
label-book-version = الصيغة
book-version = EPUB { $version }
label-book-fonts = الخطوط المضمّنة
book-fonts-obfuscated = { $count } ({ $obfuscated } مموّهة)
label-book-subjects = المواضيع
label-book-identifier = المعرّف
label-book-rights = الحقوق

unit-symbol-mm = مم
unit-symbol-cm = cm
unit-symbol-m = m
unit-symbol-km = km
unit-symbol-mm2 = مم²
unit-symbol-cm2 = cm²
unit-symbol-m2 = m²
unit-symbol-km2 = km²
unit-symbol-ha = ha
unit-symbol-ft = ft
unit-symbol-ft2 = ft²
unit-symbol-mi = mi
unit-symbol-mi2 = mi²
unit-symbol-ac = ac

# machine-drafted, needs native review
volume-orient-parrec = زاوية وإزاحة Philips
volume-orient-ecat = أحجام فوكسل ECAT وإزاحاتها (غير موثوقة)
label-volume-technique = التقنية
label-volume-version = إصدار التنسيق
label-volume-isotope = النظير
label-volume-tracer = المقتفي
label-volume-units = الوحدات

# Text, tables and subtitles.
label-header-encoding = مجموعة المحارف
label-header-lines = الأسطر
label-header-line-ends = نهايات الأسطر
label-header-words = الكلمات
label-header-pages = الصفحات
label-header-pictures = الصور
label-header-pictures-not-shown = صور غير معروضة
label-header-tables = الجداول
label-header-rows = صفوف الجدول
label-header-columns = الأعمدة
label-header-delimiter = الفاصل
label-header-column-names = أسماء الأعمدة
label-header-cues = الترجمات
label-header-cue-span = الترجمات من – إلى
