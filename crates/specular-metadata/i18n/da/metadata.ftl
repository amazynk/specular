# machine-drafted, needs native review
# Specular-metadatapanelet. Dansk.

## Gruppetitler i oversigten

group-camera = Kamera
group-exposure = Eksponering
group-lens = Objektiv
group-date = Dato og klokkeslæt
group-location = Placering
# machine-drafted, needs native review
# A DJI drone photo: the gimbal's and the aircraft's attitude, the height above take-off.
group-drone = Drone
label-gimbal = Gimbal
label-flight = Luftfartøj
label-relative-altitude = Over startpunktet
drone-attitude = krøjning { $yaw }, tipning { $pitch }, rulning { $roll }
# A DJI thermal camera's radiometric JPEG: its photo opens, its temperatures can't.
label-thermal-data = Temperaturer
drone-thermal-refused = Vises ikke: DJI’s SDK-licens tillader det ikke
drone-thermal-refused-detail = Aflæsning af DJI-temperaturer kræver DJI’s Thermal SDK, hvis licens kun tillader programmer, der leverer SDK’et, at bruge det. Billedet og dets metadata vises.
group-image = Billede
group-file = Fil
group-rights = Rettigheder og beskrivelse
group-credentials = Oplysninger om indhold
# machine-drafted, needs native review
group-medical = Medicinsk billede
# machine-drafted, needs native review
group-inspection = Komponent og inspektion
group-nde-image = Inspektionsbillede (DICONDE)
# machine-drafted, needs native review
# Industrial CT volumes (Zeiss Xradia, Amira, Bruker SkyScan, Nikon X-Tek, VGI, raw volumes).
group-industrial-image = Inspektionsbillede
group-science = Videnskabeligt billede
group-em = Elektronmikroskopi
group-spm = Scanningsprobemikroskopi
group-slide = Objektglas
group-microscopy = Mikroskopi
# X-ray diffraction detector frames (CBF, EDF, SMV, Bruker, MarCCD, R-AXIS).
group-diffraction = Diffraktion
group-surface = Overfladetopografi
group-ebsd = Elektron-tilbagespredningsdiffraktion
group-seismic = Seismiske data
# machine-drafted, needs native review
group-asdf = ASDF
# machine-drafted, needs native review
# CERN ROOT files (particle physics histograms and graphs).
group-root = ROOT
# machine-drafted, needs native review
# CASA images (radio interferometry: the .image, .residual, .model directories tclean writes).
group-casa = CASA-billede
# machine-drafted, needs native review
# GIFTI files: brain surfaces and the values on their vertices (Human Connectome Project
# and FreeSurfer exports).
group-gifti = GIFTI
# CIFTI-2 files: maps over cortical surfaces and subcortical voxels ("grayordinates").
group-cifti = CIFTI
# White-matter fibre tracts (TrackVis, MRtrix).
group-tractography = Traktografi
# machine-drafted, needs native review
group-annotations = Annoteringer
# Imaging mass spectrometry (imzML): a mass spectrum per pixel.
group-mass-spec = Massespektrometrisk billeddannelse
# machine-drafted, needs native review
group-thermal = Termisk billede
# machine-drafted, needs native review
# Flow cytometry list-mode data (FCS): events of a cell sorter or analyser.
group-cytometry = Flowcytometri
# machine-drafted, needs native review
group-spectroscopy = Spektroskopi
# machine-drafted, needs native review
# Unstructured meshes of ocean, coastal and climate models (UGRID, ICON, MPAS in NetCDF).
group-ugrid = Ustruktureret net
group-drawing = Tegning
# machine-drafted, needs native review
group-volume = Volumen

## Etiketter i oversigten

label-camera = Kamera
label-software = Software
label-application = Program
label-shutter = Lukkertid
label-aperture = Blænde
label-iso = ISO
label-focal-length = Brændvidde
label-exposure-bias = Eksponeringskomp.
label-program = Tilstand
label-metering = Lysmåling
label-white-balance = Hvidbalance
label-flash = Blitz
label-lens = Objektiv
label-lens-make = Objektivmærke
label-lens-range = Objektivområde
# machine-drafted, needs native review
label-focus-mode = Fokustilstand
label-stabilisation = Billedstabilisering
label-taken = Optaget
label-digitised = Digitaliseret
label-edited = Sidst redigeret
label-place = Sted
label-coordinates = Koordinater
label-altitude = Højde
label-direction = Retning
label-accuracy = Nøjagtighed
label-gps-time = GPS-tid
label-dimensions = Mål
label-encoding = Kodning
# machine-drafted, needs native review
label-motion = Bevægelse
motion-kind-motion-photo = Bevægelsesfoto
motion-kind-micro-video = Bevægelsesfoto (MicroVideo)
motion-kind-samsung = Samsung bevægelsesfoto
motion-kind-live-photo = Live Photo
motion-duration = { $seconds } s
label-colour-profile = Farveprofil
label-orientation = Orientering
label-file-name = Navn
label-folder = Mappe
label-file-size = Størrelse
label-format = Format
label-modified = Ændret
label-title = Titel
label-description = Beskrivelse
label-creator = Ophavsperson
label-copyright = Ophavsret
label-keywords = Nøgleord
label-rating = Bedømmelse
label-colour-label = Etiket
label-credentials = Oplysninger om indhold

## Gruppetitler under "Alle felter" (selve tagnavnene forbliver som i standarderne)

fields-ifd0 = IFD0 (hovedbillede)
fields-exif = Exif
fields-gps = GPS
fields-interop = Interoperabilitet
fields-ifd1 = IFD1 (miniature)
fields-icc = ICC-profilhoved
# $namespace: fra xmp-namespace; $prefix: f.eks. "dc"
fields-xmp = XMP: { $namespace } ({ $prefix })
xmp-namespace = { $prefix ->
    [dc] Dublin Core
    [xmp] Grundlæggende
    [xmpRights] Rettigheder
    [xmpMM] Mediehåndtering
    [xmpDM] Dynamiske medier
    [tiff] TIFF
    [exif] Exif
    [exifEX] Exif 2.3+
    [aux] Kameratilbehør
    [photoshop] Photoshop
    [crs] Camera Raw-indstillinger
    [lr] Lightroom
    [Iptc4xmpCore] IPTC Core
    [Iptc4xmpExt] IPTC Extension
    [plus] PLUS-licensering
    [GPano] Fotosfære
    [GCamera] Google-kamera
    [hdrgm] HDR-forstærkningskort
    [mwg-rs] Områder
    [pdf] PDF
    [stEvt] Ressourcehændelser
    [stRef] Ressourcereferencer
   *[other] Andet
}
# $number: "0xA500"; $context: "Exif", "GPS", …
unknown-tag = Ukendt tag { $number } ({ $context })
blob-bytes = { $num ->
    [one] ({ $num } byte)
   *[other] ({ $num } byte)
}
maker-note-bytes = ({ $num ->
        [one] { $num } byte
       *[other] { $num } byte
    }, producentnote, ikke afkodet)
more-values = { $shown }, … ({ $total ->
        [one] { $total } værdi
       *[other] { $total } værdier
    })

## Generelle skilletegn

# Adskiller delene i en liste, der bygges i kode (blitzbeskrivelse, stedområder).
list-separator = {", "}
# Adskiller lukkertid · blænde · ISO · brændvidde i oversigtslinjen.
one-line-separator = {" · "}

## Eksponering

