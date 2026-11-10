"""The MapInfo oracle: what GDAL's "MapInfo File" driver (through pyogrio) reads from the
MapInfo TAB and MIF files of the GIS formats corpus, for tests/mitab_oracle.rs.

Usage: python -I mitab_oracle.py CORPUS_DIR [OUT_JSONL]

  OUT_JSONL (default CORPUS_DIR/expected/mitab.jsonl): one line per (file, layer) in the
  record shape of oracle.py: feature count, field names, EPSG code, vertex count, and for
  the first 200 features the fid, attribute values and coordinates (one list per point,
  line part or ring).

Reads the corpus as data only; nothing from it is executed.
"""

import json
import math
import os
import sys

import numpy as np
import pyogrio
import pyogrio.raw
import shapely
from pyproj import CRS

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
    if g is None or g.is_empty:
        return []
    t = g.geom_type
    if t == "Point":
        return [[[g.x, g.y]]]
    if t in ("LineString", "LinearRing"):
        return [[list(c[:2]) for c in g.coords]]
    if t == "Polygon":
        return [[list(c[:2]) for c in r.coords] for r in [g.exterior, *g.interiors]]
    out = []
    for sub in getattr(g, "geoms", []):
        out.extend(parts(sub))
    return out


def main():
    root = sys.argv[1]
    out_path = sys.argv[2] if len(sys.argv) > 2 else os.path.join(root, "expected", "mitab.jsonl")
    os.makedirs(os.path.dirname(out_path), exist_ok=True)
    n = 0
    with open(out_path, "w") as out:
        for dirpath, _, files in os.walk(root):
            if "/expected" in dirpath:
                continue
            for name in sorted(files):
                if os.path.splitext(name)[1].lower() not in (".tab", ".mif"):
                    continue
                path = os.path.join(dirpath, name)
                rel = os.path.relpath(path, root)
                try:
                    layers = pyogrio.list_layers(path)
                except Exception as e:  # noqa: BLE001
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
                    shapes = [None] * len(fids) if geoms is None else shapely.from_wkb(geoms, on_invalid="ignore")
                    vertices = 0
                    feats = []
                    for i in range(len(fids)):
                        p = parts(shapes[i])
                        vertices += sum(len(x) for x in p)
                        unparsed = geoms is not None and geoms[i] is not None and shapes[i] is None
                        if i < FEATURES:
                            feats.append(
                                {
                                    "fid": int(fids[i]),
                                    "type": None if shapes[i] is None else shapes[i].geom_type,
                                    "attrs": [jsonable(col[i]) for col in fields],
                                    "parts": None if unparsed or sum(len(x) for x in p) > 5000 else p,
                                }
                            )
                    epsg = None
                    if meta.get("crs"):
                        try:
                            epsg = CRS.from_user_input(meta["crs"]).to_epsg()
                        except Exception:  # noqa: BLE001
                            epsg = None
                    out.write(
                        json.dumps(
                            {
                                "file": rel,
                                "layer": str(lname),
                                "count": len(fids),
                                "fields": [str(f) for f in meta["fields"]],
                                "epsg": epsg,
                                "vertices": vertices,
                                "features": feats,
                            }
                        )
                        + "\n"
                    )
                    n += 1
    print(f"  mitab oracle: {n} layers -> {out_path}")


if __name__ == "__main__":
    main()
