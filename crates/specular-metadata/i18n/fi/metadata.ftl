# machine-drafted, needs native review
# Specularin metatietopaneeli — suomi.
#
# Käytännöt
# - Luvut tulevat valmiiksi aluekohtaisesti muotoiltuina (ICU4X) merkkijonoina, esim.
#   $num = "2 456 789". Kun tarvitaan monikkomuotoa, raaka määrä välitetään
#   myös muuttujana $n (luku), jota käytetään vain valitsimena.
# - Päivämäärät ja kellonajat tulevat ICU4X:n valmiiksi muotoilemina ($date, $time).
# - Fluent ympäröi jokaisen lisätyn arvon Unicode-eristimillä.
# - Viestitunnisteet ovat pysyviä; älä koskaan käytä tunnistetta uudelleen eri merkityksessä.

## Yhteenvedon ryhmäotsikot

group-camera = Kamera
group-exposure = Valotus
group-lens = Objektiivi
group-date = Päivämäärä ja aika
group-location = Sijainti
# machine-drafted, needs native review
# A DJI drone photo: the gimbal's and the aircraft's attitude, the height above take-off.
group-drone = Drooni
label-gimbal = Gimbaali
label-flight = Ilma-alus
label-relative-altitude = Lähtöpaikan yläpuolella
drone-attitude = suunta { $yaw }, nousukulma { $pitch }, kallistus { $roll }
# A DJI thermal camera's radiometric JPEG: its photo opens, its temperatures can't.
label-thermal-data = Lämpötilat
drone-thermal-refused = Ei näytetä: DJI:n SDK-lisenssi ei salli sitä
drone-thermal-refused-detail = DJI-lämpötilojen lukeminen vaatii DJI:n Thermal SDK:n, jonka lisenssi sallii sen vain ohjelmille, jotka toimitetaan sen kanssa. Kuva ja sen metatiedot näytetään.
group-image = Kuva
group-file = Tiedosto
group-rights = Oikeudet ja kuvaus
group-credentials = Sisällön alkuperätiedot
# machine-drafted, needs native review
group-medical = Lääketieteellinen kuva
# machine-drafted, needs native review
group-inspection = Komponentti ja tarkastus
group-nde-image = Tarkastuskuva (DICONDE)
# machine-drafted, needs native review
# Industrial CT volumes (Zeiss Xradia, Amira, Bruker SkyScan, Nikon X-Tek, VGI, raw volumes).
group-industrial-image = Tarkastuskuva
group-science = Tieteellinen kuva
group-em = Elektronimikroskopia
group-spm = Pyyhkäisyanturimikroskopia
group-slide = Objektilasi
group-microscopy = Mikroskopia
# X-ray diffraction detector frames (CBF, EDF, SMV, Bruker, MarCCD, R-AXIS).
group-diffraction = Diffraktio
group-surface = Pinnan topografia
group-ebsd = Takaisinsironneiden elektronien diffraktio
group-seismic = Seisminen data
# machine-drafted, needs native review
group-asdf = ASDF
# machine-drafted, needs native review
# CERN ROOT files (particle physics histograms and graphs).
group-root = ROOT
# machine-drafted, needs native review
# CASA images (radio interferometry: the .image, .residual, .model directories tclean writes).
group-casa = CASA-kuva
# machine-drafted, needs native review
# GIFTI files: brain surfaces and the values on their vertices (Human Connectome Project
# and FreeSurfer exports).
group-gifti = GIFTI
# CIFTI-2 files: maps over cortical surfaces and subcortical voxels ("grayordinates").
group-cifti = CIFTI
# White-matter fibre tracts (TrackVis, MRtrix).
group-tractography = Traktografia
# machine-drafted, needs native review
group-annotations = Annotaatiot
# Imaging mass spectrometry (imzML): a mass spectrum per pixel.
group-mass-spec = Massaspektrometrinen kuvantaminen
# machine-drafted, needs native review
group-thermal = Lämpökuva
# machine-drafted, needs native review
# Flow cytometry list-mode data (FCS): events of a cell sorter or analyser.
group-cytometry = Virtaussytometria
# machine-drafted, needs native review
group-spectroscopy = Spektroskopia
# machine-drafted, needs native review
# Unstructured meshes of ocean, coastal and climate models (UGRID, ICON, MPAS in NetCDF).
group-ugrid = Rakenteeton hila
group-drawing = Piirustus
# machine-drafted, needs native review
group-volume = Tilavuus

## Yhteenvedon kohteiden nimet

label-camera = Kamera
label-software = Ohjelmisto
label-application = Sovellus
label-shutter = Suljinaika
label-aperture = Aukko
label-iso = ISO
label-focal-length = Polttoväli
label-exposure-bias = Valotuksen korjaus
label-program = Tila
label-metering = Mittaus
label-white-balance = Valkotasapaino
label-flash = Salama
label-lens = Objektiivi
label-lens-make = Objektiivin valmistaja
label-lens-range = Objektiivin alue
# machine-drafted, needs native review
label-focus-mode = Tarkennustila
label-stabilisation = Kuvanvakautus
label-taken = Otettu
label-digitised = Digitoitu
label-edited = Viimeksi muokattu
label-place = Paikka
label-coordinates = Koordinaatit
label-altitude = Korkeus
label-direction = Suunta
label-accuracy = Tarkkuus
label-gps-time = GPS-aika
label-dimensions = Mitat
label-encoding = Koodaus
# machine-drafted, needs native review
label-motion = Liike
motion-kind-motion-photo = Liikekuva
motion-kind-micro-video = Liikekuva (MicroVideo)
motion-kind-samsung = Samsungin liikekuva
motion-kind-live-photo = Live Photo
motion-duration = { $seconds } s
label-colour-profile = Väriprofiili
label-orientation = Suunta
label-file-name = Nimi
label-folder = Kansio
label-file-size = Koko
label-format = Muoto
label-modified = Muokattu
label-title = Otsikko
label-description = Kuvaus
label-creator = Tekijä
label-copyright = Tekijänoikeus
label-keywords = Avainsanat
label-rating = Arvio
label-colour-label = Värimerkki
label-credentials = Sisällön alkuperätiedot

## ”Kaikki kentät” -ryhmäotsikot (tunnisteiden nimet pysyvät standardien mukaisina)

fields-ifd0 = IFD0 (pääkuva)
fields-exif = Exif
fields-gps = GPS
fields-interop = Yhteentoimivuus
fields-ifd1 = IFD1 (pikkukuva)
fields-icc = ICC-profiilin otsake
# $namespace: xmp-namespace-viestistä; $prefix: esim. "dc"
fields-xmp = XMP: { $namespace } ({ $prefix })
xmp-namespace = { $prefix ->
    [dc] Dublin Core
    [xmp] Perustiedot
    [xmpRights] Oikeudet
    [xmpMM] Median hallinta
    [xmpDM] Dynaaminen media
    [tiff] TIFF
    [exif] Exif
    [exifEX] Exif 2.3+
    [aux] Kameran lisävarusteet
    [photoshop] Photoshop
    [crs] Camera Raw -asetukset
    [lr] Lightroom
    [Iptc4xmpCore] IPTC Core
    [Iptc4xmpExt] IPTC Extension
    [plus] PLUS-lisensointi
    [GPano] Panoraamapallo
    [GCamera] Google-kamera
    [hdrgm] HDR-vahvistuskartta
    [mwg-rs] Alueet
    [pdf] PDF
    [stEvt] Resurssitapahtumat
    [stRef] Resurssiviittaukset
   *[other] Muu
}
# $number: "0xA500"; $context: "Exif", "GPS", …
unknown-tag = Tuntematon tunniste { $number } ({ $context })
blob-bytes = { $num ->
    [one] ({ $num } tavu)
   *[other] ({ $num } tavua)
}
maker-note-bytes = ({ $num ->
        [one] { $num } tavu
       *[other] { $num } tavua
    }, valmistajan huomautus, ei purettu)
more-values = { $shown }, … ({ $total ->
        [one] { $total } arvo
       *[other] { $total } arvoa
    })

## Yleiset liitokset

# Erottaa koodissa muodostetun luettelon osat (salaman kuvaus, paikan alueet).
list-separator = {", "}
# Erottaa suljinajan · aukon · ISO:n · polttovälin yhden rivin yhteenvedossa.
one-line-separator = {" · "}

## Valotus

