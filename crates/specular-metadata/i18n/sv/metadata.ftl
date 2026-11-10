# machine-drafted, needs native review
# Specular metadatapanel — svenska. Se i18n/en/metadata.ftl för konventionerna. Tal och datum
# kommer redan formaterade av ICU4X för språket.

## Gruppnamn i sammanfattningen

group-camera = Kamera
group-exposure = Exponering
group-lens = Objektiv
group-date = Datum och tid
group-location = Plats
# machine-drafted, needs native review
# A DJI drone photo: the gimbal's and the aircraft's attitude, the height above take-off.
group-drone = Drönare
label-gimbal = Gimbal
label-flight = Farkost
label-relative-altitude = Över startpunkten
drone-attitude = gir { $yaw }, tipp { $pitch }, roll { $roll }
# A DJI thermal camera's radiometric JPEG: its photo opens, its temperatures can't.
label-thermal-data = Temperaturer
drone-thermal-refused = Visas inte: DJI:s SDK-licens tillåter det inte
drone-thermal-refused-detail = För att läsa DJI-temperaturer krävs DJI:s Thermal SDK, vars licens bara tillåter program som levereras med SDK:t att använda det. Bilden och dess metadata visas.
group-image = Bild
group-file = Fil
group-rights = Rättigheter och beskrivning
group-credentials = Uppgifter om innehåll
# machine-drafted, needs native review
group-medical = Medicinsk bild
# machine-drafted, needs native review
group-inspection = Komponent och inspektion
group-nde-image = Inspektionsbild (DICONDE)
# machine-drafted, needs native review
# Industrial CT volumes (Zeiss Xradia, Amira, Bruker SkyScan, Nikon X-Tek, VGI, raw volumes).
group-industrial-image = Inspektionsbild
group-science = Vetenskaplig bild
group-em = Elektronmikroskopi
group-spm = Svepprobsmikroskopi
group-slide = Objektglas
group-microscopy = Mikroskopi
# X-ray diffraction detector frames (CBF, EDF, SMV, Bruker, MarCCD, R-AXIS).
group-diffraction = Diffraktion
group-surface = Ytopografi
group-ebsd = Bakåtspridd elektrondiffraktion
group-seismic = Seismiska data
# machine-drafted, needs native review
group-asdf = ASDF
# machine-drafted, needs native review
# CERN ROOT files (particle physics histograms and graphs).
group-root = ROOT
# machine-drafted, needs native review
# CASA images (radio interferometry: the .image, .residual, .model directories tclean writes).
group-casa = CASA-bild
# machine-drafted, needs native review
# GIFTI files: brain surfaces and the values on their vertices (Human Connectome Project
# and FreeSurfer exports).
group-gifti = GIFTI
# CIFTI-2 files: maps over cortical surfaces and subcortical voxels ("grayordinates").
group-cifti = CIFTI
# White-matter fibre tracts (TrackVis, MRtrix).
group-tractography = Traktografi
# machine-drafted, needs native review
group-annotations = Annoteringar
# Imaging mass spectrometry (imzML): a mass spectrum per pixel.
group-mass-spec = Masspektrometrisk avbildning
# machine-drafted, needs native review
group-thermal = Värmebild
# machine-drafted, needs native review
# Flow cytometry list-mode data (FCS): events of a cell sorter or analyser.
group-cytometry = Flödescytometri
# machine-drafted, needs native review
group-spectroscopy = Spektroskopi
# machine-drafted, needs native review
# Unstructured meshes of ocean, coastal and climate models (UGRID, ICON, MPAS in NetCDF).
group-ugrid = Ostrukturerat nät
group-drawing = Ritning
# machine-drafted, needs native review
group-volume = Volym

## Etiketter i sammanfattningen

label-camera = Kamera
label-software = Programvara
label-application = Program
label-shutter = Slutartid
label-aperture = Bländare
label-iso = ISO
label-focal-length = Brännvidd
label-exposure-bias = Exponeringskomp.
label-program = Läge
label-metering = Mätning
label-white-balance = Vitbalans
label-flash = Blixt
label-lens = Objektiv
label-lens-make = Objektivmärke
label-lens-range = Objektivomfång
# machine-drafted, needs native review
label-focus-mode = Fokusläge
label-stabilisation = Bildstabilisering
label-taken = Tagen
label-digitised = Digitaliserad
label-edited = Senast redigerad
label-place = Plats
label-coordinates = Koordinater
label-altitude = Höjd
label-direction = Riktning
label-accuracy = Noggrannhet
label-gps-time = GPS-tid
label-dimensions = Mått
label-encoding = Kodning
# machine-drafted, needs native review
label-motion = Rörelse
motion-kind-motion-photo = Rörelsefoto
motion-kind-micro-video = Rörelsefoto (MicroVideo)
motion-kind-samsung = Samsung rörelsefoto
motion-kind-live-photo = Live Photo
motion-duration = { $seconds } s
label-colour-profile = Färgprofil
label-orientation = Orientering
label-file-name = Namn
label-folder = Mapp
label-file-size = Storlek
label-format = Format
label-modified = Ändrad
label-title = Titel
label-description = Beskrivning
label-creator = Upphovsperson
label-copyright = Upphovsrätt
label-keywords = Nyckelord
label-rating = Betyg
label-colour-label = Etikett
label-credentials = Uppgifter om innehåll

## Gruppnamn under "Alla fält" (taggnamnen förblir som i standarderna)

fields-ifd0 = IFD0 (huvudbild)
fields-exif = Exif
fields-gps = GPS
fields-interop = Interoperabilitet
fields-ifd1 = IFD1 (miniatyr)
fields-icc = ICC-profilhuvud
# $namespace: från xmp-namespace; $prefix: t.ex. "dc"
fields-xmp = XMP: { $namespace } ({ $prefix })
xmp-namespace = { $prefix ->
    [dc] Dublin Core
    [xmp] Grundläggande
    [xmpRights] Rättigheter
    [xmpMM] Mediehantering
    [xmpDM] Dynamiska medier
    [tiff] TIFF
    [exif] Exif
    [exifEX] Exif 2.3+
    [aux] Kameratillbehör
    [photoshop] Photoshop
    [crs] Camera Raw-inställningar
    [lr] Lightroom
    [Iptc4xmpCore] IPTC Core
    [Iptc4xmpExt] IPTC Extension
    [plus] PLUS-licensiering
    [GPano] Fotosfär
    [GCamera] Google-kamera
    [hdrgm] HDR-förstärkningskarta
    [mwg-rs] Regioner
    [pdf] PDF
    [stEvt] Resurshändelser
    [stRef] Resursreferenser
   *[other] Övrigt
}
# $number: "0xA500"; $context: "Exif", "GPS", …
unknown-tag = Okänd tagg { $number } ({ $context })
blob-bytes = { $num ->
    [one] ({ $num } byte)
   *[other] ({ $num } byte)
}
maker-note-bytes = ({ $num ->
        [one] { $num } byte
       *[other] { $num } byte
    }, tillverkaranteckning, inte avkodad)
more-values = { $shown }, … ({ $total ->
        [one] { $total } värde
       *[other] { $total } värden
    })

## Allmänna sammanfogningar

# Skiljer delarna i en lista som byggs i kod (blixtbeskrivning, områden).
list-separator = {", "}
# Skiljer slutartid · bländare · ISO · brännvidd i enradssammanfattningen.
one-line-separator = {" · "}

## Exponering

