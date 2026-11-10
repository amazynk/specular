# machine-drafted, needs native review
# חלונית המטא־נתונים של Specular — עברית (ראו i18n/en/metadata.ftl לגבי המוסכמות).

## Summary group titles

group-camera = מצלמה
group-exposure = חשיפה
group-lens = עדשה
group-date = תאריך ושעה
group-location = מיקום
# machine-drafted, needs native review
# A DJI drone photo: the gimbal's and the aircraft's attitude, the height above take-off.
group-drone = רחפן
label-gimbal = גימבל
label-flight = כלי הטיס
label-relative-altitude = מעל נקודת ההמראה
drone-attitude = סבסוב { $yaw }, עלרוד { $pitch }, גלגול { $roll }
# A DJI thermal camera's radiometric JPEG: its photo opens, its temperatures can't.
label-thermal-data = טמפרטורות
drone-thermal-refused = לא מוצג: רישיון ה־SDK של DJI לא מתיר זאת
drone-thermal-refused-detail = קריאת הטמפרטורות של DJI דורשת את Thermal SDK של DJI, שהרישיון שלו מתיר שימוש בו רק לתוכנות שמגיעות איתו. התמונה והמטא־נתונים שלה מוצגים.
group-image = תמונה
group-file = קובץ
group-rights = זכויות ותיאור
group-credentials = אישורי תוכן
# machine-drafted, needs native review
group-medical = תמונה רפואית
# machine-drafted, needs native review
group-inspection = רכיב ובדיקה
group-nde-image = תמונת בדיקה (DICONDE)
# machine-drafted, needs native review
# Industrial CT volumes (Zeiss Xradia, Amira, Bruker SkyScan, Nikon X-Tek, VGI, raw volumes).
group-industrial-image = תמונת בדיקה
group-science = תמונה מדעית
group-em = מיקרוסקופיה אלקטרונית
group-spm = מיקרוסקופיית גשש סורק
group-slide = שקופית
group-microscopy = מיקרוסקופיה
# X-ray diffraction detector frames (CBF, EDF, SMV, Bruker, MarCCD, R-AXIS).
group-diffraction = עקיפה
group-surface = טופוגרפיית משטח
group-ebsd = עקיפת אלקטרונים מפוזרים לאחור
group-seismic = נתונים סייסמיים
# machine-drafted, needs native review
group-asdf = ASDF
# machine-drafted, needs native review
# CERN ROOT files (particle physics histograms and graphs).
group-root = ROOT
# machine-drafted, needs native review
# CASA images (radio interferometry: the .image, .residual, .model directories tclean writes).
group-casa = תמונת CASA
# machine-drafted, needs native review
# GIFTI files: brain surfaces and the values on their vertices (Human Connectome Project
# and FreeSurfer exports).
group-gifti = GIFTI
# CIFTI-2 files: maps over cortical surfaces and subcortical voxels ("grayordinates").
group-cifti = CIFTI
# White-matter fibre tracts (TrackVis, MRtrix).
group-tractography = טרקטוגרפיה
# machine-drafted, needs native review
group-annotations = הערות
# Imaging mass spectrometry (imzML): a mass spectrum per pixel.
group-mass-spec = הדמיה בספקטרומטריית מסות
# machine-drafted, needs native review
group-thermal = תמונה תרמית
# machine-drafted, needs native review
# Flow cytometry list-mode data (FCS): events of a cell sorter or analyser.
group-cytometry = ציטומטריית זרימה
# machine-drafted, needs native review
group-spectroscopy = ספקטרוסקופיה
# machine-drafted, needs native review
# Unstructured meshes of ocean, coastal and climate models (UGRID, ICON, MPAS in NetCDF).
group-ugrid = רשת לא מובנית
group-drawing = שרטוט
# machine-drafted, needs native review
group-volume = נפח

## Summary item labels

label-camera = מצלמה
label-software = תוכנה
label-application = יישום
label-shutter = מהירות תריס
label-aperture = צמצם
label-iso = ISO
label-focal-length = אורך מוקד
label-exposure-bias = פיצוי חשיפה
label-program = מצב
label-metering = מצב מדידה
label-white-balance = איזון לבן
label-flash = מבזק
label-lens = עדשה
label-lens-make = יצרן העדשה
label-lens-range = טווח העדשה
# machine-drafted, needs native review
label-focus-mode = מצב מיקוד
label-stabilisation = ייצוב תמונה
label-taken = צולם
label-digitised = עבר דיגיטציה
label-edited = נערך לאחרונה
label-place = מקום
label-coordinates = קואורדינטות
label-altitude = גובה
label-direction = כיוון צילום
label-accuracy = דיוק
label-gps-time = שעת GPS
label-dimensions = ממדים
label-encoding = קידוד
# machine-drafted, needs native review
label-motion = תנועה
motion-kind-motion-photo = תמונה בתנועה
motion-kind-micro-video = תמונה בתנועה (MicroVideo)
motion-kind-samsung = תמונה בתנועה של Samsung
motion-kind-live-photo = Live Photo
motion-duration = { $seconds } ש׳
label-colour-profile = פרופיל צבע
label-orientation = כיוון התמונה
label-file-name = שם
label-folder = תיקייה
label-file-size = גודל
label-format = פורמט
label-modified = שונה
label-title = כותרת
label-description = תיאור
label-creator = יוצר
label-copyright = זכויות יוצרים
label-keywords = מילות מפתח
label-rating = דירוג
label-colour-label = תווית
label-credentials = אישורי תוכן

## "All fields" group titles

fields-ifd0 = IFD0 (תמונה ראשית)
fields-exif = Exif
fields-gps = GPS
fields-interop = יכולת פעולה הדדית
fields-ifd1 = IFD1 (תמונה ממוזערת)
fields-icc = כותרת פרופיל ICC
fields-xmp = XMP: { $namespace } ({ $prefix })
xmp-namespace = { $prefix ->
    [dc] Dublin Core
    [xmp] בסיסי
    [xmpRights] זכויות
    [xmpMM] ניהול מדיה
    [xmpDM] מדיה דינמית
    [tiff] TIFF
    [exif] Exif
    [exifEX] Exif 2.3+
    [aux] אביזרי מצלמה
    [photoshop] Photoshop
    [crs] הגדרות Camera Raw
    [lr] Lightroom
    [Iptc4xmpCore] IPTC Core
    [Iptc4xmpExt] IPTC Extension
    [plus] רישוי PLUS
    [GPano] תמונה כדורית
    [GCamera] מצלמת Google
    [hdrgm] מפת הגבר HDR
    [mwg-rs] אזורים
    [pdf] PDF
    [stEvt] אירועי משאב
    [stRef] הפניות למשאבים
   *[other] אחר
}
unknown-tag = תג לא ידוע { $number } ({ $context })
blob-bytes = { $num ->
    [one] ({ $num } בית)
    [two] ({ $num } בתים)
   *[other] ({ $num } בתים)
}
maker-note-bytes = ({ $num ->
        [one] בית אחד
        [two] שני בתים
       *[other] { $num } בתים
    }, הערת יצרן, לא פוענח)
more-values = { $shown }, … ({ $total ->
        [one] ערך אחד
        [two] שני ערכים
       *[other] { $total } ערכים
    })

## Generic glue

list-separator = {", "}
one-line-separator = {" · "}

## Exposure

