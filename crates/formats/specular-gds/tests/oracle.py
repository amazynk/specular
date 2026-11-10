#!/usr/bin/env python3
"""The gdstk oracle: writes what gdstk reads from a GDSII file, for tests/oracle.rs.

    oracle.py FILE.gds OUT.expect [--max-polygons N]

Lines are tab-separated: the cells and top cells, the shapes per layer/datatype of each
top cell with every reference expanded (counted through the hierarchy, never expanded),
the top cells' bounds in database units, and, when a file expands to at most N shapes
(300 000 by default), every expanded polygon in a canonical form (consecutive duplicate
and collinear vertices removed, counter-clockwise, starting at the smallest vertex,
coordinates rounded to 0.01 database units). tests/oracle.rs computes the same with
specular-gds and compares. gdstk ignores absolute magnification and rotation, so files
using them are not compared.
"""

import sys
from collections import Counter

import gdstk


def r2(v):
    s = "%.2f" % v
    return "0.00" if s == "-0.00" else s


def canon(pts):
    out = []
    for x, y in pts:
        q = (float(x), float(y))
        if not out or abs(q[0] - out[-1][0]) > 1e-9 or abs(q[1] - out[-1][1]) > 1e-9:
            out.append(q)
    while len(out) > 1 and abs(out[0][0] - out[-1][0]) <= 1e-9 and abs(out[0][1] - out[-1][1]) <= 1e-9:
        out.pop()
    i = 0
    while len(out) >= 3 and i < len(out):
        n = len(out)
        a, b, c = out[i - 1], out[i], out[(i + 1) % n]
        ux, uy = b[0] - a[0], b[1] - a[1]
        vx, vy = c[0] - b[0], c[1] - b[1]
        lu = (ux * ux + uy * uy) ** 0.5
        lv = (vx * vx + vy * vy) ** 0.5
        if abs(ux * vy - uy * vx) <= 1e-9 * lu * lv:
            del out[i]
            i = max(i - 1, 0)
        else:
            i += 1
    area = 0.0
    for k in range(len(out)):
        x0, y0 = out[k - 1]
        x1, y1 = out[k]
        area += x0 * y1 - x1 * y0
    if area < 0:
        out.reverse()
    s = [(r2(x), r2(y)) for x, y in out]
    if not s:
        return ""
    k = min(range(len(s)), key=lambda j: (float(s[j][0]), float(s[j][1])))
    s = s[k:] + s[:k]
    return ";".join("%s,%s" % p for p in s)


def main():
    path, out_path = sys.argv[1], sys.argv[2]
    max_polys = 300_000
    if "--max-polygons" in sys.argv:
        max_polys = int(sys.argv[sys.argv.index("--max-polygons") + 1])
    lib = gdstk.read_gds(path)
    f = lib.unit / lib.precision
    memo = {}

    def counts(cell, stack=()):
        if cell.name in memo:
            return memo[cell.name]
        c = Counter()
        for p in cell.polygons:
            c[(p.layer, p.datatype)] += 1
        for p in cell.paths:
            for l, d in zip(p.layers, p.datatypes):
                c[(l, d)] += 1
        for r in cell.references:
            if isinstance(r.cell, str) or r.cell.name in stack:
                continue
            n = r.repetition.size if r.repetition.size > 0 else 1
            for k, v in counts(r.cell, stack + (cell.name,)).items():
                c[k] += v * n
        memo[cell.name] = c
        return c

    lines = ["cells\t%d" % len(lib.cells)]
    lines += ["cell\t%s" % n for n in sorted(c.name for c in lib.cells)]
    tops = sorted(lib.top_level(), key=lambda c: c.name)
    lines += ["top\t%s" % c.name for c in tops]
    total = 0
    per_top = []
    for t in tops:
        c = counts(t)
        per_top.append(c)
        total += sum(c.values())
        for (l, d), n in sorted(c.items()):
            lines.append("count\t%s\t%d\t%d\t%d" % (t.name, l, d, n))
    exact = total <= max_polys
    for t in tops:
        if exact:
            polys = t.get_polygons(apply_repetitions=True, include_paths=True)
            keys = []
            x0 = y0 = float("inf")
            x1 = y1 = float("-inf")
            for p in polys:
                pts = p.points * f
                if len(pts):
                    x0 = min(x0, pts[:, 0].min())
                    x1 = max(x1, pts[:, 0].max())
                    y0 = min(y0, pts[:, 1].min())
                    y1 = max(y1, pts[:, 1].max())
                keys.append("poly\t%s\t%d\t%d\t%s" % (t.name, p.layer, p.datatype, canon(pts)))
            if keys:
                lines.append("bbox\t%s\t%.3f\t%.3f\t%.3f\t%.3f" % (t.name, x0, y0, x1, y1))
            lines.append("polys\t%s\t%d" % (t.name, len(keys)))
            lines += sorted(keys)
        else:
            bb = t.bounding_box()
            if bb is not None:
                (x0, y0), (x1, y1) = bb
                lines.append("bbox\t%s\t%.3f\t%.3f\t%.3f\t%.3f" % (t.name, x0 * f, y0 * f, x1 * f, y1 * f))
    with open(out_path, "w") as fh:
        fh.write("\n".join(lines) + "\n")


if __name__ == "__main__":
    main()
