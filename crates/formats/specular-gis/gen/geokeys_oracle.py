"""GeoTIFFs with user-defined projections, written by GDAL (rasterio), and PROJ's longitude
and latitude (WGS 84) of sample pixels: the oracle for GeoKey interpretation
(`specular-decode/tests/geokeys_oracle.rs` reads DIR from SPECULAR_GEOKEYS_ORACLE).

Usage: python -I geokeys_oracle.py DIR
"""

import json
import sys

import numpy as np
import rasterio
from pyproj import CRS, Transformer
from rasterio.transform import from_origin

d = sys.argv[1]
CASES = {
    # name: (PROJ string, origin x, origin y, pixel)
    "tm_south": ("+proj=tmerc +lat_0=0 +lon_0=27 +k=1 +x_0=0 +y_0=0 +axis=wsu +ellps=WGS84 +units=m", -50000, 3000000, 100),
    "polar_a": ("+proj=stere +lat_0=90 +lon_0=0 +k=0.994 +x_0=2000000 +y_0=2000000 +ellps=WGS84 +units=m", 1900000, 2100000, 1000),
    "polar_b": ("+proj=stere +lat_0=-90 +lat_ts=-71 +lon_0=0 +x_0=0 +y_0=0 +ellps=WGS84 +units=m", -100000, 100000, 1000),
    "laea": ("+proj=laea +lat_0=52 +lon_0=10 +x_0=4321000 +y_0=3210000 +ellps=GRS80 +units=m", 4300000, 3300000, 500),
    "sterea": ("+proj=sterea +lat_0=52.1561605555556 +lon_0=5.38763888888889 +k=0.9999079 +x_0=155000 +y_0=463000 +ellps=bessel +towgs84=565.4171,50.3319,465.5524,1.9342,-1.6677,9.1019,4.0725 +units=m", 150000, 470000, 50),
    "omerc": ("+proj=omerc +lat_0=4 +lonc=102.25 +alpha=323.0257905 +gamma=323.1301023611 +k=0.99984 +x_0=804671 +y_0=0 +ellps=evrst69 +units=m", 400000, 500000, 100),
    "cass": ("+proj=cass +lat_0=31.7340969444444 +lon_0=35.2120805555556 +x_0=170251.555 +y_0=126867.909 +a=6378300.789 +b=6356566.435 +towgs84=-275.722,94.7824,340.894,-8.001,-4.42,-11.821,1 +units=m", 160000, 140000, 50),
    "eqc": ("+proj=eqc +lat_ts=30 +lon_0=0 +x_0=0 +y_0=0 +ellps=WGS84 +units=m", 1000000, 4000000, 1000),
    "lcc_towgs84": ("+proj=lcc +lat_0=46.5 +lon_0=3 +lat_1=49 +lat_2=44 +x_0=700000 +y_0=6600000 +ellps=intl +towgs84=-87,-98,-121,0,0,0,0 +units=m", 650000, 6650000, 100),
}
out = []
for name, (p, x0, y0, px) in CASES.items():
    crs = CRS.from_proj4(p)
    path = f"{d}/{name}.tif"
    with rasterio.open(
        path,
        "w",
        driver="GTiff",
        width=16,
        height=16,
        count=1,
        dtype="uint8",
        crs=crs,
        transform=from_origin(x0, y0, px, px),
    ) as w:
        w.write(np.zeros((1, 16, 16), dtype="uint8"))
    t = Transformer.from_crs(crs, CRS.from_epsg(4326), always_xy=True)
    pts = []
    for i, j in [(0.5, 0.5), (8, 8), (15.5, 15.5)]:
        mx, my = x0 + i * px, y0 - j * px
        lon, lat = t.transform(mx, my)
        pts.append([i, j, mx, my, lon, lat])
    out.append({"file": f"{name}.tif", "proj": p, "points": pts})
with open(f"{d}/expected.json", "w") as f:
    json.dump(out, f, indent=1)
print(f"{len(out)} files")
