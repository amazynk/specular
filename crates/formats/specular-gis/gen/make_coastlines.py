"""Packs the Natural Earth 1:110m coastlines (public domain) into src/coastline110.bin.

usage: make_coastlines.py ne_110m_coastline.geojson

Get the input from
https://raw.githubusercontent.com/nvkelso/natural-earth-vector/master/geojson/ne_110m_coastline.geojson

Format (read by src/coastline.rs), all integers unsigned LEB128 varints:

    b"NECL" 1 (version)  line count
    per line: vertex count, then per vertex the zigzag-coded change of longitude and latitude
              from the previous vertex (the first from 0, 0), in thousandths of a degree.
"""

import json
import sys

SCALE = 1000


def varint(out, v):
    while True:
        b = v & 0x7F
        v >>= 7
        if v:
            out.append(b | 0x80)
        else:
            out.append(b)
            return


def zigzag(v):
    return (v << 1) ^ (v >> 63) if v < 0 else v << 1


def main():
    src = json.load(open(sys.argv[1], encoding="utf-8"))
    lines = []
    for f in src["features"]:
        g = f["geometry"]
        parts = g["coordinates"] if g["type"] == "MultiLineString" else [g["coordinates"]]
        for p in parts:
            pts = [(round(x * SCALE), round(y * SCALE)) for x, y, *_ in p]
            # Drop repeats that quantisation made.
            q = [pts[0]] + [b for a, b in zip(pts, pts[1:]) if a != b]
            if len(q) >= 2:
                lines.append(q)
    out = bytearray(b"NECL")
    varint(out, 1)
    varint(out, len(lines))
    vertices = 0
    for q in lines:
        varint(out, len(q))
        px, py = 0, 0
        for x, y in q:
            varint(out, zigzag(x - px))
            varint(out, zigzag(y - py))
            px, py = x, y
        vertices += len(q)
    dst = __file__.rsplit("/", 2)[0] + "/src/coastline110.bin"
    with open(dst, "wb") as f:
        f.write(out)
    print(f"{len(lines)} lines, {vertices} vertices, {len(out)} bytes -> {dst}")


if __name__ == "__main__":
    main()
