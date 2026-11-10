"""The oracle for the sidecar and gridded-raster corpus: what GDAL (through rasterio) and
PROJ (through pyproj) read from every candidate file under a corpus folder.

Usage: python -I expect_geosidecar_corpus.py CORPUS_DIR [--convert]

--convert first writes GDAL conversions the corpus needs (an EnMAP scene as ENVI BSQ and
BIL with wavelengths, from the GeoTIFF the fetch script downloads), then the oracle runs.

Writes CORPUS_DIR/expected/index.json: one entry per file in the schema of
make_geosidecar_fixtures.py (`file` relative to CORPUS_DIR; `values` relative to
`expected/`, little-endian float32 of every band, nodata as NaN, written when the file has
at most 16M samples), plus `error` for files GDAL cannot open.

Candidates: Esri ASCII grids, DTED, ERDAS IMAGINE, ENVI/ESRI raw data beside a `.hdr`,
and PNG/JPEG/TIFF/GIF/BMP pictures beside a world file. The files are data, never run.
"""

import json
import math
import os
import sys
import warnings

import numpy as np
import rasterio
from pyproj import CRS, Transformer

warnings.filterwarnings("ignore")

ROOT = os.path.abspath(sys.argv[1])
EXP = os.path.join(ROOT, "expected")
MAX_SAMPLES = 16 << 20

WORLD = {".png": ".pgw", ".jpg": ".jgw", ".jpeg": ".jgw", ".tif": ".tfw", ".tiff": ".tfw",
         ".gif": ".gfw", ".bmp": ".bpw"}
RAW = {".bil", ".bsq", ".bip", ".dat", ".raw", ".img", ".bin", ""}


def convert():
    src = os.path.join(ROOT, "enmap", "enmap_potsdam.tif")
    if not os.path.exists(src):
        return
    with rasterio.open(src) as ds:
        profile = ds.profile
        data = ds.read()
        tags = [ds.tags(b) for b in range(1, ds.count + 1)]
        descs = ds.descriptions
    for il in ("bsq", "bil"):
        # One stem each: the two would otherwise share one `.hdr`.
        out = os.path.join(ROOT, "enmap", f"enmap_potsdam_{il}.{il}")
        p = dict(profile, driver="ENVI", interleave=il.upper())
        for k in ("blockxsize", "blockysize", "tiled", "compress", "photometric"):
            p.pop(k, None)
        with rasterio.open(out, "w", **p) as ds:
            ds.write(data)
            for b, t in enumerate(tags, start=1):
                ds.update_tags(b, **t)
                if descs[b - 1]:
                    ds.set_band_description(b, descs[b - 1])
        aux = out + ".aux.xml"
        if os.path.exists(aux):
            os.remove(aux)
        # GDAL's ENVI writer keeps the band names but not the wavelengths: add them (and a
        # description that is not this machine's path) to the header the way ENVI does.
        hdr = os.path.join(ROOT, "enmap", f"enmap_potsdam_{il}.hdr")
        with open(hdr) as f:
            text = f.read()
        head, _, rest = text.partition("description = {")
        _, _, rest = rest.partition("}")
        text = head + "description = {EnMAP-Box Potsdam scene}" + rest
        wl = [t.get("wavelength") for t in tags]
        if all(wl):
            unit = tags[0].get("wavelength_units", "Nanometers")
            text = text.rstrip("\n") + "\nwavelength units = " + unit
            text += "\nwavelength = {" + ", ".join(wl) + "}\n"
            fwhm = [t.get("fwhm") for t in tags]
            if all(fwhm):
                text += "fwhm = {" + ", ".join(fwhm) + "}\n"
        with open(hdr, "w") as f:
            f.write(text)


def siblings(path):
    d = os.path.dirname(path)
    stem = os.path.splitext(os.path.basename(path))[0].lower()
    return {f.lower(): f for f in os.listdir(d) if f.lower().startswith(stem + ".")}, stem


