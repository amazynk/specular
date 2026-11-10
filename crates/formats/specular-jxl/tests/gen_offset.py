"""A JPEG XL fixture whose only frame is larger than the image and placed at a negative
offset, like libjxl conformance `sunset_logo`.

    python3 -I gen_offset.py OUT_DIR

Made with libjxl (libjxl.so.0.11 through ctypes, reusing the bindings of gen_gainmap.py).

offset_frame.jxl  20x12 image, lossless modular, 10-bit RGBA (straight alpha). One frame of
                  FW x FH = 34x22 samples at (-9, -5): image pixel (x, y) is frame sample
                  (x + 9, y + 5) = px(x + 9, y + 5); the rest of the frame falls outside.
offset_frame_pq.jxl  The same samples tagged PQ, BT.2100 primaries (for the 16-bit HDR path).

tests/differential.rs computes px; keep them in step.
"""

import ctypes
import os
import struct
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import gen_gainmap as g  # noqa: E402

W, H = 20, 12
FW, FH, X0, Y0 = 34, 22, -9, -5
BITS = 10
lib = g.lib


class BlendInfo(ctypes.Structure):
    _fields_ = [
        ("blendmode", ctypes.c_int),
        ("source", ctypes.c_uint32),
        ("alpha", ctypes.c_uint32),
        ("clamp", ctypes.c_int),
    ]


class LayerInfo(ctypes.Structure):
    _fields_ = [
        ("have_crop", ctypes.c_int),
        ("crop_x0", ctypes.c_int32),
        ("crop_y0", ctypes.c_int32),
        ("xsize", ctypes.c_uint32),
        ("ysize", ctypes.c_uint32),
        ("blend_info", BlendInfo),
        ("save_as_reference", ctypes.c_uint32),
    ]


class FrameHeader(ctypes.Structure):
    _fields_ = [
        ("duration", ctypes.c_uint32),
        ("timecode", ctypes.c_uint32),
        ("name_length", ctypes.c_uint32),
        ("is_last", ctypes.c_int),
        ("layer_info", LayerInfo),
    ]


class BitDepth(ctypes.Structure):
    _fields_ = [
        ("type", ctypes.c_int),
        ("bits_per_sample", ctypes.c_uint32),
        ("exponent_bits_per_sample", ctypes.c_uint32),
    ]


lib.JxlEncoderInitFrameHeader.argtypes = [ctypes.POINTER(FrameHeader)]
lib.JxlEncoderSetFrameHeader.argtypes = [ctypes.c_void_p, ctypes.POINTER(FrameHeader)]
lib.JxlEncoderSetFrameBitDepth.argtypes = [ctypes.c_void_p, ctypes.POINTER(BitDepth)]
BIT_DEPTH_FROM_CODESTREAM, BLEND_REPLACE = 1, 0


def px(fx, fy):
    """Frame sample (fx, fy): RGBA, 10 bits each."""
    return (
        (fx * 37 + fy * 11) % 1024,
        (fx * fy * 7 + 100) % 1024,
        1023 - (fx * 20 + fy * 3) % 1024,
        512 + (fx + fy) * 13 % 512,
    )


def encode(pq):
    enc = lib.JxlEncoderCreate(None)
    assert lib.JxlEncoderUseContainer(enc, 0) == 0
    info = g.BasicInfo()
    lib.JxlEncoderInitBasicInfo(ctypes.byref(info))
    info.xsize, info.ysize = W, H
    info.bits_per_sample, info.exponent_bits_per_sample = BITS, 0
    info.num_color_channels = 3
    info.num_extra_channels = 1
    info.alpha_bits, info.alpha_exponent_bits = BITS, 0
    info.uses_original_profile = 1
    assert lib.JxlEncoderSetBasicInfo(enc, ctypes.byref(info)) == 0
    ce = g.ColorEncoding()
    lib.JxlColorEncodingSetToSRGB(ctypes.byref(ce), 0)
    if pq:
        ce.primaries, ce.transfer_function = g.PRIM_2100, g.TF_PQ
    assert lib.JxlEncoderSetColorEncoding(enc, ctypes.byref(ce)) == 0
    fs = lib.JxlEncoderFrameSettingsCreate(enc, None)
    assert lib.JxlEncoderSetFrameLossless(fs, 1) == 0
    depth = BitDepth(BIT_DEPTH_FROM_CODESTREAM, BITS, 0)
    assert lib.JxlEncoderSetFrameBitDepth(fs, ctypes.byref(depth)) == 0
    fh = FrameHeader()
    lib.JxlEncoderInitFrameHeader(ctypes.byref(fh))
    fh.layer_info.have_crop = 1
    fh.layer_info.crop_x0, fh.layer_info.crop_y0 = X0, Y0
    fh.layer_info.xsize, fh.layer_info.ysize = FW, FH
    fh.layer_info.blend_info.blendmode = BLEND_REPLACE
    assert lib.JxlEncoderSetFrameHeader(fs, ctypes.byref(fh)) == 0
    samples = [v for fy in range(FH) for fx in range(FW) for v in px(fx, fy)]
    buf = struct.pack(f"<{len(samples)}H", *samples)
    fmt = g.PixelFormat(4, g.TYPE_UINT16, 1, 0)  # little endian
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


def main(out):
    for name, pq in [("offset_frame.jxl", False), ("offset_frame_pq.jxl", True)]:
        data = encode(pq)
        with open(f"{out}/{name}", "wb") as f:
            f.write(data)
        print(name, len(data))


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else ".")
