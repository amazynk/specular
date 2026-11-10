"""Builds the font fixtures from scratch with fontTools (simple geometric glyphs).

Run from anywhere with a Python that has fontTools and brotli:
    python -I make.py
    python -I make.py cbdt        # only cbdt.ttf and cbdt-only.ttf
It writes the fonts next to this script, plus `expected/*.json` (fontTools' own reading of
names, cmap and advances) and `expected/*.woff2.sfnt` (fontTools' WOFF2 decompression) for
the oracle tests.
"""

import os
import struct
import sys
import zlib

from fontTools.fontBuilder import FontBuilder
from fontTools.pens.ttGlyphPen import TTGlyphPen
from fontTools.pens.t2CharStringPen import T2CharStringPen
from fontTools.misc.psCharStrings import T2CharString
from fontTools.ttLib import TTFont, newTable
from fontTools.ttLib.ttCollection import TTCollection
from fontTools.ttLib.tables import _s_b_i_x, sbixStrike, sbixGlyph
from fontTools.colorLib.builder import buildCOLR, buildCPAL
from fontTools.ttLib import woff2

HERE = os.path.dirname(os.path.abspath(__file__))
EXP = os.path.join(HERE, "expected")
os.makedirs(EXP, exist_ok=True)

UPM = 1000
ORDER = [".notdef", "space", "A", "B", "a", "o", "one", "two", "acute", "Aacute", "Asmall", "Aslant"]
CMAP = {0x20: "space", 0x41: "A", 0x42: "B", 0x61: "a", 0x6F: "o", 0x31: "one", 0x32: "two",
        0xB4: "acute", 0xC1: "Aacute", 0x1F600: "Asmall"}
ADV = {".notdef": 500, "space": 250, "A": 600, "B": 580, "a": 520, "o": 540, "one": 500,
       "two": 500, "acute": 300, "Aacute": 600, "Asmall": 300, "Aslant": 600}


def names(family, style, **extra):
    n = {
        "familyName": family,
        "styleName": style,
        "uniqueFontIdentifier": f"{family}-{style};fixture",
        "fullName": f"{family} {style}",
        "version": "Version 1.002",
        "psName": f"{family.replace(' ', '')}-{style}",
        "copyright": "Copyright 2026 Specular fixtures",
        "trademark": "Not a trademark",
        "manufacturer": "Specular",
        "designer": "Test Designer",
        "description": "A synthetic font for tests.",
        "vendorURL": "https://example.invalid/vendor",
        "designerURL": "https://example.invalid/designer",
        "licenseDescription": "This Font Software is licensed under the SIL Open Font License, Version 1.1.",
        "licenseInfoURL": "https://openfontlicense.org",
        "sampleText": "Aa Bb Oo 12",
    }
    n.update(extra)
    return n


