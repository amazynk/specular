"""Own writer for the HAP fixtures FFmpeg can't encode (sections per the HAP specification).

    python3 -I hapwrite.py hapm HAPQ.frames HAPQ.out W H   HAP Q Alpha from stored HAP Q frames
    python3 -I hapwrite.py hapalpha-only - - W H            HAP Alpha-Only (BC4)
    python3 -I hapwrite.py hapr - - W H                     HAP R (BC7 mode 6)

Writes NAME.frames (see pack.py) and NAME.out (expected RGBA). The BC4 and BC7 encoders
choose endpoints whose interpolated values are exact integers (BC4 spans in multiples of 7 or
5) or use the BC7 specification's integer interpolation, so the expected pixels come from this
script's own block decode with no rounding choice left open. HAP Q Alpha's colour is FFmpeg's
decode of the same HAP Q textures; its alpha this script's BC4 decode.
"""

import struct
import sys

import numpy as np

name, frames_path, rgb_path, W, H = sys.argv[1], sys.argv[2], sys.argv[3], int(sys.argv[4]), int(sys.argv[5])
PW, PH = (W + 3) // 4 * 4, (H + 3) // 4 * 4


def sect(t, body, ext=False):
    n = len(body)
    if ext or n >= 1 << 24:
        return bytes([0, 0, 0, t]) + struct.pack("<I", n) + body
    return struct.pack("<I", n)[:3] + bytes([t]) + body


def read_frames(path):
    d = open(path, "rb").read()
    i = 12
    out = []
    while i < len(d):
        (n,) = struct.unpack(">I", d[i : i + 4])
        out.append(d[i + 4 : i + 4 + n])
        i += 4 + n
    return out


def body_of(frame):
    if frame[:3] == b"\0\0\0":
        (n,) = struct.unpack("<I", frame[4:8])
        return frame[3], frame[8 : 8 + n]
    n = frame[0] | frame[1] << 8 | frame[2] << 16
    return frame[3], frame[4 : 4 + n]


