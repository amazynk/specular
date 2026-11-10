# machine-drafted, needs native review
# Specular — الاستيراد من الكاميرات والهواتف وبطاقات الذاكرة. العربية.
# تُقرأ البطاقة فقط؛ ولا تُحذف منها الملفات إلا عند الطلب وبعد نسخة متحقَّق منها.

palette-import = استيراد الصور…
disabled-no-import-source = لا توجد بطاقة ذاكرة أو كاميرا أو هاتف متصل

## الإعدادات → الاستيراد
settings-section-import = الاستيراد
settings-import-on-card = عند إدخال بطاقة
settings-import-on-card-help = ما يحدث عند توصيل بطاقة ذاكرة أو كاميرا أو هاتف فيه صور.
settings-import-on-card-ask = اسأل
settings-import-on-card-open = افتح الاستيراد
settings-import-on-card-nothing = لا تفعل شيئًا
settings-import-destination = الاستيراد إلى
settings-import-destination-help = المجلد الذي توضع فيه الصور ومقاطع الفيديو المستوردة. إن لم يُحدَّد، توضع في مجلد الصور.
settings-import-destination-unset = مجلد الصور
settings-import-folder-pattern = المجلدات
settings-import-folder-pattern-help = المجلدات التي تُنشأ داخله: {"{"}year{"}"} و{"{"}month{"}"} و{"{"}day{"}"} و{"{"}camera{"}"} و{"{"}event{"}"}، مفصولة بـ /.
settings-import-rename-pattern = إعادة التسمية
settings-import-rename-pattern-help = أسماء ملفات جديدة، كما في التحويل: {"{"}name{"}"}، {"{"}n:03{"}"}، {"{"}date:%Y%m%d{"}"}، {"{"}camera{"}"}، {"{"}event{"}"}. إن تُرك فارغًا تبقى أسماء الكاميرا.
settings-import-keep-pairs = إبقاء RAW وJPEG معًا
settings-import-keep-pairs-help = يوضع ملف RAW وملف JPEG الملتقط معه في مجلد واحد باسم واحد، حتى عند إعادة التسمية.
settings-import-low-res-videos = استيراد نسخ الفيديو منخفضة الدقة
settings-import-low-res-videos-help = تسجّل بعض الكاميرات نسخة معاينة صغيرة من كل فيديو (.LRV). عند الإيقاف تبقى على البطاقة.
settings-import-eject = الإخراج عند الانتهاء
settings-import-eject-help = يُخرج البطاقة بعد نسخ كل شيء، لتتمكن من نزعها.

## العرض عند إدخال بطاقة
import-offer = الاستيراد من { $name }؟
import-offer-button = استيراد

## عرض الاستيراد
import-title = استيراد
import-from = من { $name }
import-source-next = المصدر التالي
import-close = إغلاق
import-scanning = جارٍ البحث عن صور جديدة… { $done } من { $total }
import-scanning-start = جارٍ البحث عن صور جديدة…
import-none-new = كل ما على { $name } استُورد من قبل.
import-empty = لا توجد صور أو مقاطع فيديو على { $name }.
import-no-source = لا توجد بطاقة ذاكرة أو كاميرا أو هاتف متصل.
import-summary = { $count ->
    [zero] لا عناصر جديدة
    [one] عنصر جديد واحد
    [two] عنصران جديدان
    [few] { $count } عناصر جديدة
    [many] { $count } عنصرًا جديدًا
   *[other] { $count } عنصر جديد
} · { $size }
import-already = استُورد من قبل: { $count }
import-unreadable = { $count ->
    [zero] لم يتعذر قراءة أي ملف
    [one] تعذرت قراءة ملف واحد
    [two] تعذرت قراءة ملفين
    [few] تعذرت قراءة { $count } ملفات
    [many] تعذرت قراءة { $count } ملفًا
   *[other] تعذرت قراءة { $count } ملف
}
import-selected = محدد { $count } من { $total }
import-select-all = تحديد الكل
import-select-none = إلغاء التحديد
import-badge-pair = RAW+JPEG
import-badge-raw = RAW
import-badge-video = فيديو

