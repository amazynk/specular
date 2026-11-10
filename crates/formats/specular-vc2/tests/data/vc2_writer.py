"""A small own VC-2 stream writer for the decoder's fixtures (low-delay and high-quality
pictures with chosen coefficients, any wavelet, depth, chroma format and field coding).

It writes quantised coefficients directly (a deterministic pattern: a smooth DC band and sparse
small highpass values) instead of transforming a picture, so every wavelet filter and syntax
feature can be exercised; FFmpeg's decode of the result is the oracle. Written from the Dirac
specification 2.2.3 and SMPTE ST 2042-1; no other implementation was consulted.

usage: python3 -I vc2_writer.py OUT.drc key=value ...
keys: syntax=ld|hq w= h= chroma=0|1|2 range=1|2|3|4 wavelet=0..6 depth= sx= sy= frames=
      fields=0|1 major= qm=default|custom bytes=NUM/DEN prefix= scaler= q= seed= interlaced=0|1
      probe=1 (low delay: print the bytes the largest slice needs, write nothing)
"""

import sys

import numpy as np


class Bits:
    def __init__(self):
        self.b = []

    def bit(self, v):
        self.b.append(1 if v else 0)

    def nbits(self, v, n):
        for i in range(n - 1, -1, -1):
            self.bit((v >> i) & 1)

    def uint(self, v):
        v += 1
        k = v.bit_length() - 1
        for i in range(k - 1, -1, -1):
            self.bit(0)
            self.bit((v >> i) & 1)
        self.bit(1)

    def sint(self, v):
        self.uint(abs(v))
        if v != 0:
            self.bit(1 if v < 0 else 0)

    def align(self):
        while len(self.b) % 8:
            self.b.append(0)

    def bytes(self):
        self.align()
        out = bytearray()
        for i in range(0, len(self.b), 8):
            v = 0
            for x in self.b[i:i + 8]:
                v = (v << 1) | x
            out.append(v)
        return bytes(out)


def intlog2(n):
    return 0 if n <= 1 else (n - 1).bit_length()


DEFAULT_QM = {
    0: [5, 3, 3, 0, 4, 4, 1, 5, 5, 2, 6, 6, 3],
    1: [4, 2, 2, 0, 4, 4, 2, 5, 5, 3, 7, 7, 5],
    2: [5, 3, 3, 0, 4, 4, 1, 5, 5, 2, 6, 6, 3],
    4: [8, 4, 4, 0, 4, 4, 0, 4, 4, 0, 4, 4, 0],
    5: [0, 4, 4, 8, 8, 8, 12, 13, 13, 17, 17, 17, 21],
    6: [3, 1, 1, 0, 4, 4, 2, 6, 6, 5, 9, 9, 7],
}

RANGES = {1: (0, 255, 128, 255), 2: (16, 219, 128, 224), 3: (64, 876, 512, 896),
          4: (256, 3504, 2048, 3584)}


def parse_info(code, body, prev):
    nxt = 13 + len(body) if code != 0x10 else 0
    return b"BBCD" + bytes([code]) + nxt.to_bytes(4, "big") + prev.to_bytes(4, "big") + body


def seq_header(o):
    b = Bits()
    b.uint(o["major"])
    b.uint(0)
    b.uint(0 if o["syntax"] == "ld" else 3)
    b.uint(0)
    b.uint(0)  # custom base format
    b.bit(1); b.uint(o["w"]); b.uint(o["h"])
    b.bit(1); b.uint(o["chroma"])
    b.bit(1); b.uint(o["interlaced"])
    b.bit(1); b.uint(0); b.uint(25); b.uint(1)
    b.bit(1); b.uint(1)
    b.bit(0)
    b.bit(1); b.uint(o["range"])
    b.bit(1); b.uint(3)
    b.uint(o["fields"])
    return b.bytes()


