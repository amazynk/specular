#!/usr/bin/env python3
"""Writes the GDSII test fixtures into tests/data/ and their oracle expectations.

Layouts that gdstk can write are made with it; records it never writes (BOX, NODE,
PROPATTR on elements, pathtype 4, absolute transforms, cycles, odd and cut-off files)
are written by hand. Every fixture stays well under 200 KB.

    python3 -m venv $CARGO_TARGET_DIR/venv && $CARGO_TARGET_DIR/venv/bin/pip install gdstk
    $CARGO_TARGET_DIR/venv/bin/python tests/gen_fixtures.py
"""

import math
import os
import struct
import subprocess
import sys

import gdstk

HERE = os.path.dirname(os.path.abspath(__file__))
DATA = os.path.join(HERE, "data")


# ---------------------------------------------------------------- raw record writer

def real8(v):
    if v == 0:
        return b"\0" * 8
    sign = 0x80 if v < 0 else 0
    m = abs(v)
    e = 64
    while m >= 1:
        m /= 16
        e += 1
    while m < 1 / 16:
        m *= 16
        e -= 1
    mant = int(round(m * 2**56))
    return bytes([sign | e]) + mant.to_bytes(8, "big")[1:]


class W:
    def __init__(self):
        self.b = bytearray()

    def rec(self, rtype, dtype, data=b""):
        self.b += struct.pack(">HBB", 4 + len(data), rtype, dtype) + data
        return self

    def i16(self, rtype, *v):
        return self.rec(rtype, 2, b"".join(struct.pack(">h", x) for x in v))

    def u16(self, rtype, *v):
        return self.rec(rtype, 2, b"".join(struct.pack(">H", x) for x in v))

    def i32(self, rtype, *v):
        return self.rec(rtype, 3, b"".join(struct.pack(">i", x) for x in v))

    def real(self, rtype, *v):
        return self.rec(rtype, 5, b"".join(real8(x) for x in v))

    def s(self, rtype, text):
        d = text.encode("latin-1")
        if len(d) % 2:
            d += b"\0"
        return self.rec(rtype, 6, d)

    def none(self, rtype):
        return self.rec(rtype, 0)

    def head(self, name="RAW", units=(1e-3, 1e-9)):
        return (self.i16(0x00, 600).i16(0x01, *([2026, 10, 11, 12, 0, 0] * 2))
                .s(0x02, name).real(0x03, *units))

    def cell(self, name):
        return self.i16(0x05, *([2026, 10, 11, 12, 0, 0] * 2)).s(0x06, name)

    def end_cell(self):
        return self.none(0x07)

    def boundary(self, layer, dt, pts, props=()):
        self.none(0x08).u16(0x0D, layer).u16(0x0E, dt)
        flat = [c for p in pts + [pts[0]] for c in p]
        self.i32(0x10, *flat)
        for attr, value in props:
            self.i16(0x2B, attr).s(0x2C, value)
        return self.none(0x11)

    def box(self, layer, boxtype, x0, y0, x1, y1):
        return (self.none(0x2D).u16(0x0D, layer).u16(0x2E, boxtype)
                .i32(0x10, x0, y0, x1, y0, x1, y1, x0, y1, x0, y0).none(0x11))

    def node(self, layer, nodetype, pts):
        self.none(0x15).u16(0x0D, layer).u16(0x2A, nodetype)
        return self.i32(0x10, *[c for p in pts for c in p]).none(0x11)

    def path(self, layer, dt, width, pts, pathtype=0, ext=None):
        self.none(0x09).u16(0x0D, layer).u16(0x0E, dt).i16(0x21, pathtype).i32(0x0F, width)
        if ext is not None:
            self.i32(0x30, ext[0]).i32(0x31, ext[1])
        return self.i32(0x10, *[c for p in pts for c in p]).none(0x11)

    def strans(self, reflect, mag, angle, abs_mag=False, abs_angle=False):
        bits = (0x8000 if reflect else 0) | (4 if abs_mag else 0) | (2 if abs_angle else 0)
        if bits or mag != 1 or angle:
            self.u16(0x1A, bits)
            if mag != 1:
                self.real(0x1B, mag)
            if angle:
                self.real(0x1C, angle)
        return self

    def sref(self, name, x, y, reflect=False, mag=1, angle=0, **kw):
        self.none(0x0A).s(0x12, name).strans(reflect, mag, angle, **kw)
        return self.i32(0x10, x, y).none(0x11)

    def aref(self, name, cols, rows, p1, p2, p3, reflect=False, mag=1, angle=0):
        self.none(0x0B).s(0x12, name).strans(reflect, mag, angle).i16(0x13, cols, rows)
        return self.i32(0x10, *p1, *p2, *p3).none(0x11)

    def text(self, layer, tt, x, y, string, pres=0, mag=1, angle=0, reflect=False):
        self.none(0x0C).u16(0x0D, layer).u16(0x16, tt).u16(0x17, pres)
        self.strans(reflect, mag, angle)
        return self.i32(0x10, x, y).s(0x19, string).none(0x11)

    def end(self):
        return bytes(self.none(0x04).b)


