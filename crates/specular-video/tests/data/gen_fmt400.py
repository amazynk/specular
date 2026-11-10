#!/usr/bin/env python3
"""Writes the fmt400 fixtures that no installed tool writes (run by gen_fixtures_fmt400.sh).

Usage: python3 -I gen_fmt400.py OUTDIR SRC.wav

- fmt400-shn.shn: a Shorten stream (version 2: DIFF0-3, QLPC, ZERO blocks, running means,
  the WAVE header kept verbatim) of SRC.wav, coded here from the format's description.
- fmt400-fib.8svx: mono Fibonacci-delta 8SVX of SRC.wav's left channel.
- fmt400-bw64.bw64: SRC.wav rewritten as BW64 (ds64, data size 0xFFFFFFFF).
- fmt400-dsd.dsf / fmt400-dsd.dff: the same 0.1 s of DSD64 stereo (a second-order
  sigma-delta modulation of two tones) in both containers.
- fmt400.mod, fmt400.s3m, fmt400.xm, fmt400.it: small modules (a looped sine and a decaying
  noise sample; notes, arpeggio, slides, vibrato, an XM volume envelope, an IT 2.14-packed
  sample). ffmpeg's libopenmpt renders them as the oracle.
- fmt400.cdg: CD+G packets (palette, presets, tiles, XOR tiles, a scroll).
Output is deterministic.
"""

import math
import os
import struct
import sys
import wave

