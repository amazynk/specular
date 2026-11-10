"""The GIS formats oracle: what GDAL (pyogrio) reads from the FlatGeobuf, GML and
OpenStreetMap files of the gis-formats corpus, and what GeoPandas (pyarrow + shapely)
reads from its GeoParquet and Arrow files, written to OUT/expected/formats.jsonl in the
record shape of oracle.py (one line per file and layer).

Usage: python -I formats_oracle.py CORPUS_DIR

Reads the corpus as data only; nothing from it is executed. GML files are read from a
copy in SCRATCH (default: a gml-copy folder beside the corpus), where GDAL may write its
.gfs schema files, so the corpus stays unchanged.
"""

import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import geopandas  # noqa: E402
import pyarrow  # noqa: E402
import pyarrow.ipc  # noqa: E402
import pyarrow.parquet  # noqa: E402
import pyogrio  # noqa: E402
import pyogrio.raw  # noqa: E402
import shapely  # noqa: E402
from oracle import FEATURES, jsonable, parts  # noqa: E402
from pyproj import CRS  # noqa: E402

OGR_EXT = {".fgb", ".gml", ".xml", ".osm", ".pbf"}
ARROW_EXT = {".parquet", ".feather", ".arrow", ".arrows"}
SKIP_DIRS = ("/expected", "/gen/cog")


def epsg_of(crs):
    if not crs:
        return None
    try:
        return CRS.from_user_input(crs).to_epsg()
    except Exception:  # noqa: BLE001
        return None


def record(rel, layer, fids, shapes, fields, cols, epsg, raw=None):
    """raw: the encoded geometries; one GEOS cannot parse is compared on attributes only."""
    vertices = 0
    feats = []
    for i in range(len(shapes)):
        p = parts(shapes[i])
        unparsed = raw is not None and raw[i] is not None and shapes[i] is None
        vertices += sum(len(x) for x in p)
        if i < FEATURES:
            feats.append(
                {
                    "fid": int(fids[i]) if fids is not None else i,
                    "attrs": [jsonable(c[i]) for c in cols],
                    "parts": None if unparsed or sum(len(x) for x in p) > 5000 else p,
                }
            )
    return {
        "file": rel,
        "layer": layer,
        "count": len(shapes),
        "fields": fields,
        "epsg": epsg,
        "vertices": vertices,
        "features": feats,
    }


def ogr(path, rel, out):
    opts = {"WRITE_GFS": "NO"} if path.lower().endswith((".gml", ".xml")) else {}
    try:
        layers = pyogrio.list_layers(path)
    except Exception as e:  # noqa: BLE001
        out.write(json.dumps({"file": rel, "error": str(e)[:200]}) + "\n")
        return 0
    n = 0
    for lname, _ in layers:
        try:
            meta, fids, geoms, fields = pyogrio.raw.read(
                path, layer=str(lname), return_fids=True, datetime_as_string=True, **opts
            )
        except Exception as e:  # noqa: BLE001
            out.write(json.dumps({"file": rel, "layer": str(lname), "error": str(e)[:200]}) + "\n")
            continue
        shapes = [None] * len(fids) if geoms is None else shapely.from_wkb(geoms, on_invalid="ignore")
        rec = record(rel, str(lname), fids, shapes, [str(f) for f in meta["fields"]], fields, epsg_of(meta.get("crs")), geoms)
        out.write(json.dumps(rec) + "\n")
        n += 1
    return n


def arrow_table(path):
    if path.endswith(".parquet"):
        return pyarrow.parquet.read_table(path)
    with open(path, "rb") as f:
        head = f.read(6)
    if head == b"ARROW1":
        return pyarrow.ipc.open_file(path).read_all()
    return pyarrow.ipc.open_stream(path).read_all()


def geo_frame(path):
    if path.endswith(".parquet"):
        try:
            return geopandas.read_parquet(path)
        except Exception:  # noqa: BLE001 - no "geo" metadata: try GeoArrow types
            pass
    elif path.endswith(".feather"):
        try:
            return geopandas.read_feather(path)
        except Exception:  # noqa: BLE001
            pass
    return geopandas.GeoDataFrame.from_arrow(arrow_table(path))


def geo_meta_shapes(table):
    """Geometries and CRS from GeoParquet "geo" metadata with WKB or WKT encoding."""
    meta = (table.schema.metadata or {}).get(b"geo")
    if not meta or not meta.strip():
        return None
    g = json.loads(meta)
    col = g.get("primary_column")
    c = g.get("columns", {}).get(col, {})
    enc = c.get("encoding", "WKB").upper()
    if enc not in ("WKB", "WKT"):
        return None
    vals = table.column(col).to_pylist()
    shapes = shapely.from_wkb(vals, on_invalid="ignore") if enc == "WKB" else shapely.from_wkt(vals, on_invalid="ignore")
    crs = c.get("crs", "OGC:CRS84")
    epsg = epsg_of(json.dumps(crs) if isinstance(crs, dict) else crs)
    return col, list(shapes), epsg, vals


