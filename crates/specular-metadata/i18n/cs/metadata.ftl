# machine-drafted, needs native review
# Specular — panel metadat. Čeština.
#
# Konvence
# - Čísla přicházejí již naformátovaná pro národní prostředí (ICU4X) jako text, např.
#   $num = "2 456 789". Kde je potřeba tvar množného čísla, předává se i surový počet
#   jako $n (číslo), který slouží jen k výběru tvaru.
# - Data a časy přicházejí již naformátované pomocí ICU4X ($date, $time).
# - Každá vložená hodnota je ve Fluentu obalena izolačními znaky Unicode.
# - Identifikátory zpráv jsou stálé; nikdy nepoužívejte identifikátor pro jiný význam.

## Názvy skupin souhrnu

group-camera = Fotoaparát
group-exposure = Expozice
group-lens = Objektiv
group-date = Datum a čas
group-location = Poloha
# machine-drafted, needs native review
# A DJI drone photo: the gimbal's and the aircraft's attitude, the height above take-off.
group-drone = Dron
label-gimbal = Gimbal
label-flight = Letoun
label-relative-altitude = Nad místem vzletu
drone-attitude = stáčení { $yaw }, klopení { $pitch }, klonění { $roll }
# A DJI thermal camera's radiometric JPEG: its photo opens, its temperatures can't.
label-thermal-data = Teploty
drone-thermal-refused = Nezobrazeno: licence SDK od DJI to nedovoluje
drone-thermal-refused-detail = Čtení teplot DJI vyžaduje DJI Thermal SDK, jehož licence dovoluje jeho použití jen programům, které ho dodávají. Snímek a jeho metadata se zobrazují.
group-image = Obrázek
group-file = Soubor
group-rights = Práva a popis
group-credentials = Pověření obsahu
# machine-drafted, needs native review
group-medical = Lékařský snímek
# machine-drafted, needs native review
group-inspection = Součást a kontrola
group-nde-image = Kontrolní snímek (DICONDE)
# machine-drafted, needs native review
# Industrial CT volumes (Zeiss Xradia, Amira, Bruker SkyScan, Nikon X-Tek, VGI, raw volumes).
group-industrial-image = Kontrolní snímek
group-science = Vědecký snímek
group-em = Elektronová mikroskopie
group-spm = Rastrovací sondová mikroskopie
group-slide = Preparát
group-microscopy = Mikroskopie
# X-ray diffraction detector frames (CBF, EDF, SMV, Bruker, MarCCD, R-AXIS).
group-diffraction = Difrakce
group-surface = Topografie povrchu
group-ebsd = Difrakce zpětně odražených elektronů
group-seismic = Seismická data
# machine-drafted, needs native review
group-asdf = ASDF
# machine-drafted, needs native review
# CERN ROOT files (particle physics histograms and graphs).
group-root = ROOT
# machine-drafted, needs native review
# CASA images (radio interferometry: the .image, .residual, .model directories tclean writes).
group-casa = Obraz CASA
# machine-drafted, needs native review
# GIFTI files: brain surfaces and the values on their vertices (Human Connectome Project
# and FreeSurfer exports).
group-gifti = GIFTI
# CIFTI-2 files: maps over cortical surfaces and subcortical voxels ("grayordinates").
group-cifti = CIFTI
# White-matter fibre tracts (TrackVis, MRtrix).
group-tractography = Traktografie
# machine-drafted, needs native review
group-annotations = Anotace
# Imaging mass spectrometry (imzML): a mass spectrum per pixel.
group-mass-spec = Zobrazovací hmotnostní spektrometrie
# machine-drafted, needs native review
group-thermal = Termosnímek
# machine-drafted, needs native review
# Flow cytometry list-mode data (FCS): events of a cell sorter or analyser.
group-cytometry = Průtoková cytometrie
# machine-drafted, needs native review
group-spectroscopy = Spektroskopie
# machine-drafted, needs native review
# Unstructured meshes of ocean, coastal and climate models (UGRID, ICON, MPAS in NetCDF).
group-ugrid = Nestrukturovaná síť
group-drawing = Výkres
# machine-drafted, needs native review
group-volume = Objem

## Popisky položek souhrnu

label-camera = Fotoaparát
label-software = Software
label-application = Aplikace
label-shutter = Čas závěrky
label-aperture = Clona
label-iso = ISO
label-focal-length = Ohnisková vzdálenost
label-exposure-bias = Korekce expozice
label-program = Režim
label-metering = Měření
label-white-balance = Vyvážení bílé
label-flash = Blesk
label-lens = Objektiv
label-lens-make = Výrobce objektivu
label-lens-range = Rozsah objektivu
# machine-drafted, needs native review
label-focus-mode = Režim ostření
label-stabilisation = Stabilizace obrazu
label-taken = Pořízeno
label-digitised = Digitalizováno
label-edited = Naposledy upraveno
label-place = Místo
label-coordinates = Souřadnice
label-altitude = Nadmořská výška
label-direction = Směr
label-accuracy = Přesnost
label-gps-time = Čas GPS
label-dimensions = Rozměry
label-encoding = Kódování
# machine-drafted, needs native review
label-motion = Pohyb
motion-kind-motion-photo = Pohyblivá fotografie
motion-kind-micro-video = Pohyblivá fotografie (MicroVideo)
motion-kind-samsung = Pohyblivá fotografie Samsung
motion-kind-live-photo = Live Photo
motion-duration = { $seconds } s
label-colour-profile = Barevný profil
label-orientation = Orientace
label-file-name = Název
label-folder = Složka
label-file-size = Velikost
label-format = Formát
label-modified = Změněno
label-title = Název
label-description = Popis
label-creator = Autor
label-copyright = Autorská práva
label-keywords = Klíčová slova
label-rating = Hodnocení
label-colour-label = Štítek
label-credentials = Pověření obsahu

## Názvy skupin „Všechna pole“ (názvy značek zůstávají podle standardů)

fields-ifd0 = IFD0 (hlavní obrázek)
fields-exif = Exif
fields-gps = GPS
fields-interop = Interoperabilita
fields-ifd1 = IFD1 (miniatura)
fields-icc = Hlavička profilu ICC
# $namespace: z xmp-namespace; $prefix: např. "dc"
fields-xmp = XMP: { $namespace } ({ $prefix })
xmp-namespace = { $prefix ->
    [dc] Dublin Core
    [xmp] Základní
    [xmpRights] Práva
    [xmpMM] Správa médií
    [xmpDM] Dynamická média
    [tiff] TIFF
    [exif] Exif
    [exifEX] Exif 2.3+
    [aux] Příslušenství fotoaparátu
    [photoshop] Photoshop
    [crs] Nastavení Camera Raw
    [lr] Lightroom
    [Iptc4xmpCore] IPTC Core
    [Iptc4xmpExt] IPTC Extension
    [plus] Licencování PLUS
    [GPano] Fotosféra
    [GCamera] Fotoaparát Google
    [hdrgm] Mapa zisku HDR
    [mwg-rs] Oblasti
    [pdf] PDF
    [stEvt] Události zdroje
    [stRef] Odkazy na zdroje
   *[other] Jiné
}
# $number: "0xA500"; $context: "Exif", "GPS", …
unknown-tag = Neznámá značka { $number } ({ $context })
blob-bytes = { $num ->
    [one] ({ $num } bajt)
    [few] ({ $num } bajty)
    [many] ({ $num } bajtu)
   *[other] ({ $num } bajtů)
}
maker-note-bytes = ({ $num } B, poznámka výrobce, nedekódováno)
more-values = { $shown }, … (hodnot celkem: { $total })

## Obecné spojovací prvky

# Odděluje části seznamu sestaveného v kódu (popis blesku, oblasti místa).
list-separator = {", "}
# Odděluje čas · clonu · ISO · ohniskovou vzdálenost v jednořádkovém souhrnu.
one-line-separator = {" · "}

## Expozice

