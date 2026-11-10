# machine-drafted, needs native review
# Specular — Asetukset-ikkuna. Suomi.
#
# Tunnisteiden rakenne, tuotettu asetusskeemasta (crates/specular-settings):
#   settings-section-<section>              osion otsikko
#   settings-<section>-<key>                asetuksen nimi
#   settings-<section>-<key>-help           ohjeteksti nimen alla
#   settings-<section>-<key>-<value>        valinta-asetuksen yksi vaihtoehto
# Avaimet ja arvot ovat settings.toml-nimiä, joissa "_" on kirjoitettu "-".

## Ikkunan kehys

settings-title = Asetukset
settings-search-placeholder = Hae asetuksista
settings-search-no-results = Ei hakua vastaavia asetuksia: { $query }
settings-reset = Palauta
settings-reset-section = Palauta osio
settings-modified = Muutettu
settings-default-value = Oletus: { $value }
settings-restart-required = Tulee voimaan uudelleenkäynnistyksen jälkeen
settings-restart-now = Käynnistä uudelleen nyt
settings-undo = Kumoa
settings-redo = Tee uudelleen
settings-value-on = Käytössä
settings-value-off = Ei käytössä
settings-value-auto = Automaattinen
settings-value-system = Järjestelmän mukaan
settings-value-system-language = Järjestelmän oletus
settings-value-follow-language = Käyttöliittymän kielen mukaan
settings-value-default-location = Oletussijainti
settings-choose = Valitse…
settings-clear = Tyhjennä
settings-file-reloaded = settings.toml-tiedostoa muutettiin { -app-name }in ulkopuolella, ja se on ladattu uudelleen.
settings-warning-unknown-key = Tuntematon asetus settings.toml-tiedostossa: { $key }. Se säilytetään tiedostossa, mutta ohitetaan.
settings-warning-invalid-value = Virheellinen arvo asetukselle { $key }: { $reason } Sen sijaan käytetään oletusta.
settings-warning-newer-version = settings.toml on kirjoitettu { -app-name }in uudemmalla versiolla. Asetukset, joita tämä versio ei tunne, säilytetään.
settings-warning-syntax = settings.toml-tiedostoa ei voi lukea (rivi { $line }): { $reason } Oletuksia käytetään, kunnes virhe on korjattu.

## Tarkistusvirheet (tulostaa myös `specular --set`)

settings-error-unknown-key = Asetusta ei ole olemassa: { $key }.
settings-error-unknown-key-suggestion = Asetusta ei ole olemassa: { $key }. Tarkoititko { $suggestion }?
settings-error-not-bool = Odotettiin arvoa päällä tai pois (true tai false).
settings-error-not-integer = Odotettiin kokonaislukua.
settings-error-not-number = Odotettiin lukua.
settings-error-out-of-range = Arvon on oltava välillä { $min }–{ $max }.
settings-error-out-of-range-auto = Arvon on oltava 0 (automaattinen) tai välillä { $min }–{ $max }.
settings-error-unknown-option = Odotettiin jotakin seuraavista: { $options }.
settings-error-bad-colour = Odotettiin väriä, esim. #1E1E1E tai #1E1E1E80.
settings-error-bad-language = Odotettiin kielitunnistetta, esim. de, pt-BR tai zh-Hans, tai ”system”.
settings-error-bad-path = Odotettiin tiedoston tai kansion polkua.
settings-error-not-settable = Tämä kohde on komento, eikä sillä ole arvoa.
settings-error-syntax = Odotettiin muotoa avain=arvo, esim. viewing.wheel_behaviour=navigate.
# machine-drafted, needs native review
settings-error-bad-text = Ei voi sisältää sarkaimia, rivinvaihtoja tai muita ohjausmerkkejä.
settings-error-invalid = { $reason }
settings-error-tile-url = Tarvitaan http- tai https-osoite, jossa on {"{"}z{"}"}, {"{"}x{"}"} ja {"{"}y{"}"}.

## Yksiköt

unit-percent = { PERCENT($value) }
unit-milliseconds = { $value } ms
unit-seconds = { $value } s
unit-mebibytes = { $value } MiB
# machine-drafted, needs native review
unit-luminance = { $value } cd/m²
unit-images =
    { $value ->
        [one] { $value } kuva
       *[other] { $value } kuvaa
    }
unit-threads =
    { $value ->
        [one] { $value } säie
       *[other] { $value } säiettä
    }
unit-bytes =
    { $value ->
        [one] { $value } tavu
       *[other] { $value } tavua
    }
unit-kilobytes = { $value } kB
unit-megabytes = { $value } MB
unit-gigabytes = { $value } GB
unit-terabytes = { $value } TB
unit-kibibytes = { $value } KiB
unit-gibibytes = { $value } GiB
unit-tebibytes = { $value } TiB
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

## Osiot

settings-section-general = Yleiset
settings-section-viewing = Katselu
settings-section-browsing = Selaaminen
settings-section-colour = Väri
settings-section-metadata = Metatiedot ja yksityisyys
# machine-drafted, needs native review
settings-section-privacy = Yksityisyys
settings-section-performance = Suorituskyky
settings-section-accessibility = Helppokäyttöisyys
settings-section-keys = Näppäimet
settings-section-advanced = Lisäasetukset

## Yleiset

