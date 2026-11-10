#!/usr/bin/env python3
"""Fixtures and libtiff references for specular-tiff's NeXT, PixarLog and JBIG paths
(crates/formats/specular-tiff/tests/gaps.rs).

    tiff-gaps.py generate DIR     write PixarLog (8/16-bit, float; contiguous and planar;
                                  strips and 16×16 tiles)
                                  and JBIG fixtures with the system libtiff (ctypes); for
                                  PixarLog also NAME.src, the source samples as linear float
    tiff-gaps.py reference DIR    for every DIR/*.tif: NAME.rgba (TIFFReadRGBAImageOriented,
                                  top-left, 8-bit RGBA) and NAME.raw + NAME.txt (the strips
                                  as TIFFReadEncodedStrip returns them, and "w h spp bits
                                  sampleformat planar rowsperstrip")

The system libtiff (libtiff.so.6, built with JBIG-KIT and PixarLog) is the oracle; it runs
as a separate tool process, never linked into Specular. Needs numpy. Nothing here touches the
network (tools/corpus/fetch-tiff-gaps.sh fetches the NeXT samples).
"""

import ctypes
import ctypes.util
import os
import sys

import numpy as np

T_WIDTH, T_LENGTH, T_BPS, T_COMPRESSION, T_PHOTOMETRIC = 256, 257, 258, 259, 262
T_SPP, T_RPS, T_PLANAR, T_SAMPLEFORMAT = 277, 278, 284, 339
T_PIXARLOGDATAFMT, T_PIXARLOGQUALITY = 65549, 65558
T_TILEWIDTH, T_TILELENGTH = 322, 323


def lib():
    name = ctypes.util.find_library("tiff") or "libtiff.so.6"
    t = ctypes.CDLL(name)
    t.TIFFOpen.restype = ctypes.c_void_p
    t.TIFFOpen.argtypes = [ctypes.c_char_p, ctypes.c_char_p]
    t.TIFFClose.argtypes = [ctypes.c_void_p]
    t.TIFFWriteEncodedStrip.argtypes = [ctypes.c_void_p, ctypes.c_uint32, ctypes.c_void_p,
                                        ctypes.c_ssize_t]
    t.TIFFWriteEncodedStrip.restype = ctypes.c_ssize_t
    t.TIFFReadEncodedStrip.argtypes = [ctypes.c_void_p, ctypes.c_uint32, ctypes.c_void_p,
                                       ctypes.c_ssize_t]
    t.TIFFReadEncodedStrip.restype = ctypes.c_ssize_t
    t.TIFFNumberOfStrips.argtypes = [ctypes.c_void_p]
    t.TIFFNumberOfStrips.restype = ctypes.c_uint32
    t.TIFFStripSize.argtypes = [ctypes.c_void_p]
    t.TIFFStripSize.restype = ctypes.c_ssize_t
    t.TIFFReadRGBAImageOriented.argtypes = [ctypes.c_void_p, ctypes.c_uint32, ctypes.c_uint32,
                                            ctypes.c_void_p, ctypes.c_int, ctypes.c_int]
    t.TIFFGetField.restype = ctypes.c_int
    t.TIFFIsCODECConfigured.argtypes = [ctypes.c_uint16]
    for f in ("TIFFWriteEncodedTile", "TIFFReadEncodedTile"):
        getattr(t, f).argtypes = [ctypes.c_void_p, ctypes.c_uint32, ctypes.c_void_p,
                                  ctypes.c_ssize_t]
        getattr(t, f).restype = ctypes.c_ssize_t
    t.TIFFIsTiled.argtypes = [ctypes.c_void_p]
    t.TIFFTileSize.argtypes = [ctypes.c_void_p]
    t.TIFFTileSize.restype = ctypes.c_ssize_t
    return t


