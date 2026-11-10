# machine-drafted, needs native review
# Specular-metagegevenspaneel — Nederlands.
# Zie i18n/en/metadata.ftl voor de conventies. Getallen en datums komen al
# door ICU4X opgemaakt voor de landinstelling binnen.

## Titels van overzichtsgroepen

group-camera = Camera
group-exposure = Belichting
group-lens = Objectief
group-date = Datum en tijd
group-location = Locatie
# machine-drafted, needs native review
# A DJI drone photo: the gimbal's and the aircraft's attitude, the height above take-off.
group-drone = Drone
label-gimbal = Gimbal
label-flight = Toestel
label-relative-altitude = Boven het startpunt
drone-attitude = gieren { $yaw }, stampen { $pitch }, rollen { $roll }
# A DJI thermal camera's radiometric JPEG: its photo opens, its temperatures can't.
label-thermal-data = Temperaturen
drone-thermal-refused = Niet getoond: de SDK-licentie van DJI staat het niet toe
drone-thermal-refused-detail = Voor DJI-temperaturen is de DJI Thermal SDK nodig, waarvan de licentie hem alleen toestaat in programma’s die hem meeleveren. Het beeld en de metadata worden getoond.
group-image = Afbeelding
group-file = Bestand
group-rights = Rechten en beschrijving
group-credentials = Herkomst van inhoud
# machine-drafted, needs native review
group-medical = Medisch beeld
# machine-drafted, needs native review
group-inspection = Onderdeel en inspectie
group-nde-image = Inspectiebeeld (DICONDE)
# machine-drafted, needs native review
# Industrial CT volumes (Zeiss Xradia, Amira, Bruker SkyScan, Nikon X-Tek, VGI, raw volumes).
group-industrial-image = Inspectiebeeld
group-science = Wetenschappelijk beeld
group-em = Elektronenmicroscopie
group-spm = Scanning-probe-microscopie
group-slide = Objectglaasje
group-microscopy = Microscopie
# X-ray diffraction detector frames (CBF, EDF, SMV, Bruker, MarCCD, R-AXIS).
group-diffraction = Diffractie
group-surface = Oppervlaktetopografie
group-ebsd = Elektronen-terugverstrooiingsdiffractie
group-seismic = Seismische data
# machine-drafted, needs native review
group-asdf = ASDF
# machine-drafted, needs native review
# CERN ROOT files (particle physics histograms and graphs).
group-root = ROOT
# machine-drafted, needs native review
# CASA images (radio interferometry: the .image, .residual, .model directories tclean writes).
group-casa = CASA-beeld
# machine-drafted, needs native review
# GIFTI files: brain surfaces and the values on their vertices (Human Connectome Project
# and FreeSurfer exports).
group-gifti = GIFTI
# CIFTI-2 files: maps over cortical surfaces and subcortical voxels ("grayordinates").
group-cifti = CIFTI
# White-matter fibre tracts (TrackVis, MRtrix).
group-tractography = Tractografie
# machine-drafted, needs native review
group-annotations = Annotaties
# Imaging mass spectrometry (imzML): a mass spectrum per pixel.
group-mass-spec = Massaspectrometrische beeldvorming
# machine-drafted, needs native review
group-thermal = Warmtebeeld
# machine-drafted, needs native review
# Flow cytometry list-mode data (FCS): events of a cell sorter or analyser.
group-cytometry = Flowcytometrie
# machine-drafted, needs native review
group-spectroscopy = Spectroscopie
# machine-drafted, needs native review
# Unstructured meshes of ocean, coastal and climate models (UGRID, ICON, MPAS in NetCDF).
group-ugrid = Ongestructureerd rooster
group-drawing = Tekening
# machine-drafted, needs native review
group-volume = Volume

## Labels van overzichtsitems

label-camera = Camera
label-software = Software
label-application = Toepassing
label-shutter = Sluitertijd
label-aperture = Diafragma
label-iso = ISO
label-focal-length = Brandpuntsafstand
label-exposure-bias = Correctie belichting
label-program = Modus
label-metering = Lichtmeting
label-white-balance = Witbalans
label-flash = Flitser
label-lens = Objectief
label-lens-make = Objectiefmerk
label-lens-range = Objectiefbereik
# machine-drafted, needs native review
label-focus-mode = Scherpstelmodus
label-stabilisation = Beeldstabilisatie
label-taken = Gemaakt
label-digitised = Gedigitaliseerd
label-edited = Laatst bewerkt
label-place = Plaats
label-coordinates = Coördinaten
label-altitude = Hoogte
label-direction = Richting
label-accuracy = Nauwkeurigheid
label-gps-time = GPS-tijd
label-dimensions = Afmetingen
label-encoding = Codering
# machine-drafted, needs native review
label-motion = Beweging
motion-kind-motion-photo = Bewegende foto
motion-kind-micro-video = Bewegende foto (MicroVideo)
motion-kind-samsung = Samsung bewegende foto
motion-kind-live-photo = Live Photo
motion-duration = { $seconds } s
label-colour-profile = Kleurprofiel
label-orientation = Oriëntatie
label-file-name = Naam
label-folder = Map
label-file-size = Grootte
label-format = Indeling
label-modified = Gewijzigd
label-title = Titel
label-description = Beschrijving
label-creator = Maker
label-copyright = Copyright
label-keywords = Trefwoorden
label-rating = Beoordeling
label-colour-label = Label
label-credentials = Herkomst van inhoud

## Groepstitels bij ‘Alle velden’ (tagnamen blijven zoals in de standaarden)

fields-ifd0 = IFD0 (hoofdafbeelding)
fields-exif = Exif
fields-gps = GPS
fields-interop = Interoperabiliteit
fields-ifd1 = IFD1 (miniatuur)
fields-icc = Kop van ICC-profiel
# $namespace: uit xmp-namespace; $prefix: bijv. "dc"
fields-xmp = XMP: { $namespace } ({ $prefix })
xmp-namespace = { $prefix ->
    [dc] Dublin Core
    [xmp] Basis
    [xmpRights] Rechten
    [xmpMM] Mediabeheer
    [xmpDM] Dynamische media
    [tiff] TIFF
    [exif] Exif
    [exifEX] Exif 2.3+
    [aux] Camera-accessoires
    [photoshop] Photoshop
    [crs] Camera Raw-instellingen
    [lr] Lightroom
    [Iptc4xmpCore] IPTC Core
    [Iptc4xmpExt] IPTC Extension
    [plus] PLUS-licenties
    [GPano] Fotobol
    [GCamera] Google-camera
    [hdrgm] HDR-gainmap
    [mwg-rs] Regio’s
    [pdf] PDF
    [stEvt] Bronbewerkingen
    [stRef] Bronverwijzingen
   *[other] Overig
}
# $number: "0xA500"; $context: "Exif", "GPS", …
unknown-tag = Onbekende tag { $number } ({ $context })
blob-bytes = { $num ->
    [one] ({ $num } byte)
   *[other] ({ $num } bytes)
}
maker-note-bytes = ({ $num ->
        [one] { $num } byte
       *[other] { $num } bytes
    }, makernote, niet gedecodeerd)
more-values = { $shown }, … ({ $total ->
        [one] { $total } waarde
       *[other] { $total } waarden
    })

## Algemene verbindingstekens

# Scheidt de delen van een lijst die in code wordt opgebouwd.
list-separator = {", "}
# Scheidt sluitertijd · diafragma · ISO · brandpuntsafstand in het overzicht op één regel.
one-line-separator = {" · "}

## Belichting