exposure-time-fraction = 1/{ $denominator } s
exposure-time-seconds = { $seconds } s
aperture-value = f/{ $f }
iso-value = ISO { $iso }
focal-length-value = { $mm } מ״מ
focal-length-with-equiv = { $mm } מ״מ (שווה ערך ל־{ $equiv } מ״מ)
focal-length-equiv-only = שווה ערך ל־{ $equiv } מ״מ
exposure-bias-value = { $sign ->
    [plus] +{ $ev } EV
    [minus] −{ $ev } EV
   *[zero] 0 EV
}
exposure-program = { $value ->
    [1] ידני
    [2] תוכנית (אוטומטי)
    [3] עדיפות צמצם
    [4] עדיפות תריס
    [5] יצירתי (עומק שדה)
    [6] פעולה (תריס מהיר)
    [7] דיוקן
    [8] נוף
   *[other] לא ידוע
}
metering-mode = { $value ->
    [1] ממוצע
    [2] ממוצע משוקלל למרכז
    [3] נקודתי
    [4] רב־נקודתי
    [5] מטריצה (הערכתי)
    [6] חלקי
   *[other] אחר
}
# machine-drafted, needs native review
# Canon maker note CameraSettings FocusMode; 256–258 are the live-view variants of 0–2.
canon-focus-mode = { $value ->
    [0] One-Shot AF
    [1] AI Servo AF
    [2] AI Focus AF
    [3] מיקוד ידני
    [4] יחיד
    [5] רציף
    [6] מיקוד ידני
    [16] Pan focus
    [256] One-Shot AF (תצוגה חיה)
    [257] AI Servo AF (תצוגה חיה)
    [258] AI Focus AF (תצוגה חיה)
    [512] מיקוד Movie Snap
    [519] Movie Servo AF
   *[other] אחר
}
# Canon maker note CameraSettings ImageStabilization; 256–260 repeat 0–4.
canon-stabilisation = { $value ->
    [0] כבוי
    [1] פועל
    [2] בצילום בלבד
    [3] פנינג
    [4] דינמי
    [256] כבוי
    [257] פועל
    [258] בצילום בלבד
    [259] פנינג
    [260] דינמי
   *[other] אחר
}
# machine-drafted, needs native review
panasonic-focus-mode = { $value ->
    [1] אוטומטי
    [2] מיקוד ידני
    [4] אוטומטי (לחצן מיקוד)
    [5] אוטומטי (רציף)
    [6] AF-S
    [7] AF-C
    [8] AF-F
   *[other] אחר
}
# machine-drafted, needs native review
panasonic-stabilisation = { $value ->
    [2] פועל (אופטי)
    [3] כבוי
    [4] פועל (מצב 2)
    [5] פועל (אופטי, מעקב)
    [6] פועל (גוף בלבד)
    [7] פועל (גוף בלבד, מעקב)
    [9] Dual IS
    [10] Dual IS (מעקב)
    [11] Dual IS 2
    [12] Dual IS 2 (מעקב)
   *[other] אחר
}
label-comment = הערה
fields-tags = תגי { $kind }
group-picture = הגדרות תמונה
fields-fujifilm = נתוני היצרן של Fujifilm
label-film-simulation = הדמיית פילם
label-dynamic-range = טווח דינמי
label-highlight-tone = אזורים בהירים
label-shadow-tone = צללים
label-colour-setting = צבע
label-sharpness = חדות
label-noise-reduction = הפחתת רעש
label-clarity = בהירות
label-grain-effect = אפקט גרעיניות
label-colour-chrome = כרום צבעוני
label-colour-chrome-blue = כרום צבעוני כחול
label-shutter-type = סוג תריס
label-drive-mode = מצב צילום
fuji-step = { $sign ->
    [plus] +{ $n }
    [minus] −{ $n }
   *[zero] 0
}
fuji-film-mode = { $value ->
    [0] Provia (רגיל)
    [256] דיוקן סטודיו
    [272] דיוקן סטודיו (רוויה גבוהה)
    [288] Astia (רך)
    [304] דיוקן סטודיו (חד יותר)
    [512] Velvia (חי)
    [768] דיוקן סטודיו Ex
    [1024] Velvia
    [1280] Pro Neg. Std
    [1281] Pro Neg. Hi
    [1536] Classic Chrome
    [1792] Eterna
    [2048] Classic Neg.
    [2304] Eterna Bleach Bypass
    [2560] Nostalgic Neg.
    [2816] Reala Ace
   *[other] אחר
}
fuji-monochrome = { $value ->
    [768] שחור־לבן
    [769] שחור־לבן, מסנן אדום
    [770] שחור־לבן, מסנן צהוב
    [771] שחור־לבן, מסנן ירוק
    [784] ספיה
    [1280] Acros
    [1281] Acros, מסנן אדום
    [1282] Acros, מסנן צהוב
    [1283] Acros, מסנן ירוק
   *[other] שחור־לבן
}
fuji-white-balance = { $value ->
    [0] אוטומטי
    [1] אוטומטי (עדיפות ללבן)
    [2] אוטומטי (עדיפות לאווירה)
    [256] אור יום
    [512] מעונן
    [768] פלורסנט אור יום
    [769] פלורסנט לבן יום
    [770] פלורסנט לבן
    [771] פלורסנט לבן חם
    [772] פלורסנט לבן חם לבית
    [1024] נורת להט
    [1280] מבזק
    [1536] מתחת למים
    [3840] מותאם אישית 1
    [3841] מותאם אישית 2
    [3842] מותאם אישית 3
    [3843] מותאם אישית 4
    [3844] מותאם אישית 5
    [4080] טמפרטורת צבע
   *[other] אחר
}
fuji-white-balance-kelvin = { $kelvin } K
fuji-dynamic-range = DR{ $percent }
fuji-dynamic-range-auto = אוטומטי (DR{ $percent })
fuji-dynamic-range-setting = { $value ->
    [0] אוטומטי
    [256] רגיל (DR100)
    [512] רחב 1 (DR230)
    [513] רחב 2 (DR400)
   *[other] אחר
}
fuji-effect = { $value ->
    [0] כבוי
    [32] חלש
    [64] חזק
   *[other] אחר
}
fuji-grain-size = { $value ->
    [16] קטן
    [32] גדול
   *[other] אחר
}
fuji-grain = { $strength }, { $size }
fuji-shutter-type = { $value ->
    [0] מכני
    [1] אלקטרוני
    [2] אלקטרוני (חשיפות ארוכות)
    [3] וילון ראשון אלקטרוני
   *[other] אחר
}
fuji-drive-mode = { $value ->
    [0] תמונה בודדת
    [1] רצף, מהירות נמוכה
    [2] רצף, מהירות גבוהה
   *[other] אחר
}
fuji-focus-mode = { $value ->
    [0] ידני
    [1] AF-S
    [2] AF-C
    [100] אוטומטי
    [101] ידני
   *[other] אחר
}
fuji-stabilisation = { $kind } ({ $mode })
fuji-stabilisation-kind = { $value ->
    [0] ללא
    [1] אופטי
    [2] הזזת חיישן
    [3] עדשה
    [258] הזזת חיישן ודיגיטלי
    [512] דיגיטלי
   *[other] אחר
}
fuji-stabilisation-mode = { $value ->
    [0] כבוי
    [1] פועל, רציף
    [2] פועל, בעת צילום
   *[other] אחר
}
light-source = { $value ->
    [1] אור יום
    [2] פלואורסצנטי
    [3] טונגסטן
    [4] מבזק
    [9] מזג אוויר נאה
    [10] מעונן
    [11] צל
    [12] פלואורסצנטי אור יום
    [13] פלואורסצנטי לבן יום
    [14] פלואורסצנטי לבן קר
    [15] פלואורסצנטי לבן
    [16] פלואורסצנטי לבן חם
    [17] תאורה תקנית A
    [18] תאורה תקנית B
    [19] תאורה תקנית C
    [20] D55
    [21] D65
    [22] D75
    [23] D50
    [24] טונגסטן סטודיו
   *[other] אחר
}
white-balance-auto = אוטומטי
white-balance-manual = ידני
white-balance-manual-with-light = ידני ({ $light })
flash-none = אין פונקציית מבזק
flash-fired = המבזק הופעל
flash-not-fired = המבזק לא הופעל
flash-forced-on = הפעלה כפויה
flash-forced-off = כיבוי כפוי
flash-auto = מצב אוטומטי
flash-return-not-detected = לא זוהה אור חוזר
flash-return-detected = זוהה אור חוזר
flash-red-eye = הפחתת עיניים אדומות

## Lens

lens-focal-single = { $mm } מ״מ
lens-focal-range = { $min }–{ $max } מ״מ
lens-aperture-single = f/{ $f }
lens-aperture-range = f/{ $min }–{ $max }
lens-range = { $focal } { $aperture }

## Date and time

date-with-weekday = { $weekday }, { $date }
date-time = { $date }, { $time }
date-time-zone = { $datetime } ({ $zone })
zone-utc = UTC
zone-offset = UTC{ $offset }
gps-date-time = { $date }, { $time } UTC
detail-zone-from-gps = אזור הזמן חושב לפי שעון ה־GPS

## Location

coordinates = { $lat }° { $latRef ->
    [south] ד׳
   *[north] צ׳
}, { $lon }° { $lonRef ->
    [west] מע׳
   *[east] מז׳
}
altitude = { $ref ->
    [above] { $m } מ׳ מעל פני הים
    [below] { $m } מ׳ מתחת לפני הים
   *[sea] בגובה פני הים
}
compass-point = { $point ->
    [n] צפון
    [nne] צפון־צפון־מזרח
    [ne] צפון־מזרח
    [ene] מזרח־צפון־מזרח
    [e] מזרח
    [ese] מזרח־דרום־מזרח
    [se] דרום־מזרח
    [sse] דרום־דרום־מזרח
    [s] דרום
    [ssw] דרום־דרום־מערב
    [sw] דרום־מערב
    [wsw] מערב־דרום־מערב
    [w] מערב
    [wnw] מערב־צפון־מערב
    [nw] צפון־מערב
   *[nnw] צפון־צפון־מערב
}
facing = פונה ל־{ $point } ({ $degrees }°)
detail-true-north = ביחס לצפון הגאוגרפי
detail-magnetic-north = ביחס לצפון המגנטי
accuracy = ±{ $m } מ׳
distance-km = { $km } ק״מ
place = { $city }, { $country }
place-city-only = { $city }
place-near = בקרבת { $place }
place-detail = { $areas } · { $distance } מ־{ $city }
place-detail-distance = { $distance } מ־{ $city }

## Image

dimensions = { $width } × { $height }
dimensions-megapixels = { $width } × { $height } ({ $mp } MP)
encoding-bit-depth = { $bits } סיביות
encoding-progressive = { $format }, פרוגרסיבי
encoding-baseline = { $format }, בסיסי
encoding-hierarchical = { $format }, היררכי
# machine-drafted, needs native review
encoding-alpha = { $format } { $alpha ->
    [premultiplied] עם אלפא מוכפל מראש
    [transparency] עם שקיפות
   *[alpha] עם אלפא
}
# [Lab], [Palette] and the variants after them: machine-drafted, needs native review
colour-model = { $model ->
    [YCbCr] YCbCr
    [RGB] RGB
    [Greyscale] גווני אפור
    [CMYK] CMYK
    [YCCK] YCCK
    [Lab] L*a*b*
    [Palette] צבע מאונדקס
    [Multichannel] רב־ערוצי
    [Duotone] דו־גוני
    [YCgCo] YCgCo
    [XYB] XYB
    [YC] בהירות/כרומה (YC)
    [Bayer] RAW בתבנית Bayer
    [X-Trans] RAW בתבנית X-Trans
    [CFA] RAW עם מערך מסנני צבע
    [LinearRaw] RAW ליניארי
    [RGBE] RGBE (מעריך משותף)
    [XYZE] XYZE (מעריך משותף)
    [R] ערוץ יחיד (R)
    [RG] שני ערוצים (RG)
   *[other] { $model }
}
adobe-rgb-from-exif = Adobe RGB (לפי תג EXIF; אין פרופיל מוטמע)
orientation = { $value ->
    [1] זקוף (אין צורך בסיבוב)
    [2] שיקוף אופקי (הוחל)
    [3] סיבוב ב־180° (הוחל)
    [4] שיקוף אנכי (הוחל)
    [5] שיקוף אופקי וסיבוב ב־90° נגד כיוון השעון (הוחל)
    [6] סיבוב ב־90° בכיוון השעון (הוחל)
    [7] שיקוף אופקי וסיבוב ב־90° בכיוון השעון (הוחל)
    [8] סיבוב ב־90° נגד כיוון השעון (הוחל)
   *[other] לא ידוע ({ $value })
}
orientation-short = { $value ->
    [1] זקוף
    [2] שיקוף אופקי
    [3] סיבוב ב־180°
    [4] שיקוף אנכי
    [5] שיקוף אופקי וסיבוב ב־90° נגד כיוון השעון
    [6] סיבוב ב־90° בכיוון השעון
    [7] שיקוף אופקי וסיבוב ב־90° בכיוון השעון
    [8] סיבוב ב־90° נגד כיוון השעון
   *[other] ערך { $value }
}

