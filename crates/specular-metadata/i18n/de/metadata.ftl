# Specular Metadaten-Bereich — Deutsch.
# Siehe i18n/en/metadata.ftl für die Konventionen. Zahlen und Datumsangaben
# kommen bereits von ICU4X für das Gebietsschema formatiert an.

## Gruppentitel

group-camera = Kamera
group-exposure = Belichtung
group-lens = Objektiv
group-date = Datum und Uhrzeit
group-location = Ort
# machine-drafted, needs native review
# A DJI drone photo: the gimbal's and the aircraft's attitude, the height above take-off.
group-drone = Drohne
label-gimbal = Gimbal
label-flight = Fluggerät
label-relative-altitude = Über dem Startpunkt
drone-attitude = Gieren { $yaw }, Nicken { $pitch }, Rollen { $roll }
# A DJI thermal camera's radiometric JPEG: its photo opens, its temperatures can't.
label-thermal-data = Temperaturen
drone-thermal-refused = Nicht angezeigt: Die SDK-Lizenz von DJI erlaubt es nicht
drone-thermal-refused-detail = Für DJI-Temperaturen braucht es das DJI Thermal SDK, dessen Lizenz es nur Programmen erlaubt, die es mitliefern. Bild und Metadaten werden angezeigt.
group-image = Bild
group-file = Datei
group-rights = Rechte und Beschreibung
group-credentials = Inhaltsnachweise
# machine-drafted, needs native review
group-medical = Medizinisches Bild
# machine-drafted, needs native review
group-inspection = Bauteil und Prüfung
group-nde-image = Prüfbild (DICONDE)
# machine-drafted, needs native review
# Industrial CT volumes (Zeiss Xradia, Amira, Bruker SkyScan, Nikon X-Tek, VGI, raw volumes).
group-industrial-image = Prüfbild
group-science = Wissenschaftliches Bild
group-em = Elektronenmikroskopie
group-spm = Rastersondenmikroskopie
group-slide = Objektträger
group-microscopy = Mikroskopie
# X-ray diffraction detector frames (CBF, EDF, SMV, Bruker, MarCCD, R-AXIS).
group-diffraction = Beugung
group-surface = Oberflächentopografie
group-ebsd = Elektronenrückstreubeugung
group-seismic = Seismikdaten
# machine-drafted, needs native review
group-asdf = ASDF
# machine-drafted, needs native review
# CERN ROOT files (particle physics histograms and graphs).
group-root = ROOT
# machine-drafted, needs native review
# CASA images (radio interferometry: the .image, .residual, .model directories tclean writes).
group-casa = CASA-Bild
# machine-drafted, needs native review
# GIFTI files: brain surfaces and the values on their vertices (Human Connectome Project
# and FreeSurfer exports).
group-gifti = GIFTI
# CIFTI-2 files: maps over cortical surfaces and subcortical voxels ("grayordinates").
group-cifti = CIFTI
# White-matter fibre tracts (TrackVis, MRtrix).
group-tractography = Traktografie
# machine-drafted, needs native review
group-annotations = Annotationen
# Imaging mass spectrometry (imzML): a mass spectrum per pixel.
group-mass-spec = Massenspektrometrische Bildgebung
# machine-drafted, needs native review
group-thermal = Wärmebild
# machine-drafted, needs native review
# Flow cytometry list-mode data (FCS): events of a cell sorter or analyser.
group-cytometry = Durchflusszytometrie
# machine-drafted, needs native review
group-spectroscopy = Spektroskopie
# machine-drafted, needs native review
# Unstructured meshes of ocean, coastal and climate models (UGRID, ICON, MPAS in NetCDF).
group-ugrid = Unstrukturiertes Gitter
group-drawing = Zeichnung
# machine-drafted, needs native review
group-volume = Volumen

## Feldbezeichnungen

label-camera = Kamera
label-software = Software
label-application = Anwendung
label-shutter = Belichtungszeit
label-aperture = Blende
label-iso = ISO
label-focal-length = Brennweite
label-exposure-bias = Belichtungskorrektur
label-program = Modus
label-metering = Belichtungsmessung
label-white-balance = Weißabgleich
label-flash = Blitz
label-lens = Objektiv
label-lens-make = Objektivhersteller
label-lens-range = Objektivbereich
# machine-drafted, needs native review
label-focus-mode = Fokusmodus
label-stabilisation = Bildstabilisierung
label-taken = Aufgenommen
label-digitised = Digitalisiert
label-edited = Zuletzt bearbeitet
label-place = Ort
label-coordinates = Koordinaten
label-altitude = Höhe
label-direction = Richtung
label-accuracy = Genauigkeit
label-gps-time = GPS-Zeit
label-dimensions = Abmessungen
label-encoding = Kodierung
# machine-drafted, needs native review
label-motion = Bewegung
motion-kind-motion-photo = Bewegtes Foto
motion-kind-micro-video = Bewegtes Foto (MicroVideo)
motion-kind-samsung = Samsung Motion Photo
motion-kind-live-photo = Live Photo
motion-duration = { $seconds } s
label-colour-profile = Farbprofil
label-orientation = Ausrichtung
label-file-name = Name
label-folder = Ordner
label-file-size = Größe
label-format = Format
label-modified = Geändert
label-title = Titel
label-description = Beschreibung
label-creator = Urheber
label-copyright = Copyright
label-keywords = Stichwörter
label-rating = Bewertung
label-colour-label = Markierung
label-credentials = Inhaltsnachweise

## Gruppen unter „Alle Felder“

fields-ifd0 = IFD0 (Hauptbild)
fields-exif = Exif
fields-gps = GPS
fields-interop = Interoperabilität
fields-ifd1 = IFD1 (Vorschaubild)
fields-icc = ICC-Profilkopf
fields-xmp = XMP: { $namespace } ({ $prefix })
xmp-namespace = { $prefix ->
    [dc] Dublin Core
    [xmp] Basis
    [xmpRights] Rechte
    [xmpMM] Medienverwaltung
    [xmpDM] Dynamische Medien
    [tiff] TIFF
    [exif] Exif
    [exifEX] Exif 2.3+
    [aux] Kamerazubehör
    [photoshop] Photoshop
    [crs] Camera-Raw-Einstellungen
    [lr] Lightroom
    [Iptc4xmpCore] IPTC Core
    [Iptc4xmpExt] IPTC Extension
    [plus] PLUS-Lizenzierung
    [GPano] Panorama
    [GCamera] Google-Kamera
    [hdrgm] HDR-Gain-Map
    [mwg-rs] Bereiche
    [pdf] PDF
    [stEvt] Ressourcenereignisse
    [stRef] Ressourcenverweise
   *[other] Sonstige
}
unknown-tag = Unbekanntes Tag { $number } ({ $context })
blob-bytes = { $num ->
    [one] ({ $num } Byte)
   *[other] ({ $num } Bytes)
}
maker-note-bytes = ({ $num ->
        [one] { $num } Byte
       *[other] { $num } Bytes
    }, Herstellerdaten, nicht dekodiert)
more-values = { $shown }, … ({ $total ->
        [one] { $total } Wert
       *[other] { $total } Werte
    })

## Verbindungen

list-separator = {", "}
one-line-separator = {" · "}

## Belichtung

