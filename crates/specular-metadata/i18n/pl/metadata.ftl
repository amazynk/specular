# machine-drafted, needs native review
# Specular — panel metadanych. Polski.
# Konwencje: zob. i18n/en/metadata.ftl. Liczby i daty są już sformatowane przez ICU4X;
# $n (liczba) służy tylko do wyboru formy liczby mnogiej.

## Tytuły grup podsumowania

group-camera = Aparat
group-exposure = Ekspozycja
group-lens = Obiektyw
group-date = Data i godzina
group-location = Lokalizacja
# machine-drafted, needs native review
# A DJI drone photo: the gimbal's and the aircraft's attitude, the height above take-off.
group-drone = Dron
label-gimbal = Gimbal
label-flight = Statek powietrzny
label-relative-altitude = Nad miejscem startu
drone-attitude = odchylenie { $yaw }, pochylenie { $pitch }, przechylenie { $roll }
# A DJI thermal camera's radiometric JPEG: its photo opens, its temperatures can't.
label-thermal-data = Temperatury
drone-thermal-refused = Nie wyświetlono: licencja SDK firmy DJI na to nie pozwala
drone-thermal-refused-detail = Odczyt temperatur DJI wymaga DJI Thermal SDK, którego licencja pozwala go używać tylko programom, które je dostarczają. Obraz i jego metadane są wyświetlane.
group-image = Obraz
group-file = Plik
group-rights = Prawa i opis
group-credentials = Poświadczenia treści
# machine-drafted, needs native review
group-medical = Obraz medyczny
# machine-drafted, needs native review
group-inspection = Element i badanie
group-nde-image = Obraz badania (DICONDE)
# machine-drafted, needs native review
# Industrial CT volumes (Zeiss Xradia, Amira, Bruker SkyScan, Nikon X-Tek, VGI, raw volumes).
group-industrial-image = Obraz badania
group-science = Obraz naukowy
group-em = Mikroskopia elektronowa
group-spm = Mikroskopia z sondą skanującą
group-slide = Preparat
group-microscopy = Mikroskopia
# X-ray diffraction detector frames (CBF, EDF, SMV, Bruker, MarCCD, R-AXIS).
group-diffraction = Dyfrakcja
group-surface = Topografia powierzchni
group-ebsd = Dyfrakcja elektronów wstecznie rozproszonych
group-seismic = Dane sejsmiczne
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
group-tractography = Traktografia
# machine-drafted, needs native review
group-annotations = Adnotacje
# Imaging mass spectrometry (imzML): a mass spectrum per pixel.
group-mass-spec = Obrazowanie metodą spektrometrii mas
# machine-drafted, needs native review
group-thermal = Termogram
# machine-drafted, needs native review
# Flow cytometry list-mode data (FCS): events of a cell sorter or analyser.
group-cytometry = Cytometria przepływowa
# machine-drafted, needs native review
group-spectroscopy = Spektroskopia
# machine-drafted, needs native review
# Unstructured meshes of ocean, coastal and climate models (UGRID, ICON, MPAS in NetCDF).
group-ugrid = Siatka niestrukturalna
group-drawing = Rysunek
# machine-drafted, needs native review
group-volume = Objętość

## Etykiety elementów podsumowania

label-camera = Aparat
label-software = Oprogramowanie
label-application = Aplikacja
label-shutter = Czas naświetlania
label-aperture = Przysłona
label-iso = ISO
label-focal-length = Ogniskowa
label-exposure-bias = Korekta ekspozycji
label-program = Tryb
label-metering = Pomiar światła
label-white-balance = Balans bieli
label-flash = Lampa błyskowa
label-lens = Obiektyw
label-lens-make = Producent obiektywu
label-lens-range = Zakres obiektywu
# machine-drafted, needs native review
label-focus-mode = Tryb ostrzenia
label-stabilisation = Stabilizacja obrazu
label-taken = Wykonano
label-digitised = Zdigitalizowano
label-edited = Ostatnia edycja
label-place = Miejsce
label-coordinates = Współrzędne
label-altitude = Wysokość
label-direction = Kierunek
label-accuracy = Dokładność
label-gps-time = Czas GPS
label-dimensions = Wymiary
label-encoding = Kodowanie
# machine-drafted, needs native review
label-motion = Ruch
motion-kind-motion-photo = Zdjęcie ruchome
motion-kind-micro-video = Zdjęcie ruchome (MicroVideo)
motion-kind-samsung = Zdjęcie ruchome Samsung
motion-kind-live-photo = Live Photo
motion-duration = { $seconds } s
label-colour-profile = Profil kolorów
label-orientation = Orientacja
label-file-name = Nazwa
label-folder = Folder
label-file-size = Rozmiar
label-format = Format
label-modified = Zmodyfikowano
label-title = Tytuł
label-description = Opis
label-creator = Autor
label-copyright = Prawa autorskie
label-keywords = Słowa kluczowe
label-rating = Ocena
label-colour-label = Etykieta
label-credentials = Poświadczenia treści

## Tytuły grup „Wszystkie pola” (nazwy znaczników pozostają jak w standardach)

fields-ifd0 = IFD0 (obraz główny)
fields-exif = Exif
fields-gps = GPS
fields-interop = Interoperacyjność
fields-ifd1 = IFD1 (miniatura)
fields-icc = Nagłówek profilu ICC
# $namespace: z xmp-namespace; $prefix: np. „dc”
fields-xmp = XMP: { $namespace } ({ $prefix })
xmp-namespace = { $prefix ->
    [dc] Dublin Core
    [xmp] Podstawowe
    [xmpRights] Prawa
    [xmpMM] Zarządzanie multimediami
    [xmpDM] Dynamiczne multimedia
    [tiff] TIFF
    [exif] Exif
    [exifEX] Exif 2.3+
    [aux] Akcesoria aparatu
    [photoshop] Photoshop
    [crs] Ustawienia Camera Raw
    [lr] Lightroom
    [Iptc4xmpCore] IPTC Core
    [Iptc4xmpExt] IPTC Extension
    [plus] Licencje PLUS
    [GPano] Zdjęcie sferyczne
    [GCamera] Aparat Google
    [hdrgm] Mapa wzmocnienia HDR
    [mwg-rs] Regiony
    [pdf] PDF
    [stEvt] Zdarzenia zasobu
    [stRef] Odwołania do zasobów
   *[other] Inne
}
# $number: „0xA500”; $context: „Exif”, „GPS”, …
unknown-tag = Nieznany znacznik { $number } ({ $context })
blob-bytes = { $num ->
    [one] ({ $num } bajt)
    [few] ({ $num } bajty)
    [many] ({ $num } bajtów)
   *[other] ({ $num } bajta)
}
maker-note-bytes = ({ $num } B, notatka producenta, nie zdekodowano)
more-values = { $shown }, … (wartości: { $total })

## Ogólne łączniki

# Oddziela części listy budowanej w kodzie (opis lampy, obszary miejsca).
list-separator = {", "}
# Oddziela czas · przysłonę · ISO · ogniskową w jednowierszowym podsumowaniu.
one-line-separator = {" · "}

## Ekspozycja