def band_sizes(w, h, depth):
    s = 1 << depth
    pw, ph = -(-w // s) * s, -(-h // s) * s
    out = []
    for level in range(depth + 1):
        k = depth if level == 0 else depth - level + 1
        out.append((pw >> k, ph >> k))
    return out


def coeffs(rng, sizes, depth, frame, comp):
    """Quantised coefficients per (level, orient)."""
    out = {}
    w0, h0 = sizes[0]
    yy, xx = np.mgrid[0:h0, 0:w0]
    ll = (20 * np.sin((xx + 3 * frame + comp) / 2.0) + 15 * np.cos(yy / 1.7) + 4 * comp).astype(int)
    out[(0, 0)] = ll
    for level in range(1, depth + 1):
        bw, bh = sizes[level]
        for orient in range(3):
            v = rng.integers(-6, 7, size=(bh, bw))
            v[rng.random((bh, bw)) < 0.55] = 0
            out[(level, orient)] = v
    return out


def rect(size, sx, sy, nx, ny):
    w, h = size
    return (w * sx // nx, h * sy // ny, w * (sx + 1) // nx, h * (sy + 1) // ny)


def order(depth):
    v = [(0, 0, 0)]
    for level in range(1, depth + 1):
        for orient in range(3):
            v.append((level, orient, 1 + 3 * (level - 1) + orient))
    return v


NEED = [0]


def picture(o, rng, number):
    w, h = o["w"], o["h"]
    if o["fields"]:
        h //= 2
    cw, ch = w, h
    if o["chroma"] >= 1:
        cw //= 2
    if o["chroma"] == 2:
        ch //= 2
    depth, nx, ny = o["depth"], o["sx"], o["sy"]
    sizes = [band_sizes(w, h, depth), band_sizes(cw, ch, depth), band_sizes(cw, ch, depth)]
    data = [coeffs(rng, sizes[c], depth, number, c) for c in range(3)]
    b = Bits()
    b.uint(o["wavelet"])
    b.uint(depth)
    if o["major"] >= 3:
        b.bit(0)
        b.bit(0)
    b.uint(nx)
    b.uint(ny)
    if o["syntax"] == "ld":
        num, den = o["bytes"]
        b.uint(num)
        b.uint(den)
    else:
        b.uint(o["prefix"])
        b.uint(o["scaler"])
    if o["qm"] == "custom":
        b.bit(1)
        for i in range(1 + 3 * depth):
            b.uint((7 * i + 3) % 11)
    else:
        b.bit(0)
    head = number.to_bytes(4, "big") + b.bytes()
    out = bytearray(head)
    for i in range(nx * ny):
        sx, sy = i % nx, i // nx
        q = (o["q"] + 3 * i) % 24
        parts = []
        for c in range(3):
            parts.append([])
            for (level, orient, _) in order(depth):
                l, t, r, bo = rect(sizes[c][level], sx, sy, nx, ny)
                parts[c].append(data[c][(level, orient)][t:bo, l:r].flatten())
        if o["syntax"] == "ld":
            num, den = o["bytes"]
            nbytes = (i + 1) * num // den - i * num // den
            bits = 8 * nbytes
            lb = intlog2(bits - 7)
            # Early termination (§13.5.2, A.3.1): trailing zero coefficients are left out of
            # each block, since a decoder reads 1s (zero codes) past a block's end. Codes are
            # never cut part-way: FFmpeg does not follow A.3.1 there, so such a stream would
            # have no oracle.
            ys = [int(v) for arr in parts[0] for v in arr]
            cs = [int(v) for a1, a2 in zip(parts[1], parts[2]) for p in zip(a1, a2) for v in p]
            while ys and ys[-1] == 0:
                ys.pop()
            while cs and cs[-1] == 0:
                cs.pop()
            yb = Bits()
            for v in ys:
                yb.sint(v)
            cb = Bits()
            for v in cs:
                cb.sint(v)
            room = bits - 7 - lb
            need = (len(yb.b) + len(cb.b) + 7 + lb + 7) // 8
            NEED[0] = max(NEED[0], need)
            if o["probe"]:
                continue
            assert len(yb.b) + len(cb.b) <= room, f"slice {i} needs {need} bytes; raise bytes"
            ylen = len(yb.b)
            s = Bits()
            s.nbits(q & 127, 7)
            s.nbits(ylen, lb)
            s.b += yb.b
            s.b += cb.b
            # The chroma block runs to the slice end: pad with zero codes.
            s.b += [1] * (bits - len(s.b))
            out += s.bytes()
        else:
            scaler = o["scaler"]
            out += bytes(o["prefix"])
            out.append(q)
            for c in range(3):
                cb = Bits()
                for arr in parts[c]:
                    for v in arr:
                        cb.sint(int(v))
                raw = cb.bytes()
                n = -(-len(raw) // scaler)
                assert n <= 255, "slice component too long; raise the scaler"
                out.append(n)
                out += raw + bytes(n * scaler - len(raw))
    return bytes(out)


def main():
    path = sys.argv[1]
    o = dict(syntax="hq", w=64, h=48, chroma=2, range=1, wavelet=0, depth=3, sx=4, sy=3,
             frames=2, fields=0, major=2, qm="default", bytes="60/1", prefix=0, scaler=1,
             q=4, seed=1, interlaced=0, probe=0)
    for kv in sys.argv[2:]:
        k, v = kv.split("=")
        o[k] = v
    for k in ("w", "h", "chroma", "range", "wavelet", "depth", "sx", "sy", "frames", "fields",
              "major", "prefix", "scaler", "q", "seed", "interlaced", "probe"):
        o[k] = int(o[k])
    num, den = o["bytes"].split("/")
    o["bytes"] = (int(num), int(den))
    rng = np.random.default_rng(o["seed"])
    code = 0xC8 if o["syntax"] == "ld" else 0xE8
    units = [(0x00, seq_header(o))]
    pictures = o["frames"] * (2 if o["fields"] else 1)
    for n in range(pictures):
        units.append((code, picture(o, rng, n)))
    units.append((0x10, b""))
    out = bytearray()
    prev = 0
    for code, body in units:
        unit = parse_info(code, body, prev)
        prev = len(unit)
        out += unit
    if o["probe"]:
        print("largest slice needs", NEED[0], "bytes")
        return
    open(path, "wb").write(out)


main()