exposure-time-fraction = 1/{ $denominator } s
exposure-time-seconds = { $seconds } s
aperture-value = f/{ $f }
iso-value = ISO { $iso }
focal-length-value = { $mm } mm
focal-length-with-equiv = { $mm } mm (vastaa { $equiv } mm)
focal-length-equiv-only = vastaa { $equiv } mm
# $ev on itseisarvo; etumerkki tulee valitsimesta.
exposure-bias-value = { $sign ->
    [plus] +{ $ev } EV
    [minus] −{ $ev } EV
   *[zero] 0 EV
}
exposure-program = { $value ->
    [1] Käsisäätö
    [2] Ohjelma (automaatti)
    [3] Aukon esivalinta
    [4] Suljinajan esivalinta
    [5] Luova (syväterävyys)
    [6] Toiminta (lyhyt suljinaika)
    [7] Muotokuva
    [8] Maisema
   *[other] Tuntematon
}
metering-mode = { $value ->
    [1] Keskiarvo
    [2] Keskustapainotteinen
    [3] Piste
    [4] Monipiste
    [5] Matriisi (arvioiva)
    [6] Osa-alue
   *[other] Muu
}
# machine-drafted, needs native review
# Canon maker note CameraSettings FocusMode; 256–258 are the live-view variants of 0–2.
canon-focus-mode = { $value ->
    [0] One-Shot AF
    [1] AI Servo AF
    [2] AI Focus AF
    [3] Käsitarkennus
    [4] Kerta
    [5] Jatkuva
    [6] Käsitarkennus
    [16] Pan focus
    [256] One-Shot AF (Live View)
    [257] AI Servo AF (Live View)
    [258] AI Focus AF (Live View)
    [512] Movie snap -tarkennus
    [519] Movie Servo AF
   *[other] Muu
}
# Canon maker note CameraSettings ImageStabilization; 256–260 repeat 0–4.
canon-stabilisation = { $value ->
    [0] Pois
    [1] Päällä
    [2] Vain kuvattaessa
    [3] Panorointi
    [4] Dynaaminen
    [256] Pois
    [257] Päällä
    [258] Vain kuvattaessa
    [259] Panorointi
    [260] Dynaaminen
   *[other] Muu
}
# machine-drafted, needs native review
panasonic-focus-mode = { $value ->
    [1] Automaattinen
    [2] Käsitarkennus
    [4] Automaattinen (tarkennuspainike)
    [5] Automaattinen (jatkuva)
    [6] AF-S
    [7] AF-C
    [8] AF-F
   *[other] Muu
}
# machine-drafted, needs native review
panasonic-stabilisation = { $value ->
    [2] Päällä (optinen)
    [3] Pois
    [4] Päällä (tila 2)
    [5] Päällä (optinen, panorointi)
    [6] Päällä (vain runko)
    [7] Päällä (vain runko, panorointi)
    [9] Dual IS
    [10] Dual IS (panorointi)
    [11] Dual IS 2
    [12] Dual IS 2 (panorointi)
   *[other] Muu
}
label-comment = Kommentti
fields-tags = { $kind }-tagit
group-picture = Kuva-asetukset
fields-fujifilm = Fujifilmin valmistajatiedot
label-film-simulation = Filmisimulaatio
label-dynamic-range = Dynaaminen alue
label-highlight-tone = Valoalueet
label-shadow-tone = Varjot
label-colour-setting = Väri
label-sharpness = Terävyys
label-noise-reduction = Kohinanvaimennus
label-clarity = Selkeys
label-grain-effect = Rakeisuustehoste
label-colour-chrome = Värikromi
label-colour-chrome-blue = Värikromi sininen
label-shutter-type = Suljintyyppi
label-drive-mode = Kuvaustapa
fuji-step = { $sign ->
    [plus] +{ $n }
    [minus] −{ $n }
   *[zero] 0
}
fuji-film-mode = { $value ->
    [0] Provia (vakio)
    [256] Studiomuotokuva
    [272] Studiomuotokuva (kylläisempi)
    [288] Astia (pehmeä)
    [304] Studiomuotokuva (terävämpi)
    [512] Velvia (eloisa)
    [768] Studiomuotokuva Ex
    [1024] Velvia
    [1280] Pro Neg. Std
    [1281] Pro Neg. Hi
    [1536] Classic Chrome
    [1792] Eterna
    [2048] Classic Neg.
    [2304] Eterna Bleach Bypass
    [2560] Nostalgic Neg.
    [2816] Reala Ace
   *[other] Muu
}
fuji-monochrome = { $value ->
    [768] Mustavalko
    [769] Mustavalko, punasuodin
    [770] Mustavalko, keltasuodin
    [771] Mustavalko, vihreäsuodin
    [784] Seepia
    [1280] Acros
    [1281] Acros, punasuodin
    [1282] Acros, keltasuodin
    [1283] Acros, vihreäsuodin
   *[other] Mustavalko
}
fuji-white-balance = { $value ->
    [0] Automaattinen
    [1] Automaattinen (valkoisen etusija)
    [2] Automaattinen (tunnelman etusija)
    [256] Päivänvalo
    [512] Pilvinen
    [768] Päivänvaloloisteputki
    [769] Päivänvalkoinen loisteputki
    [770] Valkoinen loisteputki
    [771] Lämmin valkoinen loisteputki
    [772] Lämmin valkoinen kodin loisteputki
    [1024] Hehkulamppu
    [1280] Salama
    [1536] Vedenalainen
    [3840] Mukautettu 1
    [3841] Mukautettu 2
    [3842] Mukautettu 3
    [3843] Mukautettu 4
    [3844] Mukautettu 5
    [4080] Värilämpötila
   *[other] Muu
}
fuji-white-balance-kelvin = { $kelvin } K
fuji-dynamic-range = DR{ $percent }
fuji-dynamic-range-auto = Automaattinen (DR{ $percent })
fuji-dynamic-range-setting = { $value ->
    [0] Automaattinen
    [256] Vakio (DR100)
    [512] Laaja 1 (DR230)
    [513] Laaja 2 (DR400)
   *[other] Muu
}
fuji-effect = { $value ->
    [0] Pois
    [32] Heikko
    [64] Voimakas
   *[other] Muu
}
fuji-grain-size = { $value ->
    [16] pieni
    [32] suuri
   *[other] muu
}
fuji-grain = { $strength }, { $size }
fuji-shutter-type = { $value ->
    [0] Mekaaninen
    [1] Elektroninen
    [2] Elektroninen (pitkät valotukset)
    [3] Elektroninen ensimmäinen verho
   *[other] Muu
}
fuji-drive-mode = { $value ->
    [0] Yksittäiskuva
    [1] Sarja, hidas
    [2] Sarja, nopea
   *[other] Muu
}
fuji-focus-mode = { $value ->
    [0] Käsin
    [1] AF-S
    [2] AF-C
    [100] Automaattinen
    [101] Käsin
   *[other] Muu
}
fuji-stabilisation = { $kind } ({ $mode })
fuji-stabilisation-kind = { $value ->
    [0] Ei
    [1] Optinen
    [2] Kennon siirto
    [3] Objektiivi
    [258] Kennon siirto ja digitaalinen
    [512] Digitaalinen
   *[other] Muu
}
fuji-stabilisation-mode = { $value ->
    [0] Pois
    [1] päällä, jatkuva
    [2] päällä, kuvattaessa
   *[other] Muu
}
light-source = { $value ->
    [1] Päivänvalo
    [2] Loisteputki
    [3] Hehkulamppu
    [4] Salama
    [9] Kaunis sää
    [10] Pilvinen
    [11] Varjo
    [12] Päivänvaloloisteputki
    [13] Päivänvalkoinen loisteputki
    [14] Kylmänvalkoinen loisteputki
    [15] Valkoinen loisteputki
    [16] Lämpimänvalkoinen loisteputki
    [17] Vakiovalo A
    [18] Vakiovalo B
    [19] Vakiovalo C
    [20] D55
    [21] D65
    [22] D75
    [23] D50
    [24] Studiohehkulamppu
   *[other] Muu
}
white-balance-auto = Automaattinen
white-balance-manual = Käsisäätö
white-balance-manual-with-light = Käsisäätö ({ $light })
# Salaman kuvaus muodostetaan näistä osista list-separator-erottimella.
flash-none = Ei salamatoimintoa
flash-fired = Salama laukesi
flash-not-fired = Salama ei lauennut
flash-forced-on = pakotettu päälle
flash-forced-off = pakotettu pois
flash-auto = automaattitila
flash-return-not-detected = heijastusta ei havaittu
flash-return-detected = heijastus havaittu
flash-red-eye = punasilmäisyyden vähennys

## Objektiivi

lens-focal-single = { $mm } mm
lens-focal-range = { $min }–{ $max } mm
lens-aperture-single = f/{ $f }
lens-aperture-range = f/{ $min }–{ $max }
lens-range = { $focal } { $aperture }

## Päivämäärä ja aika

# $date: "tiistai 14. toukokuuta 2024"; $time: "18.32"
# $weekday: "tiistai" (ICU); $date: "14. toukokuuta 2024" (ICU)
date-with-weekday = { $weekday } { $date }
date-time = { $date } klo { $time }
date-time-zone = { $datetime } ({ $zone })
zone-utc = UTC
# $offset: "+1", "+5:30", "−3"
zone-offset = UTC{ $offset }
gps-date-time = { $date } klo { $time } UTC
detail-zone-from-gps = Aikavyöhyke päätelty GPS-kellosta

## Sijainti

# $lat ja $lon ovat etumerkittömiä; pallonpuolisko tulee valitsimesta.
coordinates = { $lat }° { $latRef ->
    [south] E
   *[north] P
}, { $lon }° { $lonRef ->
    [west] L
   *[east] I
}
altitude = { $ref ->
    [above] { $m } m merenpinnan yläpuolella
    [below] { $m } m merenpinnan alapuolella
   *[sea] Merenpinnan tasolla
}
compass-point = { $point ->
    [n] pohjoinen
    [nne] pohjoiskoillinen
    [ne] koillinen
    [ene] itäkoillinen
    [e] itä
    [ese] itäkaakko
    [se] kaakko
    [sse] eteläkaakko
    [s] etelä
    [ssw] etelälounas
    [sw] lounas
    [wsw] länsilounas
    [w] länsi
    [wnw] länsiluode
    [nw] luode
   *[nnw] pohjoisluode
}
# $point: compass-point-viestistä; $degrees: "287"
facing = Suunta: { $point } ({ $degrees }°)
detail-true-north = Suhteessa maantieteelliseen pohjoiseen
detail-magnetic-north = Suhteessa magneettiseen pohjoiseen
accuracy = ±{ $m } m
distance-km = { $km } km
place = { $city }, { $country }
place-city-only = { $city }
# Yli 30 km:n päässä löydetystä kaupungista.
place-near = lähellä: { $place }
place-detail = { $areas } · { $distance } paikasta { $city }
place-detail-distance = { $distance } paikasta { $city }

## Kuva