## File

file-size-bytes = { $num ->
    [one] { $num } בית
    [two] { $num } בתים
   *[other] { $num } בתים
}
file-size-unit = { $num } { $unit ->
    [kb] KB
    [mb] MB
    [gb] GB
    [tb] TB
   *[pb] PB
}

## Rights

rating-rejected = נדחה
rating-stars = { $stars } ({ $n } מתוך 5)

## Content credentials

c2pa-present = קיים (לא אומת)
c2pa-detail = קובץ זה נושא מניפסט C2PA; החתימה שלו לא נבדקה.

## ICC header values

icc-class = { $class ->
    [scnr] התקן קלט
    [mntr] התקן תצוגה
    [prtr] התקן פלט
    [link] קישור התקנים
    [spac] מרחב צבע
    [abst] מופשט
    [nmcl] צבע בעל שם
   *[other] לא ידוע
}
icc-intent = { $intent ->
    [0] תפיסתי
    [1] קולורימטרי יחסי
    [2] רוויה
    [3] קולורימטרי מוחלט
   *[other] לא ידוע
}
icc-platform = { $platform ->
    [APPL] Apple
    [MSFT] Microsoft
    [SGI] Silicon Graphics
    [SUNW] Sun Microsystems
    [none] לא צוין
   *[other] לא ידוע
}
icc-coded = { $name } ({ $code })

# machine-drafted, needs native review
label-modality = מודליות
label-body-part = איבר
label-study = בדיקה
label-series = סדרה
label-slice-thickness = עובי פרוסה
label-pixel-spacing = מרווח פיקסלים
label-imager-pixel-spacing = מרווח פיקסלים בגלאי
label-frames = פריימים
label-transfer-syntax = תחביר העברה
label-document = מסמך
label-document-type = סוג מסמך
medical-spacing = { $row } × { $column } מ״מ
medical-mm = { $mm } מ״מ
medical-lossy = { $name } (דחיסה מאבדת מידע)
detail-pixel-spacing = המרחק בין מרכזי הפיקסלים בגוף המטופל (שורות × עמודות).
# machine-drafted, needs native review
# Pixel spacing of an industrial inspection image (DICONDE, CT of an object).
detail-pixel-spacing-object = המרחק בין מרכזי הפיקסלים בעצם הנבדק (שורות × עמודות).
detail-imager-pixel-spacing = נמדד בגלאי, ללא תיקון הגדלה: המרחקים בגוף המטופל בדרך כלל קטנים יותר.

## בדיקה תעשייתית (DICONDE)

# machine-drafted, needs native review
label-nde-component-shape = צורת הרכיב
label-nde-component-manufacturer = יצרן הרכיב
label-nde-material = חומר
label-nde-thickness = עובי החומר
label-nde-pipe-diameter = קוטר הצינור
label-nde-source = מקור
label-nde-environment = סביבה
label-nde-iqi = מחוון איכות תמונה
label-nde-indications = אינדיקציות
label-nde-standard = תקן

## נפחי מחקר (NIfTI, Analyze, NRRD, MetaImage)

# machine-drafted, needs native review
label-voxel-size = גודל ווקסל
label-data-type = סוג נתונים
label-time-points = נקודות זמן
label-orientation-source = כיוון לפי
label-intent = ערכים
label-compression = דחיסה
volume-voxel-size = { $x } × { $y } × { $z } מ״מ
volume-orient-sform = NIfTI sform
volume-orient-qform = NIfTI qform
volume-orient-analyze = כותרת Analyze (לא אמינה)
volume-orient-space = כיווני מרחב של NRRD
volume-orient-transform = טרנספורמציה של MetaImage
volume-orient-spacings = מרווח ווקסלים בלבד
volume-orient-none = לא צוין
volume-orient-vox2ras = FreeSurfer vox2ras
volume-orient-mgh-default = ברירת המחדל של FreeSurfer (לא אמינה)
volume-orient-minc = קוסינוסי כיוון של ממדי MINC
volume-orient-object = קואורדינטות העצם (ללא כיוון מטופל)
volume-content-projections = הטלות (סריקה גולמית)
volume-content-reconstruction = נפח משוחזר
volume-coords-rectilinear = ישר-קווית (מרווח לא אחיד, מוצגת אחידה)
# Scan parameters and tags a FreeSurfer MGH file keeps after its voxels.
label-volume-tr = זמן חזרה
label-volume-te = זמן הד
label-volume-ti = זמן היפוך
label-volume-flip = זווית היפוך
label-volume-fov = שדה ראייה
label-volume-field = עוצמת שדה
label-volume-pedir = קידוד פאזה
label-volume-cmdline = שורת פקודה
label-volume-xform = טרנספורמציית Talairach
label-volume-content = תוכן
label-volume-projections = הטלות
label-volume-angles = טווח סיבוב
label-volume-angle-step = צעד סיבוב
label-volume-kv = מתח השפופרת
label-volume-current = זרם השפופרת
label-volume-power = הספק המקור
label-volume-exposure = חשיפה
label-volume-sod = מקור–עצם
label-volume-odd = עצם–גלאי
label-volume-sdd = מקור–גלאי
label-volume-magnification = הגדלה אופטית
label-volume-binning = בינינג
label-volume-objective = עדשה אובייקטיבית
label-volume-filter = מסנן המקור
label-volume-camera = גלאי
label-volume-instrument = מכשיר
label-volume-software = תוכנה
label-volume-date = נרכש
label-volume-ring = תיקון ארטיפקטים טבעתיים
label-volume-smoothing = החלקה
label-volume-coords = רשת
# machine-drafted, needs native review
# An eye scan (OCT): which eye.
label-volume-eye = עין
volume-eye-right = ימין (OD)
volume-eye-left = שמאל (OS)
detail-volume-unreliable = הקובץ אינו מציין לאן פונים הצירים שלו בגוף המטופל: ייתכן שימין ושמאל הוחלפו, ולכן לא מוצגות תוויות כיוון.
# machine-drafted, needs native review
# An industrial CT volume: its axes are the scanner's, there is no patient.
detail-volume-object = הצירים הם של הסורק, לא של מטופל, ולכן לא מוצגות תוויות כיוון.

## Scientific images (FITS, XISF)

