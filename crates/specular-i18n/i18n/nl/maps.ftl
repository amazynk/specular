# machine-drafted, needs native review
# Specular — Maps (the Maps profile): the map view of georeferenced and vector map files, its
# layer panel, attribute table, coordinates, measuring, elevation profile and map furniture.
# Nederlands.

keys-mode-map = Kaarten
# Cheat sheet groups of the Maps tab.
cheatsheet-map-layers = Lagen
cheatsheet-map-coordinates = Coördinaten
cheatsheet-map-measure = Meten

## Palette entries and their one-line descriptions.
palette-map-layers = Lagenpaneel tonen of verbergen
palette-map-layers-help = De lagen van de kaart: volgorde, dekking, zichtbaarheid en stijl.
palette-map-add-layer = Laag toevoegen uit bestand…
palette-map-add-layer-help = Teken een ander kaartbestand over deze, zoals een GPX-track over een shapefile.
palette-map-zoom-to-layer = Zoomen naar de laag
palette-map-zoom-to-layer-help = Toon de hele laag die in het lagenpaneel is gekozen.
palette-map-attribute-table = Attributentabel tonen of verbergen
palette-map-attribute-table-help = De objecten van de gekozen laag als tabel: sorteren, filteren, zoeken en selecteren.
palette-map-coordinate-format = Volgende coördinaatnotatie
palette-map-coordinate-format-help = Decimale graden, graden minuten seconden, UTM of MGRS.
palette-map-units = Metrische of imperiale eenheden
palette-map-units-help = Lengtes en oppervlakten in meters of in voet en mijl.
palette-map-copy-coordinate = Coördinaat kopiëren
palette-map-copy-coordinate-help = Kopieer de positie onder de aanwijzer, of die van het midden van de kaart.
palette-map-go-to-coordinate = Naar coördinaat…
palette-map-go-to-coordinate-help = Typ een positie in elke notatie om de kaart daar te centreren.
palette-map-measure-distance = Afstand meten
palette-map-measure-distance-help = Klik punten op de kaart: de lengte over het aardoppervlak en de koers.
palette-map-measure-area = Oppervlakte meten
palette-map-measure-area-help = Klik de hoeken van een gebied: de grootte en omtrek over het aardoppervlak.
palette-map-elevation-profile = Hoogteprofiel
palette-map-elevation-profile-help = Teken een lijn over een hoogtemodel om de hoogtes in een grafiek te zien.
palette-map-overview = Overzichtskaart tonen of verbergen
palette-map-overview-help = Een kleine kaart van het hele gebied, met het deel op het scherm gemarkeerd.

## Why a map command is unavailable.
disabled-not-map = Alleen voor kaarten, met kaarthulpmiddelen aan
disabled-no-lonlat = De kaart heeft geen bekend coördinatenstelsel
disabled-no-features = De kaart heeft geen objecten
disabled-no-heights = De kaart heeft geen hoogtes

## Settings › Maps.
settings-section-maps = Kaarten
settings-maps-coordinate-format = Coördinaatnotatie
settings-maps-coordinate-format-help = Hoe posities worden getoond en gekopieerd. Naar coördinaat leest elke notatie.
settings-maps-coordinate-format-dd = Decimale graden
settings-maps-coordinate-format-dms = Graden, minuten, seconden
settings-maps-coordinate-format-utm = UTM
settings-maps-coordinate-format-mgrs = MGRS / USNG
settings-maps-units = Eenheden
settings-maps-units-help = Lengtes en oppervlakten op kaarten: de schaalbalk en metingen. Automatisch volgt Metagegevens › Eenheden.
settings-maps-units-auto = Automatisch
settings-maps-units-metric = Metrisch
settings-maps-units-imperial = Imperiaal

## The pointer readout. $position is written in the chosen format.
map-position = { $format ->
    [utm] UTM { $position }
    [mgrs] MGRS { $position }
    [dms] { $position }
   *[dd] Breedte, lengte { $position }
}
map-height = Hoogte { $height }
map-height-label = Hoogte
# A length or area with its unit symbol ("12.3 km", "4.5 ha").
map-length = { $value } { $unit }
map-scale-label = Schaalbalk: { $length }
map-north-label = Noordpijl: noord ligt { $degrees }° met de klok mee vanaf boven
# The coordinate formats, for the toast after "Next coordinate format".
map-format-name = { $format ->
    [dms] graden, minuten, seconden
    [utm] UTM
    [mgrs] MGRS
   *[dd] decimale graden
}
map-format-toast = Coördinaten in { $format }
map-units-metric = Eenheden: meters en kilometers
map-units-imperial = Eenheden: voet en mijlen
toast-copied-coordinate = Coördinaat gekopieerd

