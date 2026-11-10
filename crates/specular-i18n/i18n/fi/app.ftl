# machine-drafted, needs native review
# Specular — katseluikkuna, komentopaletti, virheilmoitukset, Tietoja-ikkuna. Suomi.

-app-name = Specular

## Työkalurivin työkaluvihjeet ja saavutettavuusnimet

toolbar-open = Avaa kuva…
toolbar-prev = Edellinen kuva
toolbar-next = Seuraava kuva
toolbar-back-gallery = Takaisin galleriaan
toolbar-zoom-out = Loitonna
toolbar-zoom-in = Lähennä
toolbar-zoom-level = Zoomaustaso ja esiasetukset
toolbar-zoom-fit = Sovita ikkunaan
toolbar-zoom-100 = Todellinen koko ({ PERCENT(100) })
toolbar-rotate-left = Kierrä vastapäivään
toolbar-rotate-right = Kierrä myötäpäivään
toolbar-fullscreen-enter = Koko näyttö
toolbar-fullscreen-exit = Poistu koko näytöstä
toolbar-info = Kuvan tiedot
toolbar-settings = Asetukset
toolbar-delete = Siirrä roskakoriin
toolbar-copy = Kopioi kuva
toolbar-palette = Komentopaletti
toolbar-go-to = Siirry kuvaan…
toolbar-go-to-range = 1–{ $count }
# machine-drafted, needs native review
toolbar-customise = Mukauta työkaluriviä…
toolbar-more = Lisää
toolbar-announce-moved = { $item }: sijainti { $index }/{ $count }
toolbar-announce-added = { $item } lisätty: sijainti { $index }/{ $count }
toolbar-announce-removed = { $item } poistettu työkaluriviltä
toolbar-announce-reset = Työkalurivi palautettu oletukseksi
# Työkaluvihjeen teksti ja sen pikanäppäin.
tooltip-with-shortcut = { $label } ({ $shortcut })
# Juuri nyt käyttökelvottoman säätimen työkaluvihje; $reason on jokin disabled-*-viesteistä.
tooltip-unavailable = { $label } (ei käytettävissä: { $reason })
# Miksi säädin tai komento ei ole käytettävissä (työkaluvihjeet, komentopaletti, ruudunlukijat).
disabled-no-image = Kuvaa ei ole avattu
disabled-still-loading = Kuvaa ladataan vielä
disabled-decode-failed = Kuvan purkaminen epäonnistui
disabled-no-other-images = Kansiossa ei ole muita kuvia

## Lukemat

# Sijainti kansiossa, esim. "12 / 348".
readout-position = { $index } / { $count }
# Ruudunlukijan lukema sijainti.
readout-position-a11y = Kuva { $index }/{ $count }
# $zoom on valmiiksi muotoiltu prosenttiluku, esim. "150 %".
readout-zoom = { $zoom }
readout-zoom-a11y = Zoomaus { $zoom }
readout-zoom-fit = Sovitettu ({ $zoom })
zoom-preset-fit = Sovita ikkunaan
zoom-preset-fill = Täytä ikkuna
zoom-preset-actual = Todellinen koko
# machine-drafted, needs native review
# The last entry of the zoom presets: the window fills the screen (ticked while it does).
zoom-preset-fullscreen = Koko näyttö
# $zoom on valmiiksi muotoiltu prosenttiluku.
zoom-preset-percent = { $zoom }
# Kuvan mitat pikseleinä, esim. "6000 × 4000".
readout-dimensions = { $width } × { $height }
# Nykyisen kansion kuvien määrä.
status-image-count =
    { $count ->
        [one] { $count } kuva
       *[other] { $count } kuvaa
    }

## Komentopaletti

