# machine-drafted, needs native review
# Specular — Maps tools: terrain (slope, aspect, contours, 3-D), time, wind and weather colours,
# online map services, photos and geotagging along GPS tracks, and map export.

## Cheat sheet groups of the Maps tab.
cheatsheet-map-terrain = Terräng
cheatsheet-map-time = Tid och väder
cheatsheet-map-online = Onlinetjänster
cheatsheet-map-photos = Foton och GPS
cheatsheet-map-output = Utdata

## Palette entries and their one-line descriptions.
palette-map-slope = Lutningsfärger
palette-map-slope-help = Färglägg en höjdmodell efter hur brant den är.
palette-map-aspect = Exponeringsfärger
palette-map-aspect-help = Färglägg en höjdmodell efter åt vilket håll sluttningarna vetter.
palette-map-contours = Visa eller dölj höjdkurvor
palette-map-contours-help = Höjdkurvor över en höjdmodell, märkta med några kurvors mellanrum.
palette-map-terrain-3d = Terräng i 3D
palette-map-terrain-3d-help = Visa höjdmodellen som en 3D-yta med färgerna draperade över.
palette-map-time = Visa eller dölj tidsreglaget
palette-map-time-help = Stega genom ett rutnäts tider eller längs ett spårs tider.
palette-map-play = Spela upp eller pausa tiden
palette-map-play-help = Låt tidsreglaget gå av sig självt, steg för steg.
palette-map-wind = Visa eller dölj vindpilar
palette-map-wind-help = Pilar för vindhastighet och vindriktning från rutnätets u- och v-komponenter.
palette-map-weather-colours = Nästa väderfärgkarta
palette-map-weather-colours-help = Färgkartor för temperatur, regn, vind och lufttryck.
palette-map-services = Onlinekarttjänster…
palette-map-services-help = Lägg till lager från WMS-, WMTS-, WFS-, OGC API- eller STAC-servrar.
palette-map-photos-on-track = Visa eller dölj foton på spåret
palette-map-photos-on-track-help = Placera mappens foton längs GPS-spåret efter när de togs.
palette-map-geotag = Geotagga foton från spåret…
palette-map-geotag-help = Skriv spårets positioner till XMP-sidofiler för mappens foton.
palette-map-export = Exportera karta…
palette-map-export-help = Spara kartan som PNG eller PDF i en skala, med teckenförklaring, skalstock och nordpil.
palette-map-print = Skriv ut karta i skala…
palette-map-print-help = Skriv ut kartan i en vald skala, till exempel 1:{ NUMBER(25000) }.
palette-map-copy-extent = Kopiera utbredning
palette-map-copy-extent-help = Kopiera området på skärmen som en omslutande rektangel, eller som WKT.

## Why a map command is unavailable.
disabled-no-time = Kartan har inga tider
disabled-no-wind = Kartan har inga vindkomponenter (u och v)
disabled-no-track = Inget GPS-spår med tider
disabled-not-grid = Kartan är inte ett rutnät av värden

## Settings › Maps and Metadata.
settings-maps-services = Onlinekarttjänster
settings-maps-services-help = Lägg till lager från WMS-, WMTS-, WFS-, OGC API- och STAC-servrar. Av: inget hämtas.
settings-maps-services-allowed = Tillåtna kartservrar
settings-maps-services-allowed-help = Servrar du har godkänt efter förklaringen av vad de ser.
settings-metadata-basemap-service = Ett lager från en karttjänst
settings-metadata-basemap-service-help = Lagret som valts i kartans panel för onlinetjänster.