exposure-time-fraction = 1/{ $denominator } s
exposure-time-seconds = { $seconds } s
aperture-value = f/{ $f }
iso-value = ISO { $iso }
focal-length-value = { $mm } mm
focal-length-with-equiv = { $mm } mm (ekv. { $equiv } mm)
focal-length-equiv-only = ekv. { $equiv } mm
# $ev je velikost; znaménko určuje selektor.
exposure-bias-value = { $sign ->
    [plus] +{ $ev } EV
    [minus] −{ $ev } EV
   *[zero] 0 EV
}
exposure-program = { $value ->
    [1] Manuální
    [2] Program (automatika)
    [3] Priorita clony
    [4] Priorita závěrky
    [5] Kreativní (hloubka ostrosti)
    [6] Akce (krátký čas)
    [7] Portrét
    [8] Krajina
   *[other] Neznámý
}
metering-mode = { $value ->
    [1] Průměrové
    [2] Zdůrazněný střed
    [3] Bodové
    [4] Vícebodové
    [5] Maticové (poměrové)
    [6] Částečné
   *[other] Jiné
}
# machine-drafted, needs native review
# Canon maker note CameraSettings FocusMode; 256–258 are the live-view variants of 0–2.
canon-focus-mode = { $value ->
    [0] One-Shot AF
    [1] AI Servo AF
    [2] AI Focus AF
    [3] Ruční zaostřování
    [4] Jednorázové
    [5] Průběžné
    [6] Ruční zaostřování
    [16] Pan focus
    [256] One-Shot AF (živý náhled)
    [257] AI Servo AF (živý náhled)
    [258] AI Focus AF (živý náhled)
    [512] Zaostřování Movie Snap
    [519] Movie Servo AF
   *[other] Jiné
}
# Canon maker note CameraSettings ImageStabilization; 256–260 repeat 0–4.
canon-stabilisation = { $value ->
    [0] Vypnuto
    [1] Zapnuto
    [2] Pouze při snímání
    [3] Panorámování
    [4] Dynamická
    [256] Vypnuto
    [257] Zapnuto
    [258] Pouze při snímání
    [259] Panorámování
    [260] Dynamická
   *[other] Jiné
}
# machine-drafted, needs native review
panasonic-focus-mode = { $value ->
    [1] Automaticky
    [2] Ruční ostření
    [4] Automaticky (tlačítko ostření)
    [5] Automaticky (průběžně)
    [6] AF-S
    [7] AF-C
    [8] AF-F
   *[other] Jiné
}
# machine-drafted, needs native review
panasonic-stabilisation = { $value ->
    [2] Zapnuto (optická)
    [3] Vypnuto
    [4] Zapnuto (režim 2)
    [5] Zapnuto (optická, panorámování)
    [6] Zapnuto (jen tělo)
    [7] Zapnuto (jen tělo, panorámování)
    [9] Dual IS
    [10] Dual IS (panorámování)
    [11] Dual IS 2
    [12] Dual IS 2 (panorámování)
   *[other] Jiné
}
label-comment = Komentář
fields-tags = Štítky { $kind }
group-picture = Nastavení obrazu
fields-fujifilm = Data výrobce Fujifilm
label-film-simulation = Simulace filmu
label-dynamic-range = Dynamický rozsah
label-highlight-tone = Světla
label-shadow-tone = Stíny
label-colour-setting = Barva
label-sharpness = Ostrost
label-noise-reduction = Potlačení šumu
label-clarity = Čistota
label-grain-effect = Efekt zrna
label-colour-chrome = Barevný chrom
label-colour-chrome-blue = Barevný chrom modrá
label-shutter-type = Typ závěrky
label-drive-mode = Režim snímání
fuji-step = { $sign ->
    [plus] +{ $n }
    [minus] −{ $n }
   *[zero] 0
}
fuji-film-mode = { $value ->
    [0] Provia (standardní)
    [256] Studiový portrét
    [272] Studiový portrét (větší sytost)
    [288] Astia (měkký)
    [304] Studiový portrét (ostřejší)
    [512] Velvia (živý)
    [768] Studiový portrét Ex
    [1024] Velvia
    [1280] Pro Neg. Std
    [1281] Pro Neg. Hi
    [1536] Classic Chrome
    [1792] Eterna
    [2048] Classic Neg.
    [2304] Eterna Bleach Bypass
    [2560] Nostalgic Neg.
    [2816] Reala Ace
   *[other] Jiné
}
fuji-monochrome = { $value ->
    [768] Černobílý
    [769] Černobílý, červený filtr
    [770] Černobílý, žlutý filtr
    [771] Černobílý, zelený filtr
    [784] Sépie
    [1280] Acros
    [1281] Acros, červený filtr
    [1282] Acros, žlutý filtr
    [1283] Acros, zelený filtr
   *[other] Černobílý
}
fuji-white-balance = { $value ->
    [0] Automatické
    [1] Automatické (priorita bílé)
    [2] Automatické (priorita atmosféry)
    [256] Denní světlo
    [512] Zataženo
    [768] Zářivka denní světlo
    [769] Zářivka denní bílá
    [770] Zářivka bílá
    [771] Zářivka teplá bílá
    [772] Zářivka teplá bílá do obytných prostor
    [1024] Žárovka
    [1280] Blesk
    [1536] Pod vodou
    [3840] Vlastní 1
    [3841] Vlastní 2
    [3842] Vlastní 3
    [3843] Vlastní 4
    [3844] Vlastní 5
    [4080] Teplota barev
   *[other] Jiné
}
fuji-white-balance-kelvin = { $kelvin } K
fuji-dynamic-range = DR{ $percent }
fuji-dynamic-range-auto = Automatické (DR{ $percent })
fuji-dynamic-range-setting = { $value ->
    [0] Automatické
    [256] Standardní (DR100)
    [512] Široký 1 (DR230)
    [513] Široký 2 (DR400)
   *[other] Jiné
}
fuji-effect = { $value ->
    [0] Vypnuto
    [32] Slabý
    [64] Silný
   *[other] Jiné
}
fuji-grain-size = { $value ->
    [16] malé
    [32] velké
   *[other] jiné
}
fuji-grain = { $strength }, { $size }
fuji-shutter-type = { $value ->
    [0] Mechanická
    [1] Elektronická
    [2] Elektronická (dlouhé expozice)
    [3] Elektronická první lamela
   *[other] Jiné
}
fuji-drive-mode = { $value ->
    [0] Jednotlivý snímek
    [1] Sériové, nízká rychlost
    [2] Sériové, vysoká rychlost
   *[other] Jiné
}
fuji-focus-mode = { $value ->
    [0] Ruční
    [1] AF-S
    [2] AF-C
    [100] Automatické
    [101] Ruční
   *[other] Jiné
}
fuji-stabilisation = { $kind } ({ $mode })
fuji-stabilisation-kind = { $value ->
    [0] Žádná
    [1] Optická
    [2] Posun snímače
    [3] Objektiv
    [258] Posun snímače a digitální
    [512] Digitální
   *[other] Jiné
}
fuji-stabilisation-mode = { $value ->
    [0] Vypnuto
    [1] zapnuto, nepřetržitě
    [2] zapnuto, při snímání
   *[other] Jiné
}
light-source = { $value ->
    [1] Denní světlo
    [2] Zářivka
    [3] Žárovka
    [4] Blesk
    [9] Jasno
    [10] Zataženo
    [11] Stín
    [12] Zářivka denní světlo
    [13] Zářivka denní bílá
    [14] Zářivka studená bílá
    [15] Zářivka bílá
    [16] Zářivka teplá bílá
    [17] Standardní světlo A
    [18] Standardní světlo B
    [19] Standardní světlo C
    [20] D55
    [21] D65
    [22] D75
    [23] D50
    [24] Studiová žárovka
   *[other] Jiné
}
white-balance-auto = Automatické
white-balance-manual = Ruční
white-balance-manual-with-light = Ruční ({ $light })
# Popis blesku se skládá z těchto částí spojených pomocí list-separator.
flash-none = Bez funkce blesku
flash-fired = Blesk odpálen
flash-not-fired = Blesk neodpálen
flash-forced-on = vynuceně zapnut
flash-forced-off = vynuceně vypnut
flash-auto = automatický režim
flash-return-not-detected = odražené světlo nezjištěno
flash-return-detected = odražené světlo zjištěno
flash-red-eye = redukce červených očí

## Objektiv

lens-focal-single = { $mm } mm
lens-focal-range = { $min }–{ $max } mm
lens-aperture-single = f/{ $f }
lens-aperture-range = f/{ $min }–{ $max }
lens-range = { $focal } { $aperture }

## Datum a čas

# $weekday: "úterý" (ICU); $date: "14. května 2024" (ICU)
date-with-weekday = { $weekday } { $date }
date-time = { $date }, { $time }
date-time-zone = { $datetime } ({ $zone })
zone-utc = UTC
# $offset: "+1", "+5:30", "−3"
zone-offset = UTC{ $offset }
gps-date-time = { $date }, { $time } UTC
detail-zone-from-gps = Časové pásmo odvozeno z hodin GPS

## Poloha

# $lat a $lon jsou bez znaménka; polokouli určuje selektor.
coordinates = { $lat }° { $latRef ->
    [south] j. š.
   *[north] s. š.
}, { $lon }° { $lonRef ->
    [west] z. d.
   *[east] v. d.
}
altitude = { $ref ->
    [above] { $m } m nad mořem
    [below] { $m } m pod hladinou moře
   *[sea] Na úrovni hladiny moře
}
compass-point = { $point ->
    [n] sever
    [nne] severoseverovýchod
    [ne] severovýchod
    [ene] východoseverovýchod
    [e] východ
    [ese] východojihovýchod
    [se] jihovýchod
    [sse] jihojihovýchod
    [s] jih
    [ssw] jihojihozápad
    [sw] jihozápad
    [wsw] západojihozápad
    [w] západ
    [wnw] západoseverozápad
    [nw] severozápad
   *[nnw] severoseverozápad
}
# $point: z compass-point; $degrees: "287"
facing = Směr: { $point } ({ $degrees }°)
detail-true-north = Vzhledem k zeměpisnému severu
detail-magnetic-north = Vzhledem k magnetickému severu
accuracy = ±{ $m } m
distance-km = { $km } km
place = { $city }, { $country }
place-city-only = { $city }
# Více než 30 km od nalezené obce.
place-near = poblíž: { $place }
place-detail = { $areas } · { $distance } od: { $city }
place-detail-distance = { $distance } od: { $city }

## Obrázek

