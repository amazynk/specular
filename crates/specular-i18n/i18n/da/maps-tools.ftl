# machine-drafted, needs native review
# Specular — Maps tools: terrain (slope, aspect, contours, 3-D), time, wind and weather colours,
# online map services, photos and geotagging along GPS tracks, and map export.

## Cheat sheet groups of the Maps tab.
cheatsheet-map-terrain = Terræn
cheatsheet-map-time = Tid og vejr
cheatsheet-map-online = Onlinetjenester
cheatsheet-map-photos = Fotos og GPS
cheatsheet-map-output = Output

## Palette entries and their one-line descriptions.
palette-map-slope = Hældningsfarver
palette-map-slope-help = Farv en højdemodel efter, hvor stejl den er.
palette-map-aspect = Eksponeringsfarver
palette-map-aspect-help = Farv en højdemodel efter den retning, skråningerne vender.
palette-map-contours = Vis eller skjul højdekurver
palette-map-contours-help = Højdekurver over en højdemodel, med etiket for hver par kurver.
palette-map-terrain-3d = Terræn i 3D
palette-map-terrain-3d-help = Vis højdemodellen som en 3D-flade med farverne lagt hen over.
palette-map-time = Vis eller skjul tidsskyderen
palette-map-time-help = Gå gennem et gitters tider eller langs et spors tider.
palette-map-play = Afspil eller sæt tiden på pause
palette-map-play-help = Lad tidsskyderen køre af sig selv, trin for trin.
palette-map-wind = Vis eller skjul vindpile
palette-map-wind-help = Pile for vindhastighed og vindretning fra gitterets u- og v-komponenter.
palette-map-weather-colours = Næste vejrfarvekort
palette-map-weather-colours-help = Farvekort til temperatur, regn, vind og lufttryk.
palette-map-services = Onlinekorttjenester…
palette-map-services-help = Tilføj lag fra WMS-, WMTS-, WFS-, OGC API- eller STAC-servere.
palette-map-photos-on-track = Vis eller skjul fotos på sporet
palette-map-photos-on-track-help = Placér mappens fotos langs GPS-sporet efter, hvornår de blev taget.
palette-map-geotag = Geotag fotos fra sporet…
palette-map-geotag-help = Skriv sporets positioner til XMP-sidevognsfiler for mappens fotos.
palette-map-export = Eksportér kort…
palette-map-export-help = Gem kortet som PNG eller PDF i et målestoksforhold, med signaturforklaring, målestok og nordpil.
palette-map-print = Udskriv kort i målestoksforhold…
palette-map-print-help = Udskriv kortet i et valgt målestoksforhold, for eksempel 1:{ NUMBER(25000) }.
palette-map-copy-extent = Kopiér udstrækning
palette-map-copy-extent-help = Kopiér området på skærmen som et afgrænsende rektangel eller som WKT.

## Why a map command is unavailable.
disabled-no-time = Kortet har ingen tider
disabled-no-wind = Kortet har ingen vindkomponenter (u og v)
disabled-no-track = Intet GPS-spor med tider
disabled-not-grid = Kortet er ikke et gitter af værdier

## Settings › Maps and Metadata.
settings-maps-services = Onlinekorttjenester
settings-maps-services-help = Tilføj lag fra WMS-, WMTS-, WFS-, OGC API- og STAC-servere. Fra: intet hentes.
settings-maps-services-allowed = Tilladte kortservere
settings-maps-services-allowed-help = Servere, du har accepteret efter forklaringen af, hvad de ser.
settings-metadata-basemap-service = Et lag fra en korttjeneste
settings-metadata-basemap-service-help = Det lag, der er valgt i kortets panel for onlinetjenester.