settings-general-language = Käyttöliittymän kieli
settings-general-language-help = Muutokset tulevat voimaan heti. ”Järjestelmän oletus” seuraa käyttöjärjestelmän kieltä.
settings-general-region-formats = Alueelliset muodot
settings-general-region-formats-help = Miten päivämäärät, kellonajat, luvut ja koot kirjoitetaan. Oletuksena ne seuraavat käyttöliittymän kieltä.
settings-general-theme = Teema
settings-general-theme-help = Ikkunoiden ja säätimien vaalea tai tumma ulkoasu. Kuvia ei koskaan muuteta.
settings-general-theme-system = Järjestelmän mukaan
settings-general-theme-light = Vaalea
settings-general-theme-dark = Tumma
# machine-drafted, needs native review
settings-general-on-start = Kun { -app-name } avataan ilman tiedostoa
settings-general-on-start-help = Mitä näytetään, kun { -app-name } avataan yksinään. Tiedostonhallinnasta tai komentoriviltä avattu tiedosto näkyy aina katseluohjelmassa.
settings-general-on-start-start-screen = Aloitusnäkymä
settings-general-on-start-gallery = Viimeisimmän kansion galleria
settings-general-on-start-library = Kirjasto
settings-general-start-tips = Näytä ”Kokeile näitä” aloitusnäkymässä
settings-general-start-tips-help = Rivi kortteja hyödyllisistä toiminnoista ja niiden näppäimistä. ”Näytä tervetuloa” komentopaletissa tuo sen takaisin.
settings-general-tips = Näytä vinkit
settings-general-tips-help = Lyhyt vinkki kerran, kun toiminnosta voi ensimmäistä kertaa olla apua, kuten näppäin, joka näyttää kansion galleriana. Ei koskaan diaesityksen aikana.
settings-general-tips-shown = Jo näytetyt vinkit
settings-general-tips-shown-help = Jo annetut kertavinkit, jotta niitä ei näytetä uudelleen.
# machine-drafted, needs native review
settings-general-your-name = Nimesi
settings-general-your-name-help = Näkyy lisäämissäsi leimoissa ja merkinnöissä. Tallennetaan vain luomiisi merkintätiedostoihin.
settings-general-single-instance = Avaa tiedostot olemassa olevaan ikkunaan
settings-general-single-instance-help = Kun { -app-name } on jo käynnissä, toinen avattu kuva näytetään siinä ikkunassa uuden käynnistämisen sijaan.
settings-general-file-associations = Tiedostokytkennät…
settings-general-file-associations-help = Tee { -app-name }ista tukemiensa kuvamuotojen oletuskatselin.

## Katselu

settings-viewing-initial-zoom = Alkuzoomaus
settings-viewing-initial-zoom-help = Kuinka suurena juuri avattu kuva näytetään.
settings-viewing-initial-zoom-fit = Sovita ikkunaan
settings-viewing-initial-zoom-fit-if-larger = Sovita suuret kuvat, näytä pienet { PERCENT(100) }:ssa
settings-viewing-initial-zoom-actual-size = Todellinen koko ({ PERCENT(100) })
settings-viewing-initial-zoom-fill = Täytä ikkuna
settings-viewing-wheel-behaviour = Hiiren rulla
settings-viewing-wheel-behaviour-help = Mitä rullan vierittäminen tekee. Pidä Ctrl painettuna toista toimintoa varten.
settings-viewing-wheel-behaviour-help-macos = Mitä rullan vierittäminen tekee. Pidä ⌘ painettuna toista toimintoa varten.
settings-viewing-wheel-behaviour-zoom = Zoomaa
settings-viewing-wheel-behaviour-navigate = Seuraava ja edellinen kuva
settings-viewing-keep-zoom = Säilytä zoomaus kuvien välillä
settings-viewing-keep-zoom-help = Lähennettynä seuraava ja edellinen kuva avautuvat samalla zoomauksella ja näyttävät saman kohdan kuvasta: kätevä tarkennuksen tarkistamiseen sarjasta.
settings-viewing-zoom-animation = Animoi zoomaus
settings-viewing-zoom-animation-help = Zoomaa pehmeästi hyppäämisen sijaan. Poistuu käytöstä automaattisesti, kun ”Vähennä liikettä” on päällä.
settings-viewing-zoom-animation-speed = Zoomausanimaation nopeus
settings-viewing-zoom-animation-speed-help = Kuinka nopeasti animoitu zoomaus asettuu.
settings-viewing-zoom-animation-speed-relaxed = Rauhallinen
settings-viewing-zoom-animation-speed-normal = Normaali
settings-viewing-zoom-animation-speed-fast = Nopea
settings-viewing-background = Tausta
settings-viewing-background-help = Kuvan taustaväri ikkunassa.
settings-viewing-background-theme = Teeman mukainen
settings-viewing-background-black = Musta
settings-viewing-background-dark-grey = Tummanharmaa
settings-viewing-background-light-grey = Vaaleanharmaa
settings-viewing-background-white = Valkoinen
settings-viewing-background-custom = Mukautettu väri
settings-viewing-background-color = Mukautettu taustaväri
settings-viewing-background-color-help = Käytetään, kun Tausta on ”Mukautettu väri”.
settings-viewing-fullscreen-background = Koko näytön tausta
settings-viewing-fullscreen-background-help = Kuvan taustaväri koko näytön tilassa.
settings-viewing-transparency = Läpinäkyvät alueet
settings-viewing-transparency-help = Miten kuvan läpinäkyvät osat piirretään.
settings-viewing-transparency-checkerboard = Shakkiruudukko
settings-viewing-transparency-background = Taustaväri
settings-viewing-transparency-black = Musta
settings-viewing-transparency-white = Valkoinen
settings-viewing-pixel-grid-threshold = Näytä pikseliruudukko alkaen
settings-viewing-pixel-grid-threshold-help = Piirrä ruudukko pikselien väliin, kun on lähennetty vähintään näin paljon.
settings-viewing-smoothing-high-zoom = Suurennetut pikselit
settings-viewing-smoothing-high-zoom-help = Miltä pikselit näyttävät, kun kuva näytetään yli { PERCENT(100) }:n kokoisena.
settings-viewing-smoothing-high-zoom-auto = Pehmeä, terävä { PERCENT(400) }:sta alkaen
settings-viewing-smoothing-high-zoom-smooth = Aina pehmeä
settings-viewing-smoothing-high-zoom-pixelated = Aina terävät neliöt
settings-viewing-double-click = Kaksoisnapsautus kuvassa
settings-viewing-double-click-help = Tyhjän taustan kaksoisnapsautus vaihtaa aina koko näytön tilaa.
settings-viewing-double-click-toggle-zoom = Vaihda sovitetun ja { PERCENT(100) }:n välillä
settings-viewing-double-click-fullscreen = Koko näyttö päälle/pois
settings-viewing-double-click-nothing = Älä tee mitään
settings-viewing-fullscreen-on-open = Avaa kuvat koko näyttöön
settings-viewing-fullscreen-on-open-help = Aloita koko näytön tilassa, kun kuva avataan { -app-name }in ulkopuolelta.
settings-viewing-fullscreen-mode = Koko näytön tyyli
settings-viewing-fullscreen-mode-help = Natiivi koko näyttö käyttää omaa Spacea; välitön koko näyttö peittää nykyisen ilman animaatiota.
settings-viewing-fullscreen-mode-native = Natiivi (oma Space)
settings-viewing-fullscreen-mode-borderless = Välitön
settings-viewing-toolbar-autohide = Piilota työkalurivi automaattisesti
settings-viewing-toolbar-autohide-help = Työkalurivi ja reunanuolet häivytetään, kun osoitin on paikallaan, ja ne palaavat, kun se liikkuu.
settings-viewing-toolbar-autohide-delay = Piilota työkalurivi viiveellä
settings-viewing-toolbar-autohide-delay-help = Kuinka kauan osoittimen on oltava paikallaan ennen kuin työkalurivi häivytetään.
settings-viewing-toolbar-items = Työkalurivin painikkeet
settings-viewing-toolbar-items-help = Mitkä painikkeet työkalurivi näyttää ja missä järjestyksessä.
settings-viewing-toolbar-drag = Järjestä työkalurivi vetämällä
settings-viewing-toolbar-drag-help = Siirrä painiketta vetämällä sitä työkalurivillä. Lyhyt napsautus painaa sitä edelleen; Esc palauttaa vedetyn painikkeen paikalleen.
settings-viewing-toolbar-items-open = Avaa
settings-viewing-toolbar-items-prev = Edellinen kuva
settings-viewing-toolbar-items-next = Seuraava kuva
settings-viewing-toolbar-items-zoom-out = Loitonna
settings-viewing-toolbar-items-zoom-level = Zoomaustaso
settings-viewing-toolbar-items-zoom-in = Lähennä
settings-viewing-toolbar-items-fit = Sovita ikkunaan
settings-viewing-toolbar-items-rotate-left = Kierrä vastapäivään
settings-viewing-toolbar-items-rotate-right = Kierrä myötäpäivään
settings-viewing-toolbar-items-info = Tiedot
settings-viewing-toolbar-items-copy = Kopioi kuva
settings-viewing-toolbar-items-delete = Siirrä roskakoriin
settings-viewing-toolbar-items-palette = Komentopaletti
settings-viewing-toolbar-items-settings = Asetukset
settings-viewing-toolbar-items-fullscreen = Koko näyttö
# machine-drafted, needs native review
settings-viewing-toolbar-items-spacer = Joustava väli
settings-viewing-toolbar-items-separator = Erotin
settings-toolbar-preview = Työkalurivin esikatselu
settings-toolbar-hidden = Piilotetut painikkeet
settings-toolbar-add-spacer = Lisää joustava väli
settings-toolbar-add-separator = Lisää erotin
settings-toolbar-reset = Palauta oletukset
settings-toolbar-remove = Poista { $item }
settings-toolbar-hint = Siirrä riviä vetämällä, tai valitse se ja paina Alt+↑ tai Alt+↓. Sarkain siirtyy hakukentän ja luettelon välillä.
settings-toolbar-move-up = Siirrä ylös
settings-toolbar-move-down = Siirrä alas
settings-toolbar-move-start = Siirrä alkuun
settings-toolbar-move-end = Siirrä loppuun
settings-toolbar-menu-hide = Piilota
settings-toolbar-menu-show = Näytä
settings-toolbar-menu-remove = Poista
settings-toolbar-row-menu = Lisää toimintoja: { $item }
settings-toolbar-position = Sijainti { $index }/{ $count }
settings-toolbar-hidden-state = Piilotettu

