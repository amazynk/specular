#!/usr/bin/env python3
"""Regenerates the chroma-siting fixtures (siting-*.avif) in this directory.

Each fixture is a lossless libaom AVIF of synthetic Y'CbCr planes (BT.601, limited range):
flat luma, and chroma that alternates between red, blue, green and grey from one chroma sample
to the next (a 1-sample checker in the top half, a fixed pseudo-random pattern below), so that
every chroma upsampling filter and sample position gives a visibly different picture. The raw
planes are piped to ffmpeg (libaom-av1, lossless=1); neither ffmpeg nor libaom write
`chroma_sample_position`, so this script then sets the 2-bit field in place in both the AV1
sequence header OBUs and the `av1C` box.

Usage: make_siting.py [out_dir]   (default: this directory). Needs ffmpeg with libaom-av1.
Made with ffmpeg 8.0.1 / libaom 3.13.1.
"""

import pathlib
import subprocess
import sys

# (name, width, height, 4:2:0?, bits, chroma_sample_position: 0 unknown, 1 vertical, 2 colocated)
FIXTURES = [
    ("siting420-unknown", 32, 24, True, 8, 0),
    ("siting420-vertical", 32, 24, True, 8, 1),
    ("siting420-colocated", 32, 24, True, 8, 2),
    ("siting420-odd-unknown", 33, 25, True, 8, 0),
    ("siting420-odd-colocated", 33, 25, True, 8, 2),
    ("siting420-10bit-colocated", 32, 24, True, 10, 2),
    ("siting422", 32, 24, False, 8, 0),
    ("siting422-odd", 33, 25, False, 8, 0),
]

# (Cb, Cr) at 8 bits: red, blue, green, grey. Chosen so that RGB does not clip with Y = 126.
COLOURS = [(68, 188), (188, 68), (68, 68), (128, 128)]
LUMA = 126


def planes(w, h, is420, bits):
    cw, ch = (w + 1) // 2, ((h + 1) // 2 if is420 else h)
    k = 1 << (bits - 8)
    state = 12345
    u, v = [], []
    for cy in range(ch):
        for cx in range(cw):
            if cy < ch // 2:
                c = COLOURS[(cx + cy) & 1]
            else:
                state = (state * 1103515245 + 12345) & 0x7FFFFFFF
                c = COLOURS[(state >> 16) & 3]
            u.append(c[0] * k)
            v.append(c[1] * k)
    y = [LUMA * k] * (w * h)
    size = 1 if bits == 8 else 2
    pack = lambda s: b"".join(x.to_bytes(size, "little") for x in s)
    return pack(y) + pack(u) + pack(v)


class Bits:
    def __init__(self, data, pos):
        self.data, self.bit = data, pos * 8

    def f(self, n):
        v = 0
        for _ in range(n):
            v = (v << 1) | ((self.data[self.bit >> 3] >> (7 - (self.bit & 7))) & 1)
            self.bit += 1
        return v

    def uvlc(self):
        zeros = 0
        while self.f(1) == 0:
            zeros += 1
        return self.f(zeros) + (1 << zeros) - 1 if zeros < 32 else (1 << 32) - 1