dimensions = { $width } × { $height }
dimensions-megapixels = { $width } × { $height } ({ $mp } MP)
encoding-bit-depth = { $bits } bitů
# $format: "8 bitů YCbCr 4:2:0"
encoding-progressive = { $format }, progresivní
encoding-baseline = { $format }, základní (baseline)
encoding-hierarchical = { $format }, hierarchický
# machine-drafted, needs native review
encoding-alpha = { $format } { $alpha ->
    [premultiplied] s předem vynásobeným alfa kanálem
    [transparency] s průhledností
   *[alpha] s alfa kanálem
}
# [Lab], [Palette] and the variants after them: machine-drafted, needs native review
colour-model = { $model ->
    [YCbCr] YCbCr
    [RGB] RGB
    [Greyscale] Stupně šedi
    [CMYK] CMYK
    [YCCK] YCCK
    [Lab] L*a*b*
    [Palette] Indexované barvy
    [Multichannel] Vícekanálový
    [Duotone] Duplex
    [YCgCo] YCgCo
    [XYB] XYB
    [YC] Jas/chrominance (YC)
    [Bayer] Bayer RAW
    [X-Trans] X-Trans RAW
    [CFA] RAW s maskou barevných filtrů
    [LinearRaw] Lineární RAW
    [RGBE] RGBE (sdílený exponent)
    [XYZE] XYZE (sdílený exponent)
    [R] Jeden kanál (R)
    [RG] Dva kanály (RG)
   *[other] { $model }
}
adobe-rgb-from-exif = Adobe RGB (ze značky EXIF; bez vloženého profilu)
orientation = { $value ->
    [1] Na výšku správně (bez otočení)
    [2] Zrcadleno vodorovně (použito)
    [3] Otočeno o 180° (použito)
    [4] Zrcadleno svisle (použito)
    [5] Zrcadleno vodorovně a otočeno o 90° proti směru hodinových ručiček (použito)
    [6] Otočeno o 90° po směru hodinových ručiček (použito)
    [7] Zrcadleno vodorovně a otočeno o 90° po směru hodinových ručiček (použito)
    [8] Otočeno o 90° proti směru hodinových ručiček (použito)
   *[other] Neznámá ({ $value })
}
# Krátký tvar pro varování.
orientation-short = { $value ->
    [1] Bez otočení
    [2] Zrcadleno vodorovně
    [3] Otočeno o 180°
    [4] Zrcadleno svisle
    [5] Zrcadleno vodorovně a otočeno o 90° proti směru hodinových ručiček
    [6] Otočeno o 90° po směru hodinových ručiček
    [7] Zrcadleno vodorovně a otočeno o 90° po směru hodinových ručiček
    [8] Otočeno o 90° proti směru hodinových ručiček
   *[other] hodnota { $value }
}

## Soubor

file-size-bytes = { $num ->
    [one] { $num } bajt
    [few] { $num } bajty
    [many] { $num } bajtu
   *[other] { $num } bajtů
}
# Desítkové jednotky (1 kB = 1000 bajtů).
file-size-unit = { $num } { $unit ->
    [kb] kB
    [mb] MB
    [gb] GB
    [tb] TB
   *[pb] PB
}

## Práva

rating-rejected = Zamítnuto
# $stars: "★★★★☆"
rating-stars = { $stars } ({ $n } z 5)

## Pověření obsahu

c2pa-present = Přítomno (neověřeno)
c2pa-detail = Tento soubor obsahuje manifest C2PA; jeho podpis nebyl ověřen.

## Hodnoty hlavičky ICC

icc-class = { $class ->
    [scnr] Vstupní zařízení
    [mntr] Zobrazovací zařízení
    [prtr] Výstupní zařízení
    [link] Propojení zařízení
    [spac] Barevný prostor
    [abst] Abstraktní
    [nmcl] Pojmenované barvy
   *[other] Neznámá
}
icc-intent = { $intent ->
    [0] Perceptuální
    [1] Relativní kolorimetrický
    [2] Sytost
    [3] Absolutní kolorimetrický
   *[other] Neznámý
}
icc-platform = { $platform ->
    [APPL] Apple
    [MSFT] Microsoft
    [SGI] Silicon Graphics
    [SUNW] Sun Microsystems
    [none] Neuvedeno
   *[other] Neznámá
}
icc-coded = { $name } ({ $code })

# machine-drafted, needs native review
label-modality = Modalita
label-body-part = Část těla
label-study = Studie
label-series = Série
label-slice-thickness = Tloušťka řezu
label-pixel-spacing = Rozteč pixelů
label-imager-pixel-spacing = Rozteč pixelů na detektoru
label-frames = Snímky
label-transfer-syntax = Přenosová syntaxe
label-document = Dokument
label-document-type = Typ dokumentu
medical-spacing = { $row } × { $column } mm
medical-mm = { $mm } mm
medical-lossy = { $name } (ztrátová komprese)
detail-pixel-spacing = Vzdálenost středů pixelů v pacientovi (řádky × sloupce).
# machine-drafted, needs native review
# Pixel spacing of an industrial inspection image (DICONDE, CT of an object).
detail-pixel-spacing-object = Vzdálenost středů pixelů v objektu (řádky × sloupce).
detail-imager-pixel-spacing = Měřeno na detektoru, bez korekce zvětšení: vzdálenosti v pacientovi bývají menší.

## Průmyslová kontrola (DICONDE)

# machine-drafted, needs native review
label-nde-component-shape = Tvar součásti
label-nde-component-manufacturer = Výrobce součásti
label-nde-material = Materiál
label-nde-thickness = Tloušťka materiálu
label-nde-pipe-diameter = Průměr trubky
label-nde-source = Zdroj
label-nde-environment = Prostředí
label-nde-iqi = Indikátor kvality obrazu
label-nde-indications = Indikace
label-nde-standard = Norma

## Výzkumné objemy (NIfTI, Analyze, NRRD, MetaImage)

# machine-drafted, needs native review
label-voxel-size = Velikost voxelu
label-data-type = Datový typ
label-time-points = Časové body
label-orientation-source = Orientace z
label-intent = Hodnoty
label-compression = Komprese
volume-voxel-size = { $x } × { $y } × { $z } mm
volume-orient-sform = NIfTI sform
volume-orient-qform = NIfTI qform
volume-orient-analyze = Hlavička Analyze (nespolehlivá)
volume-orient-space = Prostorové směry NRRD
volume-orient-transform = Transformace MetaImage
volume-orient-spacings = Pouze rozteč voxelů
volume-orient-none = Neuvedeno
volume-orient-vox2ras = FreeSurfer vox2ras
volume-orient-mgh-default = Výchozí FreeSurfer (nespolehlivé)
volume-orient-minc = Směrové kosiny dimenzí MINC
volume-orient-object = Souřadnice objektu (bez orientace pacienta)
volume-content-projections = Projekce (surový sken)
volume-content-reconstruction = Rekonstruovaný objem
volume-coords-rectilinear = Pravoúhlá (nerovnoměrné rozestupy, zobrazeno rovnoměrně)
# Scan parameters and tags a FreeSurfer MGH file keeps after its voxels.
label-volume-tr = Repetiční čas
label-volume-te = Echo čas
label-volume-ti = Inverzní čas
label-volume-flip = Sklápěcí úhel
label-volume-fov = Zorné pole
label-volume-field = Síla pole
label-volume-pedir = Fázové kódování
label-volume-cmdline = Příkazový řádek
label-volume-xform = Transformace Talairach
label-volume-content = Obsah
label-volume-projections = Projekce
label-volume-angles = Rozsah otáčení
label-volume-angle-step = Krok otáčení
label-volume-kv = Napětí rentgenky
label-volume-current = Proud rentgenky
label-volume-power = Výkon zdroje
label-volume-exposure = Expozice
label-volume-sod = Zdroj–objekt
label-volume-odd = Objekt–detektor
label-volume-sdd = Zdroj–detektor
label-volume-magnification = Optické zvětšení
label-volume-binning = Binning
label-volume-objective = Objektiv
label-volume-filter = Filtr zdroje
label-volume-camera = Detektor
label-volume-instrument = Přístroj
label-volume-software = Software
label-volume-date = Pořízeno
label-volume-ring = Korekce prstencových artefaktů
label-volume-smoothing = Vyhlazení
label-volume-coords = Mřížka
# machine-drafted, needs native review
# An eye scan (OCT): which eye.
label-volume-eye = Oko
volume-eye-right = Pravé (OD)
volume-eye-left = Levé (OS)
detail-volume-unreliable = Soubor neuvádí, kam v pacientovi směřují jeho osy: levá a pravá strana mohou být prohozené, proto se nezobrazují popisky orientace.
# machine-drafted, needs native review
# An industrial CT volume: its axes are the scanner's, there is no patient.
detail-volume-object = Osy patří skeneru, ne pacientovi, proto se nezobrazují popisky orientace.

## Vědecké snímky (FITS, XISF)

