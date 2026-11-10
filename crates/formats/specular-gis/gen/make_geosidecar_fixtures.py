"""Synthetic fixtures for sidecar georeferencing and the gridded raster readers, written by
GDAL (through rasterio), with what GDAL and PROJ (pyproj) read back from them.

Usage: python -I make_geosidecar_fixtures.py OUT_DIR

Writes into OUT_DIR:
  world files    scene.png + .pgw + .prj (ESRI WKT), photo.jpg + .JGW + legacy .prj,
                 dem.tif (no GeoKeys) + .tfw + .prj
  .aux.xml       aux.png + aux.png.aux.xml (GeoTransform, SRS, nodata, scale/offset, unit,
                 description, statistics)
  grids          grid.asc (+ .prj), centre.asc (xllcenter), tile.dt0 (DTED level 0)
  ENVI           bsq.img + bsq.hdr (UTM map info, wavelengths, ignore value),
                 rotated.dat + rotated.hdr (rotated map info, BIL, float),
                 bip.raw + bip.hdr (geographic, BIP, big-endian)
  ESRI .hdr      elev.bil + elev.hdr + elev.prj
  IMAGINE        albers.img (float, uncompressed), rle.img (run-length int16, UTM),
                 rgb.img (run-length bytes, 3 bands, geographic), lcc.img (uint16)
  expected.json  one entry per file: size, bands, dtype, transform (GDAL order a b c d e f),
                 EPSG, nodata, wavelengths, the centre pixel's lon/lat (pyproj, on the CRS's
                 own datum), and `values`: a little-endian float32 file of every band, nodata
                 as NaN
Every file is well under 200 KB.
"""

import json
import math
import os
import sys
import warnings

import numpy as np
import rasterio
from pyproj import CRS, Transformer
from rasterio.transform import Affine

warnings.filterwarnings("ignore")

OUT = sys.argv[1]
os.makedirs(OUT, exist_ok=True)
for f in os.listdir(OUT):
    p = os.path.join(OUT, f)
    if os.path.isfile(p):
        os.remove(p)

rng = np.random.default_rng(7)
expected = []


def terrain(h, w, base=200.0, amp=900.0):
    y, x = np.mgrid[0:h, 0:w].astype(np.float64)
    z = base + amp * (np.sin(x / 9.0) * np.cos(y / 13.0) + 0.3 * np.sin((x + y) / 5.0))
    return z


def drop_aux(name):
    p = os.path.join(OUT, name + ".aux.xml")
    if os.path.exists(p):
        os.remove(p)


def epsg_of(crs):
    if crs is None:
        return None
    try:
        c = CRS.from_user_input(crs.to_wkt() if hasattr(crs, "to_wkt") else crs)
        e = c.to_epsg(min_confidence=70)
        if e is None and c.to_authority() == ("OGC", "CRS84"):
            # Longitude/latitude WGS 84, as GDAL reads an ESRI GCS_WGS_1984.
            e = 4326
        return e
    except Exception:
        return None


def centre_lonlat(crs, t, w, h):
    if crs is None:
        return None
    c = CRS.from_user_input(crs.to_wkt() if hasattr(crs, "to_wkt") else crs)
    x, y = t * (w / 2.0, h / 2.0)
    if c.is_geographic:
        return [x, y]
    tr = Transformer.from_crs(c, c.geodetic_crs, always_xy=True)
    lon, lat = tr.transform(x, y)
    return [lon, lat]