exposure-time-fraction = 1/{ $denominator } s
exposure-time-seconds = { $seconds } s
aperture-value = f/{ $f }
iso-value = ISO { $iso }
focal-length-value = { $mm } mm
focal-length-with-equiv = { $mm } mm (svarer til { $equiv } mm)
focal-length-equiv-only = svarer til { $equiv } mm
# $ev er størrelsen; fortegnet kommer fra vælgeren.
exposure-bias-value = { $sign ->
    [plus] +{ $ev } EV
    [minus] −{ $ev } EV
   *[zero] 0 EV
}
exposure-program = { $value ->
    [1] Manuel
    [2] Program (auto)
    [3] Blændeprioritet
    [4] Lukkerprioritet
    [5] Kreativ (dybdeskarphed)
    [6] Action (hurtig lukker)
    [7] Portræt
    [8] Landskab
   *[other] Ukendt
}
metering-mode = { $value ->
    [1] Gennemsnit
    [2] Centervægtet gennemsnit
    [3] Spot
    [4] Multispot
    [5] Matrix (evaluerende)
    [6] Delvis
   *[other] Andet
}
# machine-drafted, needs native review
# Canon maker note CameraSettings FocusMode; 256–258 are the live-view variants of 0–2.
canon-focus-mode = { $value ->
    [0] One-Shot AF
    [1] AI Servo AF
    [2] AI Focus AF
    [3] Manuel fokus
    [4] Enkelt
    [5] Kontinuerlig
    [6] Manuel fokus
    [16] Pan focus
    [256] One-Shot AF (Live View)
    [257] AI Servo AF (Live View)
    [258] AI Focus AF (Live View)
    [512] Movie snap-fokus
    [519] Movie Servo AF
   *[other] Andet
}
# Canon maker note CameraSettings ImageStabilization; 256–260 repeat 0–4.
canon-stabilisation = { $value ->
    [0] Fra
    [1] Til
    [2] Kun ved optagelse
    [3] Panorering
    [4] Dynamisk
    [256] Fra
    [257] Til
    [258] Kun ved optagelse
    [259] Panorering
    [260] Dynamisk
   *[other] Andet
}
# machine-drafted, needs native review
panasonic-focus-mode = { $value ->
    [1] Auto
    [2] Manuel fokus
    [4] Auto (fokusknap)
    [5] Auto (kontinuerlig)
    [6] AF-S
    [7] AF-C
    [8] AF-F
   *[other] Andet
}
# machine-drafted, needs native review
panasonic-stabilisation = { $value ->
    [2] Til (optisk)
    [3] Fra
    [4] Til (tilstand 2)
    [5] Til (optisk, panorering)
    [6] Til (kun hus)
    [7] Til (kun hus, panorering)
    [9] Dual IS
    [10] Dual IS (panorering)
    [11] Dual IS 2
    [12] Dual IS 2 (panorering)
   *[other] Andet
}
label-comment = Kommentar
fields-tags = { $kind }-tags
group-picture = Billedindstillinger
fields-fujifilm = Fujifilm-producentdata
label-film-simulation = Filmsimulering
label-dynamic-range = Dynamisk område
label-highlight-tone = Højlys
label-shadow-tone = Skygger
label-colour-setting = Farve
label-sharpness = Skarphed
label-noise-reduction = Støjreduktion
label-clarity = Klarhed
label-grain-effect = Korneffekt
label-colour-chrome = Farvekrom
label-colour-chrome-blue = Farvekrom blå
label-shutter-type = Lukkertype
label-drive-mode = Fremføringstilstand
fuji-step = { $sign ->
    [plus] +{ $n }
    [minus] −{ $n }
   *[zero] 0
}
fuji-film-mode = { $value ->
    [0] Provia (standard)
    [256] Studieportræt
    [272] Studieportræt (mere mætning)
    [288] Astia (blød)
    [304] Studieportræt (skarpere)
    [512] Velvia (levende)
    [768] Studieportræt Ex
    [1024] Velvia
    [1280] Pro Neg. Std
    [1281] Pro Neg. Hi
    [1536] Classic Chrome
    [1792] Eterna
    [2048] Classic Neg.
    [2304] Eterna Bleach Bypass
    [2560] Nostalgic Neg.
    [2816] Reala Ace
   *[other] Andet
}
fuji-monochrome = { $value ->
    [768] Monokrom
    [769] Monokrom, rødt filter
    [770] Monokrom, gult filter
    [771] Monokrom, grønt filter
    [784] Sepia
    [1280] Acros
    [1281] Acros, rødt filter
    [1282] Acros, gult filter
    [1283] Acros, grønt filter
   *[other] Monokrom
}
fuji-white-balance = { $value ->
    [0] Auto
    [1] Auto (hvidprioritet)
    [2] Auto (stemningsprioritet)
    [256] Dagslys
    [512] Overskyet
    [768] Dagslysfluorescerende
    [769] Dagshvidt fluorescerende
    [770] Hvidt fluorescerende
    [771] Varmhvidt fluorescerende
    [772] Varmhvidt fluorescerende til stuer
    [1024] Glødelampe
    [1280] Blitz
    [1536] Under vand
    [3840] Brugerdefineret 1
    [3841] Brugerdefineret 2
    [3842] Brugerdefineret 3
    [3843] Brugerdefineret 4
    [3844] Brugerdefineret 5
    [4080] Farvetemperatur
   *[other] Andet
}
fuji-white-balance-kelvin = { $kelvin } K
fuji-dynamic-range = DR{ $percent }
fuji-dynamic-range-auto = Auto (DR{ $percent })
fuji-dynamic-range-setting = { $value ->
    [0] Auto
    [256] Standard (DR100)
    [512] Bred 1 (DR230)
    [513] Bred 2 (DR400)
   *[other] Andet
}
fuji-effect = { $value ->
    [0] Fra
    [32] Svag
    [64] Stærk
   *[other] Andet
}
fuji-grain-size = { $value ->
    [16] lille
    [32] stor
   *[other] andet
}
fuji-grain = { $strength }, { $size }
fuji-shutter-type = { $value ->
    [0] Mekanisk
    [1] Elektronisk
    [2] Elektronisk (lange eksponeringer)
    [3] Elektronisk første lukkergardin
   *[other] Andet
}
fuji-drive-mode = { $value ->
    [0] Enkeltbillede
    [1] Serie, lav hastighed
    [2] Serie, høj hastighed
   *[other] Andet
}
fuji-focus-mode = { $value ->
    [0] Manuel
    [1] AF-S
    [2] AF-C
    [100] Auto
    [101] Manuel
   *[other] Andet
}
fuji-stabilisation = { $kind } ({ $mode })
fuji-stabilisation-kind = { $value ->
    [0] Ingen
    [1] Optisk
    [2] Sensorforskydning
    [3] Objektiv
    [258] Sensorforskydning og digital
    [512] Digital
   *[other] Andet
}
fuji-stabilisation-mode = { $value ->
    [0] Fra
    [1] til, kontinuerlig
    [2] til, ved optagelse
   *[other] Andet
}
light-source = { $value ->
    [1] Dagslys
    [2] Lysstofrør
    [3] Glødelampe
    [4] Blitz
    [9] Godt vejr
    [10] Overskyet
    [11] Skygge
    [12] Dagslyslysstofrør
    [13] Dagshvidt lysstofrør
    [14] Koldhvidt lysstofrør
    [15] Hvidt lysstofrør
    [16] Varmhvidt lysstofrør
    [17] Standardlys A
    [18] Standardlys B
    [19] Standardlys C
    [20] D55
    [21] D65
    [22] D75
    [23] D50
    [24] Studieglødelampe
   *[other] Andet
}
white-balance-auto = Auto
white-balance-manual = Manuel
white-balance-manual-with-light = Manuel ({ $light })
# Blitzbeskrivelsen bygges af disse dele, sammensat med list-separator.
flash-none = Ingen blitzfunktion
flash-fired = Blitz udløst
flash-not-fired = Blitz ikke udløst
flash-forced-on = tvunget til
flash-forced-off = tvunget fra
flash-auto = autotilstand
flash-return-not-detected = returlys ikke registreret
flash-return-detected = returlys registreret
flash-red-eye = rødøjereduktion

## Objektiv

lens-focal-single = { $mm } mm
lens-focal-range = { $min }–{ $max } mm
lens-aperture-single = f/{ $f }
lens-aperture-range = f/{ $min }–{ $max }
lens-range = { $focal } { $aperture }

## Dato og klokkeslæt

# $date: "tirsdag 14. maj 2024"; $time: "18.32"
# $weekday: "tirsdag" (ICU); $date: "14. maj 2024" (ICU)
date-with-weekday = { $weekday } { $date }
date-time = { $date }, { $time }
date-time-zone = { $datetime } ({ $zone })
zone-utc = UTC
# $offset: "+1", "+5:30", "−3"
zone-offset = UTC{ $offset }
gps-date-time = { $date }, { $time } UTC
detail-zone-from-gps = Tidszonen er udledt af GPS-uret

## Placering

# $lat og $lon er uden fortegn; halvkuglen kommer fra vælgeren.
coordinates = { $lat }° { $latRef ->
    [south] S
   *[north] N
}, { $lon }° { $lonRef ->
    [west] V
   *[east] Ø
}
altitude = { $ref ->
    [above] { $m } m over havets overflade
    [below] { $m } m under havets overflade
   *[sea] Ved havets overflade
}
compass-point = { $point ->
    [n] nord
    [nne] nord-nordøst
    [ne] nordøst
    [ene] øst-nordøst
    [e] øst
    [ese] øst-sydøst
    [se] sydøst
    [sse] syd-sydøst
    [s] syd
    [ssw] syd-sydvest
    [sw] sydvest
    [wsw] vest-sydvest
    [w] vest
    [wnw] vest-nordvest
    [nw] nordvest
   *[nnw] nord-nordvest
}
# $point: fra compass-point; $degrees: "287"
facing = Mod { $point } ({ $degrees }°)
detail-true-north = I forhold til geografisk nord
detail-magnetic-north = I forhold til magnetisk nord
accuracy = ±{ $m } m
distance-km = { $km } km
place = { $city }, { $country }
place-city-only = { $city }
# Mere end 30 km fra den fundne by.
place-near = nær { $place }
place-detail = { $areas } · { $distance } fra { $city }
place-detail-distance = { $distance } fra { $city }

## Billede

