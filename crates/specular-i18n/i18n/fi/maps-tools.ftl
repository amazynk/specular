# machine-drafted, needs native review
# Specular — Maps tools: terrain (slope, aspect, contours, 3-D), time, wind and weather colours,
# online map services, photos and geotagging along GPS tracks, and map export.
# Suomi.

## Cheat sheet groups of the Maps tab.
cheatsheet-map-terrain = Maasto
cheatsheet-map-time = Aika ja sää
cheatsheet-map-online = Verkkopalvelut
cheatsheet-map-photos = Kuvat ja GPS
cheatsheet-map-output = Tuloste

## Palette entries and their one-line descriptions.
palette-map-slope = Kaltevuusvärit
palette-map-slope-help = Väritä korkeusmalli sen jyrkkyyden mukaan.
palette-map-aspect = Rinteen suunnan värit
palette-map-aspect-help = Väritä korkeusmalli sen mukaan, mihin suuntaan rinteet viettävät.
palette-map-contours = Näytä tai piilota korkeuskäyrät
palette-map-contours-help = Korkeuskäyrät korkeusmallin päällä, nimiöt muutaman käyrän välein.
palette-map-terrain-3d = Maasto 3D:nä
palette-map-terrain-3d-help = Näytä korkeusmalli 3D-pintana, jonka päälle sen värit levitetään.
palette-map-time = Näytä tai piilota aikaliukusäädin
palette-map-time-help = Siirry ruudukon aikojen tai reitin aikojen läpi.
palette-map-play = Toista tai keskeytä aika
palette-map-play-help = Aja aikaliukusäädintä itsestään askel kerrallaan.
palette-map-wind = Näytä tai piilota tuulinuolet
palette-map-wind-help = Tuulen nopeuden ja suunnan nuolet ruudukon u- ja v-komponenteista.
palette-map-weather-colours = Seuraava säävärikartta
palette-map-weather-colours-help = Värikartat lämpötilalle, sateelle, tuulelle ja ilmanpaineelle.
palette-map-services = Verkon karttapalvelut…
palette-map-services-help = Lisää tasoja WMS-, WMTS-, WFS-, OGC API- tai STAC-palvelimilta.
palette-map-photos-on-track = Näytä tai piilota kuvat reitillä
palette-map-photos-on-track-help = Sijoita kansion kuvat GPS-reitin varrelle niiden ottoajan mukaan.
palette-map-geotag = Paikkatiedot kuviin reitiltä…
palette-map-geotag-help = Kirjoita reitin sijainnit kansion kuvien XMP-sivutiedostoihin.
palette-map-export = Vie kartta…
palette-map-export-help = Tallenna kartta PNG- tai PDF-muodossa mittakaavassa, selitteen, mittakaavapalkin ja pohjoisnuolen kanssa.
palette-map-print = Tulosta kartta mittakaavassa…
palette-map-print-help = Tulosta kartta valitussa mittakaavassa, kuten 1:{ NUMBER(25000) }.
palette-map-copy-extent = Kopioi rajaus
palette-map-copy-extent-help = Kopioi näytöllä oleva alue rajaavana suorakulmiona tai WKT-muodossa.

## Why a map command is unavailable.
disabled-no-time = Kartalla ei ole aikoja
disabled-no-wind = Kartalla ei ole tuulikomponentteja (u ja v)
disabled-no-track = Ei GPS-reittiä aikoineen
disabled-not-grid = Kartta ei ole arvojen ruudukko

## Settings › Maps and Metadata.
settings-maps-services = Verkon karttapalvelut
settings-maps-services-help = Lisää tasoja WMS-, WMTS-, WFS-, OGC API- ja STAC-palvelimilta. Pois: mitään ei haeta.
settings-maps-services-allowed = Sallitut karttapalvelimet
settings-maps-services-allowed-help = Palvelimet, jotka hyväksyit luettuasi, mitä ne näkevät.
settings-metadata-basemap-service = Karttapalvelun taso
settings-metadata-basemap-service-help = Kartan verkkopalvelupaneelissa valittu taso.