exposure-time-fraction = 1/{ $denominator } s
exposure-time-seconds = { $seconds } s
aperture-value = f/{ $f }
iso-value = ISO { $iso }
focal-length-value = { $mm } mm
focal-length-with-equiv = { $mm } mm ({ $equiv } mm equiv.)
focal-length-equiv-only = { $equiv } mm equiv.
# $ev is de grootte; het teken komt uit de selector.
exposure-bias-value = { $sign ->
    [plus] +{ $ev } EV
    [minus] −{ $ev } EV
   *[zero] 0 EV
}
exposure-program = { $value ->
    [1] Handmatig
    [2] Programma (auto)
    [3] Diafragmavoorkeur
    [4] Sluitertijdvoorkeur
    [5] Creatief (scherptediepte)
    [6] Actie (korte sluitertijd)
    [7] Portret
    [8] Landschap
   *[other] Onbekend
}
metering-mode = { $value ->
    [1] Gemiddeld
    [2] Centrumgericht gemiddeld
    [3] Spot
    [4] Multispot
    [5] Matrix (meervlaks)
    [6] Deelmeting
   *[other] Overig
}
# machine-drafted, needs native review
# Canon maker note CameraSettings FocusMode; 256–258 are the live-view variants of 0–2.
canon-focus-mode = { $value ->
    [0] One-Shot AF
    [1] AI Servo AF
    [2] AI Focus AF
    [3] Handmatig scherpstellen
    [4] Enkel
    [5] Continu
    [6] Handmatig scherpstellen
    [16] Pan focus
    [256] One-Shot AF (Live View)
    [257] AI Servo AF (Live View)
    [258] AI Focus AF (Live View)
    [512] Movie Snap-scherpstelling
    [519] Movie Servo AF
   *[other] Overig
}
# Canon maker note CameraSettings ImageStabilization; 256–260 repeat 0–4.
canon-stabilisation = { $value ->
    [0] Uit
    [1] Aan
    [2] Alleen bij opname
    [3] Meetrekken
    [4] Dynamisch
    [256] Uit
    [257] Aan
    [258] Alleen bij opname
    [259] Meetrekken
    [260] Dynamisch
   *[other] Overig
}
# machine-drafted, needs native review
panasonic-focus-mode = { $value ->
    [1] Automatisch
    [2] Handmatige scherpstelling
    [4] Automatisch (scherpstelknop)
    [5] Automatisch (continu)
    [6] AF-S
    [7] AF-C
    [8] AF-F
   *[other] Anders
}
# machine-drafted, needs native review
panasonic-stabilisation = { $value ->
    [2] Aan (optisch)
    [3] Uit
    [4] Aan (modus 2)
    [5] Aan (optisch, meetrekken)
    [6] Aan (alleen body)
    [7] Aan (alleen body, meetrekken)
    [9] Dual IS
    [10] Dual IS (meetrekken)
    [11] Dual IS 2
    [12] Dual IS 2 (meetrekken)
   *[other] Anders
}
label-comment = Opmerking
fields-tags = { $kind }-tags
group-picture = Beeldinstellingen
fields-fujifilm = Fujifilm-fabrikantgegevens
label-film-simulation = Filmsimulatie
label-dynamic-range = Dynamisch bereik
label-highlight-tone = Hoge lichten
label-shadow-tone = Schaduwen
label-colour-setting = Kleur
label-sharpness = Scherpte
label-noise-reduction = Ruisonderdrukking
label-clarity = Helderheid
label-grain-effect = Korreleffect
label-colour-chrome = Kleurchroom
label-colour-chrome-blue = Kleurchroom blauw
label-shutter-type = Sluitertype
label-drive-mode = Opnamestand
fuji-step = { $sign ->
    [plus] +{ $n }
    [minus] −{ $n }
   *[zero] 0
}
fuji-film-mode = { $value ->
    [0] Provia (standaard)
    [256] Studioportret
    [272] Studioportret (meer verzadiging)
    [288] Astia (zacht)
    [304] Studioportret (scherper)
    [512] Velvia (levendig)
    [768] Studioportret Ex
    [1024] Velvia
    [1280] Pro Neg. Std
    [1281] Pro Neg. Hi
    [1536] Classic Chrome
    [1792] Eterna
    [2048] Classic Neg.
    [2304] Eterna Bleach Bypass
    [2560] Nostalgic Neg.
    [2816] Reala Ace
   *[other] Anders
}
fuji-monochrome = { $value ->
    [768] Monochroom
    [769] Monochroom, roodfilter
    [770] Monochroom, geelfilter
    [771] Monochroom, groenfilter
    [784] Sepia
    [1280] Acros
    [1281] Acros, roodfilter
    [1282] Acros, geelfilter
    [1283] Acros, groenfilter
   *[other] Monochroom
}
fuji-white-balance = { $value ->
    [0] Automatisch
    [1] Automatisch (witprioriteit)
    [2] Automatisch (sfeerprioriteit)
    [256] Daglicht
    [512] Bewolkt
    [768] Daglicht-tl
    [769] Dagwit-tl
    [770] Wit-tl
    [771] Warmwit-tl
    [772] Warmwit-tl voor woonruimte
    [1024] Gloeilamp
    [1280] Flitser
    [1536] Onder water
    [3840] Aangepast 1
    [3841] Aangepast 2
    [3842] Aangepast 3
    [3843] Aangepast 4
    [3844] Aangepast 5
    [4080] Kleurtemperatuur
   *[other] Anders
}
fuji-white-balance-kelvin = { $kelvin } K
fuji-dynamic-range = DR{ $percent }
fuji-dynamic-range-auto = Automatisch (DR{ $percent })
fuji-dynamic-range-setting = { $value ->
    [0] Automatisch
    [256] Standaard (DR100)
    [512] Breed 1 (DR230)
    [513] Breed 2 (DR400)
   *[other] Anders
}
fuji-effect = { $value ->
    [0] Uit
    [32] Zwak
    [64] Sterk
   *[other] Anders
}
fuji-grain-size = { $value ->
    [16] klein
    [32] groot
   *[other] anders
}
fuji-grain = { $strength }, { $size }
fuji-shutter-type = { $value ->
    [0] Mechanisch
    [1] Elektronisch
    [2] Elektronisch (lange belichtingen)
    [3] Elektronisch eerste sluitergordijn
   *[other] Anders
}
fuji-drive-mode = { $value ->
    [0] Enkele opname
    [1] Continu, lage snelheid
    [2] Continu, hoge snelheid
   *[other] Anders
}
fuji-focus-mode = { $value ->
    [0] Handmatig
    [1] AF-S
    [2] AF-C
    [100] Automatisch
    [101] Handmatig
   *[other] Anders
}
fuji-stabilisation = { $kind } ({ $mode })
fuji-stabilisation-kind = { $value ->
    [0] Geen
    [1] Optisch
    [2] Sensorverschuiving
    [3] Objectief
    [258] Sensorverschuiving en digitaal
    [512] Digitaal
   *[other] Anders
}
fuji-stabilisation-mode = { $value ->
    [0] Uit
    [1] aan, doorlopend
    [2] aan, bij opname
   *[other] Anders
}
light-source = { $value ->
    [1] Daglicht
    [2] Tl-licht
    [3] Gloeilamp
    [4] Flitser
    [9] Zonnig weer
    [10] Bewolkt
    [11] Schaduw
    [12] Daglicht-tl
    [13] Dagwit tl-licht
    [14] Koelwit tl-licht
    [15] Wit tl-licht
    [16] Warmwit tl-licht
    [17] Standaardlicht A
    [18] Standaardlicht B
    [19] Standaardlicht C
    [20] D55
    [21] D65
    [22] D75
    [23] D50
    [24] Studiogloeilamp
   *[other] Overig
}
white-balance-auto = Automatisch
white-balance-manual = Handmatig
white-balance-manual-with-light = Handmatig ({ $light })
# De flitserbeschrijving wordt uit deze delen opgebouwd, gescheiden door list-separator.
flash-none = Geen flitserfunctie
flash-fired = Flitser afgegaan
flash-not-fired = Flitser niet afgegaan
flash-forced-on = geforceerd aan
flash-forced-off = geforceerd uit
flash-auto = automatische modus
flash-return-not-detected = terugkerend licht niet gedetecteerd
flash-return-detected = terugkerend licht gedetecteerd
flash-red-eye = rodeogenreductie

## Objectief

lens-focal-single = { $mm } mm
lens-focal-range = { $min }–{ $max } mm
lens-aperture-single = f/{ $f }
lens-aperture-range = f/{ $min }–{ $max }
lens-range = { $focal } { $aperture }

## Datum en tijd

# $weekday: "dinsdag" (ICU); $date: "14 mei 2024" (ICU)
date-with-weekday = { $weekday } { $date }
date-time = { $date }, { $time }
date-time-zone = { $datetime } ({ $zone })
zone-utc = UTC
# $offset: "+1", "+5:30", "−3"
zone-offset = UTC{ $offset }
gps-date-time = { $date }, { $time } UTC
detail-zone-from-gps = Tijdzone afgeleid uit de GPS-klok

## Locatie

# $lat en $lon zijn zonder teken; het halfrond komt uit de selector.
coordinates = { $lat }° { $latRef ->
    [south] Z
   *[north] N
}, { $lon }° { $lonRef ->
    [west] W
   *[east] O
}
altitude = { $ref ->
    [above] { $m } m boven zeeniveau
    [below] { $m } m onder zeeniveau
   *[sea] Op zeeniveau
}
compass-point = { $point ->
    [n] noord
    [nne] noord-noordoost
    [ne] noordoost
    [ene] oost-noordoost
    [e] oost
    [ese] oost-zuidoost
    [se] zuidoost
    [sse] zuid-zuidoost
    [s] zuid
    [ssw] zuid-zuidwest
    [sw] zuidwest
    [wsw] west-zuidwest
    [w] west
    [wnw] west-noordwest
    [nw] noordwest
   *[nnw] noord-noordwest
}
# $point: uit compass-point; $degrees: "287"
facing = Richting { $point } ({ $degrees }°)
detail-true-north = Ten opzichte van het geografische noorden
detail-magnetic-north = Ten opzichte van het magnetische noorden
accuracy = ±{ $m } m
distance-km = { $km } km
place = { $city }, { $country }
place-city-only = { $city }
# Meer dan 30 km van de gevonden plaats.
place-near = bij { $place }
place-detail = { $areas } · { $distance } van { $city }
place-detail-distance = { $distance } van { $city }

## Afbeelding