## Terrain.
# A compass point, short: n, ne, e, se, s, sw, w, nw.
map-compass = { $point ->
    [ne] NØ
    [e] Ø
    [se] SØ
    [s] S
    [sw] SV
    [w] V
    [nw] NV
   *[n] N
}
map-slope-aspect = Hældning { $slope }° · vender mod { $direction } ({ $aspect }°)
map-slope-flat = Hældning { $slope }° · fladt
map-slope-on = Hældningsfarver
map-aspect-on = Eksponeringsfarver
map-relief-off = Højdefarver
map-contours-on = Højdekurver til
map-contours-off = Højdekurver fra
map-legend-slope = Hældning
map-legend-aspect = Eksponering: den vej skråningerne vender
map-legend-degrees = { $degrees }°
map-legend-degrees-more = { $degrees }° og mere
map-terrain-3d-on = Terræn i 3D. Esc går tilbage til kortet.
map-terrain-3d-on-exaggerated = Terræn i 3D, højder × { $times }. Esc går tilbage til kortet.
map-terrain-exaggeration = Højder overdrevet { $times } gange.
map-terrain-drape = Kortfarver

## Time, weather and wind.
map-time-grid = { $when } · { $step } af { $steps }
map-time-track = { $when } · efter { $elapsed }
map-time-speed = × { $times }
map-time-play = Afspil
map-time-pause = Pause
map-weather-map = Farver: { $name }
map-weather-file = Farver fra filen
map-wind-on = Vindpile til
map-wind-off = Vindpile fra
map-wind-at = Vind { $speed } { $unit } fra { $degrees }° ({ $direction })

## Copy extent.
map-extent-copied = Afgrænsende rektangel kopieret. Kopiér igen for WKT.
map-extent-copied-wkt = Udstrækning kopieret som WKT

## Online map services.
map-services-title = Onlinekorttjenester
map-service-off = Onlinekorttjenester er slået fra.
map-service-off-why = Slå dem til for at tilføje lag fra servere på internettet.
map-service-turn-on = Slå til
map-service-close = Luk
map-service-back = Tilbage
map-service-cancel = Annuller
map-service-connect = Forbind
map-service-allow = Tillad
map-service-remove = Fjern
map-service-download = Hent
map-service-list-hint = Enter åbner en server. Delete fjerner den.
map-service-add = Tilføj en server…
map-service-add-sub = WMS, WMTS, WFS, OGC API eller STAC
map-service-add-hint = Serverens adresse. Enter forbinder.
map-service-bad-address = Ikke en webadresse (http eller https, uden brugernavn).
map-service-server-sub = { $kind } · { $host }
map-service-consent-title = Onlinekorttjeneste
map-service-consent-server = Specular vil kontakte { $host }.
map-service-consent-sees = Den server ser din internetadresse og det kortområde, du beder om.
map-service-consent-private = Dine filer og deres navne sendes ikke.
map-service-asking = Spørger { $host }…
map-service-downloading = Henter fra { $host }…
map-service-layers-hint = { $kind }-lag. Enter bruger et.
map-service-no-layers = Serveren viser ingen lag.
map-service-offer-tiles = Fliser: bliver fotokortets baggrundskort
map-service-offer-features = Objekter: føjes til dette kort for området på skærmen
map-service-offer-search = Billeder: søg i området på skærmen
map-service-refused-mercator = Tilbydes ikke i Web Mercator (EPSG:3857), som baggrundskortet kræver.
map-service-refused-format = Tilbydes ikke som PNG- eller JPEG-billeder.
map-service-refused-geojson = Tilbydes ikke som GeoJSON, det format der læses her.
map-service-items-hint = { $count ->
        [one] { $count } billede fundet. Enter henter det.
       *[other] { $count } billeder fundet. Enter henter et.
    }