## Terrain.
# A compass point, short: n, ne, e, se, s, sw, w, nw.
map-compass = { $point ->
    [ne] KO
    [e] I
    [se] KA
    [s] E
    [sw] LO
    [w] L
    [nw] LU
   *[n] P
}
map-slope-aspect = Kaltevuus { $slope }° · suunta { $direction } ({ $aspect }°)
map-slope-flat = Kaltevuus { $slope }° · tasainen
map-slope-on = Kaltevuusvärit
map-aspect-on = Rinteen suunnan värit
map-relief-off = Korkeusvärit
map-contours-on = Korkeuskäyrät päällä
map-contours-off = Korkeuskäyrät pois
map-legend-slope = Kaltevuus
map-legend-aspect = Suunta: mihin rinteet viettävät
map-legend-degrees = { $degrees }°
map-legend-degrees-more = { $degrees }° tai enemmän
map-terrain-3d-on = Maasto 3D:nä. Esc palaa karttaan.
map-terrain-3d-on-exaggerated = Maasto 3D:nä, korkeudet × { $times }. Esc palaa karttaan.
map-terrain-exaggeration = Korkeudet liioiteltu { $times }-kertaisiksi.
map-terrain-drape = Kartan värit

## Time, weather and wind.
map-time-grid = { $when } · { $step }/{ $steps }
map-time-track = { $when } · { $elapsed } alusta
map-time-speed = × { $times }
map-time-play = Toista
map-time-pause = Keskeytä
map-weather-map = Värit: { $name }
map-weather-file = Värit tiedostosta
map-wind-on = Tuulinuolet päällä
map-wind-off = Tuulinuolet pois
map-wind-at = Tuuli { $speed } { $unit } suunnasta { $degrees }° ({ $direction })

## Copy extent.
map-extent-copied = Rajaava suorakulmio kopioitu. Kopioi uudelleen saadaksesi WKT:n.
map-extent-copied-wkt = Rajaus kopioitu WKT-muodossa

## Online map services.
map-services-title = Verkon karttapalvelut
map-service-off = Verkon karttapalvelut ovat pois päältä.
map-service-off-why = Ota ne käyttöön lisätäksesi tasoja internetin palvelimilta.
map-service-turn-on = Ota käyttöön
map-service-close = Sulje
map-service-back = Takaisin
map-service-cancel = Peruuta
map-service-connect = Yhdistä
map-service-allow = Salli
map-service-remove = Poista
map-service-download = Lataa
map-service-list-hint = Enter avaa palvelimen. Delete poistaa sen.
map-service-add = Lisää palvelin…
map-service-add-sub = WMS, WMTS, WFS, OGC API tai STAC
map-service-add-hint = Palvelimen osoite. Enter yhdistää.
map-service-bad-address = Ei verkko-osoite (http tai https, ei käyttäjänimeä).
map-service-server-sub = { $kind } · { $host }
map-service-consent-title = Verkon karttapalvelu
map-service-consent-server = Specular ottaa yhteyden palvelimeen { $host }.
map-service-consent-sees = Palvelin näkee internetosoitteesi ja pyytämäsi kartta-alueen.
map-service-consent-private = Tiedostojasi ja niiden nimiä ei lähetetä.
map-service-asking = Kysytään palvelimelta { $host }…
map-service-downloading = Ladataan palvelimelta { $host }…
map-service-layers-hint = { $kind }-tasot. Enter käyttää yhtä.
map-service-no-layers = Palvelin ei luettele tasoja.
map-service-offer-tiles = Ruudut: tulee kuvakartan taustakartaksi
map-service-offer-features = Kohteet: lisätään tähän karttaan näytöllä olevalta alueelta
map-service-offer-search = Kuvat: hae näytöllä olevalta alueelta
map-service-refused-mercator = Ei tarjolla Web Mercator -projektiossa (EPSG:3857), jota taustakartta tarvitsee.
map-service-refused-format = Ei tarjolla PNG- tai JPEG-kuvina.
map-service-refused-geojson = Ei tarjolla GeoJSON-muodossa, jota tässä luetaan.
map-service-items-hint = { $count ->
        [one] { $count } kuva löytyi. Enter lataa sen.
       *[other] { $count } kuvaa löytyi. Enter lataa yhden.
    }