# ---------------------------------------------------------------- fixtures by gdstk

def basic():
    lib = gdstk.Library("BASIC", unit=1e-6, precision=1e-9)
    leaf = lib.new_cell("LEAF")
    leaf.add(gdstk.rectangle((0, 0), (2, 1), layer=1, datatype=0))
    leaf.add(gdstk.Polygon([(0, 0), (1, 3), (2, 0.5), (1.5, -1)], layer=2, datatype=3))
    leaf.add(gdstk.regular_polygon((5, 5), 1.5, 7, layer=3))
    mid = lib.new_cell("MID")
    mid.add(gdstk.rectangle((-1, -1), (10, 8), layer=10, datatype=1))
    for i, (rot, refl, mag) in enumerate([(0, False, 1), (math.pi / 2, False, 1),
                                          (math.pi, True, 2), (3 * math.pi / 2, True, 0.5),
                                          (math.radians(30), False, 1.5),
                                          (math.radians(-135), True, 1)]):
        mid.add(gdstk.Reference(leaf, (20 * i, 3), rotation=rot, x_reflection=refl,
                                magnification=mag))
    top = lib.new_cell("TOP")
    top.add(gdstk.Reference(mid, (0, 0)))
    top.add(gdstk.Reference(mid, (0, 50), rotation=math.pi / 2, x_reflection=True))
    top.add(gdstk.rectangle((-5, -5), (200, -2), layer=63, datatype=0))
    # A second, unplaced cell: two top cells.
    other = lib.new_cell("OTHER")
    other.add(gdstk.Reference(leaf, (0, 0), magnification=3))
    other.add(gdstk.ellipse((0, 0), 4, inner_radius=2, layer=7))
    return lib


def arrays():
    lib = gdstk.Library("ARRAYS", unit=1e-6, precision=1e-9)
    unit = lib.new_cell("UNIT")
    unit.add(gdstk.rectangle((0, 0), (0.8, 0.5), layer=1))
    unit.add(gdstk.Polygon([(0, 0), (0.4, 0.9), (0.8, 0)], layer=2, datatype=5))
    top = lib.new_cell("TOP")
    cases = [(0, False, 1), (math.pi / 2, False, 1), (math.pi, True, 1),
             (math.pi / 2, True, 2), (math.radians(45), False, 1),
             (math.radians(200), True, 0.75)]
    for i, (rot, refl, mag) in enumerate(cases):
        top.add(gdstk.Reference(unit, (40 * i, 0), rotation=rot, x_reflection=refl,
                                magnification=mag, columns=4 + i, rows=3,
                                spacing=(1.5, 2.0)))
    # An array of arrays: 8 × 8 × 4 × 3 placements.
    big = lib.new_cell("BIG")
    big.add(gdstk.Reference(unit, (0, 0), columns=4, rows=3, spacing=(1, 1)))
    top.add(gdstk.Reference(big, (0, 100), columns=8, rows=8, spacing=(5, 4)))
    return lib


