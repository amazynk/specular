"""Hand-built fuzz seeds for formats ImageMagick cannot write. Used by make-seeds.sh.

    python3 seeds.py ani <frame.ico> <out.ani>
"""
import io
import struct
import sys


def chunk(tag, body):
    pad = b"\0" if len(body) % 2 else b""
    return tag + struct.pack("<I", len(body)) + body + pad


def ani(ico_path, out):
    ico = open(ico_path, "rb").read()
    # anih: size, frames, steps, w, h, bpp, planes, rate (jiffies), flags (1 = ICO frames)
    anih = struct.pack("<9I", 36, 2, 3, 0, 0, 0, 0, 6, 1)
    fram = b"fram" + chunk(b"icon", ico) + chunk(b"icon", ico)
    body = b"ACON" + chunk(b"anih", anih) + chunk(b"rate", struct.pack("<3I", 3, 4, 5))
    body += chunk(b"seq ", struct.pack("<3I", 0, 1, 0)) + chunk(b"LIST", fram)
    open(out, "wb").write(b"RIFF" + struct.pack("<I", len(body)) + body)


def fli(kind, out):
    """An 8x4 FLI (`kind` = "fli") or FLC with a palette, BRUN, delta and COPY frames."""
    flc = kind == "flc"
    w, h = 8, 4

    def sub(t, body):
        return struct.pack("<IH", 6 + len(body), t) + body

    def frame(chunks):
        body = b"".join(chunks)
        return struct.pack("<IHHHHHH", 16 + len(body), 0xF1FA, len(chunks), 0, 0, 0, 0) + body

    pal = bytes((i * 37 + c * 91) & (255 if flc else 63) for i in range(16) for c in range(3))
    color = sub(4 if flc else 11, struct.pack("<HBB", 1, 0, 16) + pal)
    brun = b"".join(bytes([2, 3, y, 0xFB]) + bytes(range(5)) for y in range(h))
    if flc:  # SS2: 2 lines; line 1 skips 1 line, then one packet of 2 literal words
        delta = sub(7, struct.pack("<HHBb", 2, 1, 0, 1) + b"\x05\x06" + struct.pack("<hHBb", -1, 1, 2, -2) + b"\x07\x08")
    else:  # LC: skip 1 line, 1 line, one packet: skip 1, 3 literal bytes
        delta = sub(12, struct.pack("<HHBBb", 1, 1, 1, 1, 3) + b"\x09\x0a\x0b")
    frames = [frame([color, sub(15, brun)]), frame([delta]), frame([sub(13, b"")]),
              frame([sub(16, bytes(range(w * h)))])]
    body = b"".join(frames)
    hdr = bytearray(128)
    struct.pack_into("<IHHHHHH", hdr, 0, 128 + len(body), 0xAF12 if flc else 0xAF11, 3, w, h, 8, 3)
    if flc:
        struct.pack_into("<I", hdr, 16, 40)
        struct.pack_into("<I", hdr, 80, 128)
    else:
        struct.pack_into("<H", hdr, 16, 5)
    open(out, "wb").write(bytes(hdr) + body)


def radiance(out_dir):
    """PF4 (little-endian RGBA PFM), and RGBE/XYZE files with old-style RLE, a rotated
    resolution string and EXPOSURE/PRIMARIES/COLORCORR header lines."""
    w, h = 3, 2
    px = b"".join(struct.pack("<4f", x * 0.5, y * 0.25, 1.5, 0.75) for y in range(h) for x in range(w))
    open(out_dir + "/rgba.pfm", "wb").write(b"PF4\n%d %d\n-1.0\n" % (w, h) + px)
    gray = b"".join(struct.pack(">f", (x + y) * 0.125) for y in range(h) for x in range(w))
    open(out_dir + "/gray_be.pfm", "wb").write(b"Pf\n%d %d\n1.0\n" % (w, h) + gray)
    hdr = (b"#?RADIANCE\nFORMAT=32-bit_rle_rgbe\nEXPOSURE=0.5\nEXPOSURE=2\n"
           b"PRIMARIES=0.640 0.330 0.300 0.600 0.150 0.060 0.3127 0.3290\n"
           b"COLORCORR=1 1 1\nPIXASPECT=1\n\n")
    # Old-style RLE: a pixel followed by (1,1,1,n) repeats it n more times.
    line = bytes([10, 20, 30, 128, 1, 1, 1, 2])
    open(out_dir + "/oldrle.hdr", "wb").write(hdr + b"-Y 2 +X 3\n" + line * 2)
    xyze = b"#?RGBE\nFORMAT=32-bit_rle_xyze\n\n+X 3 -Y 2\n"
    open(out_dir + "/xyze.hdr", "wb").write(xyze + bytes([40, 50, 60, 129]) * 6)


def _blocks(n, seed):
    """`n` pseudo-random bytes (deterministic): stand-in BCn block data."""
    x, out = seed, bytearray()
    for _ in range(n):
        x = (x * 1103515245 + 12345) & 0x7FFFFFFF
        out.append(x >> 16 & 255)
    return bytes(out)


def _dds(w, h, pf, levels, dx10=None):
    hdr = struct.pack("<7I", 124, 0x1 | 0x2 | 0x4 | 0x1000 | 0x20000, h, w, 0, 0, len(levels))
    hdr += b"\0" * 44 + pf + struct.pack("<5I", 0x1000 | 0x400000, 0, 0, 0, 0)
    ext = struct.pack("<5I", dx10, 3, 0, 1, 0) if dx10 is not None else b""
    return b"DDS " + hdr + ext + b"".join(levels)


def _pf_fourcc(cc):
    return struct.pack("<2I", 32, 0x4) + cc + b"\0" * 20


def _pf_masks(bits, r, g, b, a, flags=0x40):
    return struct.pack("<8I", 32, flags | (0x1 if a else 0), 0, bits, r, g, b, a)


def _zstd_raw(data):
    """A Zstandard frame holding `data` (< 256 bytes) in one raw block."""
    return (b"\x28\xb5\x2f\xfd" + bytes([0x20, len(data)])
            + struct.pack("<I", (len(data) << 3) | 1)[:3] + data)


def _ktx2(vk, w, h, sc, levels, kv=b""):
    import zlib
    n = len(levels)
    packed = [{0: l, 2: _zstd_raw(l), 3: zlib.compress(l)}[sc] for l in levels]
    kvd = b""
    if kv:
        kvd = struct.pack("<I", len(kv)) + kv
        kvd += b"\0" * (-len(kvd) % 4)
    kvd_off = 80 + 24 * n
    d = b"\xabKTX 20\xbb\r\n\x1a\n" + struct.pack("<9I", vk, 1, w, h, 0, 0, 1, n, sc)
    d += struct.pack("<4I", 0, 0, kvd_off if kvd else 0, len(kvd)) + struct.pack("<2Q", 0, 0)
    off = (kvd_off + len(kvd) + 15) // 16 * 16
    index, data = b"", b""
    for p, l in zip(packed, levels):
        index += struct.pack("<3Q", off + len(data), len(p), len(l))
        data += p + b"\0" * (-len(p) % 16)
    head = d + index + kvd
    return head + b"\0" * (off - len(head)) + data