map-service-no-items = Täältä ei löytynyt kuvia.
map-service-item-cloud = { $when } · pilviä { PERCENT($cloud) }
map-service-item-no-cog = Ei Cloud-Optimized GeoTIFF -tiedostoa
map-service-download-ask = Ladataanko { $size } palvelimelta { $host }?
map-service-download-ask-unknown = Ladataanko tämä kuva palvelimelta { $host }? Sen kokoa ei tiedetä.
map-service-download-cache = Se tallennetaan karttavälimuistiin ja avataan.
map-service-size-mb = { $size } Mt
map-service-size-gb = { $size } Gt
map-service-too-big = Kuva on { $size }, liian suuri ladattavaksi.
map-service-opened = Avataan ladattu kuva
map-service-basemap = { $layer } on nyt kuvakartan taustakartta. Kuvakartta kysyy ennen ensimmäistä latausta.
map-service-status = Palvelin vastasi { $status }.
map-service-unknown = Ei karttapalvelu, jota tämä versio lukee.
map-service-exception = Palvelin ilmoitti: { $message }
map-service-malformed = Palvelimen vastaus on rikki: { $reason }
map-service-remove-ask = Poistetaanko ”{ $server }” tallennetuista palvelimista?
map-service-remove-detail = Levyltä ei katoa mitään. Kartalla jo olevat tasot pysyvät, kunnes kartta suljetaan.
map-service-removed = { $server } poistettu
map-service-saved = Tallennetut karttapalvelimet
map-service-saved-none = Ei vielä yhtään. Lisää palvelin kartan verkkopalvelupaneelissa.
map-service-keywords = WMS WMTS WFS OGC API STAC palvelimet karttapalvelu

## Photos and geotagging.
map-geotag-title = Paikkatiedot kuviin reitiltä
map-photos-on = Kuvat reitillä
map-photos-off = Kuvat reitillä piilotettu
map-geotag-offset = Kameran kello: { $offset } GPS-ajasta. ← → muuttaa minuutin, Shift tunnin, Alt sekunnin.
map-geotag-reading = Luetaan kuvia…
map-geotag-summary = { $count }/{ $total } kuvaa saa sijainnin.
map-geotag-sidecars = Sijainnit kirjoitetaan kuvien viereen XMP-sivutiedostoihin. Kuvatiedostoja ei muuteta.
map-geotag-writing = Kirjoitetaan…
map-geotag-write = Kirjoita sivutiedostot…
map-geotag-guess = Arvaa ero
map-geotag-will-write = Saa sijainnin { $position }
map-geotag-has-position = Sijainti on jo: jätetään ennalleen
map-geotag-no-time = Ei ottoaikaa: jätetään ennalleen
map-geotag-outside = Otettu { $offset } reitin ulkopuolella: jätetään ennalleen
map-geotag-gap = Reitin aukossa ({ $seconds } s paikannuksesta): jätetään ennalleen
map-geotag-nothing = Mikään kuva ei saa sijaintia tällä erolla.
map-geotag-confirm = Kirjoitetaanko sijainnit { $count ->
    [one] { $count } XMP-sivutiedostoon
   *[other] { $count } XMP-sivutiedostoon
}? Itse kuvia ei muuteta.
map-geotag-confirm-ok = Kirjoita sivutiedostot
map-geotag-done = { $count ->
    [one] { $count } sivutiedosto kirjoitettu
   *[other] { $count } sivutiedostoa kirjoitettu
}
map-geotag-done-failed = { $count ->
        [one] { $count } sivutiedosto kirjoitettu
       *[other] { $count } sivutiedostoa kirjoitettu
    }; { $failed } epäonnistui: { $reason }
map-geotag-not-sidecar = Ei sivutiedoston nimi.

## Export and printing.
map-export-title = Vie kartta
map-print-title = Tulosta kartta mittakaavassa
map-export-keys = ↑ ↓ valitsee, ← → muuttaa, Enter suorittaa.
map-export-format = Muoto
map-export-scale = Mittakaava
map-export-scale-value = 1:{ $n }
map-export-resolution = Tarkkuus
map-export-dpi = { $dpi } dpi
map-export-paper = Paperi
map-export-size = { $width } × { $height } px · { $across } × { $down } km maastossa
map-export-save = Tallenna…
map-export-print = Tulosta…
map-export-too-large = Liian suuri tässä mittakaavassa: { $width } × { $height } px. Valitse pienempi mittakaava tai tarkkuus.
map-export-no-scale = Tällä kartalla ei ole mittakaavaa maastossa.
map-export-no-gpu = Vienti vaatii näytönohjaimen.
map-export-subtitle = Mittakaava { $scale } · { $crs }
map-export-data = Aineisto: { $sources }
map-export-saved = { $name } tallennettu
map-export-printing = Tulostetaan…
map-export-printed = Lähetetty tulostimelle
map-export-print-failed = Ei tulostettu: { $reason }