dimensions = { $width } × { $height }
dimensions-megapixels = { $width } × { $height } ({ $mp } MP)
encoding-bit-depth = { $bits } bit
# $format: "8 bit YCbCr 4:2:0"
encoding-progressive = { $format }, progressief
encoding-baseline = { $format }, baseline
encoding-hierarchical = { $format }, hiërarchisch
# machine-drafted, needs native review
encoding-alpha = { $format } { $alpha ->
    [premultiplied] met voorvermenigvuldigde alfa
    [transparency] met transparantie
   *[alpha] met alfa
}
# [Lab], [Palette] and the variants after them: machine-drafted, needs native review
colour-model = { $model ->
    [YCbCr] YCbCr
    [RGB] RGB
    [Greyscale] Grijswaarden
    [CMYK] CMYK
    [YCCK] YCCK
    [Lab] L*a*b*
    [Palette] Geïndexeerde kleuren
    [Multichannel] Meerkanaals
    [Duotone] Duotoon
    [YCgCo] YCgCo
    [XYB] XYB
    [YC] Luminantie/chrominantie (YC)
    [Bayer] Bayer-raw
    [X-Trans] X-Trans-raw
    [CFA] Raw met kleurfilterpatroon
    [LinearRaw] Lineaire raw
    [RGBE] RGBE (gedeelde exponent)
    [XYZE] XYZE (gedeelde exponent)
    [R] Eén kanaal (R)
    [RG] Twee kanalen (RG)
   *[other] { $model }
}
adobe-rgb-from-exif = Adobe RGB (uit EXIF-tag; geen ingesloten profiel)
orientation = { $value ->
    [1] Rechtop (geen draaiing nodig)
    [2] Horizontaal gespiegeld (toegepast)
    [3] 180° gedraaid (toegepast)
    [4] Verticaal gespiegeld (toegepast)
    [5] Horizontaal gespiegeld en 90° linksom gedraaid (toegepast)
    [6] 90° rechtsom gedraaid (toegepast)
    [7] Horizontaal gespiegeld en 90° rechtsom gedraaid (toegepast)
    [8] 90° linksom gedraaid (toegepast)
   *[other] Onbekend ({ $value })
}
# Korte vorm voor waarschuwingen.
orientation-short = { $value ->
    [1] Rechtop
    [2] Horizontaal gespiegeld
    [3] 180° gedraaid
    [4] Verticaal gespiegeld
    [5] Horizontaal gespiegeld en 90° linksom gedraaid
    [6] 90° rechtsom gedraaid
    [7] Horizontaal gespiegeld en 90° rechtsom gedraaid
    [8] 90° linksom gedraaid
   *[other] waarde { $value }
}

## Bestand

file-size-bytes = { $num ->
    [one] { $num } byte
   *[other] { $num } bytes
}
# Decimale eenheden (1 KB = 1000 bytes).
file-size-unit = { $num } { $unit ->
    [kb] KB
    [mb] MB
    [gb] GB
    [tb] TB
   *[pb] PB
}

## Rechten

rating-rejected = Afgewezen
# $stars: "★★★★☆"
rating-stars = { $stars } ({ $n } van 5)

## Inhoudsreferenties

c2pa-present = Aanwezig (niet geverifieerd)
c2pa-detail = Dit bestand bevat een C2PA-manifest; de handtekening is niet gecontroleerd.

## Waarden in de ICC-kop

icc-class = { $class ->
    [scnr] Invoerapparaat
    [mntr] Weergaveapparaat
    [prtr] Uitvoerapparaat
    [link] Apparaatkoppeling
    [spac] Kleurruimte
    [abst] Abstract
    [nmcl] Benoemde kleur
   *[other] Onbekend
}
icc-intent = { $intent ->
    [0] Perceptueel
    [1] Relatief colorimetrisch
    [2] Verzadiging
    [3] Absoluut colorimetrisch
   *[other] Onbekend
}
icc-platform = { $platform ->
    [APPL] Apple
    [MSFT] Microsoft
    [SGI] Silicon Graphics
    [SUNW] Sun Microsystems
    [none] Niet opgegeven
   *[other] Onbekend
}
icc-coded = { $name } ({ $code })

# machine-drafted, needs native review
label-modality = Modaliteit
label-body-part = Lichaamsdeel
label-study = Onderzoek
label-series = Serie
label-slice-thickness = Plakdikte
label-pixel-spacing = Pixelafstand
label-imager-pixel-spacing = Pixelafstand op de detector
label-frames = Frames
label-transfer-syntax = Transfersyntax
label-document = Document
label-document-type = Documenttype
medical-spacing = { $row } × { $column } mm
medical-mm = { $mm } mm
medical-lossy = { $name } (compressie met verlies)
detail-pixel-spacing = Afstand tussen pixelmiddens in de patiënt (rijen × kolommen).
# machine-drafted, needs native review
# Pixel spacing of an industrial inspection image (DICONDE, CT of an object).
detail-pixel-spacing-object = Afstand tussen pixelmiddens in het object (rijen × kolommen).
detail-imager-pixel-spacing = Gemeten op de detector, niet gecorrigeerd voor vergroting: afstanden in de patiënt zijn meestal kleiner.

## Industriële inspectie (DICONDE)

# machine-drafted, needs native review
label-nde-component-shape = Vorm van onderdeel
label-nde-component-manufacturer = Fabrikant van onderdeel
label-nde-material = Materiaal
label-nde-thickness = Materiaaldikte
label-nde-pipe-diameter = Buisdiameter
label-nde-source = Bron
label-nde-environment = Omgeving
label-nde-iqi = Beeldkwaliteitsindicator
label-nde-indications = Indicaties
label-nde-standard = Norm

## Onderzoeksvolumes (NIfTI, Analyze, NRRD, MetaImage)

# machine-drafted, needs native review
label-voxel-size = Voxelgrootte
label-data-type = Gegevenstype
label-time-points = Tijdpunten
label-orientation-source = Oriëntatie uit
label-intent = Waarden
label-compression = Compressie
volume-voxel-size = { $x } × { $y } × { $z } mm
volume-orient-sform = NIfTI sform
volume-orient-qform = NIfTI qform
volume-orient-analyze = Analyze-header (onbetrouwbaar)
volume-orient-space = NRRD-ruimterichtingen
volume-orient-transform = MetaImage-transformatie
volume-orient-spacings = Alleen voxelafstand
volume-orient-none = Niet vermeld
volume-orient-vox2ras = FreeSurfer vox2ras
volume-orient-mgh-default = FreeSurfer-standaard (niet betrouwbaar)
volume-orient-minc = MINC-richtingscosinussen
volume-orient-object = Objectcoördinaten (geen patiëntoriëntatie)
volume-content-projections = Projecties (ruwe scan)
volume-content-reconstruction = Gereconstrueerd volume
volume-coords-rectilinear = Rechtlijnig (ongelijke afstand, gelijkmatig getoond)
# Scan parameters and tags a FreeSurfer MGH file keeps after its voxels.
label-volume-tr = Repetitietijd
label-volume-te = Echotijd
label-volume-ti = Inversietijd
label-volume-flip = Fliphoek
label-volume-fov = Gezichtsveld
label-volume-field = Veldsterkte
label-volume-pedir = Fasecodering
label-volume-cmdline = Opdrachtregel
label-volume-xform = Talairach-transformatie
label-volume-content = Inhoud
label-volume-projections = Projecties
label-volume-angles = Rotatiebereik
label-volume-angle-step = Rotatiestap
label-volume-kv = Buisspanning
label-volume-current = Buisstroom
label-volume-power = Bronvermogen
label-volume-exposure = Belichting
label-volume-sod = Bron tot object
label-volume-odd = Object tot detector
label-volume-sdd = Bron tot detector
label-volume-magnification = Optische vergroting
label-volume-binning = Binning
label-volume-objective = Objectief
label-volume-filter = Bronfilter
label-volume-camera = Detector
label-volume-instrument = Instrument
label-volume-software = Software
label-volume-date = Opgenomen
label-volume-ring = Correctie van ringartefacten
label-volume-smoothing = Afvlakking
label-volume-coords = Raster
# machine-drafted, needs native review
# An eye scan (OCT): which eye.
label-volume-eye = Oog
volume-eye-right = Rechts (OD)
volume-eye-left = Links (OS)
detail-volume-unreliable = Het bestand vermeldt niet waar de assen in de patiënt naartoe wijzen: links en rechts kunnen verwisseld zijn, dus er worden geen oriëntatielabels getoond.
# machine-drafted, needs native review
# An industrial CT volume: its axes are the scanner's, there is no patient.
detail-volume-object = De assen zijn die van de scanner, niet van een patiënt, dus er worden geen oriëntatielabels getoond.

## Wetenschappelijke beelden (FITS, XISF)

