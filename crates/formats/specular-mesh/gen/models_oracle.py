#!/usr/bin/env python3
"""Oracle facts for the 3D and CAD corpus (tools/corpus/models): USD through usd-core (pxr),
COLLADA through pycollada, 3DS, LightWave and FBX (and COLLADA again) through assimp.

Run with `python -I` from the venv described in tools/corpus/fetch-models.sh:

    python -I crates/formats/specular-mesh/gen/models_oracle.py tools/corpus/models

Writes OUT/oracle/models.tsv with one row per file and fact:
    relpath  reader  key  value
keys: error, tris (faces with three or more corners, triangulated), meshes, min, max (of the
transformed vertices that faces use, in the file's own frame and units), up, unit,
implicit (USD implicit shapes or instancers present: triangle counts not comparable),
first (three transformed vertices of the first mesh).
"""
import math
import os
import sys
import traceback


def fmt(v):
    return ",".join(f"{x:.6g}" for x in v)


def bounds_rows(rel, reader, lo, hi, rows):
    if lo is not None and all(math.isfinite(x) for x in lo + hi):
        rows.append((rel, reader, "min", fmt(lo)))
        rows.append((rel, reader, "max", fmt(hi)))


def usd_facts(root, rel, rows):
    from pxr import Usd, UsdGeom

    stage = Usd.Stage.Open(os.path.join(root, rel))
    t = Usd.TimeCode.EarliestTime()
    cache = UsdGeom.XformCache(t)
    tris = 0
    meshes = 0
    implicit = False
    lo = hi = None
    first = None
    it = iter(Usd.PrimRange(stage.GetPseudoRoot(),
                            Usd.TraverseInstanceProxies(Usd.PrimDefaultPredicate)))
    for prim in it:
        img = UsdGeom.Imageable(prim)
        if img:
            if img.ComputeVisibility(t) == UsdGeom.Tokens.invisible:
                it.PruneChildren()
                continue
            purpose = img.GetPurposeAttr().Get(t)
            if purpose in (UsdGeom.Tokens.guide, UsdGeom.Tokens.proxy):
                it.PruneChildren()
                continue
        tn = prim.GetTypeName()
        if tn == "PointInstancer":
            implicit = True
            it.PruneChildren()
            continue
        if tn in ("Cube", "Sphere", "Cylinder", "Cone", "Capsule", "Plane"):
            implicit = True
            continue
        if tn != "Mesh":
            continue
        m = UsdGeom.Mesh(prim)
        pts = m.GetPointsAttr().Get(t)
        counts = m.GetFaceVertexCountsAttr().Get(t)
        idx = m.GetFaceVertexIndicesAttr().Get(t)
        if pts is None or counts is None or idx is None or len(pts) == 0:
            continue
        meshes += 1
        xf = cache.GetLocalToWorldTransform(prim)
        used = set()
        at = 0
        for c in counts:
            if c >= 3 and at + c <= len(idx) and all(0 <= idx[at + k] < len(pts) for k in range(c)):
                tris += c - 2
                used.update(idx[at:at + c])
            at += c
        for i in sorted(used):
            p = xf.Transform(pts[i])
            q = [p[0], p[1], p[2]]
            if lo is None:
                lo, hi = list(q), list(q)
            else:
                lo = [min(a, b) for a, b in zip(lo, q)]
                hi = [max(a, b) for a, b in zip(hi, q)]
        if first is None and len(pts) >= 3:
            first = [xf.Transform(pts[k]) for k in range(3)]
    rows.append((rel, "pxr", "tris", str(tris)))
    rows.append((rel, "pxr", "meshes", str(meshes)))
    rows.append((rel, "pxr", "implicit", "1" if implicit else "0"))
    rows.append((rel, "pxr", "up", str(UsdGeom.GetStageUpAxis(stage))))
    rows.append((rel, "pxr", "unit", f"{UsdGeom.GetStageMetersPerUnit(stage):.6g}"))
    bounds_rows(rel, "pxr", lo, hi, rows)
    if first is not None:
        rows.append((rel, "pxr", "first", ";".join(fmt(p) for p in first)))