dimensions = { $width } × { $height }
dimensions-megapixels = { $width } × { $height } ({ $mp } MP)
encoding-bit-depth = { $bits } bit
# $format: "8 bit YCbCr 4:2:0"
encoding-progressive = { $format }, progressiv
encoding-baseline = { $format }, baseline
encoding-hierarchical = { $format }, hierarkisk
# machine-drafted, needs native review
encoding-alpha = { $format } { $alpha ->
    [premultiplied] med præmultipliceret alfa
    [transparency] med gennemsigtighed
   *[alpha] med alfa
}
# [Lab], [Palette] and the variants after them: machine-drafted, needs native review
colour-model = { $model ->
    [YCbCr] YCbCr
    [RGB] RGB
    [Greyscale] Gråtoner
    [CMYK] CMYK
    [YCCK] YCCK
    [Lab] L*a*b*
    [Palette] Indekserede farver
    [Multichannel] Flerkanals
    [Duotone] Duotone
    [YCgCo] YCgCo
    [XYB] XYB
    [YC] Luminans/krominans (YC)
    [Bayer] Bayer-raw
    [X-Trans] X-Trans-raw
    [CFA] Raw med farvefiltermatrix
    [LinearRaw] Lineær raw
    [RGBE] RGBE (delt eksponent)
    [XYZE] XYZE (delt eksponent)
    [R] Én kanal (R)
    [RG] To kanaler (RG)
   *[other] { $model }
}
adobe-rgb-from-exif = Adobe RGB (fra EXIF-tag; ingen indlejret profil)
orientation = { $value ->
    [1] Opret (ingen rotation nødvendig)
    [2] Spejlvendt vandret (anvendt)
    [3] Roteret 180° (anvendt)
    [4] Spejlvendt lodret (anvendt)
    [5] Spejlvendt vandret og roteret 90° mod uret (anvendt)
    [6] Roteret 90° med uret (anvendt)
    [7] Spejlvendt vandret og roteret 90° med uret (anvendt)
    [8] Roteret 90° mod uret (anvendt)
   *[other] Ukendt ({ $value })
}
# Kort form til advarsler.
orientation-short = { $value ->
    [1] Opret
    [2] Spejlvendt vandret
    [3] Roteret 180°
    [4] Spejlvendt lodret
    [5] Spejlvendt vandret og roteret 90° mod uret
    [6] Roteret 90° med uret
    [7] Spejlvendt vandret og roteret 90° med uret
    [8] Roteret 90° mod uret
   *[other] værdi { $value }
}

## Fil

file-size-bytes = { $num ->
    [one] { $num } byte
   *[other] { $num } byte
}
# Decimalenheder (1 KB = 1000 byte).
file-size-unit = { $num } { $unit ->
    [kb] KB
    [mb] MB
    [gb] GB
    [tb] TB
   *[pb] PB
}

## Rettigheder

rating-rejected = Afvist
# $stars: "★★★★☆"
rating-stars = { $stars } ({ $n } af 5)

## Indholdslegitimation

c2pa-present = Til stede (ikke verificeret)
c2pa-detail = Filen indeholder et C2PA-manifest; signaturen er ikke kontrolleret.

## Værdier i ICC-profilhovedet

icc-class = { $class ->
    [scnr] Inputenhed
    [mntr] Skærmenhed
    [prtr] Outputenhed
    [link] Enhedslink
    [spac] Farverum
    [abst] Abstrakt
    [nmcl] Navngiven farve
   *[other] Ukendt
}
icc-intent = { $intent ->
    [0] Perceptuel
    [1] Relativ kolorimetrisk
    [2] Mætning
    [3] Absolut kolorimetrisk
   *[other] Ukendt
}
icc-platform = { $platform ->
    [APPL] Apple
    [MSFT] Microsoft
    [SGI] Silicon Graphics
    [SUNW] Sun Microsystems
    [none] Ikke angivet
   *[other] Ukendt
}
icc-coded = { $name } ({ $code })

# machine-drafted, needs native review
label-modality = Modalitet
label-body-part = Kropsdel
label-study = Undersøgelse
label-series = Serie
label-slice-thickness = Snittykkelse
label-pixel-spacing = Pixelafstand
label-imager-pixel-spacing = Pixelafstand ved detektoren
label-frames = Billeder
label-transfer-syntax = Overførselssyntaks
label-document = Dokument
label-document-type = Dokumenttype
medical-spacing = { $row } × { $column } mm
medical-mm = { $mm } mm
medical-lossy = { $name } (komprimering med tab)
detail-pixel-spacing = Afstand mellem pixelcentre i patienten (rækker × kolonner).
# machine-drafted, needs native review
# Pixel spacing of an industrial inspection image (DICONDE, CT of an object).
detail-pixel-spacing-object = Afstand mellem pixelcentre i objektet (rækker × kolonner).
detail-imager-pixel-spacing = Målt ved detektoren, ikke korrigeret for forstørrelse: afstande i patienten er som regel mindre.

## Industriel inspektion (DICONDE)

# machine-drafted, needs native review
label-nde-component-shape = Komponentform
label-nde-component-manufacturer = Komponentproducent
label-nde-material = Materiale
label-nde-thickness = Materialetykkelse
label-nde-pipe-diameter = Rørdiameter
label-nde-source = Kilde
label-nde-environment = Omgivelser
label-nde-iqi = Billedkvalitetsindikator
label-nde-indications = Indikationer
label-nde-standard = Standard

## Forskningsvolumener (NIfTI, Analyze, NRRD, MetaImage)

# machine-drafted, needs native review
label-voxel-size = Voxelstørrelse
label-data-type = Datatype
label-time-points = Tidspunkter
label-orientation-source = Orientering fra
label-intent = Værdier
label-compression = Komprimering
volume-voxel-size = { $x } × { $y } × { $z } mm
volume-orient-sform = NIfTI sform
volume-orient-qform = NIfTI qform
volume-orient-analyze = Analyze-header (ikke pålidelig)
volume-orient-space = NRRD space directions
volume-orient-transform = MetaImage-transformation
volume-orient-spacings = Kun voxelafstand
volume-orient-none = Ikke angivet
volume-orient-vox2ras = FreeSurfer vox2ras
volume-orient-mgh-default = FreeSurfer-standard (ikke pålidelig)
volume-orient-minc = MINC-dimensionscosinusser
volume-orient-object = Objektkoordinater (ingen patientorientering)
volume-content-projections = Projektioner (rå scanning)
volume-content-reconstruction = Rekonstrueret volumen
volume-coords-rectilinear = Retlinet (ujævn afstand, vist jævnt)
# Scan parameters and tags a FreeSurfer MGH file keeps after its voxels.
label-volume-tr = Repetitionstid
label-volume-te = Ekkotid
label-volume-ti = Inversionstid
label-volume-flip = Flipvinkel
label-volume-fov = Synsfelt
label-volume-field = Feltstyrke
label-volume-pedir = Fasekodning
label-volume-cmdline = Kommandolinje
label-volume-xform = Talairach-transformation
label-volume-content = Indhold
label-volume-projections = Projektioner
label-volume-angles = Rotationsområde
label-volume-angle-step = Rotationstrin
label-volume-kv = Rørspænding
label-volume-current = Rørstrøm
label-volume-power = Kildeeffekt
label-volume-exposure = Eksponering
label-volume-sod = Kilde til objekt
label-volume-odd = Objekt til detektor
label-volume-sdd = Kilde til detektor
label-volume-magnification = Optisk forstørrelse
label-volume-binning = Binning
label-volume-objective = Objektiv
label-volume-filter = Kildefilter
label-volume-camera = Detektor
label-volume-instrument = Instrument
label-volume-software = Software
label-volume-date = Optaget
label-volume-ring = Korrektion af ringartefakter
label-volume-smoothing = Udglatning
label-volume-coords = Gitter
# machine-drafted, needs native review
# An eye scan (OCT): which eye.
label-volume-eye = Øje
volume-eye-right = Højre (OD)
volume-eye-left = Venstre (OS)
detail-volume-unreliable = Filen angiver ikke, hvor dens akser peger i patienten: venstre og højre kan være byttet om, så der vises ingen orienteringsetiketter.
# machine-drafted, needs native review
# An industrial CT volume: its axes are the scanner's, there is no patient.
detail-volume-object = Akserne er scannerens, ikke en patients, så der vises ingen orienteringsetiketter.

## Videnskabelige billeder (FITS, XISF)

