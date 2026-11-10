"""The File Geodatabase oracle: what GDAL's OpenFileGDB driver (through pyogrio) reads
from every .gdb directory of the GIS formats corpus, written to
CORPUS/expected/filegdb.jsonl for tests/filegdb_oracle.rs.

Usage: python -I filegdb_oracle.py CORPUS_DIR [EXTRA_DIR]

  Every *.gdb directory under CORPUS_DIR/gdal/filegdb (the zipped ones unpacked beside
  their zip as NAME.unzipped/) is read; EXTRA_DIR, when given, stands in for
  CORPUS_DIR/gdal/filegdb (zips unpacked elsewhere), and its files are recorded under
  gdal/filegdb/ all the same. One line per (file, layer) in the shape of oracle.py.

Reads the corpus as data only; nothing from it is executed.
"""

import json
import struct
import os
import sys

import pyogrio
import pyogrio.raw
import shapely
from pyproj import CRS

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from oracle import FEATURES, jsonable, parts  # noqa: E402


def wkb_parts(b):
    """Parts of a WKB geometry GEOS cannot parse (TIN, PolyhedralSurface, collections of
    them; ISO or extended Z/M flags): one list of [x, y] per point, line or ring."""
    out = []

    def geom(o, depth):
        if depth > 16:
            raise ValueError("deep")
        e = "<" if b[o] == 1 else ">"
        t = struct.unpack_from(e + "I", b, o + 1)[0]
        o += 5
        z, m = bool(t & 0x80000000), bool(t & 0x40000000)
        if t & 0x20000000:
            o += 4
        t &= 0x0FFFFFFF
        base, dim = t % 1000, t // 1000
        dims = 2 + (1 if z or dim in (1, 3) else 0) + (1 if m or dim in (2, 3) else 0)

        def coords(o):
            n = struct.unpack_from(e + "I", b, o)[0]
            o += 4
            pts = [list(struct.unpack_from(e + "dd", b, o + i * 8 * dims)) for i in range(n)]
            return pts, o + 8 * dims * n

        if base == 1:
            out.append([list(struct.unpack_from(e + "dd", b, o))])
            return o + 8 * dims
        if base in (2, 8):
            pts, o = coords(o)
            out.append(pts)
            return o
        if base in (3, 17):
            n = struct.unpack_from(e + "I", b, o)[0]
            o += 4
            for _ in range(n):
                pts, o = coords(o)
                out.append(pts)
            return o
        n = struct.unpack_from(e + "I", b, o)[0]
        o += 4
        for _ in range(n):
            o = geom(o, depth + 1)
        return o

    try:
        geom(0, 0)
    except (struct.error, ValueError, IndexError):
        return None
    return out


def gdb_dirs(base):
    for dirpath, dirs, _ in os.walk(base):
        for d in sorted(dirs):
            if d.lower().endswith(".gdb"):
                yield os.path.join(dirpath, d)


def record(path, rel, out):
    try:
        layers = pyogrio.list_layers(path)
    except Exception as e:  # noqa: BLE001 - GDAL refuses the folder
        out.write(json.dumps({"file": rel, "error": str(e)[:200]}) + "\n")
        return 0
    n = 0
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
        unparsed_any = False
        feats = []
        for i in range(len(fids)):
            p = parts(shapes[i])
            unparsed = geoms is not None and geoms[i] is not None and shapes[i] is None
            if unparsed:
                # Multipatches come as TINs and polyhedral surfaces: walked here instead.
                w = wkb_parts(bytes(geoms[i]))
                if w is not None:
                    p, unparsed = w, False
            vertices += sum(len(x) for x in p)
            unparsed_any = unparsed_any or unparsed
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
        out.write(
            json.dumps(
                {
                    "file": rel,
                    "layer": str(lname),
                    "count": len(fids),
                    "fields": [str(f) for f in meta["fields"]],
                    "epsg": epsg,
                    "vertices": vertices,
                    "unparsed": unparsed_any,
                    "features": feats,
                }
            )
            + "\n"
        )
        n += 1
    return n


def main():
    root = sys.argv[1]
    bases = [os.path.join(root, "gdal", "filegdb")] + sys.argv[2:3]
    os.makedirs(os.path.join(root, "expected"), exist_ok=True)
    n = files = 0
    seen = set()
    with open(os.path.join(root, "expected", "filegdb.jsonl"), "w") as out:
        for base in bases:
            for path in gdb_dirs(base):
                rel = os.path.join("gdal", "filegdb", os.path.relpath(path, base))
                if rel in seen:
                    continue
                seen.add(rel)
                n += record(path, rel, out)
                files += 1
    print(f"  filegdb oracle: {files} geodatabases, {n} layers")


if __name__ == "__main__":
    main()