## Selaaminen

settings-browsing-sort-order = Lajittele kuvat
settings-browsing-sort-order-help = Järjestys, jossa kansiota selataan. Nimijärjestys asettaa ”img2”:n ennen ”img10”:tä ja noudattaa kielesi aakkosjärjestystä.
settings-browsing-sort-order-natural = Nimi
settings-browsing-sort-order-date-taken = Kuvauspäivä
settings-browsing-sort-order-modified = Muokkauspäivä
settings-browsing-sort-order-size = Tiedostokoko
settings-browsing-sort-order-type = Tiedostotyyppi
settings-browsing-sort-descending = Käänteinen järjestys
settings-browsing-sort-descending-help = Lajittele viimeisestä ensimmäiseen.
settings-browsing-loop-folder = Aloita alusta kansion lopussa
settings-browsing-loop-folder-help = Jatka viimeisen kuvan jälkeen ensimmäisestä ja päinvastoin.
settings-browsing-include-subfolders = Sisällytä alikansiot
settings-browsing-include-subfolders-help = Selaa myös alikansioiden kuvia.
settings-browsing-hold-acceleration = Kiihdytys pidettäessä seuraava- tai edellinen-näppäintä
settings-browsing-hold-acceleration-help = Kuinka nopeasti selaaminen kiihtyy, kun näppäintä tai painiketta pidetään painettuna.
settings-browsing-hold-acceleration-off = Ei kiihdytystä
settings-browsing-hold-acceleration-gentle = Loiva
settings-browsing-hold-acceleration-normal = Normaali
settings-browsing-hold-acceleration-fast = Nopea
settings-browsing-confirm-trash = Kysy ennen roskakoriin siirtoa
settings-browsing-confirm-trash-help = Roskakoriin siirron voi aina kumota, joten { -app-name } ei oletuksena kysy.
settings-browsing-arrows-follow-reading-direction = Nuolinäppäimet seuraavat lukusuuntaa
settings-browsing-arrows-follow-reading-direction-help = Oikealta vasemmalle kirjoitettavissa kielissä vasen nuolinäppäin siirtää seuraavaan kuvaan.

