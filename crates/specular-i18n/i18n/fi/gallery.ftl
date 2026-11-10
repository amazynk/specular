# machine-drafted, needs native review
# Specular — galleria (pikkukuvatila, DESIGN §10) ja Asetukset → Galleria.

## Paletti ja valikot

palette-gallery = Galleria
gallery-palette-view = Gallerianäkymä…
gallery-palette-layout = Asettelu
gallery-palette-sort = Lajittele
gallery-palette-group = Ryhmittele
gallery-palette-filter = Suodata…
gallery-palette-quick-filters = Pikasuodattimet
gallery-palette-details = Tiedot-näkymä
gallery-palette-info-overlay = Tiedot pikkukuvissa
gallery-palette-stacks = Pinoa RAW+JPEG ja sarjat
gallery-palette-subfolders = Sisällytä alikansiot
gallery-palette-compare = Vertaa valittuja
gallery-palette-rate = Arvioi…
gallery-palette-settings = Gallerian asetukset…
gallery-view-menu = Näytä
gallery-more-menu = Lisää
gallery-open = Avaa
gallery-new-window = Avaa uudessa ikkunassa
gallery-reveal = Näytä kansiossa
gallery-properties = Ominaisuudet
gallery-copy-paths = Kopioi polut
gallery-select-all = Valitse kaikki
gallery-select-none = Poista valinnat
gallery-select-invert = Käännä valinta
gallery-next-unrated = Seuraava arvioimaton
gallery-stack-toggle = Laajenna tai supista pino
gallery-run-on = { $name } (tiedostoja: { $count })
gallery-depth-off = Vain tämä kansio
gallery-depth-all = Kaikki alikansiot
gallery-depth-levels = { $depth ->
    [one] Yksi alikansiotaso
   *[other] { $depth } alikansiotasoa
}

## Tila ja tyhjät tilat

gallery-items = { $count ->
    [one] { $count } kohde
   *[other] { $count } kohdetta
}
gallery-selected = { $count }/{ $total } valittu
gallery-shown-of = { $count }/{ $total } näytetään
gallery-listing = Luetaan kansiota… { $count }
gallery-empty = Kansiossa ei ole kuvia.
gallery-empty-filter = Mikään kuva ei vastaa suodatinta.
gallery-list-error = Kansiota ei voi lukea: { $reason }
gallery-no-folder = Ei avattua kansiota
gallery-kind-folder = Kansio
gallery-drag-count = { $count ->
    [one] { $count } tiedosto
   *[other] { $count } tiedostoa
}
gallery-badge-pages = { $count } s.

## Suodatinpalkki

# machine-drafted, needs native review
# The filter field takes words (matched against names) and field:value terms; the field
# names and words of the query (rating, camera, ext, is:pick, …) stay in English.
gallery-filter-query-placeholder = Nimi tai rating>=3 camera:X-T5 …
gallery-filter-clear = Tyhjennä suodatin
gallery-filter-rating = { $stars ->
    [one] ★ 1 tai enemmän
   *[other] ★ { $stars } tai enemmän
}
gallery-filter-label = Värimerkki: { $label }
gallery-filter-picks = Valitut
gallery-filter-hide-rejects = Piilota hylätyt
gallery-filter-unrated = Arvioimattomat
gallery-filter-raw = RAW
gallery-filter-animated = Animoidut
gallery-filter-hdr = HDR
gallery-filter-landscape = Vaaka
gallery-filter-portrait = Pysty

## Query errors and facets

