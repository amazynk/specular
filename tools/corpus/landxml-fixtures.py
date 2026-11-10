#!/usr/bin/env python3
"""Writes crates/specular-landxml/tests/data/site.xml: our own small LandXML 1.2 site.

A TIN (a hill on a 7 x 7 grid, triangulated), a breakline and boundaries; an alignment of a
line, a clothoid in, a circular arc, a clothoid out and a line (spiral ends computed here by
Simpson integration of the heading, independently of the Rust reader); a design profile with
parabolic vertical curves and a ground profile; units and a coordinate system.

    python3 -I tools/corpus/landxml-fixtures.py
"""

import math
import os

OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "crates",
                   "formats", "specular-landxml", "tests", "data", "site.xml")


def ne(x, y, z=None):
    return f"{y:.6f} {x:.6f}" + (f" {z:.6f}" if z is not None else "")


def clothoid(x, y, d, length, k0, k1, n=20000):
    """Point at the end by Simpson's rule on cos/sin of the heading."""
    dk = (k1 - k0) / length
    h = length / n
    sx = sy = 0.0
    for i in range(n + 1):
        t = i * h
        th = d + k0 * t + dk * t * t / 2
        w = 1 if i in (0, n) else (4 if i % 2 else 2)
        sx += w * math.cos(th)
        sy += w * math.sin(th)
    return x + sx * h / 3, y + sy * h / 3, d + k0 * length + dk * length * length / 2


def main():
    # Surface: z = 100 + 6 exp(-r²/600) over a 60 m square, 10 m grid.
    pts = []
    for j in range(7):
        for i in range(7):
            x, y = 1000 + 10 * i, 2000 + 10 * j
            r2 = (x - 1030) ** 2 + (y - 2030) ** 2
            pts.append((x, y, 100 + 6 * math.exp(-r2 / 600)))
    faces = []
    for j in range(6):
        for i in range(6):
            a = j * 7 + i + 1
            b, c, d = a + 1, a + 8, a + 7
            faces += [(a, b, c), (a, c, d)]
    # Alignment: east 100, clothoid L=40 into R=200 left, arc 30°, clothoid out, line 50.
    R, L = 200.0, 40.0
    x, y, d = 900.0, 1980.0, 0.0
    geo = []
    x1 = x + 100
    geo.append(f'<Line length="100"><Start>{ne(x, y)}</Start><End>{ne(x1, y)}</End></Line>')
    x, y = x1, y
    sx, sy = x, y
    ex, ey, d = clothoid(x, y, 0.0, L, 0.0, 1 / R)
    # PI: where the start tangent meets the end tangent.
    t = ((ex - sx) * math.sin(d) - (ey - sy) * math.cos(d)) / (math.sin(d) * 1 - 0 * math.cos(d))
    pi = (sx + t, sy)
    geo.append(f'<Spiral length="{L}" radiusStart="INF" radiusEnd="{R}" rot="ccw" '
               f'spiType="clothoid"><Start>{ne(sx, sy)}</Start><PI>{ne(*pi)}</PI>'
               f'<End>{ne(ex, ey)}</End></Spiral>')
    x, y = ex, ey
    cx, cy = x - R * math.sin(d), y + R * math.cos(d)
    a0 = math.atan2(y - cy, x - cx)
    a1 = a0 + math.radians(30)
    ax, ay = cx + R * math.cos(a1), cy + R * math.sin(a1)
    geo.append(f'<Curve rot="ccw" radius="{R}" length="{R * math.radians(30):.6f}">'
               f'<Start>{ne(x, y)}</Start><Center>{ne(cx, cy)}</Center><End>{ne(ax, ay)}</End>'
               '</Curve>')
    d += math.radians(30)
    x, y = ax, ay
    ex, ey, d2 = clothoid(x, y, d, L, 1 / R, 0.0)
    pix = x + 10 * math.cos(d)
    piy = y + 10 * math.sin(d)
    geo.append(f'<Spiral length="{L}" radiusStart="{R}" radiusEnd="INF" rot="ccw" '
               f'spiType="clothoid"><Start>{ne(x, y)}</Start><PI>{ne(pix, piy)}</PI>'
               f'<End>{ne(ex, ey)}</End></Spiral>')
    x, y = ex, ey
    lx, ly = x + 50 * math.cos(d2), y + 50 * math.sin(d2)
    geo.append(f'<Line length="50"><Start>{ne(x, y)}</Start><End>{ne(lx, ly)}</End></Line>')
    length = 100 + L + R * math.radians(30) + L + 50

    surf = "\n".join(f'          <P id="{k + 1}">{ne(*p)}</P>' for k, p in enumerate(pts))
    fcs = "\n".join(f"          <F>{a} {b} {c}</F>" for a, b, c in faces)
    xml = f'''<?xml version="1.0" encoding="UTF-8"?>
<LandXML xmlns="http://www.landxml.org/schema/LandXML-1.2" version="1.2" date="2026-10-25" time="12:00:00">
  <Units><Metric areaUnit="squareMeter" linearUnit="meter" volumeUnit="cubicMeter" temperatureUnit="celsius" pressureUnit="milliBars" angularUnit="decimal degrees" directionUnit="decimal degrees"/></Units>
  <CoordinateSystem name="ETRS89 / UTM zone 32N" epsgCode="25832" horizontalDatum="ETRS89" verticalDatum="DHHN2016"/>
  <Project name="Specular test site"/>
  <Application name="specular fixture" version="1"/>
  <Surfaces>
    <Surface name="Existing ground">
      <SourceData>
        <Breaklines><Breakline name="ridge"><PntList3D>{ne(1000, 2030, 100.1)} {ne(1030, 2030, 106)} {ne(1060, 2030, 100.1)}</PntList3D></Breakline></Breaklines>
        <Boundaries><Boundary name="site" bndType="outer"><PntList2D>{ne(1000, 2000)} {ne(1060, 2000)} {ne(1060, 2060)} {ne(1000, 2060)} {ne(1000, 2000)}</PntList2D></Boundary></Boundaries>
      </SourceData>
      <Definition surfType="TIN">
        <Pnts>
{surf}
        </Pnts>
        <Faces>
{fcs}
          <F i="1">1 2 9</F>
        </Faces>
      </Definition>
    </Surface>
  </Surfaces>
  <Alignments name="roads">
    <Alignment name="Main road" length="{length:.6f}" staStart="1000">
      <CoordGeom>
        {chr(10).join("        " + g for g in geo).strip()}
      </CoordGeom>
      <Profile name="Main road">
        <ProfSurf name="Existing ground"><PntList2D>1000 100.2 1050 101.0 1100 103.5 1150 104.0 1200 102.0 1250 101.5 1300 100.8</PntList2D></ProfSurf>
        <ProfAlign name="Design">
          <PVI>1000 100.5</PVI>
          <ParaCurve length="60">1100 104.5</ParaCurve>
          <ParaCurve length="80">1200 101.0</ParaCurve>
          <PVI>1300 101.2</PVI>
        </ProfAlign>
      </Profile>
    </Alignment>
  </Alignments>
</LandXML>
'''
    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    with open(OUT, "w") as f:
        f.write(xml)
    print(OUT, round(length, 6))


if __name__ == "__main__":
    main()
