# machine-drafted, needs native review
# כלי בדיקה: אזהרות קיטום, הדגשת מיקוד, היסטוגרמה, בודק פיקסלים.

## לוח הפקודות
palette-clipping = הצגה או הסתרה של אזהרות קיטום
palette-focus-peaking = הצגה או הסתרה של הדגשת מיקוד
palette-histogram = הצגה או הסתרה של ההיסטוגרמה
palette-pixel-inspector = הצגה או הסתרה של בודק הפיקסלים
# machine-drafted, needs native review
palette-crosshair = כוונת מקלדת
palette-announce-cursor = הקראת הערך שמתחת לסמן

## היסטוגרמה
histogram-title = היסטוגרמה
histogram-clipped = קיטום: אורות { $high }, צללים { $low }
histogram-empty = אין תמונה גלויה
histogram-a11y = היסטוגרמה של התמונה הגלויה: בהירות ממוצעת { $mean }, אורות קטומים { $high }, צללים קטומים { $low }

## בודק פיקסלים
inspector-title = בודק פיקסלים
inspector-position = מיקום
inspector-source-rgb = מקור RGB
inspector-source-rgba = מקור RGBA
inspector-source-ycc = מקור Y′CbCr
inspector-source-gray = מקור אפור
inspector-source-gray16 = מקור אפור (16 ביט)
inspector-source-gray-alpha16 = מקור אפור + אלפא (16 ביט)
inspector-source-rgb16 = מקור RGB (16 ביט)
inspector-source-rgba16 = מקור RGBA (16 ביט)
inspector-source-pending = מקור
inspector-srgb = sRGB
inspector-hex = הקסדצימלי
inspector-linear = ליניארי
inspector-approximate = מתצוגה מקדימה מוקטנת; יש להגדיל לקבלת ערכים מדויקים
inspector-outside = יש להצביע על התמונה כדי לבדוק פיקסל
inspector-a11y = פיקסל ב־{ $x }, { $y }: sRGB { $value }

## הגדרות
settings-section-inspection = בדיקה
settings-inspection-clip-highlight = רמת קיטום של אורות
settings-inspection-clip-highlight-help = פיקסלים שבהם ערוץ כלשהו מגיע לערך 8 סיביות זה או עולה עליו מסומנים כאורות קטומים.
settings-inspection-clip-shadow = רמת קיטום של צללים
settings-inspection-clip-shadow-help = פיקסלים שבהם ערוץ כלשהו נמצא בערך 8 סיביות זה או מתחתיו מסומנים כצללים קטומים.
settings-inspection-highlight-color = צבע אזהרת אורות
settings-inspection-highlight-color-help = הצבע שמסמן אורות קטומים.
settings-inspection-shadow-color = צבע אזהרת צללים
settings-inspection-shadow-color-help = הצבע שמסמן צללים קטומים.
settings-inspection-peaking-sensitivity = רגישות הדגשת המיקוד
settings-inspection-peaking-sensitivity-help = ערכים גבוהים יותר מסמנים גם קצוות רכים יותר כממוקדים.
settings-inspection-peaking-color = צבע הדגשת המיקוד
settings-inspection-peaking-color-help = הצבע שמסמן קצוות חדים.
settings-inspection-histogram-channels = ערוצי ההיסטוגרמה
settings-inspection-histogram-channels-help = אילו עקומות ההיסטוגרמה מציגה.
settings-inspection-histogram-channels-colour = אדום, ירוק וכחול
settings-inspection-histogram-channels-luminance = בהירות
settings-inspection-histogram-channels-both = צבע ובהירות
settings-inspection-histogram-log = היסטוגרמה לוגריתמית
settings-inspection-histogram-log-help = הצגת ההיסטוגרמה בקנה מידה לוגריתמי כדי שכמויות קטנות יישארו גלויות.

## Channel isolation (machine-drafted, needs native review)
palette-channel-red = הצגת הערוץ האדום
palette-channel-green = הצגת הערוץ הירוק
palette-channel-blue = הצגת הערוץ הכחול
palette-channel-alpha = הצגת ערוץ האלפא (שוב: מעל לוח שחמט, ואז מעל צבע)
palette-channel-luma = הצגת הבהירות
channel-normal = כל הערוצים
channel-red = ערוץ אדום
channel-green = ערוץ ירוק
channel-blue = ערוץ כחול
channel-alpha = ערוץ אלפא
channel-alpha-checker = שקיפות מעל לוח שחמט
channel-alpha-matte = שקיפות מעל צבע
channel-luma = בהירות
disabled-no-alpha = לתמונה זו אין שקיפות
settings-inspection-alpha-matte-color = צבע רקע לאלפא
settings-inspection-alpha-matte-color-help = צבע אחיד מאחורי התמונה בתצוגת הערוץ ״שקיפות מעל צבע״.

