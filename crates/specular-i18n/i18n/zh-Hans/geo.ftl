# machine-drafted, needs native review
# Geodata: the readout over a map (GeoTIFF, Shapefile, GeoJSON, GeoPackage, KML, GPX) and
# the info panel's group for a clicked feature.

# The pointer's map coordinates in the CRS's unit ("m", "US survey foot"): easting, northing.
geo-map-position = 东 { $x }  北 { $y } { $unit }
# The same with no known unit.
geo-map-position-plain = x { $x }  y { $y }
# Longitude and latitude in decimal degrees on WGS 84.
geo-lonlat = 经度 { $lon }°  纬度 { $lat }°
# machine-drafted, needs native review
# The same on the file's own datum, when its shift to WGS 84 is not known; $datum is its
# name ("North American Datum 1927").
geo-lonlat-datum = 经度 { $lon }°  纬度 { $lat }° ({ $datum })
# Shown under the pointer when the map's projection cannot be converted to longitude and
# latitude.
geo-unsupported = 不支持此投影的坐标
# The feature under the pointer.
geo-feature = 要素：{ $name }
geo-feature-in-layer = 要素：{ $name }（{ $layer }）

# Info panel group listing the clicked feature's attributes.
geo-feature-group = 要素
geo-feature-layer = 图层
geo-feature-geometry = 几何
# $kind is the geometry type ("Polygon", "LineString"…), not translated.
geo-feature-geometry-value = { $kind }，{ $vertices ->
   *[other] { $vertices } 个顶点
}
geo-feature-id = 要素 ID
