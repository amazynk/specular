"""The cross-format alpha test grid (tests/alpha_grid.rs): the reference image and every
fixture whose bytes we want under exact control, written here without an encoder library.
The external encoders (ImageMagick, cwebp, heif-enc, ffmpeg) run from make_fixtures.sh on the
reference PNGs this writes first.

    python3 -I grid.py OUT_DIR

The reference is 32x32 and every feature sits on the 4x4 block grid, so the BCn fixtures
encode it almost exactly. Four bands of 8 rows (16-bit values; 8-bit is v/257 rounded):
  0  alpha ramp 0..65535 over x (not multiples of 257, to catch 16-bit truncation), opaque
     colour orange (rows 0-3) and blue (rows 4-7)
  1  fully transparent pixels whose colour is saturated and non-black, 4-px cells
  2  the same colours half (rows 16-19) and quarter (rows 20-23) transparent
  3  a 4x4 checkerboard of opaque white/green and transparent magenta (1-bit alpha)
alpha_grid.rs computes the same image; keep the two in step."""

import struct
import sys
import zlib

W = H = 32
COLS = [(255, 0, 0), (0, 255, 0), (0, 0, 255), (0, 255, 255), (255, 0, 255), (255, 255, 0),
        (255, 255, 255), (128, 128, 128)]


