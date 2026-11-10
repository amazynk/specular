---
id: maps
title: Maps and terrain
section: modes
contexts: maps
keywords: map, gis, geotiff, shapefile, geojson, geopackage, kml, gpx, srtm, dted, usgs dem, esri ascii grid, terrain, elevation, dem, hillshade, coordinates, latitude, longitude, utm, mgrs, usng, dms, layers, attribute table, measure, distance, area, bearing, elevation profile, scale bar, north arrow, overview, mbtiles, pmtiles, basemap, tiles, openstreetmap, datum, wgs 84, flatgeobuf, geoparquet, geoarrow, gml, osm, pbf, mapinfo, file geodatabase, fit, tcx, nitf, geojp2, gmljp2, ecw, mrsid, geopdf, cog, slope, aspect, contours, 3d, time slider, netcdf, grib, wind, weather, wms, wmts, wfs, ogc api, stac, geotag, xmp, export, print, scale, wkt, nautical chart, bsb, kap, s-57, enc, er mapper, ers, saga, sgrd, arc/info grid, adf, vrt, virtual raster, topojson, igc, flight log, glider, nmea
---
Map files open where they are on the Earth: vector maps, georeferenced images and elevation models.

JPEG 2000 images are placed by their GeoJP2 or GMLJP2 data.

A GDAL virtual raster (.vrt) is put together from the rasters it names. They are read from disk, never from a server.

TopoJSON files open like GeoJSON: one layer per collection. ECW and MrSID images don’t open: see [Optional decoders](optional-decoders).

The Maps [profile](profiles) adds the map view. Without it, maps open as images and drawings.

If a map opens without Maps on, the toolbar chip offers the Maps tools.

## Coordinates {#coordinates}
The readout shows the position under the pointer. A projected map also shows its own coordinates.

Longitude and latitude are on WGS 84, as GPS gives them. When the datum's shift to WGS 84 is not known, the datum is named after them.

Some projections are not supported. The readout then says so instead of showing wrong numbers.

- [[key:map-coordinate-format]]: decimal degrees, degrees minutes seconds, UTM or MGRS
- [[key:map-copy-coordinate]]: copies the position under the pointer, or of the centre
- [[key:map-go-to-coordinate]]: type a position in any of these formats; Enter centres the map there
- [[key:map-units]]: metres or feet and miles
- [[key:coordinate-grid]]: draws a coordinate grid over a georeferenced image

Coordinate commands are unavailable when the map's coordinate system is unknown.

The format and units are also in Settings › Maps.

## Layers {#layers}
- [[key:map-layers]]: the layer panel; the top layer is listed first
- [[key:map-add-layer]]: draws another map file over this one, such as a GPX track over a shapefile
- [[key:map-zoom-to-layer]]: shows the whole of the chosen layer

In the panel, ↑ and ↓ choose a layer and Space shows or hides it.

← and → walk the chosen layer's buttons: opacity, colour, line width, point size, order, zoom, table.

Colours from the file are kept (KML styles, GeoJSON simplestyle) until you choose another.

Removing an added layer asks first. The file stays on disk.

## Identify and attributes {#attributes}
Click a feature to see its attributes; Shift-click adds it to the selection.

Click an image or elevation model to see the value in each band.

- [[key:map-attribute-table]]: the chosen layer's features as a table

Click a heading to sort. Type in the search field to filter: any text, or `field = value`, `field > 10`, `field: text`.

Selected rows are selected on the map, and the other way round.

The line under the table gives the focused column's statistics.

The table is unavailable for maps without features.

## Measuring {#measure}
Lengths and areas are measured on the Earth's ellipsoid (WGS 84).

- [[key:map-measure-distance]]: click points; shows the distance and the bearing
- [[key:map-measure-area]]: click corners; shows the area and the perimeter

Enter finishes, Backspace removes the last point, Esc closes.

From the keyboard, the arrows move a crosshair and Space places a point.

## Elevation profile {#profile}
- [[key:map-elevation-profile]]: draw a line over an elevation model, then press Enter

The chart shows the length, lowest and highest points, and total climb and descent.

← and → move along the chart; the point is marked on the map.

The profile is unavailable for maps without heights.

## Map furniture {#furniture}
- The scale bar shows a round length at the centre of the view.
- The north arrow appears when north is not up.
- [[key:map-overview]]: a small world map with the map and the part on screen

