# machine-drafted, needs native review
# Geodata: the readout over a map (GeoTIFF, Shapefile, GeoJSON, GeoPackage, KML, GPX) and
# the info panel's group for a clicked feature.

# The pointer's map coordinates in the CRS's unit ("m", "US survey foot"): easting, northing.
geo-map-position = O { $x }  N { $y } { $unit }
# The same with no known unit.
geo-map-position-plain = x { $x }  y { $y }
# Longitude and latitude in decimal degrees on WGS 84.
geo-lonlat = Lengte { $lon }°  Breedte { $lat }°
# machine-drafted, needs native review
# The same on the file's own datum, when its shift to WGS 84 is not known; $datum is its
# name ("North American Datum 1927").
geo-lonlat-datum = Lengte { $lon }°  Breedte { $lat }° ({ $datum })
# Shown under the pointer when the map's projection cannot be converted to longitude and
# latitude.
geo-unsupported = Coördinaten niet ondersteund voor deze projectie
# The feature under the pointer.
geo-feature = Object: { $name }
geo-feature-in-layer = Object: { $name } ({ $layer })

# Info panel group listing the clicked feature's attributes.
geo-feature-group = Object
geo-feature-layer = Laag
geo-feature-geometry = Geometrie
# $kind is the geometry type ("Polygon", "LineString"…), not translated.
geo-feature-geometry-value = { $kind }, { $vertices ->
    [one] { $vertices } hoekpunt
   *[other] { $vertices } hoekpunten
}
geo-feature-id = Object-ID
