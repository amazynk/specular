#!/usr/bin/env python3
"""Writes the synthetic thermal fixtures in tests/data for specular-fmt-thermal.

    python3 -I make_fixtures.py OUT_DIR

Needs numpy and Pillow (for the small visible JPEGs only). Every file is built here from
the format layouts; nothing is copied from a real camera file.

  fff_raw_le.fff     FFF, little-endian header, raw counts little-endian
  fff_raw_be.fff     FFF, big-endian header, CameraInfo and raw counts big-endian
  fff_png.fff        raw counts in a 16-bit PNG holding little-endian bytes (FLIR's quirk),
                     with palette, embedded visible JPEG and PiP records
  fff_tiff_le.fff    raw counts in an uncompressed little-endian TIFF
  fff_tiff_be.fff    the same, big-endian TIFF
  rjpeg.jpg          a JPEG carrying fff_png's FFF split over three APP1 segments
  flir_noraw.jpg     a JPEG with a FLIR APP1 FFF holding no RawData record
  irg_c201.irg       InfiRay C201 header, 1/16 K counts, visible JPEG of stated length
  irg_c201_k10.irg   C201 header with flag 0 = 1 (1/10 K), no JPEG
  irg_other.irg      the BA AB header, 1/10 K, JPEG to the end of the file
  irg_p200.irg       P200 header (04 A0), 1/10 K from 273.2 K
  irb_c0.irb         InfraTec, uncompressed table indices and fractions
  irb_c1.irb         InfraTec, run-length coded indices
  irb_c2.irb         InfraTec, 16-bit centikelvin in the delta-coded bit stream
"""

import io
import math
import os
import struct
import sys
import zlib

import numpy as np
from PIL import Image

OUT = sys.argv[1]
os.makedirs(OUT, exist_ok=True)

W, H = 16, 12

R1, R2, B, F, O = 16951.796875, 0.0142948674, 1435.1, 1.0, -7142


def counts(t):
    return R1 / (R2 * (math.exp(B / (t + 273.15)) - F)) - O


def scene():
    """Counts of a 10..60 degree ramp with a hot spot."""
    y, x = np.mgrid[0:H, 0:W]
    t = 10 + 50 * (x + y * W) / (W * H - 1)
    t[5:8, 6:10] = 85.0
    return np.round(np.vectorize(counts)(t)).astype(np.uint16)


def visible_jpeg(w, h, seed):
    rng = np.random.default_rng(seed)
    a = (rng.random((h, w, 3)) * 255).astype(np.uint8)
    b = io.BytesIO()
    Image.fromarray(a, "RGB").save(b, "JPEG", quality=80)
    return b.getvalue()


def png16(px, swap):
    """A 16-bit grey PNG; `swap` stores each value's little-endian bytes."""
    h, w = px.shape
    data = px.astype("<u2" if swap else ">u2").tobytes()
    raw = b"".join(b"\0" + data[r * 2 * w:(r + 1) * 2 * w] for r in range(h))

    def chunk(t, d):
        return struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d))

    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 16, 0, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(raw)) + chunk(b"IEND", b""))


def tiff16(px, le):
    h, w = px.shape
    e = "<" if le else ">"
    tags = [(256, 3, 1, w), (257, 3, 1, h), (258, 3, 1, 16), (259, 3, 1, 1), (262, 3, 1, 1),
            (273, 4, 1, 0), (277, 3, 1, 1), (278, 3, 1, h), (279, 4, 1, w * h * 2)]
    ifd_len = 2 + 12 * len(tags) + 4
    data_at = 8 + ifd_len
    out = (b"II*\0" if le else b"MM\0*") + struct.pack(e + "I", 8) + struct.pack(e + "H", len(tags))
    for tag, ty, n, v in tags:
        if tag == 273:
            v = data_at
        val = struct.pack(e + "HH", v, 0) if ty == 3 else struct.pack(e + "I", v)
        out += struct.pack(e + "HHI", tag, ty, n) + val
    out += struct.pack(e + "I", 0)
    return out + px.astype(e + "u2").tobytes()


def raw_record(px, payload, le):
    h, w = px.shape
    e = "<" if le else ">"
    return struct.pack(e + "HHH", 2, w, h) + bytes(26) + payload


