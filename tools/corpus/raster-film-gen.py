"""Writes the DPX and Cineon variants no available tool writes (for fetch-raster-film.sh):
separate R/G/B(/A) elements, RGB plus an alpha element, luma plus colour-difference
elements, signed samples, user-defined 2 to 5 channel elements, composite video, run-length
encoded floats; Cineon with 4 channels, mixed depths and signed samples. The pictures are
smooth synthetic gradients. Own minimal writers (SMPTE 268M, Kodak Cineon 4.5).

Beside each file it writes what a reader should get (EXPECTDIR/<file>.truth.pam.gz, the
first page's code values scaled to 16 bits, two's complement bits for signed samples, each
channel scaled from its own depth; .truthf.pam.gz for floats, clipped to 0..1), the oracle
for variants no other reader reads correctly.

Usage: python -I raster-film-gen.py OUTDIR EXPECTDIR
"""
import gzip
import os
import struct
import sys

import numpy as np

W, H = 67, 41


def picture(c, bits, signed=False):
    """c channels of `bits`-bit codes, smooth and distinct per channel."""
    y, x = np.mgrid[0:H, 0:W].astype(np.float64)
    out = []
    for k in range(c):
        v = 0.5 + 0.45 * np.sin(x / (9 + 3 * k) + y / (13 - k) + k)
        out.append(v)
    a = np.stack(out, axis=-1)
    if signed:
        half = 1 << (bits - 1)
        return np.round((a * 2 - 1) * (half - 1)).astype(np.int64)
    return np.round(a * ((1 << bits) - 1)).astype(np.int64)


def pack(samples, bits, packing, be, low_first):
    """A row of codes (non-negative ints) as bytes, padded to 32 bits."""
    s = [int(v) for v in samples]
    e = ">" if be else "<"
    if bits == 8:
        b = bytes(v & 0xFF for v in s)
    elif bits == 16:
        b = struct.pack(e + "%dH" % len(s), *[v & 0xFFFF for v in s])
    elif bits == 10 and packing in (1, 2):
        words = []
        for i in range(0, len(s), 3):
            g = s[i:i + 3] + [0] * (3 - len(s[i:i + 3]))
            if low_first:
                g = g[::-1]
            w = (g[0] << 20) | (g[1] << 10) | g[2]
            if packing == 1:
                w <<= 2
            words.append(w)
        b = struct.pack(e + "%dI" % len(words), *words)
    elif bits == 12 and packing in (1, 2):
        b = struct.pack(e + "%dH" % len(s), *[(v << 4) if packing == 1 else v for v in s])
    else:
        raise ValueError((bits, packing))
    # 8- and 16-bit rows unpadded, as OpenImageIO and FFmpeg write them (the reader takes
    # either); the others padded to 32 bits.
    return b if bits in (8, 16) else b + b"\0" * (-len(b) % 4)