dimensions = { $width } × { $height }
dimensions-megapixels = { $width } × { $height } ({ $mp } MP)
encoding-bit-depth = { $bits }-bittinen
# $format: "8-bittinen YCbCr 4:2:0"
encoding-progressive = { $format }, progressiivinen
encoding-baseline = { $format }, perus
encoding-hierarchical = { $format }, hierarkkinen
# machine-drafted, needs native review
encoding-alpha = { $format }, { $alpha ->
    [premultiplied] esikerrottu alfa
    [transparency] läpinäkyvyys
   *[alpha] alfa
}
# [Lab], [Palette] and the variants after them: machine-drafted, needs native review
colour-model = { $model ->
    [YCbCr] YCbCr
    [RGB] RGB
    [Greyscale] Harmaasävy
    [CMYK] CMYK
    [YCCK] YCCK
    [Lab] L*a*b*
    [Palette] Indeksoidut värit
    [Multichannel] Monikanavainen
    [Duotone] Duotone
    [YCgCo] YCgCo
    [XYB] XYB
    [YC] Luminanssi/krominanssi (YC)
    [Bayer] Bayer-raaka
    [X-Trans] X-Trans-raaka
    [CFA] Värisuodinmatriisiraaka
    [LinearRaw] Lineaarinen raaka
    [RGBE] RGBE (jaettu eksponentti)
    [XYZE] XYZE (jaettu eksponentti)
    [R] Yksi kanava (R)
    [RG] Kaksi kanavaa (RG)
   *[other] { $model }
}
adobe-rgb-from-exif = Adobe RGB (EXIF-tunnisteesta; ei upotettua profiilia)
orientation = { $value ->
    [1] Pystyssä (ei kiertoa)
    [2] Peilattu vaakasuunnassa (otettu huomioon)
    [3] Kierretty 180° (otettu huomioon)
    [4] Peilattu pystysuunnassa (otettu huomioon)
    [5] Peilattu vaakasuunnassa ja kierretty 90° vastapäivään (otettu huomioon)
    [6] Kierretty 90° myötäpäivään (otettu huomioon)
    [7] Peilattu vaakasuunnassa ja kierretty 90° myötäpäivään (otettu huomioon)
    [8] Kierretty 90° vastapäivään (otettu huomioon)
   *[other] Tuntematon ({ $value })
}
# Lyhyt muoto varoituksiin.
orientation-short = { $value ->
    [1] Pystyssä
    [2] Peilattu vaakasuunnassa
    [3] Kierretty 180°
    [4] Peilattu pystysuunnassa
    [5] Peilattu vaakasuunnassa ja kierretty 90° vastapäivään
    [6] Kierretty 90° myötäpäivään
    [7] Peilattu vaakasuunnassa ja kierretty 90° myötäpäivään
    [8] Kierretty 90° vastapäivään
   *[other] arvo { $value }
}

## Tiedosto

file-size-bytes = { $num ->
    [one] { $num } tavu
   *[other] { $num } tavua
}
# Desimaaliyksiköt (1 KB = 1000 tavua).
file-size-unit = { $num } { $unit ->
    [kb] KB
    [mb] MB
    [gb] GB
    [tb] TB
   *[pb] PB
}

## Oikeudet

rating-rejected = Hylätty
# $stars: "★★★★☆"
rating-stars = { $stars } ({ $n }/5)

## Sisällön alkuperätiedot

c2pa-present = Mukana (ei tarkistettu)
c2pa-detail = Tiedostossa on C2PA-luettelo; sen allekirjoitusta ei ole tarkistettu.

## ICC-otsakkeen arvot

icc-class = { $class ->
    [scnr] Syöttölaite
    [mntr] Näyttölaite
    [prtr] Tulostuslaite
    [link] Laitelinkki
    [spac] Väriavaruus
    [abst] Abstrakti
    [nmcl] Nimetty väri
   *[other] Tuntematon
}
icc-intent = { $intent ->
    [0] Havainnollinen
    [1] Suhteellinen kolorimetrinen
    [2] Kylläisyys
    [3] Absoluuttinen kolorimetrinen
   *[other] Tuntematon
}
icc-platform = { $platform ->
    [APPL] Apple
    [MSFT] Microsoft
    [SGI] Silicon Graphics
    [SUNW] Sun Microsystems
    [none] Ei määritetty
   *[other] Tuntematon
}
icc-coded = { $name } ({ $code })

# machine-drafted, needs native review
label-modality = Modaliteetti
label-body-part = Kehonosa
label-study = Tutkimus
label-series = Sarja
label-slice-thickness = Leikepaksuus
label-pixel-spacing = Pikseliväli
label-imager-pixel-spacing = Pikseliväli ilmaisimella
label-frames = Kehykset
label-transfer-syntax = Siirtosyntaksi
label-document = Asiakirja
label-document-type = Asiakirjan tyyppi
medical-spacing = { $row } × { $column } mm
medical-mm = { $mm } mm
medical-lossy = { $name } (häviöllinen pakkaus)
detail-pixel-spacing = Pikselien keskipisteiden etäisyys potilaassa (rivit × sarakkeet).
# machine-drafted, needs native review
# Pixel spacing of an industrial inspection image (DICONDE, CT of an object).
detail-pixel-spacing-object = Pikselien keskipisteiden etäisyys kohteessa (rivit × sarakkeet).
detail-imager-pixel-spacing = Mitattu ilmaisimelta, suurennusta ei ole korjattu: etäisyydet potilaassa ovat yleensä pienempiä.

## Teollinen tarkastus (DICONDE)

# machine-drafted, needs native review
label-nde-component-shape = Komponentin muoto
label-nde-component-manufacturer = Komponentin valmistaja
label-nde-material = Materiaali
label-nde-thickness = Materiaalin paksuus
label-nde-pipe-diameter = Putken halkaisija
label-nde-source = Lähde
label-nde-environment = Ympäristö
label-nde-iqi = Kuvanlaatuindikaattori
label-nde-indications = Indikaatiot
label-nde-standard = Standardi

## Tutkimustilavuudet (NIfTI, Analyze, NRRD, MetaImage)

# machine-drafted, needs native review
label-voxel-size = Vokselin koko
label-data-type = Tietotyyppi
label-time-points = Aikapisteet
label-orientation-source = Suunta lähteestä
label-intent = Arvot
label-compression = Pakkaus
volume-voxel-size = { $x } × { $y } × { $z } mm
volume-orient-sform = NIfTI sform
volume-orient-qform = NIfTI qform
volume-orient-analyze = Analyze-otsake (epäluotettava)
volume-orient-space = NRRD-avaruussuunnat
volume-orient-transform = MetaImage-muunnos
volume-orient-spacings = Vain vokseliväli
volume-orient-none = Ei ilmoitettu
volume-orient-vox2ras = FreeSurfer vox2ras
volume-orient-mgh-default = FreeSurferin oletus (epäluotettava)
volume-orient-minc = MINC-ulottuvuuksien suuntakosinit
volume-orient-object = Kohteen koordinaatit (ei potilaan suuntausta)
volume-content-projections = Projektiot (raakakuvaus)
volume-content-reconstruction = Rekonstruoitu tilavuus
volume-coords-rectilinear = Suorakulmainen (epätasainen väli, näytetään tasaisena)
# Scan parameters and tags a FreeSurfer MGH file keeps after its voxels.
label-volume-tr = Toistoaika
label-volume-te = Kaikuaika
label-volume-ti = Inversioaika
label-volume-flip = Kääntökulma
label-volume-fov = Kuvakenttä
label-volume-field = Kentänvoimakkuus
label-volume-pedir = Vaihekoodaus
label-volume-cmdline = Komentorivi
label-volume-xform = Talairach-muunnos
label-volume-content = Sisältö
label-volume-projections = Projektiot
label-volume-angles = Kiertoalue
label-volume-angle-step = Kiertoaskel
label-volume-kv = Putkijännite
label-volume-current = Putkivirta
label-volume-power = Lähteen teho
label-volume-exposure = Valotus
label-volume-sod = Lähteestä kohteeseen
label-volume-odd = Kohteesta ilmaisimeen
label-volume-sdd = Lähteestä ilmaisimeen
label-volume-magnification = Optinen suurennus
label-volume-binning = Binning
label-volume-objective = Objektiivi
label-volume-filter = Lähteen suodatin
label-volume-camera = Ilmaisin
label-volume-instrument = Laite
label-volume-software = Ohjelmisto
label-volume-date = Kuvattu
label-volume-ring = Rengasartefaktien korjaus
label-volume-smoothing = Tasoitus
label-volume-coords = Ruudukko
# machine-drafted, needs native review
# An eye scan (OCT): which eye.
label-volume-eye = Silmä
volume-eye-right = Oikea (OD)
volume-eye-left = Vasen (OS)
detail-volume-unreliable = Tiedosto ei kerro, mihin sen akselit osoittavat potilaassa: vasen ja oikea voivat olla vaihtuneet, joten suuntamerkintöjä ei näytetä.
# machine-drafted, needs native review
# An industrial CT volume: its axes are the scanner's, there is no patient.
detail-volume-object = Akselit ovat skannerin eivätkä potilaan, joten suuntamerkintöjä ei näytetä.

## Tieteelliset kuvat (FITS, XISF)

