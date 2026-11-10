#!/usr/bin/env python3
"""TIFF compression oracle for specular-tiff.

    tiff-oracle.py generate <dir>            write synthetic fixtures (tifffile + imagecodecs,
                                             Pillow/libtiff for CCITT)
    tiff-oracle.py variants <dir>            write fixtures of the rarer layouts: 17/20/24/
                                             64-bit and signed integers, subsampled YCbCr,
                                             old-style LZW, LogL/LogLuv (libtiff), LERC v6
                                             lossless float, planar and 12-bit JPEG, tiled
                                             planar files, floating-point predictor cases
    tiff-oracle.py compare <dump> <files...> decode each file with the reference stack and
                                             compare with specular-tiff's output
    tiff-oracle.py check <file> <raw> <w> <h> <format> <truncated>
                                             one file decoded elsewhere (tests/oracle.rs):
                                             prints PASS, FAIL or SKIP and the detail

<dump> is the `dump` example's binary; its output goes to ./oracle-out (run this from a
scratch directory such as target/, never from inside a corpus). The reference is tifffile
(imagecodecs codecs, libtiff semantics); CCITT and old-style JPEG go through Pillow's libtiff;
ThunderScan, old-style LZW, YCbCr outside JPEG, SGI LogL/LogLuv (as float XYZ) and 24-bit
integers through libtiff itself (the copy bundled with Pillow, called with ctypes). Needs a
venv with numpy, tifffile, imagecodecs and Pillow. Nothing here touches the network.
"""

import os
import subprocess
import sys

import numpy as np


def gradient(h, w, c, dtype, seed=1):
    rng = np.random.default_rng(seed)
    y, x = np.mgrid[0:h, 0:w].astype(np.float64)
    planes = []
    for k in range(c):
        v = 0.5 + 0.35 * np.sin(x / (17 + 5 * k) + y / (23 + 3 * k)) + 0.1 * rng.random((h, w))
        planes.append(v)
    a = np.stack(planes, -1) if c > 1 else planes[0]
    dt = np.dtype(dtype)
    if dt.kind == 'f':
        return (a * 2.5).astype(dt)  # some values above 1: HDR
    if dt.kind == 'i':
        info = np.iinfo(dt)
        return (a * (info.max - info.min) + info.min).clip(info.min, info.max).astype(dt)
    return (a * np.iinfo(dt).max).clip(0, np.iinfo(dt).max).astype(dt)


