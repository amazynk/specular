# machine-drafted, needs native review
# Specular — Maps (the Maps profile): the map view of georeferenced and vector map files, its
# layer panel, attribute table, coordinates, measuring, elevation profile and map furniture.
# English.

keys-mode-map = Kort
# Cheat sheet groups of the Maps tab.
cheatsheet-map-layers = Lag
cheatsheet-map-coordinates = Koordinater
cheatsheet-map-measure = Måling

## Palette entries and their one-line descriptions.
palette-map-layers = Vis eller skjul lagpanelet
palette-map-layers-help = Kortets lag: rækkefølge, uigennemsigtighed, synlighed og stil.
palette-map-add-layer = Tilføj et lag fra en fil…
palette-map-add-layer-help = Tegn en anden kortfil oven på denne, fx et GPX-spor over en shapefil.
palette-map-zoom-to-layer = Zoom til laget
palette-map-zoom-to-layer-help = Vis hele det lag, der er valgt i lagpanelet.
palette-map-attribute-table = Vis eller skjul attributtabellen
palette-map-attribute-table-help = Det valgte lags objekter som en tabel: sortér, filtrér, søg og vælg.
palette-map-coordinate-format = Næste koordinatformat
palette-map-coordinate-format-help = Decimalgrader, grader minutter sekunder, UTM eller MGRS.
palette-map-units = Metriske eller britisk/amerikanske enheder
palette-map-units-help = Længder og arealer i meter eller i fod og miles.
palette-map-copy-coordinate = Kopiér koordinat
palette-map-copy-coordinate-help = Kopiér positionen under markøren eller kortets midte.
palette-map-go-to-coordinate = Gå til koordinat…
palette-map-go-to-coordinate-help = Skriv en position i et hvilket som helst format for at centrere kortet der.
palette-map-measure-distance = Mål en afstand
palette-map-measure-distance-help = Klik på punkter på kortet: længden på Jordens overflade og retningen.
palette-map-measure-area = Mål et areal
palette-map-measure-area-help = Klik på hjørnerne af et område: dets størrelse og omkreds på Jordens overflade.
palette-map-elevation-profile = Højdeprofil
palette-map-elevation-profile-help = Tegn en linje over en højdemodel for at vise dens højder i et diagram.
palette-map-overview = Vis eller skjul oversigtskortet
palette-map-overview-help = Et lille kort over hele udstrækningen, med den viste del markeret.

## Why a map command is unavailable.
disabled-not-map = Kun for kort, med kortværktøjer slået til
disabled-no-lonlat = Kortet har intet kendt koordinatsystem
disabled-no-features = Kortet har ingen objekter
disabled-no-heights = Kortet har ingen højder

## Settings › Maps.
settings-section-maps = Kort
settings-maps-coordinate-format = Koordinatformat
settings-maps-coordinate-format-help = Hvordan positioner vises og kopieres. Gå til koordinat læser alle formater.
settings-maps-coordinate-format-dd = Decimalgrader
settings-maps-coordinate-format-dms = Grader, minutter, sekunder
settings-maps-coordinate-format-utm = UTM
settings-maps-coordinate-format-mgrs = MGRS / USNG
settings-maps-units = Enheder
settings-maps-units-help = Længder og arealer på kort: målestokken og målinger. Automatisk følger Metadata › Enheder.
settings-maps-units-auto = Automatisk
settings-maps-units-metric = Metrisk
settings-maps-units-imperial = Britisk/amerikansk

## The pointer readout. $position is written in the chosen format.
map-position = { $format ->
    [utm] UTM { $position }
    [mgrs] MGRS { $position }
    [dms] { $position }
   *[dd] Bredde, længde { $position }
}
map-height = Højde { $height }
map-height-label = Højde
# A length or area with its unit symbol ("12.3 km", "4.5 ha").
map-length = { $value } { $unit }
map-scale-label = Målestok: { $length }
map-north-label = Nordpil: nord er { $degrees }° med uret fra op
# The coordinate formats, for the toast after "Next coordinate format".
map-format-name = { $format ->
    [dms] grader, minutter, sekunder
    [utm] UTM
    [mgrs] MGRS
   *[dd] decimalgrader
}
map-format-toast = Koordinater i { $format }
map-units-metric = Enheder: meter og kilometer
map-units-imperial = Enheder: fod og miles
toast-copied-coordinate = Koordinat kopieret