def draw(pen, name, quad):
    """Draws glyph `name` with `pen` (quadratic curves when `quad`)."""
    def curve(*pts):
        if quad:
            pen.qCurveTo(*pts)
        else:
            # Same shape as cubic: each off-curve run becomes one cubic.
            if len(pts) == 2:
                (cx, cy), (x, y) = pts
                pen.curveTo((cx, cy), (cx, cy), (x, y))
            else:
                pen.curveTo(*pts[:3])
    if name == ".notdef":
        pen.moveTo((50, 0)); pen.lineTo((450, 0)); pen.lineTo((450, 700)); pen.lineTo((50, 700)); pen.closePath()
        pen.moveTo((100, 50)); pen.lineTo((100, 650)); pen.lineTo((400, 650)); pen.lineTo((400, 50)); pen.closePath()
    elif name == "A":
        pen.moveTo((0, 0)); pen.lineTo((100, 0)); pen.lineTo((300, 600)); pen.lineTo((500, 0)); pen.lineTo((600, 0))
        pen.lineTo((350, 700)); pen.lineTo((250, 700)); pen.closePath()
        pen.moveTo((200, 200)); pen.lineTo((400, 200)); pen.lineTo((380, 260)); pen.lineTo((220, 260)); pen.closePath()
    elif name == "B":
        pen.moveTo((60, 0)); pen.lineTo((300, 0))
        curve((540, 0), (540, 340)) if quad else pen.curveTo((460, 0), (540, 80), (540, 180))
        if quad:
            pen.lineTo((540, 360))
        else:
            pen.lineTo((540, 360))
        pen.lineTo((300, 700)); pen.lineTo((60, 700)); pen.closePath()
    elif name == "a":
        if quad:
            # Consecutive off-curve points: implied on-curve midpoints.
            pen.moveTo((260, 0))
            pen.qCurveTo((60, 0), (60, 250), (260, 250))
            pen.lineTo((400, 250)); pen.lineTo((400, 0)); pen.closePath()
        else:
            pen.moveTo((260, 0))
            pen.curveTo((120, 0), (60, 60), (60, 125))
            pen.curveTo((60, 190), (120, 250), (260, 250))
            pen.lineTo((400, 250)); pen.lineTo((400, 0)); pen.closePath()
    elif name == "o":
        if quad:
            # All off-curve contour (a TrueType circle without on-curve points).
            pen.qCurveTo((270, 0), (500, 250), (270, 500), (40, 250), None)
            pen.closePath()
        else:
            pen.moveTo((270, 0))
            pen.curveTo((400, 0), (500, 110), (500, 250))
            pen.curveTo((500, 390), (400, 500), (270, 500))
            pen.curveTo((140, 500), (40, 390), (40, 250))
            pen.curveTo((40, 110), (140, 0), (270, 0))
            pen.closePath()
    elif name == "one":
        pen.moveTo((200, 0)); pen.lineTo((300, 0)); pen.lineTo((300, 700)); pen.lineTo((150, 600)); pen.closePath()
    elif name == "two":
        pen.moveTo((60, 0)); pen.lineTo((440, 0)); pen.lineTo((440, 80)); pen.lineTo((180, 80))
        curve((440, 300), (440, 500)) if quad else pen.curveTo((300, 200), (440, 350), (440, 500))
        pen.lineTo((60, 500)); pen.closePath()
    elif name == "acute":
        pen.moveTo((100, 750)); pen.lineTo((180, 750)); pen.lineTo((260, 850)); pen.lineTo((200, 850)); pen.closePath()


def glyf_glyphs(scale=1.0):
    out = {}
    for n in ORDER:
        pen = TTGlyphPen(out)
        if n == "Aacute":
            pen.addComponent("A", (1, 0, 0, 1, 0, 0))
            pen.addComponent("acute", (1, 0, 0, 1, 150, 0))
        elif n == "Asmall":
            pen.addComponent("A", (0.5, 0, 0, 0.5, 0, 0))
        elif n == "Aslant":
            pen.addComponent("Asmall", (1, 0, 0.25, 1.5, 20, -10))
        elif n == "space":
            pass
        else:
            draw(ScalePen(pen, scale), n, True)
        out[n] = pen.glyph()
    return out


class ScalePen:
    """Scales x by `s` (for the bold master)."""
    def __init__(self, pen, s):
        self.pen, self.s = pen, s

    def _p(self, p):
        return None if p is None else (round(p[0] * self.s), p[1])

    def moveTo(self, p): self.pen.moveTo(self._p(p))
    def lineTo(self, p): self.pen.lineTo(self._p(p))
    def qCurveTo(self, *ps): self.pen.qCurveTo(*[self._p(p) for p in ps])
    def curveTo(self, *ps): self.pen.curveTo(*[self._p(p) for p in ps])
    def closePath(self): self.pen.closePath()
    def endPath(self): self.pen.endPath()


def cff_glyphs(scale=1.0):
    out = {}
    for n in ORDER:
        pen = T2CharStringPen(ADV[n], None)
        if n in ("Aacute", "Asmall", "Aslant"):
            # CFF has no components: draw A (and the accent) directly.
            draw(ScalePen(pen, scale), "A", False)
            if n == "Aacute":
                draw(pen, "acute", False)
        elif n != "space":
            draw(ScalePen(pen, scale), n, False)
        out[n] = pen.getCharString()
    return out


