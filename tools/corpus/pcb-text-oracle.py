#!/usr/bin/env python3
"""Independent expected shapes of the eng3 board fixtures (pcb-text-fixtures.py) for
specular-pcb's tests/eng3_corpus.rs.

For each ODB++ job and step, and for the IPC-2581 file: per layer, every drawn shape's
bounding box (mm) and whether it is drawn clear, in draw order: pads (round and rectangular
standard symbols), round lines, text strokes, and for a NEGATIVE layer the step's profile
first with every feature's polarity turned over. Step-and-repeat copies are expanded.

Text follows the ODB++ specification as read for Specular: `T x y font polarity orient
xsize ysize width_factor 'text' version`, the lower-left corner at x y, cells xsize × ysize
with the font's OFFSET between them (scaled like the cell), strokes width_factor × 12 mil;
the font's cell is centred on its capitals' and digits' strokes, its bottom at their
baseline; characters the font lacks come from Specular's built-in stroke font (the table
below, Specular's own design). IPC-2581 Text fills its BoundingBox (height; width
stretched), embedded glyphs share one cell (the largest glyph box) with strokes as wide as
the widest LineDesc scaled to the text, others use the built-in font with strokes 1/8 of the
height; then the Xform.

Written with the standard library, separately from the Rust reader.

    python3 -I tools/corpus/pcb-text-oracle.py FIXTURES > FIXTURES/eng3-oracle.json
"""

import json
import math
import os
import sys
import xml.etree.ElementTree as ET

MIL12 = 12 * 0.0254

# Specular's built-in stroke font: grid x 0-4, y 0-9, baseline 2, cap height 8; a 6-unit
# cell starting one unit left of the grid.
BUILTIN = {
    "!": "2824 22", '"': "1817 3837", "#": "1317 3337 0444 0646",
    "$": "473818070615354443321203 2921", "%": "0248 0818170708 3343423233",
    "&": "4216172837360403122244", "'": "2826", "(": "392816142231", ")": "192836342211",
    "*": "2723 0644 0446", "+": "2327 0545", ",": "232211", "-": "0545", ".": "22",
    "/": "0248", "0": "183847433212030718 0347", "1": "172822 1232", "2": "07183847460242",
    "3": "07183847463515 354443321203", "4": "32380444", "5": "480805354443321203",
    "6": "473818070312324344351504", "7": "084812", "8": "15060718384746351504031232434435",
    "9": "031232434738180706153546", ":": "26 23", ";": "26 232211", "<": "480542",
    "=": "0646 0444", ">": "084502", "?": "071838474624 22", "@": "34141636334347381807031242",
    "A": "0206284642 0545", "B": "02083847463505 3544433202", "C": "4738180703123243",
    "D": "02083847433202", "E": "48080242 0535", "F": "480802 0535",
    "G": "47381807031232434525", "H": "0208 4842 0545", "I": "1838 2822 1232",
    "J": "2848 3833221203", "K": "0208 4804 1542", "L": "080242", "M": "0208254842",
    "N": "02084248", "O": "183847433212030718", "P": "02083847463505",
    "Q": "183847433212030718 2442", "R": "02083847463505 2542", "S": "473818070615354443321203",
    "T": "0848 2822", "U": "080312324348", "V": "082248", "W": "0812253248", "X": "0842 0248",
    "Y": "082548 2522", "Z": "08480242", "[": "39292131", "\\": "0842", "]": "19292111",
    "^": "062846", "_": "0141", "`": "1827", "a": "16364542 433212031444",
    "b": "0802 0516364543321203", "c": "4536160503123243", "d": "4842 4536160503123243",
    "e": "04444536160503123243", "f": "38281712 0636", "g": "4536160504133344 4641301001",
    "h": "0802 0516364542", "i": "2622 28", "j": "3631201001 38", "k": "0802 4603 1442",
    "l": "18282332", "m": "0206 05162522 25364542", "n": "0206 0516364542",
    "o": "163645433212030516", "p": "0600 0516364543321203", "q": "4640 4536160503123243",
    "r": "0206 05163645", "s": "45361605143443321203", "t": "18132232 0636",
    "u": "0603123243 4642", "v": "062246", "w": "0612243246", "x": "0642 0246",
    "y": "0623 4610", "z": "06460242", "{": "39282615242231", "|": "2921",
    "}": "19282635242211", "~": "05163546",
}


class Font:
    def __init__(self, xsize, ysize, offset, origin, glyphs):
        self.xsize, self.ysize, self.offset, self.origin, self.glyphs = (
            xsize, ysize, offset, origin, glyphs)