def dpx(path, elements, be=True, width=W, height=H):
    """elements: dicts with descriptor, bits, packing, data (bytes), and optional sign,
    transfer, encoding, low, high."""
    e = ">" if be else "<"
    hdr = bytearray(b"\xff" * 2048)
    off = 2048
    offsets = []
    for el in elements:
        offsets.append(off)
        off += len(el["data"])
    hdr[0:4] = b"SDPX" if be else b"XPDS"
    struct.pack_into(e + "I", hdr, 4, 2048)
    hdr[8:16] = b"V2.0\0\0\0\0"
    struct.pack_into(e + "I", hdr, 16, off)
    struct.pack_into(e + "IIII", hdr, 20, 1, 1664, 384, 0)
    hdr[36:136] = b"\0" * 100
    hdr[136:160] = b"2026:10:24:12:00:00\0\0\0\0\0"
    hdr[160:260] = b"raster-film-gen.py".ljust(100, b"\0")
    hdr[260:660] = b"\0" * 400
    struct.pack_into(e + "HHII", hdr, 768, 0, len(elements), width, height)
    for i, el in enumerate(elements):
        o = 780 + i * 72
        struct.pack_into(e + "I", hdr, o, el.get("sign", 0))
        struct.pack_into(e + "I", hdr, o + 4, el.get("low", 0))
        struct.pack_into(e + "f", hdr, o + 8, 0.0)
        struct.pack_into(e + "I", hdr, o + 12, el.get("high", (1 << min(el["bits"], 16)) - 1))
        struct.pack_into(e + "f", hdr, o + 16, 2.048)
        hdr[o + 20] = el["descriptor"]
        hdr[o + 21] = el.get("transfer", 0)
        hdr[o + 22] = el.get("transfer", 0)
        hdr[o + 23] = el["bits"]
        struct.pack_into(e + "HH", hdr, o + 24, el["packing"], el.get("encoding", 0))
        struct.pack_into(e + "III", hdr, o + 28, offsets[i], 0, 0)
        hdr[o + 40:o + 72] = b"\0" * 32
    with open(path, "wb") as f:
        f.write(bytes(hdr))
        for el in elements:
            f.write(el["data"])


def element(codes, descriptor, bits, packing=1, be=True, **kw):
    """codes: H x n array (n samples per row)."""
    one = codes.ndim == 2 and codes.shape[1] == W and descriptor in (1, 2, 3, 4, 6, 8, 9)
    rows = [pack(r, bits, packing, be, one and bits == 10) for r in codes.reshape(H, -1)]
    return dict(descriptor=descriptor, bits=bits, packing=packing, data=b"".join(rows), **kw)


def twos(a, bits):
    return np.where(a < 0, a + (1 << bits), a)


def rle_floats(rows, comps):
    """Run-length encodes float pixels (flags as floats): repeats of equal pixels, literal
    runs otherwise."""
    out = []
    flat = rows.reshape(-1, comps)
    i = 0
    while i < len(flat):
        j = i
        while j + 1 < len(flat) and np.array_equal(flat[j + 1], flat[i]) and j - i < 1000:
            j += 1
        if j > i:
            out.append(float(((j - i + 1) << 1) | 1))
            out.extend(flat[i].tolist())
            i = j + 1
        else:
            k = i
            while k + 1 < len(flat) and not np.array_equal(flat[k + 1], flat[k]) and k - i < 1000:
                k += 1
            n = k - i + 1
            out.append(float(n << 1))
            out.extend(flat[i:i + n].reshape(-1).tolist())
            i += n
    return struct.pack(">%df" % len(out), *out)


