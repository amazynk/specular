# Specular — Scientific images (FITS, XISF): stretch, limits, planes, channels, coordinates. עברית.

keys-mode-science = תמונות מדעיות
cheatsheet-science = תמונות מדעיות
cheatsheet-science-look = מתיחה וגבולות
cheatsheet-science-planes = מישורים וערוצים
# machine-drafted, needs native review
cheatsheet-science-channels-1-9 = הצגה או הסתרה של ערוצים 1 עד 9
cheatsheet-science-channels-10-16 = הצגה או הסתרה של ערוצים 10 עד 16
palette-science-stretch = המתיחה הבאה
palette-science-limits = הגבולות הבאים
palette-science-next-plane = המישור הבא
palette-science-prev-plane = המישור הקודם
palette-science-next-axis = מעבר בציר אחר
palette-science-composite = הרכבת הערוצים בצבע
palette-science-channel-1 = הצגה או הסתרה של ערוץ 1
palette-science-channel-2 = הצגה או הסתרה של ערוץ 2
palette-science-channel-3 = הצגה או הסתרה של ערוץ 3
palette-science-channel-4 = הצגה או הסתרה של ערוץ 4
palette-science-channel-5 = הצגה או הסתרה של ערוץ 5
palette-science-channel-6 = הצגה או הסתרה של ערוץ 6
palette-science-channel-7 = הצגה או הסתרה של ערוץ 7
palette-science-channel-8 = הצגה או הסתרה של ערוץ 8
palette-science-channel-9 = הצגה או הסתרה של ערוץ 9
# machine-drafted, needs native review
palette-science-channel-10 = הצגה או הסתרה של ערוץ 10
palette-science-channel-11 = הצגה או הסתרה של ערוץ 11
palette-science-channel-12 = הצגה או הסתרה של ערוץ 12
palette-science-channel-13 = הצגה או הסתרה של ערוץ 13
palette-science-channel-14 = הצגה או הסתרה של ערוץ 14
palette-science-channel-15 = הצגה או הסתרה של ערוץ 15
palette-science-channel-16 = הצגה או הסתרה של ערוץ 16
palette-science-grid = הצגה או הסתרה של רשת הקואורדינטות
palette-science-reset = מראה אוטומטי
palette-science-level = יישור מפות גובה (הפחתת המישור הממוצע)
palette-science-wiggle = עקבות סייסמיים (wiggle) עם שטח משתנה, מעל הצבעים
# machine-drafted, needs native review
palette-science-wireframe = קצוות של רשת לא מובנית (מסגרת תיל), מעל הצבעים
palette-science-import-lut = טבלת צבעים מקובץ (ImageJ .lut)…
palette-science-hillshade = הצללת תבליט של מודלי גובה (הצללת תבליט)
palette-science-fft = ספקטרום הספק (FFT) של המישור
palette-science-catalogue = הצגת קטלוג מקורות מעל התמונה (VOTable, טבלת FITS, CSV)…
palette-science-clear-catalogues = הסרת הקטלוגים המוצגים מעל התמונה
# A catalogue overlaid on a sky image: its name (the table's or the file's), how many of its
# sources fall on the image, and how many of those are drawn (at most a set number).
science-catalogue-toast = { $name }: { $count ->
        [one] מקור אחד בתמונה
        [two] שני מקורות בתמונה
       *[other] { $count } מקורות בתמונה
    }
