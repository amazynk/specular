"""Compares what Specular reads from MBTiles and PMTiles fixtures (written by
`specular-decode/tests/tiles_oracle.rs` into DIR) with GDAL (rasterio for raster MBTiles,
pyogrio for vector MBTiles and PMTiles) and the PMTiles reference reader (`pmtiles`).

Usage: python -I tiles_oracle.py DIR
Prints each comparison and the mismatch count; exits 1 on a mismatch.
"""

import json
import sys

import pyogrio
import rasterio
from pmtiles.reader import MmapSource, Reader

d = sys.argv[1]
bad = 0
checks = 0
for line in open(f"{d}/specular.jsonl"):
    m = json.loads(line)
    path = f"{d}/{m['file']}"
    if m["kind"] == "raster":
        with rasterio.open(path) as r:
            t = r.transform
            gt = [t.a, t.b, t.c, t.d, t.e, t.f]
            # GDAL takes the origin from the metadata `bounds` (rounded to 4 decimals in
            # most files), Specular from the tile grid: within half a pixel is agreement.
            px_size = abs(gt[0])
            ok = (r.width, r.height) == (m["width"], m["height"]) and all(
                abs(a - b) < (0.5 * px_size if k in (2, 5) else 1e-6)
                for k, (a, b) in enumerate(zip(gt, m["transform"]))
            )
            checks += 1
            bad += not ok
            print(f"{m['file']}: GDAL {r.width}x{r.height} {gt}; specular {m['width']}x{m['height']} {m['transform']}: {'ok' if ok else 'MISMATCH'}")
            px = r.read([1, 2, 3])
            off = sum(
                1
                for x, y, R, G, B in m["samples"]
                if [int(px[0, y, x]), int(px[1, y, x]), int(px[2, y, x])] != [R, G, B]
            )
            checks += len(m["samples"])
            bad += off
            print(f"  pixels: {len(m['samples']) - off}/{len(m['samples'])} equal")
    elif m["kind"] == "pmtiles":
        with open(path, "rb") as f:
            rd = Reader(MmapSource(f))
            off = 0
            for z, x, y, n in m["tiles"]:
                t = rd.get(z, x, y)
                checks += 1
                if t is None or len(t) != n:
                    off += 1
            bad += off
            print(f"{m['file']}: {len(m['tiles']) - off}/{len(m['tiles'])} tiles as the reference reader reads them")
    elif m["kind"] == "vector":
        opts = {"ZOOM_LEVEL": str(m["zoom"])}
        for layer, n in m["layers"].items():
            info = pyogrio.read_info(path, layer=layer, **opts)
            g = info["features"]
            checks += 1
            ok = g == n
            bad += not ok
            print(f"{m['file']} {layer}: GDAL {g}, specular {n}: {'ok' if ok else 'MISMATCH'}")
print(f"{checks} checks, {bad} mismatches")
sys.exit(1 if bad else 0)
