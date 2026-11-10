"""JPEG XL gain-map fixtures (`jhgm` box) for specular-jxl and specular-decode.

    python3 -I gen_gainmap.py OUT_DIR

The codestreams are lossless and made with libjxl (libjxl.so.0.11 through ctypes; no tool
or Python binding needed); the container and the `jhgm` box (JPEG XL gain map bundle,
ISO/IEC 18181-2 amendment) are written here byte by byte:

    jhgm_version u8 (0) | metadata_size u16 BE | ISO 21496-1 metadata |
    colour_encoding_size u8 | JXL ColorEncoding bundle | alt_icc_size u32 BE |
    compressed alternate ICC | gain-map codestream (to the end of the box)

gainmap_sdr.jxl      32x16 sRGB 8-bit base, a 16x8 grey map (horizontal ramp), one-channel
                     metadata with a common denominator: base headroom 0, alternate 2 stops,
                     gain 0..2 stops, gamma 1, offsets 1/64. No alternate colour encoding.
gainmap_hdr.jxl      32x16 PQ BT.2100 16-bit base whose SDR alternate is sdr_px(x, y)
                     (8-bit sRGB) and whose HDR rendition is that, linear, times
                     2^(2y/15) (up to 4x reference white). The full-resolution RGB map
                     holds log2((hdr + 1/64) / (sdr + 1/64)) in BT.2020 (the base space,
                     use_base_colour_space = 1), per channel, from the metadata's per-value
                     denominators. Base headroom 2 stops, alternate 0. The alternate
                     colour encoding (sRGB) is written out field by field, not all_default.

jxl_gainmap.rs and specular-decode's tests/gainmap.rs compute sdr_px; keep them in step.
"""

import ctypes
import math
import struct
import sys

W, H = 32, 16
OFF = 1.0 / 64.0

lib = ctypes.CDLL("libjxl.so.0.11")


class BasicInfo(ctypes.Structure):
    _fields_ = [
        ("have_container", ctypes.c_int),
        ("xsize", ctypes.c_uint32),
        ("ysize", ctypes.c_uint32),
        ("bits_per_sample", ctypes.c_uint32),
        ("exponent_bits_per_sample", ctypes.c_uint32),
        ("intensity_target", ctypes.c_float),
        ("min_nits", ctypes.c_float),
        ("relative_to_max_display", ctypes.c_int),
        ("linear_below", ctypes.c_float),
        ("uses_original_profile", ctypes.c_int),
        ("have_preview", ctypes.c_int),
        ("have_animation", ctypes.c_int),
        ("orientation", ctypes.c_int),
        ("num_color_channels", ctypes.c_uint32),
        ("num_extra_channels", ctypes.c_uint32),
        ("alpha_bits", ctypes.c_uint32),
        ("alpha_exponent_bits", ctypes.c_uint32),
        ("alpha_premultiplied", ctypes.c_int),
        ("preview", ctypes.c_uint32 * 2),
        ("animation", ctypes.c_uint32 * 4),
        ("intrinsic_xsize", ctypes.c_uint32),
        ("intrinsic_ysize", ctypes.c_uint32),
        ("padding", ctypes.c_uint8 * 100),
    ]


class ColorEncoding(ctypes.Structure):
    _fields_ = [
        ("color_space", ctypes.c_int),
        ("white_point", ctypes.c_int),
        ("white_point_xy", ctypes.c_double * 2),
        ("primaries", ctypes.c_int),
        ("primaries_red_xy", ctypes.c_double * 2),
        ("primaries_green_xy", ctypes.c_double * 2),
        ("primaries_blue_xy", ctypes.c_double * 2),
        ("transfer_function", ctypes.c_int),
        ("gamma", ctypes.c_double),
        ("rendering_intent", ctypes.c_int),
    ]


class PixelFormat(ctypes.Structure):
    _fields_ = [
        ("num_channels", ctypes.c_uint32),
        ("data_type", ctypes.c_int),
        ("endianness", ctypes.c_int),
        ("align", ctypes.c_size_t),
    ]


lib.JxlEncoderCreate.restype = ctypes.c_void_p
lib.JxlEncoderFrameSettingsCreate.restype = ctypes.c_void_p
lib.JxlEncoderUseContainer.argtypes = [ctypes.c_void_p, ctypes.c_int]
lib.JxlEncoderSetBasicInfo.argtypes = [ctypes.c_void_p, ctypes.POINTER(BasicInfo)]
lib.JxlEncoderSetColorEncoding.argtypes = [ctypes.c_void_p, ctypes.POINTER(ColorEncoding)]
lib.JxlEncoderFrameSettingsCreate.argtypes = [ctypes.c_void_p, ctypes.c_void_p]
lib.JxlEncoderSetFrameLossless.argtypes = [ctypes.c_void_p, ctypes.c_int]
lib.JxlEncoderAddImageFrame.argtypes = [ctypes.c_void_p, ctypes.POINTER(PixelFormat),
                                        ctypes.c_void_p, ctypes.c_size_t]