## Väri

settings-colour-management = Värinhallinta
settings-colour-management-help = Näytä värit sellaisina kuin ne on tarkoitettu kuvan ja näytön väriprofiilien avulla.
settings-colour-display-profile = Näytön profiili
settings-colour-display-profile-help = Näyttösi väriprofiili. Automaattinen käyttää käyttöjärjestelmän ilmoittamaa profiilia.
settings-colour-rendering-intent = Toistotapa
settings-colour-rendering-intent-help = Miten värit, joita näyttö ei pysty näyttämään, tuodaan sen alueelle.
settings-colour-rendering-intent-perceptual = Havainnollinen
settings-colour-rendering-intent-relative-colorimetric = Suhteellinen kolorimetrinen
settings-colour-rendering-intent-saturation = Kylläisyys
settings-colour-rendering-intent-absolute-colorimetric = Absoluuttinen kolorimetrinen
settings-colour-black-point-compensation = Mustapisteen kompensointi
settings-colour-black-point-compensation-help = Säilytä varjojen yksityiskohdat, kun näytön musta on vaaleampi kuin kuvan.
settings-colour-untagged-images = Kuvat ilman väriprofiilia
settings-colour-untagged-images-help = Miten näytetään kuvat, jotka eivät kerro käyttämiään värejä.
settings-colour-untagged-images-srgb = Käsittele sRGB:nä
settings-colour-untagged-images-display = Näytä muuttamattomana
settings-colour-dithering = Rasterointi
settings-colour-dithering-help = Lisää hienoa sinistä kohinaa värisyvyyttä vähennettäessä, mikä estää raidoittumisen taivaissa ja liukuvärjäyksissä.
settings-colour-hdr-output = HDR-ulostulo
settings-colour-hdr-output-help = Näytä HDR-kuvat täydellä kirkkaudella sitä tukevilla näytöillä. Automaattinen käyttää HDR:ää, kun näyttö ilmoittaa lisäkirkkaudesta; muuten kirkkaat alueet pakataan hellävaraisesti mahtumaan.
settings-colour-hdr-output-auto = Automaattinen
settings-colour-hdr-output-on = Käytössä
settings-colour-hdr-output-off = Ei käytössä
# machine-drafted, needs native review
settings-colour-ocio-config = OCIO-määritys
settings-colour-ocio-config-help = OpenColorIO-määritys (.ocio) EXR-, Radiance HDR- ja PFM-kuville. Tyhjänä käytetään sitä, jonka $OCIO nimeää.
settings-colour-ocio-display = OCIO-näyttö
settings-colour-ocio-display-help = OCIO-määrityksen näyttö, joka vastaa näyttöäsi.
settings-colour-ocio-display-default = Määrityksen ensimmäinen
settings-colour-ocio-view = OCIO-näkymä
settings-colour-ocio-view-help = Miten scene-linear-kuvat näytetään kyseisellä näytöllä, esimerkiksi sävykartoitettu näkymä.
settings-colour-ocio-view-default = Näytön ensimmäinen
settings-colour-ocio-input = OCIO-syöteväriavaruus
settings-colour-ocio-input-help = Kuvien arvojen väriavaruus.
settings-colour-ocio-input-default = Määrityksen scene-linear-rooli
# machine-drafted, needs native review
settings-colour-proof-profile = Näyttövedoksen profiili
settings-colour-proof-profile-help = Tulostinprofiili, jota Kehityksen näyttövedos käytti viimeksi.
settings-colour-proof-intent = Näyttövedoksen toistotapa
settings-colour-proof-intent-help = Miten värit, joita tulostin ei pysty tulostamaan, sovitetaan.
settings-colour-proof-intent-perceptual = Havaintoon perustuva
settings-colour-proof-intent-relative = Suhteellinen kolorimetrinen
settings-colour-proof-bpc = Näyttövedoksen mustan pisteen kompensointi
settings-colour-proof-bpc-help = Kuvaa mustan tulostimen mustaksi.
settings-colour-proof-paper = Näyttövedoksen paperin valkoinen
settings-colour-proof-paper-help = Näytä paperin oma väri.
settings-colour-proof-ink = Näyttövedoksen musteen musta
settings-colour-proof-ink-help = Näytä, kuinka tumma musteen musta on.

## Metatiedot ja yksityisyys