def builtin_font():
    glyphs = {}
    for c, s in BUILTIN.items():
        strokes = []
        for run in s.split():
            pts = [(int(run[i]), int(run[i + 1])) for i in range(0, len(run), 2)]
            if len(pts) == 1:
                strokes.append((pts[0], pts[0], False))
            for a, b in zip(pts, pts[1:]):
                strokes.append((a, b, False))
        glyphs[c] = strokes
    return Font(6.0, 6.0, 0.0, (-1.0, 2.0), glyphs)


BUILT_IN = builtin_font()


def odb_font(text):
    xs = ys = off = 0.0
    glyphs, cur = {}, None
    for line in text.splitlines():
        t = line.split()
        if not t:
            continue
        k = t[0].upper()
        if k == "XSIZE":
            xs = float(t[1])
        elif k == "YSIZE":
            ys = float(t[1])
        elif k == "OFFSET":
            off = float(t[1])
        elif k == "CHAR":
            rest = line.strip()[4:]
            rest = rest[1:] if rest.startswith(" ") else rest
            cur = (rest[:1] or " ", [])
        elif k == "LINE" and cur is not None:
            v = [float(x) for x in t[1:5]]
            square = len(t) > 6 and t[6].upper() == "S"
            cur[1].append(((v[0], v[1]), (v[2], v[3]), square))
        elif k == "ECHAR" and cur is not None:
            glyphs[cur[0]] = cur[1]
            cur = None
    x0 = y0 = math.inf
    x1 = -math.inf
    for c, ss in glyphs.items():
        if c.isascii() and (c.isupper() or c.isdigit()):
            for a, b, _ in ss:
                x0, x1 = min(x0, a[0], b[0]), max(x1, a[0], b[0])
                y0 = min(y0, a[1], b[1])
    return Font(xs, ys, off, ((x0 + x1) / 2 - xs / 2, y0), glyphs)


# Affines as (a, b, c, d, e, f): x' = a x + c y + e, y' = b x + d y + f.
def mul(m, n):
    a, b, c, d, e, f = m
    p, q, r, s, t, u = n
    return (a * p + c * q, b * p + d * q, a * r + c * s, b * r + d * s,
            a * t + c * u + e, b * t + d * u + f)


def tr(x, y):
    return (1, 0, 0, 1, x, y)


def rot(deg):
    r = math.radians(deg)
    return (math.cos(r), math.sin(r), -math.sin(r), math.cos(r), 0, 0)


def sc(x, y):
    return (x, 0, 0, y, 0, 0)


FLIP_X = (-1, 0, 0, 1, 0, 0)


def ap(m, p):
    a, b, c, d, e, f = m
    return (a * p[0] + c * p[1] + e, b * p[0] + d * p[1] + f)


def seg_box(a, b, w, square):
    """The box of a stroke from a to b, w wide: round ends, or square ones extended."""
    if not square or math.hypot(b[0] - a[0], b[1] - a[1]) < 1e-12:
        r = w / 2
        return (min(a[0], b[0]) - r, min(a[1], b[1]) - r, max(a[0], b[0]) + r,
                max(a[1], b[1]) + r)
    t = math.atan2(b[1] - a[1], b[0] - a[0])
    ux, uy = math.cos(t) * w / 2, math.sin(t) * w / 2
    nx, ny = -uy, ux
    pts = [(a[0] - ux + sx * nx, a[1] - uy + sx * ny) for sx in (-1, 1)]
    pts += [(b[0] + ux + sx * nx, b[1] + uy + sx * ny) for sx in (-1, 1)]
    xs, ys = [p[0] for p in pts], [p[1] for p in pts]
    return (min(xs), min(ys), max(xs), max(ys))


def draw_text(font, s, cw, ch, gap, width, place, dark, fallback=BUILT_IN):
    out = []
    col, line = 0, 0
    for c in s:
        if c == "\n":
            line, col = line + 1, 0
            continue
        f = font if c in font.glyphs else (fallback if c in fallback.glyphs else None)
        if f is None:
            col += 1
            continue
        m = mul(place, mul(tr(col * (cw + gap), -line * ch * 1.5),
                           mul(sc(cw / f.xsize, ch / f.ysize), tr(-f.origin[0], -f.origin[1]))))
        for a, b, square in f.glyphs[c]:
            out.append((seg_box(ap(m, a), ap(m, b), width, square), not dark))
        col += 1
    return out


# ---------------------------------------------------------------- ODB++

