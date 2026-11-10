#!/usr/bin/env python3
"""Independent facts for the X3D corpus (tools/corpus/models-extra/x3d): a small reader of X3D's
XML encoding written apart from the Rust one, with only the standard library.

    python -I crates/formats/specular-mesh/gen/x3d_oracle.py tools/corpus/models-extra

Writes OUT/oracle/x3d.tsv with one row per file and fact:
    relpath  key  value
keys: tris (triangles of the face and triangle sets, quad sets, elevation grids and boxes:
k - 2 per face of k >= 3 corners), min, max (of the transformed vertices those faces use,
plus box corners), round (1 when the file has spheres, cylinders, cones or extrusions, whose
tessellation is the reader's own: triangle counts and bounds are then not compared), error.
Grouping: Transform (centre, scale orientation), Group, StaticGroup, Collision, Anchor,
Billboard, the CAD nodes, Switch by whichChoice, LOD's first child, Inline (same folder),
DEF/USE.
"""
import math
import os
import sys
import xml.etree.ElementTree as ET

ROUND = {"Sphere", "Cylinder", "Cone", "Extrusion"}
GROUPS = {"Group", "StaticGroup", "Collision", "Anchor", "Billboard", "CADAssembly",
          "CADLayer", "Scene"}


def nums(s):
    out = []
    for t in (s or "").replace(",", " ").split():
        try:
            out.append(float(int(t, 16)) if t.lower().startswith("0x") else float(t))
        except ValueError:
            out.append(float("nan"))
    return out


def strings(s):
    if s is None:
        return []
    if '"' not in s:
        return [s.strip()] if s.strip() else []
    out, cur, esc = [], None, False
    for c in s:
        if cur is not None:
            if esc:
                cur += c
                esc = False
            elif c == "\\":
                esc = True
            elif c == '"':
                out.append(cur)
                cur = None
            else:
                cur += c
        elif c == '"':
            cur = ""
    return out


def matmul(a, b):
    return [[sum(a[r][k] * b[k][c] for k in range(4)) for c in range(4)] for r in range(4)]


def ident():
    return [[1.0 if r == c else 0.0 for c in range(4)] for r in range(4)]


def trans(t):
    m = ident()
    for i in range(3):
        m[i][3] = t[i]
    return m


def rot(r):
    x, y, z, a = r
    n = math.sqrt(x * x + y * y + z * z)
    if n < 1e-12:
        return ident()
    x, y, z = x / n, y / n, z / n
    c, s, t = math.cos(a), math.sin(a), 1 - math.cos(a)
    return [[t * x * x + c, t * x * y - s * z, t * x * z + s * y, 0],
            [t * x * y + s * z, t * y * y + c, t * y * z - s * x, 0],
            [t * x * z - s * y, t * y * z + s * x, t * z * z + c, 0],
            [0, 0, 0, 1]]


def scl(s):
    m = ident()
    for i in range(3):
        m[i][i] = s[i]
    return m


def vec(e, k, d):
    v = nums(e.get(k))
    return v[:len(d)] if len(v) >= len(d) and all(math.isfinite(x) for x in v[:len(d)]) else d


def transform(e):
    t = vec(e, "translation", [0, 0, 0])
    c = vec(e, "center", [0, 0, 0])
    r = vec(e, "rotation", [0, 0, 1, 0])
    s = vec(e, "scale", [1, 1, 1])
    so = vec(e, "scaleOrientation", [0, 0, 1, 0])
    m = trans(t)
    for x in (trans(c), rot(r), rot(so), scl(s), rot(so[:3] + [-so[3]]),
              trans([-v for v in c])):
        m = matmul(m, x)
    return m


def apply(m, p):
    return [m[r][0] * p[0] + m[r][1] * p[1] + m[r][2] * p[2] + m[r][3] for r in range(3)]


class State:
    def __init__(self, folder):
        self.folder = folder
        self.tris = 0
        self.lo = [math.inf] * 3
        self.hi = [-math.inf] * 3
        self.round = False
        self.inlines = {}
        self.stack = []

    def add(self, p):
        if all(math.isfinite(v) for v in p):
            for i in range(3):
                self.lo[i] = min(self.lo[i], p[i])
                self.hi[i] = max(self.hi[i], p[i])


def coords(g, defs):
    for c in g:
        c = resolve(c, defs)
        if c.tag in ("Coordinate", "CoordinateDouble"):
            v = nums(c.get("point"))
            return [v[i:i + 3] for i in range(0, len(v) - 2, 3)]
    return []


def resolve(e, defs):
    u = e.get("USE")
    if u is not None:
        return defs.get(u, e)
    d = e.get("DEF")
    if d is not None:
        defs[d] = e
    return e


