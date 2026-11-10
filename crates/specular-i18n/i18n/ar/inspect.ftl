# machine-drafted, needs native review
# أدوات الفحص: تحذيرات القص، إبراز التركيز، المدرج التكراري، فاحص البكسل.

## لوحة الأوامر
palette-clipping = إظهار تحذيرات القص أو إخفاؤها
palette-focus-peaking = إظهار إبراز التركيز أو إخفاؤه
palette-histogram = إظهار المدرج التكراري أو إخفاؤه
palette-pixel-inspector = إظهار فاحص البكسل أو إخفاؤه
# machine-drafted, needs native review
palette-crosshair = مؤشر التقاطع بلوحة المفاتيح
palette-announce-cursor = قراءة القيمة تحت المؤشر

## المدرج التكراري
histogram-title = المدرج التكراري
histogram-clipped = القص: الإضاءات { $high }، الظلال { $low }
histogram-empty = لا توجد صورة ظاهرة
histogram-a11y = المدرج التكراري للصورة الظاهرة: متوسط السطوع { $mean }، الإضاءات المقصوصة { $high }، الظلال المقصوصة { $low }

## فاحص البكسل
inspector-title = فاحص البكسل
inspector-position = الموضع
inspector-source-rgb = المصدر RGB
inspector-source-rgba = المصدر RGBA
inspector-source-ycc = المصدر Y′CbCr
inspector-source-gray = المصدر رمادي
inspector-source-gray16 = المصدر رمادي (16 بت)
inspector-source-gray-alpha16 = المصدر رمادي + ألفا (16 بت)
inspector-source-rgb16 = المصدر RGB (16 بت)
inspector-source-rgba16 = المصدر RGBA (16 بت)
inspector-source-pending = المصدر
inspector-srgb = sRGB
inspector-hex = ست عشري
inspector-linear = خطي
inspector-approximate = من معاينة مصغّرة؛ كبّر للحصول على قيم دقيقة
inspector-outside = أشر إلى الصورة لفحص بكسل
inspector-a11y = البكسل عند { $x }، { $y }: sRGB { $value }

## الإعدادات
settings-section-inspection = الفحص
settings-inspection-clip-highlight = مستوى قص الإضاءات
settings-inspection-clip-highlight-help = تُعلَّم البكسلات التي تبلغ إحدى قنواتها هذه القيمة ذات 8 بت أو تتجاوزها كإضاءات مقصوصة.
settings-inspection-clip-shadow = مستوى قص الظلال
settings-inspection-clip-shadow-help = تُعلَّم البكسلات التي تكون إحدى قنواتها عند هذه القيمة ذات 8 بت أو أقل كظلال مقصوصة.
settings-inspection-highlight-color = لون تحذير الإضاءات
settings-inspection-highlight-color-help = اللون الذي يعلّم الإضاءات المقصوصة.
settings-inspection-shadow-color = لون تحذير الظلال
settings-inspection-shadow-color-help = اللون الذي يعلّم الظلال المقصوصة.
settings-inspection-peaking-sensitivity = حساسية إبراز التركيز
settings-inspection-peaking-sensitivity-help = القيم الأعلى تعلّم الحواف الأنعم أيضًا كحواف واضحة.
settings-inspection-peaking-color = لون إبراز التركيز
settings-inspection-peaking-color-help = اللون الذي يعلّم الحواف الحادة.
settings-inspection-histogram-channels = قنوات المدرج التكراري
settings-inspection-histogram-channels-help = المنحنيات التي يعرضها المدرج التكراري.
settings-inspection-histogram-channels-colour = الأحمر والأخضر والأزرق
settings-inspection-histogram-channels-luminance = الإضاءة
settings-inspection-histogram-channels-both = اللون والإضاءة
settings-inspection-histogram-log = مدرج تكراري لوغاريتمي
settings-inspection-histogram-log-help = عرض المدرج التكراري بمقياس لوغاريتمي لتبقى الأعداد الصغيرة ظاهرة.

## Channel isolation (machine-drafted, needs native review)
palette-channel-red = إظهار القناة الحمراء
palette-channel-green = إظهار القناة الخضراء
palette-channel-blue = إظهار القناة الزرقاء
palette-channel-alpha = إظهار قناة ألفا (مرة أخرى: فوق رقعة شطرنج، ثم فوق لون)
palette-channel-luma = إظهار الإضاءة
channel-normal = كل القنوات
channel-red = القناة الحمراء
channel-green = القناة الخضراء
channel-blue = القناة الزرقاء
channel-alpha = قناة ألفا
channel-alpha-checker = الشفافية فوق رقعة شطرنج
channel-alpha-matte = الشفافية فوق لون
channel-luma = الإضاءة
disabled-no-alpha = لا تحتوي هذه الصورة على شفافية
settings-inspection-alpha-matte-color = لون خلفية ألفا
settings-inspection-alpha-matte-color-help = لون ثابت خلف الصورة في عرض القناة «الشفافية فوق لون».