exposure-time-fraction = 1/{ $denominator } s
exposure-time-seconds = { $seconds } s
aperture-value = f/{ $f }
iso-value = ISO { $iso }
focal-length-value = { $mm } mm
focal-length-with-equiv = { $mm } mm (entspricht { $equiv } mm)
focal-length-equiv-only = entspricht { $equiv } mm
exposure-bias-value = { $sign ->
    [plus] +{ $ev } LW
    [minus] −{ $ev } LW
   *[zero] 0 LW
}
exposure-program = { $value ->
    [1] Manuell
    [2] Programmautomatik
    [3] Zeitautomatik (Blendenvorwahl)
    [4] Blendenautomatik (Zeitvorwahl)
    [5] Kreativprogramm (Schärfentiefe)
    [6] Action (kurze Belichtungszeit)
    [7] Porträt
    [8] Landschaft
   *[other] Unbekannt
}
metering-mode = { $value ->
    [1] Integral
    [2] Mittenbetont
    [3] Spot
    [4] Mehrfach-Spot
    [5] Matrix (Mehrfeld)
    [6] Selektiv
   *[other] Sonstige
}
# machine-drafted, needs native review
# Canon maker note CameraSettings FocusMode; 256–258 are the live-view variants of 0–2.
canon-focus-mode = { $value ->
    [0] One-Shot AF
    [1] AI Servo AF
    [2] AI Focus AF
    [3] Manueller Fokus
    [4] Einzel
    [5] Kontinuierlich
    [6] Manueller Fokus
    [16] Pan-Fokus
    [256] One-Shot AF (Livebild)
    [257] AI Servo AF (Livebild)
    [258] AI Focus AF (Livebild)
    [512] Movie-Snap-Fokus
    [519] Movie-Servo-AF
   *[other] Sonstige
}
# Canon maker note CameraSettings ImageStabilization; 256–260 repeat 0–4.
canon-stabilisation = { $value ->
    [0] Aus
    [1] Ein
    [2] Nur bei Aufnahme
    [3] Mitziehen
    [4] Dynamisch
    [256] Aus
    [257] Ein
    [258] Nur bei Aufnahme
    [259] Mitziehen
    [260] Dynamisch
   *[other] Sonstige
}
# machine-drafted, needs native review
panasonic-focus-mode = { $value ->
    [1] Automatisch
    [2] Manueller Fokus
    [4] Automatisch (Fokustaste)
    [5] Automatisch (kontinuierlich)
    [6] AF-S
    [7] AF-C
    [8] AF-F
   *[other] Andere
}
# machine-drafted, needs native review
panasonic-stabilisation = { $value ->
    [2] Ein (optisch)
    [3] Aus
    [4] Ein (Modus 2)
    [5] Ein (optisch, Mitzieher)
    [6] Ein (nur Gehäuse)
    [7] Ein (nur Gehäuse, Mitzieher)
    [9] Dual IS
    [10] Dual IS (Mitzieher)
    [11] Dual IS 2
    [12] Dual IS 2 (Mitzieher)
   *[other] Andere
}
label-comment = Kommentar
fields-tags = { $kind }-Tags
group-picture = Bildeinstellungen
fields-fujifilm = Fujifilm-Herstellerdaten
label-film-simulation = Filmsimulation
label-dynamic-range = Dynamikbereich
label-highlight-tone = Lichter
label-shadow-tone = Schatten
label-colour-setting = Farbe
label-sharpness = Schärfe
label-noise-reduction = Rauschunterdrückung
label-clarity = Klarheit
label-grain-effect = Körnungseffekt
label-colour-chrome = Farbchrom
label-colour-chrome-blue = Farbchrom Blau
label-shutter-type = Verschlussart
label-drive-mode = Aufnahmebetriebsart
fuji-step = { $sign ->
    [plus] +{ $n }
    [minus] −{ $n }
   *[zero] 0
}
fuji-film-mode = { $value ->
    [0] Provia (Standard)
    [256] Studioporträt
    [272] Studioporträt (mehr Sättigung)
    [288] Astia (weich)
    [304] Studioporträt (schärfer)
    [512] Velvia (lebhaft)
    [768] Studioporträt Ex
    [1024] Velvia
    [1280] Pro Neg. Std
    [1281] Pro Neg. Hi
    [1536] Classic Chrome
    [1792] Eterna
    [2048] Classic Neg.
    [2304] Eterna Bleach Bypass
    [2560] Nostalgic Neg.
    [2816] Reala Ace
   *[other] Andere
}
fuji-monochrome = { $value ->
    [768] Monochrom
    [769] Monochrom, Rotfilter
    [770] Monochrom, Gelbfilter
    [771] Monochrom, Grünfilter
    [784] Sepia
    [1280] Acros
    [1281] Acros, Rotfilter
    [1282] Acros, Gelbfilter
    [1283] Acros, Grünfilter
   *[other] Monochrom
}
fuji-white-balance = { $value ->
    [0] Automatisch
    [1] Automatisch (Weiß-Priorität)
    [2] Automatisch (Stimmungspriorität)
    [256] Tageslicht
    [512] Bewölkt
    [768] Tageslicht-Leuchtstofflampe
    [769] Neutralweiß-Leuchtstofflampe
    [770] Weiß-Leuchtstofflampe
    [771] Warmweiß-Leuchtstofflampe
    [772] Wohnraum-Warmweiß-Leuchtstofflampe
    [1024] Glühlampe
    [1280] Blitz
    [1536] Unterwasser
    [3840] Benutzerdefiniert 1
    [3841] Benutzerdefiniert 2
    [3842] Benutzerdefiniert 3
    [3843] Benutzerdefiniert 4
    [3844] Benutzerdefiniert 5
    [4080] Farbtemperatur
   *[other] Andere
}
fuji-white-balance-kelvin = { $kelvin } K
fuji-dynamic-range = DR{ $percent }
fuji-dynamic-range-auto = Automatisch (DR{ $percent })
fuji-dynamic-range-setting = { $value ->
    [0] Automatisch
    [256] Standard (DR100)
    [512] Weit 1 (DR230)
    [513] Weit 2 (DR400)
   *[other] Andere
}
fuji-effect = { $value ->
    [0] Aus
    [32] Schwach
    [64] Stark
   *[other] Andere
}
fuji-grain-size = { $value ->
    [16] klein
    [32] groß
   *[other] andere
}
fuji-grain = { $strength }, { $size }
fuji-shutter-type = { $value ->
    [0] Mechanisch
    [1] Elektronisch
    [2] Elektronisch (Langzeitbelichtung)
    [3] Elektronischer erster Verschlussvorhang
   *[other] Andere
}
fuji-drive-mode = { $value ->
    [0] Einzelbild
    [1] Serie, langsam
    [2] Serie, schnell
   *[other] Andere
}
fuji-focus-mode = { $value ->
    [0] Manuell
    [1] AF-S
    [2] AF-C
    [100] Automatisch
    [101] Manuell
   *[other] Andere
}
fuji-stabilisation = { $kind } ({ $mode })
fuji-stabilisation-kind = { $value ->
    [0] Keine
    [1] Optisch
    [2] Sensor-Shift
    [3] Objektiv
    [258] Sensor-Shift und digital
    [512] Digital
   *[other] Andere
}
fuji-stabilisation-mode = { $value ->
    [0] Aus
    [1] ein, durchgehend
    [2] ein, nur bei Aufnahme
   *[other] Andere
}
light-source = { $value ->
    [1] Tageslicht
    [2] Leuchtstofflampe
    [3] Glühlampe
    [4] Blitz
    [9] Schönes Wetter
    [10] Bewölkt
    [11] Schatten
    [12] Tageslicht-Leuchtstofflampe
    [13] Tagweiße Leuchtstofflampe
    [14] Kaltweiße Leuchtstofflampe
    [15] Weiße Leuchtstofflampe
    [16] Warmweiße Leuchtstofflampe
    [17] Normlicht A
    [18] Normlicht B
    [19] Normlicht C
    [20] D55
    [21] D65
    [22] D75
    [23] D50
    [24] Studio-Glühlampe
   *[other] Sonstige
}
white-balance-auto = Automatisch
white-balance-manual = Manuell
white-balance-manual-with-light = Manuell ({ $light })
flash-none = Keine Blitzfunktion
flash-fired = Blitz ausgelöst
flash-not-fired = Blitz nicht ausgelöst
flash-forced-on = erzwungen ein
flash-forced-off = erzwungen aus
flash-auto = Automatik
flash-return-not-detected = Reflexion nicht erkannt
flash-return-detected = Reflexion erkannt
flash-red-eye = Rote-Augen-Reduktion

## Objektiv

lens-focal-single = { $mm } mm
lens-focal-range = { $min }–{ $max } mm
lens-aperture-single = f/{ $f }
lens-aperture-range = f/{ $min }–{ $max }
lens-range = { $focal } { $aperture }

## Datum und Uhrzeit

date-with-weekday = { $weekday }, { $date }
date-time = { $date }, { $time }
date-time-zone = { $datetime } ({ $zone })
zone-utc = UTC
zone-offset = UTC{ $offset }
gps-date-time = { $date }, { $time } UTC
detail-zone-from-gps = Zeitzone aus der GPS-Uhr ermittelt

## Ort

coordinates = { $lat }° { $latRef ->
    [south] S
   *[north] N
}, { $lon }° { $lonRef ->
    [west] W
   *[east] O
}
altitude = { $ref ->
    [above] { $m } m über dem Meeresspiegel
    [below] { $m } m unter dem Meeresspiegel
   *[sea] Auf Meereshöhe
}
compass-point = { $point ->
    [n] Nord
    [nne] Nordnordost
    [ne] Nordost
    [ene] Ostnordost
    [e] Ost
    [ese] Ostsüdost
    [se] Südost
    [sse] Südsüdost
    [s] Süd
    [ssw] Südsüdwest
    [sw] Südwest
    [wsw] Westsüdwest
    [w] West
    [wnw] Westnordwest
    [nw] Nordwest
   *[nnw] Nordnordwest
}
facing = Blickrichtung { $point } ({ $degrees }°)
detail-true-north = Bezogen auf geografisch Nord
detail-magnetic-north = Bezogen auf magnetisch Nord
accuracy = ±{ $m } m
distance-km = { $km } km
place = { $city }, { $country }
place-city-only = { $city }
place-near = in der Nähe von { $place }
place-detail = { $areas } · { $distance } von { $city }
place-detail-distance = { $distance } von { $city }

## Bild

