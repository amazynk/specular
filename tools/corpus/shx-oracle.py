#!/usr/bin/env python3
"""Independent SHX reader for oracle runs: prints JSON with, per font and shape number, the
pen's end and the bounding box of the strokes (shape units, arcs sampled every degree).

Written separately from specular-vector's reader, from the published shape description, as a
second implementation to compare against:

    python3 -I tools/corpus/shx-oracle.py FONT.shx... > oracle.json
    SHX_ORACLE=oracle.json cargo test -p specular-vector --test shx_fonts -- --ignored

tools/corpus/shx-oracle.mjs does the same with @mlightcad/shx-parser where running it is allowed.
"""

import json
import math
import os
import struct
import sys

DIRS = [(1, 0), (1, .5), (1, 1), (.5, 1), (0, 1), (-.5, 1), (-1, 1), (-1, .5), (-1, 0),
        (-1, -.5), (-1, -1), (-.5, -1), (0, -1), (.5, -1), (1, -1), (1, -.5)]


def read_font(b):
    end = b.index(b"\r\n\x1a")
    sig = b[:end].decode("latin-1").lower()
    p = end + 3
    shapes, kind, ext = {}, None, None
    if " shapes " in sig:
        kind = "shapes"
        n = struct.unpack_from("<H", b, p + 4)[0]
        idx = [struct.unpack_from("<2H", b, p + 6 + 4 * i) for i in range(n)]
        q = p + 6 + 4 * n
        for code, ln in idx:
            data = b[q:q + ln]
            q += ln
            shapes[code] = data if code == 0 else data[data.index(0) + 1:]
    elif " bigfont " in sig:
        kind = "bigfont"
        _, n, nr = struct.unpack_from("<3H", b, p)
        q = p + 6 + 4 * nr
        for i in range(n):
            code, ln, off = struct.unpack_from("<2HI", b, q + 8 * i)
            if code or ln or off:
                shapes[code] = b[off:off + ln]
    else:
        kind = "unifont"
        n, il = struct.unpack_from("<IH", b, p)
        shapes[0] = b[p + 6:p + 6 + il]
        q = p + 6 + il
        for _ in range(n - 1):
            code, ln = struct.unpack_from("<2H", b, q)
            data = b[q + 4:q + 4 + ln]
            q += 4 + ln
            shapes[code] = data[data.index(0) + 1:]
    info = shapes[0][shapes[0].index(0) + 1:]
    if kind == "bigfont" and len(info) >= 5:
        ext = (info[0], info[3])
    return kind, shapes, ext, info[0]


