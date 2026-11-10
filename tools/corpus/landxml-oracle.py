#!/usr/bin/env python3
"""Independent LandXML numbers for specular-landxml's tests: per file, per TIN surface the
point and visible triangle counts, plan and sloped areas, the volume above the lowest point
and the elevation range, and the elevation at probe points inside its first triangles (for
the 3D model's draping); per alignment and per parcel the summed element lengths (arcs from
centre and ends, spirals by their stated length); the survey points (CgPoint, pntRef
resolved) by count and coordinate sums. Standard library only (ElementTree), written
separately from the Rust reader.

    python3 -I tools/corpus/landxml-oracle.py FILE_OR_DIR... > oracle.json
"""

import json
import math
import os
import sys
import xml.etree.ElementTree as ET


def local(t):
    return t.rsplit("}", 1)[-1]


def kids(e, name):
    return [c for c in e if local(c.tag) == name]


def first(e, name):
    k = kids(e, name)
    return k[0] if k else None


def nums(t):
    out = []
    for s in (t or "").split():
        try:
            out.append(float(s))
        except ValueError:
            pass
    return out


def surface(s):
    d = first(s, "Definition")
    if d is None or d.get("surfType", "TIN").upper() != "TIN":
        return None
    pts = {}
    pn = first(d, "Pnts")
    for p in (kids(pn, "P") if pn is not None else []):
        v = nums(p.text)
        if len(v) >= 2:
            pts[p.get("id")] = (v[1], v[0], v[2] if len(v) > 2 else 0.0)
    tris = []
    fc = first(d, "Faces")
    for f in (kids(fc, "F") if fc is not None else []):
        if f.get("i") == "1":
            continue
        ids = (f.text or "").split()
        if len(ids) == 3 and all(i in pts for i in ids):
            tris.append([pts[i] for i in ids])
    if not tris:
        return None
    zs = [p[2] for t in tris for p in t]
    zmin, zmax = min(zs), max(zs)
    a2 = a3 = vol = 0.0
    for a, b, c in tris:
        u = [b[k] - a[k] for k in range(3)]
        v = [c[k] - a[k] for k in range(3)]
        cx = u[1] * v[2] - u[2] * v[1]
        cy = u[2] * v[0] - u[0] * v[2]
        cz = u[0] * v[1] - u[1] * v[0]
        area = abs(cz) / 2
        a2 += area
        a3 += math.sqrt(cx * cx + cy * cy + cz * cz) / 2
        vol += area * ((a[2] + b[2] + c[2]) / 3 - zmin)
    # Probes: a point a tenth of the way from each of the first 50 triangles' centroid to
    # its first corner, with the plane's elevation there.
    probes = []
    for a, b, c in tris[:50]:
        g = [(a[k] + b[k] + c[k]) / 3 for k in range(3)]
        probes.append([g[k] + (a[k] - g[k]) * 0.1 for k in range(3)])
    return dict(name=s.get("name", ""), points=len(pts), triangles=len(tris), area_2d=a2,
                area_3d=a3, volume=vol, z_min=zmin, z_max=zmax, probes=probes)


def alignment(a):
    total = 0.0
    cg = first(a, "CoordGeom")
    for e in (list(cg) if cg is not None else []):
        n = local(e.tag)
        pt = lambda k: (lambda v: (v[1], v[0]) if len(v) >= 2 else None)(nums(first(e, k).text)) if first(e, k) is not None else None
        if n == "Line":
            s, t = pt("Start"), pt("End")
            if s and t:
                total += math.hypot(t[0] - s[0], t[1] - s[1])
        elif n == "Curve":
            s, t, c = pt("Start"), pt("End"), pt("Center")
            if s and t and c:
                r = math.hypot(s[0] - c[0], s[1] - c[1])
                a0 = math.atan2(s[1] - c[1], s[0] - c[0])
                a1 = math.atan2(t[1] - c[1], t[0] - c[0])
                sw = a1 - a0
                if (e.get("rot") or "ccw").lower() == "cw":
                    while sw >= 0:
                        sw -= 2 * math.pi
                else:
                    while sw <= 0:
                        sw += 2 * math.pi
                total += r * abs(sw)
        elif n == "Spiral":
            if pt("Start") and e.get("length"):
                total += float(e.get("length"))
    return dict(name=a.get("name", ""), length=total)


def read(path):
    root = ET.parse(path).getroot()
    out = dict(surfaces=[], alignments=[])
    for ss in kids(root, "Surfaces"):
        for s in kids(ss, "Surface"):
            r = surface(s)
            if r:
                out["surfaces"].append(r)
    for al in kids(root, "Alignments"):
        for a in kids(al, "Alignment"):
            out["alignments"].append(alignment(a))
    out["parcels"] = [alignment(p) for ps in kids(root, "Parcels")
                      for p in ps.iter() if local(p.tag) == "Parcel"]
    named, refs = {}, []
    for g in kids(root, "CgPoints"):
        for p in g.iter():
            if local(p.tag) != "CgPoint":
                continue
            v = nums(p.text)
            if len(v) >= 2:
                named[p.get("name", "")] = (v[1], v[0])
                refs.append((v[1], v[0]))
            elif p.get("pntRef"):
                refs.append(p.get("pntRef"))
    pts = [r if isinstance(r, tuple) else named.get(r) for r in refs]
    pts = [p for p in pts if p is not None]
    out["points"] = dict(count=len(pts), sum_e=sum(p[0] for p in pts),
                         sum_n=sum(p[1] for p in pts))
    return out


def main():
    res = {}
    for arg in sys.argv[1:]:
        paths = [arg] if os.path.isfile(arg) else sorted(
            os.path.join(d, f) for d, _, fs in os.walk(arg) for f in fs
            if f.lower().endswith(".xml"))
        for p in paths:
            try:
                res[os.path.basename(p)] = read(p)
            except ET.ParseError as e:
                print(p, e, file=sys.stderr)
    json.dump(res, sys.stdout, indent=1)


if __name__ == "__main__":
    main()