def camera_info(le, emissivity=0.95, distance=1.5, refl=21.0, atm=18.0, rh=0.45):
    e = "<" if le else ">"
    r = bytearray(0x9ac)

    def f(at, v):
        r[at:at + 4] = struct.pack(e + "f", v)

    def s(at, n, v):
        r[at:at + len(v)] = v.encode()

    r[0:2] = struct.pack(e + "H", 2)
    f(0x20, emissivity)
    f(0x24, distance)
    f(0x28, refl + 273.15)
    f(0x2c, atm + 273.15)
    f(0x30, atm + 273.15)
    f(0x34, 1.0)
    f(0x3c, rh)
    f(0x58, R1)
    f(0x5c, B)
    f(0x60, F)
    f(0x70, 0.006569)
    f(0x74, 0.01262)
    f(0x78, -0.002276)
    f(0x7c, -0.00667)
    f(0x80, 1.9)
    f(0x90, 150 + 273.15)
    f(0x94, -20 + 273.15)
    s(0xd4, 32, "Synthetic FLIR")
    s(0xf4, 16, "00000-0000")
    s(0x104, 16, "12345678")
    s(0x114, 16, "1.2.3")
    s(0x170, 32, "FOL7")
    s(0x190, 16, "T000000")
    s(0x1a0, 16, "87654321")
    f(0x1b4, 45.0)
    r[0x308:0x30c] = struct.pack(e + "i", O)
    f(0x30c, R2)
    r[0x384:0x38e] = struct.pack(e + "IIh", 1700000000, 250, -60)
    return bytes(r)


def palette_info():
    """16 entries from black through red to yellow-white, as Y, Cr, Cb."""
    n = 16
    r = bytearray(0x70 + 3 * n)
    r[0] = n
    for i in range(n):
        t = i / (n - 1)
        rgb = (255 * min(1, 2 * t), 255 * max(0, 2 * t - 1), 255 * max(0, 4 * t - 3))
        y = 0.299 * rgb[0] + 0.587 * rgb[1] + 0.114 * rgb[2]
        cr = 128 + 0.713 * (rgb[0] - y)
        cb = 128 + 0.564 * (rgb[2] - y)
        r[0x70 + 3 * i:0x73 + 3 * i] = bytes(int(round(min(255, max(0, v)))) for v in (y, cr, cb))
    r[0x06:0x09] = bytes((235, 128, 128))
    r[0x09:0x0c] = bytes((16, 128, 128))
    r[0x12:0x15] = bytes((150, 44, 21))
    r[0x50:0x50 + 4] = b"Test"
    return bytes(r)


def embedded(jpeg, w, h):
    return struct.pack("<HHH", 2, w, h) + bytes(26) + jpeg


def pip():
    return struct.pack("<fhhHHHH", 1.5, 3, -2, 0, 15, 0, 11) + bytes(0x50)


def fff(records, be_header):
    e = ">" if be_header else "<"
    n = len(records)
    head = b"FFF\0" + b"Specular test\0\0\0" + struct.pack(e + "III", 100, 64, n) + bytes(64 - 32)
    body = b""
    entries = b""
    at = 64 + 32 * n
    for i, (ty, sub, data) in enumerate(records):
        entries += struct.pack(e + "HHIIIIIII", ty, sub, 100, i + 1, at + len(body), len(data), 0, 0, 0)
        body += data
    return head + entries + body


def write(name, data):
    with open(os.path.join(OUT, name), "wb") as f:
        f.write(data)


px = scene()
write("fff_raw_le.fff", fff([(0x20, 1, camera_info(True)),
                             (0x01, 2, raw_record(px, px.astype("<u2").tobytes(), True))], False))
write("fff_raw_be.fff", fff([(0x01, 1, raw_record(px, px.astype(">u2").tobytes(), False)),
                             (0x20, 1, camera_info(False, emissivity=0.8, distance=10, refl=30, atm=25, rh=0.7))],
                            True))
vis = visible_jpeg(24, 18, 1)
png_fff = fff([(0x0e, 1, embedded(vis, 24, 18)),
               (0x20, 1, camera_info(True, emissivity=0.98, distance=0.5, refl=15, atm=15, rh=0.3)),
               (0x22, 1, palette_info()),
               (0x01, 3, raw_record(px, png16(px, swap=True), True)),
               (0x2a, 1, pip())], True)
write("fff_png.fff", png_fff)
write("fff_tiff_le.fff", fff([(0x20, 1, camera_info(True)),
                              (0x01, 1, raw_record(px, tiff16(px, True), True))], True))
write("fff_tiff_be.fff", fff([(0x20, 1, camera_info(True)),
                              (0x01, 1, raw_record(px, tiff16(px, False), True))], True))


