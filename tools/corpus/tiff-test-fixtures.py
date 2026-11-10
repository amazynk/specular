"""tiff-test-fixtures.py <dir>: the tiny compression fixtures of specular-tiff's tests/compression.rs
tiff-test-fixtures.py --variants <dir>: the layout fixtures of tests/variants.rs

(written to crates/formats/specular-tiff/tests/fixtures/compression and .../variants; needs
numpy, tifffile, imagecodecs and Pillow, e.g. the oracle venv; run with python -I).

Pixel values follow formulas the Rust test recomputes (keep them in sync):
  u8(x, y, c)  = (x*5 + y*3 + c*60 + (x*y) % 7) % 256
  u16(x, y, c) = (u8(x, y, c)*251 + x*17 + y) % 65536
  f(x, y, c)   = float32(x*0.05 + y*0.07 + c*0.5 - 0.3)
  bw(x, y)     = (x//3 + y//2) % 5 == 0 or (x*y) % 11 == 0   (True = white)
"""
import lzma
import os
import sys

import imagecodecs
import numpy as np
import tifffile
from PIL import Image

W, H = 37, 23


def grid(c):
    y, x = np.mgrid[0:H, 0:W].astype(np.int64)
    return [(x, y, k) for k in range(c)]


def u8(c):
    p = [(x * 5 + y * 3 + k * 60 + (x * y) % 7) % 256 for x, y, k in grid(c)]
    return (np.stack(p, -1) if c > 1 else p[0]).astype('u1')


def u16(c):
    a = u8(c).astype(np.int64)
    y, x = np.mgrid[0:H, 0:W].astype(np.int64)
    if c > 1:
        x, y = x[..., None], y[..., None]
    return ((a * 251 + x * 17 + y) % 65536).astype('u2')


def f32(c):
    p = [(x * 0.05 + y * 0.07 + k * 0.5 - 0.3) for x, y, k in grid(c)]
    return (np.stack(p, -1) if c > 1 else p[0]).astype('f4')


def tiles(a, t):
    """Padded t×t tiles of `a`, row-major."""
    for ty in range(0, a.shape[0], t):
        for tx in range(0, a.shape[1], t):
            tile = np.zeros((t, t) + a.shape[2:], a.dtype)
            part = a[ty:ty + t, tx:tx + t]
            tile[:part.shape[0], :part.shape[1]] = part
            yield tile