palette-placeholder = Kirjoita komento tai asetus…
palette-no-results = Ei vastaavia komentoja
# Asetuksen muuttaminen paletista, esim. "Aseta: Hiiren rulla → Seuraava kuva".
palette-set-setting = Aseta: { $setting } → { $value }
palette-next = Seuraava kuva
palette-prev = Edellinen kuva
palette-first = Ensimmäinen kuva
palette-last = Viimeinen kuva
palette-go-to = Siirry kuvaan…
palette-zoom-in = Lähennä
palette-zoom-out = Loitonna
palette-zoom-fit = Sovita ikkunaan
palette-zoom-fill = Täytä ikkuna
# machine-drafted, needs native review
palette-zoom-fit-width = Sovita leveyteen
palette-zoom-fit-height = Sovita korkeuteen
palette-zoom-100 = Zoomaa { PERCENT(100) }:iin
palette-zoom-200 = Zoomaa { PERCENT(200) }:iin
palette-zoom-300 = Zoomaa { PERCENT(300) }:iin
palette-zoom-400 = Zoomaa { PERCENT(400) }:iin
palette-zoom-rectangle = Zoomaa valintaan
palette-rotate-left = Kierrä vastapäivään
palette-rotate-right = Kierrä myötäpäivään
palette-flip-horizontal = Peilaa vaakasuunnassa
palette-flip-vertical = Peilaa pystysuunnassa
palette-toggle-fullscreen = Koko näyttö päälle/pois
palette-navigator = Näytä tai piilota navigaattori
palette-loupe = Suurennuslasi
palette-pixel-grid = Näytä tai piilota pikseliruudukko
palette-info-panel = Näytä tai piilota tietopaneeli
palette-copy-image = Kopioi kuva
# machine-drafted, needs native review
palette-read-codes = Lue koodit
# machine-drafted, needs native review
palette-find-text = Etsi tekstiä
# machine-drafted, needs native review
palette-face-boxes = Näytä kasvot
palette-copy-path = Kopioi tiedostopolku
palette-move-to-trash = Siirrä roskakoriin
palette-undo-trash = Kumoa roskakoriin siirto
palette-open = Avaa kuva…
# machine-drafted, needs native review
palette-open-address = Avaa osoite…
palette-settings = Asetukset
# machine-drafted, needs native review
palette-customise-toolbar = Mukauta työkaluriviä…
palette-cheat-sheet = Pikanäppäimet
palette-hud = Näytä tai piilota kuva-aikojen HUD
palette-language = Kieli
palette-theme = Teema
# machine-drafted, needs native review
palette-interface-larger = Suurempi käyttöliittymä
palette-interface-smaller = Pienempi käyttöliittymä
palette-interface-reset = Käyttöliittymä { PERCENT(100) }
palette-quit = Lopeta { -app-name }

## Pikanäppäinluettelo

cheatsheet-title = Pikanäppäimet
cheatsheet-navigation = Selaaminen
cheatsheet-zoom-pan = Zoomaus ja panorointi
cheatsheet-view = Näkymä
cheatsheet-file = Tiedosto
cheatsheet-window = Ikkuna
cheatsheet-help = Ohje
cheatsheet-hold = Pidä { $key } painettuna
cheatsheet-close-hint = Sulje painamalla { $key } tai Esc

## Virheet