def cineon(path, chans, interleave, packing, signed=0, be=True):
    """chans: list of (designator, bits, H x W codes)."""
    e = ">" if be else "<"
    hdr = bytearray(1024)
    hdr[0:4] = bytes([0x80, 0x2A, 0x5F, 0xD7]) if be else bytes([0xD7, 0x5F, 0x2A, 0x80])
    struct.pack_into(e + "IIII", hdr, 4, 1024, 1024, 0, 0)
    hdr[24:28] = b"V4.5"
    hdr[132:144] = b"2026:10:24\0\0"
    hdr[192] = 0
    hdr[193] = len(chans)
    for i, (des, bits, _) in enumerate(chans):
        o = 196 + 28 * i
        hdr[o] = 0
        hdr[o + 1] = des
        hdr[o + 2] = bits
        struct.pack_into(e + "II", hdr, o + 4, W, H)
        struct.pack_into(e + "ffff", hdr, o + 12, 0.0, 0.0, float((1 << bits) - 1), 2.048)
    hdr[680] = interleave
    hdr[681] = packing
    hdr[682] = signed
    struct.pack_into(e + "II", hdr, 684, 0, 0)

    def bitstream(vals):
        acc, n, out = 0, 0, bytearray()
        for v, b in vals:
            acc = (acc << b) | (v & ((1 << b) - 1))
            n += b
            while n >= 8:
                n -= 8
                out.append((acc >> n) & 0xFF)
        if n:
            out.append((acc << (8 - n)) & 0xFF)
        out += b"\0" * (-len(out) % 4)
        return bytes(out)

    def units(vals):
        # Tight (0), or 16-bit units left justified (3), as many samples as fit in each
        # (vals of one depth).
        if packing == 0:
            return bitstream(vals)
        if packing == 3:
            bb = vals[0][1]
            per = 16 // bb
            words = []
            for i in range(0, len(vals), per):
                w = 0
                for k, (v, _) in enumerate(vals[i:i + per]):
                    w |= (v & ((1 << bb) - 1)) << (16 - bb * (k + 1))
                words.append(w)
            b = struct.pack(e + "%dH" % len(words), *words)
            return b + b"\0" * (-len(b) % 4)
        raise ValueError(packing)

    data = bytearray()
    if interleave == 0:
        for y in range(H):
            data += units([(int(c[2][y, x]), c[1]) for x in range(W) for c in chans])
    elif interleave == 1:
        for y in range(H):
            for c in chans:
                data += units([(int(v), c[1]) for v in c[2][y]])
    else:
        for c in chans:
            for y in range(H):
                data += units([(int(v), c[1]) for v in c[2][y]])
    with open(path, "wb") as f:
        f.write(bytes(hdr))
        f.write(bytes(data))


EXPECT = None


def pam(name, a):
    """a: H x W x C uint16."""
    h, w, c = a.shape
    path = os.path.join(EXPECT, os.path.basename(name) + ".%s.pam.gz")
    return path, (
        f"P7\nWIDTH {w}\nHEIGHT {h}\nDEPTH {c}\nMAXVAL 65535\nTUPLTYPE RGB\nENDHDR\n".encode()
        + a.astype(">u2").tobytes()
    )


def truth(name, codes, bits):
    """codes: H x W x C non-negative ints; bits: one depth or one per channel."""
    bits = np.broadcast_to(np.asarray(bits), (codes.shape[-1],))
    mx = (1 << bits.astype(np.int64)) - 1
    path, data = pam(name, np.round(codes * 65535.0 / mx).astype(np.uint16))
    with gzip.open(path % "truth", "wb") as f:
        f.write(data)


def truthf(name, px):
    path, data = pam(name, np.round(np.clip(px, 0, 1) * 65535).astype(np.uint16))
    with gzip.open(path % "truthf", "wb") as f:
        f.write(data)


