# Specular — Scientific images (FITS, XISF): stretch, limits, planes, channels, coordinates. العربية.

keys-mode-science = الصور العلمية
cheatsheet-science = الصور العلمية
cheatsheet-science-look = التمديد والحدود
cheatsheet-science-planes = المستويات والقنوات
# machine-drafted, needs native review
cheatsheet-science-channels-1-9 = إظهار القنوات من 1 إلى 9 أو إخفاؤها
cheatsheet-science-channels-10-16 = إظهار القنوات من 10 إلى 16 أو إخفاؤها
palette-science-stretch = التمديد التالي
palette-science-limits = الحدود التالية
palette-science-next-plane = المستوى التالي
palette-science-prev-plane = المستوى السابق
palette-science-next-axis = التنقل في محور آخر
palette-science-composite = دمج القنوات بالألوان
palette-science-channel-1 = إظهار القناة 1 أو إخفاؤها
palette-science-channel-2 = إظهار القناة 2 أو إخفاؤها
palette-science-channel-3 = إظهار القناة 3 أو إخفاؤها
palette-science-channel-4 = إظهار القناة 4 أو إخفاؤها
palette-science-channel-5 = إظهار القناة 5 أو إخفاؤها
palette-science-channel-6 = إظهار القناة 6 أو إخفاؤها
palette-science-channel-7 = إظهار القناة 7 أو إخفاؤها
palette-science-channel-8 = إظهار القناة 8 أو إخفاؤها
palette-science-channel-9 = إظهار القناة 9 أو إخفاؤها
# machine-drafted, needs native review
palette-science-channel-10 = إظهار القناة 10 أو إخفاؤها
palette-science-channel-11 = إظهار القناة 11 أو إخفاؤها
palette-science-channel-12 = إظهار القناة 12 أو إخفاؤها
palette-science-channel-13 = إظهار القناة 13 أو إخفاؤها
palette-science-channel-14 = إظهار القناة 14 أو إخفاؤها
palette-science-channel-15 = إظهار القناة 15 أو إخفاؤها
palette-science-channel-16 = إظهار القناة 16 أو إخفاؤها
palette-science-grid = إظهار شبكة الإحداثيات أو إخفاؤها
palette-science-reset = مظهر تلقائي
palette-science-level = تسوية خرائط الارتفاع (طرح المستوى المتوسط)
palette-science-wiggle = آثار الاهتزاز السيزمية (wiggle) بمساحة متغيرة، فوق الألوان
# machine-drafted, needs native review
palette-science-wireframe = حواف الشبكة غير المنتظمة (إطار سلكي)، فوق الألوان
palette-science-import-lut = جدول ألوان من ملف (ImageJ .lut)…
palette-science-hillshade = تظليل تضاريس نماذج الارتفاع (تظليل التضاريس)
palette-science-fft = طيف القدرة (FFT) للمستوى
palette-science-catalogue = تراكب فهرس مصادر (VOTable، جدول FITS، CSV)…
palette-science-clear-catalogues = إزالة الفهارس المتراكبة
# A catalogue overlaid on a sky image: its name (the table's or the file's), how many of its
# sources fall on the image, and how many of those are drawn (at most a set number).
science-catalogue-toast = { $name }: { $count ->
        [zero] لا مصادر على الصورة
        [one] مصدر واحد على الصورة
        [two] مصدران على الصورة
        [few] { $count } مصادر على الصورة
        [many] { $count } مصدرًا على الصورة
       *[other] { $count } مصدر على الصورة
    }