## Terrain.
# A compass point, short: n, ne, e, se, s, sw, w, nw.
map-compass = { $point ->
    [ne] NO
    [e] O
    [se] SO
    [s] S
    [sw] SV
    [w] V
    [nw] NV
   *[n] N
}
map-slope-aspect = Lutning { $slope }° · vetter mot { $direction } ({ $aspect }°)
map-slope-flat = Lutning { $slope }° · plant
map-slope-on = Lutningsfärger
map-aspect-on = Exponeringsfärger
map-relief-off = Höjdfärger
map-contours-on = Höjdkurvor på
map-contours-off = Höjdkurvor av
map-legend-slope = Lutning
map-legend-aspect = Exponering: åt vilket håll sluttningarna vetter
map-legend-degrees = { $degrees }°
map-legend-degrees-more = { $degrees }° och mer
map-terrain-3d-on = Terräng i 3D. Esc går tillbaka till kartan.
map-terrain-3d-on-exaggerated = Terräng i 3D, höjder × { $times }. Esc går tillbaka till kartan.
map-terrain-exaggeration = Höjder överdrivna { $times } gånger.
map-terrain-drape = Kartfärger

## Time, weather and wind.
map-time-grid = { $when } · { $step } av { $steps }
map-time-track = { $when } · efter { $elapsed }
map-time-speed = × { $times }
map-time-play = Spela upp
map-time-pause = Pausa
map-weather-map = Färger: { $name }
map-weather-file = Färger från filen
map-wind-on = Vindpilar på
map-wind-off = Vindpilar av
map-wind-at = Vind { $speed } { $unit } från { $degrees }° ({ $direction })

## Copy extent.
map-extent-copied = Omslutande rektangel kopierad. Kopiera igen för WKT.
map-extent-copied-wkt = Utbredning kopierad som WKT

## Online map services.
map-services-title = Onlinekarttjänster
map-service-off = Onlinekarttjänster är avstängda.
map-service-off-why = Slå på dem för att lägga till lager från servrar på internet.
map-service-turn-on = Slå på
map-service-close = Stäng
map-service-back = Bakåt
map-service-cancel = Avbryt
map-service-connect = Anslut
map-service-allow = Tillåt
map-service-remove = Ta bort
map-service-download = Hämta
map-service-list-hint = Enter öppnar en server. Delete tar bort den.
map-service-add = Lägg till en server…
map-service-add-sub = WMS, WMTS, WFS, OGC API eller STAC
map-service-add-hint = Serverns adress. Enter ansluter.
map-service-bad-address = Inte en webbadress (http eller https, utan användarnamn).
map-service-server-sub = { $kind } · { $host }
map-service-consent-title = Onlinekarttjänst
map-service-consent-server = Specular kommer att kontakta { $host }.
map-service-consent-sees = Den servern ser din internetadress och kartområdet du ber om.
map-service-consent-private = Dina filer och deras namn skickas inte.
map-service-asking = Frågar { $host }…
map-service-downloading = Hämtar från { $host }…
map-service-layers-hint = { $kind }-lager. Enter använder ett.
map-service-no-layers = Servern listar inga lager.
map-service-offer-tiles = Rutor: blir fotokartans bakgrundskarta
map-service-offer-features = Objekt: läggs till på den här kartan för området på skärmen
map-service-offer-search = Bilder: sök i området på skärmen
map-service-refused-mercator = Erbjuds inte i Web Mercator (EPSG:3857), som bakgrundskartan behöver.
map-service-refused-format = Erbjuds inte som PNG- eller JPEG-bilder.
map-service-refused-geojson = Erbjuds inte som GeoJSON, formatet som läses här.
map-service-items-hint = { $count ->
        [one] { $count } bild hittades. Enter hämtar den.
       *[other] { $count } bilder hittades. Enter hämtar en.
    }