def record(name, read_path=None, values=True, extra=None, crs_from=None):
    """Reads `name` back through GDAL and records what it says."""
    path = os.path.join(OUT, read_path or name)
    with rasterio.open(path) as ds:
        t = ds.transform
        crs = ds.crs
        if crs_from is not None:
            crs = rasterio.crs.CRS.from_wkt(CRS.from_user_input(crs_from).to_wkt())
        e = {
            "file": name,
            "driver": ds.driver,
            "width": ds.width,
            "height": ds.height,
            "bands": ds.count,
            "dtype": ds.dtypes[0],
            "transform": [t.a, t.b, t.c, t.d, t.e, t.f],
            "epsg": epsg_of(crs),
            "nodata": None
            if ds.nodata is None
            else ("nan" if math.isnan(ds.nodata) else ds.nodata),
            "centre_lonlat": centre_lonlat(crs, t, ds.width, ds.height),
        }
        tags = ds.tags(1)
        if "wavelength" in tags:
            e["wavelengths"] = [float(ds.tags(b)["wavelength"]) for b in range(1, ds.count + 1)]
            e["wavelength_units"] = tags.get("wavelength_units")
        if values:
            arr = ds.read(masked=True).astype(np.float32)
            data = arr.filled(np.nan).astype("<f4")
            vname = name.replace(".", "_") + ".f32"
            data.tofile(os.path.join(OUT, vname))
            e["values"] = vname
    if extra:
        e.update(extra)
    expected.append(e)


# ---- world files -------------------------------------------------------------------

utm33 = CRS.from_epsg(32633)
h, w = 48, 64
rgb = (rng.random((3, h, w)) * 255).astype(np.uint8)
with rasterio.open(os.path.join(OUT, "scene.png"), "w", driver="PNG", width=w, height=h,
                   count=3, dtype="uint8") as ds:
    ds.write(rgb)
drop_aux("scene.png")
with open(os.path.join(OUT, "scene.pgw"), "w") as f:
    # A, D, B, E, C, F; C/F the centre of the top-left pixel.
    f.write("2.5\n0.0\n0.0\n-2.5\n455001.25\n5300998.75\n")
with open(os.path.join(OUT, "scene.prj"), "w") as f:
    f.write(utm33.to_wkt("WKT1_ESRI"))
record("scene.png", values=False, crs_from=utm33)

with rasterio.open(os.path.join(OUT, "photo.jpg"), "w", driver="JPEG", width=w, height=h,
                   count=3, dtype="uint8") as ds:
    ds.write(rgb)
drop_aux("photo.jpg")
with open(os.path.join(OUT, "photo.JGW"), "w") as f:
    # Rotated by a few degrees.
    a, b, d, e = 0.98, 0.17, 0.17, -0.98
    f.write(f"{a}\n{d}\n{b}\n{e}\n500000.5\n4649999.5\n")
with open(os.path.join(OUT, "photo.prj"), "w") as f:
    f.write("Projection    UTM\nZone          33\nDatum         WGS84\nUnits         METERS\n"
            "Spheroid      WGS84\nXshift        0.0\nYshift        0.0\nParameters\n")
record("photo.jpg", values=False, crs_from=utm33)

z = terrain(40, 50).astype(np.int16)
z[5:8, 10:14] = -32768
with rasterio.open(os.path.join(OUT, "dem.tif"), "w", driver="GTiff", width=50, height=40,
                   count=1, dtype="int16", nodata=-32768) as ds:
    ds.write(z, 1)
drop_aux("dem.tif")
with open(os.path.join(OUT, "dem.tfw"), "w") as f:
    f.write("30\n0\n0\n-30\n600015\n5000985\n")
with open(os.path.join(OUT, "dem.prj"), "w") as f:
    f.write(CRS.from_epsg(32632).to_wkt("WKT1_GDAL"))
record("dem.tif", crs_from=CRS.from_epsg(32632))

# ---- .aux.xml ------------------------------------------------------------------------

z = terrain(30, 40).astype(np.uint16)
z[0, 0:5] = 0
t = Affine(0.001, 0, 11.5, 0, -0.001, 48.2)
with rasterio.open(os.path.join(OUT, "aux.png"), "w", driver="PNG", width=40, height=30,
                   count=1, dtype="uint16", crs="EPSG:4326", transform=t) as ds:
    ds.write(z, 1)
    ds.set_band_description(1, "elevation")
    ds.update_tags(1, STATISTICS_MINIMUM=str(int(z[z > 0].min())),
                   STATISTICS_MAXIMUM=str(int(z.max())))
    ds.scales = (0.5,)
    ds.offsets = (-10.0,)
    ds.units = ("metre",)
# Nodata in the .aux.xml itself (the PNG driver would put it in a tRNS chunk instead).
aux = os.path.join(OUT, "aux.png.aux.xml")
with open(aux) as f:
    text = f.read()