# machine-drafted, needs native review
viewer-no-preview = Tiedostossa ei ole esikatselukuvaa.
error-cannot-open = Tiedostoa ei voi avata: { $file }
error-cannot-open-reason = Tiedostoa { $file } ei voi avata: { $reason }
error-unsupported-format = Tiedoston { $file } muotoa ei voi vielä lukea { -app-name }issa.
error-file-truncated = Tiedosto { $file } on vajaa. Näytetään luettavissa oleva osa.
error-file-too-large = Tiedosto { $file } on liian suuri avattavaksi ({ $megapixels } megapikseliä; raja on { $limit } megapikseliä).
error-not-found = Tiedostoa { $file } ei ole enää olemassa.
error-decoder-stopped = purkaja pysähtyi tuottamatta kuvaa
# machine-drafted, needs native review
# Shown after "Can’t open FILE:" for brain data (GIFTI .func/.shape/.label, CIFTI) that is
# drawn on a surface: no .surf.gii with that many vertices was found in the same folder.
error-needs-surface = se tarvitsee aivopinnan (.surf.gii-tiedoston, jossa on { $vertices } kärkipistettä) samassa kansiossa
error-permission-denied = Ei oikeutta lukea tiedostoa { $file }.
error-details = Tiedot
crash-last-time = { -app-name } sulkeutui odottamatta viime kerralla; raportti tallennettiin sijaintiin { $path }
error-trash-failed = Tiedostoa { $file } ei voitu siirtää roskakoriin: { $reason }
# machine-drafted, needs native review
# Syyt, jotka näytetään "Tiedostoa X ei voi avata:" -viestin ja muiden "Ei voitu …:" -viestien jälkeen,
# joten ne alkavat pienellä kirjaimella. Ne kertovat, mitä tapahtui, ja tarvittaessa mitä tehdä.
error-reason-not-found = sitä ei ole enää. Se on ehkä siirretty, nimetty uudelleen tai poistettu.
error-reason-permission = sinulla ei ole oikeutta lukea sitä. Tarkista sen omistaja ja oikeudet.
error-reason-disk-full = levy on täynnä. Vapauta tilaa ja yritä uudelleen.
error-reason-read-only = levy on vain luku -tilassa.
error-reason-truncated = tiedosto on keskeneräinen. Sen kopiointi tai lataus voi olla vielä kesken.
error-reason-is-folder = se on kansio eikä tiedosto.
# A remote store (a Zarr image on a server or in an S3 bucket).
error-reason-remote-denied = palvelin esti pääsyn. Tiedot voivat olla yksityisiä; määritä S3-säilöä varten ensin AWS-tunnistetiedot.
error-reason-remote-missing = tässä osoitteessa ei ole Zarr-tietoja.
# $status is an HTTP status code ("500").
error-reason-remote-status = palvelin vastasi virheellä { $status }.
error-reason-offline = palvelimeen ei saada yhteyttä. Tarkista osoite ja yhteys.
# machine-drafted, needs native review
error-reason-network-off = verkko on pois päältä kohdassa Asetukset › Yksityisyys.
error-reason-timed-out = asema tai palvelin ei vastannut ajoissa. Tarkista yhteys ja yritä uudelleen.
error-reason-memory = muisti ei riitä. Sulje muita kuvia tai sovelluksia ja yritä uudelleen.
error-reason-unsupported = se on muodossa, jota { -app-name } ei vielä osaa lukea.
# $detail on dekooderin oma, tekninen ja englanninkielinen kuvaus ("12-bit lossless JPEG").
error-reason-unsupported-detail = se käyttää ominaisuutta, jota { -app-name } ei vielä osaa lukea ({ $detail }).
# machine-drafted, needs native review
# A file whose extension several formats share (".img") and whose content is none of them.
# $formats lists the format names, e.g. "ERDAS IMAGINE, ADSC SMV, Analyze 7.5".
error-reason-not-one-of = se ei ole mikään { -app-name }in lukemista .{ $ext }-tiedostoista ({ $formats }).
error-reason-damaged = tiedosto on vioittunut tai ei ole sitä, mitä sen nimi kertoo ({ $detail }).
error-reason-too-large = se on liian suuri avattavaksi.
# machine-drafted, needs native review
error-reason-needs-memory = se tarvitsee { $needed } vapaata RAM-muistia, mutta vapaana on nyt vain { $free }. Sulje joitakin muita sovelluksia ja yritä uudelleen, tai avaa se tietokoneella, jossa on enemmän muistia.
error-needs-memory = Tämä kuva tarvitsee { $needed } vapaata RAM-muistia, mutta vapaana on nyt vain { $free }. Sulje joitakin muita sovelluksia ja yritä uudelleen, tai avaa se tietokoneella, jossa on enemmän muistia.
error-needs-memory-short = Tarvitsee { $needed } vapaata RAM-muistia
error-reason-cancelled = sen avaaminen keskeytettiin.
error-reason-trash-access = sitä ei ole enää, tai sinulla ei ole oikeutta muuttaa sitä.
error-reason-restore-collision = sen kansiossa on jo samanniminen tiedosto.
error-reason-trash-root = kokonaista asemaa ei voi siirtää roskakoriin.
error-reason-not-in-trash = se ei ole enää roskakorissa.
error-reason-program-missing = ohjelmaa ei ole asennettu tai se on siirretty. Valitse se uudelleen tai asenna se uudelleen.
error-reason-program-not-allowed = ohjelmaa ei saa suorittaa. Tarkista, että se on suoritettava.
# machine-drafted, needs native review
error-reason-permission-write = sinulla ei ole oikeutta muuttaa sitä. Tarkista sen omistaja ja oikeudet.
error-reason-file-has-errors = siinä on virheitä. Korjaa ne ensin, jotta mitään siitä ei menetetä.
error-reason-not-dicom = se ei ole DICOM-tiedosto.
error-reason-no-image = siinä ei ole kuvaa.
error-reason-needs-layout = se on otsakkeeton tilavuus. Sen avaamiseen tarvitaan sen koko, näytetyyppi ja tavujärjestys.
error-reason-not-archive = se ei ole arkisto, jota { -app-name } osaa lukea.
error-reason-needs-sandbox = tällaisen arkiston avaa vain { -app-name }in suojattu lukija, joka ei ole käytettävissä.
error-reason-encrypted = se on salattu ja tarvitsee salasanansa.
error-reason-wrong-password = salasana on väärä.
error-reason-checksum = tiedosto on vioittunut: sen sisältö ei vastaa tarkistussummaa.
error-reason-missing-entry = sen sisältä puuttuu tiedosto.
error-reason-busy = toinen ohjelma käyttää sitä. Sulje se siellä ja yritä uudelleen.
error-reason-catalogue-damaged = luettelotiedosto on vioittunut.
error-reason-cannot-open = sen tiedostoa ei voi avata. Tarkista, että sen kansio on olemassa ja että voit kirjoittaa siihen.
error-reason-newer-version = sen kirjoitti { -app-name }in uudempi versio. Päivitä { -app-name }, jotta voit muuttaa sitä.
error-reason-internal = { -app-name }in sisällä meni jotain vikaan. Lokissa on tiedot.
error-reason-volume-too-few = { $count ->
    [one] yksi leike on liian vähän tilavuuden muodostamiseen.
   *[other] { $count } leikettä on liian vähän tilavuuden muodostamiseen.
}
error-reason-volume-no-geometry = jokin leike ei kerro, missä se on tai mihin suuntaan se on.
error-reason-volume-not-parallel = leikkeet eivät ole yhdensuuntaisia.
error-reason-volume-shared-position = useat kuvat jakavat kunkin { $count ->
        [one] { $count } sijainnin
       *[other] { $count } sijainnista
    } (vaiheet, kaiut tai aikasarja).