gallery-query-unknown-field = Tuntematon kenttä ”{ $field }”
gallery-query-unknown-field-suggest = Tuntematon kenttä ”{ $field }”: tarkoititko ”{ $suggestion }”?
gallery-query-missing-value = ”{ $field }” tarvitsee arvon
gallery-query-bad-number = ”{ $value }” ei ole luku ({ $field })
gallery-query-bad-date = ”{ $value }” ei ole päivämäärä ({ $field }): käytä 2024, 2024-05 tai 2024-05-12
gallery-query-bad-word = ”{ $value }” ei sovi kenttään { $field }: käytä { $words }
# machine-drafted, needs native review
gallery-query-bad-exif-tag = ”{ $value }” ei ole Exif-tunnisteen nimi
gallery-query-no-compare = Kenttää { $field } ei voi verrata tässä merkeillä < tai >
gallery-query-unterminated = Lopettava lainausmerkki puuttuu
gallery-query-unavailable = ”{ $field }” vaatii kirjaston (ei käytettävissä tässä)
gallery-facets = Fasetit
gallery-facets-counting = Lasketaan…
gallery-facets-empty = Ei laskettavaa
gallery-facets-search = Hae faseteista
gallery-facets-no-match = Ei vastaavia arvoja
# The gallery's top bar button (its tooltip): back to the start screen.
gallery-home = Aloitusnäkymä
gallery-facet-format = Muoto
gallery-facet-camera = Kamera
gallery-facet-lens = Objektiivi
gallery-facet-rating = Arvio
gallery-facet-year = Vuosi
gallery-facet-orientation = Suunta
gallery-facet-label = Värimerkki
gallery-facet-properties = Ominaisuudet
gallery-facet-square = Neliö
gallery-facet-alpha = Läpinäkyvyys
gallery-facet-gps = Sijainti
gallery-facet-multipage = Useita sivuja
# Screen readers: a facet value, e.g. “Camera: X-T5, 12 files”.
gallery-facet-a11y-value = { $facet }: { $value }, { $count ->
    [one] { $count } tiedosto
   *[other] { $count } tiedostoa
}

## Ryhmät

# An event group's header: the first and the last day of a shooting session.
gallery-group-event = { $start } – { $end }
gallery-group-event-place = { $place } · { $dates }
gallery-group-this-folder = Tämä kansio
gallery-group-unknown = Tuntematon
gallery-group-unrated = Arvioimaton

## Karsinta

gallery-stars = { $stars ->
    [one] 1 tähti
   *[other] { $stars } tähteä
}
gallery-rate-none = Ei arviota
gallery-pick = Valitse
gallery-reject = Hylkää
gallery-unflag = Ei merkintää
gallery-label-red = Punainen
gallery-label-yellow = Keltainen
gallery-label-green = Vihreä
gallery-label-blue = Sininen
gallery-label-none = Ei värimerkkiä
gallery-compare-hint = Valitse 2–4 kuvaa vertailtavaksi.

## Tiedostot

gallery-trash-count = { $count ->
    [one] Siirrä { $count } tiedosto roskakoriin
   *[other] Siirrä { $count } tiedostoa roskakoriin
}
gallery-moved-to = { $count ->
    [one] { $count } tiedosto siirretty kansioon { $folder }
   *[other] { $count } tiedostoa siirretty kansioon { $folder }
}
gallery-copied-to = { $count ->
    [one] { $count } tiedosto kopioitu kansioon { $folder }
   *[other] { $count } tiedostoa kopioitu kansioon { $folder }
}
gallery-move-failed = Tiedostoa { $file } ei voitu siirtää: { $reason }
gallery-drop-move = Siirrä kansioon { $folder }
gallery-drop-copy = Kopioi kansioon { $folder }
gallery-move-here = Siirrä valitut tähän
gallery-copy-here = Kopioi valitut tähän
gallery-not-an-image = { $file } ei ole kuva, jonka { -app-name } voi avata.
gallery-cache-cleared = Pikkukuvavälimuisti tyhjennetty
# machine-drafted, needs native review
gallery-thumb-failed = Ei pikkukuvaa: { $reason }
gallery-cache-clear-failed = Pikkukuvavälimuistia ei voitu tyhjentää: { $reason }

## Ominaisuudet

gallery-prop-name = Nimi
gallery-prop-folder = Kansio
gallery-prop-size = Tiedostokoko
gallery-prop-dimensions = Mitat
gallery-prop-format = Muoto
gallery-prop-date = Päivämäärä
gallery-prop-camera = Kamera
gallery-prop-lens = Objektiivi
gallery-prop-rating = Arvio
gallery-prop-label = Värimerkki
gallery-prop-items = Kohteet

## Ruudunlukijat

