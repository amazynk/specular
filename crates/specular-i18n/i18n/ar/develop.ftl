# machine-drafted, needs native review
# Specular — Develop (non-destructive photo editing). العربية.

# Shown once when an edit opens and some of its modules were made with a version that no
# longer renders: they were moved to the nearest version that does. $count: modules moved.
# machine-drafted, needs native review
toast-develop-edit-migrated = { $count ->
    [zero] تم تحديث { $count } وحدة في هذا التعديل إلى إصدار أحدث. قد تبدو النتيجة مختلفة قليلًا.
    [one] تم تحديث وحدة واحدة في هذا التعديل إلى إصدار أحدث. قد تبدو النتيجة مختلفة قليلًا.
    [two] تم تحديث وحدتين في هذا التعديل إلى إصدار أحدث. قد تبدو النتيجة مختلفة قليلًا.
    [few] تم تحديث { $count } وحدات في هذا التعديل إلى إصدار أحدث. قد تبدو النتيجة مختلفة قليلًا.
    [many] تم تحديث { $count } وحدةً في هذا التعديل إلى إصدار أحدث. قد تبدو النتيجة مختلفة قليلًا.
   *[other] تم تحديث { $count } وحدة في هذا التعديل إلى إصدار أحدث. قد تبدو النتيجة مختلفة قليلًا.
}
# Shown when an edit holds modules this version of Specular cannot render (made with a newer
# Specular). They are kept and saved unchanged, but skipped. $count: modules skipped.
# machine-drafted, needs native review
toast-develop-edit-newer = { $count ->
    [zero] تتطلب { $count } وحدة في هذا التعديل إصدارًا أحدث من Specular‏. تبقى محفوظة لكنها لا تُعرض.
    [one] تتطلب وحدة واحدة في هذا التعديل إصدارًا أحدث من Specular‏. تبقى محفوظة لكنها لا تُعرض.
    [two] تتطلب وحدتان في هذا التعديل إصدارًا أحدث من Specular‏. تبقيان محفوظتين لكنهما لا تُعرضان.
    [few] تتطلب { $count } وحدات في هذا التعديل إصدارًا أحدث من Specular‏. تبقى محفوظة لكنها لا تُعرض.
    [many] تتطلب { $count } وحدةً في هذا التعديل إصدارًا أحدث من Specular‏. تبقى محفوظة لكنها لا تُعرض.
   *[other] تتطلب { $count } وحدة في هذا التعديل إصدارًا أحدث من Specular‏. تبقى محفوظة لكنها لا تُعرض.
}

## Develop's controls (sliders, curve, colour wheels). Numbers come formatted in the UI
## language's digits.

# machine-drafted, needs native review
# A slider's value in exposure values. $value: "+0.70".
develop-value-ev = { $value } EV
# machine-drafted, needs native review
# machine-drafted, needs native review
# An angle. $value: "2.5".
develop-value-degrees = { $value }°
# machine-drafted, needs native review
# A colour temperature. $value: "5,000".
develop-value-kelvin = { $value } K
# machine-drafted, needs native review
# The curve editor's selected point. $index, $count: point numbers; $input, $output: "25 %".
develop-curve-point = النقطة { $index } من { $count }: الدخل { $input }، الخرج { $output }
# machine-drafted, needs native review
# The curve editor's keys, read by screen readers.
develop-curve-keys = مفاتيح الأسهم تحرّك النقطة. Page Up وPage Down يختاران نقطة أخرى. Insert يضيف نقطة، وDelete يزيلها.
# machine-drafted, needs native review
# A colour wheel with no colour.
develop-wheel-neutral = محايد
# machine-drafted, needs native review
# A colour wheel's colour. $hue: degrees; $strength: "20 %".
develop-wheel-value = درجة اللون { $hue }°، الشدة { $strength }
# machine-drafted, needs native review
# A colour wheel's keys, read by screen readers.
develop-wheel-keys = مفاتيح الأسهم تحرّك اللون. Delete يعيد ضبطه.

## Develop's panel.

# machine-drafted, needs native review
develop-title = التحميض
# machine-drafted, needs native review
develop-undo = تراجع
# machine-drafted, needs native review
develop-redo = إعادة
# machine-drafted, needs native review
develop-before-after = قبل وبعد
# machine-drafted, needs native review
develop-close = مغادرة التحميض
# machine-drafted, needs native review
develop-history = السجل
# machine-drafted, needs native review
# A card's on/off switch. $module: the module's name.
develop-module-on = { $module } قيد التشغيل
develop-card-menu = خيارات { $module }
develop-card-changed = معدّل
# machine-drafted, needs native review
# The speed-edit readout over the image. $control: "Exposure"; $value: "+0.70 EV".
develop-speed-hud = { $control }: { $value }
# machine-drafted, needs native review
develop-filmstrip = شريط الصور

## Develop's module cards and controls.

