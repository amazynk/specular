#!/usr/bin/env python3
"""Oracle data for specular-geodesy's terrain tests: `gdaldem slope`, `gdaldem aspect` and
`gdal_contour` from GDAL itself (the libgdal that rasterio wheels bundle, called through
ctypes: GDALDEMProcessing and GDALContourGenerateEx, the functions behind the tools).

Run from a venv with `pip install rasterio numpy`:
    python gen/terrain_oracle.py tests/data
Writes small files the Rust tests read; nothing at test time needs Python or GDAL:
  terrain-<name>.f32       the DEM, little-endian float32, row-major, NaN no data
  terrain-<name>.tsv       w h ewres nsres, then per cell: x y slope aspect (GDAL's)
  contours-<name>.tsv      level, then the vertices of each line (pixel coordinates)
  ground-steps.tsv         per CRS and pixel: the ground metres to the next pixel east and
                           south (PROJ through pyproj, then the WGS 84 geodesic), for the
                           map view's slope and export scale
"""

import ctypes
import glob
import json
import math
import os
import sys
import tempfile
from pathlib import Path

import numpy as np
import rasterio
from rasterio.transform import Affine

OUT = Path(sys.argv[1] if len(sys.argv) > 1 else "tests/data")
OUT.mkdir(parents=True, exist_ok=True)
NODATA = -9999.0

libs = glob.glob(os.path.join(os.path.dirname(rasterio.__file__), "..", "rasterio.libs", "libgdal-*.so*"))
gdal = ctypes.CDLL(libs[0])
gdal.GDALAllRegister()
gdal.GDALVersionInfo.restype = ctypes.c_char_p
VERSION = gdal.GDALVersionInfo(b"RELEASE_NAME").decode()
for name, res in [
    ("GDALOpen", ctypes.c_void_p),
    ("GDALDEMProcessingOptionsNew", ctypes.c_void_p),
    ("GDALDEMProcessing", ctypes.c_void_p),
    ("GDALGetRasterBand", ctypes.c_void_p),
    ("GDALGetDriverByName", ctypes.c_void_p),
    ("GDALCreate", ctypes.c_void_p),
    ("GDALDatasetCreateLayer", ctypes.c_void_p),
    ("OGR_Fld_Create", ctypes.c_void_p),
]:
    getattr(gdal, name).restype = res
gdal.GDALOpen.argtypes = [ctypes.c_char_p, ctypes.c_int]
gdal.GDALDEMProcessingOptionsNew.argtypes = [ctypes.POINTER(ctypes.c_char_p), ctypes.c_void_p]
gdal.GDALDEMProcessing.argtypes = [
    ctypes.c_char_p, ctypes.c_void_p, ctypes.c_char_p, ctypes.c_char_p, ctypes.c_void_p,
    ctypes.POINTER(ctypes.c_int),
]
gdal.GDALClose.argtypes = [ctypes.c_void_p]
gdal.GDALGetRasterBand.argtypes = [ctypes.c_void_p, ctypes.c_int]
gdal.GDALGetDriverByName.argtypes = [ctypes.c_char_p]
gdal.GDALCreate.argtypes = [
    ctypes.c_void_p, ctypes.c_char_p, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_int,
    ctypes.c_void_p,
]
gdal.GDALDatasetCreateLayer.argtypes = [
    ctypes.c_void_p, ctypes.c_char_p, ctypes.c_void_p, ctypes.c_int, ctypes.c_void_p,
]
gdal.OGR_Fld_Create.argtypes = [ctypes.c_char_p, ctypes.c_int]
gdal.OGR_L_CreateField.argtypes = [ctypes.c_void_p, ctypes.c_void_p, ctypes.c_int]
gdal.GDALContourGenerateEx.argtypes = [
    ctypes.c_void_p, ctypes.c_void_p, ctypes.POINTER(ctypes.c_char_p), ctypes.c_void_p,
    ctypes.c_void_p,
]


def argv(items):
    a = (ctypes.c_char_p * (len(items) + 1))()
    for i, s in enumerate(items):
        a[i] = s.encode()
    a[len(items)] = None
    return a


def write_tif(path, dem, transform):
    with rasterio.open(
        path, "w", driver="GTiff", width=dem.shape[1], height=dem.shape[0], count=1,
        dtype="float32", transform=transform, nodata=NODATA, crs="EPSG:32633",
    ) as d:
        d.write(np.where(np.isnan(dem), NODATA, dem).astype(np.float32), 1)


def dem_processing(src_path, mode, extra):
    src = gdal.GDALOpen(src_path.encode(), 0)
    out = src_path.replace(".tif", f"-{mode}.tif")
    opts = gdal.GDALDEMProcessingOptionsNew(argv(["-compute_edges", "-alg", "Horn"] + extra), None)
    err = ctypes.c_int(0)
    dst = gdal.GDALDEMProcessing(out.encode(), src, mode.encode(), None, opts, ctypes.byref(err))
    assert dst and err.value == 0, (mode, err.value)
    gdal.GDALClose(dst)
    gdal.GDALClose(src)
    with rasterio.open(out) as d:
        return d.read(1), d.nodata


