# Specular — Maps (the Maps profile): the map view of georeferenced and vector map files, its
# layer panel, attribute table, coordinates, measuring, elevation profile and map furniture.
# English.

keys-mode-map = Maps
# Cheat sheet groups of the Maps tab.
cheatsheet-map-layers = Layers
cheatsheet-map-coordinates = Coordinates
cheatsheet-map-measure = Measuring

## Palette entries and their one-line descriptions.
palette-map-layers = Show or hide the layer panel
palette-map-layers-help = The map's layers: order, opacity, visibility and style.
palette-map-add-layer = Add a layer from a file…
palette-map-add-layer-help = Draw another map file over this one, such as a GPX track over a shapefile.
palette-map-zoom-to-layer = Zoom to the layer
palette-map-zoom-to-layer-help = Show the whole of the layer chosen in the layer panel.
palette-map-attribute-table = Show or hide the attribute table
palette-map-attribute-table-help = The features of the chosen layer as a table: sort, filter, search and select.
palette-map-coordinate-format = Next coordinate format
palette-map-coordinate-format-help = Decimal degrees, degrees minutes seconds, UTM or MGRS.
palette-map-units = Metric or imperial units
palette-map-units-help = Lengths and areas in metres or in feet and miles.
palette-map-copy-coordinate = Copy coordinate
palette-map-copy-coordinate-help = Copy the position under the pointer, or of the map's centre.
palette-map-go-to-coordinate = Go to coordinate…
palette-map-go-to-coordinate-help = Type a position in any format to centre the map there.
palette-map-measure-distance = Measure a distance
palette-map-measure-distance-help = Click points on the map: the length on the Earth's surface and the bearing.
palette-map-measure-area = Measure an area
palette-map-measure-area-help = Click the corners of an area: its size and perimeter on the Earth's surface.
palette-map-elevation-profile = Elevation profile
palette-map-elevation-profile-help = Draw a line over an elevation model to chart its heights.
palette-map-overview = Show or hide the overview map
palette-map-overview-help = A small map of the whole extent, with the part on screen marked.

## Why a map command is unavailable.
disabled-not-map = Only for maps, with Maps tools on
disabled-no-lonlat = The map has no known coordinate system
disabled-no-features = The map has no features
disabled-no-heights = The map has no heights

## Settings › Maps.
settings-section-maps = Maps
settings-maps-coordinate-format = Coordinate format
settings-maps-coordinate-format-help = How positions are shown and copied. Go to coordinate reads every format.
settings-maps-coordinate-format-dd = Decimal degrees
settings-maps-coordinate-format-dms = Degrees, minutes, seconds
settings-maps-coordinate-format-utm = UTM
settings-maps-coordinate-format-mgrs = MGRS / USNG
settings-maps-units = Units
settings-maps-units-help = Lengths and areas on maps: the scale bar and measurements. Automatic follows Metadata › Units.
settings-maps-units-auto = Automatic
settings-maps-units-metric = Metric
settings-maps-units-imperial = Imperial

## The pointer readout. $position is written in the chosen format.
map-position = { $format ->
    [utm] UTM { $position }
    [mgrs] MGRS { $position }
    [dms] { $position }
   *[dd] Lat, lon { $position }
}
map-height = Height { $height }
map-height-label = Height
# A length or area with its unit symbol ("12.3 km", "4.5 ha").
map-length = { $value } { $unit }
map-scale-label = Scale bar: { $length }
map-north-label = North arrow: north is { $degrees }° clockwise from up
# The coordinate formats, for the toast after "Next coordinate format".
map-format-name = { $format ->
    [dms] degrees, minutes, seconds
    [utm] UTM
    [mgrs] MGRS
   *[dd] decimal degrees
}
map-format-toast = Coordinates in { $format }
map-units-metric = Units: metres and kilometres
map-units-imperial = Units: feet and miles
toast-copied-coordinate = Coordinate copied

## Go to coordinate.
map-goto-hint = Decimal degrees, DMS, UTM or MGRS. Enter goes there.
map-goto-empty = Type a position.
map-goto-unrecognised = Not a position in a known format.
map-goto-range = Latitude or longitude out of range.
map-goto-no-way = This map can't place a position.
map-goto-outside = That position is outside the map.
map-goto-toast = Centred on { $position }

## The layer panel.
map-layers-title = Layers: { $shown } of { $total } shown
# Under a layer's name: its kind, features and opacity.
map-layer-sub = { $kind } · { $features } · { PERCENT($opacity) }
# A layer's row for screen readers.
map-layer-row = { $name }: { $kind }, { $features ->
    [one] { $features } feature
   *[other] { $features } features
}, { PERCENT($opacity) } opaque
map-kind-area = Areas
map-kind-line = Lines
map-kind-point = Points
map-kind-empty = No features
map-layer-visible = Show or hide the layer
map-layer-opacity-down = Less opaque
map-layer-opacity-up = More opaque
map-layer-colour = Next colour
map-layer-width = Next line width
map-layer-point = Next point size
map-layer-up = Move up
map-layer-down = Move down
map-layer-remove = Remove
map-layer-keep = Keep
map-layer-remove-ask = Remove { $name } from the map? The file stays on disk.
map-layer-adding = Adding a layer…
map-removed = Removed { $name } from the map
map-added = { $layers ->
    [one] Added { $name }
   *[other] Added { $name }: { $layers } layers
}
map-add-failed = Can't add { $name }: { $reason }
map-add-filter = Map files
map-add-not-map = it is not a map.
map-add-not-vector = it has no features.
map-add-no-crs = it has no projection file (.prj).

## Identify.
map-identify-more = and { $count } more in the attribute table
map-cell = Cell { $x }, { $y }
map-band = Band { $n }
map-no-data = No data

## The attribute table.
map-table-title = { $layer }: { $shown } of { $total } features, { $selected } selected
map-table-search = Search, or field = value
map-table-selected-only = Selected only
map-table-empty = No features match.
map-table-many = many
map-table-stats-number = { $field }: { $count ->
        [one] { $count } value
       *[other] { $count } values
    }, { $empty } empty · min { $min } · max { $max } · mean { $mean } · sum { $sum } · std. dev. { $sd }
map-table-stats-text = { $field }: { $count ->
        [one] { $count } value
       *[other] { $count } values
    }, { $empty } empty · { $distinct } distinct
map-table-stats-text-top = { $field }: { $count ->
        [one] { $count } value
       *[other] { $count } values
    }, { $empty } empty · { $distinct } distinct · most common { $top } ({ $times }×)

## Measuring and the elevation profile.
map-measure-hint-distance = Click points along the way.
map-measure-hint-area = Click the corners of the area.
map-measure-hint-profile = Click points along the line.
map-measure-keys = Enter finishes. Backspace removes the last point. Esc closes.
map-measure-distance = Distance { $length }
map-measure-bearing = Bearing { $bearing }
map-measure-segment = Last part { $length }, bearing { $bearing }
map-measure-area = Area { $area }
map-measure-perimeter = Perimeter { $length }
map-bearing-value = { $degrees }°
map-profile-summary = Length { $length } · lowest { $min } · highest { $max } · climb { $climb } · descent { $descent }
map-profile-no-heights = Length { $length }. No heights along the line.
map-profile-at = { $distance }: { $height }
map-profile-at-none = { $distance }: no height