def main():
    global EXPECT
    out = sys.argv[1]
    EXPECT = sys.argv[2]
    os.makedirs(out, exist_ok=True)
    os.makedirs(EXPECT, exist_ok=True)
    p = lambda n: os.path.join(out, "gen-py-" + n)

    # Separate R, G, B (and A) elements.
    for bits, pk, be, alpha in [(10, 1, True, False), (16, 0, False, True), (8, 0, True, False),
                                (12, 1, True, True), (10, 2, False, True)]:
        c = picture(4 if alpha else 3, bits)
        els = [element(c[..., k], d, bits, pk, be) for k, d in enumerate([1, 2, 3, 4][:c.shape[-1]])]
        tag = "rgba" if alpha else "rgb"
        n = p(f"sep-{tag}-{bits}-p{pk}-{'be' if be else 'le'}.dpx")
        dpx(n, els, be)
        truth(n, c, bits)
    # Element order B, G, R, and a depth element left as its own page.
    c = picture(4, 10)
    els = [element(c[..., 2], 3, 10), element(c[..., 1], 2, 10), element(c[..., 0], 1, 10),
           element(c[..., 3], 8, 10)]
    dpx(p("sep-bgr-depth-10.dpx"), els)
    truth(p("sep-bgr-depth-10.dpx"), c[..., :3], 10)
    # RGB plus an alpha element.
    c = picture(4, 10)
    dpx(p("rgb-plus-alpha-10.dpx"), [element(c[..., :3].reshape(H, -1), 50, 10),
                                      element(c[..., 3], 4, 10)])
    truth(p("rgb-plus-alpha-10.dpx"), c, 10)
    # Luma plus colour difference (Cb Cr per pixel pair), video range.
    y = 64 + np.round(picture(1, 10)[..., 0] * 876 / 1023).astype(np.int64)
    cc = picture(2, 10)[:, ::2, :]
    cc = 64 + np.round(cc * 896 / 1023).astype(np.int64)
    dpx(p("luma-cbcr-10.dpx"), [element(y, 6, 10, transfer=6, low=64, high=940),
                                element(cc.reshape(H, -1), 7, 10, transfer=6, low=64, high=940)])
    # Signed samples.
    for bits, d, nc, tr in [(16, 50, 3, 2), (8, 6, 1, 2), (16, 51, 4, 0), (10, 6, 1, 2)]:
        c = twos(picture(nc, bits, signed=True), bits)
        el = element(c.reshape(H, -1), d, bits, 1, True, sign=1, transfer=tr)
        dpx(p(f"signed-{bits}-d{d}.dpx"), [el])
        truth(p(f"signed-{bits}-d{d}.dpx"), c, bits)
    # User-defined 2 to 5 channel elements.
    for d in (150, 151, 152, 153):
        nc = d - 148
        c = picture(nc, 16)
        dpx(p(f"user-{nc}ch-16.dpx"), [element(c.reshape(H, -1), d, 16, 0)])
        truth(p(f"user-{nc}ch-16.dpx"), c, 16)
    # Composite video.
    c = picture(1, 10)
    dpx(p("composite-10.dpx"), [element(c[..., 0], 9, 10, transfer=9)])
    truth(p("composite-10.dpx"), c, 10)
    # Run-length encoded floats (flags as floats): flat bands repeat.
    yy, xx = np.mgrid[0:H, 0:W]
    f = np.stack([(xx // 8) / 8.0, (yy // 5) / 8.0, ((xx + yy) % 3) / 2.0], axis=-1).astype(np.float32)
    data = rle_floats(f.reshape(H, -1), 3)
    data += b"\0" * (-len(data) % 4)
    dpx(p("rle-float32.dpx"), [dict(descriptor=50, bits=32, packing=0, encoding=1, data=data,
                                    transfer=2)])
    truthf(p("rle-float32.dpx"), f)

    # Cineon: 4 channels; mixed depths; signed.
    c = picture(4, 10)
    cineon(p("cin-4ch-10.cin"), [(1, 10, c[..., 0]), (2, 10, c[..., 1]), (3, 10, c[..., 2]),
                                 (0, 10, c[..., 3])], 0, 0)
    truth(p("cin-4ch-10.cin"), c, 10)
    a, b, g = picture(1, 10)[..., 0], picture(2, 12)[..., 1], picture(3, 8)[..., 2]
    mixed = np.stack([a, b, g], axis=-1)
    for il in (0, 1, 2):
        cineon(p(f"cin-mixed-il{il}.cin"), [(1, 10, a), (2, 12, b), (3, 8, g)], il, 0)
        truth(p(f"cin-mixed-il{il}.cin"), mixed, [10, 12, 8])
    cineon(p("cin-mixed-il1-p3.cin"), [(1, 10, a), (2, 12, b), (3, 8, g)], 1, 3)
    truth(p("cin-mixed-il1-p3.cin"), mixed, [10, 12, 8])
    s = twos(picture(3, 16, signed=True), 16)
    cineon(p("cin-signed-16.cin"), [(1, 16, s[..., 0]), (2, 16, s[..., 1]), (3, 16, s[..., 2])],
           0, 3, signed=1)
    truth(p("cin-signed-16.cin"), s, 16)


main()