label-object = Objekt
label-telescope = Dalekohled
label-instrument = Přístroj
label-filter = Filtr
label-observed = Pozorováno
label-exposure-time = Expozice
label-sample = Vzorky
label-unit = Jednotka
label-hdu = Rozšíření
label-centre = Střed
label-centre-deg = Střed (stupně)
label-frame = Souřadnicová soustava
# machine-drafted, needs native review
science-frame-galactic = Galaktická
science-frame-ecliptic = Ekliptikální
label-projection = Projekce
label-pixel-scale = Měřítko pixelu
science-seconds = { $seconds } s
science-arcsec-px = { $arcsec }″ na pixel
label-distortion = Zkreslení
label-wcs = Nebeské souřadnice
science-wcs-unsupported = Nepodporováno: { $reason }
# machine-drafted, needs native review
science-wcs-approximate = Přibližné (pouze z pointace, bez zkreslení)
label-observer = Pozorovatel
label-bayer = Bayerova maska
label-capture-start = Začátek záznamu (UTC)
label-capture-end = Konec záznamu (UTC)
label-frame-rate = Snímková frekvence
science-fps = { $fps } snímků/s
science-frames-cut = { $frames } z { $declared }
# The HDF5 dataset or NetCDF variable shown.
label-dataset = Datová sada
label-pixel-size = Velikost pixelu
# "0.25 nm per pixel"; $size may be "0.25 × 0.5".
science-pixel-size = { $size } { $unit } na pixel
# Microscopy (DigitalMicrograph, MRC, Gwyddion, Igor).
label-microscope = Mikroskop
label-voltage = Urychlovací napětí
label-magnification = Zvětšení
label-imaging-mode = Režim
label-detector = Detektor
label-acquired = Pořízeno
label-field-of-view = Zorné pole
science-kilovolts = { $kv } kV
science-magnification = { $value }×
label-vendor = Výrobce
label-scanner = Skener
label-objective = Objektiv
label-pyramid-levels = Úrovně
# machine-drafted, needs native review
label-emissivity = Emisivita
label-object-distance = Vzdálenost objektu
label-reflected-temperature = Odražená teplota
label-atmospheric-temperature = Teplota atmosféry
label-humidity = Relativní vlhkost
label-raw-range = Surový rozsah
label-temperature-range = Rozsah teplot
thermal-temperature = { $value } { $unit }
thermal-metres = { $value } m
thermal-percent = { PERCENT($value) }
thermal-range = { $low } – { $high }
slide-levels = { $count }: { $sizes }
# X-ray diffraction: the experiment geometry from a detector frame's header.
label-wavelength = Vlnová délka
# Ångström, e.g. "0.97625 Å".
xtal-angstrom = { $value } Å
label-photon-energy = Energie fotonů
xtal-kev = { $value } keV
# Sample to detector distance.
label-detector-distance = Vzdálenost detektoru
xtal-mm = { $value } mm
# Where the direct beam meets the detector, in pixels from the image's top-left corner.
label-beam-centre = Střed svazku
xtal-beam-centre = x { $x }, y { $y } px
# The detector arm's swing angle (2θ).
label-detector-swing = 2θ detektoru
xtal-degrees = { $value }°
# Goniometer angles (Greek letters are the field's usual names).
label-phi = Fí (φ)
label-omega = Omega (ω)
label-chi = Chí (χ)
label-kappa = Kappa (κ)
# The sample's rotation during the exposure: start angle plus width, e.g. "12.5° + 0.1°".
label-oscillation = Oscilace
xtal-oscillation = { $start }° + { $width }°
# $axis is the axis name from the file, e.g. "phi" or "omega".
xtal-oscillation-axis = { $start }° + { $width }° kolem { $axis }
# The count at which a pixel saturates.
label-overload = Mez přetečení
# The finest lattice spacing (d) recorded at the detector's edge and in its corners.
label-resolution = Rozlišení
xtal-resolution = { $edge } Å na okraji, { $corner } Å v rozích
label-geometry = Geometrie
xtal-curved = Zakřivená paměťová fólie: bez q, d a kruhů rozlišení
xtal-incomplete = Neúplná hlavička: bez q, d a kruhů rozlišení
# All-fields group title: "FITS header".
fields-science-header = Hlavička { $format }
# machine-drafted, needs native review
fields-asdf-tree = Strom ASDF
label-asdf-model = Datový model
label-asdf-version = Verze souboru
label-asdf-standard = Standard ASDF
label-asdf-library = Zapsáno pomocí
# machine-drafted, needs native review
# All-fields group title for a CERN ROOT file's list of keys (objects).
fields-root-keys = Klíče ROOT
# CERN ROOT files: the ROOT version that wrote the file (e.g. "6.28/04"), its compression,
# the number of keys (stored objects), and the object shown: class (e.g. "TH2F"), its path in
# the file, title, number of entries and sum of weights.
label-root-version = Verze ROOT
label-root-compression = Komprese
label-root-keys = Klíče
label-root-class = Třída
label-root-name = Název
label-root-title = Titulek
label-root-entries = Záznamy
label-root-sum-weights = Součet vah
# A graph's number of points.
label-root-points = Body
# $axis is "x", "y" or "z".
label-root-axis = Osa { $axis }
# $quantity is the axis title with its unit, e.g. "Energy [GeV]"; $min and $max the axis
# limits.
root-axis-fixed = { $quantity }: { $bins ->
        [one] { $bins } bin
        [few] { $bins } biny
        [many] { $bins } binu
       *[other] { $bins } binů
    }, od { $min } do { $max }
root-axis-variable = { $quantity }: { $bins ->
        [one] { $bins } bin proměnné šířky
        [few] { $bins } biny proměnné šířky
        [many] { $bins } binu proměnné šířky
       *[other] { $bins } binů proměnné šířky
    }, od { $min } do { $max }
label-root-mean-rms = Průměr a RMS { $axis }
root-mean-rms = { $mean } ± { $rms }
# Trees (event data) are listed only, with their entry counts.
label-root-trees = Stromy
root-tree = { $name } ({ $entries ->
        [one] { $entries } záznam
        [few] { $entries } záznamy
        [many] { $entries } záznamu
       *[other] { $entries } záznamů
    })
# machine-drafted, needs native review
# CASA images: what the image is (from its directory name), its restoring beam (major and
# minor axis, position angle), the beams of its planes when they differ, the rest frequency,
# spectral reference frame (e.g. "LSRK") and velocity definition of the frequency labels,
# the default mask (e.g. "mask0") and the casacore storage manager with its byte order.
label-casa-type = Typ obrazu
casa-type-image = Restaurovaný obraz
casa-type-residual = Reziduum
casa-type-model = Model
casa-type-pb = Primární svazek
casa-type-mask = Čisticí maska
casa-type-psf = Funkce rozptylu bodu
casa-type-sumwt = Součet vah
casa-type-weight = Váhy
label-casa-beam = Restaurační svazek
casa-beam = { $major }″ × { $minor }″, PÚ { $pa }°
label-casa-plane-beams = Svazky jednotlivých rovin
casa-plane-beams = { $count ->
        [one] { $count } svazek
        [few] { $count } svazky
        [many] { $count } svazku
       *[other] { $count } svazků
    }, hlavní osa { $min }″ až { $max }″
label-casa-rest-frequency = Klidová frekvence
casa-ghz = { $ghz } GHz
label-casa-spectral-frame = Spektrální vztažná soustava
label-casa-velocity = Definice rychlosti
casa-velocity-radio = Radiová
casa-velocity-optical = Optická
casa-velocity-relativistic = Relativistická
label-casa-mask = Výchozí maska
label-casa-storage = Úložiště
casa-storage-big = { $manager }, big-endian
casa-storage-little = { $manager }, little-endian
# machine-drafted, needs native review
# Unstructured meshes: the mesh's name in the file, the convention describing it (UGRID,
# ICON, MPAS), its counts, face shapes, coordinates and where the values sit.
label-ugrid-name = Síť
label-ugrid-convention = Konvence
label-ugrid-nodes = Uzly
label-ugrid-faces = Stěny
label-ugrid-edges = Hrany
label-ugrid-shapes = Tvary stěn
label-ugrid-coordinates = Souřadnice
label-ugrid-values = Hodnoty na
label-ugrid-split = Rozděleno pro vykreslení
# $n is the formatted count, $count the number (for the plural).
ugrid-triangles = { $count ->
    [one] { $count } trojúhelník
    [few] { $count } trojúhelníky
    [many] { $count } trojúhelníku
   *[other] { $count } trojúhelníků
}
ugrid-quads = { $count ->
    [one] { $count } čtyřúhelník
    [few] { $count } čtyřúhelníky
    [many] { $count } čtyřúhelníku
   *[other] { $count } čtyřúhelníků
}
# Faces of five or more nodes; $max is the most nodes of one face.
ugrid-polygons = { $count ->
    [one] { $count } mnohoúhelník o { $max } uzlech
    [few] { $count } mnohoúhelníky o nejvýše { $max } uzlech
    [many] { $count } mnohoúhelníku o nejvýše { $max } uzlech
   *[other] { $count } mnohoúhelníků o nejvýše { $max } uzlech
}
ugrid-lonlat = Zeměpisná délka a šířka
ugrid-projected = Projektované x a y
ugrid-projected-unit = Projektované x a y ({ $unit })
ugrid-on-faces = Stěny (vykresleno jednobarevně)
ugrid-on-nodes = Uzly (interpolováno přes stěny)
# A mesh file without data variables: the image shows how many nodes each face has.
ugrid-grid-only = Žádné: obraz ukazuje počet uzlů na stěnu
# Faces cut at the antimeridian or closed to a pole so they draw on a flat map.
ugrid-split = { $count ->
    [one] { $count } stěna na antimeridiánu nebo pólu
    [few] { $count } stěny na antimeridiánu nebo pólu
    [many] { $count } stěny na antimeridiánu nebo pólu
   *[other] { $count } stěn na antimeridiánu nebo pólu
}
# machine-drafted, needs native review
# Slide annotations (NDP.view, Aperio ImageScope, ASAP sidecar files).
label-annotation-file = Soubor
# "slide.xml (Aperio ImageScope)"
annotation-source = { $file } ({ $format })
label-annotation-shapes = Tvary
label-annotation-groups = Skupiny
# Shapes or points over the reader's limits, left out.
label-annotation-skipped = Vynecháno
annotation-polygon = Mnohoúhelník
annotation-polyline = Čára
annotation-rectangle = Obdélník
annotation-ellipse = Elipsa
annotation-point = Bod
annotation-points = Body
annotation-ruler = Pravítko
annotation-arrow = Šipka
# The list of annotations is cut short: "and 1,200 more".
annotation-more = a další ({ $count })
# All-fields group title: "Annotations in slide.xml".
fields-annotations-header = Anotace v { $file }
label-drawing-version = Verze
label-drawing-units = Jednotky
label-drawing-extents = Rozsah
label-drawing-sheet = List
label-drawing-layouts = Rozvržení
label-drawing-layers = Hladiny
label-drawing-entities = Entity
label-drawing-codepage = Kódová stránka
label-drawing-missing-fonts = Nahrazená písma
# machine-drafted, needs native review
# A font file the drawing names that was not found: the styles naming it, and the font drawing its text instead.
drawing-missing-font = { $file } (styly { $styles }), vykresleno písmem { $substitute }
# A Big Font (double-byte text) that was not found; its characters come from other fonts.
drawing-missing-bigfont = { $file } (styly { $styles }), vykresleno náhradními písmy
# Survey data (LandXML).
label-survey-crs = Souřadnicový systém
survey-crs-none = Neuvedeno
label-survey-project = Název projektu
label-survey-application = Zapsáno pomocí
label-survey-surface = Model terénu
# A TIN surface: its name, counts, plan and sloped areas, the volume above its lowest point, its elevations and the contour interval drawn; unit is "m" or "ft" (areas in square units, volume in cubic units).
survey-surface = { $name }: bodů { $points }, trojúhelníků { $triangles }; plocha v půdorysu { $area } { $unit }², skutečná plocha { $area3d } { $unit }²; { $volume } { $unit }³ nad nejnižším bodem; výšky { $zmin }–{ $zmax } { $unit }; vrstevnice po { $interval } { $unit }
label-survey-alignment = Osa trasy
# An alignment: its name, length and first and last stations ("1+234.50").
survey-alignment = { $name }: { $length } { $unit }, staničení { $start } až { $end }
# SHX shape fonts found and used for the drawing's text.
label-drawing-shx-fonts = Písma SHX
label-drawing-warnings = Problémy
label-plot-paper = Papír
# "2 of 5": the page shown and how many the plot has.
plot-page-of = { $page } z { $pages }
label-plot-page = Stránka
label-plot-pens = Použitá pera
# The printer language a plot came wrapped in, e.g. "PCL 5".
label-plot-job = Tisková úloha
# What a plot draws: strokes, fills and labels.
label-plot-items = Kreslené prvky
# Width × height of what is drawn, in drawing units: "1,250 × 800 mm".
drawing-extents = { $width } × { $height } { $unit }
drawing-extents-unitless = { $width } × { $height }
drawing-model-space = Model
# "24, 3 off or frozen"
drawing-layers-off = { $num }, z toho { $off } vypnuto nebo zmrazeno
# DXF $INSUNITS.
drawing-units = { $code ->
    [1] Palce
    [2] Stopy
    [3] Míle
    [4] Milimetry
    [5] Centimetry
    [6] Metry
    [7] Kilometry
    [8] Mikropalce
    [9] Tisíciny palce
    [10] Yardy
    [11] Ångströmy
    [12] Nanometry
    [13] Mikrometry
    [14] Decimetry
    [15] Dekametry
    [16] Hektometry
    [17] Gigametry
    [18] Astronomické jednotky
    [19] Světelné roky
    [20] Parseky
    [21] Americké měřické stopy
   *[other] Bez jednotek
}
# All-fields group title: "DXF entities".
fields-drawing-entities = Entity { $format }