science-catalogue-capped = { $name }: عُرض { $shown } من أصل { $count } من المصادر على الصورة
science-catalogue-none = { $name }: لا مصادر على الصورة
science-catalogue-failed = ليس فهرسًا بمواقع سماوية: { $name }
science-catalogues-cleared = أُزيلت الفهارس
# The file dialog's filter name for catalogue files.
science-catalogue-filter = فهارس المصادر
# A GW sky map's credible region: the smallest area holding this share of the probability,
# drawn as a contour labelled e.g. "90 %".
science-credible = { PERCENT($percent) }
palette-science-ion-image = صورة أيونية عند m/z (مطيافية الكتلة)…
disabled-not-science = للصور العلمية فقط (الفلك، والمجهر الإلكتروني ومجهر المسبار الماسح)
disabled-one-plane = للصورة مستوى واحد
disabled-one-axis = لا يوجد في الصورة محور آخر للتنقل فيه
disabled-no-channels = لا توجد في الصورة قناة كهذه للدمج
disabled-no-coordinates = لا توجد في الصورة إحداثيات سماوية
disabled-not-height-map = لخرائط الارتفاع فقط (Gwyddion، Igor)
disabled-not-elevation = لنماذج الارتفاع المرجعية جغرافيًا ذات النطاق الواحد فقط
disabled-not-seismic = للبيانات السيزمية فقط (SEG-Y، Seismic Unix)
disabled-not-mass-spec = لصور مطيافية الكتلة فقط (imzML)
# machine-drafted, needs native review
disabled-not-ugrid = للشبكات غير المنتظمة فقط (NetCDF بصيغ UGRID وICON وMPAS)
disabled-not-bayer = لإطارات الكاميرات الملونة الخام فقط (FITS بنمط باير، فيديو SER)
palette-debayer = عرض إطارات الكاميرات الملونة بعد إزالة الفسيفساء أو خامًا
palette-bayer-pattern = نمط باير التالي لهذا الملف
science-debayer-on = بعد إزالة الفسيفساء ({ $pattern })
science-debayer-off = مصفوفة مرشحات الألوان الخام
science-bayer-pattern = نمط باير { $pattern }
# machine-drafted, needs native review
palette-annotations = إظهار التعليقات التوضيحية أو إخفاؤها
disabled-no-annotations = لا توجد بجانب الصورة تعليقات توضيحية (NDPA أو Aperio أو ASAP XML)
science-annotations-on = التعليقات التوضيحية ظاهرة
science-annotations-off = التعليقات التوضيحية مخفية
science-stretch-auto = تلقائي (لكل لون)
# Stretches: how values between the limits map to grey levels.
science-stretch-linear = خطي
science-stretch-sqrt = جذر تربيعي
science-stretch-log = لوغاريتمي
science-stretch-asinh = Asinh
science-stretch-histeq = معادلة المدرج التكراري
# Limits: which values show black and white. $percent is e.g. 99.5.
science-limits-zscale = Zscale
science-limits-minmax = أدنى/أقصى
science-limits-percentile = { PERCENT($percent) }
# Seismic gain: limits symmetric about zero at the $percent percentile of the amplitudes.
science-limits-symmetric = ±{ PERCENT($percent) }
# The display range the file stores (ImageJ's min and max), used until other limits are chosen.
science-limits-file = نطاق الملف
# Toasts when the look changes.
science-stretch-toast = التمديد: { $stretch }
science-limits-toast = الحدود: { $limits }
science-reset-toast = مظهر تلقائي
# An imported colour table: $name is its file name.
science-lut-toast = جدول الألوان { $name }
science-lut-failed = ليس جدول ألوان: { $name }
# The file dialog's filter name for .lut files.
science-lut-filter = جداول ألوان ImageJ
science-composite-on = دُمجت القنوات بالألوان
science-composite-off = قناة واحدة بالرمادي
science-channel-on = ظاهرة: { $channel }
science-channel-off = مخفية: { $channel }
# machine-drafted, needs native review
# A channel without a name in the file, in the shown/hidden toasts.
science-channel-unnamed = القناة { $number }
science-grid-on = شبكة الإحداثيات ظاهرة
science-grid-off = شبكة الإحداثيات مخفية
# X-ray diffraction frames: the same action shows circles of equal resolution (d-spacing).
science-rings-on = حلقات الدقة ظاهرة
science-rings-off = حلقات الدقة مخفية
# A resolution ring's label: its d-spacing in ångström, e.g. "2.5 Å".
science-ring = { $d } Å
# Seismic wiggle traces: drawn once the traces are at least 4 screen pixels apart.
science-wiggle-on = آثار الاهتزاز مفعّلة (كبّر لرؤيتها)
science-wiggle-off = آثار الاهتزاز معطّلة
# machine-drafted, needs native review
# Unstructured meshes: the edges drawn over the faces; too many to draw in view until the
# user zooms in.
science-wireframe-on = حواف الشبكة مفعّلة
science-wireframe-off = حواف الشبكة معطّلة
science-wireframe-dense = حواف الشبكة مفعّلة (كبّر لرؤيتها)
# Plane levelling (scanning probe height maps): the best-fit plane is subtracted for display.
science-level-on = تسوية المستوى مفعّلة
science-level-off = تسوية المستوى معطّلة
# Relief shading of an elevation model. $azimuth is the sun's direction in degrees clockwise
# from north, $altitude its height above the horizon in degrees.
science-hillshade-on = تظليل التضاريس مفعّل: الشمس عند { $azimuth }°، بارتفاع { $altitude }°
science-hillshade-off = تظليل التضاريس معطّل
# The power spectrum view: the plane's centred 2-D Fourier power, log scaled.
science-fft-on = طيف القدرة معروض
science-fft-off = طيف القدرة مخفي
science-axis-toast = التنقل: { $axis }
# Readout lines (bottom corner).
# The value under the pointer, with the values' unit when the file states one.
science-value = القيمة { $value }
science-value-unit = القيمة { $value } { $unit }
# machine-drafted, needs native review
science-value-flags = القيمة { $value } (البتات { $bits })
# machine-drafted, needs native review
# A class value under the pointer and its name, e.g. "Value 4 · Vegetation".
science-value-class = القيمة { $value } · { $class }
# machine-drafted, needs native review
# Sentinel-2 scene classes (L2A SCL, ESA's names) and L1C cloud masks.
s2-class-no-data = لا توجد بيانات
s2-class-defective = مشبع أو معيب
s2-class-dark = بكسلات المناطق المظلمة
s2-class-cloud-shadow = ظلال السحب
s2-class-vegetation = غطاء نباتي
s2-class-not-vegetated = بلا غطاء نباتي
s2-class-water = مياه
s2-class-unclassified = غير مصنَّف
s2-class-cloud-medium = سحب متوسطة الاحتمال
s2-class-cloud-high = سحب عالية الاحتمال
s2-class-cirrus = سحب سمحاقية رقيقة
s2-class-snow = ثلج أو جليد
s2-class-clear = صافٍ
s2-class-opaque-cloud = سحب معتمة
# Several channels under the pointer: "R 12.5  G 3.1  B 0.4".
science-values = { $values }
# Sky position under the pointer, sexagesimal then degrees.
science-radec = α { $ra }  δ { $dec }
science-radec-deg = { $ra }°  { $dec }°
# Galactic or ecliptic longitude and latitude.
science-lonlat = l { $lon }°  b { $lat }°
science-wcs-unsupported = WCS غير مدعوم: { $reason }
# machine-drafted, needs native review
# A B1950 (FK4) image: what is converted between its frame and another. Its own position
# stays B1950 in the readout. ICRS, FK4 and SAMP are names and stay as they are.
science-fk4-markers = علامات الفهرس: تحويل ICRS → FK4
science-fk4-markers-galactic = علامات الفهرس: تحويل المجري → FK4
science-fk4-samp = SAMP: تحويل FK4 → ICRS
# The look: stretch and limits.
science-look = { $stretch } · { $limits }
# Shown under the look while plane levelling is on.
science-levelled = المستوى مُسوّى
# Shown under the look while the hillshade is on.
science-hillshaded = تظليل التضاريس: الشمس عند { $azimuth }°، بارتفاع { $altitude }°
# Shown in the readout while the power spectrum is shown.
science-fft = طيف القدرة (log₁₀)
# The spectrum's value under the pointer: log₁₀ of the power.
science-fft-power = log₁₀ القدرة { $value }
# Spatial frequency under the pointer along x and y, and its unit ("1/µm", "1/px").
science-fft-frequency = التردد u { $u }  v { $v } { $unit }
# The period of that frequency: "Period 2.5 nm".
science-fft-period = الدورة { $period }
# Crystal orientation under the pointer (EBSD): Bunge Euler angles in degrees.
science-euler = φ₁ { $phi1 }  Φ { $phi }  φ₂ { $phi2 }
# The crystal phase under the pointer and its symmetry ("Iron fcc (m-3m)").
science-phase = الطور { $phase } ({ $symmetry })
# Title of the spectrum plot of the pixel under the pointer (an EDS map).
science-spectrum-title = الطيف عند { $x }، { $y }
# Physical position under the pointer (microscopy): "x 1.25 µm  y 3.5 µm".
science-position = x { $x }  y { $y }
# machine-drafted, needs native review
# CERN ROOT histograms: the bin under the pointer on one axis. $axis is the axis title
# ("p_{T}", or "x"), $range the bin's label or its edges ("20 GeV – 25 GeV"), $bin its
# number counted from 1.
science-root-bin = { $axis } { $range } (الخانة { $bin })
# A ROOT graph's point under the pointer.
science-root-point = x { $x }  y { $y }
# Under- and overflow bins (fills outside the axis ranges) hold something but are not shown.
science-root-flow = خانات التجاوز السفلي/العلوي المخفية: { $count } (المجموع { $sum })
# Bins of different widths are drawn on an even grid of pixels.
science-root-resampled = أُعيد أخذ عينات الخانات متغيرة العرض على شبكة منتظمة
# machine-drafted, needs native review
# A flow cytometry plot (FCS) under the pointer: each axis's parameter and its value.
science-cytometry-at = { $x } { $xv }  { $y } { $yv }
# A cytometry histogram under the pointer: its parameter and the value.
science-cytometry-at-x = { $x } { $xv }
# Events in the plot's bin (or the histogram's bar) under the pointer.
science-cytometry-events = { $count ->
    [zero] لا أحداث
    [one] حدث واحد
    [two] حدثان
    [few] { $count } أحداث
    [many] { $count } حدثًا
   *[other] { $count } حدث
}
# The plot's axis scales ($x, $y: Linear, Log, Logicle or Arcsinh).
science-cytometry-scales = x { $x }, y { $y }
science-cytometry-scale-x = x { $x }
# Spillover compensation is applied, from the file's keyword ($keyword, e.g. $SPILLOVER).
science-cytometry-compensated = معوَّض ({ $keyword })
science-cytometry-uncompensated = غير معوَّض
# X-ray diffraction frames: what lies under the pointer. q is the momentum transfer
# (per ångström), d the lattice spacing (ångström), 2θ the scattering angle. The symbols
# stay as they are: "q 0.512 Å⁻¹  d 12.27 Å  2θ 4.56°".
science-scatter = q { $q } Å⁻¹  d { $d } Å  2θ { $tth }°
# Seismic readout: the trace under the pointer in a 3-D volume ($inline, $crossline the
# header numbers) or a 2-D line; $trace counts from 1 in file order, $cdp is the CDP number.
science-seismic-cell = الخط الداخلي { $inline }  الخط العرضي { $crossline }
science-seismic-trace = الأثر { $trace }  CDP { $cdp }
# A cell of a 3-D volume with no trace in the file.
science-seismic-gap = لا يوجد أثر
# Two-way time (ms) or depth of the row under the pointer.
science-seismic-time = الزمن { $time } ms
science-seismic-depth = العمق { $depth } { $unit }
# The trace's CDP (or source) X and Y with the coordinate scalar applied.
science-seismic-xy = X { $x }  Y { $y }
# machine-drafted, needs native review
# Unstructured mesh readout: the face (cell) under the pointer, counting from 0 as in the
# file's connectivity, and the node nearest to the pointer when the values sit on nodes.
science-mesh-face = الوجه { $face }
science-mesh-face-node = الوجه { $face }  العقدة { $node }
# The pointer lies between the mesh's faces (land in an ocean mesh, outside its boundary).
science-mesh-outside = خارج الشبكة
# Position on an axis past the image: "Plane 3 of 40", or with its world value.
science-plane = { $axis } { $index } من { $count }
science-plane-world = { $axis } { $index } من { $count } ({ $world })
science-axis-unnamed = المستوى
science-axis-extension = الصورة
science-axis-channel = القناة
# A column of a HEALPix sky map (probability, distance…) shown as the image.
science-axis-field = الحقل
# X-ray event lists binned into an image: the bin size, in the event list's sky pixels
# ("Bin 2 of 4 (8 px)"), and the energy band of the events counted.
science-axis-bin = التجميع
science-bin-size = { $size } px
science-axis-band = الطاقة
science-band-all = كل الطاقات
# machine-drafted, needs native review
# Complex-valued arrays (NumPy, MATLAB, Zarr, TDMS): which part of each value is shown.
science-axis-part = الجزء
science-part-magnitude = المقدار
science-part-real = الجزء الحقيقي
science-part-imaginary = الجزء التخيلي
science-part-phase = الطور (راديان)
# machine-drafted, needs native review
# Complex radar (SAR) images: intensity in decibels.
science-part-db = الشدة (dB)
# machine-drafted, needs native review
# The frames of a thermal sequence (FLIR SEQ/CSQ), in time.
science-axis-frame = الإطار
science-readout = القيمة والإحداثيات
# Accessible names of the plane sliders.
science-sliders = المستويات
# The scale bar's length.
science-scale-arcsec = { $value }″
science-scale-arcmin = { $value }′
science-scale-deg = { $value }°
# A physical length, e.g. "20 nm" (electron microscopy images).
science-scale-length = { $value } { $unit }
# machine-drafted, needs native review
# A length along a named axis of a plot, e.g. "20 GeV (p_{T})".
science-scale-axis = { $length } ({ $axis })
science-scale-a11y = شريط المقياس: { $length }
# Mass spectrometry images (imzML). The image is the total ion current (each spot's summed
# intensities) until an m/z is typed; then that ion's image, its intensities summed over
# m/z ± the tolerance. $window is e.g. "760.5851 ± 5 ppm".
science-ion-tic = تيار الأيونات الكلي
science-ion-window = صورة أيونية m/z { $window }
# The m/z prompt: $text is what is typed so far (opens on the base peak under the pointer).
science-ion-prompt = صورة أيونية عند m/z: { $text }▏
# $tolerance is e.g. "10 ppm" or "0.1 Da"; a tolerance can also be typed after the m/z
# ("760.585 5 ppm"). Tab steps the tolerance; Enter on an empty prompt shows the total ion
# current again.
science-ion-prompt-hint = ± { $tolerance } · Tab السماحية · Enter عرض · Esc إلغاء
science-ion-bad = ليست قيمة m/z: { $text }
# The spot under the pointer as the file numbers it (from 1).
science-ms-spot = النقطة x { $x }  y { $y }
science-ms-spot-z = النقطة x { $x }  y { $y }  z { $z }
science-ms-no-spectrum = لا يوجد طيف في هذه النقطة
# machine-drafted, needs native review
palette-science-temperature-unit = وحدة درجة الحرارة (°C، °F، K)
palette-science-isotherm = خط تساوي الحرارة فوق أو تحت درجة الحرارة أسفل المؤشر
palette-science-thermal-parameters = الابتعاثية ودرجة الحرارة المنعكسة…
palette-science-export-temperatures = تصدير درجات الحرارة (TIFF بفاصلة عائمة أو CSV)…
palette-thermal-visible = الصورة المرئية لصورة حرارية
disabled-not-thermal = للصور الحرارية فقط
disabled-not-radiometric = يخزّن الملف درجات حرارة: لا يمكن تغيير معاملاته
disabled-no-visible-photo = لا توجد صورة مرئية للصورة الحرارية
science-temperature-unit-toast = درجات الحرارة بوحدة { $unit }
science-isotherm-above = خط تساوي الحرارة فوق { $level }
science-isotherm-below = خط تساوي الحرارة تحت { $level }
science-isotherm-off = خط تساوي الحرارة متوقف
science-thermal-emissivity-prompt = الابتعاثية: { $text }
science-thermal-reflected-prompt = درجة الحرارة المنعكسة ({ $unit }): { $text }
science-thermal-prompt-hint = Tab الحقل التالي · Enter إعادة الحساب · Esc إلغاء
science-thermal-bad = قيمة غير صالحة: { $text }
science-thermal-applied = أُعيد الحساب للابتعاثية { $emissivity } والمنعكسة { $reflected }
science-thermal-conditions = الابتعاثية { $emissivity } · المنعكسة { $reflected }
science-thermal-export-tiff = TIFF بفاصلة عائمة 32 بت
science-thermal-export-csv = CSV
science-thermal-export-done = حُفظت درجات الحرارة في { $name }
science-thermal-export-failed = تعذّر حفظ درجات الحرارة: { $reason }
# machine-drafted, needs native review
science-values-failed = تعذّر عرض القيم: { $reason }
# machine-drafted, needs native review
# Thermal sequences (FLIR SEQ/CSQ): playback and each measurement over time.
palette-science-thermal-play = تشغيل التسلسل الحراري أو إيقافه مؤقتًا
palette-science-thermal-series-csv = حفظ درجات الحرارة عبر الزمن (CSV)…
disabled-not-thermal-sequence = للتسلسلات الحرارية فقط
thermal-seq-title = درجات الحرارة عبر الزمن
# $time is seconds from the first frame, e.g. "0.12 s".
thermal-seq-frame = الإطار { $frame } من { $frames } · { $time }
thermal-seq-play = تشغيل
thermal-seq-pause = إيقاف مؤقت
thermal-seq-playing = قيد التشغيل
thermal-seq-paused = متوقف مؤقتًا
thermal-seq-save-csv = حفظ CSV…
thermal-seq-hint = أضف نقطة أو خطًا أو منطقة (M) لرسمها عبر الزمن.
thermal-seq-reading = قراءة الإطار { $done } من { $total }…
thermal-seq-failed = تعذّرت قراءة الإطارات: { $reason }
thermal-seq-no-series = لا شيء للحفظ بعد: أضف نقطة أو خطًا أو منطقة.
# For screen readers: each measurement's value at the frame shown, e.g.
# "1 Spot: 36.4 °C, 2 Rectangle: 30.1 °C".
thermal-seq-chart = مخطط: { $values }
# machine-drafted, needs native review
# The report (PDF) of the image shown: a thermal report for a thermal image, an inspection report for any other.
palette-thermal-report = تقرير (PDF)…
thermal-report-heading = تقرير حراري
thermal-report-default-title = تقرير فحص بالتصوير الحراري
thermal-report-title-label = العنوان
thermal-report-notes-label = ملاحظات
thermal-report-photo = تضمين الصورة المرئية
thermal-report-folder = إضافة الصور الحرارية الأخرى في هذا المجلد
thermal-report-save = حفظ PDF…
thermal-report-busy = جارٍ كتابة التقرير…
thermal-report-nothing = لم تُعرض درجات الحرارة بعد.
thermal-report-saved = حُفظ التقرير في { $name } ({ $count ->
    [zero] لا صور
    [one] صورة واحدة
    [two] صورتان
    [few] { $count } صور
    [many] { $count } صورة
   *[other] { $count } صورة
})
thermal-report-failed = تعذّر حفظ التقرير: { $reason }
# In the PDF.
thermal-report-notes = ملاحظات
thermal-report-measurements = القياسات
thermal-report-no-measurements = لا توجد قياسات.
thermal-report-thermal-caption = الصورة الحرارية
thermal-report-visible-caption = الصورة المرئية
thermal-report-col-measurement = القياس
thermal-report-col-min = الأدنى
thermal-report-col-max = الأقصى
thermal-report-col-mean = المتوسط
thermal-report-col-emissivity = الابتعاثية
thermal-report-whole-image = الصورة كاملة
thermal-report-camera = الكاميرا
thermal-report-lens = العدسة
thermal-report-date = تاريخ الالتقاط
thermal-report-place = المكان
thermal-report-frame = الإطار
thermal-report-emissivity = الابتعاثية
thermal-report-reflected = درجة الحرارة المنعكسة
thermal-report-distance = المسافة
thermal-report-humidity = الرطوبة
thermal-report-range = النطاق
thermal-report-format = الصيغة
thermal-report-page = الصفحة { $page } من { $pages }