gallery-a11y-list = { $folder }, { $count ->
    [one] { $count } kohde
   *[other] { $count } kohdetta
}
gallery-a11y-dims = { $width } × { $height }
gallery-a11y-folder = kansio
gallery-a11y-up = Ylös kansioon { $folder }
gallery-a11y-archive = arkisto
gallery-a11y-stack = pino, { $count } kuvaa
gallery-a11y-filter-bar = Suodatin
gallery-a11y-filter-field = Suodata nimen tai kentän mukaan
gallery-a11y-chip = Suodatusehto { $term }
gallery-a11y-chip-remove = Poista painamalla
gallery-a11y-results = { $count ->
    [0] Ei vastaavia kohteita
    [one] { $count } vastaava kohde
   *[other] { $count } vastaavaa kohdetta
}

## Asetukset → Galleria

settings-section-gallery = Galleria
settings-gallery-layout = Asettelu
settings-gallery-layout-help = Neliöt rajaavat pikkukuvat ruudukkoon; tasatut rivit säilyttävät kunkin kuvan muodon; rajaamaton sovittaa kokonaiset kuvat samankokoisiin soluihin.
settings-gallery-layout-squares = Neliöt
settings-gallery-layout-justified = Tasatut rivit
settings-gallery-layout-uncropped = Rajaamaton
settings-gallery-layout-details = Tiedot
settings-gallery-thumbnail-size = Pikkukuvien koko
settings-gallery-thumbnail-size-help = Galleriassa myös Ctrl+hiiren rulla, nipistys tai Ctrl+plus ja miinus.
settings-gallery-captions = Näytä tiedostonimet
settings-gallery-captions-help = Kuvateksti jokaisen pikkukuvan alla.
settings-gallery-badges = Näytä merkit
settings-gallery-badges-help = Pienet merkit RAW-, animoiduille, HDR- ja monisivuisille kuville sekä arviolle ja värimerkille.
settings-gallery-sort-order = Lajittele
settings-gallery-sort-order-help = Gallerian järjestys. Kansiot ovat aina ensin.
settings-gallery-sort-order-name = Nimi
settings-gallery-sort-order-date-taken = Kuvauspäivä
settings-gallery-sort-order-modified = Muokkauspäivä
settings-gallery-sort-order-size = Tiedostokoko
settings-gallery-sort-order-type = Tiedostotyyppi
settings-gallery-sort-order-dimensions = Mitat
settings-gallery-sort-descending = Käänteinen järjestys
settings-gallery-sort-descending-help = Lajittele viimeisestä ensimmäiseen.
settings-gallery-group-by = Ryhmittele
settings-gallery-group-by-help = Osioiden otsikot, jotka pysyvät ylhäällä vierittäessä.
settings-gallery-group-by-none = Ei mitään
settings-gallery-group-by-day = Päivä
settings-gallery-group-by-month = Kuukausi
settings-gallery-group-by-year = Vuosi
settings-gallery-group-by-event = Tapahtuma
settings-gallery-group-by-folder = Kansio
settings-gallery-group-by-camera = Kamera
settings-gallery-group-by-format = Muoto
settings-gallery-group-by-rating = Arvio
settings-gallery-group-by-target = Kohde
settings-gallery-group-by-filter = Suodin
settings-gallery-per-folder-view = Muista näkymä kansiokohtaisesti
settings-gallery-per-folder-view-help = Asettelu, koko, lajittelu ja ryhmittely tallennetaan kansiokohtaisesti; muuten käytetään kaikkialla samaa näkymää.
settings-gallery-open-folders-in-gallery = Avaa kansiot galleriassa
settings-gallery-open-folders-in-gallery-help = Kun avaat kansion, näytä ensin sen pikkukuvat ensimmäisen kuvan sijaan.
settings-gallery-escape-opens-gallery = Esc avaa gallerian
settings-gallery-escape-opens-gallery-help = Kun suljettavaa ei ole, Esc siirtyy katselusta galleriaan sen sijaan, että mitään ei tapahtuisi.
settings-gallery-wrap-around = Nuolinäppäimet kiertävät ympäri
settings-gallery-wrap-around-help = Luettelon lopussa nuolinäppäimet siirtyvät alkuun ja alusta loppuun. Pohjassa pidetty näppäin pysähtyy silti loppuun.
settings-gallery-show-hidden = Näytä piilotiedostot
settings-gallery-show-hidden-help = Sisällytä tiedostot ja kansiot, joiden nimi alkaa pisteellä.
settings-gallery-show-other-files = Näytä muut tiedostot
settings-gallery-show-other-files-help = Sisällytä tiedostot, joita { -app-name } ei voi avata, yleisellä kuvakkeella.
settings-gallery-show-archives = Näytä arkistot
settings-gallery-show-archives-help = Sisällytä sarjakuvat ja muut arkistot (zip, cbz, rar, 7z…) ruutuina. Arkiston voi edelleen avata suoraan.
settings-gallery-subfolder-depth = Alikansiotasot
settings-gallery-subfolder-depth-help = Näytä kuvat myös alikansioista näin monen tason syvyyteen (0: vain tämä kansio).
settings-gallery-stacks = Pinoa liittyvät tiedostot
settings-gallery-stacks-help = Näytä RAW+JPEG-parit ja sarjat yhtenä ruutuna. Laajenna pino painamalla S.
settings-gallery-info-overlay = Tiedot pikkukuvissa
settings-gallery-info-overlay-help = Mitat ja valotus jokaisen pikkukuvan päällä.
settings-gallery-prefer-embedded = Suosi upotettuja esikatseluja
settings-gallery-prefer-embedded-help = Käytä tiedostoon tallennettua esikatselua, kun se on riittävän suuri. Paljon nopeampi, hieman epätarkempi.
settings-gallery-sharper-raw-thumbnails = Terävämmät RAW-pikkukuvat
settings-gallery-sharper-raw-thumbnails-help = Muokatut RAW-kuvat kehitetään pikkukuvia varten 1024 px:n kokoon. Terävämpi, hieman hitaampi. Pois: 512 px.
settings-gallery-rating-sidecars = Kirjoita arviot XMP-sivutiedostoihin
settings-gallery-rating-sidecars-help = Arviot, värimerkit ja merkinnät tallennetaan myös kuvan viereen .xmp-tiedostoon, jota muut kuvasovellukset lukevat.
settings-gallery-cache-size = Pikkukuvavälimuistin koko
settings-gallery-cache-size-help = Vanhimmat pikkukuvat poistetaan, kun välimuisti on täynnä.
settings-gallery-cache-location = Pikkukuvavälimuistin sijainti
settings-gallery-cache-location-help = Nopealla asemalla oleva kansio toimii parhaiten.
settings-gallery-clear-cache = Tyhjennä pikkukuvavälimuisti
settings-gallery-clear-cache-help = Poistaa tallennetut pikkukuvat. Ne luodaan uudelleen selatessa.