# machine-drafted, needs native review
develop-module-exposure = التعريض
develop-module-light = الإضاءة
develop-look-profile = الملف
develop-look-standard = قياسي
develop-look-film = فيلم
develop-look-natural = طبيعي
develop-module-grading = تدرج الألوان
develop-add-tool = إضافة أداة
# machine-drafted, needs native review
develop-module-white-balance = توازن اللون الأبيض
# machine-drafted, needs native review
develop-module-tone = الدرجات اللونية
# machine-drafted, needs native review
develop-module-contrast = التباين والمدى
# machine-drafted, needs native review
develop-module-colour = اللون
# machine-drafted, needs native review
develop-module-curve = المنحنى
# machine-drafted, needs native review
develop-module-crop = الاقتصاص والتقويم
# machine-drafted, needs native review
# A module this version has no card for.
develop-module-other = وحدة أخرى
# machine-drafted, needs native review
develop-exposure = التعريض
# machine-drafted, needs native review
develop-black-level = مستوى الأسود
# machine-drafted, needs native review
develop-temperature = درجة الحرارة
# machine-drafted, needs native review
develop-tint = الصبغة
# machine-drafted, needs native review
develop-highlights = الإضاءات العالية
# machine-drafted, needs native review
develop-shadows = الظلال
# machine-drafted, needs native review
develop-contrast = التباين
# machine-drafted, needs native review
develop-whites = الأبيض
# machine-drafted, needs native review
develop-blacks = الأسود
# machine-drafted, needs native review
# The view transform: how scene brightness becomes screen brightness.
# machine-drafted, needs native review
# machine-drafted, needs native review
# machine-drafted, needs native review
# machine-drafted, needs native review
# HDR card: how far above SDR white the HDR picture may go, and which picture to show.
develop-hdr = HDR
# machine-drafted, needs native review
develop-hdr-limit = حد HDR
# machine-drafted, needs native review
develop-hdr-preview = عرض
# machine-drafted, needs native review
develop-hdr-preview-hdr = HDR
# machine-drafted, needs native review
develop-hdr-preview-sdr = SDR
# machine-drafted, needs native review
develop-vibrance = الحيوية
# machine-drafted, needs native review
develop-saturation = التشبع
# machine-drafted, needs native review
develop-colour-wheels = عجلات الألوان
# machine-drafted, needs native review
develop-curve = منحنى الدرجات اللونية
# machine-drafted, needs native review
develop-module-local-contrast = التباين المحلي
# machine-drafted, needs native review
# A switch: exposure set from the photo's own brightness.
develop-auto-exposure = تعريض تلقائي
# machine-drafted, needs native review
# How far the tone equalizer's highlights and shadows spread across an area.
develop-smoothing = التنعيم
# machine-drafted, needs native review
# Local contrast: fine detail, and larger-scale contrast.
develop-texture = الملمس
# machine-drafted, needs native review
develop-clarity = الوضوح
# machine-drafted, needs native review
# Levels on the curve card: the input that becomes black, the middle and white.
develop-levels-black = نقطة الأسود
# machine-drafted, needs native review
develop-levels-mid = النقطة الوسطى
# machine-drafted, needs native review
develop-levels-white = نقطة الأبيض
# machine-drafted, needs native review
develop-module-detail = التفاصيل
# machine-drafted, needs native review
develop-module-denoise = تقليل التشويش
# machine-drafted, needs native review
develop-module-sharpen = زيادة الحدة
# machine-drafted, needs native review
develop-noise-luminance = تشويش الإضاءة
# machine-drafted, needs native review
develop-noise-detail = تفاصيل التشويش
# machine-drafted, needs native review
develop-noise-colour = تشويش الألوان
# machine-drafted, needs native review
develop-sharpen-amount = مقدار الحدة
# machine-drafted, needs native review
develop-sharpen-radius = نصف قطر الحدة
# machine-drafted, needs native review
develop-sharpen-detail = تفاصيل الحدة
# machine-drafted, needs native review
develop-sharpen-masking = التقنيع
# machine-drafted, needs native review
develop-capture-amount = حدة الالتقاط
# machine-drafted, needs native review
develop-capture-radius = نصف قطر الالتقاط
# machine-drafted, needs native review
develop-straighten = التقويم
# machine-drafted, needs native review
# History rows of the crop tool.
develop-crop = القص
# machine-drafted, needs native review
develop-crop-turn = تدوير
# machine-drafted, needs native review
develop-crop-flip = قلب
# machine-drafted, needs native review
# The lens card: the camera's own corrections (its switch) and manual ones.
develop-module-lens = تصحيحات العدسة
develop-module-optics = البصريات
# machine-drafted, needs native review
develop-lens-distortion = التشوه
# machine-drafted, needs native review
develop-lens-vignette = التظليل الطرفي
# machine-drafted, needs native review
# Colour fringes at the edges of the frame (lateral chromatic aberration), per colour.
develop-lens-tca-red = الهالة الحمراء
# machine-drafted, needs native review
develop-lens-tca-blue = الهالة الزرقاء
# machine-drafted, needs native review
# The Lensfun lens profile for raws without corrections of their own: a picker and a note.
develop-lensfun-profile = ملف تعريف العدسة
# machine-drafted, needs native review
# The picker's first option: the profile matched to the file's lens name.
develop-lensfun-auto = تلقائي
# machine-drafted, needs native review
develop-lensfun-off = بلا
# machine-drafted, needs native review
# $lens: "Canon EF 24-70mm f/2.8L II USM". Lensfun is a name; keep it.
develop-lensfun-used = Lensfun‏: { $lens }
# machine-drafted, needs native review
develop-lensfun-none-chosen = لا يوجد ملف تعريف للعدسة.
# machine-drafted, needs native review
# $lens: the lens name the file gives.
develop-lensfun-no-match = لا يوجد ملف تعريف Lensfun للعدسة { $lens }.
# machine-drafted, needs native review
develop-lensfun-no-profile = لا يوجد ملف تعريف Lensfun لهذه العدسة.
# machine-drafted, needs native review
develop-lensfun-no-name = لا يذكر الملف اسم العدسة. اخترها أعلاه.
# machine-drafted, needs native review
# $count: how many lenses match equally well (2 or more).
develop-lensfun-ambiguous = عدد العدسات المتطابقة: { $count }. اختر إحداها أعلاه.