def assimp_facts(root, rel, rows):
    import assimp_py
    import numpy as np

    # Triangulated, split by primitive type; node transforms applied by walking the tree
    # here, but not the root node's (assimp puts its own axis conversion there; our
    # readers keep the file's frame).
    flags = assimp_py.Process_Triangulate | assimp_py.Process_SortByPType
    scene = assimp_py.import_file(os.path.join(root, rel), flags)
    tris = 0
    lo = hi = None
    meshes = scene.meshes

    def walk(node, parent, is_root):
        nonlocal tris, lo, hi
        if is_root:
            world = parent
        else:
            m = np.array(node.transformation, float)
            world = parent @ m
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
    rows.append((rel, "assimp", "tris", str(tris)))
    rows.append((rel, "assimp", "meshes", str(len(meshes))))
    bounds_rows(rel, "assimp", lo, hi, rows)


def collada_facts(root, rel, rows):
    import collada
    import numpy as np

    d = collada.Collada(os.path.join(root, rel), ignore=[collada.common.DaeUnsupportedError,
                                                           collada.common.DaeBrokenRefError])
    tris = 0
    lo = hi = None
    if d.scene is not None:
        for geom in d.scene.objects("geometry"):
            for prim in geom.primitives():
                if isinstance(prim, (collada.triangleset.BoundTriangleSet,
                                     collada.polylist.BoundPolylist,
                                     collada.polygons.BoundPolygons)):
                    t = prim if isinstance(prim, collada.triangleset.BoundTriangleSet) \
                        else prim.triangleset()
                    if len(t) == 0:
                        continue
                    tris += len(t)
                    v = t.vertex[t.vertex_index.reshape(-1)]
                    a, b = v.min(0).tolist(), v.max(0).tolist()
                    lo = a if lo is None else [min(x, y) for x, y in zip(lo, a)]
                    hi = b if hi is None else [max(x, y) for x, y in zip(hi, b)]
    rows.append((rel, "pycollada", "tris", str(tris)))
    up = d.assetInfo.upaxis if d.assetInfo is not None else None
    if up:
        rows.append((rel, "pycollada", "up", str(up)))
    if d.assetInfo is not None and d.assetInfo.unitmeter:
        rows.append((rel, "pycollada", "unit", f"{d.assetInfo.unitmeter:.6g}"))
    bounds_rows(rel, "pycollada", lo, hi, rows)


def main(root):
    rows = []
    for dirpath, _, names in os.walk(root):
        for n in sorted(names):
            path = os.path.join(dirpath, n)
            rel = os.path.relpath(path, root)
            if rel.startswith("oracle") or rel.startswith("splats"):
                continue
            ext = n.lower().rsplit(".", 1)[-1]
            jobs = []
            if ext in ("usd", "usda", "usdc", "usdz"):
                jobs.append(("pxr", usd_facts))
            if ext in ("dae",):
                jobs.append(("pycollada", collada_facts))
            if ext in ("dae", "3ds", "lwo", "fbx"):
                jobs.append(("assimp", assimp_facts))
            for reader, job in jobs:
                try:
                    job(root, rel, rows)
                except Exception as e:  # noqa: BLE001 - every failure is a fact
                    msg = (str(e).splitlines() or [type(e).__name__])[0][:160]
                    rows.append((rel, reader, "error", msg.replace("\t", " ")))
                    if os.environ.get("ORACLE_TRACE"):
                        traceback.print_exc()
    os.makedirs(os.path.join(root, "oracle"), exist_ok=True)
    with open(os.path.join(root, "oracle", "models.tsv"), "w") as f:
        for r in rows:
            f.write("\t".join(r) + "\n")
    print(f"models: {len(rows)} facts")


if __name__ == "__main__":
    main(sys.argv[1])