exposure-time-fraction = 1/{ $denominator } s
exposure-time-seconds = { $seconds } s
aperture-value = f/{ $f }
iso-value = ISO { $iso }
focal-length-value = { $mm } mm
focal-length-with-equiv = { $mm } mm (odpow. { $equiv } mm)
focal-length-equiv-only = odpow. { $equiv } mm
# $ev to wartość bezwzględna; znak wynika z selektora.
exposure-bias-value = { $sign ->
    [plus] +{ $ev } EV
    [minus] −{ $ev } EV
   *[zero] 0 EV
}
exposure-program = { $value ->
    [1] Ręczny
    [2] Program (automatyczny)
    [3] Preselekcja przysłony
    [4] Preselekcja czasu
    [5] Kreatywny (głębia ostrości)
    [6] Sport (krótki czas)
    [7] Portret
    [8] Krajobraz
   *[other] Nieznany
}
metering-mode = { $value ->
    [1] Uśredniony
    [2] Centralnie ważony
    [3] Punktowy
    [4] Wielopunktowy
    [5] Matrycowy (wielosegmentowy)
    [6] Częściowy
   *[other] Inny
}
# machine-drafted, needs native review
# Canon maker note CameraSettings FocusMode; 256–258 are the live-view variants of 0–2.
canon-focus-mode = { $value ->
    [0] One-Shot AF
    [1] AI Servo AF
    [2] AI Focus AF
    [3] Ręczne ostrzenie
    [4] Pojedynczy
    [5] Ciągły
    [6] Ręczne ostrzenie
    [16] Pan focus
    [256] One-Shot AF (podgląd na żywo)
    [257] AI Servo AF (podgląd na żywo)
    [258] AI Focus AF (podgląd na żywo)
    [512] Ostrzenie Movie Snap
    [519] Movie Servo AF
   *[other] Inny
}
# Canon maker note CameraSettings ImageStabilization; 256–260 repeat 0–4.
canon-stabilisation = { $value ->
    [0] Wyłączona
    [1] Włączona
    [2] Tylko podczas fotografowania
    [3] Panoramowanie
    [4] Dynamiczna
    [256] Wyłączona
    [257] Włączona
    [258] Tylko podczas fotografowania
    [259] Panoramowanie
    [260] Dynamiczna
   *[other] Inny
}
# machine-drafted, needs native review
panasonic-focus-mode = { $value ->
    [1] Auto
    [2] Ręczne ustawianie ostrości
    [4] Auto (przycisk ostrości)
    [5] Auto (ciągły)
    [6] AF-S
    [7] AF-C
    [8] AF-F
   *[other] Inny
}
# machine-drafted, needs native review
panasonic-stabilisation = { $value ->
    [2] Wł. (optyczna)
    [3] Wył.
    [4] Wł. (tryb 2)
    [5] Wł. (optyczna, panoramowanie)
    [6] Wł. (tylko korpus)
    [7] Wł. (tylko korpus, panoramowanie)
    [9] Dual IS
    [10] Dual IS (panoramowanie)
    [11] Dual IS 2
    [12] Dual IS 2 (panoramowanie)
   *[other] Inny
}
label-comment = Komentarz
fields-tags = Tagi { $kind }
group-picture = Ustawienia obrazu
fields-fujifilm = Dane producenta Fujifilm
label-film-simulation = Symulacja filmu
label-dynamic-range = Zakres dynamiki
label-highlight-tone = Światła
label-shadow-tone = Cienie
label-colour-setting = Kolor
label-sharpness = Ostrość
label-noise-reduction = Redukcja szumów
label-clarity = Przejrzystość
label-grain-effect = Efekt ziarna
label-colour-chrome = Efekt koloru chrom
label-colour-chrome-blue = Kolor chrom niebieski
label-shutter-type = Rodzaj migawki
label-drive-mode = Tryb wyzwalania
fuji-step = { $sign ->
    [plus] +{ $n }
    [minus] −{ $n }
   *[zero] 0
}
fuji-film-mode = { $value ->
    [0] Provia (standardowy)
    [256] Portret studyjny
    [272] Portret studyjny (większe nasycenie)
    [288] Astia (miękki)
    [304] Portret studyjny (ostrzejszy)
    [512] Velvia (żywy)
    [768] Portret studyjny Ex
    [1024] Velvia
    [1280] Pro Neg. Std
    [1281] Pro Neg. Hi
    [1536] Classic Chrome
    [1792] Eterna
    [2048] Classic Neg.
    [2304] Eterna Bleach Bypass
    [2560] Nostalgic Neg.
    [2816] Reala Ace
   *[other] Inny
}
fuji-monochrome = { $value ->
    [768] Monochromatyczny
    [769] Monochromatyczny, filtr czerwony
    [770] Monochromatyczny, filtr żółty
    [771] Monochromatyczny, filtr zielony
    [784] Sepia
    [1280] Acros
    [1281] Acros, filtr czerwony
    [1282] Acros, filtr żółty
    [1283] Acros, filtr zielony
   *[other] Monochromatyczny
}
fuji-white-balance = { $value ->
    [0] Automatyczny
    [1] Automatyczny (priorytet bieli)
    [2] Automatyczny (priorytet nastroju)
    [256] Światło dzienne
    [512] Pochmurno
    [768] Świetlówka dzienna
    [769] Świetlówka dzienna biała
    [770] Świetlówka biała
    [771] Świetlówka ciepła biała
    [772] Świetlówka ciepła biała do wnętrz
    [1024] Żarówka
    [1280] Lampa błyskowa
    [1536] Pod wodą
    [3840] Własny 1
    [3841] Własny 2
    [3842] Własny 3
    [3843] Własny 4
    [3844] Własny 5
    [4080] Temperatura barwowa
   *[other] Inny
}
fuji-white-balance-kelvin = { $kelvin } K
fuji-dynamic-range = DR{ $percent }
fuji-dynamic-range-auto = Automatyczny (DR{ $percent })
fuji-dynamic-range-setting = { $value ->
    [0] Automatyczny
    [256] Standardowy (DR100)
    [512] Szeroki 1 (DR230)
    [513] Szeroki 2 (DR400)
   *[other] Inny
}
fuji-effect = { $value ->
    [0] Wył.
    [32] Słaby
    [64] Silny
   *[other] Inny
}
fuji-grain-size = { $value ->
    [16] małe
    [32] duże
   *[other] inny
}
fuji-grain = { $strength }, { $size }
fuji-shutter-type = { $value ->
    [0] Mechaniczna
    [1] Elektroniczna
    [2] Elektroniczna (długie naświetlanie)
    [3] Elektroniczna pierwsza kurtyna
   *[other] Inny
}
fuji-drive-mode = { $value ->
    [0] Pojedyncze zdjęcie
    [1] Seryjne, wolne
    [2] Seryjne, szybkie
   *[other] Inny
}
fuji-focus-mode = { $value ->
    [0] Ręczny
    [1] AF-S
    [2] AF-C
    [100] Automatyczny
    [101] Ręczny
   *[other] Inny
}
fuji-stabilisation = { $kind } ({ $mode })
fuji-stabilisation-kind = { $value ->
    [0] Brak
    [1] Optyczna
    [2] Przesuw matrycy
    [3] Obiektyw
    [258] Przesuw matrycy i cyfrowa
    [512] Cyfrowa
   *[other] Inny
}
fuji-stabilisation-mode = { $value ->
    [0] Wył.
    [1] wł., ciągła
    [2] wł., przy zdjęciu
   *[other] Inny
}
light-source = { $value ->
    [1] Światło dzienne
    [2] Fluorescencyjne
    [3] Żarowe
    [4] Lampa błyskowa
    [9] Słonecznie
    [10] Pochmurno
    [11] Cień
    [12] Fluorescencyjne dzienne
    [13] Fluorescencyjne dzienne białe
    [14] Fluorescencyjne chłodne białe
    [15] Fluorescencyjne białe
    [16] Fluorescencyjne ciepłe białe
    [17] Światło standardowe A
    [18] Światło standardowe B
    [19] Światło standardowe C
    [20] D55
    [21] D65
    [22] D75
    [23] D50
    [24] Studyjne żarowe
   *[other] Inne
}
white-balance-auto = Automatyczny
white-balance-manual = Ręczny
white-balance-manual-with-light = Ręczny ({ $light })
# Opis lampy jest budowany z tych części, łączonych przez list-separator.
flash-none = Brak lampy błyskowej
flash-fired = Lampa błysnęła
flash-not-fired = Lampa nie błysnęła
flash-forced-on = wymuszona
flash-forced-off = wyłączona
flash-auto = tryb automatyczny
flash-return-not-detected = nie wykryto światła odbitego
flash-return-detected = wykryto światło odbite
flash-red-eye = redukcja czerwonych oczu

## Obiektyw

lens-focal-single = { $mm } mm
lens-focal-range = { $min }–{ $max } mm
lens-aperture-single = f/{ $f }
lens-aperture-range = f/{ $min }–{ $max }
lens-range = { $focal } { $aperture }

## Data i godzina

# $weekday: „wtorek” (ICU); $date: „14 maja 2024” (ICU)
date-with-weekday = { $weekday }, { $date }
date-time = { $date }, { $time }
date-time-zone = { $datetime } ({ $zone })
zone-utc = UTC
# $offset: „+1”, „+5:30”, „−3”
zone-offset = UTC{ $offset }
gps-date-time = { $date }, { $time } UTC
detail-zone-from-gps = Strefa czasowa ustalona na podstawie zegara GPS

## Lokalizacja

# $lat i $lon są bez znaku; półkula wynika z selektora.
coordinates = { $lat }° { $latRef ->
    [south] S
   *[north] N
}, { $lon }° { $lonRef ->
    [west] W
   *[east] E
}
altitude = { $ref ->
    [above] { $m } m n.p.m.
    [below] { $m } m p.p.m.
   *[sea] Na poziomie morza
}
compass-point = { $point ->
    [n] północ
    [nne] północny wschód przez północ
    [ne] północny wschód
    [ene] wschód przez północny wschód
    [e] wschód
    [ese] wschód przez południowy wschód
    [se] południowy wschód
    [sse] południowy wschód przez południe
    [s] południe
    [ssw] południowy zachód przez południe
    [sw] południowy zachód
    [wsw] zachód przez południowy zachód
    [w] zachód
    [wnw] zachód przez północny zachód
    [nw] północny zachód
   *[nnw] północny zachód przez północ
}
# $point: z compass-point; $degrees: „287”
facing = Kierunek: { $point } ({ $degrees }°)
detail-true-north = Względem północy geograficznej
detail-magnetic-north = Względem północy magnetycznej
accuracy = ±{ $m } m
distance-km = { $km } km
place = { $city }, { $country }
place-city-only = { $city }
# Ponad 30 km od dopasowanej miejscowości.
place-near = w pobliżu: { $place }
place-detail = { $areas } · { $distance } od: { $city }
place-detail-distance = { $distance } od: { $city }

## Obraz

dimensions = { $width } × { $height }
dimensions-megapixels = { $width } × { $height } ({ $mp } Mpx)
encoding-bit-depth = { $bits }-bitowe
# $format: „8-bitowe YCbCr 4:2:0”
encoding-progressive = { $format }, progresywne
encoding-baseline = { $format }, podstawowe
encoding-hierarchical = { $format }, hierarchiczne
# machine-drafted, needs native review
encoding-alpha = { $format } { $alpha ->
    [premultiplied] z alfą wstępnie przemnożoną
    [transparency] z przezroczystością
   *[alpha] z kanałem alfa
}
# [Lab], [Palette] and the variants after them: machine-drafted, needs native review
colour-model = { $model ->
    [YCbCr] YCbCr
    [RGB] RGB
    [Greyscale] Skala szarości
    [CMYK] CMYK
    [YCCK] YCCK
    [Lab] L*a*b*
    [Palette] Kolor indeksowany
    [Multichannel] Wielokanałowy
    [Duotone] Bichromia
    [YCgCo] YCgCo
    [XYB] XYB
    [YC] Luminancja/chrominancja (YC)
    [Bayer] RAW Bayer
    [X-Trans] RAW X-Trans
    [CFA] RAW z matrycą filtrów barwnych
    [LinearRaw] Liniowy RAW
    [RGBE] RGBE (wspólny wykładnik)
    [XYZE] XYZE (wspólny wykładnik)
    [R] Jeden kanał (R)
    [RG] Dwa kanały (RG)
   *[other] { $model }
}
adobe-rgb-from-exif = Adobe RGB (ze znacznika EXIF; brak osadzonego profilu)
orientation = { $value ->
    [1] Pionowo (bez obrotu)
    [2] Odbicie w poziomie (zastosowano)
    [3] Obrót o 180° (zastosowano)
    [4] Odbicie w pionie (zastosowano)
    [5] Odbicie w poziomie i obrót o 90° w lewo (zastosowano)
    [6] Obrót o 90° w prawo (zastosowano)
    [7] Odbicie w poziomie i obrót o 90° w prawo (zastosowano)
    [8] Obrót o 90° w lewo (zastosowano)
   *[other] Nieznana ({ $value })
}
# Krótka forma do ostrzeżeń.
orientation-short = { $value ->
    [1] Pionowo
    [2] Odbicie w poziomie
    [3] Obrót o 180°
    [4] Odbicie w pionie
    [5] Odbicie w poziomie i obrót o 90° w lewo
    [6] Obrót o 90° w prawo
    [7] Odbicie w poziomie i obrót o 90° w prawo
    [8] Obrót o 90° w lewo
   *[other] wartość { $value }
}

