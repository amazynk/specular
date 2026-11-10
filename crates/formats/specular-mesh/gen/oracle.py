#!/usr/bin/env python3
"""The trimesh oracle for the mesh corpus (tools/corpus/fetch-mesh.sh).

Usage: oracle.py CORPUS_DIR

Writes CORPUS_DIR/expected.tsv: for every model trimesh reads as triangles,
    path<TAB>triangles<TAB>minx miny minz<TAB>maxx maxy maxz
with the whole scene flattened (node and build transforms applied) and nothing merged or
dropped (process=False), which is what specular-mesh reports. Files trimesh cannot read, and
point clouds, are left out; the corpus test only checks that we do not panic on them.
"""

import os
import sys

import numpy as np
import trimesh

EXTS = {".stl", ".obj", ".ply", ".gltf", ".glb", ".3mf"}


def flatten(loaded):
    if isinstance(loaded, trimesh.Scene):
        tris = []
        for name in loaded.graph.nodes_geometry:
            transform, geom = loaded.graph[name]
            g = loaded.geometry[geom]
            if not isinstance(g, trimesh.Trimesh) or len(g.faces) == 0:
                continue
            v = trimesh.transform_points(np.asarray(g.vertices, dtype=np.float64), transform)
            tris.append(v[np.asarray(g.faces)])
        if not tris:
            return None
        return np.concatenate(tris)
    if isinstance(loaded, trimesh.Trimesh) and len(loaded.faces):
        return np.asarray(loaded.vertices, dtype=np.float64)[np.asarray(loaded.faces)]
    return None


def main():
    root = sys.argv[1]
    rows = []
    for d, _, files in os.walk(root):
        for f in sorted(files):
            if os.path.splitext(f)[1].lower() not in EXTS:
                continue
            path = os.path.join(d, f)
            rel = os.path.relpath(path, root)
            try:
                loaded = trimesh.load(path, process=False, maintain_order=True)
                tris = flatten(loaded)
            except Exception as e:  # noqa: BLE001 - any failure leaves the file out
                print(f"  oracle: {rel}: {type(e).__name__}", file=sys.stderr)
                continue
            if tris is None:
                continue
            lo = tris.reshape(-1, 3).min(axis=0)
            hi = tris.reshape(-1, 3).max(axis=0)
            rows.append(
                f"{rel}\t{len(tris)}\t{lo[0]:.6g} {lo[1]:.6g} {lo[2]:.6g}"
                f"\t{hi[0]:.6g} {hi[1]:.6g} {hi[2]:.6g}\n"
            )
    rows.sort()
    with open(os.path.join(root, "expected.tsv"), "w") as out:
        out.writelines(rows)
    print(f"  oracle: {len(rows)} models -> expected.tsv")


if __name__ == "__main__":
    main()
