# Specular — Medical analysis: segmentations, SUV, fusion, TMTV, time-intensity curves. العربية. machine-drafted, needs native review

palette-segments = إظهار التقسيمات أو إخفاؤها
palette-segment-opacity = عتامة تعبئة التقسيم التالية
palette-fusion = دمج السلسلة ذات الإطار المرجعي نفسه
palette-fusion-colormap = خريطة ألوان الدمج التالية
palette-fusion-blend = عتامة الدمج التالية
palette-fusion-nudge = تحريك السلسلة المدمجة يدويًا
palette-tmtv = أداة TMTV (كرة، صندوق، إيقاف)
palette-tmtv-threshold = عتبة TMTV (SUV { NUMBER(2.5) } أو { PERCENT(41) } من SUVmax)
palette-time-curve = منحنى الشدة الزمني لآخر منطقة
palette-export-curve = حفظ منحنى الشدة الزمني بصيغة CSV

analysis-searching = لا يزال البحث جاريًا عن تقسيمات وسلاسل للدمج
segments-none = لم يُعثر على تقسيم لهذا الإطار المرجعي
segments-shown = التقسيمات ظاهرة
segments-hidden = التقسيمات مخفية
segments-fill = تعبئة التقسيم { PERCENT($percent) }
segments-failed = لم يُعرض التقسيم: { $reason }
segment-entry = { $name } · { $ml } mL

suv-value = SUVbw { $value }
suv-factor = معامل SUVbw { $factor }، بعد { $minutes } min من الحقن
suv-refused = لا SUV: { $reason }
suv-preset = SUV 0–{ $upper }
suv-refused-not-pet = ليست صورة PET
suv-refused-units = وحدات البكسل ليست Bq/mL
suv-refused-corrected = غير مصححة للتوهين والاضمحلال
suv-refused-decay = تصحيح اضمحلال غير مدعوم
suv-refused-weight = لا يوجد وزن المريض
suv-refused-dose = لا توجد الجرعة المحقونة
suv-refused-half-life = لا يوجد عمر النصف للنويدة المشعة
suv-refused-injection = لا يوجد وقت الحقن
suv-refused-scan-time = لا يوجد وقت الاقتناء
suv-refused-delay = الحقن بعد المسح
suv-refused-scale = لا يوجد معامل مقياس SUV من Philips
suv-refused-height = لا يوجد طول المريض
suv-refused-sex = لا يوجد جنس المريض
suv-refused-lbm = الكتلة الجسمية الخالية من الدهون خارج النطاق

colormap-hot-iron = حديد متوهج
colormap-rainbow = قوس قزح
colormap-viridis = Viridis
fusion-none = لا توجد سلسلة PET أو SPECT أو CT أو MR لهذا الإطار المرجعي للدمج
fusion-needed = ادمج سلسلة أولًا (Shift+U)
fusion-opening = جارٍ دمج { $modality }
fusion-off = الدمج متوقف
fusion-refused = تعذّر الدمج: { $reason }
fusion-on = { $modality } مدمج: { $colormap }، { PERCENT($blend) }
fusion-hidden = { $modality } المدمج مخفي
fusion-window-suv = نافذة الدمج SUV { $low }–{ $high }
fusion-value = { $modality } { $value }
fusion-value-suv = { $modality } SUVbw { $value }
fusion-blend-toast = عتامة الدمج { PERCENT($percent) }
fusion-moved = حُرّك يدويًا: { $x }, { $y }, { $z } مم، { $degrees }°
fusion-nudge-hint = الأسهم تحرك السلسلة المدمجة 1 مم، وShift+Up/Down على امتداد العرض، و[ و] تديرها
fusion-nudge-off = السلسلة المدمجة مثبتة

tmtv-hint-sphere = TMTV: اسحب كرة من مركزها
tmtv-hint-box = TMTV: اسحب صندوقًا
tmtv-off = TMTV متوقف
tmtv-threshold-suv = العتبة SUV { $suv }
tmtv-threshold-share = العتبة { PERCENT($share) } من SUVmax
tmtv-result = TMTV { $ml } mL في { $count ->
        [zero] لا مناطق
        [one] منطقة واحدة
        [two] منطقتين
        [few] { $count } مناطق
        [many] { $count } منطقة
       *[other] { $count } من المناطق
    }
tmtv-stats = SUVmax { $max }، SUVmean { $mean }، TLG { $tlg }
tmtv-outside = المنطقة خارج الحجم
tmtv-no-pet = يحتاج TMTV إلى حجم PET مع SUV

analysis-mpr-only = يعمل الدمج وTMTV في MPR (Shift+R)
curve-not-dynamic = ليست صورة متعددة الإطارات ولا سلسلة ديناميكية
curve-not-mpr = تُرسم منحنيات الشدة الزمنية على الصورة، خارج MPR
curve-no-roi = ارسم قطعًا ناقصًا أو مستطيلًا أولًا
curve-loading = { $count ->
        [zero] لا نقاط زمنية للقراءة…
        [one] جارٍ قراءة نقطة زمنية واحدة…
        [two] جارٍ قراءة نقطتين زمنيتين…
        [few] جارٍ قراءة { $count } نقاط زمنية…
        [many] جارٍ قراءة { $count } نقطة زمنية…
       *[other] جارٍ قراءة { $count } من النقاط الزمنية…
    }
curve-title = منحنى الشدة الزمني
curve-mean = المتوسط
curve-max = الأقصى
curve-seconds = s
curve-saved = حُفظ المنحنى
curve-none = لا يوجد منحنى للحفظ
curve-failed = لم يُنشأ المنحنى: { $reason }