NATIVE = {
    "geoarrow.point": (shapely.GeometryType.POINT, 0),
    "geoarrow.linestring": (shapely.GeometryType.LINESTRING, 1),
    "geoarrow.multipoint": (shapely.GeometryType.MULTIPOINT, 1),
    "geoarrow.polygon": (shapely.GeometryType.POLYGON, 2),
    "geoarrow.multilinestring": (shapely.GeometryType.MULTILINESTRING, 2),
    "geoarrow.multipolygon": (shapely.GeometryType.MULTIPOLYGON, 3),
}


def native_shapes(table):
    """Geometries of a GeoArrow native column (extension metadata may be empty)."""
    import numpy as np

    for i, f in enumerate(table.schema):
        ext = (f.metadata or {}).get(b"ARROW:extension:name", b"").decode()
        if ext not in NATIVE:
            continue
        gtype, depth = NATIVE[ext]
        arr = table.column(i).combine_chunks()
        valid = np.asarray(arr.is_valid())
        offsets = []
        cur = arr
        for _ in range(depth):
            offsets.append(np.asarray(cur.offsets))
            cur = cur.flatten() if not hasattr(cur, "values") else cur.values
        if pyarrow.types.is_struct(cur.type):
            coords = np.column_stack([np.asarray(cur.field(0)), np.asarray(cur.field(1))])
        else:
            size = cur.type.list_size
            coords = np.asarray(cur.values).reshape(-1, size)[:, :2]
        shapes = shapely.from_ragged_array(gtype, np.ascontiguousarray(coords, dtype=float), tuple(reversed(offsets)) or None)
        shapes = [s if v else None for s, v in zip(shapes, valid)]
        meta = (f.metadata or {}).get(b"ARROW:extension:metadata", b"")
        crs = None
        try:
            crs = json.loads(meta).get("crs")
        except ValueError:  # empty, or the old binary (count-prefixed) metadata
            pass
        return f.name, shapes, epsg_of(json.dumps(crs) if isinstance(crs, dict) else crs), None
    return None


def arrow(path, rel, out):
    try:
        table = arrow_table(path)
        direct = geo_meta_shapes(table) or native_shapes(table)
        if direct is not None:
            geom_col, shapes, epsg, raw = direct
            names = [n for n in table.column_names if n != geom_col]
            cols = [table.column(n).to_pylist() for n in names]
        else:
            gdf = geo_frame(path)
            geom_col = gdf.geometry.name
            names = [c for c in gdf.columns if c != geom_col]
            cols = [gdf[c].tolist() for c in names]
            shapes = list(gdf.geometry.values)
            epsg = epsg_of(gdf.crs)
            raw = None
    except Exception as e:  # noqa: BLE001
        out.write(json.dumps({"file": rel, "error": str(e)[:200]}) + "\n")
        return 0
    rec = record(rel, os.path.splitext(os.path.basename(rel))[0], None, shapes, [str(n) for n in names], cols, epsg, raw)
    out.write(json.dumps(rec) + "\n")
    return 1


def main():
    import shutil
    import tempfile

    root = sys.argv[1]
    scratch = tempfile.mkdtemp(prefix="gml-copy-", dir=os.path.dirname(os.path.abspath(root)))
    gml_src = os.path.join(root, "gdal", "gml")
    gml_copy = os.path.join(scratch, "gml")
    if os.path.isdir(gml_src):
        shutil.copytree(gml_src, gml_copy)
    os.makedirs(os.path.join(root, "expected"), exist_ok=True)
    n = 0
    with open(os.path.join(root, "expected", "formats.jsonl"), "w") as out:
        for dirpath, _, files in os.walk(root):
            if any(s in dirpath for s in SKIP_DIRS):
                continue
            for name in sorted(files):
                ext = os.path.splitext(name)[1].lower()
                path = os.path.join(dirpath, name)
                rel = os.path.relpath(path, root)
                if not (rel.startswith("gdal/fgb") or rel.startswith("fgb/") or rel.startswith("gdal/gml")
                        or rel.startswith("gdal/osm") or rel.startswith("osm/") or rel.startswith("gdal/parquet")
                        or rel.startswith("gdal/arrow") or rel.startswith("geoparquet/")):
                    continue
                if ext in OGR_EXT:
                    if rel.startswith("gdal/gml/"):
                        path = os.path.join(gml_copy, os.path.relpath(path, gml_src))
                    n += ogr(path, rel, out)
                elif ext in ARROW_EXT:
                    n += arrow(path, rel, out)
    shutil.rmtree(scratch)
    print(f"  formats oracle: {n} layers")


if __name__ == "__main__":
    main()