## Plik

file-size-bytes = { $num ->
    [one] { $num } bajt
    [few] { $num } bajty
    [many] { $num } bajtów
   *[other] { $num } bajta
}
# Jednostki dziesiętne (1 KB = 1000 bajtów).
file-size-unit = { $num } { $unit ->
    [kb] KB
    [mb] MB
    [gb] GB
    [tb] TB
   *[pb] PB
}

## Prawa

rating-rejected = Odrzucone
# $stars: „★★★★☆”
rating-stars = { $stars } ({ $n } z 5)

## Poświadczenia treści

c2pa-present = Obecne (niezweryfikowane)
c2pa-detail = Ten plik zawiera manifest C2PA; jego podpis nie został sprawdzony.

## Wartości nagłówka ICC

icc-class = { $class ->
    [scnr] Urządzenie wejściowe
    [mntr] Wyświetlacz
    [prtr] Urządzenie wyjściowe
    [link] Łącze urządzeń
    [spac] Przestrzeń kolorów
    [abst] Abstrakcyjny
    [nmcl] Kolor nazwany
   *[other] Nieznany
}
icc-intent = { $intent ->
    [0] Percepcyjna
    [1] Względna kolorymetryczna
    [2] Nasycenie
    [3] Bezwzględna kolorymetryczna
   *[other] Nieznana
}
icc-platform = { $platform ->
    [APPL] Apple
    [MSFT] Microsoft
    [SGI] Silicon Graphics
    [SUNW] Sun Microsystems
    [none] Nie określono
   *[other] Nieznana
}
icc-coded = { $name } ({ $code })

# machine-drafted, needs native review
label-modality = Modalność
label-body-part = Część ciała
label-study = Badanie
label-series = Seria
label-slice-thickness = Grubość przekroju
label-pixel-spacing = Odstęp pikseli
label-imager-pixel-spacing = Odstęp pikseli na detektorze
label-frames = Klatki
label-transfer-syntax = Składnia transferu
label-document = Dokument
label-document-type = Typ dokumentu
medical-spacing = { $row } × { $column } mm
medical-mm = { $mm } mm
medical-lossy = { $name } (kompresja stratna)
detail-pixel-spacing = Odległość między środkami pikseli w ciele pacjenta (wiersze × kolumny).
# machine-drafted, needs native review
# Pixel spacing of an industrial inspection image (DICONDE, CT of an object).
detail-pixel-spacing-object = Odległość między środkami pikseli w obiekcie (wiersze × kolumny).
detail-imager-pixel-spacing = Zmierzone na detektorze, bez korekty powiększenia: odległości w ciele pacjenta są zwykle mniejsze.

## Badania przemysłowe (DICONDE)

# machine-drafted, needs native review
label-nde-component-shape = Kształt elementu
label-nde-component-manufacturer = Producent elementu
label-nde-material = Materiał
label-nde-thickness = Grubość materiału
label-nde-pipe-diameter = Średnica rury
label-nde-source = Źródło
label-nde-environment = Środowisko
label-nde-iqi = Wskaźnik jakości obrazu
label-nde-indications = Wskazania
label-nde-standard = Norma

## Objętości badawcze (NIfTI, Analyze, NRRD, MetaImage)

# machine-drafted, needs native review
label-voxel-size = Rozmiar woksela
label-data-type = Typ danych
label-time-points = Punkty czasowe
label-orientation-source = Orientacja z
label-intent = Wartości
label-compression = Kompresja
volume-voxel-size = { $x } × { $y } × { $z } mm
volume-orient-sform = NIfTI sform
volume-orient-qform = NIfTI qform
volume-orient-analyze = Nagłówek Analyze (niewiarygodny)
volume-orient-space = Kierunki przestrzeni NRRD
volume-orient-transform = Transformacja MetaImage
volume-orient-spacings = Tylko odstępy wokseli
volume-orient-none = Nie podano
volume-orient-vox2ras = FreeSurfer vox2ras
volume-orient-mgh-default = Domyślne FreeSurfer (niewiarygodne)
volume-orient-minc = Cosinusy kierunkowe wymiarów MINC
volume-orient-object = Współrzędne obiektu (bez orientacji pacjenta)
volume-content-projections = Projekcje (surowy skan)
volume-content-reconstruction = Zrekonstruowana objętość
volume-coords-rectilinear = Prostoliniowa (nierówne odstępy, pokazane równo)
# Scan parameters and tags a FreeSurfer MGH file keeps after its voxels.
label-volume-tr = Czas repetycji
label-volume-te = Czas echa
label-volume-ti = Czas inwersji
label-volume-flip = Kąt przechylenia
label-volume-fov = Pole widzenia
label-volume-field = Natężenie pola
label-volume-pedir = Kodowanie fazy
label-volume-cmdline = Wiersz poleceń
label-volume-xform = Transformacja Talairacha
label-volume-content = Zawartość
label-volume-projections = Projekcje
label-volume-angles = Zakres obrotu
label-volume-angle-step = Krok obrotu
label-volume-kv = Napięcie lampy
label-volume-current = Prąd lampy
label-volume-power = Moc źródła
label-volume-exposure = Ekspozycja
label-volume-sod = Źródło–obiekt
label-volume-odd = Obiekt–detektor
label-volume-sdd = Źródło–detektor
label-volume-magnification = Powiększenie optyczne
label-volume-binning = Binning
label-volume-objective = Obiektyw
label-volume-filter = Filtr źródła
label-volume-camera = Detektor
label-volume-instrument = Urządzenie
label-volume-software = Oprogramowanie
label-volume-date = Data akwizycji
label-volume-ring = Korekcja artefaktów pierścieniowych
label-volume-smoothing = Wygładzanie
label-volume-coords = Siatka
# machine-drafted, needs native review
# An eye scan (OCT): which eye.
label-volume-eye = Oko
volume-eye-right = Prawe (OD)
volume-eye-left = Lewe (OS)
detail-volume-unreliable = Plik nie określa, dokąd w ciele pacjenta wskazują jego osie: lewa i prawa strona mogą być zamienione, dlatego etykiety orientacji nie są wyświetlane.
# machine-drafted, needs native review
# An industrial CT volume: its axes are the scanner's, there is no patient.
detail-volume-object = Osie należą do skanera, a nie do pacjenta, dlatego etykiety orientacji nie są wyświetlane.

## Obrazy naukowe (FITS, XISF)

