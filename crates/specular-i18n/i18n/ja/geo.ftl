# machine-drafted, needs native review
# Geodata: the readout over a map (GeoTIFF, Shapefile, GeoJSON, GeoPackage, KML, GPX) and
# the info panel's group for a clicked feature.

# The pointer's map coordinates in the CRS's unit ("m", "US survey foot"): easting, northing.
geo-map-position = 東距 { $x }  北距 { $y } { $unit }
# The same with no known unit.
geo-map-position-plain = x { $x }  y { $y }
# Longitude and latitude in decimal degrees on WGS 84.
geo-lonlat = 経度 { $lon }°  緯度 { $lat }°
# machine-drafted, needs native review
# The same on the file's own datum, when its shift to WGS 84 is not known; $datum is its
# name ("North American Datum 1927").
geo-lonlat-datum = 経度 { $lon }°  緯度 { $lat }° ({ $datum })
# Shown under the pointer when the map's projection cannot be converted to longitude and
# latitude.
geo-unsupported = この投影法の座標には対応していません
# The feature under the pointer.
geo-feature = 地物: { $name }
geo-feature-in-layer = 地物: { $name } ({ $layer })

# Info panel group listing the clicked feature's attributes.
geo-feature-group = 地物
geo-feature-layer = レイヤー
geo-feature-geometry = ジオメトリ
# $kind is the geometry type ("Polygon", "LineString"…), not translated.
geo-feature-geometry-value = { $kind }、{ $vertices ->
   *[other] 頂点 { $vertices } 個
}
geo-feature-id = 地物 ID