## The crop tool over the photo (key R). Said to screen readers, or shown over the photo.

# machine-drafted, needs native review
develop-crop-open = القص. اسحب الحواف، أو Ctrl+اسحب على امتداد خط يجب أن يكون مستويًا. Enter يُبقي، Esc يعود.
# machine-drafted, needs native review
develop-crop-kept = تم الإبقاء على القص
# machine-drafted, needs native review
develop-crop-cancelled = تم التراجع عن القص
# machine-drafted, needs native review
develop-crop-reset = تمت إعادة ضبط القص والتقويم
# machine-drafted, needs native review
# $angle: "-1.5".
develop-crop-straightened = تم التقويم إلى { $angle }°
# machine-drafted, needs native review
# $aspect: "3:2", or one of the two names below.
develop-crop-aspect = نسبة الأبعاد { $aspect }
# machine-drafted, needs native review
develop-crop-aspect-free = حرة
# machine-drafted, needs native review
develop-crop-aspect-original = الأصلية
# machine-drafted, needs native review
develop-crop-turned-right = تم التدوير إلى اليمين
# machine-drafted, needs native review
develop-crop-turned-left = تم التدوير إلى اليسار
# machine-drafted, needs native review
develop-crop-flipped-horizontal = تم القلب أفقيًا
# machine-drafted, needs native review
develop-crop-flipped-vertical = تم القلب عموديًا
# machine-drafted, needs native review
# The label over the photo. $aspect: as above; $guide: one of the guide names below.
develop-crop-label = نسبة الأبعاد { $aspect } · الدليل: { $guide }
# machine-drafted, needs native review
develop-crop-keys = A نسبة الأبعاد · X تبديل · O الدليل · [ ] تدوير · H V قلب · Enter إبقاء · Esc رجوع
# machine-drafted, needs native review
develop-crop-guide-thirds = قاعدة الأثلاث
# machine-drafted, needs native review
develop-crop-guide-golden = النسبة الذهبية
# machine-drafted, needs native review
develop-crop-guide-diagonal = الأقطار
# machine-drafted, needs native review
develop-crop-guide-grid = الشبكة

## Perspective: keystone and upright (its own card; guides are drawn in the crop tool).