import-destination = الاستيراد إلى
import-choose = اختيار…
import-folder-pattern = المجلدات
import-event = الحدث
import-event-hint = اختياري، لـ {"{"}event{"}"}
import-rename = إعادة التسمية
import-rename-hint = إن تُرك فارغًا تبقى أسماء الكاميرا
import-preview = الملف الأول: { $path }
import-pattern-error = { $error }
import-keep-pairs = إبقاء RAW وJPEG معًا
import-eject = الإخراج عند الانتهاء
import-delete = الحذف من البطاقة بعد الاستيراد
import-free = للنسخ { $size } · المساحة الفارغة { $free }
import-free-unknown = للنسخ { $size }
import-no-space = لا توجد مساحة فارغة كافية في { $folder }: يحتاج هذا الاستيراد إلى { $need } والفارغ { $free } فقط. أفرغ بعض المساحة أو اختر مجلدًا آخر.
import-start = { $count ->
    [zero] استيراد { $count } عنصر
    [one] استيراد عنصر واحد
    [two] استيراد عنصرين
    [few] استيراد { $count } عناصر
    [many] استيراد { $count } عنصرًا
   *[other] استيراد { $count } عنصر
}
import-nothing-selected = حدّد الصور المراد استيرادها

import-confirm-delete = { $count ->
    [zero] بعد النسخ والتحقق، سيُحذف { $count } ملف من { $name }. لا يمكن التراجع عن ذلك.
    [one] بعد النسخ والتحقق، سيُحذف ملف واحد من { $name }. لا يمكن التراجع عن ذلك.
    [two] بعد النسخ والتحقق، سيُحذف ملفان من { $name }. لا يمكن التراجع عن ذلك.
    [few] بعد النسخ والتحقق، ستُحذف { $count } ملفات من { $name }. لا يمكن التراجع عن ذلك.
    [many] بعد النسخ والتحقق، سيُحذف { $count } ملفًا من { $name }. لا يمكن التراجع عن ذلك.
   *[other] بعد النسخ والتحقق، سيُحذف { $count } ملف من { $name }. لا يمكن التراجع عن ذلك.
}
import-confirm-yes = استيراد وحذف
import-confirm-no = رجوع

import-progress = جارٍ استيراد { $done } من { $total } · { $size }
import-progress-eta = جارٍ استيراد { $done } من { $total } · { $size } · { $time }
import-paused = متوقف مؤقتًا · { $done } من { $total }
import-pause = إيقاف مؤقت
import-resume = استئناف
import-cancel = إلغاء
import-hide = إخفاء
import-stopping = جارٍ الإيقاف…
import-running = هناك استيراد قيد التشغيل بالفعل.

import-done = { $count ->
    [zero] استُورد { $count } ملف
    [one] استُورد ملف واحد
    [two] استُورد ملفان
    [few] استُوردت { $count } ملفات
    [many] استُورد { $count } ملفًا
   *[other] استُورد { $count } ملف
} إلى { $folder }
import-done-existed = كانت موجودة من قبل: { $count }
import-done-deleted = حُذفت من البطاقة: { $count }
import-done-failed = { $count ->
    [zero] لم يتعذر استيراد أي ملف
    [one] تعذر استيراد ملف واحد
    [two] تعذر استيراد ملفين
    [few] تعذر استيراد { $count } ملفات
    [many] تعذر استيراد { $count } ملفًا
   *[other] تعذر استيراد { $count } ملف
}
import-done-cancelled = أُلغي الاستيراد بعد { $count ->
        [zero] { $count } ملف
        [one] ملف واحد
        [two] ملفين
        [few] { $count } ملفات
        [many] { $count } ملفًا
       *[other] { $count } ملف
    }. الاستيراد مجددًا يكمل من حيث توقف.
import-done-nothing = لم يُستورد شيء جديد.
import-ejected = يمكن نزع { $name } الآن.
import-eject-failed = تعذر إخراج { $name }: { $error }
import-failed-destination = لا يمكن استخدام المجلد { $folder }: { $error }
import-ledger-failed = تعذرت قراءة قائمة الملفات المستوردة ({ $error })؛ قد تُعرض ملفات مستوردة من قبل مرة أخرى.

## قارئ الشاشة
import-a11y-group = استيراد
import-a11y-tile = { $name }، { $date }
import-a11y-tile-selected = { $name }، { $date }، محدد