label-object = Obiekt
label-telescope = Teleskop
label-instrument = Instrument
label-filter = Filtr
label-observed = Obserwacja
label-exposure-time = Ekspozycja
label-sample = Próbki
label-unit = Jednostka
label-hdu = Rozszerzenie
label-centre = Środek
label-centre-deg = Środek (stopnie)
label-frame = Układ współrzędnych
# machine-drafted, needs native review
science-frame-galactic = Galaktyczny
science-frame-ecliptic = Ekliptyczny
label-projection = Odwzorowanie
label-pixel-scale = Skala piksela
science-seconds = { $seconds } s
science-arcsec-px = { $arcsec }″ na piksel
label-distortion = Dystorsja
label-wcs = Współrzędne niebieskie
science-wcs-unsupported = Nieobsługiwane: { $reason }
# machine-drafted, needs native review
science-wcs-approximate = Przybliżone (tylko z pozycji wycelowania, bez dystorsji)
label-observer = Obserwator
label-bayer = Wzór Bayera
label-capture-start = Początek nagrania (UTC)
label-capture-end = Koniec nagrania (UTC)
label-frame-rate = Liczba klatek na sekundę
science-fps = { $fps } kl./s
science-frames-cut = { $frames } z { $declared }
# The HDF5 dataset or NetCDF variable shown.
label-dataset = Zbiór danych
label-pixel-size = Rozmiar piksela
# "0.25 nm per pixel"; $size may be "0.25 × 0.5".
science-pixel-size = { $size } { $unit } na piksel
# Microscopy (DigitalMicrograph, MRC, Gwyddion, Igor).
label-microscope = Mikroskop
label-voltage = Napięcie przyspieszające
label-magnification = Powiększenie
label-imaging-mode = Tryb
label-detector = Detektor
label-acquired = Akwizycja
label-field-of-view = Pole widzenia
science-kilovolts = { $kv } kV
science-magnification = { $value }×
label-vendor = Producent
label-scanner = Skaner
label-objective = Obiektyw
label-pyramid-levels = Poziomy
# machine-drafted, needs native review
label-emissivity = Emisyjność
label-object-distance = Odległość obiektu
label-reflected-temperature = Temperatura odbita
label-atmospheric-temperature = Temperatura atmosfery
label-humidity = Wilgotność względna
label-raw-range = Zakres surowy
label-temperature-range = Zakres temperatur
thermal-temperature = { $value } { $unit }
thermal-metres = { $value } m
thermal-percent = { PERCENT($value) }
thermal-range = { $low } – { $high }
slide-levels = { $count }: { $sizes }
# X-ray diffraction: the experiment geometry from a detector frame's header.
label-wavelength = Długość fali
# Ångström, e.g. "0.97625 Å".
xtal-angstrom = { $value } Å
label-photon-energy = Energia fotonów
xtal-kev = { $value } keV
# Sample to detector distance.
label-detector-distance = Odległość detektora
xtal-mm = { $value } mm
# Where the direct beam meets the detector, in pixels from the image's top-left corner.
label-beam-centre = Środek wiązki
xtal-beam-centre = x { $x }, y { $y } px
# The detector arm's swing angle (2θ).
label-detector-swing = Kąt 2θ detektora
xtal-degrees = { $value }°
# Goniometer angles (Greek letters are the field's usual names).
label-phi = Fi (φ)
label-omega = Omega (ω)
label-chi = Chi (χ)
label-kappa = Kappa (κ)
# The sample's rotation during the exposure: start angle plus width, e.g. "12.5° + 0.1°".
label-oscillation = Oscylacja
xtal-oscillation = { $start }° + { $width }°
# $axis is the axis name from the file, e.g. "phi" or "omega".
xtal-oscillation-axis = { $start }° + { $width }° wokół { $axis }
# The count at which a pixel saturates.
label-overload = Próg nasycenia
# The finest lattice spacing (d) recorded at the detector's edge and in its corners.
label-resolution = Rozdzielczość
xtal-resolution = { $edge } Å na krawędzi, { $corner } Å w narożnikach
label-geometry = Geometria
xtal-curved = Zakrzywiona płyta obrazowa: brak q, d i pierścieni rozdzielczości
xtal-incomplete = Niepełny nagłówek: brak q, d i pierścieni rozdzielczości
# All-fields group title: "FITS header".
fields-science-header = Nagłówek { $format }
# machine-drafted, needs native review
fields-asdf-tree = Drzewo ASDF
label-asdf-model = Model danych
label-asdf-version = Wersja pliku
label-asdf-standard = Standard ASDF
label-asdf-library = Zapisane przez
# machine-drafted, needs native review
# All-fields group title for a CERN ROOT file's list of keys (objects).
fields-root-keys = Klucze ROOT
# CERN ROOT files: the ROOT version that wrote the file (e.g. "6.28/04"), its compression,
# the number of keys (stored objects), and the object shown: class (e.g. "TH2F"), its path in
# the file, title, number of entries and sum of weights.
label-root-version = Wersja ROOT
label-root-compression = Kompresja
label-root-keys = Klucze
label-root-class = Klasa
label-root-name = Nazwa
label-root-title = Tytuł
label-root-entries = Wpisy
label-root-sum-weights = Suma wag
# A graph's number of points.
label-root-points = Punkty
# $axis is "x", "y" or "z".
label-root-axis = Oś { $axis }
# $quantity is the axis title with its unit, e.g. "Energy [GeV]"; $min and $max the axis
# limits.
root-axis-fixed = { $quantity }: { $bins ->
        [one] { $bins } bin
        [few] { $bins } biny
        [many] { $bins } binów
       *[other] { $bins } binu
    }, od { $min } do { $max }
root-axis-variable = { $quantity }: { $bins ->
        [one] { $bins } bin o zmiennej szerokości
        [few] { $bins } biny o zmiennej szerokości
        [many] { $bins } binów o zmiennej szerokości
       *[other] { $bins } binu o zmiennej szerokości
    }, od { $min } do { $max }
label-root-mean-rms = Średnia i RMS { $axis }
root-mean-rms = { $mean } ± { $rms }
# Trees (event data) are listed only, with their entry counts.
label-root-trees = Drzewa
root-tree = { $name } ({ $entries ->
        [one] { $entries } wpis
        [few] { $entries } wpisy
        [many] { $entries } wpisów
       *[other] { $entries } wpisu
    })
# machine-drafted, needs native review
# CASA images: what the image is (from its directory name), its restoring beam (major and
# minor axis, position angle), the beams of its planes when they differ, the rest frequency,
# spectral reference frame (e.g. "LSRK") and velocity definition of the frequency labels,
# the default mask (e.g. "mask0") and the casacore storage manager with its byte order.
label-casa-type = Typ obrazu
casa-type-image = Obraz odtworzony
casa-type-residual = Residuum
casa-type-model = Model
casa-type-pb = Wiązka pierwotna
casa-type-mask = Maska czyszczenia
casa-type-psf = Funkcja rozmycia punktu
casa-type-sumwt = Suma wag
casa-type-weight = Wagi
label-casa-beam = Wiązka odtwarzająca
casa-beam = { $major }″ × { $minor }″, KP { $pa }°
label-casa-plane-beams = Wiązki poszczególnych płaszczyzn
casa-plane-beams = { $count ->
        [one] { $count } wiązka
        [few] { $count } wiązki
        [many] { $count } wiązek
       *[other] { $count } wiązki
    }, oś wielka od { $min }″ do { $max }″
label-casa-rest-frequency = Częstotliwość spoczynkowa
casa-ghz = { $ghz } GHz
label-casa-spectral-frame = Spektralny układ odniesienia
label-casa-velocity = Definicja prędkości
casa-velocity-radio = Radiowa
casa-velocity-optical = Optyczna
casa-velocity-relativistic = Relatywistyczna
label-casa-mask = Maska domyślna
label-casa-storage = Przechowywanie
casa-storage-big = { $manager }, big-endian
casa-storage-little = { $manager }, little-endian
# machine-drafted, needs native review
# Unstructured meshes: the mesh's name in the file, the convention describing it (UGRID,
# ICON, MPAS), its counts, face shapes, coordinates and where the values sit.
label-ugrid-name = Siatka
label-ugrid-convention = Konwencja
label-ugrid-nodes = Węzły
label-ugrid-faces = Ściany
label-ugrid-edges = Krawędzie
label-ugrid-shapes = Kształty ścian
label-ugrid-coordinates = Współrzędne
label-ugrid-values = Wartości na
label-ugrid-split = Podzielone do rysowania
# $n is the formatted count, $count the number (for the plural).
ugrid-triangles = { $count ->
    [one] { $count } trójkąt
    [few] { $count } trójkąty
    [many] { $count } trójkątów
   *[other] { $count } trójkąta
}
ugrid-quads = { $count ->
    [one] { $count } czworokąt
    [few] { $count } czworokąty
    [many] { $count } czworokątów
   *[other] { $count } czworokąta
}
# Faces of five or more nodes; $max is the most nodes of one face.
ugrid-polygons = { $count ->
    [one] { $count } wielokąt o { $max } węzłach
    [few] { $count } wielokąty o maks. { $max } węzłach
    [many] { $count } wielokątów o maks. { $max } węzłach
   *[other] { $count } wielokąta o maks. { $max } węzłach
}
ugrid-lonlat = Długość i szerokość geograficzna
ugrid-projected = Rzutowane x i y
ugrid-projected-unit = Rzutowane x i y ({ $unit })
ugrid-on-faces = Ściany (rysowane jednolicie)
ugrid-on-nodes = Węzły (interpolowane na ścianach)
# A mesh file without data variables: the image shows how many nodes each face has.
ugrid-grid-only = Brak: obraz pokazuje liczbę węzłów na ścianę
# Faces cut at the antimeridian or closed to a pole so they draw on a flat map.
ugrid-split = { $count ->
    [one] { $count } ściana na antypołudniku lub biegunie
    [few] { $count } ściany na antypołudniku lub biegunie
    [many] { $count } ścian na antypołudniku lub biegunie
   *[other] { $count } ściany na antypołudniku lub biegunie
}
# machine-drafted, needs native review
# Slide annotations (NDP.view, Aperio ImageScope, ASAP sidecar files).
label-annotation-file = Plik
# "slide.xml (Aperio ImageScope)"
annotation-source = { $file } ({ $format })
label-annotation-shapes = Kształty
label-annotation-groups = Grupy
# Shapes or points over the reader's limits, left out.
label-annotation-skipped = Pominięte
annotation-polygon = Wielokąt
annotation-polyline = Linia
annotation-rectangle = Prostokąt
annotation-ellipse = Elipsa
annotation-point = Punkt
annotation-points = Punkty
annotation-ruler = Linijka
annotation-arrow = Strzałka
# The list of annotations is cut short: "and 1,200 more".
annotation-more = i więcej: { $count }
# All-fields group title: "Annotations in slide.xml".
fields-annotations-header = Adnotacje w { $file }
label-drawing-version = Wersja
label-drawing-units = Jednostki
label-drawing-extents = Zasięg
label-drawing-sheet = Arkusz
label-drawing-layouts = Układy
label-drawing-layers = Warstwy
label-drawing-entities = Obiekty
label-drawing-codepage = Strona kodowa
label-drawing-missing-fonts = Zastąpione czcionki
# machine-drafted, needs native review
# A font file the drawing names that was not found: the styles naming it, and the font drawing its text instead.
drawing-missing-font = { $file } (style { $styles }), zastąpiona czcionką { $substitute }
# A Big Font (double-byte text) that was not found; its characters come from other fonts.
drawing-missing-bigfont = { $file } (style { $styles }), zastąpiona czcionkami zastępczymi
# Survey data (LandXML).
label-survey-crs = Układ współrzędnych
survey-crs-none = Nie podano
label-survey-project = Nazwa projektu
label-survey-application = Zapisane przez
label-survey-surface = Model terenu
# A TIN surface: its name, counts, plan and sloped areas, the volume above its lowest point, its elevations and the contour interval drawn; unit is "m" or "ft" (areas in square units, volume in cubic units).
survey-surface = { $name }: punktów { $points }, trójkątów { $triangles }; powierzchnia w rzucie { $area } { $unit }², powierzchnia rzeczywista { $area3d } { $unit }²; { $volume } { $unit }³ powyżej najniższego punktu; wysokości { $zmin }–{ $zmax } { $unit }; warstwice co { $interval } { $unit }
label-survey-alignment = Oś trasy
# An alignment: its name, length and first and last stations ("1+234.50").
survey-alignment = { $name }: { $length } { $unit }, kilometraż od { $start } do { $end }
# SHX shape fonts found and used for the drawing's text.
label-drawing-shx-fonts = Czcionki SHX
label-drawing-warnings = Problemy
label-plot-paper = Papier
# "2 of 5": the page shown and how many the plot has.
plot-page-of = { $page } z { $pages }
label-plot-page = Strona
label-plot-pens = Użyte pisaki
# The printer language a plot came wrapped in, e.g. "PCL 5".
label-plot-job = Zadanie drukowania
# What a plot draws: strokes, fills and labels.
label-plot-items = Narysowane elementy
# Width × height of what is drawn, in drawing units: "1,250 × 800 mm".
drawing-extents = { $width } × { $height } { $unit }
drawing-extents-unitless = { $width } × { $height }
drawing-model-space = Model
# "24, 3 off or frozen"
drawing-layers-off = { $num }, w tym { $off } wyłączonych lub zamrożonych
# DXF $INSUNITS.
drawing-units = { $code ->
    [1] Cale
    [2] Stopy
    [3] Mile
    [4] Milimetry
    [5] Centymetry
    [6] Metry
    [7] Kilometry
    [8] Mikrocale
    [9] Tysięczne cala
    [10] Jardy
    [11] Ångströmy
    [12] Nanometry
    [13] Mikrometry
    [14] Decymetry
    [15] Dekametry
    [16] Hektometry
    [17] Gigametry
    [18] Jednostki astronomiczne
    [19] Lata świetlne
    [20] Parseki
    [21] Stopy geodezyjne USA
   *[other] Bez jednostek
}
# All-fields group title: "DXF entities".
fields-drawing-entities = Obiekty { $format }