## Layouty integrovaných obvodů (GDSII)

# machine-drafted, needs native review
label-layout-library = Knihovna
label-layout-units = Jednotky
# The database unit (the grid coordinates are stored in) and the user unit, with SI symbols:
# "1 nm database unit, 1 µm user unit".
layout-units = { $db } databázová jednotka, { $user } uživatelská jednotka
# The top cell shown (each top cell is a page).
label-layout-cell = Buňka
label-layout-cells = Buňky
# Cells no other cell places.
label-layout-top-cells = Buňky nejvyšší úrovně
# "TOP, TEST, PAD and 12 more"
layout-top-cells-more = { $names } a dalších { $more }
# Layer/datatype pairs.
label-layout-layers = Vrstvy
# Polygons, paths and boxes, as stored (each cell once).
label-layout-shapes = Tvary
label-layout-texts = Popisky
# Cell references and the placements they make: "1,204 (88,320 placements)".
label-layout-references = Reference
layout-references = { $refs } ({ $placements } { $placements ->
    [one] umístění
    [few] umístění
    [many] umístění
   *[other] umístění
})
# Shapes with every cell placement expanded.
label-layout-flat-shapes = Tvary po zploštění
label-layout-bounds = Hranice
# Corners of what the cell draws, micrometres: "-12.5, 0 to 2,900, 3,500 µm".
layout-bounds = { $x0 }, { $y0 } až { $x1 }, { $y1 } µm

## Desky plošných spojů a schémata (Gerber, Excellon, KiCad)

# machine-drafted, needs native review
group-board = Deska plošných spojů
group-schematic = Schéma
# The board view shown (its layers, its top or bottom), or the schematic sheet.
label-pcb-view = Pohled
label-pcb-project = Projekt
label-pcb-revision = Revize
label-pcb-date = Datum
# The CAD program that wrote the file.
label-pcb-generator = Vytvořeno v
# Width × height of the board outline: "100 × 80 mm".
label-pcb-size = Rozměr desky
label-pcb-thickness = Tloušťka
label-pcb-copper-layers = Měděné vrstvy
label-pcb-sheets = Listy
# The board's layers or files, each with its role: "board-F_Cu.gbr (Top copper)".
label-pcb-layers = Vrstvy
pcb-layer-role = { $name } ({ $role })
label-pcb-components = Pouzdra
label-pcb-symbols = Značky
label-pcb-nets = Sítě
label-pcb-tracks = Spoje
label-pcb-wires = Vodiče
label-pcb-vias = Prokovy
# Drilled holes and how many drill sizes: "1,204 in 9 sizes".
label-pcb-holes = Otvory
pcb-holes = { $holes } v { $sizes } { $sizes ->
    [one] velikosti
    [few] velikostech
    [many] velikosti
   *[other] velikostech
}
label-pcb-apertures = Clony
# What a board layer or file is, shown after its name.
pcb-role-inner-copper = Vnitřní měď { $n }
pcb-role-top-copper = Horní měď
pcb-role-bottom-copper = Spodní měď
pcb-role-top-solder-mask = Horní nepájivá maska
pcb-role-bottom-solder-mask = Spodní nepájivá maska
pcb-role-top-silkscreen = Horní potisk
pcb-role-bottom-silkscreen = Spodní potisk
pcb-role-top-paste = Horní pasta
pcb-role-bottom-paste = Spodní pasta
pcb-role-top-solder-paste = Horní pájecí pasta
pcb-role-bottom-solder-paste = Spodní pájecí pasta
pcb-role-top-adhesive = Horní lepidlo
pcb-role-bottom-adhesive = Spodní lepidlo
pcb-role-top-courtyard = Horní zástavbová plocha
pcb-role-bottom-courtyard = Spodní zástavbová plocha
pcb-role-top-fabrication = Horní výrobní vrstva
pcb-role-bottom-fabrication = Spodní výrobní vrstva
pcb-role-board-outline = Obrys desky
pcb-role-plated-drill = Prokovené vrtání
pcb-role-non-plated-drill = Neprokovené vrtání
pcb-role-drill = Vrtání
pcb-role-drill-holes = Vrtané otvory
pcb-role-mechanical = Mechanická
pcb-role-margin = Okraj
pcb-role-drawings = Výkresy
pcb-role-comments = Komentáře
pcb-role-engineering-change-1 = Technická změna 1
pcb-role-engineering-change-2 = Technická změna 2
pcb-role-vias = Prokovy
pcb-role-other = Jiná
pcb-role-unknown = Neznámá
label-pcb-macros = Makra clon

## Spektra (JCAMP-DX, SPC, mzML, mzXML, Bruker NMR)

# machine-drafted, needs native review
group-spectrum = Spektrum
label-spectrum-format = Formát
label-spectrum-technique = Technika
label-spectrum-title = Název
# The page shown: "3 of 1,204: Scan 1532".
label-spectrum-page = Strana
spectrum-page-of = { $page } z { $pages }: { $name }
# An axis with its unit: "Wavenumber (cm⁻¹)".
spectrum-axis = { $title } ({ $unit })
label-spectrum-x = Osa X
label-spectrum-y = Osa Y
label-spectrum-nucleus = Jádro
label-spectrum-frequency = Frekvence spektrometru
label-spectrum-solvent = Rozpouštědlo
label-spectrum-pulse-program = Pulzní program
label-spectrum-spectra = Spektra
label-spectrum-points = Body
label-spectrum-instrument = Přístroj
# Who or what lab measured it.
label-spectrum-origin = Původ
label-spectrum-date = Změřeno
# How Specular made the page from the raw data (an FID transformed, phased automatically).
label-spectrum-processing = Zpracování
spectrum-technique-ir = Infračervená
spectrum-technique-raman = Ramanova
spectrum-technique-uv-vis = UV-Vis
spectrum-technique-fluorescence = Fluorescence
spectrum-technique-nmr = NMR
spectrum-technique-ms = Hmotnostní spektrum
spectrum-technique-chromatogram = Chromatogram
spectrum-technique-other = Spektrum

## 3D modely (STL, OBJ, PLY, glTF, 3MF)

# machine-drafted, needs native review
group-mesh = 3D model
label-triangles = Trojúhelníky
label-vertices = Vrcholy
label-materials = Materiály
label-textures = Textury
label-up-axis = Svislá osa
# Textures the model uses, some of which could not be read: "3 (1 missing)".
mesh-textures-missing = { $count } (chybí: { $missing })

## Molekulové struktury (PDB, mmCIF, CIF, SDF, XYZ)

