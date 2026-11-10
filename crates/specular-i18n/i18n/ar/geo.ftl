# machine-drafted, needs native review
# Geodata: the readout over a map (GeoTIFF, Shapefile, GeoJSON, GeoPackage, KML, GPX) and
# the info panel's group for a clicked feature.

# The pointer's map coordinates in the CRS's unit ("m", "US survey foot"): easting, northing.
# Unit and ° come before the number so they show on its right in right-to-left text.
geo-map-position = شرقًا { $unit } { $x }  شمالًا { $unit } { $y }
# The same with no known unit.
geo-map-position-plain = x { $x }  y { $y }
# Longitude and latitude in decimal degrees on WGS 84.
geo-lonlat = خط الطول °{ $lon }  خط العرض °{ $lat }
# machine-drafted, needs native review
# The same on the file's own datum, when its shift to WGS 84 is not known; $datum is its
# name ("North American Datum 1927").
geo-lonlat-datum = خط الطول °{ $lon }  خط العرض °{ $lat } ({ $datum })
# Shown under the pointer when the map's projection cannot be converted to longitude and
# latitude.
geo-unsupported = الإحداثيات غير مدعومة لهذا الإسقاط
# The feature under the pointer.
geo-feature = المعلم: { $name }
geo-feature-in-layer = المعلم: { $name } ({ $layer })

# Info panel group listing the clicked feature's attributes.
geo-feature-group = المعلم
geo-feature-layer = الطبقة
geo-feature-geometry = الشكل الهندسي
# $kind is the geometry type ("Polygon", "LineString"…), not translated.
geo-feature-geometry-value = { $kind }، { $vertices ->
    [zero] { $vertices } رأس
    [one] رأس واحد
    [two] رأسان
    [few] { $vertices } رؤوس
    [many] { $vertices } رأسًا
   *[other] { $vertices } رأس
}
geo-feature-id = معرّف المعلم