exposure-time-fraction = 1/{ $denominator } s
exposure-time-seconds = { $seconds } s
aperture-value = f/{ $f }
iso-value = ISO { $iso }
focal-length-value = { $mm } mm
focal-length-with-equiv = { $mm } mm ({ $equiv } mm motsv.)
focal-length-equiv-only = { $equiv } mm motsv.
# $ev är beloppet; tecknet kommer från väljaren.
exposure-bias-value = { $sign ->
    [plus] +{ $ev } EV
    [minus] −{ $ev } EV
   *[zero] 0 EV
}
exposure-program = { $value ->
    [1] Manuell
    [2] Program (auto)
    [3] Bländarprioritet
    [4] Slutarprioritet
    [5] Kreativ (skärpedjup)
    [6] Action (kort slutartid)
    [7] Porträtt
    [8] Landskap
   *[other] Okänt
}
metering-mode = { $value ->
    [1] Genomsnitt
    [2] Centrumvägd genomsnittsmätning
    [3] Spot
    [4] Multispot
    [5] Matris (utvärderande)
    [6] Partiell
   *[other] Annan
}
# machine-drafted, needs native review
# Canon maker note CameraSettings FocusMode; 256–258 are the live-view variants of 0–2.
canon-focus-mode = { $value ->
    [0] One-Shot AF
    [1] AI Servo AF
    [2] AI Focus AF
    [3] Manuell fokus
    [4] Enkel
    [5] Kontinuerlig
    [6] Manuell fokus
    [16] Pan focus
    [256] One-Shot AF (Live View)
    [257] AI Servo AF (Live View)
    [258] AI Focus AF (Live View)
    [512] Movie snap-fokus
    [519] Movie Servo AF
   *[other] Annan
}
# Canon maker note CameraSettings ImageStabilization; 256–260 repeat 0–4.
canon-stabilisation = { $value ->
    [0] Av
    [1] På
    [2] Endast vid tagning
    [3] Panorering
    [4] Dynamisk
    [256] Av
    [257] På
    [258] Endast vid tagning
    [259] Panorering
    [260] Dynamisk
   *[other] Annan
}
# machine-drafted, needs native review
panasonic-focus-mode = { $value ->
    [1] Auto
    [2] Manuell fokus
    [4] Auto (fokusknapp)
    [5] Auto (kontinuerlig)
    [6] AF-S
    [7] AF-C
    [8] AF-F
   *[other] Annat
}
# machine-drafted, needs native review
panasonic-stabilisation = { $value ->
    [2] På (optisk)
    [3] Av
    [4] På (läge 2)
    [5] På (optisk, panorering)
    [6] På (endast hus)
    [7] På (endast hus, panorering)
    [9] Dual IS
    [10] Dual IS (panorering)
    [11] Dual IS 2
    [12] Dual IS 2 (panorering)
   *[other] Annat
}
label-comment = Kommentar
fields-tags = { $kind }-taggar
group-picture = Bildinställningar
fields-fujifilm = Fujifilm-tillverkardata
label-film-simulation = Filmsimulering
label-dynamic-range = Dynamiskt omfång
label-highlight-tone = Högdagrar
label-shadow-tone = Skuggor
label-colour-setting = Färg
label-sharpness = Skärpa
label-noise-reduction = Brusreducering
label-clarity = Klarhet
label-grain-effect = Korneffekt
label-colour-chrome = Färgkrom
label-colour-chrome-blue = Färgkrom blå
label-shutter-type = Slutartyp
label-drive-mode = Matningsläge
fuji-step = { $sign ->
    [plus] +{ $n }
    [minus] −{ $n }
   *[zero] 0
}
fuji-film-mode = { $value ->
    [0] Provia (standard)
    [256] Studioporträtt
    [272] Studioporträtt (mer mättnad)
    [288] Astia (mjuk)
    [304] Studioporträtt (skarpare)
    [512] Velvia (livfull)
    [768] Studioporträtt Ex
    [1024] Velvia
    [1280] Pro Neg. Std
    [1281] Pro Neg. Hi
    [1536] Classic Chrome
    [1792] Eterna
    [2048] Classic Neg.
    [2304] Eterna Bleach Bypass
    [2560] Nostalgic Neg.
    [2816] Reala Ace
   *[other] Annat
}
fuji-monochrome = { $value ->
    [768] Monokrom
    [769] Monokrom, rödfilter
    [770] Monokrom, gulfilter
    [771] Monokrom, grönfilter
    [784] Sepia
    [1280] Acros
    [1281] Acros, rödfilter
    [1282] Acros, gulfilter
    [1283] Acros, grönfilter
   *[other] Monokrom
}
fuji-white-balance = { $value ->
    [0] Auto
    [1] Auto (vitprioritet)
    [2] Auto (stämningsprioritet)
    [256] Dagsljus
    [512] Molnigt
    [768] Dagsljuslysrör
    [769] Dagsvitt lysrör
    [770] Vitt lysrör
    [771] Varmvitt lysrör
    [772] Varmvitt lysrör för bostäder
    [1024] Glödlampa
    [1280] Blixt
    [1536] Under vatten
    [3840] Anpassad 1
    [3841] Anpassad 2
    [3842] Anpassad 3
    [3843] Anpassad 4
    [3844] Anpassad 5
    [4080] Färgtemperatur
   *[other] Annat
}
fuji-white-balance-kelvin = { $kelvin } K
fuji-dynamic-range = DR{ $percent }
fuji-dynamic-range-auto = Auto (DR{ $percent })
fuji-dynamic-range-setting = { $value ->
    [0] Auto
    [256] Standard (DR100)
    [512] Brett 1 (DR230)
    [513] Brett 2 (DR400)
   *[other] Annat
}
fuji-effect = { $value ->
    [0] Av
    [32] Svag
    [64] Stark
   *[other] Annat
}
fuji-grain-size = { $value ->
    [16] liten
    [32] stor
   *[other] annat
}
fuji-grain = { $strength }, { $size }
fuji-shutter-type = { $value ->
    [0] Mekanisk
    [1] Elektronisk
    [2] Elektronisk (långa exponeringar)
    [3] Elektronisk första ridå
   *[other] Annat
}
fuji-drive-mode = { $value ->
    [0] Enstaka bild
    [1] Serie, låg hastighet
    [2] Serie, hög hastighet
   *[other] Annat
}
fuji-focus-mode = { $value ->
    [0] Manuell
    [1] AF-S
    [2] AF-C
    [100] Auto
    [101] Manuell
   *[other] Annat
}
fuji-stabilisation = { $kind } ({ $mode })
fuji-stabilisation-kind = { $value ->
    [0] Ingen
    [1] Optisk
    [2] Sensorförskjutning
    [3] Objektiv
    [258] Sensorförskjutning och digital
    [512] Digital
   *[other] Annat
}
fuji-stabilisation-mode = { $value ->
    [0] Av
    [1] på, kontinuerlig
    [2] på, vid exponering
   *[other] Annat
}
light-source = { $value ->
    [1] Dagsljus
    [2] Lysrör
    [3] Glödlampa
    [4] Blixt
    [9] Klart väder
    [10] Molnigt
    [11] Skugga
    [12] Dagsljuslysrör
    [13] Dagvitt lysrör
    [14] Kallvitt lysrör
    [15] Vitt lysrör
    [16] Varmvitt lysrör
    [17] Standardljus A
    [18] Standardljus B
    [19] Standardljus C
    [20] D55
    [21] D65
    [22] D75
    [23] D50
    [24] Studioglödljus
   *[other] Annan
}
white-balance-auto = Auto
white-balance-manual = Manuell
white-balance-manual-with-light = Manuell ({ $light })
# Blixtbeskrivningen byggs av dessa delar, sammanfogade med list-separator.
flash-none = Ingen blixtfunktion
flash-fired = Blixten utlöstes
flash-not-fired = Blixten utlöstes inte
flash-forced-on = tvingad på
flash-forced-off = tvingad av
flash-auto = autoläge
flash-return-not-detected = returljus upptäcktes inte
flash-return-detected = returljus upptäcktes
flash-red-eye = röda ögon-reducering

## Objektiv

lens-focal-single = { $mm } mm
lens-focal-range = { $min }–{ $max } mm
lens-aperture-single = f/{ $f }
lens-aperture-range = f/{ $min }–{ $max }
lens-range = { $focal } { $aperture }

## Datum och tid

# $date: "tisdag 14 maj 2024"; $time: "18:32"
# $weekday: "tisdag" (ICU); $date: "14 maj 2024" (ICU)
date-with-weekday = { $weekday } { $date }
date-time = { $date } { $time }
date-time-zone = { $datetime } ({ $zone })
zone-utc = UTC
# $offset: "+1", "+5:30", "−3"
zone-offset = UTC{ $offset }
gps-date-time = { $date } { $time } UTC
detail-zone-from-gps = Tidszonen härledd från GPS-klockan

## Plats

# $lat och $lon är utan tecken; halvklotet kommer från väljaren.
coordinates = { $lat }° { $latRef ->
    [south] S
   *[north] N
}, { $lon }° { $lonRef ->
    [west] V
   *[east] Ö
}
altitude = { $ref ->
    [above] { $m } m över havet
    [below] { $m } m under havet
   *[sea] Vid havsnivån
}
compass-point = { $point ->
    [n] norr
    [nne] nordnordost
    [ne] nordost
    [ene] ostnordost
    [e] öster
    [ese] ostsydost
    [se] sydost
    [sse] sydsydost
    [s] söder
    [ssw] sydsydväst
    [sw] sydväst
    [wsw] västsydväst
    [w] väster
    [wnw] västnordväst
    [nw] nordväst
   *[nnw] nordnordväst
}
# $point: från compass-point; $degrees: "287"
facing = Riktad mot { $point } ({ $degrees }°)
detail-true-north = Relativt geografiska nordpolen
detail-magnetic-north = Relativt magnetiska nordpolen
accuracy = ±{ $m } m
distance-km = { $km } km
place = { $city }, { $country }
place-city-only = { $city }
# Mer än 30 km från den matchade orten.
place-near = nära { $place }
place-detail = { $areas } · { $distance } från { $city }
place-detail-distance = { $distance } från { $city }

## Bild