label-object = Objekt
label-telescope = Teleskop
label-instrument = Instrument
label-filter = Filter
label-observed = Observeret
label-exposure-time = Eksponering
label-sample = Prøver
label-unit = Enhed
label-hdu = Udvidelse
label-centre = Centrum
label-centre-deg = Centrum (grader)
label-frame = Koordinatsystem
# machine-drafted, needs native review
science-frame-galactic = Galaktisk
science-frame-ecliptic = Ekliptisk
label-projection = Projektion
label-pixel-scale = Pixelskala
science-seconds = { $seconds } s
science-arcsec-px = { $arcsec }″ pr. pixel
label-distortion = Forvrængning
label-wcs = Himmelkoordinater
science-wcs-unsupported = Understøttes ikke: { $reason }
# machine-drafted, needs native review
science-wcs-approximate = Omtrentlig (kun pegeretning, uden forvrængning)
label-observer = Observatør
label-bayer = Bayer-mønster
label-capture-start = Optagelsesstart (UTC)
label-capture-end = Optagelsesslut (UTC)
label-frame-rate = Billedfrekvens
science-fps = { $fps } billeder/s
science-frames-cut = { $frames } af { $declared }
# The HDF5 dataset or NetCDF variable shown.
label-dataset = Datasæt
label-pixel-size = Pixelstørrelse
# "0.25 nm per pixel"; $size may be "0.25 × 0.5".
science-pixel-size = { $size } { $unit } pr. pixel
# Microscopy (DigitalMicrograph, MRC, Gwyddion, Igor).
label-microscope = Mikroskop
label-voltage = Accelerationsspænding
label-magnification = Forstørrelse
label-imaging-mode = Tilstand
label-detector = Detektor
label-acquired = Optaget
label-field-of-view = Synsfelt
science-kilovolts = { $kv } kV
science-magnification = { $value }×
label-vendor = Producent
label-scanner = Scanner
label-objective = Objektiv
label-pyramid-levels = Niveauer
# machine-drafted, needs native review
label-emissivity = Emissivitet
label-object-distance = Objektafstand
label-reflected-temperature = Reflekteret temperatur
label-atmospheric-temperature = Atmosfærisk temperatur
label-humidity = Relativ luftfugtighed
label-raw-range = Råt interval
label-temperature-range = Temperaturinterval
thermal-temperature = { $value } { $unit }
thermal-metres = { $value } m
thermal-percent = { PERCENT($value) }
thermal-range = { $low } – { $high }
slide-levels = { $count }: { $sizes }
# X-ray diffraction: the experiment geometry from a detector frame's header.
label-wavelength = Bølgelængde
# Ångström, e.g. "0.97625 Å".
xtal-angstrom = { $value } Å
label-photon-energy = Fotonenergi
xtal-kev = { $value } keV
# Sample to detector distance.
label-detector-distance = Detektorafstand
xtal-mm = { $value } mm
# Where the direct beam meets the detector, in pixels from the image's top-left corner.
label-beam-centre = Strålecentrum
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
xtal-oscillation-axis = { $start }° + { $width }° om { $axis }
# The count at which a pixel saturates.
label-overload = Overløbsværdi
# The finest lattice spacing (d) recorded at the detector's edge and in its corners.
label-resolution = Opløsning
xtal-resolution = { $edge } Å ved kanten, { $corner } Å i hjørnerne
label-geometry = Geometri
xtal-curved = Krum billedplade: ingen q, d eller opløsningsringe
xtal-incomplete = Ufuldstændig header: ingen q, d eller opløsningsringe
# All-fields group title: "FITS header".
fields-science-header = { $format }-header
# machine-drafted, needs native review
fields-asdf-tree = ASDF-træ
label-asdf-model = Datamodel
label-asdf-version = Filversion
label-asdf-standard = ASDF-standard
label-asdf-library = Skrevet af
# machine-drafted, needs native review
# All-fields group title for a CERN ROOT file's list of keys (objects).
fields-root-keys = ROOT-nøgler
# CERN ROOT files: the ROOT version that wrote the file (e.g. "6.28/04"), its compression,
# the number of keys (stored objects), and the object shown: class (e.g. "TH2F"), its path in
# the file, title, number of entries and sum of weights.
label-root-version = ROOT-version
label-root-compression = Komprimering
label-root-keys = Nøgler
label-root-class = Klasse
label-root-name = Navn
label-root-title = Titel
label-root-entries = Indgange
label-root-sum-weights = Sum af vægte
# A graph's number of points.
label-root-points = Punkter
# $axis is "x", "y" or "z".
label-root-axis = Akse { $axis }
# $quantity is the axis title with its unit, e.g. "Energy [GeV]"; $min and $max the axis
# limits.
root-axis-fixed = { $quantity }: { $bins ->
        [one] { $bins } bin
       *[other] { $bins } bins
    }, { $min } til { $max }
root-axis-variable = { $quantity }: { $bins ->
        [one] { $bins } variabel bin
       *[other] { $bins } variable bins
    }, { $min } til { $max }
label-root-mean-rms = Middelværdi og RMS { $axis }
root-mean-rms = { $mean } ± { $rms }
# Trees (event data) are listed only, with their entry counts.
label-root-trees = Træer
root-tree = { $name } ({ $entries ->
        [one] { $entries } indgang
       *[other] { $entries } indgange
    })
# machine-drafted, needs native review
# CASA images: what the image is (from its directory name), its restoring beam (major and
# minor axis, position angle), the beams of its planes when they differ, the rest frequency,
# spectral reference frame (e.g. "LSRK") and velocity definition of the frequency labels,
# the default mask (e.g. "mask0") and the casacore storage manager with its byte order.
label-casa-type = Billedtype
casa-type-image = Restaureret billede
casa-type-residual = Residual
casa-type-model = Model
casa-type-pb = Primær stråle
casa-type-mask = Clean-maske
casa-type-psf = Punktspredningsfunktion
casa-type-sumwt = Sum af vægte
casa-type-weight = Vægte
label-casa-beam = Restaureringsstråle
casa-beam = { $major }″ × { $minor }″, PV { $pa }°
label-casa-plane-beams = Stråler pr. plan
casa-plane-beams = { $count ->
        [one] { $count } stråle
       *[other] { $count } stråler
    }, storakse { $min }″ til { $max }″
label-casa-rest-frequency = Hvilefrekvens
casa-ghz = { $ghz } GHz
label-casa-spectral-frame = Spektralt referencesystem
label-casa-velocity = Hastighedsdefinition
casa-velocity-radio = Radio
casa-velocity-optical = Optisk
casa-velocity-relativistic = Relativistisk
label-casa-mask = Standardmaske
label-casa-storage = Lagring
casa-storage-big = { $manager }, big-endian
casa-storage-little = { $manager }, little-endian
# machine-drafted, needs native review
# Unstructured meshes: the mesh's name in the file, the convention describing it (UGRID,
# ICON, MPAS), its counts, face shapes, coordinates and where the values sit.
label-ugrid-name = Net
label-ugrid-convention = Konvention
label-ugrid-nodes = Knuder
label-ugrid-faces = Flader
label-ugrid-edges = Kanter
label-ugrid-shapes = Fladeformer
label-ugrid-coordinates = Koordinater
label-ugrid-values = Værdier på
label-ugrid-split = Delt til tegning
# $n is the formatted count, $count the number (for the plural).
ugrid-triangles = { $count ->
    [one] { $count } trekant
   *[other] { $count } trekanter
}
ugrid-quads = { $count ->
    [one] { $count } firkant
   *[other] { $count } firkanter
}
# Faces of five or more nodes; $max is the most nodes of one face.
ugrid-polygons = { $count ->
    [one] { $count } polygon med { $max } knuder
   *[other] { $count } polygoner med op til { $max } knuder
}
ugrid-lonlat = Længde- og breddegrad
ugrid-projected = Projiceret x og y
ugrid-projected-unit = Projiceret x og y ({ $unit })
ugrid-on-faces = Flader (tegnet ensfarvet)
ugrid-on-nodes = Knuder (interpoleret hen over fladerne)
# A mesh file without data variables: the image shows how many nodes each face has.
ugrid-grid-only = Ingen: billedet viser knuderne pr. flade
# Faces cut at the antimeridian or closed to a pole so they draw on a flat map.
ugrid-split = { $count ->
    [one] { $count } flade ved antimeridianen eller en pol
   *[other] { $count } flader ved antimeridianen eller en pol
}
# machine-drafted, needs native review
# Slide annotations (NDP.view, Aperio ImageScope, ASAP sidecar files).
label-annotation-file = Fil
# "slide.xml (Aperio ImageScope)"
annotation-source = { $file } ({ $format })
label-annotation-shapes = Figurer
label-annotation-groups = Grupper
# Shapes or points over the reader's limits, left out.
label-annotation-skipped = Udeladt
annotation-polygon = Polygon
annotation-polyline = Linje
annotation-rectangle = Rektangel
annotation-ellipse = Ellipse
annotation-point = Punkt
annotation-points = Punkter
annotation-ruler = Lineal
annotation-arrow = Pil
# The list of annotations is cut short: "and 1,200 more".
annotation-more = og { $count } mere
# All-fields group title: "Annotations in slide.xml".
fields-annotations-header = Annoteringer i { $file }
label-drawing-version = Version
label-drawing-units = Enheder
label-drawing-extents = Udstrækning
label-drawing-sheet = Ark
label-drawing-layouts = Layouts
label-drawing-layers = Lag
label-drawing-entities = Objekter
label-drawing-codepage = Tegntabel
label-drawing-missing-fonts = Erstattede skrifttyper
# machine-drafted, needs native review
# A font file the drawing names that was not found: the styles naming it, and the font drawing its text instead.
drawing-missing-font = { $file } (stile { $styles }), tegnet med { $substitute }
# A Big Font (double-byte text) that was not found; its characters come from other fonts.
drawing-missing-bigfont = { $file } (stile { $styles }), tegnet med reserveskrifttyper
# Survey data (LandXML).
label-survey-crs = Koordinatsystem
survey-crs-none = Ikke angivet
label-survey-project = Projektnavn
label-survey-application = Skrevet af
label-survey-surface = Terrænmodel
# A TIN surface: its name, counts, plan and sloped areas, the volume above its lowest point, its elevations and the contour interval drawn; unit is "m" or "ft" (areas in square units, volume in cubic units).
survey-surface = { $name }: { $points ->
        [one] { $points } punkt
       *[other] { $points } punkter
    }, { $triangles ->
        [one] { $triangles } trekant
       *[other] { $triangles } trekanter
    }; areal i plan { $area } { $unit }², fladeareal { $area3d } { $unit }²; { $volume } { $unit }³ over det laveste punkt; koter { $zmin }–{ $zmax } { $unit }; højdekurver for hver { $interval } { $unit }