settings-metadata-show-info-on-open = Näytä tietopaneeli avattaessa
settings-metadata-show-info-on-open-help = Avaa kameran, päivämäärän ja paikan tiedot sisältävä paneeli aina, kun kuva avataan.
settings-metadata-info-detail = Tietopaneelissa näytetään
settings-metadata-info-detail-help = Yhteenveto selittää hyödyllisimmät tiedot selkokielellä; kaikki kentät luettelee kaiken tiedostoon tallennetun.
settings-metadata-info-detail-summary = Yhteenveto
settings-metadata-info-detail-all = Kaikki kentät
settings-metadata-reverse-geocoding = Näytä paikannimet GPS-sijainneille
settings-metadata-reverse-geocoding-help = Hae lähin kaupunki sisäänrakennetuista tiedoista. Toimii ilman verkkoyhteyttä: sijainnit eivät koskaan lähde tietokoneeltasi.
settings-metadata-hide-gps = Piilota GPS-sijainnit näytöltä
settings-metadata-hide-gps-help = Älä näytä koordinaatteja tai paikkoja, esim. näyttöä jaettaessa. Tiedostoja ei muuteta.
# machine-drafted, needs native review
settings-metadata-develop-sidecar = Tallenna kehityksen muokkaukset kuvan viereen
# machine-drafted, needs native review
settings-metadata-develop-sidecar-help = Kirjoittaa IMG_1234.CR2.specular.xmp-tiedoston kuvan viereen, jolloin muokkaukset kulkevat sen mukana. Pois: muokkaukset jäävät vain kirjastoon.
# machine-drafted, needs native review
settings-metadata-develop-import-lightroom = Tuo Lightroomin asetukset Kehitykseen
# machine-drafted, needs native review
settings-metadata-develop-import-lightroom-help = Kuva, jolla on Lightroomin asetukset mutta ei muokkausta, avautuu niiden kanssa likimäärin. Pois: Kehitys tarjoaa painikkeen.
settings-metadata-units = Yksiköt
settings-metadata-units-help = Korkeuden, etäisyyksien ja nopeuksien yksiköt.
settings-metadata-units-auto = Alueellisten muotojen mukaan
settings-metadata-units-metric = Metrinen
settings-metadata-units-imperial = Brittiläinen
# machine-drafted, needs native review
settings-metadata-basemap = Taustakartta
settings-metadata-basemap-help = Kartta kuvien sijaintien alla. Verkkokartat lataavat ruudut katsotulta alueelta.
settings-metadata-basemap-off = Vain sisäänrakennettu kartta (offline)
settings-metadata-basemap-openstreetmap = OpenStreetMap (verkossa)
settings-metadata-basemap-custom = Muu karttaruutupalvelin (verkossa)
settings-metadata-basemap-file = Oma karttatiedosto
settings-metadata-basemap-url = Karttaruutupalvelimen osoite
settings-metadata-basemap-url-help = Osoite, jossa on {"{"}z{"}"}, {"{"}x{"}"} ja {"{"}y{"}"}, esimerkiksi https://tiles.example.org/{"{"}z{"}"}/{"{"}x{"}"}/{"{"}y{"}"}.png
settings-metadata-basemap-path = Karttatiedosto
settings-metadata-basemap-path-help = MBTiles- tai PMTiles-tiedosto. Toimii ilman verkkoyhteyttä.
settings-metadata-basemap-cache = Karttaruutujen välimuisti
settings-metadata-basemap-cache-help = Levytila ladatuille ruuduille. Vanhimmat poistetaan ensin.
# machine-drafted, needs native review
settings-metadata-basemap-allowed = Sallitut karttaruutupalvelimet
settings-metadata-basemap-allowed-help = Karttapalvelimet, joilta hyväksyit ruutujen lataamisen.

## Suorituskyky

settings-performance-ram-cache = Muistivälimuisti
settings-performance-ram-cache-help = Muisti, jossa purettuja kuvia pidetään valmiina. Automaattinen valitsee tietokoneen muistin määrän mukaan.
settings-performance-vram-cache = GPU-muistin välimuisti
settings-performance-vram-cache-help = Kuville ja ruuduille käytettävä GPU-muisti. Automaattinen valitsee GPU:n mukaan.
settings-performance-prefetch-depth = Etukäteen valmisteltavat kuvat
settings-performance-prefetch-depth-help = Kuinka monta kuvaa selaussuunnassa puretaan etukäteen.
settings-performance-decoder-threads = Purkusäikeet
settings-performance-decoder-threads-help = Kuvien purkuun käytettävät CPU-säikeet. Automaattinen käyttää kaikkia suorituskykyytimiä ja mukautuu tiedostoon.
settings-performance-gpu-adapter = GPU
settings-performance-gpu-adapter-help = Mitä GPU:ta käytetään, kun tietokoneessa on useampi.
settings-performance-gpu-adapter-high-performance = Paras suorituskyky
settings-performance-gpu-adapter-low-power = Pienin virrankulutus
settings-performance-processing = Missä kuvat käsitellään
settings-performance-processing-help = Osa työstä voidaan tehdä CPU:lla, GPU:lla tai laitteistopurkajalla. Automaattinen mittaa tämän tietokoneen ja valitsee nopeimman tavan katseltavalle kuvalle ja taloudellisimman tavan taustatyölle. Alla näkyy, mitä tällä tietokoneella valittiin ja miksi.
settings-performance-processing-automatic = Automaattinen
settings-performance-processing-prefer-gpu = Suosi GPU:ta
settings-performance-processing-cpu-only = Vain CPU
settings-performance-drag-preview = Esikatselu vedettäessä
settings-performance-drag-preview-help = Miten kehitys näyttää kuvan, kun vedät liukusäädintä. Automaattinen käyttää pienempää kuvaa vain, kun tietokone ei pysy mukana.
settings-performance-drag-preview-automatic = Automaattinen
settings-performance-drag-preview-sharp = Aina terävä
settings-performance-drag-preview-fast = Aina nopea