dimensions = { $width } × { $height }
dimensions-megapixels = { $width } × { $height } ({ $mp } MP)
encoding-bit-depth = { $bits } bitar
# $format: "8 bitar YCbCr 4:2:0"
encoding-progressive = { $format }, progressiv
encoding-baseline = { $format }, baseline
encoding-hierarchical = { $format }, hierarkisk
# machine-drafted, needs native review
encoding-alpha = { $format } { $alpha ->
    [premultiplied] med förmultiplicerad alfa
    [transparency] med genomskinlighet
   *[alpha] med alfa
}
# [Lab], [Palette] and the variants after them: machine-drafted, needs native review
colour-model = { $model ->
    [YCbCr] YCbCr
    [RGB] RGB
    [Greyscale] Gråskala
    [CMYK] CMYK
    [YCCK] YCCK
    [Lab] L*a*b*
    [Palette] Indexerade färger
    [Multichannel] Flerkanals
    [Duotone] Duoton
    [YCgCo] YCgCo
    [XYB] XYB
    [YC] Luminans/krominans (YC)
    [Bayer] Bayer-raw
    [X-Trans] X-Trans-raw
    [CFA] Raw med färgfiltermatris
    [LinearRaw] Linjär raw
    [RGBE] RGBE (delad exponent)
    [XYZE] XYZE (delad exponent)
    [R] En kanal (R)
    [RG] Två kanaler (RG)
   *[other] { $model }
}
adobe-rgb-from-exif = Adobe RGB (från EXIF-tagg; ingen inbäddad profil)
orientation = { $value ->
    [1] Upprätt (ingen rotation behövs)
    [2] Speglad vågrätt (tillämpat)
    [3] Roterad 180° (tillämpat)
    [4] Speglad lodrätt (tillämpat)
    [5] Speglad vågrätt och roterad 90° moturs (tillämpat)
    [6] Roterad 90° medurs (tillämpat)
    [7] Speglad vågrätt och roterad 90° medurs (tillämpat)
    [8] Roterad 90° moturs (tillämpat)
   *[other] Okänd ({ $value })
}
# Kort form för varningar.
orientation-short = { $value ->
    [1] Upprätt
    [2] Speglad vågrätt
    [3] Roterad 180°
    [4] Speglad lodrätt
    [5] Speglad vågrätt och roterad 90° moturs
    [6] Roterad 90° medurs
    [7] Speglad vågrätt och roterad 90° medurs
    [8] Roterad 90° moturs
   *[other] värde { $value }
}

## Fil

file-size-bytes = { $num ->
    [one] { $num } byte
   *[other] { $num } byte
}
# Decimala enheter (1 KB = 1000 byte).
file-size-unit = { $num } { $unit ->
    [kb] KB
    [mb] MB
    [gb] GB
    [tb] TB
   *[pb] PB
}

## Rättigheter

rating-rejected = Förkastad
# $stars: "★★★★☆"
rating-stars = { $stars } ({ $n } av 5)

## Innehållsuppgifter

c2pa-present = Finns (inte verifierat)
c2pa-detail = Filen innehåller ett C2PA-manifest; dess signatur har inte kontrollerats.

## Värden i ICC-huvudet

icc-class = { $class ->
    [scnr] Inmatningsenhet
    [mntr] Bildskärmsenhet
    [prtr] Utmatningsenhet
    [link] Enhetslänk
    [spac] Färgrymd
    [abst] Abstrakt
    [nmcl] Namngiven färg
   *[other] Okänd
}
icc-intent = { $intent ->
    [0] Perceptuell
    [1] Relativ kolorimetrisk
    [2] Mättnad
    [3] Absolut kolorimetrisk
   *[other] Okänd
}
icc-platform = { $platform ->
    [APPL] Apple
    [MSFT] Microsoft
    [SGI] Silicon Graphics
    [SUNW] Sun Microsystems
    [none] Inte angiven
   *[other] Okänd
}
icc-coded = { $name } ({ $code })

# machine-drafted, needs native review
label-modality = Modalitet
label-body-part = Kroppsdel
label-study = Undersökning
label-series = Serie
label-slice-thickness = Snittjocklek
label-pixel-spacing = Pixelavstånd
label-imager-pixel-spacing = Pixelavstånd vid detektorn
label-frames = Bildrutor
label-transfer-syntax = Överföringssyntax
label-document = Dokument
label-document-type = Dokumenttyp
medical-spacing = { $row } × { $column } mm
medical-mm = { $mm } mm
medical-lossy = { $name } (förstörande komprimering)
detail-pixel-spacing = Avstånd mellan pixelcentrum i patienten (rader × kolumner).
# machine-drafted, needs native review
# Pixel spacing of an industrial inspection image (DICONDE, CT of an object).
detail-pixel-spacing-object = Avstånd mellan pixelcentrum i objektet (rader × kolumner).
detail-imager-pixel-spacing = Mätt vid detektorn, inte korrigerat för förstoring: avstånd i patienten är oftast mindre.

## Industriell inspektion (DICONDE)

# machine-drafted, needs native review
label-nde-component-shape = Komponentform
label-nde-component-manufacturer = Komponenttillverkare
label-nde-material = Material
label-nde-thickness = Materialtjocklek
label-nde-pipe-diameter = Rördiameter
label-nde-source = Källa
label-nde-environment = Miljö
label-nde-iqi = Bildkvalitetsindikator
label-nde-indications = Indikationer
label-nde-standard = Standard

## Forskningsvolymer (NIfTI, Analyze, NRRD, MetaImage)

# machine-drafted, needs native review
label-voxel-size = Voxelstorlek
label-data-type = Datatyp
label-time-points = Tidpunkter
label-orientation-source = Orientering från
label-intent = Värden
label-compression = Komprimering
volume-voxel-size = { $x } × { $y } × { $z } mm
volume-orient-sform = NIfTI sform
volume-orient-qform = NIfTI qform
volume-orient-analyze = Analyze-huvud (opålitligt)
volume-orient-space = NRRD-rumsriktningar
volume-orient-transform = MetaImage-transformation
volume-orient-spacings = Endast voxelavstånd
volume-orient-none = Ej angiven
volume-orient-vox2ras = FreeSurfer vox2ras
volume-orient-mgh-default = FreeSurfer-standard (ej tillförlitlig)
volume-orient-minc = MINC-riktningscosinusar
volume-orient-object = Objektkoordinater (ingen patientorientering)
volume-content-projections = Projektioner (rå skanning)
volume-content-reconstruction = Rekonstruerad volym
volume-coords-rectilinear = Rätlinjig (ojämnt avstånd, visas jämnt)
# Scan parameters and tags a FreeSurfer MGH file keeps after its voxels.
label-volume-tr = Repetitionstid
label-volume-te = Ekotid
label-volume-ti = Inversionstid
label-volume-flip = Flipvinkel
label-volume-fov = Synfält
label-volume-field = Fältstyrka
label-volume-pedir = Faskodning
label-volume-cmdline = Kommandorad
label-volume-xform = Talairach-transformation
label-volume-content = Innehåll
label-volume-projections = Projektioner
label-volume-angles = Rotationsomfång
label-volume-angle-step = Rotationssteg
label-volume-kv = Rörspänning
label-volume-current = Rörström
label-volume-power = Källeffekt
label-volume-exposure = Exponering
label-volume-sod = Källa till objekt
label-volume-odd = Objekt till detektor
label-volume-sdd = Källa till detektor
label-volume-magnification = Optisk förstoring
label-volume-binning = Binning
label-volume-objective = Objektiv
label-volume-filter = Källfilter
label-volume-camera = Detektor
label-volume-instrument = Instrument
label-volume-software = Programvara
label-volume-date = Tagen
label-volume-ring = Korrigering av ringartefakter
label-volume-smoothing = Utjämning
label-volume-coords = Rutnät
# machine-drafted, needs native review
# An eye scan (OCT): which eye.
label-volume-eye = Öga
volume-eye-right = Höger (OD)
volume-eye-left = Vänster (OS)
detail-volume-unreliable = Filen anger inte vart dess axlar pekar i patienten: vänster och höger kan vara omkastade, så inga orienteringsetiketter visas.
# machine-drafted, needs native review
# An industrial CT volume: its axes are the scanner's, there is no patient.
detail-volume-object = Axlarna är skannerns, inte en patients, så inga orienteringsetiketter visas.

## Vetenskapliga bilder (FITS, XISF)

