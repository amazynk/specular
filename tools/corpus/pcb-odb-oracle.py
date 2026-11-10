#!/usr/bin/env python3
"""Independent counts of ODB++ jobs (.tgz) for specular-pcb's corpus test (tests/eng2_corpus.rs).

For every .tgz under CORPUS/odb: the step shown, the board layers that have features, feature
records by type (pads, lines, arcs, surfaces, texts; user symbols counted once where used),
components, drill holes and slots, and the profile's size in mm. Written with the standard
library (tarfile), separately from the Rust reader.

    python3 -I tools/corpus/pcb-odb-oracle.py CORPUS > CORPUS/odb-oracle.json
"""

import json
import math
import os
import re
import subprocess
import sys
import tarfile


def read_job(path):
    files = {}
    with tarfile.open(path, "r:gz") as t:
        for m in t.getmembers():
            if not m.isfile():
                continue
            data = t.extractfile(m).read()
            name = m.name
            if name.lower().endswith(".z") and data[:2] == b"\x1f\x9d":
                # Unix compress: the system's gzip reads it.
                data = subprocess.run(["gzip", "-dc"], input=data, capture_output=True,
                                      check=True).stdout
                name = name[:-2]
            files[name.lower()] = data.decode("latin-1")
    prefix = ""
    for n in files:
        i = n.find("matrix/matrix")
        if i >= 0 and (i == 0 or n[i - 1] == "/"):
            prefix = n[:i]
            break
    return {n[len(prefix):]: v for n, v in files.items() if n.startswith(prefix)}


def blocks(text):
    out, cur = [], None
    for line in text.splitlines():
        l = line.strip()
        if l.endswith("{"):
            cur = (l[:-1].strip().upper(), {})
        elif l == "}" and cur:
            out.append(cur)
            cur = None
        elif cur and "=" in l:
            k, v = l.split("=", 1)
            cur[1][k.strip().upper()] = v.strip()
    return out


def units(text):
    for l in text.splitlines()[:50]:
        u = l.strip().upper()
        if u in ("UNITS=MM", "U MM"):
            return 1.0
        if u in ("UNITS=INCH", "U INCH"):
            return 25.4
    return 25.4


def records(text):
    for line in text.splitlines():
        line = line.split(";")[0].strip()
        if line and not line.startswith("#"):
            yield line.split()


def main():
    root = sys.argv[1]
    res = {}
    for d, _, fs in sorted(os.walk(os.path.join(root, "odb"))):
        for f in sorted(fs):
            if f.endswith(".tgz"):
                res[f] = job(read_job(os.path.join(d, f)))
                print(f, res[f], file=sys.stderr)
    json.dump(res, sys.stdout, indent=1)


def job(files):
    bl = blocks(files["matrix/matrix"])
    steps = sorted((int(f.get("COL", 0)), f["NAME"]) for n, f in bl if n == "STEP" and "NAME" in f)
    steps = [s for _, s in steps]
    step = next((s for s in steps if s.lower() in ("pcb", "board", "single") or
                 s.lower().startswith("pcb")), None)
    if step is None:
        step = next((s for s in steps if "STEP-REPEAT" not in
                     files.get(f"steps/{s.lower()}/stephdr", "").upper()), steps[0])
    st = step.lower()
    layers = [f for n, f in bl if n == "LAYER" and f.get("CONTEXT", "").upper() == "BOARD"]
    c = dict(pads=0, lines=0, arcs=0, surfaces=0, texts=0, holes=0, layers=0, components=0)
    used = set()
    counted = set()

    def count(text, drill=False):
        syms = {}
        for t in records(text):
            if t[0].startswith("$") and t[0][1:].isdigit():
                syms[t[0][1:]] = t[1].lower()
        for t in records(text):
            r = t[0]
            key = {"P": "pads", "L": "lines", "A": "arcs", "S": "surfaces", "T": "texts",
                   "B": "texts"}.get(r)
            if key:
                c[key] += 1
                if drill and r in "PLAS":
                    c["holes"] += 1
            sym = None
            if r == "P" and len(t) > 3:
                sym = t[4] if t[3] == "-1" and len(t) > 4 else t[3]
            elif r == "L" and len(t) > 5:
                sym = t[5]
            elif r == "A" and len(t) > 7:
                sym = t[7]
            if sym in syms:
                used.add(syms[sym])
        # User symbols used here, once each (and theirs).
        while True:
            more = [u for u in used if u not in counted and f"symbols/{u}/features" in files]
            if not more:
                break
            for u in more:
                if u in counted:
                    continue
                counted.add(u)
                count(files[f"symbols/{u}/features"])

    for l in layers:
        ty = l.get("TYPE", "").upper()
        name = l.get("NAME", "").lower()
        if ty == "COMPONENT":
            comp = files.get(f"steps/{st}/layers/{name}/components", "")
            c["components"] += sum(1 for t in records(comp) if t[0] == "CMP")
            continue
        feat = files.get(f"steps/{st}/layers/{name}/features")
        if feat is None:
            continue
        c["layers"] += 1
        count(feat, drill=(ty == "DRILL"))
    out = dict(step=step, **c)
    prof = files.get(f"steps/{st}/profile")
    if prof:
        k = units(prof)
        pts, at = [], None
        for t in records(prof):
            if t[0] in ("OB", "OS") and len(t) >= 3:
                at = (float(t[1]) * k, float(t[2]) * k)
                pts.append(at)
            elif t[0] == "OC" and len(t) >= 6 and at:
                x, y, cx, cy = (float(v) * k for v in t[1:5])
                cw = t[5].upper() == "Y"
                r = math.hypot(at[0] - cx, at[1] - cy)
                a0 = math.atan2(at[1] - cy, at[0] - cx)
                a1 = math.atan2(y - cy, x - cx)
                sw = a1 - a0
                if cw:
                    while sw >= 0:
                        sw -= 2 * math.pi
                else:
                    while sw <= 0:
                        sw += 2 * math.pi
                n = max(2, int(abs(math.degrees(sw)) * 10))
                pts += [(cx + r * math.cos(a0 + sw * i / n), cy + r * math.sin(a0 + sw * i / n))
                        for i in range(n + 1)]
                at = (x, y)
        if pts:
            xs = [p[0] for p in pts]
            ys = [p[1] for p in pts]
            out["profile_mm"] = [max(xs) - min(xs), max(ys) - min(ys)]
    return out


if __name__ == "__main__":
    main()