label-survey-alignment = Linjeføring
# An alignment: its name, length and first and last stations ("1+234.50").
survey-alignment = { $name }: { $length } { $unit }, stationering { $start } til { $end }
# SHX shape fonts found and used for the drawing's text.
label-drawing-shx-fonts = SHX-skrifttyper
label-drawing-warnings = Problemer
label-plot-paper = Papir
# "2 of 5": the page shown and how many the plot has.
plot-page-of = { $page } af { $pages }
label-plot-page = Side
label-plot-pens = Anvendte penne
# The printer language a plot came wrapped in, e.g. "PCL 5".
label-plot-job = Printerjob
# What a plot draws: strokes, fills and labels.
label-plot-items = Tegnede elementer
# Width × height of what is drawn, in drawing units: "1,250 × 800 mm".
drawing-extents = { $width } × { $height } { $unit }
drawing-extents-unitless = { $width } × { $height }
drawing-model-space = Model
# "24, 3 off or frozen"
drawing-layers-off = { $num }, { $off } slået fra eller frosset
# DXF $INSUNITS.
drawing-units = { $code ->
    [1] Tommer
    [2] Fod
    [3] Engelske mil
    [4] Millimeter
    [5] Centimeter
    [6] Meter
    [7] Kilometer
    [8] Mikrotommer
    [9] Tusindedele tommer
    [10] Yards
    [11] Ångström
    [12] Nanometer
    [13] Mikrometer
    [14] Decimeter
    [15] Dekameter
    [16] Hektometer
    [17] Gigameter
    [18] Astronomiske enheder
    [19] Lysår
    [20] Parsec
    [21] Amerikanske landmålerfod
   *[other] Uden enhed
}
# All-fields group title: "DXF entities".
fields-drawing-entities = { $format }-objekter

## IC-layouts (GDSII)

# machine-drafted, needs native review
label-layout-library = Bibliotek
label-layout-units = Enheder
# The database unit (the grid coordinates are stored in) and the user unit, with SI symbols:
# "1 nm database unit, 1 µm user unit".
layout-units = { $db } databaseenhed, { $user } brugerenhed
# The top cell shown (each top cell is a page).
label-layout-cell = Celle
label-layout-cells = Celler
# Cells no other cell places.
label-layout-top-cells = Topceller
# "TOP, TEST, PAD and 12 more"
layout-top-cells-more = { $names } og { $more } til
# Layer/datatype pairs.
label-layout-layers = Lag
# Polygons, paths and boxes, as stored (each cell once).
label-layout-shapes = Former
label-layout-texts = Etiketter
# Cell references and the placements they make: "1,204 (88,320 placements)".
label-layout-references = Referencer
layout-references = { $refs } ({ $placements } { $placements ->
    [one] placering
   *[other] placeringer
})
# Shapes with every cell placement expanded.
label-layout-flat-shapes = Former ved udfladning
label-layout-bounds = Afgrænsning
# Corners of what the cell draws, micrometres: "-12.5, 0 to 2,900, 3,500 µm".
layout-bounds = { $x0 }, { $y0 } til { $x1 }, { $y1 } µm

## Printplader og diagrammer (Gerber, Excellon, KiCad)

# machine-drafted, needs native review
group-board = Printplade
group-schematic = Diagram
# The board view shown (its layers, its top or bottom), or the schematic sheet.
label-pcb-view = Visning
label-pcb-project = Projekt
label-pcb-revision = Revision
label-pcb-date = Dato
# The CAD program that wrote the file.
label-pcb-generator = Lavet med
# Width × height of the board outline: "100 × 80 mm".
label-pcb-size = Printpladens størrelse
label-pcb-thickness = Tykkelse
label-pcb-copper-layers = Kobberlag
label-pcb-sheets = Ark
# The board's layers or files, each with its role: "board-F_Cu.gbr (Top copper)".
label-pcb-layers = Lag
pcb-layer-role = { $name } ({ $role })
label-pcb-components = Footprints
label-pcb-symbols = Symboler
label-pcb-nets = Net
label-pcb-tracks = Baner
label-pcb-wires = Ledninger
label-pcb-vias = Vias
# Drilled holes and how many drill sizes: "1,204 in 9 sizes".
label-pcb-holes = Huller
pcb-holes = { $holes } i { $sizes } { $sizes ->
    [one] størrelse
   *[other] størrelser
}
label-pcb-apertures = Aperturer
# What a board layer or file is, shown after its name.
pcb-role-inner-copper = Indre kobber { $n }
pcb-role-top-copper = Kobber overside
pcb-role-bottom-copper = Kobber underside
pcb-role-top-solder-mask = Loddemaske overside
pcb-role-bottom-solder-mask = Loddemaske underside
pcb-role-top-silkscreen = Påtryk overside
pcb-role-bottom-silkscreen = Påtryk underside
pcb-role-top-paste = Pasta overside
pcb-role-bottom-paste = Pasta underside
pcb-role-top-solder-paste = Loddepasta overside
pcb-role-bottom-solder-paste = Loddepasta underside
pcb-role-top-adhesive = Lim overside
pcb-role-bottom-adhesive = Lim underside
pcb-role-top-courtyard = Courtyard overside
pcb-role-bottom-courtyard = Courtyard underside
pcb-role-top-fabrication = Fremstilling overside
pcb-role-bottom-fabrication = Fremstilling underside
pcb-role-board-outline = Printpladens omrids
pcb-role-plated-drill = Gennempletterede boringer
pcb-role-non-plated-drill = Ikke-gennempletterede boringer
pcb-role-drill = Boringer
pcb-role-drill-holes = Borehuller
pcb-role-mechanical = Mekanisk
pcb-role-margin = Margen
pcb-role-drawings = Tegninger
pcb-role-comments = Kommentarer
pcb-role-engineering-change-1 = Teknisk ændring 1
pcb-role-engineering-change-2 = Teknisk ændring 2
pcb-role-vias = Vias
pcb-role-other = Andet
pcb-role-unknown = Ukendt
label-pcb-macros = Aperturmakroer

## Spektre (JCAMP-DX, SPC, mzML, mzXML, Bruker NMR)

# machine-drafted, needs native review
group-spectrum = Spektrum
label-spectrum-format = Format
label-spectrum-technique = Teknik
label-spectrum-title = Titel
# The page shown: "3 of 1,204: Scan 1532".
label-spectrum-page = Side
spectrum-page-of = { $page } af { $pages }: { $name }
# An axis with its unit: "Wavenumber (cm⁻¹)".
spectrum-axis = { $title } ({ $unit })
label-spectrum-x = X-akse
label-spectrum-y = Y-akse
label-spectrum-nucleus = Kerne
label-spectrum-frequency = Spektrometerfrekvens
label-spectrum-solvent = Opløsningsmiddel
label-spectrum-pulse-program = Pulsprogram
label-spectrum-spectra = Spektre
label-spectrum-points = Punkter
label-spectrum-instrument = Instrument
# Who or what lab measured it.
label-spectrum-origin = Oprindelse
label-spectrum-date = Målt
# How Specular made the page from the raw data (an FID transformed, phased automatically).
label-spectrum-processing = Behandling
spectrum-technique-ir = Infrarød
spectrum-technique-raman = Raman
spectrum-technique-uv-vis = UV-Vis
spectrum-technique-fluorescence = Fluorescens
spectrum-technique-nmr = NMR
spectrum-technique-ms = Massespektrum
spectrum-technique-chromatogram = Kromatogram
spectrum-technique-other = Spektrum

## 3D-modeller (STL, OBJ, PLY, glTF, 3MF)

# machine-drafted, needs native review
group-mesh = 3D-model
label-triangles = Trekanter
label-vertices = Hjørnepunkter
label-materials = Materialer
label-textures = Teksturer
label-up-axis = Opad-akse
# Textures the model uses, some of which could not be read: "3 (1 missing)".
mesh-textures-missing = { $count } ({ $missing } mangler)

## Molekylstrukturer (PDB, mmCIF, CIF, SDF, XYZ)