## Asetukset → Suorituskyky → Miten { -app-name } toimii tällä tietokoneella (polunvalitsin,
## DESIGN.md §12.3.1). $engine ja $other ovat hw-report-engine-*-nimiä (pienellä alkukirjaimella,
## lauseen keskellä); paths-engine-* ovat itsenäisiä nimiä.
settings-paths-title = Miten { -app-name } toimii tällä tietokoneella
settings-paths-intro = { -app-name } ajastaa näiden tehtävien eri suoritustavat tällä tietokoneella ja käyttää parasta. Ajat koskevat { $mpx } megapikselin valokuvaa.
settings-paths-intro-measuring = Mittaus on vielä kesken: { -app-name } mittaa, kun tietokone on joutilaana.
settings-paths-intro-battery = Akkuvirralla: valinnat suosivat pienempää energiankulutusta.
settings-paths-keywords = GPU CPU NPU näytönohjain grafiikkasuoritin laitteistokiihdytys purkaja suorituskykytesti nopeus
paths-engine-cpu = CPU
paths-engine-cpu-parallel = CPU, kaikki ytimet
paths-engine-gpu = GPU
paths-engine-hw-decoder = Laitteistopurkaja
paths-engine-npu = NPU
settings-paths-your-setting = { $engine } (oma asetuksesi)
settings-paths-measured-faster = Mitattu tällä tietokoneella: { $ratio } kertaa niin nopea kuin { $other }.
settings-paths-measured-similar = Mitattu tällä tietokoneella: suunnilleen yhtä nopea kuin { $other }.
settings-paths-measured-power = Akkuvirralla tämä kuluttaa vähiten energiaa (mitattu tällä tietokoneella).
settings-paths-not-measured = Ei vielä mitattu: tavallinen valinta tällaiselle tietokoneelle.
settings-paths-set-by = Valittu asetuksellasi ”{ $setting }: { $option }”.
settings-paths-blocked-check = Poistettu käytöstä { $date }: { $engine }. Sen tulos ei vastannut CPU:n tulosta.
settings-paths-blocked-error = Poistettu käytöstä { $date } virheen jälkeen: { $engine }.
settings-paths-blocked-lost = Poistettu käytöstä { $date }, kun se lakkasi vastaamasta: { $engine }.
settings-paths-blocked-undated = Poistettu käytöstä virheen jälkeen: { $engine }.
settings-paths-checking = Ennen käyttöä katseltavaan kuvaan tulos tarkistetaan CPU:ta vasten: { $engine }.
settings-paths-exploring = Kokeillaan nyt taustalla: { $engine }.
settings-paths-unavailable = Tässä tietokoneessa ei ole tähän käytettävissä: { $engine }.
settings-paths-background-cheaper = Taustalla: { $engine }, joka mittausten mukaan käyttää { $percent } vähemmän CPU-aikaa.
settings-paths-background-power = Taustalla: { $engine }, energian säästämiseksi.
settings-paths-background = Taustalla: { $engine }.
settings-paths-detail-measured = { $engine }: { $time } ms, CPU-aikaa { $cpu } ms, { $runs ->
    [one] { $runs } mittaus
   *[other] { $runs } mittausta
} ({ $confidence })
settings-paths-detail-guess = { $engine }: noin { $time } ms (arvio)
settings-paths-confidence-low = pieni varmuus
settings-paths-confidence-medium = kohtalainen varmuus
settings-paths-confidence-high = suuri varmuus
settings-paths-machine = Mitattu kokoonpanolle { $gpu } · { $driver } · { $cpu }
settings-paths-retry = Yritä uudelleen
settings-paths-measure-again = Mittaa uudelleen
settings-paths-measure-again-question = Unohdetaanko tällä tietokoneella mitattu, myös käytöstä poistetut polut, ja mitataan uudelleen, kun tietokone on joutilaana?
settings-paths-measure-again-confirm = Unohda mittaukset
settings-performance-display-cache = Pidä näyttövälimuistia levyllä
settings-performance-display-cache-help = Tallenna kuvista näytön kokoiset kopiot, jotta katsotut kansiot avautuvat heti.
settings-performance-display-cache-size = Näyttövälimuistin koko
settings-performance-display-cache-size-help = Vanhimmat merkinnät poistetaan, kun välimuisti on täynnä.
settings-performance-display-cache-location = Näyttövälimuistin sijainti
settings-performance-display-cache-location-help = Nopealla asemalla oleva kansio toimii parhaiten.
settings-performance-fast-simd = Käytä SIMD-käskyjä
settings-performance-fast-simd-help = Käytä CPU:n vektorikäskyjä purkuun. Poista käytöstä vain ongelman selvittämiseksi.
settings-performance-fast-dct-scaling = Pura näytön kokoon
settings-performance-fast-dct-scaling-help = Pura suuret JPEG-kuvat suoraan siihen kokoon, jossa ne näytetään. Poista käytöstä vain ongelman selvittämiseksi.
settings-performance-fast-parallel-decode = Pura usealla ytimellä
settings-performance-fast-parallel-decode-help = Jaa suuret kuvat CPU-ytimien kesken. Poista käytöstä vain ongelman selvittämiseksi.
settings-performance-fast-restart-index = Indeksoi suuret JPEG-kuvat zoomausta varten
settings-performance-fast-restart-index-help = Muista, mistä JPEG-kuvan kukin osa alkaa, jotta lähennetyt alueet purkautuvat nopeasti. Poista käytöstä vain ongelman selvittämiseksi.
settings-performance-fast-yuv-upload = Muunna värit GPU:lla
settings-performance-fast-yuv-upload-help = Muunna JPEG-värit GPU:lla tai CPU:lla. Automaattinen valitsee tällä tietokoneella nopeamman. Poista käytöstä vain ongelman selvittämiseksi.
settings-performance-fast-yuv-upload-auto = Automaattinen
settings-performance-fast-yuv-upload-on = Käytössä
settings-performance-fast-yuv-upload-off = Ei käytössä
settings-performance-fast-async-io = Asynkroninen tiedostojen luku
settings-performance-fast-async-io-help = Lue etukäteen käyttöjärjestelmän nopeimmalla tiedostorajapinnalla. Poista käytöstä vain ongelman selvittämiseksi.
settings-performance-fast-mmap = Muistiin kuvatut tiedostot
settings-performance-fast-mmap-help = Lue tiedostoja kopioimatta niitä. Poista käytöstä vain ongelman selvittämiseksi.
settings-performance-hardware-decode = Laitteistopurku
settings-performance-hardware-decode-help = Anna GPU:n sisäänrakennetun purkajan avata JPEG-valokuvia. Automaattinen käyttää sitä, missä siitä on hyötyä, yleensä taustalla valmisteltaviin kuviin. Se tarkistetaan ensin tavallista purkajaa vasten, ja kaikki, mitä se ei osaa käsitellä, avataan tavalliseen tapaan.
settings-performance-hardware-decode-auto = Automaattinen
settings-performance-hardware-decode-on = Käytössä
settings-performance-hardware-decode-off = Ei käytössä
settings-performance-frame-time-hud = Kuva-aikojen HUD
settings-performance-frame-time-hud-help = Näytä purku- ja kuva-ajat sekä käytössä olevat pikapolut.
settings-performance-stall-warning = Raportoi jumittuneet lataukset viiveellä
settings-performance-stall-warning-help = Kun kuva latautuu vielä näin pitkän ajan jälkeen, kirjoita sen lataustila ja odottavat työt lokiin (vakiovirhe tai SPECULAR_LOG_FILE).
# machine-drafted, needs native review
settings-performance-insights = Suorituskykyvihjeet
settings-performance-insights-help = Kun kuvat avautuvat toistuvasti hitaasti samasta syystä, näytä kulmassa ”Miksi tämä on hidas?” ja mikä auttaisi. Opitut tiedot pysyvät tällä tietokoneella.
settings-performance-insights-storage = Vihjeet hitaasta tallennustilasta
settings-performance-insights-storage-help = Näytä vihjeitä, kun tiedostojen lukeminen (verkkojaosta, hitaalta asemalta tai arkistoista) hidastaa toistuvasti kuvien avaamista.
settings-performance-insights-decoder = Vihjeet purkamisesta
settings-performance-insights-decoder-help = Näytä vihjeitä, kun kuvat puretaan toistuvasti CPU:lla, vaikka laitteistopurkaja voisi auttaa.
# machine-drafted, needs native review
settings-performance-insights-restore = Näytä piilotetut vihjeet uudelleen
settings-performance-insights-restore-help = Palauttaa vihjeet, jotka piilotit ×:llä kohdassa ”Miksi tämä on hidasta?”, sekä kuvien avaamisesta että pikkukuvista. Ei käytettävissä, kun mitään ei ole piilotettu.