## Terrain {#terrain}
Elevation files are coloured by height.

USGS DEM (.dem) and Esri ASCII grid (.asc) files are elevation models too.

So are ER Mapper (.ers), SAGA (.sgrd) and Arc/Info binary grids. An Arc/Info grid is a folder: open the folder.

- [[key:hillshade]]: shades the land as if lit from the north-west
- [[key:map-slope]]: colours the land by how steep it is
- [[key:map-aspect]]: colours the land by the way its slopes face
- [[key:map-contours]]: contour lines, with every fifth one labelled
- [[key:map-terrain-3d]]: the elevation model in 3-D, with its colours draped over it

The readout gives the slope and the way it faces under the pointer.

In 3-D, low relief is exaggerated so it shows; the info says by how much. Esc goes back to the map.

These are unavailable for files without heights.

## Time and weather {#time}
- [[key:map-time]]: a slider through a grid's times, or along a GPS track's times
- [[key:map-play]]: plays the slider; a grid waits for each step to load
- [[key:map-weather-colours]]: colour maps made for temperature, rain, wind and pressure
- [[key:map-wind]]: wind arrows from a grid's u and v components

With the slider focused, ← and → step, Space plays, Home and End jump to the ends.

A recognised weather grid starts in its colours. The last choice goes back to the file's.

## Online map services {#online}
- [[key:map-services]]: layers from WMS, WMTS, WFS, OGC API and STAC servers

They are off until you turn them on, in the panel or in Settings › Maps › Online map services.

Before the first request to a server, Specular says what that server sees and asks.

Tile layers (WMS, WMTS, OGC API Tiles) become the photo map's base map. Feature layers (WFS, OGC API Features) are added to this map for the area on screen.

A STAC search lists the images of the area on screen. Enter downloads one into the cache and opens it.

Saved servers are listed in Settings › Maps. Removing one asks first; nothing on disk is lost.

## Nautical charts {#charts}
BSB/KAP raster charts (.kap) open in their day colours, placed by their reference points. A .bsb file opens the chart's first panel.

S-57 vector charts (.000) open with one layer per kind of object, such as DEPARE for depth areas.

Chart symbols are not drawn: areas, lines and points show in layer colours.

An S-57 update file (.001 and on) shows only what it changes.

## Photos and GPS tracks {#photos}
IGC flight logs and NMEA GPS logs open as tracks, with the flight's or walk's start, end and duration.

- [[key:map-photos-on-track]]: places the folder's photos along a GPX or TCX track by the time they were taken
- [[key:map-geotag]]: writes the track's positions into the photos' XMP sidecar files

The camera's clock is rarely right. Specular guesses the offset in whole hours; ← and → change it by a minute.

The preview lists each photo and what happens to it. Photos with a position are left alone.

Writing asks first. Only sidecar files are written; the photos themselves are never changed.

A photo's pin shows the way the camera looked, when its file records it.

## Drone photos {#drone}
On the gallery's map, a drone photo's arrow follows its gimbal, else the aircraft, else the photo's GPS direction.

Hovering a drone photo shows its height above the take-off point.

From the map's menu:
- Save photos as KMZ or KML: the selected photos, or all on the map. A KMZ holds arrows turned to each heading and a small copy of each photo.
- Save photos as KMZ with full photos: the same, with the photos themselves inside.

A KML links the photos where they are on disk.

## Export and printing {#export}
- [[key:map-export]]: saves the map as PNG or PDF at a scale, such as 1:25,000
- [[key:map-print]]: prints the map to scale
- [[key:map-copy-extent]]: copies the area on screen as a bounding box; again within a few seconds, as WKT

Under the map are the title, scale, coordinate system, a scale bar, a north arrow, a legend and the data's sources.

A PNG records its resolution, so it prints at the scale it was made for.

## Map tiles {#tiles}
MBTiles and PMTiles files open as maps. Vector tiles are drawn in a simple style.

## Base map {#basemap}
The photo map uses a built-in offline map.

Settings › Metadata and privacy › Base map can add detail:

- **My own map file**: an MBTiles or PMTiles file. It works offline.
- **OpenStreetMap (online)** or **Other tile server (online)**: tiles download for the area you view.
- **A map service layer**: chosen in the map's [online services](#online).

Before the first download, Specular explains what the tile server sees and asks.

Downloaded tiles stay in a cache. Its size is set under Map tile cache.

## Keys {#keys}
::: keys maps
