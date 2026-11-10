# machine-drafted, needs native review
# Inspection probes in the measurement tool (NDT): radiograph evaluation and crack widths.

ndt-tool = الفحص
ndt-probe = أداة الفحص: { $name }
ndt-probe-profile = مقطع على امتداد خط
ndt-probe-snr = SNR
ndt-probe-duplex-iqi = مؤشر IQI مزدوج الأسلاك
ndt-probe-wire-iqi = مؤشر IQI أحادي السلك
ndt-probe-crack = عرض الشق
ndt-probe-density = نقطة كثافة

ndt-hint-profile = اسحب خطًا لرسم القيم الرمادية على امتداده. المفتاحان [ و] يبدّلان أداة الفحص.
ndt-hint-snr = اسحب منطقة في جزء متجانس. تُكبَّر إلى 20 × 55 بكسل على الأقل.
ndt-hint-duplex-iqi = اسحب خطًا عبر كل أزواج الأسلاك، على امتداد مؤشر IQI.
ndt-hint-wire-iqi = اسحب خطًا قصيرًا عرضيًا فوق أرفع سلك يمكنك رؤيته.
ndt-hint-crack = انقر على امتداد الشق، ثم اضغط Enter. يُقرأ العرض عند 10 نقاط.
ndt-hint-density = انقر على نقطة لقراءة قيمتها الرمادية. اكتب كثافتها المعلومة للمعايرة.

ndt-refused-needs-scale = اضبط المقياس أولًا: أحجام الأسلاك تحتاج إلى المليمترات. استخدم المعايرة على طول معلوم.
ndt-refused-nothing-found = لا شيء يبرز فوق الضوضاء على امتداد هذا الخط.
ndt-refused-too-small = الصورة أصغر من منطقة SNR البالغة 20 × 55 بكسل.
ndt-refused-no-noise = لا توجد ضوضاء لقياسها في هذه المنطقة (كل البكسلات متماثلة).

ndt-reading = جارٍ القراءة…
ndt-no-pixels = تعذّرت قراءة البكسلات.

ndt-profile-range = الرمادي من { $min } إلى { $max }
ndt-profile-mean = متوسط الرمادي { $mean }
ndt-profile-density = الكثافة من { $min } إلى { $max }

ndt-snr = SNR { $value }
ndt-snrn = SNRn { $value } (SRb { $srb } µm)
ndt-snrn-needs-srb = ‏SNRn يحتاج إلى SRb: قِس مؤشر IQI مزدوج الأسلاك أو اكتب القيمة.
ndt-snr-stats = المتوسط { $mean }، σ { $sd }، المنطقة { $w } × { $h } px

ndt-duplex-result = أول زوج غير مُميَّز { $pair }: SRb { $srb } µm، عدم الحدة { $unsharpness }
ndt-duplex-all-resolved = كل الأزواج التي عُثر عليها مُميَّزة.
ndt-duplex-dips = الانخفاضات: { $dips }
# machine-drafted, needs native review
# Interpolated values at the 20 % dip (ISO 19232-5:2018).
ndt-duplex-isrb = القيم المُستكمَلة: iSRb { $isrb } µm، عدم الحدة { $unsharpness }

ndt-wire-result = السلك { $wire }، { $diameter }
ndt-wire-measured = العرض المقيس { $width } (FWHM)، أي قطر يقارب { $diameter }

ndt-crack-mean = متوسط العرض { $width }
ndt-crack-range = الأضيق { $min }، الأعرض { $max }
ndt-crack-cuts = عدد نقاط القياس: { $count }

ndt-density-value = الرمادي { $grey }، الكثافة { $density }
ndt-density-grey = الرمادي { $grey }
ndt-density-uncalibrated = لم تُعايَر الكثافة بعد: اكتب كثافات معلومة عند نقطتين.

ndt-ask-density = الكثافة المعلومة هنا: { $value } (Enter يطبّق، وEnter فارغًا يتخطّى)
ndt-ask-srb = الدقة المكانية الأساسية SRb: { $value } µm (Enter يطبّق، وEsc يتخطّى)
ndt-bad-density = اكتب كثافة من 0 إلى 10.
ndt-bad-srb = اكتب SRb بالميكرومتر، مثلًا 80.
ndt-srb-set = ضُبطت SRb على { $srb } µm لهذه الصورة.
ndt-density-one-more = حُفظت نقطة الكثافة. اكتب الكثافة عند نقطة أخرى.
ndt-density-calibrated = عُويرت الكثافة من نقطتين.
ndt-sidecar-failed = تعذّر حفظ إعدادات الفحص بجانب الملف: { $reason }

# Drone photos on the gallery's map, and KML/KMZ export.
map-height-above-takeoff = { $metres } م فوق نقطة الإقلاع
map-export-kml = حفظ الصور بصيغة KMZ أو KML…
map-export-kmz-full = حفظ الصور بصيغة KMZ مع الصور كاملة…
map-kml-heading = الاتجاه { $degrees }°
map-kml-height = { $metres } م فوق نقطة الإقلاع
map-kml-nothing = لا توجد صور ذات موقع لحفظها.
map-kml-saved = الصور المحفوظة في { $name }: { $count }.
map-kml-failed = تعذّر حفظ ملف الخريطة: { $reason }

# The inspection report (any photo), and the report's extra outputs.
inspection-report-heading = تقرير الفحص
inspection-report-default-title = تقرير الفحص
inspection-report-folder = إضافة الصور الأخرى في هذا المجلد
inspection-report-docx = حفظ مستند Word (DOCX) أيضًا
inspection-report-kmz = حفظ ملف KMZ بأماكن الصور أيضًا
inspection-report-caption = الصورة مع القياسات
inspection-report-col-value = القيمة
inspection-report-col-scale = المقياس من
inspection-report-docx-failed = تعذّرت كتابة مستند Word.
inspection-report-unreadable = تعذّرت قراءة أي من الصور.
