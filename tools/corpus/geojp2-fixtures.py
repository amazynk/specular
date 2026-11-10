#!/usr/bin/env python3
"""Georeferenced JPEG 2000 fixtures and their GDAL oracle.

Writes small JP2 files with GDAL's JP2OpenJPEG driver (through rasterio) carrying only a
GeoJP2 box, only a GMLJP2 box (v1 or v2), or both, in several CRSs (geographic with
latitude-first URNs, a northing-first projected CRS, a rotated grid), then reads each back
with GDAL and writes what GDAL says (geotransform, EPSG code) to `oracle.txt`, one line per
file: name, EPSG, six geotransform numbers.

Usage: geojp2-fixtures.py OUT_DIR   (run with the GIS venv: target/gis-venv/bin/python -I)
"""

import os
import sys

os.environ["GDAL_PAM_ENABLED"] = "NO"

import numpy as np  # noqa: E402
import rasterio  # noqa: E402
from rasterio.transform import Affine  # noqa: E402

W, H = 24, 16

CASES = [
    # name, EPSG, geotransform (GDAL order), GeoJP2, GMLJP2, GMLJP2 v2
    ("utm_geojp2", 32633, (500000.0, 10.0, 0.0, 4600000.0, 0.0, -10.0), True, False, False),
    ("utm_gml", 32633, (500000.0, 10.0, 0.0, 4600000.0, 0.0, -10.0), False, True, False),
    ("utm_gml2", 32633, (500000.0, 10.0, 0.0, 4600000.0, 0.0, -10.0), False, True, True),
    ("wgs84_geojp2", 4326, (5.5, 0.25, 0.0, 52.5, 0.0, -0.125), True, False, False),
    ("wgs84_gml", 4326, (5.5, 0.25, 0.0, 52.5, 0.0, -0.125), False, True, False),
    ("wgs84_gml2", 4326, (5.5, 0.25, 0.0, 52.5, 0.0, -0.125), False, True, True),
    ("gk_north_first_gml", 31466, (2500000.0, 2.0, 0.0, 5700000.0, 0.0, -2.0), False, True, False),
    ("rotated_gml", 32633, (500000.0, 8.0, 3.0, 4600000.0, 2.0, -9.0), False, True, False),
    ("rotated_geojp2", 32633, (500000.0, 8.0, 3.0, 4600000.0, 2.0, -9.0), True, False, False),
    ("mercator_both", 3857, (-20000.0, 30.0, 0.0, 6700000.0, 0.0, -30.0), True, True, False),
]


def main():
    out = sys.argv[1]
    os.makedirs(out, exist_ok=True)
    rng = np.random.default_rng(7)
    data = rng.integers(0, 255, size=(1, H, W), dtype=np.uint8)
    lines = []
    for name, epsg, gt, geojp2, gml, gml2 in CASES:
        path = os.path.join(out, name + ".jp2")
        opts = {
            "GeoJP2": "YES" if geojp2 else "NO",
            "GMLJP2": "YES" if gml else "NO",
            "REVERSIBLE": "YES",
            "QUALITY": "100",
        }
        if gml2:
            opts["GMLJP2V2_DEF"] = "YES"
        t = Affine.from_gdal(*gt)
        with rasterio.open(
            path, "w", driver="JP2OpenJPEG", width=W, height=H, count=1, dtype="uint8",
            crs=f"EPSG:{epsg}", transform=t, **opts,
        ) as ds:
            ds.write(data)
        for side in (path + ".aux.xml",):
            if os.path.exists(side):
                os.remove(side)
        with rasterio.open(path) as ds:
            g = ds.transform.to_gdal()
            code = ds.crs.to_epsg() if ds.crs else 0
        lines.append(" ".join([name, str(code or 0)] + [repr(float(v)) for v in g]))
    with open(os.path.join(out, "oracle.txt"), "w") as f:
        f.write("\n".join(lines) + "\n")


if __name__ == "__main__":
    main()