# machine-drafted, needs native review
group-structure = Struktura
# The entry's identifier: a PDB ID such as 1CRN, or the CIF data block's name.
label-structure-id = Záznam
label-structure-classification = Klasifikace
# How the structure was determined: X-ray diffraction, electron microscopy, solution NMR…
label-structure-method = Metoda
# The crystallographic or cryo-EM resolution, in ångströms.
label-structure-resolution = Rozlišení
structure-resolution = { $value } Å
label-structure-deposited = Uloženo
label-structure-formula = Vzorec
label-structure-chains = Řetězce
# Residues (amino acids, nucleotides) in the polymer chains.
label-structure-residues = Rezidua
label-structure-atoms = Atomy
# Atoms after the unit cell was filled by symmetry, and those the file lists: "96 (12 in the asymmetric unit)".
structure-atoms-cell = { $count } ({ $asymmetric } v asymetrické jednotce)
label-structure-bonds = Vazby
# Models in the file (an NMR ensemble), of which the first is shown.
label-structure-models = Modely
# Molecules (SDF records) or data blocks in the file, of which the first is shown.
label-structure-records = Záznamy
structure-first-of = { $count } (zobrazen první)
# Small molecules bound to the polymer, by their three-letter codes.
label-structure-ligands = Ligandy
# A list cut short: "HEM, NAG, SO4 and 9 more".
structure-ligands-more = { $list } a { $more } dalších
label-structure-cell = Elementární buňka
# Unit cell edge lengths (Å) and angles (degrees).
structure-cell = a = { $a } Å, b = { $b } Å, c = { $c } Å, α = { $alpha }°, β = { $beta }°, γ = { $gamma }°
label-structure-space-group = Prostorová grupa
label-structure-secondary = Sekundární struktura
structure-ss-file = Ze souboru
structure-ss-computed = Vypočteno z hlavního řetězce

## HDF4 a HDF-EOS2

# machine-drafted, needs native review
group-hdf4 = HDF4
# The HDF library version that wrote the file, e.g. "NCSA HDF Version 4.2 Release 15".
label-hdf4-library = Knihovna
# Counts of the file's scientific datasets, raster images, groups (Vgroups) and tables (Vdatas).
label-hdf4-datasets = Datové sady
label-hdf4-images = Rastrové obrázky
label-hdf4-vgroups = Skupiny
label-hdf4-vdatas = Tabulky
# HDF-EOS: NASA's Earth Observing System conventions on top of HDF4 (MODIS, ASTER, AIRS…).
group-hdf-eos = HDF-EOS
label-eos-version = Verze
# The grids, swaths and point structures the file defines.
label-eos-structures = Struktury
eos-grids = { $count ->
    [one] { $count } mřížka ({ $names })
    [few] { $count } mřížky ({ $names })
    [many] { $count } mřížky ({ $names })
   *[other] { $count } mřížek ({ $names })
}
eos-swaths = { $count ->
    [one] { $count } snímaný pás ({ $names })
    [few] { $count } snímané pásy ({ $names })
    [many] { $count } snímaného pásu ({ $names })
   *[other] { $count } snímaných pásů ({ $names })
}
eos-points = { $count ->
    [one] { $count } bodová struktura
    [few] { $count } bodové struktury
    [many] { $count } bodové struktury
   *[other] { $count } bodových struktur
}
# The grid or swath the dataset shown belongs to.
label-eos-structure = Struktura
# The grid's map projection as its GCTP name, e.g. GCTP_SNSOID (sinusoidal).
label-eos-projection = Kartografické zobrazení
label-eos-grid-size = Velikost mřížky
# Columns × rows.
eos-grid-size = { $width } × { $height }
# The grid's upper-left and lower-right corners.
label-eos-corners = Rohy
# The product's short name from the inventory metadata, e.g. MOD09A1.
label-eos-product = Produkt
# When the data was acquired.
label-eos-time = Čas
eos-time = od { $begin } do { $end }

## Planetární snímky (VICAR, PDS3, PDS4: data misí NASA a ESA)

# machine-drafted, needs native review
group-planetary = Planetární snímek
# The body observed, e.g. MARS, Ganymede.
label-planetary-target = Cíl
label-planetary-mission = Mise
# The spacecraft, rover or lander carrying the instrument.
label-planetary-spacecraft = Sonda
label-planetary-start = Čas začátku
label-planetary-stop = Čas konce
# The product's identifier in the archive.
label-planetary-product = ID produktu
# The map projection of a map-projected image, as the label names it.
label-planetary-projection = Kartografické zobrazení
label-planetary-resolution = Rozlišení
planetary-metres-per-pixel = { $metres } m na pixel
planetary-pixels-per-degree = { $pixels } px na stupeň
# Latitude and longitude of the map projection's centre.
label-planetary-centre = Střed zobrazení
planetary-lat-lon = { $lat }°, { $lon }°
# The latitudes and longitudes the map covers.
label-planetary-coverage = Pokrytí
planetary-coverage = šířka { $south }° až { $north }°, délka { $west }° až { $east }°
# The body's equatorial radius the map uses.
label-planetary-radius = Poloměr tělesa
planetary-km = { $km } km

## OMERO (obrázky otevřené ze serveru OMERO nebo z Image Data Resource)

# machine-drafted, needs native review
group-omero = OMERO
label-omero-server = Server
# $server the server's address; $user the user name signed in with.
omero-server-user = { $server } (jako { $user })
omero-server-public = { $server } (veřejný)
label-omero-image = Obrázek
# $name the image's name; $id its number on the server.
omero-image = { $name } (ID { $id })
# The project and dataset the image is in, as "Project › Dataset".
label-omero-project = Projekt
# The screen, plate and well, as "Screen › Plate › B4".
label-omero-screen = Screen
label-omero-owner = Vlastník
# $owner the image's owner; $group the OMERO group it belongs to.
omero-owner-group = { $owner } ({ $group })
label-omero-acquired = Pořízeno
label-omero-dimensions = Rozměry
omero-size = { $width } × { $height }
omero-z = { $count ->
    [one] { $count } rovina Z
    [few] { $count } roviny Z
    [many] { $count } roviny Z
   *[other] { $count } rovin Z
}
omero-c = { $count ->
    [one] { $count } kanál
    [few] { $count } kanály
    [many] { $count } kanálu
   *[other] { $count } kanálů
}
omero-t = { $count ->
    [one] { $count } časový bod
    [few] { $count } časové body
    [many] { $count } časového bodu
   *[other] { $count } časových bodů
}
# The physical size of a pixel along X, Y (and Z).
label-omero-physical = Velikost pixelu
label-omero-channels = Kanály
# A channel not shown in the server's rendering.
omero-channel-hidden = { $channel } (skrytý)
# How the pixels are fetched from the server: rendered tiles (JPEG) or the whole plane (PNG).
label-omero-fetched = Načítáno jako
omero-fetched-tiles = dlaždice { $encoding } { $width } × { $height }, { $levels ->
    [one] { $levels } úroveň
    [few] { $levels } úrovně
    [many] { $levels } úrovně
   *[other] { $levels } úrovní
}
omero-fetched-whole = Celé roviny jako { $encoding }

## Snímky reflektanční transformace (PTM, RTI, Relight)

# machine-drafted, needs native review
group-rti = Reflektanční transformace
# The reflectance model the coefficients belong to: PTM (polynomial), HSH (hemispherical harmonics), RBF…
label-rti-basis = Báze
label-rti-order = Řád
# Coefficients per colour channel.
label-rti-terms = Koeficienty
# How colour is stored: LRGB (one colour, varying luminance), RGB, YCC…
label-rti-colour = Uspořádání barev
# Coefficient images stored in the file.
label-rti-planes = Roviny
# The byte-to-coefficient mapping the file states, per coefficient.
label-rti-scale = Měřítko
label-rti-bias = Posun

## Mračna bodů

# machine-drafted, needs native review
group-points = Mračno bodů
label-points = Body
# Points read of those in the file, when a very large cloud was evenly thinned: "32,000,000 of 120,000,000".
points-subsampled = { $loaded } z { $total }
# The LAS point data record format number (0–10).
label-point-format = Formát bodu
# The coordinate reference system.
label-crs = Souřadnicový systém
# What each point carries besides its position.
label-attributes = Atributy
points-channel-rgb = RGB
points-channel-intensity = Intenzita
points-channel-classification = Klasifikace
points-channel-returns = Odrazy
# ASPRS classes present, with point counts: "Ground (1,234), Building (567)".
label-classes = Třídy
# E57 scans, with point counts.
label-scans = Skeny
point-class-0 = Nikdy neklasifikováno
point-class-1 = Neklasifikováno
point-class-2 = Terén
point-class-3 = Nízká vegetace
point-class-4 = Střední vegetace
point-class-5 = Vysoká vegetace
point-class-6 = Budova
point-class-7 = Nízký bod (šum)
point-class-8 = Klíčový bod modelu
point-class-9 = Voda
point-class-10 = Železnice
point-class-11 = Povrch vozovky
point-class-12 = Překryv
point-class-13 = Zemnicí lano
point-class-14 = Fázový vodič
point-class-15 = Stožár vedení
point-class-16 = Spojka vodičů
point-class-17 = Mostovka
point-class-18 = Vysoký šum
point-class-19 = Nadzemní konstrukce
point-class-20 = Ignorovaný terén
point-class-21 = Sníh
point-class-22 = Časové vyloučení
# A class without a standard name, by number.
point-class-other = Třída { $class }

## Gaussian splats

# machine-drafted, needs native review
group-splats = Gaussovské splaty
label-splats = Splaty
# Splats shown of those in the file, when a very large file was evenly thinned: "12,000,000 of 30,000,000".
splats-shown-of = { $shown } z { $total }
# The degree (1 to 3) of the spherical harmonics that give each splat its view-dependent colour.
label-splat-harmonics = Barva závislá na pohledu
splats-harmonics-none = Žádná
# $format: the file format, e.g. "SPZ".
splats-format-antialiased = { $format }, trénováno s vyhlazováním hran