label-object = Object
label-telescope = Telescoop
label-instrument = Instrument
label-filter = Filter
label-observed = Waargenomen
label-exposure-time = Belichting
label-sample = Monsters
label-unit = Eenheid
label-hdu = Extensie
label-centre = Centrum
label-centre-deg = Centrum (graden)
label-frame = Coördinatenstelsel
# machine-drafted, needs native review
science-frame-galactic = Galactisch
science-frame-ecliptic = Ecliptisch
label-projection = Projectie
label-pixel-scale = Pixelschaal
science-seconds = { $seconds } s
science-arcsec-px = { $arcsec }″ per pixel
label-distortion = Vervorming
label-wcs = Hemelcoördinaten
science-wcs-unsupported = Niet ondersteund: { $reason }
# machine-drafted, needs native review
science-wcs-approximate = Bij benadering (alleen pointing, zonder vervorming)
label-observer = Waarnemer
label-bayer = Bayerpatroon
label-capture-start = Opnamebegin (UTC)
label-capture-end = Opname-einde (UTC)
label-frame-rate = Beeldsnelheid
science-fps = { $fps } beelden/s
science-frames-cut = { $frames } van { $declared }
# The HDF5 dataset or NetCDF variable shown.
label-dataset = Dataset
label-pixel-size = Pixelgrootte
# "0.25 nm per pixel"; $size may be "0.25 × 0.5".
science-pixel-size = { $size } { $unit } per pixel
# Microscopy (DigitalMicrograph, MRC, Gwyddion, Igor).
label-microscope = Microscoop
label-voltage = Versnellingsspanning
label-magnification = Vergroting
label-imaging-mode = Modus
label-detector = Detector
label-acquired = Opgenomen
label-field-of-view = Beeldveld
science-kilovolts = { $kv } kV
science-magnification = { $value }×
label-vendor = Fabrikant
label-scanner = Scanner
label-objective = Objectief
label-pyramid-levels = Niveaus
# machine-drafted, needs native review
label-emissivity = Emissiviteit
label-object-distance = Objectafstand
label-reflected-temperature = Gereflecteerde temperatuur
label-atmospheric-temperature = Luchttemperatuur
label-humidity = Relatieve luchtvochtigheid
label-raw-range = Ruw bereik
label-temperature-range = Temperatuurbereik
thermal-temperature = { $value } { $unit }
thermal-metres = { $value } m
thermal-percent = { PERCENT($value) }
thermal-range = { $low } – { $high }
slide-levels = { $count }: { $sizes }
# X-ray diffraction: the experiment geometry from a detector frame's header.
label-wavelength = Golflengte
# Ångström, e.g. "0.97625 Å".
xtal-angstrom = { $value } Å
label-photon-energy = Fotonenergie
xtal-kev = { $value } keV
# Sample to detector distance.
label-detector-distance = Detectorafstand
xtal-mm = { $value } mm
# Where the direct beam meets the detector, in pixels from the image's top-left corner.
label-beam-centre = Bundelcentrum
xtal-beam-centre = x { $x }, y { $y } px
# The detector arm's swing angle (2θ).
label-detector-swing = Detector-2θ
xtal-degrees = { $value }°
# Goniometer angles (Greek letters are the field's usual names).
label-phi = Phi (φ)
label-omega = Omega (ω)
label-chi = Chi (χ)
label-kappa = Kappa (κ)
# The sample's rotation during the exposure: start angle plus width, e.g. "12.5° + 0.1°".
label-oscillation = Oscillatie
xtal-oscillation = { $start }° + { $width }°
# $axis is the axis name from the file, e.g. "phi" or "omega".
xtal-oscillation-axis = { $start }° + { $width }° om { $axis }
# The count at which a pixel saturates.
label-overload = Overloadwaarde
# The finest lattice spacing (d) recorded at the detector's edge and in its corners.
label-resolution = Resolutie
xtal-resolution = { $edge } Å aan de rand, { $corner } Å in de hoeken
label-geometry = Geometrie
xtal-curved = Gebogen image plate: geen q, d of resolutieringen
xtal-incomplete = Onvolledige header: geen q, d of resolutieringen
# All-fields group title: "FITS header".
fields-science-header = { $format }-header
# machine-drafted, needs native review
fields-asdf-tree = ASDF-boom
label-asdf-model = Datamodel
label-asdf-version = Bestandsversie
label-asdf-standard = ASDF-standaard
label-asdf-library = Geschreven door
# machine-drafted, needs native review
# All-fields group title for a CERN ROOT file's list of keys (objects).
fields-root-keys = ROOT-sleutels
# CERN ROOT files: the ROOT version that wrote the file (e.g. "6.28/04"), its compression,
# the number of keys (stored objects), and the object shown: class (e.g. "TH2F"), its path in
# the file, title, number of entries and sum of weights.
label-root-version = ROOT-versie
label-root-compression = Compressie
label-root-keys = Sleutels
label-root-class = Klasse
label-root-name = Naam
label-root-title = Titel
label-root-entries = Items
label-root-sum-weights = Som van de gewichten
# A graph's number of points.
label-root-points = Punten
# $axis is "x", "y" or "z".
label-root-axis = As { $axis }
# $quantity is the axis title with its unit, e.g. "Energy [GeV]"; $min and $max the axis
# limits.
root-axis-fixed = { $quantity }: { $bins ->
        [one] { $bins } bin
       *[other] { $bins } bins
    }, { $min } tot { $max }
root-axis-variable = { $quantity }: { $bins ->
        [one] { $bins } variabele bin
       *[other] { $bins } variabele bins
    }, { $min } tot { $max }
label-root-mean-rms = Gemiddelde en RMS { $axis }
root-mean-rms = { $mean } ± { $rms }
# Trees (event data) are listed only, with their entry counts.
label-root-trees = Bomen
root-tree = { $name } ({ $entries ->
        [one] { $entries } vermelding
       *[other] { $entries } vermeldingen
    })
# machine-drafted, needs native review
# CASA images: what the image is (from its directory name), its restoring beam (major and
# minor axis, position angle), the beams of its planes when they differ, the rest frequency,
# spectral reference frame (e.g. "LSRK") and velocity definition of the frequency labels,
# the default mask (e.g. "mask0") and the casacore storage manager with its byte order.
label-casa-type = Beeldtype
casa-type-image = Gerestaureerd beeld
casa-type-residual = Residu
casa-type-model = Model
casa-type-pb = Primaire bundel
casa-type-mask = Clean-masker
casa-type-psf = Puntspreidingsfunctie
casa-type-sumwt = Som van gewichten
casa-type-weight = Gewichten
label-casa-beam = Restauratiebundel
casa-beam = { $major }″ × { $minor }″, PA { $pa }°
label-casa-plane-beams = Bundels per vlak
casa-plane-beams = { $count ->
        [one] { $count } bundel
       *[other] { $count } bundels
    }, hoofdas { $min }″ tot { $max }″
label-casa-rest-frequency = Rustfrequentie
casa-ghz = { $ghz } GHz
label-casa-spectral-frame = Spectraal referentiestelsel
label-casa-velocity = Snelheidsdefinitie
casa-velocity-radio = Radio
casa-velocity-optical = Optisch
casa-velocity-relativistic = Relativistisch
label-casa-mask = Standaardmasker
label-casa-storage = Opslag
casa-storage-big = { $manager }, big-endian
casa-storage-little = { $manager }, little-endian
# machine-drafted, needs native review
# Unstructured meshes: the mesh's name in the file, the convention describing it (UGRID,
# ICON, MPAS), its counts, face shapes, coordinates and where the values sit.
label-ugrid-name = Rooster
label-ugrid-convention = Conventie
label-ugrid-nodes = Knopen
label-ugrid-faces = Vlakken
label-ugrid-edges = Randen
label-ugrid-shapes = Vlakvormen
label-ugrid-coordinates = Coördinaten
label-ugrid-values = Waarden op
label-ugrid-split = Gesplitst voor weergave
# $n is the formatted count, $count the number (for the plural).
ugrid-triangles = { $count ->
    [one] { $count } driehoek
   *[other] { $count } driehoeken
}
ugrid-quads = { $count ->
    [one] { $count } vierhoek
   *[other] { $count } vierhoeken
}
# Faces of five or more nodes; $max is the most nodes of one face.
ugrid-polygons = { $count ->
    [one] { $count } veelhoek met { $max } knopen
   *[other] { $count } veelhoeken met maximaal { $max } knopen
}
ugrid-lonlat = Lengte- en breedtegraad
ugrid-projected = Geprojecteerde x en y
ugrid-projected-unit = Geprojecteerde x en y ({ $unit })
ugrid-on-faces = Vlakken (egaal getekend)
ugrid-on-nodes = Knopen (geïnterpoleerd over de vlakken)
# A mesh file without data variables: the image shows how many nodes each face has.
ugrid-grid-only = Geen: de afbeelding toont de knopen per vlak
# Faces cut at the antimeridian or closed to a pole so they draw on a flat map.
ugrid-split = { $count ->
    [one] { $count } vlak bij de antimeridiaan of een pool
   *[other] { $count } vlakken bij de antimeridiaan of een pool
}
# machine-drafted, needs native review
# Slide annotations (NDP.view, Aperio ImageScope, ASAP sidecar files).
label-annotation-file = Bestand
# "slide.xml (Aperio ImageScope)"
annotation-source = { $file } ({ $format })
label-annotation-shapes = Vormen
label-annotation-groups = Groepen
# Shapes or points over the reader's limits, left out.
label-annotation-skipped = Weggelaten
annotation-polygon = Veelhoek
annotation-polyline = Lijn
annotation-rectangle = Rechthoek
annotation-ellipse = Ellips
annotation-point = Punt
annotation-points = Punten
annotation-ruler = Liniaal
annotation-arrow = Pijl
# The list of annotations is cut short: "and 1,200 more".
annotation-more = en nog { $count }
# All-fields group title: "Annotations in slide.xml".
fields-annotations-header = Annotaties in { $file }
label-drawing-version = Versie
label-drawing-units = Eenheden
label-drawing-extents = Omvang
label-drawing-sheet = Blad
label-drawing-layouts = Lay-outs
label-drawing-layers = Lagen
label-drawing-entities = Entiteiten
label-drawing-codepage = Codetabel
label-drawing-missing-fonts = Vervangen lettertypen
# machine-drafted, needs native review
# A font file the drawing names that was not found: the styles naming it, and the font drawing its text instead.
drawing-missing-font = { $file } (stijlen { $styles }), getekend met { $substitute }
# A Big Font (double-byte text) that was not found; its characters come from other fonts.
drawing-missing-bigfont = { $file } (stijlen { $styles }), getekend met vervangende lettertypen
# Survey data (LandXML).
label-survey-crs = Coördinatenstelsel
survey-crs-none = Niet opgegeven
label-survey-project = Projectnaam
label-survey-application = Geschreven door
label-survey-surface = Terreinmodel
# A TIN surface: its name, counts, plan and sloped areas, the volume above its lowest point, its elevations and the contour interval drawn; unit is "m" or "ft" (areas in square units, volume in cubic units).
survey-surface = { $name }: { $points ->
        [one] { $points } punt
       *[other] { $points } punten
    }, { $triangles ->
        [one] { $triangles } driehoek
       *[other] { $triangles } driehoeken
    }; oppervlakte in plan { $area } { $unit }², werkelijke oppervlakte { $area3d } { $unit }²; { $volume } { $unit }³ boven het laagste punt; hoogten { $zmin }–{ $zmax } { $unit }; hoogtelijnen om de { $interval } { $unit }