label-object = Kohde
label-telescope = Kaukoputki
label-instrument = Instrumentti
label-filter = Suodin
label-observed = Havaittu
label-exposure-time = Valotus
label-sample = Näytteet
label-unit = Yksikkö
label-hdu = Laajennus
label-centre = Keskipiste
label-centre-deg = Keskipiste (asteina)
label-frame = Koordinaatisto
# machine-drafted, needs native review
science-frame-galactic = Galaktinen
science-frame-ecliptic = Ekliptinen
label-projection = Projektio
label-pixel-scale = Pikseliskaala
science-seconds = { $seconds } s
science-arcsec-px = { $arcsec }″ pikseliä kohden
label-distortion = Vääristymä
label-wcs = Taivaankoordinaatit
science-wcs-unsupported = Ei tuettu: { $reason }
# machine-drafted, needs native review
science-wcs-approximate = Likimääräinen (vain osoitus, ei vääristymää)
label-observer = Havaitsija
label-bayer = Bayer-kuvio
label-capture-start = Tallennuksen alku (UTC)
label-capture-end = Tallennuksen loppu (UTC)
label-frame-rate = Kuvanopeus
science-fps = { $fps } kuvaa/s
science-frames-cut = { $frames }/{ $declared }
# The HDF5 dataset or NetCDF variable shown.
label-dataset = Tietojoukko
label-pixel-size = Pikselikoko
# "0.25 nm per pixel"; $size may be "0.25 × 0.5".
science-pixel-size = { $size } { $unit } pikseliä kohden
# Microscopy (DigitalMicrograph, MRC, Gwyddion, Igor).
label-microscope = Mikroskooppi
label-voltage = Kiihdytysjännite
label-magnification = Suurennos
label-imaging-mode = Tila
label-detector = Ilmaisin
label-acquired = Tallennettu
label-field-of-view = Näkökenttä
science-kilovolts = { $kv } kV
science-magnification = { $value }×
label-vendor = Valmistaja
label-scanner = Skanneri
label-objective = Objektiivi
label-pyramid-levels = Tasot
# machine-drafted, needs native review
label-emissivity = Emissiivisyys
label-object-distance = Kohteen etäisyys
label-reflected-temperature = Heijastunut lämpötila
label-atmospheric-temperature = Ilman lämpötila
label-humidity = Suhteellinen kosteus
label-raw-range = Raaka-arvojen alue
label-temperature-range = Lämpötila-alue
thermal-temperature = { $value } { $unit }
thermal-metres = { $value } m
thermal-percent = { PERCENT($value) }
thermal-range = { $low } – { $high }
slide-levels = { $count }: { $sizes }
# X-ray diffraction: the experiment geometry from a detector frame's header.
label-wavelength = Aallonpituus
# Ångström, e.g. "0.97625 Å".
xtal-angstrom = { $value } Å
label-photon-energy = Fotonin energia
xtal-kev = { $value } keV
# Sample to detector distance.
label-detector-distance = Ilmaisimen etäisyys
xtal-mm = { $value } mm
# Where the direct beam meets the detector, in pixels from the image's top-left corner.
label-beam-centre = Säteen keskipiste
xtal-beam-centre = x { $x }, y { $y } px
# The detector arm's swing angle (2θ).
label-detector-swing = Ilmaisimen 2θ
xtal-degrees = { $value }°
# Goniometer angles (Greek letters are the field's usual names).
label-phi = Fii (φ)
label-omega = Oomega (ω)
label-chi = Khii (χ)
label-kappa = Kappa (κ)
# The sample's rotation during the exposure: start angle plus width, e.g. "12.5° + 0.1°".
label-oscillation = Oskillaatio
xtal-oscillation = { $start }° + { $width }°
# $axis is the axis name from the file, e.g. "phi" or "omega".
xtal-oscillation-axis = { $start }° + { $width }° akselin { $axis } ympäri
# The count at which a pixel saturates.
label-overload = Ylivuotoraja
# The finest lattice spacing (d) recorded at the detector's edge and in its corners.
label-resolution = Erotuskyky
xtal-resolution = { $edge } Å reunalla, { $corner } Å kulmissa
label-geometry = Geometria
xtal-curved = Kaareva kuvalevy: ei q:ta, d:tä eikä erotuskykyrenkaita
xtal-incomplete = Puutteellinen otsake: ei q:ta, d:tä eikä erotuskykyrenkaita
# All-fields group title: "FITS header".
fields-science-header = { $format }-otsake
# machine-drafted, needs native review
fields-asdf-tree = ASDF-puu
label-asdf-model = Datamalli
label-asdf-version = Tiedostoversio
label-asdf-standard = ASDF-standardi
label-asdf-library = Kirjoittanut
# machine-drafted, needs native review
# All-fields group title for a CERN ROOT file's list of keys (objects).
fields-root-keys = ROOT-avaimet
# CERN ROOT files: the ROOT version that wrote the file (e.g. "6.28/04"), its compression,
# the number of keys (stored objects), and the object shown: class (e.g. "TH2F"), its path in
# the file, title, number of entries and sum of weights.
label-root-version = ROOT-versio
label-root-compression = Pakkaus
label-root-keys = Avaimet
label-root-class = Luokka
label-root-name = Nimi
label-root-title = Otsikko
label-root-entries = Merkinnät
label-root-sum-weights = Painojen summa
# A graph's number of points.
label-root-points = Pisteet
# $axis is "x", "y" or "z".
label-root-axis = Akseli { $axis }
# $quantity is the axis title with its unit, e.g. "Energy [GeV]"; $min and $max the axis
# limits.
root-axis-fixed = { $quantity }: { $bins ->
        [one] { $bins } bini
       *[other] { $bins } biniä
    }, { $min } – { $max }
root-axis-variable = { $quantity }: { $bins ->
        [one] { $bins } vaihtelevanlevyinen bini
       *[other] { $bins } vaihtelevanlevyistä biniä
    }, { $min } – { $max }
label-root-mean-rms = Keskiarvo ja RMS { $axis }
root-mean-rms = { $mean } ± { $rms }
# Trees (event data) are listed only, with their entry counts.
label-root-trees = Puut
root-tree = { $name } ({ $entries ->
        [one] { $entries } merkintä
       *[other] { $entries } merkintää
    })
# machine-drafted, needs native review
# CASA images: what the image is (from its directory name), its restoring beam (major and
# minor axis, position angle), the beams of its planes when they differ, the rest frequency,
# spectral reference frame (e.g. "LSRK") and velocity definition of the frequency labels,
# the default mask (e.g. "mask0") and the casacore storage manager with its byte order.
label-casa-type = Kuvan tyyppi
casa-type-image = Palautettu kuva
casa-type-residual = Residuaali
casa-type-model = Malli
casa-type-pb = Primäärikeila
casa-type-mask = Clean-maski
casa-type-psf = Pistelevityfunktio
casa-type-sumwt = Painojen summa
casa-type-weight = Painot
label-casa-beam = Palautuskeila
casa-beam = { $major }″ × { $minor }″, PA { $pa }°
label-casa-plane-beams = Tasokohtaiset keilat
casa-plane-beams = { $count ->
        [one] { $count } keila
       *[other] { $count } keilaa
    }, isoakseli { $min }″–{ $max }″
label-casa-rest-frequency = Lepotaajuus
casa-ghz = { $ghz } GHz
label-casa-spectral-frame = Spektrinen vertailukehys
label-casa-velocity = Nopeuden määritelmä
casa-velocity-radio = Radio
casa-velocity-optical = Optinen
casa-velocity-relativistic = Relativistinen
label-casa-mask = Oletusmaski
label-casa-storage = Tallennus
casa-storage-big = { $manager }, big-endian
casa-storage-little = { $manager }, little-endian
# machine-drafted, needs native review
# Unstructured meshes: the mesh's name in the file, the convention describing it (UGRID,
# ICON, MPAS), its counts, face shapes, coordinates and where the values sit.
label-ugrid-name = Hila
label-ugrid-convention = Käytäntö
label-ugrid-nodes = Solmut
label-ugrid-faces = Tahkot
label-ugrid-edges = Reunat
label-ugrid-shapes = Tahkojen muodot
label-ugrid-coordinates = Koordinaatit
label-ugrid-values = Arvojen sijainti
label-ugrid-split = Jaettu piirtämistä varten
# $n is the formatted count, $count the number (for the plural).
ugrid-triangles = { $count ->
    [one] { $count } kolmio
   *[other] { $count } kolmiota
}
ugrid-quads = { $count ->
    [one] { $count } nelikulmio
   *[other] { $count } nelikulmiota
}
# Faces of five or more nodes; $max is the most nodes of one face.
ugrid-polygons = { $count ->
    [one] { $count } monikulmio, { $max } solmua
   *[other] { $count } monikulmiota, enintään { $max } solmua
}
ugrid-lonlat = Pituus- ja leveysaste
ugrid-projected = Projisoidut x ja y
ugrid-projected-unit = Projisoidut x ja y ({ $unit })
ugrid-on-faces = Tahkot (tasavärisinä)
ugrid-on-nodes = Solmut (interpoloituna tahkojen yli)
# A mesh file without data variables: the image shows how many nodes each face has.
ugrid-grid-only = Ei mitään: kuva näyttää solmujen määrän tahkoa kohden
# Faces cut at the antimeridian or closed to a pole so they draw on a flat map.
ugrid-split = { $count ->
    [one] { $count } tahko antimeridiaanilla tai navalla
   *[other] { $count } tahkoa antimeridiaanilla tai navalla
}
# machine-drafted, needs native review
# Slide annotations (NDP.view, Aperio ImageScope, ASAP sidecar files).
label-annotation-file = Tiedosto
# "slide.xml (Aperio ImageScope)"
annotation-source = { $file } ({ $format })
label-annotation-shapes = Muodot
label-annotation-groups = Ryhmät
# Shapes or points over the reader's limits, left out.
label-annotation-skipped = Jätetty pois
annotation-polygon = Monikulmio
annotation-polyline = Viiva
annotation-rectangle = Suorakulmio
annotation-ellipse = Ellipsi
annotation-point = Piste
annotation-points = Pisteet
annotation-ruler = Viivain
annotation-arrow = Nuoli
# The list of annotations is cut short: "and 1,200 more".
annotation-more = ja { $count } muuta
# All-fields group title: "Annotations in slide.xml".
fields-annotations-header = Annotaatiot: { $file }
label-drawing-version = Versio
label-drawing-units = Yksiköt
label-drawing-extents = Laajuus
label-drawing-sheet = Arkki
label-drawing-layouts = Asettelut
label-drawing-layers = Tasot
label-drawing-entities = Objektit
label-drawing-codepage = Koodisivu
label-drawing-missing-fonts = Korvatut fontit
# machine-drafted, needs native review
# A font file the drawing names that was not found: the styles naming it, and the font drawing its text instead.
drawing-missing-font = { $file } (tyylit { $styles }), korvattu fontilla { $substitute }
# A Big Font (double-byte text) that was not found; its characters come from other fonts.
drawing-missing-bigfont = { $file } (tyylit { $styles }), korvattu varafonteilla
# Survey data (LandXML).
label-survey-crs = Koordinaattijärjestelmä
survey-crs-none = Ei ilmoitettu
label-survey-project = Projektin nimi
label-survey-application = Kirjoittanut
label-survey-surface = Maastomalli
# A TIN surface: its name, counts, plan and sloped areas, the volume above its lowest point, its elevations and the contour interval drawn; unit is "m" or "ft" (areas in square units, volume in cubic units).
survey-surface = { $name }: { $points ->
        [one] { $points } piste
       *[other] { $points } pistettä
    }, { $triangles ->
        [one] { $triangles } kolmio
       *[other] { $triangles } kolmiota
    }; pinta-ala tasossa { $area } { $unit }², todellinen pinta-ala { $area3d } { $unit }²; { $volume } { $unit }³ alimman pisteen yläpuolella; korkeudet { $zmin }–{ $zmax } { $unit }; korkeuskäyrien väli { $interval } { $unit }