def texture(out_dir):
    """DDS files with DX10 headers (BC6H, BC7, half float, RGB9E5), FourCC BC4/BC5/DXT3 and
    bit masks (565, BGRA), and KTX2 files with no, Zstandard and zlib supercompression."""
    w, h = 8, 4

    def save(name, data):
        open(out_dir + "/" + name, "wb").write(data)

    bc16 = [_blocks(32, 1), _blocks(16, 2)]  # 8x4 then 4x2 (one block)
    save("bc7.dds", _dds(w, h, _pf_fourcc(b"DX10"), bc16, dx10=98))
    save("bc6h.dds", _dds(w, h, _pf_fourcc(b"DX10"), bc16, dx10=95))
    save("half.dds", _dds(2, 2, _pf_fourcc(b"DX10"), [_blocks(32, 3)], dx10=10))
    save("rgb9e5.dds", _dds(2, 2, _pf_fourcc(b"DX10"), [_blocks(16, 4)], dx10=67))
    save("bc4.dds", _dds(w, h, _pf_fourcc(b"ATI1"), [_blocks(16, 5)]))
    save("bc5.dds", _dds(w, h, _pf_fourcc(b"ATI2"), [_blocks(32, 6)]))
    save("dxt3.dds", _dds(w, h, _pf_fourcc(b"DXT3"), [_blocks(32, 7)]))
    save("r5g6b5.dds", _dds(3, 2, _pf_masks(16, 0xF800, 0x07E0, 0x001F, 0), [_blocks(12, 8)]))
    save("bgra.dds", _dds(3, 2, _pf_masks(32, 0xFF0000, 0xFF00, 0xFF, 0xFF000000), [_blocks(24, 9)]))
    save("bc7.ktx2", _ktx2(145, w, h, 0, bc16, kv=b"KTXorientation\0ru\0"))
    save("bc7_zstd.ktx2", _ktx2(145, w, h, 2, bc16))
    save("rgba8_zlib.ktx2", _ktx2(37, 3, 2, 3, [_blocks(24, 10), _blocks(4, 11)]))


def _png_chunk(tag, body):
    import zlib
    return struct.pack(">I", len(body)) + tag + body + struct.pack(">I", zlib.crc32(tag + body))