## Layouty układów scalonych (GDSII)

# machine-drafted, needs native review
label-layout-library = Biblioteka
label-layout-units = Jednostki
# The database unit (the grid coordinates are stored in) and the user unit, with SI symbols:
# "1 nm database unit, 1 µm user unit".
layout-units = { $db } jednostka bazy danych, { $user } jednostka użytkownika
# The top cell shown (each top cell is a page).
label-layout-cell = Komórka
label-layout-cells = Komórki
# Cells no other cell places.
label-layout-top-cells = Komórki najwyższego poziomu
# "TOP, TEST, PAD and 12 more"
layout-top-cells-more = { $names } i jeszcze { $more }
# Layer/datatype pairs.
label-layout-layers = Warstwy
# Polygons, paths and boxes, as stored (each cell once).
label-layout-shapes = Kształty
label-layout-texts = Etykiety
# Cell references and the placements they make: "1,204 (88,320 placements)".
label-layout-references = Referencje
layout-references = { $refs } ({ $placements } { $placements ->
    [one] umieszczenie
    [few] umieszczenia
    [many] umieszczeń
   *[other] umieszczenia
})
# Shapes with every cell placement expanded.
label-layout-flat-shapes = Kształty po spłaszczeniu
label-layout-bounds = Obwiednia
# Corners of what the cell draws, micrometres: "-12.5, 0 to 2,900, 3,500 µm".
layout-bounds = { $x0 }, { $y0 } do { $x1 }, { $y1 } µm

## Płytki drukowane i schematy (Gerber, Excellon, KiCad)

# machine-drafted, needs native review
group-board = Płytka drukowana
group-schematic = Schemat
# The board view shown (its layers, its top or bottom), or the schematic sheet.
label-pcb-view = Widok
label-pcb-project = Projekt
label-pcb-revision = Rewizja
label-pcb-date = Data
# The CAD program that wrote the file.
label-pcb-generator = Utworzono w
# Width × height of the board outline: "100 × 80 mm".
label-pcb-size = Rozmiar płytki
label-pcb-thickness = Grubość
label-pcb-copper-layers = Warstwy miedzi
label-pcb-sheets = Arkusze
# The board's layers or files, each with its role: "board-F_Cu.gbr (Top copper)".
label-pcb-layers = Warstwy
pcb-layer-role = { $name } ({ $role })
label-pcb-components = Footprinty
label-pcb-symbols = Symbole
label-pcb-nets = Sieci
label-pcb-tracks = Ścieżki
label-pcb-wires = Połączenia
label-pcb-vias = Przelotki
# Drilled holes and how many drill sizes: "1,204 in 9 sizes".
label-pcb-holes = Otwory
pcb-holes = { $holes } w { $sizes } { $sizes ->
    [one] rozmiarze
    [few] rozmiarach
    [many] rozmiarach
   *[other] rozmiarach
}
label-pcb-apertures = Apertury
# What a board layer or file is, shown after its name.
pcb-role-inner-copper = Miedź wewnętrzna { $n }
pcb-role-top-copper = Miedź górna
pcb-role-bottom-copper = Miedź dolna
pcb-role-top-solder-mask = Maska lutownicza górna
pcb-role-bottom-solder-mask = Maska lutownicza dolna
pcb-role-top-silkscreen = Opisy górne
pcb-role-bottom-silkscreen = Opisy dolne
pcb-role-top-paste = Pasta górna
pcb-role-bottom-paste = Pasta dolna
pcb-role-top-solder-paste = Pasta lutownicza górna
pcb-role-bottom-solder-paste = Pasta lutownicza dolna
pcb-role-top-adhesive = Klej górny
pcb-role-bottom-adhesive = Klej dolny
pcb-role-top-courtyard = Obrys zajętości górny
pcb-role-bottom-courtyard = Obrys zajętości dolny
pcb-role-top-fabrication = Produkcyjna górna
pcb-role-bottom-fabrication = Produkcyjna dolna
pcb-role-board-outline = Obrys płytki
pcb-role-plated-drill = Wiercenia metalizowane
pcb-role-non-plated-drill = Wiercenia niemetalizowane
pcb-role-drill = Wiercenia
pcb-role-drill-holes = Otwory wiercone
pcb-role-mechanical = Mechaniczna
pcb-role-margin = Margines
pcb-role-drawings = Rysunki
pcb-role-comments = Komentarze
pcb-role-engineering-change-1 = Zmiana techniczna 1
pcb-role-engineering-change-2 = Zmiana techniczna 2
pcb-role-vias = Przelotki
pcb-role-other = Inna
pcb-role-unknown = Nieznana
label-pcb-macros = Makra apertur

## Widma (JCAMP-DX, SPC, mzML, mzXML, Bruker NMR)

# machine-drafted, needs native review
group-spectrum = Widmo
label-spectrum-format = Format
label-spectrum-technique = Technika
label-spectrum-title = Tytuł
# The page shown: "3 of 1,204: Scan 1532".
label-spectrum-page = Strona
spectrum-page-of = { $page } z { $pages }: { $name }
# An axis with its unit: "Wavenumber (cm⁻¹)".
spectrum-axis = { $title } ({ $unit })
label-spectrum-x = Oś X
label-spectrum-y = Oś Y
label-spectrum-nucleus = Jądro
label-spectrum-frequency = Częstotliwość spektrometru
label-spectrum-solvent = Rozpuszczalnik
label-spectrum-pulse-program = Program impulsowy
label-spectrum-spectra = Widma
label-spectrum-points = Punkty
label-spectrum-instrument = Instrument
# Who or what lab measured it.
label-spectrum-origin = Pochodzenie
label-spectrum-date = Zmierzono
# How Specular made the page from the raw data (an FID transformed, phased automatically).
label-spectrum-processing = Przetwarzanie
spectrum-technique-ir = Podczerwień
spectrum-technique-raman = Raman
spectrum-technique-uv-vis = UV-Vis
spectrum-technique-fluorescence = Fluorescencja
spectrum-technique-nmr = NMR
spectrum-technique-ms = Widmo masowe
spectrum-technique-chromatogram = Chromatogram
spectrum-technique-other = Widmo

## Modele 3D (STL, OBJ, PLY, glTF, 3MF)

# machine-drafted, needs native review
group-mesh = Model 3D
label-triangles = Trójkąty
label-vertices = Wierzchołki
label-materials = Materiały
label-textures = Tekstury
label-up-axis = Oś pionowa
# Textures the model uses, some of which could not be read: "3 (1 missing)".
mesh-textures-missing = { $count } (brak: { $missing })

## Struktury cząsteczkowe (PDB, mmCIF, CIF, SDF, XYZ)