label-survey-alignment = Linja
# An alignment: its name, length and first and last stations ("1+234.50").
survey-alignment = { $name }: { $length } { $unit }, paaluväli { $start }–{ $end }
# SHX shape fonts found and used for the drawing's text.
label-drawing-shx-fonts = SHX-fontit
label-drawing-warnings = Ongelmat
label-plot-paper = Paperi
# "2 of 5": the page shown and how many the plot has.
plot-page-of = { $page }/{ $pages }
label-plot-page = Sivu
label-plot-pens = Käytetyt kynät
# The printer language a plot came wrapped in, e.g. "PCL 5".
label-plot-job = Tulostustyö
# What a plot draws: strokes, fills and labels.
label-plot-items = Piirretyt kohteet
# Width × height of what is drawn, in drawing units: "1,250 × 800 mm".
drawing-extents = { $width } × { $height } { $unit }
drawing-extents-unitless = { $width } × { $height }
drawing-model-space = Malli
# "24, 3 off or frozen"
drawing-layers-off = { $num }, joista { $off } pois päältä tai jäädytetty
# DXF $INSUNITS.
drawing-units = { $code ->
    [1] Tuumat
    [2] Jalat
    [3] Mailit
    [4] Millimetrit
    [5] Senttimetrit
    [6] Metrit
    [7] Kilometrit
    [8] Mikrotuumat
    [9] Tuuman tuhannesosat
    [10] Jaardit
    [11] Ångströmit
    [12] Nanometrit
    [13] Mikrometrit
    [14] Desimetrit
    [15] Dekametrit
    [16] Hehtometrit
    [17] Gigametrit
    [18] Astronomiset yksiköt
    [19] Valovuodet
    [20] Parsekit
    [21] Yhdysvaltain maanmittausjalat
   *[other] Ei yksikköä
}
# All-fields group title: "DXF entities".
fields-drawing-entities = { $format }-objektit

## IC-layoutit (GDSII)

# machine-drafted, needs native review
label-layout-library = Kirjasto
label-layout-units = Yksiköt
# The database unit (the grid coordinates are stored in) and the user unit, with SI symbols:
# "1 nm database unit, 1 µm user unit".
layout-units = { $db } tietokantayksikkö, { $user } käyttäjäyksikkö
# The top cell shown (each top cell is a page).
label-layout-cell = Solu
label-layout-cells = Solut
# Cells no other cell places.
label-layout-top-cells = Ylätason solut
# "TOP, TEST, PAD and 12 more"
layout-top-cells-more = { $names } ja { $more } muuta
# Layer/datatype pairs.
label-layout-layers = Tasot
# Polygons, paths and boxes, as stored (each cell once).
label-layout-shapes = Muodot
label-layout-texts = Nimiöt
# Cell references and the placements they make: "1,204 (88,320 placements)".
label-layout-references = Viittaukset
layout-references = { $refs } ({ $placements } { $placements ->
    [one] sijoitus
   *[other] sijoitusta
})
# Shapes with every cell placement expanded.
label-layout-flat-shapes = Muodot litistettynä
label-layout-bounds = Rajat
# Corners of what the cell draws, micrometres: "-12.5, 0 to 2,900, 3,500 µm".
layout-bounds = { $x0 }, { $y0 } – { $x1 }, { $y1 } µm

## Piirilevyt ja piirikaaviot (Gerber, Excellon, KiCad)

# machine-drafted, needs native review
group-board = Piirilevy
group-schematic = Piirikaavio
# The board view shown (its layers, its top or bottom), or the schematic sheet.
label-pcb-view = Näkymä
label-pcb-project = Projekti
label-pcb-revision = Revisio
label-pcb-date = Päivämäärä
# The CAD program that wrote the file.
label-pcb-generator = Tehty ohjelmalla
# Width × height of the board outline: "100 × 80 mm".
label-pcb-size = Levyn koko
label-pcb-thickness = Paksuus
label-pcb-copper-layers = Kuparikerrokset
label-pcb-sheets = Arkit
# The board's layers or files, each with its role: "board-F_Cu.gbr (Top copper)".
label-pcb-layers = Kerrokset
pcb-layer-role = { $name } ({ $role })
label-pcb-components = Footprintit
label-pcb-symbols = Symbolit
label-pcb-nets = Netit
label-pcb-tracks = Johtimet
label-pcb-wires = Johdot
label-pcb-vias = Läpiviennit
# Drilled holes and how many drill sizes: "1,204 in 9 sizes".
label-pcb-holes = Reiät
pcb-holes = { $holes }, { $sizes } { $sizes ->
    [one] koko
   *[other] kokoa
}
label-pcb-apertures = Aukot
# What a board layer or file is, shown after its name.
pcb-role-inner-copper = Sisäkupari { $n }
pcb-role-top-copper = Kupari yläpuoli
pcb-role-bottom-copper = Kupari alapuoli
pcb-role-top-solder-mask = Juotosmaski yläpuoli
pcb-role-bottom-solder-mask = Juotosmaski alapuoli
pcb-role-top-silkscreen = Komponenttipainatus yläpuoli
pcb-role-bottom-silkscreen = Komponenttipainatus alapuoli
pcb-role-top-paste = Pasta yläpuoli
pcb-role-bottom-paste = Pasta alapuoli
pcb-role-top-solder-paste = Juotospasta yläpuoli
pcb-role-bottom-solder-paste = Juotospasta alapuoli
pcb-role-top-adhesive = Liima yläpuoli
pcb-role-bottom-adhesive = Liima alapuoli
pcb-role-top-courtyard = Varausalue yläpuoli
pcb-role-bottom-courtyard = Varausalue alapuoli
pcb-role-top-fabrication = Valmistus yläpuoli
pcb-role-bottom-fabrication = Valmistus alapuoli
pcb-role-board-outline = Levyn ääriviiva
pcb-role-plated-drill = Pinnoitetut porareiät
pcb-role-non-plated-drill = Pinnoittamattomat porareiät
pcb-role-drill = Poraus
pcb-role-drill-holes = Porareiät
pcb-role-mechanical = Mekaaninen
pcb-role-margin = Reunus
pcb-role-drawings = Piirustukset
pcb-role-comments = Kommentit
pcb-role-engineering-change-1 = Tekninen muutos 1
pcb-role-engineering-change-2 = Tekninen muutos 2
pcb-role-vias = Läpiviennit
pcb-role-other = Muu
pcb-role-unknown = Tuntematon
label-pcb-macros = Aukkomakrot

## Spektrit (JCAMP-DX, SPC, mzML, mzXML, Bruker NMR)

# machine-drafted, needs native review
group-spectrum = Spektri
label-spectrum-format = Muoto
label-spectrum-technique = Menetelmä
label-spectrum-title = Otsikko
# The page shown: "3 of 1,204: Scan 1532".
label-spectrum-page = Sivu
spectrum-page-of = { $page }/{ $pages }: { $name }
# An axis with its unit: "Wavenumber (cm⁻¹)".
spectrum-axis = { $title } ({ $unit })
label-spectrum-x = X-akseli
label-spectrum-y = Y-akseli
label-spectrum-nucleus = Ydin
label-spectrum-frequency = Spektrometrin taajuus
label-spectrum-solvent = Liuotin
label-spectrum-pulse-program = Pulssiohjelma
label-spectrum-spectra = Spektrit
label-spectrum-points = Pisteet
label-spectrum-instrument = Instrumentti
# Who or what lab measured it.
label-spectrum-origin = Alkuperä
label-spectrum-date = Mitattu
# How Specular made the page from the raw data (an FID transformed, phased automatically).
label-spectrum-processing = Käsittely
spectrum-technique-ir = Infrapuna
spectrum-technique-raman = Raman
spectrum-technique-uv-vis = UV-Vis
spectrum-technique-fluorescence = Fluoresenssi
spectrum-technique-nmr = NMR
spectrum-technique-ms = Massaspektri
spectrum-technique-chromatogram = Kromatogrammi
spectrum-technique-other = Spektri

## 3D-mallit (STL, OBJ, PLY, glTF, 3MF)

# machine-drafted, needs native review
group-mesh = 3D-malli
label-triangles = Kolmiot
label-vertices = Kärkipisteet
label-materials = Materiaalit
label-textures = Tekstuurit
label-up-axis = Pystyakseli
# Textures the model uses, some of which could not be read: "3 (1 missing)".
mesh-textures-missing = { $count } ({ $missing } puuttuu)

## Molekyylirakenteet (PDB, mmCIF, CIF, SDF, XYZ)

