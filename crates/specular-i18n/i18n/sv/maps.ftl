# machine-drafted, needs native review
# Specular — Maps (the Maps profile): the map view of georeferenced and vector map files, its
# layer panel, attribute table, coordinates, measuring, elevation profile and map furniture.
# Svenska.

keys-mode-map = Kartor
# Cheat sheet groups of the Maps tab.
cheatsheet-map-layers = Lager
cheatsheet-map-coordinates = Koordinater
cheatsheet-map-measure = Mätning

## Palette entries and their one-line descriptions.
palette-map-layers = Visa eller dölj lagerpanelen
palette-map-layers-help = Kartans lager: ordning, opacitet, synlighet och stil.
palette-map-add-layer = Lägg till ett lager från en fil…
palette-map-add-layer-help = Rita en annan kartfil över den här, till exempel ett GPX-spår över en shapefil.
palette-map-zoom-to-layer = Zooma till lagret
palette-map-zoom-to-layer-help = Visa hela lagret som valts i lagerpanelen.
palette-map-attribute-table = Visa eller dölj attributtabellen
palette-map-attribute-table-help = Det valda lagrets objekt som tabell: sortera, filtrera, sök och markera.
palette-map-coordinate-format = Nästa koordinatformat
palette-map-coordinate-format-help = Decimalgrader, grader minuter sekunder, UTM eller MGRS.
palette-map-units = Metriska eller brittiska/amerikanska enheter
palette-map-units-help = Längder och ytor i meter eller i fot och miles.
palette-map-copy-coordinate = Kopiera koordinat
palette-map-copy-coordinate-help = Kopiera positionen under pekaren, eller kartans mittpunkt.
palette-map-go-to-coordinate = Gå till koordinat…
palette-map-go-to-coordinate-help = Skriv en position i valfritt format för att centrera kartan där.
palette-map-measure-distance = Mät ett avstånd
palette-map-measure-distance-help = Klicka punkter på kartan: längden på jordytan och bäringen.
palette-map-measure-area = Mät en yta
palette-map-measure-area-help = Klicka hörnen på ett område: dess storlek och omkrets på jordytan.
palette-map-elevation-profile = Höjdprofil
palette-map-elevation-profile-help = Rita en linje över en höjdmodell för att se höjderna i ett diagram.
palette-map-overview = Visa eller dölj översiktskartan
palette-map-overview-help = En liten karta över hela utbredningen, med delen på skärmen markerad.

## Why a map command is unavailable.
disabled-not-map = Bara för kartor, med kartverktyg på
disabled-no-lonlat = Kartan har inget känt koordinatsystem
disabled-no-features = Kartan har inga objekt
disabled-no-heights = Kartan har inga höjder

## Settings › Maps.
settings-section-maps = Kartor
settings-maps-coordinate-format = Koordinatformat
settings-maps-coordinate-format-help = Hur positioner visas och kopieras. Gå till koordinat läser alla format.
settings-maps-coordinate-format-dd = Decimalgrader
settings-maps-coordinate-format-dms = Grader, minuter, sekunder
settings-maps-coordinate-format-utm = UTM
settings-maps-coordinate-format-mgrs = MGRS / USNG
settings-maps-units = Enheter
settings-maps-units-help = Längder och ytor på kartor: skalstocken och mätningar. Automatiskt följer Metadata › Enheter.
settings-maps-units-auto = Automatiskt
settings-maps-units-metric = Metriska
settings-maps-units-imperial = Brittiska/amerikanska

## The pointer readout. $position is written in the chosen format.
map-position = { $format ->
    [utm] UTM { $position }
    [mgrs] MGRS { $position }
    [dms] { $position }
   *[dd] Lat, long { $position }
}
map-height = Höjd { $height }
map-height-label = Höjd
# A length or area with its unit symbol ("12.3 km", "4.5 ha").
map-length = { $value } { $unit }
map-scale-label = Skalstock: { $length }
map-north-label = Nordpil: norr är { $degrees }° medurs från uppåt
# The coordinate formats, for the toast after "Next coordinate format".
map-format-name = { $format ->
    [dms] grader, minuter, sekunder
    [utm] UTM
    [mgrs] MGRS
   *[dd] decimalgrader
}
map-format-toast = Koordinater i { $format }
map-units-metric = Enheter: meter och kilometer
map-units-imperial = Enheter: fot och miles
toast-copied-coordinate = Koordinaten kopierades

