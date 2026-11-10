# The play queue: what plays next, repeat and shuffle, playlists (M3U, PLS, XSPF), the
# queue panel and the sound transport's buttons.

# machine-drafted, needs native review
cheatsheet-video-queue = قائمة الانتظار والتكرار والخلط
# machine-drafted, needs native review
palette-next-track = العنصر التالي
# machine-drafted, needs native review
palette-next-track-help = تشغيل العنصر التالي في قائمة الانتظار، وإلا فالملف التالي من النوع نفسه في المجلد.
# machine-drafted, needs native review
palette-prev-track = العنصر السابق
# machine-drafted, needs native review
palette-prev-track-help = تشغيل العنصر السابق. بعد 3 ثوانٍ يبدأ العنصر الحالي من أوله.
# machine-drafted, needs native review
palette-cycle-repeat = التكرار
# machine-drafted, needs native review
palette-cycle-repeat-help = التكرار متوقف، أو للكل، أو لعنصر واحد.
# machine-drafted, needs native review
palette-shuffle = الخلط
# machine-drafted, needs native review
palette-shuffle-help = التشغيل بترتيب عشوائي، كل عنصر مرة واحدة في كل جولة.
# machine-drafted, needs native review
palette-play-queue = قائمة التشغيل التالية
# machine-drafted, needs native review
palette-play-queue-help = عرض ما سيُشغَّل لاحقًا: إعادة الترتيب والإزالة والحفظ.
# machine-drafted, needs native review
palette-queue-play-next = تشغيل تاليًا
# machine-drafted, needs native review
palette-queue-play-next-help = تشغيل الملف (في المعرض: التحديد) بعد العنصر الجاري.
# machine-drafted, needs native review
palette-queue-add = إضافة إلى قائمة الانتظار
# machine-drafted, needs native review
palette-queue-add-help = إضافة الملف (في المعرض: التحديد) إلى آخر قائمة الانتظار.
# machine-drafted, needs native review
palette-save-queue = حفظ قائمة الانتظار كقائمة تشغيل…
# machine-drafted, needs native review
palette-save-queue-help = حفظ قائمة الانتظار كقائمة تشغيل بصيغة M3U8 أو XSPF.
# machine-drafted, needs native review
transport-prev-track = العنصر السابق
# machine-drafted, needs native review
transport-next-track = العنصر التالي
# machine-drafted, needs native review
queue-title = قائمة التشغيل التالية
# machine-drafted, needs native review
queue-summary = { $count ->
    [zero] لا عناصر
    [one] عنصر واحد
    [two] عنصران
    [few] { $count } عناصر
    [many] { $count } عنصرًا
   *[other] { $count } عنصر
}
# machine-drafted, needs native review
queue-summary-time = { $count ->
    [zero] لا عناصر
    [one] عنصر واحد
    [two] عنصران
    [few] { $count } عناصر
    [many] { $count } عنصرًا
   *[other] { $count } عنصر
} · { $time }
# machine-drafted, needs native review
queue-empty = قائمة الانتظار فارغة.
    أضف ملفات صوتية ومقاطع فيديو عبر «تشغيل تاليًا» أو «إضافة إلى قائمة الانتظار»، أو افتح قائمة تشغيل.