def blocks(text):
    out, cur = [], None
    for line in text.splitlines():
        l = line.strip()
        if l.endswith("{"):
            cur = (l[:-1].strip().upper(), {})
        elif l == "}":
            if cur:
                out.append(cur)
            cur = None
        elif cur is not None and "=" in l:
            k, v = l.split("=", 1)
            cur[1][k.strip().upper()] = v.strip()
    return out


def top_field(text, key):
    depth = 0
    for line in text.splitlines():
        l = line.strip()
        if l.endswith("{"):
            depth += 1
        elif l == "}":
            depth -= 1
        elif depth == 0 and "=" in l:
            k, v = l.split("=", 1)
            if k.strip().upper() == key:
                return v.strip()
    return None


def units(text):
    for l in text.splitlines()[:50]:
        l = l.strip().upper()
        if l in ("UNITS=MM", "U MM"):
            return 1.0, 0.001
        if l in ("UNITS=INCH", "U INCH"):
            return 25.4, 0.0254
    return 25.4, 0.0254


def orient(t):
    o = int(t[0])
    if o == 8:
        return float(t[1]), False
    if o == 9:
        return float(t[1]), True
    return (o % 4) * 90.0, o >= 4


def circle_box(c, d):
    r = d / 2
    return (c[0] - r, c[1] - r, c[0] + r, c[1] + r)


def poly_box(pts):
    xs, ys = [p[0] for p in pts], [p[1] for p in pts]
    return (min(xs), min(ys), max(xs), max(ys))


class Job:
    def __init__(self, root):
        self.files = {}
        for dp, _, fs in os.walk(root):
            for f in fs:
                p = os.path.join(dp, f)
                rel = os.path.relpath(p, root).replace(os.sep, "/").lower()
                with open(p, encoding="latin-1") as fh:
                    self.files[rel] = fh.read()
        self.fonts = {}

    def font(self, name):
        name = name.lower()
        if name not in self.fonts:
            t = self.files.get("fonts/" + name)
            self.fonts[name] = odb_font(t) if t else None
        return self.fonts[name]

    def features(self, text):
        """Shapes of a features file: (kind, data, clear). Kinds: box (a final box), seg
        (ends, width, square) and pad (centre, symbol, rotation, mirror)."""
        cu, su = units(text)
        syms = {}
        out = []
        for raw in text.splitlines():
            line = raw.split(";")[0].strip()
            if not line or line.startswith("#"):
                continue
            t = line.split()
            if t[0].startswith("$"):
                syms[int(t[0][1:])] = t[1].lower()
                continue
            if t[0] == "P":
                at = (float(t[1]) * cu, float(t[2]) * cu)
                clear = t[4] == "N"
                deg, mir = orient(t[6:])
                out.append(("pad", (at, syms[int(t[3])], su, deg, mir), clear))
            elif t[0] == "L":
                a = (float(t[1]) * cu, float(t[2]) * cu)
                b = (float(t[3]) * cu, float(t[4]) * cu)
                d = float(syms[int(t[5])][1:]) * su
                out.append(("seg", (a, b, d, False), t[6] == "N"))
            elif t[0] == "T":
                q0, q1 = line.index("'"), line.rindex("'")
                head, s = line[:q0].split(), line[q0 + 1:q1]
                two = head[5] in ("8", "9")
                rest = head[7:] if two else head[6:]
                deg, mir = orient(head[5:])
                xs, ys, wf = float(rest[0]) * cu, float(rest[1]) * cu, float(rest[2])
                place = mul(tr(float(head[1]) * cu, float(head[2]) * cu), rot(-deg))
                if mir:
                    place = mul(place, FLIP_X)
                f = self.font(head[3]) or BUILT_IN
                gap = f.offset * xs / f.xsize
                for box, clear in draw_text(f, s, xs, ys, gap, wf * MIL12, place, head[4] != "N"):
                    out.append(("box", box, clear))
        return out

    def step_layer(self, step, layer, depth=0):
        text = self.files.get("steps/%s/layers/%s/features" % (step, layer))
        objs = [(k, d, c, (1, 0, 0, 1, 0, 0)) for k, d, c in self.features(text)] if text else []
        hdr = self.files.get("steps/%s/stephdr" % step, "")
        cu, _ = units(hdr)
        for name, f in blocks(hdr):
            if name != "STEP-REPEAT" or depth >= 4:
                continue
            child = self.step_layer(f["NAME"].lower(), layer, depth + 1)
            ch = self.files.get("steps/%s/stephdr" % f["NAME"].lower(), "")
            ccu, _ = units(ch)
            datum = (float(top_field(ch, "X_DATUM") or 0) * ccu,
                     float(top_field(ch, "Y_DATUM") or 0) * ccu)
            base = rot(-float(f.get("ANGLE", 0)))
            if f.get("MIRROR", "NO").upper() == "YES":
                base = mul(base, FLIP_X)
            base = mul(base, tr(-datum[0], -datum[1]))
            for j in range(int(float(f.get("NY", 1)))):
                for i in range(int(float(f.get("NX", 1)))):
                    at = (float(f.get("X", 0)) * cu + i * float(f.get("DX", 0)) * cu,
                          float(f.get("Y", 0)) * cu + j * float(f.get("DY", 0)) * cu)
                    m = mul(tr(*at), base)
                    for k, d, c, m0 in child:
                        objs.append((k, d, c, mul(m, m0)))
        return objs