label-survey-alignment = As
# An alignment: its name, length and first and last stations ("1+234.50").
survey-alignment = { $name }: { $length } { $unit }, kilometrering { $start } tot { $end }
# SHX shape fonts found and used for the drawing's text.
label-drawing-shx-fonts = SHX-lettertypen
label-drawing-warnings = Problemen
label-plot-paper = Papier
# "2 of 5": the page shown and how many the plot has.
plot-page-of = { $page } van { $pages }
label-plot-page = Pagina
label-plot-pens = Gebruikte pennen
# The printer language a plot came wrapped in, e.g. "PCL 5".
label-plot-job = Afdruktaak
# What a plot draws: strokes, fills and labels.
label-plot-items = Getekende elementen
# Width × height of what is drawn, in drawing units: "1,250 × 800 mm".
drawing-extents = { $width } × { $height } { $unit }
drawing-extents-unitless = { $width } × { $height }
drawing-model-space = Model
# "24, 3 off or frozen"
drawing-layers-off = { $num }, { $off } uit of bevroren
# DXF $INSUNITS.
drawing-units = { $code ->
    [1] Inches
    [2] Voet
    [3] Mijlen
    [4] Millimeter
    [5] Centimeter
    [6] Meter
    [7] Kilometer
    [8] Micro-inches
    [9] Mils
    [10] Yards
    [11] Ångström
    [12] Nanometer
    [13] Micron
    [14] Decimeter
    [15] Decameter
    [16] Hectometer
    [17] Gigameter
    [18] Astronomische eenheden
    [19] Lichtjaren
    [20] Parsec
    [21] Amerikaanse landmeetvoet
   *[other] Zonder eenheid
}
# All-fields group title: "DXF entities".
fields-drawing-entities = { $format }-entiteiten

## IC-layouts (GDSII)

# machine-drafted, needs native review
label-layout-library = Bibliotheek
label-layout-units = Eenheden
# The database unit (the grid coordinates are stored in) and the user unit, with SI symbols:
# "1 nm database unit, 1 µm user unit".
layout-units = { $db } database-eenheid, { $user } gebruikerseenheid
# The top cell shown (each top cell is a page).
label-layout-cell = Cel
label-layout-cells = Cellen
# Cells no other cell places.
label-layout-top-cells = Topcellen
# "TOP, TEST, PAD and 12 more"
layout-top-cells-more = { $names } en nog { $more }
# Layer/datatype pairs.
label-layout-layers = Lagen
# Polygons, paths and boxes, as stored (each cell once).
label-layout-shapes = Vormen
label-layout-texts = Labels
# Cell references and the placements they make: "1,204 (88,320 placements)".
label-layout-references = Verwijzingen
layout-references = { $refs } ({ $placements } { $placements ->
    [one] plaatsing
   *[other] plaatsingen
})
# Shapes with every cell placement expanded.
label-layout-flat-shapes = Vormen na afvlakken
label-layout-bounds = Begrenzing
# Corners of what the cell draws, micrometres: "-12.5, 0 to 2,900, 3,500 µm".
layout-bounds = { $x0 }, { $y0 } tot { $x1 }, { $y1 } µm

## Printplaten en schema’s (Gerber, Excellon, KiCad)

# machine-drafted, needs native review
group-board = Printplaat
group-schematic = Schema
# The board view shown (its layers, its top or bottom), or the schematic sheet.
label-pcb-view = Weergave
label-pcb-project = Project
label-pcb-revision = Revisie
label-pcb-date = Datum
# The CAD program that wrote the file.
label-pcb-generator = Gemaakt met
# Width × height of the board outline: "100 × 80 mm".
label-pcb-size = Afmetingen printplaat
label-pcb-thickness = Dikte
label-pcb-copper-layers = Koperlagen
label-pcb-sheets = Bladen
# The board's layers or files, each with its role: "board-F_Cu.gbr (Top copper)".
label-pcb-layers = Lagen
pcb-layer-role = { $name } ({ $role })
label-pcb-components = Footprints
label-pcb-symbols = Symbolen
label-pcb-nets = Netten
label-pcb-tracks = Sporen
label-pcb-wires = Draden
label-pcb-vias = Via’s
# Drilled holes and how many drill sizes: "1,204 in 9 sizes".
label-pcb-holes = Gaten
pcb-holes = { $holes } in { $sizes } { $sizes ->
    [one] maat
   *[other] maten
}
label-pcb-apertures = Apertures
# What a board layer or file is, shown after its name.
pcb-role-inner-copper = Binnenlaag koper { $n }
pcb-role-top-copper = Koper boven
pcb-role-bottom-copper = Koper onder
pcb-role-top-solder-mask = Soldeermasker boven
pcb-role-bottom-solder-mask = Soldeermasker onder
pcb-role-top-silkscreen = Zeefdruk boven
pcb-role-bottom-silkscreen = Zeefdruk onder
pcb-role-top-paste = Pasta boven
pcb-role-bottom-paste = Pasta onder
pcb-role-top-solder-paste = Soldeerpasta boven
pcb-role-bottom-solder-paste = Soldeerpasta onder
pcb-role-top-adhesive = Lijm boven
pcb-role-bottom-adhesive = Lijm onder
pcb-role-top-courtyard = Courtyard boven
pcb-role-bottom-courtyard = Courtyard onder
pcb-role-top-fabrication = Fabricage boven
pcb-role-bottom-fabrication = Fabricage onder
pcb-role-board-outline = Omtrek printplaat
pcb-role-plated-drill = Doorgemetalliseerde boringen
pcb-role-non-plated-drill = Niet-doorgemetalliseerde boringen
pcb-role-drill = Boringen
pcb-role-drill-holes = Boorgaten
pcb-role-mechanical = Mechanisch
pcb-role-margin = Marge
pcb-role-drawings = Tekeningen
pcb-role-comments = Opmerkingen
pcb-role-engineering-change-1 = Technische wijziging 1
pcb-role-engineering-change-2 = Technische wijziging 2
pcb-role-vias = Via’s
pcb-role-other = Overig
pcb-role-unknown = Onbekend
label-pcb-macros = Aperture-macro’s

## Spectra (JCAMP-DX, SPC, mzML, mzXML, Bruker NMR)

# machine-drafted, needs native review
group-spectrum = Spectrum
label-spectrum-format = Indeling
label-spectrum-technique = Techniek
label-spectrum-title = Titel
# The page shown: "3 of 1,204: Scan 1532".
label-spectrum-page = Pagina
spectrum-page-of = { $page } van { $pages }: { $name }
# An axis with its unit: "Wavenumber (cm⁻¹)".
spectrum-axis = { $title } ({ $unit })
label-spectrum-x = X-as
label-spectrum-y = Y-as
label-spectrum-nucleus = Kern
label-spectrum-frequency = Spectrometerfrequentie
label-spectrum-solvent = Oplosmiddel
label-spectrum-pulse-program = Pulsprogramma
label-spectrum-spectra = Spectra
label-spectrum-points = Punten
label-spectrum-instrument = Instrument
# Who or what lab measured it.
label-spectrum-origin = Herkomst
label-spectrum-date = Gemeten
# How Specular made the page from the raw data (an FID transformed, phased automatically).
label-spectrum-processing = Verwerking
spectrum-technique-ir = Infrarood
spectrum-technique-raman = Raman
spectrum-technique-uv-vis = UV-Vis
spectrum-technique-fluorescence = Fluorescentie
spectrum-technique-nmr = NMR
spectrum-technique-ms = Massaspectrum
spectrum-technique-chromatogram = Chromatogram
spectrum-technique-other = Spectrum

## 3D-modellen (STL, OBJ, PLY, glTF, 3MF)

# machine-drafted, needs native review
group-mesh = 3D-model
label-triangles = Driehoeken
label-vertices = Hoekpunten
label-materials = Materialen
label-textures = Texturen
label-up-axis = Opwaartse as
# Textures the model uses, some of which could not be read: "3 (1 missing)".
mesh-textures-missing = { $count } ({ $missing } ontbreken)

## Molecuulstructuren (PDB, mmCIF, CIF, SDF, XYZ)

