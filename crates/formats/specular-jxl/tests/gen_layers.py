"""A JPEG XL fixture whose single image is a long chain of layers, each blended onto the one
before (a hostile file: rendering it recursed once per layer before the decoder blended
reference frames oldest first).

    python3 -I gen_layers.py OUT_DIR

Made with libjxl (libjxl.so.0.11 through ctypes, reusing the bindings of gen_gainmap.py).

layers_chain.jxl  8x8 image, lossless 8-bit grey. N zero-duration layers of 1x1 at (0, 0):
                  each ADDs 1 onto reference slot 1 and saves itself there; the last layer
                  is the shown frame. Pixel (0, 0) is N mod 256 (sample values wrap), the
                  rest is 0.
"""

import ctypes
import os
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import gen_gainmap as g  # noqa: E402

N = 3000
W = H = 8
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


lib.JxlEncoderInitFrameHeader.argtypes = [ctypes.POINTER(FrameHeader)]
lib.JxlEncoderSetFrameHeader.argtypes = [ctypes.c_void_p, ctypes.POINTER(FrameHeader)]
BLEND_REPLACE, BLEND_ADD = 0, 1


def encode():
    enc = lib.JxlEncoderCreate(None)
    assert lib.JxlEncoderUseContainer(enc, 0) == 0
    info = g.BasicInfo()
    lib.JxlEncoderInitBasicInfo(ctypes.byref(info))
    info.xsize, info.ysize = W, H
    info.bits_per_sample = 8
    info.num_color_channels = 1
    info.uses_original_profile = 1
    assert lib.JxlEncoderSetBasicInfo(enc, ctypes.byref(info)) == 0
    ce = g.ColorEncoding()
    lib.JxlColorEncodingSetToSRGB(ctypes.byref(ce), 1)
    assert lib.JxlEncoderSetColorEncoding(enc, ctypes.byref(ce)) == 0
    fs = lib.JxlEncoderFrameSettingsCreate(enc, None)
    assert lib.JxlEncoderSetFrameLossless(fs, 1) == 0
    fmt = g.PixelFormat(1, g.TYPE_UINT8, g.BIG_ENDIAN, 0)
    # The first layer: the whole (black) canvas, saved as reference 1.
    fh = FrameHeader()
    lib.JxlEncoderInitFrameHeader(ctypes.byref(fh))
    fh.layer_info.blend_info.blendmode = BLEND_REPLACE
    fh.layer_info.save_as_reference = 1
    assert lib.JxlEncoderSetFrameHeader(fs, ctypes.byref(fh)) == 0
    buf = bytes(W * H)
    assert lib.JxlEncoderAddImageFrame(fs, ctypes.byref(fmt), buf, len(buf)) == 0
    for _ in range(N):
        fh = FrameHeader()
        lib.JxlEncoderInitFrameHeader(ctypes.byref(fh))
        li = fh.layer_info
        li.have_crop, li.crop_x0, li.crop_y0, li.xsize, li.ysize = 1, 0, 0, 1, 1
        li.blend_info.blendmode = BLEND_ADD
        li.blend_info.source = 1
        li.save_as_reference = 1
        assert lib.JxlEncoderSetFrameHeader(fs, ctypes.byref(fh)) == 0
        one = bytes([1])
        assert lib.JxlEncoderAddImageFrame(fs, ctypes.byref(fmt), one, 1) == 0
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


def main():
    out_dir = sys.argv[1]
    data = encode()
    with open(os.path.join(out_dir, "layers_chain.jxl"), "wb") as f:
        f.write(data)
    print(len(data), "bytes")


if __name__ == "__main__":
    main()
