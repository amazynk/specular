"""Builds the retro-format fixtures and their oracle decodes.

Usage: python3 -I retro-oracle.py OUT_DIR

Writes `<name>.<ext>` fixtures into OUT_DIR, and for each one an allowed reference decoder
reads, `<name>.<ext>.oracle` (a "W H\\n" line, then RGBA bytes):

* ImageMagick (`magick`, Apache-style ImageMagick licence; a tool, its code is not read)
  writes Sixel and AVS and reads Sixel, AVS, MacPaint, Alias PIX, Wavefront RLA, Utah RLE,
  Dr. Halo CUT, ZX Spectrum SCR and the JPEG inside a QuickTime image.
* Pillow (HPND) writes and reads BLP1/BLP2 palette textures, reads BLP2 DXT1, and writes
  MPO stereo pairs.

Fixtures without a reference decoder (DEGAS, NEOchrome, Koala, GEM, Softimage PIC, WAL,
X cursor, USGS DEM, OFF) are written for eye checks only. The test
`crates/specular-decode/tests/retro_oracle.rs` (SPECULAR_RETRO_CORPUS=OUT_DIR) compares.
"""
import io
import os
import struct
import subprocess
import sys

import numpy as np
from PIL import Image

OUT = sys.argv[1]
os.makedirs(OUT, exist_ok=True)
W, H = 64, 48


def path(name):
    return os.path.join(OUT, name)