## Colour-vision-safe display: false colour, colour bars, channel colours, CVD simulation, HiLo
palette-false-colour = הצגה או הסתרה של צבע מדומה
palette-next-colour-map = מפת הצבעים הבאה
palette-channel-colours = צבעי הערוצים הבאים (אדום–ירוק, מג׳נטה–ירוק, מג׳נטה–צהוב–ציאן)
palette-hilo = הצגה או הסתרה של HiLo (פיקסלים במקסימום ובמינימום של עומק הסיביות)
palette-colour-vision = הדמיית ראיית הצבעים הבאה
palette-colour-vision-off = הפסקת הדמיית ראיית הצבעים
palette-colour-vision-protanopia = הדמיית פרוטנופיה (ללא מדוכים אדומים)
palette-colour-vision-deuteranopia = הדמיית דויטרנופיה (ללא מדוכים ירוקים)
palette-colour-vision-tritanopia = הדמיית טריטנופיה (ללא מדוכים כחולים)
palette-colour-vision-achromatopsia = הדמיית אכרומטופסיה (ללא צבע)
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
colour-map-jet = Jet (לא בטוחה לעיוורון צבעים)
# The colour bar's title. $map is a colour map name, $unit the value unit (HU, counts…).
colour-bar-title = { $map }
colour-bar-title-unit = { $map } · { $unit }
# Unit of raw sample values without a physical unit.
colour-bar-unit-raw = ערכים גולמיים
# Screen reader summary of the colour bar.
colour-bar-a11y = סרגל צבעים { $map }: מ-{ $low } עד { $high } { $unit }
# Badges at the top of the window while a display aid is on (also announced).
badge-false-colour = צבע מדומה: { $map }
badge-hilo = HiLo: מסמן פיקסלים במקסימום ובמינימום
badge-colour-vision-protanopia = מדמה פרוטנופיה
badge-colour-vision-deuteranopia = מדמה דויטרנופיה
badge-colour-vision-tritanopia = מדמה טריטנופיה
badge-colour-vision-achromatopsia = מדמה אכרומטופסיה
channel-colours-red-green = ערוצים: אדום, ירוק, כחול
channel-colours-magenta-green = ערוצים: מג׳נטה, ירוק
channel-colours-magenta-yellow-cyan = ערוצים: מג׳נטה, צהוב, ציאן
settings-inspection-false-colour-map = מפת צבע מדומה
settings-inspection-false-colour-map-help = מפת הצבעים לצבע מדומה ולסרגל הצבעים שלו. כולן מלבד jet ו־phase נשארות קריאות בכל סוג של ראיית צבעים.
settings-inspection-false-colour-map-viridis = Viridis
settings-inspection-false-colour-map-cividis = Cividis
settings-inspection-false-colour-map-batlow = Batlow
settings-inspection-false-colour-map-roma = Roma (מתפצלת)
settings-inspection-false-colour-map-vik = Vik (מתפצלת)
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
settings-inspection-false-colour-map-balance = Balance (מתפצלת)
settings-inspection-false-colour-map-curl = Curl (מתפצלת)
settings-inspection-false-colour-map-phase = Phase (מחזורית)
settings-inspection-false-colour-map-jet = Jet (לא בטוחה לעיוורון צבעים)
settings-inspection-colour-bar = סרגל צבעים
settings-inspection-colour-bar-help = הצגת סרגל צבעים עם ערכים ביחידות של התמונה עצמה כשצבע מדומה פעיל.
settings-inspection-channel-colours = צבעי ערוצים
settings-inspection-channel-colours-help = אופן הצגת ערוצי הצבע. מג׳נטה–ירוק ומג׳נטה–צהוב–ציאן נשארים נבדלים גם בעיוורון אדום–ירוק.
settings-inspection-channel-colours-red-green = אדום, ירוק, כחול
settings-inspection-channel-colours-magenta-green = מג׳נטה, ירוק
settings-inspection-channel-colours-magenta-yellow-cyan = מג׳נטה, צהוב, ציאן
settings-inspection-colour-vision = הדמיית ראיית צבעים
settings-inspection-colour-vision-help = הצגת החלון כולו כפי שהוא נראה עם ליקוי בראיית צבעים (Machado et al. 2009). ייצואים לעולם אינם מושפעים.
settings-inspection-colour-vision-off = כבויה
settings-inspection-colour-vision-protanopia = פרוטנופיה
settings-inspection-colour-vision-deuteranopia = דויטרנופיה
settings-inspection-colour-vision-tritanopia = טריטנופיה
settings-inspection-colour-vision-achromatopsia = אכרומטופסיה
settings-inspection-auto-range = טווח אוטומטי לגווני אפור בעומק סיביות גבוה
settings-inspection-auto-range-help = מתיחת תמונות אפור של 16 סיביות (TIFF,‏ PNG,‏ FITS) מהערך הכהה ביותר שלהן עד הבהיר ביותר. תמונות DICOM שומרות על החלון שלהן.

## Keyboard crosshair and the value under the cursor (machine-drafted, needs native review)

crosshair-on = הכוונת פועלת. מקשי החצים מזיזים אותה, Shift לצעדים גדולים יותר. Enter מבצע לחיצה. Esc מסיים.
crosshair-off = הכוונת כבויה
announce-cursor-nothing = אין דבר מתחת לסמן