def generate(out):
    import tifffile
    import imagecodecs
    from PIL import Image

    os.makedirs(out, exist_ok=True)
    made = []

    def w(name, data, **kw):
        path = os.path.join(out, name)
        try:
            tifffile.imwrite(path, data, **kw)
            made.append(name)
        except Exception as e:  # codec not available etc.
            print(f'skip {name}: {e}', file=sys.stderr)

    h, wd = 300, 410
    # Zstandard and LZMA, with and without predictors, strips and tiles.
    for comp in ('zstd', 'lzma'):
        w(f'{comp}-u8-rgb.tif', gradient(h, wd, 3, 'u1'), compression=comp)
        w(f'{comp}-u16-gray-pred.tif', gradient(h, wd, 1, 'u2'), compression=comp, predictor=2)
        w(f'{comp}-u16-rgb-tiled.tif', gradient(h, wd, 3, 'u2'), compression=comp, tile=(128, 128),
          photometric='rgb')
        w(f'{comp}-f32-gray-pred3.tif', gradient(h, wd, 1, 'f4'), compression=comp, predictor=3)
    w('zstd-u16-gray-big.tif', gradient(1024, 1024, 1, 'u2'), compression='zstd', predictor=2,
      tile=(256, 256))
    # Deflate with the floating-point predictor, both byte orders, 16/32/64-bit.
    for bo, tag in (('<', 'le'), ('>', 'be')):
        for dt in ('f2', 'f4', 'f8'):
            w(f'deflate-{dt}-rgb-pred3-{tag}.tif', gradient(h, wd, 3, dt), compression='zlib',
              predictor=3, byteorder=bo, photometric='rgb')
        w(f'deflate-f4-gray-pred3-tiled-{tag}.tif', gradient(h, wd, 1, 'f4'), compression='zlib',
          predictor=3, byteorder=bo, tile=(64, 64))
    rgba = gradient(h, wd, 4, 'f4')
    rgba[..., 3] = np.clip(rgba[..., 3] / 2.5, 0, 1)
    w('none-f4-rgba.tif', rgba, photometric='rgb', extrasamples=['unassalpha'])
    w('none-f4-gray-miniswhite.tif', gradient(h, wd, 1, 'f4') / 3, photometric='miniswhite')
    # LERC: data types, lossy, masks (NaN), depth, extra compression.
    for dt in ('u1', 'i1', 'u2', 'i2', 'u4', 'i4', 'f4', 'f8'):
        w(f'lerc-{dt}-gray.tif', gradient(h, wd, 1, dt), compression='lerc', tile=(256, 256))
    w('lerc-u1-rgb.tif', gradient(h, wd, 3, 'u1'), compression='lerc', photometric='rgb')
    w('lerc-u2-rgb-tiled.tif', gradient(h, wd, 3, 'u2'), compression='lerc', tile=(128, 128),
      photometric='rgb')
    w('lerc-f4-rgb.tif', gradient(h, wd, 3, 'f4'), compression='lerc', photometric='rgb')
    w('lerc-f4-lossy.tif', gradient(h, wd, 1, 'f4'), compression='lerc',
      compressionargs={'level': 0.01})
    w('lerc-u2-lossy.tif', gradient(h, wd, 1, 'u2'), compression='lerc',
      compressionargs={'level': 4})
    dem = (gradient(512, 512, 1, 'f4') * 400 + 1200).astype('f4')
    dem[100:180, 50:300] = np.nan
    w('lerc-f4-dem-nodata.tif', dem, compression='lerc', tile=(256, 256))
    w('lerc-deflate-f4.tif', gradient(h, wd, 1, 'f4'), compression='lerc',
      compressionargs={'compression': 'deflate'})
    w('lerc-zstd-u2.tif', gradient(h, wd, 1, 'u2'), compression='lerc',
      compressionargs={'compression': 'zstd'})
    flat = np.full((h, wd), 7, 'u2')
    w('lerc-u2-const.tif', flat, compression='lerc')
    for v in (2, 3, 5, 6):
        w(f'lerc-v{v}-u2.tif', gradient(h, wd, 1, 'u2'), compression='lerc',
          compressionargs={'version': v})
        w(f'lerc-v{v}-f4.tif', gradient(h, wd, 1, 'f4'), compression='lerc',
          compressionargs={'version': v})
    w('lerc-v5-u1-rgb.tif', gradient(h, wd, 3, 'u1'), compression='lerc', photometric='rgb',
      compressionargs={'version': 5})
    w('lerc-v5-u2-rgb.tif', gradient(h, wd, 3, 'u2'), compression='lerc', photometric='rgb',
      compressionargs={'version': 5})
    w('lerc-v6-f4-rgb.tif', gradient(h, wd, 3, 'f4'), compression='lerc', photometric='rgb',
      compressionargs={'version': 6, 'level': 0.001})
    # WebP, JPEG XL, JPEG 2000 units.
    w('webp-rgb.tif', gradient(h, wd, 3, 'u1'), compression='webp', tile=(128, 128),
      compressionargs={'level': 100, 'lossless': True})
    w('webp-rgba.tif', gradient(h, wd, 4, 'u1'), compression='webp', tile=(128, 128),
      photometric='rgb', extrasamples=['unassalpha'],
      compressionargs={'lossless': True})
    w('jxl-rgb.tif', gradient(h, wd, 3, 'u1'), compression='jpegxl', tile=(128, 128),
      compressionargs={'lossless': True})
    w('jxl-gray.tif', gradient(h, wd, 1, 'u1'), compression='jpegxl',
      compressionargs={'lossless': True})
    w('j2k-rgb.tif', gradient(h, wd, 3, 'u1'), compression='jpeg2000', tile=(128, 128),
      compressionargs={'level': 0})
    w('j2k-u16-gray.tif', gradient(h, wd, 1, 'u2'), compression='jpeg2000',
      compressionargs={'level': 0})
    # Aperio-style 33003 (YCbCr codestreams without the colour transform) and 33005 (RGB).
    rgb = gradient(512, 512, 3, 'u1')

    def tiles(a, ycbcr):
        for ty in range(0, 512, 256):
            for tx in range(0, 512, 256):
                t = a[ty:ty + 256, tx:tx + 256].astype(np.float64)
                if ycbcr:
                    r, g, b = t[..., 0], t[..., 1], t[..., 2]
                    y = 0.299 * r + 0.587 * g + 0.114 * b
                    cb = (b - y) / 1.772 + 128
                    cr = (r - y) / 1.402 + 128
                    t = np.stack([y, cb, cr], -1)
                t = np.round(t).clip(0, 255).astype('u1')
                yield imagecodecs.jpeg2k_encode(t, level=0, codecformat='J2K', mct=not ycbcr)

    for code, ycc in ((33003, True), (33005, False)):
        name = f'aperio-{code}.tif'
        try:
            tifffile.imwrite(os.path.join(out, name), tiles(rgb, ycc), shape=rgb.shape,
                             dtype='u1', tile=(256, 256), compression=code, photometric='rgb')
            made.append(name)
        except Exception as e:
            print(f'skip {name}: {e}', file=sys.stderr)
    # CCITT through Pillow's libtiff.
    bw = (gradient(h, wd, 1, 'u1') > 128)
    img = Image.fromarray(bw)
    for comp in ('group4', 'group3', 'tiff_ccitt'):
        name = f'ccitt-{comp}.tif'
        img.save(os.path.join(out, name), compression=comp)
        made.append(name)
    # An A0 sheet at 400 dpi (46.8 × 33.1 in), CCITT G4: lines and text-like blocks.
    a0 = np.ones((13244, 18720), bool)
    a0[::97, :] = False
    a0[:, ::131] = False
    rng = np.random.default_rng(3)
    for _ in range(4000):
        y, x = rng.integers(0, 13200), rng.integers(0, 18600)
        a0[y:y + 30, x:x + rng.integers(10, 120)] = rng.random() < 0.5
    Image.fromarray(a0).save(os.path.join(out, 'ccitt-a0-g4.tif'), compression='group4')
    made.append('ccitt-a0-g4.tif')
    print('\n'.join(made))


def write_tiff(path, w, h, bits, spp, data, photometric, compression=1, sampleformat=1,
               extra=(), le=True, rows=None, planar=False):
    """A minimal classic TIFF: `data` is a list of already coded strips (bytes)."""
    import struct
    e = '<' if le else '>'
    rows = rows or h
    nstrips = len(data)
    tags = [(256, 4, [w]), (257, 4, [h]), (258, 3, [bits] * spp), (259, 3, [compression]),
            (262, 3, [photometric]), (273, 4, None), (277, 3, [spp]), (278, 4, [rows]),
            (279, 4, [len(d) for d in data]), (284, 3, [2 if planar else 1]),
            (339, 3, [sampleformat] * spp)]
    tags += list(extra)
    tags.sort()
    size = {3: 2, 4: 4, 5: 8}
    head = 8
    ifd_len = 2 + 12 * len(tags) + 4
    blob = bytearray()
    out_tags = []
    base = head + ifd_len
    strip_at = []
    for d in data:
        strip_at.append(base + len(blob))
        blob += d
        if len(blob) % 2:
            blob += b'\0'
    for tag, typ, vals in tags:
        if tag == 273:
            vals = strip_at
        n = len(vals)
        if typ == 5:
            raw = b''.join(struct.pack(e + 'II', a, b) for a, b in vals)
        else:
            raw = b''.join(struct.pack(e + ('H' if typ == 3 else 'I'), v) for v in vals)
        if len(raw) <= 4:
            out_tags.append((tag, typ, n, raw.ljust(4, b'\0')))
        else:
            off = base + len(blob)
            blob += raw
            if len(blob) % 2:
                blob += b'\0'
            out_tags.append((tag, typ, n, struct.pack(e + 'I', off)))
    f = bytearray((b'II' if le else b'MM') + struct.pack(e + 'HI', 42, head))
    f += struct.pack(e + 'H', len(out_tags))
    for tag, typ, n, val in out_tags:
        f += struct.pack(e + 'HHI', tag, typ, n) + val
    f += struct.pack(e + 'I', 0)
    f += blob
    with open(path, 'wb') as fh:
        fh.write(f)