# machine-drafted, needs native review
develop-module-perspective = المنظور
# machine-drafted, needs native review
# Upright: how the camera's tilt is found.
develop-persp-upright = تقويم المنظور
# machine-drafted, needs native review
develop-persp-mode-off = إيقاف
# machine-drafted, needs native review
develop-persp-mode-auto = تلقائي
# machine-drafted, needs native review
develop-persp-mode-level = تسوية
# machine-drafted, needs native review
develop-persp-mode-vertical = عمودي
# machine-drafted, needs native review
develop-persp-mode-full = كامل
# machine-drafted, needs native review
develop-persp-mode-guided = موجَّه
# machine-drafted, needs native review
develop-persp-vertical = عمودي
# machine-drafted, needs native review
develop-persp-horizontal = أفقي
# machine-drafted, needs native review
develop-persp-rotate = تدوير
# machine-drafted, needs native review
develop-persp-aspect = النسبة
# machine-drafted, needs native review
develop-persp-scale = التحجيم
# machine-drafted, needs native review
develop-persp-offset-x = جانبيًا
# machine-drafted, needs native review
develop-persp-offset-y = لأعلى أو لأسفل
# machine-drafted, needs native review
# Zoom so no empty edges show.
develop-persp-auto-scale = تجنّب الحواف الفارغة
# machine-drafted, needs native review
develop-persp-guides = خطوط الإرشاد
# machine-drafted, needs native review
develop-persp-focal = البعد البؤري
# machine-drafted, needs native review
develop-persp-draw-guides = رسم خطوط الإرشاد
# machine-drafted, needs native review
# $n: guides drawn; $max: 4.
develop-persp-draw-guides-n = رسم خطوط الإرشاد ({ $n } من { $max })
# machine-drafted, needs native review
develop-persp-reset = إعادة ضبط المنظور
# machine-drafted, needs native review
# $mode: one of the mode names above.
develop-persp-mode-set = تقويم المنظور: { $mode }
# machine-drafted, needs native review
develop-persp-analysing = جارٍ البحث عن الخطوط…
# machine-drafted, needs native review
# $n: the guides drawn.
develop-persp-reset-confirm = إعادة ضبط المنظور؟ ستُزال خطوط الإرشاد ({ $n }). التراجع يعيدها.
# machine-drafted, needs native review
develop-persp-reset-ok = إعادة ضبط
# machine-drafted, needs native review
develop-persp-reset-done = تمت إعادة ضبط المنظور
# machine-drafted, needs native review
# $n: the guide's number.
develop-persp-guide-remove-confirm = إزالة خط الإرشاد { $n }؟ التراجع يعيده.
# machine-drafted, needs native review
develop-persp-guide-remove-ok = إزالة
# machine-drafted, needs native review
develop-persp-guide-removed = أُزيل خط الإرشاد
# machine-drafted, needs native review
# $vertical, $horizontal: the slider values.
develop-persp-keystone-now = عمودي { $vertical }، أفقي { $horizontal }
# machine-drafted, needs native review
# $error: what went wrong.
develop-persp-failed = فشل تقويم المنظور: { $error }
# machine-drafted, needs native review
# $mode: a mode name; the angles in degrees ("2.5").
develop-persp-solved = { $mode }: الميل { $vertical }°، الالتفاف { $horizontal }°، الدوران { $rotate }°
# machine-drafted, needs native review
# $mode: a mode name.
develop-perspective-few-lines = يحتاج { $mode } إلى خطين مستقيمين على الأقل يمكن تمييزهما. جرّب وضعًا آخر، أو ارسم خطوط إرشاد.
# machine-drafted, needs native review
# $n: 4.
develop-persp-guides-full = الحد الأقصى لخطوط الإرشاد: { $n }. اسحب طرفًا لتحريك أحدها.
# machine-drafted, needs native review
develop-persp-guides-on = خطوط الإرشاد. اسحب على امتداد خط يجب أن يكون عموديًا أو أفقيًا.
# machine-drafted, needs native review
develop-persp-guides-off = القص
# machine-drafted, needs native review
develop-persp-guide-moved = نُقل خط الإرشاد
# machine-drafted, needs native review
# $n: how many guides now.
develop-persp-guide-added = أُضيف خط الإرشاد { $n }
# machine-drafted, needs native review
develop-persp-guides-keys = اسحب للإضافة · اسحب طرفًا للتحريك · Delete يزيل · Shift+R القص
# machine-drafted, needs native review
# $n: guides drawn; $max: 4.
develop-persp-guides-label = خطوط الإرشاد: { $n } من { $max }
# machine-drafted, needs native review
# The three colour wheels.
develop-wheel-shadows = الظلال
# machine-drafted, needs native review
develop-wheel-midtones = الدرجات المتوسطة
# machine-drafted, needs native review
develop-wheel-highlights = الإضاءات العالية
# machine-drafted, needs native review
# The luminance slider under each colour wheel (read by screen readers).
develop-luminance-shadows = سطوع الظلال
# machine-drafted, needs native review
develop-luminance-midtones = سطوع الدرجات المتوسطة
# machine-drafted, needs native review
develop-luminance-highlights = سطوع الإضاءات العالية
# machine-drafted, needs native review
# The fourth colour wheel: the whole picture.
develop-wheel-global = شامل
# machine-drafted, needs native review
develop-luminance-global = السطوع الشامل
# machine-drafted, needs native review
# Colour card: colourfulness changed evenly, and the lightness of colourful areas.
develop-chroma = شدة اللون
# machine-drafted, needs native review
develop-brilliance = سطوع الألوان

## Colour mixer: hue, saturation and brightness per colour.

# machine-drafted, needs native review
develop-module-colour-mixer = مازج الألوان
# machine-drafted, needs native review
develop-colour-red = الأحمر
# machine-drafted, needs native review
develop-colour-orange = البرتقالي
# machine-drafted, needs native review
develop-colour-yellow = الأصفر
# machine-drafted, needs native review
develop-colour-green = الأخضر
# machine-drafted, needs native review
develop-colour-aqua = الأكوا
# machine-drafted, needs native review
develop-colour-blue = الأزرق
# machine-drafted, needs native review
develop-colour-purple = البنفسجي
# machine-drafted, needs native review
develop-colour-magenta = الأرجواني
# machine-drafted, needs native review
# $colour: one of the colour names above.
develop-mixer-hue = درجة لون { $colour }
# machine-drafted, needs native review
develop-mixer-saturation = تشبع { $colour }
# machine-drafted, needs native review
develop-mixer-brightness = سطوع { $colour }

## Point colour: pick a colour in the photo, then shift it and its neighbours.

# machine-drafted, needs native review
develop-module-point-colour = لون النقطة
# machine-drafted, needs native review
develop-point-pick = اختيار لون
# machine-drafted, needs native review
# On the pick button once a colour is picked.
develop-point-pick-again = اختيار لون آخر
# machine-drafted, needs native review
# Said and shown while picking.
develop-point-picking = انقر اللون المراد تغييره. Esc يلغي.
# machine-drafted, needs native review
develop-point-picked = تم اختيار اللون
# machine-drafted, needs native review
develop-point-cancelled = أُلغي الاختيار
# machine-drafted, needs native review
# How far from the picked colour changes reach: overall, then by hue, saturation and lightness.
develop-point-range = النطاق
# machine-drafted, needs native review
develop-point-hue-range = نطاق الصبغة
# machine-drafted, needs native review
develop-point-saturation-range = نطاق التشبع
# machine-drafted, needs native review
develop-point-lightness-range = نطاق الإضاءة
# machine-drafted, needs native review
develop-point-hue = إزاحة درجة اللون
# machine-drafted, needs native review
develop-point-saturation = إزاحة التشبع
# machine-drafted, needs native review
develop-point-lightness = إزاحة الإضاءة

