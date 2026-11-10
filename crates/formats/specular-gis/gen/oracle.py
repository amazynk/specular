"""The geospatial oracle: what GDAL (through pyogrio and rasterio) and PROJ (pyproj) read
from the corpus, written to OUT/expected/ for the specular-gis corpus tests.

Usage: python -I oracle.py CORPUS_DIR

  expected/vectors.jsonl   one line per (file, layer): feature count, field names, and for
                           the first 200 features the fid, attribute values and coordinates
                           (rings/parts as lists of [x, y]), and the layer's vertex count
  expected/rasters.jsonl   one line per GeoTIFF rasterio opens with a CRS or transform: EPSG
                           code, affine transform, nodata, size, band count, overview
                           factors, and pyproj's lon/lat of the centre pixel on the CRS's own datum

Reads the corpus as data only; nothing from it is executed.
"""

import json
import math
import os
import sys

import numpy as np
import pyogrio
import pyogrio.raw
import rasterio
import shapely
from pyproj import CRS, Transformer

VECTOR_EXT = {".shp", ".gpkg", ".kml", ".kmz", ".gpx", ".geojson", ".json"}
FEATURES = 200


def jsonable(v):
    if v is None:
        return None
    if isinstance(v, (np.floating, float)):
        f = float(v)
        return None if math.isnan(f) else f
    if isinstance(v, (np.integer, int)) and not isinstance(v, bool):
        return int(v)
    if isinstance(v, (np.bool_, bool)):
        return bool(v)
    if isinstance(v, bytes):
        return {"blob": len(v)}
    return str(v)


def parts(g):
    """A geometry as a list of coordinate lists: points one each, line parts, rings."""
    if g is None or g.is_empty:
        return []
    t = g.geom_type
    if t == "Point":
        return [[[g.x, g.y]]]
    if t == "LineString" or t == "LinearRing":
        return [[list(c[:2]) for c in g.coords]]
    if t == "Polygon":
        return [[list(c[:2]) for c in r.coords] for r in [g.exterior, *g.interiors]]
    out = []
    for sub in getattr(g, "geoms", []):
        out.extend(parts(sub))
    return out


def vectors(root, out):
    n = 0
    for dirpath, _, files in os.walk(root):
        if "/expected" in dirpath:
            continue
        for name in sorted(files):
            ext = os.path.splitext(name)[1].lower()
            if ext not in VECTOR_EXT:
                continue
            path = os.path.join(dirpath, name)
            rel = os.path.relpath(path, root)
            try:
                layers = pyogrio.list_layers(path)
            except Exception as e:  # noqa: BLE001 - GDAL refuses the file
                out.write(json.dumps({"file": rel, "error": str(e)[:200]}) + "\n")
                continue
            for lname, _ in layers:
                try:
                    meta, fids, geoms, fields = pyogrio.raw.read(
                        path, layer=str(lname), return_fids=True, datetime_as_string=True
                    )
                except Exception as e:  # noqa: BLE001
                    out.write(json.dumps({"file": rel, "layer": str(lname), "error": str(e)[:200]}) + "\n")
                    continue
                if geoms is None:
                    shapes = [None] * len(fids)
                else:
                    # GDAL hands over some curve and broken geometries GEOS cannot parse;
                    # those features are compared on attributes only.
                    shapes = shapely.from_wkb(geoms, on_invalid="ignore")
                vertices = 0
                feats = []
                for i in range(len(fids)):
                    p = parts(shapes[i])
                    vertices += sum(len(x) for x in p)
                    # None: GEOS could not parse what GDAL gave (TINs, broken rings), or
                    # too big to list; the feature is compared on attributes only.
                    unparsed = geoms is not None and geoms[i] is not None and shapes[i] is None
                    if i < FEATURES:
                        feats.append(
                            {
                                "fid": int(fids[i]),
                                "attrs": [jsonable(col[i]) for col in fields],
                                "parts": None if unparsed or sum(len(x) for x in p) > 5000 else p,
                            }
                        )
                crs = meta.get("crs")
                epsg = None
                if crs:
                    try:
                        epsg = CRS.from_user_input(crs).to_epsg()
                    except Exception:  # noqa: BLE001
                        epsg = None
                rec = {
                    "file": rel,
                    "layer": str(lname),
                    "count": len(fids),
                    "fields": [str(f) for f in meta["fields"]],
                    "epsg": epsg,
                    "vertices": vertices,
                    "features": feats,
                }
                out.write(json.dumps(rec) + "\n")
                n += 1
    return n


def rasters(root, out):
    n = 0
    for dirpath, _, files in os.walk(root):
        for name in sorted(files):
            if os.path.splitext(name)[1].lower() not in {".tif", ".tiff"}:
                continue
            path = os.path.join(dirpath, name)
            rel = os.path.relpath(path, root)
            try:
                with rasterio.open(path) as ds:
                    if ds.crs is None and ds.transform.is_identity:
                        continue
                    epsg = ds.crs.to_epsg() if ds.crs else None
                    t = ds.transform
                    rec = {
                        "file": rel,
                        "epsg": epsg,
                        "transform": [t.a, t.b, t.c, t.d, t.e, t.f],
                        "nodata": jsonable(ds.nodata),
                        "width": ds.width,
                        "height": ds.height,
                        "bands": ds.count,
                        "overviews": ds.overviews(1),
                    }
                    if ds.crs is not None:
                        cx, cy = t * (ds.width / 2.0, ds.height / 2.0)
                        rec["centre"] = [cx, cy]
                        try:
                            # Longitude/latitude on the CRS's own datum (no datum shift).
                            crs = CRS.from_wkt(ds.crs.to_wkt())
                            tr = Transformer.from_crs(crs, crs.geodetic_crs, always_xy=True)
                            lon, lat = tr.transform(cx, cy)
                            if math.isfinite(lon) and math.isfinite(lat):
                                rec["lonlat"] = [lon, lat]
                        except Exception:  # noqa: BLE001
                            pass
                    out.write(json.dumps(rec) + "\n")
                    n += 1
            except Exception:  # noqa: BLE001 - not a raster rasterio reads
                continue
    return n


def main():
    root = sys.argv[1]
    os.makedirs(os.path.join(root, "expected"), exist_ok=True)
    with open(os.path.join(root, "expected", "vectors.jsonl"), "w") as out:
        nv = vectors(root, out)
    with open(os.path.join(root, "expected", "rasters.jsonl"), "w") as out:
        nr = rasters(root, out)
    print(f"  oracle: {nv} vector layers, {nr} GeoTIFFs")


if __name__ == "__main__":
    main()