## Varování

# machine-drafted, needs native review
warn-not-diagnostic = Není určeno k diagnostice. Specular je obecný prohlížeč obrázků, nikoli zdravotnický prostředek: jeho zobrazení není kalibrováno pro diagnostiku.

warn-gps = Tento soubor uvádí, kde byl pořízen. Má-li poloha zůstat soukromá, před sdílením ji odstraňte.
warn-null-island = Poloha GPS je přesně 0°, 0°; fotoaparát pravděpodobně neměl signál, takže poloha je nejspíš chybná.
warn-date-future = Datum pořízení je v budoucnosti; hodiny fotoaparátu byly pravděpodobně špatně nastaveny.
warn-icc-damaged = Vložený barevný profil je poškozený a nelze ho přečíst.
warn-metadata-failed = Metadata nelze přečíst kvůli chybě v aplikaci Specular. Samotný obrázek je v pořádku.
warn-orientation-conflict = EXIF a XMP se neshodují v orientaci (EXIF: { $exif }, XMP: { $xmp }). Použije se hodnota EXIF.
warn-exif-damaged = Některá data EXIF nelze přečíst ({ $error }); zobrazuje se zbytek.
warn-xmp-damaged = Metadata XMP jsou chybně utvořená ({ $error }); podařilo se přečíst jen jejich část.

## Okno O aplikaci