def base_font(fb, family, style, quad, fs_type, weight=400, scale=1.0):
    fb.setupGlyphOrder(ORDER)
    fb.setupCharacterMap(CMAP)
    if quad:
        fb.setupGlyf(glyf_glyphs(scale))
        metrics = {n: (ADV[n], fb.font["glyf"][n].xMin if hasattr(fb.font["glyf"][n], "xMin") else 0) for n in ORDER}
    else:
        fb.setupCFF(names(family, style)["psName"], {"FullName": f"{family} {style}"}, cff_glyphs(scale), {})
        metrics = {n: (ADV[n], 0) for n in ORDER}
    fb.setupHorizontalMetrics(metrics)
    fb.setupHorizontalHeader(ascent=800, descent=-200)
    fb.setupNameTable(names(family, style))
    fb.setupOS2(sTypoAscender=800, sTypoDescender=-200, usWinAscent=850, usWinDescent=200,
                fsType=fs_type, usWeightClass=weight, sxHeight=500, sCapHeight=700, version=4)
    fb.setupPost()


def ttf(family="Spec Sans", style="Regular", fs_type=0, weight=400, scale=1.0):
    fb = FontBuilder(UPM, isTTF=True)
    base_font(fb, family, style, True, fs_type, weight, scale)
    return fb


def otf(family="Spec Serif", style="Regular", fs_type=4, weight=400, scale=1.0):
    fb = FontBuilder(UPM, isTTF=False)
    base_font(fb, family, style, False, fs_type, weight, scale)
    return fb


def add_cff_subrs(font):
    """Rewrites a few charstrings to call local and global subroutines, and adds hint masks
    and the less common curve operators."""
    from fontTools.cffLib import SubrsIndex, GlobalSubrsIndex
    cff = font["CFF "].cff
    top = cff.topDictIndex[0]
    priv = top.Private
    cs = top.CharStrings
    gsubrs = cff.GlobalSubrs
    subrs = SubrsIndex()
    # Local subr 0: a square contour from the current point.
    subrs.append(T2CharString(program=[100, 0, 0, 100, -100, 0, "rlineto", "return"]))
    # Local subr 1 calls global subr 0.
    subrs.append(T2CharString(program=[0 - 107, "callgsubr", "return"]))
    gsubrs.append(T2CharString(program=[50, 50, 50, 0, 50, -50, "rrcurveto", "return"]))
    priv.Subrs = subrs
    # "one": hints, a hintmask, h/v line runs and subroutines.
    def t2(program):
        return T2CharString(program=program, private=priv, globalSubrs=gsubrs)
    for s in list(subrs) + list(gsubrs):
        s.private, s.globalSubrs = priv, gsubrs
    cs["one"] = t2([
        500, 0, 50, 600, 50, "hstemhm", 200, 100, "hintmask", bytes([0xE0]),
        200, 0, "rmoveto", 100, 700, -150, -100, "hlineto",
        100, 100, "rmoveto", 0 - 107, "callsubr",
        50, 0, "rmoveto", 1 - 107, "callsubr",
        "endchar"])
    # "two": hvcurveto, vhcurveto, hhcurveto, vvcurveto, rcurveline, rlinecurve, flex.
    cs["two"] = t2([
        500, 60, 0, "rmoveto",
        100, 50, 50, 100, "hvcurveto",
        100, 50, 50, 100, 20, "vhcurveto",
        10, 50, 30, -20, 40, "hhcurveto",
        5, -40, 30, 20, -50, "vvcurveto",
        10, 10, 20, 20, 30, 0, 15, -15, "rcurveline",
        -10, -10, -5, -20, -20, -30, -40, -50, "rlinecurve",
        20, "hmoveto",
        10, 0, 20, 5, 30, 0, 30, 0, 20, -5, 10, 0, 50, "flex",
        0, 30, "rmoveto",
        10, 20, 5, 20, 30, 10, 20, "hflex",
        0, 30, "rmoveto",
        10, 5, 20, 5, 30, 20, 10, -5, 10, "hflex1",
        0, 30, "rmoveto",
        10, 5, 20, 5, 30, 5, 20, -5, 10, -5, 15, "flex1",
        "endchar"])


