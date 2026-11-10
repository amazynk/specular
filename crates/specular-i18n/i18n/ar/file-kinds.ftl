# Specular — File kinds: which format a shared extension is, Open as…, remembered choices. العربية.

## Which kind of file? Files whose extension several formats share.

# machine-drafted, needs native review
file-kind-title = ما نوع الملف { $file }؟
# machine-drafted, needs native review
file-kind-why-content = يناسب محتواه أكثر من تنسيق واحد.
# machine-drafted, needs native review
file-kind-why-name = لا يدلّ محتواه على ذلك. هذه التنسيقات تستخدم ملفات .{ $ext }.
# machine-drafted, needs native review
file-kind-why-none = لا يتعرّف أي تنسيق على محتواه.
# machine-drafted, needs native review
file-kind-certain = مؤكد
# machine-drafted, needs native review
file-kind-likely = مرجّح
# machine-drafted, needs native review
file-kind-possible = محتمل
# machine-drafted, needs native review
file-kind-guess = من الاسم فقط
# machine-drafted, needs native review
file-kind-hint-same = مثل ملفات .{ $ext } الأخرى هنا
# machine-drafted, needs native review
file-kind-hint-companions = ملفاته المعتادة بجانبه
# machine-drafted, needs native review
file-kind-scope = استخدامه لـ
# machine-drafted, needs native review
file-kind-scope-file = هذا الملف
# machine-drafted, needs native review
file-kind-scope-folder = ملفات .{ $ext } في هذا المجلد
# machine-drafted, needs native review
file-kind-scope-everywhere = كل ملفات .{ $ext }
# machine-drafted, needs native review
file-kind-later = يمكن تغييره لاحقًا عبر «فتح كـ…» في قائمة الملف.
# machine-drafted, needs native review
file-kind-open = فتح
# machine-drafted, needs native review
file-kind-raw = عيّنات خام
# machine-drafted, needs native review
file-kind-open-as-none = لا شيء آخر يقرأ { $file }.
# machine-drafted, needs native review
error-reason-ambiguous = قد يكون واحدًا من عدة تنسيقات. اختر واحدًا لفتحه.

## Open as… (the palette and the gallery's file menu)

# machine-drafted, needs native review
palette-open-as = فتح كـ…
# machine-drafted, needs native review
palette-open-as-help = اختيار التنسيق الذي يُقرأ به الملف.

## Settings › Optional decoders › File kinds

# machine-drafted, needs native review
settings-decoders-file-kinds = نسيان كل أنواع الملفات…
# machine-drafted, needs native review
settings-decoders-file-kinds-help = التنسيقات التي اخترتها لامتدادات تتشاركها عدة تنسيقات. سيُسأل مجددًا في المرة القادمة.
# machine-drafted, needs native review
file-kind-rules-intro = كل قاعدة تفتح ملفات امتداد ما بتنسيق واحد.
# machine-drafted, needs native review
file-kind-rules-none = لا قواعد بعد. اختر «ملفات .ext في هذا المجلد» أو «كل ملفات .ext» عند السؤال.
# machine-drafted, needs native review
file-kind-rule = ملفات .{ $ext }: { $format }
# machine-drafted, needs native review
file-kind-rule-folder = في { $folder }
# machine-drafted, needs native review
file-kind-rule-everywhere = في كل مكان
# machine-drafted, needs native review
file-kind-forget = نسيان…
# machine-drafted, needs native review
file-kind-forget-ask = نسيان «ملفات .{ $ext }: { $format }»؟
# machine-drafted, needs native review
file-kind-forget-detail = سيسأل { -app-name } مجددًا في المرة القادمة. لا يُغيَّر أي ملف ولا يُزال.
# machine-drafted, needs native review
file-kind-forget-ok = نسيان
# machine-drafted, needs native review
file-kind-forgotten = نُسيت ملفات .{ $ext }: { $format }.
# machine-drafted, needs native review
file-kind-forget-all-ask = { $count ->
    [zero] نسيان { $count } قاعدة لأنواع الملفات؟
    [one] نسيان قاعدة نوع الملف؟
    [two] نسيان قاعدتَي نوع الملف؟
    [few] نسيان كل القواعد الـ{ $count } لأنواع الملفات؟
    [many] نسيان كل الـ{ $count } قاعدة لأنواع الملفات؟
   *[other] نسيان كل الـ{ $count } قاعدة لأنواع الملفات؟
}
# machine-drafted, needs native review
file-kind-forgotten-all = { $count ->
    [zero] نُسيت { $count } قاعدة لأنواع الملفات.
    [one] نُسيت قاعدة نوع الملف.
    [two] نُسيت قاعدتا نوع الملف.
    [few] نُسيت { $count } قواعد لأنواع الملفات.
    [many] نُسيت { $count } قاعدة لأنواع الملفات.
   *[other] نُسيت { $count } قاعدة لأنواع الملفات.
}
# machine-drafted, needs native review
file-kind-keywords = امتداد، تنسيق، فتح كـ، أي نوع، نوع الملف