# machine-drafted, needs native review
group-structure = Struktura
# The entry's identifier: a PDB ID such as 1CRN, or the CIF data block's name.
label-structure-id = Wpis
label-structure-classification = Klasyfikacja
# How the structure was determined: X-ray diffraction, electron microscopy, solution NMR…
label-structure-method = Metoda
# The crystallographic or cryo-EM resolution, in ångströms.
label-structure-resolution = Rozdzielczość
structure-resolution = { $value } Å
label-structure-deposited = Zdeponowano
label-structure-formula = Wzór
label-structure-chains = Łańcuchy
# Residues (amino acids, nucleotides) in the polymer chains.
label-structure-residues = Reszty
label-structure-atoms = Atomy
# Atoms after the unit cell was filled by symmetry, and those the file lists: "96 (12 in the asymmetric unit)".
structure-atoms-cell = { $count } ({ $asymmetric } w jednostce asymetrycznej)
label-structure-bonds = Wiązania
# Models in the file (an NMR ensemble), of which the first is shown.
label-structure-models = Modele
# Molecules (SDF records) or data blocks in the file, of which the first is shown.
label-structure-records = Rekordy
structure-first-of = { $count } (wyświetlany jest pierwszy)
# Small molecules bound to the polymer, by their three-letter codes.
label-structure-ligands = Ligandy
# A list cut short: "HEM, NAG, SO4 and 9 more".
structure-ligands-more = { $list } i jeszcze { $more }
label-structure-cell = Komórka elementarna
# Unit cell edge lengths (Å) and angles (degrees).
structure-cell = a = { $a } Å, b = { $b } Å, c = { $c } Å, α = { $alpha }°, β = { $beta }°, γ = { $gamma }°
label-structure-space-group = Grupa przestrzenna
label-structure-secondary = Struktura drugorzędowa
structure-ss-file = Z pliku
structure-ss-computed = Obliczona na podstawie łańcucha głównego

## HDF4 i HDF-EOS2

# machine-drafted, needs native review
group-hdf4 = HDF4
# The HDF library version that wrote the file, e.g. "NCSA HDF Version 4.2 Release 15".
label-hdf4-library = Biblioteka
# Counts of the file's scientific datasets, raster images, groups (Vgroups) and tables (Vdatas).
label-hdf4-datasets = Zbiory danych
label-hdf4-images = Obrazy rastrowe
label-hdf4-vgroups = Grupy
label-hdf4-vdatas = Tabele
# HDF-EOS: NASA's Earth Observing System conventions on top of HDF4 (MODIS, ASTER, AIRS…).
group-hdf-eos = HDF-EOS
label-eos-version = Wersja
# The grids, swaths and point structures the file defines.
label-eos-structures = Struktury
eos-grids = { $count ->
    [one] { $count } siatka ({ $names })
    [few] { $count } siatki ({ $names })
    [many] { $count } siatek ({ $names })
   *[other] { $count } siatki ({ $names })
}
eos-swaths = { $count ->
    [one] { $count } pas skanowania ({ $names })
    [few] { $count } pasy skanowania ({ $names })
    [many] { $count } pasów skanowania ({ $names })
   *[other] { $count } pasa skanowania ({ $names })
}
eos-points = { $count ->
    [one] { $count } struktura punktowa
    [few] { $count } struktury punktowe
    [many] { $count } struktur punktowych
   *[other] { $count } struktury punktowej
}
# The grid or swath the dataset shown belongs to.
label-eos-structure = Struktura
# The grid's map projection as its GCTP name, e.g. GCTP_SNSOID (sinusoidal).
label-eos-projection = Odwzorowanie
label-eos-grid-size = Rozmiar siatki
# Columns × rows.
eos-grid-size = { $width } × { $height }
# The grid's upper-left and lower-right corners.
label-eos-corners = Narożniki
# The product's short name from the inventory metadata, e.g. MOD09A1.
label-eos-product = Produkt
# When the data was acquired.
label-eos-time = Czas
eos-time = od { $begin } do { $end }

## Obrazy planetarne (VICAR, PDS3, PDS4: dane misji NASA i ESA)

# machine-drafted, needs native review
group-planetary = Obraz planetarny
# The body observed, e.g. MARS, Ganymede.
label-planetary-target = Cel
label-planetary-mission = Misja
# The spacecraft, rover or lander carrying the instrument.
label-planetary-spacecraft = Statek kosmiczny
label-planetary-start = Czas rozpoczęcia
label-planetary-stop = Czas zakończenia
# The product's identifier in the archive.
label-planetary-product = ID produktu
# The map projection of a map-projected image, as the label names it.
label-planetary-projection = Odwzorowanie kartograficzne
label-planetary-resolution = Rozdzielczość
planetary-metres-per-pixel = { $metres } m na piksel
planetary-pixels-per-degree = { $pixels } px na stopień
# Latitude and longitude of the map projection's centre.
label-planetary-centre = Środek odwzorowania
planetary-lat-lon = { $lat }°, { $lon }°
# The latitudes and longitudes the map covers.
label-planetary-coverage = Zasięg
planetary-coverage = szerokość od { $south }° do { $north }°, długość od { $west }° do { $east }°
# The body's equatorial radius the map uses.
label-planetary-radius = Promień ciała
planetary-km = { $km } km

## OMERO (obrazy otwarte z serwera OMERO lub z Image Data Resource)

# machine-drafted, needs native review
group-omero = OMERO
label-omero-server = Serwer
# $server the server's address; $user the user name signed in with.
omero-server-user = { $server } (jako { $user })
omero-server-public = { $server } (publiczny)
label-omero-image = Obraz
# $name the image's name; $id its number on the server.
omero-image = { $name } (ID { $id })
# The project and dataset the image is in, as "Project › Dataset".
label-omero-project = Projekt
# The screen, plate and well, as "Screen › Plate › B4".
label-omero-screen = Ekran
label-omero-owner = Właściciel
# $owner the image's owner; $group the OMERO group it belongs to.
omero-owner-group = { $owner } ({ $group })
label-omero-acquired = Akwizycja
label-omero-dimensions = Wymiary
omero-size = { $width } × { $height }
omero-z = { $count ->
    [one] { $count } płaszczyzna Z
    [few] { $count } płaszczyzny Z
    [many] { $count } płaszczyzn Z
   *[other] { $count } płaszczyzny Z
}
omero-c = { $count ->
    [one] { $count } kanał
    [few] { $count } kanały
    [many] { $count } kanałów
   *[other] { $count } kanału
}
omero-t = { $count ->
    [one] { $count } punkt czasowy
    [few] { $count } punkty czasowe
    [many] { $count } punktów czasowych
   *[other] { $count } punktu czasowego
}
# The physical size of a pixel along X, Y (and Z).
label-omero-physical = Rozmiar piksela
label-omero-channels = Kanały
# A channel not shown in the server's rendering.
omero-channel-hidden = { $channel } (ukryty)
# How the pixels are fetched from the server: rendered tiles (JPEG) or the whole plane (PNG).
label-omero-fetched = Pobierane jako
omero-fetched-tiles = kafelki { $encoding } { $width } × { $height }, { $levels ->
    [one] { $levels } poziom
    [few] { $levels } poziomy
    [many] { $levels } poziomów
   *[other] { $levels } poziomu
}
omero-fetched-whole = Całe płaszczyzny jako { $encoding }

## Obrazy transformacji odbicia (PTM, RTI, Relight)

# machine-drafted, needs native review
group-rti = Transformacja odbicia
# The reflectance model the coefficients belong to: PTM (polynomial), HSH (hemispherical harmonics), RBF…
label-rti-basis = Baza
label-rti-order = Rząd
# Coefficients per colour channel.
label-rti-terms = Współczynniki
# How colour is stored: LRGB (one colour, varying luminance), RGB, YCC…
label-rti-colour = Układ kolorów
# Coefficient images stored in the file.
label-rti-planes = Płaszczyzny
# The byte-to-coefficient mapping the file states, per coefficient.
label-rti-scale = Skala
label-rti-bias = Przesunięcie

## Chmury punktów

# machine-drafted, needs native review
group-points = Chmura punktów
label-points = Punkty
# Points read of those in the file, when a very large cloud was evenly thinned: "32,000,000 of 120,000,000".
points-subsampled = { $loaded } z { $total }
# The LAS point data record format number (0–10).
label-point-format = Format punktu
# The coordinate reference system.
label-crs = Układ współrzędnych
# What each point carries besides its position.
label-attributes = Atrybuty
points-channel-rgb = RGB
points-channel-intensity = Intensywność
points-channel-classification = Klasyfikacja
points-channel-returns = Odbicia
# ASPRS classes present, with point counts: "Ground (1,234), Building (567)".
label-classes = Klasy
# E57 scans, with point counts.
label-scans = Skany
point-class-0 = Nigdy niesklasyfikowane
point-class-1 = Niesklasyfikowane
point-class-2 = Grunt
point-class-3 = Niska roślinność
point-class-4 = Średnia roślinność
point-class-5 = Wysoka roślinność
point-class-6 = Budynek
point-class-7 = Niski punkt (szum)
point-class-8 = Punkt kluczowy modelu
point-class-9 = Woda
point-class-10 = Tory kolejowe
point-class-11 = Nawierzchnia drogi
point-class-12 = Nakładanie się
point-class-13 = Linka odgromowa
point-class-14 = Przewód fazowy
point-class-15 = Słup przesyłowy
point-class-16 = Złącze przewodów
point-class-17 = Pomost mostu
point-class-18 = Wysoki szum
point-class-19 = Konstrukcja napowietrzna
point-class-20 = Pominięty grunt
point-class-21 = Śnieg
point-class-22 = Wykluczenie czasowe
# A class without a standard name, by number.
point-class-other = Klasa { $class }

## Gaussian splats

# machine-drafted, needs native review
group-splats = Splaty gaussowskie
label-splats = Splaty
# Splats shown of those in the file, when a very large file was evenly thinned: "12,000,000 of 30,000,000".
splats-shown-of = { $shown } z { $total }
# The degree (1 to 3) of the spherical harmonics that give each splat its view-dependent colour.
label-splat-harmonics = Kolor zależny od kierunku patrzenia
splats-harmonics-none = Brak
# $format: the file format, e.g. "SPZ".
splats-format-antialiased = { $format }, trenowany z antyaliasingiem

## Ostrzeżenia