def save(font, name):
    path = os.path.join(HERE, name)
    font.save(path)
    return path


def png(w, h, rgba):
    raw = b"".join(b"\0" + bytes(rgba) * w for _ in range(h))
    def chunk(t, d):
        return struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d) & 0xFFFFFFFF)
    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(raw)) + chunk(b"IEND", b""))


def variable(quad):
    """A two-master variable font (wght 300..700, default 400 via avar-like mapping)."""
    from fontTools.designspaceLib import DesignSpaceDocument, AxisDescriptor, SourceDescriptor, InstanceDescriptor
    from fontTools import varLib
    ext = "ttf" if quad else "otf"
    tmp = os.environ.get("FIXTURE_TMP", HERE)
    masters = []
    for w, s in ((300, 1.0), (700, 1.3)):
        fb = ttf("Spec Var", "Light" if w == 300 else "Bold", 0, w, s) if quad else otf("Spec VarCFF", "Light" if w == 300 else "Bold", 0, w, s)
        p = os.path.join(tmp, f"_master{w}.{ext}")
        fb.font.save(p)
        masters.append((w, p))
    ds = DesignSpaceDocument()
    a = AxisDescriptor()
    a.tag, a.name, a.minimum, a.default, a.maximum = "wght", "Weight", 300, 300, 700
    if quad:
        a.map = [(300, 300), (400, 380), (700, 700)]
    ds.addAxis(a)
    for w, p in masters:
        s = SourceDescriptor()
        s.path, s.location = p, {"Weight": w}
        ds.addSource(s)
    for w, nm in ((300, "Light"), (400, "Regular"), (700, "Bold")):
        i = InstanceDescriptor()
        i.styleName, i.location = nm, {"Weight": w}
        i.postScriptFontName = f"SpecVar-{nm}"
        ds.addInstance(i)
    font, _, _ = varLib.build(ds)
    for _, p in masters:
        os.remove(p)
    return font


KNOWN = [t.ljust(4) for t in woff2.woff2KnownTags]


def u255(v):
    if v < 253:
        return bytes([v])
    if v < 506:
        return bytes([255, v - 253])
    return bytes([253]) + struct.pack(">H", v)


def base128(v):
    out = [v & 0x7F]
    v >>= 7
    while v:
        out.append(0x80 | (v & 0x7F))
        v >>= 7
    return bytes(reversed(out))


def woff2_collection(ttc, dst):
    """A WOFF2 collection with untransformed tables (fontTools writes none)."""
    import brotli
    data = open(ttc, "rb").read()
    n = struct.unpack(">I", data[8:12])[0]
    offs = struct.unpack(">%dI" % n, data[12:12 + 4 * n])
    tables = []  # (tag, offset, length), unique by offset
    fonts = []
    for o in offs:
        flavor, nt = struct.unpack(">IH", data[o:o + 6])
        idx = []
        for i in range(nt):
            tag, _, off, ln = struct.unpack(">4sIII", data[o + 12 + 16 * i:o + 28 + 16 * i])
            key = (tag, off, ln)
            if key not in tables:
                tables.append(key)
            idx.append(tables.index(key))
        fonts.append((flavor, idx))
    dir_ = b""
    stream = b""
    for tag, off, ln in tables:
        t = tag.decode("latin-1")
        flags = KNOWN.index(t) if t in KNOWN else 63
        if t in ("glyf", "loca"):
            flags |= 3 << 6
        dir_ += bytes([flags]) + (tag if flags & 63 == 63 else b"") + base128(ln)
        stream += data[off:off + ln]
    coll = struct.pack(">I", 0x00010000) + u255(len(fonts))
    for flavor, idx in fonts:
        coll += u255(len(idx)) + struct.pack(">I", flavor) + b"".join(u255(i) for i in idx)
    comp = brotli.compress(stream)
    body = dir_ + coll
    total = 48 + len(body) + len(comp)
    total4 = (total + 3) & ~3
    hdr = struct.pack(">4sIIHHIIHHIIIII", b"wOF2", 0x74746366, total4, len(tables), 0,
                      len(data), len(comp), 1, 0, 0, 0, 0, 0, 0)
    open(dst, "wb").write(hdr + body + comp + b"\0" * (total4 - total))


