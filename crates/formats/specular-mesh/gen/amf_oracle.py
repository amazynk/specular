#!/usr/bin/env python3
"""Oracle facts for the AMF corpus (tools/corpus/models-extra/amf): a small reader of plain AMF
written apart from the Rust one (standard library only), and assimp (assimp-py).

    python -I crates/formats/specular-mesh/gen/amf_oracle.py tools/corpus/models-extra

Writes OUT/oracle/amf.tsv with one row per file and fact:
    relpath  key  value
keys: error, tris, min, max from the small reader (every volume, constellations expanded:
rotation about x, then y, then z, in degrees, then the offset); assimp_tris, assimp_min,
assimp_max from assimp (its root node transform not applied). assimp reads only the first
volume of an object and places constellations its own way, so its facts are compared only
for files with neither.
"""
import math
import os
import sys


def facts(path, rel, rows):
    import assimp_py
    import numpy as np

    flags = assimp_py.Process_Triangulate | assimp_py.Process_SortByPType
    scene = assimp_py.import_file(path, flags)
    tris = 0
    lo = hi = None
    meshes = scene.meshes

    def walk(node, parent, is_root):
        nonlocal tris, lo, hi
        world = parent if is_root else parent @ np.array(node.transformation, float)
        for mi in node.mesh_indices:
            mesh = meshes[mi]
            if mesh.num_indices != 3 * mesh.num_faces:
                continue
            idx = np.asarray(mesh.indices, dtype=np.int64)
            v = np.asarray(mesh.vertices, dtype=float).reshape(-1, 3)
            tris += mesh.num_faces
            used = np.unique(idx)
            if len(used) == 0:
                continue
            p = v[used] @ world[:3, :3].T + world[:3, 3]
            a, b = p.min(0).tolist(), p.max(0).tolist()
            lo = a if lo is None else [min(x, y) for x, y in zip(lo, a)]
            hi = b if hi is None else [max(x, y) for x, y in zip(hi, b)]
        for c in node.children:
            walk(c, world, False)

    walk(scene.root_node, np.eye(4), True)
    rows.append((rel, "assimp_tris", str(tris)))
    if lo is not None and all(math.isfinite(x) for x in lo + hi):
        rows.append((rel, "assimp_min", ",".join(f"{x:.6g}" for x in lo)))
        rows.append((rel, "assimp_max", ",".join(f"{x:.6g}" for x in hi)))


def own_facts(path, rel, rows):
    import xml.etree.ElementTree as ET

    root = ET.parse(path).getroot()
    if root.tag != "amf":
        raise ValueError("not amf")
    num = lambda e, k, d=0.0: float(e.findtext(k)) if e is not None and e.findtext(k) else d
    objects = {}
    for o in root.findall("object"):
        verts = [[num(v.find("coordinates"), k) for k in "xyz"]
                 for v in o.findall("mesh/vertices/vertex")]
        tris = []
        for vol in o.findall("mesh/volume"):
            for t in vol.findall("triangle"):
                ix = [int(t.findtext(k)) for k in ("v1", "v2", "v3")]
                if all(0 <= i < len(verts) for i in ix):
                    tris.append(ix)
        objects[o.get("id")] = (verts, tris)
    cons = {c.get("id"): c.findall("instance") for c in root.findall("constellation")}

    def rot(rx, ry, rz):
        a, b, c = (math.radians(v) for v in (rx, ry, rz))
        X = [[1, 0, 0], [0, math.cos(a), -math.sin(a)], [0, math.sin(a), math.cos(a)]]
        Y = [[math.cos(b), 0, math.sin(b)], [0, 1, 0], [-math.sin(b), 0, math.cos(b)]]
        Z = [[math.cos(c), -math.sin(c), 0], [math.sin(c), math.cos(c), 0], [0, 0, 1]]
        mul = lambda p, q: [[sum(p[r][k] * q[k][j] for k in range(3)) for j in range(3)]
                            for r in range(3)]
        return mul(Z, mul(Y, X))

    def compose(outer, inner):
        (r1, t1), (r2, t2) = outer, inner
        r = [[sum(r1[i][k] * r2[k][j] for k in range(3)) for j in range(3)] for i in range(3)]
        t = [sum(r1[i][k] * t2[k] for k in range(3)) + t1[i] for i in range(3)]
        return r, t

    ident = ([[1, 0, 0], [0, 1, 0], [0, 0, 1]], [0, 0, 0])
    placed = []

    def expand(cid, xf, depth):
        for i in cons.get(cid, []):
            x = compose(xf, (rot(num(i, "rx"), num(i, "ry"), num(i, "rz")),
                             [num(i, "deltax"), num(i, "deltay"), num(i, "deltaz")]))
            oid = i.get("objectid")
            if oid in cons and depth < 16:
                expand(oid, x, depth + 1)
            else:
                placed.append((oid, x))

    if cons:
        inner = {i.get("objectid") for v in cons.values() for i in v}
        for cid in sorted(c for c in cons if c not in inner):
            expand(cid, ident, 0)
    else:
        placed = [(o.get("id"), ident) for o in root.findall("object")]
    n, lo, hi = 0, [math.inf] * 3, [-math.inf] * 3
    for oid, (r, t) in placed:
        if oid not in objects:
            continue
        verts, tris = objects[oid]
        n += len(tris)
        for tri in tris:
            for i in tri:
                p = verts[i]
                q = [sum(r[a][k] * p[k] for k in range(3)) + t[a] for a in range(3)]
                lo = [min(x, y) for x, y in zip(lo, q)]
                hi = [max(x, y) for x, y in zip(hi, q)]
    rows.append((rel, "tris", str(n)))
    if n:
        rows.append((rel, "min", ",".join(f"{x:.6g}" for x in lo)))
        rows.append((rel, "max", ",".join(f"{x:.6g}" for x in hi)))
    multi = any(len(o.findall("mesh/volume")) > 1 for o in root.findall("object"))
    rows.append((rel, "assimp_comparable", "0" if multi or cons else "1"))


def main():
    out = sys.argv[1]
    folder = os.path.join(out, "amf")
    rows = []
    for name in sorted(os.listdir(folder)):
        if not name.endswith(".amf"):
            continue
        rel = "amf/" + name
        for job in (own_facts, facts):
            try:
                job(os.path.join(folder, name), rel, rows)
            except Exception as e:  # noqa: BLE001
                rows.append((rel, "error", f"{job.__name__}: {type(e).__name__}"))
    os.makedirs(os.path.join(out, "oracle"), exist_ok=True)
    with open(os.path.join(out, "oracle", "amf.tsv"), "w") as f:
        for r in rows:
            f.write("\t".join(r) + "\n")
    print(f"amf oracle: {len(rows)} rows")


main()