# machine-drafted, needs native review
group-structure = Structuur
# The entry's identifier: a PDB ID such as 1CRN, or the CIF data block's name.
label-structure-id = Item
label-structure-classification = Classificatie
# How the structure was determined: X-ray diffraction, electron microscopy, solution NMR…
label-structure-method = Methode
# The crystallographic or cryo-EM resolution, in ångströms.
label-structure-resolution = Resolutie
structure-resolution = { $value } Å
label-structure-deposited = Gedeponeerd
label-structure-formula = Formule
label-structure-chains = Ketens
# Residues (amino acids, nucleotides) in the polymer chains.
label-structure-residues = Residuen
label-structure-atoms = Atomen
# Atoms after the unit cell was filled by symmetry, and those the file lists: "96 (12 in the asymmetric unit)".
structure-atoms-cell = { $count } ({ $asymmetric } in de asymmetrische eenheid)
label-structure-bonds = Bindingen
# Models in the file (an NMR ensemble), of which the first is shown.
label-structure-models = Modellen
# Molecules (SDF records) or data blocks in the file, of which the first is shown.
label-structure-records = Records
structure-first-of = { $count } (de eerste wordt getoond)
# Small molecules bound to the polymer, by their three-letter codes.
label-structure-ligands = Liganden
# A list cut short: "HEM, NAG, SO4 and 9 more".
structure-ligands-more = { $list } en nog { $more }
label-structure-cell = Eenheidscel
# Unit cell edge lengths (Å) and angles (degrees).
structure-cell = a = { $a } Å, b = { $b } Å, c = { $c } Å, α = { $alpha }°, β = { $beta }°, γ = { $gamma }°
label-structure-space-group = Ruimtegroep
label-structure-secondary = Secundaire structuur
structure-ss-file = Uit het bestand
structure-ss-computed = Berekend uit de hoofdketen

## HDF4 en HDF-EOS2

# machine-drafted, needs native review
group-hdf4 = HDF4
# The HDF library version that wrote the file, e.g. "NCSA HDF Version 4.2 Release 15".
label-hdf4-library = Bibliotheek
# Counts of the file's scientific datasets, raster images, groups (Vgroups) and tables (Vdatas).
label-hdf4-datasets = Datasets
label-hdf4-images = Rasterafbeeldingen
label-hdf4-vgroups = Groepen
label-hdf4-vdatas = Tabellen
# HDF-EOS: NASA's Earth Observing System conventions on top of HDF4 (MODIS, ASTER, AIRS…).
group-hdf-eos = HDF-EOS
label-eos-version = Versie
# The grids, swaths and point structures the file defines.
label-eos-structures = Structuren
eos-grids = { $count ->
    [one] { $count } raster ({ $names })
   *[other] { $count } rasters ({ $names })
}
eos-swaths = { $count ->
    [one] { $count } scanstrook ({ $names })
   *[other] { $count } scanstroken ({ $names })
}
eos-points = { $count ->
    [one] { $count } puntstructuur
   *[other] { $count } puntstructuren
}
# The grid or swath the dataset shown belongs to.
label-eos-structure = Structuur
# The grid's map projection as its GCTP name, e.g. GCTP_SNSOID (sinusoidal).
label-eos-projection = Projectie
label-eos-grid-size = Rastergrootte
# Columns × rows.
eos-grid-size = { $width } × { $height }
# The grid's upper-left and lower-right corners.
label-eos-corners = Hoeken
# The product's short name from the inventory metadata, e.g. MOD09A1.
label-eos-product = Product
# When the data was acquired.
label-eos-time = Tijd
eos-time = { $begin } tot { $end }

## Planetaire afbeeldingen (VICAR, PDS3, PDS4: missiedata van NASA en ESA)

# machine-drafted, needs native review
group-planetary = Planetaire afbeelding
# The body observed, e.g. MARS, Ganymede.
label-planetary-target = Doel
label-planetary-mission = Missie
# The spacecraft, rover or lander carrying the instrument.
label-planetary-spacecraft = Ruimtevaartuig
label-planetary-start = Begintijd
label-planetary-stop = Eindtijd
# The product's identifier in the archive.
label-planetary-product = Product-ID
# The map projection of a map-projected image, as the label names it.
label-planetary-projection = Kaartprojectie
label-planetary-resolution = Resolutie
planetary-metres-per-pixel = { $metres } m per pixel
planetary-pixels-per-degree = { $pixels } pixels per graad
# Latitude and longitude of the map projection's centre.
label-planetary-centre = Projectiecentrum
planetary-lat-lon = { $lat }°, { $lon }°
# The latitudes and longitudes the map covers.
label-planetary-coverage = Dekking
planetary-coverage = breedte { $south }° tot { $north }°, lengte { $west }° tot { $east }°
# The body's equatorial radius the map uses.
label-planetary-radius = Straal van het hemellichaam
planetary-km = { $km } km

## OMERO (afbeeldingen geopend van een OMERO-server of de Image Data Resource)

# machine-drafted, needs native review
group-omero = OMERO
label-omero-server = Server
# $server the server's address; $user the user name signed in with.
omero-server-user = { $server } (als { $user })
omero-server-public = { $server } (openbaar)
label-omero-image = Afbeelding
# $name the image's name; $id its number on the server.
omero-image = { $name } (ID { $id })
# The project and dataset the image is in, as "Project › Dataset".
label-omero-project = Project
# The screen, plate and well, as "Screen › Plate › B4".
label-omero-screen = Screen
label-omero-owner = Eigenaar
# $owner the image's owner; $group the OMERO group it belongs to.
omero-owner-group = { $owner } ({ $group })
label-omero-acquired = Opgenomen
label-omero-dimensions = Afmetingen
omero-size = { $width } × { $height }
omero-z = { $count ->
    [one] { $count } Z-vlak
   *[other] { $count } Z-vlakken
}
omero-c = { $count ->
    [one] { $count } kanaal
   *[other] { $count } kanalen
}
omero-t = { $count ->
    [one] { $count } tijdstip
   *[other] { $count } tijdstippen
}
# The physical size of a pixel along X, Y (and Z).
label-omero-physical = Pixelgrootte
label-omero-channels = Kanalen
# A channel not shown in the server's rendering.
omero-channel-hidden = { $channel } (verborgen)
# How the pixels are fetched from the server: rendered tiles (JPEG) or the whole plane (PNG).
label-omero-fetched = Opgehaald als
omero-fetched-tiles = { $encoding }-tegels van { $width } × { $height }, { $levels ->
    [one] { $levels } niveau
   *[other] { $levels } niveaus
}
omero-fetched-whole = Volledige vlakken als { $encoding }

## Reflectietransformatiebeelden (PTM, RTI, Relight)

# machine-drafted, needs native review
group-rti = Reflectietransformatie
# The reflectance model the coefficients belong to: PTM (polynomial), HSH (hemispherical harmonics), RBF…
label-rti-basis = Basis
label-rti-order = Orde
# Coefficients per colour channel.
label-rti-terms = Coëfficiënten
# How colour is stored: LRGB (one colour, varying luminance), RGB, YCC…
label-rti-colour = Kleurindeling
# Coefficient images stored in the file.
label-rti-planes = Vlakken
# The byte-to-coefficient mapping the file states, per coefficient.
label-rti-scale = Schaal
label-rti-bias = Verschuiving

## Puntenwolken

# machine-drafted, needs native review
group-points = Puntenwolk
label-points = Punten
# Points read of those in the file, when a very large cloud was evenly thinned: "32,000,000 of 120,000,000".
points-subsampled = { $loaded } van { $total }
# The LAS point data record format number (0–10).
label-point-format = Puntformaat
# The coordinate reference system.
label-crs = Coördinatenstelsel
# What each point carries besides its position.
label-attributes = Attributen
points-channel-rgb = RGB
points-channel-intensity = Intensiteit
points-channel-classification = Classificatie
points-channel-returns = Echo’s
# ASPRS classes present, with point counts: "Ground (1,234), Building (567)".
label-classes = Klassen
# E57 scans, with point counts.
label-scans = Scans
point-class-0 = Nooit geclassificeerd
point-class-1 = Niet geclassificeerd
point-class-2 = Maaiveld
point-class-3 = Lage vegetatie
point-class-4 = Middelhoge vegetatie
point-class-5 = Hoge vegetatie
point-class-6 = Gebouw
point-class-7 = Laag punt (ruis)
point-class-8 = Modelsleutelpunt
point-class-9 = Water
point-class-10 = Spoorweg
point-class-11 = Wegdek
point-class-12 = Overlap
point-class-13 = Bliksemdraad
point-class-14 = Geleider
point-class-15 = Hoogspanningsmast
point-class-16 = Draadverbinding
point-class-17 = Brugdek
point-class-18 = Hoge ruis
point-class-19 = Bovenliggende constructie
point-class-20 = Genegeerd maaiveld
point-class-21 = Sneeuw
point-class-22 = Tijdelijke uitsluiting
# A class without a standard name, by number.
point-class-other = Klasse { $class }

## Gaussian splats

# machine-drafted, needs native review
group-splats = Gaussische splats
label-splats = Splats
# Splats shown of those in the file, when a very large file was evenly thinned: "12,000,000 of 30,000,000".
splats-shown-of = { $shown } van { $total }
# The degree (1 to 3) of the spherical harmonics that give each splat its view-dependent colour.
label-splat-harmonics = Kijkrichtingsafhankelijke kleur
splats-harmonics-none = Geen
# $format: the file format, e.g. "SPZ".
splats-format-antialiased = { $format }, getraind met anti-aliasing

## Waarschuwingen

# machine-drafted, needs native review
warn-not-diagnostic = Niet voor diagnostisch gebruik. Specular is een algemene afbeeldingsviewer, geen medisch hulpmiddel: de weergave is niet gekalibreerd voor diagnose.

