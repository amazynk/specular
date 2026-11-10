"""A direct transcription of the Dirac specification 2.2.3 decoding steps that vc2_writer.py
streams exercise, as a second oracle where FFmpeg 8.0 departs from the specification (see
gen_fixtures.sh): inverse quantisation (§13.2), intra DC prediction of low-delay pictures
(§13.3, §13.5.1), the lifting synthesis (§15.6, Tables 15.1-15.7), pad removal (§15.7) and the
output offset and clipping. It runs the writer with the same arguments to get the quantised
coefficients and per-slice quantisers, so it never parses the stream: the decoder's syntax
handling is checked by the fixtures FFmpeg decodes, its arithmetic here.

usage: python3 -I vc2_model.py OUT.yuv key=value ...   (the writer's keys, frame coding only)
Writes planar frames, 8-bit or 16-bit little-endian as FFmpeg's rawvideo does.
"""

import os
import sys

import numpy as np

here = os.path.dirname(os.path.abspath(__file__))
src = open(os.path.join(here, "vc2_writer.py")).read().rsplit("\nmain()", 1)[0]
out_path = sys.argv[1]
keys = sys.argv[2:]
sys.argv = ["vc2_writer.py", os.devnull] + keys
g = {"__name__": "vc2_writer"}
exec(compile(src, "vc2_writer.py", "exec"), g)
seen = []
coeffs = g["coeffs"]


def spy(*a):
    r = coeffs(*a)
    seen.append(r)
    return r


g["coeffs"] = spy
g["open"] = lambda *a, **k: open(os.devnull, "wb")
g["main"]()

o = dict(syntax="hq", w=64, h=48, chroma=2, range=1, wavelet=0, depth=3, sx=4, sy=3, q=4,
         qm="default", fields=0)
o.update(kv.split("=") for kv in keys)
for k in ("w", "h", "chroma", "range", "wavelet", "depth", "sx", "sy", "q", "fields"):
    o[k] = int(o[k])
assert not o["fields"], "frame coding only"
w, h, depth, wav, nx, ny = o["w"], o["h"], o["depth"], o["wavelet"], o["sx"], o["sy"]
if o["qm"] == "custom":
    qm = [(7 * i + 3) % 11 for i in range(1 + 3 * depth)]
else:
    assert wav != 3 and depth <= 4, "default matrix not modelled"
    qm = g["DEFAULT_QM"][wav][:1 + 3 * depth]


