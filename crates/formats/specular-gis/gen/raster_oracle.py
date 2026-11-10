"""The raster oracle of the GIS formats corpus: what GDAL (through rasterio) reads from the
NITF files, the GeoPDFs and the Cloud-Optimized GeoTIFFs, written to OUT/expected/rasters.jsonl.

Per file: size, band count, data type, EPSG code, the affine transform (pixel corner to
map), the RPC model's offsets when there is one, NITF compression, and the values of up to
three bands on a 16 x 16 grid of pixel centres (for the pixel comparison).

Usage: python -I raster_oracle.py CORPUS_DIR

Reads the corpus as data only; nothing from it is executed. GDAL's .aux.xml sidecars are
not written (PAM off), so the corpus stays unchanged.
"""

import glob
import json
import os
import sys

import numpy as np
import rasterio


def record(path, rel):
    with rasterio.open(path) as d:
        t = d.transform
        rec = {
            "file": rel,
            "width": d.width,
            "height": d.height,
            "bands": d.count,
            "dtype": d.dtypes[0],
            "epsg": d.crs.to_epsg() if d.crs else None,
            "transform": [t.a, t.b, t.c, t.d, t.e, t.f],
            "ic": d.tags().get("NITF_IC"),
            "abpp": d.tags().get("NITF_ABPP"),
        }
        if d.rpcs:
            r = d.rpcs
            rec["rpc"] = {"lat_off": r.lat_off, "long_off": r.long_off, "line_off": r.line_off, "samp_off": r.samp_off}
        n = 16
        xs = [int((i + 0.5) * d.width / n) for i in range(n)]
        ys = [int((j + 0.5) * d.height / n) for j in range(n)]
        bands = list(range(1, min(d.count, 3) + 1))
        data = d.read(bands)
        rec["samples"] = [[[float(data[b][y][x]) for x in xs] for y in ys] for b in range(len(bands))]
        rec["xs"] = xs
        rec["ys"] = ys
        rec["mean"] = [float(np.mean(data[b])) for b in range(len(bands))]
        if d.count == 1:
            try:
                cmap = d.colormap(1)
                rec["cmap"] = [list(cmap.get(i, (0, 0, 0, 0))[:3]) for i in range(min(256, max(cmap) + 1))]
            except ValueError:  # no colour table
                pass
        return rec


def main():
    root = sys.argv[1]
    os.makedirs(os.path.join(root, "expected"), exist_ok=True)
    files = []
    for pat in ("gdal/nitf/*", "gen/nitf/*", "gdal/pdf/*.pdf", "gen/pdf/*.pdf", "gen/cog/*.tif"):
        files += sorted(glob.glob(os.path.join(root, pat)))
    n = 0
    with rasterio.Env(GDAL_PAM_ENABLED="NO"), open(os.path.join(root, "expected", "rasters.jsonl"), "w") as out:
        for path in files:
            rel = os.path.relpath(path, root)
            if rel.endswith((".hdr", ".nfw", ".vrt", ".TOC", ".ON2", ".on9")) or ".ntf.r" in rel:
                continue
            try:
                rec = record(path, rel)
            except Exception as e:  # noqa: BLE001 - GDAL refuses the file
                rec = {"file": rel, "error": str(e)[:200]}
            out.write(json.dumps(rec) + "\n")
            n += 1
    print(f"  raster oracle: {n} files")


if __name__ == "__main__":
    main()
