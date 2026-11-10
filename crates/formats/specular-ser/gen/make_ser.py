#!/usr/bin/env python3
"""Generates the SER and one-shot-colour FITS corpus (tools/corpus/fetch-astrowcs.sh).

    make_ser.py samples DIR   small SER captures: every colour layout, 8 / 16 bit, both byte
                              orders and the LittleEndian flag quirk, timestamps, a cut file
    make_ser.py cfa DIR       raw one-shot-colour FITS: BAYERPAT / COLORTYP, offsets, ROWORDER
                              (needs astropy)
    make_ser.py big DIR       timing files: a 4 GB SER (sparse: only the first frames and the
                              timestamp trailer are written) and a 60 MP CFA FITS

Every image is a "planet" (a lit disc, colour banded) drifting over a dark sky, so frames
differ, a debayered frame shows red / green / blue bands in the right places, and the 16-bit
byte order is the smoother one. SER layout: Grischa Hahn's SER format description, v3.
"""

import pathlib
import struct
import sys

import numpy as np

TICKS_1970 = 621_355_968_000_000_000  # .NET ticks of 1970-01-01 00:00 UTC
START = TICKS_1970 + 1_785_622_200 * 10_000_000  # 2026-08-01 22:10:00 UTC
COLOUR_IDS = {"MONO": 0, "RGGB": 8, "GRBG": 9, "GBRG": 10, "BGGR": 11, "RGB": 100, "BGR": 101}


def scene(w, h, t, depth):
    """Display-order (top-down) RGB float image in [0, 2^depth - 1]: a banded disc."""
    y, x = np.mgrid[0:h, 0:w].astype(np.float32)
    cx, cy, r = w * (0.4 + 0.02 * t), h * 0.5, min(w, h) * 0.3
    d = np.hypot(x - cx, y - cy) / r
    disc = np.clip(1.2 - d, 0.0, 1.0) ** 0.5
    band = (y - (cy - r)) / (2 * r)
    rgb = np.stack(
        [
            disc * np.clip(1.4 - 2 * band, 0.1, 1.0),
            disc * np.clip(1.0 - abs(band - 0.5) * 2, 0.1, 1.0),
            disc * np.clip(2 * band - 0.4, 0.1, 1.0),
        ],
        axis=-1,
    )
    top = 2**depth - 1
    return (0.03 + 0.9 * rgb) * top


