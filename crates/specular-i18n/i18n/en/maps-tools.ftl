# Specular — Maps tools: terrain (slope, aspect, contours, 3-D), time, wind and weather colours,
# online map services, photos and geotagging along GPS tracks, and map export.
# English.

## Cheat sheet groups of the Maps tab.
cheatsheet-map-terrain = Terrain
cheatsheet-map-time = Time and weather
cheatsheet-map-online = Online services
cheatsheet-map-photos = Photos and GPS
cheatsheet-map-output = Output

## Palette entries and their one-line descriptions.
palette-map-slope = Slope colours
palette-map-slope-help = Colour an elevation model by how steep it is.
palette-map-aspect = Aspect colours
palette-map-aspect-help = Colour an elevation model by the direction its slopes face.
palette-map-contours = Show or hide contour lines
palette-map-contours-help = Contour lines over an elevation model, labelled every few lines.
palette-map-terrain-3d = Terrain in 3-D
palette-map-terrain-3d-help = Show the elevation model as a 3-D surface with its colours draped over it.
palette-map-time = Show or hide the time slider
palette-map-time-help = Step through a grid's times or along a track's times.
palette-map-play = Play or pause time
palette-map-play-help = Run the time slider on its own, step by step.
palette-map-wind = Show or hide wind arrows
palette-map-wind-help = Arrows of wind speed and direction from the grid's u and v components.
palette-map-weather-colours = Next weather colour map
palette-map-weather-colours-help = Colour maps made for temperature, rain, wind and pressure.
palette-map-services = Online map services…
palette-map-services-help = Add layers from WMS, WMTS, WFS, OGC API or STAC servers.
palette-map-photos-on-track = Show or hide photos on the track
palette-map-photos-on-track-help = Place the folder's photos along the GPS track by the time they were taken.
palette-map-geotag = Geotag photos from the track…
palette-map-geotag-help = Write the track's positions into XMP sidecar files of the folder's photos.
palette-map-export = Export map…
palette-map-export-help = Save the map as PNG or PDF at a scale, with legend, scale bar and north arrow.
palette-map-print = Print map to scale…
palette-map-print-help = Print the map at a chosen scale, such as 1:{ NUMBER(25000) }.
palette-map-copy-extent = Copy extent
palette-map-copy-extent-help = Copy the area on screen as a bounding box, or as WKT.

## Why a map command is unavailable.
disabled-no-time = The map has no times
disabled-no-wind = The map has no wind components (u and v)
disabled-no-track = No GPS track with times
disabled-not-grid = The map is not a grid of values

## Settings › Maps and Metadata.
settings-maps-services = Online map services
settings-maps-services-help = Add layers from WMS, WMTS, WFS, OGC API and STAC servers. Off: nothing is fetched.
settings-maps-services-allowed = Allowed map servers
settings-maps-services-allowed-help = Servers you agreed to after the explanation of what they see.
settings-metadata-basemap-service = A map service layer
settings-metadata-basemap-service-help = The layer chosen in the map's online services panel.

## Terrain.
# A compass point, short: n, ne, e, se, s, sw, w, nw.
map-compass = { $point ->
    [ne] NE
    [e] E
    [se] SE
    [s] S
    [sw] SW
    [w] W
    [nw] NW
   *[n] N
}
map-slope-aspect = Slope { $slope }° · faces { $direction } ({ $aspect }°)
map-slope-flat = Slope { $slope }° · flat
map-slope-on = Slope colours
map-aspect-on = Aspect colours
map-relief-off = Height colours
map-contours-on = Contour lines on
map-contours-off = Contour lines off
map-legend-slope = Slope
map-legend-aspect = Aspect: the way slopes face
map-legend-degrees = { $degrees }°
map-legend-degrees-more = { $degrees }° and more
map-terrain-3d-on = Terrain in 3-D. Esc goes back to the map.
map-terrain-3d-on-exaggerated = Terrain in 3-D, heights × { $times }. Esc goes back to the map.
map-terrain-exaggeration = Heights exaggerated { $times } times.
map-terrain-drape = Map colours

## Time, weather and wind.
map-time-grid = { $when } · { $step } of { $steps }
map-time-track = { $when } · { $elapsed } in
map-time-speed = × { $times }
map-time-play = Play
map-time-pause = Pause
map-weather-map = Colours: { $name }
map-weather-file = Colours from the file
map-wind-on = Wind arrows on
map-wind-off = Wind arrows off
map-wind-at = Wind { $speed } { $unit } from { $degrees }° ({ $direction })

## Copy extent.
map-extent-copied = Bounding box copied. Copy again for WKT.
map-extent-copied-wkt = Extent copied as WKT