dimensions = { $width } × { $height }
dimensions-megapixels = { $width } × { $height } ({ $mp } MP)
encoding-bit-depth = { $bits } Bit
encoding-progressive = { $format }, progressiv
encoding-baseline = { $format }, Baseline
encoding-hierarchical = { $format }, hierarchisch
# machine-drafted, needs native review
encoding-alpha = { $format } { $alpha ->
    [premultiplied] mit vormultipliziertem Alpha
    [transparency] mit Transparenz
   *[alpha] mit Alpha
}
# [Lab], [Palette] and the variants after them: machine-drafted, needs native review
colour-model = { $model ->
    [YCbCr] YCbCr
    [RGB] RGB
    [Greyscale] Graustufen
    [CMYK] CMYK
    [YCCK] YCCK
    [Lab] L*a*b*
    [Palette] Indizierte Farben
    [Multichannel] Mehrkanal
    [Duotone] Duplex
    [YCgCo] YCgCo
    [XYB] XYB
    [YC] Luminanz/Chrominanz (YC)
    [Bayer] Bayer-RAW
    [X-Trans] X-Trans-RAW
    [CFA] RAW mit Farbfiltermatrix
    [LinearRaw] Lineares RAW
    [RGBE] RGBE (gemeinsamer Exponent)
    [XYZE] XYZE (gemeinsamer Exponent)
    [R] Ein Kanal (R)
    [RG] Zwei Kanäle (RG)
   *[other] { $model }
}
adobe-rgb-from-exif = Adobe RGB (laut EXIF-Tag; kein eingebettetes Profil)
orientation = { $value ->
    [1] Aufrecht (keine Drehung nötig)
    [2] Horizontal gespiegelt (angewendet)
    [3] Um 180° gedreht (angewendet)
    [4] Vertikal gespiegelt (angewendet)
    [5] Horizontal gespiegelt und um 90° gegen den Uhrzeigersinn gedreht (angewendet)
    [6] Um 90° im Uhrzeigersinn gedreht (angewendet)
    [7] Horizontal gespiegelt und um 90° im Uhrzeigersinn gedreht (angewendet)
    [8] Um 90° gegen den Uhrzeigersinn gedreht (angewendet)
   *[other] Unbekannt ({ $value })
}
orientation-short = { $value ->
    [1] Aufrecht
    [2] Horizontal gespiegelt
    [3] Um 180° gedreht
    [4] Vertikal gespiegelt
    [5] Horizontal gespiegelt und um 90° gegen den Uhrzeigersinn gedreht
    [6] Um 90° im Uhrzeigersinn gedreht
    [7] Horizontal gespiegelt und um 90° im Uhrzeigersinn gedreht
    [8] Um 90° gegen den Uhrzeigersinn gedreht
   *[other] Wert { $value }
}

## Datei

file-size-bytes = { $num ->
    [one] { $num } Byte
   *[other] { $num } Bytes
}
file-size-unit = { $num } { $unit ->
    [kb] KB
    [mb] MB
    [gb] GB
    [tb] TB
   *[pb] PB
}

## Rechte

rating-rejected = Abgelehnt
rating-stars = { $stars } ({ $n } von 5)

## Inhaltsnachweise

c2pa-present = Vorhanden (nicht geprüft)
c2pa-detail = Diese Datei enthält ein C2PA-Manifest; seine Signatur wurde nicht geprüft.

## ICC-Kopfdaten

icc-class = { $class ->
    [scnr] Eingabegerät
    [mntr] Anzeigegerät
    [prtr] Ausgabegerät
    [link] Geräteverknüpfung
    [spac] Farbraum
    [abst] Abstrakt
    [nmcl] Benannte Farbe
   *[other] Unbekannt
}
icc-intent = { $intent ->
    [0] Wahrnehmungsorientiert
    [1] Relativ farbmetrisch
    [2] Sättigung
    [3] Absolut farbmetrisch
   *[other] Unbekannt
}
icc-platform = { $platform ->
    [APPL] Apple
    [MSFT] Microsoft
    [SGI] Silicon Graphics
    [SUNW] Sun Microsystems
    [none] Nicht angegeben
   *[other] Unbekannt
}
icc-coded = { $name } ({ $code })

# machine-drafted, needs native review
label-modality = Modalität
label-body-part = Körperregion
label-study = Studie
label-series = Serie
label-slice-thickness = Schichtdicke
label-pixel-spacing = Pixelabstand
label-imager-pixel-spacing = Pixelabstand am Detektor
label-frames = Einzelbilder
label-transfer-syntax = Transfersyntax
label-document = Dokument
label-document-type = Dokumenttyp
medical-spacing = { $row } × { $column } mm
medical-mm = { $mm } mm
medical-lossy = { $name } (verlustbehaftete Kompression)
detail-pixel-spacing = Abstand der Pixelmitten im Patienten (Zeilen × Spalten).
# machine-drafted, needs native review
# Pixel spacing of an industrial inspection image (DICONDE, CT of an object).
detail-pixel-spacing-object = Abstand der Pixelmitten im Objekt (Zeilen × Spalten).
detail-imager-pixel-spacing = Am Detektor gemessen, nicht um die Vergrößerung korrigiert: Abstände im Patienten sind meist kleiner.

## Industrielle Prüfung (DICONDE)

# machine-drafted, needs native review
label-nde-component-shape = Bauteilform
label-nde-component-manufacturer = Bauteilhersteller
label-nde-material = Werkstoff
label-nde-thickness = Wanddicke
label-nde-pipe-diameter = Rohrdurchmesser
label-nde-source = Strahlenquelle
label-nde-environment = Umgebung
label-nde-iqi = Bildgüteprüfkörper
label-nde-indications = Anzeigen
label-nde-standard = Norm

## Forschungsvolumen (NIfTI, Analyze, NRRD, MetaImage)

# machine-drafted, needs native review
label-voxel-size = Voxelgröße
label-data-type = Datentyp
label-time-points = Zeitpunkte
label-orientation-source = Orientierung aus
label-intent = Werte
label-compression = Kompression
volume-voxel-size = { $x } × { $y } × { $z } mm
volume-orient-sform = NIfTI sform
volume-orient-qform = NIfTI qform
volume-orient-analyze = Analyze-Header (unzuverlässig)
volume-orient-space = NRRD-Raumrichtungen
volume-orient-transform = MetaImage-Transformation
volume-orient-spacings = Nur Voxelabstand
volume-orient-none = Nicht angegeben
volume-orient-vox2ras = FreeSurfer vox2ras
volume-orient-mgh-default = FreeSurfer-Standard (nicht verlässlich)
volume-orient-minc = MINC-Richtungskosinusse
volume-orient-object = Objektkoordinaten (keine Patientenorientierung)
volume-content-projections = Projektionen (Rohscan)
volume-content-reconstruction = Rekonstruiertes Volumen
volume-coords-rectilinear = Rechtwinklig (ungleiche Abstände, gleichmäßig angezeigt)
# Scan parameters and tags a FreeSurfer MGH file keeps after its voxels.
label-volume-tr = Repetitionszeit
label-volume-te = Echozeit
label-volume-ti = Inversionszeit
label-volume-flip = Flipwinkel
label-volume-fov = Sichtfeld
label-volume-field = Feldstärke
label-volume-pedir = Phasenkodierung
label-volume-cmdline = Befehlszeile
label-volume-xform = Talairach-Transformation
label-volume-content = Inhalt
label-volume-projections = Projektionen
label-volume-angles = Drehbereich
label-volume-angle-step = Drehschritt
label-volume-kv = Röhrenspannung
label-volume-current = Röhrenstrom
label-volume-power = Quellenleistung
label-volume-exposure = Belichtung
label-volume-sod = Quelle–Objekt
label-volume-odd = Objekt–Detektor
label-volume-sdd = Quelle–Detektor
label-volume-magnification = Optische Vergrößerung
label-volume-binning = Binning
label-volume-objective = Objektiv
label-volume-filter = Quellenfilter
label-volume-camera = Detektor
label-volume-instrument = Gerät
label-volume-software = Software
label-volume-date = Aufgenommen
label-volume-ring = Ringartefaktkorrektur
label-volume-smoothing = Glättung
label-volume-coords = Gitter
# machine-drafted, needs native review
# An eye scan (OCT): which eye.
label-volume-eye = Auge
volume-eye-right = Rechts (OD)
volume-eye-left = Links (OS)
detail-volume-unreliable = Die Datei gibt nicht an, wohin ihre Achsen im Patienten zeigen: Links und rechts können vertauscht sein, daher werden keine Orientierungsbeschriftungen angezeigt.
# machine-drafted, needs native review
# An industrial CT volume: its axes are the scanner's, there is no patient.
detail-volume-object = Die Achsen sind die des Scanners, nicht die eines Patienten, daher werden keine Orientierungsbeschriftungen angezeigt.

## Wissenschaftliche Bilder (FITS, XISF)