# machine-drafted, needs native review
queue-empty-short = قائمة الانتظار فارغة.
# machine-drafted, needs native review
queue-playing = قيد التشغيل
# machine-drafted, needs native review
queue-repeat = التكرار
# machine-drafted, needs native review
queue-repeat-off = التكرار: متوقف
# machine-drafted, needs native review
queue-repeat-all = التكرار: الكل
# machine-drafted, needs native review
queue-repeat-one = التكرار: عنصر واحد
# machine-drafted, needs native review
queue-shuffle = الخلط
# machine-drafted, needs native review
queue-shuffle-on = الخلط: مفعّل
# machine-drafted, needs native review
queue-shuffle-off = الخلط: متوقف
# machine-drafted, needs native review
queue-save = حفظ كقائمة تشغيل…
# machine-drafted, needs native review
queue-save-tip = حفظ قائمة الانتظار كقائمة تشغيل بصيغة M3U8 أو XSPF
# machine-drafted, needs native review
queue-clear = إفراغ…
# machine-drafted, needs native review
queue-clear-tip = إفراغ قائمة الانتظار. تبقى الملفات على القرص.
# machine-drafted, needs native review
queue-remove = إزالة من قائمة الانتظار (يبقى الملف على القرص)
# machine-drafted, needs native review
queue-network-item = عنصر شبكة، لا يُشغَّل: { $address }
# machine-drafted, needs native review
queue-artist-title = { $artist } – { $title }
# machine-drafted, needs native review
queue-at-end = لا يوجد عنصر تالٍ.
# machine-drafted, needs native review
queue-at-start = لا يوجد عنصر سابق.
# machine-drafted, needs native review
queue-missing = الملف { $name } لم يعد موجودًا. تم تخطيه.
# machine-drafted, needs native review
queue-network-not-played = عناصر الشبكة لا تُشغَّل بعد.
# machine-drafted, needs native review
queue-nothing-to-add = لا شيء لإضافته. لا يدخل قائمة الانتظار إلا الملفات الصوتية ومقاطع الفيديو وقوائم التشغيل.
# machine-drafted, needs native review
queue-added = { $count ->
    [zero] لم يُضَف أي عنصر.
    [one] أُضيف عنصر واحد إلى قائمة الانتظار.
    [two] أُضيف عنصران إلى قائمة الانتظار.
    [few] أُضيفت { $count } عناصر إلى قائمة الانتظار.
    [many] أُضيف { $count } عنصرًا إلى قائمة الانتظار.
   *[other] أُضيف { $count } عنصر إلى قائمة الانتظار.
}
# machine-drafted, needs native review
queue-added-next = { $count ->
    [zero] لا عناصر تالية.
    [one] عنصر واحد يُشغَّل تاليًا.
    [two] عنصران يُشغَّلان تاليًا.
    [few] { $count } عناصر تُشغَّل تاليًا.
    [many] { $count } عنصرًا يُشغَّل تاليًا.
   *[other] { $count } عنصر يُشغَّل تاليًا.
}
# machine-drafted, needs native review
queue-playlist-opened = في { $name }: { $count ->
    [zero] لا عناصر في قائمة الانتظار.
    [one] عنصر واحد في قائمة الانتظار.
    [two] عنصران في قائمة الانتظار.
    [few] { $count } عناصر في قائمة الانتظار.
    [many] { $count } عنصرًا في قائمة الانتظار.
   *[other] { $count } عنصر في قائمة الانتظار.
}
# machine-drafted, needs native review
queue-playlist-nothing = لا شيء في { $name } يمكن تشغيله هنا. تعرض قائمة الانتظار ما فيها.
# machine-drafted, needs native review
queue-playlist-failed = تعذّر فتح { $name }: { $reason }
# machine-drafted, needs native review
queue-playlist-unreadable = تعذّرت قراءة الملف.
# machine-drafted, needs native review
queue-playlist-damaged = ليست قائمة تشغيل يستطيع Specular قراءتها.
# machine-drafted, needs native review
queue-clear-ask = { $count ->
    [zero] إفراغ قائمة الانتظار؟
    [one] إفراغ قائمة الانتظار (عنصر واحد)؟
    [two] إفراغ قائمة الانتظار (عنصران)؟
    [few] إفراغ قائمة الانتظار ({ $count } عناصر)؟
    [many] إفراغ قائمة الانتظار ({ $count } عنصرًا)؟
   *[other] إفراغ قائمة الانتظار ({ $count } عنصر)؟
}
# machine-drafted, needs native review
queue-clear-detail = تُفرَغ القائمة فقط. تبقى الملفات على القرص.
# machine-drafted, needs native review
queue-clear-ok = إفراغ قائمة الانتظار
# machine-drafted, needs native review
queue-cleared = أُفرغت قائمة الانتظار.
# machine-drafted, needs native review
queue-save-title = حفظ قائمة الانتظار كقائمة تشغيل
# machine-drafted, needs native review
queue-default-name = قائمة الانتظار
# machine-drafted, needs native review
queue-format-m3u8 = قائمة تشغيل M3U8
# machine-drafted, needs native review
queue-format-xspf = قائمة تشغيل XSPF
# machine-drafted, needs native review
queue-saved = حُفظ { $name }.
# machine-drafted, needs native review
queue-save-failed = تعذّر حفظ { $name }: { $reason }
# machine-drafted, needs native review
settings-viewing-sound-play-next = تشغيل الملف الصوتي التالي
# machine-drafted, needs native review
settings-viewing-sound-play-next-help = عند انتهاء ملف صوتي، يُشغَّل الملف الصوتي التالي في المجلد.
# machine-drafted, needs native review
settings-viewing-play-repeat = التكرار
# machine-drafted, needs native review
settings-viewing-play-repeat-help = للملفات الصوتية ومقاطع الفيديو وقائمة الانتظار.
# machine-drafted, needs native review
settings-viewing-play-repeat-off = متوقف
# machine-drafted, needs native review
settings-viewing-play-repeat-all = الكل
# machine-drafted, needs native review
settings-viewing-play-repeat-one = عنصر واحد
# machine-drafted, needs native review
settings-viewing-play-shuffle = الخلط
# machine-drafted, needs native review
settings-viewing-play-shuffle-help = التشغيل بترتيب عشوائي، كل عنصر مرة واحدة في كل جولة.