def source_rgb(w=W, h=H):
    y, x = np.mgrid[0:h, 0:w]
    r = (x * 255 // max(w - 1, 1)).astype(np.uint8)
    g = (y * 255 // max(h - 1, 1)).astype(np.uint8)
    b = (((x // 8) + (y // 8)) % 2 * 200 + 30).astype(np.uint8)
    img = np.stack([r, g, b], -1)
    img[h // 3:h // 2, w // 4:w // 2] = [250, 250, 20]
    return img


def oracle_magick(fixture, fmt=None):
    spec = f"{fmt}:{fixture}" if fmt else fixture
    out = subprocess.run(["magick", spec + "[0]", "-depth", "8", "rgba:-"],
                         capture_output=True, check=True).stdout
    ident = subprocess.run(["magick", "identify", "-format", "%w %h", spec + "[0]"],
                           capture_output=True, check=True).stdout.decode().split()
    w, h = int(ident[0]), int(ident[1])
    write_oracle(fixture, w, h, out[:w * h * 4])


def write_oracle(fixture, w, h, rgba):
    with open(fixture + ".oracle", "wb") as f:
        f.write(f"{w} {h}\n".encode())
        f.write(bytes(rgba))


def packbits(row):
    out = bytearray()
    i = 0
    n = len(row)
    while i < n:
        run = 1
        while i + run < n and row[i + run] == row[i] and run < 128:
            run += 1
        if run >= 3:
            out += bytes([(257 - run) & 0xff, row[i]])
            i += run
        else:
            s = i
            while i < n and i - s < 128 and not (i + 2 < n and row[i] == row[i + 1] == row[i + 2]):
                i += 1
            out.append(i - s - 1)
            out += bytes(row[s:i])
    return bytes(out)


# ---- ImageMagick-written: Sixel, AVS -------------------------------------------------------
src = path("src.png")
Image.fromarray(source_rgb()).save(src)
for name, fmt in [("im.six", "sixel"), ("im.avs", "avs")]:
    subprocess.run(["magick", src, f"{fmt}:{path(name)}"], check=True)
    oracle_magick(path(name), fmt)

# ---- MacPaint: 576×720 bits, PackBits rows, with and without MacBinary ---------------------
yy, xx = np.mgrid[0:720, 0:576]
bits = (((xx // 7) ^ (yy // 5)) & 1) | ((xx - 288) ** 2 + (yy - 360) ** 2 < 150 ** 2)
packed = np.packbits(bits.astype(np.uint8), axis=1)
body = struct.pack(">I", 2) + bytes(508) + b"".join(packbits(r.tobytes()) for r in packed)
with open(path("plain.mac"), "wb") as f:
    f.write(body)
mb = bytearray(128)
mb[1] = 5
mb[2:7] = b"paint"
mb[65:69] = b"PNTG"
mb[69:73] = b"MPNT"
with open(path("macbinary.pntg"), "wb") as f:
    f.write(bytes(mb) + body)
oracle_magick(path("plain.mac"), "mac")
# ImageMagick's MacPaint reader skips a MacBinary header only by its own guess: the same
# pixels as the plain file are the reference.
with open(path("plain.mac.oracle"), "rb") as a, open(path("macbinary.pntg.oracle"), "wb") as b:
    b.write(a.read())

# ---- Alias PIX (24-bit, runs within lines) -------------------------------------------------
img = source_rgb()
pix = bytearray(struct.pack(">HHHHH", W, H, 0, 0, 24))
for row in img:
    x = 0
    while x < W:
        n = 1
        while x + n < W and n < 255 and (row[x + n] == row[x]).all():
            n += 1
        r, g, b = row[x]
        pix += bytes([n, b, g, r])
        x += n
with open(path("a.pix"), "wb") as f:
    f.write(pix)
oracle_magick(path("a.pix"), "pix")

# ---- Wavefront RLA (3 colour + 1 matte, 8-bit) ---------------------------------------------
def rla_rle(vals):
    out = bytearray()
    i = 0
    n = len(vals)
    while i < n:
        run = 1
        while i + run < n and vals[i + run] == vals[i] and run < 128:
            run += 1
        if run >= 2:
            out += bytes([run - 1, vals[i]])
            i += run
        else:
            s = i
            while i < n and i - s < 128 and not (i + 1 < n and vals[i] == vals[i + 1]):
                i += 1
            out += bytes([(256 - (i - s)) & 0xff]) + bytes(vals[s:i])
    return bytes(out)


alpha = np.fromfunction(lambda y, x: (x * 4 + y * 2) % 256, (H, W)).astype(np.uint8)
hdr = bytearray(740)
struct.pack_into(">8h", hdr, 0, 0, W - 1, 0, H - 1, 0, W - 1, 0, H - 1)
struct.pack_into(">hhhhhh", hdr, 16, 1, 0, 3, 1, 0, -2)
struct.pack_into(">hhhhh", hdr, 658, 8, 0, 8, 0, 0)
data = bytearray()
table = bytearray()
base = 740 + 4 * H
for line in range(H):
    y = H - 1 - line
    table += struct.pack(">I", base + len(data))
    for ch in [img[y, :, 0], img[y, :, 1], img[y, :, 2], alpha[y]]:
        enc = rla_rle(list(ch))
        data += struct.pack(">H", len(enc)) + enc
with open(path("a.rla"), "wb") as f:
    f.write(bytes(hdr) + table + data)
oracle_magick(path("a.rla"), "rla")

# ---- Utah RLE (3 channels, background, byte and run data) ----------------------------------
rle = bytearray(struct.pack("<HhhHH", 0xCC52, 0, 0, W, H))
rle += bytes([0, 3, 8, 0, 0])  # flags, ncolors, pixelbits, ncmap, cmaplen
rle += bytes([10, 20, 30])      # background (3: odd, no filler)
for line in range(H):
    y = H - 1 - line
    if line:
        rle += bytes([1, 1])     # skip one line
    for c in range(3):
        rle += bytes([2, c])     # set channel
        row = list(img[y, :, c])
        x = 0
        while x < W:
            n = 1
            while x + n < W and row[x + n] == row[x] and n < 256:
                n += 1
            if n >= 3:
                rle += bytes([6, n - 1, row[x], 0])
                x += n
            else:
                s = x
                x = min(W, x + 8)
                k = x - s
                rle += bytes([5, k - 1]) + bytes(row[s:x]) + (b"\0" if k % 2 else b"")
rle += bytes([7, 0])
with open(path("a.rle"), "wb") as f:
    f.write(rle)
oracle_magick(path("a.rle"), "rle")

# ---- Dr. Halo CUT + PAL ----------------------------------------------------------------------
q = Image.fromarray(source_rgb()).quantize(64)
idx = np.array(q)
palette = q.getpalette()[:64 * 3]
cut = bytearray(struct.pack("<HHH", W, H, 0))
for row in idx:
    enc = bytearray()
    x = 0
    while x < W:
        n = 1
        while x + n < W and row[x + n] == row[x] and n < 127:
            n += 1
        if n >= 2:
            enc += bytes([0x80 | n, row[x]])
            x += n
        else:
            s = x
            while x < W and x - s < 127 and not (x + 1 < W and row[x] == row[x + 1]):
                x += 1
            enc += bytes([x - s]) + bytes(row[s:x])
    enc.append(0)
    cut += struct.pack("<H", len(enc)) + enc
pal = bytearray(b"AH" + struct.pack("<HH", 0xE3, 0) + bytes([0x0A, 0]) + struct.pack("<HHHHHH", 0, 0, 63, 255, 255, 255))
pal += b"Dr. Halo".ljust(20, b"\0")
for k in range(64):
    if len(pal) % 512 + 6 > 512:
        pal += bytes(512 - len(pal) % 512)
    pal += struct.pack("<HHH", *palette[3 * k:3 * k + 3])
with open(path("a.cut"), "wb") as f:
    f.write(cut)
with open(path("a.pal"), "wb") as f:
    f.write(pal)
oracle_magick(path("a.cut"), "cut")

# ---- ZX Spectrum screen ------------------------------------------------------------------------
rng = np.random.default_rng(7)
scr = bytearray(rng.integers(0, 256, 6144, dtype=np.uint8).tobytes())
scr += bytes(int(v) & 0x7f for v in rng.integers(0, 256, 768))  # no FLASH
with open(path("a.scr"), "wb") as f:
    f.write(scr)
oracle_magick(path("a.scr"), "scr")

# ---- QuickTime image: an ImageMagick JPEG inside idsc/idat ------------------------------------
jpg = io.BytesIO()
subprocess.run(["magick", src, "-quality", "90", path("qt-inner.jpg")], check=True)
with open(path("qt-inner.jpg"), "rb") as f:
    jpeg = f.read()
desc = bytearray(86)
struct.pack_into(">I4s", desc, 0, 86, b"jpeg")
struct.pack_into(">HH", desc, 32, W, H)
struct.pack_into(">I", desc, 44, len(jpeg))
struct.pack_into(">H", desc, 48, 1)
struct.pack_into(">hh", desc, 82, 24, -1)
qt = struct.pack(">I4s", 8 + len(desc), b"idsc") + desc + struct.pack(">I4s", 8 + len(jpeg), b"idat") + jpeg
with open(path("a.qtif"), "wb") as f:
    f.write(qt)
subprocess.run(["magick", path("qt-inner.jpg"), path("qt-inner.png")], check=True)
oracle_magick(path("qt-inner.jpg"))
os.replace(path("qt-inner.jpg.oracle"), path("a.qtif.oracle"))
os.remove(path("qt-inner.jpg"))
os.remove(path("qt-inner.png"))
# Raw 24-bit RGB.
raw = source_rgb().tobytes()
desc = bytearray(86)
struct.pack_into(">I4s", desc, 0, 86, b"raw ")
struct.pack_into(">HH", desc, 32, W, H)
struct.pack_into(">hh", desc, 82, 24, -1)
with open(path("raw.qtif"), "wb") as f:
    f.write(struct.pack(">I4s", 8 + 86, b"idsc") + desc + struct.pack(">I4s", 8 + len(raw), b"idat") + raw)
write_oracle(path("raw.qtif"), W, H, np.dstack([source_rgb(), np.full((H, W), 255, np.uint8)]).tobytes())

# ---- BLP: Pillow writes palette BLP1/BLP2; DXT1 hand-built, Pillow reads all -------------------
pq = Image.fromarray(source_rgb()).quantize(200)
for v in ["blp1", "blp2"]:
    p = path(f"pillow-{v}.blp")
    pq.save(p, "BLP", blp_version=v)
    o = Image.open(p).convert("RGBA")
    write_oracle(p, o.width, o.height, o.tobytes())
blocks = bytearray()
for by in range(H // 4):
    for bx in range(W // 4):
        c0 = ((bx * 2) << 11) | ((by * 5) << 5) | (bx + by)
        c1 = 0xFFFF - c0
        if (bx + by) % 3 == 0:
            c0, c1 = min(c0, c1), max(c0, c1)  # 3-colour + transparent mode
        bits = (bx * 0x9E3779B1 + by * 0x85EBCA6B) & 0xFFFFFFFF
        blocks += struct.pack("<HHI", c0, c1, bits)
hdr = bytearray(b"BLP2" + struct.pack("<I", 1) + bytes([2, 1, 0, 0]) + struct.pack("<II", W, H))
offs = [0] * 16
sizes = [0] * 16
offs[0] = 20 + 128 + 1024
sizes[0] = len(blocks)
hdr += struct.pack("<16I", *offs) + struct.pack("<16I", *sizes) + bytes(1024)
with open(path("dxt1.blp"), "wb") as f:
    f.write(bytes(hdr) + blocks)
o = Image.open(path("dxt1.blp")).convert("RGBA")
write_oracle(path("dxt1.blp"), o.width, o.height, o.tobytes())

# ---- MPO stereo pair (Pillow) ------------------------------------------------------------------
left = Image.fromarray(source_rgb())
right = Image.fromarray(source_rgb()[:, ::-1].copy())
left.save(path("pair.mpo"), "MPO", save_all=True, append_images=[right], quality=95)
# Pillow marks the images "baseline primary" and "undefined"; stereo cameras write the
# disparity type (0x020002) for both, as CIPA DC-007 asks: patch the MP entries.
with open(path("pair.mpo"), "rb") as f:
    mpo = bytearray(f.read())
at = mpo.index(b"MPF\0") + 4
le = mpo[at:at + 2] == b"II"
end = "<" if le else ">"
ifd = at + struct.unpack_from(end + "I", mpo, at + 4)[0]
for k in range(struct.unpack_from(end + "H", mpo, ifd)[0]):
    e = ifd + 2 + 12 * k
    tag, _, count, off = struct.unpack_from(end + "HHII", mpo, e)
    if tag == 0xB002:
        for i in range(count // 16):
            a = at + off + 16 * i
            attr = struct.unpack_from(end + "I", mpo, a)[0]
            struct.pack_into(end + "I", mpo, a, (attr & 0xFF000000) | 0x020002)
with open(path("pair.mpo"), "wb") as f:
    f.write(mpo)

# ---- Eye-check fixtures (no reference decoder) -------------------------------------------------
# DEGAS low resolution: 16 colours in 4 interleaved planes.
def st_planes(idx, planes):
    h, w = idx.shape
    out = bytearray()
    for y in range(h):
        for g in range(w // 16):
            for p in range(planes):
                word = 0
                for b in range(16):
                    word |= ((int(idx[y, g * 16 + b]) >> p) & 1) << (15 - b)
                out += struct.pack(">H", word)
    return out


yy, xx = np.mgrid[0:200, 0:320]
idx16 = ((xx // 20) + (yy // 50) * 4) % 16
pal = [((i & 7) << 8) | ((7 - (i & 7)) << 4) | (i >> 1) for i in range(16)]
with open(path("low.pi1"), "wb") as f:
    f.write(struct.pack(">H16H", 0, *pal) + st_planes(idx16, 4))
neo = bytearray(128)
struct.pack_into(">HH16H", neo, 0, 0, 0, *pal)
with open(path("low.neo"), "wb") as f:
    f.write(bytes(neo) + st_planes(idx16, 4))
# Koala: stripes of every colour.
bm = bytearray()
for cell in range(1000):
    bm += bytes([0b00011011] * 8)
scr_mem = bytes(((i % 16) << 4) | ((i // 40) % 16) for i in range(1000))
col_mem = bytes((i * 7) % 16 for i in range(1000))
with open(path("a.koa"), "wb") as f:
    f.write(b"\x00\x60" + bm + scr_mem + col_mem + b"\x00")
# GEM: 1-plane circle.
gw, gh = 96, 64
yy, xx = np.mgrid[0:gh, 0:gw]
g = ((xx - 48) ** 2 + (yy - 32) ** 2 < 28 ** 2).astype(np.uint8)
gem = bytearray(struct.pack(">8H", 1, 8, 1, 2, 85, 85, gw, gh))
for row in np.packbits(g, axis=1):
    gem += b"\x80" + bytes([len(row)]) + row.tobytes()
with open(path("circle.img"), "wb") as f:
    f.write(gem)
# Softimage PIC: RGB mixed RLE + alpha pure RLE.
pic = bytearray(struct.pack(">I", 0x5380F634) + struct.pack(">f", 3.71) + bytes(80) + b"PICT")
pic += struct.pack(">HHfHH", W, H, 1.0, 3, 0) + bytes([1, 8, 2, 0xE0, 0, 8, 1, 0x10])
for y in range(H):
    row = img[y]
    pic += bytes([W - 1]) + row.tobytes()  # literal run of W pixels (W ≤ 128)
    pic += bytes([W, 255])                  # opaque alpha run
with open(path("a.pic"), "wb") as f:
    f.write(pic)
# Quake 2 WAL with a colormap.pcx beside it.
wal = bytearray(100)
wal[:8] = b"test/wal"
n = W * H
struct.pack_into("<II4I", wal, 32, W, H, 100, 100 + n, 100 + n + n // 4, 100 + n + n // 4 + n // 16)
wi = (np.add.outer(np.arange(H), np.arange(W)) % 256).astype(np.uint8)
mips = wi.tobytes() + wi[::2, ::2].tobytes() + wi[::4, ::4].tobytes() + wi[::8, ::8].tobytes()
with open(path("a.wal"), "wb") as f:
    f.write(bytes(wal) + mips)
pcx = bytearray(128)
pcx[0] = 0x0A
pcx += bytes(16) + b"\x0c" + bytes(v for i in range(256) for v in (i, 255 - i, (i * 3) % 256))
with open(path("colormap.pcx"), "wb") as f:
    f.write(pcx)
# X cursor: two sizes.
def xcur(images):
    d = bytearray(b"Xcur" + struct.pack("<III", 16, 0x10000, len(images)))
    pos = 16 + 12 * len(images)
    for nom, w, h, _ in images:
        d += struct.pack("<III", 0xFFFD0002, nom, pos)
        pos += 36 + w * h * 4
    for nom, w, h, px in images:
        d += struct.pack("<9I", 36, 0xFFFD0002, nom, 1, w, h, 1, 1, 50) + px
    return bytes(d)


def arrow(s):
    a = np.zeros((s, s, 4), np.uint8)
    for y in range(s):
        for x in range(s):
            if x <= y and x + y < s * 1.5:
                edge = x == 0 or x == y or x + y >= s * 1.5 - 1
                v = 0 if edge else 255
                a[y, x] = [v, v, v, 255]  # premultiplied BGRA = straight when opaque
    return a.tobytes()


with open(path("left_ptr"), "wb") as f:
    f.write(xcur([(24, 24, 24, arrow(24)), (48, 48, 48, arrow(48))]))
print("fixtures in", OUT)