## Compare
# machine-drafted, needs native review

compare-side = Rinnakkain
compare-overlay = Päällekkäin
compare-difference = Ero
compare-flicker = Vilkutus
compare-blend = Sekoitus
compare-kind-absolute = Absoluuttinen virhe
compare-kind-squared = Neliövirhe
compare-kind-relative = Suhteellinen virhe
compare-scale-down = Vähemmän herkkä
compare-scale-up = Herkempi
compare-scale = Täysi asteikko { $value }
compare-space = Lineaarinen valo, kuten näytetään
compare-space-help = Virheet ja mittarit mitataan lineaarisessa valossa kuvan näyttötavan mukaan, sävykartoituksen jälkeen, kanavittain alfa mukaan lukien.
compare-metrics = MAE { $mae }  ·  MSE { $mse }  ·  PSNR { $psnr } dB
compare-sampled = näytteistetty koossa { $width } × { $height }
compare-aligned = Tasattu vasempaan yläkulmaan, { $width } × { $height } verrattu
compare-vs = { $b } verrattuna kuvaan { $a }
compare-overlay-label = { $b } kuvan { $a } päällä, { PERCENT($percent) }
compare-pending = Mitataan…
# machine-drafted, needs native review
compare-diff-failed = Eroa ei voi näyttää: { $reason }
compare-no-overlap = Näkymässä ei ole verrattavaa
compare-zoom-fit = Sovita
compare-zoom = { PERCENT($percent) }
compare-showing = Näytetään { $name }
compare-a11y-group = Vertailu
compare-a11y-blend = Sekoita kuvien välillä
compare-a11y-zoom = Zoomaus
gallery-palette-compare-side = Vertaa rinnakkain
gallery-palette-compare-overlay = Vertaa päällekkäin
gallery-palette-compare-difference = Vertaa eroa: { $kind }
gallery-palette-compare-flicker = Vilkuta verrattuja kuvia
gallery-palette-compare-actual = Vertaa { PERCENT(100) }:ssa