## Helppokäyttöisyys

settings-accessibility-reduce-motion = Vähennä liikettä
settings-accessibility-reduce-motion-help = Korvaa zoomaus-, liuku- ja häivytysanimaatiot välittömillä muutoksilla.
settings-accessibility-reduce-motion-system = Järjestelmän mukaan
settings-accessibility-reduce-motion-on = Käytössä
settings-accessibility-reduce-motion-off = Ei käytössä
settings-accessibility-increase-contrast = Lisää kontrastia
settings-accessibility-increase-contrast-help = Vahvemmat reunat ja teksti sekä vähemmän läpikuultavuutta.
settings-accessibility-increase-contrast-system = Järjestelmän mukaan
settings-accessibility-increase-contrast-on = Käytössä
settings-accessibility-increase-contrast-off = Ei käytössä
settings-accessibility-text-scale = Tekstin koko
settings-accessibility-text-scale-help = Tekstin koko suhteessa vakiokokoon.
settings-accessibility-ui-scale = Käyttöliittymän skaalaus
settings-accessibility-ui-scale-help = Koko käyttöliittymän koko käyttöjärjestelmässä asetetun näytön skaalauksen lisäksi.
settings-accessibility-focus-ring-always = Näytä kohdistuskehys aina
settings-accessibility-focus-ring-always-help = Näytä, missä säätimessä näppäimistön kohdistus on, myös hiirtä käytettäessä.
settings-accessibility-screen-reader-verbosity = Ruudunlukijan yksityiskohdat
# machine-drafted, needs native review
settings-accessibility-screen-reader-verbosity-help = Kuinka paljon ruudunlukija kertoo, kun siirryt kuvien välillä, ja kuinka paljon osoittimen alla olevasta lukemasta se lukee.
settings-accessibility-screen-reader-verbosity-concise = Lyhyt
settings-accessibility-screen-reader-verbosity-normal = Normaali
settings-accessibility-screen-reader-verbosity-verbose = Yksityiskohtainen
# machine-drafted, needs native review
settings-accessibility-announce-cursor = Lue osoittimen alla oleva arvo
settings-accessibility-announce-cursor-help = Ruudunlukija lukee osoittimen tai näppäimistötähtäimen alla olevan pikseliarvon, HU:n, lämpötilan tai sijainnin, kun se lakkaa liikkumasta.

## Yksityisyys

# machine-drafted, needs native review
settings-privacy-no-network = Ei verkkoa
# machine-drafted, needs native review
settings-privacy-no-network-help = Pois: vain nämä muodostavat yhteyden, kukin vasta kun otat sen käyttöön tai avaat sillä jotain: päivitysten tarkistus, paikallisen tekoälyn mallien lataus, verkkokartat, kuvat verkko-osoitteesta tai pilvitallennustilasta sekä lisäämäsi palvelimet (DICOMweb, OMERO, kartat). Päällä: mikään ei muodosta yhteyttä.

## Lisäasetukset

settings-advanced-portable-mode = Siirrettävä tila
settings-advanced-portable-mode-help = Säilytä asetukset ja välimuistit ohjelman vieressä olevassa kansiossa käyttäjäprofiilin sijaan.
settings-advanced-log-level = Lokin tarkkuus
settings-advanced-log-level-help = Kuinka paljon { -app-name } kirjoittaa lokitiedostoonsa.
settings-advanced-log-level-error = Vain virheet
settings-advanced-log-level-warn = Varoitukset
settings-advanced-log-level-info = Tiedot
settings-advanced-log-level-debug = Virheenjäljitys
settings-advanced-log-level-trace = Kaikki
settings-advanced-developer-mode = Kehittäjätila
settings-advanced-developer-mode-help = Näytä { -app-name }in kehittämiseen ja testaamiseen tarkoitetut työkalut, kuten pseudokielet kieliluettelossa.
# machine-drafted, needs native review
settings-advanced-audit-log = Paikallinen tarkastusloki
# machine-drafted, needs native review
settings-advanced-audit-log-help = Kirjaa avatut lääketieteelliset kuvat, viennit, anonymisoinnit, tulosteet ja lähetykset PACSiin tiedostoon audit.jsonl datakansiossa. Se pysyy tällä tietokoneella.
settings-advanced-hw-report = Laitteistoraportti…
settings-advanced-hw-report-help = Näytä, mitkä CPU- ja GPU-ominaisuudet { -app-name } tunnisti ja mitä pikapolkuja se käyttää.
# machine-drafted, needs native review
settings-advanced-renderer-info = Renderöijän tiedot…
settings-advanced-renderer-info-help = Näytä näytönohjain, ajuri, näyttömuodot ja muistirajat, joita { -app-name } käyttää, valmiina kopioitavaksi vikailmoitukseen.
settings-advanced-edit-settings-file = Muokkaa settings.tomlia
settings-advanced-edit-settings-file-help = Muokkaa asetustiedostoa { -app-name }n omassa editorissa. Se tarkistaa tiedoston kirjoittaessasi ja tallentaa sen vain, kun siinä ei ole virheitä.
settings-advanced-import = Tuo asetukset…
settings-advanced-import-help = Lataa asetukset { -app-name }ista viedystä tiedostosta.
settings-advanced-export = Vie asetukset…
settings-advanced-export-help = Tallenna kaikki asetukset yhteen tiedostoon varmuuskopioksi tai toista tietokonetta varten.
settings-advanced-reset-all = Palauta kaikki asetukset…
settings-advanced-reset-all-help = Palauta jokainen asetus oletusarvoonsa. Tämän voi kumota.

