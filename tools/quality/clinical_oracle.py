#!/usr/bin/env python3
"""Oracle for the vet, orthopaedic and cephalometric geometry (specular-measure `clinical`).

Recomputes every readout from the dumped points with numpy, written independently of the
Rust code: circles by least squares (x² + y² = 2ax + 2by + c), angles from arccos of unit
vectors, line distances by projection, anterior sides from sella's side.

    SPECULAR_ORACLE_DIR=<dir> cargo test -p specular-measure clinical_dump -- --ignored
    python3 -I tools/quality/clinical_oracle.py <dir>
"""
import json
import pathlib
import sys

import numpy as np

TOL = 1e-6


def circle(p):
    p = np.asarray(p)
    a = np.column_stack([2 * p[:, 0], 2 * p[:, 1], np.ones(len(p))])
    b = (p**2).sum(axis=1)
    (cx, cy, c), *_ = np.linalg.lstsq(a, b, rcond=None)
    return np.array([cx, cy]), float(np.sqrt(c + cx * cx + cy * cy))


def ang(u, w):
    u, w = np.asarray(u, float), np.asarray(w, float)
    c = np.dot(u, w) / np.linalg.norm(u) / np.linalg.norm(w)
    return float(np.degrees(np.arccos(np.clip(c, -1.0, 1.0))))


def line_ang(u, w):
    a = ang(u, w)
    return min(a, 180.0 - a)


def dist_line(p, a, b):
    d = (b - a) / np.linalg.norm(b - a)
    v = p - a
    return float(np.linalg.norm(v - np.dot(v, d) * d))


def anterior(p, a, b, back):
    """Signed distance of p from line ab, positive on the side away from `back`."""
    d = (b - a) / np.linalg.norm(b - a)
    nrm = np.array([-d[1], d[0]])
    s = np.dot(nrm, p - a)
    return float(s if np.dot(nrm, back - a) < 0 else -s)


def vert(length, marks):
    steps = np.linalg.norm(np.diff(marks, axis=0), axis=1)
    cum = np.concatenate([[0.0], np.cumsum(steps)])
    if length <= cum[-1]:
        return float(np.interp(length, cum, np.arange(len(cum))))
    return float(len(steps) + (length - cum[-1]) / steps.mean())


def expected(t, q):
    e = {}
    if t == "vhs":
        lv = vert(np.linalg.norm(q[1] - q[0]), q[4:])
        sv = vert(np.linalg.norm(q[3] - q[2]), q[4:])
        e.update({"vhs": lv + sv, "long-axis": lv, "short-axis": sv})
        e["long-axis-length"] = np.linalg.norm(q[1] - q[0])
        e["short-axis-length"] = np.linalg.norm(q[3] - q[2])
    elif t == "vlas":
        e["vlas"] = vert(np.linalg.norm(q[1] - q[0]), q[2:])
        e["vlas-length"] = np.linalg.norm(q[1] - q[0])
    elif t == "norberg":
        cl, _ = circle(q[0:3])
        cr, _ = circle(q[3:6])
        e["norberg-left"] = ang(cr - cl, q[6] - cl)
        e["norberg-right"] = ang(cl - cr, q[7] - cr)
    elif t == "tpa":
        e["tpa"] = 90.0 - ang(q[0] - q[1], q[2] - q[3])
    elif t == "distraction-index":
        ch, rh = circle(q[0:3])
        ca, _ = circle(q[3:6])
        e["distraction-index"] = np.linalg.norm(ca - ch) / rh
        e["centre-distance"] = np.linalg.norm(ca - ch)
        e["head-radius"] = rh
    elif t == "femoral-head":
        e["head-diameter"] = 2 * circle(q[0:3])[1]
    elif t == "femoral-offset":
        c, r = circle(q[0:3])
        e["offset"] = dist_line(c, q[3], q[4])
        e["head-diameter"] = 2 * r
    elif t == "leg-length":
        left, right = dist_line(q[2], q[0], q[1]), dist_line(q[3], q[0], q[1])
        e.update({"leg-left": left, "leg-right": right, "leg-difference": left - right})
    elif t == "acetabular-angle":
        e["acetabular-angle"] = ang(q[0] - q[1], q[2] - q[0])
    elif t == "steiner":
        s, n, a, b, pog, go, gn, u1t, u1a, l1t, l1a, ocf, ocb = q
        e["sna"], e["snb"] = ang(s - n, a - n), ang(s - n, b - n)
        e["anb"] = e["sna"] - e["snb"]
        e["sn-gogn"] = line_ang(n - s, gn - go)
        e["u1-na-angle"] = line_ang(u1t - u1a, a - n)
        e["u1-na-mm"] = anterior(u1t, n, a, s)
        e["l1-nb-angle"] = line_ang(l1t - l1a, b - n)
        e["l1-nb-mm"] = anterior(l1t, n, b, s)
        e["pog-nb-mm"] = anterior(pog, n, b, s)
        e["interincisal"] = ang(u1a - u1t, l1a - l1t)
        e["occlusal-sn"] = line_ang(n - s, ocf - ocb)
    elif t == "downs":
        po, orb, s, n, a, b, pog, gn, go, me, u1t, u1a, l1t, l1a, ocf, ocb = q
        e["facial-angle"] = ang(po - orb, pog - n)
        conv = 180.0 - ang(n - a, pog - a)
        e["convexity"] = conv if anterior(a, n, pog, s) >= 0 else -conv
        ab = line_ang(b - a, pog - n)
        ahead = anterior(b, n, pog, s) - anterior(a, n, pog, s)
        e["ab-plane"] = ab if ahead >= 0 else -ab
        e["mandibular-plane"] = line_ang(orb - po, me - go)
        e["y-axis"] = ang(gn - s, orb - po)
        e["occlusal-cant"] = line_ang(orb - po, ocf - ocb)
        e["interincisal"] = ang(u1a - u1t, l1a - l1t)
        e["l1-occlusal"] = ang(l1t - l1a, ocb - ocf) - 90.0
        e["l1-mandibular"] = ang(l1t - l1a, go - me) - 90.0
        e["u1-apog-mm"] = anterior(u1t, a, pog, s)
    return e


def main(d: pathlib.Path) -> int:
    cases = json.loads((d / "clinical.json").read_text())
    values = mismatches = 0
    per = {}
    for c in cases:
        sx, sy = c["spacing"]
        q = np.array(c["points"], float) * np.array([sx, sy])
        e = expected(c["template"], q)
        got = c["readouts"]
        if set(e) != set(got):
            print(f"{c['template']}: keys differ {sorted(set(e) ^ set(got))}")
            mismatches += 1
            continue
        for k, v in e.items():
            values += 1
            # Relative for large values, absolute near zero.
            if abs(got[k] - v) > TOL * max(1.0, abs(v)):
                mismatches += 1
                print(f"{c['template']} {k}: specular {got[k]!r}, numpy {v!r}")
        per[c["template"]] = per.get(c["template"], 0) + 1
    print(f"{len(cases)} cases ({per}), {values} values, {mismatches} mismatches")
    return 1 if mismatches else 0


if __name__ == "__main__":
    sys.exit(main(pathlib.Path(sys.argv[1])))