warn-gps = Dit bestand legt vast waar het is gemaakt. Verwijder de locatie voor het delen als die privé moet blijven.
warn-null-island = De GPS-positie is precies 0°, 0°; de camera had waarschijnlijk geen fix, dus de locatie klopt vermoedelijk niet.
warn-date-future = De opnamedatum ligt in de toekomst; de cameraklok stond waarschijnlijk verkeerd.
warn-icc-damaged = Het ingesloten kleurprofiel is beschadigd en kan niet worden gelezen.
warn-metadata-failed = De metadata konden door een fout in Specular niet worden gelezen. De afbeelding zelf is in orde.
warn-orientation-conflict = EXIF en XMP zijn het niet eens over de oriëntatie (EXIF: { $exif }, XMP: { $xmp }). De EXIF-waarde wordt gebruikt.
warn-exif-damaged = Sommige EXIF-gegevens kunnen niet worden gelezen ({ $error }); de rest wordt getoond.
warn-xmp-damaged = De XMP-metagegevens zijn misvormd ({ $error }); slechts een deel kan worden gelezen.

## Over

geonames-attribution = Plaatsnamen: GeoNames (www.geonames.org), gelicentieerd onder Creative Commons Naamsvermelding 4.0 (CC BY 4.0, https://creativecommons.org/licenses/by/4.0/). Offline opzoeken met de crate reverse_geocoder van Grant Miner (MIT of Apache-2.0). De gegevens zijn omgezet naar een lijst met plaatsen; ze worden geleverd zoals ze zijn, zonder garantie.

## TIFF and DNG structure values (All fields)
# machine-drafted, needs native review

subfile-type = { $value ->
    [0] Afbeelding op volledige resolutie
    [1] Afbeelding op verlaagde resolutie
    [2] Eén pagina van een afbeelding met meerdere pagina’s
    [4] Transparantiemasker
   *[other] Overig
}
predictor = { $value ->
    [1] Geen
    [2] Horizontaal verschil
    [3] Drijvendekommaverschil
   *[other] Overig
}
extra-samples = { $value ->
    [0] Niet opgegeven
    [1] Voorvermenigvuldigde alfa
    [2] Rechte alfa
   *[other] Overig
}
sample-format = { $value ->
    [1] Geheel getal zonder teken
    [2] Geheel getal met teken
    [3] Drijvende komma
    [4] Niet gedefinieerd
   *[other] Overig
}

## Geodata (GeoTIFF, GeoJSON, Shapefile, GeoPackage, KML/KMZ, GPX)

# machine-drafted, needs native review
group-geospatial = Geografisch
label-geo-crs = Coördinatenstelsel
label-geo-datum = Datum
label-geo-projection = Projectie
label-geo-unit = Eenheden
label-geo-bounds = Omvang
label-geo-lonlat-bounds = Lengte- en breedtegraad
label-geo-pixel-size = Pixelgrootte
label-geo-overviews = Overzichten
label-geo-nodata = NoData-waarde
label-geo-raster-type = Rastertype
label-geo-vertical = Verticale referentie
label-geo-citation = Bronvermelding
label-geo-features = Objecten
label-geo-vertices = Hoekpunten
label-geo-layer = Laag
label-geo-layers = Lagen
label-geo-tiles = Tegels
label-geo-encoding = Tekstcodering
label-geo-warnings = Problemen
# The companion files a georeference was read from ("scene.jgw, scene.prj").
label-geo-sidecars = Begeleidende bestanden
# machine-drafted, needs native review
# A projection Specular cannot convert to longitude and latitude: $method is its name.
geo-projection-unsupported = { $method } (coördinaten niet ondersteund)
# How longitude and latitude are brought to WGS 84 (what GPS and web maps use).
label-geo-to-wgs84 = Naar WGS 84
geo-to-wgs84-same = Niet nodig
geo-to-wgs84-helmert = Datumverschuiving (Helmert)
geo-to-wgs84-by-area = Datumverschuiving (Helmert, gekozen op gebied)
geo-to-wgs84-unknown = Onbekend: coördinaten staan in het eigen datum van het bestand
# A coordinate system's name and its EPSG code: "WGS 84 / UTM zone 33N (EPSG:32633)".
geo-crs-epsg = { $name } (EPSG:{ $code })
geo-crs-unknown = Niet opgegeven
# Map bounds: the lower-left and upper-right corners.
geo-bounds = { $x0 }, { $y0 } tot { $x1 }, { $y1 }
# Bounds in degrees: west to east longitude, south to north latitude.
geo-lonlat-bounds = { $west }° tot { $east }° lengte, { $south }° tot { $north }° breedte
geo-pixel-size = { $x } × { $y }
geo-overviews = { $n ->
    [one] { $n } overzicht: { $sizes }
   *[other] { $n } overzichten: { $sizes }
}
geo-pixel-is-point = Pixels zijn punten
geo-layer = { $features ->
    [one] { $features } object
   *[other] { $features } objecten
}, { $fields ->
    [one] { $fields } veld
   *[other] { $fields } velden
}
# A GeoPackage tile pyramid: its table, the zoom level shown and its tile count.
geo-tiles = { $table }, zoomniveau { $zoom }, { $tiles ->
    [one] { $tiles } tegel
   *[other] { $tiles } tegels
}
fields-geo-geokeys = GeoTIFF-sleutels
fields-geo-gdal = GDAL-metadata
fields-geo-wkt = Coördinatenstelsel (WKT)
fields-geo-projjson = Coördinatenstelsel (PROJJSON)
# machine-drafted, needs native review
# What a web map tile set (MBTiles, PMTiles) says about itself: container, tile type, zoom
# levels, description and other metadata.
fields-geo-tileset = Tegelset
# The credit line a map's data asks to be shown with it ("© OpenStreetMap contributors").
label-geo-attribution = Bronvermelding
# The header lines of a georeferenced raster format (ENVI, ASCII grid, DTED, ERDAS IMAGINE).
fields-geo-header = Header
# machine-drafted, needs native review
# GIFTI files: the format version, what the file holds, the number of data arrays and the
# encoding of their values (e.g. "GZipBase64Binary").
label-gifti-version = GIFTI-versie
label-gifti-role = Bevat
gifti-role-surface = Oppervlak
gifti-role-labels = Labels per hoekpunt
gifti-role-data = Waarden per hoekpunt
label-gifti-arrays = Data-arrays
label-gifti-encoding = Codering
label-gifti-vertices = Hoekpunten
label-gifti-triangles = Driehoeken
# The brain structure, e.g. "CortexLeft".
label-gifti-structure = Structuur
# A surface's geometric type, e.g. "Anatomical", "Inflated".
label-gifti-geometry = Geometrisch type
# The coordinate transform of the vertices: $from and $to are coordinate spaces such as
# "scanner", "Talairach" or "MNI 152".
label-gifti-transform = Coördinaten
gifti-transform = { $from } (transformatie naar { $to } niet nodig)
gifti-transform-applied = { $from } → { $to } (toegepast)
# The surface file a data file is drawn on, found beside it.
label-gifti-surface = Getoond op
# The data array shown: its name, NIfTI intent (e.g. "NIFTI_INTENT_SHAPE") and data type.
label-gifti-array = Array
label-gifti-intent = Intent
label-gifti-data-type = Gegevenstype
# Entries of the label table (names and colours of regions).
label-gifti-labels = Labeltabel
# CIFTI-2 files: the version, NIfTI intent (e.g. "ConnDenseScalar") and matrix size.
label-cifti-version = CIFTI-versie
label-cifti-intent = Intent
label-cifti-matrix = Matrix
cifti-matrix = { $rows } × { $columns }
# The maps (pages) of the file and what they are.
label-cifti-maps = Kaarten
cifti-maps-scalars = { $count ->
        [one] { $count } scalaire kaart
       *[other] { $count } scalaire kaarten
    }
cifti-maps-labels = { $count ->
        [one] { $count } labelkaart
       *[other] { $count } labelkaarten
    }
cifti-maps-series = { $count ->
        [one] { $count } tijdpunt
       *[other] { $count } tijdpunten
    }
cifti-maps-parcels = { $count ->
        [one] { $count } parcel
       *[other] { $count } parcellen
    }
cifti-maps-other = { $count }
# machine-drafted, needs native review
# A dense connectivity file (.dconn.nii): every brain place's connectivity to every other.
# $seeds: how many places can be picked; $size: the bytes read when one is picked (e.g. "357 KB").
label-cifti-dense = Connectiviteit
cifti-dense = { $seeds ->
        [one] { $seeds } seed
       *[other] { $seeds } seeds
    }, { $size } gelezen per seed
# A time series' start and step; $unit is e.g. "second".
label-cifti-series = Reeks
cifti-series = vanaf { $start }, elke { $step } ({ $unit })
label-cifti-models = Hersenmodellen
label-cifti-parcels = Parcellen
label-cifti-structures = Structuren
label-cifti-voxels = Subcorticale voxels
# A surface the file's values lie on: the brain structure (e.g. "CORTEX_LEFT"), its vertex
# count, and the surface file found beside the file for it, or that none was found.
label-cifti-surface = Oppervlak
cifti-surface-found = { $structure }: { $vertices ->
        [one] { $vertices } hoekpunt
       *[other] { $vertices } hoekpunten
    }, { $file }
cifti-surface-missing = { $structure }: heeft een oppervlak met { $vertices ->
        [one] { $vertices } hoekpunt
       *[other] { $vertices } hoekpunten
    } nodig (geen naast het bestand)
# How the file is shown: on the surfaces found beside it, or as its subcortical voxels.
label-cifti-shown = Getoond als
cifti-shown-surfaces = Gekleurde oppervlakken
cifti-shown-voxels = Volume van de subcorticale voxels
# machine-drafted, needs native review
# Shown with a picked seed's row: click a surface vertex, or move the crosshair in MPR.
cifti-shown-connectivity = Connectiviteit van de gekozen seed
# Maps left out of the voxel volume, which would be too large with them.
label-cifti-left-out = Weggelaten kaarten
# Tractograms: format, number of streamlines (fibre tracts) and points.
label-tract-format = Formaat
label-tract-streamlines = Streamlines
# A large tractogram draws only some of its streamlines.
tract-streamlines-shown = { $count } ({ $shown } getoond)
label-tract-points = Punten
label-tract-mean-length = Gemiddelde lengte
# A length in millimetres.
tract-mm = { $value } mm
# Values per point and per streamline the file stores, by name.
label-tract-scalars = Scalairen per punt
label-tract-properties = Eigenschappen per streamline
# TrackVis: the voxel grid the tracts were made in, its voxel size and voxel order (e.g. "LAS").
label-tract-grid = Raster
label-tract-voxel-size = Voxelgrootte
label-tract-voxel-order = Voxelvolgorde
# How the points were brought into scanner (RAS) millimetres.
label-tract-space = Coördinaten
tract-space-vox-to-ras = Uit de voxel-naar-RAS-matrix
tract-space-voxel-order = Uit de voxelvolgorde (geen voxel-naar-RAS-matrix)
tract-space-scanner = Scannerruimte
# MRtrix: the tracking step size.
label-tract-step = Stapgrootte

## File headers (DPX, Cineon, XWD, MIFF, TIM, WSQ, JBIG)

# machine-drafted, needs native review
# What the file's own header records, for formats without Exif or XMP.
group-header = Bestandsheader
label-header-created = Gemaakt
# The program or person that wrote the file.
label-header-creator = Maker
label-header-copyright = Copyright
label-header-project = Project
label-header-comment = Opmerking
# The scanner, camera or other device the image came from.
label-header-device = Apparaat
# SMPTE time code of the frame: "01:00:00:12".
label-header-timecode = Tijdcode
# A film frame's edge code (KeyKode).
label-header-edge-code = Randnummer
label-header-frame-rate = Beeldsnelheid
# Where the frame sits in its sequence.
label-header-frame = Frame
# Film stock and slate.
label-header-film = Film
# How code values relate to light: printing density, linear, BT.709…
label-header-transfer = Overdrachtsfunctie
# The colour primaries the file states.
label-header-colorimetric = Colorimetrie
# The X window a screen dump was taken of.
label-header-window = Venster
label-header-resolution = Resolutie
label-header-compression = Compressie
# machine-drafted, needs native review
# JPEG XS: the profile and the level the codestream states ("Main 422.10"; "4k-1, sublevel 6 bpp").
label-header-profile = Profiel
# machine-drafted, needs native review
label-header-level = Niveau
# machine-drafted, needs native review
# JPEG XS: how many wavelet levels, horizontal and vertical ("5 horizontal, 1 vertical").
label-header-decomposition = Waveletniveaus
# machine-drafted, needs native review
# JPEG XS: the colour transform ("None", "Reversible colour transform (RCT)", "Star-Tetrix, RGGB mosaic").
label-header-colour-transform = Kleurtransformatie
# machine-drafted, needs native review
# Photoshop: the artboards, by name and size ("Phone (375 × 667), Tablet (768 × 1024)").
label-header-artboards = Tekengebieden
# machine-drafted, needs native review
# Photoshop file without a merged image, shown from its layers: the features it leaves out,
# by Photoshop's names ("Color Lookup, Knockout").
label-header-not-drawn = Niet getekend
# machine-drafted, needs native review
# The same, for the features drawn only approximately ("Bevel & Emboss, Satin").
label-header-approximated = Bij benadering getekend

## Fonts (TTF, OTF, TTC/OTC, WOFF, WOFF2)

# machine-drafted, needs native review
group-font = Lettertype
label-font-family = Familie
label-font-style = Stijl
# The face's place in a font collection: "2 of 4".
label-font-face = Letterbeeld
font-face-of = { $index } van { $count }
label-font-version = Versie
label-font-designer = Ontwerper
# The font's maker (the name table's manufacturer).
label-font-manufacturer = Lettergieterij
label-font-glyphs = Glyfen
label-font-outlines = Contouren
font-outlines-truetype = TrueType (kwadratisch)
font-outlines-cff = PostScript (CFF, kubisch)
font-outlines-cff2 = PostScript (CFF2, kubisch)
font-outlines-bitmap = Alleen bitmaps
font-outlines-none = Geen
# What the font's licence flags (OS/2 fsType) allow documents that embed it to do.
label-font-embedding = Insluiten
font-embedding-installable = Mag worden ingesloten en geïnstalleerd.
font-embedding-restricted = Mag niet worden ingesloten.
font-embedding-preview-print = Mag alleen worden ingesloten om te bekijken en af te drukken.
font-embedding-editable = Mag worden ingesloten in documenten die worden bewerkt.
font-embedding-no-subsetting = Alleen volledig, niet als subset.
font-embedding-bitmap-only = Alleen als bitmaps.
# Variation axes of a variable font: "Weight 100–900 (400)".
label-font-axes = Variatieassen
font-axis = { $name } { $min }–{ $max } ({ $default })
label-font-instances = Benoemde stijlen
label-font-colour = Kleurglyphs
font-colour-colr0 = COLR-lagen
font-colour-colr1 = COLR v1-verlopen
font-colour-sbix = Bitmaps (sbix)
font-colour-cbdt = Bitmaps (CBDT)
font-colour-svg = SVG
label-font-weight = Gewicht
label-font-units-per-em = Eenheden per em
label-font-tables = Tabellen
label-font-licence = Licentie
label-font-licence-url = Licentie-URL
label-font-copyright = Copyright
label-font-trademark = Handelsmerk

## Office documents (Word, Excel, PowerPoint, iWork, OpenDocument)

# machine-drafted, needs native review
group-document = Document
label-document-kind = Soort
document-kind-word = Word-document
document-kind-excel = Excel-werkmap
document-kind-powerpoint = PowerPoint-presentatie
document-kind-pages = Pages-document
document-kind-numbers = Numbers-spreadsheet
document-kind-keynote = Keynote-presentatie
document-kind-odf-text = OpenDocument-tekst
document-kind-odf-spreadsheet = OpenDocument-spreadsheet
document-kind-odf-presentation = OpenDocument-presentatie
document-kind-odf-graphics = OpenDocument-tekening
document-kind-odf-other = OpenDocument-bestand
# machine-drafted, needs native review
document-kind-playlist = Afspeellijst
# machine-drafted, needs native review
label-playlist-items = Items
# Appended to the kind: "Word document, template, with macros".
document-template = sjabloon
document-macros = met macro’s
document-legacy = Office 97–2003-indeling
label-document-preview = Voorbeeld
# The image format of the preview stored in the file, e.g. "JPEG image".
document-preview-image = { $format }-afbeelding
document-preview-none = Geen in dit bestand
label-document-subject = Onderwerp
label-document-author = Auteur
label-document-description = Beschrijving
label-document-category = Categorie
label-document-last-saved-by = Laatst opgeslagen door
label-document-created = Gemaakt
label-document-modified = Gewijzigd
label-document-printed = Laatst afgedrukt
label-document-pages = Pagina’s
label-document-slides = Dia’s
label-document-hidden-slides = Verborgen dia’s
label-document-notes = Notitiepagina’s
label-document-sheets = Werkbladen
label-document-words = Woorden
label-document-characters = Tekens
label-document-paragraphs = Alinea’s
label-document-lines = Regels
label-document-editing-time = Bewerkingstijd
document-minutes = { $minutes } min
label-document-revision = Revisie
label-document-application = Opgeslagen met
label-document-company = Bedrijf
label-document-template = Sjabloon

## Books (EPUB)

# machine-drafted, needs native review
group-book = Boek
label-book-authors = Auteurs
label-book-publisher = Uitgever
label-book-date = Verschenen
label-book-language = Taal
label-book-pages = Pagina's
# Entries in the book's table of contents.
label-book-contents = Items in inhoudsopgave
label-book-layout = Opmaak
book-layout-reflowable = Herschikbaar
book-layout-fixed = Vaste opmaak
book-right-to-left = van rechts naar links
label-book-version = Formaat
book-version = EPUB { $version }
label-book-fonts = Ingesloten lettertypen
book-fonts-obfuscated = { $count } ({ $obfuscated } versluierd)
label-book-subjects = Onderwerpen
label-book-identifier = Identificatie
label-book-rights = Rechten

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
volume-orient-parrec = Philips-angulatie en off-center
volume-orient-ecat = ECAT-voxelgroottes en verschuivingen (onbetrouwbaar)
label-volume-technique = Techniek
label-volume-version = Formaatversie
label-volume-isotope = Isotoop
label-volume-tracer = Tracer
label-volume-units = Eenheden

# Text, tables and subtitles.
label-header-encoding = Tekenset
label-header-lines = Regels
label-header-line-ends = Regeleinden
label-header-words = Woorden
label-header-pages = Pagina's
label-header-pictures = Afbeeldingen
label-header-pictures-not-shown = Niet getoonde afbeeldingen
label-header-tables = Tabellen
label-header-rows = Rijen
label-header-columns = Kolommen
label-header-delimiter = Scheidingsteken
label-header-column-names = Kolomnamen
label-header-cues = Ondertitels
label-header-cue-span = Ondertitels van – tot
