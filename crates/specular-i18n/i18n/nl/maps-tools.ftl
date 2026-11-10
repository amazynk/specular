# machine-drafted, needs native review
# Specular — Maps tools: terrain (slope, aspect, contours, 3-D), time, wind and weather colours,
# online map services, photos and geotagging along GPS tracks, and map export.

## Cheat sheet groups of the Maps tab.
cheatsheet-map-terrain = Terrein
cheatsheet-map-time = Tijd en weer
cheatsheet-map-online = Onlinediensten
cheatsheet-map-photos = Foto’s en gps
cheatsheet-map-output = Uitvoer

## Palette entries and their one-line descriptions.
palette-map-slope = Hellingkleuren
palette-map-slope-help = Een hoogtemodel kleuren naar hoe steil het is.
palette-map-aspect = Expositiekleuren
palette-map-aspect-help = Een hoogtemodel kleuren naar de richting waarin de hellingen liggen.
palette-map-contours = Hoogtelijnen tonen of verbergen
palette-map-contours-help = Hoogtelijnen over een hoogtemodel, om de paar lijnen gelabeld.
palette-map-terrain-3d = Terrein in 3D
palette-map-terrain-3d-help = Het hoogtemodel tonen als 3D-oppervlak met de kleuren eroverheen.
palette-map-time = Tijdschuif tonen of verbergen
palette-map-time-help = Door de tijden van een raster of langs de tijden van een track stappen.
palette-map-play = Tijd afspelen of pauzeren
palette-map-play-help = De tijdschuif vanzelf laten lopen, stap voor stap.
palette-map-wind = Windpijlen tonen of verbergen
palette-map-wind-help = Pijlen voor windsnelheid en -richting uit de u- en v-componenten van het raster.
palette-map-weather-colours = Volgende weerkleurenkaart
palette-map-weather-colours-help = Kleurenkaarten voor temperatuur, regen, wind en luchtdruk.
palette-map-services = Online kaartdiensten…
palette-map-services-help = Lagen toevoegen van WMS-, WMTS-, WFS-, OGC API- of STAC-servers.
palette-map-photos-on-track = Foto’s op de track tonen of verbergen
palette-map-photos-on-track-help = De foto’s van de map langs de gps-track plaatsen op het moment van opname.
palette-map-geotag = Foto’s geotaggen met de track…
palette-map-geotag-help = De posities van de track in XMP-sidecars van de foto’s in de map schrijven.
palette-map-export = Kaart exporteren…
palette-map-export-help = De kaart als PNG of PDF op schaal opslaan, met legenda, schaalbalk en noordpijl.
palette-map-print = Kaart op schaal afdrukken…
palette-map-print-help = De kaart afdrukken op een gekozen schaal, zoals 1:{ NUMBER(25000) }.
palette-map-copy-extent = Extent kopiëren
palette-map-copy-extent-help = Het gebied in beeld kopiëren als omsluitend kader, of als WKT.

## Why a map command is unavailable.
disabled-no-time = De kaart heeft geen tijden
disabled-no-wind = De kaart heeft geen windcomponenten (u en v)
disabled-no-track = Geen gps-track met tijden
disabled-not-grid = De kaart is geen raster van waarden

## Settings › Maps and Metadata.
settings-maps-services = Online kaartdiensten
settings-maps-services-help = Lagen toevoegen van WMS-, WMTS-, WFS-, OGC API- en STAC-servers. Uit: er wordt niets opgehaald.
settings-maps-services-allowed = Toegestane kaartservers
settings-maps-services-allowed-help = Servers waarmee je hebt ingestemd na de uitleg over wat ze zien.
settings-metadata-basemap-service = Een laag van een kaartdienst
settings-metadata-basemap-service-help = De laag die gekozen is in het paneel met onlinediensten van de kaart.