def faces_of(g, pts):
    tag = g.tag
    ints = lambda k: [int(v) if math.isfinite(v) and v == int(v) else -1 for v in nums(g.get(k))]
    out = []
    if tag == "IndexedFaceSet":
        cur = []
        for v in ints("coordIndex"):
            if v < 0:
                out.append(cur)
                cur = []
            else:
                cur.append(v)
        out.append(cur)
    elif tag in ("IndexedTriangleSet", "TriangleSet", "IndexedQuadSet", "QuadSet"):
        seq = [v for v in ints("index") if v >= 0] if tag.startswith("Indexed") else list(range(len(pts)))
        k = 3 if "Triangle" in tag else 4
        out = [seq[i:i + k] for i in range(0, len(seq) - k + 1, k)]
    elif tag in ("IndexedTriangleFanSet", "TriangleFanSet", "IndexedTriangleStripSet",
                 "TriangleStripSet"):
        if tag.startswith("Indexed"):
            runs, cur = [], []
            for v in ints("index"):
                if v < 0:
                    runs.append(cur)
                    cur = []
                else:
                    cur.append(v)
            runs.append(cur)
        else:
            runs, at = [], 0
            key = "fanCount" if "Fan" in tag else "stripCount"
            for c in ints(key):
                if c < 0 or at + c > len(pts):
                    break
                runs.append(list(range(at, at + c)))
                at += c
        for r in runs:
            for k in range(len(r) - 2):
                out.append([r[0], r[k + 1], r[k + 2]] if "Fan" in tag else r[k:k + 3])
    return [f for f in out if len(f) >= 3 and all(0 <= v < len(pts) for v in f)]


def geometry(g, m, st, defs):
    tag = g.tag
    if tag in ROUND:
        st.round = True
        return
    if tag == "Box":
        s = [v / 2 for v in vec(g, "size", [2, 2, 2])]
        st.tris += 12
        for i in range(8):
            st.add(apply(m, [s[0] * (1 if i & 1 else -1), s[1] * (1 if i & 2 else -1),
                             s[2] * (1 if i & 4 else -1)]))
        return
    if tag == "ElevationGrid":
        xd, zd = int(nums(g.get("xDimension"))[0] if g.get("xDimension") else 0), \
            int(nums(g.get("zDimension"))[0] if g.get("zDimension") else 0)
        xs = (nums(g.get("xSpacing")) or [1.0])[0]
        zs = (nums(g.get("zSpacing")) or [1.0])[0]
        h = nums(g.get("height"))
        if xd < 2 or zd < 2 or len(h) < xd * zd:
            return
        st.tris += 2 * (xd - 1) * (zd - 1)
        for j in range(zd):
            for i in range(xd):
                st.add(apply(m, [i * xs, h[i + j * xd], j * zs]))
        return
    pts = coords(g, defs)
    for f in faces_of(g, pts):
        st.tris += len(f) - 2
        for v in f:
            st.add(apply(m, pts[v]))


def walk(e, m, st, defs, depth=0):
    if depth > 200:
        return
    e = resolve(e, defs)
    tag = e.tag
    if tag in ("Transform", "CADPart"):
        m2 = matmul(m, transform(e))
        for c in e:
            walk(c, m2, st, defs, depth + 1)
    elif tag in GROUPS:
        for c in e:
            walk(c, m, st, defs, depth + 1)
    elif tag == "CADFace":
        for c in e:
            walk(c, m, st, defs, depth + 1)
    elif tag == "Switch":
        w = int((nums(e.get("whichChoice")) or [-1])[0])
        kids = list(e)
        if 0 <= w < len(kids):
            walk(kids[w], m, st, defs, depth + 1)
    elif tag == "LOD":
        kids = list(e)
        if kids:
            walk(kids[0], m, st, defs, depth + 1)
    elif tag == "Shape":
        for c in e:
            c = resolve(c, defs)
            if c.tag not in ("Appearance",) and c.get("containerField", "geometry") == "geometry":
                geometry(c, m, st, defs)
    elif tag == "Inline":
        for u in strings(e.get("url")):
            u = u.split("#")[0]
            if u.startswith("./"):
                u = u[2:]
            if not u or any(ch in u for ch in "/\\:"):
                continue
            path = os.path.join(st.folder, u)
            if not os.path.isfile(path) or u in st.stack:
                continue
            if not u.endswith(".x3d"):
                continue
            root = ET.parse(path).getroot()
            sc = root.find("Scene")
            st.stack.append(u)
            inner = {}
            for c in (sc if sc is not None else []):
                walk(c, m, st, inner, depth + 1)
            st.stack.pop()
            break


def main():
    out = sys.argv[1]
    folder = os.path.join(out, "x3d")
    rows = []
    for name in sorted(os.listdir(folder)):
        if not name.endswith(".x3d"):
            continue
        rel = "x3d/" + name
        try:
            root = ET.parse(os.path.join(folder, name)).getroot()
            st = State(folder)
            sc = root.find("Scene")
            defs = {}
            for c in (sc if sc is not None else []):
                walk(c, ident(), st, defs)
            rows.append((rel, "tris", str(st.tris)))
            rows.append((rel, "round", "1" if st.round else "0"))
            if st.tris:
                rows.append((rel, "min", ",".join(f"{v:.6g}" for v in st.lo)))
                rows.append((rel, "max", ",".join(f"{v:.6g}" for v in st.hi)))
        except Exception as e:  # noqa: BLE001
            rows.append((rel, "error", type(e).__name__))
    os.makedirs(os.path.join(out, "oracle"), exist_ok=True)
    with open(os.path.join(out, "oracle", "x3d.tsv"), "w") as f:
        for r in rows:
            f.write("\t".join(r) + "\n")
    print(f"x3d oracle: {len(rows)} rows")


main()