## Go to coordinate.
map-goto-hint = Decimale graden, DMS, UTM of MGRS. Enter gaat erheen.
map-goto-empty = Typ een positie.
map-goto-unrecognised = Geen positie in een bekende notatie.
map-goto-range = Breedte of lengte buiten bereik.
map-goto-no-way = Deze kaart kan geen positie plaatsen.
map-goto-outside = Die positie ligt buiten de kaart.
map-goto-toast = Gecentreerd op { $position }

## The layer panel.
map-layers-title = Lagen: { $shown } van { $total } getoond
# Under a layer's name: its kind, features and opacity.
map-layer-sub = { $kind } · { $features } · { PERCENT($opacity) }
# A layer's row for screen readers.
map-layer-row = { $name }: { $kind }, { $features ->
    [one] { $features } object
   *[other] { $features } objecten
}, { PERCENT($opacity) } dekkend
map-kind-area = Vlakken
map-kind-line = Lijnen
map-kind-point = Punten
map-kind-empty = Geen objecten
map-layer-visible = Laag tonen of verbergen
map-layer-opacity-down = Minder dekkend
map-layer-opacity-up = Meer dekkend
map-layer-colour = Volgende kleur
map-layer-width = Volgende lijndikte
map-layer-point = Volgende puntgrootte
map-layer-up = Omhoog
map-layer-down = Omlaag
map-layer-remove = Verwijderen
map-layer-keep = Behouden
map-layer-remove-ask = { $name } van de kaart verwijderen? Het bestand blijft op de schijf.
map-layer-adding = Laag toevoegen…
map-removed = { $name } van de kaart verwijderd
map-added = { $layers ->
    [one] { $name } toegevoegd
   *[other] { $name } toegevoegd: { $layers } lagen
}
map-add-failed = Kan { $name } niet toevoegen: { $reason }
map-add-filter = Kaartbestanden
map-add-not-map = het is geen kaart.
map-add-not-vector = het heeft geen objecten.
map-add-no-crs = het heeft geen projectiebestand (.prj).

## Identify.
map-identify-more = en nog { $count } in de attributentabel
map-cell = Cel { $x }, { $y }
map-band = Band { $n }
map-no-data = Geen gegevens

## The attribute table.
map-table-title = { $layer }: { $shown } van { $total } objecten, { $selected } geselecteerd
map-table-search = Zoeken, of veld = waarde
map-table-selected-only = Alleen geselecteerd
map-table-empty = Geen objecten komen overeen.
map-table-many = veel
map-table-stats-number = { $field }: { $count ->
        [one] { $count } waarde
       *[other] { $count } waarden
    }, { $empty } leeg · min { $min } · max { $max } · gemiddelde { $mean } · som { $sum } · std.afw. { $sd }
map-table-stats-text = { $field }: { $count ->
        [one] { $count } waarde
       *[other] { $count } waarden
    }, { $empty } leeg · { $distinct } verschillend
map-table-stats-text-top = { $field }: { $count ->
        [one] { $count } waarde
       *[other] { $count } waarden
    }, { $empty } leeg · { $distinct } verschillend · meest voorkomend { $top } ({ $times }×)

## Measuring and the elevation profile.
map-measure-hint-distance = Klik punten langs de route.
map-measure-hint-area = Klik de hoeken van het gebied.
map-measure-hint-profile = Klik punten langs de lijn.
map-measure-keys = Enter rondt af. Backspace verwijdert het laatste punt. Esc sluit.
map-measure-distance = Afstand { $length }
map-measure-bearing = Koers { $bearing }
map-measure-segment = Laatste deel { $length }, koers { $bearing }
map-measure-area = Oppervlakte { $area }
map-measure-perimeter = Omtrek { $length }
map-bearing-value = { $degrees }°
map-profile-summary = Lengte { $length } · laagste { $min } · hoogste { $max } · stijging { $climb } · daling { $descent }
map-profile-no-heights = Lengte { $length }. Geen hoogtes langs de lijn.
map-profile-at = { $distance }: { $height }
map-profile-at-none = { $distance }: geen hoogte