def refs(out):
    """Shrinks the oracles: libopenmpt's renders to loudness and zero-crossing envelopes
    (20 ms windows), ffmpeg's CD+G frames to three, run-length coded."""
    for m in ("mod", "s3m", "xm", "it"):
        p = os.path.join(out, "fmt400-%s.pcm" % m)
        a = open(p, "rb").read()
        v = struct.unpack("<%dh" % (len(a) // 2), a)
        mono = [(v[2 * i] + v[2 * i + 1]) / 2 for i in range(len(v) // 2)]
        lines = ["%d" % len(mono)]
        for k in range(0, len(mono) - 959, 960):
            w = mono[k:k + 960]
            rms = math.sqrt(sum(x * x for x in w) / len(w))
            zc = sum(1 for i in range(1, len(w)) if (w[i - 1] < 0) != (w[i] < 0))
            lines.append("%.1f %d" % (rms, zc))
        open(os.path.join(out, "fmt400-%s.env" % m), "w").write("\n".join(lines) + "\n")
        os.remove(p)
    pk = open(os.path.join(out, "fmt400.cdg"), "rb").read()
    rgb = open(os.path.join(out, "fmt400-cdg.rgb"), "rb").read()
    size = 300 * 216 * 3
    frames = len(rgb) // size
    ours = len(pk) // 24 // 12
    blob = b""
    for j in (2, 6, ours - 1):
        drawn = sum(1 for i in range(min(12 * j + 12, len(pk) // 24)) if pk[24 * i] == 9)
        f = rgb[(drawn - 1) * size:drawn * size]
        rle = bytearray()
        i = 0
        while i < len(f):
            px = f[i:i + 3]
            n = 1
            while n < 65535 and i + 3 * n < len(f) and f[i + 3 * n:i + 3 * n + 3] == px:
                n += 1
            rle += struct.pack("<H", n) + px
            i += 3 * n
        blob += struct.pack("<II", j, len(rle)) + bytes(rle)
    assert frames >= 1
    open(os.path.join(out, "fmt400-cdg.ref"), "wb").write(blob)
    os.remove(os.path.join(out, "fmt400-cdg.rgb"))


if sys.argv[1] == "--refs":
    refs(sys.argv[2])
    sys.exit(0)

OUT = sys.argv[1]
SRC = sys.argv[2]


def put(name, data):
    with open(os.path.join(OUT, name), "wb") as f:
        f.write(data)


with wave.open(SRC, "rb") as w:
    CH = w.getnchannels()
    RATE = w.getframerate()
    FRAMES = w.getnframes()
    raw = w.readframes(FRAMES)
pcm = struct.unpack("<%dh" % (len(raw) // 2), raw)
chans = [list(pcm[c::CH]) for c in range(CH)]
wav_bytes = open(SRC, "rb").read()


# ---------------------------------------------------------------- Shorten
class BitWriter:
    def __init__(self):
        self.bits = []

    def put(self, v, n):
        for i in range(n - 1, -1, -1):
            self.bits.append((v >> i) & 1)

    def uvar(self, v, n):
        hi = v >> n
        self.bits.extend([0] * hi)
        self.bits.append(1)
        self.put(v & ((1 << n) - 1), n)

    def svar(self, v, n):
        u = (v << 1) if v >= 0 else (((~v) << 1) | 1)
        self.uvar(u, n + 1)

    def ulong(self, v):
        nb = max(v.bit_length(), 0)
        self.uvar(nb, 2)
        self.uvar(v, nb)

    def bytes(self):
        b = self.bits + [0] * (-len(self.bits) % 8)
        return bytes(int("".join(map(str, b[i:i + 8])), 2) for i in range(0, len(b), 8))


QLPC_ROUND = int(os.environ.get("QLPC_ROUND", "32"))
QLPC_OFFSET = int(os.environ.get("QLPC_OFFSET", "1"))


def tdiv(a, b):
    q = abs(a) // abs(b)
    return q if (a >= 0) == (b > 0) else -q


def shorten():
    block, nmean, wrap, maxnlpc = 256, 4, 3, 2
    w = BitWriter()
    for v in (5, CH, block, maxnlpc, nmean, 0):
        w.ulong(v)
    head = wav_bytes[:44]
    w.uvar(9, 2)
    w.uvar(len(head), 5)
    for b in head:
        w.uvar(b, 8)
    hist = [[0] * wrap for _ in range(CH)]
    means = [[0] * nmean for _ in range(CH)]
    nblocks = (FRAMES + block - 1) // block
    for k in range(nblocks):
        n = min(block, FRAMES - k * block)
        if n != block:
            w.uvar(5, 2)
            w.ulong(n)
            block = n
        for c in range(CH):
            x = chans[c][k * 256:k * 256 + n]
            offset = tdiv(nmean // 2 + sum(means[c]), nmean)
            cmd = [0, 1, 2, 3, 7, 1][k % 6] if k != 2 else 8
            if cmd == 8:
                x = [0] * n
                w.uvar(8, 2)
            elif cmd == 7:
                coefs = [40, -9]
                w.uvar(7, 2)
                resn = 6
                w.uvar(resn, 3)
                w.uvar(len(coefs), 2)
                for q in coefs:
                    w.svar(q, 5)
                off = offset * QLPC_OFFSET
                y = [v - off for v in hist[c][-2:]] + [v - off for v in x]
                for i in range(n):
                    s = QLPC_ROUND + coefs[0] * y[i + 1] + coefs[1] * y[i]
                    w.svar(y[i + 2] - (s >> 5), resn)
            else:
                full = hist[c] + x
                res = []
                for i in range(n):
                    j = i + wrap
                    a, b, cc = full[j - 1], full[j - 2], full[j - 3]
                    pred = [offset, a, 2 * a - b, 3 * (a - b) + cc][cmd]
                    res.append(full[j] - pred)
                resn = max(1, max(abs(r) for r in res).bit_length() - 1)
                w.uvar(cmd, 2)
                w.uvar(resn, 3)
                for r in res:
                    w.svar(r, resn)
            s = n // 2 + sum(x)
            means[c] = means[c][1:] + [tdiv(s, n)]
            hist[c] = (hist[c] + x)[-wrap:]
            if cmd == 8:
                # A ZERO block in the source: the fixture's sound has it too.
                for i in range(n):
                    chans[c][k * 256 + i] = 0
    w.uvar(4, 2)
    put("fmt400-shn.shn", b"ajkg\x02" + w.bytes())
    # The same stream in a NIST SPHERE file, as the LDC's corpora ship it.
    head = ("NIST_1A\n   1024\nsample_count -i %d\nchannel_count -i %d\nsample_n_bytes -i 2\n"
            "sample_rate -i %d\nsample_byte_format -s2 01\n"
            "sample_coding -s26 pcm,embedded-shorten-v2.00\nend_head\n" % (FRAMES, CH, RATE))
    put("fmt400-shn.sph", head.encode().ljust(1024, b" ") + b"ajkg\x02" + w.bytes())
    # What the stream decodes to (the zero block included), for ffmpeg to agree with.
    inter = b"".join(struct.pack("<%dh" % CH, *[chans[c][i] for c in range(CH)]) for i in range(FRAMES))
    put("fmt400-shn.src", inter)


# ---------------------------------------------------------------- 8SVX Fibonacci
FIB = [-34, -21, -13, -8, -5, -3, -2, -1, 0, 1, 2, 3, 5, 8, 13, 21]


def fib8svx():
    left = [max(-128, min(127, v >> 8)) for v in chans[0]]
    if len(left) % 2:
        left = left[:-1]
    cur = left[0]
    body = bytearray([0, cur & 0xFF])
    decoded = []
    for i in range(0, len(left) - 2, 2):
        out = 0
        # The high nibble first (D1Unpack in EA's 8SVX appendix; ffmpeg reads the low
        # one first, so its decode is not the oracle here: these values are).
        for t in left[i + 1:i + 3]:
            best = min(range(16), key=lambda k: abs(((cur + FIB[k] + 128) % 256 - 128) - t))
            cur = (cur + FIB[best] + 128) % 256 - 128
            decoded.append(cur)
            out = (out << 4) | best
        body.append(out)
    put("fmt400-fib.pcm", struct.pack("<%dh" % len(decoded), *[v << 8 for v in decoded]))
    frames = 2 * (len(body) - 2)
    vhdr = struct.pack(">IIIHBBI", frames, 0, 0, RATE, 1, 1, 0x10000)
    form = b"8SVX" + b"VHDR" + struct.pack(">I", len(vhdr)) + vhdr
    form += b"BODY" + struct.pack(">I", len(body)) + bytes(body) + (b"\0" if len(body) % 2 else b"")
    put("fmt400-fib.8svx", b"FORM" + struct.pack(">I", len(form)) + form)


# ---------------------------------------------------------------- BW64
def bw64():
    data = wav_bytes[44:]
    fmt = wav_bytes[12:36]
    ds64 = b"ds64" + struct.pack("<IQQQI", 28, 0, len(data), FRAMES, 0)
    body = b"WAVE" + ds64 + fmt + b"data" + struct.pack("<I", 0xFFFFFFFF) + data
    put("fmt400-bw64.bw64", b"BW64" + struct.pack("<I", 0xFFFFFFFF) + body)


# ---------------------------------------------------------------- DSD
def dsd():
    rate = 2822400
    n = rate // 10
    chans_bits = []
    for c, f in enumerate((1000.0, 3000.0)):
        bits = bytearray(n)
        i1 = i2 = 0.0
        y = 0.0
        for k in range(n):
            v = 0.4 * math.sin(2 * math.pi * f * k / rate)
            i1 += v - y
            i2 += i1 - y
            b = 1 if i2 >= 0 else 0
            y = 1.0 if b else -1.0
            bits[k] = b
        chans_bits.append(bits)
    nbytes = n // 8
    msb = [bytes(sum(bits[8 * i + j] << (7 - j) for j in range(8)) for i in range(nbytes)) for bits in chans_bits]
    # DSDIFF: interleaved by byte, oldest bit on top.
    inter = bytes(b for i in range(nbytes) for b in (msb[0][i], msb[1][i]))
    fs = b"FS  " + struct.pack(">QI", 4, rate)
    chnl = b"CHNL" + struct.pack(">QH", 10, 2) + b"SLFTSRGT"
    cmpr = b"CMPR" + struct.pack(">Q", 20) + b"DSD " + bytes([14]) + b"not compressed" + b"\0"
    prop = b"SND " + fs + chnl + cmpr
    body = b"DSD " + b"FVER" + struct.pack(">QI", 4, 0x01050000) + b"PROP" + struct.pack(">Q", len(prop)) + prop
    body += b"DSD " + struct.pack(">Q", len(inter)) + inter
    put("fmt400-dsd.dff", b"FRM8" + struct.pack(">Q", len(body)) + body)
    # DSF: 4096-byte blocks a channel in turn, least significant bit first.
    lsb = [bytes(int("{:08b}".format(b)[::-1], 2) for b in m) for m in msb]
    blocks = (nbytes + 4095) // 4096
    data = bytearray()
    for k in range(blocks):
        for c in range(2):
            blk = lsb[c][k * 4096:(k + 1) * 4096]
            data += blk + b"\0" * (4096 - len(blk))
    fmt = b"fmt " + struct.pack("<QIIIIIIQII", 52, 1, 0, 2, 2, rate, 1, n, 4096, 0)
    dat = b"data" + struct.pack("<Q", 12 + len(data)) + bytes(data)
    total = 28 + len(fmt) + len(dat)
    put("fmt400-dsd.dsf", b"DSD " + struct.pack("<QQQ", 28, total, 0) + fmt + dat)


# ---------------------------------------------------------------- modules
def sine8(n=32):
    return [int(round(100 * math.sin(2 * math.pi * i / n))) for i in range(n)]


def noise8(n=4000):
    x = 12345
    out = []
    for i in range(n):
        x = (x * 1103515245 + 12345) & 0x7FFFFFFF
        out.append(int(((x >> 16) % 200 - 100) * (1 - i / n)))
    return out


SINE = sine8() * 8  # 256 frames, loops whole
NOISE = noise8()
# (row, channel, note 0-11 + 12*octave from C-0, sample 1-2, effect)
SONG = [
    (0, 0, 48, 1, None), (0, 1, 52, 1, None), (0, 2, 55, 2, None),
    (8, 0, 50, 1, ("arp", 0x47)), (8, 3, 36, 2, None),
    (16, 1, 57, 1, ("vibrato", 0x48)), (24, 0, 45, 1, ("slide_down", 2)),
    (32, 2, 60, 2, None), (40, 0, 48, 1, ("volslide_down", 4)),
    (48, 3, 43, 1, ("porta_up", 3)), (56, 1, 55, 1, None),
]
AMIGA = [1712, 1616, 1525, 1440, 1357, 1281, 1209, 1141, 1077, 1017, 961, 907]


def mod_file():
    # MOD note n (C-0 = 0): octave 1 is C-1 = 12 in this numbering; period = AMIGA >> octave.
    head = b"fmt400 mod test".ljust(20, b"\0")
    smp = b""
    for name, data, loop in ((b"sine", SINE, True), (b"noise", NOISE, False)):
        words = len(data) // 2
        smp += name.ljust(22, b"\0") + struct.pack(">HBBHH", words, 0, 64, 0, words if loop else 1)
    smp += (b"\0" * 22 + struct.pack(">HBBHH", 0, 0, 0, 0, 1)) * 29
    orders = bytes([0, 0]) + b"\0" * 126
    pat = bytearray(64 * 4 * 4)
    for row, ch, note, s, fx in SONG:
        period = AMIGA[note % 12] >> (note // 12 - 1) if note >= 12 else 0
        fxc, prm = {None: (0, 0), "arp": (0, 0x47), "vibrato": (4, 0x48), "slide_down": (2, 2),
                    "volslide_down": (10, 0x04), "porta_up": (1, 3)}[fx[0] if fx else None]
        o = (row * 4 + ch) * 4
        pat[o:o + 4] = bytes([(s & 0xF0) | (period >> 8), period & 0xFF, ((s & 15) << 4) | fxc, prm])
    o = (63 * 4 + 3) * 4
    pat[o + 2] |= 0x0F
    pat[o + 3] = 4  # F04: speed 4 on the last row
    data = head + smp + bytes([2, 127]) + orders + b"M.K." + bytes(pat)
    data += bytes(v & 0xFF for v in SINE) + bytes(v & 0xFF for v in NOISE)
    put("fmt400.mod", data)


def s3m_file():
    insnum, patnum, ordnum = 2, 1, 2
    hdr = bytearray(96)
    hdr[0:15] = b"fmt400 s3m test"
    hdr[28] = 0x1A
    hdr[29] = 16
    struct.pack_into("<HHHHHH", hdr, 32, ordnum, insnum, patnum, 0, 0x1320, 2)
    hdr[44:48] = b"SCRM"
    hdr[48], hdr[49], hdr[50], hdr[51] = 64, 6, 125, 0xB0
    hdr[64:96] = bytes([0, 8, 1, 9] + [255] * 28)
    orders = bytes([0, 255])
    base = 96 + ordnum + 2 * (insnum + patnum)
    base = (base + 15) // 16 * 16
    ins_at = [base, base + 80]
    pat_at = base + 160
    # Pattern
    rows = [[] for _ in range(64)]
    for row, ch, note, s, fx in SONG:
        cell = bytes([0x20 | 0x40 | ch, ((note // 12) << 4) | (note % 12), s, 64])
        if fx:
            f, p = {"arp": (10, 0x47), "vibrato": (8, 0x48), "slide_down": (5, 2),
                    "volslide_down": (4, 0x04), "porta_up": (6, 3)}[fx[0]]
            cell = bytes([cell[0] | 0x80]) + cell[1:] + bytes([f, p])
        rows[row].append(cell)
    packed = b"".join(b"".join(r) + b"\0" for r in rows)
    pat = struct.pack("<H", len(packed) + 2) + packed
    smp_at = pat_at + (len(pat) + 15) // 16 * 16
    inst = b""
    sdata = b""
    for k, (name, data, loop) in enumerate(((b"sine", SINE, True), (b"noise", NOISE, False))):
        ptr = (smp_at + len(sdata)) // 16
        h = bytearray(80)
        h[0] = 1
        h[13] = ptr >> 16
        struct.pack_into("<HIIIBBBBI", h, 14, ptr & 0xFFFF, len(data), 0, len(data), 64, 0, 0, 1 if loop else 0, 8363)
        h[48:52] = name
        h[76:80] = b"SCRS"
        inst += bytes(h)
        blob = bytes((v + 128) & 0xFF for v in data)
        sdata += blob + b"\0" * (-len(blob) % 16)
    para = struct.pack("<HHH", ins_at[0] // 16, ins_at[1] // 16, pat_at // 16)
    out = bytes(hdr) + orders + para
    out += b"\0" * (base - len(out)) + inst
    out += b"\0" * (pat_at - len(out)) + pat
    out += b"\0" * (smp_at - len(out)) + sdata
    put("fmt400.s3m", out)


def xm_file():
    ch = 4
    hdr = b"Extended Module: " + b"fmt400 xm test".ljust(20, b"\0") + b"\x1a" + b"specular".ljust(20, b"\0")
    hdr += struct.pack("<HIHHHHHHHH", 0x0104, 276, 2, 0, ch, 1, 2, 1, 6, 125)
    hdr += bytes([0, 0]) + b"\0" * 254
    cells = [[None] * ch for _ in range(64)]
    for row, c, note, s, fx in SONG:
        f, p = {None: (0, 0), "arp": (0, 0x47), "vibrato": (4, 0x48), "slide_down": (2, 2),
                "volslide_down": (10, 0x04), "porta_up": (1, 3)}[fx[0] if fx else None]
        cells[row][c] = (note + 1, s, 0x50, f, p)
    cells[60][0] = (97, 0, 0, 0, 0)  # key off: the envelope's release
    pdata = bytearray()
    for r in cells:
        for c in r:
            if c is None:
                pdata.append(0x80)
            else:
                pdata += bytes(c)
    pat = struct.pack("<IBHH", 9, 0, 64, len(pdata)) + bytes(pdata)
    insts = b""
    for k, (name, data, loop) in enumerate(((b"sine", SINE, True), (b"noise", NOISE, False))):
        ih = bytearray(263)
        struct.pack_into("<I", ih, 0, 263)
        ih[4:4 + len(name)] = name
        struct.pack_into("<HI", ih, 27, 1, 40)
        env = [(0, 64), (10, 40), (40, 30), (80, 0)]
        for i, (x, y) in enumerate(env):
            struct.pack_into("<HH", ih, 129 + 4 * i, x, y)
        ih[225] = len(env)
        ih[227] = 2  # sustain at point 2
        ih[233] = 1 | 2 if k == 0 else 0
        struct.pack_into("<H", ih, 239, 800)
        sh = struct.pack("<IIIBbBBbB", len(data), 0, len(data) if loop else 0, 64, 0, 1 if loop else 0, 128, 0, 0)
        sh += name.ljust(22, b"\0")
        prev = 0
        delta = bytearray()
        for v in data:
            delta.append((v - prev) & 0xFF)
            prev = v
        insts += bytes(ih) + sh + bytes(delta)
    put("fmt400.xm", hdr + pat + insts)


def it214(data):
    """IT 2.14 packing at the widest code (9 bits a delta, no width changes)."""
    bits = []
    prev = 0
    for v in data:
        # At 9 bits the delta is the low 8 (the top bit set is a width change).
        d = (v - prev) & 0xFF
        prev = v
        for i in range(9):
            bits.append((d >> i) & 1)
    bits += [0] * (-len(bits) % 8)
    blob = bytes(sum(bits[i + j] << j for j in range(8)) for i in range(0, len(bits), 8))
    return struct.pack("<H", len(blob)) + blob


def it_file():
    ch = 4
    ordnum, insnum, smpnum, patnum = 2, 0, 2, 1
    hdr = bytearray(0xC0)
    hdr[0:4] = b"IMPM"
    hdr[4:18] = b"fmt400 it test"
    struct.pack_into("<HHHHHHHH", hdr, 0x20, ordnum, insnum, smpnum, patnum, 0x0214, 0x0214, 1 | 8, 0)
    hdr[0x30], hdr[0x31], hdr[0x32], hdr[0x33], hdr[0x34] = 128, 48, 6, 125, 128
    hdr[0x40:0x80] = bytes([16, 48, 32, 32] + [32 | 128] * 60)
    hdr[0x80:0xC0] = bytes([64] * 64)
    orders = bytes([0, 255])
    off_at = 0xC0 + ordnum
    smp_hdr_at = off_at + 4 * (smpnum + patnum)
    pat_at = smp_hdr_at + 80 * smpnum
    rows = [[] for _ in range(64)]
    for row, c, note, s, fx in SONG:
        f, p = {None: (0, 0), "arp": (10, 0x47), "vibrato": (8, 0x48), "slide_down": (5, 2),
                "volslide_down": (4, 0x04), "porta_up": (6, 3)}[fx[0] if fx else None]
        mask = 1 | 2 | (8 if fx else 0)
        cell = bytes([(c + 1) | 0x80, mask, note + 12, s]) + (bytes([f, p]) if fx else b"")
        rows[row].append(cell)
    packed = b"".join(b"".join(r) + b"\0" for r in rows)
    pat = struct.pack("<HH4x", len(packed), 64) + packed
    smp_at = pat_at + len(pat)
    shdrs = b""
    sdata = b""
    for k, (name, data, loop) in enumerate(((b"sine", SINE, True), (b"noise", NOISE, False))):
        packed_sample = k == 1
        blob = it214(data) if packed_sample else bytes(v & 0xFF for v in data)
        flags = 1 | (16 if loop else 0) | (8 if packed_sample else 0)
        h = bytearray(80)
        h[0:4] = b"IMPS"
        h[17], h[18], h[19] = 64, flags, 64
        h[20:20 + len(name)] = name
        h[46] = 1
        h[47] = 32
        struct.pack_into("<IIIIIII", h, 48, len(data), 0, len(data), 8363, 0, 0, smp_at + len(sdata))
        shdrs += bytes(h)
        sdata += blob
    offs = struct.pack("<II", smp_hdr_at, smp_hdr_at + 80) + struct.pack("<I", pat_at)
    put("fmt400.it", bytes(hdr) + orders + offs + shdrs + pat + sdata)


# ---------------------------------------------------------------- CD+G
def cdg():
    packets = []

    def pkt(instr, data):
        d = list(data) + [0] * (16 - len(data))
        packets.append(bytes([9, instr, 0, 0] + [b & 0x3F for b in d] + [0] * 4))

    def empty():
        packets.append(bytes(24))

    cols = [(0, 0, 0), (15, 0, 0), (0, 15, 0), (0, 0, 15), (15, 15, 0), (15, 15, 15), (8, 8, 8), (0, 8, 15)]
    lo = []
    for r, g, b in cols:
        v = (r << 8) | (g << 4) | b
        lo += [v >> 6, v & 0x3F]
    pkt(30, lo)
    pkt(31, lo)
    pkt(1, [6, 0])
    pkt(2, [3])
    for k in range(80):
        row, col = 1 + k % 16, 1 + (k * 7) % 48
        pat = [(0x2A if (i + k) % 2 else 0x15) for i in range(12)]
        pkt(38 if k % 5 == 4 else 6, [k % 8, (k + 3) % 8, row, col] + pat)
        if k % 3 == 0:
            empty()
    pkt(20, [2, 0x10 | 2, 0x20 | 3])
    for k in range(20):
        pkt(6, [1, 4, 5 + k % 4, 10 + k] + [0x3F, 0, 0x3F, 0, 0x3F, 0, 0x3F, 0, 0x3F, 0, 0x3F, 0])
    pkt(24, [0, 0x20 | 1, 0x10 | 5])
    while len(packets) % 12:
        empty()
    for _ in range(24):
        empty()
    put("fmt400.cdg", b"".join(packets))


shorten()
fib8svx()
bw64()
dsd()
mod_file()
s3m_file()
xm_file()
it_file()
cdg()