def paths():
    lib = gdstk.Library("PATHS", unit=1e-6, precision=1e-9)
    top = lib.new_cell("TOP")
    spines = [
        [(0, 0), (10, 0), (10, 10)],
        [(0, 0), (10, 0), (0, 2)],              # sharp turn: clipped join
        [(0, 0), (10, 0), (20, 0), (20, -5)],   # collinear point
        [(0, 0), (5, 5), (10, 0), (15, 5), (20, 0)],
        [(0, 0), (10, 3), (3, 8), (12, 12), (-4, 9)],
        [(0, 0), (10, 0), (0, 0.5)],            # nearly turning back
        [(0, 0), (7, 0)],
    ]
    ends = ["flush", "extended", (0.7, 1.9), "round"]
    y = 0
    for k, sp in enumerate(spines):
        for j, e in enumerate(ends):
            pts = [(x + 30 * j, yy + y) for x, yy in sp]
            layer = 20 + j
            top.add(gdstk.FlexPath(pts, 1.0 + 0.25 * k, ends=e, joins="natural",
                                   simple_path=True, layer=layer, datatype=k))
        y += 30
    # Absolute width (written negative); gdstk scales it under magnification anyway, so
    # the magnified case is in absolute().
    cell = lib.new_cell("ABSW")
    cell.add(gdstk.FlexPath([(0, 0), (5, 0), (5, 5)], 0.5, simple_path=True,
                            scale_width=False, layer=30))
    cell.add(gdstk.FlexPath([(0, 2), (5, 2)], 0.5, simple_path=True, layer=31))
    top.add(gdstk.Reference(cell, (0, -20), rotation=math.pi / 2))
    return lib


def texts():
    lib = gdstk.Library("TEXTS", unit=1e-6, precision=1e-9)
    top = lib.new_cell("TOP")
    top.add(gdstk.rectangle((0, 0), (50, 20), layer=1))
    anchors = ["n", "s", "e", "w", "o", "ne", "nw", "se", "sw"]
    for i, a in enumerate(anchors):
        top.add(gdstk.Label("L%d_%s" % (i, a), (5 * i, 10), anchor=a,
                            rotation=math.radians(15 * i), magnification=0.5 + i / 4,
                            x_reflection=i % 2 == 1, layer=60 + i % 3, texttype=i))
    return lib


# ---------------------------------------------------------------- fixtures by hand

def raw():
    w = W().head("RAW")
    w.cell("CHILD")
    w.boundary(1, 0, [(0, 0), (1000, 0), (1000, 500), (0, 500)],
               props=[(1, "net=VDD"), (2, "x")])
    w.box(2, 7, -100, -100, 300, 50)
    w.node(3, 0, [(0, 0), (10, 10)])
    w.path(4, 1, 100, [(0, 0), (500, 0), (500, 700)], pathtype=4, ext=(30, 80))
    w.path(4, 2, -60, [(0, 900), (400, 900)], pathtype=2)
    w.path(4, 3, 40, [(0, 1100), (300, 1100), (300, 1500)], pathtype=0)
    w.boundary(40000, 65535, [(0, 0), (-50, 0), (-50, -60)])  # unsigned 16-bit numbers
    w.text(5, 2, 10, 20, "LABEL", pres=0b0110, mag=0.25, angle=90)
    w.end_cell()
    w.cell("TOP")
    w.sref("CHILD", 0, 0)
    w.sref("CHILD", 5000, 0, reflect=True, mag=2, angle=90)
    w.sref("CHILD", 0, 5000, angle=270)
    w.sref("CHILD", 9000, 9000, reflect=True, angle=33.5, mag=0.5)
    w.aref("CHILD", 3, 2, (20000, 0), (20000 + 3 * 1500, 0), (20000, 2 * 2000))
    w.aref("CHILD", 2, 4, (40000, 0), (40000, 2 * 1700), (40000 - 4 * 1200, 0),
           reflect=True, angle=90, mag=1.5)
    w.aref("CHILD", 3, 3, (0, 30000), (3 * 1000, 3 * 1000), (-3 * 1000, 3 * 1000),
           angle=45)
    # Element properties on a reference.
    w.none(0x0A).s(0x12, "CHILD").i32(0x10, 70000, 0).i16(0x2B, 5).s(0x2C, "p").none(0x11)
    w.end_cell()
    return w.end()


