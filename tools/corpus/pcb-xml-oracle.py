#!/usr/bin/env python3
"""Independent counts of IPC-2581 files for specular-pcb's corpus test (tests/eng2_corpus.rs).

For every .xml under CORPUS/ipc2581: whether it has board data (Ecad), and for the first
non-panel step the placed pads, lines (incl. polylines), arcs, contours, components, drill
holes, the layers that draw something, and the profile's size in mm. Written with the
standard library's ElementTree, separately from the Rust reader.

    python3 -I tools/corpus/pcb-xml-oracle.py CORPUS > CORPUS/ipc2581-oracle.json
"""

import json
import math
import os
import sys
import xml.etree.ElementTree as ET

PRIMS = {"Circle", "RectCenter", "RectRound", "RectCham", "Oval", "Diamond", "Octagon",
         "Ellipse", "Triangle", "Hexagon", "Donut", "Thermal", "RectCorner"}
UNITS = {"INCH": 25.4, "MICRON": 0.001, "MILS": 0.0254, "MIL": 0.0254}


def local(tag):
    return tag.rsplit("}", 1)[-1]


def kids(e, name=None):
    return [c for c in e if name is None or local(c.tag) == name]


def places_primitive(e):
    """A Pad or Features element that places one primitive itself."""
    for c in e:
        n = local(c.tag)
        if n in ("StandardPrimitiveRef", "UserPrimitiveRef") or n in PRIMS:
            return n
    return None


def arc_points(x0, y0, x1, y1, cx, cy, cw):
    r = math.hypot(x0 - cx, y0 - cy)
    a0 = math.atan2(y0 - cy, x0 - cx)
    a1 = math.atan2(y1 - cy, x1 - cx)
    if cw:
        sweep = a1 - a0
        while sweep >= 0:
            sweep -= 2 * math.pi
    else:
        sweep = a1 - a0
        while sweep <= 0:
            sweep += 2 * math.pi
    if math.hypot(x1 - x0, y1 - y0) > 1e-9 and abs(abs(sweep) - 2 * math.pi) < 1e-12:
        sweep = 0.0
    n = max(2, int(abs(math.degrees(sweep)) * 10))
    return [(cx + r * math.cos(a0 + sweep * i / n), cy + r * math.sin(a0 + sweep * i / n))
            for i in range(n + 1)]


def polygon_points(poly, k):
    pts = []
    at = None
    for s in poly:
        n = local(s.tag)
        x, y = float(s.get("x", 0)) * k, float(s.get("y", 0)) * k
        if n == "PolyBegin":
            at = (x, y)
            pts.append(at)
        elif n == "PolyStepSegment" and at:
            at = (x, y)
            pts.append(at)
        elif n == "PolyStepCurve" and at:
            cx, cy = float(s.get("centerX", 0)) * k, float(s.get("centerY", 0)) * k
            cw = s.get("clockwise", "false").lower() in ("true", "1")
            pts += arc_points(at[0], at[1], x, y, cx, cy, cw)
            at = (x, y)
    return pts


def count(path):
    root = ET.parse(path).getroot()
    out = {"ecad": False}
    ecad = next(iter(kids(root, "Ecad")), None)
    if ecad is None:
        return out
    out["ecad"] = True
    head = next(iter(kids(ecad, "CadHeader")), None)
    k = UNITS.get((head.get("units", "") if head is not None else "").upper(), 1.0)
    cad = kids(ecad, "CadData")[0]
    steps = kids(cad, "Step")
    step = next((s for s in steps if s.get("type", "").upper() != "PANEL"), steps[0])
    layers = {l.get("name"): l.get("layerFunction", "").upper() for l in kids(cad, "Layer")}
    padstacks = {}
    for ps in kids(step, "PadStackDef"):
        padstacks[ps.get("name")] = [p for p in kids(ps, "PadstackPadDef")]
    c = dict(pads=0, lines=0, arcs=0, contours=0, holes=0, layers=0)
    for lf in kids(step, "LayerFeature"):
        ref = lf.get("layerRef")
        if ref not in layers:
            continue
        if layers[ref] == "DRILL":
            n = sum(len(kids(s, "Hole")) for s in kids(lf, "Set"))
            c["holes"] += n
            c["layers"] += 1 if n else 0
            continue
        c["layers"] += 1

        def walk(e):
            for f in e:
                n = local(f.tag)
                if n == "Line":
                    c["lines"] += 1
                elif n == "Polyline":
                    c["lines"] += 1
                elif n == "Arc":
                    c["arcs"] += 1
                elif n == "Contour":
                    c["contours"] += 1
                elif n in ("Features", "UserSpecial"):
                    p = places_primitive(f)
                    if p:
                        if p != "UserPrimitiveRef":
                            c["pads"] += 1
                    else:
                        walk(f)

        for s in kids(lf, "Set"):
            for e in s:
                n = local(e.tag)
                if n == "Pad":
                    p = places_primitive(e)
                    if p:
                        if p != "UserPrimitiveRef":
                            c["pads"] += 1
                    elif e.get("padstackDefRef") in padstacks:
                        for pd in padstacks[e.get("padstackDefRef")]:
                            q = pd.get("layerRef") == ref and places_primitive(pd)
                            if q and q != "UserPrimitiveRef":
                                c["pads"] += 1
                elif n == "Features":
                    p = places_primitive(e)
                    if p:
                        if p != "UserPrimitiveRef":
                            c["pads"] += 1
                    else:
                        walk(e)
    out.update(c)
    out["components"] = len(kids(step, "Component"))
    prof = next(iter(kids(step, "Profile")), None)
    if prof is not None:
        pts = []
        for poly in kids(prof, "Polygon"):
            pts += polygon_points(poly, k)
        if pts:
            xs = [p[0] for p in pts]
            ys = [p[1] for p in pts]
            out["profile_mm"] = [max(xs) - min(xs), max(ys) - min(ys)]
    return out


def main():
    root = sys.argv[1]
    res = {}
    for d, _, fs in sorted(os.walk(os.path.join(root, "ipc2581"))):
        for f in sorted(fs):
            if f.lower().endswith(".xml"):
                res[f] = count(os.path.join(d, f))
                print(f, res[f], file=sys.stderr)
    json.dump(res, sys.stdout, indent=1)


if __name__ == "__main__":
    main()