error-reason-volume-missing = { $count ->
        [one] noin { $count } leike puuttuu.
       *[other] noin { $count } leikettä puuttuu.
    }
error-reason-volume-mismatch = leikkeiden koko tai pikseliväli eroaa.
error-reason-volume-not-grey = leikkeet ovat värillisiä, eivät harmaasävyisiä.
error-reason-volume-skewed = tilavuuden akselit eivät ole suorassa kulmassa.
error-reason-volume-too-large = se tarvitsee { $need } MiB, ja vain { $budget } MiB on sallittu.
error-reason-url-not-specular = se ei ole { -app-name }-linkki.
error-reason-url-too-long = se on liian pitkä.
error-reason-url-damaged = se on vioittunut. Se on ehkä katkennut tai muuttunut kopioitaessa.
error-reason-url-unknown = se pyytää jotain, mitä tämä { -app-name }-versio ei tunne. { -app-name }in päivittäminen voi auttaa.
error-reason-url-not-absolute = sen tiedostopolkujen on oltava täydellisiä polkuja.
error-reason-url-no-paths = se ei nimeä yhtään tiedostoa.
error-reason-url-too-many = se nimeää liian monta tiedostoa.

## Ilmoitukset

# machine-drafted, needs native review
# A background task (a listing, an export, a file operation) stopped part way; $reason is
# error-reason-internal.
background-failed = Ei voitu viimeistellä: { $reason }
toast-moved-to-trash = Siirretty roskakoriin
toast-moved-to-trash-named = { $file } siirretty roskakoriin
toast-moved-to-trash-count =
    { $count ->
        [one] { $count } kuva siirretty roskakoriin
       *[other] { $count } kuvaa siirretty roskakoriin
    }
toast-undo = Kumoa
toast-restored = { $file } palautettu
toast-copied-image = Kuva kopioitu
toast-copied-path = Polku kopioitu
toast-copied-report = Raportti kopioitu
toast-copied-value = Arvo kopioitu
toast-loop-start = Takaisin ensimmäiseen kuvaan
toast-loop-end = Siirrytty viimeiseen kuvaan
# machine-drafted, needs native review
toast-interface-size = Käyttöliittymän koko { PERCENT($percent) }
toast-language-changed = Kieleksi vaihdettu: { $language }

## Tyhjät tilat

empty-drop = Pudota kuva tähän tai paina { $shortcut }
empty-open-button = Avaa kuva…
empty-drop-release = Avaa vapauttamalla
empty-folder = Kansiossa ei ole kuvia, joita { -app-name } voi avata.
# machine-drafted, needs native review
# "Avaa kuva…" -painikkeen alla tyhjässä ikkunassa: mistä kaikki muu löytyy.
empty-hint-keys = Paina { $cheatsheet } nähdäksesi pikanäppäimet tai { $palette } löytääksesi minkä tahansa komennon.
empty-hint-cheatsheet = Paina { $cheatsheet } nähdäksesi pikanäppäimet.
empty-hint-palette = Paina { $palette } löytääksesi minkä tahansa komennon.
# Latausosoittimen alla, kun tiedoston avaaminen kestää hetken.
viewer-opening = Avataan { $file }…
fullscreen-exit-hint = Poistu koko näytöstä painamalla Esc
# Näytetään koko näyttöön siirryttäessä; lähennettynä Esc palauttaa ensin zoomauksen.
fullscreen-back-hint = Palauta zoomaus tai poistu koko näytöstä painamalla Esc
# Pikanäppäinluettelo ja komentopaletti: mitä Esc tekee, yksi vaihe painallusta kohden.
action-back = Sulje paneeli (valintaikkunassa ensin askel taaksepäin), palauta zoomaus, poistu koko näytöstä; galleriassa kansio ylöspäin