map-service-no-items = Ingen billeder fundet her.
map-service-item-cloud = { $when } · { PERCENT($cloud) } skyer
map-service-item-no-cog = Ingen Cloud-Optimized GeoTIFF
map-service-download-ask = Hent { $size } fra { $host }?
map-service-download-ask-unknown = Hent dette billede fra { $host }? Størrelsen er ukendt.
map-service-download-cache = Det gemmes i kortcachen og åbnes.
map-service-size-mb = { $size } MB
map-service-size-gb = { $size } GB
map-service-too-big = Billedet er { $size }, for stort til at hente.
map-service-opened = Åbner det hentede billede
map-service-basemap = { $layer } er nu fotokortets baggrundskort. Fotokortet spørger før første download.
map-service-status = Serveren svarede { $status }.
map-service-unknown = Ikke en korttjeneste, som denne version kan læse.
map-service-exception = Serveren sagde: { $message }
map-service-malformed = Serverens svar er ødelagt: { $reason }
map-service-remove-ask = Fjern “{ $server }” fra de gemte servere?
map-service-remove-detail = Intet på disken går tabt. Lag, der allerede er på kortet, bliver, til det lukkes.
map-service-removed = Fjernede { $server }
map-service-saved = Gemte kortservere
map-service-saved-none = Ingen endnu. Tilføj en i kortets panel for onlinetjenester.
map-service-keywords = WMS WMTS WFS OGC API STAC servere korttjenester

## Photos and geotagging.
map-geotag-title = Geotag fotos fra sporet
map-photos-on = Fotos på sporet
map-photos-off = Fotos på sporet skjult
map-geotag-offset = Kameraets ur: { $offset } fra GPS-tid. ← → ændrer et minut, med Shift en time, med Alt et sekund.
map-geotag-reading = Læser fotos…
map-geotag-summary = { $count } af { $total } fotos får en position.
map-geotag-sidecars = Positionerne skrives i XMP-sidevognsfiler ved siden af fotoene. Fotofilerne ændres ikke.
map-geotag-writing = Skriver…
map-geotag-write = Skriv sidevognsfiler…
map-geotag-guess = Gæt forskydning
map-geotag-will-write = Får { $position }
map-geotag-has-position = Har allerede en position: forbliver som den er
map-geotag-no-time = Intet optagelsestidspunkt: forbliver som den er
map-geotag-outside = Taget { $offset } uden for sporet: forbliver som den er
map-geotag-gap = I et hul i sporet ({ $seconds } s fra et fix): forbliver som den er
map-geotag-nothing = Intet foto får en position med denne forskydning.
map-geotag-confirm = Skriv positioner til { $count ->
    [one] { $count } XMP-sidevognsfil
   *[other] { $count } XMP-sidevognsfiler
}? Selve fotoene ændres ikke.
map-geotag-confirm-ok = Skriv sidevognsfiler
map-geotag-done = { $count ->
    [one] Skrev { $count } sidevognsfil
   *[other] Skrev { $count } sidevognsfiler
}
map-geotag-done-failed = Skrev { $count ->
        [one] { $count } sidevognsfil
       *[other] { $count } sidevognsfiler
    }; { $failed } mislykkedes: { $reason }
map-geotag-not-sidecar = Ikke et navn på en sidevognsfil.

## Export and printing.
map-export-title = Eksportér kort
map-print-title = Udskriv kort i målestoksforhold
map-export-keys = ↑ ↓ vælger, ← → ændrer, Enter kører.
map-export-format = Format
map-export-scale = Målestoksforhold
map-export-scale-value = 1:{ $n }
map-export-resolution = Opløsning
map-export-dpi = { $dpi } dpi
map-export-paper = Papir
map-export-size = { $width } × { $height } px · { $across } × { $down } km i terrænet
map-export-save = Gem…
map-export-print = Udskriv…
map-export-too-large = For stort i dette målestoksforhold: { $width } × { $height } px. Vælg et mindre målestoksforhold eller en lavere opløsning.
map-export-no-scale = Dette kort har intet målestoksforhold i terrænet.
map-export-no-gpu = Eksport kræver grafikprocessoren.
map-export-subtitle = Målestoksforhold { $scale } · { $crs }
map-export-data = Data: { $sources }
map-export-saved = Gemte { $name }
map-export-printing = Udskriver…
map-export-printed = Sendt til printeren
map-export-print-failed = Ikke udskrevet: { $reason }