def main(out):
    os.makedirs(out, exist_ok=True)

    def w(name, data, **kw):
        tifffile.imwrite(os.path.join(out, name), data, **kw)

    w('zstd-u16-pred2-tiled.tif', u16(1), compression='zstd', predictor=2, tile=(16, 16))
    w('lzma-u8-gray.tif', u8(1), compression='lzma', rowsperstrip=8)
    # libtiff's LZMA: xz with a Delta filter (distance = samples per pixel) before LZMA2.
    rgb = u8(3)
    filters = [{'id': lzma.FILTER_DELTA, 'dist': 3}, {'id': lzma.FILTER_LZMA2, 'preset': 6}]
    xz = (lzma.compress(t.tobytes(), format=lzma.FORMAT_XZ, filters=filters)
          for t in tiles(rgb, 16))
    tifffile.imwrite(os.path.join(out, 'lzma-delta-u8-rgb.tif'), xz, shape=rgb.shape, dtype='u1',
                     tile=(16, 16), compression=34925, photometric='rgb')
    w('deflate-f4-rgb-pred3-be.tif', f32(3), compression='zlib', predictor=3, byteorder='>',
      photometric='rgb', rowsperstrip=5)
    w('deflate-f2-gray-pred3-le.tif', f32(1).astype('f2'), compression='zlib', predictor=3,
      byteorder='<', tile=(16, 16))
    w('lerc-u2-tiled.tif', u16(1), compression='lerc', tile=(16, 16))
    w('lerc-v5-u2-rgb.tif', u16(3), compression='lerc', photometric='rgb',
      compressionargs={'version': 5})
    w('lerc-u1-rgb-deflate.tif', u8(3), compression='lerc', photometric='rgb',
      compressionargs={'compression': 'deflate'})
    nan = f32(1)
    nan[3:9, 4:20] = np.nan
    w('lerc-f4-nan-zstd.tif', nan, compression='lerc', compressionargs={'compression': 'zstd'})
    rgba = np.concatenate([u8(3), u8(1)[..., None] | 0x80], -1)
    w('webp-lossless-rgba.tif', rgba, compression='webp', tile=(16, 16), photometric='rgb',
      extrasamples=['unassalpha'], compressionargs={'lossless': True})
    w('webp-lossy-rgb.tif', u8(3), compression='webp', tile=(16, 16),
      compressionargs={'level': 95})
    w('jxl-lossless-rgb.tif', u8(3), compression='jpegxl', tile=(16, 16),
      compressionargs={'lossless': True})
    w('j2k-lossless-u16-gray.tif', u16(1), compression='jpeg2000', compressionargs={'level': 0})
    # Aperio 33003: YCbCr codestreams without the colour transform.
    a = u8(3).astype(np.float64)
    r, g, b = a[..., 0], a[..., 1], a[..., 2]
    yy = 0.299 * r + 0.587 * g + 0.114 * b
    ycc = np.stack([yy, (b - yy) / 1.772 + 128, (r - yy) / 1.402 + 128], -1)
    ycc = np.round(ycc).clip(0, 255).astype('u1')
    j2k = (imagecodecs.jpeg2k_encode(t, level=0, codecformat='J2K', mct=False)
           for t in tiles(ycc, 16))
    tifffile.imwrite(os.path.join(out, 'aperio-33003.tif'), j2k, shape=ycc.shape, dtype='u1',
                     tile=(16, 16), compression=33003, photometric='rgb')
    y, x = np.mgrid[0:H, 0:W]
    bw = ((x // 3 + y // 2) % 5 == 0) | ((x * y) % 11 == 0)
    img = Image.fromarray(bw)
    for comp in ('group4', 'group3', 'tiff_ccitt'):
        img.save(os.path.join(out, f'ccitt-{comp}.tif'), compression=comp)
    for f in sorted(os.listdir(out)):
        print(f, os.path.getsize(os.path.join(out, f)))


def variants(out):
    """Rarer layouts: wide and odd integers, subsampled YCbCr, old-style LZW, LogLuv and
    LERC 6 lossless float. Writers from tiff-oracle.py."""
    import importlib.util
    spec = importlib.util.spec_from_file_location(
        'tiff_oracle', os.path.join(os.path.dirname(os.path.abspath(__file__)), 'tiff-oracle.py'))
    o = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(o)
    os.makedirs(out, exist_ok=True)
    p = lambda n: os.path.join(out, n)
    # 24-bit little-endian grey: u16(x, y, 0) * 256 + u8(x, y, 0).
    v24 = u16(1).astype('<u4') * 256 + u8(1)
    b = v24.astype('<u4').view('u1').reshape(H, W, 4)[..., :3]
    o.write_tiff(p('int24-le-gray.tif'), W, H, 24, 1, [b.tobytes()], 1)
    # 17-bit big-endian RGB, packed: u16(x, y, c) * 2 + (x & 1).
    x = np.mgrid[0:H, 0:W][1][..., None]
    v17 = u16(3).astype(np.int64) * 2 + (x & 1)
    o.write_tiff(p('int17-be-rgb.tif'), W, H, 17, 3, [o.pack_bits(v17, 17)], 2, le=False)
    # 64-bit signed grey: (u16(x, y, 0) - 32768) << 40.
    v64 = (u16(1).astype(np.int64) - 32768) << 40
    tifffile.imwrite(p('int64-signed-gray.tif'), v64)
    # YCbCr 4:2:2 (2×1) and 4:2:0 (2×2), uncompressed, 8 rows per strip.
    rgb = u8(3)
    for hs, vs in ((2, 1), (2, 2)):
        strips = [o.ycbcr_blocks(rgb[y0:y0 + 8], hs, vs) for y0 in range(0, H, 8)]
        o.write_tiff(p(f'ycbcr-{hs}{vs}.tif'), W, H, 8, 3, strips, 6, rows=8,
                     extra=[(530, 3, [hs, vs])])
    # Old-style LZW, RGB, 8 rows per strip.
    strips = [o.lzw_old(rgb[y0:y0 + 8].tobytes()) for y0 in range(0, H, 8)]
    o.write_tiff(p('lzw-old-rgb.tif'), W, H, 8, 3, strips, 2, compression=5, rows=8)
    # LogLuv32 through libtiff: XYZ of the linear RGB f(x, y, c) + 1.
    lin = f32(3).astype(np.float64) + 1.0
    o.logluv_write(p('logluv32.tif'), lin @ np.linalg.inv(o.xyz_e_to_rgb()).T)
    # LogLuv24 through libtiff: the same XYZ.
    o.logluv_write(p('logluv24.tif'), lin @ np.linalg.inv(o.xyz_e_to_rgb()).T, 34677)
    # LERC 6 lossless float (FPL): f(x, y, 0) as float32, and f(x, y, c) as float64 RGB.
    tifffile.imwrite(p('lerc-fpl-f4.tif'), f32(1), compression='lerc',
                     compressionargs={'version': 6, 'level': 0.0})
    tifffile.imwrite(p('lerc-fpl-f8-rgb.tif'), f32(3).astype('f8'), compression='lerc',
                     photometric='rgb', compressionargs={'version': 6, 'level': 0.0})
    for f in sorted(os.listdir(out)):
        print(f, os.path.getsize(os.path.join(out, f)))


if sys.argv[1] == '--variants':
    variants(sys.argv[2])
else:
    main(sys.argv[1])