## Kielivalitsin

language-picker-title = Kieli
# machine-drafted, needs native review
language-picker-search = Hae kieliä
# $percent on valmiiksi muotoiltu prosenttiluku, esim. "87 %".
language-completeness = { $percent } käännetty
language-system-default = Järjestelmän oletus ({ $language })
language-pseudo-accented = Pseudokieli: aksentoitu ja pidempi
language-pseudo-rtl = Pseudokieli: peilattu oikealta vasemmalle

## Tietoja-ikkuna

about-title = Tietoja { -app-name }ista
about-version = Versio { $version }
about-tagline = Nopea ja uskollinen kuvien katseluohjelma.
about-copyright = © { $year } { -app-name }in tekijät
about-license = Lisensoitu MIT License / Apache License 2.0 -lisenssillä.
about-geonames = Paikannimet: GeoNames (geonames.org), lisenssi Creative Commons Nimeä 4.0.
about-unicode = Aluekohtaiset tiedot: Unicode CLDR, ICU4X:n kautta. © Unicode, Inc.
# machine-drafted, needs native review
about-lensfun = Objektiiviprofiilit Lensfun-tietokannasta (lensfun.github.io), lisenssi Creative Commons Attribution-ShareAlike 3.0.
about-third-party = Kolmansien osapuolten lisenssit
about-third-party-file = { -app-name } sisältää kolmansien osapuolten ohjelmistoja ja tietoja omilla lisensseillään; katso THIRD-PARTY.md, joka asennetaan { -app-name }in mukana.
about-copy-info = Kopioi järjestelmätiedot
about-close = Sulje

## Katseluikkuna (Stage 1 -sovellus)

app-close = Sulje
# Merkki kuvassa, jonka tiedosto päättyi ennenaikaisesti.
app-truncated-badge = Vajaa tiedosto
app-info-summary = Yhteenveto
app-info-all-fields = Kaikki kentät
app-info-copy-value = Kopioi arvo
app-info-copy-field = Kopioi kentän { $field } arvo
app-info-loading = Luetaan metatietoja…
# machine-drafted, needs native review
# Kaikki kentät, kun tiedosto ei tallenna muita metatietoja kuin pikselinsä.
app-info-no-fields = Tiedostossa ei ole metatietokenttiä.
# Gain-map row of the info summary (machine-drafted, needs native review)
app-info-gain-map = HDR-vahvistuskartta
app-info-gain-map-headroom = Jopa { $headroom }× SDR-kirkkaus
app-copy-failed = Kopiointi epäonnistui: { $reason }
app-restore-failed = Tiedostoa { $file } ei voitu palauttaa: { $reason }
app-open-dialog-title = Avaa kuva
app-open-dialog-filter = Tuetut tiedostot
app-open-dialog-all = Kaikki tiedostot
app-open-group-images = Kuvat
app-open-group-raw = Kameran RAW
app-open-group-video = Video
app-open-group-sound = Ääni
app-open-group-documents = Asiakirjat
app-open-group-drawings = Piirrokset ja CAD
app-open-group-models = 3D-mallit
app-open-group-maps = Kartat ja reitit
app-open-group-science = Tiede ja lääketiede
app-open-group-fonts = Fontit
app-open-group-other = Muut

## Renderöijän tiedot (Tietoja, Asetukset › Lisäasetukset)

# machine-drafted, needs native review
renderer-title = Renderöijän tiedot
renderer-section-adapter = Näytönohjain
renderer-section-limits = Rajat ja ominaisuudet
renderer-section-display = Näyttö
renderer-section-memory = Muisti
renderer-section-decoders = Valinnaiset dekooderit
renderer-unknown = Tuntematon
# The PCI vendor and device IDs ("1002:747e").
renderer-ids = Valmistajan ja laitteen tunnus
# Wayland, X11, Windows or macOS.
renderer-display-server = Näyttöpalvelin
renderer-features = Käytetyt ominaisuudet
renderer-surface-format = Ikkunan muoto
renderer-surface-formats = Tarjotut muodot
renderer-present-mode = Esitystila
renderer-present-modes = Tarjotut tilat
renderer-hdr = HDR
# $headroom is how much brighter than white the display goes ("4.0").
renderer-hdr-on = Käytössä ({ $headroom }× headroom)
renderer-hdr-off = Ei käytössä (näyttö pystyy näyttämään HDR:ää)
renderer-hdr-unavailable = Ei käytettävissä tällä näytöllä
renderer-decode-budget = Dekoodaus voi käyttää
renderer-decode-budget-detail = { PERCENT(80) } vapaasta muistista, jaettuna kaikkien avattavien kuvien kesken.
renderer-display-cache = Näytön välimuisti