label-object = עצם
label-telescope = טלסקופ
label-instrument = מכשיר
label-filter = מסנן
label-observed = מועד התצפית
label-exposure-time = חשיפה
label-sample = דגימות
label-unit = יחידה
label-hdu = הרחבה
label-centre = מרכז
label-centre-deg = מרכז (מעלות)
label-frame = מערכת קואורדינטות
# machine-drafted, needs native review
science-frame-galactic = גלקטי
science-frame-ecliptic = אקליפטי
label-projection = היטל
label-pixel-scale = קנה מידה של פיקסל
science-seconds = { $seconds } שנ׳
science-arcsec-px = { $arcsec }″ לפיקסל
label-distortion = עיוות
label-wcs = קואורדינטות שמימיות
science-wcs-unsupported = לא נתמך: { $reason }
# machine-drafted, needs native review
science-wcs-approximate = משוער (לפי הכיוון בלבד, ללא עיוות)
label-observer = צופה
label-bayer = תבנית באייר
label-capture-start = תחילת הצילום (UTC)
label-capture-end = סוף הצילום (UTC)
label-frame-rate = קצב פריימים
science-fps = { $fps } פריימים/ש׳
science-frames-cut = { $frames } מתוך { $declared }
# The HDF5 dataset or NetCDF variable shown.
label-dataset = מערך נתונים
label-pixel-size = גודל פיקסל
# "0.25 nm per pixel"; $size may be "0.25 × 0.5".
science-pixel-size = { $size } { $unit } לפיקסל
# Microscopy (DigitalMicrograph, MRC, Gwyddion, Igor).
label-microscope = מיקרוסקופ
label-voltage = מתח האצה
label-magnification = הגדלה
label-imaging-mode = מצב
label-detector = גלאי
label-acquired = מועד הרכישה
label-field-of-view = שדה ראייה
science-kilovolts = { $kv } kV
science-magnification = { $value }×
label-vendor = יצרן
label-scanner = סורק
label-objective = עדשת אובייקטיב
label-pyramid-levels = רמות
# machine-drafted, needs native review
label-emissivity = פליטות
label-object-distance = מרחק לעצם
label-reflected-temperature = טמפרטורה מוחזרת
label-atmospheric-temperature = טמפרטורת האטמוספרה
label-humidity = לחות יחסית
label-raw-range = טווח גולמי
label-temperature-range = טווח טמפרטורות
thermal-temperature = { $value } { $unit }
thermal-metres = { $value } מ׳
thermal-percent = { PERCENT($value) }
thermal-range = { $low } – { $high }
slide-levels = { $count }: { $sizes }
# X-ray diffraction: the experiment geometry from a detector frame's header.
label-wavelength = אורך גל
# Ångström, e.g. "0.97625 Å".
xtal-angstrom = { $value } Å
label-photon-energy = אנרגיית פוטון
xtal-kev = { $value } keV
# Sample to detector distance.
label-detector-distance = מרחק הגלאי
xtal-mm = { $value } מ״מ
# Where the direct beam meets the detector, in pixels from the image's top-left corner.
label-beam-centre = מרכז האלומה
xtal-beam-centre = x { $x }, y { $y } px
# The detector arm's swing angle (2θ).
label-detector-swing = 2θ של הגלאי
xtal-degrees = { $value }°
# Goniometer angles (Greek letters are the field's usual names).
label-phi = פי (φ)
label-omega = אומגה (ω)
label-chi = חי (χ)
label-kappa = קאפה (κ)
# The sample's rotation during the exposure: start angle plus width, e.g. "12.5° + 0.1°".
label-oscillation = תנודה
xtal-oscillation = { $start }° + { $width }°
# $axis is the axis name from the file, e.g. "phi" or "omega".
xtal-oscillation-axis = { $start }° + { $width }° סביב { $axis }
# The count at which a pixel saturates.
label-overload = ערך רוויה
# The finest lattice spacing (d) recorded at the detector's edge and in its corners.
label-resolution = רזולוציה
xtal-resolution = { $edge } Å בשוליים, { $corner } Å בפינות
label-geometry = גאומטריה
xtal-curved = לוחית הדמיה מעוקלת: ללא q, d או טבעות רזולוציה
xtal-incomplete = כותרת לא שלמה: ללא q, d או טבעות רזולוציה
# All-fields group title: "FITS header".
fields-science-header = כותרת { $format }
# machine-drafted, needs native review
fields-asdf-tree = עץ ASDF
label-asdf-model = מודל נתונים
label-asdf-version = גרסת קובץ
label-asdf-standard = תקן ASDF
label-asdf-library = נכתב על ידי
# machine-drafted, needs native review
# All-fields group title for a CERN ROOT file's list of keys (objects).
fields-root-keys = מפתחות ROOT
# CERN ROOT files: the ROOT version that wrote the file (e.g. "6.28/04"), its compression,
# the number of keys (stored objects), and the object shown: class (e.g. "TH2F"), its path in
# the file, title, number of entries and sum of weights.
label-root-version = גרסת ROOT
label-root-compression = דחיסה
label-root-keys = מפתחות
label-root-class = מחלקה
label-root-name = שם
label-root-title = כותרת
label-root-entries = רשומות
label-root-sum-weights = סכום המשקלים
# A graph's number of points.
label-root-points = נקודות
# $axis is "x", "y" or "z".
label-root-axis = ציר { $axis }
# $quantity is the axis title with its unit, e.g. "Energy [GeV]"; $min and $max the axis
# limits.
root-axis-fixed = { $quantity }: { $bins ->
        [one] תא אחד
        [two] { $bins } תאים
       *[other] { $bins } תאים
    }, { $min } עד { $max }
root-axis-variable = { $quantity }: { $bins ->
        [one] תא אחד ברוחב משתנה
        [two] { $bins } תאים ברוחב משתנה
       *[other] { $bins } תאים ברוחב משתנה
    }, { $min } עד { $max }
label-root-mean-rms = ממוצע ו-RMS { $axis }
root-mean-rms = { $mean } ± { $rms }
# Trees (event data) are listed only, with their entry counts.
label-root-trees = עצים
root-tree = { $name } ({ $entries ->
        [one] רשומה אחת
        [two] { $entries } רשומות
       *[other] { $entries } רשומות
    })
# machine-drafted, needs native review
# CASA images: what the image is (from its directory name), its restoring beam (major and
# minor axis, position angle), the beams of its planes when they differ, the rest frequency,
# spectral reference frame (e.g. "LSRK") and velocity definition of the frequency labels,
# the default mask (e.g. "mask0") and the casacore storage manager with its byte order.
label-casa-type = סוג תמונה
casa-type-image = תמונה משוחזרת
casa-type-residual = שארית
casa-type-model = מודל
casa-type-pb = אלומה ראשית
casa-type-mask = מסכת ניקוי
casa-type-psf = פונקציית פיזור נקודה
casa-type-sumwt = סכום משקלים
casa-type-weight = משקלים
label-casa-beam = אלומת שחזור
casa-beam = { $major }″ × { $minor }″, זמ { $pa }°
label-casa-plane-beams = אלומות לפי מישור
casa-plane-beams = { $count ->
        [one] { $count } אלומה
        [two] { $count } אלומות
       *[other] { $count } אלומות
    }, ציר ראשי { $min }″ עד { $max }″
label-casa-rest-frequency = תדר מנוחה
casa-ghz = { $ghz } GHz
label-casa-spectral-frame = מערכת ייחוס ספקטרלית
label-casa-velocity = הגדרת מהירות
casa-velocity-radio = רדיו
casa-velocity-optical = אופטית
casa-velocity-relativistic = יחסותית
label-casa-mask = מסכת ברירת מחדל
label-casa-storage = אחסון
casa-storage-big = { $manager }, big-endian
casa-storage-little = { $manager }, little-endian
# machine-drafted, needs native review
# Unstructured meshes: the mesh's name in the file, the convention describing it (UGRID,
# ICON, MPAS), its counts, face shapes, coordinates and where the values sit.
label-ugrid-name = רשת
label-ugrid-convention = מוסכמה
label-ugrid-nodes = צמתים
label-ugrid-faces = פאות
label-ugrid-edges = קצוות
label-ugrid-shapes = צורות הפאות
label-ugrid-coordinates = קואורדינטות
label-ugrid-values = ערכים על
label-ugrid-split = פוצלו לציור
# $n is the formatted count, $count the number (for the plural).
ugrid-triangles = { $count ->
    [one] { $count } משולש
    [two] { $count } משולשים
   *[other] { $count } משולשים
}
ugrid-quads = { $count ->
    [one] { $count } מרובע
    [two] { $count } מרובעים
   *[other] { $count } מרובעים
}
# Faces of five or more nodes; $max is the most nodes of one face.
ugrid-polygons = { $count ->
    [one] { $count } מצולע בן { $max } צמתים
    [two] { $count } מצולעים בני עד { $max } צמתים
   *[other] { $count } מצולעים בני עד { $max } צמתים
}
ugrid-lonlat = קו אורך וקו רוחב
ugrid-projected = x ו-y מוטלים
ugrid-projected-unit = x ו-y מוטלים ({ $unit })
ugrid-on-faces = פאות (בצבע אחיד)
ugrid-on-nodes = צמתים (באינטרפולציה על פני הפאות)
# A mesh file without data variables: the image shows how many nodes each face has.
ugrid-grid-only = אין: התמונה מציגה את מספר הצמתים בכל פאה
# Faces cut at the antimeridian or closed to a pole so they draw on a flat map.
ugrid-split = { $count ->
    [one] { $count } פאה באנטי-מרידיאן או בקוטב
    [two] { $count } פאות באנטי-מרידיאן או בקוטב
   *[other] { $count } פאות באנטי-מרידיאן או בקוטב
}
# machine-drafted, needs native review
# Slide annotations (NDP.view, Aperio ImageScope, ASAP sidecar files).
label-annotation-file = קובץ
# "slide.xml (Aperio ImageScope)"
annotation-source = { $file } ({ $format })
label-annotation-shapes = צורות
label-annotation-groups = קבוצות
# Shapes or points over the reader's limits, left out.
label-annotation-skipped = הושמטו
annotation-polygon = מצולע
annotation-polyline = קו
annotation-rectangle = מלבן
annotation-ellipse = אליפסה
annotation-point = נקודה
annotation-points = נקודות
annotation-ruler = סרגל
annotation-arrow = חץ
# The list of annotations is cut short: "and 1,200 more".
annotation-more = ועוד { $count }
# All-fields group title: "Annotations in slide.xml".
fields-annotations-header = הערות ב־{ $file }
label-drawing-version = גרסה
label-drawing-units = יחידות
label-drawing-extents = תחום
label-drawing-sheet = גיליון
label-drawing-layouts = פריסות
label-drawing-layers = שכבות
label-drawing-entities = ישויות
label-drawing-codepage = דף קוד
label-drawing-missing-fonts = גופנים חלופיים
# machine-drafted, needs native review
# A font file the drawing names that was not found: the styles naming it, and the font drawing its text instead.
drawing-missing-font = { $file } (סגנונות { $styles }), הוחלף בגופן { $substitute }
# A Big Font (double-byte text) that was not found; its characters come from other fonts.
drawing-missing-bigfont = { $file } (סגנונות { $styles }), הוחלף בגופנים חלופיים
# Survey data (LandXML).
label-survey-crs = מערכת קואורדינטות
survey-crs-none = לא צוין
label-survey-project = שם הפרויקט
label-survey-application = נכתב על ידי
label-survey-surface = מודל פני קרקע
# A TIN surface: its name, counts, plan and sloped areas, the volume above its lowest point, its elevations and the contour interval drawn; unit is "m" or "ft" (areas in square units, volume in cubic units).
survey-surface = { $name }: { $points ->
        [one] נקודה אחת
        [two] שתי נקודות
       *[other] { $points } נקודות
    }, { $triangles ->
        [one] משולש אחד
        [two] שני משולשים
       *[other] { $triangles } משולשים
    }; שטח בהיטל { $area } { $unit }², שטח פנים { $area3d } { $unit }²; { $volume } { $unit }³ מעל הנקודה הנמוכה ביותר; גבהים { $zmin }–{ $zmax } { $unit }; קווי גובה כל { $interval } { $unit }