science-catalogue-capped = { $name }: מוצגים { $shown } מתוך { $count } מקורות בתמונה
science-catalogue-none = { $name }: אין מקורות בתמונה
science-catalogue-failed = לא קטלוג עם מיקומים בשמיים: { $name }
science-catalogues-cleared = הקטלוגים הוסרו
# The file dialog's filter name for catalogue files.
science-catalogue-filter = קטלוגי מקורות
# A GW sky map's credible region: the smallest area holding this share of the probability,
# drawn as a contour labelled e.g. "90 %".
science-credible = ‏{ PERCENT($percent) }
palette-science-ion-image = תמונת יונים ב־m/z (ספקטרומטריית מסות)…
disabled-not-science = רק לתמונות מדעיות (אסטרונומיה, מיקרוסקופיה אלקטרונית ומיקרוסקופיית גשש סורק)
disabled-one-plane = בתמונה יש מישור אחד
disabled-one-axis = אין בתמונה ציר אחר למעבר
disabled-no-channels = אין בתמונה ערוץ כזה להרכבה
disabled-no-coordinates = אין בתמונה קואורדינטות שמיים
disabled-not-height-map = רק למפות גובה (Gwyddion, Igor)
disabled-not-elevation = רק למודלי גובה בעלי ייחוס גאוגרפי עם ערוץ אחד
disabled-not-seismic = רק לנתונים סייסמיים (SEG-Y, Seismic Unix)
disabled-not-mass-spec = רק לתמונות ספקטרומטריית מסות (imzML)
# machine-drafted, needs native review
disabled-not-ugrid = רק לרשתות לא מובנות (NetCDF מסוג UGRID, ICON ו-MPAS)
disabled-not-bayer = רק לפריימים גולמיים ממצלמות צבע (FITS עם תבנית באייר, וידאו SER)
palette-debayer = הצגת פריימים ממצלמות צבע לאחר דה־באייר או גולמיים
palette-bayer-pattern = תבנית באייר הבאה לקובץ זה
science-debayer-on = לאחר דה־באייר ({ $pattern })
science-debayer-off = מערך מסנני צבע גולמי
science-bayer-pattern = תבנית באייר { $pattern }
# machine-drafted, needs native review
palette-annotations = הצגה או הסתרה של הערות
disabled-no-annotations = אין לצד התמונה הערות (NDPA,‏ Aperio או ASAP XML)
science-annotations-on = ההערות מוצגות
science-annotations-off = ההערות מוסתרות
science-stretch-auto = אוטומטי (לכל צבע)
# Stretches: how values between the limits map to grey levels.
science-stretch-linear = ליניארי
science-stretch-sqrt = שורש ריבועי
science-stretch-log = לוגריתמי
science-stretch-asinh = Asinh
science-stretch-histeq = השוואת היסטוגרמה
# Limits: which values show black and white. $percent is e.g. 99.5.
science-limits-zscale = Zscale
science-limits-minmax = מינ׳/מקס׳
science-limits-percentile = { PERCENT($percent) }
# Seismic gain: limits symmetric about zero at the $percent percentile of the amplitudes.
science-limits-symmetric = ±{ PERCENT($percent) }
# The display range the file stores (ImageJ's min and max), used until other limits are chosen.
science-limits-file = טווח הקובץ
# Toasts when the look changes.
science-stretch-toast = מתיחה: { $stretch }
science-limits-toast = גבולות: { $limits }
science-reset-toast = מראה אוטומטי
# An imported colour table: $name is its file name.
science-lut-toast = טבלת צבעים { $name }
science-lut-failed = אינו טבלת צבעים: { $name }
# The file dialog's filter name for .lut files.
science-lut-filter = טבלאות צבעים של ImageJ
science-composite-on = הערוצים הורכבו בצבע
science-composite-off = ערוץ אחד בגווני אפור
science-channel-on = מוצג: { $channel }
science-channel-off = מוסתר: { $channel }
# machine-drafted, needs native review
# A channel without a name in the file, in the shown/hidden toasts.
science-channel-unnamed = ערוץ { $number }
science-grid-on = רשת הקואורדינטות מוצגת
science-grid-off = רשת הקואורדינטות מוסתרת
# X-ray diffraction frames: the same action shows circles of equal resolution (d-spacing).
science-rings-on = טבעות הרזולוציה מוצגות
science-rings-off = טבעות הרזולוציה מוסתרות
# A resolution ring's label: its d-spacing in ångström, e.g. "2.5 Å".
science-ring = { $d } Å
# Seismic wiggle traces: drawn once the traces are at least 4 screen pixels apart.
science-wiggle-on = עקבות wiggle מופעלים (יש להגדיל כדי לראות אותם)
science-wiggle-off = עקבות wiggle כבויים
# machine-drafted, needs native review
# Unstructured meshes: the edges drawn over the faces; too many to draw in view until the
# user zooms in.
science-wireframe-on = קצוות הרשת מופעלים
science-wireframe-off = קצוות הרשת כבויים
science-wireframe-dense = קצוות הרשת מופעלים (יש להתקרב כדי לראותם)
# Plane levelling (scanning probe height maps): the best-fit plane is subtracted for display.
science-level-on = יישור המישור מופעל
science-level-off = יישור המישור כבוי
# Relief shading of an elevation model. $azimuth is the sun's direction in degrees clockwise
# from north, $altitude its height above the horizon in degrees.
science-hillshade-on = הצללת תבליט מופעלת: שמש ב־{ $azimuth }°, בגובה { $altitude }°
science-hillshade-off = הצללת תבליט כבויה
# The power spectrum view: the plane's centred 2-D Fourier power, log scaled.
science-fft-on = ספקטרום ההספק מוצג
science-fft-off = ספקטרום ההספק מוסתר
science-axis-toast = מעבר: { $axis }
# Readout lines (bottom corner).
# The value under the pointer, with the values' unit when the file states one.
science-value = ערך { $value }
science-value-unit = ערך { $value } { $unit }
# machine-drafted, needs native review
science-value-flags = ערך { $value } (סיביות { $bits })
# machine-drafted, needs native review
# A class value under the pointer and its name, e.g. "Value 4 · Vegetation".
science-value-class = ערך { $value } · { $class }
# machine-drafted, needs native review
# Sentinel-2 scene classes (L2A SCL, ESA's names) and L1C cloud masks.
s2-class-no-data = אין נתונים
s2-class-defective = רווי או פגום
s2-class-dark = פיקסלים של אזורים כהים
s2-class-cloud-shadow = צללי עננים
s2-class-vegetation = צמחייה
s2-class-not-vegetated = ללא צמחייה
s2-class-water = מים
s2-class-unclassified = לא מסווג
s2-class-cloud-medium = עננים בהסתברות בינונית
s2-class-cloud-high = עננים בהסתברות גבוהה
s2-class-cirrus = ענני נוצה דקים
s2-class-snow = שלג או קרח
s2-class-clear = בהיר
s2-class-opaque-cloud = עננים אטומים
# Several channels under the pointer: "R 12.5  G 3.1  B 0.4".
science-values = { $values }
# Sky position under the pointer, sexagesimal then degrees.
science-radec = α { $ra }  δ { $dec }
science-radec-deg = { $ra }°  { $dec }°
# Galactic or ecliptic longitude and latitude.
science-lonlat = l { $lon }°  b { $lat }°
science-wcs-unsupported = WCS לא נתמך: { $reason }
# machine-drafted, needs native review
# A B1950 (FK4) image: what is converted between its frame and another. Its own position
# stays B1950 in the readout. ICRS, FK4 and SAMP are names and stay as they are.
science-fk4-markers = סמני קטלוג: ICRS → FK4 הומר
science-fk4-markers-galactic = סמני קטלוג: גלקטי → FK4 הומר
science-fk4-samp = SAMP: FK4 → ICRS הומר
# The look: stretch and limits.
science-look = { $stretch } · { $limits }
# Shown under the look while plane levelling is on.
science-levelled = המישור יושר
# Shown under the look while the hillshade is on.
science-hillshaded = הצללת תבליט: שמש ב־{ $azimuth }°, בגובה { $altitude }°
# Shown in the readout while the power spectrum is shown.
science-fft = ספקטרום הספק (log₁₀)
# The spectrum's value under the pointer: log₁₀ of the power.
science-fft-power = log₁₀ הספק { $value }
# Spatial frequency under the pointer along x and y, and its unit ("1/µm", "1/px").
science-fft-frequency = תדר u { $u }  v { $v } { $unit }
# The period of that frequency: "Period 2.5 nm".
science-fft-period = מחזור { $period }
# Crystal orientation under the pointer (EBSD): Bunge Euler angles in degrees.
science-euler = φ₁ { $phi1 }  Φ { $phi }  φ₂ { $phi2 }
# The crystal phase under the pointer and its symmetry ("Iron fcc (m-3m)").
science-phase = פאזה { $phase } ({ $symmetry })
# Title of the spectrum plot of the pixel under the pointer (an EDS map).
science-spectrum-title = ספקטרום ב־{ $x }, { $y }
# Physical position under the pointer (microscopy): "x 1.25 µm  y 3.5 µm".
science-position = x { $x }  y { $y }
# machine-drafted, needs native review
# CERN ROOT histograms: the bin under the pointer on one axis. $axis is the axis title
# ("p_{T}", or "x"), $range the bin's label or its edges ("20 GeV – 25 GeV"), $bin its
# number counted from 1.
science-root-bin = { $axis } { $range } (תא { $bin })
# A ROOT graph's point under the pointer.
science-root-point = x { $x }  y { $y }
# Under- and overflow bins (fills outside the axis ranges) hold something but are not shown.
science-root-flow = תאי גלישה (underflow/overflow) מוסתרים: { $count } (סכום { $sum })
# Bins of different widths are drawn on an even grid of pixels.
science-root-resampled = תאים ברוחב משתנה נדגמו מחדש לרשת אחידה
# machine-drafted, needs native review
# A flow cytometry plot (FCS) under the pointer: each axis's parameter and its value.
science-cytometry-at = { $x } { $xv }  { $y } { $yv }
# A cytometry histogram under the pointer: its parameter and the value.
science-cytometry-at-x = { $x } { $xv }
# Events in the plot's bin (or the histogram's bar) under the pointer.
science-cytometry-events = { $count ->
    [one] { $count } אירוע
    [two] { $count } אירועים
   *[other] { $count } אירועים
}
# The plot's axis scales ($x, $y: Linear, Log, Logicle or Arcsinh).
science-cytometry-scales = x { $x }, y { $y }
science-cytometry-scale-x = x { $x }
# Spillover compensation is applied, from the file's keyword ($keyword, e.g. $SPILLOVER).
science-cytometry-compensated = מפוצה ({ $keyword })
science-cytometry-uncompensated = לא מפוצה
# X-ray diffraction frames: what lies under the pointer. q is the momentum transfer
# (per ångström), d the lattice spacing (ångström), 2θ the scattering angle. The symbols
# stay as they are: "q 0.512 Å⁻¹  d 12.27 Å  2θ 4.56°".
science-scatter = q { $q } Å⁻¹  d { $d } Å  2θ { $tth }°
# Seismic readout: the trace under the pointer in a 3-D volume ($inline, $crossline the
# header numbers) or a 2-D line; $trace counts from 1 in file order, $cdp is the CDP number.
science-seismic-cell = אינליין { $inline }  קרוסליין { $crossline }
science-seismic-trace = עקבה { $trace }  CDP { $cdp }
# A cell of a 3-D volume with no trace in the file.
science-seismic-gap = אין עקבה
# Two-way time (ms) or depth of the row under the pointer.
science-seismic-time = זמן { $time } ms
science-seismic-depth = עומק { $depth } { $unit }
# The trace's CDP (or source) X and Y with the coordinate scalar applied.
science-seismic-xy = X { $x }  Y { $y }
# machine-drafted, needs native review
# Unstructured mesh readout: the face (cell) under the pointer, counting from 0 as in the
# file's connectivity, and the node nearest to the pointer when the values sit on nodes.
science-mesh-face = פאה { $face }
science-mesh-face-node = פאה { $face }  צומת { $node }
# The pointer lies between the mesh's faces (land in an ocean mesh, outside its boundary).
science-mesh-outside = מחוץ לרשת
# Position on an axis past the image: "Plane 3 of 40", or with its world value.
science-plane = { $axis } { $index } מתוך { $count }
science-plane-world = { $axis } { $index } מתוך { $count } ({ $world })
science-axis-unnamed = מישור
science-axis-extension = תמונה
science-axis-channel = ערוץ
# A column of a HEALPix sky map (probability, distance…) shown as the image.
science-axis-field = שדה
# X-ray event lists binned into an image: the bin size, in the event list's sky pixels
# ("Bin 2 of 4 (8 px)"), and the energy band of the events counted.
science-axis-bin = תא
science-bin-size = { $size } px
science-axis-band = אנרגיה
science-band-all = כל האנרגיות
# machine-drafted, needs native review
# Complex-valued arrays (NumPy, MATLAB, Zarr, TDMS): which part of each value is shown.
science-axis-part = רכיב
science-part-magnitude = גודל
science-part-real = החלק הממשי
science-part-imaginary = החלק המדומה
science-part-phase = פאזה (רדיאנים)
# machine-drafted, needs native review
# Complex radar (SAR) images: intensity in decibels.
science-part-db = עוצמה (dB)
# machine-drafted, needs native review
# The frames of a thermal sequence (FLIR SEQ/CSQ), in time.
science-axis-frame = פריים
science-readout = ערך וקואורדינטות
# Accessible names of the plane sliders.
science-sliders = מישורים
# The scale bar's length.
science-scale-arcsec = { $value }″
science-scale-arcmin = { $value }′
science-scale-deg = { $value }°
# A physical length, e.g. "20 nm" (electron microscopy images).
science-scale-length = { $value } { $unit }
# machine-drafted, needs native review
# A length along a named axis of a plot, e.g. "20 GeV (p_{T})".
science-scale-axis = { $length } ({ $axis })
science-scale-a11y = סרגל קנה מידה: { $length }
# Mass spectrometry images (imzML). The image is the total ion current (each spot's summed
# intensities) until an m/z is typed; then that ion's image, its intensities summed over
# m/z ± the tolerance. $window is e.g. "760.5851 ± 5 ppm".
science-ion-tic = זרם יונים כולל
science-ion-window = תמונת יונים m/z { $window }
# The m/z prompt: $text is what is typed so far (opens on the base peak under the pointer).
science-ion-prompt = תמונת יונים ב־m/z: { $text }▏
# $tolerance is e.g. "10 ppm" or "0.1 Da"; a tolerance can also be typed after the m/z
# ("760.585 5 ppm"). Tab steps the tolerance; Enter on an empty prompt shows the total ion
# current again.
science-ion-prompt-hint = ± { $tolerance } · Tab סבולת · Enter הצגה · Esc ביטול
science-ion-bad = אינו ערך m/z: { $text }
# The spot under the pointer as the file numbers it (from 1).
science-ms-spot = נקודה x { $x }  y { $y }
science-ms-spot-z = נקודה x { $x }  y { $y }  z { $z }
science-ms-no-spectrum = אין ספקטרום בנקודה זו
# machine-drafted, needs native review
palette-science-temperature-unit = יחידת טמפרטורה (°C, °F, K)
palette-science-isotherm = איזותרמה מעל או מתחת לטמפרטורה שמתחת למצביע
palette-science-thermal-parameters = פליטות וטמפרטורה מוחזרת…
palette-science-export-temperatures = ייצוא טמפרטורות (TIFF בנקודה צפה או CSV)…
palette-thermal-visible = תמונה גלויה של תמונה תרמית
disabled-not-thermal = רק לתמונות תרמיות
disabled-not-radiometric = הקובץ שומר טמפרטורות: לא ניתן לשנות את הפרמטרים שלו
disabled-no-visible-photo = לתמונה התרמית אין תמונה גלויה
science-temperature-unit-toast = טמפרטורות ב־{ $unit }
science-isotherm-above = איזותרמה מעל { $level }
science-isotherm-below = איזותרמה מתחת ל־{ $level }
science-isotherm-off = איזותרמה כבויה
science-thermal-emissivity-prompt = פליטות: { $text }
science-thermal-reflected-prompt = טמפרטורה מוחזרת ({ $unit }): { $text }
science-thermal-prompt-hint = Tab השדה הבא · Enter חישוב מחדש · Esc ביטול
science-thermal-bad = ערך לא תקין: { $text }
science-thermal-applied = חושב מחדש עבור פליטות { $emissivity }, מוחזרת { $reflected }
science-thermal-conditions = פליטות { $emissivity } · מוחזרת { $reflected }
science-thermal-export-tiff = TIFF בנקודה צפה של 32 סיביות
science-thermal-export-csv = CSV
science-thermal-export-done = הטמפרטורות נשמרו ב-{ $name }
science-thermal-export-failed = לא ניתן לשמור את הטמפרטורות: { $reason }
# machine-drafted, needs native review
science-values-failed = לא ניתן להציג את הערכים: { $reason }
# machine-drafted, needs native review
# Thermal sequences (FLIR SEQ/CSQ): playback and each measurement over time.
palette-science-thermal-play = הפעלה או השהיה של הרצף התרמי
palette-science-thermal-series-csv = שמירת הטמפרטורות לאורך זמן (CSV)…
disabled-not-thermal-sequence = רק לרצפים תרמיים
thermal-seq-title = טמפרטורות לאורך זמן
# $time is seconds from the first frame, e.g. "0.12 s".
thermal-seq-frame = פריים { $frame } מתוך { $frames } · { $time }
thermal-seq-play = הפעלה
thermal-seq-pause = השהיה
thermal-seq-playing = מתנגן
thermal-seq-paused = מושהה
thermal-seq-save-csv = שמירת CSV…
thermal-seq-hint = יש להוסיף נקודה, קו או אזור (M) כדי לשרטט אותם לאורך זמן.
thermal-seq-reading = קריאת פריים { $done } מתוך { $total }…
thermal-seq-failed = לא ניתן לקרוא את הפריימים: { $reason }
thermal-seq-no-series = אין עדיין מה לשמור: יש להוסיף נקודה, קו או אזור.
# For screen readers: each measurement's value at the frame shown, e.g.
# "1 Spot: 36.4 °C, 2 Rectangle: 30.1 °C".
thermal-seq-chart = תרשים: { $values }
# machine-drafted, needs native review
# The report (PDF) of the image shown: a thermal report for a thermal image, an inspection report for any other.
palette-thermal-report = דוח (PDF)…
thermal-report-heading = דוח תרמי
thermal-report-default-title = דוח בדיקה תרמוגרפית
thermal-report-title-label = כותרת
thermal-report-notes-label = הערות
thermal-report-photo = הכללת התמונה הגלויה
thermal-report-folder = הוספת שאר התמונות התרמיות בתיקייה הזו
thermal-report-save = שמירת PDF…
thermal-report-busy = הדוח נכתב…
thermal-report-nothing = הטמפרטורות עדיין לא מוצגות.
thermal-report-saved = הדוח נשמר ב־{ $name } ({ $count ->
    [one] תמונה אחת
    [two] שתי תמונות
   *[other] { $count } תמונות
})
thermal-report-failed = לא ניתן לשמור את הדוח: { $reason }
# In the PDF.
thermal-report-notes = הערות
thermal-report-measurements = מדידות
thermal-report-no-measurements = אין מדידות.
thermal-report-thermal-caption = תמונה תרמית
thermal-report-visible-caption = תמונה גלויה
thermal-report-col-measurement = מדידה
thermal-report-col-min = מינ׳
thermal-report-col-max = מקס׳
thermal-report-col-mean = ממוצע
thermal-report-col-emissivity = פליטות
thermal-report-whole-image = כל התמונה
thermal-report-camera = מצלמה
thermal-report-lens = עדשה
thermal-report-date = צולם
thermal-report-place = מקום
thermal-report-frame = פריים
thermal-report-emissivity = פליטות
thermal-report-reflected = טמפרטורה מוחזרת
thermal-report-distance = מרחק
thermal-report-humidity = לחות
thermal-report-range = טווח
thermal-report-format = פורמט
thermal-report-page = עמוד { $page } מתוך { $pages }