# machine-drafted, needs native review
group-structure = Rakenne
# The entry's identifier: a PDB ID such as 1CRN, or the CIF data block's name.
label-structure-id = Merkintä
label-structure-classification = Luokitus
# How the structure was determined: X-ray diffraction, electron microscopy, solution NMR…
label-structure-method = Menetelmä
# The crystallographic or cryo-EM resolution, in ångströms.
label-structure-resolution = Erotuskyky
structure-resolution = { $value } Å
label-structure-deposited = Talletettu
label-structure-formula = Kaava
label-structure-chains = Ketjut
# Residues (amino acids, nucleotides) in the polymer chains.
label-structure-residues = Tähteet
label-structure-atoms = Atomit
# Atoms after the unit cell was filled by symmetry, and those the file lists: "96 (12 in the asymmetric unit)".
structure-atoms-cell = { $count } ({ $asymmetric } asymmetrisessa yksikössä)
label-structure-bonds = Sidokset
# Models in the file (an NMR ensemble), of which the first is shown.
label-structure-models = Mallit
# Molecules (SDF records) or data blocks in the file, of which the first is shown.
label-structure-records = Tietueet
structure-first-of = { $count } (ensimmäinen näytetään)
# Small molecules bound to the polymer, by their three-letter codes.
label-structure-ligands = Ligandit
# A list cut short: "HEM, NAG, SO4 and 9 more".
structure-ligands-more = { $list } ja { $more } muuta
label-structure-cell = Alkeiskoppi
# Unit cell edge lengths (Å) and angles (degrees).
structure-cell = a = { $a } Å, b = { $b } Å, c = { $c } Å, α = { $alpha }°, β = { $beta }°, γ = { $gamma }°
label-structure-space-group = Avaruusryhmä
label-structure-secondary = Sekundaarirakenne
structure-ss-file = Tiedostosta
structure-ss-computed = Laskettu pääketjusta

## HDF4 ja HDF-EOS2

# machine-drafted, needs native review
group-hdf4 = HDF4
# The HDF library version that wrote the file, e.g. "NCSA HDF Version 4.2 Release 15".
label-hdf4-library = Kirjasto
# Counts of the file's scientific datasets, raster images, groups (Vgroups) and tables (Vdatas).
label-hdf4-datasets = Tietojoukot
label-hdf4-images = Rasterikuvat
label-hdf4-vgroups = Ryhmät
label-hdf4-vdatas = Taulukot
# HDF-EOS: NASA's Earth Observing System conventions on top of HDF4 (MODIS, ASTER, AIRS…).
group-hdf-eos = HDF-EOS
label-eos-version = Versio
# The grids, swaths and point structures the file defines.
label-eos-structures = Rakenteet
eos-grids = { $count ->
    [one] { $count } ruudukko ({ $names })
   *[other] { $count } ruudukkoa ({ $names })
}
eos-swaths = { $count ->
    [one] { $count } keilauskaista ({ $names })
   *[other] { $count } keilauskaistaa ({ $names })
}
eos-points = { $count ->
    [one] { $count } pisterakenne
   *[other] { $count } pisterakennetta
}
# The grid or swath the dataset shown belongs to.
label-eos-structure = Rakenne
# The grid's map projection as its GCTP name, e.g. GCTP_SNSOID (sinusoidal).
label-eos-projection = Projektio
label-eos-grid-size = Ruudukon koko
# Columns × rows.
eos-grid-size = { $width } × { $height }
# The grid's upper-left and lower-right corners.
label-eos-corners = Kulmat
# The product's short name from the inventory metadata, e.g. MOD09A1.
label-eos-product = Tuote
# When the data was acquired.
label-eos-time = Aika
eos-time = { $begin } – { $end }

## Planeettakuvat (VICAR, PDS3, PDS4: NASAn ja ESAn missioiden data)

# machine-drafted, needs native review
group-planetary = Planeettakuva
# The body observed, e.g. MARS, Ganymede.
label-planetary-target = Kohde
label-planetary-mission = Missio
# The spacecraft, rover or lander carrying the instrument.
label-planetary-spacecraft = Avaruusalus
label-planetary-start = Alkuaika
label-planetary-stop = Loppuaika
# The product's identifier in the archive.
label-planetary-product = Tuotetunnus
# The map projection of a map-projected image, as the label names it.
label-planetary-projection = Karttaprojektio
label-planetary-resolution = Erotuskyky
planetary-metres-per-pixel = { $metres } m pikseliä kohden
planetary-pixels-per-degree = { $pixels } pikseliä astetta kohden
# Latitude and longitude of the map projection's centre.
label-planetary-centre = Projektion keskipiste
planetary-lat-lon = { $lat }°, { $lon }°
# The latitudes and longitudes the map covers.
label-planetary-coverage = Kattavuus
planetary-coverage = leveysaste { $south }° – { $north }°, pituusaste { $west }° – { $east }°
# The body's equatorial radius the map uses.
label-planetary-radius = Kappaleen säde
planetary-km = { $km } km

## OMERO (OMERO-palvelimelta tai Image Data Resourcesta avatut kuvat)

# machine-drafted, needs native review
group-omero = OMERO
label-omero-server = Palvelin
# $server the server's address; $user the user name signed in with.
omero-server-user = { $server } (käyttäjänä { $user })
omero-server-public = { $server } (julkinen)
label-omero-image = Kuva
# $name the image's name; $id its number on the server.
omero-image = { $name } (ID { $id })
# The project and dataset the image is in, as "Project › Dataset".
label-omero-project = Projekti
# The screen, plate and well, as "Screen › Plate › B4".
label-omero-screen = Seulonta
label-omero-owner = Omistaja
# $owner the image's owner; $group the OMERO group it belongs to.
omero-owner-group = { $owner } ({ $group })
label-omero-acquired = Kuvattu
label-omero-dimensions = Mitat
omero-size = { $width } × { $height }
omero-z = { $count ->
    [one] { $count } Z-taso
   *[other] { $count } Z-tasoa
}
omero-c = { $count ->
    [one] { $count } kanava
   *[other] { $count } kanavaa
}
omero-t = { $count ->
    [one] { $count } aikapiste
   *[other] { $count } aikapistettä
}
# The physical size of a pixel along X, Y (and Z).
label-omero-physical = Pikselin koko
label-omero-channels = Kanavat
# A channel not shown in the server's rendering.
omero-channel-hidden = { $channel } (piilotettu)
# How the pixels are fetched from the server: rendered tiles (JPEG) or the whole plane (PNG).
label-omero-fetched = Haettu muodossa
omero-fetched-tiles = { $encoding }-ruutuja { $width } × { $height }, { $levels ->
    [one] { $levels } taso
   *[other] { $levels } tasoa
}
omero-fetched-whole = Kokonaiset tasot muodossa { $encoding }

## Heijastusmuunnoskuvat (PTM, RTI, Relight)

# machine-drafted, needs native review
group-rti = Heijastusmuunnos
# The reflectance model the coefficients belong to: PTM (polynomial), HSH (hemispherical harmonics), RBF…
label-rti-basis = Kanta
label-rti-order = Kertaluku
# Coefficients per colour channel.
label-rti-terms = Kertoimet
# How colour is stored: LRGB (one colour, varying luminance), RGB, YCC…
label-rti-colour = Värien asettelu
# Coefficient images stored in the file.
label-rti-planes = Tasot
# The byte-to-coefficient mapping the file states, per coefficient.
label-rti-scale = Skaala
label-rti-bias = Siirtymä

## Pistepilvet

# machine-drafted, needs native review
group-points = Pistepilvi
label-points = Pisteet
# Points read of those in the file, when a very large cloud was evenly thinned: "32,000,000 of 120,000,000".
points-subsampled = { $loaded } / { $total }
# The LAS point data record format number (0–10).
label-point-format = Pisteformaatti
# The coordinate reference system.
label-crs = Koordinaattijärjestelmä
# What each point carries besides its position.
label-attributes = Ominaisuudet
points-channel-rgb = RGB
points-channel-intensity = Intensiteetti
points-channel-classification = Luokittelu
points-channel-returns = Paluukaiut
# ASPRS classes present, with point counts: "Ground (1,234), Building (567)".
label-classes = Luokat
# E57 scans, with point counts.
label-scans = Skannaukset
point-class-0 = Ei koskaan luokiteltu
point-class-1 = Luokittelematon
point-class-2 = Maanpinta
point-class-3 = Matala kasvillisuus
point-class-4 = Keskikorkea kasvillisuus
point-class-5 = Korkea kasvillisuus
point-class-6 = Rakennus
point-class-7 = Matala piste (kohina)
point-class-8 = Mallin avainpiste
point-class-9 = Vesi
point-class-10 = Rautatie
point-class-11 = Tienpinta
point-class-12 = Päällekkäisyys
point-class-13 = Ukkosjohdin
point-class-14 = Vaihejohdin
point-class-15 = Voimajohtopylväs
point-class-16 = Johtoliitin
point-class-17 = Sillan kansi
point-class-18 = Korkea kohina
point-class-19 = Yläpuolinen rakenne
point-class-20 = Ohitettu maanpinta
point-class-21 = Lumi
point-class-22 = Ajallinen poissulku
# A class without a standard name, by number.
point-class-other = Luokka { $class }

## Gaussian splats

# machine-drafted, needs native review
group-splats = Gaussiset splatit
label-splats = Splatit
# Splats shown of those in the file, when a very large file was evenly thinned: "12,000,000 of 30,000,000".
splats-shown-of = { $shown } / { $total }
# The degree (1 to 3) of the spherical harmonics that give each splat its view-dependent colour.
label-splat-harmonics = Katselukulmasta riippuva väri
splats-harmonics-none = Ei mitään
# $format: the file format, e.g. "SPZ".
splats-format-antialiased = { $format }, koulutettu reunanpehmennyksellä

## Varoitukset

