#!/usr/bin/env python3
"""The oracle for tests/oracle.rs: counts and outline bounds of every KiCad board and
schematic under a corpus folder, written as JSON lines.

Usage: python -I oracle.py CORPUS_DIR OUT.jsonl

Two independent readings per file: a small S-expression walk written here (counts, the
board outline's centre-line bounds, the schematic hierarchy expanded through its sheet
files) and kiutils (footprints, pads, tracks, vias, zones; symbols, wires, labels, sheets)
where kiutils can read the file. Files older than KiCad 6 are listed as legacy.
"""
import json
import math
import os
import re
import sys

TOKEN = re.compile(r'\s*(?:(\()|(\))|"((?:[^"\\]|\\.)*)"|([^\s()"]+))', re.S)


def parse(text):
    stack = [[]]
    pos = 0
    n = len(text)
    while pos < n:
        m = TOKEN.match(text, pos)
        if not m:
            break
        pos = m.end()
        if m.group(1):
            stack.append([])
        elif m.group(2):
            done = stack.pop()
            stack[-1].append(done)
            if len(stack) == 1:
                return done
        elif m.group(3) is not None:
            stack[-1].append(('s', m.group(3)))
        elif m.group(4) is not None:
            stack[-1].append(m.group(4))
    raise ValueError('unbalanced')


def head(n):
    return n[0] if isinstance(n, list) and n and isinstance(n[0], str) else None


def kids(n, name):
    return [c for c in n[1:] if isinstance(c, list) and head(c) == name]


def kid(n, name):
    k = kids(n, name)
    return k[0] if k else None


def text(a):
    if isinstance(a, tuple):
        return a[1]
    return a


def num(n, i):
    try:
        return float(text(n[i]))
    except (IndexError, ValueError, TypeError):
        return None


def xy(n):
    return (num(n, 1), num(n, 2))


def at(n):
    a = kid(n, 'at')
    if a is None:
        return (0.0, 0.0, 0.0)
    return (num(a, 1) or 0.0, num(a, 2) or 0.0, num(a, 3) or 0.0)


def rot(p, deg):
    # KiCad: y down, angles counter-clockwise on screen.
    r = math.radians(deg)
    x, y = p
    return (x * math.cos(r) + y * math.sin(r), -x * math.sin(r) + y * math.cos(r))


def arc_points(a, m, b, steps=64):
    ax, ay = a
    mx, my = m
    bx, by = b
    d = 2 * (ax * (my - by) + mx * (by - ay) + bx * (ay - my))
    if abs(d) < 1e-12:
        return [a, b]
    a2, m2, b2 = ax * ax + ay * ay, mx * mx + my * my, bx * bx + by * by
    cx = (a2 * (my - by) + m2 * (by - ay) + b2 * (ay - my)) / d
    cy = (a2 * (bx - mx) + m2 * (ax - bx) + b2 * (mx - ax)) / d
    r = math.hypot(ax - cx, ay - cy)
    ang = lambda p: math.atan2(p[1] - cy, p[0] - cx)
    sa, ma, ea = ang(a), ang(m), ang(b)
    norm = lambda x: x % (2 * math.pi)
    ccw = norm(ea - sa)
    sweep = ccw if norm(ma - sa) <= ccw else ccw - 2 * math.pi
    return [(cx + r * math.cos(sa + sweep * i / steps), cy + r * math.sin(sa + sweep * i / steps))
            for i in range(steps + 1)]


def shape_points(n):
    h = head(n)
    kind = h.split('_', 1)[1] if '_' in h else h
    s, e = kid(n, 'start'), kid(n, 'end')
    if kind == 'line' and s and e:
        return [xy(s), xy(e)]
    if kind == 'rect' and s and e:
        return [xy(s), xy(e)]
    if kind == 'circle':
        c, e = kid(n, 'center'), kid(n, 'end')
        if c and e:
            (cx, cy), (ex, ey) = xy(c), xy(e)
            r = math.hypot(ex - cx, ey - cy)
            return [(cx - r, cy - r), (cx + r, cy + r)]
    if kind == 'arc':
        m = kid(n, 'mid')
        if s and m and e:
            return arc_points(xy(s), xy(m), xy(e))
    if kind == 'poly':
        pts = kid(n, 'pts')
        out = []
        for p in (pts[1:] if pts else []):
            if head(p) == 'xy':
                out.append(xy(p))
            elif head(p) == 'arc':
                out += arc_points(xy(kid(p, 'start')), xy(kid(p, 'mid')), xy(kid(p, 'end')))
        return out
    if kind == 'curve':
        pts = kid(n, 'pts')
        return [xy(p) for p in pts[1:] if head(p) == 'xy'] if pts else []
    return []


