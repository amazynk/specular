#!/usr/bin/env python3
"""Writes the SHX test fonts in crates/specular-vector/tests/data/shx/.

Our own shapes, compiled by this script into the three binary layouts AutoCAD uses
(shapes 1.0, bigfont 1.0 incl. the extended form, unifont 1.0), following the published
shape description. No Autodesk font or code is involved.

    python3 -I tools/corpus/shx-fixtures.py
"""

import os
import struct
import sys

OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..",
                   "crates", "specular-vector", "tests", "data", "shx")


def v(length, direction):
    """A vector byte: length 1-15 in one of 16 directions (0 east, 4 north...)."""
    assert 1 <= length <= 15 and 0 <= direction <= 15
    return [length << 4 | direction]


def s8(x):
    assert -128 <= x <= 127
    return x & 0xFF


def disp(x, y):
    return [8, s8(x), s8(y)]


def disps(*pts):
    out = [9]
    for x, y in pts:
        out += [s8(x), s8(y)]
    return out + [0, 0]


def octarc(radius, start, count, cw=False):
    b = start << 4 | (count & 7)
    return [10, radius, s8(-b) if cw else b]


def fracarc(start_off, end_off, radius, start, count, cw=False):
    b = start << 4 | (count & 7)
    return [11, start_off, end_off, radius >> 8, radius & 0xFF, s8(-b) if cw else b]


def bulge(x, y, b):
    return [12, s8(x), s8(y), s8(b)]


def polyarc(*segs):
    out = [13]
    for x, y, b in segs:
        out += [s8(x), s8(y), s8(b)]
    return out + [0, 0]


UP, DOWN = [1], [2]

# Text font: cap height 6, descender 2. Every shape ends where the next starts.
TEXT = {
    32: ("spc", DOWN + v(6, 0)),
    48: ("0", DOWN + disp(3, 0) + UP + polyarc((0, 6, 90), (0, -6, 90)) + DOWN + disp(3, 0)),
    49: ("1", DOWN + disp(1, 5) + UP + v(1, 2) + v(6, 12) + DOWN + disp(3, 0)),
    65: ("A", UP + v(6, 3) + v(6, 13) + DOWN + disp(-5, 2) + UP + v(4, 0) + DOWN + disp(2, -2)),
    66: ("B", UP + v(6, 4) + v(2, 0) + octarc(1, 2, 4, cw=True) + octarc(1, 2, 4, cw=True)
           + v(2, 8) + DOWN + disp(5, 0)),
    67: ("C", DOWN + disp(4, 6) + UP + fracarc(128, 128, 3, 1, 7) + DOWN + disp(1, -2)),
    68: ("D", UP + v(6, 4) + bulge(0, -6, -127) + DOWN + disp(5, 0)),
    69: ("E", [5] + UP + v(4, 0) + DOWN + [6] + [5] + UP + v(6, 4) + v(4, 0) + DOWN + [6]
           + disp(0, 3) + UP + v(3, 0) + DOWN + disp(3, -3)),
    72: ("H", UP + v(6, 4) + DOWN + disp(0, -3) + UP + v(4, 0) + DOWN + disp(0, 3)
           + UP + v(6, 12) + DOWN + v(2, 0)),
    73: ("I", UP + v(6, 4) + DOWN + disp(2, -6)),
    76: ("L", DOWN + v(6, 4) + UP + v(6, 12) + v(4, 0) + DOWN + v(2, 0)),
    79: ("O", DOWN + disp(6, 3) + UP + octarc(3, 0, 0) + DOWN + disp(2, -3)),
    83: ("S", DOWN + disp(4, 6) + UP + [3, 2] + polyarc((-4, 0, 60), (0, -6, 127), (0, -6, -127),
                                                       (-4, 0, 0)) + [4, 2] + DOWN + disp(6, 0)),
    84: ("T", DOWN + v(6, 4) + UP + v(4, 0) + DOWN + disp(-2, 0) + UP + v(6, 12)
           + DOWN + disp(4, 0)),
    # Scale codes: the bar of a half-size "T" drawn by multiplying back.
    90: ("Z", DOWN + v(6, 4) + UP + [3, 2] + v(8, 0) + [4, 2] + v(6, 10) + v(4, 0)
           + DOWN + v(2, 0)),
    # Vertical-text-only displacement (skipped) and a subshape.
    95: ("_", [14] + disp(-3, -8) + [7, 73] + UP + v(4, 0) + DOWN + v(2, 0)),
    127: ("deg", DOWN + disp(1, 5) + UP + octarc(1, 4, 0) + DOWN + disp(3, -5)),
    128: ("pm", DOWN + disp(0, 3) + UP + v(4, 0) + DOWN + disp(-2, -2) + UP + v(4, 4)
            + DOWN + disp(4, -5) + UP + v(4, 8) + DOWN + disp(6, 0)),
}