# machine-drafted, needs native review
warn-not-diagnostic = Ei diagnostiseen käyttöön. Specular on yleinen kuvankatseluohjelma, ei lääkinnällinen laite: sen näyttöä ei ole kalibroitu diagnostiikkaan.

warn-gps = Tiedostoon on tallennettu kuvauspaikka. Poista sijainti ennen jakamista, jos sen pitää pysyä yksityisenä.
warn-null-island = GPS-sijainti on tasan 0°, 0°; kameralla ei luultavasti ollut paikannusta, joten sijainti on todennäköisesti väärä.
warn-date-future = Kuvauspäivä on tulevaisuudessa; kameran kello oli luultavasti asetettu väärin.
warn-icc-damaged = Upotettu väriprofiili on vioittunut, eikä sitä voitu lukea.
warn-metadata-failed = Metatietoja ei voitu lukea Specularin virheen vuoksi. Itse kuva on kunnossa.
warn-orientation-conflict = EXIF ja XMP ovat eri mieltä suunnasta (EXIF: { $exif }, XMP: { $xmp }). Käytetään EXIF-arvoa.
warn-exif-damaged = Osaa EXIF-tiedoista ei voitu lukea ({ $error }); loput näytetään.
warn-xmp-damaged = XMP-metatiedot ovat virheellisiä ({ $error }); vain osa voitiin lukea.

## Tietoja-ikkuna