## Suorituskyky-HUD (F12)

app-hud-scale = Mittakaava
app-hud-level = Taso
app-hud-level-thumbnail = Upotettu pikkukuva
app-hud-level-full = Täysi tarkkuus
# $denominator on 2, 4 tai 8: kuva purettu 1/2-, 1/4- tai 1/8-kokoon.
app-hud-level-scaled = 1/{ $denominator }
app-hud-open = Avaus
app-hud-decode = Purku
app-hud-upload = Siirto
app-hud-frame = Kehys
app-hud-threads = Säikeet
app-hud-threads-value = { $threads } ({ $queued } jonossa)
app-hud-tiles = Ruudut
app-hud-tiles-value = { $tiles }/{ $capacity }
app-hud-ram = Muisti
app-hud-vram = GPU-muisti
app-hud-gpu = GPU

# Roskakoriin siirron vahvistus
app-confirm-trash-title = Siirretäänkö { $file } roskakoriin?
# machine-drafted, needs native review
app-confirm-trash-sidecars = Myös sivutiedostot siirtyvät roskakoriin: { $files }
app-cancel = Peruuta
# Varoitus ennen hyvin suuren kuvan purkamista. $need ja $available ovat muistimääriä
# ("3,2 GiB").
toast-low-memory = Kuva voi tarvita noin { $need } muistia, mutta vain { $available } on vapaana. Lataus voi olla hidas tai epäonnistua.

# Laitteistoraportti (Asetukset → Lisäasetukset). Sovellus täyttää arvot.
hw-report-title = Laitteistoraportti
hw-report-copy = Kopioi raportti
hw-report-version = Versio
hw-report-cpu = CPU
hw-report-cores = Ytimet
hw-report-cores-value = { $logical } loogista, { $physical } fyysistä
hw-report-performance-cores = Suorituskykyytimet
hw-report-cpu-features = CPU:n pikapolut
hw-report-none = Ei mitään
hw-report-memory = Muisti
hw-report-memory-value = { $total } yhteensä, { $available } vapaana
hw-report-graphics-api = Grafiikkarajapinta
hw-report-driver = Ajuri
hw-report-max-texture = Suurin tekstuuri
hw-report-pixels = { $width } × { $height } px
hw-report-decode-threads = Purkusäikeet
hw-report-ram-cache = Kuvien muistivälimuisti
hw-report-vram-cache = GPU-muistin välimuisti
hw-report-processing = Käsittely
hw-report-path-jpeg-decode = JPEG-purku
hw-report-path-ycbcr-to-rgb = Värimuunnos
hw-report-path-png-unfilter = PNG-purku
hw-report-path-heic-decode = HEIC-purku
hw-report-path-inference = Kuva-analyysi
hw-report-engine-cpu = CPU
hw-report-engine-cpu-parallel = CPU, rinnakkain
hw-report-engine-gpu = GPU
hw-report-engine-hw-decoder = laitteistopurkaja
hw-report-engine-npu = NPU
hw-report-build-debug = virheenjäljitysversio
hw-report-build-release = julkaisuversio

## Saavutettavuus (ruudunlukijat)
# Kuva ruudunlukijan ilmoittamana: tiedostonimi ja koko pikseleinä.
a11y-image = { $name }, { $width } × { $height } pikseliä
# Alue, jolla lyhyet ilmoitukset näkyvät.
a11y-notifications = Ilmoitukset
# machine-drafted, needs native review
a11y-transport-playback = Toisto
a11y-transport-pages = Sivut
a11y-settings-sections = Osiot

