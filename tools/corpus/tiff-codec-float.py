"""Fixtures for specular-tiff's float JPEG XL and signed JPEG 2000 units (tests/codec_float.rs).

Usage (a venv with tifffile and imagecodecs): python -I tiff-codec-float.py OUTDIR
Writes NAME.tif (lossless, 16x16 tiles) and NAME.src (the source samples, native order).
Float JPEG 2000 is attempted to show it fails: the codec holds integers only.
"""
import sys, numpy as np, tifffile, imagecodecs
out = sys.argv[1]
y, x = np.mgrid[0:37, 0:45].astype(np.float64)
g = 0.5 + 0.4 * np.sin(x / 7 + y / 11)
cases = {
    "jxl_f32_gray": (g.astype(np.float32), "jpegxl", {"lossless": True}),
    "jxl_f32_rgb": (np.stack([g, g * 0.8, 1 - g], -1).astype(np.float32), "jpegxl", {"lossless": True}),
    "jxl_f16_gray": (g.astype(np.float16), "jpegxl", {"lossless": True}),
    "j2k_i16_gray": (((g - 0.5) * 60000).astype(np.int16), "jpeg2000", {"level": 0}),
    "j2k_i8_rgb": (((np.stack([g, 1 - g, g * g], -1) - 0.5) * 250).astype(np.int8), "jpeg2000", {"level": 0}),
    "j2k_f32_gray": (g.astype(np.float32), "jpeg2000", {}),
}
for name, (a, comp, args) in cases.items():
    try:
        p = f"{out}/{name}.tif"
        tifffile.imwrite(p, a, compression=comp, compressionargs=args, tile=(16, 16))
        back = tifffile.imread(p)
        a.tofile(f"{out}/{name}.src")
        print(name, "ok", a.dtype, a.shape, "roundtrip exact:", np.array_equal(back, a), "max err", float(np.abs(back.astype(np.float64) - a).max()))
    except Exception as e:
        print(name, "FAILED", type(e).__name__, str(e)[:100])