label-object = Objekt
label-telescope = Teleskop
label-instrument = Instrument
label-filter = Filter
label-observed = Beobachtet
label-exposure-time = Belichtung
label-sample = Abtastwerte
label-unit = Einheit
label-hdu = Erweiterung
label-centre = Mitte
label-centre-deg = Mitte (Grad)
label-frame = Koordinatensystem
# machine-drafted, needs native review
science-frame-galactic = Galaktisch
science-frame-ecliptic = Ekliptikal
label-projection = Projektion
label-pixel-scale = Pixelmaßstab
science-seconds = { $seconds } s
science-arcsec-px = { $arcsec }″ pro Pixel
label-distortion = Verzeichnung
label-wcs = Weltkoordinaten
science-wcs-unsupported = Nicht unterstützt: { $reason }
# machine-drafted, needs native review
science-wcs-approximate = Näherungsweise (nur Pointing, ohne Verzeichnung)
label-observer = Beobachter
label-bayer = Bayer-Muster
label-capture-start = Aufnahmebeginn (UTC)
label-capture-end = Aufnahmeende (UTC)
label-frame-rate = Bildrate
science-fps = { $fps } Bilder/s
science-frames-cut = { $frames } von { $declared }
# The HDF5 dataset or NetCDF variable shown.
label-dataset = Datensatz
label-pixel-size = Pixelgröße
# "0.25 nm per pixel"; $size may be "0.25 × 0.5".
science-pixel-size = { $size } { $unit } pro Pixel
# Microscopy (DigitalMicrograph, MRC, Gwyddion, Igor).
label-microscope = Mikroskop
label-voltage = Beschleunigungsspannung
label-magnification = Vergrößerung
label-imaging-mode = Modus
label-detector = Detektor
label-acquired = Aufgenommen
label-field-of-view = Bildfeld
science-kilovolts = { $kv } kV
science-magnification = { $value }×
label-vendor = Hersteller
label-scanner = Scanner
label-objective = Objektiv
label-pyramid-levels = Stufen
# machine-drafted, needs native review
label-emissivity = Emissionsgrad
label-object-distance = Objektabstand
label-reflected-temperature = Reflektierte Temperatur
label-atmospheric-temperature = Atmosphärentemperatur
label-humidity = Relative Luftfeuchte
label-raw-range = Rohwertbereich
label-temperature-range = Temperaturbereich
thermal-temperature = { $value } { $unit }
thermal-metres = { $value } m
thermal-percent = { PERCENT($value) }
thermal-range = { $low } – { $high }
slide-levels = { $count }: { $sizes }
# X-ray diffraction: the experiment geometry from a detector frame's header.
label-wavelength = Wellenlänge
# Ångström, e.g. "0.97625 Å".
xtal-angstrom = { $value } Å
label-photon-energy = Photonenenergie
xtal-kev = { $value } keV
# Sample to detector distance.
label-detector-distance = Detektorabstand
xtal-mm = { $value } mm
# Where the direct beam meets the detector, in pixels from the image's top-left corner.
label-beam-centre = Strahlzentrum
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
label-oscillation = Oszillation
xtal-oscillation = { $start }° + { $width }°
# $axis is the axis name from the file, e.g. "phi" or "omega".
xtal-oscillation-axis = { $start }° + { $width }° um { $axis }
# The count at which a pixel saturates.
label-overload = Überlaufwert
# The finest lattice spacing (d) recorded at the detector's edge and in its corners.
label-resolution = Auflösung
xtal-resolution = { $edge } Å am Rand, { $corner } Å in den Ecken
label-geometry = Geometrie
xtal-curved = Gekrümmte Bildplatte: keine q-, d- oder Auflösungsringe
xtal-incomplete = Unvollständiger Header: keine q-, d- oder Auflösungsringe
# All-fields group title: "FITS header".
fields-science-header = { $format }-Header
# machine-drafted, needs native review
fields-asdf-tree = ASDF-Baum
label-asdf-model = Datenmodell
label-asdf-version = Dateiversion
label-asdf-standard = ASDF-Standard
label-asdf-library = Geschrieben von
# machine-drafted, needs native review
# All-fields group title for a CERN ROOT file's list of keys (objects).
fields-root-keys = ROOT-Schlüssel
# CERN ROOT files: the ROOT version that wrote the file (e.g. "6.28/04"), its compression,
# the number of keys (stored objects), and the object shown: class (e.g. "TH2F"), its path in
# the file, title, number of entries and sum of weights.
label-root-version = ROOT-Version
label-root-compression = Kompression
label-root-keys = Schlüssel
label-root-class = Klasse
label-root-name = Name
label-root-title = Titel
label-root-entries = Einträge
label-root-sum-weights = Summe der Gewichte
# A graph's number of points.
label-root-points = Punkte
# $axis is "x", "y" or "z".
label-root-axis = Achse { $axis }
# $quantity is the axis title with its unit, e.g. "Energy [GeV]"; $min and $max the axis
# limits.
root-axis-fixed = { $quantity }: { $bins ->
        [one] { $bins } Bin
       *[other] { $bins } Bins
    }, { $min } bis { $max }
root-axis-variable = { $quantity }: { $bins ->
        [one] { $bins } variables Bin
       *[other] { $bins } variable Bins
    }, { $min } bis { $max }
label-root-mean-rms = Mittelwert und RMS { $axis }
root-mean-rms = { $mean } ± { $rms }
# Trees (event data) are listed only, with their entry counts.
label-root-trees = Bäume
root-tree = { $name } ({ $entries ->
        [one] { $entries } Eintrag
       *[other] { $entries } Einträge
    })
# machine-drafted, needs native review
# CASA images: what the image is (from its directory name), its restoring beam (major and
# minor axis, position angle), the beams of its planes when they differ, the rest frequency,
# spectral reference frame (e.g. "LSRK") and velocity definition of the frequency labels,
# the default mask (e.g. "mask0") and the casacore storage manager with its byte order.
label-casa-type = Bildtyp
casa-type-image = Restauriertes Bild
casa-type-residual = Residuum
casa-type-model = Modell
casa-type-pb = Primärkeule
casa-type-mask = Clean-Maske
casa-type-psf = Punktspreizfunktion
casa-type-sumwt = Summe der Gewichte
casa-type-weight = Gewichte
label-casa-beam = Restaurierungskeule
casa-beam = { $major }″ × { $minor }″, PW { $pa }°
label-casa-plane-beams = Keulen je Ebene
casa-plane-beams = { $count ->
        [one] { $count } Keule
       *[other] { $count } Keulen
    }, Hauptachse { $min }″ bis { $max }″
label-casa-rest-frequency = Ruhefrequenz
casa-ghz = { $ghz } GHz
label-casa-spectral-frame = Spektrales Bezugssystem
label-casa-velocity = Geschwindigkeitsdefinition
casa-velocity-radio = Radio
casa-velocity-optical = Optisch
casa-velocity-relativistic = Relativistisch
label-casa-mask = Standardmaske
label-casa-storage = Speicherung
casa-storage-big = { $manager }, Big-Endian
casa-storage-little = { $manager }, Little-Endian
# machine-drafted, needs native review
# Unstructured meshes: the mesh's name in the file, the convention describing it (UGRID,
# ICON, MPAS), its counts, face shapes, coordinates and where the values sit.
label-ugrid-name = Gitter
label-ugrid-convention = Konvention
label-ugrid-nodes = Knoten
label-ugrid-faces = Flächen
label-ugrid-edges = Kanten
label-ugrid-shapes = Flächenformen
label-ugrid-coordinates = Koordinaten
label-ugrid-values = Werte auf
label-ugrid-split = Für die Darstellung geteilt
# $n is the formatted count, $count the number (for the plural).
ugrid-triangles = { $count ->
    [one] { $count } Dreieck
   *[other] { $count } Dreiecke
}
ugrid-quads = { $count ->
    [one] { $count } Viereck
   *[other] { $count } Vierecke
}
# Faces of five or more nodes; $max is the most nodes of one face.
ugrid-polygons = { $count ->
    [one] { $count } Polygon mit { $max } Knoten
   *[other] { $count } Polygone mit bis zu { $max } Knoten
}
ugrid-lonlat = Längen- und Breitengrad
ugrid-projected = Projiziertes x und y
ugrid-projected-unit = Projiziertes x und y ({ $unit })
ugrid-on-faces = Flächen (flach gezeichnet)
ugrid-on-nodes = Knoten (über die Flächen interpoliert)
# A mesh file without data variables: the image shows how many nodes each face has.
ugrid-grid-only = Keine: das Bild zeigt die Knoten pro Fläche
# Faces cut at the antimeridian or closed to a pole so they draw on a flat map.
ugrid-split = { $count ->
    [one] { $count } Fläche am Antimeridian oder an einem Pol
   *[other] { $count } Flächen am Antimeridian oder an einem Pol
}
# machine-drafted, needs native review
# Slide annotations (NDP.view, Aperio ImageScope, ASAP sidecar files).
label-annotation-file = Datei
# "slide.xml (Aperio ImageScope)"
annotation-source = { $file } ({ $format })
label-annotation-shapes = Formen
label-annotation-groups = Gruppen
# Shapes or points over the reader's limits, left out.
label-annotation-skipped = Ausgelassen
annotation-polygon = Polygon
annotation-polyline = Linie
annotation-rectangle = Rechteck
annotation-ellipse = Ellipse
annotation-point = Punkt
annotation-points = Punkte
annotation-ruler = Lineal
annotation-arrow = Pfeil
# The list of annotations is cut short: "and 1,200 more".
annotation-more = und { $count } weitere
# All-fields group title: "Annotations in slide.xml".
fields-annotations-header = Annotationen in { $file }
label-drawing-version = Version
label-drawing-units = Einheiten
label-drawing-extents = Ausdehnung
label-drawing-sheet = Blatt
label-drawing-layouts = Layouts
label-drawing-layers = Layer
label-drawing-entities = Objekte
label-drawing-codepage = Codepage
label-drawing-missing-fonts = Ersetzte Schriften
# machine-drafted, needs native review
# A font file the drawing names that was not found: the styles naming it, and the font drawing its text instead.
drawing-missing-font = { $file } (Stile { $styles }), dargestellt mit { $substitute }
# A Big Font (double-byte text) that was not found; its characters come from other fonts.
drawing-missing-bigfont = { $file } (Stile { $styles }), dargestellt mit Ersatzschriften
# Survey data (LandXML).
label-survey-crs = Koordinatenbezugssystem
survey-crs-none = Nicht angegeben
label-survey-project = Projektname
label-survey-application = Geschrieben von
label-survey-surface = Geländemodell
# A TIN surface: its name, counts, plan and sloped areas, the volume above its lowest point, its elevations and the contour interval drawn; unit is "m" or "ft" (areas in square units, volume in cubic units).
survey-surface = { $name }: { $points ->
        [one] { $points } Punkt
       *[other] { $points } Punkte
    }, { $triangles ->
        [one] { $triangles } Dreieck
       *[other] { $triangles } Dreiecke
    }; Grundfläche { $area } { $unit }², Oberfläche { $area3d } { $unit }²; { $volume } { $unit }³ über dem tiefsten Punkt; Höhen { $zmin }–{ $zmax } { $unit }; Höhenlinien alle { $interval } { $unit }