def finish(obj):
    """An object under its transform as a box."""
    k, d, clear, m = obj
    if k == "seg":
        a, b, w, sq = d
        return seg_box(ap(m, a), ap(m, b), w, sq), clear
    if k == "pad":
        at, sym, su, deg, mir = d
        local = mul(tr(*at), rot(-deg))
        if mir:
            local = mul(local, FLIP_X)
        full = mul(m, local)
        if sym.startswith("rect"):
            w, h = (float(v) * su for v in sym[4:].split("x")[:2])
            pts = [ap(full, (sx * w / 2, sy * h / 2)) for sx in (-1, 1) for sy in (-1, 1)]
            return poly_box(pts), clear
        dia = float(sym[1:]) * su
        return circle_box(ap(full, (0, 0)), dia), clear
    # A box made in the features file's frame: its corners through m (axis-aligned for the
    # quarter turns the fixtures use).
    x0, y0, x1, y1 = d
    return poly_box([ap(m, p) for p in ((x0, y0), (x1, y0), (x0, y1), (x1, y1))]), clear


def odb_job(root):
    job = Job(root)
    mat = blocks(job.files["matrix/matrix"])
    steps = [f["NAME"].lower() for n, f in sorted(
        ((n, f) for n, f in mat if n == "STEP"), key=lambda x: int(x[1].get("COL", 0)))]
    layers = [f for n, f in sorted(((n, f) for n, f in mat if n == "LAYER"),
                                   key=lambda x: int(x[1].get("ROW", 0)))
              if f.get("CONTEXT", "").upper() == "BOARD"]
    out = {}
    for step in steps:
        prof = job.files.get("steps/%s/profile" % step)
        pbox = None
        if prof:
            cu, _ = units(prof)
            pts = []
            for l in prof.splitlines():
                t = l.split()
                if t and t[0] in ("OB", "OS"):
                    pts.append((float(t[1]) * cu, float(t[2]) * cu))
            pbox = poly_box(pts) if pts else None
        res = {}
        for lay in layers:
            name = lay["NAME"].lower()
            objs = [finish(o) for o in job.step_layer(step, name)]
            if not objs:
                continue
            if lay.get("POLARITY", "").upper() == "NEGATIVE":
                objs = [(pbox, False)] + [(b, not c) for b, c in objs]
            res[lay["NAME"]] = [[round(v, 6) for v in b] + [c] for b, c in objs]
        out[step] = res
    return out


# ---------------------------------------------------------------- IPC-2581

def local(tag):
    return tag.rsplit("}", 1)[-1]


def kids(n, name):
    return [c for c in n if local(c.tag) == name]


def num(n, a, default=0.0):
    v = n.get(a)
    return float(v) if v is not None else default


def arc_pts(a, b, c, cw):
    r = math.hypot(a[0] - c[0], a[1] - c[1])
    a0 = math.atan2(a[1] - c[1], a[0] - c[0])
    sweep = math.atan2(b[1] - c[1], b[0] - c[0]) - a0
    if cw:
        while sweep >= 0:
            sweep -= 2 * math.pi
    else:
        while sweep <= 0:
            sweep += 2 * math.pi
    n = min(max(math.ceil(abs(sweep) / math.radians(10)), 1), 36)
    pts = [(c[0] + r * math.cos(a0 + sweep * i / n), c[1] + r * math.sin(a0 + sweep * i / n))
           for i in range(n)]
    return pts + [b]