lib.JxlEncoderProcessOutput.argtypes = [ctypes.c_void_p,
                                        ctypes.POINTER(ctypes.POINTER(ctypes.c_uint8)),
                                        ctypes.POINTER(ctypes.c_size_t)]
lib.JxlEncoderCloseInput.argtypes = [ctypes.c_void_p]
lib.JxlEncoderDestroy.argtypes = [ctypes.c_void_p]
lib.JxlEncoderInitBasicInfo.argtypes = [ctypes.POINTER(BasicInfo)]
lib.JxlColorEncodingSetToSRGB.argtypes = [ctypes.POINTER(ColorEncoding), ctypes.c_int]

# libjxl enum values.
TYPE_UINT8, TYPE_UINT16, BIG_ENDIAN = 2, 3, 2
CS_RGB, WP_D65, PRIM_2100, TF_PQ, INTENT_REL = 0, 1, 9, 16, 1


def encode(w, h, channels, bits, samples, pq=False):
    """A bare lossless codestream of `samples` (row-major, interleaved)."""
    enc = lib.JxlEncoderCreate(None)
    assert lib.JxlEncoderUseContainer(enc, 0) == 0
    info = BasicInfo()
    lib.JxlEncoderInitBasicInfo(ctypes.byref(info))
    info.xsize, info.ysize = w, h
    info.bits_per_sample = bits
    info.num_color_channels = channels
    info.uses_original_profile = 1
    if pq:
        info.intensity_target = 1000.0
    assert lib.JxlEncoderSetBasicInfo(enc, ctypes.byref(info)) == 0
    ce = ColorEncoding()
    lib.JxlColorEncodingSetToSRGB(ctypes.byref(ce), 1 if channels == 1 else 0)
    if pq:
        ce.color_space, ce.white_point, ce.primaries = CS_RGB, WP_D65, PRIM_2100
        ce.transfer_function, ce.rendering_intent = TF_PQ, INTENT_REL
    assert lib.JxlEncoderSetColorEncoding(enc, ctypes.byref(ce)) == 0
    fs = lib.JxlEncoderFrameSettingsCreate(enc, None)
    assert lib.JxlEncoderSetFrameLossless(fs, 1) == 0
    if bits == 8:
        buf = bytes(samples)
        fmt = PixelFormat(channels, TYPE_UINT8, BIG_ENDIAN, 0)
    else:
        buf = struct.pack(f">{len(samples)}H", *samples)
        fmt = PixelFormat(channels, TYPE_UINT16, BIG_ENDIAN, 0)
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
    return unwrap(bytes(out))


def unwrap(out):
    """(codestream, level box or b"") of the encoder output. libjxl wraps codestreams that
    need level 10 (here: the PQ one) in a container with a `jxll` box."""
    if out[:2] == b"\xff\x0a":
        return out, b""
    i, cs, level = 0, None, b""
    while i + 8 <= len(out):
        size, kind = struct.unpack(">I4s", out[i : i + 8])
        if kind == b"jxlc":
            cs = out[i + 8 : i + size]
        elif kind == b"jxll":
            level = out[i : i + size]
        i += size
    assert cs and cs[:2] == b"\xff\x0a"
    return cs, level


def box(kind, body):
    return struct.pack(">I", 8 + len(body)) + kind + body


def container(codestream, gain_map_box):
    cs, level = codestream
    sig = box(b"JXL ", b"\r\n\x87\n")
    ftyp = box(b"ftyp", b"jxl " + struct.pack(">I", 0) + b"jxl ")
    return sig + ftyp + level + gain_map_box + box(b"jxlc", cs)


def jhgm(metadata, colour_encoding, codestream):
    body = bytes([0]) + struct.pack(">H", len(metadata)) + metadata
    body += bytes([len(colour_encoding)]) + colour_encoding
    body += struct.pack(">I", 0)  # no alternate ICC profile
    cs, level = codestream
    assert not level, "the gain map must not need level 10"
    return box(b"jhgm", body + cs)