label-survey-alignment = Achse
# An alignment: its name, length and first and last stations ("1+234.50").
survey-alignment = { $name }: { $length } { $unit }, Stationen { $start } bis { $end }
# SHX shape fonts found and used for the drawing's text.
label-drawing-shx-fonts = SHX-Schriften
label-drawing-warnings = Probleme
label-plot-paper = Papier
# "2 of 5": the page shown and how many the plot has.
plot-page-of = { $page } von { $pages }
label-plot-page = Seite
label-plot-pens = Verwendete Stifte
# The printer language a plot came wrapped in, e.g. "PCL 5".
label-plot-job = Druckauftrag
# What a plot draws: strokes, fills and labels.
label-plot-items = Gezeichnete Elemente
# Width × height of what is drawn, in drawing units: "1,250 × 800 mm".
drawing-extents = { $width } × { $height } { $unit }
drawing-extents-unitless = { $width } × { $height }
drawing-model-space = Modell
# "24, 3 off or frozen"
drawing-layers-off = { $num }, { $off } aus oder gefroren
# DXF $INSUNITS.
drawing-units = { $code ->
    [1] Zoll
    [2] Fuß
    [3] Meilen
    [4] Millimeter
    [5] Zentimeter
    [6] Meter
    [7] Kilometer
    [8] Mikrozoll
    [9] Mil
    [10] Yards
    [11] Ångström
    [12] Nanometer
    [13] Mikrometer
    [14] Dezimeter
    [15] Dekameter
    [16] Hektometer
    [17] Gigameter
    [18] Astronomische Einheiten
    [19] Lichtjahre
    [20] Parsec
    [21] US-Vermessungsfuß
   *[other] Ohne Einheit
}
# All-fields group title: "DXF entities".
fields-drawing-entities = { $format }-Objekte

## IC-Layouts (GDSII)

# machine-drafted, needs native review
label-layout-library = Bibliothek
label-layout-units = Einheiten
# The database unit (the grid coordinates are stored in) and the user unit, with SI symbols:
# "1 nm database unit, 1 µm user unit".
layout-units = { $db } Datenbankeinheit, { $user } Benutzereinheit
# The top cell shown (each top cell is a page).
label-layout-cell = Zelle
label-layout-cells = Zellen
# Cells no other cell places.
label-layout-top-cells = Top-Zellen
# "TOP, TEST, PAD and 12 more"
layout-top-cells-more = { $names } und { $more } weitere
# Layer/datatype pairs.
label-layout-layers = Layer
# Polygons, paths and boxes, as stored (each cell once).
label-layout-shapes = Formen
label-layout-texts = Beschriftungen
# Cell references and the placements they make: "1,204 (88,320 placements)".
label-layout-references = Referenzen
layout-references = { $refs } ({ $placements } { $placements ->
    [one] Platzierung
   *[other] Platzierungen
})
# Shapes with every cell placement expanded.
label-layout-flat-shapes = Formen bei aufgelöster Hierarchie
label-layout-bounds = Begrenzung
# Corners of what the cell draws, micrometres: "-12.5, 0 to 2,900, 3,500 µm".
layout-bounds = { $x0 }, { $y0 } bis { $x1 }, { $y1 } µm

## Leiterplatten und Schaltpläne (Gerber, Excellon, KiCad)

# machine-drafted, needs native review
group-board = Leiterplatte
group-schematic = Schaltplan
# The board view shown (its layers, its top or bottom), or the schematic sheet.
label-pcb-view = Ansicht
label-pcb-project = Projekt
label-pcb-revision = Revision
label-pcb-date = Datum
# The CAD program that wrote the file.
label-pcb-generator = Erstellt mit
# Width × height of the board outline: "100 × 80 mm".
label-pcb-size = Platinengröße
label-pcb-thickness = Dicke
label-pcb-copper-layers = Kupferlagen
label-pcb-sheets = Blätter
# The board's layers or files, each with its role: "board-F_Cu.gbr (Top copper)".
label-pcb-layers = Lagen
pcb-layer-role = { $name } ({ $role })
label-pcb-components = Footprints
label-pcb-symbols = Symbole
label-pcb-nets = Netze
label-pcb-tracks = Leiterbahnen
label-pcb-wires = Verbindungen
label-pcb-vias = Vias
# Drilled holes and how many drill sizes: "1,204 in 9 sizes".
label-pcb-holes = Bohrungen
pcb-holes = { $holes } in { $sizes } { $sizes ->
    [one] Größe
   *[other] Größen
}
label-pcb-apertures = Blenden
# What a board layer or file is, shown after its name.
pcb-role-inner-copper = Innenlage { $n }
pcb-role-top-copper = Kupfer oben
pcb-role-bottom-copper = Kupfer unten
pcb-role-top-solder-mask = Lötstopplack oben
pcb-role-bottom-solder-mask = Lötstopplack unten
pcb-role-top-silkscreen = Bestückungsdruck oben
pcb-role-bottom-silkscreen = Bestückungsdruck unten
pcb-role-top-paste = Paste oben
pcb-role-bottom-paste = Paste unten
pcb-role-top-solder-paste = Lotpaste oben
pcb-role-bottom-solder-paste = Lotpaste unten
pcb-role-top-adhesive = Kleber oben
pcb-role-bottom-adhesive = Kleber unten
pcb-role-top-courtyard = Sperrfläche oben
pcb-role-bottom-courtyard = Sperrfläche unten
pcb-role-top-fabrication = Fertigung oben
pcb-role-bottom-fabrication = Fertigung unten
pcb-role-board-outline = Platinenumriss
pcb-role-plated-drill = Bohrungen durchkontaktiert
pcb-role-non-plated-drill = Bohrungen nicht durchkontaktiert
pcb-role-drill = Bohrdaten
pcb-role-drill-holes = Bohrlöcher
pcb-role-mechanical = Mechanik
pcb-role-margin = Rand
pcb-role-drawings = Zeichnungen
pcb-role-comments = Kommentare
pcb-role-engineering-change-1 = Änderung 1
pcb-role-engineering-change-2 = Änderung 2
pcb-role-vias = Vias
pcb-role-other = Sonstige
pcb-role-unknown = Unbekannt
label-pcb-macros = Blendenmakros

## Spektren (JCAMP-DX, SPC, mzML, mzXML, Bruker NMR)

# machine-drafted, needs native review
group-spectrum = Spektrum
label-spectrum-format = Format
label-spectrum-technique = Messverfahren
label-spectrum-title = Titel
# The page shown: "3 of 1,204: Scan 1532".
label-spectrum-page = Seite
spectrum-page-of = { $page } von { $pages }: { $name }
# An axis with its unit: "Wavenumber (cm⁻¹)".
spectrum-axis = { $title } ({ $unit })
label-spectrum-x = X-Achse
label-spectrum-y = Y-Achse
label-spectrum-nucleus = Kern
label-spectrum-frequency = Spektrometerfrequenz
label-spectrum-solvent = Lösungsmittel
label-spectrum-pulse-program = Pulsprogramm
label-spectrum-spectra = Spektren
label-spectrum-points = Punkte
label-spectrum-instrument = Instrument
# Who or what lab measured it.
label-spectrum-origin = Herkunft
label-spectrum-date = Gemessen
# How Specular made the page from the raw data (an FID transformed, phased automatically).
label-spectrum-processing = Verarbeitung
spectrum-technique-ir = Infrarot
spectrum-technique-raman = Raman
spectrum-technique-uv-vis = UV-Vis
spectrum-technique-fluorescence = Fluoreszenz
spectrum-technique-nmr = NMR
spectrum-technique-ms = Massenspektrum
spectrum-technique-chromatogram = Chromatogramm
spectrum-technique-other = Spektrum

## 3D-Modelle (STL, OBJ, PLY, glTF, 3MF)

# machine-drafted, needs native review
group-mesh = 3D-Modell
label-triangles = Dreiecke
label-vertices = Eckpunkte
label-materials = Materialien
label-textures = Texturen
label-up-axis = Hochachse
# Textures the model uses, some of which could not be read: "3 (1 missing)".
mesh-textures-missing = { $count } ({ $missing } fehlen)

## Molekülstrukturen (PDB, mmCIF, CIF, SDF, XYZ)

# machine-drafted, needs native review
group-structure = Struktur
# The entry's identifier: a PDB ID such as 1CRN, or the CIF data block's name.
label-structure-id = Eintrag
label-structure-classification = Klassifikation
# How the structure was determined: X-ray diffraction, electron microscopy, solution NMR…
label-structure-method = Methode
# The crystallographic or cryo-EM resolution, in ångströms.
label-structure-resolution = Auflösung
structure-resolution = { $value } Å
label-structure-deposited = Hinterlegt
label-structure-formula = Formel
label-structure-chains = Ketten
# Residues (amino acids, nucleotides) in the polymer chains.
label-structure-residues = Reste
label-structure-atoms = Atome
# Atoms after the unit cell was filled by symmetry, and those the file lists: "96 (12 in the asymmetric unit)".
structure-atoms-cell = { $count } ({ $asymmetric } in der asymmetrischen Einheit)
label-structure-bonds = Bindungen
# Models in the file (an NMR ensemble), of which the first is shown.
label-structure-models = Modelle
# Molecules (SDF records) or data blocks in the file, of which the first is shown.
label-structure-records = Datensätze
structure-first-of = { $count } (der erste wird angezeigt)
# Small molecules bound to the polymer, by their three-letter codes.
label-structure-ligands = Liganden
# A list cut short: "HEM, NAG, SO4 and 9 more".
structure-ligands-more = { $list } und { $more } weitere
label-structure-cell = Elementarzelle
# Unit cell edge lengths (Å) and angles (degrees).
structure-cell = a = { $a } Å, b = { $b } Å, c = { $c } Å, α = { $alpha }°, β = { $beta }°, γ = { $gamma }°
label-structure-space-group = Raumgruppe
label-structure-secondary = Sekundärstruktur
structure-ss-file = Aus der Datei
structure-ss-computed = Aus dem Rückgrat berechnet

