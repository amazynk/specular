# Compare: split, signed difference, flicker, alignment, metrics and the pixel probe
# (DESIGN.md §6.3).

## Actions (palette, cheat sheet, remote control)
palette-compare-with = مقارنة مع…
palette-compare-with-help = مقارنة هذه الصورة بصورة أخرى تختارها.
palette-compare-side = مقارنة جنبًا إلى جنب
palette-compare-side-help = عرض الصور المقارنة بجانب بعضها.
palette-compare-split = مقارنة بفاصل
palette-compare-split-help = عرض الصورة الثانية فوق الأولى، مقطوعة عند فاصل يمكنك سحبه.
palette-compare-split-direction = تدوير الفاصل
palette-compare-split-direction-help = جعل الفاصل أفقيًا بدل عمودي، أو العكس.
palette-compare-overlay = مقارنة بالتراكب
palette-compare-overlay-help = عرض الصورة الثانية فوق الأولى، ممزوجة.
palette-compare-difference = مقارنة الفرق
palette-compare-difference-help = عرض مواضع اختلاف الصور؛ مرة أخرى للنوع التالي.
palette-compare-flicker = التبديل السريع بين الصور المقارنة
palette-compare-flicker-help = عرض الصورة الأخرى فورًا.
palette-compare-auto-flicker = تبديل سريع تلقائي
palette-compare-auto-flicker-help = التبديل بين الصورتين تلقائيًا، أو الإيقاف.
palette-compare-align = محاذاة الصور المقارنة
palette-compare-align-help = محاذاة صور بأحجام مختلفة: أعلى اليسار، أو في الوسط، أو بالتحجيم، أو تلقائيًا.
palette-compare-metric-space = تبديل فضاء القياس
palette-compare-metric-space-help = القياس في الضوء الخطي أو في sRGB المرمّز.
palette-compare-whole-image = قياس الصورة كاملة
palette-compare-whole-image-help = قياس الصورة كاملة بدل الجزء الظاهر، أو العكس.
palette-compare-probe = مسبار البكسل للمقارنة
palette-compare-probe-help = عرض قيم الصورتين تحت المؤشر.
palette-compare-link = ربط الصور المقارنة أو فكّها
palette-compare-link-help = تكبير كل من الصورتين وتحريكها وحدها، أو معًا من جديد.
palette-compare-close = إغلاق المقارنة
palette-compare-close-help = العودة إلى المعرض.
disabled-compare-closed = افتح المقارنة أولًا
disabled-compare-not-two = يعمل مع صورتين

## Cheat sheet
cheatsheet-compare-split = فاصل أو جنبًا إلى جنب
cheatsheet-compare-split-direction = الفاصل أفقي أو عمودي
cheatsheet-compare-auto-flicker = تبديل سريع تلقائي
cheatsheet-compare-align = المحاذاة التالية
cheatsheet-compare-space = فضاء القياس
cheatsheet-compare-whole = الصورة كاملة أو الجزء الظاهر
cheatsheet-compare-probe = مسبار البكسل
cheatsheet-compare-link = ربط صورتين أو فكّهما

## Settings
settings-gallery-compare-metric-space = فضاء القياس في المقارنة
settings-gallery-compare-metric-space-help = أين تقيس المقارنة MAE و MSE و PSNR و SSIM. الضوء الخطي هو ما تعرضه الشاشة. يطابق sRGB الأدوات الأخرى في ملفات 8-bit.
settings-gallery-compare-metric-space-linear = ضوء خطي
settings-gallery-compare-metric-space-srgb = sRGB مرمّز
settings-gallery-compare-flicker-limit = أسرع تبديل سريع تلقائي
settings-gallery-compare-flicker-limit-help = أقصى عدد لتغيّر الصورة في الثانية. فوق 3 قد يسبب الوميض نوبات صرع.

## Toolbar and captions
compare-split = تقسيم
compare-split-down = فاصل عمودي
compare-split-across = فاصل أفقي
compare-link = مرتبطة
compare-auto-flicker = تلقائي
compare-flicker-rate = { $rate } تغييرات في الثانية
compare-gain = تضخيم ×{ $gain }
compare-probe = مسبار
compare-kind-signed = خطأ بإشارة
compare-kind-on-image = الخطأ فوق الصورة
compare-align-top-left = أعلى اليسار
compare-align-centre = في الوسط
compare-align-scaled = تحجيم B إلى A
compare-align-auto = محاذاة تلقائية
compare-space-linear = ضوء خطي
compare-space-srgb = sRGB
compare-space-help-linear = قيس في الضوء الخطي، كما يُعرض بعد تعيين الدرجات اللونية. PSNR هنا ليس PSNR المعتاد لملفات sRGB بعمق 8-bit.
compare-space-help-srgb = قيس على قيم sRGB المرمّزة، كما تفعل الأدوات الأخرى مع ملفات 8-bit. يُحسب HDR الذي يتجاوز أبيض SDR أبيض.
compare-space-help-common = ΔE2000 دائمًا في CIELAB (D65)، وأبيض SDR هو L* 100، و HDR مقصوص. الصور الأكبر من { $size } بكسل تُقاس من نسخة بحجم { $size } بكسل.
compare-region-view = الجزء الظاهر
compare-region-whole = الصورة كاملة
compare-ssim = SSIM { $ssim }
compare-ssim-small = يحتاج SSIM إلى 11 بكسل
compare-delta-e = ΔE { $mean } ({ PERCENT(95) }: { $p95 }، الأقصى { $max })
compare-delta-e-hdr = قُصّ HDR من أجل ΔE
compare-aligned-centre = في الوسط، قورن { $width } × { $height }
compare-aligned-scaled = حُجّمت B بمقدار { $scale }×، قورن { $width } × { $height }
compare-aligned-auto = محاذاة { $angle }° { $scale }×، قورن { $width } × { $height }
compare-aligning = جارٍ المحاذاة…
compare-align-failed = تعذّرت محاذاة هذه الصور. العودة إلى أعلى اليسار.
compare-split-label = { $b } فوق { $a }، الفاصل عند { PERCENT($percent) }
compare-showing-which = يُعرض { $which }: { $name }
compare-auto-flicker-on = تبديل سريع، { $rate } تغييرات في الثانية
compare-auto-flicker-off = توقف التبديل السريع
compare-linked = الصور مرتبطة
compare-unlinked = فُكّ ربط الصور: كل صورة تتحرك وحدها
compare-with-title = مقارنة مع
compare-probe-delta-e = ΔE2000
compare-probe-a11y = A: { $a }. B: { $b }. ΔE2000 { $delta }.
compare-a11y-divider = فاصل التقسيم
compare-a11y-space = فضاء القياس
compare-a11y-align = المحاذاة