# machine-drafted, needs native review
warn-not-diagnostic = Nie do celów diagnostycznych. Specular to ogólna przeglądarka obrazów, a nie wyrób medyczny: wyświetlanie nie jest skalibrowane do diagnostyki.

warn-gps = Ten plik zawiera miejsce wykonania zdjęcia. Jeśli ma ono pozostać prywatne, usuń lokalizację przed udostępnieniem.
warn-null-island = Pozycja GPS to dokładnie 0°, 0°; aparat prawdopodobnie nie ustalił pozycji, więc lokalizacja jest zapewne błędna.
warn-date-future = Data wykonania leży w przyszłości; zegar aparatu był prawdopodobnie źle ustawiony.
warn-icc-damaged = Osadzony profil kolorów jest uszkodzony i nie można go odczytać.
warn-metadata-failed = Nie udało się odczytać metadanych z powodu błędu w programie Specular. Sam obraz jest w porządku.
warn-orientation-conflict = EXIF i XMP podają różną orientację (EXIF: { $exif }, XMP: { $xmp }). Używana jest wartość EXIF.
warn-exif-damaged = Części danych EXIF nie można odczytać ({ $error }); pokazano resztę.
warn-xmp-damaged = Metadane XMP są uszkodzone ({ $error }); odczytano tylko ich część.

## Okno „O programie”