## One line on what each format is, under its name in the prompt.

# machine-drafted, needs native review
file-kind-about-other = تنسيق آخر يستخدم ملفات .{ $ext }.
# machine-drafted, needs native review
file-kind-about-raw-volume = عيّنات بلا ترويسة. تحدّد العرض والارتفاع والمقاطع ونوع العيّنة.
# machine-drafted, needs native review
file-kind-about-raw = صورة RAW من الكاميرا.
# machine-drafted, needs native review
file-kind-about-flir-fff = صورة حرارية من FLIR مع درجات الحرارة.
# machine-drafted, needs native review
file-kind-about-fits = صورة فلكية (FITS).
# machine-drafted, needs native review
file-kind-about-fit = مسار نشاط من Garmin: نقاط GPS وبيانات المستشعرات.
# machine-drafted, needs native review
file-kind-about-mif = حجم طبي MRtrix (تصوير بالرنين المغناطيسي).
# machine-drafted, needs native review
file-kind-about-map-info-mif = خريطة متجهية MapInfo مع بيانات .mid الخاصة بها.
# machine-drafted, needs native review
file-kind-about-net-cdf = بيانات علمية شبكية (المناخ، المحيطات).
# machine-drafted, needs native review
file-kind-about-hdf5 = بيانات علمية HDF5.
# machine-drafted, needs native review
file-kind-about-playlist = قائمة تشغيل صوتية (PLS).
# machine-drafted, needs native review
file-kind-about-gerber = طبقة لوحة دارات (Gerber)، مثل معجون اللحام.
# machine-drafted, needs native review
file-kind-about-ser = فيديو فلكي من كاميرا كواكب (SER).
# machine-drafted, needs native review
file-kind-about-tia = سلسلة مجهر إلكتروني (FEI TIA).
file-kind-about-pcd = سحابة نقاط ثلاثية الأبعاد (PCD).
file-kind-about-photo-cd = مسح Kodak Photo CD.
file-kind-about-zx-screen = لقطة شاشة ZX Spectrum.
file-kind-about-cpc-screen = لقطة شاشة Amstrad CPC.
file-kind-about-msx-screen = لقطة شاشة MSX.
file-kind-about-scc = تسميات توضيحية مغلقة من Scenarist.
# machine-drafted, needs native review
file-kind-about-pict = صورة Mac كلاسيكية (PICT).
# machine-drafted, needs native review
file-kind-about-hdr = صورة HDR من Radiance.
# machine-drafted, needs native review
file-kind-about-analyze = حجم طبي Analyze 7.5 مع ملف .hdr الخاص به.
# machine-drafted, needs native review
file-kind-about-envi = صورة نقطية للاستشعار عن بُعد (ENVI) مع ملف .hdr الخاص بها.
# machine-drafted, needs native review
file-kind-about-pds3 = صورة من مهمة كوكبية (PDS3) مع ملصقها.
# machine-drafted, needs native review
file-kind-about-mpeg-ps = فيديو كاميرا محمولة (تدفق برامج MPEG، MOD/TOD).
# machine-drafted, needs native review
file-kind-about-tracker = وحدة موسيقى Tracker (MOD، S3M، XM، IT، 669، MTM، STM، OKT، MED، ULT، FAR).
# machine-drafted, needs native review
file-kind-about-softimage-pic = تصيير ثلاثي الأبعاد من Softimage (PIC).
# machine-drafted, needs native review
file-kind-about-dr-halo = صورة Dr. Halo مع ملف لوحة الألوان .pal الخاص بها.
# machine-drafted, needs native review
file-kind-about-wal = خامة Quake 2 ملوّنة بلوحة ألوان اللعبة.
# machine-drafted, needs native review
file-kind-about-quick-time = فيلم QuickTime.
file-kind-about-pro-res-raw = فيديو Apple ProRes RAW: إطارات مستشعر الكاميرا.
file-kind-about-cinema-raw-light = فيديو Canon Cinema RAW Light: إطارات مستشعر الكاميرا.
# machine-drafted, needs native review
file-kind-about-gimp-brush = فرشاة GIMP.
# machine-drafted, needs native review
file-kind-about-swatch-exchange = لوحة ألوان Adobe Swatch Exchange.
# machine-drafted, needs native review
file-kind-about-aseprite = رسم Aseprite بالبكسل.