geonames-attribution = Paikannimet: GeoNames (www.geonames.org), lisenssi Creative Commons Nimeä 4.0 (CC BY 4.0, https://creativecommons.org/licenses/by/4.0/). Offline-haku Grant Minerin reverse_geocoder-kirjastolla (MIT tai Apache-2.0). Tiedot on muunnettu kaupunkiluetteloksi; ne toimitetaan sellaisenaan ilman takuuta.

## TIFF and DNG structure values (All fields)
# machine-drafted, needs native review

subfile-type = { $value ->
    [0] Täysitarkkuuksinen kuva
    [1] Pienennetyn tarkkuuden kuva
    [2] Monisivuisen kuvan yksi sivu
    [4] Läpinäkyvyysmaski
   *[other] Muu
}
predictor = { $value ->
    [1] Ei mitään
    [2] Vaakasuuntainen erotus
    [3] Liukulukuerotus
   *[other] Muu
}
extra-samples = { $value ->
    [0] Määrittämätön
    [1] Esikerrottu alfa
    [2] Suora alfa
   *[other] Muu
}
sample-format = { $value ->
    [1] Etumerkitön kokonaisluku
    [2] Etumerkillinen kokonaisluku
    [3] Liukuluku
    [4] Määrittelemätön
   *[other] Muu
}

## Paikkatieto (GeoTIFF, GeoJSON, Shapefile, GeoPackage, KML/KMZ, GPX)

# machine-drafted, needs native review
group-geospatial = Paikkatieto
label-geo-crs = Koordinaattijärjestelmä
label-geo-datum = Datumi
label-geo-projection = Projektio
label-geo-unit = Yksiköt
label-geo-bounds = Laajuus
label-geo-lonlat-bounds = Pituus- ja leveysaste
label-geo-pixel-size = Pikselikoko
label-geo-overviews = Yleiskuvat
label-geo-nodata = NoData-arvo
label-geo-raster-type = Rasterin tyyppi
label-geo-vertical = Korkeusjärjestelmä
label-geo-citation = Lähdeviite
label-geo-features = Kohteet
label-geo-vertices = Taitepisteet
label-geo-layer = Taso
label-geo-layers = Tasot
label-geo-tiles = Tiilet
label-geo-encoding = Tekstin merkistö
label-geo-warnings = Ongelmat
# The companion files a georeference was read from ("scene.jgw, scene.prj").
label-geo-sidecars = Liitetiedostot
# machine-drafted, needs native review
# A projection Specular cannot convert to longitude and latitude: $method is its name.
geo-projection-unsupported = { $method } (koordinaatteja ei tueta)
# How longitude and latitude are brought to WGS 84 (what GPS and web maps use).
label-geo-to-wgs84 = WGS 84:ään
geo-to-wgs84-same = Ei tarvita
geo-to-wgs84-helmert = Datumimuunnos (Helmert)
geo-to-wgs84-by-area = Datumimuunnos (Helmert, valittu alueen mukaan)
geo-to-wgs84-unknown = Tuntematon: koordinaatit ovat tiedoston omassa datumissa
# A coordinate system's name and its EPSG code: "WGS 84 / UTM zone 33N (EPSG:32633)".
geo-crs-epsg = { $name } (EPSG:{ $code })
geo-crs-unknown = Ei ilmoitettu
# Map bounds: the lower-left and upper-right corners.
geo-bounds = { $x0 }, { $y0 } – { $x1 }, { $y1 }
# Bounds in degrees: west to east longitude, south to north latitude.
geo-lonlat-bounds = pituus { $west }° – { $east }°, leveys { $south }° – { $north }°
geo-pixel-size = { $x } × { $y }
geo-overviews = { $n ->
    [one] { $n } yleiskuva: { $sizes }
   *[other] { $n } yleiskuvaa: { $sizes }
}
geo-pixel-is-point = Pikselit ovat pisteitä
geo-layer = { $features ->
    [one] { $features } kohde
   *[other] { $features } kohdetta
}, { $fields ->
    [one] { $fields } kenttä
   *[other] { $fields } kenttää
}
# A GeoPackage tile pyramid: its table, the zoom level shown and its tile count.
geo-tiles = { $table }, zoomaustaso { $zoom }, { $tiles ->
    [one] { $tiles } tiili
   *[other] { $tiles } tiiltä
}
fields-geo-geokeys = GeoTIFF-avaimet
fields-geo-gdal = GDAL-metatiedot
fields-geo-wkt = Koordinaattijärjestelmä (WKT)
fields-geo-projjson = Koordinaattijärjestelmä (PROJJSON)
# machine-drafted, needs native review
# What a web map tile set (MBTiles, PMTiles) says about itself: container, tile type, zoom
# levels, description and other metadata.
fields-geo-tileset = Ruutujoukko
# The credit line a map's data asks to be shown with it ("© OpenStreetMap contributors").
label-geo-attribution = Lähdemaininta
# The header lines of a georeferenced raster format (ENVI, ASCII grid, DTED, ERDAS IMAGINE).
fields-geo-header = Otsake
# machine-drafted, needs native review
# GIFTI files: the format version, what the file holds, the number of data arrays and the
# encoding of their values (e.g. "GZipBase64Binary").
label-gifti-version = GIFTI-versio
label-gifti-role = Sisältää
gifti-role-surface = Pinta
gifti-role-labels = Nimiöt kärkipisteittäin
gifti-role-data = Arvot kärkipisteittäin
label-gifti-arrays = Datataulukot
label-gifti-encoding = Koodaus
label-gifti-vertices = Kärkipisteet
label-gifti-triangles = Kolmiot
# The brain structure, e.g. "CortexLeft".
label-gifti-structure = Rakenne
# A surface's geometric type, e.g. "Anatomical", "Inflated".
label-gifti-geometry = Geometrinen tyyppi
# The coordinate transform of the vertices: $from and $to are coordinate spaces such as
# "scanner", "Talairach" or "MNI 152".
label-gifti-transform = Koordinaatit
gifti-transform = { $from } (muunnosta kohteeseen { $to } ei tarvita)
gifti-transform-applied = { $from } → { $to } (käytetty)
# The surface file a data file is drawn on, found beside it.
label-gifti-surface = Näytetään pinnalla
# The data array shown: its name, NIfTI intent (e.g. "NIFTI_INTENT_SHAPE") and data type.
label-gifti-array = Taulukko
label-gifti-intent = Intent
label-gifti-data-type = Tietotyyppi
# Entries of the label table (names and colours of regions).
label-gifti-labels = Nimiötaulukko
# CIFTI-2 files: the version, NIfTI intent (e.g. "ConnDenseScalar") and matrix size.
label-cifti-version = CIFTI-versio
label-cifti-intent = Intent
label-cifti-matrix = Matriisi
cifti-matrix = { $rows } × { $columns }
# The maps (pages) of the file and what they are.
label-cifti-maps = Kartat
cifti-maps-scalars = Skalaarikarttoja: { $count }
cifti-maps-labels = Nimiökarttoja: { $count }
cifti-maps-series = Aikapisteitä: { $count }
cifti-maps-parcels = Parselleja: { $count }
cifti-maps-other = { $count }
# machine-drafted, needs native review
# A dense connectivity file (.dconn.nii): every brain place's connectivity to every other.
# $seeds: how many places can be picked; $size: the bytes read when one is picked (e.g. "357 KB").
label-cifti-dense = Konnektiivisuus
cifti-dense = { $seeds ->
        [one] { $seeds } siemenpiste
       *[other] { $seeds } siemenpistettä
    }, { $size } luetaan pistettä kohden
# A time series' start and step; $unit is e.g. "second".
label-cifti-series = Sarja
cifti-series = alkaen { $start }, välein { $step } ({ $unit })
label-cifti-models = Aivomallit
label-cifti-parcels = Parsellit
label-cifti-structures = Rakenteet
label-cifti-voxels = Subkortikaaliset vokselit
# A surface the file's values lie on: the brain structure (e.g. "CORTEX_LEFT"), its vertex
# count, and the surface file found beside the file for it, or that none was found.
label-cifti-surface = Pinta
cifti-surface-found = { $structure }: { $vertices ->
        [one] { $vertices } kärkipiste
       *[other] { $vertices } kärkipistettä
    }, { $file }
cifti-surface-missing = { $structure }: tarvitsee pinnan, jossa on { $vertices ->
        [one] { $vertices } kärkipiste
       *[other] { $vertices } kärkipistettä
    } (tiedoston vieressä ei ole yhtään)
# How the file is shown: on the surfaces found beside it, or as its subcortical voxels.
label-cifti-shown = Näytetään
cifti-shown-surfaces = Väritettyinä pintoina
cifti-shown-voxels = Subkortikaalisten vokselien tilavuutena
# machine-drafted, needs native review
# Shown with a picked seed's row: click a surface vertex, or move the crosshair in MPR.
cifti-shown-connectivity = Valitun siemenpisteen konnektiivisuus
# Maps left out of the voxel volume, which would be too large with them.
label-cifti-left-out = Pois jätetyt kartat
# Tractograms: format, number of streamlines (fibre tracts) and points.
label-tract-format = Muoto
label-tract-streamlines = Virtaviivat
# A large tractogram draws only some of its streamlines.
tract-streamlines-shown = { $count } ({ $shown } näytetään)
label-tract-points = Pisteet
label-tract-mean-length = Keskipituus
# A length in millimetres.
tract-mm = { $value } mm
# Values per point and per streamline the file stores, by name.
label-tract-scalars = Skalaarit pisteittäin
label-tract-properties = Ominaisuudet virtaviivoittain
# TrackVis: the voxel grid the tracts were made in, its voxel size and voxel order (e.g. "LAS").
label-tract-grid = Hila
label-tract-voxel-size = Vokselin koko
label-tract-voxel-order = Vokselijärjestys
# How the points were brought into scanner (RAS) millimetres.
label-tract-space = Koordinaatit
tract-space-vox-to-ras = Vokseli–RAS-matriisista
tract-space-voxel-order = Vokselijärjestyksestä (ei vokseli–RAS-matriisia)
tract-space-scanner = Skannerin koordinaatisto
# MRtrix: the tracking step size.
label-tract-step = Askelpituus

## File headers (DPX, Cineon, XWD, MIFF, TIM, WSQ, JBIG)

# machine-drafted, needs native review
# What the file's own header records, for formats without Exif or XMP.
group-header = Tiedoston otsake
label-header-created = Luotu
# The program or person that wrote the file.
label-header-creator = Tekijä
label-header-copyright = Tekijänoikeus
label-header-project = Projekti
label-header-comment = Kommentti
# The scanner, camera or other device the image came from.
label-header-device = Laite
# SMPTE time code of the frame: "01:00:00:12".
label-header-timecode = Aikakoodi
# A film frame's edge code (KeyKode).
label-header-edge-code = Reunakoodi
label-header-frame-rate = Kuvanopeus
# Where the frame sits in its sequence.
label-header-frame = Kehys
# Film stock and slate.
label-header-film = Filmi
# How code values relate to light: printing density, linear, BT.709…
label-header-transfer = Siirtofunktio
# The colour primaries the file states.
label-header-colorimetric = Kolorimetria
# The X window a screen dump was taken of.
label-header-window = Ikkuna
label-header-resolution = Erotuskyky
label-header-compression = Pakkaus
# machine-drafted, needs native review
# JPEG XS: the profile and the level the codestream states ("Main 422.10"; "4k-1, sublevel 6 bpp").
label-header-profile = Profiili
# machine-drafted, needs native review
label-header-level = Taso
# machine-drafted, needs native review
# JPEG XS: how many wavelet levels, horizontal and vertical ("5 horizontal, 1 vertical").
label-header-decomposition = Wavelet-tasot
# machine-drafted, needs native review
# JPEG XS: the colour transform ("None", "Reversible colour transform (RCT)", "Star-Tetrix, RGGB mosaic").
label-header-colour-transform = Värimuunnos
# machine-drafted, needs native review
# Photoshop: the artboards, by name and size ("Phone (375 × 667), Tablet (768 × 1024)").
label-header-artboards = Piirtoalueet
# machine-drafted, needs native review
# Photoshop file without a merged image, shown from its layers: the features it leaves out,
# by Photoshop's names ("Color Lookup, Knockout").
label-header-not-drawn = Ei piirretty
# machine-drafted, needs native review
# The same, for the features drawn only approximately ("Bevel & Emboss, Satin").
label-header-approximated = Piirretty likimääräisesti

## Fonts (TTF, OTF, TTC/OTC, WOFF, WOFF2)

# machine-drafted, needs native review
group-font = Fontti
label-font-family = Perhe
label-font-style = Tyyli
# The face's place in a font collection: "2 of 4".
label-font-face = Leikkaus
font-face-of = { $index }/{ $count }
label-font-version = Versio
label-font-designer = Suunnittelija
# The font's maker (the name table's manufacturer).
label-font-manufacturer = Valmistaja
label-font-glyphs = Glyfit
label-font-outlines = Ääriviivat
font-outlines-truetype = TrueType (neliölliset)
font-outlines-cff = PostScript (CFF, kuutiolliset)
font-outlines-cff2 = PostScript (CFF2, kuutiolliset)
font-outlines-bitmap = Vain bittikartat
font-outlines-none = Ei mitään
# What the font's licence flags (OS/2 fsType) allow documents that embed it to do.
label-font-embedding = Upottaminen
font-embedding-installable = Saa upottaa ja asentaa.
font-embedding-restricted = Ei saa upottaa.
font-embedding-preview-print = Saa upottaa vain katselua ja tulostusta varten.
font-embedding-editable = Saa upottaa muokattaviin asiakirjoihin.
font-embedding-no-subsetting = Vain kokonaan, ei osajoukkona.
font-embedding-bitmap-only = Vain bittikarttoina.
# Variation axes of a variable font: "Weight 100–900 (400)".
label-font-axes = Muunnosakselit
font-axis = { $name } { $min }–{ $max } ({ $default })
label-font-instances = Nimetyt tyylit
label-font-colour = Väriglyfit
font-colour-colr0 = COLR-kerrokset
font-colour-colr1 = COLR v1 -liukuvärit
font-colour-sbix = Bittikartat (sbix)
font-colour-cbdt = Bittikartat (CBDT)
font-colour-svg = SVG
label-font-weight = Paino
label-font-units-per-em = Yksikköä per em
label-font-tables = Taulukot
label-font-licence = Lisenssi
label-font-licence-url = Lisenssin URL
label-font-copyright = Tekijänoikeus
label-font-trademark = Tavaramerkki

## Office documents (Word, Excel, PowerPoint, iWork, OpenDocument)

# machine-drafted, needs native review
group-document = Asiakirja
label-document-kind = Laji
document-kind-word = Word-asiakirja
document-kind-excel = Excel-työkirja
document-kind-powerpoint = PowerPoint-esitys
document-kind-pages = Pages-asiakirja
document-kind-numbers = Numbers-laskentataulukko
document-kind-keynote = Keynote-esitys
document-kind-odf-text = OpenDocument-teksti
document-kind-odf-spreadsheet = OpenDocument-laskentataulukko
document-kind-odf-presentation = OpenDocument-esitys
document-kind-odf-graphics = OpenDocument-piirros
document-kind-odf-other = OpenDocument-tiedosto
# machine-drafted, needs native review
document-kind-playlist = Soittolista
# machine-drafted, needs native review
label-playlist-items = Kohteet
# Appended to the kind: "Word document, template, with macros".
document-template = malli
document-macros = sisältää makroja
document-legacy = Office 97–2003 -muoto
label-document-preview = Esikatselu
# The image format of the preview stored in the file, e.g. "JPEG image".
document-preview-image = { $format }-kuva
document-preview-none = Ei tiedostossa
label-document-subject = Aihe
label-document-author = Tekijä
label-document-description = Kuvaus
label-document-category = Luokka
label-document-last-saved-by = Viimeksi tallentanut
label-document-created = Luotu
label-document-modified = Muokattu
label-document-printed = Viimeksi tulostettu
label-document-pages = Sivut
label-document-slides = Diat
label-document-hidden-slides = Piilotetut diat
label-document-notes = Muistiinpanosivut
label-document-sheets = Taulukot
label-document-words = Sanat
label-document-characters = Merkit
label-document-paragraphs = Kappaleet
label-document-lines = Rivit
label-document-editing-time = Muokkausaika
document-minutes = { $minutes } min
label-document-revision = Versio
label-document-application = Tallennettu ohjelmalla
label-document-company = Yritys
label-document-template = Malli

## Books (EPUB)

# machine-drafted, needs native review
group-book = Kirja
label-book-authors = Tekijät
label-book-publisher = Kustantaja
label-book-date = Julkaistu
label-book-language = Kieli
label-book-pages = Sivuja
# Entries in the book's table of contents.
label-book-contents = Sisällysluettelon kohdat
label-book-layout = Asettelu
book-layout-reflowable = Mukautuva
book-layout-fixed = Kiinteä asettelu
book-right-to-left = oikealta vasemmalle
label-book-version = Muoto
book-version = EPUB { $version }
label-book-fonts = Upotetut fontit
book-fonts-obfuscated = { $count } ({ $obfuscated } peitettyä)
label-book-subjects = Aiheet
label-book-identifier = Tunniste
label-book-rights = Oikeudet

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
volume-orient-parrec = Philipsin kulma ja keskipistesiirto
volume-orient-ecat = ECAT-vokselikoot ja siirrot (epäluotettava)
label-volume-technique = Tekniikka
label-volume-version = Muodon versio
label-volume-isotope = Isotooppi
label-volume-tracer = Merkkiaine
label-volume-units = Yksiköt

# Text, tables and subtitles.
label-header-encoding = Merkistö
label-header-lines = Rivit
label-header-line-ends = Rivinvaihdot
label-header-words = Sanat
label-header-pages = Sivut
label-header-pictures = Kuvat
label-header-pictures-not-shown = Näyttämättömät kuvat
label-header-tables = Taulukot
label-header-rows = Taulukon rivit
label-header-columns = Sarakkeet
label-header-delimiter = Erotin
label-header-column-names = Sarakkeiden nimet
label-header-cues = Tekstitykset
label-header-cue-span = Tekstitykset alkaen – asti