## settings.toml editor (machine-drafted, needs native review)

config-editor-title = settings.toml
config-editor-save = Tallenna
config-editor-revert = Palauta
config-editor-close = Sulje
config-editor-modified = Muutettu
config-editor-text = Asetustiedosto
config-editor-position = Rivi { $line }, sarake { $column }
config-editor-saved = settings.toml tallennettiin ja otettiin käyttöön.
config-editor-cant-save =
    { $count ->
        [one] Ei voi tallentaa: 1 ongelma. Korjaa se ensin.
       *[other] Ei voi tallentaa: { $count } ongelmaa. Korjaa ne ensin.
    }
config-editor-save-failed = settings.tomlia ei voitu tallentaa: { $reason }
config-editor-conflict = settings.tomlia on muutettu muualla.
config-editor-reload = Lataa uudelleen
config-editor-keep-mine = Säilytä omani
config-editor-discard-question = Hylätäänkö settings.tomlin tallentamattomat muutokset?
config-editor-discard = Hylkää
config-editor-keep-editing = Jatka muokkaamista
config-editor-find-placeholder = Etsi tiedostosta
config-editor-find-count = { $current }/{ $total }
config-editor-find-none = Ei osumia
config-editor-find-previous = Edellinen osuma
config-editor-find-next = Seuraava osuma
config-editor-find-close = Sulje haku
config-editor-problems = Ongelmat
config-editor-problems-count =
    { $count ->
        [0] Ei ongelmia
        [one] 1 ongelma
       *[other] { $count } ongelmaa
    }
config-editor-no-problems = Ei ongelmia. Tiedoston voi tallentaa.
config-editor-problem-at = Rivi { $line }: { $message }
config-editor-severity-error = Virhe
config-editor-severity-warning = Varoitus
config-editor-severity-info = Huomautus
config-editor-problem-syntax = Virheellinen TOML: { $reason }
config-editor-problem-duplicate = { $key } on asetettu kahdesti samassa taulukossa. Poista toinen.
config-editor-problem-unknown-section = Asetusosiota [{ $key }] ei ole.
config-editor-problem-unknown-section-suggestion = Asetusosiota [{ $key }] ei ole. Tarkoititko [{ $suggestion }]?
config-editor-problem-unknown-key = Asetusta { $key } ei ole.
config-editor-problem-unknown-key-suggestion = Asetusta { $key } ei ole. Tarkoititko { $suggestion }?
config-editor-problem-invalid = Virheellinen arvo kohteelle { $key }: { $reason }
config-editor-problem-migrated = { $key } on kirjoitettu kuten { -app-name }n vanhempi versio sen kirjoitti. Se muunnetaan luettaessa.
config-editor-problem-newer-version = Tämän tiedoston kirjoitti { -app-name }n uudempi versio (muoto { $found }; tämä versio lukee { $supported }). Tuntemattomat asetukset säilytetään.
config-editor-problem-command = Komento { $index } kohdassa [[commands]]: { $reason }
config-editor-problem-hook = Koukku { $index } kohdassa [[hooks]]: { $reason }

## keymap.toml in the same editor (machine-drafted, needs native review)

config-editor-keymap-title = keymap.toml
config-editor-keymap-text = Pikanäppäintiedosto
config-editor-keymap-saved = keymap.toml tallennettiin ja otettiin käyttöön.
config-editor-keymap-save-failed = keymap.toml-tiedostoa ei voi tallentaa: { $reason }
config-editor-keymap-conflict = keymap.toml muuttui muualla.
config-editor-keymap-discard-question = Hylätäänkö keymap.toml-tiedoston tallentamattomat muutokset?
config-editor-new-file = uusi tiedosto, luodaan tallennettaessa
config-editor-no-folder = asetuskansiota, johon sen voisi tallentaa, ei ole.

## Reset all settings confirmation (machine-drafted, needs native review)

settings-reset-all-confirm = Palautetaanko kaikki asetukset oletuksiin?
settings-reset-all-confirm-detail = Tämä koskee kaikkia asetusten osioita. Sen voi perua jälkikäteen.
settings-reset-all-confirm-ok = Palauta kaikki
settings-reset-all-done = Kaikki asetukset palautettiin oletuksiin.

## Hardware video encoding (machine-drafted, needs native review)
paths-engine-hw-encoder = Laitteistokoodaaja

## Path timing units (machine-drafted, needs native review)
settings-paths-checking-before-use = Ennen käyttöä tulos tarkistetaan CPU:ta vasten: { $engine }.
settings-paths-unit-thumbnail = Ajat koskevat yhtä pikkukuvaa, koko { $size }
settings-paths-unit-frame = Ajat ovat videokehystä kohden, koko { $size }
# machine-drafted, needs native review
settings-metadata-develop-calibration = Kalibrointikuvien kansio
# machine-drafted, needs native review
settings-metadata-develop-calibration-help = Flat- ja dark-kuvat Kehitykseen. Tiedosto- tai kansionimissä on "flat" tai "dark".

## Colour swatches under a colour setting's field (machine-drafted, needs native review)
settings-colour-white = Valkoinen
settings-colour-light-grey = Vaaleanharmaa
settings-colour-grey = Harmaa
settings-colour-dark-grey = Tummanharmaa
settings-colour-black = Musta
settings-colour-yellow = Keltainen
settings-colour-orange = Oranssi
settings-colour-red = Punainen
settings-colour-magenta = Magenta
settings-colour-blue = Sininen
settings-colour-cyan = Syaani
settings-colour-green = Vihreä