def quant_factor(q):
    base = 2 ** (q // 4)
    return [4 * base, (503829 * base + 52958) // 105917, (665857 * base + 58854) // 117708,
            (440253 * base + 32722) // 65444][q % 4]


def quant_offset(q):
    return 1 if q == 0 else 2 if q == 1 else (quant_factor(q) + 1) // 2


def inverse_quant(v, q):
    m = abs(int(v))
    if m:
        m = (m * quant_factor(q) + quant_offset(q) + 2) // 4
    return -m if v < 0 else m


# (type, S, D, taps) per lifting stage and the filter shift, Tables 15.1-15.7.
FILTERS = {
    0: ([(2, 2, 0, [1, 1]), (3, 4, -1, [-1, 9, 9, -1])], 1),
    1: ([(2, 2, 0, [1, 1]), (3, 1, 0, [1, 1])], 1),
    2: ([(2, 5, -1, [-1, 9, 9, -1]), (3, 4, -1, [-1, 9, 9, -1])], 1),
    3: ([(2, 1, 1, [1]), (3, 0, 0, [1])], 0),
    4: ([(2, 1, 1, [1]), (3, 0, 0, [1])], 1),
    5: ([(3, 8, -3, [-2, 10, -25, 81, 81, -25, 10, -2]),
         (2, 8, -3, [-8, 21, -46, 161, 161, -46, 21, -8])], 0),
    6: ([(2, 12, 0, [1817, 1817]), (4, 12, 0, [3616, 3616]), (1, 12, 0, [217, 217]),
         (3, 12, 0, [6497, 6497])], 1),
}


def lift(a, kind, s, d, taps):
    n2 = len(a)
    for n in range(n2 // 2):
        total = 0
        for i in range(d, len(taps) + d):
            if kind in (1, 2):
                pos = min(max(2 * (n + i) - 1, 1), n2 - 1)
            else:
                pos = min(max(2 * (n + i), 0), n2 - 2)
            total += taps[i - d] * a[pos]
        if s > 0:
            total += 1 << (s - 1)
        v = total >> s
        if kind == 1:
            a[2 * n] += v
        elif kind == 2:
            a[2 * n] -= v
        elif kind == 3:
            a[2 * n + 1] += v
        else:
            a[2 * n + 1] -= v


def synth_1d(a):
    for stage in FILTERS[wav][0]:
        lift(a, *stage)


def vh_synth(ll, hl, lh, hh):
    bh, bw = ll.shape
    s = np.zeros((2 * bh, 2 * bw), dtype=object)
    s[0::2, 0::2], s[0::2, 1::2], s[1::2, 0::2], s[1::2, 1::2] = ll, hl, lh, hh
    for x in range(2 * bw):
        col = list(s[:, x])
        synth_1d(col)
        s[:, x] = col
    for y in range(2 * bh):
        row = list(s[y])
        synth_1d(row)
        s[y] = row
    shift = FILTERS[wav][1]
    if shift:
        s = (s + (1 << (shift - 1))) // (1 << shift)
    return s


def dc_prediction(band):
    bh, bw = band.shape
    for v in range(bh):
        for x in range(bw):
            if x > 0 and v > 0:
                p = (band[v, x - 1] + band[v - 1, x - 1] + band[v - 1, x] + 1) // 3
            elif x > 0:
                p = band[0, x - 1]
            elif v > 0:
                p = band[v - 1, 0]
            else:
                p = 0
            band[v, x] += p


def slice_quants(size, slot):
    """The quantiser of every coefficient of a band: slice i uses (q + 3 i) % 24."""
    bw, bh = size
    out = np.zeros((bh, bw), dtype=int)
    for i in range(nx * ny):
        sx, sy = i % nx, i // nx
        qindex = (o["q"] + 3 * i) % 24
        if o["syntax"] == "ld":
            qindex &= 127
        l, t, r, b = g["rect"](size, sx, sy, nx, ny)
        out[t:b, l:r] = max(qindex - qm[slot], 0)
    return out


luma_off, luma_exc, chroma_off, chroma_exc = g["RANGES"][o["range"]]
bits = [g["intlog2"](luma_exc + 1), g["intlog2"](chroma_exc + 1)]
cw, ch = w, h
if o["chroma"] >= 1:
    cw //= 2
if o["chroma"] == 2:
    ch //= 2
dims = [(w, h), (cw, ch), (cw, ch)]
out = bytearray()
for p in range(len(seen) // 3):
    for c in range(3):
        d = seen[3 * p + c]
        cdw, cdh = dims[c]
        sizes = g["band_sizes"](cdw, cdh, depth)
        deq = {}
        for (level, orient, slot) in g["order"](depth):
            q = slice_quants(sizes[level], slot)
            v = d[(level, orient)]
            deq[(level, orient)] = np.array(
                [[inverse_quant(v[y, x], q[y, x]) for x in range(v.shape[1])]
                 for y in range(v.shape[0])], dtype=object).reshape(v.shape)
        if o["syntax"] == "ld":
            dc_prediction(deq[(0, 0)])
        ll = deq[(0, 0)]
        for level in range(1, depth + 1):
            ll = vh_synth(ll, deq[(level, 0)], deq[(level, 1)], deq[(level, 2)])
        depth_bits = bits[0 if c == 0 else 1]
        half = 1 << (depth_bits - 1)
        pic = np.clip(ll[:cdh, :cdw].astype(np.int64), -half, half - 1) + half
        if bits[0] > 8:
            out += pic.astype("<u2").tobytes()
        else:
            out += pic.astype(np.uint8).tobytes()
open(out_path, "wb").write(bytes(out))