## Read codes (QR codes in the picture) (machine-drafted, needs native review)
codes-placeholder = Tämän kuvan koodit
codes-looking = Etsitään koodeja…
codes-none = Kuvasta ei löytynyt koodeja.
codes-failed = Kuvaa ei voitu lukea: { $reason }
codes-copy = Kopioi ”{ $text }”
# $kind is "QR", $version a number.
codes-kind = { $kind } { $version }
# Data Matrix: its size in modules, e.g. "Data Matrix 12×12".
# machine-drafted, needs native review
codes-kind-size = { $kind } { $rows }×{ $cols }
codes-open-row = Avaa linkki kohteeseen { $host }…
codes-network-off = Mikään verkko ei ole päällä, joten linkit eivät avaudu.
codes-open-title = Avataanko linkki kohteeseen { $host }?
codes-open-detail = Linkki avautuu verkkoselaimessa. Avaa vain linkkejä, joihin luotat.
codes-open = Avaa selaimessa
# machine-drafted, needs native review
# Find text: the text in the picture shown, read by the Local AI pack's Text models.
ocr-placeholder = Tämän kuvan teksti
# machine-drafted, needs native review
ocr-looking = Luetaan tekstiä…
# machine-drafted, needs native review
ocr-none = Kuvasta ei löytynyt tekstiä.
# machine-drafted, needs native review
ocr-failed = Tekstiä ei voitu lukea: { $reason }
# machine-drafted, needs native review
ocr-needs-pack = Etsi tekstiä tarvitsee paikallisen tekoälypaketin tekstimallit (Asetukset › Paikallinen tekoäly).
# machine-drafted, needs native review
ocr-loading = Ladataan tekstimalleja…
# machine-drafted, needs native review
# A row: choosing it copies the line. $text is the line (one line, shortened).
ocr-copy = Kopioi ”{ $text }”
# machine-drafted, needs native review
ocr-copy-all = Kopioi kaikki teksti
# machine-drafted, needs native review
# A row's note: how sure the reading is, and the add-on that read it. $percent is formatted.
ocr-confidence = Varmuus { $percent }

## Hardware report panel (machine-drafted, needs native review)
hw-report-section-system = Järjestelmä
hw-report-section-gpu = GPU
hw-report-section-decoding = Purku
hw-report-operating-system = Käyttöjärjestelmä
hw-report-adapter = Sovitin
hw-report-gpu-type = Tyyppi
hw-report-gpu-init = Käynnistysaika
hw-report-max-layers = Tekstuurikerrokset
hw-report-compression = Tekstuurien pakkaus
hw-report-unified-memory = CPU:n kanssa jaettu muisti
hw-report-display = Näyttö
hw-report-hw-decode = Laitteistopurku
hw-report-core-cluster = Ydinryhmä
hw-report-power = Virta
hw-report-calibration = Nopeusmittaukset
hw-report-codec-others = Muut
hw-report-performance-cores-value = { $count }/{ $total }
hw-report-cluster-value = { $name }: { $physical } fyysistä, { $logical } loogista
hw-report-thermal-nominal = Normaali lämpötila
hw-report-thermal-fair = Lämmin
hw-report-thermal-serious = Kuuma, hidastaa
hw-report-thermal-critical = Ylikuumentunut
hw-report-power-battery = Akkuvirralla ({ $percent })
hw-report-power-battery-unknown = Akkuvirralla
hw-report-power-mains = Verkkovirralla
hw-report-power-mains-percent = Verkkovirralla ({ $percent })
hw-report-power-saving = Virransäästö käytössä
hw-report-gpu-discrete = Erillinen
hw-report-gpu-integrated = Integroitu
hw-report-gpu-virtual = Virtuaalinen
hw-report-gpu-software = Ohjelmisto, CPU:lla
hw-report-gpu-other = Muu
hw-report-gpu-unavailable = Ei saatavilla
hw-report-ms = { $ms } ms
hw-report-yes = Kyllä
hw-report-no = Ei
hw-report-unified-platform-differs = Järjestelmä ilmoittaa toisin
hw-report-display-value = { $fps } Hz, HDR-varaa nyt { $now }, enintään { $max }
hw-report-hw-decode-value = { $api } laitteella { $device }
hw-report-hw-decode-checking = Tarkistetaan yhä…
hw-report-codec-up-to = Enintään { $width } × { $height }
hw-report-codec-none = Ei saatavilla
hw-report-codec-h264-encode = H.264-koodaus
hw-report-supported = tuettu: { $list }
hw-report-unsupported = ei tuettu: { $list }
hw-report-calibration-done = Mitattu tällä tietokoneella
hw-report-calibration-pending = Ei vielä mitattu
hw-report-paths-power-saving = Virransäästö: vähemmän energiaa kuluttavia polkuja suositaan
hw-report-paths-caption = Polut { $mpx } megapikselin kuvalle
hw-report-col-task = Tehtävä
hw-report-col-path = Polku
hw-report-col-time = Aika
hw-report-col-cpu = CPU-aika (kaikki säikeet)
hw-report-col-runs = Mittaukset
hw-report-col-use = Käyttö
hw-report-runs-untested = Ei testattu
hw-report-use-viewing = Katselu ({ $reason })
hw-report-use-background = Tausta ({ $reason })
hw-report-reason-only = ainoa vaihtoehto
hw-report-reason-faster = nopein
hw-report-reason-cheaper = vähiten CPU-aikaa
hw-report-reason-lower-power = vähiten energiaa
hw-report-reason-explore = kokeilussa
hw-report-reason-forced = valittu Asetuksissa
hw-report-reason-prefer-gpu = Suosi GPU:ta
hw-report-reason-cpu-only = Vain CPU
hw-report-reason-calibrate = mitataan
hw-report-reason-scaled = pienennetty koko
hw-report-trust-unverified = Ei vielä tarkistettu
hw-report-trust-blocked = Poistettu käytöstä virheen jälkeen
hw-report-trust-absent = Ei tässä tietokoneessa