## Terrain.
# A compass point, short: n, ne, e, se, s, sw, w, nw.
map-compass = { $point ->
    [ne] NO
    [e] O
    [se] ZO
    [s] Z
    [sw] ZW
    [w] W
    [nw] NW
   *[n] N
}
map-slope-aspect = Helling { $slope }° · ligt op { $direction } ({ $aspect }°)
map-slope-flat = Helling { $slope }° · vlak
map-slope-on = Hellingkleuren
map-aspect-on = Expositiekleuren
map-relief-off = Hoogtekleuren
map-contours-on = Hoogtelijnen aan
map-contours-off = Hoogtelijnen uit
map-legend-slope = Helling
map-legend-aspect = Expositie: de richting van de hellingen
map-legend-degrees = { $degrees }°
map-legend-degrees-more = { $degrees }° en meer
map-terrain-3d-on = Terrein in 3D. Esc gaat terug naar de kaart.
map-terrain-3d-on-exaggerated = Terrein in 3D, hoogtes × { $times }. Esc gaat terug naar de kaart.
map-terrain-exaggeration = Hoogtes { $times } keer overdreven.
map-terrain-drape = Kaartkleuren

## Time, weather and wind.
map-time-grid = { $when } · { $step } van { $steps }
map-time-track = { $when } · na { $elapsed }
map-time-speed = × { $times }
map-time-play = Afspelen
map-time-pause = Pauzeren
map-weather-map = Kleuren: { $name }
map-weather-file = Kleuren uit het bestand
map-wind-on = Windpijlen aan
map-wind-off = Windpijlen uit
map-wind-at = Wind { $speed } { $unit } uit { $degrees }° ({ $direction })

## Copy extent.
map-extent-copied = Omsluitend kader gekopieerd. Kopieer opnieuw voor WKT.
map-extent-copied-wkt = Extent gekopieerd als WKT

## Online map services.
map-services-title = Online kaartdiensten
map-service-off = Online kaartdiensten staan uit.
map-service-off-why = Zet ze aan om lagen van servers op internet toe te voegen.
map-service-turn-on = Aanzetten
map-service-close = Sluiten
map-service-back = Terug
map-service-cancel = Annuleren
map-service-connect = Verbinden
map-service-allow = Toestaan
map-service-remove = Verwijderen
map-service-download = Downloaden
map-service-list-hint = Enter opent een server. Delete verwijdert hem.
map-service-add = Server toevoegen…
map-service-add-sub = WMS, WMTS, WFS, OGC API of STAC
map-service-add-hint = Het adres van de server. Enter maakt verbinding.
map-service-bad-address = Geen webadres (http of https, zonder gebruikersnaam).
map-service-server-sub = { $kind } · { $host }
map-service-consent-title = Online kaartdienst
map-service-consent-server = Specular gaat contact maken met { $host }.
map-service-consent-sees = Die server ziet je internetadres en het kaartgebied dat je opvraagt.
map-service-consent-private = Je bestanden en hun namen worden niet verstuurd.
map-service-asking = { $host } wordt bevraagd…
map-service-downloading = Downloaden van { $host }…
map-service-layers-hint = { $kind }-lagen. Enter gebruikt er een.
map-service-no-layers = De server toont geen lagen.
map-service-offer-tiles = Tegels: wordt de achtergrondkaart van de fotokaart
map-service-offer-features = Objecten: toegevoegd aan deze kaart voor het gebied in beeld
map-service-offer-search = Beelden: zoeken in het gebied in beeld
map-service-refused-mercator = Niet aangeboden in Web Mercator (EPSG:3857), dat de achtergrondkaart nodig heeft.
map-service-refused-format = Niet aangeboden als PNG- of JPEG-afbeeldingen.
map-service-refused-geojson = Niet aangeboden als GeoJSON, het formaat dat hier gelezen wordt.
map-service-items-hint = { $count ->
        [one] { $count } beeld gevonden. Enter downloadt het.
       *[other] { $count } beelden gevonden. Enter downloadt er een.
    }