def cid_font():
    """A CID-keyed CFF with two FDArray fonts picked by a format 3 FDSelect."""
    from fontTools.cffLib import FDArrayIndex, FontDict, FDSelect
    import copy
    order = [".notdef"] + ["cid%05d" % i for i in range(1, len(ORDER))]
    ren = dict(zip(ORDER, order))
    fb = FontBuilder(1000, isTTF=False)
    fb.setupGlyphOrder(order)
    fb.setupCharacterMap({c: ren[g] for c, g in CMAP.items()})
    cs = cff_glyphs()
    fb.setupCFF("SpecCID-Regular", {"FullName": "Spec CID"}, {ren[k]: v for k, v in cs.items()}, {})
    fb.setupHorizontalMetrics({ren[n]: (ADV[n], 0) for n in ORDER})
    fb.setupHorizontalHeader(ascent=800, descent=-200)
    fb.setupNameTable(names("Spec CID", "Regular"))
    fb.setupOS2(fsType=0, version=4, sxHeight=500, sCapHeight=700)
    fb.setupPost()
    f = fb.font
    cff = f["CFF "].cff
    top = cff.topDictIndex[0]
    top.ROS = ("Adobe", "Identity", 0)
    top.CIDCount = len(order)
    fda = FDArrayIndex()
    for i in range(2):
        fd = FontDict()
        fd.setCFF2(False)
        fd.Private = copy.deepcopy(top.Private)
        fd.FontName = "SpecCID-FD%d" % i
        fda.append(fd)
    top.FDArray = fda
    sel = FDSelect()
    sel.format = 3
    sel.gidArray = [0 if i < 6 else 1 for i in range(len(order))]
    top.FDSelect = sel
    for i, n in enumerate(order):
        c = top.CharStrings[n]
        c.private = fda[sel.gidArray[i]].Private
    del top.Private
    return f


def colr1_paints(sans):
    """COLR v1 paint graph fixture: "A" a radial gradient (red centre to
    blue) under a 90° rotation; "o" layers: a sweep gradient, then a solid green bar
    composited with DEST_OUT (the "one" bar cuts a hole); "a" a translated PaintColrGlyph of
    "o"."""
    f = TTFont(sans)
    solid = lambda i: {"Format": 2, "PaletteIndex": i, "Alpha": 1.0}
    stops = lambda a, b: {"ColorStop": [{"StopOffset": 0, "PaletteIndex": a}, {"StopOffset": 1, "PaletteIndex": b}]}
    radial = {"Format": 10, "Glyph": "A", "Paint": {
        "Format": 26, "angle": 90, "centerX": 300, "centerY": 350, "Paint": {
            "Format": 6, "ColorLine": stops(0, 1), "x0": 300, "y0": 350, "r0": 0, "x1": 300, "y1": 350, "r1": 400}}}
    sweep = {"Format": 10, "Glyph": "o", "Paint": {
        "Format": 8, "ColorLine": stops(0, 1), "centerX": 280, "centerY": 250, "startAngle": 0, "endAngle": 360}}
    hole = {"Format": 32, "CompositeMode": 8,
            "SourcePaint": {"Format": 10, "Glyph": "one", "Paint": solid(2)},
            "BackdropPaint": sweep}
    o = {"Format": 1, "Layers": [hole]}
    a = {"Format": 14, "dx": 40, "dy": 0, "Paint": {"Format": 11, "Glyph": "o"}}
    f["COLR"] = buildCOLR({"A": radial, "o": o, "a": a}, version=1)
    f["CPAL"] = buildCPAL([[(1, 0, 0, 1), (0, 0, 1, 1), (0, 0.6, 0, 1)]])
    return save(f, "colr1-paints.ttf")