## HDF4 und HDF-EOS2

# machine-drafted, needs native review
group-hdf4 = HDF4
# The HDF library version that wrote the file, e.g. "NCSA HDF Version 4.2 Release 15".
label-hdf4-library = Bibliothek
# Counts of the file's scientific datasets, raster images, groups (Vgroups) and tables (Vdatas).
label-hdf4-datasets = Datensätze
label-hdf4-images = Rasterbilder
label-hdf4-vgroups = Gruppen
label-hdf4-vdatas = Tabellen
# HDF-EOS: NASA's Earth Observing System conventions on top of HDF4 (MODIS, ASTER, AIRS…).
group-hdf-eos = HDF-EOS
label-eos-version = Version
# The grids, swaths and point structures the file defines.
label-eos-structures = Strukturen
eos-grids = { $count ->
    [one] { $count } Gitter ({ $names })
   *[other] { $count } Gitter ({ $names })
}
eos-swaths = { $count ->
    [one] { $count } Aufnahmestreifen ({ $names })
   *[other] { $count } Aufnahmestreifen ({ $names })
}
eos-points = { $count ->
    [one] { $count } Punktstruktur
   *[other] { $count } Punktstrukturen
}
# The grid or swath the dataset shown belongs to.
label-eos-structure = Struktur
# The grid's map projection as its GCTP name, e.g. GCTP_SNSOID (sinusoidal).
label-eos-projection = Projektion
label-eos-grid-size = Gittergröße
# Columns × rows.
eos-grid-size = { $width } × { $height }
# The grid's upper-left and lower-right corners.
label-eos-corners = Ecken
# The product's short name from the inventory metadata, e.g. MOD09A1.
label-eos-product = Produkt
# When the data was acquired.
label-eos-time = Zeit
eos-time = { $begin } bis { $end }

## Planetenbilder (VICAR, PDS3, PDS4: Missionsdaten von NASA und ESA)

# machine-drafted, needs native review
group-planetary = Planetenbild
# The body observed, e.g. MARS, Ganymede.
label-planetary-target = Ziel
label-planetary-mission = Mission
# The spacecraft, rover or lander carrying the instrument.
label-planetary-spacecraft = Raumsonde
label-planetary-start = Startzeit
label-planetary-stop = Endzeit
# The product's identifier in the archive.
label-planetary-product = Produkt-ID
# The map projection of a map-projected image, as the label names it.
label-planetary-projection = Kartenprojektion
label-planetary-resolution = Auflösung
planetary-metres-per-pixel = { $metres } m pro Pixel
planetary-pixels-per-degree = { $pixels } Pixel pro Grad
# Latitude and longitude of the map projection's centre.
label-planetary-centre = Projektionsmitte
planetary-lat-lon = { $lat }°, { $lon }°
# The latitudes and longitudes the map covers.
label-planetary-coverage = Abdeckung
planetary-coverage = Breite { $south }° bis { $north }°, Länge { $west }° bis { $east }°
# The body's equatorial radius the map uses.
label-planetary-radius = Körperradius
planetary-km = { $km } km

## OMERO (Bilder von einem OMERO-Server oder der Image Data Resource)

# machine-drafted, needs native review
group-omero = OMERO
label-omero-server = Server
# $server the server's address; $user the user name signed in with.
omero-server-user = { $server } (als { $user })
omero-server-public = { $server } (öffentlich)
label-omero-image = Bild
# $name the image's name; $id its number on the server.
omero-image = { $name } (ID { $id })
# The project and dataset the image is in, as "Project › Dataset".
label-omero-project = Projekt
# The screen, plate and well, as "Screen › Plate › B4".
label-omero-screen = Screen
label-omero-owner = Eigentümer
# $owner the image's owner; $group the OMERO group it belongs to.
omero-owner-group = { $owner } ({ $group })
label-omero-acquired = Aufgenommen
label-omero-dimensions = Abmessungen
omero-size = { $width } × { $height }
omero-z = { $count ->
    [one] { $count } Z-Ebene
   *[other] { $count } Z-Ebenen
}
omero-c = { $count ->
    [one] { $count } Kanal
   *[other] { $count } Kanäle
}
omero-t = { $count ->
    [one] { $count } Zeitpunkt
   *[other] { $count } Zeitpunkte
}
# The physical size of a pixel along X, Y (and Z).
label-omero-physical = Pixelgröße
label-omero-channels = Kanäle
# A channel not shown in the server's rendering.
omero-channel-hidden = { $channel } (ausgeblendet)
# How the pixels are fetched from the server: rendered tiles (JPEG) or the whole plane (PNG).
label-omero-fetched = Abgerufen als
omero-fetched-tiles = { $encoding }-Kacheln zu { $width } × { $height }, { $levels ->
    [one] { $levels } Ebene
   *[other] { $levels } Ebenen
}
omero-fetched-whole = Ganze Ebenen als { $encoding }

## Reflexionstransformationsbilder (PTM, RTI, Relight)

# machine-drafted, needs native review
group-rti = Reflexionstransformation
# The reflectance model the coefficients belong to: PTM (polynomial), HSH (hemispherical harmonics), RBF…
label-rti-basis = Basis
label-rti-order = Ordnung
# Coefficients per colour channel.
label-rti-terms = Koeffizienten
# How colour is stored: LRGB (one colour, varying luminance), RGB, YCC…
label-rti-colour = Farbanordnung
# Coefficient images stored in the file.
label-rti-planes = Ebenen
# The byte-to-coefficient mapping the file states, per coefficient.
label-rti-scale = Skalierung
label-rti-bias = Versatz

## Punktwolken

# machine-drafted, needs native review
group-points = Punktwolke
label-points = Punkte
# Points read of those in the file, when a very large cloud was evenly thinned: "32,000,000 of 120,000,000".
points-subsampled = { $loaded } von { $total }
# The LAS point data record format number (0–10).
label-point-format = Punktformat
# The coordinate reference system.
label-crs = Koordinatensystem
# What each point carries besides its position.
label-attributes = Attribute
points-channel-rgb = RGB
points-channel-intensity = Intensität
points-channel-classification = Klassifizierung
points-channel-returns = Echos
# ASPRS classes present, with point counts: "Ground (1,234), Building (567)".
label-classes = Klassen
# E57 scans, with point counts.
label-scans = Scans
point-class-0 = Nie klassifiziert
point-class-1 = Nicht klassifiziert
point-class-2 = Boden
point-class-3 = Niedrige Vegetation
point-class-4 = Mittlere Vegetation
point-class-5 = Hohe Vegetation
point-class-6 = Gebäude
point-class-7 = Tiefpunkt (Rauschen)
point-class-8 = Modellschlüsselpunkt
point-class-9 = Wasser
point-class-10 = Schiene
point-class-11 = Straßenoberfläche
point-class-12 = Überlappung
point-class-13 = Erdseil
point-class-14 = Leiterseil
point-class-15 = Freileitungsmast
point-class-16 = Leitungsverbinder
point-class-17 = Brückendeck
point-class-18 = Hohes Rauschen
point-class-19 = Überkopfstruktur
point-class-20 = Ignorierter Boden
point-class-21 = Schnee
point-class-22 = Zeitlicher Ausschluss
# A class without a standard name, by number.
point-class-other = Klasse { $class }

## Gaussian splats

# machine-drafted, needs native review
group-splats = Gaussian Splats
label-splats = Splats
# Splats shown of those in the file, when a very large file was evenly thinned: "12,000,000 of 30,000,000".
splats-shown-of = { $shown } von { $total }
# The degree (1 to 3) of the spherical harmonics that give each splat its view-dependent colour.
label-splat-harmonics = Blickwinkelabhängige Farbe
splats-harmonics-none = Keine
# $format: the file format, e.g. "SPZ".
splats-format-antialiased = { $format }, mit Anti-Aliasing trainiert

## Warnungen

# machine-drafted, needs native review
warn-not-diagnostic = Nicht für diagnostische Zwecke. Specular ist ein allgemeiner Bildbetrachter, kein Medizinprodukt: Die Darstellung ist nicht für die Diagnose kalibriert.