def candidate(path):
    ext = os.path.splitext(path)[1].lower()
    if ext in (".asc", ".dt0", ".dt1", ".dt2"):
        return True
    names, stem = siblings(path)
    if ext in WORLD:
        if path.lower().endswith(".aux.xml"):
            return False
        w = WORLD[ext]
        return any(stem + x in names for x in (w, ext + "w", ".wld"))
    if ext == ".img":
        with open(path, "rb") as f:
            if f.read(16).startswith(b"EHFA_HEADER_TAG"):
                return True
    if ext in RAW and ext != ".hdr":
        hdr = names.get(stem + ".hdr") or names.get(os.path.basename(path).lower() + ".hdr")
        if hdr:
            with open(os.path.join(os.path.dirname(path), hdr), "rb") as f:
                head = f.read(4096).lower()
            return head.startswith(b"envi") or b"nrows" in head or b"ncols" in head
    return False


def epsg_of(crs):
    if crs is None:
        return None
    try:
        c = CRS.from_user_input(crs.to_wkt())
        e = c.to_epsg(min_confidence=70)
        if e is None and c.to_authority() == ("OGC", "CRS84"):
            e = 4326
        return e
    except Exception:
        return None


def centre_lonlat(crs, t, w, h):
    if crs is None:
        return None
    try:
        c = CRS.from_user_input(crs.to_wkt())
        x, y = t * (w / 2.0, h / 2.0)
        if c.is_geographic:
            return [x, y]
        lon, lat = Transformer.from_crs(c, c.geodetic_crs, always_xy=True).transform(x, y)
        return [lon, lat] if math.isfinite(lon) and math.isfinite(lat) else None
    except Exception:
        return None


def record(rel):
    path = os.path.join(ROOT, rel)
    e = {"file": rel}
    try:
        ds = rasterio.open(path)
    except Exception as err:
        e["error"] = str(err)[:200]
        return e
    with ds:
        t = ds.transform
        e.update({
            "driver": ds.driver,
            "width": ds.width,
            "height": ds.height,
            "bands": ds.count,
            "dtype": ds.dtypes[0],
            "transform": [t.a, t.b, t.c, t.d, t.e, t.f],
            "georeferenced": not t.is_identity,
            "epsg": epsg_of(ds.crs),
            "nodata": None if ds.nodata is None else ("nan" if math.isnan(ds.nodata) else ds.nodata),
            "centre_lonlat": centre_lonlat(ds.crs, t, ds.width, ds.height),
        })
        tags = ds.tags(1)
        if "wavelength" in tags:
            try:
                e["wavelengths"] = [float(ds.tags(b)["wavelength"]) for b in range(1, ds.count + 1)]
                e["wavelength_units"] = tags.get("wavelength_units")
            except (KeyError, ValueError):
                pass
        if ds.count * ds.width * ds.height <= MAX_SAMPLES and ds.dtypes[0] not in ("complex64", "complex128", "complex_int16"):
            try:
                arr = ds.read(masked=True).astype(np.float32)
                vname = rel.replace("/", "__") + ".f32"
                arr.filled(np.nan).astype("<f4").tofile(os.path.join(EXP, vname))
                e["values"] = vname
            except Exception as err:
                e["values_error"] = str(err)[:200]
    return e


def main():
    if "--convert" in sys.argv:
        convert()
    os.makedirs(EXP, exist_ok=True)
    for f in os.listdir(EXP):
        os.remove(os.path.join(EXP, f))
    entries = []
    for d, dirs, files in os.walk(ROOT):
        dirs[:] = sorted(x for x in dirs if x != "expected" and not x.startswith("."))
        for f in sorted(files):
            p = os.path.join(d, f)
            if candidate(p):
                entries.append(record(os.path.relpath(p, ROOT)))
    with open(os.path.join(EXP, "index.json"), "w") as f:
        json.dump(entries, f, indent=1)
    print(f"{len(entries)} files, {sum('error' in e for e in entries)} GDAL cannot open")


main()