## Search value list (machine-drafted, needs native review)
gallery-picker-hint = Välilyönti tai Ctrl+Enter valitsee useita

## Search value picker, part 2 (machine-drafted, needs native review)
gallery-facet-profile = Väriprofiili
gallery-facet-date = Kuvauspäivä
gallery-facet-picked = Valitut
gallery-facet-rejected = Hylätyt
gallery-facet-flagged = Valitut tai hylätyt
gallery-facet-rated = Arvioidut
gallery-facet-labelled = Värimerkityt

## Search value picker, part 3 (machine-drafted, needs native review)

gallery-facet-bits = Bittisyvyys
gallery-facet-pages = Sivut
gallery-facet-width = Leveys
gallery-facet-height = Korkeus
gallery-facet-megapixels = Megapikselit
gallery-picker-bits = { $bits }-bittinen
gallery-picker-pages = { $pages ->
    [one] { $pages } sivu
   *[other] { $pages } sivua
}
gallery-picker-pixels = { $px } px
gallery-picker-megapixels = { $mp } Mpx
gallery-picker-group-any = Mikä tahansa ryhmässä
gallery-picker-group-keep = Ehdotus: säilytä
gallery-picker-group-discard = Ehdotus: hylkää
gallery-picker-ops = Vertaa
gallery-picker-op-eq = Yhtä suuri kuin
gallery-picker-op-ge = Vähintään
gallery-picker-op-le = Enintään
gallery-picker-ge = vähintään { $value }
gallery-picker-gt = yli { $value }
gallery-picker-le = enintään { $value }
gallery-picker-lt = alle { $value }

# Map view.
gallery-facet-place = Paikka
gallery-palette-map = Kartta
map-title = Kartta
map-summary = Kuvia, joilla on sijainti: { $count }
map-photos = Kuvia: { $count }
map-zoom-in = Lähennä
map-zoom-out = Loitonna
map-show-all = Näytä kaikki
map-done = Valmis
map-credit = Karttatiedot: Natural Earth
# machine-drafted, needs native review
# With a base map's tiles on screen: $credit is the tile source's credit ("© OpenStreetMap
# contributors").
map-credit-tiles = Kartta: { $credit } · Natural Earth
# The one-time explanation before Specular downloads map tiles from a server; one idea per
# line. $host is the server's name, $size the cache size ("256 MB").
map-basemap-consent-title = Verkon karttaruudut
map-basemap-consent-server = Karttaruudut tulevat palvelimelta { $host }.
map-basemap-consent-sees = Palvelin näkee internetosoitteesi ja alueet, joita katselet.
map-basemap-consent-private = Kuviasi, niiden nimiä ja sijainteja ei lähetetä.
map-basemap-consent-cache = Ruudut tallennetaan tälle tietokoneelle, enintään { $size }.
map-basemap-allow = Käytä verkkokarttaa
map-basemap-decline = Pidä offline-kartta
# Why the base map is not shown; $reason is one of the lines below or a file error.
map-basemap-error = Taustakarttaa ei näytetä: { $reason }
map-basemap-bad-address = karttaruutupalvelimen osoitteessa on oltava {"{"}z{"}"}, {"{"}x{"}"} ja {"{"}y{"}"}.
map-basemap-unknown-tiles = karttatiedoston ruudut ovat tuntematonta tyyppiä.
map-loading = Luetaan kuvien sijainteja…
map-empty = Millään näistä kuvista ei ole sijaintia
map-hidden = Kuvien sijainnit on piilotettu (Asetukset › Metatiedot ja yksityisyys)
map-a11y-group = Kuvien sijaintien kartta
map-a11y-status = Paikkoja näkyvissä: { $count }, zoomaustaso { $zoom }