def csp_bit(data, payload):
    """Bit offset of chroma_sample_position in the sequence header OBU payload at `payload`,
    or None when the header has no such field (AV1 spec 5.5)."""
    b = Bits(data, payload)
    profile = b.f(3)
    b.f(1)  # still_picture
    reduced = b.f(1)
    decoder_model = False
    if reduced:
        b.f(5)
    else:
        if b.f(1):  # timing_info_present_flag
            b.f(32), b.f(32)
            if b.f(1):
                b.uvlc()
            decoder_model = b.f(1)
            if decoder_model:
                n = b.f(5) + 1
                b.f(32), b.f(5), b.f(5)
        initial_delay = b.f(1)
        for _ in range(b.f(5) + 1):
            b.f(12)
            if b.f(5) > 7:
                b.f(1)
            if decoder_model and b.f(1):
                b.f(n), b.f(n), b.f(1)
            if initial_delay and b.f(1):
                b.f(4)
    wb, hb = b.f(4) + 1, b.f(4) + 1
    b.f(wb), b.f(hb)
    if not reduced and b.f(1):  # frame_id_numbers_present_flag
        b.f(4), b.f(3)
    b.f(3)  # 128x128 superblock, filter intra, intra edge filter
    if not reduced:
        b.f(4)  # interintra, masked compound, warped motion, dual filter
        order_hint = b.f(1)
        if order_hint:
            b.f(2)
        force_sct = 2 if b.f(1) else b.f(1)
        if force_sct > 0 and not b.f(1):
            b.f(1)
        if order_hint:
            b.f(3)
    b.f(3)  # superres, cdef, restoration
    high = b.f(1)
    bits = 8
    if profile == 2 and high:
        bits = 12 if b.f(1) else 10
    elif high:
        bits = 10
    mono = 0 if profile == 1 else b.f(1)
    cp, tc, mc = 2, 2, 2
    if b.f(1):
        cp, tc, mc = b.f(8), b.f(8), b.f(8)
    if mono or (cp == 1 and tc == 13 and mc == 0):
        return None
    b.f(1)  # color_range
    if profile == 0:
        ssx, ssy = 1, 1
    elif profile == 1:
        ssx, ssy = 0, 0
    elif bits == 12:
        ssx = b.f(1)
        ssy = b.f(1) if ssx else 0
    else:
        ssx, ssy = 1, 0
    return b.bit if ssx and ssy else None


def leb128(data, pos):
    v, i = 0, 0
    while True:
        byte = data[pos + i]
        v |= (byte & 0x7F) << (7 * i)
        i += 1
        if not byte & 0x80:
            return v, i


def set_csp(data, csp):
    data = bytearray(data)
    patched = 0
    # The item data is the only thing in `mdat` (one still image): walk its OBUs.
    mdat = data.find(b"mdat")
    end = mdat - 4 + int.from_bytes(data[mdat - 4 : mdat], "big")
    pos = mdat + 4
    while pos < end:
        header = data[pos]
        assert header & 0x02, "OBU without size field"
        ext = (header >> 2) & 1
        size, n = leb128(data, pos + 1 + ext)
        payload = pos + 1 + ext + n
        if (header >> 3) & 0xF == 1:  # OBU_SEQUENCE_HEADER
            bit = csp_bit(data, payload)
            assert bit is not None and bit + 2 <= (payload + size) * 8
            for i in range(2):
                byte, sh = (bit + i) >> 3, 7 - ((bit + i) & 7)
                data[byte] = (data[byte] & ~(1 << sh)) | (((csp >> (1 - i)) & 1) << sh)
            patched += 1
        pos = payload + size
    av1c = data.find(b"av1C")
    # libaom leaves configOBUs in av1C empty: one sequence header, in the item data.
    assert av1c > 0 and pos == end and patched == 1, f"found {patched} sequence headers"
    data[av1c + 6] = (data[av1c + 6] & 0xFC) | csp
    return bytes(data)


def main():
    out = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else pathlib.Path(__file__).parent)
    tmp = out / "siting.tmp.avif"  # the AVIF muxer needs a seekable output
    for name, w, h, is420, bits, csp in FIXTURES:
        pix = ("yuv420p" if is420 else "yuv422p") + ("10le" if bits == 10 else "")
        cmd = ["ffmpeg", "-v", "error", "-y", "-f", "rawvideo", "-pix_fmt", pix,
               "-s", f"{w}x{h}", "-i", "-", "-frames:v", "1",
               "-color_range", "tv", "-colorspace", "smpte170m",
               "-color_primaries", "smpte170m", "-color_trc", "smpte170m",
               "-c:v", "libaom-av1", "-aom-params", "lossless=1", "-still-picture", "1",
               "-f", "avif", str(tmp)]
        subprocess.run(cmd, input=planes(w, h, is420, bits), check=True)
        avif = tmp.read_bytes()
        tmp.unlink()
        if csp:
            avif = set_csp(avif, csp)
        (out / f"{name}.avif").write_bytes(avif)
        print(f"{name}.avif: {len(avif)} bytes")


if __name__ == "__main__":
    main()