text = text.replace("<PAMRasterBand band=\"1\">",
                    "<PAMRasterBand band=\"1\">\n    <NoDataValue>0</NoDataValue>", 1)
with open(aux, "w") as f:
    f.write(text)
record("aux.png", extra={"scale": 0.5, "offset": -10.0, "unit": "metre",
                         "description": "elevation"})

# ---- Esri ASCII grids ----------------------------------------------------------------

z = terrain(25, 30).astype(np.float32) / 3.0
z[3, 4] = -9999
with rasterio.open(os.path.join(OUT, "grid.asc"), "w", driver="AAIGrid", width=30, height=25,
                   count=1, dtype="float32", nodata=-9999, crs="EPSG:32631",
                   transform=Affine(25, 0, 400000, 0, -25, 5500625)) as ds:
    ds.write(z, 1)
drop_aux("grid.asc")
record("grid.asc")

with open(os.path.join(OUT, "centre.asc"), "w") as f:
    f.write("NCOLS 4\nNROWS 3\nXLLCENTER -10.0\nYLLCENTER 40.0\nCELLSIZE 0.5\n"
            "NODATA_VALUE -1\n1 2 3 4\n5 -1 7 8\n9 10 11 12\n")
record("centre.asc")

# ---- DTED level 0 ----------------------------------------------------------------------

z = terrain(121, 121, base=100, amp=400).astype(np.int16)
z[60:63, 60:63] = -32767
z[10, 10] = -45
with rasterio.open(os.path.join(OUT, "tile_mem.tif"), "w", driver="GTiff", width=121,
                   height=121, count=1, dtype="int16", crs="EPSG:4326", nodata=-32767,
                   transform=Affine(1 / 120, 0, 6 - 0.5 / 120, 0, -1 / 120,
                                    46 + 0.5 / 120)) as ds:
    ds.write(z, 1)
with rasterio.open(os.path.join(OUT, "tile_mem.tif")) as src:
    profile = src.profile
    profile.update(driver="DTED")
    profile.pop("tiled", None)
    profile.pop("blockxsize", None)
    profile.pop("blockysize", None)
    profile.pop("compress", None)
    profile.pop("interleave", None)
    with rasterio.open(os.path.join(OUT, "tile.dt0"), "w", **profile) as dst:
        dst.write(src.read())
os.remove(os.path.join(OUT, "tile_mem.tif"))
drop_aux("tile.dt0")
record("tile.dt0")

# ---- ENVI ----------------------------------------------------------------------------


def append_hdr(path, lines):
    with open(path, "a") as f:
        f.write("".join(l + "\n" for l in lines))


n = 5
cube = np.stack([terrain(20, 24, base=500 + 100 * b, amp=300) for b in range(n)]).astype(np.uint16)
cube[:, 2, 3] = 0
with rasterio.open(os.path.join(OUT, "bsq.img"), "w", driver="ENVI", width=24, height=20,
                   count=n, dtype="uint16", nodata=0, crs="EPSG:32633",
                   transform=Affine(30, 0, 455000, 0, -30, 5301000),
                   INTERLEAVE="BSQ") as ds:
    ds.write(cube)
drop_aux("bsq.img")
append_hdr(os.path.join(OUT, "bsq.hdr"), [
    "wavelength units = Nanometers",
    "wavelength = {",
    " 450.5, 550.0, 650.25,",
    " 860.0, 1610.0}",
    "fwhm = {20, 20, 20, 30, 80}",
])
record("bsq.img")

f = (terrain(16, 18) / 100.0).astype(np.float32)
rot = Affine.translation(700000, 4500000) * Affine.rotation(-20) * Affine.scale(10, -10)
with rasterio.open(os.path.join(OUT, "rotated.dat"), "w", driver="ENVI", width=18, height=16,
                   count=2, dtype="float32", crs="EPSG:32617", transform=rot,
                   INTERLEAVE="BIL") as ds:
    ds.write(np.stack([f, -f]))