def pack_bits(a, bits):
    """Rows of unsigned integers packed MSB first, each row padded to a byte."""
    out = bytearray()
    for row in a.reshape(a.shape[0], -1):
        acc, n = 0, 0
        for v in row.tolist():
            acc = (acc << bits) | int(v)
            n += bits
            while n >= 8:
                n -= 8
                out.append((acc >> n) & 0xff)
            acc &= (1 << n) - 1
        if n:
            out.append((acc << (8 - n)) & 0xff)
    return bytes(out)


def lzw_old(data):
    """Old-style (LSB-first, no early change) TIFF LZW, as GIF codes it with 8-bit roots."""
    table = {bytes([i]): i for i in range(256)}
    nxt, width = 258, 9
    acc, nacc, out = 0, 0, bytearray()

    def put(code):
        nonlocal acc, nacc
        acc |= code << nacc
        nacc += width
        while nacc >= 8:
            out.append(acc & 0xff)
            acc >>= 8
            nacc -= 8

    put(256)
    cur = b''
    for b in data:
        nb = cur + bytes([b])
        if nb in table:
            cur = nb
            continue
        put(table[cur])
        if nxt < 4093:
            # The decoder adds this entry one code later, so it widens one code later too.
            if nxt >= (1 << width) and width < 12:
                width += 1
            table[nb] = nxt
            nxt += 1
        else:
            put(256)
            table = {bytes([i]): i for i in range(256)}
            nxt, width = 258, 9
        cur = bytes([b])
    if cur:
        put(table[cur])
    put(257)
    if nacc:
        out.append(acc & 0xff)
    return bytes(out)