def with_flir(jpeg, payload, parts):
    size = -(-len(payload) // parts)
    segs = b""
    for i in range(parts):
        p = b"FLIR\0\x01" + bytes((i, parts - 1)) + payload[i * size:(i + 1) * size]
        segs += b"\xff\xe1" + struct.pack(">H", len(p) + 2) + p
    return jpeg[:2] + segs + jpeg[2:]


main = visible_jpeg(32, 24, 2)
write("rjpeg.jpg", with_flir(main, png_fff, 3))
write("flir_noraw.jpg", with_flir(main, fff([(0x20, 1, camera_info(True))], True), 1))


# InfiRay IRG.
def irg(magic, flag0, jpeg_len, kelvin_div, temps_k, tail, end_marker=True):
    hl = 128
    h, w = temps_k.shape
    fine = np.round(temps_k * kelvin_div).astype("<u2")
    coarse = np.round(255 * (temps_k - temps_k.min()) / np.ptp(temps_k)).astype(np.uint8)
    hdr = bytearray(hl)
    hdr[0:2] = magic
    hdr[2:4] = struct.pack("<H", hl)
    hdr[4:34] = struct.pack("<IHHHHHHHIHHI", w * h, h, w, flag0, 0, 0, hl + w * h, 0, jpeg_len, h, w, 9500)
    hdr[34:78] = struct.pack("<9IHHI", 10000, 10000, 25000, 0, 0, 0, 0, 0, 0, 0, 1, 0)
    if end_marker:
        hdr[hl - 2:hl] = b"\xac\xca"
    return bytes(hdr) + coarse.tobytes() + fine.tobytes() + tail


yy, xx = np.mgrid[0:H, 0:W]
tk = 273.15 + 5 + 45 * (xx + yy * W) / (W * H - 1)
vis2 = visible_jpeg(20, 15, 3)
write("irg_c201.irg", irg(b"\xca\xac", 0, len(vis2), 16, tk, vis2))
write("irg_c201_k10.irg", irg(b"\xca\xac", 1, 0, 10, tk + 30, b""))
write("irg_other.irg", irg(b"\xba\xab", 0, 0, 10, tk - 20, vis2))
write("irg_p200.irg", irg(b"\x04\xa0", 0, 0, 10, tk + 100, b'{"roi":[]}', end_marker=False))


# InfraTec IRB.
TABLE = np.linspace(250.0, 400.0, 256).astype("<f4")


def irb_image(comp, pixels):
    h, w = H, W
    hdr = struct.pack("<hhhhihhhhhhfffhhfhhfhhhh", 2, comp, w, h, 0, 0, w - 1, 0, h - 1, 0, 0,
                      0.93, 2.5, 296.15, 0, 0, 294.15, 0x65, 0, 9.5, 0, 0x4080, 9, 0x101)
    assert len(hdr) == 60
    meta = bytearray(644)
    meta[92:100] = struct.pack("<ff", 233.15, 773.15)
    meta[142:142 + 9] = b"VarioTEST"
    meta[164:164 + 6] = b"SN0042"
    meta[202:202 + 9] = b"OPTIC 30\0"
    meta[450:450 + 6] = b"LS0007"
    meta[540:552] = struct.pack("<di", 45000.5, 123)
    meta[554:554 + 7] = b"30 mm\0\0"
    return hdr + TABLE.tobytes() + bytes(meta) + pixels


def irb(comp, pixels):
    img = irb_image(comp, pixels)
    head = bytearray(64)
    head[0:5] = b"\xffIRB\0"
    head[5:13] = b"IRBACS\0\0"
    head[13:21] = b"IRBACS\0\0"
    head[21:33] = struct.pack("<iii", 100, 64, 2)
    data_at = 64 + 2 * 32
    entries = struct.pack("<8i", 1, 100, 0, data_at, len(img), 0, 0, 0)
    entries += struct.pack("<8i", 0, 0, 0, 0, 0, 0, 0, 0)
    return bytes(head) + entries + img


rng = np.random.default_rng(7)
idx = np.clip((xx + yy * W) * 254 // (W * H - 1), 0, 254).astype(np.uint8)
frac = rng.integers(0, 256, size=(H, W), dtype=np.uint8)
write("irb_c0.irb", irb(0, np.stack([frac, idx], axis=-1).tobytes()))

flat_idx = idx.flatten()
runs = b""
i = 0
while i < len(flat_idx):
    j = i
    while j < len(flat_idx) and flat_idx[j] == flat_idx[i] and j - i < 255:
        j += 1
    runs += bytes((j - i, int(flat_idx[i])))
    i = j
write("irb_c1.irb", irb(1, frac.tobytes() + runs))


def delta_code(v):
    bits = []

    def put(x, n):
        bits.extend((x >> k) & 1 for k in range(n - 1, -1, -1))

    for a, b in zip(v[:-1], v[1:]):
        d = int(b) - int(a)
        u = (-d << 1) | 1 if d < 0 else d << 1
        if u < 192:
            put(u, 8)
        else:
            c = u + 192 * 2047
            put(c >> 11, 8)
            put(c & 2047, 11)
    while len(bits) % 16:
        bits.append(0)
    out = struct.pack("<H", int(v[0]))
    for k in range(0, len(bits), 16):
        word = 0
        for bit in bits[k:k + 16]:
            word = (word << 1) | bit
        out += struct.pack("<H", word)
    return out


cent = np.round((tk + 10 * np.sin(xx)) * 100).astype(np.uint16).flatten()
cent[5] = cent[4] + 3000
write("irb_c2.irb", irb(2, delta_code(cent)))
print("fixtures written to", OUT)