warn-gps = Diese Datei speichert, wo sie aufgenommen wurde. Entfernen Sie den Ort vor dem Teilen, wenn er privat bleiben soll.
warn-null-island = Die GPS-Position ist genau 0°, 0°; die Kamera hatte wahrscheinlich keinen Empfang, der Ort ist vermutlich falsch.
warn-date-future = Das Aufnahmedatum liegt in der Zukunft; die Kamerauhr war wahrscheinlich falsch eingestellt.
warn-icc-damaged = Das eingebettete Farbprofil ist beschädigt und konnte nicht gelesen werden.
warn-metadata-failed = Die Metadaten konnten wegen eines Fehlers in Specular nicht gelesen werden. Das Bild selbst ist in Ordnung.
warn-orientation-conflict = EXIF und XMP widersprechen sich bei der Ausrichtung (EXIF: { $exif }, XMP: { $xmp }). Der EXIF-Wert wird verwendet.
warn-exif-damaged = Ein Teil der EXIF-Daten konnte nicht gelesen werden ({ $error }); der Rest wird angezeigt.
warn-xmp-damaged = Die XMP-Metadaten sind fehlerhaft ({ $error }); sie konnten nur teilweise gelesen werden.

## Über-Dialog

geonames-attribution = Ortsnamen: GeoNames (www.geonames.org), lizenziert unter Creative Commons Namensnennung 4.0 (CC BY 4.0, https://creativecommons.org/licenses/by/4.0/). Offline-Suche mit der Bibliothek reverse_geocoder von Grant Miner (MIT oder Apache-2.0). Die Daten wurden in eine Städteliste umgewandelt; sie werden ohne Gewähr bereitgestellt.

## TIFF and DNG structure values (All fields)

subfile-type = { $value ->
    [0] Bild in voller Auflösung
    [1] Bild in reduzierter Auflösung
    [2] Eine Seite eines mehrseitigen Bildes
    [4] Transparenzmaske
   *[other] Sonstige
}
predictor = { $value ->
    [1] Keiner
    [2] Horizontale Differenzen
    [3] Gleitkomma-Differenzen
   *[other] Sonstige
}
extra-samples = { $value ->
    [0] Nicht angegeben
    [1] Vormultiplizierter Alphakanal
    [2] Unabhängiger Alphakanal
   *[other] Sonstige
}
sample-format = { $value ->
    [1] Ganzzahl ohne Vorzeichen
    [2] Ganzzahl mit Vorzeichen
    [3] Gleitkomma
    [4] Nicht definiert
   *[other] Sonstige
}

## Geodaten (GeoTIFF, GeoJSON, Shapefile, GeoPackage, KML/KMZ, GPX)

# machine-drafted, needs native review
group-geospatial = Geodaten
label-geo-crs = Koordinatenbezugssystem
label-geo-datum = Datum
label-geo-projection = Projektion
label-geo-unit = Einheiten
label-geo-bounds = Ausdehnung
label-geo-lonlat-bounds = Länge und Breite
label-geo-pixel-size = Pixelgröße
label-geo-overviews = Übersichten
label-geo-nodata = NoData-Wert
label-geo-raster-type = Rastertyp
label-geo-vertical = Höhenbezug
label-geo-citation = Quellenangabe
label-geo-features = Objekte
label-geo-vertices = Stützpunkte
label-geo-layer = Layer
label-geo-layers = Layer
label-geo-tiles = Kacheln
label-geo-encoding = Textkodierung
label-geo-warnings = Probleme
# The companion files a georeference was read from ("scene.jgw, scene.prj").
label-geo-sidecars = Begleitdateien
# machine-drafted, needs native review
# A projection Specular cannot convert to longitude and latitude: $method is its name.
geo-projection-unsupported = { $method } (Koordinaten nicht unterstützt)
# How longitude and latitude are brought to WGS 84 (what GPS and web maps use).
label-geo-to-wgs84 = Nach WGS 84
geo-to-wgs84-same = Nicht nötig
geo-to-wgs84-helmert = Datumsübergang (Helmert)
geo-to-wgs84-by-area = Datumsübergang (Helmert, nach Gebiet gewählt)
geo-to-wgs84-unknown = Unbekannt: Koordinaten im eigenen Datum der Datei
# A coordinate system's name and its EPSG code: "WGS 84 / UTM zone 33N (EPSG:32633)".
geo-crs-epsg = { $name } (EPSG:{ $code })
geo-crs-unknown = Nicht angegeben
# Map bounds: the lower-left and upper-right corners.
geo-bounds = { $x0 }, { $y0 } bis { $x1 }, { $y1 }
# Bounds in degrees: west to east longitude, south to north latitude.
geo-lonlat-bounds = { $west }° bis { $east }° Länge, { $south }° bis { $north }° Breite
geo-pixel-size = { $x } × { $y }
geo-overviews = { $n ->
    [one] { $n } Übersicht: { $sizes }
   *[other] { $n } Übersichten: { $sizes }
}
geo-pixel-is-point = Pixel sind Punkte
geo-layer = { $features ->
    [one] { $features } Objekt
   *[other] { $features } Objekte
}, { $fields ->
    [one] { $fields } Feld
   *[other] { $fields } Felder
}
# A GeoPackage tile pyramid: its table, the zoom level shown and its tile count.
geo-tiles = { $table }, Zoomstufe { $zoom }, { $tiles ->
    [one] { $tiles } Kachel
   *[other] { $tiles } Kacheln
}
fields-geo-geokeys = GeoTIFF-Schlüssel
fields-geo-gdal = GDAL-Metadaten
fields-geo-wkt = Koordinatenbezugssystem (WKT)
fields-geo-projjson = Koordinatenbezugssystem (PROJJSON)
# machine-drafted, needs native review
# What a web map tile set (MBTiles, PMTiles) says about itself: container, tile type, zoom
# levels, description and other metadata.
fields-geo-tileset = Kachelsatz
# The credit line a map's data asks to be shown with it ("© OpenStreetMap contributors").
label-geo-attribution = Quellenangabe
# The header lines of a georeferenced raster format (ENVI, ASCII grid, DTED, ERDAS IMAGINE).
fields-geo-header = Header
# machine-drafted, needs native review
# GIFTI files: the format version, what the file holds, the number of data arrays and the
# encoding of their values (e.g. "GZipBase64Binary").
label-gifti-version = GIFTI-Version
label-gifti-role = Enthält
gifti-role-surface = Oberfläche
gifti-role-labels = Labels pro Eckpunkt
gifti-role-data = Werte pro Eckpunkt
label-gifti-arrays = Datenarrays
label-gifti-encoding = Kodierung
label-gifti-vertices = Eckpunkte
label-gifti-triangles = Dreiecke
# The brain structure, e.g. "CortexLeft".
label-gifti-structure = Struktur
# A surface's geometric type, e.g. "Anatomical", "Inflated".
label-gifti-geometry = Geometrischer Typ
# The coordinate transform of the vertices: $from and $to are coordinate spaces such as
# "scanner", "Talairach" or "MNI 152".
label-gifti-transform = Koordinaten
gifti-transform = { $from } (Transformation nach { $to } nicht nötig)
gifti-transform-applied = { $from } → { $to } (angewendet)
# The surface file a data file is drawn on, found beside it.
label-gifti-surface = Dargestellt auf
# The data array shown: its name, NIfTI intent (e.g. "NIFTI_INTENT_SHAPE") and data type.
label-gifti-array = Array
label-gifti-intent = Intent
label-gifti-data-type = Datentyp
# Entries of the label table (names and colours of regions).
label-gifti-labels = Labeltabelle
# CIFTI-2 files: the version, NIfTI intent (e.g. "ConnDenseScalar") and matrix size.
label-cifti-version = CIFTI-Version
label-cifti-intent = Intent
label-cifti-matrix = Matrix
cifti-matrix = { $rows } × { $columns }
# The maps (pages) of the file and what they are.
label-cifti-maps = Karten
cifti-maps-scalars = { $count ->
        [one] { $count } Skalarkarte
       *[other] { $count } Skalarkarten
    }
cifti-maps-labels = { $count ->
        [one] { $count } Labelkarte
       *[other] { $count } Labelkarten
    }
cifti-maps-series = { $count ->
        [one] { $count } Zeitpunkt
       *[other] { $count } Zeitpunkte
    }
cifti-maps-parcels = { $count ->
        [one] { $count } Parzelle
       *[other] { $count } Parzellen
    }
cifti-maps-other = { $count }
# machine-drafted, needs native review
# A dense connectivity file (.dconn.nii): every brain place's connectivity to every other.
# $seeds: how many places can be picked; $size: the bytes read when one is picked (e.g. "357 KB").
label-cifti-dense = Konnektivität
cifti-dense = { $seeds ->
        [one] { $seeds } Seed
       *[other] { $seeds } Seeds
    }, { $size } gelesen pro Seed
# A time series' start and step; $unit is e.g. "second".
label-cifti-series = Serie
cifti-series = ab { $start }, alle { $step } ({ $unit })
label-cifti-models = Hirnmodelle
label-cifti-parcels = Parzellen
label-cifti-structures = Strukturen
label-cifti-voxels = Subkortikale Voxel
# A surface the file's values lie on: the brain structure (e.g. "CORTEX_LEFT"), its vertex
# count, and the surface file found beside the file for it, or that none was found.
label-cifti-surface = Oberfläche
cifti-surface-found = { $structure }: { $vertices ->
        [one] { $vertices } Eckpunkt
       *[other] { $vertices } Eckpunkte
    }, { $file }
cifti-surface-missing = { $structure }: benötigt eine Oberfläche mit { $vertices ->
        [one] { $vertices } Eckpunkt
       *[other] { $vertices } Eckpunkten
    } (keine neben der Datei)
# How the file is shown: on the surfaces found beside it, or as its subcortical voxels.
label-cifti-shown = Dargestellt als
cifti-shown-surfaces = Eingefärbte Oberflächen
cifti-shown-voxels = Volumen der subkortikalen Voxel
# machine-drafted, needs native review
# Shown with a picked seed's row: click a surface vertex, or move the crosshair in MPR.
cifti-shown-connectivity = Konnektivität des gewählten Seeds
# Maps left out of the voxel volume, which would be too large with them.
label-cifti-left-out = Ausgelassene Karten
# Tractograms: format, number of streamlines (fibre tracts) and points.
label-tract-format = Format
label-tract-streamlines = Streamlines
# A large tractogram draws only some of its streamlines.
tract-streamlines-shown = { $count } ({ $shown } angezeigt)
label-tract-points = Punkte
label-tract-mean-length = Mittlere Länge
# A length in millimetres.
tract-mm = { $value } mm
# Values per point and per streamline the file stores, by name.
label-tract-scalars = Skalare pro Punkt
label-tract-properties = Eigenschaften pro Streamline
# TrackVis: the voxel grid the tracts were made in, its voxel size and voxel order (e.g. "LAS").
label-tract-grid = Raster
label-tract-voxel-size = Voxelgröße
label-tract-voxel-order = Voxelreihenfolge
# How the points were brought into scanner (RAS) millimetres.
label-tract-space = Koordinaten
tract-space-vox-to-ras = Aus der Voxel-zu-RAS-Matrix
tract-space-voxel-order = Aus der Voxelreihenfolge (keine Voxel-zu-RAS-Matrix)
tract-space-scanner = Scannerraum
# MRtrix: the tracking step size.
label-tract-step = Schrittweite

## File headers (DPX, Cineon, XWD, MIFF, TIM, WSQ, JBIG)

# machine-drafted, needs native review
# What the file's own header records, for formats without Exif or XMP.
group-header = Dateikopf
label-header-created = Erstellt
# The program or person that wrote the file.
label-header-creator = Urheber
label-header-copyright = Copyright
label-header-project = Projekt
label-header-comment = Kommentar
# The scanner, camera or other device the image came from.
label-header-device = Gerät
# SMPTE time code of the frame: "01:00:00:12".
label-header-timecode = Timecode
# A film frame's edge code (KeyKode).
label-header-edge-code = Randnummer
label-header-frame-rate = Bildrate
# Where the frame sits in its sequence.
label-header-frame = Einzelbild
# Film stock and slate.
label-header-film = Film
# How code values relate to light: printing density, linear, BT.709…
label-header-transfer = Transferfunktion
# The colour primaries the file states.
label-header-colorimetric = Farbmetrik
# The X window a screen dump was taken of.
label-header-window = Fenster
label-header-resolution = Auflösung
label-header-compression = Kompression
# machine-drafted, needs native review
# JPEG XS: the profile and the level the codestream states ("Main 422.10"; "4k-1, sublevel 6 bpp").
label-header-profile = Profil
# machine-drafted, needs native review
label-header-level = Level
# machine-drafted, needs native review
# JPEG XS: how many wavelet levels, horizontal and vertical ("5 horizontal, 1 vertical").
label-header-decomposition = Wavelet-Stufen
# machine-drafted, needs native review
# JPEG XS: the colour transform ("None", "Reversible colour transform (RCT)", "Star-Tetrix, RGGB mosaic").
label-header-colour-transform = Farbtransformation
# machine-drafted, needs native review
# Photoshop: the artboards, by name and size ("Phone (375 × 667), Tablet (768 × 1024)").
label-header-artboards = Zeichenflächen
# machine-drafted, needs native review
# Photoshop file without a merged image, shown from its layers: the features it leaves out,
# by Photoshop's names ("Color Lookup, Knockout").
label-header-not-drawn = Nicht gezeichnet
# machine-drafted, needs native review
# The same, for the features drawn only approximately ("Bevel & Emboss, Satin").
label-header-approximated = Näherungsweise gezeichnet

## Fonts (TTF, OTF, TTC/OTC, WOFF, WOFF2)

# machine-drafted, needs native review
group-font = Schrift
label-font-family = Familie
label-font-style = Stil
# The face's place in a font collection: "2 of 4".
label-font-face = Schnitt
font-face-of = { $index } von { $count }
label-font-version = Version
label-font-designer = Gestaltung
# The font's maker (the name table's manufacturer).
label-font-manufacturer = Schriftgießerei
label-font-glyphs = Glyphen
label-font-outlines = Konturen
font-outlines-truetype = TrueType (quadratisch)
font-outlines-cff = PostScript (CFF, kubisch)
font-outlines-cff2 = PostScript (CFF2, kubisch)
font-outlines-bitmap = Nur Bitmaps
font-outlines-none = Keine
# What the font's licence flags (OS/2 fsType) allow documents that embed it to do.
label-font-embedding = Einbetten
font-embedding-installable = Darf eingebettet und installiert werden.
font-embedding-restricted = Darf nicht eingebettet werden.
font-embedding-preview-print = Darf nur zum Ansehen und Drucken eingebettet werden.
font-embedding-editable = Darf in bearbeitbare Dokumente eingebettet werden.
font-embedding-no-subsetting = Nur vollständig, nicht als Teilmenge.
font-embedding-bitmap-only = Nur als Bitmaps.
# Variation axes of a variable font: "Weight 100–900 (400)".
label-font-axes = Variationsachsen
font-axis = { $name } { $min }–{ $max } ({ $default })
label-font-instances = Benannte Stile
label-font-colour = Farbglyphen
font-colour-colr0 = COLR-Ebenen
font-colour-colr1 = COLR-v1-Verläufe
font-colour-sbix = Bitmaps (sbix)
font-colour-cbdt = Bitmaps (CBDT)
font-colour-svg = SVG
label-font-weight = Schriftstärke
label-font-units-per-em = Einheiten pro Geviert
label-font-tables = Tabellen
label-font-licence = Lizenz
label-font-licence-url = Lizenz-URL
label-font-copyright = Copyright
label-font-trademark = Marke

## Office documents (Word, Excel, PowerPoint, iWork, OpenDocument)

# machine-drafted, needs native review
group-document = Dokument
label-document-kind = Art
document-kind-word = Word-Dokument
document-kind-excel = Excel-Arbeitsmappe
document-kind-powerpoint = PowerPoint-Präsentation
document-kind-pages = Pages-Dokument
document-kind-numbers = Numbers-Tabelle
document-kind-keynote = Keynote-Präsentation
document-kind-odf-text = OpenDocument-Text
document-kind-odf-spreadsheet = OpenDocument-Tabelle
document-kind-odf-presentation = OpenDocument-Präsentation
document-kind-odf-graphics = OpenDocument-Zeichnung
document-kind-odf-other = OpenDocument-Datei
# machine-drafted, needs native review
document-kind-playlist = Playlist
# machine-drafted, needs native review
label-playlist-items = Einträge
# Appended to the kind: "Word document, template, with macros".
document-template = Vorlage
document-macros = mit Makros
document-legacy = Office 97–2003-Format
label-document-preview = Vorschau
# The image format of the preview stored in the file, e.g. "JPEG image".
document-preview-image = { $format }-Bild
document-preview-none = Keine in dieser Datei
label-document-subject = Thema
label-document-author = Autor
label-document-description = Beschreibung
label-document-category = Kategorie
label-document-last-saved-by = Zuletzt gespeichert von
label-document-created = Erstellt
label-document-modified = Geändert
label-document-printed = Zuletzt gedruckt
label-document-pages = Seiten
label-document-slides = Folien
label-document-hidden-slides = Ausgeblendete Folien
label-document-notes = Notizseiten
label-document-sheets = Blätter
label-document-words = Wörter
label-document-characters = Zeichen
label-document-paragraphs = Absätze
label-document-lines = Zeilen
label-document-editing-time = Bearbeitungszeit
document-minutes = { $minutes } Min.
label-document-revision = Revision
label-document-application = Gespeichert mit
label-document-company = Firma
label-document-template = Vorlage

## Books (EPUB)

# machine-drafted, needs native review
group-book = Buch
label-book-authors = Autoren
label-book-publisher = Verlag
label-book-date = Erschienen
label-book-language = Sprache
label-book-pages = Seiten
# Entries in the book's table of contents.
label-book-contents = Inhaltseinträge
label-book-layout = Layout
book-layout-reflowable = Umfließend
book-layout-fixed = Festes Layout
book-right-to-left = von rechts nach links
label-book-version = Format
book-version = EPUB { $version }
label-book-fonts = Eingebettete Schriften
book-fonts-obfuscated = { $count } ({ $obfuscated } verschleiert)
label-book-subjects = Themen
label-book-identifier = Kennung
label-book-rights = Rechte

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
volume-orient-parrec = Philips-Angulation und Off-Center
volume-orient-ecat = ECAT-Voxelgrößen und Versätze (unzuverlässig)
label-volume-technique = Technik
label-volume-version = Formatversion
label-volume-isotope = Isotop
label-volume-tracer = Tracer
label-volume-units = Einheiten

# Text, tables and subtitles.
label-header-encoding = Zeichensatz
label-header-lines = Zeilen
label-header-line-ends = Zeilenenden
label-header-words = Wörter
label-header-pages = Seiten
label-header-pictures = Bilder
label-header-pictures-not-shown = Nicht gezeigte Bilder
label-header-tables = Tabellen
label-header-rows = Tabellenzeilen
label-header-columns = Spalten
label-header-delimiter = Trennzeichen
label-header-column-names = Spaltennamen
label-header-cues = Einblendungen
label-header-cue-span = Einblendungen von – bis
