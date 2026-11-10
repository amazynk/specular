# machine-drafted, needs native review
# Animaatioiden toisto ja lukutila (arkistot, sarjakuvat).

disabled-not-animated = Tämä kuva ei ole animoitu
disabled-in-archive = Arkiston sisällä olevia sivuja ei voi siirtää roskakoriin

## Komentopaletti ja pikanäppäinluettelo

palette-play-pause = Toista tai keskeytä animaatio
palette-next-frame = Seuraava ruutu
palette-prev-frame = Edellinen ruutu
palette-faster = Toista nopeammin
palette-slower = Toista hitaammin
palette-normal-speed = Normaali toistonopeus
palette-loop-forever = Toista animaatiota jatkuvasti
palette-right-to-left = Sivujärjestys oikealta vasemmalle
# machine-drafted, needs native review
palette-contents = Sisällysluettelo
palette-spreads = Aukeamat

## Animaation säätimet (työkalurivin yläpuolella)

# Toistosäädinryhmän saavutettavuusnimi.
transport-region = Animaation säätimet
transport-play = Toista
transport-pause = Keskeytä
transport-prev-frame = Edellinen ruutu
transport-next-frame = Seuraava ruutu
transport-slower = Hitaammin
transport-faster = Nopeammin
# $index on näytettävä ruutu, $count ruutujen määrä.
transport-frame = Ruutu { $index } / { $count }
# Ruutujen määrä ei ole tiedossa ennen kuin animaatio on toistettu kerran.
transport-frame-unknown = Ruutu { $index }
# $speed on muotoiltu luku, esim. 1,5.
transport-speed = { $speed }×
transport-speed-name = Toistonopeus { $speed }×
# machine-drafted, needs native review
motion-badge = Liike
motion-badge-live = Live
motion-play = Toista liikeleike
motion-play-live = Toista Live Photo
motion-stop = Takaisin still-kuvaan
toast-motion-failed = Liikeleikettä ei voi toistaa: { $reason }

## Ilmoitukset

toast-speed = Toistonopeus { $speed }×
toast-loop-forever-on = Toistetaan jatkuvasti
toast-loop-forever-off = Toistetaan tiedoston mukaan
toast-right-to-left-on = Sivut luetaan oikealta vasemmalle
toast-right-to-left-off = Sivut luetaan vasemmalta oikealle
toast-spreads-on = Aukeamat
toast-spreads-off = Yksi sivu kerrallaan

## Arkistot

# Arkiston sivun ikkunan otsikko.
title-archive-page = { $page } — { $archive }
# Sijaintilukema, kun näytetään kaksi sivua.
readout-position-spread = { $first }–{ $last } / { $count }
# $format on RAR.
archive-needs-sandbox = { $format }-arkistoja voi avata vain hiekkalaatikossa toimiva purkaja, jota tämä versio ei vielä sisällä.
archive-empty = Arkistossa ei ole kuvia.
archive-failed = Arkistoa ei voitu avata: { $error }
archive-encrypted = Arkisto on salattu. Avaa se uudelleen ja anna salasana.
archive-wrong-password = Arkiston salasana on väärä.
password-title = Salasana tarvitaan
password-archive = ”{ $archive }” on salattu.
password-label = Salasana
password-show = Näytä salasana
password-wrong = Väärä salasana. Yritä uudelleen.
password-open = Avaa
password-cancel = Peruuta

## Asetukset