def png(out_dir):
    """PNGs ImageMagick does not write: an APNG with an offset, blended sub-frame and a
    fdAT-only first frame variant, all five filter types, cICP/mDCv/cLLi/eXIf chunks, and an
    Apple iDOT file with two independently compressed halves."""
    import zlib
    w, h = 6, 4
    sig = b"\x89PNG\r\n\x1a\n"
    ihdr = lambda w, h, ct=6, i=0: _png_chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, ct, 0, 0, i))
    rows = lambda w, h, f=0: b"".join(bytes([(y + f) % 5]) + _blocks(w * 4, y + 1) for y in range(h))
    idat = lambda raw: _png_chunk(b"IDAT", zlib.compress(raw))
    iend = _png_chunk(b"IEND", b"")

    def fctl(seq, fw, fh, x, y, dispose, blend):
        return _png_chunk(b"fcTL", struct.pack(">IIIIIHHBB", seq, fw, fh, x, y, 1, 10, dispose, blend))

    anim = sig + ihdr(w, h) + _png_chunk(b"acTL", struct.pack(">II", 3, 0))
    anim += fctl(0, w, h, 0, 0, 1, 0) + idat(rows(w, h))
    anim += fctl(1, 3, 2, 2, 1, 2, 1) + _png_chunk(b"fdAT", struct.pack(">I", 2) + zlib.compress(rows(3, 2, 1)))
    anim += fctl(3, 2, 2, 4, 2, 0, 1) + _png_chunk(b"fdAT", struct.pack(">I", 4) + zlib.compress(rows(2, 2, 3)))
    open(out_dir + "/apng_blend.png", "wb").write(anim + iend)

    # Default image not part of the animation: IDAT before the first fcTL.
    hidden = sig + ihdr(w, h) + _png_chunk(b"acTL", struct.pack(">II", 1, 1)) + idat(rows(w, h))
    hidden += fctl(0, w, h, 0, 0, 0, 0) + _png_chunk(b"fdAT", struct.pack(">I", 1) + zlib.compress(rows(w, h, 2)))
    open(out_dir + "/apng_hidden_default.png", "wb").write(hidden + iend)

    meta = sig + ihdr(w, h) + _png_chunk(b"cICP", bytes([9, 16, 0, 1]))
    meta += _png_chunk(b"mDCv", struct.pack(">8H2I", 35400, 14600, 8500, 39850, 6550, 2300, 15635, 16450, 10000000, 1))
    meta += _png_chunk(b"cLLi", struct.pack(">2I", 10000000, 4000000))
    meta += _png_chunk(b"gAMA", struct.pack(">I", 45455)) + _png_chunk(b"pHYs", struct.pack(">IIB", 2835, 2835, 1))
    meta += _png_chunk(b"eXIf", b"MM\0*\0\0\0\x08\0\x01\x01\x12\0\x03\0\0\0\x01\0\x06\0\0\0\0\0\0")
    open(out_dir + "/meta_filters.png", "wb").write(meta + idat(rows(w, h)) + iend)

    # iDOT: one zlib stream fully flushed at the half-way row; each half sits in its own IDAT,
    # located by the chunk's offsets (relative to the iDOT chunk start).
    h2 = 8
    z = zlib.compressobj()
    top = z.compress(rows(w, h2 // 2)) + z.flush(zlib.Z_FULL_FLUSH)
    bottom = z.compress(rows(w, h2 // 2, 1)) + z.flush()
    idot_len = 12 + 4 + 2 * 12
    off1, off2 = idot_len, idot_len + 12 + len(top)
    body = struct.pack(">I6I", 2, 0, h2 // 2, off1, h2 // 2, h2 // 2, off2)
    data = sig + ihdr(w, h2) + _png_chunk(b"iDOT", body) + _png_chunk(b"IDAT", top) + _png_chunk(b"IDAT", bottom)
    open(out_dir + "/idot.png", "wb").write(data + iend)


def tiff(out_dir):
    """A reduced-resolution pyramid ImageMagick will not write at seed sizes: an 8x6 RGB page
    with a 4x3 level in the IFD chain (NewSubfileType 1) and a 2x2 level as a SubIFD."""
    out = bytearray(b"II*\0\0\0\0\0")

    def ifd(w, h, subfile, sub_ifds=()):
        strip = len(out)
        out.extend(_blocks(w * h * 3, w))
        tags = [(254, 4, [subfile]), (256, 3, [w]), (257, 3, [h]), (258, 3, [8, 8, 8]), (259, 3, [1]),
                (262, 3, [2]), (273, 4, [strip]), (277, 3, [3]), (278, 3, [h]), (279, 4, [w * h * 3])]
        if sub_ifds:
            tags.append((330, 13, list(sub_ifds)))
        if len(out) % 2:
            out.append(0)
        extra = len(out)
        for _, typ, vals in tags:  # out-of-line values
            size = 2 if typ == 3 else 4
            if len(vals) * size > 4:
                out.extend(b"".join(v.to_bytes(size, "little") for v in vals))
        at = len(out)
        out.extend(struct.pack("<H", len(tags)))
        for tag, typ, vals in tags:
            size = 2 if typ == 3 else 4
            raw = b"".join(v.to_bytes(size, "little") for v in vals)
            if len(raw) > 4:
                val, extra = struct.pack("<I", extra), extra + len(raw)
            else:
                val = raw.ljust(4, b"\0")
            out.extend(struct.pack("<HHI", tag, typ, len(vals)) + val)
        next_at = len(out)
        out.extend(b"\0\0\0\0")
        return at, next_at

    sub, _ = ifd(2, 2, 1)
    level, _ = ifd(4, 3, 1)
    base, base_next = ifd(8, 6, 0, [sub])
    struct.pack_into("<I", out, 4, base)
    struct.pack_into("<I", out, base_next, level)
    open(out_dir + "/pyramid_sub.tif", "wb").write(out)


def _psd(w, h, depth, mode, planes, comp=0, color_data=b"", resources=b""):
    import zlib
    out = b"8BPS" + struct.pack(">H6xHIIHH", 1, len(planes), h, w, depth, mode)
    out += struct.pack(">I", len(color_data)) + color_data
    out += struct.pack(">I", len(resources)) + resources + struct.pack(">I", 0)
    rb = (w + 7) // 8 if depth == 1 else w * depth // 8
    if comp == 0:
        return out + struct.pack(">H", 0) + b"".join(planes)
    body = b""
    for p in planes:  # comp 3: ZIP with per-row delta prediction, one stream per channel
        rows = []
        for y in range(h):
            r = bytearray(p[y * rb:(y + 1) * rb])
            if depth == 32:
                n = w
                r = bytearray(r[x * 4 + b] for b in range(4) for x in range(n))
                r = bytearray([r[0]] + [(r[i] - r[i - 1]) & 255 for i in range(1, len(r))])
            elif depth == 16:
                v = [int.from_bytes(r[i:i + 2], "big") for i in range(0, len(r), 2)]
                r = b"".join(((v[i] - (v[i - 1] if i else 0)) & 0xFFFF).to_bytes(2, "big") for i in range(len(v)))
            else:
                r = bytearray([r[0]] + [(r[i] - r[i - 1]) & 255 for i in range(1, len(r))])
            rows.append(bytes(r))
        body += zlib.compress(b"".join(rows))
    return out + struct.pack(">H", comp) + body


def _psd_res(rid, data):
    pad = data + (b"\0" if len(data) % 2 else b"")
    return b"8BIM" + struct.pack(">HH", rid, 0) + struct.pack(">I", len(data)) + pad


def psd(out_dir, thumb_jpeg=None):
    """PSD variants ImageMagick does not write: 1-bit Bitmap, Lab, Multichannel, Duotone,
    32-bit float and 16-bit with ZIP prediction, Indexed with a transparent index, and a JFIF
    thumbnail resource."""
    w, h = 9, 4
    plane = lambda c, bps=1: _blocks(w * h * bps, c + 3)
    open(out_dir + "/bitmap1.psd", "wb").write(_psd(w, h, 1, 0, [_blocks(2 * h, 5)]))
    open(out_dir + "/lab8.psd", "wb").write(_psd(w, h, 8, 9, [plane(c) for c in range(3)]))
    open(out_dir + "/multichannel.psd", "wb").write(_psd(w, h, 8, 7, [plane(c) for c in range(2)]))
    open(out_dir + "/duotone.psd", "wb").write(_psd(w, h, 8, 8, [plane(0)], color_data=bytes(range(32))))
    floats = lambda c: b"".join(struct.pack(">f", ((i * 7 + c * 3) % 17) / 8.0) for i in range(w * h))
    open(out_dir + "/rgb32_zippred.psd", "wb").write(_psd(w, h, 32, 3, [floats(c) for c in range(4)], comp=3))
    open(out_dir + "/rgb16_zippred.psd", "wb").write(_psd(w, h, 16, 3, [plane(c, 2) for c in range(3)], comp=3))
    pal = bytes((i * 37) & 255 for i in range(768))
    res = _psd_res(1047, struct.pack(">H", 2))
    if thumb_jpeg:
        jpg = open(thumb_jpeg, "rb").read()
        res += _psd_res(1036, struct.pack(">IIIIIIHH", 1, 4, 3, 12, 36, len(jpg), 24, 1) + jpg)
    open(out_dir + "/indexed_thumb.psd", "wb").write(_psd(w, h, 8, 2, [plane(0)], color_data=pal, resources=res))


def _exr_attr(name, typ, value):
    return name.encode() + b"\0" + typ.encode() + b"\0" + struct.pack("<i", len(value)) + value


def _exr_header(w, h, chans, extra=b"", ptype=1):
    """Attributes of an uncompressed part; `chans` are channel names (sorted on disk)."""
    cl = b"".join(c.encode() + b"\0" + struct.pack("<iB3xii", ptype, 0, 1, 1) for c in sorted(chans)) + b"\0"
    box = struct.pack("<4i", 0, 0, w - 1, h - 1)
    return (_exr_attr("channels", "chlist", cl) + _exr_attr("compression", "compression", b"\0")
            + _exr_attr("dataWindow", "box2i", box) + _exr_attr("displayWindow", "box2i", box)
            + _exr_attr("lineOrder", "lineOrder", b"\0") + _exr_attr("pixelAspectRatio", "float", struct.pack("<f", 1))
            + _exr_attr("screenWindowCenter", "v2f", struct.pack("<2f", 0, 0))
            + _exr_attr("screenWindowWidth", "float", struct.pack("<f", 1)) + extra)


def _half(i, c):
    return struct.pack("<e", ((i * 5 + c * 3) % 11) / 4.0)


def exr(out_dir):
    """EXR layouts ImageMagick does not write: a tiled mipmapped file (4x4 tiles, round-down
    levels) and a two-part file whose second part has a named layer. Uncompressed, half."""
    import math
    # Tiled mipmap: 8x6 -> 4x3 -> 2x1 -> 1x1.
    w, h, t = 8, 6, 4
    chans = ["A", "B", "G", "R"]
    hdr = _exr_header(w, h, chans, _exr_attr("tiles", "tiledesc", struct.pack("<IIB", t, t, 1)))
    levels, lw, lh = [], w, h
    while True:
        levels.append((lw, lh))
        if lw == 1 and lh == 1:
            break
        lw, lh = max(lw // 2, 1), max(lh // 2, 1)
    chunks = []
    for li, (lw, lh) in enumerate(levels):
        for ty in range(math.ceil(lh / t)):
            for tx in range(math.ceil(lw / t)):
                tw, th = min(t, lw - tx * t), min(t, lh - ty * t)
                data = b"".join(_half(y * tw + x + li, c) for y in range(th) for c in range(4) for x in range(tw))
                chunks.append(struct.pack("<5i", tx, ty, li, li, len(data)) + data)
    head = b"\x76\x2f\x31\x01" + struct.pack("<I", 2 | 0x200) + hdr + b"\0"
    pos = len(head) + 8 * len(chunks)
    table = b""
    for c in chunks:
        table += struct.pack("<Q", pos)
        pos += len(c)
    open(out_dir + "/tiled_mip.exr", "wb").write(head + table + b"".join(chunks))

    # Multipart: part 0 "beauty" RGB scanlines, part 1 "aov" with diffuse.R/G/B + Z (float).
    w, h = 5, 3
    parts = [("beauty", ["B", "G", "R"], 1), ("aov", ["Z", "diffuse.B", "diffuse.G", "diffuse.R"], 2)]
    heads, bodies = b"", []
    for name, chans, ptype in parts:
        extra = (_exr_attr("name", "string", name.encode()) + _exr_attr("type", "string", b"scanlineimage")
                 + _exr_attr("chunkCount", "int", struct.pack("<i", h)))
        heads += _exr_header(w, h, chans, extra, ptype) + b"\0"
        size = 2 if ptype == 1 else 4
        lines = []
        for y in range(h):
            if ptype == 1:
                data = b"".join(_half(y * w + x, c) for c in range(len(chans)) for x in range(w))
            else:
                data = b"".join(struct.pack("<f", (y * w + x + c) / 7.0) for c in range(len(chans)) for x in range(w))
            assert len(data) == size * w * len(chans)
            lines.append((y, data))
        bodies.append(lines)
    head = b"\x76\x2f\x31\x01" + struct.pack("<I", 2 | 0x1000) + heads + b"\0"
    pos = len(head) + 8 * h * len(parts)
    tables, chunks = [], []
    for pi, lines in enumerate(bodies):
        for y, data in lines:
            c = struct.pack("<Iii", pi, y, len(data)) + data
            tables.append(struct.pack("<Q", pos))
            chunks.append(c)
            pos += len(c)
    open(out_dir + "/multipart.exr", "wb").write(head + b"".join(tables) + b"".join(chunks))

    # 32-bit float RGB scanlines, ZIP (16 lines per chunk): byte de-interleave + delta, zlib.
    import zlib
    w, h = 6, 20
    comp = _exr_attr("compression", "compression", b"\3")
    hdr = _exr_header(w, h, ["B", "G", "R"], ptype=2).replace(_exr_attr("compression", "compression", b"\0"), comp)
    chunks = []
    for y0 in range(0, h, 16):
        ys = range(y0, min(y0 + 16, h))
        raw = b"".join(struct.pack("<f", ((y * w + x) * (c + 2) % 13) / 3.0) for y in ys for c in range(3) for x in range(w))
        t = raw[0::2] + raw[1::2]
        t = bytes([t[0]] + [(t[i] - t[i - 1] + 128) & 255 for i in range(1, len(t))])
        z = zlib.compress(t)
        chunks.append(struct.pack("<ii", y0, len(z)) + z)
    head = b"\x76\x2f\x31\x01" + struct.pack("<I", 2) + hdr + b"\0"
    pos, table = len(head) + 8 * len(chunks), b""
    for c in chunks:
        table += struct.pack("<Q", pos)
        pos += len(c)
    open(out_dir + "/float_zip.exr", "wb").write(head + table + b"".join(chunks))

_COMIC = (b'<?xml version="1.0"?>\n<ComicInfo><Manga>YesAndRightToLeft</Manga><Pages>'
          b'<Page Image="0" Type="FrontCover"/><Page Image="1" DoublePage="true" ImageWidth="2"/>'
          b'</Pages></ComicInfo>\n')


def _icns_rle(plane):
    """ICNS PackBits-like runs: 0x80 + n - 3 repeats the next byte n times, else n + 1 literals."""
    out, i = bytearray(), 0
    while i < len(plane):
        run = 1
        while i + run < len(plane) and run < 130 and plane[i + run] == plane[i]:
            run += 1
        if run >= 3:
            out += bytes([0x80 + run - 3, plane[i]])
            i += run
        else:
            j = i
            while j < len(plane) and j - i < 128 and not (j + 2 < len(plane) and plane[j] == plane[j + 1] == plane[j + 2]):
                j += 1
            out += bytes([j - i - 1]) + bytes(plane[i:j])
            i = j
    return bytes(out)


def icns(out_dir, page):
    """ICNS files: legacy RLE (is32 + s8mk, it32 with its zero prefix), ARGB (ic04), 1-bit
    ICN# with mask, and a PNG payload (ic07) holding `page`."""
    import os

    def entry(cc, data):
        return cc + struct.pack(">I", len(data) + 8) + data

    def file(*entries):
        body = b"".join(entries)
        return b"icns" + struct.pack(">I", len(body) + 8) + body

    def planes(n):
        return [bytes((i * k) & 255 for i in range(n * n)) for k in (3, 5, 7, 11)]

    r, g, b, a = planes(16)
    rgb16 = _icns_rle(r) + _icns_rle(g) + _icns_rle(b)
    # it32 is 128x128: smooth planes so the seed stays small.
    r, g, b = (bytes((i // (128 * k)) & 255 for i in range(128 * 128)) for k in (3, 5, 7))
    files = {
        "is32.icns": file(entry(b"is32", rgb16), entry(b"s8mk", a)),
        "it32.icns": file(entry(b"it32", bytes(4) + _icns_rle(r) + _icns_rle(g) + _icns_rle(b))),
        "argb.icns": file(entry(b"ic04", b"ARGB" + b"".join(_icns_rle(x) for x in planes(16)))),
        "mono.icns": file(entry(b"ICN#", bytes(range(128)) + b"\xff" * 128)),
        "png.icns": file(entry(b"icnV", b"\0\0\0\1"), entry(b"ic07", open(page, "rb").read()),
                         entry(b"is32", rgb16)),
    }
    for name, data in files.items():
        open(os.path.join(out_dir, name), "wb").write(data)


def ora(out_dir, page1, page2):
    """OpenRaster and Krita documents: merged image (stored and deflated), an ORA flattened from
    offset, translucent and grouped layers, a Krita preview-only file and a late mimetype."""
    import os
    import zipfile
    p1, p2 = open(page1, "rb").read(), open(page2, "rb").read()
    stack = (b'<image w="4" h="3"><stack><layer src="data/a.png" x="1" y="-1" opacity="0.5"/>'
             b'<stack opacity="0.7" x="1"><layer src="data/b.png"/><layer src="data/c.png" visibility="hidden"'
             b' composite-op="svg:multiply"/></stack><layer src="data/b.png"/></stack></image>')
    maindoc = b'<?xml version="1.0"?><DOC editor="Krita"><IMAGE width="2" height="2" mime="application/x-kra"/></DOC>'

    def doc(name, members, deflate=True):
        with zipfile.ZipFile(os.path.join(out_dir, name), "w") as z:
            for n, d in members:
                m = zipfile.ZIP_STORED if n == "mimetype" or not deflate else zipfile.ZIP_DEFLATED
                z.writestr(zipfile.ZipInfo(n, (2020, 1, 2, 3, 4, 6)), d, compress_type=m)
    ora_mime = ("mimetype", b"image/openraster")
    kra_mime = ("mimetype", b"application/x-krita")
    doc("merged.ora", [ora_mime, ("stack.xml", stack), ("mergedimage.png", p1), ("Thumbnails/thumbnail.png", p2)])
    doc("stored.ora", [ora_mime, ("stack.xml", stack), ("mergedimage.png", p1)], deflate=False)
    doc("layers.ora", [ora_mime, ("stack.xml", stack), ("data/a.png", p1), ("data/b.png", p2)])
    doc("merged.kra", [kra_mime, ("maindoc.xml", maindoc), ("mergedimage.png", p1), ("preview.png", p2)])
    doc("preview.kra", [kra_mime, ("maindoc.xml", maindoc), ("preview.png", p2)])
    doc("late.kra", [("maindoc.xml", maindoc), ("mimetype", b"application/x-krita"), ("mergedimage.png", p1)])


def archive(out_dir, page1, page2):
    """ZIP and TAR variants holding two tiny pages, ComicInfo.xml and a nested archive name."""
    import tarfile
    import zipfile
    tarfile.RECORDSIZE = 1024  # end blocks only, not 10 KiB of padding
    import os
    os.makedirs(out_dir + "/comicinfo", exist_ok=True)
    open(out_dir + "/comicinfo/ComicInfo.xml", "wb").write(_COMIC)
    p1, p2 = open(page1, "rb").read(), open(page2, "rb").read()
    files = [("ComicInfo.xml", _COMIC), ("01.png", p1), ("sub/02.png", p2), ("__MACOSX/._01.png", b"x"),
             ("inner.cbz", b"PK\5\6" + bytes(18))]

    def zip_(name, method, **kw):
        with zipfile.ZipFile(out_dir + "/" + name, "w", method, **kw) as z:
            for n, d in files:
                z.writestr(zipfile.ZipInfo(n, (2020, 1, 2, 3, 4, 6)), d, compress_type=method)
    zip_("stored.cbz", zipfile.ZIP_STORED)
    zip_("deflate.zip", zipfile.ZIP_DEFLATED)
    with zipfile.ZipFile(out_dir + "/zip64.zip", "w", zipfile.ZIP_STORED, allowZip64=True) as z:
        with z.open(zipfile.ZipInfo("01.png", (2020, 1, 2, 3, 4, 6)), "w", force_zip64=True) as f:
            f.write(p1)
    # Self-extractor style: a prefix before the first local header.
    open(out_dir + "/sfx.zip", "wb").write(b"MZ" + bytes(30) + open(out_dir + "/stored.cbz", "rb").read())
    for name, fmt in (("ustar.cbt", tarfile.USTAR_FORMAT), ("gnu.tar", tarfile.GNU_FORMAT), ("pax.tar", tarfile.PAX_FORMAT)):
        with tarfile.open(out_dir + "/" + name, "w", format=fmt) as t:
            extra = [("d" * 120 + "/long.png", p2)] if fmt != tarfile.USTAR_FORMAT else []
            for n, d in files + extra:
                i = tarfile.TarInfo(n)
                i.size, i.mtime, i.mode = len(d), 1577934246, 0o644
                t.addfile(i, io.BytesIO(d))


def metadata(out_dir, icc, *jpegs):
    """Payload-target seeds `[flags][exif len][xmp len][exif][xmp][icc]` from the metadata
    crate's synthetic test JPEGs, plus one with the given ICC profile."""
    import os
    os.makedirs(out_dir, exist_ok=True)
    icc_data = open(icc, "rb").read()
    for path in jpegs:
        b = open(path, "rb").read()
        exif = xmp = b""
        icc_parts = []
        i = 2
        while i + 4 <= len(b) and b[i] == 0xFF and b[i + 1] not in (0xDA, 0xD9):
            n = struct.unpack(">H", b[i + 2:i + 4])[0]
            seg = b[i + 4:i + 2 + n]
            if b[i + 1] == 0xE1 and seg.startswith(b"Exif\0\0"):
                exif = seg[6:]
            elif b[i + 1] == 0xE1 and seg.startswith(b"http://ns.adobe.com/xap/1.0/\0"):
                xmp = seg[29:]
            elif b[i + 1] == 0xE2 and seg.startswith(b"ICC_PROFILE\0"):
                icc_parts.append(seg[14:])
            i += 2 + n
        flags = (1 if exif else 0) | (2 if xmp else 0) | (4 if icc_parts else 0)
        name = os.path.splitext(os.path.basename(path))[0]
        body = struct.pack("<BHH", flags, len(exif), len(xmp)) + exif + xmp + b"".join(icc_parts)
        open(f"{out_dir}/{name}.bin", "wb").write(body)
    open(out_dir + "/icc_only.bin", "wb").write(struct.pack("<BHH", 4 | 0x10, 0, 0) + icc_data)


# --- ICC profiles (specular-color) ---

D50 = (0.9642, 1.0, 0.8249)


def _s15(v):
    return struct.pack(">i", round(v * 65536))


def _xyz(v):
    return b"XYZ \0\0\0\0" + b"".join(_s15(c) for c in v)


def _curv(entries):
    return b"curv\0\0\0\0" + struct.pack(">I", len(entries)) + b"".join(struct.pack(">H", e) for e in entries)


def _para(kind, params):
    return b"para\0\0\0\0" + struct.pack(">HH", kind, 0) + b"".join(_s15(v) for v in params)


SRGB_PARA = (2.4, 1 / 1.055, 0.055 / 1.055, 1 / 12.92, 0.04045)


def _icc(version, cls, space, pcs, tags):
    """A profile from (signature, body) tags; equal bodies are stored once and shared."""
    n = len(tags)
    off = 128 + 4 + 12 * n
    table, data, seen = b"", b"", {}
    for sig, body in tags:
        if body not in seen:
            seen[body] = off + len(data)
            data += body + b"\0" * (-len(body) % 4)
        table += sig + struct.pack(">II", seen[body], len(body))
    size = off + len(data)
    hdr = struct.pack(">I", size) + b"spec" + struct.pack(">I", version) + cls + space + pcs
    hdr += struct.pack(">6H", 2026, 1, 2, 3, 4, 5) + b"acsp" + b"\0" * 4 + struct.pack(">I", 0)
    hdr += b"\0" * 16 + struct.pack(">I", 0) + b"".join(_s15(c) for c in D50) + b"spec"
    hdr += b"\0" * (128 - len(hdr))
    return hdr + struct.pack(">I", n) + table + data


def _grid(inputs, g):
    """Grid nodes in ICC order (first input slowest), values in [0, 1]."""
    import itertools
    return [tuple(i / (g - 1) for i in node) for node in itertools.product(range(g), repeat=inputs)]


def _cmyk_lab(c, m, y, k):
    """A rough ink → normalized L*a*b* model (L 0..1, a/b 0.5 = neutral)."""
    l = (1 - 0.8 * max(c, m, y) * 0.5 - 0.3 * (c + m + y) / 3) * (1 - 0.85 * k)
    return (max(0.0, l), 0.5 + 0.25 * (m - c * 0.5 - y * 0.3), 0.5 + 0.25 * (y - c * 0.6 - m * 0.2))


def _rgb_lab(r, g, b):
    l = 0.3 * r + 0.59 * g + 0.11 * b
    return (l, 0.5 + 0.3 * (r - g), 0.5 + 0.3 * (g - b) * 0.5)


def _lab_cmyk(l, a, b):
    k = max(0.0, 0.6 - l) * 0.8
    return (max(0.0, 1 - l - (a - 0.5)), max(0.0, 1 - l + (a - 0.5)), max(0.0, 1 - l + (b - 0.5)), k)


def _mft(wide, inputs, outputs, g, f, matrix=(1, 0, 0, 0, 1, 0, 0, 0, 1), tab=2):
    """lut16Type (`wide`, `tab`-entry curves) or lut8Type with identity curves."""
    q = (lambda v: struct.pack(">H", max(0, min(65535, round(v * 65535))))) if wide else \
        (lambda v: struct.pack(">B", max(0, min(255, round(v * 255)))))
    n = tab if wide else 256
    body = (b"mft2" if wide else b"mft1") + b"\0" * 4 + struct.pack(">BBBB", inputs, outputs, g, 0)
    body += b"".join(_s15(v) for v in matrix)
    if wide:
        body += struct.pack(">HH", n, n)
    curve = b"".join(q(i / (n - 1)) for i in range(n))
    body += curve * inputs
    body += b"".join(q(v) for node in _grid(inputs, g) for v in f(*node))
    return body + curve * outputs


def _mab(a2b, inputs, outputs, b=None, matrix=None, m=None, clut=None, a=None):
    """lutAtoBType / lutBtoAType from element bodies (curve lists, a 12-value matrix and a
    (grid, precision, f) CLUT); missing elements get offset 0."""
    body = bytearray((b"mAB " if a2b else b"mBA ") + b"\0" * 4 + struct.pack(">BBH", inputs, outputs, 0) + b"\0" * 20)
    offs = []
    for el in (b, matrix, m, clut, a):
        if el is None:
            offs.append(0)
            continue
        body += b"\0" * (-len(body) % 4)
        offs.append(len(body))
        if el is matrix:
            body += b"".join(_s15(v) for v in el)
        elif el is clut:
            g, prec, f = el
            body += bytes(g for _ in range(inputs)) + bytes(16 - inputs) + struct.pack(">B3x", prec)
            pk = (lambda v: struct.pack(">B", max(0, min(255, round(v * 255))))) if prec == 1 else \
                (lambda v: struct.pack(">H", max(0, min(65535, round(v * 65535)))))
            body += b"".join(pk(v) for node in _grid(inputs, g) for v in f(*node))
        else:
            for c in el:
                body += c + b"\0" * (-len(c) % 4)
    body[12:32] = struct.pack(">5I", *offs)
    return bytes(body)


def icc(out_dir):
    """Small synthetic ICC profiles: v2 and v4 matrix/TRC RGB (gamma, table and parametric
    curves), v2/v4 gray, lut8/lut16 and lutAtoB/lutBtoA (RGB and CMYK, Lab and XYZ PCS)."""
    import os
    os.makedirs(out_dir, exist_ok=True)
    v2, v4 = 0x02100000, 0x04300000
    # sRGB colorants adapted to D50.
    cols = [(0.4361, 0.2225, 0.0139), (0.3851, 0.7169, 0.0971), (0.1431, 0.0606, 0.7141)]
    wtpt = _xyz(D50)
    mats = [(b"rXYZ", _xyz(cols[0])), (b"gXYZ", _xyz(cols[1])), (b"bXYZ", _xyz(cols[2]))]

    def rgb(trc):
        return [(b"wtpt", wtpt)] + mats + [(s, trc) for s in (b"rTRC", b"gTRC", b"bTRC")]

    profiles = {
        "rgb_v2_gamma.icc": _icc(v2, b"mntr", b"RGB ", b"XYZ ", rgb(_curv([563]))),
        "rgb_v2_table.icc": _icc(v2, b"mntr", b"RGB ", b"XYZ ", rgb(_curv([round(65535 * (i / 15) ** 2.2) for i in range(16)]))),
        "rgb_v4_para.icc": _icc(v4, b"mntr", b"RGB ", b"XYZ ", rgb(_para(3, SRGB_PARA))),
        "gray_v2.icc": _icc(v2, b"mntr", b"GRAY", b"XYZ ", [(b"wtpt", wtpt), (b"kTRC", _curv([461]))]),
        "gray_v4_para.icc": _icc(v4, b"mntr", b"GRAY", b"Lab ", [(b"wtpt", wtpt), (b"kTRC", _para(4, SRGB_PARA + (0.01, 0.0)))]),
        "rgb_v2_lut8_lab.icc": _icc(v2, b"scnr", b"RGB ", b"Lab ", [
            (b"wtpt", wtpt), (b"A2B0", _mft(False, 3, 3, 2, _rgb_lab))]),
        "rgb_v2_lut16_xyz.icc": _icc(v2, b"mntr", b"RGB ", b"XYZ ", [
            (b"wtpt", wtpt),
            (b"A2B0", _mft(True, 3, 3, 3, lambda r, g, b: tuple(v * 0.5 for v in (0.4 * r + 0.4 * g + 0.2 * b, 0.2 * r + 0.7 * g + 0.1 * b, 0.8 * b + 0.05)), tab=4)),
            (b"B2A0", _mft(True, 3, 3, 2, lambda x, y, z: (x, y, z), matrix=(1.1, 0, 0, 0, 1, 0, 0, 0, 0.9)))]),
        "cmyk_v2_lut16.icc": _icc(v2, b"prtr", b"CMYK", b"Lab ", [
            (b"wtpt", wtpt), (b"A2B0", _mft(True, 4, 3, 2, _cmyk_lab, tab=3)),
            (b"B2A0", _mft(True, 3, 4, 2, _lab_cmyk))]),
    }
    ident = _para(0, (1.0,))
    profiles["cmyk_v4_mab.icc"] = _icc(v4, b"prtr", b"CMYK", b"Lab ", [
        (b"wtpt", wtpt),
        (b"A2B0", _mab(True, 4, 3, b=[ident] * 3, clut=(3, 1, _cmyk_lab), a=[_curv([]), _para(0, (1.2,)), _curv([0, 30000, 65535]), ident])),
        (b"A2B1", _mab(True, 4, 3, b=[ident] * 3, clut=(2, 2, _cmyk_lab), a=[ident] * 4)),
        (b"B2A0", _mab(False, 3, 4, b=[ident] * 3, clut=(2, 2, _lab_cmyk), a=[_para(1, (1.0, 1.0, 0.0))] * 4))])
    profiles["rgb_v4_mab_matrix.icc"] = _icc(v4, b"mntr", b"RGB ", b"XYZ ", [
        (b"wtpt", wtpt),
        (b"A2B0", _mab(True, 3, 3, b=[ident] * 3, matrix=[v * 0.5 for c in zip(*cols) for v in c] + [0.0, 0.0, 0.0],
                       m=[_para(3, SRGB_PARA)] * 3)),
        (b"B2A0", _mab(False, 3, 3, b=[ident] * 3, clut=(2, 2, lambda x, y, z: (x, y, z)), a=[_para(2, (0.4545, 1.0, 0.0, 0.0))] * 3))])
    for name, data in profiles.items():
        open(os.path.join(out_dir, name), "wb").write(data)


# --- XMP and ICC in headers (specular-decode's locate target) ---

_XMP = (b'<?xpacket begin="" id="W5M0MpCehiHzreSzNTczkc9d"?><x:xmpmeta xmlns:x="adobe:ns:meta/">'
        b'<rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"><rdf:Description '
        b'xmlns:xmp="http://ns.adobe.com/xap/1.0/" xmp:Rating="3"/></rdf:RDF></x:xmpmeta>'
        b'<?xpacket end="w"?>')


def _tiny_icc():
    text = b"Seed RGB\0"
    desc = b"desc\0\0\0\0" + struct.pack(">I", len(text)) + text + bytes(8 + 3 + 67)
    head = bytearray(128)
    head[8] = 2
    head[12:24] = b"mntrRGB XYZ "
    head[36:40] = b"acsp"
    p = bytes(head) + struct.pack(">I4sII", 1, b"desc", 144, len(desc)) + desc
    return struct.pack(">I", len(p)) + p[4:]


def _bx(kind, body):
    return struct.pack(">I", len(body) + 8) + kind + body


def _heif(brand, coded, xmp, icc, in_idat, encoding=None, exif=None):
    """Primary item 1 (`coded`) with a `colr` `prof` property and an XMP `mime` item 2, in
    `mdat` (iloc v1, method 0) or `idat` (method 1); ipma with 16-bit indices for HEIC. With
    `encoding`, item 2's `infe` names it as its `content_encoding` and `xmp` is stored as given.
    With `exif` = (data, encoding), an `application/exif` `mime` item 3 follows it."""
    def infe(i, kind, extra=b""):
        return _bx(b"infe", bytes([2, 0, 0, 0, 0, i, 0, 0]) + kind + b"\0" + extra)
    mime = b"application/rdf+xml\0" + (encoding + b"\0" if encoding is not None else b"")
    items = [(2, xmp)]
    iinf = infe(1, coded) + infe(2, b"mime", mime)
    cdsc = _bx(b"cdsc", b"\0\2\0\1\0\1")
    if exif is not None:
        items.append((3, exif[0]))
        iinf += infe(3, b"mime", b"application/exif\0" + exif[1] + b"\0")
        cdsc += _bx(b"cdsc", b"\0\3\0\1\0\1")
    iinf = bytes(4) + struct.pack(">H", 1 + len(items)) + iinf
    payload = b"".join(d for _, d in items)
    ipco = _bx(b"ispe", bytes(4) + struct.pack(">II", 1, 1)) + _bx(b"colr", b"prof" + icc)
    if brand == b"heic":
        ipma = b"\0\0\0\1" + struct.pack(">IHB", 1, 1, 2) + struct.pack(">HH", 0x8001, 0x8002)
    else:
        ipma = bytes(4) + struct.pack(">IHB", 1, 1, 2) + b"\x81\x82"
    iprp = _bx(b"ipco", ipco) + _bx(b"ipma", ipma)
    ftyp = _bx(b"ftyp", brand + bytes(4) + b"mif1" + brand)

    def meta(at):
        method = 1 if in_idat else 0
        iloc = b"\1\0\0\0" + bytes([0x44, 0x00]) + struct.pack(">H", len(items))
        for item, data in items:
            iloc += struct.pack(">HHHHII", item, method, 0, 1, at, len(data))
            at += len(data)
        m = (_bx(b"hdlr", bytes(24)) + _bx(b"pitm", bytes(4) + b"\0\1") + _bx(b"iinf", iinf)
             + _bx(b"iref", bytes(4) + cdsc) + _bx(b"iprp", iprp)
             + _bx(b"iloc", iloc))
        if in_idat:
            m += _bx(b"idat", payload)
        return _bx(b"meta", bytes(4) + m)
    if in_idat:
        return ftyp + meta(0) + _bx(b"mdat", b"")
    at = len(ftyp) + len(meta(0)) + 8
    return ftyp + meta(at) + _bx(b"mdat", payload)


def _psd(version, resources):
    """A PSD (`version` 1) or PSB (2) header, an empty colour mode section and image resources
    `(id, data)`, each named "" except the first."""
    body = b""
    for n, (rid, data) in enumerate(resources):
        name = b"\3res" if n == 0 else b"\0\0"
        body += b"8BIM" + struct.pack(">H", rid) + name + struct.pack(">I", len(data)) + data
        body += b"\0" if len(data) % 2 else b""
    head = b"8BPS" + struct.pack(">H", version) + bytes(6) + struct.pack(">HIIHH", 3, 1, 1, 8, 3)
    return head + bytes(4) + struct.pack(">I", len(body)) + body + bytes(8)


_CANON_UUID = bytes.fromhex("85c0b687820f11e08111f4ce462b6a48")
_XMP_UUID = bytes.fromhex("be7acfcb97a942e89c71999491e3afac")


def _tiff_ifd(order, entries):
    """A one-IFD TIFF; `entries` are `(tag, type, count, value bytes)` already in `order`."""
    t = (b"II*\0" if order == "<" else b"MM\0*") + struct.pack(order + "IH", 8, len(entries))
    tail_at = 8 + 2 + 12 * len(entries) + 4
    tail = b""
    for tag, typ, count, value in entries:
        if len(value) <= 4:
            t += struct.pack(order + "HHI", tag, typ, count) + value.ljust(4, b"\0")
        else:
            t += struct.pack(order + "HHII", tag, typ, count, tail_at + len(tail))
            tail += value + (b"\0" if len(value) % 2 else b"")
    return t + bytes(4) + tail


def _cr3(xmp):
    """A Canon CR3 skeleton: CMT1 (IFD0), CMT2 (Exif IFD), CMT3 (a Canon maker note:
    CameraSettings, a junk-padded LensModel and a made-up serial number) and a big-endian CMT4
    (GPS IFD) in `moov`'s Canon uuid, and `xmp` NUL-padded in the top-level XMP uuid."""
    le = "<"
    cmt1 = _tiff_ifd(le, [(0x010F, 2, 6, b"Canon\0"), (0x0110, 2, 10, b"Seed Cam\0\0"),
                          (0x0112, 3, 1, struct.pack("<H", 6)), (0x8769, 4, 1, struct.pack("<I", 99))])
    cmt2 = _tiff_ifd(le, [(0x829A, 5, 1, struct.pack("<II", 1, 250)), (0x8827, 3, 1, struct.pack("<H", 400)),
                          (0x9003, 2, 20, b"2024:05:06 07:08:09\0")])
    rat = lambda *v: b"".join(struct.pack(">II", n, d) for n, d in v)
    cmt4 = _tiff_ifd(">", [(0x0000, 1, 4, b"\2\3\0\0"), (0x0001, 2, 2, b"N\0"),
                           (0x0002, 5, 3, rat((48, 1), (30, 1), (0, 1))), (0x0003, 2, 2, b"W\0"),
                           (0x0004, 5, 3, rat((11, 1), (15, 1), (0, 1)))])
    cs = [0] * 40
    cs[0], cs[7], cs[23], cs[24], cs[25], cs[26], cs[34] = 80, 1, 105, 24, 1, 128, 257
    cmt3 = _tiff_ifd(le, [(0x0001, 3, 40, struct.pack("<40H", *cs)), (0x000C, 4, 1, struct.pack("<I", 1234)),
                          (0x0095, 2, 24, b"Seed 24-105mm\0".ljust(24, b"\x05"))])
    canon = _bx(b"CNCV", b"CanonCR3_001/00.09.00/00.00.00")
    for kind, data in ((b"CMT1", cmt1), (b"CMT2", cmt2), (b"CMT3", cmt3), (b"CMT4", cmt4)):
        canon += _bx(kind, data)
    moov = _bx(b"moov", _bx(b"uuid", _CANON_UUID + canon) + _bx(b"mvhd", bytes(100)))
    return (_bx(b"ftyp", b"crx \0\0\0\1crx isom") + moov
            + _bx(b"uuid", _XMP_UUID + xmp + bytes(64)) + _bx(b"mdat", bytes(16)))


def _ifd_at(at, entries, big=False):
    """A little-endian IFD written at offset `at`, its values after it (offsets from the TIFF
    header); classic, or BigTIFF (8-byte counts and offsets, values up to 8 bytes inline)."""
    cs, es, fs, off = (8, 20, 8, "Q") if big else (2, 12, 4, "I")
    tail_at = at + cs + es * len(entries) + fs
    t, tail = struct.pack("<Q" if big else "<H", len(entries)), b""
    for tag, typ, count, value in entries:
        t += struct.pack("<HH" + off, tag, typ, count)
        if len(value) <= fs:
            t += value.ljust(fs, b"\0")
        else:
            t += struct.pack("<" + off, tail_at + len(tail))
            tail += value + (b"\0" if len(value) % 2 else b"")
    return t + bytes(fs) + tail


def _big_exif(xmp, icc):
    """A BigTIFF with Make, Model, XMP, ICC and an Exif IFD (IFD8 pointer, a LONG8
    PixelXDimension)."""
    def ifd0(exif_at):
        return _ifd_at(16, [(0x010F, 2, 5, b"Seed\0"), (0x0110, 2, 9, b"Seed Cam\0"),
                            (700, 1, len(xmp), xmp), (0x8769, 18, 1, struct.pack("<Q", exif_at)),
                            (34675, 7, len(icc), icc)], True)
    exif_at = 16 + len(ifd0(0))
    exif = _ifd_at(exif_at, [(0x829A, 5, 1, struct.pack("<II", 1, 250)),
                             (0xA002, 16, 1, struct.pack("<Q", 640))], True)
    return b"II+\0" + struct.pack("<HHQ", 8, 0, 16) + ifd0(exif_at) + exif


def _canon_tiff(xmp, icc):
    """A CR2-like TIFF (with XMP and ICC) whose Canon maker note (offsets from the header) comes
    first, before IFD0 and the Exif IFD, so the rebuilt Exif must rewrite it."""
    cs = [80, 0, 0, 0, 0, 0, 0, 1] + [0] * 15 + [85, 17, 1] + [0] * 14
    lens = b"EF-S17-85mm\0"
    note = _ifd_at(8, [(0x0001, 3, 40, struct.pack("<40H", *cs)), (0x000C, 4, 1, struct.pack("<I", 1234)),
                       (0x0095, 2, len(lens), lens)])
    ifd0_at = 8 + len(note)

    def ifd0(exif_at):
        return _ifd_at(ifd0_at, [(0x010F, 2, 6, b"Canon\0"), (0x0110, 2, 10, b"Seed Cam\0\0"),
                                 (700, 1, len(xmp), xmp), (0x8769, 4, 1, struct.pack("<I", exif_at)),
                                 (34675, 7, len(icc), icc)])
    exif_at = ifd0_at + len(ifd0(0))
    exif = _ifd_at(exif_at, [(0x829A, 5, 1, struct.pack("<II", 1, 250)),
                             (0x927C, 7, len(note), struct.pack("<I", 8))])
    return b"II*\0" + struct.pack("<I", ifd0_at) + note + ifd0(exif_at) + exif


def embedded(out_dir):
    """Seeds for specular-decode's `locate` target: the places XMP and an ICC profile sit in
    PNG (iTXt, iCCP), WebP (XMP , ICCP), HEIC/AVIF (mime item, colr; also deflate/zlib/gzip-
    coded XMP items and coded application/exif items), TIFF (700, 34675), JPEG XL (xml box),
    PSD/PSB (image resources 1039, 1058, 1060) and CR3 (CMT boxes with a Canon maker note, XMP
    uuid); BigTIFF Exif, and a CR2-like TIFF whose Canon maker note precedes its IFDs."""
    import gzip
    import os
    import zlib
    os.makedirs(out_dir, exist_ok=True)
    icc, xmp = _tiny_icc(), _XMP
    files = {}
    ihdr = _png_chunk(b"IHDR", struct.pack(">IIBBBBB", 1, 1, 8, 2, 0, 0, 0))
    idat = _png_chunk(b"IDAT", zlib.compress(b"\0\0\0\0"))
    files["meta.png"] = (b"\x89PNG\r\n\x1a\n" + ihdr + _png_chunk(b"iCCP", b"ICC\0\0" + zlib.compress(icc))
                         + idat + _png_chunk(b"iTXt", b"XML:com.adobe.xmp\0\1\0\0\0" + zlib.compress(xmp))
                         + _png_chunk(b"IEND", b""))
    files["meta_plain.png"] = (b"\x89PNG\r\n\x1a\n" + ihdr + idat
                               + _png_chunk(b"iTXt", b"XML:com.adobe.xmp\0\0\0\0\0" + xmp)
                               + _png_chunk(b"IEND", b""))
    body = b"WEBP"
    for k, d in ((b"VP8X", b"\x24" + bytes(9)), (b"ICCP", icc), (b"VP8L", b"\x2f\0\0\0\0"), (b"XMP ", xmp)):
        body += k + struct.pack("<I", len(d)) + d + (b"\0" if len(d) % 2 else b"")
    files["meta.webp"] = b"RIFF" + struct.pack("<I", len(body)) + body
    files["meta.heic"] = _heif(b"heic", b"hvc1", xmp, icc, False)
    files["meta_idat.avif"] = _heif(b"avif", b"av01", xmp, icc, True)
    for order, f in ((">", b"MM\0*"), ("<", b"II*\0")):
        at = 8 + 2 + 2 * 12 + 4
        t = f + struct.pack(order + "IH", 8, 2)
        t += struct.pack(order + "HHII", 700, 1, len(xmp), at)
        t += struct.pack(order + "HHII", 34675, 7, len(icc), at + len(xmp))
        files["meta_%s.tif" % ("be" if order == ">" else "le")] = t + bytes(4) + xmp + icc
    files["meta_zlib.heic"] = _heif(b"heic", b"hvc1", zlib.compress(xmp), icc, False, b"deflate")
    raw = zlib.compressobj(9, zlib.DEFLATED, -15)
    files["meta_raw.avif"] = _heif(b"avif", b"av01", raw.compress(xmp) + raw.flush(), icc, True, b"deflate")
    files["meta_ident.heic"] = _heif(b"heic", b"hvc1", xmp, icc, True, b"identity")
    files["meta_gzip.heic"] = _heif(b"heic", b"hvc1", gzip.compress(xmp, mtime=0), icc, False, b"gzip")
    # Compressed Exif items: zlib (with the 4-byte offset prefix) and gzip (bare TIFF).
    tiff_exif = _tiff_ifd("<", [(0x010F, 2, 5, b"Seed\0"), (0x0110, 2, 9, b"Seed Cam\0")])
    files["meta_exif_zlib.heic"] = _heif(b"heic", b"hvc1", xmp, icc, False,
                                         exif=(zlib.compress(bytes(4) + tiff_exif), b"deflate"))
    files["meta_exif_gzip.avif"] = _heif(b"avif", b"av01", xmp, icc, True,
                                         exif=(gzip.compress(tiff_exif, mtime=0), b"gzip"))
    exif = b"Exif\0\0" + _tiff_ifd("<", [(0x0110, 2, 5, b"Seed\0")])
    files["meta.psd"] = _psd(1, [(1005, bytes(16)), (1039, icc), (1058, exif), (1060, xmp)])
    files["meta.psb"] = _psd(2, [(1039, icc), (1059, exif[6:]), (1060, xmp)])
    files["meta.cr3"] = _cr3(xmp)
    files["meta_exif_big.tif"] = _big_exif(xmp, icc)
    files["meta_canon_note.cr2"] = _canon_tiff(xmp, icc)
    files["meta.jxl"] = (_bx(b"JXL ", b"\r\n\x87\n") + _bx(b"ftyp", b"jxl \0\0\0\0jxl ")
                         + _bx(b"xml ", xmp) + _bx(b"jxlc", b"\xff\x0a\0\0"))
    for name, data in files.items():
        open(os.path.join(out_dir, name), "wb").write(data)


if __name__ == "__main__":
    {"ani": ani, "fli": fli, "radiance": radiance, "texture": texture, "png": png, "tiff": tiff, "psd": psd, "exr": exr, "archive": archive, "icns": icns, "ora": ora, "metadata": metadata, "icc": icc, "embedded": embedded}[sys.argv[1]](*sys.argv[2:])