def px16(x, y):
    """Straight RGBA, 16 bits a sample."""
    band = y // 8
    if band == 0:
        c = (255, 128, 0) if y < 4 else (0, 64, 255)
        a = (x * 65535 + 15) // 31
    elif band == 1:
        c, a = COLS[x // 4 if y < 12 else 7 - x // 4], 0
    elif band == 2:
        c, a = COLS[x // 4 if y < 20 else 7 - x // 4], 32768 if y < 20 else 16384
    else:
        cx, cy = x // 4, (y - 24) // 4
        if (cx + cy) % 2 == 0:
            c, a = ((255, 255, 255) if cx % 4 < 2 else (0, 255, 0)), 65535
        else:
            c, a = (255, 0, 255), 0
    return (c[0] * 257, c[1] * 257, c[2] * 257, a)


def to8(v):
    return (v * 255 + 32767) // 65535


def px8(x, y):
    return tuple(to8(v) for v in px16(x, y))


def px8_1bit(x, y):
    r, g, b, a = px8(x, y)
    return (r, g, b, 255 if px16(x, y)[3] >= 32768 else 0)


def grey8(p):
    return (2 * p[0] + 5 * p[1] + p[2] + 4) // 8


def rows(f):
    return [[f(x, y) for x in range(W)] for y in range(H)]


def premul(c, a, top):
    return (c * a + top // 2) // top


# --- PNG ---------------------------------------------------------------------------------

def chunk(tag, body):
    return struct.pack(">I", len(body)) + tag + body + struct.pack(">I", zlib.crc32(tag + body))


def ihdr(depth, ctype, w=W, h=H):
    return chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, depth, ctype, 0, 0, 0))


def idat_bytes(raw_rows):
    return zlib.compress(b"".join(b"\0" + r for r in raw_rows), 9)


def png(depth, ctype, raw_rows, extra=b""):
    return (b"\x89PNG\r\n\x1a\n" + ihdr(depth, ctype) + extra
            + chunk(b"IDAT", idat_bytes(raw_rows)) + chunk(b"IEND", b""))


def rgba8_rows(f=px8):
    return [bytes(v for x in range(W) for v in f(x, y)) for y in range(H)]


def rgba16_rows():
    return [b"".join(struct.pack(">4H", *px16(x, y)) for x in range(W)) for y in range(H)]


def pngs(out):
    save(out, "rgba8.png", png(8, 6, rgba8_rows()))
    save(out, "rgba8-1bit.png", png(8, 6, rgba8_rows(px8_1bit)))
    save(out, "rgba16.png", png(16, 6, rgba16_rows()))
    la8 = [bytes(v for x in range(W) for v in (grey8(px8(x, y)), px8(x, y)[3])) for y in range(H)]
    save(out, "la8.png", png(8, 4, la8))
    la16 = [b"".join(struct.pack(">2H", grey8(px8(x, y)) * 257, px16(x, y)[3]) for x in range(W))
            for y in range(H)]
    save(out, "la16.png", png(16, 4, la16))
    # Palette with a full tRNS: every distinct (colour, alpha) of the 8-bit reference.
    pal, idx = [], []
    for y in range(H):
        row = bytearray()
        for x in range(W):
            p = px8(x, y)
            if p not in pal:
                pal.append(p)
            row.append(pal.index(p))
        idx.append(bytes(row))
    assert len(pal) <= 256, len(pal)
    plte = chunk(b"PLTE", bytes(v for p in pal for v in p[:3]))
    trns = chunk(b"tRNS", bytes(p[3] for p in pal))
    save(out, "palette-trns.png", png(8, 3, idx, plte + trns))
    # Truecolour with a tRNS colour key: transparent pixels of the 1-bit reference become the
    # key colour, which no opaque pixel uses.
    key = (10, 20, 30)
    rgb = [bytes(v for x in range(W) for v in (px8_1bit(x, y)[:3] if px8_1bit(x, y)[3] else key))
           for y in range(H)]
    save(out, "rgb-trns.png", png(8, 2, rgb, chunk(b"tRNS", struct.pack(">3H", *key))))
    # APNG: frame 0 is the reference (blend SOURCE), frame 1 draws it again OVER frame 0.
    def fctl(seq, blend):
        return chunk(b"fcTL", struct.pack(">IIIIIHHBB", seq, W, H, 0, 0, 1, 10, 0, blend))
    data = idat_bytes(rgba8_rows())
    apng = (b"\x89PNG\r\n\x1a\n" + ihdr(8, 6) + chunk(b"acTL", struct.pack(">II", 2, 0))
            + fctl(0, 0) + chunk(b"IDAT", data) + fctl(1, 1)
            + chunk(b"fdAT", struct.pack(">I", 2) + data) + chunk(b"IEND", b""))
    save(out, "apng.png", apng)


# --- TIFF --------------------------------------------------------------------------------

def tiff(spp, bits, sample_format, extra, photometric, data, colormap=None):
    """Uncompressed little-endian single-strip TIFF; `colormap` is the ColorMap tag's values."""
    entries = []
    blobs = bytearray()
    base = 8

    def add(tag, typ, values):
        size = {3: 2, 4: 4}[typ]
        fmt = {3: "H", 4: "I"}[typ]
        raw = struct.pack("<%d%s" % (len(values), fmt), *values)
        entries.append((tag, typ, len(values), raw))

    add(256, 3, [W])
    add(257, 3, [H])
    add(258, 3, [bits] * spp)
    add(259, 3, [1])
    add(262, 3, [photometric])
    add(273, 4, [0])  # patched below
    add(277, 3, [spp])
    add(278, 3, [H])
    add(279, 4, [len(data)])
    add(284, 3, [1])
    if colormap is not None:
        add(320, 3, colormap)
    if extra is not None:
        add(338, 3, [extra])
    add(339, 3, [sample_format] * spp)
    n = len(entries)
    ifd_size = 2 + 12 * n + 4
    blob_off = base + ifd_size
    out_entries = []
    for tag, typ, count, raw in entries:
        if len(raw) <= 4:
            out_entries.append(struct.pack("<HHI", tag, typ, count) + raw.ljust(4, b"\0"))
        else:
            out_entries.append(struct.pack("<HHII", tag, typ, count, blob_off + len(blobs)))
            blobs += raw
            blobs += b"\0" * (len(blobs) % 2)
    data_off = blob_off + len(blobs)
    for i, (tag, typ, count, raw) in enumerate(entries):
        if tag == 273:
            out_entries[i] = struct.pack("<HHII", 273, 4, 1, data_off)
    ifd = struct.pack("<H", n) + b"".join(out_entries) + struct.pack("<I", 0)
    return b"II*\0" + struct.pack("<I", 8) + ifd + bytes(blobs) + data


def tiffs(out):
    for assoc in (False, True):
        tag = "assoc" if assoc else "unassoc"
        d8 = bytearray()
        d16 = bytearray()
        df = bytearray()
        for y in range(H):
            for x in range(W):
                p8, p16 = px8(x, y), px16(x, y)
                c8 = [premul(c, p8[3], 255) if assoc else c for c in p8[:3]]
                d8 += bytes(c8 + [p8[3]])
                c16 = [premul(c, p16[3], 65535) if assoc else c for c in p16[:3]]
                d16 += struct.pack("<4H", *c16, p16[3])
                a = p16[3] / 65535
                cf = [c / 65535 * (a if assoc else 1) for c in p16[:3]]
                df += struct.pack("<4f", *cf, a)
        extra = 1 if assoc else 2
        save(out, "rgba8-%s.tif" % tag, tiff(4, 8, 1, extra, 2, bytes(d8)))
        save(out, "rgba16-%s.tif" % tag, tiff(4, 16, 1, extra, 2, bytes(d16)))
        save(out, "rgba32f-%s.tif" % tag, tiff(4, 32, 3, extra, 2, bytes(df)))
    ga = bytearray()
    for y in range(H):
        for x in range(W):
            p = px8(x, y)
            ga += bytes([grey8(p), p[3]])
    save(out, "la8-unassoc.tif", tiff(2, 8, 1, 2, 1, bytes(ga)))


# --- OpenEXR -----------------------------------------------------------------------------

def exr_attr(name, typ, value):
    return name.encode() + b"\0" + typ.encode() + b"\0" + struct.pack("<i", len(value)) + value


def exr(ptype):
    """Uncompressed scanline RGBA, premultiplied as the spec requires; colour = v16/65535 as
    linear light. ptype 1 half, 2 float."""
    chans = ["A", "B", "G", "R"]
    cl = b"".join(c.encode() + b"\0" + struct.pack("<iB3xii", ptype, 0, 1, 1) for c in chans) + b"\0"
    box = struct.pack("<4i", 0, 0, W - 1, H - 1)
    hdr = (exr_attr("channels", "chlist", cl) + exr_attr("compression", "compression", b"\0")
           + exr_attr("dataWindow", "box2i", box) + exr_attr("displayWindow", "box2i", box)
           + exr_attr("lineOrder", "lineOrder", b"\0")
           + exr_attr("pixelAspectRatio", "float", struct.pack("<f", 1))
           + exr_attr("screenWindowCenter", "v2f", struct.pack("<2f", 0, 0))
           + exr_attr("screenWindowWidth", "float", struct.pack("<f", 1)) + b"\0")
    head = b"\x76\x2f\x31\x01" + struct.pack("<I", 2) + hdr
    fmt = "<e" if ptype == 1 else "<f"
    blocks = []
    for y in range(H):
        line = bytearray()
        for ci in (3, 2, 1, 0):  # A B G R on disk
            for x in range(W):
                p = px16(x, y)
                a = p[3] / 65535
                v = a if ci == 3 else p[ci] / 65535 * a
                line += struct.pack(fmt, v)
        blocks.append(struct.pack("<ii", y, len(line)) + bytes(line))
    off = len(head) + 8 * H
    table = b""
    for b in blocks:
        table += struct.pack("<Q", off)
        off += len(b)
    return head + table + b"".join(blocks)


# --- BMP, ICO/CUR, TGA -------------------------------------------------------------------

def bmp_v5(bpp, masks, pixel):
    """BI_BITFIELDS bottom-up BMP with a V5 header; `pixel(x, y)` gives the packed value."""
    fmt = {32: "<I", 16: "<H"}[bpp]
    rows_ = []
    for y in reversed(range(H)):
        r = b"".join(struct.pack(fmt, pixel(x, y)) for x in range(W))
        rows_.append(r + b"\0" * (-len(r) % 4))
    img = b"".join(rows_)
    v5 = struct.pack("<IiiHHIIiiII", 124, W, H, 1, bpp, 3, len(img), 2835, 2835, 0, 0)
    v5 += struct.pack("<4I", *masks) + b"BGRs" + b"\0" * 36 + b"\0" * 12
    v5 += struct.pack("<4I", 4, 0, 0, 0)
    off = 14 + len(v5)
    return b"BM" + struct.pack("<IHHI", off + len(img), 0, 0, off) + v5 + img


def bmp_v3_32(pixel):
    """BI_RGB 32-bit with a 40-byte header: the fourth byte is alpha by convention only."""
    img = b"".join(struct.pack("<I", pixel(x, y)) for y in reversed(range(H)) for x in range(W))
    hdr = struct.pack("<IiiHHIIiiII", 40, W, H, 1, 32, 0, len(img), 2835, 2835, 0, 0)
    off = 14 + len(hdr)
    return b"BM" + struct.pack("<IHHI", off + len(img), 0, 0, off) + hdr + img


def argb(p):
    return p[3] << 24 | p[0] << 16 | p[1] << 8 | p[2]


def q(v, bits):
    return (v * ((1 << bits) - 1) + 127) // 255


def bmps(out):
    save(out, "argb32-bitfields.bmp",
         bmp_v5(32, (0xFF0000, 0xFF00, 0xFF, 0xFF000000), lambda x, y: argb(px8(x, y))))
    # Unusual mask order: R in the top byte, alpha in the bottom.
    save(out, "rgba32-bitfields.bmp",
         bmp_v5(32, (0xFF000000, 0xFF0000, 0xFF00, 0xFF),
                lambda x, y: (lambda p: p[0] << 24 | p[1] << 16 | p[2] << 8 | p[3])(px8(x, y))))

    def a4r4g4b4(x, y):
        p = px8(x, y)
        return q(p[3], 4) << 12 | q(p[0], 4) << 8 | q(p[1], 4) << 4 | q(p[2], 4)
    save(out, "argb4444-bitfields.bmp", bmp_v5(16, (0xF00, 0xF0, 0xF, 0xF000), a4r4g4b4))

    def a1r5g5b5(x, y):
        p = px8_1bit(x, y)
        return (p[3] >> 7) << 15 | q(p[0], 5) << 10 | q(p[1], 5) << 5 | q(p[2], 5)
    save(out, "argb1555-bitfields.bmp", bmp_v5(16, (0x7C00, 0x3E0, 0x1F, 0x8000), a1r5g5b5))
    save(out, "argb32-birgb.bmp", bmp_v3_32(lambda x, y: argb(px8(x, y))))


def dib(bpp, pixel, mask):
    """ICO/CUR image: BITMAPINFOHEADER (double height), XOR bitmap, AND mask."""
    stride = (W * bpp // 8 + 3) // 4 * 4
    xor = b""
    for y in reversed(range(H)):
        r = b"".join(pixel(x, y) for x in range(W))
        xor += r + b"\0" * (stride - len(r))
    mstride = (W + 31) // 32 * 4
    andm = b""
    for y in reversed(range(H)):
        bits = bytearray(mstride)
        for x in range(W):
            if mask(x, y):
                bits[x // 8] |= 0x80 >> (x % 8)
        andm += bytes(bits)
    hdr = struct.pack("<IiiHHIIiiII", 40, W, 2 * H, 1, bpp, 0, len(xor) + len(andm), 0, 0, 0, 0)
    return hdr + xor + andm


def icon(kind, images):
    """`images`: list of (bpp, bytes)."""
    d = struct.pack("<HHH", 0, kind, len(images))
    off = 6 + 16 * len(images)
    body = b""
    for bpp, img in images:
        p1, p2 = (1, bpp) if kind == 1 else (3, 5)  # CUR: hotspot (3, 5)
        d += struct.pack("<BBBBHHII", W, H, 0, 0, p1, p2, len(img), off + len(body))
        body += img
    return d + body


def icos(out, rgba8_png):
    bgra = lambda x, y: (lambda p: bytes((p[2], p[1], p[0], p[3])))(px8(x, y))
    img32 = dib(32, bgra, lambda x, y: px8(x, y)[3] == 0)
    save(out, "bgra32.ico", icon(1, [(32, img32)]))
    save(out, "bgra32.cur", icon(2, [(32, img32)]))

    def bgr_masked(x, y):
        p = px8_1bit(x, y)
        return bytes((p[2], p[1], p[0])) if p[3] else b"\0\0\0"
    save(out, "bgr24-mask.ico", icon(1, [(24, dib(24, bgr_masked, lambda x, y: px8_1bit(x, y)[3] == 0))]))
    save(out, "png.ico", icon(1, [(32, rgba8_png)]))


def tga(bpp, attr_bits, pixel, alpha_type=None, rle=False):
    hdr = struct.pack("<BBBHHBHHHHBB", 0, 0, 10 if rle else 2, 0, 0, 0, 0, 0, W, H, bpp,
                      attr_bits | 0x20)
    px = [pixel(x, y) for y in range(H) for x in range(W)]
    if rle:
        body = b""
        i = 0
        while i < len(px):
            run = 1
            while run < 128 and i + run < len(px) and px[i + run] == px[i] and (i + run) % W:
                run += 1
            if run > 1:
                body += bytes([0x80 | (run - 1)]) + px[i]
                i += run
            else:
                body += b"\0" + px[i]
                i += 1
    else:
        body = b"".join(px)
    data = hdr + body
    if alpha_type is None:
        return data
    ext = bytearray(495)
    struct.pack_into("<H", ext, 0, 495)
    ext[494] = alpha_type
    ext_off = len(data)
    return data + bytes(ext) + struct.pack("<II", ext_off, 0) + b"TRUEVISION-XFILE.\0"


def tgas(out):
    bgra = lambda x, y: (lambda p: bytes((p[2], p[1], p[0], p[3])))(px8(x, y))
    save(out, "bgra32.tga", tga(32, 8, bgra))
    save(out, "bgra32-rle.tga", tga(32, 8, bgra, rle=True))
    save(out, "bgra32-straight.tga", tga(32, 8, bgra, alpha_type=3))

    def bgra_premul(x, y):
        p = px8(x, y)
        return bytes((premul(p[2], p[3], 255), premul(p[1], p[3], 255), premul(p[0], p[3], 255), p[3]))
    save(out, "bgra32-premul.tga", tga(32, 8, bgra_premul, alpha_type=4))

    def a1r5g5b5(x, y):
        p = px8_1bit(x, y)
        return struct.pack("<H", (p[3] >> 7) << 15 | q(p[0], 5) << 10 | q(p[1], 5) << 5 | q(p[2], 5))
    save(out, "argb1555.tga", tga(16, 1, a1r5g5b5))


# --- DDS / KTX2 and the BCn encoders -----------------------------------------------------

def block(f, bx, by):
    return [f(bx * 4 + i % 4, by * 4 + i // 4) for i in range(16)]


def rgb565(c):
    return q(c[0], 5) << 11 | q(c[1], 6) << 5 | q(c[2], 5)


def bc1_colour(px, punch):
    """Colour half of a BCn block. Every block of the grid has one colour (the cells are
    4x4-aligned), so the endpoints are equal; `punch` (BC1 only) makes transparent texels
    index 3 in the three-colour mode."""
    opaque = [p for p in px if not punch or p[3]]
    c = rgb565(opaque[0]) if opaque else 0
    if punch:
        # c0 <= c1 selects the 3-colour + transparent mode.
        idx = 0
        for i, p in enumerate(px):
            idx |= (0 if p[3] else 3) << (2 * i)
        return struct.pack("<HHI", c, c, idx)
    return struct.pack("<HHI", c, c, 0)


def bc2_alpha(px):
    v = 0
    for i, p in enumerate(px):
        v |= q(p[3], 4) << (4 * i)
    return struct.pack("<Q", v)


def bc3_alpha(px):
    return bc3_alpha_decoded(px)[0]


def bc3_alpha_decoded(px):
    """The BC3 alpha half and the alpha each texel decodes to."""
    a = [p[3] for p in px]
    hi, lo = max(a), min(a)
    if hi == lo:
        return struct.pack("<BB6x", hi, lo), [hi] * 16
    # 8-level mode (a0 > a1): index 0 = a0, 1 = a1, 2..7 interpolate from a0 towards a1.
    pal = [hi, lo] + [((7 - i) * hi + i * lo + 3) // 7 for i in range(1, 7)]
    bits = 0
    out = []
    for i, v in enumerate(a):
        best = min(range(8), key=lambda k: abs(pal[k] - v))
        bits |= best << (3 * i)
        out.append(pal[best])
    return struct.pack("<BB", hi, lo) + bits.to_bytes(6, "little"), out


def bc_premul_colour(px, alpha):
    """Colour half of a DXT2/DXT4 block: the block's one straight colour premultiplied by the
    alpha each texel decodes to, so the colours lie on a line from black; endpoints at the
    highest and lowest alpha, four-colour mode, nearest index."""
    pm = [tuple(premul(c, a, 255) for c in p[:3]) for p, a in zip(px, alpha)]
    hi = max(range(16), key=lambda i: alpha[i])
    lo = min(range(16), key=lambda i: alpha[i])
    c0, c1 = rgb565(pm[hi]), rgb565(pm[lo])

    def rgb(c):
        return ((c >> 11) * 255 / 31, (c >> 5 & 63) * 255 / 63, (c & 31) * 255 / 31)
    e0, e1 = rgb(c0), rgb(c1)
    pal = [e0, e1] + [tuple((w * a + (3 - w) * b) / 3 for a, b in zip(e0, e1)) for w in (2, 1)]
    idx = 0
    for i, p in enumerate(pm):
        best = min(range(4), key=lambda k: sum((pal[k][c] - p[c]) ** 2 for c in range(3)))
        idx |= best << (2 * i)
    return struct.pack("<HHI", c0, c1, idx)


def bcn_premul(kind):
    """DXT2 (kind 2) or DXT4 (kind 3) of the 8-bit reference: colour premultiplied."""
    out = b""
    for by in range(H // 4):
        for bx in range(W // 4):
            px = block(px8, bx, by)
            if kind == 2:
                out += bc2_alpha(px) + bc_premul_colour(px, [q(p[3], 4) * 17 for p in px])
            else:
                a, dec = bc3_alpha_decoded(px)
                out += a + bc_premul_colour(px, dec)
    return out


BC7_W4 = [0, 4, 9, 13, 17, 21, 26, 30, 34, 38, 43, 47, 51, 55, 60, 64]


def bc7_mode6(px):
    """BC7 mode 6 (one subset, RGBA 7.7.7.7 endpoints + a p-bit each, 4-bit indices)."""
    def quant(v, p):
        best = min(range(128), key=lambda k: abs(((k << 1) | p) - v))
        return best
    lo = [min(p[c] for p in px) for c in range(4)]
    hi = [max(p[c] for p in px) for c in range(4)]
    best = None
    for p0 in (0, 1):
        for p1 in (0, 1):
            e0 = [quant(v, p0) for v in lo]
            e1 = [quant(v, p1) for v in hi]
            f0 = [(v << 1) | p0 for v in e0]
            f1 = [(v << 1) | p1 for v in e1]
            pal = [[((64 - w) * f0[c] + w * f1[c] + 32) >> 6 for c in range(4)] for w in BC7_W4]
            idx = [min(range(16), key=lambda k: sum((pal[k][c] - p[c]) ** 2 for c in range(4)))
                   for p in px]
            err = sum(sum((pal[i][c] - p[c]) ** 2 for c in range(4)) for i, p in zip(idx, px))
            if best is None or err < best[0]:
                best = (err, e0, e1, p0, p1, idx)
    _, e0, e1, p0, p1, idx = best
    if idx[0] >= 8:  # the anchor index's top bit is implicit 0: swap the endpoints
        e0, e1, p0, p1 = e1, e0, p1, p0
        idx = [15 - i for i in idx]
    bits, n = 1 << 6, 7

    def put(v, w):
        nonlocal bits, n
        bits |= v << n
        n += w
    for c in range(4):
        put(e0[c], 7)
        put(e1[c], 7)
    put(p0, 1)
    put(p1, 1)
    for i, v in enumerate(idx):
        put(v, 3 if i == 0 else 4)
    assert n == 128
    return bits.to_bytes(16, "little")


def bcn(kind, f):
    out = b""
    for by in range(H // 4):
        for bx in range(W // 4):
            px = block(f, bx, by)
            if kind == 1:
                out += bc1_colour(px, True)
            elif kind == 2:
                out += bc2_alpha(px) + bc1_colour(px, False)
            elif kind == 3:
                out += bc3_alpha(px) + bc1_colour(px, False)
            else:
                out += bc7_mode6(px)
    return out


def dds(pf, data, dx10=None, pitch=0):
    flags = 0x1 | 0x2 | 0x4 | 0x1000 | (0x8 if pitch else 0x80000)
    hdr = struct.pack("<7I", 124, flags, H, W, pitch or len(data), 0, 1)
    hdr += b"\0" * 44 + pf + struct.pack("<5I", 0x1000, 0, 0, 0, 0)
    ext = struct.pack("<5I", dx10, 3, 0, 1, 0) if dx10 is not None else b""
    return b"DDS " + hdr + ext + data


def fourcc(cc):
    return struct.pack("<2I", 32, 0x4) + cc + b"\0" * 20


def ktx2(vk, data):
    d = b"\xabKTX 20\xbb\r\n\x1a\n" + struct.pack("<9I", vk, 1, W, H, 0, 0, 1, 1, 0)
    d += struct.pack("<4I", 0, 0, 0, 0) + struct.pack("<2Q", 0, 0)
    off = (len(d) + 24 + 15) // 16 * 16
    d += struct.pack("<3Q", off, len(data), len(data))
    return d + b"\0" * (off - len(d)) + data


def textures(out):
    save(out, "bc1.dds", dds(fourcc(b"DXT1"), bcn(1, px8_1bit)))
    save(out, "bc2.dds", dds(fourcc(b"DXT3"), bcn(2, px8)))
    save(out, "bc3.dds", dds(fourcc(b"DXT5"), bcn(3, px8)))
    save(out, "bc7.dds", dds(fourcc(b"DX10"), bcn(7, px8), dx10=98))
    bgra = b"".join(bytes((p[2], p[1], p[0], p[3])) for p in (px8(x, y) for y in range(H) for x in range(W)))
    save(out, "bgra8.dds", dds(struct.pack("<8I", 32, 0x41, 0, 32, 0xFF0000, 0xFF00, 0xFF, 0xFF000000),
                               bgra, pitch=W * 4))
    rgba = b"".join(bytes(px8(x, y)) for y in range(H) for x in range(W))
    save(out, "rgba8-dx10.dds", dds(fourcc(b"DX10"), rgba, dx10=28, pitch=W * 4))
    rgba16 = b"".join(struct.pack("<4H", *px16(x, y)) for y in range(H) for x in range(W))
    save(out, "rgba16-dx10.dds", dds(fourcc(b"DX10"), rgba16, dx10=11, pitch=W * 8))
    half = b"".join(struct.pack("<4e", *(v / 65535 for v in px16(x, y))) for y in range(H) for x in range(W))
    save(out, "rgba16f-dx10.dds", dds(fourcc(b"DX10"), half, dx10=10, pitch=W * 8))
    save(out, "rgba8.ktx2", ktx2(37, rgba))
    save(out, "bc7.ktx2", ktx2(145, bcn(7, px8)))


# --- Opaque files of formats that can carry alpha: `has_alpha` must be false ------------------

def opaque8(x, y):
    """The 8-bit reference with alpha ignored (the colour under transparent pixels shows)."""
    return px8(x, y)[:3] + (255,)


def distinct(f):
    """The image as palette indices, and the palette (every distinct value of `f`)."""
    pal, idx = [], []
    for y in range(H):
        row = bytearray()
        for x in range(W):
            p = f(x, y)
            if p not in pal:
                pal.append(p)
            row.append(pal.index(p))
        idx.append(bytes(row))
    assert len(pal) <= 256, len(pal)
    return idx, pal


def xpm(f, unused_none):
    """XPM of `f` (alpha 0 or 255), one character a pixel; `unused_none` adds a transparent
    colour no pixel uses."""
    chars = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789"
    idx, pal = distinct(f)
    lines = ["%d %d %d 1" % (W, H, len(pal) + unused_none)]
    for ch, p in zip(chars, pal):
        lines.append("%s c %s" % (ch, "#%02X%02X%02X" % p[:3] if p[3] else "None"))
    if unused_none:
        lines.append("%s c None" % chars[len(pal)])
    lines += ["".join(chars[i] for i in r) for r in idx]
    body = ",\n".join('"%s"' % l for l in lines)
    return ("/* XPM */\nstatic char *opaque[] = {\n%s\n};\n" % body).encode()


def farbfeld(f16):
    return b"farbfeld" + struct.pack(">II", W, H) + b"".join(
        struct.pack(">4H", *f16(x, y)) for y in range(H) for x in range(W))


def opaques(out):
    o8 = opaque8
    save(out, "opaque-argb32-bitfields.bmp",
         bmp_v5(32, (0xFF0000, 0xFF00, 0xFF, 0xFF000000), lambda x, y: argb(o8(x, y))))
    save(out, "opaque-bgra32.tga", tga(32, 8, lambda x, y: bytes(o8(x, y)[2::-1] + (255,))))
    save(out, "opaque-bgr24-mask.ico",
         icon(1, [(24, dib(24, lambda x, y: bytes(o8(x, y)[2::-1]), lambda x, y: False))]))
    save(out, "opaque-bc1.dds", dds(fourcc(b"DXT1"), bcn(1, o8)))
    idx, pal = distinct(o8)
    pal += [(0, 0, 0, 255)] * (256 - len(pal))
    cmap = [p[c] * 257 for c in range(3) for p in pal]
    save(out, "opaque-palette.tif", tiff(1, 8, 1, None, 3, b"".join(idx), cmap))
    save(out, "opaque.ff", farbfeld(lambda x, y: px16(x, y)[:3] + (65535,)))
    save(out, "opaque-unused-none.xpm", xpm(o8, True))


# --- Premultiplied DDS (DXT2, DXT4) and indexed Photoshop ----------------------------------

def psd_indexed(f, transparent):
    """8-bit indexed PSD, raw, no layer section. With `transparent`, pixels with alpha 0 use a
    palette entry named by resource 1047; otherwise the alpha is a second (merged) channel."""
    if transparent:
        # Transparent pixels keep their colour in their own palette entry: the last one.
        idx, pal = distinct(lambda x, y: f(x, y) if f(x, y)[3] else (0, 0, 0, 0))
    else:
        idx, pal = distinct(lambda x, y: f(x, y)[:3])
    pal = [p[:3] for p in pal] + [(0, 0, 0)] * (256 - len(pal))
    channels = 1 if transparent else 2
    d = b"8BPS" + struct.pack(">H6xHIIHH", 1, channels, H, W, 8, 2)
    d += struct.pack(">I", 768) + bytes(p[c] for c in range(3) for p in pal)
    res = b""
    if transparent:
        t = next(i for i, p in enumerate(pal) if p == (0, 0, 0))
        res += b"8BIM" + struct.pack(">HBxIH", 1047, 0, 2, t)
    d += struct.pack(">I", len(res)) + res
    d += struct.pack(">I", 0)  # no layer and mask section
    d += struct.pack(">H", 0) + b"".join(idx)
    if not transparent:
        d += bytes(f(x, y)[3] for y in range(H) for x in range(W))
    return d


def fixes(out):
    save(out, "dxt2.dds", dds(fourcc(b"DXT2"), bcn_premul(2)))
    save(out, "dxt4.dds", dds(fourcc(b"DXT4"), bcn_premul(3)))
    save(out, "index-trns.psd", psd_indexed(px8_1bit, True))
    save(out, "index-alpha.psd", psd_indexed(px8, False))


def save(out, name, data):
    with open(out + "/" + name, "wb") as f:
        f.write(data)


def main():
    out = sys.argv[1]
    pngs(out)
    tiffs(out)
    save(out, "half.exr", exr(1))
    save(out, "float.exr", exr(2))
    bmps(out)
    with open(out + "/rgba8.png", "rb") as f:
        icos(out, f.read())
    tgas(out)
    textures(out)
    opaques(out)
    fixes(out)


main()
