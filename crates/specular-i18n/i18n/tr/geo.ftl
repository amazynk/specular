# machine-drafted, needs native review
# Geodata: the readout over a map (GeoTIFF, Shapefile, GeoJSON, GeoPackage, KML, GPX) and
# the info panel's group for a clicked feature.

# The pointer's map coordinates in the CRS's unit ("m", "US survey foot"): easting, northing.
geo-map-position = D { $x }  K { $y } { $unit }
# The same with no known unit.
geo-map-position-plain = x { $x }  y { $y }
# Longitude and latitude in decimal degrees on WGS 84.
geo-lonlat = Boylam { $lon }°  Enlem { $lat }°
# machine-drafted, needs native review
# The same on the file's own datum, when its shift to WGS 84 is not known; $datum is its
# name ("North American Datum 1927").
geo-lonlat-datum = Boylam { $lon }°  Enlem { $lat }° ({ $datum })
# Shown under the pointer when the map's projection cannot be converted to longitude and
# latitude.
geo-unsupported = Bu projeksiyon için koordinatlar desteklenmiyor
# The feature under the pointer.
geo-feature = Obje: { $name }
geo-feature-in-layer = Obje: { $name } ({ $layer })

# Info panel group listing the clicked feature's attributes.
geo-feature-group = Obje
geo-feature-layer = Katman
geo-feature-geometry = Geometri
# $kind is the geometry type ("Polygon", "LineString"…), not translated.
geo-feature-geometry-value = { $kind }, { $vertices ->
   *[other] { $vertices } köşe
}
geo-feature-id = Obje kimliği