label-survey-alignment = תוואי
# An alignment: its name, length and first and last stations ("1+234.50").
survey-alignment = { $name }: { $length } { $unit }, קילומטראז׳ { $start } עד { $end }
# SHX shape fonts found and used for the drawing's text.
label-drawing-shx-fonts = גופני SHX
label-drawing-warnings = בעיות
label-plot-paper = נייר
# "2 of 5": the page shown and how many the plot has.
plot-page-of = { $page } מתוך { $pages }
label-plot-page = עמוד
label-plot-pens = עטים בשימוש
# The printer language a plot came wrapped in, e.g. "PCL 5".
label-plot-job = עבודת הדפסה
# What a plot draws: strokes, fills and labels.
label-plot-items = פריטים משורטטים
# Width × height of what is drawn, in drawing units: "1,250 × 800 mm".
drawing-extents = { $width } × { $height } { $unit }
drawing-extents-unitless = { $width } × { $height }
drawing-model-space = מודל
# "24, 3 off or frozen"
drawing-layers-off = { $num }, מהן { $off } כבויות או מוקפאות
# DXF $INSUNITS.
drawing-units = { $code ->
    [1] אינצ׳ים
    [2] רגל
    [3] מיילים
    [4] מילימטרים
    [5] סנטימטרים
    [6] מטרים
    [7] קילומטרים
    [8] מיקרו־אינצ׳ים
    [9] אלפיות אינץ׳
    [10] יארדים
    [11] אנגסטרום
    [12] ננומטרים
    [13] מיקרונים
    [14] דצימטרים
    [15] דקמטרים
    [16] הקטומטרים
    [17] גיגה־מטרים
    [18] יחידות אסטרונומיות
    [19] שנות אור
    [20] פרסקים
    [21] רגל מדידה אמריקנית
   *[other] ללא יחידות
}
# All-fields group title: "DXF entities".
fields-drawing-entities = ישויות { $format }

## פריסות מעגלים משולבים (GDSII)

# machine-drafted, needs native review
label-layout-library = ספרייה
label-layout-units = יחידות
# The database unit (the grid coordinates are stored in) and the user unit, with SI symbols:
# "1 nm database unit, 1 µm user unit".
layout-units = { $db } יחידת מסד נתונים, { $user } יחידת משתמש
# The top cell shown (each top cell is a page).
label-layout-cell = תא
label-layout-cells = תאים
# Cells no other cell places.
label-layout-top-cells = תאים עליונים
# "TOP, TEST, PAD and 12 more"
layout-top-cells-more = { $names } ועוד { $more }
# Layer/datatype pairs.
label-layout-layers = שכבות
# Polygons, paths and boxes, as stored (each cell once).
label-layout-shapes = צורות
label-layout-texts = תוויות
# Cell references and the placements they make: "1,204 (88,320 placements)".
label-layout-references = הפניות
layout-references = { $refs } ({ $placements } { $placements ->
    [one] מיקום
    [two] מיקומים
   *[other] מיקומים
})
# Shapes with every cell placement expanded.
label-layout-flat-shapes = צורות לאחר שיטוח
label-layout-bounds = גבולות
# Corners of what the cell draws, micrometres: "-12.5, 0 to 2,900, 3,500 µm".
layout-bounds = { $x0 }, { $y0 } עד { $x1 }, { $y1 } µm

## מעגלים מודפסים ושרטוטים חשמליים (Gerber, Excellon, KiCad)

# machine-drafted, needs native review
group-board = מעגל מודפס
group-schematic = שרטוט חשמלי
# The board view shown (its layers, its top or bottom), or the schematic sheet.
label-pcb-view = תצוגה
label-pcb-project = פרויקט
label-pcb-revision = גרסה
label-pcb-date = תאריך
# The CAD program that wrote the file.
label-pcb-generator = נוצר באמצעות
# Width × height of the board outline: "100 × 80 mm".
label-pcb-size = גודל הלוח
label-pcb-thickness = עובי
label-pcb-copper-layers = שכבות נחושת
label-pcb-sheets = גיליונות
# The board's layers or files, each with its role: "board-F_Cu.gbr (Top copper)".
label-pcb-layers = שכבות
pcb-layer-role = { $name } ({ $role })
label-pcb-components = טביעות רגל
label-pcb-symbols = סמלים
label-pcb-nets = נטים
label-pcb-tracks = מסלולים
label-pcb-wires = חוטים
label-pcb-vias = ויות
# Drilled holes and how many drill sizes: "1,204 in 9 sizes".
label-pcb-holes = חורים
pcb-holes = { $holes } ב-{ $sizes } { $sizes ->
    [one] גודל
    [two] גדלים
   *[other] גדלים
}
label-pcb-apertures = צמצמים
# What a board layer or file is, shown after its name.
pcb-role-inner-copper = נחושת פנימית { $n }
pcb-role-top-copper = נחושת עליונה
pcb-role-bottom-copper = נחושת תחתונה
pcb-role-top-solder-mask = מסכת הלחמה עליונה
pcb-role-bottom-solder-mask = מסכת הלחמה תחתונה
pcb-role-top-silkscreen = הדפס משי עליון
pcb-role-bottom-silkscreen = הדפס משי תחתון
pcb-role-top-paste = משחה עליונה
pcb-role-bottom-paste = משחה תחתונה
pcb-role-top-solder-paste = משחת הלחמה עליונה
pcb-role-bottom-solder-paste = משחת הלחמה תחתונה
pcb-role-top-adhesive = דבק עליון
pcb-role-bottom-adhesive = דבק תחתון
pcb-role-top-courtyard = אזור רכיב עליון
pcb-role-bottom-courtyard = אזור רכיב תחתון
pcb-role-top-fabrication = ייצור עליון
pcb-role-bottom-fabrication = ייצור תחתון
pcb-role-board-outline = קו מתאר הלוח
pcb-role-plated-drill = קידוח מצופה
pcb-role-non-plated-drill = קידוח לא מצופה
pcb-role-drill = קידוח
pcb-role-drill-holes = חורי קידוח
pcb-role-mechanical = מכנית
pcb-role-margin = שוליים
pcb-role-drawings = שרטוטים
pcb-role-comments = הערות
pcb-role-engineering-change-1 = שינוי הנדסי 1
pcb-role-engineering-change-2 = שינוי הנדסי 2
pcb-role-vias = ויות
pcb-role-other = אחרת
pcb-role-unknown = לא ידועה
label-pcb-macros = מאקרו צמצמים

## ספקטרומים (JCAMP-DX, SPC, mzML, mzXML, Bruker NMR)

# machine-drafted, needs native review
group-spectrum = ספקטרום
label-spectrum-format = פורמט
label-spectrum-technique = שיטה
label-spectrum-title = כותרת
# The page shown: "3 of 1,204: Scan 1532".
label-spectrum-page = עמוד
spectrum-page-of = { $page } מתוך { $pages }: { $name }
# An axis with its unit: "Wavenumber (cm⁻¹)".
spectrum-axis = { $title } ({ $unit })
label-spectrum-x = ציר X
label-spectrum-y = ציר Y
label-spectrum-nucleus = גרעין
label-spectrum-frequency = תדר הספקטרומטר
label-spectrum-solvent = ממס
label-spectrum-pulse-program = תוכנית פולסים
label-spectrum-spectra = ספקטרומים
label-spectrum-points = נקודות
label-spectrum-instrument = מכשיר
# Who or what lab measured it.
label-spectrum-origin = מקור
label-spectrum-date = נמדד
# How Specular made the page from the raw data (an FID transformed, phased automatically).
label-spectrum-processing = עיבוד
spectrum-technique-ir = אינפרה-אדום
spectrum-technique-raman = ראמאן
spectrum-technique-uv-vis = UV-Vis
spectrum-technique-fluorescence = פלואורסצנציה
spectrum-technique-nmr = NMR
spectrum-technique-ms = ספקטרום מסות
spectrum-technique-chromatogram = כרומטוגרמה
spectrum-technique-other = ספקטרום

## מודלים בתלת-ממד (STL, OBJ, PLY, glTF, 3MF)

# machine-drafted, needs native review
group-mesh = מודל תלת-ממדי
label-triangles = משולשים
label-vertices = קודקודים
label-materials = חומרים
label-textures = טקסטורות
label-up-axis = ציר מעלה
# Textures the model uses, some of which could not be read: "3 (1 missing)".
mesh-textures-missing = { $count } ({ $missing } חסרים)

## מבנים מולקולריים (PDB, mmCIF, CIF, SDF, XYZ)

# machine-drafted, needs native review
group-structure = מבנה
# The entry's identifier: a PDB ID such as 1CRN, or the CIF data block's name.
label-structure-id = רשומה
label-structure-classification = סיווג
# How the structure was determined: X-ray diffraction, electron microscopy, solution NMR…
label-structure-method = שיטה
# The crystallographic or cryo-EM resolution, in ångströms.
label-structure-resolution = רזולוציה
structure-resolution = { $value } Å
label-structure-deposited = הפקדה
label-structure-formula = נוסחה
label-structure-chains = שרשראות
# Residues (amino acids, nucleotides) in the polymer chains.
label-structure-residues = שיירים
label-structure-atoms = אטומים
# Atoms after the unit cell was filled by symmetry, and those the file lists: "96 (12 in the asymmetric unit)".
structure-atoms-cell = { $count } ({ $asymmetric } ביחידה האסימטרית)
label-structure-bonds = קשרים
# Models in the file (an NMR ensemble), of which the first is shown.
label-structure-models = מודלים
# Molecules (SDF records) or data blocks in the file, of which the first is shown.
label-structure-records = רשומות
structure-first-of = { $count } (מוצג הראשון)
# Small molecules bound to the polymer, by their three-letter codes.
label-structure-ligands = ליגנדים
# A list cut short: "HEM, NAG, SO4 and 9 more".
structure-ligands-more = { $list } ועוד { $more }
label-structure-cell = תא יחידה
# Unit cell edge lengths (Å) and angles (degrees).
structure-cell = a = { $a } Å, b = { $b } Å, c = { $c } Å, α = { $alpha }°, β = { $beta }°, γ = { $gamma }°
label-structure-space-group = חבורת מרחב
label-structure-secondary = מבנה שניוני
structure-ss-file = מהקובץ
structure-ss-computed = מחושב מהשרשרת הראשית