label-object = Objekt
label-telescope = Teleskop
label-instrument = Instrument
label-filter = Filter
label-observed = Observerad
label-exposure-time = Exponering
label-sample = Sampel
label-unit = Enhet
label-hdu = Tillägg
label-centre = Centrum
label-centre-deg = Centrum (grader)
label-frame = Koordinatsystem
# machine-drafted, needs native review
science-frame-galactic = Galaktisk
science-frame-ecliptic = Ekliptisk
label-projection = Projektion
label-pixel-scale = Pixelskala
science-seconds = { $seconds } s
science-arcsec-px = { $arcsec }″ per pixel
label-distortion = Distorsion
label-wcs = Himmelskoordinater
science-wcs-unsupported = Stöds inte: { $reason }
# machine-drafted, needs native review
science-wcs-approximate = Ungefärliga (endast pekning, utan distorsion)
label-observer = Observatör
label-bayer = Bayermönster
label-capture-start = Inspelningsstart (UTC)
label-capture-end = Inspelningsslut (UTC)
label-frame-rate = Bildfrekvens
science-fps = { $fps } bilder/s
science-frames-cut = { $frames } av { $declared }
# The HDF5 dataset or NetCDF variable shown.
label-dataset = Datamängd
label-pixel-size = Pixelstorlek
# "0.25 nm per pixel"; $size may be "0.25 × 0.5".
science-pixel-size = { $size } { $unit } per pixel
# Microscopy (DigitalMicrograph, MRC, Gwyddion, Igor).
label-microscope = Mikroskop
label-voltage = Accelerationsspänning
label-magnification = Förstoring
label-imaging-mode = Läge
label-detector = Detektor
label-acquired = Registrerad
label-field-of-view = Synfält
science-kilovolts = { $kv } kV
science-magnification = { $value }×
label-vendor = Tillverkare
label-scanner = Skanner
label-objective = Objektiv
label-pyramid-levels = Nivåer
# machine-drafted, needs native review
label-emissivity = Emissivitet
label-object-distance = Objektavstånd
label-reflected-temperature = Reflekterad temperatur
label-atmospheric-temperature = Atmosfärstemperatur
label-humidity = Relativ luftfuktighet
label-raw-range = Råintervall
label-temperature-range = Temperaturintervall
thermal-temperature = { $value } { $unit }
thermal-metres = { $value } m
thermal-percent = { PERCENT($value) }
thermal-range = { $low } – { $high }
slide-levels = { $count }: { $sizes }
# X-ray diffraction: the experiment geometry from a detector frame's header.
label-wavelength = Våglängd
# Ångström, e.g. "0.97625 Å".
xtal-angstrom = { $value } Å
label-photon-energy = Fotonenergi
xtal-kev = { $value } keV
# Sample to detector distance.
label-detector-distance = Detektoravstånd
xtal-mm = { $value } mm
# Where the direct beam meets the detector, in pixels from the image's top-left corner.
label-beam-centre = Strålcentrum
xtal-beam-centre = x { $x }, y { $y } px
# The detector arm's swing angle (2θ).
label-detector-swing = Detektor-2θ
xtal-degrees = { $value }°
# Goniometer angles (Greek letters are the field's usual names).
label-phi = Phi (φ)
label-omega = Omega (ω)
label-chi = Chi (χ)
label-kappa = Kappa (κ)
# The sample's rotation during the exposure: start angle plus width, e.g. "12.5° + 0.1°".
label-oscillation = Oscillation
xtal-oscillation = { $start }° + { $width }°
# $axis is the axis name from the file, e.g. "phi" or "omega".
xtal-oscillation-axis = { $start }° + { $width }° kring { $axis }
# The count at which a pixel saturates.
label-overload = Överflödesvärde
# The finest lattice spacing (d) recorded at the detector's edge and in its corners.
label-resolution = Upplösning
xtal-resolution = { $edge } Å vid kanten, { $corner } Å i hörnen
label-geometry = Geometri
xtal-curved = Böjd bildplatta: inga q, d eller upplösningsringar
xtal-incomplete = Ofullständigt huvud: inga q, d eller upplösningsringar
# All-fields group title: "FITS header".
fields-science-header = { $format }-huvud
# machine-drafted, needs native review
fields-asdf-tree = ASDF-träd
label-asdf-model = Datamodell
label-asdf-version = Filversion
label-asdf-standard = ASDF-standard
label-asdf-library = Skriven av
# machine-drafted, needs native review
# All-fields group title for a CERN ROOT file's list of keys (objects).
fields-root-keys = ROOT-nycklar
# CERN ROOT files: the ROOT version that wrote the file (e.g. "6.28/04"), its compression,
# the number of keys (stored objects), and the object shown: class (e.g. "TH2F"), its path in
# the file, title, number of entries and sum of weights.
label-root-version = ROOT-version
label-root-compression = Komprimering
label-root-keys = Nycklar
label-root-class = Klass
label-root-name = Namn
label-root-title = Titel
label-root-entries = Poster
label-root-sum-weights = Summa av vikter
# A graph's number of points.
label-root-points = Punkter
# $axis is "x", "y" or "z".
label-root-axis = Axel { $axis }
# $quantity is the axis title with its unit, e.g. "Energy [GeV]"; $min and $max the axis
# limits.
root-axis-fixed = { $quantity }: { $bins ->
        [one] { $bins } bin
       *[other] { $bins } binnar
    }, { $min } till { $max }
root-axis-variable = { $quantity }: { $bins ->
        [one] { $bins } bin med varierande bredd
       *[other] { $bins } binnar med varierande bredd
    }, { $min } till { $max }
label-root-mean-rms = Medelvärde och RMS { $axis }
root-mean-rms = { $mean } ± { $rms }
# Trees (event data) are listed only, with their entry counts.
label-root-trees = Träd
root-tree = { $name } ({ $entries ->
        [one] { $entries } post
       *[other] { $entries } poster
    })
# machine-drafted, needs native review
# CASA images: what the image is (from its directory name), its restoring beam (major and
# minor axis, position angle), the beams of its planes when they differ, the rest frequency,
# spectral reference frame (e.g. "LSRK") and velocity definition of the frequency labels,
# the default mask (e.g. "mask0") and the casacore storage manager with its byte order.
label-casa-type = Bildtyp
casa-type-image = Återställd bild
casa-type-residual = Residual
casa-type-model = Modell
casa-type-pb = Primärlob
casa-type-mask = Clean-mask
casa-type-psf = Punktspridningsfunktion
casa-type-sumwt = Viktsumma
casa-type-weight = Vikter
label-casa-beam = Återställningslob
casa-beam = { $major }″ × { $minor }″, PV { $pa }°
label-casa-plane-beams = Lober per plan
casa-plane-beams = { $count ->
        [one] { $count } lob
       *[other] { $count } lober
    }, storaxel { $min }″ till { $max }″
label-casa-rest-frequency = Vilofrekvens
casa-ghz = { $ghz } GHz
label-casa-spectral-frame = Spektralt referenssystem
label-casa-velocity = Hastighetsdefinition
casa-velocity-radio = Radio
casa-velocity-optical = Optisk
casa-velocity-relativistic = Relativistisk
label-casa-mask = Standardmask
label-casa-storage = Lagring
casa-storage-big = { $manager }, big-endian
casa-storage-little = { $manager }, little-endian
# machine-drafted, needs native review
# Unstructured meshes: the mesh's name in the file, the convention describing it (UGRID,
# ICON, MPAS), its counts, face shapes, coordinates and where the values sit.
label-ugrid-name = Nät
label-ugrid-convention = Konvention
label-ugrid-nodes = Noder
label-ugrid-faces = Ytor
label-ugrid-edges = Kanter
label-ugrid-shapes = Ytformer
label-ugrid-coordinates = Koordinater
label-ugrid-values = Värden på
label-ugrid-split = Delade för ritning
# $n is the formatted count, $count the number (for the plural).
ugrid-triangles = { $count ->
    [one] { $count } triangel
   *[other] { $count } trianglar
}
ugrid-quads = { $count ->
    [one] { $count } fyrhörning
   *[other] { $count } fyrhörningar
}
# Faces of five or more nodes; $max is the most nodes of one face.
ugrid-polygons = { $count ->
    [one] { $count } polygon med { $max } noder
   *[other] { $count } polygoner med upp till { $max } noder
}
ugrid-lonlat = Longitud och latitud
ugrid-projected = Projicerade x och y
ugrid-projected-unit = Projicerade x och y ({ $unit })
ugrid-on-faces = Ytor (ritade enfärgat)
ugrid-on-nodes = Noder (interpolerade över ytorna)
# A mesh file without data variables: the image shows how many nodes each face has.
ugrid-grid-only = Inga: bilden visar noderna per yta
# Faces cut at the antimeridian or closed to a pole so they draw on a flat map.
ugrid-split = { $count ->
    [one] { $count } yta vid antimeridianen eller en pol
   *[other] { $count } ytor vid antimeridianen eller en pol
}
# machine-drafted, needs native review
# Slide annotations (NDP.view, Aperio ImageScope, ASAP sidecar files).
label-annotation-file = Fil
# "slide.xml (Aperio ImageScope)"
annotation-source = { $file } ({ $format })
label-annotation-shapes = Former
label-annotation-groups = Grupper
# Shapes or points over the reader's limits, left out.
label-annotation-skipped = Utelämnade
annotation-polygon = Polygon
annotation-polyline = Linje
annotation-rectangle = Rektangel
annotation-ellipse = Ellips
annotation-point = Punkt
annotation-points = Punkter
annotation-ruler = Linjal
annotation-arrow = Pil
# The list of annotations is cut short: "and 1,200 more".
annotation-more = och { $count } till
# All-fields group title: "Annotations in slide.xml".
fields-annotations-header = Annoteringar i { $file }
label-drawing-version = Version
label-drawing-units = Enheter
label-drawing-extents = Utsträckning
label-drawing-sheet = Ark
label-drawing-layouts = Layouter
label-drawing-layers = Lager
label-drawing-entities = Objekt
label-drawing-codepage = Teckentabell
label-drawing-missing-fonts = Ersatta typsnitt
# machine-drafted, needs native review
# A font file the drawing names that was not found: the styles naming it, and the font drawing its text instead.
drawing-missing-font = { $file } (stilar { $styles }), ritat med { $substitute }
# A Big Font (double-byte text) that was not found; its characters come from other fonts.
drawing-missing-bigfont = { $file } (stilar { $styles }), ritat med reservtypsnitt
# Survey data (LandXML).
label-survey-crs = Koordinatsystem
survey-crs-none = Inte angivet
label-survey-project = Projektnamn
label-survey-application = Skriven av
label-survey-surface = Terrängmodell
# A TIN surface: its name, counts, plan and sloped areas, the volume above its lowest point, its elevations and the contour interval drawn; unit is "m" or "ft" (areas in square units, volume in cubic units).
survey-surface = { $name }: { $points ->
        [one] { $points } punkt
       *[other] { $points } punkter
    }, { $triangles ->
        [one] { $triangles } triangel
       *[other] { $triangles } trianglar
    }; area i plan { $area } { $unit }², verklig area { $area3d } { $unit }²; { $volume } { $unit }³ över den lägsta punkten; höjder { $zmin }–{ $zmax } { $unit }; höjdkurvor var { $interval } { $unit }