drop_aux("rotated.dat")
append_hdr(os.path.join(OUT, "rotated.hdr"), [
    "wavelength units = Micrometers",
    "wavelength = {1.25, 2.2}",
])
record("rotated.dat")

g = terrain(12, 14).astype(np.int16)
with rasterio.open(os.path.join(OUT, "bip.raw"), "w", driver="ENVI", width=14, height=12,
                   count=3, dtype="int16", crs="EPSG:4326",
                   transform=Affine(0.01, 0, 2.0, 0, -0.01, 41.0), INTERLEAVE="BIP") as ds:
    ds.write(np.stack([g, g // 2, g // 3]))
drop_aux("bip.raw")
# Rewrite as big-endian with "byte order = 1".
hdr = os.path.join(OUT, "bip.hdr")
raw = np.fromfile(os.path.join(OUT, "bip.raw"), dtype="<i2")
raw.astype(">i2").tofile(os.path.join(OUT, "bip.raw"))
text = open(hdr).read()
text = text.replace("byte order = 0", "byte order = 1")
open(hdr, "w").write(text)
record("bip.raw")

# ---- ESRI .hdr labelled BIL ------------------------------------------------------------

e = terrain(22, 26).astype(np.int16)
e[0, :3] = -9999
with rasterio.open(os.path.join(OUT, "elev.bil"), "w", driver="EHdr", width=26, height=22,
                   count=2, dtype="int16", nodata=-9999, crs="EPSG:4326",
                   transform=Affine(1 / 1200, 0, -120.0, 0, -1 / 1200, 38.0)) as ds:
    ds.write(np.stack([e, e + 5]))
drop_aux("elev.bil")
for extra in ("elev.stx",):
    p = os.path.join(OUT, extra)
    if os.path.exists(p):
        os.remove(p)
record("elev.bil")

# ---- ERDAS IMAGINE -------------------------------------------------------------------

a = terrain(70, 90).astype(np.float32)
a[10:12, 20:30] = -1e30
with rasterio.open(os.path.join(OUT, "albers.img"), "w", driver="HFA", width=90, height=70,
                   count=1, dtype="float32", nodata=-1e30, crs="EPSG:5070",
                   transform=Affine(30, 0, 1000000, 0, -30, 2000000)) as ds:
    ds.write(a, 1)
drop_aux("albers.img")
record("albers.img")

r = (np.round(terrain(100, 130) / 50.0) * 50).astype(np.int16)
r[40:60, :] = 7
r[0:3, 0:3] = -9999
with rasterio.open(os.path.join(OUT, "rle.img"), "w", driver="HFA", width=130, height=100,
                   count=1, dtype="int16", nodata=-9999, crs="EPSG:32633",
                   transform=Affine(10, 0, 500000, 0, -10, 4000000),
                   COMPRESSED="YES") as ds:
    ds.write(r, 1)
drop_aux("rle.img")
record("rle.img")

c = np.zeros((3, 80, 70), np.uint8)
c[0] = 200
c[1, :, :35] = 17
c[2] = (rng.random((80, 70)) * 4).astype(np.uint8) * 60
with rasterio.open(os.path.join(OUT, "rgb.img"), "w", driver="HFA", width=70, height=80,
                   count=3, dtype="uint8", crs="EPSG:4326",
                   transform=Affine(0.0005, 0, 13.0, 0, -0.0005, 52.5),
                   COMPRESSED="YES") as ds:
    ds.write(c)
drop_aux("rgb.img")
record("rgb.img")

u = terrain(66, 66).astype(np.uint16)
with rasterio.open(os.path.join(OUT, "lcc.img"), "w", driver="HFA", width=66, height=66,
                   count=1, dtype="uint16", crs="EPSG:2154",
                   transform=Affine(50, 0, 650000, 0, -50, 6865000)) as ds:
    ds.write(u, 1)
drop_aux("lcc.img")
record("lcc.img")

# ------------------------------------------------------------------------------------------

for name in sorted(os.listdir(OUT)):
    size = os.path.getsize(os.path.join(OUT, name))
    assert size < 200_000, f"{name} is {size} bytes"

with open(os.path.join(OUT, "expected.json"), "w") as f:
    json.dump(expected, f, indent=1)