def ipc_font(entry):
    emb = [e for e in entry.iter() if local(e.tag) == "FontDefEmbedded"]
    if not emb:
        return None
    glyphs, pen = {}, 0.0
    x0 = y0 = math.inf
    x1 = y1 = 0.0
    for g in kids(emb[0], "Glyph"):
        c = chr(int(g.get("charCode"), 16))
        lx, ly = num(g, "lowerLeftX"), num(g, "lowerLeftY")
        ux, uy = num(g, "upperRightX"), num(g, "upperRightY")
        x0, y0, x1, y1 = min(x0, lx), min(y0, ly), max(x1, ux - lx), max(y1, uy - ly)
        strokes = []
        for s in g:
            ld = kids(s, "LineDesc")
            sq = bool(ld) and ld[0].get("lineEnd", "").upper() == "SQUARE"
            if ld:
                pen = max(pen, num(ld[0], "lineWidth"))
            t = local(s.tag)
            if t == "Line":
                strokes.append(((num(s, "startX"), num(s, "startY")),
                                (num(s, "endX"), num(s, "endY")), sq))
            elif t == "Arc":
                pts = arc_pts((num(s, "startX"), num(s, "startY")), (num(s, "endX"), num(s, "endY")),
                              (num(s, "centerX"), num(s, "centerY")), s.get("clockwise") == "true")
                strokes += [(p, q, sq) for p, q in zip(pts, pts[1:])]
            elif t == "Polyline":
                at = None
                for v in s:
                    q = (num(v, "x"), num(v, "y"))
                    if local(v.tag) == "PolyStepSegment" and at:
                        strokes.append((at, q, sq))
                    if local(v.tag) in ("PolyBegin", "PolyStepSegment"):
                        at = q
        glyphs[c] = strokes
    return Font(x1, y1, 0.0, (x0, y0), glyphs), pen


def xform_of(n):
    x = kids(n, "Xform")
    if not x:
        return (1, 0, 0, 1, 0, 0)
    x = x[0]
    m = mul(tr(num(x, "xOffset"), num(x, "yOffset")), mul(rot(num(x, "rotation")),
                                                          sc(num(x, "scale", 1), num(x, "scale", 1))))
    if x.get("mirror") in ("true", "1"):
        m = mul(m, FLIP_X)
    return m


def ipc_file(path):
    root = ET.parse(path).getroot()
    fonts = {}
    for e in root.iter():
        if local(e.tag) == "EntryFont":
            fonts[e.get("id")] = ipc_font(e)
    out = {}
    for lf in root.iter():
        if local(lf.tag) != "LayerFeature":
            continue
        shapes = []
        for st in kids(lf, "Set"):
            dark = st.get("polarity", "").upper() != "NEGATIVE"
            for feat in kids(st, "Features"):
                loc = kids(feat, "Location")[0]
                a = mul(tr(num(loc, "x"), num(loc, "y")), xform_of(feat))
                for t in kids(feat, "Text"):
                    s = t.get("textString")
                    bb = kids(t, "BoundingBox")
                    ll, w, h = (0.0, 0.0), 0.0, num(t, "fontSize")
                    if bb:
                        b = bb[0]
                        ll = (min(num(b, "lowerLeftX"), num(b, "upperRightX")),
                              min(num(b, "lowerLeftY"), num(b, "upperRightY")))
                        w = abs(num(b, "upperRightX") - num(b, "lowerLeftX"))
                        h = abs(num(b, "upperRightY") - num(b, "lowerLeftY")) or h
                    fr = kids(t, "FontRef")
                    found = fonts.get(fr[0].get("id")) if fr else None
                    font, pen = found if found else (BUILT_IN, 0.0)
                    cw = font.xsize * h / font.ysize
                    gap = font.offset * h / font.ysize
                    n = max(len(l) for l in s.split("\n"))
                    natural = n * cw + (n - 1) * gap
                    if w > 0 and natural > 0:
                        cw, gap = cw * w / natural, gap * w / natural
                    width = pen * h / font.ysize if pen > 0 else h / 8
                    place = mul(a, mul(xform_of(t), tr(*ll)))
                    shapes += draw_text(font, s, cw, h, gap, width, place, dark)
        out[lf.get("layerRef")] = [[round(v, 6) for v in b] + [c] for b, c in shapes]
    return out


def main():
    root = sys.argv[1]
    res = {"odb": {}, "ipc2581": {}}
    for j in sorted(os.listdir(os.path.join(root, "odb"))):
        res["odb"][j] = odb_job(os.path.join(root, "odb", j))
    for f in sorted(os.listdir(os.path.join(root, "ipc2581"))):
        res["ipc2581"][f] = ipc_file(os.path.join(root, "ipc2581", f))
    json.dump(res, sys.stdout, indent=1)


if __name__ == "__main__":
    main()