def ycbcr_blocks(rgb, hs, vs):
    """8-bit RGB to TIFF YCbCr blocks (`hs`×`vs` luma, then mean Cb and Cr), padded."""
    h, w = rgb.shape[:2]
    H, W = -(-h // vs) * vs, -(-w // hs) * hs
    a = np.pad(rgb.astype(np.float64), ((0, H - h), (0, W - w), (0, 0)), mode='edge')
    r, g, b = a[..., 0], a[..., 1], a[..., 2]
    y = 0.299 * r + 0.587 * g + 0.114 * b
    cb = (b - y) / 1.772 + 128
    cr = (r - y) / 1.402 + 128
    q = lambda v: np.round(v).clip(0, 255).astype('u1')
    out = bytearray()
    for by in range(0, H, vs):
        for bx in range(0, W, hs):
            out += q(y[by:by + vs, bx:bx + hs]).tobytes()
            out.append(q(cb[by:by + vs, bx:bx + hs].mean()))
            out.append(q(cr[by:by + vs, bx:bx + hs].mean()))
    return bytes(out)


def logluv_write(path, xyz, compression=34676):
    """LogLuv32 (3 floats) or LogL (1 float) through libtiff's SGILOG encoder; with
    compression 34677 (SGILOG24), LogLuv24."""
    import ctypes
    lib = libtiff()
    h, w = xyz.shape[:2]
    spp = 1 if xyz.ndim == 2 else 3
    tif = lib.TIFFOpen(path.encode(), b'w')
    if not tif:
        raise RuntimeError('libtiff cannot write')
    t = ctypes.c_void_p(tif)
    u = lambda v: ctypes.c_int(v)
    try:
        for tag, v in ((256, w), (257, h), (258, 16), (277, spp), (259, compression),
                       (262, 32845 if spp == 3 else 32844), (284, 1), (278, 16),
                       (65560, 0)):
            lib.TIFFSetField(t, ctypes.c_uint32(tag), u(v))
        a = np.ascontiguousarray(xyz.astype(np.float32).reshape(h, -1))
        for y in range(h):
            if lib.TIFFWriteScanline(t, ctypes.c_void_p(a[y].ctypes.data), ctypes.c_uint32(y),
                                     ctypes.c_uint16(0)) < 0:
                raise RuntimeError('TIFFWriteScanline')
    finally:
        lib.TIFFClose(tif)


def variants(out):
    import tifffile
    os.makedirs(out, exist_ok=True)
    made = []

    def done(name):
        made.append(name)

    h, w = 61, 83  # odd sizes: partial blocks and padded rows
    # Integers of 17, 20 and 24 bits (packed MSB first; 24 in the file's byte order), grey
    # and RGB, contiguous and planar; signed 24-bit.
    for bits, spp in ((17, 1), (20, 3), (24, 1), (24, 3), (31, 1)):
        a = gradient(h, w, spp, 'u4') >> (32 - bits)
        for le in (True, False):
            name = f'int{bits}-{spp}s-{"le" if le else "be"}.tif'
            if bits == 24:
                b = a.astype('<u4' if le else '>u4').view('u1').reshape(h, w * spp, 4)
                data = (b[..., :3] if le else b[..., 1:]).tobytes()
            else:
                data = pack_bits(a, bits)
            write_tiff(os.path.join(out, name), w, h, bits, spp, [data],
                       2 if spp == 3 else 1, le=le)
            done(name)
    a = gradient(h, w, 3, 'u4') >> 8
    planes = []
    for c in range(3):
        b = a[..., c].astype('<u4').view('u1').reshape(h, w, 4)
        planes.append(b[..., :3].tobytes())
    write_tiff(os.path.join(out, 'int24-rgb-planar.tif'), w, h, 24, 3, planes, 2, planar=True)
    done('int24-rgb-planar.tif')
    s24 = (gradient(h, w, 1, 'i4') >> 8).astype('<i4').view('u1').reshape(h, w, 4)
    write_tiff(os.path.join(out, 'int24-signed.tif'), w, h, 24, 1, [s24[..., :3].tobytes()], 1,
               sampleformat=2)
    done('int24-signed.tif')
    # 64-bit integers, signed integer RGB (8/16/32-bit) through tifffile.
    for name, data, kw in (
            ('int64-gray.tif', gradient(h, w, 1, 'u8'), {}),
            ('int64-signed-rgb.tif', gradient(h, w, 3, 'i8'), {'photometric': 'rgb'}),
            ('int64-gray-deflate-pred.tif', gradient(h, w, 1, 'u8'),
             {'compression': 'zlib', 'predictor': 2}),
            ('int8-signed-rgb.tif', gradient(h, w, 3, 'i1'), {'photometric': 'rgb'}),
            ('int16-signed-rgb.tif', gradient(h, w, 3, 'i2'), {'photometric': 'rgb'}),
            ('int32-signed-rgba.tif', gradient(h, w, 4, 'i4'),
             {'photometric': 'rgb', 'extrasamples': ['unassalpha']}),
            ('int16-signed-gray.tif', gradient(h, w, 1, 'i2'), {})):
        tifffile.imwrite(os.path.join(out, name), data, **kw)
        done(name)
    # Subsampled YCbCr, uncompressed and LZW-free Deflate, every sampling libtiff reads.
    for hs, vs in ((2, 2), (2, 1), (4, 1), (4, 2), (4, 4), (1, 2), (1, 1)):
        # libtiff's 4×4 reader garbles partial blocks at the edges: whole blocks there.
        w, h = (84, 64) if (hs, vs) == (4, 4) else (83, 61)
        rgb = gradient(h, w, 3, 'u1')
        rps = 2 * vs * 4
        strips = []
        for y0 in range(0, h, rps):
            strips.append(ycbcr_blocks(rgb[y0:y0 + rps], hs, vs))
        refbw = (532, 5, [(0, 1), (255, 1), (128, 1), (255, 1), (128, 1), (255, 1)])
        sub = (530, 3, [hs, vs])
        name = f'ycbcr-{hs}{vs}-none.tif'
        write_tiff(os.path.join(out, name), w, h, 8, 3, strips, 6, rows=rps, extra=[sub, refbw])
        done(name)
        if (hs, vs) == (2, 2):
            import zlib
            name = 'ycbcr-22-deflate.tif'
            write_tiff(os.path.join(out, name), w, h, 8, 3, [zlib.compress(s) for s in strips],
                       6, compression=8, rows=rps, extra=[sub, refbw])
            done(name)
    w, h = 83, 61
    # Old-style LZW, several strips, grey and RGB.
    for spp in (1, 3):
        a = gradient(h, w, spp, 'u1')
        rps = 16
        strips = [lzw_old(a[y0:y0 + rps].tobytes()) for y0 in range(0, h, rps)]
        name = f'lzw-old-{spp}s.tif'
        write_tiff(os.path.join(out, name), w, h, 8, spp, strips, 2 if spp == 3 else 1,
                   compression=5, rows=rps)
        done(name)
    # LogL and LogLuv32 (HDR: luminance from 1e-3 to 1e3).
    y = np.exp(np.linspace(np.log(1e-3), np.log(1e3), w))[None, :] * np.ones((h, 1))
    logluv_write(os.path.join(out, 'logl.tif'), y)
    done('logl.tif')
    hdr = gradient(h, w, 3, 'f4').astype(np.float64) * y[..., None]
    m = np.linalg.inv(xyz_e_to_rgb())
    logluv_write(os.path.join(out, 'logluv32.tif'), hdr @ m.T)
    done('logluv32.tif')
    # LogLuv24 (10-bit log luminance, 14-bit chroma cell; HDR within its 2^-12..2^4 range).
    y24 = np.exp(np.linspace(np.log(1e-3), np.log(10.0), w))[None, :] * np.ones((h, 1))
    hdr24 = gradient(h, w, 3, 'f4').astype(np.float64) * y24[..., None]
    logluv_write(os.path.join(out, 'logluv24.tif'), hdr24 @ m.T, 34677)
    done('logluv24.tif')
    # LERC version 6 lossless float (the FPL mode), float and double, grey and RGB.
    for dt, spp in (('f4', 1), ('f8', 1), ('f4', 3)):
        name = f'lerc-v6-lossless-{dt}-{spp}s.tif'
        a = gradient(97, 131, spp, dt)
        a.reshape(-1)[::17] = np.float32(np.pi)  # values that do not round-trip at low cost
        try:
            tifffile.imwrite(os.path.join(out, name), a, compression='lerc',
                             photometric='rgb' if spp == 3 else 'minisblack',
                             compressionargs={'version': 6, 'level': 0.0})
            done(name)
        except Exception as e:
            print(f'skip {name}: {e}', file=sys.stderr)
    # Planar JPEG (one JPEG per sample), 12-bit JPEG, tiled planar files with every
    # compression and predictor, the floating-point predictor at odd sizes and with alpha.
    def tw(name, data, **kw):
        if kw.get('planarconfig') == 'separate':
            data = np.moveaxis(data, -1, 0)
        try:
            tifffile.imwrite(os.path.join(out, name), data, **kw)
            done(name)
        except Exception as e:
            print(f'skip {name}: {e}', file=sys.stderr)

    rgb = dict(photometric='rgb')
    tw('jpeg-planar-rgb.tif', gradient(61, 83, 3, 'u1'), compression='jpeg',
       planarconfig='separate', **rgb)
    tw('jpeg-planar-rgb-tiled.tif', gradient(130, 150, 3, 'u1'), compression='jpeg',
       planarconfig='separate', tile=(64, 64), **rgb)
    tw('jpeg12-gray.tif', gradient(61, 83, 1, 'u2') >> 4, compression='jpeg', bitspersample=12)
    for comp in ('zlib', 'lzw', 'packbits', None):
        n = comp or 'none'
        tw(f'tiled-planar-{n}-u16.tif', gradient(130, 150, 3, 'u2'), compression=comp,
           planarconfig='separate', tile=(64, 48), **rgb)
        tw(f'tiled-planar-{n}-rgba.tif', gradient(130, 150, 4, 'u1'), compression=comp,
           planarconfig='separate', tile=(32, 32), extrasamples=['unassalpha'], **rgb)
    tw('tiled-planar-pred2-u16.tif', gradient(130, 150, 3, 'u2'), compression='zlib',
       predictor=2, planarconfig='separate', tile=(64, 48), **rgb)
    tw('tiled-planar-pred3-f4.tif', gradient(130, 150, 3, 'f4'), compression='zlib',
       predictor=3, planarconfig='separate', tile=(64, 48), **rgb)
    for dt in ('f2', 'f4', 'f8'):
        for bo, tag in (('<', 'le'), ('>', 'be')):
            tw(f'pred3-{dt}-{tag}-odd.tif', gradient(37, 53, 1, dt), compression='lzw',
               predictor=3, byteorder=bo, rowsperstrip=5)
            tw(f'pred3-{dt}-{tag}-rgba.tif', gradient(37, 53, 4, dt), compression='zlib',
               predictor=3, byteorder=bo, extrasamples=['unassalpha'], **rgb)
    print('\n'.join(made))


def reference(path):
    """The reference decode: (array HxWxC or HxW, photometric, extrasamples info)."""
    import tifffile
    with tifffile.TiffFile(path) as t:
        p = t.pages[0]
        # A missing PhotometricInterpretation means grey (RGB for 3+ samples) to the decoder.
        photometric = p.tags.valueof(262)
        if photometric is None:
            photometric = 2 if p.samplesperpixel >= 3 else 1
        photometric = int(photometric)
        comp = int(p.compression)
        alpha = bool(p.extrasamples) and int(p.extrasamples[0]) in (1, 2)
        if alpha and int(p.extrasamples[0]) == 1:
            alpha = 'associated'
        orientation = int(p.tags.valueof(274) or 1)
        bps = p.bitspersample if isinstance(p.bitspersample, int) else p.bitspersample[0]
        # YCbCr outside JPEG: tifffile hands back the YCbCr samples (or refuses subsampling).
        subsampled = photometric == 6 and comp not in (6, 7)
        if comp in (34676, 34677):
            return libtiff_logluv(path), 2 if photometric == 32845 else 1, False, 'libtiff'
        if subsampled and comp == 1 and tuple(p.tags.valueof(530) or ()) == (4, 4):
            # libtiff leaves the last 4×4 block of each strip black: decode the blocks here.
            return ycbcr_model(t, p), 2, False, 'model'
        if comp == 32809 or subsampled or (comp == 5 and old_lzw(t, p)):
            return libtiff_rgba(path, alpha), 2, bool(alpha), 'libtiff'
        if comp in (2, 3, 4, 6, 32771) or (comp == 7 and photometric == 6):
            from PIL import Image
            Image.MAX_IMAGE_PIXELS = None
            im = Image.open(path)
            im.load()
            if im.mode == '1':
                a = np.array(im, dtype=np.uint8) * 255
                return unorient(a, orientation), 1, False, 'pil'
            a = np.array(im.convert('RGBA' if alpha else 'RGB'))
            return unorient(a, orientation), 2, bool(alpha), 'pil'
        if bps == 24 and int(p.sampleformat) in (1, 2):
            # tifffile reads 24-bit integers big-endian; libtiff keeps the file's order.
            a = libtiff_int24(path, int(p.sampleformat) == 2)
        else:
            a = p.asarray()
        if int(p.planarconfig) == 2 and a.ndim == 3 and a.shape[0] == p.samplesperpixel:
            a = np.moveaxis(a, 0, -1)
        if a.dtype.kind == 'i' and bps not in (8, 16, 32, 64):
            # Signed n-bit: offset to unsigned (as the decoder does), then scaled below.
            a = (a.astype(np.int64) + (1 << (bps - 1))).astype(np.uint64 if bps > 32 else np.uint32)
        if a.dtype.kind == 'u' and bps not in (8, 16, 32, 64):
            # Scale n-bit samples to their container's full range, as the decoder does.
            top = np.iinfo(a.dtype).max
            a = np.minimum(a.astype(np.float64) * top / ((1 << bps) - 1) + 0.5, float(top)).astype(a.dtype)
        if comp == 33003:
            # Aperio YCbCr codestreams: tifffile hands back the YCbCr samples.
            y, cb, cr = (a[..., i].astype(np.float64) for i in range(3))
            cb, cr = cb - 128, cr - 128
            a = np.stack([y + 1.402 * cr, y - 0.344136 * cb - 0.714136 * cr, y + 1.772 * cb], -1)
            a = np.round(a).clip(0, 255).astype(np.uint8)
        return a, photometric, alpha, 'tifffile'


def libtiff():
    """libtiff (the copy bundled with Pillow) through ctypes, for codings tifffile lacks."""
    import ctypes
    import glob
    import PIL
    from PIL import Image  # noqa: F401  (loads the libraries libtiff links against)
    libs = glob.glob(os.path.join(os.path.dirname(PIL.__file__), '..', 'pillow.libs',
                                  'libtiff*.so*'))
    if not libs:
        raise RuntimeError('no libtiff in pillow.libs')
    lib = ctypes.CDLL(libs[0])
    lib.TIFFOpen.restype = ctypes.c_void_p
    lib.TIFFOpen.argtypes = [ctypes.c_char_p, ctypes.c_char_p]
    lib.TIFFClose.argtypes = [ctypes.c_void_p]
    lib.TIFFReadScanline.argtypes = [ctypes.c_void_p, ctypes.c_void_p, ctypes.c_uint32,
                                     ctypes.c_uint16]
    lib.TIFFReadRGBAImageOriented.argtypes = [ctypes.c_void_p, ctypes.c_uint32, ctypes.c_uint32,
                                              ctypes.c_void_p, ctypes.c_int, ctypes.c_int]
    lib.TIFFScanlineSize.restype = ctypes.c_int64
    lib.TIFFScanlineSize.argtypes = [ctypes.c_void_p]
    return lib


def _open(lib, path):
    import ctypes
    tif = lib.TIFFOpen(path.encode(), b'r')
    if not tif:
        raise RuntimeError('libtiff cannot open')
    w, h = ctypes.c_uint32(), ctypes.c_uint32()
    lib.TIFFGetField(ctypes.c_void_p(tif), ctypes.c_uint32(256), ctypes.byref(w))
    lib.TIFFGetField(ctypes.c_void_p(tif), ctypes.c_uint32(257), ctypes.byref(h))
    return tif, w.value, h.value


def libtiff_rgba(path, alpha):
    """libtiff's RGBA reading (upright), RGB or RGBA uint8."""
    lib = libtiff()
    tif, w, h = _open(lib, path)
    import ctypes
    try:
        if lib.TIFFIsTiled(ctypes.c_void_p(tif)):
            buf = np.zeros(w * h, dtype=np.uint32)
            if not lib.TIFFReadRGBAImageOriented(tif, w, h, buf.ctypes.data, 1, 0):
                raise RuntimeError('TIFFReadRGBAImageOriented failed')
            a = buf.view(np.uint8).reshape(h, w, 4)
        else:
            # Strip by strip, stopping at the first strip libtiff cannot read (a damaged or
            # short file): the caller then compares the rows both decoded.
            rps = ctypes.c_uint32(h)
            lib.TIFFGetField(ctypes.c_void_p(tif), ctypes.c_uint32(278), ctypes.byref(rps))
            rps = max(1, min(rps.value, h))
            parts = []
            for y in range(0, h, rps):
                n = min(rps, h - y)
                buf = np.zeros(w * rps, dtype=np.uint32)
                if not lib.TIFFReadRGBAStripExt(ctypes.c_void_p(tif), ctypes.c_uint32(y),
                                                ctypes.c_void_p(buf.ctypes.data),
                                                ctypes.c_int(1)):
                    break
                # The strip comes bottom-up.
                parts.append(buf[:w * n].view(np.uint8).reshape(n, w, 4)[::-1])
            a = np.concatenate(parts) if parts else np.zeros((0, w, 4), np.uint8)
    finally:
        lib.TIFFClose(tif)
    return a if alpha else a[..., :3]


# XYZ (equal-energy white) to linear RGB with the BT.709 primaries, white-balanced to that
# white: what LogLuv's XYZ means as display RGB.
def xyz_e_to_rgb():
    prim = np.array([[0.64, 0.33], [0.30, 0.60], [0.15, 0.06]])
    m = np.stack([prim[:, 0] / prim[:, 1], np.ones(3), (1 - prim.sum(1)) / prim[:, 1]])
    s = np.linalg.solve(m, np.ones(3))
    return np.linalg.inv(m * s)


def libtiff_logluv(path):
    """SGI LogL / LogLuv decoded by libtiff as float Y or XYZ, as linear grey or RGB."""
    import ctypes
    lib = libtiff()
    tif, w, h = _open(lib, path)
    try:
        # TIFFTAG_SGILOGDATAFMT = SGILOGDATAFMT_FLOAT.
        lib.TIFFSetField(ctypes.c_void_p(tif), ctypes.c_uint32(65560), ctypes.c_int(0))
        n = lib.TIFFScanlineSize(tif) // 4
        out = np.zeros((h, n), dtype=np.float32)
        for y in range(h):
            row = np.zeros(n, dtype=np.float32)
            if lib.TIFFReadScanline(tif, row.ctypes.data, y, 0) < 0:
                raise RuntimeError(f'row {y}')
            out[y] = row
    finally:
        lib.TIFFClose(tif)
    if n == w:
        return out
    xyz = out.reshape(h, w, 3).astype(np.float64)
    return (xyz @ xyz_e_to_rgb().T).astype(np.float32)


def libtiff_int24(path, signed):
    """24-bit integer samples as libtiff delivers them (host order), as int32/uint32 HxW[xC]."""
    import ctypes
    lib = libtiff()
    tif, w, h = _open(lib, path)
    t = ctypes.c_void_p(tif)
    try:
        spp, planar, rps = ctypes.c_uint16(1), ctypes.c_uint16(1), ctypes.c_uint32(h)
        lib.TIFFGetField(t, ctypes.c_uint32(277), ctypes.byref(spp))
        lib.TIFFGetField(t, ctypes.c_uint32(284), ctypes.byref(planar))
        lib.TIFFGetField(t, ctypes.c_uint32(278), ctypes.byref(rps))
        spp, planar, rps = spp.value, planar.value, max(1, min(rps.value, h))
        nstrips = lib.TIFFNumberOfStrips(t)
        lib.TIFFStripSize.restype = ctypes.c_int64
        size = lib.TIFFStripSize(t)
        chunks = []
        for i in range(nstrips):
            buf = np.zeros(size, np.uint8)
            n = lib.TIFFReadEncodedStrip(t, ctypes.c_uint32(i), ctypes.c_void_p(buf.ctypes.data),
                                         ctypes.c_int64(-1))
            if n < 0:
                raise RuntimeError(f'strip {i}')
            chunks.append(buf[:n])
    finally:
        lib.TIFFClose(tif)
    per = 1 if planar == 2 else spp
    data = np.concatenate(chunks)
    planes = spp if planar == 2 else 1
    b = data[:planes * h * w * per * 3].reshape(planes, h, w * per, 3).astype(np.uint32)
    v = b[..., 0] | b[..., 1] << 8 | b[..., 2] << 16
    if signed:
        v = (v.astype(np.int64) - ((v & 0x800000) << 1)).astype(np.int32)
    if planar == 2:
        return np.moveaxis(v, 0, -1) if spp > 1 else v[0]
    v = v[0].reshape(h, w, spp)
    return v[..., 0] if spp == 1 else v


def ycbcr_model(t, p):
    """Uncompressed subsampled 8-bit YCbCr (TIFF 6.0 section 21 blocks), JFIF full range."""
    hs, vs = p.tags.valueof(530)
    h, w = p.imagelength, p.imagewidth
    rps = p.tags.valueof(278) or h
    out = np.zeros((h, w, 3))
    fh = t.filehandle
    bw = -(-w // hs)
    for i, (off, cnt) in enumerate(zip(p.dataoffsets, p.databytecounts)):
        fh.seek(off)
        d = np.frombuffer(fh.read(cnt), np.uint8)
        y0 = i * rps
        for by in range(-(-min(rps, h - y0) // vs)):
            for bx in range(bw):
                b = d[(by * bw + bx) * (hs * vs + 2):][:hs * vs + 2].astype(np.float64)
                for j in range(vs):
                    for k in range(hs):
                        y, x = y0 + by * vs + j, bx * hs + k
                        if y < h and x < w:
                            out[y, x] = (b[j * hs + k], b[-2] - 128, b[-1] - 128)
    yy, cb, cr = out[..., 0], out[..., 1], out[..., 2]
    rgb = np.stack([yy + 1.402 * cr, yy - 0.344136 * cb - 0.714136 * cr, yy + 1.772 * cb], -1)
    return np.round(rgb).clip(0, 255).astype(np.uint8)


def old_lzw(t, p):
    """Whether the first strip is old-style (LSB-first) LZW."""
    fh = t.filehandle
    fh.seek(p.dataoffsets[0])
    b = fh.read(2)
    return len(b) == 2 and b[0] == 0 and b[1] & 1


def unorient(a, o):
    """Undo TIFF Orientation (1..8). libtiff's RGBA path (Pillow) presents pages upright;
    specular-tiff returns stored order and leaves orientation to the viewer."""
    o = {6: 8, 8: 6}.get(o, o)
    if o in (5, 6, 7, 8):
        a = np.swapaxes(a, 0, 1)
        o = {5: 1, 6: 2, 7: 3, 8: 4}[o]
    if o in (2, 3):
        a = a[:, ::-1]
    if o in (3, 4):
        a = a[::-1]
    return np.ascontiguousarray(a)


def half_array(raw, w, h):
    return np.frombuffer(raw, dtype=np.float16).reshape(h, w, 4).astype(np.float32)


def compare(dump, files):
    scratch = os.path.join(os.getcwd(), 'oracle-out')
    os.makedirs(scratch, exist_ok=True)
    out = subprocess.run([dump, scratch] + files, capture_output=True, text=True, check=True)
    passed = failed = 0
    for line, path in zip(out.stdout.strip().split('\n'), files):
        parts = line.split('\t')
        name = parts[0]
        if parts[1] == 'ERR':
            print(f'FAIL {name}: ours: {parts[2]}')
            failed += 1
            continue
        w, h, fmt, trunc, ms = int(parts[1]), int(parts[2]), parts[3], parts[4], float(parts[5])
        raw = open(os.path.join(scratch, name + '.raw'), 'rb').read()
        tag, why = check_file(path, raw, w, h, fmt, trunc == 'true')
        if tag == 'SKIP':
            print(f'SKIP {name}: {why}')
            continue
        ok = tag == 'PASS'
        passed += ok
        failed += not ok
        print(f'{tag} {name}: {fmt} {w}x{h} {ms:.1f} ms{why}'
              + (' truncated' if trunc == 'true' else ''))
    print(f'{passed} passed, {failed} failed')
    return failed == 0


def check_file(path, raw, w, h, fmt, trunc):
    """Our decode `raw` (w, h, output format, truncated) against the reference: (PASS | FAIL |
    SKIP, detail)."""
    try:
        ref, photometric, alpha, by = reference(path)
        import tifffile
        with tifffile.TiffFile(path) as t:
            lossy = int(t.pages[0].compression) in LOSSY_UPSAMPLED
    except Exception as e:
        return 'SKIP', f'reference: {e}'
    if trunc and by == 'libtiff' and 0 < np.asarray(ref).shape[0] < h:
        # Both stopped early: compare the rows libtiff decoded.
        raw = raw[:len(raw) // h * np.asarray(ref).shape[0]]
        h = np.asarray(ref).shape[0]
    if photometric not in (0, 1, 2, 6, 32844, 32845):
        # Palette, CMYK, Lab, ...: colour conversion, not a codec question.
        return 'SKIP', f'photometric {photometric}'
    try:
        ok, why = check(raw, w, h, fmt, ref, photometric, alpha, lossy)
    except ValueError as e:
        # Layouts the checker does not model (extra non-alpha samples, ...).
        return 'SKIP', f'layout: {e}'
    return ('PASS' if ok else 'FAIL'), f' (vs {by})' + ('' if ok else ': ' + why)


# JPEG-family units whose decoders legitimately differ in chroma upsampling and rounding
# (libtiff's RGBA path replicates chroma; libjpeg/libwebp interpolate): compared by
# statistics, not per sample.
LOSSY_UPSAMPLED = (6, 7, 50001)


def check(raw, w, h, fmt, ref, photometric, alpha, lossy=False):
    ref = np.asarray(ref)
    if ref.ndim == 2:
        ref = ref[..., None]
    if ref.shape[0] != h or ref.shape[1] != w:
        return False, f'size {w}x{h} vs {ref.shape[1]}x{ref.shape[0]}'
    c = ref.shape[2]
    miniswhite = photometric == 0
    if fmt == 'Rgba16F':
        got = half_array(raw, w, h)
        r = ref.astype(np.float32)
        r = np.nan_to_num(r, nan=0.0)
        if c in (1, 2):
            g = 1.0 - r[..., 0] if miniswhite else r[..., 0]
            want = np.stack([g, g, g], -1)
        else:
            want = r[..., :3]
        want = np.clip(want, -65504, 65504)
        err = np.abs(got[..., :3] - want) / np.maximum(np.abs(want), 1e-3)
        bad = (err > 2e-3) & (np.abs(got[..., :3] - want) > 1e-4)
        if alpha:
            a_err = np.abs(got[..., 3] - np.clip(r[..., c - 1], 0, 1)).max()
            if a_err > 1e-3:
                return False, f'alpha error {a_err}'
        if bad.any():
            i = np.argwhere(bad)[0]
            return False, f'{bad.sum()} samples off, first at {tuple(i)}: {got[tuple(i)]} vs {want[tuple(i)]}'
        return True, ''
    if fmt in ('Gray8', 'Gray16'):
        dt = np.uint8 if fmt == 'Gray8' else np.uint16
        got = np.frombuffer(raw, dtype=dt).reshape(h, w)
        want = ref[..., 0]
        if want.dtype == bool:
            want = want.astype(np.uint8) * 255
        if want.dtype.kind == 'i':
            info = np.iinfo(want.dtype)
            want = want.astype(np.int64) - info.min
            want = (want * (np.iinfo(dt).max) / (info.max - info.min) + 0.5).astype(np.int64)
        elif want.dtype.itemsize * 8 > np.dtype(dt).itemsize * 8:
            bits = want.dtype.itemsize * 8
            m = (1 << bits) - 1
            want = (want.astype(np.float64) * np.iinfo(dt).max / m + 0.5).astype(np.int64)
        if miniswhite:
            want = np.iinfo(dt).max - want.astype(np.int64)
        d = np.abs(got.astype(np.int64) - want.astype(np.int64))
        if d.max() > 1:
            i = np.unravel_index(d.argmax(), d.shape)
            return False, f'max error {d.max()} at {i}: {got[i]} vs {want[i]}'
        return True, ''
    if fmt in ('Rgba8', 'Rgba16', 'GrayAlpha16'):
        dt = np.uint8 if fmt == 'Rgba8' else np.uint16
        n = 2 if fmt == 'GrayAlpha16' else 4
        got = np.frombuffer(raw, dtype=dt).reshape(h, w, n).astype(np.int64)
        want = ref.astype(np.int64)
        if ref.dtype.kind == 'i':
            # Signed integers are shown offset to unsigned (as signed grey).
            u = np.dtype(f'u{ref.dtype.itemsize}')
            ref = ref.view(u) ^ u.type(1 << (8 * ref.dtype.itemsize - 1))
            want = ref.astype(np.float64) if ref.dtype.itemsize == 8 else ref.astype(np.int64)
        if ref.dtype.kind == 'u' and ref.dtype.itemsize * 8 > np.dtype(dt).itemsize * 8:
            m = (1 << (ref.dtype.itemsize * 8)) - 1
            top = np.iinfo(dt).max
            want = (want.astype(np.float64) * top / m + 0.5).astype(np.int64)
        if n == 2:
            cmp = got[..., :1]
            want_c = want[..., :1]
        elif c >= 3:
            cmp, want_c = got[..., :3], want[..., :3]
        else:
            cmp, want_c = got[..., :3], np.repeat(want[..., :1], 3, -1)
        if alpha == 'associated' and c >= 2:
            # The decoder divides associated alpha out.
            top = np.iinfo(dt).max
            a = want[..., c - 1:c].astype(np.float64)
            un = np.where(a > 0, want_c * top / np.maximum(a, 1), 0)
            want_c = np.minimum(np.round(un), top).astype(np.int64)
        d = np.abs(cmp - want_c)
        if alpha:
            d = np.maximum(d, np.abs(got[..., n - 1:] - want[..., c - 1:c]))
        if lossy and d.max() > 1:
            mean, p99 = d.mean(), np.percentile(d, 99)
            if mean <= 1.5 and p99 <= 8:
                return True, ''
            return False, f'mean error {mean:.2f}, p99 {p99:.0f}, max {d.max()}'
        if d.max() > 1:
            i = np.unravel_index(d.argmax(), d.shape)
            return False, f'max error {d.max()} at {i}'
        return True, ''
    return False, f'format {fmt} not compared'


if __name__ == '__main__':
    if len(sys.argv) >= 3 and sys.argv[1] == 'generate':
        generate(sys.argv[2])
    elif len(sys.argv) >= 3 and sys.argv[1] == 'variants':
        variants(sys.argv[2])
    elif len(sys.argv) == 8 and sys.argv[1] == 'check':
        # check <file.tif> <raw> <width> <height> <format> <truncated: true|false>
        f, r, w, h, fmt, tr = sys.argv[2:]
        tag, why = check_file(f, open(r, 'rb').read(), int(w), int(h), fmt, tr == 'true')
        print(f'{tag}{why}')
    elif len(sys.argv) >= 4 and sys.argv[1] == 'compare':
        sys.exit(0 if compare(sys.argv[2], sys.argv[3:]) else 1)
    else:
        print(__doc__)
        sys.exit(2)