settings-viewing-animation-autoplay = Toista animaatiot automaattisesti
settings-viewing-animation-autoplay-help = Käynnistä animoidut kuvat (GIF, APNG, WebP…) heti avattaessa.
settings-viewing-animation-loop = Animaation toisto
settings-viewing-animation-loop-help = Kuinka monta kertaa animaatiot toistetaan.
settings-viewing-animation-loop-file = Tiedoston mukaan
settings-viewing-animation-loop-forever = Jatkuvasti
settings-viewing-animation-loop-once = Kerran
settings-viewing-animation-short-delays = Hyvin lyhyet ruutuviiveet
settings-viewing-animation-short-delays-help = Enintään 10 ms kestäviksi tarkoitetut ruudut: hidasta ne 100 ms:iin kuten verkkoselaimet, tai toista tallennetulla tavalla.
settings-viewing-animation-short-delays-browser = Kuten verkkoselaimet (100 ms)
settings-viewing-animation-short-delays-as-stored = Tallennetulla tavalla
settings-browsing-reading-order = Sivujärjestys arkistoissa
settings-browsing-reading-order-help = Sarjakuva-arkistot luetaan oikealta vasemmalle, kun niiden ComicInfo.xml niin määrää (manga), ellei tässä toisin aseteta.
settings-browsing-reading-order-auto = Arkiston mukaan
settings-browsing-reading-order-left-to-right = Vasemmalta oikealle
settings-browsing-reading-order-right-to-left = Oikealta vasemmalle
settings-browsing-page-layout = Sivut arkistoissa
settings-browsing-page-layout-help = Näytä yksi sivu tai kaksi rinnakkain; kansi näytetään aina yksinään.
settings-browsing-page-layout-auto = Arkiston mukaan
settings-browsing-page-layout-single = Yksi sivu
settings-browsing-page-layout-spreads = Aukeamat

## Pages of multi-image files (ICNS sizes, texture layers, faces and slices, TIFF pages) (machine-drafted, needs native review)
palette-next-page = Seuraava sivu
palette-prev-page = Edellinen sivu
palette-first-page = Ensimmäinen sivu
palette-last-page = Viimeinen sivu
disabled-one-page = Kuvassa on vain yksi sivu
page-indicator = Sivu { $index }/{ $count }
page-indicator-labelled = { $page }, { $label }
page-label-icon = { $width }×{ $height }
page-label-icon-density = { $width }×{ $height } @{ $density }x
page-label-layer = Kerros { $index }
page-label-face = Tahko { $face }
page-label-sphere = Pallokartta
page-label-slice = Leike { $index }
page-label-join = { $a } · { $b }
page-label-slide = Objektilasi
page-label-slide-label = Etiketti
page-label-slide-macro = Yleiskuva
page-label-slide-thumbnail = Pikkukuva
# machine-drafted, needs native review
page-label-thermal = Lämpökuva
page-label-visible = Näkyvän valon kuva
# machine-drafted, needs native review
page-label-left-view = Vasen näkymä
# machine-drafted, needs native review
page-label-right-view = Oikea näkymä
# machine-drafted, needs native review
# An object of a CERN ROOT file in the page picker: its path in the file and its class, e.g.
# "run1/hits (TH2D)".
page-label-object = { $name } ({ $class })
# machine-drafted, needs native review
# A view of one step of a circuit board job ("panel · Top").
page-label-board-step = { $step } · { $view }
# machine-drafted, needs native review
# A focal plane and a time point of a remote microscopy image (OMERO), counted from 1.
page-label-z = Z { $index }/{ $count }
page-label-t = T { $index }/{ $count }
# machine-drafted, needs native review
# Views of a remote-sensing product (an ENVI cube, a Sentinel-2 product) in the page picker.
page-label-product-composite = Oletuskanavat
page-label-product-bands = Kaikki kanavat
page-label-product-true-colour = Luonnolliset värit
page-label-product-false-colour = Väärävärit (lähi-infrapuna)
page-label-product-swir = Lyhytaaltoinen infrapuna
page-label-product-agriculture = Maatalous
page-label-product-scene-class = Näkymän luokittelu
page-label-product-cloud = Pilvisyyden todennäköisyys
# A product view at a pixel size, e.g. "True colour, 10 m".
page-label-product-at = { $view }, { $metres } m
title-file-page = { $file } — { $page }
info-page-group = Sivu
info-page-position = Sijainti
info-page-contents = Sisältö

# machine-drafted, needs native review
# OpenColorIO (Settings › Colour): why scene-linear images are shown without it.
ocio-failed = OCIO ei ole käytössä: { $reason }
ocio-builtin-config = $OCIO viittaa sisäänrakennettuun määritykseen (ocio://); tallenna se tiedostoksi ja valitse se tiedosto.

## Page labels for icons and collections (machine-drafted, needs native review)
page-label-size = { $width }×{ $height }
page-label-size-bits = { $width }×{ $height } · { $bits }-bittinen

## Page labels for Photoshop layers (machine-drafted, needs native review)
page-label-merged = Yhdistetty kuva
page-label-from-layers = Koottu tasoista
page-label-layer-hidden = { $name } (piilotettu)
page-label-layer-unnamed = Nimetön taso { $index }