## Black and white.

# machine-drafted, needs native review
develop-module-black-white = أبيض وأسود
# machine-drafted, needs native review
# $colour: a colour name; how light that colour turns in grey.
develop-bw-mix = سطوع { $colour }

## LUT: a colour look-up table file.

# machine-drafted, needs native review
develop-module-lut = LUT
# machine-drafted, needs native review
develop-lut-choose = اختيار ملف LUT…
# machine-drafted, needs native review
# The file chosen. $name: its file name.
develop-lut-file = الملف: { $name }
# machine-drafted, needs native review
# The file can't be used. $name: its file name.
develop-lut-unreadable = تتعذر قراءة { $name }
# machine-drafted, needs native review
# The file dialog.
develop-lut-dialog = اختيار ملف LUT
# machine-drafted, needs native review
develop-lut-kind = ملفات LUT
# machine-drafted, needs native review
# The encoding the LUT was made for.
develop-lut-space = مُعَدّ لـ
# machine-drafted, needs native review
develop-lut-space-linear = خطي (Rec.2020)
# machine-drafted, needs native review
develop-lut-space-acescg = ACES خطي (ACEScg)
# machine-drafted, needs native review
develop-lut-space-acescct = ACES لوغاريتمي (ACEScct)
# machine-drafted, needs native review
develop-lut-space-srgb = صورة (sRGB)
# machine-drafted, needs native review
develop-lut-space-rec709 = فيديو (Rec.709)
# machine-drafted, needs native review
develop-lut-interpolation = الطريقة
# machine-drafted, needs native review
develop-lut-tetrahedral = دقيقة
# machine-drafted, needs native review
develop-lut-trilinear = بسيطة
# machine-drafted, needs native review
develop-lut-strength = الشدة

## Develop's history and status.

# machine-drafted, needs native review
develop-history-original = الأصل
# machine-drafted, needs native review
# A module switched on or off. $module: its name.
develop-history-on = { $module } قيد التشغيل
# machine-drafted, needs native review
develop-history-off = { $module } متوقف
# machine-drafted, needs native review
# Said to screen readers after a click on a history row. $step: the row's name.
develop-at-original = العودة إلى الأصل
# machine-drafted, needs native review
develop-at-step = العودة إلى: { $step }
# machine-drafted, needs native review
develop-undone = تم التراجع عن: { $step }
# machine-drafted, needs native review
develop-redone = تمت إعادة: { $step }
# machine-drafted, needs native review
develop-entered = التحميض. التعديلات لا تغيّر الملف أبدًا.
# machine-drafted, needs native review
develop-left = تمت مغادرة التحميض
# machine-drafted, needs native review
develop-split-on = قبل وبعد جنبًا إلى جنب
# machine-drafted, needs native review
develop-split-off = التعديل فقط
# machine-drafted, needs native review
develop-showing-before = يُعرض الأصل
# machine-drafted, needs native review
develop-showing-after = يُعرض التعديل
# machine-drafted, needs native review
# Labels over the image.
develop-before = قبل
# machine-drafted, needs native review
develop-after = بعد
# machine-drafted, needs native review
develop-working = جارٍ التحميض…
# machine-drafted, needs native review
develop-preview-failed = يتعذر عرض التعديل لهذا الملف

## Develop's commands (palette, cheat sheet, Settings → Keys) and why they are unavailable.