## Online map services.
map-services-title = Online map services
map-service-off = Online map services are off.
map-service-off-why = Turn them on to add layers from servers on the internet.
map-service-turn-on = Turn on
map-service-close = Close
map-service-back = Back
map-service-cancel = Cancel
map-service-connect = Connect
map-service-allow = Allow
map-service-remove = Remove
map-service-download = Download
map-service-list-hint = Enter opens a server. Delete removes it.
map-service-add = Add a server…
map-service-add-sub = WMS, WMTS, WFS, OGC API or STAC
map-service-add-hint = The server's address. Enter connects.
map-service-bad-address = Not a web address (http or https, no user name).
map-service-server-sub = { $kind } · { $host }
map-service-consent-title = Online map service
map-service-consent-server = Specular will contact { $host }.
map-service-consent-sees = That server sees your internet address and the map area you ask for.
map-service-consent-private = Your files and their names are not sent.
map-service-asking = Asking { $host }…
map-service-downloading = Downloading from { $host }…
map-service-layers-hint = { $kind } layers. Enter uses one.
map-service-no-layers = The server lists no layers.
map-service-offer-tiles = Tiles: becomes the photo map's base map
map-service-offer-features = Features: added to this map for the area on screen
map-service-offer-search = Images: search the area on screen
map-service-refused-mercator = Not offered in Web Mercator (EPSG:3857), which the base map needs.
map-service-refused-format = Not offered as PNG or JPEG images.
map-service-refused-geojson = Not offered as GeoJSON, the format read here.
map-service-items-hint = { $count ->
        [one] { $count } image found. Enter downloads it.
       *[other] { $count } images found. Enter downloads one.
    }
map-service-no-items = No images found here.
map-service-item-cloud = { $when } · { PERCENT($cloud) } cloud
map-service-item-no-cog = No Cloud-Optimized GeoTIFF
map-service-download-ask = Download { $size } from { $host }?
map-service-download-ask-unknown = Download this image from { $host }? Its size is not known.
map-service-download-cache = It is kept in the map cache and opened.
map-service-size-mb = { $size } MB
map-service-size-gb = { $size } GB
map-service-too-big = The image is { $size }, too large to download.
map-service-opened = Opening the downloaded image
map-service-basemap = { $layer } is now the photo map's base map. The photo map asks before its first download.
map-service-status = The server answered { $status }.
map-service-unknown = Not a map service this version reads.
map-service-exception = The server said: { $message }
map-service-malformed = The server's answer is broken: { $reason }
map-service-remove-ask = Remove “{ $server }” from the saved servers?
map-service-remove-detail = Nothing on disk is lost. Layers already on the map stay until it is closed.
map-service-removed = Removed { $server }
map-service-saved = Saved map servers
map-service-saved-none = None yet. Add one in the map's online services panel.
map-service-keywords = WMS WMTS WFS OGC API STAC servers

## Photos and geotagging.
map-geotag-title = Geotag photos from the track
map-photos-on = Photos on the track
map-photos-off = Photos on the track hidden
map-geotag-offset = Camera clock: { $offset } from GPS time. ← → change it by a minute, with Shift by an hour, Alt by a second.
map-geotag-reading = Reading the photos…
map-geotag-summary = { $count } of { $total } photos get a position.
map-geotag-sidecars = Positions go into XMP sidecar files beside the photos. The photo files are not changed.
map-geotag-writing = Writing…
map-geotag-write = Write sidecars…
map-geotag-guess = Guess offset
map-geotag-will-write = Gets { $position }
map-geotag-has-position = Has a position already: left as it is
map-geotag-no-time = No capture time: left as it is
map-geotag-outside = Taken { $offset } outside the track: left as it is
map-geotag-gap = In a gap of the track ({ $seconds } s from a fix): left as it is
map-geotag-nothing = No photo gets a position at this offset.
map-geotag-confirm = Write positions into { $count ->
    [one] { $count } XMP sidecar file
   *[other] { $count } XMP sidecar files
}? The photos themselves are not changed.
map-geotag-confirm-ok = Write sidecars
map-geotag-done = { $count ->
    [one] Wrote { $count } sidecar
   *[other] Wrote { $count } sidecars
}
map-geotag-done-failed = Wrote { $count ->
        [one] { $count } sidecar
       *[other] { $count } sidecars
    }; { $failed } failed: { $reason }
map-geotag-not-sidecar = Not a sidecar file name.

## Export and printing.
map-export-title = Export map
map-print-title = Print map to scale
map-export-keys = ↑ ↓ choose, ← → change, Enter goes.
map-export-format = Format
map-export-scale = Scale
map-export-scale-value = 1:{ $n }
map-export-resolution = Resolution
map-export-dpi = { $dpi } dpi
map-export-paper = Paper
map-export-size = { $width } × { $height } px · { $across } × { $down } km on the ground
map-export-save = Save…
map-export-print = Print…
map-export-too-large = Too large at this scale: { $width } × { $height } px. Choose a smaller scale or resolution.
map-export-no-scale = This map has no scale on the ground.
map-export-no-gpu = Export needs the graphics processor.
map-export-subtitle = Scale { $scale } · { $crs }
map-export-data = Data: { $sources }
map-export-saved = Saved { $name }
map-export-printing = Printing…
map-export-printed = Sent to the printer
map-export-print-failed = Not printed: { $reason }