def cbdt_tables(font, glyphs, ppem, rgba):
    """CBLC/CBDT bytes: one strike at `ppem`, index format 1, image format 17 (small metrics
    + PNG), a `ppem // 2` square per glyph sitting on the baseline."""
    order = font.getGlyphOrder()
    ids = sorted(order.index(g) for g in glyphs)
    first, last = ids[0], ids[-1]
    side = ppem // 2
    data = bytearray(struct.pack(">HH", 3, 0))
    offsets = []
    for gid in range(first, last + 1):
        offsets.append(len(data) - 4)
        if gid in ids:
            img = png(side, side, rgba)
            # height, width, bearingX, bearingY (top above the baseline), advance
            data += struct.pack(">BBbbB", side, side, 1, side, side + 2)
            data += struct.pack(">I", len(img)) + img
    offsets.append(len(data) - 4)
    # Index subtable: format 1, image format 17, offset of the glyph data in CBDT.
    sub = struct.pack(">HHI", 1, 17, 4) + b"".join(struct.pack(">I", o) for o in offsets)
    list_ = struct.pack(">HHI", first, last, 8) + sub
    line = struct.pack(">bbBbbbbbbbbb", side, 0, side, 0, 0, 0, 0, 0, 0, 0, 0, 0)
    size = struct.pack(">IIII", 8 + 48, len(list_), 1, 0) + line + line
    size += struct.pack(">HHBBBb", first, last, ppem, ppem, 32, 1)
    cblc = struct.pack(">HHI", 3, 0, 1) + size + list_
    return cblc, bytes(data)


def cbdt_fonts(sans):
    """`cbdt.ttf`: green CBDT bitmaps for "A" and "a" over their outlines; `cbdt-only.ttf`:
    the same with those two glyphs' outlines removed (a bitmap-only emoji font)."""
    from fontTools.ttLib.tables._g_l_y_f import Glyph
    for name, keep in (("cbdt.ttf", True), ("cbdt-only.ttf", False)):
        f = TTFont(sans)
        cblc, cbdt = cbdt_tables(f, ("A", "a"), 64, (0, 160, 0, 255))
        f["CBLC"] = newTable("CBLC")
        f["CBLC"].decompile(cblc, f)
        f["CBDT"] = newTable("CBDT")
        f["CBDT"].decompile(cbdt, f)
        if not keep:
            for g in ("A", "a"):
                f["glyf"][g] = Glyph()
        save(f, name)