map-service-no-items = Inga bilder hittades här.
map-service-item-cloud = { $when } · { PERCENT($cloud) } moln
map-service-item-no-cog = Ingen Cloud-Optimized GeoTIFF
map-service-download-ask = Hämta { $size } från { $host }?
map-service-download-ask-unknown = Hämta den här bilden från { $host }? Storleken är okänd.
map-service-download-cache = Den sparas i kartcachen och öppnas.
map-service-size-mb = { $size } MB
map-service-size-gb = { $size } GB
map-service-too-big = Bilden är { $size }, för stor för att hämta.
map-service-opened = Öppnar den hämtade bilden
map-service-basemap = { $layer } är nu fotokartans bakgrundskarta. Fotokartan frågar innan den hämtar första gången.
map-service-status = Servern svarade { $status }.
map-service-unknown = Inte en karttjänst som den här versionen läser.
map-service-exception = Servern sa: { $message }
map-service-malformed = Serverns svar är trasigt: { $reason }
map-service-remove-ask = Ta bort ”{ $server }” från de sparade servrarna?
map-service-remove-detail = Inget på disken går förlorat. Lager som redan finns på kartan stannar tills den stängs.
map-service-removed = Tog bort { $server }
map-service-saved = Sparade kartservrar
map-service-saved-none = Inga än. Lägg till en i kartans panel för onlinetjänster.
map-service-keywords = WMS WMTS WFS OGC API STAC servrar karttjänster

## Photos and geotagging.
map-geotag-title = Geotagga foton från spåret
map-photos-on = Foton på spåret
map-photos-off = Foton på spåret dolda
map-geotag-offset = Kamerans klocka: { $offset } från GPS-tid. ← → ändrar en minut, med Skift en timme, med Alt en sekund.
map-geotag-reading = Läser fotona…
map-geotag-summary = { $count } av { $total } foton får en position.
map-geotag-sidecars = Positionerna skrivs till XMP-sidofiler bredvid fotona. Fotofilerna ändras inte.
map-geotag-writing = Skriver…
map-geotag-write = Skriv sidofiler…
map-geotag-guess = Gissa förskjutning
map-geotag-will-write = Får { $position }
map-geotag-has-position = Har redan en position: lämnas som den är
map-geotag-no-time = Ingen tagningstid: lämnas som den är
map-geotag-outside = Tagen { $offset } utanför spåret: lämnas som den är
map-geotag-gap = I ett glapp i spåret ({ $seconds } s från en fix): lämnas som den är
map-geotag-nothing = Inget foto får en position med den här förskjutningen.
map-geotag-confirm = Skriva positioner till { $count ->
    [one] { $count } XMP-sidofil
   *[other] { $count } XMP-sidofiler
}? Själva fotona ändras inte.
map-geotag-confirm-ok = Skriv sidofiler
map-geotag-done = { $count ->
    [one] Skrev { $count } sidofil
   *[other] Skrev { $count } sidofiler
}
map-geotag-done-failed = Skrev { $count ->
        [one] { $count } sidofil
       *[other] { $count } sidofiler
    }; { $failed } misslyckades: { $reason }
map-geotag-not-sidecar = Inte ett namn på en sidofil.

## Export and printing.
map-export-title = Exportera karta
map-print-title = Skriv ut karta i skala
map-export-keys = ↑ ↓ väljer, ← → ändrar, Enter kör.
map-export-format = Format
map-export-scale = Skala
map-export-scale-value = 1:{ $n }
map-export-resolution = Upplösning
map-export-dpi = { $dpi } dpi
map-export-paper = Papper
map-export-size = { $width } × { $height } px · { $across } × { $down } km i terrängen
map-export-save = Spara…
map-export-print = Skriv ut…
map-export-too-large = För stor i den här skalan: { $width } × { $height } px. Välj en mindre skala eller upplösning.
map-export-no-scale = Den här kartan har ingen skala i terrängen.
map-export-no-gpu = Export kräver grafikprocessorn.
map-export-subtitle = Skala { $scale } · { $crs }
map-export-data = Data: { $sources }
map-export-saved = Sparade { $name }
map-export-printing = Skriver ut…
map-export-printed = Skickat till skrivaren
map-export-print-failed = Inte utskriven: { $reason }
