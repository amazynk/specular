"""OpenImageIO's reading of a DPX / Cineon file as a 16-bit PAM (the oracle for
fetch-raster-film.sh). Integer files: the stored values as OpenImageIO scales them to 16 bits
(no colour conversion, alpha as stored). Float files: clamped to 0..1.

Signed 8- and 16-bit samples are written as their two's-complement bits (the stored codes),
and OpenImageIO's float reading of them (−1 to 1, clamped to 0..1) beside OUT as
<name>.oiiof.pam. A DPX whose single-channel image elements (subimages) are R, G and B (and
A) of one type is written as one image of those channels, as a reader combining them shows it.

Usage: python -I raster-film-oiio.py IN OUT.pam
"""
import sys

import numpy as np
import OpenImageIO as oiio


def write(dst, px):
    h, w, c = px.shape
    tupl = {1: "GRAYSCALE", 2: "GRAYSCALE_ALPHA", 3: "RGB", 4: "RGB_ALPHA"}.get(c, "RGB")
    with open(dst, "wb") as f:
        f.write(
            f"P7\nWIDTH {w}\nHEIGHT {h}\nDEPTH {c}\nMAXVAL 65535\nTUPLTYPE {tupl}\nENDHDR\n".encode()
        )
        f.write(px.astype(">u2").tobytes())


def combined(src):
    """The R, G, B (A) single-channel subimages stacked, or None."""
    found = {}
    fmt = None
    i = 0
    while True:
        buf = oiio.ImageBuf(src, i, 0)
        if buf.has_error:
            buf.geterror()
            break
        spec = buf.spec()
        if spec.nchannels == 1:
            name = spec.channelnames[0]
            if name in "RGBA" and name not in found:
                if fmt is not None and spec.format != fmt:
                    return None
                fmt = spec.format
                found[name] = buf.get_pixels(oiio.UINT16)
        i += 1
    if i < 2 or not all(k in found for k in "RGB"):
        return None
    chans = [found[k] for k in "RGBA" if k in found]
    return np.concatenate(chans, axis=-1)


def main():
    src, dst = sys.argv[1], sys.argv[2]
    stacked = combined(src)
    if stacked is not None:
        write(dst, stacked)
        return
    buf = oiio.ImageBuf(src)
    if buf.has_error:
        sys.exit(1)
    spec = buf.spec()
    if spec.format.basetype in (oiio.FLOAT, oiio.DOUBLE, oiio.HALF):
        px = buf.get_pixels(oiio.FLOAT)
        px = (np.clip(np.nan_to_num(px), 0.0, 1.0) * 65535.0 + 0.5).astype(np.uint16)
    elif spec.format.basetype in (oiio.INT8, oiio.INT16):
        bits = spec.get_int_attribute("oiio:BitsPerSample", 0)
        if bits not in (8, 16):
            # OpenImageIO widens other signed depths in its own way; no code oracle.
            sys.exit(1)
        raw = buf.get_pixels(spec.format)
        if raw is None:
            sys.exit(1)
        px = raw.view(np.uint8 if bits == 8 else np.uint16).astype(np.uint16)
        if bits == 8:
            px = px << 8
        f = buf.get_pixels(oiio.FLOAT)
        f = (np.clip(np.nan_to_num(f), 0.0, 1.0) * 65535.0 + 0.5).astype(np.uint16)
        write(dst.replace(".oiio.pam", ".oiiof.pam"), f)
    else:
        px = buf.get_pixels(oiio.UINT16)
    if px is None or buf.has_error:
        sys.exit(1)
    write(dst, px)


main()