def shapes_file(info, shapes):
    entries = [(0, info)]
    for code in sorted(shapes):
        name, spec = shapes[code]
        entries.append((code, name.encode() + b"\0" + bytes(spec + [0])))
    out = b"AutoCAD-86 shapes 1.0\r\n\x1a"
    codes = [e[0] for e in entries]
    out += struct.pack("<3H", min(codes), max(codes), len(entries))
    for code, data in entries:
        out += struct.pack("<2H", code, len(data))
    for _, data in entries:
        out += data
    return out


def bigfont_file(info, shapes, ranges):
    head = b"AutoCAD-86 bigfont 1.0\r\n\x1a"
    entries = [(0, info)] + [(c, bytes(shapes[c] + [0])) for c in sorted(shapes)]
    # One empty index slot, as compilers leave them.
    n = len(entries) + 1
    index_len = 6 + 4 * len(ranges) + 8 * n
    out = head + struct.pack("<3H", index_len, n, len(ranges))
    for lo, hi in ranges:
        out += struct.pack("<2H", lo, hi)
    off = len(out) + 8 * n
    index = b""
    data = b""
    for code, d in entries:
        index += struct.pack("<2HI", code, len(d), off + len(data))
        data += d
    index += struct.pack("<2HI", 0, 0, 0)
    return out + index + data


def unifont_file(info, shapes):
    out = b"AutoCAD-86 unifont 1.0\r\n\x1a"
    out += struct.pack("<IH", len(shapes) + 1, len(info)) + info
    for code in sorted(shapes):
        name, spec = shapes[code]
        d = name.encode() + b"\0" + bytes(spec + [0])
        out += struct.pack("<2H", code, len(d)) + d
    return out


def main():
    os.makedirs(OUT, exist_ok=True)
    fonts = {}
    fonts["eng2-text.shx"] = shapes_file(b"Specular test text\0" + bytes([6, 2, 0, 0]), TEXT)

    # A Big Font for Shift-JIS: 日 (0x93FA) and 本 (0x967B) as boxes, cap height 8.
    big = {
        0x93FA: DOWN + disp(1, 0) + UP + v(8, 4) + v(6, 0) + v(8, 12) + v(6, 8) + DOWN
        + disp(0, 4) + UP + v(6, 0) + DOWN + disp(3, -4),
        0x967B: DOWN + disp(1, 6) + UP + v(8, 0) + DOWN + disp(-4, 2) + UP + v(10, 12)
        + DOWN + disp(-3, 3) + UP + v(6, 0) + DOWN + disp(2, -3),
    }
    fonts["eng2-big.shx"] = bigfont_file(b"Specular test big\0" + bytes([8, 0, 0, 0]), big,
                                         [(0x81, 0x9F), (0xE0, 0xEF)])

    # An extended Big Font: one primitive (a box on a 16-unit cell) placed twice.
    ext = {
        0x8CFB: UP + v(15, 4) + v(1, 4) + v(15, 0) + v(1, 0) + v(15, 12) + v(1, 12)
        + v(15, 8) + v(1, 8),
        # 日: the box, full height, left half; and a half-size box at the right.
        0x93FA: [7, 0, 0x8C, 0xFB, 0, 0, 8, 16] + [7, 0, 0x8C, 0xFB, 8, 4, 8, 8] + DOWN
        + disp(16, 0),
    }
    fonts["eng2-extbig.shx"] = bigfont_file(
        b"Specular test extended\0" + bytes([16, 0, 0, 16, 0]), ext, [(0x81, 0x9F)])

    uni = {
        0x0041: ("A", UP + v(6, 3) + v(6, 13) + DOWN + disp(-5, 2) + UP + v(4, 0) + DOWN
                 + disp(2, -2)),
        0x0020: ("spc", DOWN + v(6, 0)),
        0x03A9: ("omega", DOWN + disp(1, 0) + UP + v(2, 0) + polyarc((0, 2, 0), (4, 0, -127),
                                                                   (0, -2, 0)) + v(2, 0)
                 + DOWN + disp(1, 0)),
        0x2205: ("diam", DOWN + disp(4, 3) + UP + octarc(3, 0, 0) + DOWN + disp(-5, -3)
                 + UP + v(6, 2) + DOWN + disp(3, -6)),
        # A subshape by two-byte number.
        0x00C5: ("Aring", [7, 0x00, 0x41] + DOWN + disp(-5, 7) + UP + octarc(1, 6, 0)
                 + DOWN + disp(5, -7)),
    }
    fonts["eng2-uni.shx"] = unifont_file(
        b"Specular test unicode\0" + bytes([6, 2, 0, 0, 0, 0]), uni)

    for name, data in fonts.items():
        with open(os.path.join(OUT, name), "wb") as f:
            f.write(data)
        print(f"{name}: {len(data)} bytes")


if __name__ == "__main__":
    sys.exit(main())
