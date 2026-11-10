#!/usr/bin/env python3
"""Oracle data for specular-geodesy's tests: GeographicLib (geodesics, polygon areas),
PROJ through pyproj (UTM, UPS) and the `mgrs` package (GeoTrans) for MGRS.

Run from a venv with `pip install geographiclib pyproj mgrs packaging`:
    python gen/oracle.py tests/data
Writes small TSV files the Rust tests read; nothing at test time needs Python.
"""

import math
import random
import sys
from pathlib import Path

from geographiclib.geodesic import Geodesic
from pyproj import Transformer
import mgrs as mgrs_mod

G = Geodesic.WGS84
OUT = Path(sys.argv[1] if len(sys.argv) > 1 else "tests/data")
OUT.mkdir(parents=True, exist_ok=True)
rng = random.Random(20261018)


def r(v):
    return repr(float(v))


def rand_pt():
    # Uniform on the sphere-ish (latitude from asin of uniform).
    return math.degrees(math.asin(rng.uniform(-1, 1))), rng.uniform(-180, 180)


def inverse_cases():
    cases = []
    for _ in range(600):  # random pairs
        a, b = rand_pt(), rand_pt()
        cases.append((*a, *b))
    for _ in range(150):  # short lines, < 1 m to 10 km
        la, lo = rand_pt()
        d = 10 ** rng.uniform(-3, 4)
        g = G.Direct(la, lo, rng.uniform(-180, 180), d)
        cases.append((la, lo, g["lat2"], g["lon2"]))
    for _ in range(150):  # nearly antipodal
        la, lo = rand_pt()
        cases.append((la, lo, -la + rng.uniform(-0.5, 0.5), lo + 180 + rng.uniform(-0.5, 0.5)))
    for _ in range(50):  # exactly antipodal and on the equator
        la = rng.uniform(-90, 90)
        cases.append((la, 0.0, -la, 180.0))
        cases.append((0.0, rng.uniform(-180, 180), 0.0, rng.uniform(-180, 180)))
    for _ in range(50):  # meridians and poles
        lo = rng.uniform(-180, 180)
        cases.append((rng.uniform(-90, 90), lo, rng.uniform(-90, 90), lo))
        cases.append((90.0, rng.uniform(-180, 180), rng.uniform(-90, 90), rng.uniform(-180, 180)))
    for _ in range(20):  # coincident
        la, lo = rand_pt()
        cases.append((la, lo, la, lo))
    return cases


def write_inverse():
    with open(OUT / "inverse.tsv", "w") as f:
        f.write("# lat1 lon1 lat2 lon2 s12 azi1 azi2 (GeographicLib Geodesic.WGS84)\n")
        for la1, lo1, la2, lo2 in inverse_cases():
            g = G.Inverse(la1, lo1, la2, lo2)
            f.write("\t".join(map(r, (la1, lo1, la2, lo2, g["s12"], g["azi1"], g["azi2"]))) + "\n")


def write_direct():
    with open(OUT / "direct.tsv", "w") as f:
        f.write("# lat1 lon1 azi1 s12 lat2 lon2 azi2\n")
        for _ in range(500):
            la, lo = rand_pt()
            az = rng.uniform(-180, 180)
            s = 10 ** rng.uniform(-2, 7.3)
            g = G.Direct(la, lo, az, s)
            f.write("\t".join(map(r, (la, lo, az, s, g["lat2"], g["lon2"], g["azi2"]))) + "\n")