## HDF4 ו־HDF-EOS2

# machine-drafted, needs native review
group-hdf4 = HDF4
# The HDF library version that wrote the file, e.g. "NCSA HDF Version 4.2 Release 15".
label-hdf4-library = ספרייה
# Counts of the file's scientific datasets, raster images, groups (Vgroups) and tables (Vdatas).
label-hdf4-datasets = מערכי נתונים
label-hdf4-images = תמונות רסטר
label-hdf4-vgroups = קבוצות
label-hdf4-vdatas = טבלאות
# HDF-EOS: NASA's Earth Observing System conventions on top of HDF4 (MODIS, ASTER, AIRS…).
group-hdf-eos = HDF-EOS
label-eos-version = גרסה
# The grids, swaths and point structures the file defines.
label-eos-structures = מבנים
eos-grids = { $count ->
    [one] { $count } רשת ({ $names })
    [two] { $count } רשתות ({ $names })
   *[other] { $count } רשתות ({ $names })
}
eos-swaths = { $count ->
    [one] { $count } רצועת סריקה ({ $names })
    [two] { $count } רצועות סריקה ({ $names })
   *[other] { $count } רצועות סריקה ({ $names })
}
eos-points = { $count ->
    [one] { $count } מבנה נקודות
    [two] { $count } מבני נקודות
   *[other] { $count } מבני נקודות
}
# The grid or swath the dataset shown belongs to.
label-eos-structure = מבנה
# The grid's map projection as its GCTP name, e.g. GCTP_SNSOID (sinusoidal).
label-eos-projection = היטל
label-eos-grid-size = גודל רשת
# Columns × rows.
eos-grid-size = { $width } × { $height }
# The grid's upper-left and lower-right corners.
label-eos-corners = פינות
# The product's short name from the inventory metadata, e.g. MOD09A1.
label-eos-product = מוצר
# When the data was acquired.
label-eos-time = זמן
eos-time = { $begin } עד { $end }

## תמונות פלנטריות (VICAR, PDS3, PDS4: נתוני משימות של NASA ו־ESA)

# machine-drafted, needs native review
group-planetary = תמונה פלנטרית
# The body observed, e.g. MARS, Ganymede.
label-planetary-target = מטרה
label-planetary-mission = משימה
# The spacecraft, rover or lander carrying the instrument.
label-planetary-spacecraft = חללית
label-planetary-start = שעת התחלה
label-planetary-stop = שעת סיום
# The product's identifier in the archive.
label-planetary-product = מזהה מוצר
# The map projection of a map-projected image, as the label names it.
label-planetary-projection = היטל מפה
label-planetary-resolution = רזולוציה
planetary-metres-per-pixel = { $metres } מ׳ לפיקסל
planetary-pixels-per-degree = { $pixels } פיקסלים לכל מעלה
# Latitude and longitude of the map projection's centre.
label-planetary-centre = מרכז ההיטל
planetary-lat-lon = °{ $lat }, °{ $lon }
# The latitudes and longitudes the map covers.
label-planetary-coverage = כיסוי
planetary-coverage = רוחב °{ $south } עד °{ $north }, אורך °{ $west } עד °{ $east }
# The body's equatorial radius the map uses.
label-planetary-radius = רדיוס הגוף
planetary-km = { $km } ק״מ

## OMERO (תמונות שנפתחו משרת OMERO או מ־Image Data Resource)

# machine-drafted, needs native review
group-omero = OMERO
label-omero-server = שרת
# $server the server's address; $user the user name signed in with.
omero-server-user = { $server } (בתור { $user })
omero-server-public = { $server } (ציבורי)
label-omero-image = תמונה
# $name the image's name; $id its number on the server.
omero-image = { $name } (מזהה { $id })
# The project and dataset the image is in, as "Project › Dataset".
label-omero-project = פרויקט
# The screen, plate and well, as "Screen › Plate › B4".
label-omero-screen = סינון
label-omero-owner = בעלים
# $owner the image's owner; $group the OMERO group it belongs to.
omero-owner-group = { $owner } ({ $group })
label-omero-acquired = נרכש
label-omero-dimensions = ממדים
omero-size = { $width } × { $height }
omero-z = { $count ->
    [one] מישור Z אחד
    [two] { $count } מישורי Z
   *[other] { $count } מישורי Z
}
omero-c = { $count ->
    [one] ערוץ אחד
    [two] { $count } ערוצים
   *[other] { $count } ערוצים
}
omero-t = { $count ->
    [one] נקודת זמן אחת
    [two] { $count } נקודות זמן
   *[other] { $count } נקודות זמן
}
# The physical size of a pixel along X, Y (and Z).
label-omero-physical = גודל פיקסל
label-omero-channels = ערוצים
# A channel not shown in the server's rendering.
omero-channel-hidden = { $channel } (מוסתר)
# How the pixels are fetched from the server: rendered tiles (JPEG) or the whole plane (PNG).
label-omero-fetched = נטען בתור
omero-fetched-tiles = אריחי { $encoding } בגודל { $width } × { $height }, { $levels ->
    [one] רמה אחת
    [two] { $levels } רמות
   *[other] { $levels } רמות
}
omero-fetched-whole = מישורים שלמים בתור { $encoding }

## תמונות טרנספורמציית החזרה (PTM, RTI, Relight)

# machine-drafted, needs native review
group-rti = טרנספורמציית החזרה
# The reflectance model the coefficients belong to: PTM (polynomial), HSH (hemispherical harmonics), RBF…
label-rti-basis = בסיס
label-rti-order = סדר
# Coefficients per colour channel.
label-rti-terms = מקדמים
# How colour is stored: LRGB (one colour, varying luminance), RGB, YCC…
label-rti-colour = פריסת צבע
# Coefficient images stored in the file.
label-rti-planes = מישורים
# The byte-to-coefficient mapping the file states, per coefficient.
label-rti-scale = קנה מידה
label-rti-bias = היסט

## ענני נקודות

# machine-drafted, needs native review
group-points = ענן נקודות
label-points = נקודות
# Points read of those in the file, when a very large cloud was evenly thinned: "32,000,000 of 120,000,000".
points-subsampled = { $loaded } מתוך { $total }
# The LAS point data record format number (0–10).
label-point-format = תבנית נקודה
# The coordinate reference system.
label-crs = מערכת קואורדינטות
# What each point carries besides its position.
label-attributes = מאפיינים
points-channel-rgb = RGB
points-channel-intensity = עוצמה
points-channel-classification = סיווג
points-channel-returns = החזרים
# ASPRS classes present, with point counts: "Ground (1,234), Building (567)".
label-classes = מחלקות
# E57 scans, with point counts.
label-scans = סריקות
point-class-0 = מעולם לא סווג
point-class-1 = לא מסווג
point-class-2 = קרקע
point-class-3 = צמחייה נמוכה
point-class-4 = צמחייה בינונית
point-class-5 = צמחייה גבוהה
point-class-6 = מבנה
point-class-7 = נקודה נמוכה (רעש)
point-class-8 = נקודת מפתח של המודל
point-class-9 = מים
point-class-10 = מסילת ברזל
point-class-11 = פני כביש
point-class-12 = חפיפה
point-class-13 = תיל הגנה
point-class-14 = מוליך
point-class-15 = עמוד מתח גבוה
point-class-16 = מחבר כבלים
point-class-17 = משטח גשר
point-class-18 = רעש גבוה
point-class-19 = מבנה עילי
point-class-20 = קרקע שהתעלמו ממנה
point-class-21 = שלג
point-class-22 = החרגה זמנית
# A class without a standard name, by number.
point-class-other = מחלקה { $class }

## Gaussian splats

# machine-drafted, needs native review
group-splats = ספלאטים גאוסיאניים
label-splats = ספלאטים
# Splats shown of those in the file, when a very large file was evenly thinned: "12,000,000 of 30,000,000".
splats-shown-of = { $shown } מתוך { $total }
# The degree (1 to 3) of the spherical harmonics that give each splat its view-dependent colour.
label-splat-harmonics = צבע תלוי זווית צפייה
splats-harmonics-none = אין
# $format: the file format, e.g. "SPZ".
splats-format-antialiased = { $format }, אומן עם החלקת קצוות

## Warnings

# machine-drafted, needs native review
warn-not-diagnostic = לא לשימוש אבחוני. Specular הוא מציג תמונות כללי, לא מכשיר רפואי: התצוגה שלו אינה מכוילת לאבחון.

warn-gps = קובץ זה מתעד את המקום שבו צולם. אם המיקום אמור להישאר פרטי, יש להסיר אותו לפני השיתוף.
warn-null-island = מיקום ה־GPS הוא בדיוק 0°, 0°; כנראה שלמצלמה לא הייתה קליטת לוויינים, ולכן המיקום ככל הנראה שגוי.
warn-date-future = תאריך הצילום הוא בעתיד; כנראה ששעון המצלמה כוון באופן שגוי.
warn-icc-damaged = פרופיל הצבע המוטמע פגום ולא ניתן היה לקרוא אותו.
warn-metadata-failed = לא ניתן היה לקרוא את המטא-נתונים בגלל שגיאה ב-Specular. התמונה עצמה תקינה.
warn-orientation-conflict = יש סתירה בין EXIF ל־XMP לגבי כיוון התמונה (EXIF: { $exif }, XMP: { $xmp }). נעשה שימוש בערך של EXIF.
warn-exif-damaged = לא ניתן היה לקרוא חלק מנתוני ה־EXIF ({ $error }); השאר מוצג.
warn-xmp-damaged = המטא־נתונים של XMP פגומים ({ $error }); ניתן היה לקרוא רק חלק מהם.