label-survey-alignment = Linjeföring
# An alignment: its name, length and first and last stations ("1+234.50").
survey-alignment = { $name }: { $length } { $unit }, sektion { $start } till { $end }
# SHX shape fonts found and used for the drawing's text.
label-drawing-shx-fonts = SHX-typsnitt
label-drawing-warnings = Problem
label-plot-paper = Papper
# "2 of 5": the page shown and how many the plot has.
plot-page-of = { $page } av { $pages }
label-plot-page = Sida
label-plot-pens = Använda pennor
# The printer language a plot came wrapped in, e.g. "PCL 5".
label-plot-job = Utskriftsjobb
# What a plot draws: strokes, fills and labels.
label-plot-items = Ritade element
# Width × height of what is drawn, in drawing units: "1,250 × 800 mm".
drawing-extents = { $width } × { $height } { $unit }
drawing-extents-unitless = { $width } × { $height }
drawing-model-space = Modell
# "24, 3 off or frozen"
drawing-layers-off = { $num }, { $off } av eller frysta
# DXF $INSUNITS.
drawing-units = { $code ->
    [1] Tum
    [2] Fot
    [3] Engelska mil
    [4] Millimeter
    [5] Centimeter
    [6] Meter
    [7] Kilometer
    [8] Mikrotum
    [9] Tusendels tum
    [10] Yards
    [11] Ångström
    [12] Nanometer
    [13] Mikrometer
    [14] Decimeter
    [15] Dekameter
    [16] Hektometer
    [17] Gigameter
    [18] Astronomiska enheter
    [19] Ljusår
    [20] Parsec
    [21] Amerikanska lantmäterifot
   *[other] Enhetslös
}
# All-fields group title: "DXF entities".
fields-drawing-entities = { $format }-objekt

## IC-layouter (GDSII)

# machine-drafted, needs native review
label-layout-library = Bibliotek
label-layout-units = Enheter
# The database unit (the grid coordinates are stored in) and the user unit, with SI symbols:
# "1 nm database unit, 1 µm user unit".
layout-units = { $db } databasenhet, { $user } användarenhet
# The top cell shown (each top cell is a page).
label-layout-cell = Cell
label-layout-cells = Celler
# Cells no other cell places.
label-layout-top-cells = Toppceller
# "TOP, TEST, PAD and 12 more"
layout-top-cells-more = { $names } och { $more } till
# Layer/datatype pairs.
label-layout-layers = Lager
# Polygons, paths and boxes, as stored (each cell once).
label-layout-shapes = Former
label-layout-texts = Etiketter
# Cell references and the placements they make: "1,204 (88,320 placements)".
label-layout-references = Referenser
layout-references = { $refs } ({ $placements } { $placements ->
    [one] placering
   *[other] placeringar
})
# Shapes with every cell placement expanded.
label-layout-flat-shapes = Former vid utplattning
label-layout-bounds = Gränser
# Corners of what the cell draws, micrometres: "-12.5, 0 to 2,900, 3,500 µm".
layout-bounds = { $x0 }, { $y0 } till { $x1 }, { $y1 } µm

## Kretskort och scheman (Gerber, Excellon, KiCad)

# machine-drafted, needs native review
group-board = Kretskort
group-schematic = Schema
# The board view shown (its layers, its top or bottom), or the schematic sheet.
label-pcb-view = Vy
label-pcb-project = Projekt
label-pcb-revision = Revision
label-pcb-date = Datum
# The CAD program that wrote the file.
label-pcb-generator = Skapad med
# Width × height of the board outline: "100 × 80 mm".
label-pcb-size = Kortstorlek
label-pcb-thickness = Tjocklek
label-pcb-copper-layers = Kopparlager
label-pcb-sheets = Blad
# The board's layers or files, each with its role: "board-F_Cu.gbr (Top copper)".
label-pcb-layers = Lager
pcb-layer-role = { $name } ({ $role })
label-pcb-components = Fotavtryck
label-pcb-symbols = Symboler
label-pcb-nets = Nät
label-pcb-tracks = Ledare
label-pcb-wires = Ledningar
label-pcb-vias = Vior
# Drilled holes and how many drill sizes: "1,204 in 9 sizes".
label-pcb-holes = Hål
pcb-holes = { $holes } i { $sizes } { $sizes ->
    [one] storlek
   *[other] storlekar
}
label-pcb-apertures = Aperturer
# What a board layer or file is, shown after its name.
pcb-role-inner-copper = Inre koppar { $n }
pcb-role-top-copper = Koppar ovansida
pcb-role-bottom-copper = Koppar undersida
pcb-role-top-solder-mask = Lödmask ovansida
pcb-role-bottom-solder-mask = Lödmask undersida
pcb-role-top-silkscreen = Märktryck ovansida
pcb-role-bottom-silkscreen = Märktryck undersida
pcb-role-top-paste = Pasta ovansida
pcb-role-bottom-paste = Pasta undersida
pcb-role-top-solder-paste = Lodpasta ovansida
pcb-role-bottom-solder-paste = Lodpasta undersida
pcb-role-top-adhesive = Lim ovansida
pcb-role-bottom-adhesive = Lim undersida
pcb-role-top-courtyard = Courtyard ovansida
pcb-role-bottom-courtyard = Courtyard undersida
pcb-role-top-fabrication = Tillverkning ovansida
pcb-role-bottom-fabrication = Tillverkning undersida
pcb-role-board-outline = Kortkontur
pcb-role-plated-drill = Pläterade borrhål
pcb-role-non-plated-drill = Opläterade borrhål
pcb-role-drill = Borrning
pcb-role-drill-holes = Borrhål
pcb-role-mechanical = Mekaniskt
pcb-role-margin = Marginal
pcb-role-drawings = Ritningar
pcb-role-comments = Kommentarer
pcb-role-engineering-change-1 = Teknisk ändring 1
pcb-role-engineering-change-2 = Teknisk ändring 2
pcb-role-vias = Vior
pcb-role-other = Övrigt
pcb-role-unknown = Okänt
label-pcb-macros = Aperturmakron

## Spektra (JCAMP-DX, SPC, mzML, mzXML, Bruker NMR)

# machine-drafted, needs native review
group-spectrum = Spektrum
label-spectrum-format = Format
label-spectrum-technique = Teknik
label-spectrum-title = Titel
# The page shown: "3 of 1,204: Scan 1532".
label-spectrum-page = Sida
spectrum-page-of = { $page } av { $pages }: { $name }
# An axis with its unit: "Wavenumber (cm⁻¹)".
spectrum-axis = { $title } ({ $unit })
label-spectrum-x = X-axel
label-spectrum-y = Y-axel
label-spectrum-nucleus = Kärna
label-spectrum-frequency = Spektrometerfrekvens
label-spectrum-solvent = Lösningsmedel
label-spectrum-pulse-program = Pulsprogram
label-spectrum-spectra = Spektra
label-spectrum-points = Punkter
label-spectrum-instrument = Instrument
# Who or what lab measured it.
label-spectrum-origin = Ursprung
label-spectrum-date = Uppmätt
# How Specular made the page from the raw data (an FID transformed, phased automatically).
label-spectrum-processing = Bearbetning
spectrum-technique-ir = Infraröd
spectrum-technique-raman = Raman
spectrum-technique-uv-vis = UV-Vis
spectrum-technique-fluorescence = Fluorescens
spectrum-technique-nmr = NMR
spectrum-technique-ms = Masspektrum
spectrum-technique-chromatogram = Kromatogram
spectrum-technique-other = Spektrum

## 3D-modeller (STL, OBJ, PLY, glTF, 3MF)

# machine-drafted, needs native review
group-mesh = 3D-modell
label-triangles = Trianglar
label-vertices = Hörn
label-materials = Material
label-textures = Texturer
label-up-axis = Uppåtaxel
# Textures the model uses, some of which could not be read: "3 (1 missing)".
mesh-textures-missing = { $count } ({ $missing } saknas)

## Molekylstrukturer (PDB, mmCIF, CIF, SDF, XYZ)