# machine-drafted, needs native review
keys-mode-develop = التحميض
# machine-drafted, needs native review
# The cheat sheet's heading over the speed-edit keys.
cheatsheet-develop-speed = التعديل السريع: اضغط مطولًا ثم مرّر
# machine-drafted, needs native review
palette-develop = التحميض
# machine-drafted, needs native review
palette-develop-help = تعديل هذه الصورة دون تغيير الملف. التشغيل مرة أخرى يغادره.
# machine-drafted, needs native review
palette-develop-before-after = قبل وبعد
# machine-drafted, needs native review
palette-develop-before-after-help = عرض الأصل، أو التعديل مرة أخرى.
# machine-drafted, needs native review
palette-develop-split = قبل وبعد مقسومان
# machine-drafted, needs native review
palette-develop-split-help = عرض الأصل والتعديل جنبًا إلى جنب.
# machine-drafted, needs native review
palette-develop-panel = الانتقال إلى لوحة التحميض
# machine-drafted, needs native review
palette-develop-panel-help = نقل لوحة المفاتيح إلى اللوحة. Esc يعود.
# machine-drafted, needs native review
palette-develop-speed-exposure = تعديل سريع للتعريض
# machine-drafted, needs native review
palette-develop-speed-exposure-help = اضغط مطولًا ثم مرّر أو اضغط الأسهم لتغيير التعريض.
# machine-drafted, needs native review
palette-develop-speed-contrast = تعديل سريع للتباين
# machine-drafted, needs native review
palette-develop-speed-contrast-help = اضغط مطولًا ثم مرّر أو اضغط الأسهم لتغيير التباين.
# machine-drafted, needs native review
palette-develop-speed-highlights = تعديل سريع للإضاءات العالية
# machine-drafted, needs native review
palette-develop-speed-highlights-help = اضغط مطولًا ثم مرّر أو اضغط الأسهم لتغيير الإضاءات العالية.
# machine-drafted, needs native review
palette-develop-speed-shadows = تعديل سريع للظلال
# machine-drafted, needs native review
palette-develop-speed-shadows-help = اضغط مطولًا ثم مرّر أو اضغط الأسهم لتغيير الظلال.
# machine-drafted, needs native review
palette-develop-speed-whites = تعديل سريع للأبيض
# machine-drafted, needs native review
palette-develop-speed-whites-help = اضغط مطولًا ثم مرّر أو اضغط الأسهم لتغيير الأبيض.
# machine-drafted, needs native review
palette-develop-speed-blacks = تعديل سريع للأسود
# machine-drafted, needs native review
palette-develop-speed-blacks-help = اضغط مطولًا ثم مرّر أو اضغط الأسهم لتغيير الأسود.
# machine-drafted, needs native review
palette-develop-speed-temperature = تعديل سريع لدرجة الحرارة
# machine-drafted, needs native review
palette-develop-speed-temperature-help = اضغط مطولًا ثم مرّر أو اضغط الأسهم لتغيير درجة حرارة توازن اللون الأبيض.
# machine-drafted, needs native review
palette-develop-speed-vibrance = تعديل سريع للحيوية
# machine-drafted, needs native review
palette-develop-speed-vibrance-help = اضغط مطولًا ثم مرّر أو اضغط الأسهم لتغيير الحيوية.
# machine-drafted, needs native review
palette-develop-crop = القص والتقويم
# machine-drafted, needs native review
palette-develop-crop-help = قص الصورة أو تقويمها أو تدويرها أو قلبها. Enter يُبقي، Esc يعود.
# machine-drafted, needs native review
disabled-not-photo = للصور الثابتة فقط
# machine-drafted, needs native review
disabled-develop-off = التحميض متوقف في الملفين الشخصيين طب وعلوم
# machine-drafted, needs native review
develop-module-corrections = تصحيحات
# machine-drafted, needs native review
develop-module-raw = Raw
# machine-drafted, needs native review
develop-off = إيقاف
# machine-drafted, needs native review
develop-on = تشغيل
# machine-drafted, needs native review
develop-highlight-method = إضاءات محترقة
# machine-drafted, needs native review
develop-highlight-opposed = إعادة بناء اللون
# machine-drafted, needs native review
develop-highlight-rebuild = إعادة بناء المساحات الكبيرة
# machine-drafted, needs native review
develop-highlight-clip = قص إلى الأبيض
# machine-drafted, needs native review
develop-highlight-threshold = مستوى القص
# machine-drafted, needs native review
develop-module-hot-pixels = بكسلات ساخنة
# machine-drafted, needs native review
develop-hot-threshold = عتبة البكسلات الساخنة
# machine-drafted, needs native review
develop-module-ca = الزيغ اللوني
# machine-drafted, needs native review
develop-ca-mode = الحواف
# machine-drafted, needs native review
develop-ca-auto = تلقائي
# machine-drafted, needs native review
develop-ca-manual = يدوي
# machine-drafted, needs native review
develop-ca-red = إزاحة الأحمر
# machine-drafted, needs native review
develop-ca-blue = إزاحة الأزرق
# machine-drafted, needs native review
develop-module-defringe = إزالة الحواف الملونة
# machine-drafted, needs native review
develop-defringe = إزالة الحواف الملونة
# machine-drafted, needs native review
develop-defringe-purple = تدرج بنفسجي
# machine-drafted, needs native review
develop-defringe-green = تدرج أخضر
# machine-drafted, needs native review
develop-defringe-range = نطاق الصبغة
# machine-drafted, needs native review
develop-defringe-edge = تباين الحواف
# machine-drafted, needs native review
develop-module-flat-field = إطارات المعايرة
# machine-drafted, needs native review
develop-calibration = المعايرة
# machine-drafted, needs native review
develop-calibration-auto = من مجلد
# machine-drafted, needs native review
develop-calibration-chosen = ملفات مختارة
# machine-drafted, needs native review
develop-flat-choose = اختيار إطار flat…
# machine-drafted, needs native review
develop-dark-choose = اختيار إطار dark…
# machine-drafted, needs native review
develop-flat-file = إطار flat: { $name }
# machine-drafted, needs native review
develop-dark-file = إطار dark: { $name }
# machine-drafted, needs native review
develop-flat-use = إطار flat
# machine-drafted, needs native review
develop-flat-use-full = تظليل الأطراف والغبار
# machine-drafted, needs native review
develop-flat-use-vignetting = تظليل الأطراف فقط
# machine-drafted, needs native review
develop-flat-dialog = اختيار إطار flat
# machine-drafted, needs native review
develop-dark-dialog = اختيار إطار dark
# machine-drafted, needs native review
develop-raw-kind = ملفات Raw
# machine-drafted, needs native review
develop-flat-frame = إطار flat
# machine-drafted, needs native review
develop-dark-frame = إطار dark