## Colour-vision-safe display: false colour, colour bars, channel colours, CVD simulation, HiLo
palette-false-colour = إظهار الألوان الزائفة أو إخفاؤها
palette-next-colour-map = خريطة الألوان التالية
palette-channel-colours = ألوان القنوات التالية (أحمر–أخضر، أرجواني–أخضر، أرجواني–أصفر–سماوي)
palette-hilo = إظهار HiLo أو إخفاؤه (البكسلات عند أقصى عمق البت وأدناه)
palette-colour-vision = محاكاة رؤية الألوان التالية
palette-colour-vision-off = إيقاف محاكاة رؤية الألوان
palette-colour-vision-protanopia = محاكاة عمى الأحمر (بلا مخاريط حمراء)
palette-colour-vision-deuteranopia = محاكاة عمى الأخضر (بلا مخاريط خضراء)
palette-colour-vision-tritanopia = محاكاة عمى الأزرق (بلا مخاريط زرقاء)
palette-colour-vision-achromatopsia = محاكاة عمى الألوان الكلي (بلا ألوان)
# Colour map names. Proper names of published maps; usually left untranslated.
colour-map-viridis = Viridis
colour-map-cividis = Cividis
colour-map-batlow = Batlow
colour-map-roma = Roma
colour-map-vik = Vik
colour-map-oslo = Oslo
colour-map-lajolla = Lajolla
colour-map-thermal = Thermal
colour-map-haline = Haline
colour-map-solar = Solar
colour-map-ice = Ice
colour-map-deep = Deep
colour-map-dense = Dense
colour-map-algae = Algae
colour-map-rain = Rain
colour-map-speed = Speed
colour-map-balance = Balance
colour-map-curl = Curl
colour-map-phase = Phase
colour-map-jet = Jet (غير آمنة لقصور رؤية الألوان)
# The colour bar's title. $map is a colour map name, $unit the value unit (HU, counts…).
colour-bar-title = { $map }
colour-bar-title-unit = { $map } · { $unit }
# Unit of raw sample values without a physical unit.
colour-bar-unit-raw = قيم خام
# Screen reader summary of the colour bar.
colour-bar-a11y = شريط الألوان { $map }: من { $low } إلى { $high } { $unit }
# Badges at the top of the window while a display aid is on (also announced).
badge-false-colour = ألوان زائفة: { $map }
badge-hilo = HiLo: تعليم البكسلات عند الحد الأقصى والأدنى
badge-colour-vision-protanopia = محاكاة عمى الأحمر
badge-colour-vision-deuteranopia = محاكاة عمى الأخضر
badge-colour-vision-tritanopia = محاكاة عمى الأزرق
badge-colour-vision-achromatopsia = محاكاة عمى الألوان الكلي
channel-colours-red-green = القنوات: أحمر، أخضر، أزرق
channel-colours-magenta-green = القنوات: أرجواني، أخضر
channel-colours-magenta-yellow-cyan = القنوات: أرجواني، أصفر، سماوي
settings-inspection-false-colour-map = خريطة الألوان الزائفة
settings-inspection-false-colour-map-help = خريطة الألوان للألوان الزائفة وشريط ألوانها. كلها عدا jet وphase تبقى مقروءة مع أي رؤية للألوان.
settings-inspection-false-colour-map-viridis = Viridis
settings-inspection-false-colour-map-cividis = Cividis
settings-inspection-false-colour-map-batlow = Batlow
settings-inspection-false-colour-map-roma = Roma (متباعدة)
settings-inspection-false-colour-map-vik = Vik (متباعدة)
settings-inspection-false-colour-map-oslo = Oslo
settings-inspection-false-colour-map-lajolla = Lajolla
settings-inspection-false-colour-map-thermal = Thermal
settings-inspection-false-colour-map-haline = Haline
settings-inspection-false-colour-map-solar = Solar
settings-inspection-false-colour-map-ice = Ice
settings-inspection-false-colour-map-deep = Deep
settings-inspection-false-colour-map-dense = Dense
settings-inspection-false-colour-map-algae = Algae
settings-inspection-false-colour-map-rain = Rain
settings-inspection-false-colour-map-speed = Speed
settings-inspection-false-colour-map-balance = Balance (متباعدة)
settings-inspection-false-colour-map-curl = Curl (متباعدة)
settings-inspection-false-colour-map-phase = Phase (دورية)
settings-inspection-false-colour-map-jet = Jet (غير آمنة لقصور رؤية الألوان)
settings-inspection-colour-bar = شريط الألوان
settings-inspection-colour-bar-help = إظهار شريط ألوان بقيم بوحدات الصورة نفسها أثناء تفعيل الألوان الزائفة.
settings-inspection-channel-colours = ألوان القنوات
settings-inspection-channel-colours-help = طريقة عرض قنوات الألوان. يبقى الأرجواني–الأخضر والأرجواني–الأصفر–السماوي مميزين مع عمى الأحمر والأخضر.
settings-inspection-channel-colours-red-green = أحمر، أخضر، أزرق
settings-inspection-channel-colours-magenta-green = أرجواني، أخضر
settings-inspection-channel-colours-magenta-yellow-cyan = أرجواني، أصفر، سماوي
settings-inspection-colour-vision = محاكاة رؤية الألوان
settings-inspection-colour-vision-help = عرض النافذة كلها كما تُرى مع قصور في رؤية الألوان (Machado et al. 2009). لا تتأثر عمليات التصدير أبدًا.
settings-inspection-colour-vision-off = إيقاف
settings-inspection-colour-vision-protanopia = عمى الأحمر
settings-inspection-colour-vision-deuteranopia = عمى الأخضر
settings-inspection-colour-vision-tritanopia = عمى الأزرق
settings-inspection-colour-vision-achromatopsia = عمى الألوان الكلي
settings-inspection-auto-range = نطاق تلقائي للرمادي عالي البت
settings-inspection-auto-range-help = مدّ صور التدرج الرمادي 16 بت (TIFF، PNG، FITS) من أغمق قيمة إلى أسطعها. تحتفظ صور DICOM بنافذتها الخاصة.

## Keyboard crosshair and the value under the cursor (machine-drafted, needs native review)

crosshair-on = مؤشر التقاطع قيد التشغيل. مفاتيح الأسهم تحركه، وShift لخطوات أكبر. Enter ينقر. Esc ينهي.
crosshair-off = مؤشر التقاطع متوقف
announce-cursor-nothing = لا شيء تحت المؤشر