def bc4_block(v, k):
    """16 values -> (8 bytes, 16 decoded values); 8-value mode for even k, else 6-value."""
    lo, hi = int(v.min()), int(v.max())
    if k % 2 == 0:
        span = min(252, max(7, -(-(hi - lo) // 7) * 7))
        a1 = lo
        a0 = lo + span
        if a0 > 255:
            a0, a1 = 255, 255 - span
        pal = [a0, a1] + [((8 - i) * a0 + (i - 1) * a1) // 7 for i in range(2, 8)]
        assert all(((8 - i) * a0 + (i - 1) * a1) % 7 == 0 for i in range(2, 8))
    else:
        span = -(-(hi - lo) // 5) * 5
        a0 = lo
        a1 = lo + span
        if a1 > 255:
            a1, a0 = 255, 255 - span
        pal = [a0, a1] + [((6 - i) * a0 + (i - 1) * a1) // 5 for i in range(2, 6)] + [0, 255]
    pal = np.array(pal)
    idx = [int(np.argmin(np.abs(pal - int(x)))) for x in v]
    bits = 0
    for j, ix in enumerate(idx):
        bits |= ix << (3 * j)
    return bytes([a0, a1]) + bits.to_bytes(6, "little"), pal[idx]


def bc4(plane):
    """Plane PH x PW -> (blocks, decoded plane)."""
    out = bytearray()
    dec = np.zeros_like(plane)
    k = 0
    for by in range(0, PH, 4):
        for bx in range(0, PW, 4):
            blk = plane[by : by + 4, bx : bx + 4].reshape(16)
            b, d = bc4_block(blk, k)
            out += b
            dec[by : by + 4, bx : bx + 4] = d.reshape(4, 4)
            k += 1
    return bytes(out), dec


W4 = [0, 4, 9, 13, 17, 21, 26, 30, 34, 38, 43, 47, 51, 55, 60, 64]


def bc7_block(px):
    """16x4 RGBA -> (16 bytes, decoded 16x4), mode 6."""
    lo = px.min(axis=0).astype(int)
    hi = px.max(axis=0).astype(int)
    e = [(lo & ~1, 0), (hi | 1, 1)]

    def palette(e0, e1):
        return np.array([((64 - w) * e0 + w * e1 + 32) >> 6 for w in W4])

    def best(e0, e1):
        pal = palette(e0, e1)
        return [int(np.argmin(((pal - p.astype(int)) ** 2).sum(axis=1))) for p in px], pal

    idx, pal = best(e[0][0], e[1][0])
    if idx[0] >= 8:
        e = [e[1], e[0]]
        idx, pal = best(e[0][0], e[1][0])
        assert idx[0] < 8
    bits = 1 << 6
    pos = 7
    for c in range(4):
        for j in range(2):
            bits |= int(e[j][0][c] >> 1) << pos
            pos += 7
    bits |= e[0][1] << pos
    bits |= e[1][1] << (pos + 1)
    pos += 2
    for j, ix in enumerate(idx):
        n = 3 if j == 0 else 4
        bits |= ix << pos
        pos += n
    assert pos == 128
    return bits.to_bytes(16, "little"), pal[idx]


def pattern(f):
    y, x = np.mgrid[0:PH, 0:PW]
    rng = np.random.default_rng(7 + f)
    return y, x, rng


frames = []
expected = bytearray()
if name == "hapm":
    q = read_frames(frames_path)
    rgb = np.frombuffer(open(rgb_path, "rb").read(), np.uint8).reshape(-1, H, W, 4)
    for f, fr in enumerate(q):
        t, tex = body_of(fr)
        assert t == 0xAF
        y, x, rng = pattern(f)
        alpha = ((x * 7 + y * 11 + f * 40) % 256).astype(np.int64)
        alpha[8:16, 8:24] = rng.integers(0, 256, (8, 16))
        blocks, dec = bc4(alpha)
        # Colour as stored chunks (decode instructions), alpha stored.
        half = len(tex) // 2
        instr = sect(0x02, bytes([0x0A, 0x0A])) + sect(0x03, struct.pack("<II", half, len(tex) - half))
        colour = sect(0xCF, sect(0x01, instr) + tex)
        frames.append(sect(0x0D, colour + sect(0xA1, blocks, ext=(f == 1))))
        e = rgb[f].copy()
        e[:, :, 3] = dec[:H, :W]
        expected += e.tobytes()
elif name == "hapalpha-only":
    for f in range(2):
        y, x, rng = pattern(f)
        a = ((x * 3 + y * 5 + f * 99) % 256).astype(np.int64)
        a[4:12, 0:8] = rng.integers(0, 256, (8, 8))
        blocks, dec = bc4(a)
        frames.append(sect(0xA1, blocks, ext=(f == 1)))
        g = dec[:H, :W].astype(np.uint8)
        expected += np.stack([g, g, g, np.full_like(g, 255)], axis=-1).tobytes()
elif name == "hapr":
    for f in range(2):
        y, x, rng = pattern(f)
        img = np.stack([(x * 3 + f * 17) % 256, (y * 5) % 256, (x + y) * 2 % 256, 255 - (x * 2) % 256], axis=-1)
        img[16:32, 16:40] = rng.integers(0, 256, (16, 24, 4))
        img = img.astype(np.int64)
        out = bytearray()
        dec = np.zeros_like(img)
        for by in range(0, PH, 4):
            for bx in range(0, PW, 4):
                b, d = bc7_block(img[by : by + 4, bx : bx + 4].reshape(16, 4))
                out += b
                dec[by : by + 4, bx : bx + 4] = d.reshape(4, 4, 4)
        frames.append(sect(0xAC, bytes(out)))
        expected += dec[:H, :W].astype(np.uint8).tobytes()
else:
    sys.exit("unknown fixture " + name)

data = struct.pack(">III", W, H, 0)
for fr in frames:
    data += struct.pack(">I", len(fr)) + fr
open(name + ".frames", "wb").write(data)
open(name + ".out", "wb").write(bytes(expected))