def contours(src_path, interval, base, tmp):
    src = gdal.GDALOpen(src_path.encode(), 0)
    band = gdal.GDALGetRasterBand(src, 1)
    drv = gdal.GDALGetDriverByName(b"GeoJSON")
    path = os.path.join(tmp, "c.geojson")
    ds = gdal.GDALCreate(drv, path.encode(), 0, 0, 0, 0, None)
    layer = gdal.GDALDatasetCreateLayer(ds, b"contour", None, 2, None)  # wkbLineString
    gdal.OGR_L_CreateField(layer, gdal.OGR_Fld_Create(b"ID", 0), 1)  # OFTInteger
    gdal.OGR_L_CreateField(layer, gdal.OGR_Fld_Create(b"ELEV", 2), 1)  # OFTReal
    opts = argv([
        f"LEVEL_INTERVAL={interval}", f"LEVEL_BASE={base}", "ID_FIELD=0", "ELEV_FIELD=1",
        f"NODATA={NODATA}",
    ])
    assert gdal.GDALContourGenerateEx(band, layer, opts, None, None) == 0
    gdal.GDALClose(ds)
    gdal.GDALClose(src)
    with open(path) as f:
        fc = json.load(f)
    lines = []
    for feat in fc["features"]:
        g = feat["geometry"]
        parts = [g["coordinates"]] if g["type"] == "LineString" else g["coordinates"]
        for p in parts:
            lines.append((feat["properties"]["ELEV"], p))
    return lines


def terrain(w, h, seed, hole):
    rng = np.random.default_rng(seed)
    y, x = np.mgrid[0:h, 0:w].astype(np.float64)
    z = 400.0 + 2.0 * x - 1.5 * y
    for _ in range(6):
        cx, cy = rng.uniform(0, w), rng.uniform(0, h)
        s = rng.uniform(4, 14)
        z += rng.uniform(-120, 220) * np.exp(-((x - cx) ** 2 + (y - cy) ** 2) / (2 * s * s))
    z += 15.0 * np.sin(x / 3.1) * np.cos(y / 4.7)
    # Some flat ground (aspect undefined there).
    z[2:6, w - 8:w - 3] = 500.0
    if hole:
        z[h // 2:h // 2 + 4, w // 3:w // 3 + 5] = np.nan
    return z.astype(np.float32)


def main():
    tmp = tempfile.mkdtemp(dir=os.environ.get("TMPDIR"))
    cases = [
        # name, w, h, ewres, nsres, seed, hole
        ("square", 64, 48, 30.0, -30.0, 1, True),
        ("oblong", 40, 56, 30.0, -20.0, 2, False),
    ]
    for name, w, h, ew, ns, seed, hole in cases:
        dem = terrain(w, h, seed, hole)
        (OUT / f"terrain-{name}.f32").write_bytes(dem.astype("<f4").tobytes())
        path = os.path.join(tmp, f"{name}.tif")
        write_tif(path, dem, Affine(ew, 0, 500000.0, 0, ns, 5000000.0))
        slope, snd = dem_processing(path, "slope", [])
        aspect, and_ = dem_processing(path, "aspect", [])
        with open(OUT / f"terrain-{name}.tsv", "w") as f:
            f.write(f"# GDAL {VERSION}: gdaldem slope/aspect -alg Horn -compute_edges\n")
            f.write(f"{w}\t{h}\t{ew!r}\t{ns!r}\n")
            for yy in range(h):
                for xx in range(w):
                    s = float(slope[yy, xx])
                    a = float(aspect[yy, xx])
                    s = "nan" if s == snd else repr(s)
                    a = "nan" if a == and_ else repr(a)
                    f.write(f"{xx}\t{yy}\t{s}\t{a}\n")
        # Contours on a grid with pixel coordinates (origin 0, 0; one unit per pixel, y down).
        cpath = os.path.join(tmp, f"{name}-px.tif")
        write_tif(cpath, dem, Affine(1.0, 0, 0.0, 0, 1.0, 0.0))
        lines = contours(cpath, 25.0, 0.0, tmp)
        with open(OUT / f"contours-{name}.tsv", "w") as f:
            f.write(f"# GDAL {VERSION}: gdal_contour -i 25 -off 0, pixel coordinates\n")
            for level, pts in lines:
                f.write(repr(float(level)) + "\t" + "\t".join(f"{p[0]!r},{p[1]!r}" for p in pts) + "\n")
        print(name, "lines", len(lines))


def ground_steps():
    from pyproj import Geod, Transformer
    geod = Geod(ellps="WGS84")
    rng = np.random.default_rng(7)
    rows = []
    # (EPSG, origin x, origin y, pixel x, pixel y)
    grids = [
        (32633, 500000.0, 5000000.0, 30.0, -30.0),
        (2056, 2600000.0, 1200000.0, 2.0, -2.0),
        (3857, 950000.0, 6000000.0, 10.0, -10.0),
        (4326, 8.0, 47.5, 1.0 / 3600.0, -1.0 / 3600.0),
        (32718, 300000.0, 8500000.0, 90.0, -90.0),
    ]
    for epsg, ox, oy, sx, sy in grids:
        t = Transformer.from_crs(epsg, 4326, always_xy=True)
        for _ in range(40):
            px, py = rng.uniform(0, 2000), rng.uniform(0, 2000)
            def ll(x, y):
                return t.transform(ox + sx * x, oy + sy * y)
            lon, lat = ll(px, py)
            lon_e, lat_e = ll(px + 1, py)
            lon_s, lat_s = ll(px, py + 1)
            east = geod.inv(lon, lat, lon_e, lat_e)[2]
            south = geod.inv(lon, lat, lon_s, lat_s)[2]
            sign = -1.0 if lat_s < lat else 1.0
            rows.append((epsg, ox, oy, sx, sy, px, py, east, sign * south))
    with open(OUT / "ground-steps.tsv", "w") as f:
        import pyproj
        f.write(f"# PROJ {pyproj.proj_version_str} via pyproj {pyproj.__version__}: epsg ox oy sx sy px py east north\n")
        for r in rows:
            f.write("\t".join(repr(float(v)) if i else str(v) for i, v in enumerate(r)) + "\n")


main()
ground_steps()