def mosaic(rgb, pattern):
    """The raw CFA frame of `rgb` (display order) under `pattern` ("RGGB", top row first)."""
    h, w, _ = rgb.shape
    out = np.empty((h, w), np.float32)
    idx = {"R": 0, "G": 1, "B": 2}
    for i, c in enumerate(pattern):
        out[i // 2 :: 2, i % 2 :: 2] = rgb[i // 2 :: 2, i % 2 :: 2, idx[c]]
    return out


def frame(w, h, t, depth, layout):
    rgb = scene(w, h, t, depth)
    if layout == "MONO":
        f = rgb.mean(axis=-1)
    elif layout in ("RGB", "BGR"):
        f = rgb if layout == "RGB" else rgb[..., ::-1]
    else:
        f = mosaic(rgb, layout)
    return np.round(f).astype(np.uint16 if depth > 8 else np.uint8)


def text40(s):
    return s.encode("latin-1")[:40].ljust(40, b"\0")


def header(layout, flag, w, h, depth, frames):
    return (
        b"LUCAM-RECORDER"
        + struct.pack("<7i", 0, COLOUR_IDS[layout], flag, w, h, depth, frames)
        + text40("Specular test corpus")
        + text40("ZWO ASI462MC")
        + text40("C14 EdgeHD")
        + struct.pack("<qq", START + 2 * 36_000_000_000, START)
    )


def frame_bytes(f, big_endian):
    if f.dtype == np.uint16:
        return f.astype(">u2" if big_endian else "<u2").tobytes()
    return f.tobytes()


def write_ser(path, layout, w, h, depth, n, flag=0, big_endian=False, stamps=True,
              declared=None, keep=None, fps=100.0):
    data = bytearray(header(layout, flag, w, h, depth, declared or n))
    for t in range(n):
        data += frame_bytes(frame(w, h, t, depth, layout), big_endian)
    if stamps:
        step = int(1e7 / fps)
        data += b"".join(struct.pack("<q", START + t * step) for t in range(n))
    if keep is not None:
        data = data[:keep]
    path.write_bytes(bytes(data))


def samples(out):
    out.mkdir(parents=True, exist_ok=True)
    w, h = 96, 64
    write_ser(out / "mono8.ser", "MONO", w, h, 8, 12)
    write_ser(out / "mono12-le-flag0.ser", "MONO", w, h, 12, 12, flag=0)
    write_ser(out / "mono16-le-flag1.ser", "MONO", w, h, 16, 12, flag=1)
    write_ser(out / "mono16-be-flag1.ser", "MONO", w, h, 16, 12, flag=1, big_endian=True)
    write_ser(out / "mono8-no-stamps.ser", "MONO", w, h, 8, 8, stamps=False)
    for pat in ("RGGB", "GRBG", "GBRG", "BGGR"):
        write_ser(out / f"bayer-{pat.lower()}8.ser", pat, w, h, 8, 10)
    write_ser(out / "bayer-rggb16.ser", "RGGB", w, h, 16, 10)
    write_ser(out / "bayer-gbrg-odd.ser", "GBRG", 95, 63, 8, 6)
    write_ser(out / "rgb8.ser", "RGB", w, h, 8, 8)
    write_ser(out / "bgr16.ser", "BGR", w, h, 16, 8)
    # 20 frames declared, 13 and a half present (a capture stopped by a full disk).
    fb = w * h
    write_ser(out / "mono8-cut.ser", "MONO", w, h, 8, 20, stamps=False, keep=178 + fb * 27 // 2)


def shift(p, dx, dy):
    return "".join(p[((i & 1) + dx) % 2 + 2 * ((i // 2 + dy) % 2)] for i in range(4))


def write_cfa(path, w, h, depth, shown, roworder="TOP-DOWN", xoff=0, yoff=0, key="BAYERPAT"):
    """A raw frame that, read as the header says, shows the pattern `shown` at the top-left."""
    from astropy.io import fits

    raw = frame(w, h, 0, depth, shown).astype(np.uint16)
    # The keyword names the pattern at the first stored row (TOP-DOWN: FITS row 1, the bottom
    # row as shown; BOTTOM-UP: the top row as shown), before the offsets.
    first = shown if roworder == "BOTTOM-UP" else shift(shown, 0, (h - 1) % 2)
    stored = np.flipud(raw)
    hdu = fits.PrimaryHDU(stored)
    hdu.header[key] = shift(first, xoff, yoff)
    if xoff or yoff:
        hdu.header["XBAYROFF"] = xoff
        hdu.header["YBAYROFF"] = yoff
    hdu.header["ROWORDER"] = roworder
    hdu.header["OBJECT"] = "Synthetic planet"
    hdu.header["INSTRUME"] = "ZWO ASI2600MC Pro"
    hdu.header["EXPTIME"] = 0.01
    hdu.writeto(path, overwrite=True)


def cfa(out):
    out.mkdir(parents=True, exist_ok=True)
    write_cfa(out / "rggb-topdown.fits", 160, 120, 14, "RGGB")
    write_cfa(out / "rggb-bottomup.fits", 160, 120, 14, "RGGB", roworder="BOTTOM-UP")
    write_cfa(out / "gbrg-odd-topdown.fits", 161, 121, 16, "GBRG")
    write_cfa(out / "bggr-offset.fits", 160, 120, 12, "BGGR", xoff=1, yoff=1)
    write_cfa(out / "grbg-colortyp.fits", 160, 120, 16, "GRBG", key="COLORTYP")


def big(out):
    out.mkdir(parents=True, exist_ok=True)
    # 4 GB of 1920x1080 16-bit RGGB frames; only the first 4 hold data, the rest is a hole.
    w, h, n = 1920, 1080, 1036
    fb = w * h * 2
    path = out / "capture-4g.ser"
    with open(path, "wb") as f:
        f.write(header("RGGB", 0, w, h, 16, n))
        for t in range(4):
            f.write(frame_bytes(frame(w, h, t, 12, "RGGB"), False))
        f.seek(178 + fb * n)
        f.write(b"".join(struct.pack("<q", START + t * 100_000) for t in range(n)))
    # 60 MP (9504x6336) 14-bit RGGB, as a full-frame one-shot-colour camera writes it.
    write_cfa(out / "cfa-60mp.fits", 9504, 6336, 14, "RGGB")


def main():
    if len(sys.argv) != 3 or sys.argv[1] not in ("samples", "cfa", "big"):
        sys.exit(__doc__)
    {"samples": samples, "cfa": cfa, "big": big}[sys.argv[1]](pathlib.Path(sys.argv[2]))


if __name__ == "__main__":
    main()
