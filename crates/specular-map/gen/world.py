"""Generates data/world.bin, the map view's offline base map.

Source: Natural Earth (public domain, https://www.naturalearthdata.com/), as GeoJSON from
https://github.com/nvkelso/natural-earth-vector (geojson/): 1:50m land, 1:110m lakes and
1:50m admin-0 boundary lines (land). Download them once into a directory of their own, then:

    python -I gen/world.py NE_DIR data/world.bin

Coastlines are the land polygons' outlines (the renderer leaves out the edges at the
antimeridian and the bottom of the map), so they are not stored twice.

Format (little endian), everything in Web Mercator world units quantised to 1/2^20 (the world
is the unit square, x east from 180°W, y south from 85.0511°N; one unit is about 38 m at the
equator):

    b"SPMAPW01"
    3 layers, in order: land rings, lake rings, border lines. Each:
        varint  number of rings / lines
        per ring: varint point count, then the points as zigzag varint deltas (x, y) from
        the previous point (the first from the previous ring's last; from (0, 0) at the
        start of the layer). Rings are closed implicitly (the last point is not repeated).
"""
import json
import math
import os
import sys

BITS = 20
Q = 1 << BITS
MAX_LAT = 85.0511287798066


def project(lon, lat):
    lat = max(-MAX_LAT, min(MAX_LAT, lat))
    x = (lon + 180.0) / 360.0
    s = math.sin(math.radians(lat))
    y = 0.5 - math.log((1.0 + s) / (1.0 - s)) / (4.0 * math.pi)
    xi = int(round(x * Q))
    yi = int(round(y * Q))
    return max(0, min(Q, xi)), max(0, min(Q, yi))


def rings_of(path, polygons):
    d = json.load(open(path))
    out = []
    for f in d["features"]:
        g = f["geometry"]
        if g is None:
            continue
        t, cs = g["type"], g["coordinates"]
        if polygons:
            polys = [cs] if t == "Polygon" else cs if t == "MultiPolygon" else []
            for poly in polys:
                for r in poly:
                    out.append(r)
        else:
            lines = [cs] if t == "LineString" else cs if t == "MultiLineString" else []
            out.extend(lines)
    return out


def quantise(line, closed):
    pts = []
    for c in line:
        p = project(c[0], c[1])
        if not pts or pts[-1] != p:
            pts.append(p)
    if closed and len(pts) > 1 and pts[0] == pts[-1]:
        pts.pop()
    return pts if len(pts) >= (3 if closed else 2) else None


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
    return (v << 1) if v >= 0 else ((-v) << 1) - 1


def layer(out, lines):
    varint(out, len(lines))
    px, py = 0, 0
    for pts in lines:
        varint(out, len(pts))
        for x, y in pts:
            varint(out, zigzag(x - px))
            varint(out, zigzag(y - py))
            px, py = x, y


def main(src, dst):
    land = [q for r in rings_of(os.path.join(src, "ne_50m_land.geojson"), True)
            if (q := quantise(r, True))]
    lakes = [q for r in rings_of(os.path.join(src, "ne_110m_lakes.geojson"), True)
             if (q := quantise(r, True))]
    borders = [q for r in rings_of(
        os.path.join(src, "ne_50m_admin_0_boundary_lines_land.geojson"), False)
        if (q := quantise(r, False))]
    out = bytearray(b"SPMAPW01")
    for ls in (land, lakes, borders):
        layer(out, ls)
    with open(dst, "wb") as f:
        f.write(out)
    n = sum(len(x) for x in land + lakes + borders)
    print(f"{dst}: {len(out)} bytes, {len(land)} land rings, {len(lakes)} lakes, "
          f"{len(borders)} border lines, {n} points")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