## Go to coordinate.
map-goto-hint = Decimalgrader, DMS, UTM eller MGRS. Enter går derhen.
map-goto-empty = Skriv en position.
map-goto-unrecognised = Ikke en position i et kendt format.
map-goto-range = Bredde eller længde uden for området.
map-goto-no-way = Dette kort kan ikke placere en position.
map-goto-outside = Den position ligger uden for kortet.
map-goto-toast = Centreret på { $position }

## The layer panel.
map-layers-title = Lag: { $shown } af { $total } vist
# Under a layer's name: its kind, features and opacity.
map-layer-sub = { $kind } · { $features } · { PERCENT($opacity) }
# A layer's row for screen readers.
map-layer-row = { $name }: { $kind }, { $features ->
    [one] { $features } objekt
   *[other] { $features } objekter
}, { PERCENT($opacity) } uigennemsigtig
map-kind-area = Flader
map-kind-line = Linjer
map-kind-point = Punkter
map-kind-empty = Ingen objekter
map-layer-visible = Vis eller skjul laget
map-layer-opacity-down = Mindre uigennemsigtig
map-layer-opacity-up = Mere uigennemsigtig
map-layer-colour = Næste farve
map-layer-width = Næste linjebredde
map-layer-point = Næste punktstørrelse
map-layer-up = Flyt op
map-layer-down = Flyt ned
map-layer-remove = Fjern
map-layer-keep = Behold
map-layer-remove-ask = Fjern { $name } fra kortet? Filen bliver på disken.
map-layer-adding = Tilføjer et lag…
map-removed = { $name } fjernet fra kortet
map-added = { $layers ->
    [one] { $name } tilføjet
   *[other] { $name } tilføjet: { $layers } lag
}
map-add-failed = Kan ikke tilføje { $name }: { $reason }
map-add-filter = Kortfiler
map-add-not-map = den er ikke et kort.
map-add-not-vector = den har ingen objekter.
map-add-no-crs = den har ingen projektionsfil (.prj).

## Identify.
map-identify-more = og { $count } mere i attributtabellen
map-cell = Celle { $x }, { $y }
map-band = Bånd { $n }
map-no-data = Ingen data

## The attribute table.
map-table-title = { $layer }: { $shown } af { $total } objekter, { $selected } valgt
map-table-search = Søg, eller felt = værdi
map-table-selected-only = Kun valgte
map-table-empty = Ingen objekter passer.
map-table-many = mange
map-table-stats-number = { $field }: { $count ->
        [one] { $count } værdi
       *[other] { $count } værdier
    }, { $empty } tomme · min. { $min } · maks. { $max } · middel { $mean } · sum { $sum } · std.afv. { $sd }
map-table-stats-text = { $field }: { $count ->
        [one] { $count } værdi
       *[other] { $count } værdier
    }, { $empty } tomme · { $distinct } forskellige
map-table-stats-text-top = { $field }: { $count ->
        [one] { $count } værdi
       *[other] { $count } værdier
    }, { $empty } tomme · { $distinct } forskellige · hyppigst { $top } ({ $times }×)

## Measuring and the elevation profile.
map-measure-hint-distance = Klik på punkter langs vejen.
map-measure-hint-area = Klik på områdets hjørner.
map-measure-hint-profile = Klik på punkter langs linjen.
map-measure-keys = Enter afslutter. Backspace fjerner det sidste punkt. Esc lukker.
map-measure-distance = Afstand { $length }
map-measure-bearing = Retning { $bearing }
map-measure-segment = Sidste stykke { $length }, retning { $bearing }
map-measure-area = Areal { $area }
map-measure-perimeter = Omkreds { $length }
map-bearing-value = { $degrees }°
map-profile-summary = Længde { $length } · laveste { $min } · højeste { $max } · stigning { $climb } · fald { $descent }
map-profile-no-heights = Længde { $length }. Ingen højder langs linjen.
map-profile-at = { $distance }: { $height }
map-profile-at-none = { $distance }: ingen højde