# machine-drafted, needs native review
group-structure = Struktur
# The entry's identifier: a PDB ID such as 1CRN, or the CIF data block's name.
label-structure-id = Post
label-structure-classification = Klassificering
# How the structure was determined: X-ray diffraction, electron microscopy, solution NMR…
label-structure-method = Metod
# The crystallographic or cryo-EM resolution, in ångströms.
label-structure-resolution = Upplösning
structure-resolution = { $value } Å
label-structure-deposited = Deponerad
label-structure-formula = Formel
label-structure-chains = Kedjor
# Residues (amino acids, nucleotides) in the polymer chains.
label-structure-residues = Rester
label-structure-atoms = Atomer
# Atoms after the unit cell was filled by symmetry, and those the file lists: "96 (12 in the asymmetric unit)".
structure-atoms-cell = { $count } ({ $asymmetric } i den asymmetriska enheten)
label-structure-bonds = Bindningar
# Models in the file (an NMR ensemble), of which the first is shown.
label-structure-models = Modeller
# Molecules (SDF records) or data blocks in the file, of which the first is shown.
label-structure-records = Poster
structure-first-of = { $count } (den första visas)
# Small molecules bound to the polymer, by their three-letter codes.
label-structure-ligands = Ligander
# A list cut short: "HEM, NAG, SO4 and 9 more".
structure-ligands-more = { $list } och { $more } till
label-structure-cell = Enhetscell
# Unit cell edge lengths (Å) and angles (degrees).
structure-cell = a = { $a } Å, b = { $b } Å, c = { $c } Å, α = { $alpha }°, β = { $beta }°, γ = { $gamma }°
label-structure-space-group = Rymdgrupp
label-structure-secondary = Sekundärstruktur
structure-ss-file = Från filen
structure-ss-computed = Beräknad från huvudkedjan

## HDF4 och HDF-EOS2

# machine-drafted, needs native review
group-hdf4 = HDF4
# The HDF library version that wrote the file, e.g. "NCSA HDF Version 4.2 Release 15".
label-hdf4-library = Bibliotek
# Counts of the file's scientific datasets, raster images, groups (Vgroups) and tables (Vdatas).
label-hdf4-datasets = Datamängder
label-hdf4-images = Rasterbilder
label-hdf4-vgroups = Grupper
label-hdf4-vdatas = Tabeller
# HDF-EOS: NASA's Earth Observing System conventions on top of HDF4 (MODIS, ASTER, AIRS…).
group-hdf-eos = HDF-EOS
label-eos-version = Version
# The grids, swaths and point structures the file defines.
label-eos-structures = Strukturer
eos-grids = { $count ->
    [one] { $count } rutnät ({ $names })
   *[other] { $count } rutnät ({ $names })
}
eos-swaths = { $count ->
    [one] { $count } avsökningsstråk ({ $names })
   *[other] { $count } avsökningsstråk ({ $names })
}
eos-points = { $count ->
    [one] { $count } punktstruktur
   *[other] { $count } punktstrukturer
}
# The grid or swath the dataset shown belongs to.
label-eos-structure = Struktur
# The grid's map projection as its GCTP name, e.g. GCTP_SNSOID (sinusoidal).
label-eos-projection = Projektion
label-eos-grid-size = Rutnätsstorlek
# Columns × rows.
eos-grid-size = { $width } × { $height }
# The grid's upper-left and lower-right corners.
label-eos-corners = Hörn
# The product's short name from the inventory metadata, e.g. MOD09A1.
label-eos-product = Produkt
# When the data was acquired.
label-eos-time = Tid
eos-time = { $begin } till { $end }

## Planetbilder (VICAR, PDS3, PDS4: uppdragsdata från NASA och ESA)

# machine-drafted, needs native review
group-planetary = Planetbild
# The body observed, e.g. MARS, Ganymede.
label-planetary-target = Mål
label-planetary-mission = Uppdrag
# The spacecraft, rover or lander carrying the instrument.
label-planetary-spacecraft = Rymdfarkost
label-planetary-start = Starttid
label-planetary-stop = Sluttid
# The product's identifier in the archive.
label-planetary-product = Produkt-id
# The map projection of a map-projected image, as the label names it.
label-planetary-projection = Kartprojektion
label-planetary-resolution = Upplösning
planetary-metres-per-pixel = { $metres } m per pixel
planetary-pixels-per-degree = { $pixels } pixlar per grad
# Latitude and longitude of the map projection's centre.
label-planetary-centre = Projektionscentrum
planetary-lat-lon = { $lat }°, { $lon }°
# The latitudes and longitudes the map covers.
label-planetary-coverage = Täckning
planetary-coverage = latitud { $south }° till { $north }°, longitud { $west }° till { $east }°
# The body's equatorial radius the map uses.
label-planetary-radius = Kroppens radie
planetary-km = { $km } km

## OMERO (bilder öppnade från en OMERO-server eller Image Data Resource)

# machine-drafted, needs native review
group-omero = OMERO
label-omero-server = Server
# $server the server's address; $user the user name signed in with.
omero-server-user = { $server } (som { $user })
omero-server-public = { $server } (offentlig)
label-omero-image = Bild
# $name the image's name; $id its number on the server.
omero-image = { $name } (ID { $id })
# The project and dataset the image is in, as "Project › Dataset".
label-omero-project = Projekt
# The screen, plate and well, as "Screen › Plate › B4".
label-omero-screen = Screen
label-omero-owner = Ägare
# $owner the image's owner; $group the OMERO group it belongs to.
omero-owner-group = { $owner } ({ $group })
label-omero-acquired = Tagen
label-omero-dimensions = Dimensioner
omero-size = { $width } × { $height }
omero-z = { $count ->
    [one] { $count } Z-plan
   *[other] { $count } Z-plan
}
omero-c = { $count ->
    [one] { $count } kanal
   *[other] { $count } kanaler
}
omero-t = { $count ->
    [one] { $count } tidpunkt
   *[other] { $count } tidpunkter
}
# The physical size of a pixel along X, Y (and Z).
label-omero-physical = Pixelstorlek
label-omero-channels = Kanaler
# A channel not shown in the server's rendering.
omero-channel-hidden = { $channel } (dold)
# How the pixels are fetched from the server: rendered tiles (JPEG) or the whole plane (PNG).
label-omero-fetched = Hämtad som
omero-fetched-tiles = { $encoding }-rutor på { $width } × { $height }, { $levels ->
    [one] { $levels } nivå
   *[other] { $levels } nivåer
}
omero-fetched-whole = Hela plan som { $encoding }

## Reflektanstransformationsbilder (PTM, RTI, Relight)

# machine-drafted, needs native review
group-rti = Reflektanstransformation
# The reflectance model the coefficients belong to: PTM (polynomial), HSH (hemispherical harmonics), RBF…
label-rti-basis = Bas
label-rti-order = Ordning
# Coefficients per colour channel.
label-rti-terms = Koefficienter
# How colour is stored: LRGB (one colour, varying luminance), RGB, YCC…
label-rti-colour = Färglayout
# Coefficient images stored in the file.
label-rti-planes = Plan
# The byte-to-coefficient mapping the file states, per coefficient.
label-rti-scale = Skala
label-rti-bias = Förskjutning

## Punktmoln

# machine-drafted, needs native review
group-points = Punktmoln
label-points = Punkter
# Points read of those in the file, when a very large cloud was evenly thinned: "32,000,000 of 120,000,000".
points-subsampled = { $loaded } av { $total }
# The LAS point data record format number (0–10).
label-point-format = Punktformat
# The coordinate reference system.
label-crs = Koordinatsystem
# What each point carries besides its position.
label-attributes = Attribut
points-channel-rgb = RGB
points-channel-intensity = Intensitet
points-channel-classification = Klassificering
points-channel-returns = Returer
# ASPRS classes present, with point counts: "Ground (1,234), Building (567)".
label-classes = Klasser
# E57 scans, with point counts.
label-scans = Skanningar
point-class-0 = Aldrig klassificerad
point-class-1 = Oklassificerad
point-class-2 = Mark
point-class-3 = Låg vegetation
point-class-4 = Medelhög vegetation
point-class-5 = Hög vegetation
point-class-6 = Byggnad
point-class-7 = Låg punkt (brus)
point-class-8 = Modellnyckelpunkt
point-class-9 = Vatten
point-class-10 = Järnväg
point-class-11 = Vägyta
point-class-12 = Överlapp
point-class-13 = Topplina
point-class-14 = Fasledare
point-class-15 = Kraftledningsstolpe
point-class-16 = Ledningsskarv
point-class-17 = Brobana
point-class-18 = Högt brus
point-class-19 = Överliggande konstruktion
point-class-20 = Ignorerad mark
point-class-21 = Snö
point-class-22 = Tidsmässig uteslutning
# A class without a standard name, by number.
point-class-other = Klass { $class }

## Gaussian splats

# machine-drafted, needs native review
group-splats = Gaussiska splats
label-splats = Splats
# Splats shown of those in the file, when a very large file was evenly thinned: "12,000,000 of 30,000,000".
splats-shown-of = { $shown } av { $total }
# The degree (1 to 3) of the spherical harmonics that give each splat its view-dependent colour.
label-splat-harmonics = Vyberoende färg
splats-harmonics-none = Ingen
# $format: the file format, e.g. "SPZ".
splats-format-antialiased = { $format }, tränad med kantutjämning

## Varningar

# machine-drafted, needs native review
warn-not-diagnostic = Inte för diagnostiskt bruk. Specular är en allmän bildvisare, inte en medicinteknisk produkt: återgivningen är inte kalibrerad för diagnostik.

