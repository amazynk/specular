"""HDR JPEG XL fixtures without a gain map, for specular-jxl and specular-decode.

    python3 -I gen_hdr.py OUT_DIR

Bare codestreams (or the level-10 container libjxl wraps some in), made with libjxl
(libjxl.so.0.11 through ctypes, reusing the bindings of gen_gainmap.py). All are 32x16.
`lin(x, y)` below is linear light in units of SDR reference white (203 cd/m2 for PQ):

    lin(x, y) = (0.05 + x / 31 * 3.95 * (1 - y / 30), 0.02 + y / 15 * 2.0, 0.5 + (x % 4) * 0.25)

hdr_pq_p3.jxl          16-bit lossless, PQ, Display P3: round(pq(lin * 203) * 65535).
hdr_hlg_2020_alpha.jxl 16-bit lossless, HLG, BT.2020, 16-bit straight alpha
                       alpha(x, y) = x * 2113 + y; the colour is round(hlg(lin / 4) * 65535)
                       (the HLG OETF of scene light, 1.0 = its peak).
hdr_pq_709_lossy.jxl   VarDCT (distance 1, XYB) from the PQ BT.709 16-bit signal of lin.
linear_f32.jxl         lossless 32-bit float, linear, sRGB primaries: lin (up to 4.0),
                       intensity target left at its default (255 cd/m2).
linear_f16_2020_lossy.jxl  VarDCT (distance 0.2, XYB) from half floats, linear BT.2020: lin / 4,
                       intensity target 812 cd/m2 (1.0 = four times reference white; the
                       encoder clips above 1.0, the intensity target being the peak).

jxl_dec.rs and tests/hdr.rs compute lin; keep them in step.
"""

import ctypes
import math
import os
import struct
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import gen_gainmap as g  # noqa: E402

W, H = 32, 16
lib = g.lib
lib.JxlEncoderSetFrameDistance.argtypes = [ctypes.c_void_p, ctypes.c_float]

TYPE_FLOAT, TYPE_UINT16, TYPE_FLOAT16, LITTLE_ENDIAN = 0, 3, 5, 1
PRIM_SRGB, PRIM_2100, PRIM_P3 = 1, 9, 11
TF_LINEAR, TF_PQ, TF_HLG = 8, 16, 18


def lin(x, y):
    return (
        0.05 + x / 31 * 3.95 * (1 - y / 30),
        0.02 + y / 15 * 2.0,
        0.5 + (x % 4) * 0.25,
    )


def hlg_oetf(e):
    a, b = 0.17883277, 1 - 4 * 0.17883277
    c = 0.5 - a * math.log(4 * a)
    e = max(e, 0.0)
    return math.sqrt(3 * e) if e <= 1 / 12 else a * math.log(12 * e - b) + c


def encode(channels, prim, tf, kind, samples, lossy=None, intensity=None):
    """`kind`: "u16" or "f32" (lossless) or "f16" (half input). `channels` 3 or 4 (alpha
    of the same kind)."""
    enc = lib.JxlEncoderCreate(None)
    assert lib.JxlEncoderUseContainer(enc, 0) == 0
    info = g.BasicInfo()
    lib.JxlEncoderInitBasicInfo(ctypes.byref(info))
    info.xsize, info.ysize = W, H
    bits, exp = {"u16": (16, 0), "f32": (32, 8), "f16": (16, 5)}[kind]
    info.bits_per_sample, info.exponent_bits_per_sample = bits, exp
    info.num_color_channels = 3
    if channels == 4:
        info.num_extra_channels = 1
        info.alpha_bits, info.alpha_exponent_bits = bits, exp
    info.uses_original_profile = 0 if lossy else 1
    if intensity:
        info.intensity_target = intensity
    assert lib.JxlEncoderSetBasicInfo(enc, ctypes.byref(info)) == 0
    ce = g.ColorEncoding()
    lib.JxlColorEncodingSetToSRGB(ctypes.byref(ce), 0)
    ce.primaries, ce.transfer_function = prim, tf
    assert lib.JxlEncoderSetColorEncoding(enc, ctypes.byref(ce)) == 0
    fs = lib.JxlEncoderFrameSettingsCreate(enc, None)
    if lossy:
        assert lib.JxlEncoderSetFrameDistance(fs, lossy) == 0
    else:
        assert lib.JxlEncoderSetFrameLossless(fs, 1) == 0
    fmt_char, dtype = {"u16": ("H", TYPE_UINT16), "f32": ("f", TYPE_FLOAT), "f16": ("e", TYPE_FLOAT16)}[kind]
    buf = struct.pack(f"<{len(samples)}{fmt_char}", *samples)
    fmt = g.PixelFormat(channels, dtype, LITTLE_ENDIAN, 0)
    assert lib.JxlEncoderAddImageFrame(fs, ctypes.byref(fmt), buf, len(buf)) == 0
    lib.JxlEncoderCloseInput(enc)
    out = bytearray()
    while True:
        chunk = (ctypes.c_uint8 * 65536)()
        ptr = ctypes.cast(chunk, ctypes.POINTER(ctypes.c_uint8))
        avail = ctypes.c_size_t(len(chunk))
        st = lib.JxlEncoderProcessOutput(enc, ctypes.byref(ptr), ctypes.byref(avail))
        out += bytes(chunk)[: len(chunk) - avail.value]
        if st == 0:
            break
        assert st == 2, f"encoder status {st}"
    lib.JxlEncoderDestroy(enc)
    return bytes(out)


def pixels(f, alpha=None):
    out = []
    for y in range(H):
        for x in range(W):
            out += [f(v) for v in lin(x, y)]
            if alpha:
                out.append(alpha(x, y))
    return out


def main(out):
    files = {
        "hdr_pq_p3.jxl": encode(
            3, PRIM_P3, TF_PQ, "u16",
            pixels(lambda v: round(g.pq_oetf(v * 203.0) * 65535)), intensity=1000.0),
        "hdr_hlg_2020_alpha.jxl": encode(
            4, PRIM_2100, TF_HLG, "u16",
            pixels(lambda v: round(hlg_oetf(v / 4) * 65535), lambda x, y: x * 2113 + y)),
        "hdr_pq_709_lossy.jxl": encode(
            3, PRIM_SRGB, TF_PQ, "u16",
            pixels(lambda v: round(g.pq_oetf(v * 203.0) * 65535)), lossy=1.0,
            intensity=1000.0),
        "linear_f32.jxl": encode(3, PRIM_SRGB, TF_LINEAR, "f32", pixels(lambda v: v)),
        "linear_f16_2020_lossy.jxl": encode(
            3, PRIM_2100, TF_LINEAR, "f16", pixels(lambda v: v / 4), lossy=0.2,
            intensity=812.0),
    }
    for name, data in files.items():
        with open(f"{out}/{name}", "wb") as f:
            f.write(data)
        print(name, len(data))


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else ".")