class Pen:
    def __init__(self, font):
        self.kind, self.shapes, self.ext, self.above = font
        self.x = self.y = 0.0
        self.down = True
        self.scale = 1.0
        self.sx = self.sy = 1.0
        self.stack = []
        self.pts = []

    def to(self, x, y):
        if self.down and (x, y) != (self.x, self.y):
            self.pts += [(self.x, self.y), (x, y)]
        self.x, self.y = x, y

    def arc(self, cx, cy, rx, ry, a0, sweep):
        steps = max(1, int(abs(math.degrees(sweep))))
        for k in range(1, steps + 1):
            a = a0 + sweep * k / steps
            self.to(cx + rx * math.cos(a), cy + ry * math.sin(a))

    def run(self, s, depth=0, skip_next=False):
        i = 0
        sb = lambda j: struct.unpack("b", bytes([s[j]]))[0] if j < len(s) else 0
        ub = lambda j: s[j] if j < len(s) else 0
        while i < len(s):
            c = s[i]
            start = i
            if c >> 4:
                dx, dy = DIRS[c & 15]
                n = c >> 4
                i += 1
                if not skip_next:
                    k = self.scale
                    self.to(self.x + dx * n * k * self.sx, self.y + dy * n * k * self.sy)
            elif c == 0:
                return False
            elif c in (1, 2):
                if not skip_next:
                    self.down = c == 1
                i += 1
            elif c in (3, 4):
                if not skip_next and ub(i + 1):
                    self.scale = self.scale / ub(i + 1) if c == 3 else self.scale * ub(i + 1)
                i += 2
            elif c == 5:
                if not skip_next:
                    self.stack.append((self.x, self.y))
                i += 1
            elif c == 6:
                if not skip_next and self.stack:
                    self.x, self.y = self.stack.pop()
                i += 1
            elif c == 7:
                if self.kind == "unifont":
                    code, i, place = ub(i + 1) << 8 | ub(i + 2), i + 3, None
                elif self.kind == "bigfont" and ub(i + 1) == 0:
                    code = ub(i + 2) << 8 | ub(i + 3)
                    place = (sb(i + 4), sb(i + 5), ub(i + 6), ub(i + 7))
                    i += 8
                else:
                    code, i, place = ub(i + 1), i + 2, None
                if not skip_next and depth < 8 and code in self.shapes:
                    if place is None:
                        self.run(self.shapes[code], depth + 1)
                    else:
                        bx, by = self.x, self.y
                        saved = (self.sx, self.sy, self.down)
                        cell = self.ext[0] if self.ext else self.above
                        self.x += place[0] * self.scale * self.sx
                        self.y += place[1] * self.scale * self.sy
                        self.sx *= place[2] / cell if place[2] else 1
                        self.sy *= place[3] / cell if place[3] else 1
                        self.down = True
                        self.run(self.shapes[code], depth + 1)
                        self.sx, self.sy, self.down = saved
                        self.x, self.y = bx, by
            elif c == 8:
                if not skip_next:
                    k = self.scale
                    self.to(self.x + sb(i + 1) * k * self.sx, self.y + sb(i + 2) * k * self.sy)
                i += 3
            elif c in (9, 13):
                j = i + 1
                while j + 1 < len(s):
                    x, y = sb(j), sb(j + 1)
                    if x == 0 and y == 0:
                        j += 2
                        break
                    if c == 9:
                        j += 2
                        if not skip_next:
                            self.disp(x, y, 0)
                    else:
                        if not skip_next:
                            self.disp(x, y, sb(j + 2))
                        j += 3
                i = j
            elif c == 10:
                if not skip_next:
                    r = ub(i + 1) * self.scale
                    self.octant(r, ub(i + 2), 0, 0)
                i += 3
            elif c == 11:
                if not skip_next:
                    r = (ub(i + 3) * 256 + ub(i + 4)) * self.scale
                    self.octant(r, ub(i + 5), ub(i + 1), ub(i + 2), frac=True)
                i += 6
            elif c == 12:
                if not skip_next:
                    self.disp(sb(i + 1), sb(i + 2), sb(i + 3))
                i += 4
            elif c == 14:
                # Horizontal text: the next command is skipped.
                i += 1 + (command_len(s[i + 1:], self.kind) if i + 1 < len(s) else 0)
            else:
                i += 1
            if skip_next:
                return i - start
        return False

    def disp(self, x, y, b):
        k = self.scale
        dx, dy = x * k * self.sx, y * k * self.sy
        if b == 0:
            self.to(self.x + dx, self.y + dy)
            return
        t = b / 127.0
        sweep = 4 * math.atan(t)
        chord = math.hypot(dx, dy)
        mx, my = self.x + dx / 2, self.y + dy / 2
        nx, ny = -dy / chord, dx / chord
        h = chord / 2 * (1 - t * t) / (2 * t)
        cx, cy = mx + nx * h, my + ny * h
        r = math.hypot(self.x - cx, self.y - cy)
        a0 = math.atan2(self.y - cy, self.x - cx)
        self.arc(cx, cy, r, r, a0, sweep)
        self.x, self.y = mx + dx / 2, my + dy / 2

    def octant(self, r, sc, so, eo, frac=False):
        v = struct.unpack("b", bytes([sc]))[0]
        d = -1 if v < 0 else 1
        a = abs(v)
        s, c = (a >> 4) & 7, (a & 7) or 8
        oct_ = math.pi / 4
        if frac:
            whole = c - 1 if eo else c
            a0 = s * oct_ + d * so * oct_ / 256
            a1 = (s + d * whole) * oct_ + d * eo * oct_ / 256
        else:
            a0, a1 = s * oct_, (s + d * c) * oct_
        rx, ry = r * self.sx, r * self.sy
        cx, cy = self.x - rx * math.cos(a0), self.y - ry * math.sin(a0)
        self.arc(cx, cy, rx, ry, a0, a1 - a0)


def command_len(s, kind):
    c = s[0]
    if c >> 4 or c in (1, 2, 5, 6):
        return 1
    if c in (3, 4):
        return 2
    if c == 7:
        if kind == "unifont":
            return 3
        if kind == "bigfont" and len(s) > 1 and s[1] == 0:
            return 8
        return 2
    if c == 8:
        return 3
    if c in (9, 13):
        j = 1
        while j + 1 < len(s):
            if s[j] == 0 and s[j + 1] == 0:
                return j + 2
            j += 2 if c == 9 else 3
        return j
    return {10: 3, 11: 6, 12: 4, 14: 1 + command_len(s[1:], kind)}.get(c, 1)


def main():
    out = {}
    for path in sys.argv[1:]:
        font = read_font(open(path, "rb").read())
        glyphs = {}
        for code, spec in font[1].items():
            if code == 0:
                continue
            p = Pen(font)
            p.run(spec)
            xs = [q[0] for q in p.pts]
            ys = [q[1] for q in p.pts]
            glyphs[code] = {
                "end": [p.x, p.y],
                "bbox": [min(xs), min(ys), max(xs), max(ys)] if xs else None,
            }
        out[os.path.basename(path)] = glyphs
    json.dump(out, sys.stdout, indent=1)


if __name__ == "__main__":
    main()