warn-gps = Filen innehåller platsen där bilden togs. Ta bort platsen innan du delar den om den ska förbli privat.
warn-null-island = GPS-positionen är exakt 0°, 0°; kameran hade troligen ingen positionsbestämning, så platsen är sannolikt fel.
warn-date-future = Fotodatumet ligger i framtiden; kamerans klocka var troligen fel inställd.
warn-icc-damaged = Den inbäddade färgprofilen är skadad och kunde inte läsas.
warn-metadata-failed = Metadata kunde inte läsas på grund av ett fel i Specular. Själva bilden är hel.
warn-orientation-conflict = EXIF och XMP anger olika orientering (EXIF: { $exif }, XMP: { $xmp }). EXIF-värdet används.
warn-exif-damaged = En del EXIF-data kunde inte läsas ({ $error }); resten visas.
warn-xmp-damaged = XMP-metadata är felformaterad ({ $error }); bara en del kunde läsas.

## Om

geonames-attribution = Ortnamn: GeoNames (www.geonames.org), licensierade under Creative Commons Erkännande 4.0 (CC BY 4.0, https://creativecommons.org/licenses/by/4.0/). Offlinesökning med craten reverse_geocoder av Grant Miner (MIT eller Apache-2.0). Data har konverterats till en ortlista; den tillhandahålls i befintligt skick, utan garantier.

## TIFF and DNG structure values (All fields)
# machine-drafted, needs native review

subfile-type = { $value ->
    [0] Bild i full upplösning
    [1] Bild i reducerad upplösning
    [2] En sida i en flersidig bild
    [4] Transparensmask
   *[other] Annat
}
predictor = { $value ->
    [1] Ingen
    [2] Horisontell differens
    [3] Flyttalsdifferens
   *[other] Annat
}
extra-samples = { $value ->
    [0] Ej angivet
    [1] Förmultiplicerad alfa
    [2] Rak alfa
   *[other] Annat
}
sample-format = { $value ->
    [1] Heltal utan tecken
    [2] Heltal med tecken
    [3] Flyttal
    [4] Odefinierat
   *[other] Annat
}

## Geodata (GeoTIFF, GeoJSON, Shapefile, GeoPackage, KML/KMZ, GPX)

# machine-drafted, needs native review
group-geospatial = Geodata
label-geo-crs = Koordinatsystem
label-geo-datum = Datum
label-geo-projection = Projektion
label-geo-unit = Enheter
label-geo-bounds = Utbredning
label-geo-lonlat-bounds = Longitud och latitud
label-geo-pixel-size = Pixelstorlek
label-geo-overviews = Översikter
label-geo-nodata = NoData-värde
label-geo-raster-type = Rastertyp
label-geo-vertical = Vertikal referens
label-geo-citation = Källhänvisning
label-geo-features = Objekt
label-geo-vertices = Brytpunkter
label-geo-layer = Lager
label-geo-layers = Lager
label-geo-tiles = Plattor
label-geo-encoding = Textkodning
label-geo-warnings = Problem
# The companion files a georeference was read from ("scene.jgw, scene.prj").
label-geo-sidecars = Medföljande filer
# machine-drafted, needs native review
# A projection Specular cannot convert to longitude and latitude: $method is its name.
geo-projection-unsupported = { $method } (koordinater stöds inte)
# How longitude and latitude are brought to WGS 84 (what GPS and web maps use).
label-geo-to-wgs84 = Till WGS 84
geo-to-wgs84-same = Behövs inte
geo-to-wgs84-helmert = Datumskifte (Helmert)
geo-to-wgs84-by-area = Datumskifte (Helmert, valt efter område)
geo-to-wgs84-unknown = Okänt: koordinaterna är på filens eget datum
# A coordinate system's name and its EPSG code: "WGS 84 / UTM zone 33N (EPSG:32633)".
geo-crs-epsg = { $name } (EPSG:{ $code })
geo-crs-unknown = Inte angivet
# Map bounds: the lower-left and upper-right corners.
geo-bounds = { $x0 }, { $y0 } till { $x1 }, { $y1 }
# Bounds in degrees: west to east longitude, south to north latitude.
geo-lonlat-bounds = { $west }° till { $east }° longitud, { $south }° till { $north }° latitud
geo-pixel-size = { $x } × { $y }
geo-overviews = { $n ->
    [one] { $n } översikt: { $sizes }
   *[other] { $n } översikter: { $sizes }
}
geo-pixel-is-point = Pixlar är punkter
geo-layer = { $features ->
    [one] { $features } objekt
   *[other] { $features } objekt
}, { $fields ->
    [one] { $fields } fält
   *[other] { $fields } fält
}
# A GeoPackage tile pyramid: its table, the zoom level shown and its tile count.
geo-tiles = { $table }, zoomnivå { $zoom }, { $tiles ->
    [one] { $tiles } platta
   *[other] { $tiles } plattor
}
fields-geo-geokeys = GeoTIFF-nycklar
fields-geo-gdal = GDAL-metadata
fields-geo-wkt = Koordinatsystem (WKT)
fields-geo-projjson = Koordinatsystem (PROJJSON)
# machine-drafted, needs native review
# What a web map tile set (MBTiles, PMTiles) says about itself: container, tile type, zoom
# levels, description and other metadata.
fields-geo-tileset = Rutuppsättning
# The credit line a map's data asks to be shown with it ("© OpenStreetMap contributors").
label-geo-attribution = Källhänvisning
# The header lines of a georeferenced raster format (ENVI, ASCII grid, DTED, ERDAS IMAGINE).
fields-geo-header = Huvud
# machine-drafted, needs native review
# GIFTI files: the format version, what the file holds, the number of data arrays and the
# encoding of their values (e.g. "GZipBase64Binary").
label-gifti-version = GIFTI-version
label-gifti-role = Innehåller
gifti-role-surface = Yta
gifti-role-labels = Etiketter per hörn
gifti-role-data = Värden per hörn
label-gifti-arrays = Datamatriser
label-gifti-encoding = Kodning
label-gifti-vertices = Hörn
label-gifti-triangles = Trianglar
# The brain structure, e.g. "CortexLeft".
label-gifti-structure = Struktur
# A surface's geometric type, e.g. "Anatomical", "Inflated".
label-gifti-geometry = Geometrisk typ
# The coordinate transform of the vertices: $from and $to are coordinate spaces such as
# "scanner", "Talairach" or "MNI 152".
label-gifti-transform = Koordinater
gifti-transform = { $from } (transformation till { $to } behövs inte)
gifti-transform-applied = { $from } → { $to } (tillämpad)
# The surface file a data file is drawn on, found beside it.
label-gifti-surface = Visas på
# The data array shown: its name, NIfTI intent (e.g. "NIFTI_INTENT_SHAPE") and data type.
label-gifti-array = Matris
label-gifti-intent = Intent
label-gifti-data-type = Datatyp
# Entries of the label table (names and colours of regions).
label-gifti-labels = Etikettabell
# CIFTI-2 files: the version, NIfTI intent (e.g. "ConnDenseScalar") and matrix size.
label-cifti-version = CIFTI-version
label-cifti-intent = Intent
label-cifti-matrix = Matris
cifti-matrix = { $rows } × { $columns }
# The maps (pages) of the file and what they are.
label-cifti-maps = Kartor
cifti-maps-scalars = { $count ->
        [one] { $count } skalärkarta
       *[other] { $count } skalärkartor
    }
cifti-maps-labels = { $count ->
        [one] { $count } etikettkarta
       *[other] { $count } etikettkartor
    }
cifti-maps-series = { $count ->
        [one] { $count } tidpunkt
       *[other] { $count } tidpunkter
    }
cifti-maps-parcels = { $count ->
        [one] { $count } parcell
       *[other] { $count } parceller
    }
cifti-maps-other = { $count }
# machine-drafted, needs native review
# A dense connectivity file (.dconn.nii): every brain place's connectivity to every other.
# $seeds: how many places can be picked; $size: the bytes read when one is picked (e.g. "357 KB").
label-cifti-dense = Konnektivitet
cifti-dense = { $seeds ->
        [one] { $seeds } seed
       *[other] { $seeds } seeds
    }, { $size } läses per seed
# A time series' start and step; $unit is e.g. "second".
label-cifti-series = Serie
cifti-series = från { $start }, var { $step } ({ $unit })
label-cifti-models = Hjärnmodeller
label-cifti-parcels = Parceller
label-cifti-structures = Strukturer
label-cifti-voxels = Subkortikala voxlar
# A surface the file's values lie on: the brain structure (e.g. "CORTEX_LEFT"), its vertex
# count, and the surface file found beside the file for it, or that none was found.
label-cifti-surface = Yta
cifti-surface-found = { $structure }: { $vertices ->
        [one] { $vertices } hörn
       *[other] { $vertices } hörn
    }, { $file }
cifti-surface-missing = { $structure }: kräver en yta med { $vertices ->
        [one] { $vertices } hörn
       *[other] { $vertices } hörn
    } (ingen bredvid filen)
# How the file is shown: on the surfaces found beside it, or as its subcortical voxels.
label-cifti-shown = Visas som
cifti-shown-surfaces = Färgade ytor
cifti-shown-voxels = Volym av de subkortikala voxlarna
# machine-drafted, needs native review
# Shown with a picked seed's row: click a surface vertex, or move the crosshair in MPR.
cifti-shown-connectivity = Konnektivitet för vald seed
# Maps left out of the voxel volume, which would be too large with them.
label-cifti-left-out = Utelämnade kartor
# Tractograms: format, number of streamlines (fibre tracts) and points.
label-tract-format = Format
label-tract-streamlines = Streamlines
# A large tractogram draws only some of its streamlines.
tract-streamlines-shown = { $count } ({ $shown } visas)
label-tract-points = Punkter
label-tract-mean-length = Medellängd
# A length in millimetres.
tract-mm = { $value } mm
# Values per point and per streamline the file stores, by name.
label-tract-scalars = Skalärer per punkt
label-tract-properties = Egenskaper per streamline
# TrackVis: the voxel grid the tracts were made in, its voxel size and voxel order (e.g. "LAS").
label-tract-grid = Rutnät
label-tract-voxel-size = Voxelstorlek
label-tract-voxel-order = Voxelordning
# How the points were brought into scanner (RAS) millimetres.
label-tract-space = Koordinater
tract-space-vox-to-ras = Från voxel-till-RAS-matrisen
tract-space-voxel-order = Från voxelordningen (ingen voxel-till-RAS-matris)
tract-space-scanner = Skannerrymd
# MRtrix: the tracking step size.
label-tract-step = Steglängd

## File headers (DPX, Cineon, XWD, MIFF, TIM, WSQ, JBIG)

# machine-drafted, needs native review
# What the file's own header records, for formats without Exif or XMP.
group-header = Filhuvud
label-header-created = Skapad
# The program or person that wrote the file.
label-header-creator = Upphovsperson
label-header-copyright = Upphovsrätt
label-header-project = Projekt
label-header-comment = Kommentar
# The scanner, camera or other device the image came from.
label-header-device = Enhet
# SMPTE time code of the frame: "01:00:00:12".
label-header-timecode = Tidkod
# A film frame's edge code (KeyKode).
label-header-edge-code = Kantkod
label-header-frame-rate = Bildfrekvens
# Where the frame sits in its sequence.
label-header-frame = Bildruta
# Film stock and slate.
label-header-film = Film
# How code values relate to light: printing density, linear, BT.709…
label-header-transfer = Överföringsfunktion
# The colour primaries the file states.
label-header-colorimetric = Kolorimetri
# The X window a screen dump was taken of.
label-header-window = Fönster
label-header-resolution = Upplösning
label-header-compression = Komprimering
# machine-drafted, needs native review
# JPEG XS: the profile and the level the codestream states ("Main 422.10"; "4k-1, sublevel 6 bpp").
label-header-profile = Profil
# machine-drafted, needs native review
label-header-level = Nivå
# machine-drafted, needs native review
# JPEG XS: how many wavelet levels, horizontal and vertical ("5 horizontal, 1 vertical").
label-header-decomposition = Waveletnivåer
# machine-drafted, needs native review
# JPEG XS: the colour transform ("None", "Reversible colour transform (RCT)", "Star-Tetrix, RGGB mosaic").
label-header-colour-transform = Färgtransform
# machine-drafted, needs native review
# Photoshop: the artboards, by name and size ("Phone (375 × 667), Tablet (768 × 1024)").
label-header-artboards = Ritytor
# machine-drafted, needs native review
# Photoshop file without a merged image, shown from its layers: the features it leaves out,
# by Photoshop's names ("Color Lookup, Knockout").
label-header-not-drawn = Ritas inte
# machine-drafted, needs native review
# The same, for the features drawn only approximately ("Bevel & Emboss, Satin").
label-header-approximated = Ritas ungefärligt

## Fonts (TTF, OTF, TTC/OTC, WOFF, WOFF2)

# machine-drafted, needs native review
group-font = Typsnitt
label-font-family = Familj
label-font-style = Stil
# The face's place in a font collection: "2 of 4".
label-font-face = Snitt
font-face-of = { $index } av { $count }
label-font-version = Version
label-font-designer = Formgivare
# The font's maker (the name table's manufacturer).
label-font-manufacturer = Stilgjuteri
label-font-glyphs = Glyfer
label-font-outlines = Konturer
font-outlines-truetype = TrueType (kvadratiska)
font-outlines-cff = PostScript (CFF, kubiska)
font-outlines-cff2 = PostScript (CFF2, kubiska)
font-outlines-bitmap = Bara bitmappar
font-outlines-none = Inga
# What the font's licence flags (OS/2 fsType) allow documents that embed it to do.
label-font-embedding = Inbäddning
font-embedding-installable = Får bäddas in och installeras.
font-embedding-restricted = Får inte bäddas in.
font-embedding-preview-print = Får bara bäddas in för visning och utskrift.
font-embedding-editable = Får bäddas in i dokument som redigeras.
font-embedding-no-subsetting = Bara helt, inte som delmängd.
font-embedding-bitmap-only = Bara som bitmappar.
# Variation axes of a variable font: "Weight 100–900 (400)".
label-font-axes = Variationsaxlar
font-axis = { $name } { $min }–{ $max } ({ $default })
label-font-instances = Namngivna stilar
label-font-colour = Färgglyfer
font-colour-colr0 = COLR-lager
font-colour-colr1 = COLR v1-övertoningar
font-colour-sbix = Bitmappar (sbix)
font-colour-cbdt = Bitmappar (CBDT)
font-colour-svg = SVG
label-font-weight = Vikt
label-font-units-per-em = Enheter per em
label-font-tables = Tabeller
label-font-licence = Licens
label-font-licence-url = Licens-URL
label-font-copyright = Upphovsrätt
label-font-trademark = Varumärke

## Office documents (Word, Excel, PowerPoint, iWork, OpenDocument)

# machine-drafted, needs native review
group-document = Dokument
label-document-kind = Typ
document-kind-word = Word-dokument
document-kind-excel = Excel-arbetsbok
document-kind-powerpoint = PowerPoint-presentation
document-kind-pages = Pages-dokument
document-kind-numbers = Numbers-kalkylblad
document-kind-keynote = Keynote-presentation
document-kind-odf-text = OpenDocument-text
document-kind-odf-spreadsheet = OpenDocument-kalkylblad
document-kind-odf-presentation = OpenDocument-presentation
document-kind-odf-graphics = OpenDocument-teckning
document-kind-odf-other = OpenDocument-fil
# machine-drafted, needs native review
document-kind-playlist = Spellista
# machine-drafted, needs native review
label-playlist-items = Objekt
# Appended to the kind: "Word document, template, with macros".
document-template = mall
document-macros = med makron
document-legacy = Office 97–2003-format
label-document-preview = Förhandsvisning
# The image format of the preview stored in the file, e.g. "JPEG image".
document-preview-image = { $format }-bild
document-preview-none = Ingen i filen
label-document-subject = Ämne
label-document-author = Författare
label-document-description = Beskrivning
label-document-category = Kategori
label-document-last-saved-by = Senast sparad av
label-document-created = Skapad
label-document-modified = Ändrad
label-document-printed = Senast utskriven
label-document-pages = Sidor
label-document-slides = Bilder
label-document-hidden-slides = Dolda bilder
label-document-notes = Anteckningssidor
label-document-sheets = Blad
label-document-words = Ord
label-document-characters = Tecken
label-document-paragraphs = Stycken
label-document-lines = Rader
label-document-editing-time = Redigeringstid
document-minutes = { $minutes } min
label-document-revision = Revision
label-document-application = Sparad med
label-document-company = Företag
label-document-template = Mall

## Books (EPUB)

# machine-drafted, needs native review
group-book = Bok
label-book-authors = Författare
label-book-publisher = Förlag
label-book-date = Utgiven
label-book-language = Språk
label-book-pages = Sidor
# Entries in the book's table of contents.
label-book-contents = Poster i innehållsförteckningen
label-book-layout = Layout
book-layout-reflowable = Flytande
book-layout-fixed = Fast layout
book-right-to-left = höger till vänster
label-book-version = Format
book-version = EPUB { $version }
label-book-fonts = Inbäddade typsnitt
book-fonts-obfuscated = { $count } ({ $obfuscated } maskerade)
label-book-subjects = Ämnen
label-book-identifier = Identifierare
label-book-rights = Rättigheter

unit-symbol-mm = mm
unit-symbol-cm = cm
unit-symbol-m = m
unit-symbol-km = km
unit-symbol-mm2 = mm²
unit-symbol-cm2 = cm²
unit-symbol-m2 = m²
unit-symbol-km2 = km²
unit-symbol-ha = ha
unit-symbol-ft = ft
unit-symbol-ft2 = ft²
unit-symbol-mi = mi
unit-symbol-mi2 = mi²
unit-symbol-ac = ac

# machine-drafted, needs native review
volume-orient-parrec = Philips vinkling och förskjutning
volume-orient-ecat = ECAT-voxelstorlekar och förskjutningar (opålitliga)
label-volume-technique = Teknik
label-volume-version = Formatversion
label-volume-isotope = Isotop
label-volume-tracer = Spårämne
label-volume-units = Enheter

# Text, tables and subtitles.
label-header-encoding = Teckenuppsättning
label-header-lines = Rader
label-header-line-ends = Radslut
label-header-words = Ord
label-header-pages = Sidor
label-header-pictures = Bilder
label-header-pictures-not-shown = Bilder som inte visas
label-header-tables = Tabeller
label-header-rows = Tabellrader
label-header-columns = Kolumner
label-header-delimiter = Avgränsare
label-header-column-names = Kolumnnamn
label-header-cues = Undertexter
label-header-cue-span = Undertexter från – till