def write_area():
    with open(OUT / "area.tsv", "w") as f:
        f.write("# area perimeter then lat lon pairs (GeographicLib Polygon, unsigned smaller)\n")
        polys = []
        for _ in range(80):  # random small to continental polygons
            la, lo = rng.uniform(-75, 75), rng.uniform(-180, 180)
            size = 10 ** rng.uniform(-4, 1.2)
            n = rng.randint(3, 12)
            pts = []
            for k in range(n):
                ang = 2 * math.pi * k / n + rng.uniform(-0.2, 0.2)
                rad = size * rng.uniform(0.5, 1.0)
                pts.append((la + rad * math.sin(ang), lo + rad * math.cos(ang)))
            polys.append(pts)
        for _ in range(10):  # around a pole
            lat = rng.uniform(60, 89) * rng.choice([-1, 1])
            n = rng.randint(4, 30)
            polys.append([(lat + rng.uniform(-0.5, 0.5), -180 + 360 * k / n) for k in range(n)])
        for _ in range(10):  # across the antimeridian
            la = rng.uniform(-60, 60)
            polys.append([(la, 179.0), (la, -179.0), (la + 1.5, -179.5), (la + 1.0, 178.5)])
        polys.append([(0.0, 0.0), (0.0, 1.0), (1.0, 1.0), (1.0, 0.0)])
        polys.append([(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)])
        for pts in polys:
            p = G.Polygon()
            for la, lo in pts:
                p.AddPoint(la, lo)
            _, per, area = p.Compute(False, True)
            area0 = 4 * math.pi * G._c2
            a = abs(area)
            if a > area0 / 2:
                a = area0 - a
            f.write("\t".join([r(a), r(per)] + [r(v) for pt in pts for v in pt]) + "\n")


def write_utm():
    with open(OUT / "utm.tsv", "w") as f:
        f.write("# lat lon zone north easting northing (PROJ via pyproj; zone 0 = UPS)\n")
        rows = []
        for _ in range(900):
            la = rng.uniform(-80, 84)
            lo = rng.uniform(-180, 180)
            zone = min(int((lo + 180) // 6) + 1, 60)
            # Sometimes a neighbouring zone (points up to ~9° from the central meridian).
            zone = ((zone - 1 + rng.choice([0, 0, 0, -1, 1])) % 60) + 1
            rows.append((la, lo, zone, la >= 0))
        for la in (-80.0, 0.0, 84.0, 56.0, 72.0):
            for lo in (-180.0, -177.0, 0.0, 3.0, 9.0, 179.999):
                zone = min(int((lo + 180) // 6) + 1, 60)
                rows.append((la, lo, zone, la >= 0))
        for _ in range(150):
            north = rng.random() < 0.5
            la = rng.uniform(80, 90) if north else rng.uniform(-90, -79)
            rows.append((la, rng.uniform(-180, 180), 0, north))
        tf = {}
        for la, lo, zone, north in rows:
            if zone == 0:
                epsg = 32661 if north else 32761
            else:
                epsg = (32600 if north else 32700) + zone
            t = tf.get(epsg)
            if t is None:
                t = tf[epsg] = Transformer.from_crs(4326, epsg, always_xy=True)
            e, n = t.transform(lo, la)
            f.write("\t".join([r(la), r(lo), str(zone), "1" if north else "0", r(e), r(n)]) + "\n")


def write_mgrs():
    m = mgrs_mod.MGRS()
    with open(OUT / "mgrs.tsv", "w") as f:
        f.write("# lat lon mgrs(5 digits, GeoTrans via the mgrs package)\n")
        pts = [rand_pt() for _ in range(1200)]
        pts += [(rng.uniform(-90, -80.001), rng.uniform(-180, 180)) for _ in range(50)]
        pts += [(rng.uniform(84.001, 90), rng.uniform(-180, 180)) for _ in range(50)]
        pts += [(rng.uniform(56, 64), rng.uniform(0, 12)) for _ in range(50)]
        pts += [(rng.uniform(72, 84), rng.uniform(0, 42)) for _ in range(80)]
        for la, lo in pts:
            if la == 90 or la == -90:
                continue
            s = m.toMGRS(la, lo, MGRSPrecision=5)
            if isinstance(s, bytes):
                s = s.decode()
            f.write(f"{r(la)}\t{r(lo)}\t{s}\n")


write_inverse()
write_direct()
write_area()
write_utm()
write_mgrs()
print("wrote", OUT)