# machine-drafted, needs native review
# machine-drafted, needs native review
develop-mask-add = إضافة قناع
# machine-drafted, needs native review
develop-mask-kind-ellipse = قطع ناقص
# machine-drafted, needs native review
develop-mask-kind-rectangle = مستطيل
# machine-drafted, needs native review
develop-mask-kind-polygon = مضلع
# machine-drafted, needs native review
develop-mask-kind-linear = تدرج خطي
# machine-drafted, needs native review
develop-mask-kind-radial = تدرج دائري
# machine-drafted, needs native review
develop-mask-kind-brush = فرشاة
# machine-drafted, needs native review
develop-mask-kind-luminance = نطاق السطوع
# machine-drafted, needs native review
develop-mask-kind-hue = نطاق الصبغة
# machine-drafted, needs native review
develop-mask-kind-chroma = نطاق التشبع
# machine-drafted, needs native review
develop-mask-kind-colour = نطاق اللون
# machine-drafted, needs native review
develop-mask-kind-other = قناع آخر
# machine-drafted, needs native review
develop-mask-row = { $kind } { $n }
# machine-drafted, needs native review
develop-mask-mode = المزج
# machine-drafted, needs native review
develop-blend-normal = عادي
# machine-drafted, needs native review
develop-blend-multiply = ضرب
# machine-drafted, needs native review
develop-blend-screen = شاشة
# machine-drafted, needs native review
develop-blend-overlay = تراكب
# machine-drafted, needs native review
develop-blend-soft-light = ضوء ناعم
# machine-drafted, needs native review
develop-blend-lightness = السطوع فقط
# machine-drafted, needs native review
develop-blend-colour = اللون فقط
# machine-drafted, needs native review
develop-mask-amount = قوة القناع
# machine-drafted, needs native review
develop-mask-invert = عكس القناع
# machine-drafted, needs native review
develop-mask-off = إيقاف
# machine-drafted, needs native review
develop-mask-on = تشغيل
# machine-drafted, needs native review
develop-mask-refine = تحسين الحواف
# machine-drafted, needs native review
develop-mask-show = إظهار القناع
# machine-drafted, needs native review
develop-mask-show-red = أحمر
# machine-drafted, needs native review
develop-mask-show-grey = رمادي
# machine-drafted, needs native review
develop-mask-remove = إزالة القناع
# machine-drafted, needs native review
develop-mask-combine-add = إضافة
# machine-drafted, needs native review
develop-mask-combine-subtract = طرح
# machine-drafted, needs native review
develop-mask-combine-intersect = تقاطع
# machine-drafted, needs native review
develop-mask-row-opacity = العتامة
# machine-drafted, needs native review
develop-mask-row-invert = عكس
# machine-drafted, needs native review
develop-mask-feather = التنعيم
# machine-drafted, needs native review
develop-mask-roundness = تدوير الزوايا
# machine-drafted, needs native review
develop-mask-brush-size = حجم الفرشاة
# machine-drafted, needs native review
develop-mask-brush-feather = نعومة الفرشاة
# machine-drafted, needs native review
develop-mask-brush-flow = التدفق
# machine-drafted, needs native review
develop-mask-brush-mode = الفرشاة
# machine-drafted, needs native review
develop-mask-brush-paint = رسم
# machine-drafted, needs native review
develop-mask-brush-erase = محو
# machine-drafted, needs native review
develop-mask-range-from = من
# machine-drafted, needs native review
develop-mask-range-full-from = كامل من
# machine-drafted, needs native review
develop-mask-range-full-to = كامل حتى
# machine-drafted, needs native review
develop-mask-range-to = حتى
# machine-drafted, needs native review
develop-mask-range-test = الاختبار
# machine-drafted, needs native review
develop-mask-range-input = الدخل
# machine-drafted, needs native review
develop-mask-range-output = الخرج
# machine-drafted, needs native review
develop-mask-colour-pick = اختيار لون
# machine-drafted, needs native review
develop-mask-colour-pick-more = اختيار لون آخر
# machine-drafted, needs native review
develop-mask-colour-range = النطاق
# machine-drafted, needs native review
develop-mask-colour-falloff = التلاشي
# machine-drafted, needs native review
develop-mask-edit = التحرير على الصورة
# machine-drafted, needs native review
develop-mask-row-remove = إزالة { $name }
# machine-drafted, needs native review
develop-mask-remove-confirm = إزالة قناع { $module }؟ ستُحذف أشكاله. تبقى الصورة على القرص.
# machine-drafted, needs native review
develop-mask-row-remove-confirm = إزالة { $name } من { $module }؟ تبقى الأقنعة الأخرى والصورة على القرص.
# machine-drafted, needs native review
develop-mask-remove-ok = إزالة
# machine-drafted, needs native review
develop-mask-removed = أُزيل القناع.
# machine-drafted, needs native review
develop-mask-added = أُضيف { $name }.
# machine-drafted, needs native review
develop-mask-full = بلغ هذا القناع أقصى عدد من الأجزاء.
# machine-drafted, needs native review
develop-mask-editing = تحرير { $name }. اسحب المقابض، أو اضغط Tab والأسهم.
# machine-drafted, needs native review
develop-mask-painting = رسم { $name }. اسحب على الصورة.
# machine-drafted, needs native review
develop-mask-picking = انقر على الصورة لاختيار لون.
# machine-drafted, needs native review
develop-mask-picked = اختير اللون.
# machine-drafted, needs native review
develop-mask-done = انتهى تحرير القناع.
# machine-drafted, needs native review
develop-mask-handles = مقابض القناع
# machine-drafted, needs native review
develop-handle-centre = المركز
# machine-drafted, needs native review
develop-handle-width = العرض
# machine-drafted, needs native review
develop-handle-height = الارتفاع
# machine-drafted, needs native review
develop-handle-rotate = تدوير
# machine-drafted, needs native review
develop-handle-feather = التنعيم
# machine-drafted, needs native review
develop-handle-inner = الحافة الداخلية
# machine-drafted, needs native review
develop-handle-start = البداية
# machine-drafted, needs native review
develop-handle-end = النهاية
# machine-drafted, needs native review
develop-handle-point = النقطة { $n }
# machine-drafted, needs native review
develop-instance = الطبقة
# machine-drafted, needs native review
develop-instance-n = { $n }
# machine-drafted, needs native review
develop-instance-chosen = الطبقة { $n }.
# machine-drafted, needs native review
develop-instance-new = طبقة جديدة
# machine-drafted, needs native review
develop-instance-duplicate = تكرار الطبقة
# machine-drafted, needs native review
develop-instance-delete = حذف الطبقة
# machine-drafted, needs native review
develop-instance-earlier = نقل إلى الأمام
# machine-drafted, needs native review
develop-instance-later = نقل إلى الخلف
# machine-drafted, needs native review
develop-instance-delete-confirm = حذف الطبقة { $n } من { $module }؟ ستُحذف إعداداتها وأقنعتها. تبقى الصورة على القرص.
# machine-drafted, needs native review
develop-instance-delete-ok = حذف
# machine-drafted, needs native review
develop-instance-deleted = حُذفت الطبقة.
# machine-drafted, needs native review
develop-history-mask = القناع: { $module }
# machine-drafted, needs native review
develop-history-mask-shapes = أشكال القناع: { $module }
# machine-drafted, needs native review
develop-history-instance-new = جديد: { $module }
# machine-drafted, needs native review
develop-history-instance-delete = محذوف: { $module }
# machine-drafted, needs native review
develop-history-instance-move = الترتيب: { $module }
# Other tools' edits and sidecar conflicts.
# machine-drafted, needs native review
develop-others-title = برامج أخرى
# machine-drafted, needs native review
develop-darktable-edited = عُدّلت في darktable:
# machine-drafted, needs native review
develop-darktable-open = افتح في darktable
# machine-drafted, needs native review
develop-darktable-opening = يُفتح في darktable
# machine-drafted, needs native review
develop-darktable-failed = لم يبدأ darktable: { $reason }
# machine-drafted, needs native review
develop-lightroom-import = استيراد من Lightroom
# machine-drafted, needs native review
develop-lightroom-imported = { $count ->
    [zero] استُوردت إعدادات من Lightroom: { $count }. النتيجة تقريبية.
    [one] استُورد إعداد واحد من Lightroom. النتيجة تقريبية.
    [two] استُوردت إعدادات من Lightroom: { $count }. النتيجة تقريبية.
    [few] استُوردت إعدادات من Lightroom: { $count }. النتيجة تقريبية.
    [many] استُوردت إعدادات من Lightroom: { $count }. النتيجة تقريبية.
   *[other] استُوردت إعدادات من Lightroom: { $count }. النتيجة تقريبية.
}
# machine-drafted, needs native review
develop-lightroom-nothing = لا شيء للاستيراد: هذه الإعدادات لا تغيّر شيئًا هنا.
# machine-drafted, needs native review
develop-history-lightroom = مستورد من Lightroom (تقريبيًا)
# machine-drafted, needs native review
develop-conflict-title = تغيّر تعديل { $file } خارج Specular
# machine-drafted, needs native review
develop-conflict-file = الملف المرافق: حُفظ { $time }
# machine-drafted, needs native review
develop-conflict-library = المكتبة: حُفظت { $time }
# machine-drafted, needs native review
develop-conflict-writer = كتب الملف المرافق { $writer }.
# machine-drafted, needs native review
develop-conflict-detail = احتفظ بواحد. يُستبدل الآخر.
# machine-drafted, needs native review
develop-conflict-use-file = استخدم ما في الملف
# machine-drafted, needs native review
develop-conflict-keep-library = احتفظ بما في المكتبة
# machine-drafted, needs native review
develop-conflict-used-file = يُستخدم التعديل من الملف المرافق
# machine-drafted, needs native review
develop-conflict-kept-library = يُحتفظ بتعديل المكتبة
# machine-drafted, needs native review
toast-develop-edit-newer-process = صُنع هذا التعديل بإصدار أحدث من Specular. قد يبدو مختلفًا هنا.
# machine-drafted, needs native review
info-darktable-edited = عُدّلت في darktable
# machine-drafted, needs native review
info-darktable-no-modules = لا وحدات مفعّلة
# machine-drafted, needs native review
info-darktable-group = الملف
# machine-drafted, needs native review
toast-develop-edit-partial = صُنع هذا التعديل بإصدار أحدث من Specular. أُغفلت الأجزاء التي لا يعرضها هذا الإصدار. لا تُحفظ التغييرات هنا.
# machine-drafted, needs native review
toast-develop-sidecar-unreadable = تعذّرت قراءة ملف التعديل بجانب هذه الصورة. يبقى كما هو. لا تُحفظ التغييرات هنا.
# machine-drafted, needs native review
develop-demosaic = تفاصيل Raw
develop-demosaic-directional = قياسي
develop-demosaic-dual = تنعيم المساحات المستوية
develop-demosaic-fast = سريع
develop-dual-threshold = تباين المساحات المستوية