def board(root):
    out = {'kind': 'pcb', 'version': int(float(text(kid(root, 'version')[1])))}
    c = dict(footprints=0, pads=0, segments=0, arcs=0, vias=0, zones=0, filled=0)
    edge = []
    for n in root[1:]:
        h = head(n)
        if h == 'footprint':
            c['footprints'] += 1
            fx, fy, fa = at(n)
            for k in n[1:]:
                kh = head(k)
                if kh == 'pad':
                    c['pads'] += 1
                elif kh == 'zone':
                    c['zones'] += 1
                    c['filled'] += len(kids(k, 'filled_polygon'))
                elif kh and kh.startswith('fp_') and kid(k, 'layer') and text(kid(k, 'layer')[1]) == 'Edge.Cuts':
                    for p in shape_points(k):
                        q = rot(p, fa)
                        edge.append((q[0] + fx, q[1] + fy))
        elif h == 'segment':
            c['segments'] += 1
        elif h == 'arc':
            c['arcs'] += 1
        elif h == 'via':
            c['vias'] += 1
        elif h == 'zone':
            c['zones'] += 1
            c['filled'] += len(kids(n, 'filled_polygon'))
        elif h and h.startswith('gr_') and kid(n, 'layer') and text(kid(n, 'layer')[1]) == 'Edge.Cuts':
            edge += shape_points(n)
    out.update(c)
    out['nets'] = sum(1 for n in kids(root, 'net') if len(n) > 2 and text(n[2]) != '')
    edge = [p for p in edge if p[0] is not None and p[1] is not None]
    if edge:
        xs, ys = [p[0] for p in edge], [p[1] for p in edge]
        out['edge'] = [max(xs) - min(xs), max(ys) - min(ys)]
    return out


def is_power(lib):
    return lib is not None and any(c == 'power' or head(c) == 'power' for c in lib[1:])


def schematic(path, root, stack, acc):
    acc['pages'] += 1
    libs = {text(s[1]): s for s in kids(kid(root, 'lib_symbols') or ['lib_symbols'], 'symbol')}
    for n in root[1:]:
        h = head(n)
        if h == 'symbol':
            lid = kid(n, 'lib_name') or kid(n, 'lib_id')
            lib = libs.get(text(lid[1])) if lid else None
            acc['power' if is_power(lib) else 'symbols'] += 1
        elif h == 'wire':
            acc['wires'] += 1
        elif h == 'bus':
            acc['buses'] += 1
        elif h in ('label', 'global_label', 'hierarchical_label'):
            acc['labels'] += 1
        elif h in ('directive_label', 'netclass_flag'):
            acc['directives'] += 1
        elif h == 'junction':
            acc['junctions'] += 1
        elif h == 'sheet':
            acc['sheets'] += 1
            f = None
            for p in kids(n, 'property'):
                if text(p[1]) in ('Sheetfile', 'Sheet file'):
                    f = text(p[2])
            if not f or f in stack or len(stack) > 64:
                continue
            sub = os.path.join(os.path.dirname(path), f)
            try:
                with open(sub, encoding='utf-8', errors='replace') as fh:
                    t = parse(fh.read())
            except (OSError, ValueError):
                continue
            if head(t) != 'kicad_sch':
                continue
            schematic(path, t, stack + [f], acc)


def kiutils_board(path):
    from kiutils.board import Board
    b = Board().from_file(path)
    pads = sum(len(f.pads) for f in b.footprints)
    segs = sum(1 for t in b.traceItems if type(t).__name__ == 'Segment')
    arcs = sum(1 for t in b.traceItems if type(t).__name__ == 'Arc')
    vias = sum(1 for t in b.traceItems if type(t).__name__ == 'Via')
    return dict(footprints=len(b.footprints), pads=pads, segments=segs, arcs=arcs, vias=vias,
                zones=len(b.zones) + sum(len(getattr(f, 'zones', []) or []) for f in b.footprints))


def kiutils_schematic(path):
    from kiutils.schematic import Schematic
    s = Schematic().from_file(path)
    wires = sum(1 for g in s.graphicalItems if type(g).__name__ == 'Connection' and g.type == 'wire')
    buses = sum(1 for g in s.graphicalItems if type(g).__name__ == 'Connection' and g.type == 'bus')
    return dict(symbols=len(s.schematicSymbols), wires=wires, buses=buses,
                labels=len(s.labels) + len(s.globalLabels) + len(s.hierarchicalLabels),
                junctions=len(s.junctions), sheets=len(s.sheets))


def main():
    corpus, out = sys.argv[1], sys.argv[2]
    lines = []
    for dirpath, _, files in sorted(os.walk(corpus)):
        for name in sorted(files):
            if not name.endswith(('.kicad_pcb', '.kicad_sch')):
                continue
            path = os.path.join(dirpath, name)
            rel = os.path.relpath(path, corpus)
            with open(path, encoding='utf-8', errors='replace') as fh:
                data = fh.read()
            try:
                root = parse(data)
            except ValueError:
                lines.append({'file': rel, 'kind': 'broken'})
                continue
            version = int(float(text(kid(root, 'version')[1]))) if kid(root, 'version') else 0
            rec = {'file': rel}
            if head(root) == 'kicad_pcb':
                if version < 20200000:
                    rec['kind'] = 'legacy'
                else:
                    rec.update(board(root))
                    try:
                        rec['kiutils'] = kiutils_board(path)
                    except Exception as e:  # kiutils lags new formats
                        rec['kiutils_error'] = type(e).__name__
            elif head(root) == 'kicad_sch':
                acc = dict(pages=0, symbols=0, power=0, wires=0, buses=0, labels=0, directives=0, junctions=0, sheets=0)
                schematic(path, root, [], acc)
                rec.update(kind='sch', version=version, **acc)
                try:
                    rec['kiutils'] = kiutils_schematic(path)
                except Exception as e:
                    rec['kiutils_error'] = type(e).__name__
            else:
                continue
            lines.append(rec)
    with open(out, 'w') as f:
        for r in lines:
            f.write(json.dumps(r) + '\n')
    print(len(lines), 'files')


if __name__ == '__main__':
    main()