def write(t, path, a, bits, compression, photometric, planar=False, rps=7, fmt=1, extra=(),
          tile=None):
    """`a`: (h, w) or (h, w, spp) samples of the stored type; `extra`: ExtraSamples values;
    `tile`: (width, height) to write tiles (contiguous only) instead of strips."""
    if not t.TIFFIsCODECConfigured(compression):
        raise RuntimeError(f"libtiff lacks compression {compression}")
    h, w = a.shape[:2]
    spp = 1 if a.ndim == 2 else a.shape[2]
    tif = t.TIFFOpen(path.encode(), b"w")
    if not tif:
        raise RuntimeError("TIFFOpen failed")
    u = ctypes.c_int
    # The codec first: PixarLog's data format (8/16-bit, float) then sets BitsPerSample.
    tags = [(T_COMPRESSION, compression)]
    if compression == 32909:
        tags.append((T_PIXARLOGDATAFMT, {8: 0, 16: 4, 32: 5}[bits]))
    tags += [(T_WIDTH, w), (T_LENGTH, h), (T_BPS, bits), (T_SPP, spp),
             (T_PHOTOMETRIC, photometric), (T_PLANAR, 2 if planar else 1),
             (T_SAMPLEFORMAT, fmt)]
    # Tiles: libtiff's PixarLog sizes its buffer from RowsPerStrip even then.
    tags += [(T_RPS, tile[1] if tile else rps)]
    tags += [(T_TILEWIDTH, tile[0]), (T_TILELENGTH, tile[1])] if tile else []
    for tag, v in tags:
        t.TIFFSetField(ctypes.c_void_p(tif), ctypes.c_uint32(tag), u(v))
    if extra:
        # ExtraSamples: a count and an array of uint16.
        arr = (ctypes.c_uint16 * len(extra))(*extra)
        t.TIFFSetField(ctypes.c_void_p(tif), ctypes.c_uint32(338), u(len(extra)), arr)
    if tile:
        tw, th = tile
        n = 0
        for y0 in range(0, h, th):
            for x0 in range(0, w, tw):
                blk = np.zeros((th, tw) + a.shape[2:], a.dtype)
                part = a[y0:y0 + th, x0:x0 + tw]
                blk[:part.shape[0], :part.shape[1]] = part
                buf = blk.tobytes()
                if t.TIFFWriteEncodedTile(ctypes.c_void_p(tif), n, buf, len(buf)) < 0:
                    raise RuntimeError("TIFFWriteEncodedTile failed")
                n += 1
    strip = 0
    planes = [a[..., k] for k in range(spp)] if planar and spp > 1 else [a]
    if tile:
        planes = []
    for p in planes:
        for y0 in range(0, h, rps):
            if bits == 1:
                rows = p[y0:y0 + rps]
                buf = np.packbits(rows.astype(np.uint8), axis=1).tobytes()
            else:
                buf = np.ascontiguousarray(p[y0:y0 + rps]).tobytes()
            if t.TIFFWriteEncodedStrip(ctypes.c_void_p(tif), strip, buf, len(buf)) < 0:
                raise RuntimeError("TIFFWriteEncodedStrip failed")
            strip += 1
    t.TIFFClose(ctypes.c_void_p(tif))
    if compression == 32909:
        # The source as linear light (what PixarLog codes), interleaved, native float32.
        scale = {8: 255.0, 16: 65535.0, 32: 1.0}[bits]
        (a.astype(np.float64) / scale).astype(np.float32).tofile(path[:-4] + ".src")
    print(path)


def gradient(h, w, c, scale, seed):
    rng = np.random.default_rng(seed)
    y, x = np.mgrid[0:h, 0:w].astype(np.float64)
    planes = [0.5 + 0.4 * np.sin(x / (9 + 4 * k) + y / (13 + 2 * k)) + 0.05 * rng.random((h, w))
              for k in range(c)]
    v = np.stack(planes, -1) if c > 1 else planes[0]
    return v * scale