def main():
    if sys.argv[1:] == ["cbdt"]:
        # Only the CBDT fixtures, leaving the others as they are.
        cbdt_fonts(os.path.join(HERE, "sans.ttf"))
        return
    out = {}
    # TrueType with composites; installable.
    f = ttf().font
    out["sans.ttf"] = save(f, "sans.ttf")
    # CFF with subroutines; preview & print.
    f = otf().font
    add_cff_subrs(f)
    out["serif.otf"] = save(f, "serif.otf")
    # Restricted, no subsetting, bitmap only flags.
    f = ttf("Spec Locked", "Bold", fs_type=0x0302, weight=700).font
    out["locked.ttf"] = save(f, "locked.ttf")
    f = otf("Spec Edit", "Italic", fs_type=0x0008).font
    out["edit.otf"] = save(f, "edit.otf")
    out["cid.otf"] = save(cid_font(), "cid.otf")
    # Variable fonts.
    f = variable(True)
    out["var.ttf"] = save(f, "var.ttf")
    f = variable(False)
    out["var-cff2.otf"] = save(f, "var-cff2.otf")
    # Collections.
    c = TTCollection()
    c.fonts = [TTFont(out["sans.ttf"]), TTFont(out["locked.ttf"])]
    c.save(os.path.join(HERE, "pair.ttc"))
    c = TTCollection()
    c.fonts = [TTFont(out["serif.otf"]), TTFont(out["edit.otf"])]
    c.save(os.path.join(HERE, "pair.otc"))
    # WOFF and WOFF2.
    for src, dst in (("sans.ttf", "sans.woff"), ("serif.otf", "serif.woff")):
        f = TTFont(out[src])
        f.flavor = "woff"
        from fontTools.ttLib.woff2 import WOFFFlavorData
        fd = WOFFFlavorData()
        fd.metaData = b'<?xml version="1.0" encoding="UTF-8"?><metadata version="1.0"><vendor name="Specular"/></metadata>'
        fd.privData = b"private!"
        f.flavorData = fd
        save(f, dst)
    for src, dst in (("sans.ttf", "sans.woff2"), ("serif.otf", "serif.woff2"), ("var.ttf", "var.woff2")):
        f = TTFont(out[src])
        f.flavor = "woff2"
        save(f, dst)
    woff2.compress(out["sans.ttf"], os.path.join(HERE, "sans-hmtx.woff2"), transform_tables={"glyf", "loca", "hmtx"})
    woff2_collection(os.path.join(HERE, "pair.ttc"), os.path.join(HERE, "pair.woff2"))
    # Colour: COLR v0 + CPAL, COLR v1, sbix.
    f = TTFont(out["sans.ttf"])
    f["COLR"] = buildCOLR({"A": [("A", 0), ("acute", 1)], "o": [("o", 2)]}, version=0)
    f["CPAL"] = buildCPAL([[(1, 0, 0, 1), (0, 0.5, 0, 1), (0, 0, 1, 0.5)], [(0, 0, 0, 1), (1, 1, 1, 1), (0.5, 0.5, 0.5, 1)]])
    save(f, "colr0.ttf")
    f = TTFont(out["sans.ttf"])
    paint = {"Format": 10, "Glyph": "A", "Paint": {
        "Format": 4, "ColorLine": {"ColorStop": [{"StopOffset": 0, "PaletteIndex": 0}, {"StopOffset": 1, "PaletteIndex": 1}]},
        "x0": 0, "y0": 0, "x1": 600, "y1": 700, "x2": 0, "y2": 700}}
    f["COLR"] = buildCOLR({"A": paint, "o": {"Format": 10, "Glyph": "o", "Paint": {"Format": 2, "PaletteIndex": 2, "Alpha": 1.0}}}, version=1)
    f["CPAL"] = buildCPAL([[(1, 0, 0, 1), (0, 0, 1, 1), (0, 0.6, 0, 1)]])
    save(f, "colr1.ttf")
    f = TTFont(out["sans.ttf"])
    sbix = newTable("sbix")
    sbix.version, sbix.flags, sbix.strikes = 1, 1, {}
    for ppem in (20, 40):
        st = sbixStrike.Strike(ppem=ppem, resolution=72)
        for g in ("A", "a"):
            st.glyphs[g] = sbixGlyph.Glyph(glyphName=g, graphicType="png ", imageData=png(ppem // 4, ppem // 4, (255, 0, 0, 255)))
        sbix.strikes[ppem] = st
    f["sbix"] = sbix
    save(f, "sbix.ttf")

    colr1_paints(out["sans.ttf"])
    cbdt_fonts(out["sans.ttf"])

    # Oracle data: fontTools' WOFF2 decompression (it reads no WOFF2 collections).
    for name in sorted(os.listdir(HERE)):
        if name.endswith(".woff2") and not name.startswith("pair"):
            f = TTFont(os.path.join(HERE, name))
            f.flavor = None
            f.save(os.path.join(EXP, name + ".sfnt"), reorderTables=True)


if __name__ == "__main__":
    main()