## Keyboard cheat sheet tabs and sections (machine-drafted, needs native review)
cheatsheet-mode-gallery = Galleria
cheatsheet-mode-compare = Vertailu
cheatsheet-gallery-browse = Selaaminen
cheatsheet-gallery-select = Valitseminen
cheatsheet-gallery-mark = Arvioi ja merkitse
cheatsheet-gallery-move = Siirry kuvien välillä
cheatsheet-gallery-page = Sivu ylös tai alas
cheatsheet-gallery-ends = Ensimmäinen tai viimeinen kuva
cheatsheet-gallery-up = Ylös yläkansioon
cheatsheet-gallery-history-back = Takaisin edelliseen kansioon
cheatsheet-gallery-history-forward = Eteenpäin seuraavaan kansioon
cheatsheet-gallery-back = Sulje päällimmäinen näkymä ja palaa sitten
cheatsheet-gallery-viewer = Takaisin katseluun
cheatsheet-gallery-album-add = Lisää viimeisimpään albumiin
cheatsheet-gallery-select-one = Valitse tai poista valinta
cheatsheet-gallery-quick-look = Pikakatselu
cheatsheet-gallery-filter = Suodata
cheatsheet-gallery-bigger = Suuremmat pikkukuvat
cheatsheet-gallery-smaller = Pienemmät pikkukuvat
cheatsheet-gallery-default-size = Pikkukuvien vakiokoko
cheatsheet-gallery-refresh = Päivitä kansio
cheatsheet-gallery-menu = Pikavalikko
cheatsheet-compare-views = Vertailu
cheatsheet-compare-overlay = Päällekkäin tai rinnakkain
cheatsheet-compare-difference = Ero (uudelleen: seuraava laji)
cheatsheet-compare-step-down = Vähemmän sekoitusta tai eron vahvistusta
cheatsheet-compare-step-up = Enemmän sekoitusta tai eron vahvistusta
cheatsheet-compare-pick = Kuva, johon näppäimet vaikuttavat
cheatsheet-compare-close = Sulje vertailu
cheatsheet-channels = Kanavat
cheatsheet-medical-window = Ikkunointi
cheatsheet-medical-measure = Mittaa ja merkitse
cheatsheet-medical-frames = Kehykset ja sarjat
# machine-drafted, needs native review
cheatsheet-medical-3d = 3D ja uudelleenleikkaus
cheatsheet-medical-analysis = Segmentointi, fuusio ja PET
# machine-drafted, needs native review
cheatsheet-medical-studies = Tutkimukset ja jakaminen
cheatsheet-palette-only-title = Komentopaletissa ({ $key })
cheatsheet-palette-only-title-nokey = Komentopaletissa
cheatsheet-palette-only-tag = Paletti
cheatsheet-palette-only-line = { $action }: ei näppäintä; komentopaletissa ({ $key })
cheatsheet-palette-only-line-nokey = { $action }: ei näppäintä; komentopaletissa
# Esc pikanäppäinluettelossa (paletin pitkä kuvaus ei mahdu).
cheatsheet-back = Sulje tai palaa takaisin

## Hardware video encoding (machine-drafted, needs native review)
hw-report-path-video-encode = Videon vienti
hw-report-engine-hw-encoder = laitteistokoodaaja

## Path timing units (machine-drafted, needs native review)
hw-report-task-thumbnail = { $task } (pikkukuvaa kohden, koko { $size })
hw-report-task-frame = { $task } (kehystä kohden, koko { $size })

## Hardware report, software-only formats (machine-drafted, needs native review)
hw-report-codec-software = Vain ohjelmistolla

about-natural-earth = Karttatiedot: Natural Earth (naturalearthdata.com), vapaasti käytettävissä.
app-info-show-on-map = Näytä kartalla