class Bits:
    """JPEG XL bit writer (least significant bit first)."""

    def __init__(self):
        self.v, self.n = 0, 0

    def put(self, value, bits):
        self.v |= value << self.n
        self.n += bits

    def enum(self, value):
        """U32(Val(0), Val(1), BitsOffset(4, 2), BitsOffset(6, 18))."""
        if value < 2:
            self.put(value, 2)
        elif value < 18:
            self.put(2, 2)
            self.put(value - 2, 4)
        else:
            self.put(3, 2)
            self.put(value - 18, 6)

    def bytes(self):
        return self.v.to_bytes((self.n + 7) // 8, "little")


def srgb_bundle():
    """ColorEncoding: sRGB, every field written."""
    b = Bits()
    b.put(0, 1)  # all_default
    b.put(0, 1)  # want_icc
    b.enum(0)  # colour_space RGB
    b.enum(1)  # white_point D65
    b.enum(1)  # primaries sRGB
    b.put(0, 1)  # have_gamma
    b.enum(13)  # transfer sRGB
    b.enum(1)  # rendering intent relative
    return b.bytes()


def iso_common(base_hr, alt_hr, gmin, gmax, gamma, ob, oa):
    """ISO 21496-1 metadata, one channel, one common denominator."""
    d = 10_000
    r = lambda v: round(v * d)  # noqa: E731
    b = struct.pack(">HHB", 0, 0, 0x08) + struct.pack(">III", d, r(base_hr), r(alt_hr))
    b += struct.pack(">iiIii", r(gmin), r(gmax), r(gamma), r(ob), r(oa))
    return b


def iso_per_channel(base_hr, alt_hr, gmin, gmax, ob, oa):
    """ISO 21496-1 metadata, three channels, in the base colour space, per-value
    denominators."""
    d = 1_000_000
    r = lambda v: round(v * d)  # noqa: E731
    flags = 0x80 | 0x40  # multichannel, use_base_colour_space
    b = struct.pack(">HHB", 0, 0, flags)
    b += struct.pack(">IIII", r(base_hr), d, r(alt_hr), d)
    for c in range(3):
        b += struct.pack(">iI", r(gmin[c]), d) + struct.pack(">iI", r(gmax[c]), d)
        b += struct.pack(">II", d, d)  # gamma 1
        b += struct.pack(">iI", r(ob), d) + struct.pack(">iI", r(oa), d)
    return b


def sdr_px(x, y):
    return (40 + 6 * x, 30 + 12 * y, 200 - 4 * x)


def srgb_to_linear(v):
    return v / 12.92 if v <= 0.04045 else ((v + 0.055) / 1.055) ** 2.4


# BT.709 -> BT.2020 linear (ITU-R BT.2087).
TO_2020 = [[0.6274, 0.3293, 0.0433], [0.0691, 0.9195, 0.0114], [0.0164, 0.0880, 0.8956]]


def pq_oetf(nits):
    m1, m2 = 2610 / 16384, 2523 / 4096 * 128
    c1, c2, c3 = 3424 / 4096, 2413 / 4096 * 32, 2392 / 4096 * 32
    y = max(nits, 0.0) / 10000.0
    p = y ** m1
    return ((c1 + c2 * p) / (1 + c3 * p)) ** m2


def main(out):
    # 1. SDR base, grey half-resolution map.
    base = [v for y in range(H) for x in range(W) for v in (x * 8, y * 16, 128)]
    mw, mh = W // 2, H // 2
    ramp = [round(x * 255 / (mw - 1)) for _ in range(mh) for x in range(mw)]
    meta = iso_common(0.0, 2.0, 0.0, 2.0, 1.0, OFF, OFF)
    cs = encode(W, H, 3, 8, base)
    gm = encode(mw, mh, 1, 8, ramp)
    with open(f"{out}/gainmap_sdr.jxl", "wb") as f:
        f.write(container(cs, jhgm(meta, b"", gm)))

    # 2. PQ BT.2100 base, full-resolution RGB map towards the SDR alternate.
    hdr, gains = [], []
    for y in range(H):
        k = 2.0 ** (2.0 * y / (H - 1))
        for x in range(W):
            sdr709 = [srgb_to_linear(v / 255.0) for v in sdr_px(x, y)]
            sdr = [sum(TO_2020[i][j] * sdr709[j] for j in range(3)) for i in range(3)]
            hdr_px = [v * k for v in sdr]
            hdr += [round(pq_oetf(v * 203.0) * 65535) for v in hdr_px]
            gains.append([math.log2((h + OFF) / (s + OFF)) for h, s in zip(hdr_px, sdr)])
    gmin = [min(g[c] for g in gains) for c in range(3)]
    gmax = [max(g[c] for g in gains) for c in range(3)]
    gmax = [max(hi, lo + 1e-3) for lo, hi in zip(gmin, gmax)]
    gmap = [round((g[c] - gmin[c]) / (gmax[c] - gmin[c]) * 255) for g in gains for c in range(3)]
    meta = iso_per_channel(2.0, 0.0, gmin, gmax, OFF, OFF)
    cs = encode(W, H, 3, 16, hdr, pq=True)
    gm = encode(W, H, 3, 8, gmap)
    with open(f"{out}/gainmap_hdr.jxl", "wb") as f:
        f.write(container(cs, jhgm(meta, srgb_bundle(), gm)))


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else ".")