geonames-attribution = Názvy míst: GeoNames (www.geonames.org), licencováno pod Creative Commons Attribution 4.0 (CC BY 4.0, https://creativecommons.org/licenses/by/4.0/). Offline vyhledávání pomocí knihovny reverse_geocoder od Granta Minera (MIT nebo Apache-2.0). Data byla převedena na seznam měst; poskytují se tak, jak jsou, bez záruky.

## TIFF and DNG structure values (All fields)
# machine-drafted, needs native review

subfile-type = { $value ->
    [0] Obrázek v plném rozlišení
    [1] Obrázek ve sníženém rozlišení
    [2] Jedna stránka vícestránkového obrázku
    [4] Maska průhlednosti
   *[other] Jiné
}
predictor = { $value ->
    [1] Žádný
    [2] Vodorovné rozdíly
    [3] Rozdíly v plovoucí čárce
   *[other] Jiné
}
extra-samples = { $value ->
    [0] Neuvedeno
    [1] Předem vynásobený alfa kanál
    [2] Přímý alfa kanál
   *[other] Jiné
}
sample-format = { $value ->
    [1] Celé číslo bez znaménka
    [2] Celé číslo se znaménkem
    [3] Plovoucí čárka
    [4] Nedefinováno
   *[other] Jiné
}

## Geoprostorová data (GeoTIFF, GeoJSON, Shapefile, GeoPackage, KML/KMZ, GPX)

# machine-drafted, needs native review
group-geospatial = Geoprostorová data
label-geo-crs = Souřadnicový systém
label-geo-datum = Datum
label-geo-projection = Kartografické zobrazení
label-geo-unit = Jednotky
label-geo-bounds = Rozsah
label-geo-lonlat-bounds = Zeměpisná délka a šířka
label-geo-pixel-size = Velikost pixelu
label-geo-overviews = Přehledy
label-geo-nodata = Hodnota NoData
label-geo-raster-type = Typ rastru
label-geo-vertical = Výškový systém
label-geo-citation = Citace
label-geo-features = Prvky
label-geo-vertices = Vrcholy
label-geo-layer = Vrstva
label-geo-layers = Vrstvy
label-geo-tiles = Dlaždice
label-geo-encoding = Kódování textu
label-geo-warnings = Problémy
# The companion files a georeference was read from ("scene.jgw, scene.prj").
label-geo-sidecars = Doprovodné soubory
# machine-drafted, needs native review
# A projection Specular cannot convert to longitude and latitude: $method is its name.
geo-projection-unsupported = { $method } (souřadnice nejsou podporovány)
# How longitude and latitude are brought to WGS 84 (what GPS and web maps use).
label-geo-to-wgs84 = Na WGS 84
geo-to-wgs84-same = Není potřeba
geo-to-wgs84-helmert = Převod data (Helmert)
geo-to-wgs84-by-area = Převod data (Helmert, podle oblasti)
geo-to-wgs84-unknown = Neznámý: souřadnice jsou ve vlastním datu souboru
# A coordinate system's name and its EPSG code: "WGS 84 / UTM zone 33N (EPSG:32633)".
geo-crs-epsg = { $name } (EPSG:{ $code })
geo-crs-unknown = Neuvedeno
# Map bounds: the lower-left and upper-right corners.
geo-bounds = { $x0 }, { $y0 } až { $x1 }, { $y1 }
# Bounds in degrees: west to east longitude, south to north latitude.
geo-lonlat-bounds = zeměpisná délka { $west }° až { $east }°, šířka { $south }° až { $north }°
geo-pixel-size = { $x } × { $y }
geo-overviews = { $n ->
    [one] { $n } přehled: { $sizes }
    [few] { $n } přehledy: { $sizes }
    [many] { $n } přehledu: { $sizes }
   *[other] { $n } přehledů: { $sizes }
}
geo-pixel-is-point = Pixely jsou body
geo-layer = { $features ->
    [one] { $features } prvek
    [few] { $features } prvky
    [many] { $features } prvku
   *[other] { $features } prvků
}, { $fields ->
    [one] { $fields } pole
    [few] { $fields } pole
    [many] { $fields } pole
   *[other] { $fields } polí
}
# A GeoPackage tile pyramid: its table, the zoom level shown and its tile count.
geo-tiles = { $table }, úroveň přiblížení { $zoom }, { $tiles ->
    [one] { $tiles } dlaždice
    [few] { $tiles } dlaždice
    [many] { $tiles } dlaždice
   *[other] { $tiles } dlaždic
}
fields-geo-geokeys = Klíče GeoTIFF
fields-geo-gdal = Metadata GDAL
fields-geo-wkt = Souřadnicový systém (WKT)
fields-geo-projjson = Souřadnicový systém (PROJJSON)
# machine-drafted, needs native review
# What a web map tile set (MBTiles, PMTiles) says about itself: container, tile type, zoom
# levels, description and other metadata.
fields-geo-tileset = Sada dlaždic
# The credit line a map's data asks to be shown with it ("© OpenStreetMap contributors").
label-geo-attribution = Zdroj dat
# The header lines of a georeferenced raster format (ENVI, ASCII grid, DTED, ERDAS IMAGINE).
fields-geo-header = Hlavička
# machine-drafted, needs native review
# GIFTI files: the format version, what the file holds, the number of data arrays and the
# encoding of their values (e.g. "GZipBase64Binary").
label-gifti-version = Verze GIFTI
label-gifti-role = Obsahuje
gifti-role-surface = Povrch
gifti-role-labels = Popisky pro vrcholy
gifti-role-data = Hodnoty pro vrcholy
label-gifti-arrays = Datová pole
label-gifti-encoding = Kódování
label-gifti-vertices = Vrcholy
label-gifti-triangles = Trojúhelníky
# The brain structure, e.g. "CortexLeft".
label-gifti-structure = Struktura
# A surface's geometric type, e.g. "Anatomical", "Inflated".
label-gifti-geometry = Geometrický typ
# The coordinate transform of the vertices: $from and $to are coordinate spaces such as
# "scanner", "Talairach" or "MNI 152".
label-gifti-transform = Souřadnice
gifti-transform = { $from } (transformace do { $to } není potřeba)
gifti-transform-applied = { $from } → { $to } (použita)
# The surface file a data file is drawn on, found beside it.
label-gifti-surface = Zobrazeno na
# The data array shown: its name, NIfTI intent (e.g. "NIFTI_INTENT_SHAPE") and data type.
label-gifti-array = Pole
label-gifti-intent = Intent
label-gifti-data-type = Datový typ
# Entries of the label table (names and colours of regions).
label-gifti-labels = Tabulka popisků
# CIFTI-2 files: the version, NIfTI intent (e.g. "ConnDenseScalar") and matrix size.
label-cifti-version = Verze CIFTI
label-cifti-intent = Intent
label-cifti-matrix = Matice
cifti-matrix = { $rows } × { $columns }
# The maps (pages) of the file and what they are.
label-cifti-maps = Mapy
cifti-maps-scalars = Skalární mapy: { $count }
cifti-maps-labels = Mapy popisků: { $count }
cifti-maps-series = Časové body: { $count }
cifti-maps-parcels = Parcely: { $count }
cifti-maps-other = { $count }
# machine-drafted, needs native review
# A dense connectivity file (.dconn.nii): every brain place's connectivity to every other.
# $seeds: how many places can be picked; $size: the bytes read when one is picked (e.g. "357 KB").
label-cifti-dense = Konektivita
cifti-dense = Seedy: { $seeds }, { $size } čteno na seed
# A time series' start and step; $unit is e.g. "second".
label-cifti-series = Řada
cifti-series = od { $start }, krok { $step } ({ $unit })
label-cifti-models = Modely mozku
label-cifti-parcels = Parcely
label-cifti-structures = Struktury
label-cifti-voxels = Podkorové voxely
# A surface the file's values lie on: the brain structure (e.g. "CORTEX_LEFT"), its vertex
# count, and the surface file found beside the file for it, or that none was found.
label-cifti-surface = Povrch
cifti-surface-found = { $structure }: { $vertices ->
        [one] { $vertices } vrchol
        [few] { $vertices } vrcholy
        [many] { $vertices } vrcholu
       *[other] { $vertices } vrcholů
    }, { $file }
cifti-surface-missing = { $structure }: potřebuje povrch s { $vertices ->
        [one] { $vertices } vrcholem
        [few] { $vertices } vrcholy
        [many] { $vertices } vrcholu
       *[other] { $vertices } vrcholy
    } (vedle souboru žádný není)
# How the file is shown: on the surfaces found beside it, or as its subcortical voxels.
label-cifti-shown = Zobrazeno jako
cifti-shown-surfaces = Obarvené povrchy
cifti-shown-voxels = Objem podkorových voxelů
# machine-drafted, needs native review
# Shown with a picked seed's row: click a surface vertex, or move the crosshair in MPR.
cifti-shown-connectivity = Konektivita vybraného seedu
# Maps left out of the voxel volume, which would be too large with them.
label-cifti-left-out = Vynechané mapy
# Tractograms: format, number of streamlines (fibre tracts) and points.
label-tract-format = Formát
label-tract-streamlines = Proudnice
# A large tractogram draws only some of its streamlines.
tract-streamlines-shown = { $count } (zobrazeno { $shown })
label-tract-points = Body
label-tract-mean-length = Průměrná délka
# A length in millimetres.
tract-mm = { $value } mm
# Values per point and per streamline the file stores, by name.
label-tract-scalars = Skaláry pro body
label-tract-properties = Vlastnosti pro proudnice
# TrackVis: the voxel grid the tracts were made in, its voxel size and voxel order (e.g. "LAS").
label-tract-grid = Mřížka
label-tract-voxel-size = Velikost voxelu
label-tract-voxel-order = Pořadí voxelů
# How the points were brought into scanner (RAS) millimetres.
label-tract-space = Souřadnice
tract-space-vox-to-ras = Z matice voxel–RAS
tract-space-voxel-order = Z pořadí voxelů (bez matice voxel–RAS)
tract-space-scanner = Prostor skeneru
# MRtrix: the tracking step size.
label-tract-step = Velikost kroku

## File headers (DPX, Cineon, XWD, MIFF, TIM, WSQ, JBIG)

# machine-drafted, needs native review
# What the file's own header records, for formats without Exif or XMP.
group-header = Hlavička souboru
label-header-created = Vytvořeno
# The program or person that wrote the file.
label-header-creator = Autor
label-header-copyright = Autorská práva
label-header-project = Projekt
label-header-comment = Komentář
# The scanner, camera or other device the image came from.
label-header-device = Zařízení
# SMPTE time code of the frame: "01:00:00:12".
label-header-timecode = Časový kód
# A film frame's edge code (KeyKode).
label-header-edge-code = Okrajový kód
label-header-frame-rate = Snímková frekvence
# Where the frame sits in its sequence.
label-header-frame = Snímek
# Film stock and slate.
label-header-film = Film
# How code values relate to light: printing density, linear, BT.709…
label-header-transfer = Přenosová funkce
# The colour primaries the file states.
label-header-colorimetric = Kolorimetrie
# The X window a screen dump was taken of.
label-header-window = Okno
label-header-resolution = Rozlišení
label-header-compression = Komprese
# machine-drafted, needs native review
# JPEG XS: the profile and the level the codestream states ("Main 422.10"; "4k-1, sublevel 6 bpp").
label-header-profile = Profil
# machine-drafted, needs native review
label-header-level = Úroveň
# machine-drafted, needs native review
# JPEG XS: how many wavelet levels, horizontal and vertical ("5 horizontal, 1 vertical").
label-header-decomposition = Vlnkové úrovně
# machine-drafted, needs native review
# JPEG XS: the colour transform ("None", "Reversible colour transform (RCT)", "Star-Tetrix, RGGB mosaic").
label-header-colour-transform = Barevná transformace
# machine-drafted, needs native review
# Photoshop: the artboards, by name and size ("Phone (375 × 667), Tablet (768 × 1024)").
label-header-artboards = Kreslicí plátna
# machine-drafted, needs native review
# Photoshop file without a merged image, shown from its layers: the features it leaves out,
# by Photoshop's names ("Color Lookup, Knockout").
label-header-not-drawn = Nevykresleno
# machine-drafted, needs native review
# The same, for the features drawn only approximately ("Bevel & Emboss, Satin").
label-header-approximated = Vykresleno přibližně

## Fonts (TTF, OTF, TTC/OTC, WOFF, WOFF2)

# machine-drafted, needs native review
group-font = Písmo
label-font-family = Rodina
label-font-style = Styl
# The face's place in a font collection: "2 of 4".
label-font-face = Řez
font-face-of = { $index } z { $count }
label-font-version = Verze
label-font-designer = Návrhář
# The font's maker (the name table's manufacturer).
label-font-manufacturer = Písmolijna
label-font-glyphs = Glyfy
label-font-outlines = Obrysy
font-outlines-truetype = TrueType (kvadratické)
font-outlines-cff = PostScript (CFF, kubické)
font-outlines-cff2 = PostScript (CFF2, kubické)
font-outlines-bitmap = Jen bitmapy
font-outlines-none = Žádné
# What the font's licence flags (OS/2 fsType) allow documents that embed it to do.
label-font-embedding = Vkládání
font-embedding-installable = Smí se vložit a nainstalovat.
font-embedding-restricted = Nesmí se vkládat.
font-embedding-preview-print = Smí se vložit jen pro zobrazení a tisk.
font-embedding-editable = Smí se vložit do upravovaných dokumentů.
font-embedding-no-subsetting = Jen celé, ne jako podmnožina.
font-embedding-bitmap-only = Jen jako bitmapy.
# Variation axes of a variable font: "Weight 100–900 (400)".
label-font-axes = Osy variací
font-axis = { $name } { $min }–{ $max } ({ $default })
label-font-instances = Pojmenované styly
label-font-colour = Barevné glyfy
font-colour-colr0 = Vrstvy COLR
font-colour-colr1 = Přechody COLR v1
font-colour-sbix = Bitmapy (sbix)
font-colour-cbdt = Bitmapy (CBDT)
font-colour-svg = SVG
label-font-weight = Tloušťka
label-font-units-per-em = Jednotek na em
label-font-tables = Tabulky
label-font-licence = Licence
label-font-licence-url = Adresa URL licence
label-font-copyright = Autorská práva
label-font-trademark = Ochranná známka

## Office documents (Word, Excel, PowerPoint, iWork, OpenDocument)

# machine-drafted, needs native review
group-document = Dokument
label-document-kind = Druh
document-kind-word = Dokument Word
document-kind-excel = Sešit Excel
document-kind-powerpoint = Prezentace PowerPoint
document-kind-pages = Dokument Pages
document-kind-numbers = Tabulka Numbers
document-kind-keynote = Prezentace Keynote
document-kind-odf-text = Text OpenDocument
document-kind-odf-spreadsheet = Tabulka OpenDocument
document-kind-odf-presentation = Prezentace OpenDocument
document-kind-odf-graphics = Kresba OpenDocument
document-kind-odf-other = Soubor OpenDocument
# machine-drafted, needs native review
document-kind-playlist = Seznam skladeb
# machine-drafted, needs native review
label-playlist-items = Položky
# Appended to the kind: "Word document, template, with macros".
document-template = šablona
document-macros = s makry
document-legacy = formát Office 97–2003
label-document-preview = Náhled
# The image format of the preview stored in the file, e.g. "JPEG image".
document-preview-image = Obrázek { $format }
document-preview-none = V souboru není
label-document-subject = Předmět
label-document-author = Autor
label-document-description = Popis
label-document-category = Kategorie
label-document-last-saved-by = Naposledy uložil
label-document-created = Vytvořeno
label-document-modified = Změněno
label-document-printed = Naposledy vytištěno
label-document-pages = Stránky
label-document-slides = Snímky
label-document-hidden-slides = Skryté snímky
label-document-notes = Stránky poznámek
label-document-sheets = Listy
label-document-words = Slova
label-document-characters = Znaky
label-document-paragraphs = Odstavce
label-document-lines = Řádky
label-document-editing-time = Doba úprav
document-minutes = { $minutes } min
label-document-revision = Revize
label-document-application = Uloženo v
label-document-company = Společnost
label-document-template = Šablona

## Books (EPUB)

# machine-drafted, needs native review
group-book = Kniha
label-book-authors = Autoři
label-book-publisher = Vydavatel
label-book-date = Vydáno
label-book-language = Jazyk
label-book-pages = Stránky
# Entries in the book's table of contents.
label-book-contents = Položky obsahu
label-book-layout = Rozvržení
book-layout-reflowable = Přizpůsobivé
book-layout-fixed = Pevné rozvržení
book-right-to-left = zprava doleva
label-book-version = Formát
book-version = EPUB { $version }
label-book-fonts = Vložená písma
book-fonts-obfuscated = { $count } (zamaskováno: { $obfuscated })
label-book-subjects = Témata
label-book-identifier = Identifikátor
label-book-rights = Práva

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
volume-orient-parrec = Úhel a posun Philips
volume-orient-ecat = Velikosti voxelů a posuny ECAT (nespolehlivé)
label-volume-technique = Technika
label-volume-version = Verze formátu
label-volume-isotope = Izotop
label-volume-tracer = Radiofarmakum
label-volume-units = Jednotky

# Text, tables and subtitles.
label-header-encoding = Znaková sada
label-header-lines = Řádky
label-header-line-ends = Konce řádků
label-header-words = Slova
label-header-pages = Stránky
label-header-pictures = Obrázky
label-header-pictures-not-shown = Nezobrazené obrázky
label-header-tables = Tabulky
label-header-rows = Řádky tabulky
label-header-columns = Sloupce
label-header-delimiter = Oddělovač
label-header-column-names = Názvy sloupců
label-header-cues = Titulky
label-header-cue-span = Titulky od – do