# machine-drafted, needs native review
group-structure = Struktur
# The entry's identifier: a PDB ID such as 1CRN, or the CIF data block's name.
label-structure-id = Post
label-structure-classification = Klassifikation
# How the structure was determined: X-ray diffraction, electron microscopy, solution NMR…
label-structure-method = Metode
# The crystallographic or cryo-EM resolution, in ångströms.
label-structure-resolution = Opløsning
structure-resolution = { $value } Å
label-structure-deposited = Deponeret
label-structure-formula = Formel
label-structure-chains = Kæder
# Residues (amino acids, nucleotides) in the polymer chains.
label-structure-residues = Residuer
label-structure-atoms = Atomer
# Atoms after the unit cell was filled by symmetry, and those the file lists: "96 (12 in the asymmetric unit)".
structure-atoms-cell = { $count } ({ $asymmetric } i den asymmetriske enhed)
label-structure-bonds = Bindinger
# Models in the file (an NMR ensemble), of which the first is shown.
label-structure-models = Modeller
# Molecules (SDF records) or data blocks in the file, of which the first is shown.
label-structure-records = Poster
structure-first-of = { $count } (den første vises)
# Small molecules bound to the polymer, by their three-letter codes.
label-structure-ligands = Ligander
# A list cut short: "HEM, NAG, SO4 and 9 more".
structure-ligands-more = { $list } og { $more } til
label-structure-cell = Enhedscelle
# Unit cell edge lengths (Å) and angles (degrees).
structure-cell = a = { $a } Å, b = { $b } Å, c = { $c } Å, α = { $alpha }°, β = { $beta }°, γ = { $gamma }°
label-structure-space-group = Rumgruppe
label-structure-secondary = Sekundær struktur
structure-ss-file = Fra filen
structure-ss-computed = Beregnet ud fra hovedkæden

## HDF4 og HDF-EOS2

# machine-drafted, needs native review
group-hdf4 = HDF4
# The HDF library version that wrote the file, e.g. "NCSA HDF Version 4.2 Release 15".
label-hdf4-library = Bibliotek
# Counts of the file's scientific datasets, raster images, groups (Vgroups) and tables (Vdatas).
label-hdf4-datasets = Datasæt
label-hdf4-images = Rasterbilleder
label-hdf4-vgroups = Grupper
label-hdf4-vdatas = Tabeller
# HDF-EOS: NASA's Earth Observing System conventions on top of HDF4 (MODIS, ASTER, AIRS…).
group-hdf-eos = HDF-EOS
label-eos-version = Version
# The grids, swaths and point structures the file defines.
label-eos-structures = Strukturer
eos-grids = { $count ->
    [one] { $count } gitter ({ $names })
   *[other] { $count } gitre ({ $names })
}
eos-swaths = { $count ->
    [one] { $count } scanningsstribe ({ $names })
   *[other] { $count } scanningsstriber ({ $names })
}
eos-points = { $count ->
    [one] { $count } punktstruktur
   *[other] { $count } punktstrukturer
}
# The grid or swath the dataset shown belongs to.
label-eos-structure = Struktur
# The grid's map projection as its GCTP name, e.g. GCTP_SNSOID (sinusoidal).
label-eos-projection = Projektion
label-eos-grid-size = Gitterstørrelse
# Columns × rows.
eos-grid-size = { $width } × { $height }
# The grid's upper-left and lower-right corners.
label-eos-corners = Hjørner
# The product's short name from the inventory metadata, e.g. MOD09A1.
label-eos-product = Produkt
# When the data was acquired.
label-eos-time = Tid
eos-time = { $begin } til { $end }

## Planetbilleder (VICAR, PDS3, PDS4: missionsdata fra NASA og ESA)

# machine-drafted, needs native review
group-planetary = Planetbillede
# The body observed, e.g. MARS, Ganymede.
label-planetary-target = Mål
label-planetary-mission = Mission
# The spacecraft, rover or lander carrying the instrument.
label-planetary-spacecraft = Rumfartøj
label-planetary-start = Starttid
label-planetary-stop = Sluttid
# The product's identifier in the archive.
label-planetary-product = Produkt-id
# The map projection of a map-projected image, as the label names it.
label-planetary-projection = Kortprojektion
label-planetary-resolution = Opløsning
planetary-metres-per-pixel = { $metres } m pr. pixel
planetary-pixels-per-degree = { $pixels } pixel pr. grad
# Latitude and longitude of the map projection's centre.
label-planetary-centre = Projektionscentrum
planetary-lat-lon = { $lat }°, { $lon }°
# The latitudes and longitudes the map covers.
label-planetary-coverage = Dækker
planetary-coverage = breddegrad { $south }° til { $north }°, længdegrad { $west }° til { $east }°
# The body's equatorial radius the map uses.
label-planetary-radius = Legemets radius
planetary-km = { $km } km

## OMERO (billeder åbnet fra en OMERO-server eller Image Data Resource)

# machine-drafted, needs native review
group-omero = OMERO
label-omero-server = Server
# $server the server's address; $user the user name signed in with.
omero-server-user = { $server } (som { $user })
omero-server-public = { $server } (offentlig)
label-omero-image = Billede
# $name the image's name; $id its number on the server.
omero-image = { $name } (ID { $id })
# The project and dataset the image is in, as "Project › Dataset".
label-omero-project = Projekt
# The screen, plate and well, as "Screen › Plate › B4".
label-omero-screen = Screen
label-omero-owner = Ejer
# $owner the image's owner; $group the OMERO group it belongs to.
omero-owner-group = { $owner } ({ $group })
label-omero-acquired = Optaget
label-omero-dimensions = Dimensioner
omero-size = { $width } × { $height }
omero-z = { $count ->
    [one] { $count } Z-plan
   *[other] { $count } Z-planer
}
omero-c = { $count ->
    [one] { $count } kanal
   *[other] { $count } kanaler
}
omero-t = { $count ->
    [one] { $count } tidspunkt
   *[other] { $count } tidspunkter
}
# The physical size of a pixel along X, Y (and Z).
label-omero-physical = Pixelstørrelse
label-omero-channels = Kanaler
# A channel not shown in the server's rendering.
omero-channel-hidden = { $channel } (skjult)
# How the pixels are fetched from the server: rendered tiles (JPEG) or the whole plane (PNG).
label-omero-fetched = Hentet som
omero-fetched-tiles = { $encoding }-fliser på { $width } × { $height }, { $levels ->
    [one] { $levels } niveau
   *[other] { $levels } niveauer
}
omero-fetched-whole = Hele planer som { $encoding }

## Reflektanstransformationsbilleder (PTM, RTI, Relight)

# machine-drafted, needs native review
group-rti = Reflektanstransformation
# The reflectance model the coefficients belong to: PTM (polynomial), HSH (hemispherical harmonics), RBF…
label-rti-basis = Basis
label-rti-order = Orden
# Coefficients per colour channel.
label-rti-terms = Koefficienter
# How colour is stored: LRGB (one colour, varying luminance), RGB, YCC…
label-rti-colour = Farvelayout
# Coefficient images stored in the file.
label-rti-planes = Planer
# The byte-to-coefficient mapping the file states, per coefficient.
label-rti-scale = Skala
label-rti-bias = Forskydning

## Punktskyer

# machine-drafted, needs native review
group-points = Punktsky
label-points = Punkter
# Points read of those in the file, when a very large cloud was evenly thinned: "32,000,000 of 120,000,000".
points-subsampled = { $loaded } af { $total }
# The LAS point data record format number (0–10).
label-point-format = Punktformat
# The coordinate reference system.
label-crs = Koordinatsystem
# What each point carries besides its position.
label-attributes = Attributter
points-channel-rgb = RGB
points-channel-intensity = Intensitet
points-channel-classification = Klassifikation
points-channel-returns = Returer
# ASPRS classes present, with point counts: "Ground (1,234), Building (567)".
label-classes = Klasser
# E57 scans, with point counts.
label-scans = Scanninger
point-class-0 = Aldrig klassificeret
point-class-1 = Uklassificeret
point-class-2 = Terræn
point-class-3 = Lav vegetation
point-class-4 = Middelhøj vegetation
point-class-5 = Høj vegetation
point-class-6 = Bygning
point-class-7 = Lavt punkt (støj)
point-class-8 = Modelnøglepunkt
point-class-9 = Vand
point-class-10 = Jernbane
point-class-11 = Vejoverflade
point-class-12 = Overlap
point-class-13 = Jordline
point-class-14 = Faseleder
point-class-15 = Højspændingsmast
point-class-16 = Ledningssamling
point-class-17 = Brodæk
point-class-18 = Høj støj
point-class-19 = Overliggende konstruktion
point-class-20 = Ignoreret terræn
point-class-21 = Sne
point-class-22 = Tidsmæssig udelukkelse
# A class without a standard name, by number.
point-class-other = Klasse { $class }

## Gaussian splats

# machine-drafted, needs native review
group-splats = Gaussiske splats
label-splats = Splats
# Splats shown of those in the file, when a very large file was evenly thinned: "12,000,000 of 30,000,000".
splats-shown-of = { $shown } af { $total }
# The degree (1 to 3) of the spherical harmonics that give each splat its view-dependent colour.
label-splat-harmonics = Visningsafhængig farve
splats-harmonics-none = Ingen
# $format: the file format, e.g. "SPZ".
splats-format-antialiased = { $format }, trænet med kantudjævning

## Advarsler

# machine-drafted, needs native review
warn-not-diagnostic = Ikke til diagnostisk brug. Specular er en generel billedfremviser, ikke medicinsk udstyr: gengivelsen er ikke kalibreret til diagnostik.