def absolute():
    """Absolute magnification and rotation (gdstk ignores them; checked by hand)."""
    w = W().head("ABS")
    w.cell("LEAF").boundary(1, 0, [(0, 0), (100, 0), (100, 10), (0, 10)]).end_cell()
    w.cell("MID").sref("LEAF", 0, 0, mag=1, angle=0, abs_mag=True, abs_angle=True).end_cell()
    w.cell("AW").path(2, 0, -10, [(0, 0), (100, 0)]).end_cell()
    w.cell("TOP").sref("MID", 1000, 0, mag=2, angle=90).sref("AW", 0, 500, mag=3).end_cell()
    return w.end()


def cycle():
    w = W().head("CYCLE")
    w.cell("A").boundary(1, 0, [(0, 0), (10, 0), (10, 10)]).sref("B", 100, 0).end_cell()
    w.cell("B").boundary(2, 0, [(0, 0), (10, 0), (10, 10)]).sref("C", 0, 100).end_cell()
    w.cell("C").sref("A", 50, 50).sref("MISSING", 0, 0).end_cell()
    w.cell("SELF").sref("SELF", 1, 1).end_cell()
    return w.end()


def deep():
    """A deep hierarchy whose flat shape count runs into the billions."""
    w = W().head("DEEP")
    w.cell("L0").boundary(1, 0, [(0, 0), (8, 0), (8, 8), (0, 8)]).end_cell()
    for i in range(1, 13):
        s = 10 * 4**(i - 1)
        w.cell("L%d" % i)
        w.aref("L%d" % (i - 1), 4, 4, (0, 0), (4 * s, 0), (0, 4 * s))
        w.boundary(2 + i % 3, i, [(0, 0), (4 * s, 0), (0, -s)])
        w.end_cell()
    return w.end()


def main():
    os.makedirs(DATA, exist_ok=True)
    made = []
    for name, f in [("basic", basic), ("arrays", arrays), ("paths", paths),
                    ("texts", texts)]:
        p = os.path.join(DATA, name + ".gds")
        f().write_gds(p, timestamp=__import__("datetime").datetime(2026, 10, 11))
        made.append(p)
    for name, f in [("raw", raw), ("absolute", absolute), ("cycle", cycle),
                    ("deep", deep)]:
        p = os.path.join(DATA, name + ".gds")
        with open(p, "wb") as fh:
            fh.write(f())
        if name in ("raw", "deep"):
            made.append(p)
    full = raw()
    with open(os.path.join(DATA, "truncated.gds"), "wb") as fh:
        fh.write(full[: len(full) * 2 // 3 + 1])
    with open(os.path.join(DATA, "odd.gds"), "wb") as fh:
        fh.write(full[:42] + b"\x00\x07\x08\x00\x00" + full[42:])
    with open(os.path.join(DATA, "tiny.oas"), "wb") as fh:
        fh.write(b"%SEMI-OASIS\r\n\x01\x031.0\x00\x00\x00")
    oracle = os.path.join(HERE, "oracle.py")
    for p in made:
        subprocess.run([sys.executable, "-I", oracle, p, p[:-4] + ".expect"], check=True)
    for p in sorted(os.listdir(DATA)):
        print("%8d %s" % (os.path.getsize(os.path.join(DATA, p)), p))


if __name__ == "__main__":
    main()
