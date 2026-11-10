#!/usr/bin/env python3
"""Oracle facts for the IFC corpus (tools/corpus/models-extra/ifc): every element IfcOpenShell
draws, with its triangle count, volume, area and bounds, in metres (IfcOpenShell's own
output unit). IfcOpenShell is LGPL and used here only as a test oracle; Specular never
links it.

Run with `python -I` from the venv described in tools/corpus/fetch-models-extra.sh:

    python -I crates/formats/specular-mesh/gen/ifc_oracle.py tools/corpus/models-extra

Writes OUT/oracle/ifc.tsv with rows
    relpath  id  type  tris  volume  area  min  max
(`id` the STEP instance number; volume empty when IfcOpenShell's shape is not closed), and
rows `relpath  -  error  message` for files it cannot read. Files over MAX_BYTES are
skipped.
"""
import multiprocessing
import os
import subprocess
import sys

import ifcopenshell
import ifcopenshell.geom
import ifcopenshell.util.shape

MAX_BYTES = 60 << 20
TIMEOUT = 900


def fmt(v):
    return ",".join(f"{x:.6g}" for x in v)


def closed(faces):
    edges = {}
    for k in range(0, len(faces), 3):
        t = faces[k:k + 3]
        for e in range(3):
            a, b = t[e], t[(e + 1) % 3]
            key = (min(a, b), max(a, b))
            edges[key] = edges.get(key, 0) + (1 if a < b else -1)
    return all(v == 0 for v in edges.values())


def one(path, rel, rows):
    f = ifcopenshell.open(path)
    s = ifcopenshell.geom.settings()
    s.set("use-world-coords", True)
    s.set("weld-vertices", True)
    it = ifcopenshell.geom.iterator(s, f, max(1, multiprocessing.cpu_count() // 2))
    if not it.initialize():
        rows.append((rel, "-", "error", "nothing to draw"))
        return
    while True:
        sh = it.get()
        g = sh.geometry
        v = g.verts
        faces = g.faces
        if len(faces) >= 3:
            xs, ys, zs = v[0::3], v[1::3], v[2::3]
            vol = ""
            if closed(faces):
                try:
                    vol = f"{ifcopenshell.util.shape.get_volume(g):.6g}"
                except Exception:
                    vol = ""
            area = ifcopenshell.util.shape.get_area(g)
            rows.append((rel, str(sh.id), sh.type, str(len(faces) // 3), vol, f"{area:.6g}",
                         fmt([min(xs), min(ys), min(zs)]), fmt([max(xs), max(ys), max(zs)])))
        if not it.next():
            break


def child(path, rel):
    rows = []
    try:
        one(path, rel, rows)
    except Exception as e:
        rows.append((rel, "-", "error", (str(e) or type(e).__name__).replace("\t", " ").replace("\n", " ")[:200]))
    for r in rows:
        print("\t".join(r))


def main():
    if sys.argv[1] == "--one":
        child(sys.argv[2], sys.argv[3])
        return
    root = sys.argv[1]
    ifc = os.path.join(root, "ifc")
    rows = []
    for dirpath, _, files in os.walk(ifc):
        for name in sorted(files):
            if not name.lower().endswith((".ifc", ".ifczip")):
                continue
            path = os.path.join(dirpath, name)
            rel = os.path.relpath(path, root)
            if os.path.getsize(path) > MAX_BYTES:
                continue
            # Each file in a process of its own: a crash or a hang costs that file only.
            try:
                out = subprocess.run([sys.executable, "-I", __file__, "--one", path, rel],
                                     capture_output=True, text=True, timeout=TIMEOUT)
                got = [tuple(l.split("\t")) for l in out.stdout.splitlines() if l.strip()]
                if out.returncode != 0 and not got:
                    got = [(rel, "-", "error", f"IfcOpenShell exited with {out.returncode}")]
                rows.extend(got)
            except subprocess.TimeoutExpired:
                rows.append((rel, "-", "error", f"over {TIMEOUT} s"))
            print(rel, file=sys.stderr)
    os.makedirs(os.path.join(root, "oracle"), exist_ok=True)
    with open(os.path.join(root, "oracle", "ifc.tsv"), "w") as out:
        for r in rows:
            out.write("\t".join(r) + "\n")


if __name__ == "__main__":
    main()