def generate(out):
    os.makedirs(out, exist_ok=True)
    t = lib()
    j = lambda n: os.path.join(out, n)
    pl = 32909
    write(t, j("pixarlog_gray8.tif"), gradient(37, 53, 1, 255, 1).astype(np.uint8), 8, pl, 1)
    write(t, j("pixarlog_rgb8.tif"), gradient(31, 47, 3, 255, 2).astype(np.uint8), 8, pl, 2)
    write(t, j("pixarlog_rgba8.tif"), gradient(19, 23, 4, 255, 6).astype(np.uint8), 8, pl, 2,
          extra=(2,))
    write(t, j("pixarlog_rgb8_planar.tif"), gradient(29, 41, 3, 255, 3).astype(np.uint8), 8,
          pl, 2, planar=True)
    write(t, j("pixarlog_gray16.tif"), gradient(33, 45, 1, 65535, 4).astype(np.uint16), 16, pl,
          1)
    write(t, j("pixarlog_rgb16.tif"), gradient(21, 30, 3, 65535, 7).astype(np.uint16), 16, pl,
          2)
    write(t, j("pixarlog_float.tif"), gradient(27, 39, 1, 3.0, 5).astype(np.float32), 32, pl,
          1, fmt=3)
    write(t, j("pixarlog_rgb_float.tif"), gradient(17, 25, 3, 2.0, 8).astype(np.float32), 32,
          pl, 2, fmt=3)
    write(t, j("pixarlog_rgb16_tiled.tif"), gradient(37, 45, 3, 65535, 9).astype(np.uint16), 16,
          pl, 2, tile=(16, 16))
    rng = np.random.default_rng(10)
    y, x = np.mgrid[0:90, 0:133]
    bi = ((x // 7 + y // 5) % 2 == 0) ^ (rng.random((90, 133)) < 0.02)
    write(t, j("jbig_miniswhite.tif"), bi, 1, 34661, 0, rps=90)
    write(t, j("jbig_minisblack.tif"), bi[:40, :71], 1, 34661, 1, rps=40)
    # 16-bit lossless JPEG (SOF3) through tifffile + imagecodecs (libjpeg-turbo 3), when
    # installed; NAME.u16 holds the source samples (interleaved, native order).
    try:
        import tifffile
    except ImportError:
        return
    g16 = gradient(45, 61, 1, 65535, 11).astype(np.uint16)
    rgb16 = gradient(33, 40, 3, 65535, 12).astype(np.uint16)
    lossless = {"lossless": True, "bitspersample": 16}
    # RGB without the colour transform (lossless JPEG has none): outcolorspace "rgb".
    rgbargs = dict(lossless, outcolorspace="rgb")
    for name, a, kw in (
        ("ljpeg16_gray", g16, {"rowsperstrip": 13}),
        ("ljpeg16_gray_tiled", g16, {"tile": (16, 32)}),
        ("ljpeg16_rgb", rgb16, {"photometric": "rgb", "rowsperstrip": 13}),
    ):
        data = a.transpose(2, 0, 1).copy() if kw.get("planarconfig") == "separate" else a
        args = rgbargs if a.ndim == 3 else lossless
        tifffile.imwrite(j(f"{name}.tif"), data, compression="jpeg", compressionargs=args, **kw)
        a.tofile(j(f"{name}.u16"))
        print(j(f"{name}.tif"))


def reference(d):
    t = lib()
    for name in sorted(os.listdir(d)):
        if not name.endswith(".tif"):
            continue
        p = os.path.join(d, name)
        stem = p[:-4]
        tif = t.TIFFOpen(p.encode(), b"r")
        if not tif:
            print(f"{name}: libtiff cannot open")
            continue
        tf = ctypes.c_void_p(tif)

        def field16(tag, default):
            v = ctypes.c_uint16(default)
            return v.value if t.TIFFGetField(tf, ctypes.c_uint32(tag), ctypes.byref(v)) else default

        def field32(tag):
            v = ctypes.c_uint32(0)
            t.TIFFGetField(tf, ctypes.c_uint32(tag), ctypes.byref(v))
            return v.value

        w, h = field32(T_WIDTH), field32(T_LENGTH)
        bits, spp = field16(T_BPS, 1), field16(T_SPP, 1)
        fmt, planar = field16(T_SAMPLEFORMAT, 1), field16(T_PLANAR, 1)
        rps = field32(T_RPS) or h
        ss = t.TIFFStripSize(tf)
        raw = bytearray()
        buf = ctypes.create_string_buffer(ss)
        for s in range(t.TIFFNumberOfStrips(tf)):
            n = t.TIFFReadEncodedStrip(tf, s, buf, ss)
            raw += buf.raw[:max(n, 0)]
        with open(stem + ".raw", "wb") as f:
            f.write(raw)
        with open(stem + ".txt", "w") as f:
            f.write(f"{w} {h} {spp} {bits} {fmt} {planar} {rps}\n")
        rgba = (ctypes.c_uint32 * (w * h))()
        if t.TIFFReadRGBAImageOriented(tf, w, h, rgba, 1, 0):
            with open(stem + ".rgba", "wb") as f:
                f.write(bytes(rgba))
        t.TIFFClose(tf)
        if name.startswith("pixarlog"):
            # The same strips as linear floats (PIXARLOGDATAFMT_FLOAT), native byte order.
            tif = t.TIFFOpen(p.encode(), b"r")
            tf = ctypes.c_void_p(tif)
            t.TIFFSetField(tf, ctypes.c_uint32(T_PIXARLOGDATAFMT), ctypes.c_int(5))
            if t.TIFFIsTiled(tf):
                # Tiles assembled into the image, interleaved.
                tw, th = field32(T_TILEWIDTH), field32(T_TILELENGTH)
                ts = t.TIFFTileSize(tf)
                buf = ctypes.create_string_buffer(ts)
                img = np.zeros((h, w, spp), np.float32)
                n = 0
                for y0 in range(0, h, th):
                    for x0 in range(0, w, tw):
                        t.TIFFReadEncodedTile(tf, n, buf, ts)
                        blk = np.frombuffer(buf.raw, np.float32).reshape(th, tw, spp)
                        ph, pw = min(th, h - y0), min(tw, w - x0)
                        img[y0:y0 + ph, x0:x0 + pw] = blk[:ph, :pw]
                        n += 1
                t.TIFFClose(tf)
                img.tofile(stem + ".float")
                print(f"{name}: {w}x{h} tiled {tw}x{th}")
                continue
            ss = t.TIFFStripSize(tf)
            buf = ctypes.create_string_buffer(ss)
            raw = bytearray()
            for s in range(t.TIFFNumberOfStrips(tf)):
                n = t.TIFFReadEncodedStrip(tf, s, buf, ss)
                raw += buf.raw[:max(n, 0)]
            t.TIFFClose(tf)
            with open(stem + ".float", "wb") as f:
                f.write(raw)
        print(f"{name}: {w}x{h} spp {spp} bits {bits}, {len(raw)} raw bytes")


if __name__ == "__main__":
    {"generate": generate, "reference": reference}[sys.argv[1]](sys.argv[2])