warn-gps = Filen registrerer, hvor billedet blev taget. Fjern placeringen før deling, hvis den skal forblive privat.
warn-null-island = GPS-positionen er præcis 0°, 0°; kameraet havde sandsynligvis ingen position, så placeringen er formentlig forkert.
warn-date-future = Optagelsesdatoen ligger i fremtiden; kameraets ur var sandsynligvis indstillet forkert.
warn-icc-damaged = Den indlejrede farveprofil er beskadiget og kunne ikke læses.
warn-metadata-failed = Metadata kunne ikke læses på grund af en fejl i Specular. Selve billedet er i orden.
warn-orientation-conflict = EXIF og XMP er uenige om retningen (EXIF: { $exif }, XMP: { $xmp }). EXIF-værdien bruges.
warn-exif-damaged = Nogle EXIF-data kunne ikke læses ({ $error }); resten vises.
warn-xmp-damaged = XMP-metadataene er fejlformaterede ({ $error }); kun en del kunne læses.

## Om-boksen

geonames-attribution = Stednavne: GeoNames (www.geonames.org), udgivet under Creative Commons Attribution 4.0 (CC BY 4.0, https://creativecommons.org/licenses/by/4.0/). Offlineopslag via craten reverse_geocoder af Grant Miner (MIT eller Apache-2.0). Dataene er konverteret til en byliste; de leveres som de er, uden garanti.

## TIFF and DNG structure values (All fields)
# machine-drafted, needs native review

subfile-type = { $value ->
    [0] Billede i fuld opløsning
    [1] Billede i reduceret opløsning
    [2] Én side af et billede med flere sider
    [4] Gennemsigtighedsmaske
   *[other] Andet
}
predictor = { $value ->
    [1] Ingen
    [2] Vandret differens
    [3] Kommatalsdifferens
   *[other] Andet
}
extra-samples = { $value ->
    [0] Ikke angivet
    [1] Præmultipliceret alfa
    [2] Lige alfa
   *[other] Andet
}
sample-format = { $value ->
    [1] Heltal uden fortegn
    [2] Heltal med fortegn
    [3] Kommatal
    [4] Udefineret
   *[other] Andet
}

## Geodata (GeoTIFF, GeoJSON, Shapefile, GeoPackage, KML/KMZ, GPX)

# machine-drafted, needs native review
group-geospatial = Geodata
label-geo-crs = Koordinatsystem
label-geo-datum = Datum
label-geo-projection = Projektion
label-geo-unit = Enheder
label-geo-bounds = Udstrækning
label-geo-lonlat-bounds = Længde og bredde
label-geo-pixel-size = Pixelstørrelse
label-geo-overviews = Oversigter
label-geo-nodata = NoData-værdi
label-geo-raster-type = Rastertype
label-geo-vertical = Vertikal reference
label-geo-citation = Kildeangivelse
label-geo-features = Objekter
label-geo-vertices = Knudepunkter
label-geo-layer = Lag
label-geo-layers = Lag
label-geo-tiles = Fliser
label-geo-encoding = Tekstkodning
label-geo-warnings = Problemer
# The companion files a georeference was read from ("scene.jgw, scene.prj").
label-geo-sidecars = Ledsagende filer
# machine-drafted, needs native review
# A projection Specular cannot convert to longitude and latitude: $method is its name.
geo-projection-unsupported = { $method } (koordinater understøttes ikke)
# How longitude and latitude are brought to WGS 84 (what GPS and web maps use).
label-geo-to-wgs84 = Til WGS 84
geo-to-wgs84-same = Ikke nødvendig
geo-to-wgs84-helmert = Datumskift (Helmert)
geo-to-wgs84-by-area = Datumskift (Helmert, valgt efter område)
geo-to-wgs84-unknown = Ukendt: koordinaterne er på filens eget datum
# A coordinate system's name and its EPSG code: "WGS 84 / UTM zone 33N (EPSG:32633)".
geo-crs-epsg = { $name } (EPSG:{ $code })
geo-crs-unknown = Ikke angivet
# Map bounds: the lower-left and upper-right corners.
geo-bounds = { $x0 }, { $y0 } til { $x1 }, { $y1 }
# Bounds in degrees: west to east longitude, south to north latitude.
geo-lonlat-bounds = { $west }° til { $east }° længde, { $south }° til { $north }° bredde
geo-pixel-size = { $x } × { $y }
geo-overviews = { $n ->
    [one] { $n } oversigt: { $sizes }
   *[other] { $n } oversigter: { $sizes }
}
geo-pixel-is-point = Pixels er punkter
geo-layer = { $features ->
    [one] { $features } objekt
   *[other] { $features } objekter
}, { $fields ->
    [one] { $fields } felt
   *[other] { $fields } felter
}
# A GeoPackage tile pyramid: its table, the zoom level shown and its tile count.
geo-tiles = { $table }, zoomniveau { $zoom }, { $tiles ->
    [one] { $tiles } flise
   *[other] { $tiles } fliser
}
fields-geo-geokeys = GeoTIFF-nøgler
fields-geo-gdal = GDAL-metadata
fields-geo-wkt = Koordinatsystem (WKT)
fields-geo-projjson = Koordinatsystem (PROJJSON)
# machine-drafted, needs native review
# What a web map tile set (MBTiles, PMTiles) says about itself: container, tile type, zoom
# levels, description and other metadata.
fields-geo-tileset = Flisesæt
# The credit line a map's data asks to be shown with it ("© OpenStreetMap contributors").
label-geo-attribution = Kreditering
# The header lines of a georeferenced raster format (ENVI, ASCII grid, DTED, ERDAS IMAGINE).
fields-geo-header = Header
# machine-drafted, needs native review
# GIFTI files: the format version, what the file holds, the number of data arrays and the
# encoding of their values (e.g. "GZipBase64Binary").
label-gifti-version = GIFTI-version
label-gifti-role = Indeholder
gifti-role-surface = Overflade
gifti-role-labels = Etiketter pr. hjørnepunkt
gifti-role-data = Værdier pr. hjørnepunkt
label-gifti-arrays = Dataarrays
label-gifti-encoding = Kodning
label-gifti-vertices = Hjørnepunkter
label-gifti-triangles = Trekanter
# The brain structure, e.g. "CortexLeft".
label-gifti-structure = Struktur
# A surface's geometric type, e.g. "Anatomical", "Inflated".
label-gifti-geometry = Geometrisk type
# The coordinate transform of the vertices: $from and $to are coordinate spaces such as
# "scanner", "Talairach" or "MNI 152".
label-gifti-transform = Koordinater
gifti-transform = { $from } (transformation til { $to } ikke nødvendig)
gifti-transform-applied = { $from } → { $to } (anvendt)
# The surface file a data file is drawn on, found beside it.
label-gifti-surface = Vist på
# The data array shown: its name, NIfTI intent (e.g. "NIFTI_INTENT_SHAPE") and data type.
label-gifti-array = Array
label-gifti-intent = Intent
label-gifti-data-type = Datatype
# Entries of the label table (names and colours of regions).
label-gifti-labels = Etikettabel
# CIFTI-2 files: the version, NIfTI intent (e.g. "ConnDenseScalar") and matrix size.
label-cifti-version = CIFTI-version
label-cifti-intent = Intent
label-cifti-matrix = Matrix
cifti-matrix = { $rows } × { $columns }
# The maps (pages) of the file and what they are.
label-cifti-maps = Kort
cifti-maps-scalars = { $count ->
        [one] { $count } skalarkort
       *[other] { $count } skalarkort
    }
cifti-maps-labels = { $count ->
        [one] { $count } etiketkort
       *[other] { $count } etiketkort
    }
cifti-maps-series = { $count ->
        [one] { $count } tidspunkt
       *[other] { $count } tidspunkter
    }
cifti-maps-parcels = { $count ->
        [one] { $count } parcel
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
    }, { $size } læst pr. seed
# A time series' start and step; $unit is e.g. "second".
label-cifti-series = Serie
cifti-series = fra { $start }, hver { $step } ({ $unit })
label-cifti-models = Hjernemodeller
label-cifti-parcels = Parceller
label-cifti-structures = Strukturer
label-cifti-voxels = Subkortikale voxler
# A surface the file's values lie on: the brain structure (e.g. "CORTEX_LEFT"), its vertex
# count, and the surface file found beside the file for it, or that none was found.
label-cifti-surface = Overflade
cifti-surface-found = { $structure }: { $vertices ->
        [one] { $vertices } hjørnepunkt
       *[other] { $vertices } hjørnepunkter
    }, { $file }
cifti-surface-missing = { $structure }: kræver en overflade med { $vertices ->
        [one] { $vertices } hjørnepunkt
       *[other] { $vertices } hjørnepunkter
    } (ingen ved siden af filen)
# How the file is shown: on the surfaces found beside it, or as its subcortical voxels.
label-cifti-shown = Vist som
cifti-shown-surfaces = Farvede overflader
cifti-shown-voxels = Volumen af de subkortikale voxler
# machine-drafted, needs native review
# Shown with a picked seed's row: click a surface vertex, or move the crosshair in MPR.
cifti-shown-connectivity = Konnektivitet for det valgte seed
# Maps left out of the voxel volume, which would be too large with them.
label-cifti-left-out = Udeladte kort
# Tractograms: format, number of streamlines (fibre tracts) and points.
label-tract-format = Format
label-tract-streamlines = Streamlines
# A large tractogram draws only some of its streamlines.
tract-streamlines-shown = { $count } ({ $shown } vist)
label-tract-points = Punkter
label-tract-mean-length = Gennemsnitlig længde
# A length in millimetres.
tract-mm = { $value } mm
# Values per point and per streamline the file stores, by name.
label-tract-scalars = Skalarer pr. punkt
label-tract-properties = Egenskaber pr. streamline
# TrackVis: the voxel grid the tracts were made in, its voxel size and voxel order (e.g. "LAS").
label-tract-grid = Gitter
label-tract-voxel-size = Voxelstørrelse
label-tract-voxel-order = Voxelrækkefølge
# How the points were brought into scanner (RAS) millimetres.
label-tract-space = Koordinater
tract-space-vox-to-ras = Fra voxel-til-RAS-matricen
tract-space-voxel-order = Fra voxelrækkefølgen (ingen voxel-til-RAS-matrix)
tract-space-scanner = Scannerrum
# MRtrix: the tracking step size.
label-tract-step = Skridtlængde

## File headers (DPX, Cineon, XWD, MIFF, TIM, WSQ, JBIG)

# machine-drafted, needs native review
# What the file's own header records, for formats without Exif or XMP.
group-header = Filhoved
label-header-created = Oprettet
# The program or person that wrote the file.
label-header-creator = Ophavsperson
label-header-copyright = Ophavsret
label-header-project = Projekt
label-header-comment = Kommentar
# The scanner, camera or other device the image came from.
label-header-device = Enhed
# SMPTE time code of the frame: "01:00:00:12".
label-header-timecode = Tidskode
# A film frame's edge code (KeyKode).
label-header-edge-code = Kantkode
label-header-frame-rate = Billedfrekvens
# Where the frame sits in its sequence.
label-header-frame = Billede
# Film stock and slate.
label-header-film = Film
# How code values relate to light: printing density, linear, BT.709…
label-header-transfer = Overføringsfunktion
# The colour primaries the file states.
label-header-colorimetric = Kolorimetri
# The X window a screen dump was taken of.
label-header-window = Vindue
label-header-resolution = Opløsning
label-header-compression = Komprimering
# machine-drafted, needs native review
# JPEG XS: the profile and the level the codestream states ("Main 422.10"; "4k-1, sublevel 6 bpp").
label-header-profile = Profil
# machine-drafted, needs native review
label-header-level = Niveau
# machine-drafted, needs native review
# JPEG XS: how many wavelet levels, horizontal and vertical ("5 horizontal, 1 vertical").
label-header-decomposition = Waveletniveauer
# machine-drafted, needs native review
# JPEG XS: the colour transform ("None", "Reversible colour transform (RCT)", "Star-Tetrix, RGGB mosaic").
label-header-colour-transform = Farvetransformation
# machine-drafted, needs native review
# Photoshop: the artboards, by name and size ("Phone (375 × 667), Tablet (768 × 1024)").
label-header-artboards = Tegnebrætter
# machine-drafted, needs native review
# Photoshop file without a merged image, shown from its layers: the features it leaves out,
# by Photoshop's names ("Color Lookup, Knockout").
label-header-not-drawn = Tegnes ikke
# machine-drafted, needs native review
# The same, for the features drawn only approximately ("Bevel & Emboss, Satin").
label-header-approximated = Tegnes tilnærmet

## Fonts (TTF, OTF, TTC/OTC, WOFF, WOFF2)

# machine-drafted, needs native review
group-font = Skrifttype
label-font-family = Familie
label-font-style = Stil
# The face's place in a font collection: "2 of 4".
label-font-face = Snit
font-face-of = { $index } af { $count }
label-font-version = Version
label-font-designer = Designer
# The font's maker (the name table's manufacturer).
label-font-manufacturer = Skriftstøberi
label-font-glyphs = Glyffer
label-font-outlines = Konturer
font-outlines-truetype = TrueType (kvadratiske)
font-outlines-cff = PostScript (CFF, kubiske)
font-outlines-cff2 = PostScript (CFF2, kubiske)
font-outlines-bitmap = Kun bitmaps
font-outlines-none = Ingen
# What the font's licence flags (OS/2 fsType) allow documents that embed it to do.
label-font-embedding = Indlejring
font-embedding-installable = Må indlejres og installeres.
font-embedding-restricted = Må ikke indlejres.
font-embedding-preview-print = Må kun indlejres til visning og udskrivning.
font-embedding-editable = Må indlejres i dokumenter, der redigeres.
font-embedding-no-subsetting = Kun hel, ikke som delmængde.
font-embedding-bitmap-only = Kun som bitmaps.
# Variation axes of a variable font: "Weight 100–900 (400)".
label-font-axes = Variationsakser
font-axis = { $name } { $min }–{ $max } ({ $default })
label-font-instances = Navngivne stile
label-font-colour = Farveglyffer
font-colour-colr0 = COLR-lag
font-colour-colr1 = COLR v1-forløb
font-colour-sbix = Bitmaps (sbix)
font-colour-cbdt = Bitmaps (CBDT)
font-colour-svg = SVG
label-font-weight = Vægt
label-font-units-per-em = Enheder pr. em
label-font-tables = Tabeller
label-font-licence = Licens
label-font-licence-url = Licens-URL
label-font-copyright = Ophavsret
label-font-trademark = Varemærke

## Office documents (Word, Excel, PowerPoint, iWork, OpenDocument)

# machine-drafted, needs native review
group-document = Dokument
label-document-kind = Type
document-kind-word = Word-dokument
document-kind-excel = Excel-projektmappe
document-kind-powerpoint = PowerPoint-præsentation
document-kind-pages = Pages-dokument
document-kind-numbers = Numbers-regneark
document-kind-keynote = Keynote-præsentation
document-kind-odf-text = OpenDocument-tekst
document-kind-odf-spreadsheet = OpenDocument-regneark
document-kind-odf-presentation = OpenDocument-præsentation
document-kind-odf-graphics = OpenDocument-tegning
document-kind-odf-other = OpenDocument-fil
# machine-drafted, needs native review
document-kind-playlist = Afspilningsliste
# machine-drafted, needs native review
label-playlist-items = Elementer
# Appended to the kind: "Word document, template, with macros".
document-template = skabelon
document-macros = med makroer
document-legacy = Office 97–2003-format
label-document-preview = Forhåndsvisning
# The image format of the preview stored in the file, e.g. "JPEG image".
document-preview-image = { $format }-billede
document-preview-none = Ingen i filen
label-document-subject = Emne
label-document-author = Forfatter
label-document-description = Beskrivelse
label-document-category = Kategori
label-document-last-saved-by = Senest gemt af
label-document-created = Oprettet
label-document-modified = Ændret
label-document-printed = Senest udskrevet
label-document-pages = Sider
label-document-slides = Dias
label-document-hidden-slides = Skjulte dias
label-document-notes = Notesider
label-document-sheets = Ark
label-document-words = Ord
label-document-characters = Tegn
label-document-paragraphs = Afsnit
label-document-lines = Linjer
label-document-editing-time = Redigeringstid
document-minutes = { $minutes } min.
label-document-revision = Revision
label-document-application = Gemt med
label-document-company = Firma
label-document-template = Skabelon

## Books (EPUB)

# machine-drafted, needs native review
group-book = Bog
label-book-authors = Forfattere
label-book-publisher = Forlag
label-book-date = Udgivet
label-book-language = Sprog
label-book-pages = Sider
# Entries in the book's table of contents.
label-book-contents = Punkter i indholdsfortegnelsen
label-book-layout = Layout
book-layout-reflowable = Flydende
book-layout-fixed = Fast layout
book-right-to-left = fra højre mod venstre
label-book-version = Format
book-version = EPUB { $version }
label-book-fonts = Indlejrede skrifttyper
book-fonts-obfuscated = { $count } ({ $obfuscated } sløret)
label-book-subjects = Emner
label-book-identifier = Identifikator
label-book-rights = Rettigheder

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
volume-orient-parrec = Philips-vinkling og forskydning
volume-orient-ecat = ECAT-voxelstørrelser og forskydninger (upålidelige)
label-volume-technique = Teknik
label-volume-version = Formatversion
label-volume-isotope = Isotop
label-volume-tracer = Sporstof
label-volume-units = Enheder

# Text, tables and subtitles.
label-header-encoding = Tegnsæt
label-header-lines = Linjer
label-header-line-ends = Linjeskift
label-header-words = Ord
label-header-pages = Sider
label-header-pictures = Billeder
label-header-pictures-not-shown = Billeder, der ikke vises
label-header-tables = Tabeller
label-header-rows = Rækker
label-header-columns = Kolonner
label-header-delimiter = Skilletegn
label-header-column-names = Kolonnenavne
label-header-cues = Undertekster
label-header-cue-span = Undertekster fra – til