map-service-no-items = Hier geen beelden gevonden.
map-service-item-cloud = { $when } · { PERCENT($cloud) } bewolking
map-service-item-no-cog = Geen Cloud-Optimized GeoTIFF
map-service-download-ask = { $size } downloaden van { $host }?
map-service-download-ask-unknown = Dit beeld downloaden van { $host }? De grootte is niet bekend.
map-service-download-cache = Het wordt in de kaartcache bewaard en geopend.
map-service-size-mb = { $size } MB
map-service-size-gb = { $size } GB
map-service-too-big = Het beeld is { $size }, te groot om te downloaden.
map-service-opened = Gedownload beeld wordt geopend
map-service-basemap = { $layer } is nu de achtergrondkaart van de fotokaart. De fotokaart vraagt het eerst voor de eerste download.
map-service-status = De server antwoordde { $status }.
map-service-unknown = Geen kaartdienst die deze versie leest.
map-service-exception = De server zei: { $message }
map-service-malformed = Het antwoord van de server is beschadigd: { $reason }
map-service-remove-ask = ‘{ $server }’ uit de opgeslagen servers verwijderen?
map-service-remove-detail = Er gaat niets verloren op de schijf. Lagen die al op de kaart staan, blijven tot die gesloten wordt.
map-service-removed = { $server } verwijderd
map-service-saved = Opgeslagen kaartservers
map-service-saved-none = Nog geen. Voeg er een toe in het paneel met onlinediensten van de kaart.
map-service-keywords = WMS WMTS WFS OGC API STAC servers kaartdiensten

## Photos and geotagging.
map-geotag-title = Foto’s geotaggen met de track
map-photos-on = Foto’s op de track
map-photos-off = Foto’s op de track verborgen
map-geotag-offset = Cameraklok: { $offset } ten opzichte van gps-tijd. ← → wijzigen met een minuut, met Shift een uur, met Alt een seconde.
map-geotag-reading = Foto’s worden gelezen…
map-geotag-summary = { $count } van { $total } foto’s krijgen een positie.
map-geotag-sidecars = Posities gaan in XMP-sidecars naast de foto’s. De fotobestanden worden niet gewijzigd.
map-geotag-writing = Schrijven…
map-geotag-write = Sidecars schrijven…
map-geotag-guess = Verschil schatten
map-geotag-will-write = Krijgt { $position }
map-geotag-has-position = Heeft al een positie: blijft zoals hij is
map-geotag-no-time = Geen opnametijd: blijft zoals hij is
map-geotag-outside = { $offset } buiten de track genomen: blijft zoals hij is
map-geotag-gap = In een gat in de track ({ $seconds } s van een fix): blijft zoals hij is
map-geotag-nothing = Bij dit verschil krijgt geen enkele foto een positie.
map-geotag-confirm = Posities schrijven in { $count ->
    [one] { $count } XMP-sidecar
   *[other] { $count } XMP-sidecars
}? De foto’s zelf worden niet gewijzigd.
map-geotag-confirm-ok = Sidecars schrijven
map-geotag-done = { $count ->
    [one] { $count } sidecar geschreven
   *[other] { $count } sidecars geschreven
}
map-geotag-done-failed = { $count ->
        [one] { $count } sidecar geschreven
       *[other] { $count } sidecars geschreven
    }; { $failed } mislukt: { $reason }
map-geotag-not-sidecar = Geen naam van een sidecar.

## Export and printing.
map-export-title = Kaart exporteren
map-print-title = Kaart op schaal afdrukken
map-export-keys = ↑ ↓ kiezen, ← → wijzigen, Enter start.
map-export-format = Formaat
map-export-scale = Schaal
map-export-scale-value = 1:{ $n }
map-export-resolution = Resolutie
map-export-dpi = { $dpi } dpi
map-export-paper = Papier
map-export-size = { $width } × { $height } px · { $across } × { $down } km op de grond
map-export-save = Opslaan…
map-export-print = Afdrukken…
map-export-too-large = Te groot op deze schaal: { $width } × { $height } px. Kies een kleinere schaal of resolutie.
map-export-no-scale = Deze kaart heeft geen schaal op de grond.
map-export-no-gpu = Exporteren vereist de grafische processor.
map-export-subtitle = Schaal { $scale } · { $crs }
map-export-data = Gegevens: { $sources }
map-export-saved = { $name } opgeslagen
map-export-printing = Afdrukken…
map-export-printed = Naar de printer gestuurd
map-export-print-failed = Niet afgedrukt: { $reason }