## About box

geonames-attribution = שמות מקומות: GeoNames (www.geonames.org), ברישיון Creative Commons ייחוס 4.0 (CC BY 4.0, https://creativecommons.org/licenses/by/4.0/). חיפוש לא מקוון באמצעות ספריית reverse_geocoder מאת Grant Miner (MIT או Apache-2.0). הנתונים הומרו לרשימת ערים; הם מסופקים כמות שהם, ללא אחריות.

## TIFF and DNG structure values (All fields)
# machine-drafted, needs native review

subfile-type = { $value ->
    [0] תמונה ברזולוציה מלאה
    [1] תמונה ברזולוציה מופחתת
    [2] עמוד אחד מתמונה מרובת עמודים
    [4] מסכת שקיפות
   *[other] אחר
}
predictor = { $value ->
    [1] ללא
    [2] הפרש אופקי
    [3] הפרש בנקודה צפה
   *[other] אחר
}
extra-samples = { $value ->
    [0] לא צוין
    [1] אלפא מוכפלת מראש
    [2] אלפא ישירה
   *[other] אחר
}
sample-format = { $value ->
    [1] מספר שלם ללא סימן
    [2] מספר שלם עם סימן
    [3] נקודה צפה
    [4] לא מוגדר
   *[other] אחר
}

## Geospatial (GeoTIFF, GeoJSON, Shapefile, GeoPackage, KML/KMZ, GPX)

# machine-drafted, needs native review
group-geospatial = מידע גאו-מרחבי
label-geo-crs = מערכת קואורדינטות
label-geo-datum = דאטום
label-geo-projection = היטל
label-geo-unit = יחידות
label-geo-bounds = תחום
label-geo-lonlat-bounds = קו אורך וקו רוחב
label-geo-pixel-size = גודל פיקסל
label-geo-overviews = תצוגות מוקטנות
label-geo-nodata = ערך ללא נתונים
label-geo-raster-type = סוג רסטר
label-geo-vertical = ייחוס אנכי
label-geo-citation = ציטוט
label-geo-features = ישויות
label-geo-vertices = קודקודים
label-geo-layer = שכבה
label-geo-layers = שכבות
label-geo-tiles = אריחים
label-geo-encoding = קידוד טקסט
label-geo-warnings = בעיות
# The companion files a georeference was read from ("scene.jgw, scene.prj").
label-geo-sidecars = קבצים נלווים
# machine-drafted, needs native review
# A projection Specular cannot convert to longitude and latitude: $method is its name.
geo-projection-unsupported = { $method } (אין תמיכה בקואורדינטות)
# How longitude and latitude are brought to WGS 84 (what GPS and web maps use).
label-geo-to-wgs84 = אל WGS 84
geo-to-wgs84-same = לא נדרש
geo-to-wgs84-helmert = המרת דאטום (Helmert)
geo-to-wgs84-by-area = המרת דאטום (Helmert, לפי אזור)
geo-to-wgs84-unknown = לא ידוע: הקואורדינטות בדאטום של הקובץ עצמו
# A coordinate system's name and its EPSG code: "WGS 84 / UTM zone 33N (EPSG:32633)".
geo-crs-epsg = { $name } (EPSG:{ $code })
geo-crs-unknown = לא צוין
# Map bounds: the lower-left and upper-right corners.
geo-bounds = { $x0 }, { $y0 } עד { $x1 }, { $y1 }
# Bounds in degrees: west to east longitude, south to north latitude.
geo-lonlat-bounds = אורך { $west }° עד { $east }°, רוחב { $south }° עד { $north }°
geo-pixel-size = { $x } × { $y }
geo-overviews = { $n ->
    [one] תצוגה מוקטנת אחת: { $sizes }
    [two] { $n } תצוגות מוקטנות: { $sizes }
   *[other] { $n } תצוגות מוקטנות: { $sizes }
}
geo-pixel-is-point = הפיקסלים הם נקודות
geo-layer = { $features ->
    [one] ישות אחת
    [two] { $features } ישויות
   *[other] { $features } ישויות
}, { $fields ->
    [one] שדה אחד
    [two] { $fields } שדות
   *[other] { $fields } שדות
}
# A GeoPackage tile pyramid: its table, the zoom level shown and its tile count.
geo-tiles = { $table }, רמת זום { $zoom }, { $tiles ->
    [one] אריח אחד
    [two] { $tiles } אריחים
   *[other] { $tiles } אריחים
}
fields-geo-geokeys = מפתחות GeoTIFF
fields-geo-gdal = מטא-נתונים של GDAL
fields-geo-wkt = מערכת קואורדינטות (WKT)
fields-geo-projjson = מערכת קואורדינטות (PROJJSON)
# machine-drafted, needs native review
# What a web map tile set (MBTiles, PMTiles) says about itself: container, tile type, zoom
# levels, description and other metadata.
fields-geo-tileset = ערכת אריחים
# The credit line a map's data asks to be shown with it ("© OpenStreetMap contributors").
label-geo-attribution = קרדיט
# The header lines of a georeferenced raster format (ENVI, ASCII grid, DTED, ERDAS IMAGINE).
fields-geo-header = כותרת
# machine-drafted, needs native review
# GIFTI files: the format version, what the file holds, the number of data arrays and the
# encoding of their values (e.g. "GZipBase64Binary").
label-gifti-version = גרסת GIFTI
label-gifti-role = מכיל
gifti-role-surface = משטח
gifti-role-labels = תוויות לכל קודקוד
gifti-role-data = ערכים לכל קודקוד
label-gifti-arrays = מערכי נתונים
label-gifti-encoding = קידוד
label-gifti-vertices = קודקודים
label-gifti-triangles = משולשים
# The brain structure, e.g. "CortexLeft".
label-gifti-structure = מבנה
# A surface's geometric type, e.g. "Anatomical", "Inflated".
label-gifti-geometry = סוג גאומטרי
# The coordinate transform of the vertices: $from and $to are coordinate spaces such as
# "scanner", "Talairach" or "MNI 152".
label-gifti-transform = קואורדינטות
gifti-transform = { $from } (אין צורך בהמרה ל־{ $to })
gifti-transform-applied = { $from } ← { $to } (הוחל)
# The surface file a data file is drawn on, found beside it.
label-gifti-surface = מוצג על
# The data array shown: its name, NIfTI intent (e.g. "NIFTI_INTENT_SHAPE") and data type.
label-gifti-array = מערך
label-gifti-intent = Intent
label-gifti-data-type = סוג נתונים
# Entries of the label table (names and colours of regions).
label-gifti-labels = טבלת תוויות
# CIFTI-2 files: the version, NIfTI intent (e.g. "ConnDenseScalar") and matrix size.
label-cifti-version = גרסת CIFTI
label-cifti-intent = Intent
label-cifti-matrix = מטריצה
cifti-matrix = { $rows } × { $columns }
# The maps (pages) of the file and what they are.
label-cifti-maps = מפות
cifti-maps-scalars = { $count ->
        [one] מפה סקלרית אחת
        [two] שתי מפות סקלריות
       *[other] { $count } מפות סקלריות
    }
cifti-maps-labels = { $count ->
        [one] מפת תוויות אחת
        [two] שתי מפות תוויות
       *[other] { $count } מפות תוויות
    }
cifti-maps-series = { $count ->
        [one] נקודת זמן אחת
        [two] שתי נקודות זמן
       *[other] { $count } נקודות זמן
    }
cifti-maps-parcels = { $count ->
        [one] מקטע אחד
        [two] שני מקטעים
       *[other] { $count } מקטעים
    }
cifti-maps-other = { $count }
# machine-drafted, needs native review
# A dense connectivity file (.dconn.nii): every brain place's connectivity to every other.
# $seeds: how many places can be picked; $size: the bytes read when one is picked (e.g. "357 KB").
label-cifti-dense = קישוריות
cifti-dense = נקודות זרע: { $seeds }, { $size } נקראים לכל נקודה
# A time series' start and step; $unit is e.g. "second".
label-cifti-series = סדרה
cifti-series = מ־{ $start }, כל { $step } ({ $unit })
label-cifti-models = מודלים של המוח
label-cifti-parcels = מקטעים
label-cifti-structures = מבנים
label-cifti-voxels = ווקסלים תת־קליפתיים
# A surface the file's values lie on: the brain structure (e.g. "CORTEX_LEFT"), its vertex
# count, and the surface file found beside the file for it, or that none was found.
label-cifti-surface = משטח
cifti-surface-found = { $structure }: { $vertices ->
        [one] קודקוד אחד
        [two] שני קודקודים
       *[other] { $vertices } קודקודים
    }, { $file }
cifti-surface-missing = { $structure }: נדרש משטח עם { $vertices ->
        [one] קודקוד אחד
        [two] שני קודקודים
       *[other] { $vertices } קודקודים
    } (אין ליד הקובץ)
# How the file is shown: on the surfaces found beside it, or as its subcortical voxels.
label-cifti-shown = מוצג כ־
cifti-shown-surfaces = משטחים צבועים
cifti-shown-voxels = נפח הווקסלים התת־קליפתיים
# machine-drafted, needs native review
# Shown with a picked seed's row: click a surface vertex, or move the crosshair in MPR.
cifti-shown-connectivity = קישוריות של נקודת הזרע שנבחרה
# Maps left out of the voxel volume, which would be too large with them.
label-cifti-left-out = מפות שהושמטו
# Tractograms: format, number of streamlines (fibre tracts) and points.
label-tract-format = פורמט
label-tract-streamlines = קווי זרימה
# A large tractogram draws only some of its streamlines.
tract-streamlines-shown = { $count } ({ $shown } מוצגים)
label-tract-points = נקודות
label-tract-mean-length = אורך ממוצע
# A length in millimetres.
tract-mm = { $value } מ״מ
# Values per point and per streamline the file stores, by name.
label-tract-scalars = סקלרים לכל נקודה
label-tract-properties = מאפיינים לכל קו זרימה
# TrackVis: the voxel grid the tracts were made in, its voxel size and voxel order (e.g. "LAS").
label-tract-grid = רשת
label-tract-voxel-size = גודל ווקסל
label-tract-voxel-order = סדר ווקסלים
# How the points were brought into scanner (RAS) millimetres.
label-tract-space = קואורדינטות
tract-space-vox-to-ras = ממטריצת ווקסל ל־RAS
tract-space-voxel-order = מסדר הווקסלים (אין מטריצת ווקסל ל־RAS)
tract-space-scanner = מרחב הסורק
# MRtrix: the tracking step size.
label-tract-step = גודל צעד

## File headers (DPX, Cineon, XWD, MIFF, TIM, WSQ, JBIG)

# machine-drafted, needs native review
# What the file's own header records, for formats without Exif or XMP.
group-header = כותרת הקובץ
label-header-created = נוצר
# The program or person that wrote the file.
label-header-creator = יוצר
label-header-copyright = זכויות יוצרים
label-header-project = פרויקט
label-header-comment = הערה
# The scanner, camera or other device the image came from.
label-header-device = התקן
# SMPTE time code of the frame: "01:00:00:12".
label-header-timecode = קוד זמן
# A film frame's edge code (KeyKode).
label-header-edge-code = קוד קצה
label-header-frame-rate = קצב פריימים
# Where the frame sits in its sequence.
label-header-frame = פריים
# Film stock and slate.
label-header-film = פילם
# How code values relate to light: printing density, linear, BT.709…
label-header-transfer = פונקציית העברה
# The colour primaries the file states.
label-header-colorimetric = קולורימטריה
# The X window a screen dump was taken of.
label-header-window = חלון
label-header-resolution = רזולוציה
label-header-compression = דחיסה
# machine-drafted, needs native review
# JPEG XS: the profile and the level the codestream states ("Main 422.10"; "4k-1, sublevel 6 bpp").
label-header-profile = פרופיל
# machine-drafted, needs native review
label-header-level = רמה
# machine-drafted, needs native review
# JPEG XS: how many wavelet levels, horizontal and vertical ("5 horizontal, 1 vertical").
label-header-decomposition = רמות גליות
# machine-drafted, needs native review
# JPEG XS: the colour transform ("None", "Reversible colour transform (RCT)", "Star-Tetrix, RGGB mosaic").
label-header-colour-transform = המרת צבע
# machine-drafted, needs native review
# Photoshop: the artboards, by name and size ("Phone (375 × 667), Tablet (768 × 1024)").
label-header-artboards = משטחי עבודה
# machine-drafted, needs native review
# Photoshop file without a merged image, shown from its layers: the features it leaves out,
# by Photoshop's names ("Color Lookup, Knockout").
label-header-not-drawn = לא מצויר
# machine-drafted, needs native review
# The same, for the features drawn only approximately ("Bevel & Emboss, Satin").
label-header-approximated = מצויר בקירוב

## Fonts (TTF, OTF, TTC/OTC, WOFF, WOFF2)

# machine-drafted, needs native review
group-font = גופן
label-font-family = משפחה
label-font-style = סגנון
# The face's place in a font collection: "2 of 4".
label-font-face = פנים
font-face-of = { $index } מתוך { $count }
label-font-version = גרסה
label-font-designer = מעצב
# The font's maker (the name table's manufacturer).
label-font-manufacturer = יצרן
label-font-glyphs = גליפים
label-font-outlines = קווי מתאר
font-outlines-truetype = TrueType (ריבועיים)
font-outlines-cff = PostScript (CFF, קוביים)
font-outlines-cff2 = PostScript (CFF2, קוביים)
font-outlines-bitmap = מפות סיביות בלבד
font-outlines-none = אין
# What the font's licence flags (OS/2 fsType) allow documents that embed it to do.
label-font-embedding = הטמעה
font-embedding-installable = מותר להטמיע ולהתקין.
font-embedding-restricted = אסור להטמיע.
font-embedding-preview-print = מותר להטמיע לצפייה ולהדפסה בלבד.
font-embedding-editable = מותר להטמיע במסמכים שעורכים.
font-embedding-no-subsetting = רק בשלמותו, לא כתת־קבוצה.
font-embedding-bitmap-only = רק כמפות סיביות.
# Variation axes of a variable font: "Weight 100–900 (400)".
label-font-axes = צירי וריאציה
font-axis = { $name } { $min }–{ $max } ({ $default })
label-font-instances = סגנונות בעלי שם
label-font-colour = גליפים צבעוניים
font-colour-colr0 = שכבות COLR
font-colour-colr1 = מעברי צבע COLR v1
font-colour-sbix = מפות סיביות (sbix)
font-colour-cbdt = מפות סיביות (CBDT)
font-colour-svg = SVG
label-font-weight = משקל
label-font-units-per-em = יחידות ל־em
label-font-tables = טבלאות
label-font-licence = רישיון
label-font-licence-url = כתובת הרישיון
label-font-copyright = זכויות יוצרים
label-font-trademark = סימן מסחרי

## Office documents (Word, Excel, PowerPoint, iWork, OpenDocument)

# machine-drafted, needs native review
group-document = מסמך
label-document-kind = סוג
document-kind-word = מסמך Word
document-kind-excel = חוברת עבודה של Excel
document-kind-powerpoint = מצגת PowerPoint
document-kind-pages = מסמך Pages
document-kind-numbers = גיליון Numbers
document-kind-keynote = מצגת Keynote
document-kind-odf-text = טקסט OpenDocument
document-kind-odf-spreadsheet = גיליון אלקטרוני OpenDocument
document-kind-odf-presentation = מצגת OpenDocument
document-kind-odf-graphics = ציור OpenDocument
document-kind-odf-other = קובץ OpenDocument
# machine-drafted, needs native review
document-kind-playlist = רשימת השמעה
# machine-drafted, needs native review
label-playlist-items = פריטים
# Appended to the kind: "Word document, template, with macros".
document-template = תבנית
document-macros = עם פקודות מאקרו
document-legacy = פורמט Office 97–2003
label-document-preview = תצוגה מקדימה
# The image format of the preview stored in the file, e.g. "JPEG image".
document-preview-image = תמונת { $format }
document-preview-none = אין בקובץ זה
label-document-subject = נושא
label-document-author = מחבר
label-document-description = תיאור
label-document-category = קטגוריה
label-document-last-saved-by = נשמר לאחרונה על־ידי
label-document-created = נוצר
label-document-modified = שונה
label-document-printed = הודפס לאחרונה
label-document-pages = עמודים
label-document-slides = שקופיות
label-document-hidden-slides = שקופיות מוסתרות
label-document-notes = דפי הערות
label-document-sheets = גיליונות
label-document-words = מילים
label-document-characters = תווים
label-document-paragraphs = פסקאות
label-document-lines = שורות
label-document-editing-time = זמן עריכה
document-minutes = { $minutes } דק׳
label-document-revision = גרסה
label-document-application = נשמר באמצעות
label-document-company = חברה
label-document-template = תבנית

## Books (EPUB)

# machine-drafted, needs native review
group-book = ספר
label-book-authors = מחברים
label-book-publisher = הוצאה לאור
label-book-date = פורסם
label-book-language = שפה
label-book-pages = עמודים
# Entries in the book's table of contents.
label-book-contents = פריטים בתוכן העניינים
label-book-layout = פריסה
book-layout-reflowable = זורמת
book-layout-fixed = פריסה קבועה
book-right-to-left = מימין לשמאל
label-book-version = פורמט
book-version = EPUB { $version }
label-book-fonts = גופנים מוטמעים
book-fonts-obfuscated = { $count } ({ $obfuscated } מוסתרים)
label-book-subjects = נושאים
label-book-identifier = מזהה
label-book-rights = זכויות

unit-symbol-mm = מ״מ
unit-symbol-cm = ס״מ
unit-symbol-m = מ׳
unit-symbol-km = ק״מ
unit-symbol-mm2 = ממ״ר
unit-symbol-cm2 = סמ״ר
unit-symbol-m2 = מ״ר
unit-symbol-km2 = קמ״ר
unit-symbol-ha = הקטר
unit-symbol-ft = רגל
unit-symbol-ft2 = רגל רבוע
unit-symbol-mi = מייל
unit-symbol-mi2 = מייל רבוע
unit-symbol-ac = אקר

# machine-drafted, needs native review
volume-orient-parrec = זווית והיסט של Philips
volume-orient-ecat = גודלי ווקסל והיסטים של ECAT (לא אמין)
label-volume-technique = טכניקה
label-volume-version = גרסת תבנית
label-volume-isotope = איזוטופ
label-volume-tracer = סמן
label-volume-units = יחידות

# Text, tables and subtitles.
label-header-encoding = ערכת תווים
label-header-lines = שורות
label-header-line-ends = סיומי שורות
label-header-words = מילים
label-header-pages = עמודים
label-header-pictures = תמונות
label-header-pictures-not-shown = תמונות שלא מוצגות
label-header-tables = טבלאות
label-header-rows = שורות בטבלה
label-header-columns = עמודות
label-header-delimiter = מפריד
label-header-column-names = שמות העמודות
label-header-cues = כתוביות
label-header-cue-span = כתוביות מ– עד