## Go to coordinate.
map-goto-hint = Decimalgrader, DMS, UTM eller MGRS. Enter går dit.
map-goto-empty = Skriv en position.
map-goto-unrecognised = Inte en position i ett känt format.
map-goto-range = Latitud eller longitud utanför intervallet.
map-goto-no-way = Den här kartan kan inte placera en position.
map-goto-outside = Positionen ligger utanför kartan.
map-goto-toast = Centrerad på { $position }

## The layer panel.
map-layers-title = Lager: { $shown } av { $total } visas
# Under a layer's name: its kind, features and opacity.
map-layer-sub = { $kind } · { $features } · { PERCENT($opacity) }
# A layer's row for screen readers.
map-layer-row = { $name }: { $kind }, { $features ->
    [one] { $features } objekt
   *[other] { $features } objekt
}, { PERCENT($opacity) } opak
map-kind-area = Ytor
map-kind-line = Linjer
map-kind-point = Punkter
map-kind-empty = Inga objekt
map-layer-visible = Visa eller dölj lagret
map-layer-opacity-down = Mindre opak
map-layer-opacity-up = Mer opak
map-layer-colour = Nästa färg
map-layer-width = Nästa linjebredd
map-layer-point = Nästa punktstorlek
map-layer-up = Flytta upp
map-layer-down = Flytta ned
map-layer-remove = Ta bort
map-layer-keep = Behåll
map-layer-remove-ask = Ta bort { $name } från kartan? Filen finns kvar på disken.
map-layer-adding = Lägger till ett lager…
map-removed = { $name } togs bort från kartan
map-added = { $layers ->
    [one] { $name } lades till
   *[other] { $name } lades till: { $layers } lager
}
map-add-failed = Kan inte lägga till { $name }: { $reason }
map-add-filter = Kartfiler
map-add-not-map = det är inte en karta.
map-add-not-vector = den har inga objekt.
map-add-no-crs = den har ingen projektionsfil (.prj).

## Identify.
map-identify-more = och { $count } till i attributtabellen
map-cell = Cell { $x }, { $y }
map-band = Band { $n }
map-no-data = Inga data

## The attribute table.
map-table-title = { $layer }: { $shown } av { $total } objekt, { $selected } markerade
map-table-search = Sök, eller fält = värde
map-table-selected-only = Bara markerade
map-table-empty = Inga objekt matchar.
map-table-many = många
map-table-stats-number = { $field }: { $count ->
        [one] { $count } värde
       *[other] { $count } värden
    }, { $empty } tomma · min { $min } · max { $max } · medel { $mean } · summa { $sum } · std.avv. { $sd }
map-table-stats-text = { $field }: { $count ->
        [one] { $count } värde
       *[other] { $count } värden
    }, { $empty } tomma · { $distinct } olika
map-table-stats-text-top = { $field }: { $count ->
        [one] { $count } värde
       *[other] { $count } värden
    }, { $empty } tomma · { $distinct } olika · vanligast { $top } ({ $times }×)

## Measuring and the elevation profile.
map-measure-hint-distance = Klicka punkter längs vägen.
map-measure-hint-area = Klicka områdets hörn.
map-measure-hint-profile = Klicka punkter längs linjen.
map-measure-keys = Enter avslutar. Backspace tar bort den sista punkten. Esc stänger.
map-measure-distance = Avstånd { $length }
map-measure-bearing = Bäring { $bearing }
map-measure-segment = Sista delen { $length }, bäring { $bearing }
map-measure-area = Yta { $area }
map-measure-perimeter = Omkrets { $length }
map-bearing-value = { $degrees }°
map-profile-summary = Längd { $length } · lägst { $min } · högst { $max } · stigning { $climb } · nedstigning { $descent }
map-profile-no-heights = Längd { $length }. Inga höjder längs linjen.
map-profile-at = { $distance }: { $height }
map-profile-at-none = { $distance }: ingen höjd