geonames-attribution = Nazwy miejsc: GeoNames (www.geonames.org), na licencji Creative Commons Uznanie autorstwa 4.0 (CC BY 4.0, https://creativecommons.org/licenses/by/4.0/). Wyszukiwanie offline za pomocą biblioteki reverse_geocoder autorstwa Granta Minera (MIT lub Apache-2.0). Dane przekształcono w listę miejscowości; są udostępniane w stanie, w jakim są, bez gwarancji.

## TIFF and DNG structure values (All fields)
# machine-drafted, needs native review

subfile-type = { $value ->
    [0] Obraz w pełnej rozdzielczości
    [1] Obraz w zmniejszonej rozdzielczości
    [2] Jedna strona obrazu wielostronicowego
    [4] Maska przezroczystości
   *[other] Inne
}
predictor = { $value ->
    [1] Brak
    [2] Różnicowanie poziome
    [3] Różnicowanie zmiennoprzecinkowe
   *[other] Inne
}
extra-samples = { $value ->
    [0] Nieokreślone
    [1] Alfa wstępnie przemnożona
    [2] Alfa prosta
   *[other] Inne
}
sample-format = { $value ->
    [1] Liczba całkowita bez znaku
    [2] Liczba całkowita ze znakiem
    [3] Liczba zmiennoprzecinkowa
    [4] Niezdefiniowany
   *[other] Inne
}

## Dane przestrzenne (GeoTIFF, GeoJSON, Shapefile, GeoPackage, KML/KMZ, GPX)

# machine-drafted, needs native review
group-geospatial = Dane przestrzenne
label-geo-crs = Układ współrzędnych
label-geo-datum = Datum geodezyjne
label-geo-projection = Odwzorowanie
label-geo-unit = Jednostki
label-geo-bounds = Zasięg
label-geo-lonlat-bounds = Długość i szerokość geograficzna
label-geo-pixel-size = Rozmiar piksela
label-geo-overviews = Podglądy
label-geo-nodata = Wartość NoData
label-geo-raster-type = Typ rastra
label-geo-vertical = Odniesienie pionowe
label-geo-citation = Opis źródła
label-geo-features = Obiekty
label-geo-vertices = Wierzchołki
label-geo-layer = Warstwa
label-geo-layers = Warstwy
label-geo-tiles = Kafle
label-geo-encoding = Kodowanie tekstu
label-geo-warnings = Problemy
# The companion files a georeference was read from ("scene.jgw, scene.prj").
label-geo-sidecars = Pliki towarzyszące
# machine-drafted, needs native review
# A projection Specular cannot convert to longitude and latitude: $method is its name.
geo-projection-unsupported = { $method } (współrzędne nieobsługiwane)
# How longitude and latitude are brought to WGS 84 (what GPS and web maps use).
label-geo-to-wgs84 = Do WGS 84
geo-to-wgs84-same = Niepotrzebne
geo-to-wgs84-helmert = Zmiana układu odniesienia (Helmert)
geo-to-wgs84-by-area = Zmiana układu odniesienia (Helmert, wybrana według obszaru)
geo-to-wgs84-unknown = Nieznane: współrzędne są w układzie odniesienia pliku
# A coordinate system's name and its EPSG code: "WGS 84 / UTM zone 33N (EPSG:32633)".
geo-crs-epsg = { $name } (EPSG:{ $code })
geo-crs-unknown = Nie podano
# Map bounds: the lower-left and upper-right corners.
geo-bounds = od { $x0 }, { $y0 } do { $x1 }, { $y1 }
# Bounds in degrees: west to east longitude, south to north latitude.
geo-lonlat-bounds = długość od { $west }° do { $east }°, szerokość od { $south }° do { $north }°
geo-pixel-size = { $x } × { $y }
geo-overviews = { $n ->
    [one] { $n } podgląd: { $sizes }
    [few] { $n } podglądy: { $sizes }
    [many] { $n } podglądów: { $sizes }
   *[other] { $n } podglądu: { $sizes }
}
geo-pixel-is-point = Piksele są punktami
geo-layer = { $features ->
    [one] { $features } obiekt
    [few] { $features } obiekty
    [many] { $features } obiektów
   *[other] { $features } obiektu
}, { $fields ->
    [one] { $fields } pole
    [few] { $fields } pola
    [many] { $fields } pól
   *[other] { $fields } pola
}
# A GeoPackage tile pyramid: its table, the zoom level shown and its tile count.
geo-tiles = { $table }, poziom powiększenia { $zoom }, { $tiles ->
    [one] { $tiles } kafel
    [few] { $tiles } kafle
    [many] { $tiles } kafli
   *[other] { $tiles } kafla
}
fields-geo-geokeys = Klucze GeoTIFF
fields-geo-gdal = Metadane GDAL
fields-geo-wkt = Układ współrzędnych (WKT)
fields-geo-projjson = Układ współrzędnych (PROJJSON)
# machine-drafted, needs native review
# What a web map tile set (MBTiles, PMTiles) says about itself: container, tile type, zoom
# levels, description and other metadata.
fields-geo-tileset = Zestaw kafelków
# The credit line a map's data asks to be shown with it ("© OpenStreetMap contributors").
label-geo-attribution = Źródło danych
# The header lines of a georeferenced raster format (ENVI, ASCII grid, DTED, ERDAS IMAGINE).
fields-geo-header = Nagłówek
# machine-drafted, needs native review
# GIFTI files: the format version, what the file holds, the number of data arrays and the
# encoding of their values (e.g. "GZipBase64Binary").
label-gifti-version = Wersja GIFTI
label-gifti-role = Zawiera
gifti-role-surface = Powierzchnię
gifti-role-labels = Etykiety dla wierzchołków
gifti-role-data = Wartości dla wierzchołków
label-gifti-arrays = Tablice danych
label-gifti-encoding = Kodowanie
label-gifti-vertices = Wierzchołki
label-gifti-triangles = Trójkąty
# The brain structure, e.g. "CortexLeft".
label-gifti-structure = Struktura
# A surface's geometric type, e.g. "Anatomical", "Inflated".
label-gifti-geometry = Typ geometryczny
# The coordinate transform of the vertices: $from and $to are coordinate spaces such as
# "scanner", "Talairach" or "MNI 152".
label-gifti-transform = Współrzędne
gifti-transform = { $from } (transformacja do { $to } niepotrzebna)
gifti-transform-applied = { $from } → { $to } (zastosowana)
# The surface file a data file is drawn on, found beside it.
label-gifti-surface = Wyświetlane na
# The data array shown: its name, NIfTI intent (e.g. "NIFTI_INTENT_SHAPE") and data type.
label-gifti-array = Tablica
label-gifti-intent = Intent
label-gifti-data-type = Typ danych
# Entries of the label table (names and colours of regions).
label-gifti-labels = Tabela etykiet
# CIFTI-2 files: the version, NIfTI intent (e.g. "ConnDenseScalar") and matrix size.
label-cifti-version = Wersja CIFTI
label-cifti-intent = Intent
label-cifti-matrix = Macierz
cifti-matrix = { $rows } × { $columns }
# The maps (pages) of the file and what they are.
label-cifti-maps = Mapy
cifti-maps-scalars = Mapy skalarne: { $count }
cifti-maps-labels = Mapy etykiet: { $count }
cifti-maps-series = Punkty czasowe: { $count }
cifti-maps-parcels = Parcele: { $count }
cifti-maps-other = { $count }
# machine-drafted, needs native review
# A dense connectivity file (.dconn.nii): every brain place's connectivity to every other.
# $seeds: how many places can be picked; $size: the bytes read when one is picked (e.g. "357 KB").
label-cifti-dense = Łączność
cifti-dense = Punkty seed: { $seeds }, { $size } odczytu na punkt
# A time series' start and step; $unit is e.g. "second".
label-cifti-series = Seria
cifti-series = od { $start }, co { $step } ({ $unit })
label-cifti-models = Modele mózgu
label-cifti-parcels = Parcele
label-cifti-structures = Struktury
label-cifti-voxels = Woksele podkorowe
# A surface the file's values lie on: the brain structure (e.g. "CORTEX_LEFT"), its vertex
# count, and the surface file found beside the file for it, or that none was found.
label-cifti-surface = Powierzchnia
cifti-surface-found = { $structure }: wierzchołki: { $vertices }, { $file }
cifti-surface-missing = { $structure }: potrzebna powierzchnia z { $vertices ->
        [one] { $vertices } wierzchołkiem
        [few] { $vertices } wierzchołkami
        [many] { $vertices } wierzchołkami
       *[other] { $vertices } wierzchołka
    } (brak obok pliku)
# How the file is shown: on the surfaces found beside it, or as its subcortical voxels.
label-cifti-shown = Wyświetlane jako
cifti-shown-surfaces = Pokolorowane powierzchnie
cifti-shown-voxels = Objętość wokseli podkorowych
# machine-drafted, needs native review
# Shown with a picked seed's row: click a surface vertex, or move the crosshair in MPR.
cifti-shown-connectivity = Łączność wybranego punktu seed
# Maps left out of the voxel volume, which would be too large with them.
label-cifti-left-out = Pominięte mapy
# Tractograms: format, number of streamlines (fibre tracts) and points.
label-tract-format = Format
label-tract-streamlines = Linie prądu
# A large tractogram draws only some of its streamlines.
tract-streamlines-shown = { $count } (wyświetlane: { $shown })
label-tract-points = Punkty
label-tract-mean-length = Średnia długość
# A length in millimetres.
tract-mm = { $value } mm
# Values per point and per streamline the file stores, by name.
label-tract-scalars = Skalary dla punktów
label-tract-properties = Właściwości dla linii prądu
# TrackVis: the voxel grid the tracts were made in, its voxel size and voxel order (e.g. "LAS").
label-tract-grid = Siatka
label-tract-voxel-size = Rozmiar woksela
label-tract-voxel-order = Kolejność wokseli
# How the points were brought into scanner (RAS) millimetres.
label-tract-space = Współrzędne
tract-space-vox-to-ras = Z macierzy woksel–RAS
tract-space-voxel-order = Z kolejności wokseli (brak macierzy woksel–RAS)
tract-space-scanner = Przestrzeń skanera
# MRtrix: the tracking step size.
label-tract-step = Rozmiar kroku

## File headers (DPX, Cineon, XWD, MIFF, TIM, WSQ, JBIG)

# machine-drafted, needs native review
# What the file's own header records, for formats without Exif or XMP.
group-header = Nagłówek pliku
label-header-created = Utworzono
# The program or person that wrote the file.
label-header-creator = Autor
label-header-copyright = Prawa autorskie
label-header-project = Projekt
label-header-comment = Komentarz
# The scanner, camera or other device the image came from.
label-header-device = Urządzenie
# SMPTE time code of the frame: "01:00:00:12".
label-header-timecode = Kod czasowy
# A film frame's edge code (KeyKode).
label-header-edge-code = Kod krawędziowy
label-header-frame-rate = Liczba klatek na sekundę
# Where the frame sits in its sequence.
label-header-frame = Klatka
# Film stock and slate.
label-header-film = Film
# How code values relate to light: printing density, linear, BT.709…
label-header-transfer = Funkcja przenoszenia
# The colour primaries the file states.
label-header-colorimetric = Kolorymetria
# The X window a screen dump was taken of.
label-header-window = Okno
label-header-resolution = Rozdzielczość
label-header-compression = Kompresja
# machine-drafted, needs native review
# JPEG XS: the profile and the level the codestream states ("Main 422.10"; "4k-1, sublevel 6 bpp").
label-header-profile = Profil
# machine-drafted, needs native review
label-header-level = Poziom
# machine-drafted, needs native review
# JPEG XS: how many wavelet levels, horizontal and vertical ("5 horizontal, 1 vertical").
label-header-decomposition = Poziomy falkowe
# machine-drafted, needs native review
# JPEG XS: the colour transform ("None", "Reversible colour transform (RCT)", "Star-Tetrix, RGGB mosaic").
label-header-colour-transform = Transformacja kolorów
# machine-drafted, needs native review
# Photoshop: the artboards, by name and size ("Phone (375 × 667), Tablet (768 × 1024)").
label-header-artboards = Obszary robocze
# machine-drafted, needs native review
# Photoshop file without a merged image, shown from its layers: the features it leaves out,
# by Photoshop's names ("Color Lookup, Knockout").
label-header-not-drawn = Nie rysowane
# machine-drafted, needs native review
# The same, for the features drawn only approximately ("Bevel & Emboss, Satin").
label-header-approximated = Rysowane w przybliżeniu

## Fonts (TTF, OTF, TTC/OTC, WOFF, WOFF2)

# machine-drafted, needs native review
group-font = Czcionka
label-font-family = Rodzina
label-font-style = Styl
# The face's place in a font collection: "2 of 4".
label-font-face = Krój
font-face-of = { $index } z { $count }
label-font-version = Wersja
label-font-designer = Projektant
# The font's maker (the name table's manufacturer).
label-font-manufacturer = Odlewnia
label-font-glyphs = Glify
label-font-outlines = Kontury
font-outlines-truetype = TrueType (kwadratowe)
font-outlines-cff = PostScript (CFF, sześcienne)
font-outlines-cff2 = PostScript (CFF2, sześcienne)
font-outlines-bitmap = Tylko bitmapy
font-outlines-none = Brak
# What the font's licence flags (OS/2 fsType) allow documents that embed it to do.
label-font-embedding = Osadzanie
font-embedding-installable = Można osadzać i instalować.
font-embedding-restricted = Nie można osadzać.
font-embedding-preview-print = Można osadzać tylko do podglądu i druku.
font-embedding-editable = Można osadzać w edytowanych dokumentach.
font-embedding-no-subsetting = Tylko w całości, nie jako podzbiór.
font-embedding-bitmap-only = Tylko jako bitmapy.
# Variation axes of a variable font: "Weight 100–900 (400)".
label-font-axes = Osie zmienności
font-axis = { $name } { $min }–{ $max } ({ $default })
label-font-instances = Nazwane style
label-font-colour = Glify kolorowe
font-colour-colr0 = Warstwy COLR
font-colour-colr1 = Gradienty COLR v1
font-colour-sbix = Bitmapy (sbix)
font-colour-cbdt = Bitmapy (CBDT)
font-colour-svg = SVG
label-font-weight = Grubość
label-font-units-per-em = Jednostki na em
label-font-tables = Tabele
label-font-licence = Licencja
label-font-licence-url = Adres URL licencji
label-font-copyright = Prawa autorskie
label-font-trademark = Znak towarowy

## Office documents (Word, Excel, PowerPoint, iWork, OpenDocument)

# machine-drafted, needs native review
group-document = Dokument
label-document-kind = Rodzaj
document-kind-word = Dokument Word
document-kind-excel = Skoroszyt Excel
document-kind-powerpoint = Prezentacja PowerPoint
document-kind-pages = Dokument Pages
document-kind-numbers = Arkusz kalkulacyjny Numbers
document-kind-keynote = Prezentacja Keynote
document-kind-odf-text = Tekst OpenDocument
document-kind-odf-spreadsheet = Arkusz kalkulacyjny OpenDocument
document-kind-odf-presentation = Prezentacja OpenDocument
document-kind-odf-graphics = Rysunek OpenDocument
document-kind-odf-other = Plik OpenDocument
# machine-drafted, needs native review
document-kind-playlist = Playlista
# machine-drafted, needs native review
label-playlist-items = Pozycje
# Appended to the kind: "Word document, template, with macros".
document-template = szablon
document-macros = z makrami
document-legacy = format Office 97–2003
label-document-preview = Podgląd
# The image format of the preview stored in the file, e.g. "JPEG image".
document-preview-image = Obraz { $format }
document-preview-none = Brak w tym pliku
label-document-subject = Temat
label-document-author = Autor
label-document-description = Opis
label-document-category = Kategoria
label-document-last-saved-by = Ostatnio zapisany przez
label-document-created = Utworzono
label-document-modified = Zmodyfikowano
label-document-printed = Ostatnio drukowano
label-document-pages = Strony
label-document-slides = Slajdy
label-document-hidden-slides = Ukryte slajdy
label-document-notes = Strony notatek
label-document-sheets = Arkusze
label-document-words = Wyrazy
label-document-characters = Znaki
label-document-paragraphs = Akapity
label-document-lines = Wiersze
label-document-editing-time = Czas edycji
document-minutes = { $minutes } min
label-document-revision = Poprawka
label-document-application = Zapisano w
label-document-company = Firma
label-document-template = Szablon

## Books (EPUB)

# machine-drafted, needs native review
group-book = Książka
label-book-authors = Autorzy
label-book-publisher = Wydawca
label-book-date = Wydano
label-book-language = Język
label-book-pages = Strony
# Entries in the book's table of contents.
label-book-contents = Pozycje spisu treści
label-book-layout = Układ
book-layout-reflowable = Płynny
book-layout-fixed = Stały układ
book-right-to-left = od prawej do lewej
label-book-version = Format
book-version = EPUB { $version }
label-book-fonts = Osadzone czcionki
book-fonts-obfuscated = { $count } (zaciemnione: { $obfuscated })
label-book-subjects = Tematy
label-book-identifier = Identyfikator
label-book-rights = Prawa

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
volume-orient-parrec = Kąt i przesunięcie Philips
volume-orient-ecat = Rozmiary wokseli i przesunięcia ECAT (niewiarygodne)
label-volume-technique = Technika
label-volume-version = Wersja formatu
label-volume-isotope = Izotop
label-volume-tracer = Znacznik
label-volume-units = Jednostki

# Text, tables and subtitles.
label-header-encoding = Zestaw znaków
label-header-lines = Wiersze
label-header-line-ends = Końce wierszy
label-header-words = Słowa
label-header-pages = Strony
label-header-pictures = Obrazy
label-header-pictures-not-shown = Niewyświetlone obrazy
label-header-tables = Tabele
label-header-rows = Wiersze tabeli
label-header-columns = Kolumny
label-header-delimiter = Separator
label-header-column-names = Nazwy kolumn
label-header-cues = Napisy
label-header-cue-span = Napisy od – do
