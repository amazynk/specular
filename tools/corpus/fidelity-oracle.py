#!/usr/bin/env python3
"""Fidelity: option matrices made with reference encoders, two independent reference
decoders per format, and an automatic classification of every difference.

    fidelity-oracle.py gen OUT            # write the option matrices into OUT/<group>/gen
    fidelity-oracle.py check ROOT MANIFEST

`gen` uses tifffile (+imagecodecs), Pillow, imagecodecs (libpng, libjxl, libavif),
pillow-heif (libheif), OpenEXR's Python bindings, ImageMagick and FFmpeg (DPX). Each file
it writes losslessly gets `<file>.npy` beside it: the array it was made from (stored
orientation, straight alpha, float in [0, 1] for integers) - a third, exact reference.

`check` is driven by `crates/specular-convert/tests/fidelity.rs`, which decodes every file
through the registry and writes the manifest (one line per file: decoded samples as RGBA
float32, plus the PNG the viewer would show). For each file it:
  1. decodes it with the format's reference decoders (two where two independent ones exist:
     TIFF tifffile + Pillow/libtiff; PNG Pillow + libpng; EXR OpenEXR + FFmpeg; AVIF
     libavif + Pillow; HEIC pillow-heif + heif-dec; DPX ImageMagick + FFmpeg; JPEG XL
     libjxl + the generator's array or the conformance reference), and takes agreement as
     the ground truth (disagreement is printed as an oracle disagreement);
  2. compares our samples with it (tolerance: half a step of the coarser of the two sample
     depths for lossless data; mean 1/255 and 99.9th percentile 10/255 for lossy codecs,
     where IDCT and chroma upsampling may differ);
  3. classifies any difference: size, orientation (one of the eight), shift (up to 2 px),
     channel swap, inversion, gamma/range (fitted), alpha premultiplied or not, edge-only,
     and a systematic bias - which is flagged even when the file passes;
  4. checks what the viewer shows (orientation from exiftool, ICC to sRGB with LittleCMS,
     alpha) against the reference, and our Info orientation against exiftool's.

Prints `ORACLE<TAB>FMT/<group><TAB>file<TAB>pass|fail|skip` per file plus a `#` detail line
for anything not clean, then a summary. Files listed in KNOWN fail without failing the run.

The extra set (`gen-extra OUT [jpeg webp gif bmp jp2 psd hdr film]`, corpus
`fetch-fidelity.sh --extra`): JPEG (Pillow, FFmpeg, libjpeg-turbo 3, jpegsof3 for lossless; the decoded planes against
libjpeg-turbo's), WebP (Pillow, FFmpeg, OpenImageIO for animations; the planes against
dwebp -yuv), GIF (Pillow, FFmpeg, ImageMagick, the pygif suite's expected frames), BMP
(Pillow, ImageMagick, OpenImageIO, BMP Suite's expected pictures), JPEG 2000 (OpenJPEG,
OpenJPH, FFmpeg), PSD/PSB (psd-tools, ImageMagick, OpenImageIO, LittleCMS for CMYK and Lab,
psd-tools' layer pixels for layer pages), Radiance HDR and PFM (OpenImageIO, FFmpeg,
imagecodecs) and Cineon (OpenImageIO and ImageMagick code values through the printing-density
curve). Frames (`file#k`) and pages (`file@k`) are compared as samples.
"""

import io
import json
import os
import shutil
import subprocess
import sys
import traceback
from pathlib import Path

import numpy as np

# Known differences (file name substring -> reason): counted as failures, run still passes.
KNOWN = {
    "tiff/gen/cmyk8.tif": "untagged CMYK goes through a generic CMYK profile (tiff_dec.rs); "
    "the references convert naively (1-C)(1-K)",
    "TestImages/WideFloatRange.exr": "32-bit floats out to 1.7e38: 44 % of them beyond the half-float "
    "raster's range and 44 % below its normal range (the raster is half-float by design)",
    # The extra set.
    "jpeg/gen/adobe-cmyk.jpg": "untagged CMYK goes through a generic CMYK profile (an analytic printing model, colour.rs); the references convert naively",
    "jpeg/gen/adobe-ycck.jpg": "untagged CMYK goes through a generic CMYK profile (an analytic printing model, colour.rs); the references convert naively",
    "jpeg/gen/pillow-cmyk.jpg": "untagged CMYK goes through a generic CMYK profile (an analytic printing model, colour.rs); the references convert naively",
    "psd/gen/cmyk8.psd": "untagged CMYK goes through a generic CMYK profile (an analytic printing model, colour.rs); the references convert naively",
    "psd/gen/cmyk16.psd": "untagged CMYK goes through a generic CMYK profile (an analytic printing model, colour.rs); the references convert naively",
    "psd/gen/lab": "Lab to sRGB: mean 0.3-0.5/255 from LittleCMS, up to 40/255 on out-of-gamut colours (gamut clipping differs; LabConverter clips in linear light)",
    "_lab.psd": "Lab to sRGB: mean 2.5/255 from LittleCMS on 16 pixels of far out-of-gamut colours (gamut clipping differs)",
    "psd/gen/rgb8-extra-channel.psd": "no layer section and no alpha-channel names: whether the fourth channel is transparency is unstated (we and ImageMagick take it as alpha; psd-tools un-mattes with it)",
    "psd_files/adjustment-fillers.psd": "no merged image: composited from the layers; its adjustment layers are not drawn (psd-tools draws them)",
    "psd_files/blend-and-clipping.psd": "no merged image: composited from the layers; the shapes' layer effects are not drawn",
    "layers-minimal/shape-layer.psd": "no merged image: the shape's 1 px inner vector stroke is not drawn; fill and path match psd-tools' composite",
    "psd_files/note.psd": "no merged image: composited from the layers; max 7/255 on shape edges against psd-tools' composite (antialiasing)",
    "psd_files/unicode_pathname.ps": "no merged image: one solid-colour fill layer, within 1/255 of psd-tools' composite (rounding of the descriptor's fractional colour)",
    "gif/gen/anim-dispose2.gif": "disposal 2 clears to transparent, as browsers and the GIF test suite do; Pillow and FFmpeg fill the background colour",
    "gif/gen/anim-dispose3.gif": "disposal 3 on the first frame restores the empty (transparent) canvas, as browsers do; Pillow keeps the frame, FFmpeg fills the background colour",
    "gif/gen/anim-local-palettes.gif": "disposal 2 clears to transparent, as browsers and the GIF test suite do; Pillow and FFmpeg fill the background colour",
    "test-suite/image-zero-height.gif": "a GIF whose only image is empty is refused with a reason; the suite expects an empty transparent canvas",
    "test-suite/image-zero-size.gif": "a GIF whose only image is empty is refused with a reason; the suite expects an empty transparent canvas",
    "test-suite/no-data.gif": "a GIF without image data is refused with a reason; the suite expects an empty transparent canvas",
    "test-suite/invalid-code.gif": "LZW data with an invalid code: we show what decoded and grey-fill the rest (marked truncated); the suite expects no frame, FFmpeg black",
    "q/rgb32-111110.bmp": "the suite's picture is the 8-bit original; ours rounds the 11-bit field exactly (1690/2047 is 211, the original 210): 1 code on 128 pixels",
    "q/pal1huffmsb.bmp": "OS/2 Huffman 1D (fax-coded) BMP: refused with a reason (open)",
    "q/rgb24rle24.bmp": "OS/2 RLE24 BMP: refused with a reason (open)",
    "q/rgba64.bmp": "64-bit (s2.13 fixed-point) BMP: refused with a reason (open)",
    "conformance/p0_05.j2k": "4 components, two subsampled, no colour specification (raw codestream): we show RGB + alpha; FFmpeg guesses YUV, OpenJPEG (imagecodecs) refuses",
    "conformance/p1_03.j2k": "4 components, two subsampled, no colour specification (raw codestream): we show RGB + alpha; FFmpeg guesses YUV, OpenJPEG (imagecodecs) refuses",
    # Follow-up fixes.
    "Chromaticities/Rec709_YC.exr": "subsampled chroma: OpenEXR's RgbaInputFile rebuilds it with its own long filter, we interpolate linearly; against the RGB original (Rec709.exr) ours is 0.56 % mean relative error, OpenEXR's 0.52 % (ours vs OpenEXR 0.21 % mean, 3 % p99)",
    "multilayer_examples/animals_00_multilayer_a1op.avif@": "lower layer (lsel 0 / a1op): libheif 1.21 ignores lsel and a1op and shows the top layer (2048x1536) where the item's lsel and ispe give 1024x768, and libavif reads no other items; ours, the lower layer, is the primary picture 2x smaller (mean 7.2/255 from heif-dec's top layer box-filtered)",
    "multilayer_examples/animals_00_multilayer_a1op_lsel.avif@": "lower layer (lsel 0 / a1op): libheif 1.21 ignores lsel and a1op and shows the top layer (2048x1536) where the item's lsel and ispe give 1024x768, and libavif reads no other items; ours, the lower layer, is the primary picture 2x smaller (mean 7.2/255 from heif-dec's top layer box-filtered)",
    "multilayer_examples/animals_00_multilayer_lsel.avif@": "lower layer (lsel 0): libheif 1.21 ignores lsel and shows the top layer (2048x1536) where the item's lsel and ispe give 1024x768, and libavif reads no other items; ours, the lower layer, is the primary picture 2x smaller (mean 7.2/255 from heif-dec's top layer box-filtered)",
    "multilayer_examples/animals_00_multilayer_grid_lsel.avif@": "grid of lower-layer tiles (lsel 0): libheif 1.21 decodes the tiles' top layer and so shows the top-left quarter of the picture; ours is the primary picture 2x smaller (mean 3.9/255 from heif-dec's primary box-filtered)",
    "non_compliant/truncated_elementary_stream.avif@": "a page only heif-dec reads: libheif's YUV conversion differs from libavif's by up to 31/255 on every Apple file (ours = libavif on their primaries)",
    "Link-U/kimono.crop.avif": "crop exact (clap 385x330 at 272,39, ISO/IEC 23008-12; a window one pixel off is 4x worse); the rest is the YUV conversion against libavif's (mean 0.75/255 on the uncropped kimono.avif, 1.04 in this window)",
    "Link-U/kimono.mirror-vertical.rotate270.crop.avif": "crop exact (clap 330x385, ISO/IEC 23008-12); the rest is the YUV conversion against libavif's (mean 0.75/255 on the uncropped kimono.avif, 1.04 in this window)",
    "Netflix/avis/alpha_video.avif": "alpha exact in every frame; on opaque pixels libavif's blue is 6-7/255 below both libheif's and ours (ours vs heif-dec: R +0.3, G -1.0, B -1.2 /255 mean); heif-dec reads no sequence frames",
}
# Known by our error text (counted as failures, the run still passes).
KNOWN_ERRORS = {
    "Maximize": "no merged image (saved with Maximize compatibility off) and no pixel layers to composite (only fill, shape or adjustment layers, which are not drawn); refused with that reason; psd-tools and ImageMagick draw the fills (files with pixel layers are composited from them)",
}
# Known by what the check found (counted as failures, the run still passes).
KNOWN_DETAILS = {
}

GROUP_FMT = {
    "tiff": "TIFF",
    "png": "PNG",
    "exr": "EXR",
    "jxl": "JXL",
    "avif": "AVIF",
    "heic": "HEIC",
    "dpx": "DPX",
    "raw": "RAW",
    "jpeg": "JPEG",
    "webp": "WEBP",
    "gif": "GIF",
    "bmp": "BMP",
    "jp2": "JP2",
    "psd": "PSD",
    "hdr": "HDR",
    "film": "CINEON",
}
EXACT = ("generator", "gif-suite", "bmp-suite", "apng-spec")
# Layer names of each file's pages (`file@k` rows), to tell same-named layers apart.
PAGE_LABELS = {}
GROUPS5 = ("jpeg", "webp", "gif", "bmp", "jp2", "psd", "hdr", "film")

LOSSY_HINTS = ("jpeg", "lossy", "webp", "dwa", "b44", "pxr24", "q90", "q60", "-d1")


# --------------------------------------------------------------------------- generation


def base(h, w, c, seed=1):
    """Smooth ramps, a checker, noise and a bright top-left marker (orientation shows)."""
    rng = np.random.default_rng(seed)
    y, x = np.mgrid[0:h, 0:w].astype(np.float64)
    r = x / max(w - 1, 1)
    g = y / max(h - 1, 1)
    b = 0.5 + 0.35 * np.sin(x / 3.0) * np.cos(y / 4.0) + 0.1 * rng.random((h, w))
    b[(x // 8 + y // 8) % 2 == 0] *= 0.6
    a = np.clip(1.2 - np.hypot(x - w / 2, y - h / 2) / (0.6 * max(w, h)), 0, 1)
    img = np.stack([r, g, b, a], -1)[..., :c] if c > 1 else ((r + g + b) / 3)[..., None]
    img = np.clip(img, 0, 1)
    img[:6, :6, 0] = 1.0
    if c >= 3:
        img[:6, :6, 1:3] = 0.0
    if c == 4 or c == 2:
        img[:6, :6, -1] = 1.0
    return img


def quant(img, dtype, bits=None):
    dt = np.dtype(dtype)
    if dt.kind == "f":
        return img.astype(dt)
    bits = bits or dt.itemsize * 8
    if dt.kind == "i":
        m = 2 ** (bits - 1) - 1
        return np.round(img * m).astype(dt)
    m = 2**bits - 1
    return np.round(img * m).astype(dt)


def truth_of(q, bits=None, signed=False):
    if q.dtype.kind == "f":
        return q.astype(np.float32)
    bits = bits or q.dtype.itemsize * 8
    m = 2 ** (bits - 1) - 1 if signed else 2**bits - 1
    return (q.astype(np.float64) / m).astype(np.float32)


def save_truth(path, arr, **meta):
    a = arr if arr.ndim == 3 else arr[..., None]
    np.save(str(path) + ".npy", a.astype(np.float32))
    if meta:
        Path(str(path) + ".json").write_text(json.dumps(meta))


def icc_bytes():
    for p in ("/usr/share/color/icc/AdobeCompat-v4.icc", "/usr/share/color/icc/DisplayP3-v4.icc"):
        if os.path.exists(p):
            return Path(p).read_bytes()
    return None


def gen_tiff(out):
    import tifffile

    out = out / "tiff" / "gen"
    out.mkdir(parents=True, exist_ok=True)
    H, W = 43, 67
    n = 0
    # (endian, planar, compression, predictor, tiled)
    layouts = [
        ("<", "contig", None, None, False),
        (">", "contig", "lzw", True, False),
        ("<", "separate", "deflate", True, True),
        (">", "separate", "zstd", None, True),
        ("<", "contig", "packbits", None, True),
        (">", "contig", "deflate", None, False),
    ]
    dtypes = ["u1", "u2", "u4", "i1", "i2", "i4", "f2", "f4", "f8"]
    for dt in dtypes:
        for spp in (1, 3, 4):
            for li, (endian, planar, comp, pred, tiled) in enumerate(layouts):
                if (li + spp) % 2 and dt in ("i1", "i4", "f8", "u4"):
                    continue  # fewer combinations for the rarer types
                img = base(H, W, spp)
                signed = dt.startswith("i")
                if signed:
                    img = img * 2 - 1
                q = quant(img, dt)
                kw = dict(byteorder=endian, compression=comp, metadata=None)
                if spp > 1:
                    kw["photometric"] = "rgb"
                    kw["planarconfig"] = planar
                else:
                    kw["photometric"] = "minisblack"
                if spp == 4:
                    kw["extrasamples"] = ["unassalpha"]
                if pred and comp:
                    kw["predictor"] = True
                if tiled:
                    kw["tile"] = (16, 32)
                else:
                    kw["rowsperstrip"] = 7
                name = f"{dt}-s{spp}-{'be' if endian == '>' else 'le'}-{planar}-{comp or 'none'}{'-pred' if pred and comp else ''}-{'tile' if tiled else 'strip'}.tif"
                data = np.moveaxis(q, -1, 0) if spp > 1 and planar == "separate" else (q[..., 0] if spp == 1 else q)
                try:
                    tifffile.imwrite(out / name, data, **kw)
                except Exception as e:  # noqa: BLE001 - an unsupported combination
                    print(f"gen tiff {name}: {e}", file=sys.stderr)
                    continue
                t = truth_of(q, signed=signed)
                if signed:
                    # Shown offset binary: the type's minimum black, its maximum white.
                    b = q.dtype.itemsize * 8
                    t = ((q.astype(np.float64) + 2 ** (b - 1)) / (2**b - 1)).astype(np.float32)
                save_truth(out / name, t, signed=signed)
                n += 1
    # Associated alpha (premultiplied in the file).
    img = base(H, W, 4)
    pm = img.copy()
    pm[..., :3] *= pm[..., 3:4]
    for dt in ("u1", "u2", "f4"):
        q = quant(pm, dt)
        name = f"{dt}-assoc-alpha.tif"
        tifffile.imwrite(out / name, q, photometric="rgb", extrasamples=["assocalpha"], metadata=None)
        t = truth_of(q)
        a = np.maximum(t[..., 3:4], 1e-6)
        st = np.where(t[..., 3:4] > 0, np.clip(t[..., :3] / a, 0, 1), 0)
        save_truth(out / name, np.concatenate([st, t[..., 3:4]], -1), assoc=True)
        n += 1
    # Low bit depths, MinIsWhite, palette, CMYK, LogL-free YCbCr JPEG, WebP, 12-bit.
    # One channel as a 2-D array: tifffile writes an (H, W, 1) array as H pages of W x 1
    # (gray1/2/4/12 and MinIsWhite were page stacks, every page after the first
    # failing).
    for bits in (1, 2, 4, 12):
        dt = "u1" if bits < 8 else "u2"
        q = quant(base(H, W, 1), dt, bits)[..., 0]
        name = f"gray{bits}.tif"
        try:
            tifffile.imwrite(out / name, q, bitspersample=bits, photometric="minisblack", metadata=None)
            save_truth(out / name, truth_of(q, bits))
            n += 1
        except Exception as e:  # noqa: BLE001
            print(f"gen tiff {name}: {e}", file=sys.stderr)
    q = quant(base(H, W, 1), "u1")[..., 0]
    tifffile.imwrite(out / "miniswhite8.tif", 255 - q, photometric="miniswhite", metadata=None)
    save_truth(out / "miniswhite8.tif", truth_of(q))
    q16 = quant(base(H, W, 1), "u2")[..., 0]
    tifffile.imwrite(out / "miniswhite16.tif", 65535 - q16, photometric="miniswhite", metadata=None)
    save_truth(out / "miniswhite16.tif", truth_of(q16))
    rng = np.random.default_rng(3)
    for bits in (8,):  # tifffile wants 256 colormap entries at 4 bits (not conforming)
        idx = (base(H, W, 1)[..., 0] * (2**bits - 1)).round().astype("u1")
        cmap = (rng.random((3, 2**bits)) * 65535).astype("u2")
        name = f"palette{bits}.tif"
        tifffile.imwrite(out / name, idx, photometric="palette", colormap=cmap, bitspersample=bits, metadata=None)
        save_truth(out / name, (cmap[:, idx].transpose(1, 2, 0) / 65535).astype(np.float32))
        n += 1
    cmyk = quant(np.concatenate([base(H, W, 3), base(H, W, 1) * 0.3], -1), "u1")
    tifffile.imwrite(out / "cmyk8.tif", cmyk, photometric="separated", metadata=None)
    c = cmyk.astype(np.float64) / 255
    rgb = (1 - c[..., :3]) * (1 - c[..., 3:4])
    save_truth(out / "cmyk8.tif", rgb, cmyk_naive=True)
    rgb8 = quant(base(H, W, 3), "u1")
    for sub in ((1, 1), (2, 1), (2, 2)):
        name = f"jpeg-ycbcr{sub[0]}{sub[1]}.tif"
        try:
            tifffile.imwrite(out / name, rgb8, photometric="ycbcr", compression="jpeg", subsampling=sub, metadata=None)
        except Exception as e:  # noqa: BLE001
            print(f"gen tiff {name}: {e}", file=sys.stderr)
    tifffile.imwrite(out / "jpeg-rgb.tif", rgb8, photometric="rgb", compression="jpeg", metadata=None)
    try:
        tifffile.imwrite(out / "webp-lossless.tif", rgb8, photometric="rgb", compression="webp", compressionargs={"lossless": True}, metadata=None)
        save_truth(out / "webp-lossless.tif", truth_of(rgb8))
    except Exception as e:  # noqa: BLE001
        print(f"gen tiff webp: {e}", file=sys.stderr)
    for comp in ("lzma", "lerc", "jpegxl"):
        try:
            tifffile.imwrite(out / f"{comp}-u16.tif", quant(base(H, W, 3), "u2"), photometric="rgb", compression=comp, metadata=None)
            if comp != "jpegxl":
                save_truth(out / f"{comp}-u16.tif", truth_of(quant(base(H, W, 3), "u2")))
        except Exception as e:  # noqa: BLE001
            print(f"gen tiff {comp}: {e}", file=sys.stderr)
    # Orientation tag (274), every value, and an embedded ICC profile.
    for o in range(1, 9):
        name = f"orient{o}.tif"
        tifffile.imwrite(out / name, rgb8, photometric="rgb", extratags=[(274, 3, 1, o, True)], metadata=None)
        save_truth(out / name, truth_of(rgb8), orientation=o)
    icc = icc_bytes()
    if icc:
        tifffile.imwrite(out / "icc-adobergb.tif", rgb8, photometric="rgb", iccprofile=icc, metadata=None)
        save_truth(out / "icc-adobergb.tif", truth_of(rgb8))
        tifffile.imwrite(out / "icc-adobergb-u16.tif", quant(base(H, W, 3), "u2"), photometric="rgb", iccprofile=icc, metadata=None)
        save_truth(out / "icc-adobergb-u16.tif", truth_of(quant(base(H, W, 3), "u2")))
    # BigTIFF, multi-page (first page compared), Pillow's writer (libtiff) for a second encoder.
    tifffile.imwrite(out / "bigtiff-u16.tif", quant(base(H, W, 3), "u2"), bigtiff=True, photometric="rgb", metadata=None)
    save_truth(out / "bigtiff-u16.tif", truth_of(quant(base(H, W, 3), "u2")))
    from PIL import Image

    Image.fromarray(rgb8).save(out / "pillow-tiff-lzw.tif", compression="tiff_lzw")
    save_truth(out / "pillow-tiff-lzw.tif", truth_of(rgb8))
    Image.fromarray(quant(base(H, W, 4), "u1"), "RGBA").save(out / "pillow-rgba-deflate.tif", compression="tiff_adobe_deflate")
    save_truth(out / "pillow-rgba-deflate.tif", truth_of(quant(base(H, W, 4), "u1")))
    return n


def gen_png(out):
    from PIL import Image, PngImagePlugin
    import imagecodecs

    out = out / "png" / "gen"
    out.mkdir(parents=True, exist_ok=True)
    H, W = 37, 61
    for c, mode in ((1, "L"), (2, "LA"), (3, "RGB"), (4, "RGBA")):
        q = quant(base(H, W, c), "u1")
        Image.fromarray(q[..., 0] if c == 1 else q, mode).save(out / f"pil-{mode}8.png")
        save_truth(out / f"pil-{mode}8.png", truth_of(q))
        q16 = quant(base(H, W, c), "u2")
        name = f"libpng-{mode}16.png"
        (out / name).write_bytes(imagecodecs.png_encode(q16[..., 0] if c == 1 else q16))
        save_truth(out / name, truth_of(q16))
        name = f"libpng-{mode}8-filtered.png"
        (out / name).write_bytes(imagecodecs.png_encode(q[..., 0] if c == 1 else q, level=9))
        save_truth(out / name, truth_of(q))
    for bits in (1, 2, 4):
        q = quant(base(H, W, 1), "u1", bits)
        im = Image.fromarray(q[..., 0], "L")
        pal = []
        for i in range(2**bits):
            v = round(i * 255 / (2**bits - 1))
            pal += [v, v, v]
        im = im.convert("P")
        im.putpalette(pal)
        im = Image.frombytes("P", (W, H), q[..., 0].tobytes())
        im.putpalette(pal)
        im.save(out / f"pal{bits}.png", bits=bits)
        save_truth(out / f"pal{bits}.png", np.repeat(truth_of(q, bits), 3, -1))
    # Palette with tRNS, grey and RGB with tRNS, grey 16 with tRNS.
    rng = np.random.default_rng(5)
    idx = (base(H, W, 1)[..., 0] * 15).round().astype("u1")
    pal = (rng.random((16, 3)) * 255).astype("u1")
    trns = (np.linspace(0, 255, 16)).astype("u1")
    im = Image.frombytes("P", (W, H), idx.tobytes())
    im.putpalette(pal.tobytes())
    im.info["transparency"] = trns.tobytes()
    im.save(out / "pal8-trns.png", transparency=trns.tobytes())
    t = np.concatenate([pal[idx], trns[idx][..., None]], -1)
    save_truth(out / "pal8-trns.png", truth_of(t))
    q = quant(base(H, W, 1), "u1")
    key = int(q[20, 20, 0])
    Image.fromarray(q[..., 0], "L").save(out / "gray8-trns.png", transparency=key)
    a = np.where(q[..., :1] == key, 0, 1).astype(np.float32)
    save_truth(out / "gray8-trns.png", np.concatenate([truth_of(q)] * 3 + [a], -1))
    q = quant(base(H, W, 3), "u1")
    key = tuple(int(v) for v in q[10, 10])
    Image.fromarray(q, "RGB").save(out / "rgb8-trns.png", transparency=key)
    a = np.all(q == np.array(key, "u1"), -1, keepdims=True)
    save_truth(out / "rgb8-trns.png", np.concatenate([truth_of(q), (~a).astype(np.float32)], -1))
    # iCCP, eXIf orientation, gAMA-free sRGB chunk.
    icc = icc_bytes()
    if icc:
        Image.fromarray(q, "RGB").save(out / "iccp-adobergb.png", icc_profile=icc)
        save_truth(out / "iccp-adobergb.png", truth_of(q))
    for o in (3, 6, 8):
        ex = Image.Exif()
        ex[0x0112] = o
        Image.fromarray(q, "RGB").save(out / f"exif-orient{o}.png", exif=ex.tobytes())
        save_truth(out / f"exif-orient{o}.png", truth_of(q), orientation=o)
    info = PngImagePlugin.PngInfo()
    info.add_text("Comment", "fidelity")
    Image.fromarray(q, "RGB").save(out / "text-chunks.png", pnginfo=info)
    save_truth(out / "text-chunks.png", truth_of(q))
    gen_apng(out)


def gen_apng(out):
    """APNG: every dispose_op (none, background, previous) with both blend_ops
    (source, over), frames that change only part of the canvas (Pillow writes them as
    sub-rectangles) with partial alpha, a hidden default image, and a palette animation.
    Compared frame by frame (`file#k`) with Pillow's and FFmpeg's compositing."""
    from PIL import Image

    H, W = 37, 61
    rng = np.random.default_rng(8)
    bg = quant(base(H, W, 4), "u1")
    bg[..., 3] = np.where(np.arange(W)[None, :] < W // 3, 255, 160)

    def frames(n):
        out_frames = [bg.copy()]
        for k in range(1, n):
            f = out_frames[-1].copy()
            y0, x0 = 3 + 5 * k, 4 + 9 * k
            patch = (rng.random((11, 17, 4)) * 255).astype("u1")
            patch[..., 3] = (60 * k) % 256
            f[y0:y0 + 11, x0:x0 + 17] = patch
            out_frames.append(f)
        return [Image.fromarray(f, "RGBA") for f in out_frames]

    for disposal in (0, 1, 2):
        for blend in (0, 1):
            ims = frames(4)
            name = f"apng-dispose{disposal}-blend{blend}.png"
            ims[0].save(out / name, save_all=True, append_images=ims[1:], duration=40,
                        loop=0, disposal=disposal, blend=blend)
    ims = frames(3)
    ims[0].save(out / "apng-hidden-default.png", save_all=True, append_images=ims[1:],
                duration=40, default_image=True)
    pal = [Image.fromarray(np.full((H, W), 40 * k, "u1"), "L").convert("P") for k in range(4)]
    for k, im in enumerate(pal):
        im.paste(255 - 40 * k, (5 * k, 3, 5 * k + 20, 20))
    pal[0].save(out / "apng-palette.png", save_all=True, append_images=pal[1:], duration=40)


def gen_exr(out):
    import OpenEXR

    out = out / "exr" / "gen"
    out.mkdir(parents=True, exist_ok=True)
    H, W = 41, 59
    comps = {
        "none": OpenEXR.NO_COMPRESSION,
        "rle": OpenEXR.RLE_COMPRESSION,
        "zips": OpenEXR.ZIPS_COMPRESSION,
        "zip": OpenEXR.ZIP_COMPRESSION,
        "piz": OpenEXR.PIZ_COMPRESSION,
        "pxr24": OpenEXR.PXR24_COMPRESSION,
        "b44": OpenEXR.B44_COMPRESSION,
        "b44a": OpenEXR.B44A_COMPRESSION,
        "dwaa": OpenEXR.DWAA_COMPRESSION,
        "dwab": OpenEXR.DWAB_COMPRESSION,
    }
    hdr = base(H, W, 4).astype(np.float32)
    hdr[..., :3] *= np.linspace(0.05, 6.0, W)[None, :, None]  # past 1.0: HDR
    for cn, comp in comps.items():
        for pt in ("f2", "f4"):
            for chans in ("Y", "RGB", "RGBA"):
                if cn in ("b44", "b44a") and pt == "f4":
                    continue
                d = hdr.astype(pt)
                if chans == "Y":
                    ch = {"Y": d[..., 1]}
                elif chans == "RGB":
                    ch = {"RGB": d[..., :3]}
                else:
                    ch = {"RGBA": d}
                name = f"{chans}-{pt}-{cn}.exr"
                OpenEXR.File({"compression": comp, "type": OpenEXR.scanlineimage}, ch).write(str(out / name))
                if cn not in ("pxr24", "b44", "b44a", "dwaa", "dwab"):
                    tr = d[..., 1:2] if chans == "Y" else (d[..., :3] if chans == "RGB" else d)
                    save_truth(out / name, tr.astype(np.float32), hdr=True)
    # Tiled, data window inside a larger display window, uint channel, line order.
    t = OpenEXR.TileDescription()
    t.xSize, t.ySize, t.mode = 16, 8, OpenEXR.ONE_LEVEL
    for cn in ("zip", "piz", "none"):
        h = {"compression": comps[cn], "type": OpenEXR.tiledimage, "tiles": t}
        OpenEXR.File(h, {"RGBA": hdr.astype("f2")}).write(str(out / f"tiled-{cn}.exr"))
        save_truth(out / f"tiled-{cn}.exr", hdr.astype("f2").astype(np.float32), hdr=True)
    dw = (np.array([5, 3], np.int32), np.array([5 + W - 1, 3 + H - 1], np.int32))
    disp = (np.array([0, 0], np.int32), np.array([W + 9, H + 7], np.int32))
    h = {"compression": comps["zip"], "type": OpenEXR.scanlineimage, "dataWindow": dw, "displayWindow": disp}
    OpenEXR.File(h, {"RGBA": hdr.astype("f2")}).write(str(out / "datawindow-offset.exr"))
    dw = (np.array([-4, -2], np.int32), np.array([-4 + W - 1, -2 + H - 1], np.int32))
    disp = (np.array([0, 0], np.int32), np.array([W - 9, H - 5], np.int32))
    h = {"compression": comps["zip"], "type": OpenEXR.scanlineimage, "dataWindow": dw, "displayWindow": disp}
    OpenEXR.File(h, {"RGBA": hdr.astype("f2")}).write(str(out / "datawindow-overscan.exr"))
    h = {"compression": comps["zip"], "type": OpenEXR.scanlineimage, "lineOrder": OpenEXR.DECREASING_Y}
    OpenEXR.File(h, {"RGB": hdr[..., :3].astype("f2")}).write(str(out / "decreasing-y.exr"))
    save_truth(out / "decreasing-y.exr", hdr[..., :3].astype("f2").astype(np.float32), hdr=True)
    # Diffuse white: 1.0 everywhere (finding 4: must export as 255).
    OpenEXR.File({"compression": comps["zip"], "type": OpenEXR.scanlineimage}, {"RGB": np.ones((H, W, 3), "f2")}).write(str(out / "white-1.0.exr"))
    save_truth(out / "white-1.0.exr", np.ones((H, W, 3), np.float32), hdr=True)


def gen_jxl(out):
    import imagecodecs

    out = out / "jxl" / "gen"
    out.mkdir(parents=True, exist_ok=True)
    H, W = 45, 71
    for c in (1, 2, 3, 4):
        for dt in ("u1", "u2", "f4"):
            img = base(H, W, c)
            q = quant(img, dt)
            for mode in ("lossless", "d1"):
                if dt == "f4" and mode == "lossless" and c in (2,):
                    continue
                name = f"c{c}-{dt}-{mode}.jxl"
                kw = {"lossless": True} if mode == "lossless" else {"distance": 1.0}
                d = q[..., 0] if c == 1 else q
                try:
                    (out / name).write_bytes(imagecodecs.jpegxl_encode(d, effort=3, **kw))
                except Exception as e:  # noqa: BLE001
                    print(f"gen jxl {name}: {e}", file=sys.stderr)
                    continue
                if mode == "lossless":
                    save_truth(out / name, truth_of(q))
    for eff in (1, 7, 9):
        q = quant(base(H, W, 3), "u1")
        (out / f"effort{eff}-lossless.jxl").write_bytes(imagecodecs.jpegxl_encode(q, effort=eff, lossless=True))
        save_truth(out / f"effort{eff}-lossless.jxl", truth_of(q))


def gen_avif(out):
    import imagecodecs
    from PIL import Image

    out = out / "avif" / "gen"
    out.mkdir(parents=True, exist_ok=True)
    H, W = 48, 66
    for bits in (8, 10, 12):
        for pf in ("444", "422", "420", "400"):
            for c in (3, 4):
                if pf == "400" and c == 4:
                    continue
                q = quant(base(H, W, c), "u1" if bits == 8 else "u2", bits)
                name = f"b{bits}-{pf}-c{c}.avif"
                try:
                    (out / name).write_bytes(imagecodecs.avif_encode(q, level=90, speed=8, bitspersample=bits, pixelformat=getattr(imagecodecs.AVIF.PIXEL_FORMAT, "YUV" + pf)))
                except Exception as e:  # noqa: BLE001
                    print(f"gen avif {name}: {e}", file=sys.stderr)
    q = quant(base(H, W, 3), "u1")
    try:
        (out / "lossless-444.avif").write_bytes(imagecodecs.avif_encode(q, level=100, speed=8, pixelformat=imagecodecs.AVIF.PIXEL_FORMAT.YUV444))
    except Exception as e:  # noqa: BLE001
        print(f"gen avif lossless: {e}", file=sys.stderr)
    # Orientation through irot/imir (Pillow writes them from EXIF orientation).
    for o in (3, 6, 8):
        ex = Image.Exif()
        ex[0x0112] = o
        try:
            Image.fromarray(q).save(out / f"pil-orient{o}.avif", exif=ex.tobytes(), quality=90)
            Path(str(out / f"pil-orient{o}.avif") + ".json").write_text(json.dumps({"orientation": o}))
        except Exception as e:  # noqa: BLE001
            print(f"gen avif orient: {e}", file=sys.stderr)


def gen_heic(out):
    import pillow_heif

    out = out / "heic" / "gen"
    out.mkdir(parents=True, exist_ok=True)
    H, W = 64, 96
    for mode, c, dt in (("RGB", 3, "u1"), ("RGBA", 4, "u1"), ("L", 1, "u1"), ("RGB;16", 3, "u2"), ("RGBA;16", 4, "u2")):
        q = quant(base(H, W, c), dt, 10 if dt == "u2" else None)
        if dt == "u2":
            q = (q.astype("u4") << 6).astype("u2")  # pillow-heif takes 16-bit containers
        d = q[..., 0] if c == 1 else q
        for qual in (90, -1):
            name = f"{mode.replace(';', '-')}-q{qual if qual > 0 else 'lossless'}.heic"
            try:
                hf = pillow_heif.from_bytes(mode=mode, size=(W, H), data=d.tobytes())
                kw = {"quality": qual}
                if qual < 0:
                    kw["chroma"] = 444
                hf.save(out / name, **kw)
            except Exception as e:  # noqa: BLE001
                print(f"gen heic {name}: {e}", file=sys.stderr)


def gen_dpx(out):
    from PIL import Image

    out = out / "dpx" / "gen"
    out.mkdir(parents=True, exist_ok=True)
    H, W = 40, 64
    tmp = out / "_src.png"
    Image.fromarray(quant(base(H, W, 3), "u1")).save(tmp)
    tmpa = out / "_srca.png"
    Image.fromarray(quant(base(H, W, 4), "u1"), "RGBA").save(tmpa)
    tmpg = out / "_srcg.png"
    Image.fromarray(quant(base(H, W, 1), "u1")[..., 0], "L").save(tmpg)
    for depth in (8, 10, 12, 16):
        for endian in ("MSB", "LSB"):
            for src, tag in ((tmp, "rgb"), (tmpa, "rgba"), (tmpg, "gray")):
                name = f"im-{tag}-{depth}-{endian.lower()}.dpx"
                subprocess.run(["convert", str(src), "-depth", str(depth), "-endian", endian, str(out / name)], check=False, capture_output=True)
    for pf in ("rgb24", "rgba", "rgb48le", "rgba64be", "gbrp10le", "gbrp12le", "gray", "gray16be", "gray10le", "gray12le"):
        subprocess.run(["ffmpeg", "-v", "error", "-y", "-i", str(tmpa if "a" in pf[:4] and pf.startswith("rgba") else tmp), "-pix_fmt", pf, str(out / f"ff-{pf}.dpx")], check=False, capture_output=True)
    for p in (tmp, tmpa, tmpg):
        p.unlink()


# --------------------------------------------------------------------------- extra set generation
# JPEG, WebP, GIF, BMP, JPEG 2000, PSD/PSB, Radiance HDR and PFM, Cineon. The byte-level
# writers below (GIF, BMP, PSD, Radiance, PFM, Cineon) exist to reach every variant the
# specifications allow; Pillow, FFmpeg, ImageMagick, OpenImageIO and the GIF suite's expected
# frames check that they wrote what they meant (oracle disagreement shows otherwise).


def run_tool(args, **kw):
    r = subprocess.run([str(a) for a in args], capture_output=True, timeout=120, **kw)
    if r.returncode:
        raise ValueError(f"{Path(str(args[0])).name}: {r.stderr.decode(errors='replace')[:160]}")
    return r


def write_ppm(path, q):
    """8-bit grey (PGM) or RGB (PPM) for the libjpeg-turbo and libwebp command-line tools."""
    h, w = q.shape[:2]
    c = 1 if q.ndim == 2 or q.shape[-1] == 1 else 3
    hdr = f"{'P5' if c == 1 else 'P6'}\n{w} {h}\n255\n".encode()
    Path(path).write_bytes(hdr + np.ascontiguousarray(q[..., :c] if q.ndim == 3 else q).tobytes())


def gen_jpeg(out):
    from PIL import Image
    import imagecodecs

    out = out / "jpeg" / "gen"
    out.mkdir(parents=True, exist_ok=True)
    tmp = out / "_tmp"
    tmp.mkdir(exist_ok=True)
    H, W = 37, 61
    q = quant(base(H, W, 3), "u1")
    g = quant(base(H, W, 1), "u1")
    src, srcg = tmp / "src.ppm", tmp / "src.pgm"
    write_ppm(src, q)
    write_ppm(srcg, g)

    def cj(name, *opts, inp=src):
        try:
            run_tool(["cjpeg", *opts, "-outfile", out / name, inp])
        except Exception as e:  # noqa: BLE001
            print(f"gen jpeg {name}: {e}", file=sys.stderr)

    # Sampling factors (luma h x v) x coding process.
    for samp in ("1x1", "2x1", "2x2", "1x2", "4x1", "4x2", "1x4"):
        for proc, opts in (("baseline", []), ("progressive", ["-progressive"]), ("arith", ["-arithmetic"]), ("arith-prog", ["-arithmetic", "-progressive"])):
            cj(f"q90-{samp}-{proc}.jpg", "-quality", "90", "-sample", samp, *opts)
    for proc, opts in (("baseline", []), ("progressive", ["-progressive"]), ("arith", ["-arithmetic"])):
        cj(f"gray-q90-{proc}.jpg", "-quality", "90", *opts, inp=srcg)
    # Restart intervals: every MCU, every 3 MCUs, every row; with progressive and arithmetic.
    for rst, opts in (("1", []), ("3B", []), ("1", ["-progressive"]), ("2", ["-arithmetic"]), ("1", ["-sample", "4x1"])):
        cj(f"q90-restart{rst}{''.join(o.strip('-')[:4] for o in opts)}.jpg", "-quality", "90", "-restart", rst, *opts)
    cj("q100-1x1-float.jpg", "-quality", "100", "-sample", "1x1", "-dct", "float")
    cj("q75-2x2-optimize.jpg", "-quality", "75", "-optimize")
    cj("q90-rgb-notransform.jpg", "-quality", "95", "-rgb")
    cj("q50-2x2-smooth.jpg", "-quality", "50", "-smooth", "30")
    # Odd sizes: smaller than one MCU, not a multiple of it.
    for h, w in ((1, 1), (5, 7), (9, 17), (16, 1), (1, 33)):
        p = tmp / f"o{w}x{h}.ppm"
        write_ppm(p, quant(base(h, w, 3), "u1"))
        for samp in ("2x2", "4x1"):
            cj(f"odd{w}x{h}-{samp}.jpg", "-quality", "90", "-sample", samp, inp=p)
    # 12-bit (libjpeg-turbo 3), lossless (predictors 1-7, 2-16 bits), through imagecodecs.
    q12 = quant(base(H, W, 3), "u2", 12)
    g12 = quant(base(H, W, 1), "u2", 12)[..., 0]
    for nm, d in (("rgb", q12), ("gray", g12)):
        try:
            (out / f"12bit-{nm}-q90.jpg").write_bytes(imagecodecs.jpeg8_encode(d, level=90, bitspersample=12))
            Path(str(out / f"12bit-{nm}-q90.jpg") + ".json").write_text(json.dumps({"bits": 12}))
        except Exception as e:  # noqa: BLE001
            print(f"gen jpeg 12-bit {nm}: {e}", file=sys.stderr)
    for bits in (2, 8, 12, 16):
        for pred in (1, 2, 3, 4, 5, 6, 7):
            if bits != 8 and pred not in (1, 4, 7):
                continue
            for nm, c in (("gray", 1), ("rgb", 3)):
                if bits in (2, 16) and nm == "rgb" and pred != 1:
                    continue
                dq = quant(base(H, W, c), "u1" if bits <= 8 else "u2", bits)
                d = dq[..., 0] if c == 1 else dq
                name = f"lossless-{nm}-{bits}bit-p{pred}.jpg"
                try:
                    (out / name).write_bytes(imagecodecs.jpeg8_encode(d, lossless=True, predictor=pred, bitspersample=bits))
                except Exception as e:  # noqa: BLE001
                    print(f"gen jpeg {name}: {e}", file=sys.stderr)
                    continue
                save_truth(out / name, truth_of(dq, bits), bits=bits)
    # CMYK and YCCK with the Adobe marker (libjpeg-turbo writes it for both).
    c4 = quant(base(H, W, 4), "u1")
    for oc in ("CMYK", "YCCK"):
        try:
            (out / f"adobe-{oc.lower()}.jpg").write_bytes(imagecodecs.jpeg8_encode(c4, level=95, colorspace="CMYK", outcolorspace=oc))
        except Exception as e:  # noqa: BLE001
            print(f"gen jpeg {oc}: {e}", file=sys.stderr)
    try:
        Image.fromarray(c4, "CMYK").save(out / "pillow-cmyk.jpg", quality=95)
    except Exception as e:  # noqa: BLE001
        print(f"gen jpeg pillow cmyk: {e}", file=sys.stderr)
    # EXIF orientation 1-8 (exiftool writes the tag into a copy of a baseline file).
    base_jpg = out / "q90-2x2-baseline.jpg"
    for o in range(1, 9):
        dst = out / f"exif-orient{o}.jpg"
        ex = Image.Exif()
        ex[0x0112] = o
        Image.fromarray(q).save(dst, quality=90, exif=ex.tobytes())
        Path(str(dst) + ".json").write_text(json.dumps({"orientation": o}))
    # ICC: a profile in one APP2 marker, and one split over three (padded past 64 KiB).
    icc = icc_bytes()
    if icc:
        Image.fromarray(q).save(out / "icc-p3.jpg", quality=92, icc_profile=icc)
        big = bytearray(icc) + bytes(150_000)
        big[0:4] = len(big).to_bytes(4, "big")
        Image.fromarray(q).save(out / "icc-p3-multichunk.jpg", quality=92, icc_profile=bytes(big))
    # MPF: a second, different image after the first (Pillow writes the MP index).
    try:
        q2 = quant(base(H + 6, W - 10, 3, seed=7)[:, ::-1], "u1")
        Image.fromarray(q).save(out / "mpf-2.jpg", format="MPO", save_all=True, append_images=[Image.fromarray(q2)], quality=90)
    except Exception as e:  # noqa: BLE001
        print(f"gen jpeg mpo: {e}", file=sys.stderr)
    shutil.rmtree(tmp, ignore_errors=True)
    _ = base_jpg


def gen_webp(out):
    from PIL import Image

    out = out / "webp" / "gen"
    out.mkdir(parents=True, exist_ok=True)
    tmp = out / "_tmp"
    tmp.mkdir(exist_ok=True)
    H, W = 37, 61
    q3 = quant(base(H, W, 3), "u1")
    q4 = quant(base(H, W, 4), "u1")
    s3, s4 = tmp / "s3.png", tmp / "s4.png"
    Image.fromarray(q3).save(s3)
    Image.fromarray(q4, "RGBA").save(s4)

    def cw(name, inp, *opts, truth=None):
        try:
            run_tool(["cwebp", "-quiet", *opts, inp, "-o", out / name])
        except Exception as e:  # noqa: BLE001
            print(f"gen webp {name}: {e}", file=sys.stderr)
            return
        if truth is not None:
            save_truth(out / name, truth_of(truth))

    for qv in (95, 75, 30):
        cw(f"lossy-q{qv}.webp", s3, "-q", str(qv))
    cw("lossy-q90-sharpyuv.webp", s3, "-q", "90", "-sharp_yuv")
    cw("lossy-q90-m0.webp", s3, "-q", "90", "-m", "0")
    cw("lossy-q90-m6-segments1.webp", s3, "-q", "90", "-m", "6", "-segments", "1")
    cw("lossy-q90-f100-sharp7.webp", s3, "-q", "90", "-f", "100", "-sharpness", "7")
    cw("lossy-q90-strong-off.webp", s3, "-q", "90", "-nostrong")
    cw("lossy-q90-partitions3.webp", s3, "-q", "90", "-partition_limit", "50", "-segments", "4")
    for aq, af in ((100, "none"), (100, "fast"), (100, "best"), (50, "best")):
        cw(f"lossy-alpha-q90-aq{aq}-{af}.webp", s4, "-q", "90", "-alpha_q", str(aq), "-alpha_filter", af, "-exact")
    cw("lossy-alpha-q90-alpha-nocomp.webp", s4, "-q", "90", "-alpha_method", "0", "-exact")
    for z in (0, 3, 6, 9):
        cw(f"lossless-rgb-z{z}.webp", s3, "-lossless", "-z", str(z), "-exact", truth=q3)
        cw(f"lossless-rgba-z{z}.webp", s4, "-lossless", "-z", str(z), "-exact", truth=q4)
    for m in (0, 4, 6):
        cw(f"lossless-rgba-m{m}-q100.webp", s4, "-lossless", "-m", str(m), "-q", "100", "-exact", truth=q4)
    cw("lossless-near60.webp", s3, "-near_lossless", "60")
    # Colour-indexing (palette) transforms with pixel bundling: 2, 4, 16 and 200 colours.
    rng = np.random.default_rng(11)
    for n in (2, 4, 16, 200):
        pal = (rng.random((n, 4)) * 255).astype("u1")
        pal[:, 3] = np.where(np.arange(n) % 3 == 0, 255, pal[:, 3])
        idx = (base(H, W, 1)[..., 0] * (n - 1)).round().astype(int)
        img = pal[idx]
        p = tmp / f"pal{n}.png"
        Image.fromarray(img, "RGBA").save(p)
        cw(f"lossless-palette{n}.webp", p, "-lossless", "-exact", truth=img)
    # Grey (subtract-green, cross-colour transforms trivial) and a one-colour image.
    g = np.repeat(quant(base(H, W, 1), "u1"), 3, -1)
    Image.fromarray(g).save(tmp / "g.png")
    cw("lossless-gray.webp", tmp / "g.png", "-lossless", "-exact", truth=g)
    one = np.full((H, W, 3), (40, 120, 220), "u1")
    Image.fromarray(one).save(tmp / "one.png")
    cw("lossless-onecolour.webp", tmp / "one.png", "-lossless", truth=one)
    # Odd sizes.
    for h, w in ((1, 1), (3, 1), (1, 17), (13, 31)):
        o = quant(base(h, w, 4), "u1")
        Image.fromarray(o, "RGBA").save(tmp / "o.png")
        cw(f"lossless-odd{w}x{h}.webp", tmp / "o.png", "-lossless", "-exact", truth=o)
        cw(f"lossy-odd{w}x{h}.webp", tmp / "o.png", "-q", "90")
    # Animation: frames at offsets with blend/no-blend and dispose none/background, mixed
    # lossy and lossless (webpmux -frame file +duration+x+y+dispose[+b|-b]).
    frames = []
    for k in range(4):
        fw, fh = (W, H) if k == 0 else (24 - 4 * k, 16 + 2 * k)
        f = quant(base(fh, fw, 4, seed=20 + k), "u1")
        if k == 0:
            f[..., 3] = 255
        else:
            f[..., 3] = np.where((np.arange(fw)[None, :] + np.arange(fh)[:, None]) % 5 == 0, 0, f[..., 3])
        p = tmp / f"f{k}.png"
        Image.fromarray(f, "RGBA").save(p)
        fwp = tmp / f"f{k}.webp"
        lossless = k % 2 == 0
        try:
            run_tool(["cwebp", "-quiet", *(["-lossless", "-exact"] if lossless else ["-q", "90", "-exact"]), p, "-o", fwp])
        except Exception as e:  # noqa: BLE001
            print(f"gen webp frame: {e}", file=sys.stderr)
            return
        frames.append(fwp)
    for name, specs in (
        ("anim-blend-dispose-none.webp", ["+100", "+100+10+6+0+b", "+100+20+8+0+b", "+100+30+12+0+b"]),
        ("anim-noblend-dispose-bg.webp", ["+100", "+100+10+6+1-b", "+100+20+8+1-b", "+100+30+12+0-b"]),
        ("anim-mixed.webp", ["+100", "+50+10+6+1+b", "+50+20+8+0-b", "+80+30+12+1+b"]),
    ):
        args = ["webpmux"]
        for f, s in zip(frames, specs):
            args += ["-frame", f, s]
        args += ["-loop", "0", "-bgcolor", "0,0,0,0", "-o", out / name]
        try:
            run_tool(args)
        except Exception as e:  # noqa: BLE001
            print(f"gen webp {name}: {e}", file=sys.stderr)
    try:
        fr = [Image.fromarray(quant(base(H, W, 4, seed=30 + k), "u1"), "RGBA") for k in range(3)]
        fr[0].save(out / "anim-pillow-lossless.webp", save_all=True, append_images=fr[1:], lossless=True, duration=80)
        fr[0].save(out / "anim-pillow-lossy.webp", save_all=True, append_images=fr[1:], quality=85, duration=80)
    except Exception as e:  # noqa: BLE001
        print(f"gen webp pillow anim: {e}", file=sys.stderr)
    # ICC and EXIF orientation in the extended (VP8X) container.
    icc = icc_bytes()
    if icc:
        try:
            run_tool(["webpmux", "-set", "icc", tmp / "icc.icc", out / "lossy-q95.webp", "-o", out / "icc-p3.webp"], input=None) if (tmp / "icc.icc").write_bytes(icc) else None
        except Exception as e:  # noqa: BLE001
            print(f"gen webp icc: {e}", file=sys.stderr)
    shutil.rmtree(tmp, ignore_errors=True)


def lzw_gif(idx, min_code):
    """GIF LZW (variable code size, a clear code at the start and when the table fills)."""
    clear, eoi = 1 << min_code, (1 << min_code) + 1
    size, nxt = min_code + 1, eoi + 1
    table = {(i,): i for i in range(clear)}
    codes = [(clear, size)]
    w = ()
    for k in idx.ravel().tolist():
        wk = w + (k,)
        if wk in table:
            w = wk
            continue
        codes.append((table[w], size))
        if nxt >= 4095:
            codes.append((clear, size))
            table = {(i,): i for i in range(clear)}
            size, nxt = min_code + 1, eoi + 1
        else:
            table[wk] = nxt
            nxt += 1
            if nxt > (1 << size) and size < 12:
                size += 1
        w = (k,)
    if w:
        codes.append((table[w], size))
    codes.append((eoi, size))
    acc = nbits = 0
    out = bytearray()
    for c, s in codes:
        acc |= c << nbits
        nbits += s
        while nbits >= 8:
            out.append(acc & 0xFF)
            acc >>= 8
            nbits -= 8
    if nbits:
        out.append(acc & 0xFF)
    blocks = bytearray([min_code])
    for i in range(0, len(out), 255):
        blocks += bytes([len(out[i : i + 255])]) + out[i : i + 255]
    return bytes(blocks + b"\x00")


def gif_bytes(sw, sh, gct, frames, bg=0, loop=True):
    """frames: dicts with idx (h, w), x, y, lct, interlace, transparent, disposal, delay."""
    import struct

    def table_bits(pal):
        n = max(2, len(pal))
        return max(1, int(np.ceil(np.log2(n))))

    b = bytearray(b"GIF89a")
    flags = 0
    if gct is not None:
        tb = table_bits(gct)
        flags = 0x80 | ((tb - 1) << 4) | (tb - 1)
    b += struct.pack("<HHBBB", sw, sh, flags, bg, 0)
    if gct is not None:
        pal = np.zeros((1 << tb, 3), "u1")
        pal[: len(gct)] = gct
        b += pal.tobytes()
    if loop and len(frames) > 1:
        b += b"\x21\xff\x0bNETSCAPE2.0\x03\x01\x00\x00\x00"
    for f in frames:
        t = f.get("transparent")
        packed = (f.get("disposal", 0) << 2) | (1 if t is not None else 0)
        b += struct.pack("<BBBBHBB", 0x21, 0xF9, 4, packed, f.get("delay", 10), t or 0, 0)
        idx = f["idx"]
        h, w = idx.shape
        lct = f.get("lct")
        fl = 0x40 if f.get("interlace") else 0
        if lct is not None:
            tb = table_bits(lct)
            fl |= 0x80 | (tb - 1)
        b += struct.pack("<BHHHHB", 0x2C, f.get("x", 0), f.get("y", 0), w, h, fl)
        pal_n = len(lct) if lct is not None else len(gct)
        if lct is not None:
            pal = np.zeros((1 << tb, 3), "u1")
            pal[: len(lct)] = lct
            b += pal.tobytes()
        rows = idx
        if f.get("interlace"):
            order = list(range(0, h, 8)) + list(range(4, h, 8)) + list(range(2, h, 4)) + list(range(1, h, 2))
            rows = idx[order]
        mc = max(2, int(np.ceil(np.log2(max(pal_n, 2)))))
        b += lzw_gif(rows, f.get("min_code", mc))
    b += b"\x3b"
    return bytes(b)


def gen_gif(out):
    from PIL import Image

    out = out / "gif" / "gen"
    out.mkdir(parents=True, exist_ok=True)
    rng = np.random.default_rng(3)
    H, W = 37, 61

    def pal_of(n, seed):
        return (np.random.default_rng(seed).random((n, 3)) * 255).astype("u1")

    def idx_of(h, w, n, seed=1):
        return np.clip((base(h, w, 1, seed)[..., 0] * n).astype(int), 0, n - 1).astype("u1")

    def single(name, n, interlace=False, transparent=None, h=H, w=W, min_code=None):
        pal = pal_of(n, n)
        idx = idx_of(h, w, n)
        if transparent is not None:
            idx[(np.arange(w)[None, :] + np.arange(h)[:, None]) % 7 == 0] = transparent
        f = {"idx": idx, "interlace": interlace, "transparent": transparent}
        if min_code:
            f["min_code"] = min_code
        (out / name).write_bytes(gif_bytes(w, h, pal, [f]))
        rgb = pal[idx].astype(np.float32) / 255
        a = np.ones((h, w, 1), np.float32) if transparent is None else (idx != transparent)[..., None].astype(np.float32)
        save_truth(out / name, np.concatenate([rgb, a], -1))

    for n in (2, 4, 16, 128, 256):
        single(f"pal{n}.gif", n)
        single(f"pal{n}-interlaced.gif", n, interlace=True)
    single("pal16-transparent.gif", 16, transparent=5)
    single("pal256-transparent-interlaced.gif", 256, transparent=200, interlace=True)
    single("pal2-mincode8.gif", 2, min_code=8)
    for h, w in ((1, 1), (2, 1), (1, 700), (3, 5), (9, 1), (5, 4)):
        single(f"odd{w}x{h}.gif", 16, h=h, w=w)
        single(f"odd{w}x{h}-interlaced.gif", 16, h=h, w=w, interlace=True)
    # A large picture: the code table fills and clears many times.
    single("big-pal256.gif", 256, h=211, w=307)
    # Animations: disposal 0-3, local palettes, transparency, frames at offsets.
    gct = pal_of(64, 1)
    full = idx_of(H, W, 64)
    for disp in (0, 1, 2, 3):
        frames = [{"idx": full, "delay": 20}]
        for k in range(1, 4):
            fh, fw = 9 + 3 * k, 14 + 4 * k
            idx = idx_of(fh, fw, 64, seed=40 + k)
            idx[::3, ::2] = 63
            frames.append({"idx": idx, "x": 5 * k, "y": 3 * k, "transparent": 63, "disposal": disp, "delay": 20})
        frames[0]["disposal"] = disp
        (out / f"anim-dispose{disp}.gif").write_bytes(gif_bytes(W, H, gct, frames))
    frames = [{"idx": full, "delay": 10, "disposal": 1}]
    for k in range(1, 4):
        fh, fw = 10 + k, 20 + k
        lct = pal_of(4 << k, 90 + k)
        frames.append({"idx": idx_of(fh, fw, 4 << k, seed=50 + k), "x": 7 * k, "y": 2 * k, "lct": lct, "disposal": (k % 3) + 1, "interlace": k == 2, "delay": 10})
    (out / "anim-local-palettes.gif").write_bytes(gif_bytes(W, H, gct, frames))
    # First frame not covering the canvas (transparent around it), no global table.
    lct = pal_of(16, 77)
    (out / "anim-partial-first-noglobal.gif").write_bytes(gif_bytes(W, H, None, [
        {"idx": idx_of(20, 30, 16), "x": 10, "y": 8, "lct": lct, "delay": 10, "disposal": 2},
        {"idx": idx_of(25, 40, 16, seed=3), "x": 15, "y": 5, "lct": lct[::-1].copy(), "delay": 10},
    ]))
    # A frame reaching past the logical screen (clipped).
    (out / "anim-frame-past-screen.gif").write_bytes(gif_bytes(W, H, gct, [
        {"idx": full, "delay": 10},
        {"idx": idx_of(20, 30, 64, seed=9), "x": W - 12, "y": H - 9, "delay": 10},
    ]))
    # Pillow's and giflib's own encoders.
    q = quant(base(H, W, 3), "u1")
    im = Image.fromarray(q).quantize(200)
    im.save(out / "pillow-quantized.gif")
    save_truth(out / "pillow-quantized.gif", np.asarray(im.convert("RGB")).astype(np.float32) / 255)
    try:
        import imagecodecs

        idx = idx_of(H, W, 256)
        cm = pal_of(256, 4)
        (out / "giflib.gif").write_bytes(imagecodecs.gif_encode(idx, colormap=cm))
        save_truth(out / "giflib.gif", cm[idx].astype(np.float32) / 255)
    except Exception as e:  # noqa: BLE001
        print(f"gen gif giflib: {e}", file=sys.stderr)
    _ = rng


def bmp_bytes(w, h, bpp, pix, *, header=40, comp=0, masks=None, palette=None, topdown=False, cs=None, icc=None, gamma=None, pal_entries=None):
    """A BMP of any header size (12 OS/2 core, 40, 52, 56, 108, 124). `pix` is the encoded
    pixel array (rows bottom-up unless top-down)."""
    import struct

    pal = b""
    if palette is not None:
        p = np.asarray(palette, "u1")
        if header == 12:
            pal = p[:, ::-1].tobytes()  # BGR triples
        else:
            pal = np.concatenate([p[:, ::-1], np.zeros((len(p), 1), "u1")], -1).tobytes()
    masks_b = b""
    if comp in (3, 6) and header == 40:
        masks_b = struct.pack("<III", *masks[:3]) if comp == 3 else struct.pack("<IIII", *masks)
    off = 14 + header + len(masks_b) + len(pal)
    if header == 12:
        info = struct.pack("<IHHHH", 12, w, h, 1, bpp)
    else:
        hh = -h if topdown else h
        info = struct.pack("<IiiHHIIiiII", header, w, hh, 1, bpp, comp, len(pix), 2835, 2835, pal_entries if pal_entries is not None else (len(palette) if palette is not None else 0), 0)
        if header >= 52:
            m = list(masks or (0, 0, 0, 0)) + [0] * 4
            info += struct.pack("<III", *m[:3])
        if header >= 56:
            info += struct.pack("<I", m[3])
        if header >= 108:
            cstype = cs if cs is not None else 0x73524742  # 'sRGB'
            info += struct.pack("<I", cstype)
            if gamma is not None:
                # CIEXYZTRIPLE endpoints (2.30 fixed) for sRGB-ish primaries, gamma (16.16).
                xyz = [(0.4124, 0.2126, 0.0193), (0.3576, 0.7152, 0.1192), (0.1805, 0.0722, 0.9505)]
                info += b"".join(struct.pack("<iii", *[int(v * (1 << 30)) for v in t]) for t in xyz)
                info += struct.pack("<III", *[int(gamma * 65536)] * 3)
            else:
                info += bytes(36) + bytes(12)
        if header >= 124:
            prof_off = header + len(masks_b) + len(pal) + len(pix) if icc else 0
            info += struct.pack("<IIII", 4, prof_off, len(icc) if icc else 0, 0)
    body = info + masks_b + pal + pix + (icc or b"")
    return b"BM" + struct.pack("<IHHI", 14 + len(body), 0, 0, off) + body


def pack_rows(idx, bpp, topdown=False):
    """Palette indices or packed integers, rows padded to 4 bytes, bottom-up by default."""
    h, w = idx.shape[:2]
    rows = []
    for y in range(h):
        r = idx[y]
        if bpp < 8:
            per = 8 // bpp
            v = np.zeros(((w + per - 1) // per) * per, "u1")
            v[:w] = r
            v = v.reshape(-1, per)
            byte = np.zeros(len(v), "u1")
            for i in range(per):
                byte |= (v[:, i] << (8 - bpp * (i + 1))).astype("u1")
            rb = byte.tobytes()
        elif bpp == 8:
            rb = r.astype("u1").tobytes()
        elif bpp == 16:
            rb = r.astype("<u2").tobytes()
        elif bpp == 24:
            rb = r.astype("u1").tobytes()
        else:
            rb = r.astype("<u4").tobytes()
        rb += bytes((-len(rb)) % 4)
        rows.append(rb)
    if not topdown:
        rows = rows[::-1]
    return b"".join(rows)


def rle_bmp(idx, bpp):
    """RLE8/RLE4: encoded runs and absolute runs, an end-of-line per row, end of bitmap."""
    out = bytearray()
    h, w = idx.shape
    for y in range(h - 1, -1, -1):
        r = idx[y].tolist()
        x = 0
        while x < w:
            n = 1
            if bpp == 8:
                while x + n < w and r[x + n] == r[x] and n < 255:
                    n += 1
            else:
                while x + n < w and n < 255 and r[x + n] == r[x + (n % 2)]:
                    n += 1
            if n >= 3 or w - x < 3:
                v = r[x] if bpp == 8 else (r[x] << 4) | (r[x + 1] if x + 1 < w else 0)
                out += bytes([n, v])
                x += n
                continue
            m = 3
            while x + m < w and m < 255 and not (x + m + 2 < w and r[x + m] == r[x + m + 1] == r[x + m + 2]):
                m += 1
            seg = r[x : x + m]
            if bpp == 8:
                data = bytes(seg)
            else:
                s = seg + [0] * (len(seg) % 2)
                data = bytes((s[i] << 4) | s[i + 1] for i in range(0, len(s), 2))
            out += bytes([0, m]) + data + bytes(len(data) % 2)
            x += m
        if y:
            out += b"\x00\x00"  # end of line (the last row ends with end of bitmap)
    out += b"\x00\x01"
    return bytes(out)


def gen_bmp(out):
    out = out / "bmp" / "gen"
    out.mkdir(parents=True, exist_ok=True)
    H, W = 37, 61
    q = quant(base(H, W, 4), "u1")

    def save(name, data, truth):
        (out / name).write_bytes(data)
        save_truth(out / name, truth.astype(np.float32))

    for bpp in (1, 2, 4, 8):
        n = 1 << bpp
        pal = (np.random.default_rng(bpp).random((n, 3)) * 255).astype("u1")
        idx = np.clip((base(H, W, 1)[..., 0] * n).astype(int), 0, n - 1).astype("u1")
        t = pal[idx] / 255
        for hdr in (12, 40, 108, 124):
            if hdr == 12 and bpp == 2:
                continue
            save(f"pal{bpp}-h{hdr}.bmp", bmp_bytes(W, H, bpp, pack_rows(idx, bpp), header=hdr, palette=pal), t)
        save(f"pal{bpp}-topdown.bmp", bmp_bytes(W, H, bpp, pack_rows(idx, bpp, True), palette=pal, topdown=True), t)
        if bpp == 8:
            # Fewer palette entries than the depth allows (biClrUsed).
            idx2 = (idx % 37).astype("u1")
            save("pal8-37entries.bmp", bmp_bytes(W, H, 8, pack_rows(idx2, 8), palette=pal[:37]), pal[idx2] / 255)
        if bpp in (4, 8):
            save(f"rle{bpp}.bmp", bmp_bytes(W, H, bpp, rle_bmp(idx, bpp), comp=1 if bpp == 8 else 2, palette=pal), t)
            flat = idx.copy()
            flat[:, 10:40] = 3
            save(f"rle{bpp}-runs.bmp", bmp_bytes(W, H, bpp, rle_bmp(flat, bpp), comp=1 if bpp == 8 else 2, palette=pal), pal[flat] / 255)
    rgb, a = q[..., :3].astype(np.uint32), q[..., 3].astype(np.uint32)
    # 16-bit: 555 (BI_RGB), 565, 444+4 alpha, 1555 through bit fields.
    for name, masks, hdr in (("rgb555", None, 40), ("rgb565", (0xF800, 0x07E0, 0x001F, 0), 40), ("rgb565-v4", (0xF800, 0x07E0, 0x001F, 0), 108), ("argb4444-v4", (0x0F00, 0x00F0, 0x000F, 0xF000), 108), ("argb1555-v5", (0x7C00, 0x03E0, 0x001F, 0x8000), 124), ("rgb555-bf-v3", (0x7C00, 0x03E0, 0x001F, 0), 56)):
        mk = masks or (0x7C00, 0x03E0, 0x001F, 0)
        v = np.zeros((H, W), np.uint32)
        chans = []
        for c, m in enumerate(mk):
            if not m:
                chans.append(None)
                continue
            sh = (m & -m).bit_length() - 1
            bits = bin(m).count("1")
            src = a if c == 3 else rgb[..., c]
            code = np.round(src / 255 * ((1 << bits) - 1)).astype(np.uint32)
            v |= code << sh
            chans.append(code / ((1 << bits) - 1))
        tr = np.stack([chans[0], chans[1], chans[2], chans[3] if chans[3] is not None else np.ones((H, W))], -1)
        comp = 0 if masks is None else 3
        save(f"{name}.bmp", bmp_bytes(W, H, 16, pack_rows(v, 16), header=hdr, comp=comp, masks=mk), tr)
    bgr = q[..., 2::-1]
    save("rgb24.bmp", bmp_bytes(W, H, 24, pack_rows(bgr, 24)), q[..., :3] / 255)
    save("rgb24-topdown.bmp", bmp_bytes(W, H, 24, pack_rows(bgr, 24, True), topdown=True), q[..., :3] / 255)
    save("rgb24-h12.bmp", bmp_bytes(W, H, 24, pack_rows(bgr, 24), header=12), q[..., :3] / 255)
    for w in (1, 2, 3, 5):
        o = quant(base(3, w, 3), "u1")
        save(f"rgb24-odd{w}x3.bmp", bmp_bytes(w, 3, 24, pack_rows(o[..., ::-1], 24)), o / 255)
    x32 = (rgb[..., 2] | (rgb[..., 1] << 8) | (rgb[..., 0] << 16)).astype(np.uint32)
    save("rgb32-x8.bmp", bmp_bytes(W, H, 32, pack_rows(x32, 32)), q[..., :3] / 255)
    a32 = x32 | (a << 24)
    tr4 = q / 255
    save("argb32-v4.bmp", bmp_bytes(W, H, 32, pack_rows(a32, 32), header=108, comp=3, masks=(0xFF0000, 0xFF00, 0xFF, 0xFF000000)), tr4)
    save("argb32-v5.bmp", bmp_bytes(W, H, 32, pack_rows(a32, 32), header=124, comp=3, masks=(0xFF0000, 0xFF00, 0xFF, 0xFF000000)), tr4)
    save("argb32-v5-topdown.bmp", bmp_bytes(W, H, 32, pack_rows(a32, 32, True), header=124, comp=3, masks=(0xFF0000, 0xFF00, 0xFF, 0xFF000000), topdown=True), tr4)
    save("argb32-v3-alphabitfields.bmp", bmp_bytes(W, H, 32, pack_rows(a32, 32), header=40, comp=6, masks=(0xFF0000, 0xFF00, 0xFF, 0xFF000000)), tr4)
    rgba = (rgb[..., 0] << 24) | (rgb[..., 1] << 16) | (rgb[..., 2] << 8) | a
    save("rgba32-bf-swapped.bmp", bmp_bytes(W, H, 32, pack_rows(rgba, 32), header=124, comp=3, masks=(0xFF000000, 0xFF0000, 0xFF00, 0xFF)), tr4)
    # 10-10-10 bit fields: 10-bit samples.
    q10 = quant(base(H, W, 3), "u2", 10).astype(np.uint32)
    v10 = (q10[..., 0] << 20) | (q10[..., 1] << 10) | q10[..., 2]
    save("rgb101010.bmp", bmp_bytes(W, H, 32, pack_rows(v10, 32), header=124, comp=3, masks=(0x3FF00000, 0xFFC00, 0x3FF, 0)), q10 / 1023)
    # v5 with an embedded ICC profile; v4 calibrated RGB with a gamma (stored gamma 2.2).
    icc = icc_bytes()
    if icc:
        save("rgb24-v5-icc.bmp", bmp_bytes(W, H, 24, pack_rows(bgr, 24), header=124, cs=0x4D424544, icc=icc), q[..., :3] / 255)
    save("rgb24-v4-calibrated-g22.bmp", bmp_bytes(W, H, 24, pack_rows(bgr, 24), header=108, cs=0, gamma=2.2), q[..., :3] / 255)


def psd_bytes(w, h, mode, depth, chans, *, version=1, rle=False, icc=None, cmdata=b"", layers_alpha=False):
    """A PSD/PSB with only the merged image: `chans` is a list of (h, w) planes already in the
    file's sample type."""
    import struct

    def res(rid, data):
        d = bytes(data) + bytes(len(data) % 2)
        return b"8BIM" + struct.pack(">H", rid) + b"\x00\x00" + struct.pack(">I", len(data)) + d

    b = b"8BPS" + struct.pack(">H", version) + bytes(6) + struct.pack(">HIIHH", len(chans), h, w, depth, mode)
    b += struct.pack(">I", len(cmdata)) + cmdata
    rs = res(1039, icc) if icc else b""
    b += struct.pack(">I", len(rs)) + rs
    if layers_alpha:
        # A layer section with a negative layer count: the merged image's first extra channel
        # is its transparency. No layer records follow (count -0 is not possible: use -1 with
        # no data would be malformed), so only the sign matters: an empty layer info block.
        lm = b""
    else:
        lm = b""
    b += struct.pack(">I" if version == 1 else ">Q", len(lm)) + lm
    dt = {1: None, 8: ">u1", 16: ">u2", 32: ">f4"}[depth]
    raw = []
    for c in chans:
        if depth == 1:
            rows = [np.packbits(r.astype(bool)).tobytes() for r in c]
        else:
            rows = [r.astype(dt).tobytes() for r in c]
        raw.append(rows)
    if not rle:
        b += struct.pack(">H", 0) + b"".join(b"".join(rows) for rows in raw)
        return b
    enc = [[packbits(r) for r in rows] for rows in raw]
    counts = b"".join(struct.pack(">H" if version == 1 else ">I", *[len(r)]) for rows in enc for r in rows)
    return b + struct.pack(">H", 1) + counts + b"".join(b"".join(rows) for rows in enc)


def packbits(b):
    out = bytearray()
    i, n = 0, len(b)
    while i < n:
        j = i
        while j + 1 < n and b[j + 1] == b[i] and j - i < 127:
            j += 1
        if j > i:
            out += bytes([257 - (j - i + 1), b[i]])
            i = j + 1
            continue
        j = i
        while j + 1 < n and (j + 2 >= n or b[j + 1] != b[j + 2]) and j - i < 127:
            j += 1
        out += bytes([j - i]) + b[i : j + 1]
        i = j + 1
    return bytes(out)


def gen_psd(out):
    import subprocess as sp

    out = out / "psd" / "gen"
    out.mkdir(parents=True, exist_ok=True)
    H, W = 37, 61
    img = base(H, W, 3)
    icc = icc_bytes()
    for version in (1, 2):
        ext = "psd" if version == 1 else "psb"
        for rle in (False, True):
            tag = f"{'rle' if rle else 'raw'}.{ext}"
            for depth in (8, 16, 32):
                dtn = {8: "u1", 16: "u2", 32: "f4"}[depth]
                q = quant(img, dtn)
                g = quant(base(H, W, 1), dtn)
                if depth == 32 and rle:
                    continue  # Photoshop never writes RLE for 32-bit
                nm = f"rgb{depth}-{tag}"
                (out / nm).write_bytes(psd_bytes(W, H, 3, depth, [q[..., c] for c in range(3)], version=version, rle=rle))
                save_truth(out / nm, truth_of(q), **({"linear": True} if depth == 32 else {}))
                nm = f"gray{depth}-{tag}"
                (out / nm).write_bytes(psd_bytes(W, H, 1, depth, [g[..., 0]], version=version, rle=rle))
                save_truth(out / nm, truth_of(g), **({"linear": True} if depth == 32 else {}))
            # Bitmap (1 bit, 1 = black), indexed, multichannel, duotone (shown as grey).
            bits = base(H, W, 1)[..., 0] < 0.5
            nm = f"bitmap-{tag}"
            (out / nm).write_bytes(psd_bytes(W, H, 0, 1, [bits], version=version, rle=rle))
            save_truth(out / nm, (~bits)[..., None].astype(np.float32))
            pal = (np.random.default_rng(2).random((256, 3)) * 255).astype("u1")
            idx = (base(H, W, 1)[..., 0] * 255).round().astype("u1")
            nm = f"indexed-{tag}"
            (out / nm).write_bytes(psd_bytes(W, H, 2, 8, [idx], version=version, rle=rle, cmdata=pal.T.tobytes()))
            save_truth(out / nm, pal[idx] / 255)
    q = quant(img, "u1")
    if icc:
        (out / "rgb8-icc.psd").write_bytes(psd_bytes(W, H, 3, 8, [q[..., c] for c in range(3)], rle=True, icc=icc))
        save_truth(out / "rgb8-icc.psd", truth_of(q))
    q4 = quant(base(H, W, 4), "u1")
    (out / "rgb8-extra-channel.psd").write_bytes(psd_bytes(W, H, 3, 8, [q4[..., c] for c in range(4)], rle=True))
    g = quant(base(H, W, 1), "u1")
    (out / "duotone8.psd").write_bytes(psd_bytes(W, H, 8, 8, [g[..., 0]], rle=True, cmdata=bytes(64)))
    save_truth(out / "duotone8.psd", truth_of(g))
    (out / "multichannel8.psd").write_bytes(psd_bytes(W, H, 7, 8, [q[..., c] for c in range(3)], rle=True))
    for depth, dtn in ((8, "u1"), (16, "u2")):
        c4 = quant(base(H, W, 4), dtn)
        (out / f"cmyk{depth}.psd").write_bytes(psd_bytes(W, H, 4, depth, [c4[..., c] for c in range(4)], rle=depth == 8))
        lab = quant(base(H, W, 3), dtn)
        (out / f"lab{depth}.psd").write_bytes(psd_bytes(W, H, 9, depth, [lab[..., c] for c in range(3)], rle=depth == 8))
    # Odd sizes; RLE rows that need literal and repeat packets of 128.
    for h, w in ((1, 1), (3, 129), (130, 2)):
        o = quant(base(h, w, 3), "u1")
        o[:, : w // 2] = 77
        (out / f"odd{w}x{h}-rle.psd").write_bytes(psd_bytes(w, h, 3, 8, [o[..., c] for c in range(3)], rle=True))
        save_truth(out / f"odd{w}x{h}-rle.psd", truth_of(o))
    # ImageMagick's layered PSD (merged image plus two layers).
    tmp = out / "_tmp"
    tmp.mkdir(exist_ok=True)
    from PIL import Image

    Image.fromarray(q).save(tmp / "a.png")
    Image.fromarray(quant(base(H, W, 4, seed=5), "u1"), "RGBA").save(tmp / "b.png")
    for depth in (8, 16):
        for comp in ("RLE", "None"):
            try:
                run_tool(["convert", tmp / "a.png", tmp / "a.png", tmp / "b.png", "-depth", str(depth), "-compress", comp, out / f"im-layers-{depth}-{comp.lower()}.psd"])
            except Exception as e:  # noqa: BLE001
                print(f"gen psd im: {e}", file=sys.stderr)
    shutil.rmtree(tmp, ignore_errors=True)
    _ = sp


def rgbe_of(rgb):
    """float RGB -> RGBE bytes (Ward's frexp encoding)."""
    m = rgb.max(-1)
    e = np.zeros(m.shape, np.int32)
    f, ex = np.frexp(m)
    ok = m > 1e-32
    sc = np.where(ok, f * 256.0 / np.where(ok, m, 1), 0)
    out = np.zeros(rgb.shape[:-1] + (4,), "u1")
    out[..., :3] = np.where(ok[..., None], np.floor(rgb * sc[..., None]), 0).astype("u1")
    out[..., 3] = np.where(ok, ex + 128, 0).astype("u1")
    _ = e
    return out


def rgbe_float(e):
    """RGBE bytes -> float RGB, m x 2^(e - 136) (0 for e == 0)."""
    f = np.where(e[..., 3:4] > 0, np.ldexp(1.0, e[..., 3:4].astype(np.int32) - 136), 0.0)
    return (e[..., :3] * f).astype(np.float32)


def rle_hdr_row(row):
    """New-style RLE for one scanline (width 8..32767)."""
    w = row.shape[0]
    out = bytearray([2, 2, w >> 8, w & 0xFF])
    for c in range(4):
        d = row[:, c].tolist()
        i = 0
        while i < w:
            j = i
            while j + 1 < w and d[j + 1] == d[i] and j - i < 126:
                j += 1
            if j - i >= 2:
                out += bytes([128 + j - i + 1, d[i]])
                i = j + 1
                continue
            j = i
            while j + 1 < w and j - i < 127 and not (j + 3 < w and d[j + 1] == d[j + 2] == d[j + 3]):
                j += 1
            out += bytes([j - i + 1]) + bytes(d[i : j + 1])
            i = j + 1
    return bytes(out)


def gen_hdr(out):
    out = out / "hdr" / "gen"
    out.mkdir(parents=True, exist_ok=True)
    H, W = 37, 61
    hdr = base(H, W, 3).astype(np.float64)
    hdr *= np.linspace(0.01, 40.0, W)[None, :, None]
    hdr[0, :4] = 0.0
    hdr[1, :4] = 1e-30
    e = rgbe_of(hdr)

    def write(name, rows, fmt="32-bit_rle_rgbe", res=None, extra=b"", magic=b"#?RADIANCE", rle=True, ww=W, hh=H, truth=None):
        hd = magic + b"\n# fidelity\n" + extra + b"FORMAT=" + fmt.encode() + b"\n\n" + (res or f"-Y {hh} +X {ww}").encode() + b"\n"
        body = b"".join(rle_hdr_row(r) if rle and 8 <= r.shape[0] <= 32767 else r.tobytes() for r in rows)
        (out / name).write_bytes(hd + body)
        # The pixels as shown (top row first), mantissa x 2^(e - 136) without Ward's +0.5.
        save_truth(out / name, rgbe_float(rows if truth is None else truth), hdr=True)

    write("rle.hdr", e)
    write("flat.hdr", e, rle=False)
    write("rgbe-magic.hdr", e, magic=b"#?RGBE")
    write("exposure2.hdr", e, extra=b"EXPOSURE=2.0\n")
    write("bottom-up.hdr", e[::-1], res=f"+Y {H} +X {W}", truth=e)
    write("right-to-left.hdr", e[:, ::-1], res=f"-Y {H} -X {W}", truth=e)
    # x-major: each stored scanline is a column, top to bottom.
    write("column-major.hdr", e.transpose(1, 0, 2), res=f"+X {W} -Y {H}", truth=e)
    for w in (1, 7, 8, 9):
        o = rgbe_of(base(5, w, 3).astype(np.float64) * 3)
        write(f"narrow{w}.hdr", o, ww=w, hh=5)
    # Old-style run (1,1,1,n repeats the previous pixel).
    old = []
    for r in e:
        row = bytearray()
        x = 0
        while x < W:
            row += r[x].tobytes()
            n = 1
            while x + n < W and n < 255 and (r[x + n] == r[x]).all():
                n += 1
            if n > 1:
                row += bytes([1, 1, 1, n - 1])
            x += n
        old.append(np.frombuffer(bytes(row), "u1").reshape(-1, 4))
    flat = e.copy()
    flat[:, 10:30] = e[:, 10:11]
    old = []
    for r in flat:
        row = bytearray(r[0].tobytes())
        x = 1
        while x < W:
            n = 0
            while x + n < W and n < 255 and (r[x + n] == r[x - 1]).all():
                n += 1
            if n:
                row += bytes([1, 1, 1, n])
                x += n
            else:
                row += r[x].tobytes()
                x += 1
        old.append(np.frombuffer(bytes(row), "u1").reshape(-1, 4))
    hd = b"#?RADIANCE\nFORMAT=32-bit_rle_rgbe\n\n" + f"-Y {H} +X {W}\n".encode()
    (out / "old-rle.hdr").write_bytes(hd + b"".join(r.tobytes() for r in old))
    save_truth(out / "old-rle.hdr", rgbe_float(flat), hdr=True)
    try:
        import imagecodecs

        (out / "imagecodecs-rgbe.hdr").write_bytes(imagecodecs.rgbe_encode(hdr.astype(np.float32)))
        Path(str(out / "imagecodecs-rgbe.hdr") + ".json").write_text(json.dumps({"hdr": True}))
    except Exception as ex:  # noqa: BLE001
        print(f"gen hdr imagecodecs: {ex}", file=sys.stderr)
    # PFM: colour and grey, both byte orders, rows bottom-up as the format stores them.
    for c, magic in ((3, b"PF"), (1, b"Pf")):
        d = (base(H, W, c) * np.linspace(0.1, 20, W)[None, :, None]).astype(np.float32)
        d[2, 2] = -1.5
        for order, scale in (("le", -1.0), ("be", 1.0)):
            dt = "<f4" if order == "le" else ">f4"
            body = np.ascontiguousarray(d[::-1]).astype(dt).tobytes()
            nm = f"pfm-c{c}-{order}.pfm"
            (out / nm).write_bytes(magic + f"\n{W} {H}\n{scale}\n".encode() + body)
            save_truth(out / nm, d, hdr=True)
    d = (base(H, W, 3) * 4).astype(np.float32)
    (out / "pfm-scale2.pfm").write_bytes(b"PF\n" + f"{W} {H}\n-2.0\n".encode() + np.ascontiguousarray(d[::-1]).astype("<f4").tobytes())
    save_truth(out / "pfm-scale2.pfm", d, hdr=True)


def cineon_bytes(codes, bits, packing=5, interleave=0, be=True, offset=2048):
    """Kodak Cineon 4.5: generic header, one channel record per channel, the samples."""
    import struct

    h, w, c = codes.shape
    e = ">" if be else "<"
    hd = bytearray(offset)
    struct.pack_into(e + "I", hd, 0, 0x802A5FD7)
    struct.pack_into(e + "IIIII", hd, 4, offset, 1024, 1024, 0, 0)
    hd[24:32] = b"V4.5\x00\x00\x00\x00"
    hd[192] = 0
    hd[193] = c
    for i in range(c):
        o = 196 + 28 * i
        hd[o] = 0
        hd[o + 1] = (i + 1) if c == 3 else 0
        hd[o + 2] = bits
        struct.pack_into(e + "II", hd, o + 4, w, h)
        struct.pack_into(e + "ffff", hd, o + 12, 0.0, 0.0, (1 << bits) - 1, 2.046)
    hd[680] = interleave
    hd[681] = packing
    hd[682] = 0
    hd[683] = 0
    struct.pack_into(e + "II", hd, 684, 0, 0)
    # Samples in file order.
    if interleave == 0:
        seq = codes.reshape(h, w * c)
    elif interleave == 1:
        seq = codes.transpose(0, 2, 1).reshape(h, c * w)
    else:
        seq = codes.transpose(2, 0, 1).reshape(c * h, w)
    rows = []
    for r in seq:
        rows.append(pack_codes(r.tolist(), bits, packing, be))
    return bytes(hd) + b"".join(rows)


def pack_codes(vals, bits, packing, be):
    """One line of samples. Packing 0: a bit stream (MSB first) in 32-bit words; 1/2 byte, 3/4 16-bit,
    5/6 32-bit containers, odd = left justified (samples in the high bits)."""
    if packing == 0:
        acc, n, out = 0, 0, bytearray()
        for v in vals:
            acc = (acc << bits) | v
            n += bits
            while n >= 8:
                out.append((acc >> (n - 8)) & 0xFF)
                n -= 8
        if n:
            out.append((acc << (8 - n)) & 0xFF)
        out += bytes((-len(out)) % 4)
        if not be:
            # The stream fills 32-bit words, stored in the file's byte order.
            out = b"".join(out[i : i + 4][::-1] for i in range(0, len(out), 4))
        return bytes(out)
    unit = {1: 1, 2: 1, 3: 2, 4: 2, 5: 4, 6: 4}[packing]
    if bits > 8 * unit:
        unit = 2 if bits <= 16 else 4
    per = max(1, (8 * unit) // bits)
    left = packing % 2 == 1
    out = bytearray()
    for i in range(0, len(vals), per):
        grp = vals[i : i + per]
        v = 0
        for s in grp:
            v = (v << bits) | s
        used = bits * len(grp)
        v <<= (8 * unit - bits * per) if left else 0
        v <<= bits * (per - len(grp))
        _ = used
        out += v.to_bytes(unit, "big" if be else "little")
    out += bytes((-len(out)) % 4)
    return bytes(out)


def cin_truth(path, codes, bits):
    """The code values as linear light (the header's 2.046 over 1023 codes: density 0.002).
    OpenImageIO and ImageMagick read only 10-bit samples in 32-bit words, so the other
    packings and depths compare with this alone."""
    save_truth(path, cineon_codes_to_linear(codes.astype(np.float64) / ((1 << bits) - 1), 2.046 / 1023), hdr=True)


def gen_film(out):
    out = out / "film" / "gen"
    out.mkdir(parents=True, exist_ok=True)
    H, W = 23, 37
    for bits in (8, 10, 12, 16):
        codes = quant(base(H, W, 3), "u2", bits).astype(np.uint32)
        for packing in (0, 1, 2, 3, 4, 5, 6):
            if bits > 8 and packing in (1, 2):
                continue
            if bits > 16 and packing in (3, 4):
                continue
            for be in (True, False):
                if not be and packing not in (0, 5):
                    continue
                nm = f"cin-{bits}bit-pack{packing}-{'be' if be else 'le'}.cin"
                (out / nm).write_bytes(cineon_bytes(codes, bits, packing, 0, be))
                cin_truth(out / nm, codes, bits)
    codes = quant(base(H, W, 3), "u2", 10).astype(np.uint32)
    for il in (1, 2):
        nm = f"cin-10bit-interleave{il}.cin"
        (out / nm).write_bytes(cineon_bytes(codes, 10, 5, il, True))
        cin_truth(out / nm, codes, 10)
    g = quant(base(H, W, 1), "u2", 10).astype(np.uint32)
    (out / "cin-10bit-gray.cin").write_bytes(cineon_bytes(g, 10, 5, 0, True))
    cin_truth(out / "cin-10bit-gray.cin", g, 10)
    # ImageMagick's writer (10-bit, packing 5).
    from PIL import Image

    p = out / "_src.png"
    Image.fromarray(quant(base(H, W, 3), "u1")).save(p)
    try:
        run_tool(["convert", p, "-set", "colorspace", "Log", "-depth", "10", out / "im-10bit.cin"])
        Path(str(out / "im-10bit.cin") + ".json").write_text(json.dumps({"hdr": True}))
    except Exception as e:  # noqa: BLE001
        print(f"gen film im: {e}", file=sys.stderr)
    p.unlink()


def gen_jp2(out):
    from PIL import Image
    import imagecodecs

    out = out / "jp2" / "gen"
    out.mkdir(parents=True, exist_ok=True)
    H, W = 45, 67
    q = quant(base(H, W, 3), "u1")
    q4 = quant(base(H, W, 4), "u1")
    g16 = quant(base(H, W, 1), "u2")

    def pil(name, arr, mode, truth=None, **kw):
        try:
            (Image.fromarray(arr) if arr.dtype == np.uint16 else Image.fromarray(arr, mode)).save(out / name, **kw)
        except Exception as e:  # noqa: BLE001
            print(f"gen jp2 {name}: {e}", file=sys.stderr)
            return
        if truth is not None:
            save_truth(out / name, truth)

    # Lossless (5/3, reversible colour transform) over tiles, resolution levels, code-blocks,
    # precincts and the five progression orders, in JP2 and raw J2K.
    for prog in ("LRCP", "RLCP", "RPCL", "PCRL", "CPRL"):
        pil(f"lossless-{prog.lower()}.jp2", q, "RGB", truth_of(q), progression=prog, num_resolutions=4)
    pil("lossless-tiles16x16.jp2", q, "RGB", truth_of(q), tile_size=(16, 16))
    pil("lossless-tiles32x20-offset.jp2", q, "RGB", None, tile_size=(32, 20), tile_offset=(3, 5), offset=(5, 7))
    pil("lossless-res1.j2k", q, "RGB", truth_of(q), num_resolutions=1, no_jp2=True)
    pil("lossless-res6.j2k", q, "RGB", truth_of(q), num_resolutions=6, no_jp2=True)
    pil("lossless-cblk8x8.jp2", q, "RGB", truth_of(q), codeblock_size=(8, 8))
    pil("lossless-precincts32.jp2", q, "RGB", truth_of(q), precinct_size=(32, 32), num_resolutions=3)
    pil("lossless-nomct.jp2", q, "RGB", truth_of(q), mct=0)
    pil("lossless-rgba.jp2", q4, "RGBA", truth_of(q4))
    pil("lossless-gray16.jp2", g16[..., 0], "I;16", truth_of(g16))
    pil("lossless-plt.j2k", q, "RGB", truth_of(q), plt=True, no_jp2=True)
    # Lossy 9/7 (irreversible) at a few rates and layers.
    pil("lossy-97-rate10.jp2", q, "RGB", None, irreversible=True, quality_mode="rates", quality_layers=[10])
    pil("lossy-97-layers.jp2", q, "RGB", None, irreversible=True, quality_mode="rates", quality_layers=[40, 10, 3])
    pil("lossy-53-rate20.jp2", q, "RGB", None, quality_mode="rates", quality_layers=[20])
    pil("lossy-97-tiles-rate8.j2k", q, "RGB", None, irreversible=True, tile_size=(32, 32), quality_mode="rates", quality_layers=[8], no_jp2=True)
    # imagecodecs (OpenJPEG): other depths and component counts.
    for bits, c in ((1, 1), (4, 1), (10, 3), (12, 3), (12, 1), (16, 3)):
        d = quant(base(H, W, c), "u1" if bits <= 8 else "u2", bits)
        dd = d[..., 0] if c == 1 else d
        for fmt in ("JP2", "J2K"):
            name = f"oj-{bits}bit-c{c}-lossless.{fmt.lower()}"
            try:
                (out / name).write_bytes(imagecodecs.jpeg2k_encode(dd, codecformat=fmt, bitspersample=bits, reversible=True))
            except Exception as e:  # noqa: BLE001
                print(f"gen jp2 {name}: {e}", file=sys.stderr)
                continue
            save_truth(out / name, truth_of(d, bits), bits=bits)
    # HTJ2K (OpenJPH writer): lossless and lossy, 8 to 16 bits.
    for bits, c in ((8, 3), (12, 1), (16, 3)):
        d = quant(base(H, W, c), "u1" if bits <= 8 else "u2", bits)
        dd = d[..., 0] if c == 1 else d
        for rev in (True, False):
            name = f"ht-{bits}bit-c{c}-{'lossless' if rev else 'lossy'}.j2c"
            try:
                kw = {"reversible": True} if rev else {"reversible": False, "level": 0.01}
                (out / name).write_bytes(imagecodecs.htj2k_encode(dd, bitspersample=bits, **kw))
            except Exception as e:  # noqa: BLE001
                print(f"gen jp2 {name}: {e}", file=sys.stderr)
                continue
            if rev:
                save_truth(out / name, truth_of(d, bits), bits=bits)
    icc = icc_bytes()
    if icc:
        pil("lossless-icc.jp2", q, "RGB", truth_of(q), icc_profile=icc)


GEN_EXTRA = (gen_jpeg, gen_webp, gen_gif, gen_bmp, gen_jp2, gen_psd, gen_hdr, gen_film)



def gen(out, fns=(gen_tiff, gen_png, gen_exr, gen_jxl, gen_avif, gen_heic, gen_dpx)):
    out = Path(out)
    for f in fns:
        try:
            f(out)
        except Exception as e:  # noqa: BLE001 - keep the other formats
            print(f"{f.__name__}: {e}", file=sys.stderr)


# --------------------------------------------------------------------------- references


def pil_float(im):
    """A Pillow image as float RGB(A)/grey in [0, 1], straight alpha."""
    from PIL import Image

    mode = im.mode
    if mode == "P":
        im = im.convert("RGBA" if "transparency" in im.info else "RGB")
        mode = im.mode
    if mode in ("L", "LA", "RGB", "RGBA"):
        if "transparency" in im.info and mode in ("L", "RGB"):
            im = im.convert("LA" if mode == "L" else "RGBA")
        a = np.asarray(im).astype(np.float32) / 255
        return a[..., None] if a.ndim == 2 else a
    if mode == "1":
        return np.asarray(im).astype(np.float32)[..., None]
    if mode == "CMYK":
        return np.asarray(im.convert("RGB")).astype(np.float32) / 255
    if mode.startswith("I;16"):
        return np.asarray(im).astype(np.float32)[..., None] / 65535
    if mode == "I":
        a = np.asarray(im)
        bits = im.tag_v2.get(258, (32,))[0] if hasattr(im, "tag_v2") else 32
        return (a.astype(np.float64) / (2**bits - 1)).astype(np.float32)[..., None]
    if mode == "F":
        return np.asarray(im).astype(np.float32)[..., None]
    if mode in ("YCbCr", "LAB", "HSV"):
        return np.asarray(im.convert("RGB")).astype(np.float32) / 255
    raise ValueError(f"Pillow mode {mode}")


def ref_pillow(path):
    from PIL import Image

    try:
        import pillow_heif

        pillow_heif.register_heif_opener()
    except ImportError:
        pass
    im = Image.open(path)
    im.load()
    return pil_float(im)


def ref_tifffile(path):
    import tifffile

    with tifffile.TiffFile(path) as tf:
        p = tf.pages[0]
        a = p.asarray()
        bits = p.bitspersample
        ph = p.photometric
        if p.planarconfig == 2 and a.ndim == 3 and p.samplesperpixel > 1 and a.shape[0] == p.samplesperpixel:
            a = np.moveaxis(a, 0, -1)
        if a.ndim == 2:
            a = a[..., None]
        if ph == tifffile.PHOTOMETRIC.PALETTE:
            cm = p.colormap
            return (cm[:, a[..., 0]].transpose(1, 2, 0) / 65535).astype(np.float32)
        if a.dtype.kind == "f":
            f = a.astype(np.float32)
        elif a.dtype.kind == "i":
            f = (a.astype(np.float64) + 2 ** (bits - 1)) / (2**bits - 1)  # offset binary, as we show it
        elif a.dtype == bool:
            f = a.astype(np.float32)
        else:
            f = a.astype(np.float64) / (2**bits - 1)
        if ph == tifffile.PHOTOMETRIC.MINISWHITE:
            f = 1 - f
        if ph == tifffile.PHOTOMETRIC.SEPARATED and f.shape[-1] >= 4:
            f = (1 - f[..., :3]) * (1 - f[..., 3:4])
        es = p.extrasamples
        if es and es[0] == 1 and f.shape[-1] in (2, 4):  # associated alpha
            al = f[..., -1:]
            f = np.concatenate([np.where(al > 0, np.clip(f[..., :-1] / np.maximum(al, 1e-9), 0, 1), 0), al], -1)
        return f.astype(np.float32)


def ref_libpng(path):
    import imagecodecs

    a = imagecodecs.png_decode(Path(path).read_bytes())
    if a.ndim == 2:
        a = a[..., None]
    m = 65535 if a.dtype == np.uint16 else 255
    return a.astype(np.float32) / m


def exr_rgba_tool():
    """OpenEXR's RGBA interface as a command (tools/corpus/exr-rgba-oracle.cc, built by
    fetch-fidelity.sh exr), or None."""
    t = os.environ.get("SPECULAR_EXR_RGBA")
    if not t and os.environ.get("CARGO_TARGET_DIR"):
        t = os.path.join(os.environ["CARGO_TARGET_DIR"], "bin", "exr-rgba-oracle")
    return t if t and os.access(t, os.X_OK) else None


def ref_exr_openexr(path):
    import OpenEXR

    f = OpenEXR.File(str(path))
    ch = f.channels()
    hd = f.header()
    if "RY" in ch or "BY" in ch:
        # Luminance-chroma: the library's own RGBA interface (Imf::RgbaInputFile) rebuilds
        # the subsampled chroma and converts to the file's RGB (OpenImageIO refuses
        # subsampled channels, FFmpeg has no decoder, ImageMagick reads only Y).
        tool = exr_rgba_tool()
        if tool is None:
            raise ValueError("luminance/chroma channels (exr-rgba-oracle not built)")
        (x0, y0), (x1, y1) = hd["dataWindow"]
        raw = subprocess.run([tool, str(path), str(x0), str(y0), str(x1), str(y1)], capture_output=True, timeout=120, check=True).stdout
        a = np.frombuffer(raw, "<f2").astype(np.float32).reshape(int(y1 - y0 + 1), int(x1 - x0 + 1), 4)
        if "A" not in ch:
            a = a[..., :3]
    elif "RGBA" in ch:
        a = ch["RGBA"].pixels
    elif "RGB" in ch:
        a = ch["RGB"].pixels
    elif "Y" in ch:
        a = ch["Y"].pixels[..., None]
        if "A" in ch:
            a = np.concatenate([a, ch["A"].pixels[..., None]], -1)
    else:
        names = [k for k in ch if k.split(".")[-1] in ("R", "G", "B", "A")]
        if not names:
            raise ValueError("no colour channels")
        prefix = names[0].rsplit(".", 1)[0] + "." if "." in names[0] else ""
        a = np.stack([ch[prefix + c].pixels for c in "RGB" if prefix + c in ch], -1)
    (x0, y0), (x1, y1) = hd["dataWindow"]
    (dx0, dy0), (dx1, dy1) = hd["displayWindow"]
    a = a.astype(np.float32)
    chrom = hd.get("chromaticities")
    if chrom is not None and a.shape[-1] >= 3:
        m = to_bt709(chrom)
        if m is not None:
            a = a.copy()
            a[..., :3] = a[..., :3] @ m.T.astype(np.float32)
    lossy = str(hd.get("compression", "")).split(".")[-1].split(":")[0]
    meta = {
        "dw": (int(x0), int(y0), int(x1), int(y1)),
        "disp": (int(dx0), int(dy0), int(dx1), int(dy1)),
        "lossy": lossy in ("PXR24_COMPRESSION", "B44_COMPRESSION", "B44A_COMPRESSION", "DWAA_COMPRESSION", "DWAB_COMPRESSION"),
    }
    return a, meta


def ref_exr_oiio_deep(path):
    """A deep EXR flattened by OpenImageIO (ImageBufAlgo.flatten: the samples of each pixel
    composited front to back), its data window; R, G, B (and A). Flat files are left to the
    other readers."""
    import OpenImageIO as oiio

    b = oiio.ImageBuf(str(path))
    if not b.spec().deep:
        raise ValueError("not deep")
    f = oiio.ImageBufAlgo.flatten(b)
    if f.has_error:
        raise ValueError(f.geterror()[:100])
    names = list(f.spec().channelnames)
    a = f.get_pixels(oiio.FLOAT)
    keep = [names.index(c) for c in ("R", "G", "B", "A") if c in names]
    return a[..., keep].astype(np.float32)


def to_bt709(chrom):
    """RGB in the given chromaticities (rx ry gx gy bx by wx wy) to linear BT.709 RGB with a
    Bradford adaptation to D65; None when they are BT.709 already."""

    def rgb_to_xyz(c):
        xy = np.array(c, np.float64).reshape(4, 2)
        xyz = np.array([[x / y, 1.0, (1 - x - y) / y] for x, y in xy])
        prim, white = xyz[:3].T, xyz[3]
        s = np.linalg.solve(prim, white)
        return prim * s, white

    bt709 = (0.64, 0.33, 0.30, 0.60, 0.15, 0.06, 0.3127, 0.3290)
    if np.allclose(chrom, bt709, atol=1e-3):
        return None
    m_src, w_src = rgb_to_xyz(chrom)
    m_dst, w_dst = rgb_to_xyz(bt709)
    brad = np.array([[0.8951, 0.2664, -0.1614], [-0.7502, 1.7135, 0.0367], [0.0389, -0.0685, 1.0296]])
    cone = np.diag((brad @ w_dst) / (brad @ w_src))
    adapt = np.linalg.inv(brad) @ cone @ brad
    return np.linalg.inv(m_dst) @ adapt @ m_src


def _png_unfilter(raw, w, h, bpp):
    """Reverses the PNG row filters of a non-interlaced image (`bpp` bytes per pixel)."""
    out = np.zeros((h, w * bpp), np.int64)
    s = w * bpp + 1
    zero = np.zeros(w * bpp, np.int64)
    for y in range(h):
        f = raw[y * s]
        line = np.frombuffer(raw[y * s + 1:(y + 1) * s], np.uint8).astype(np.int64)
        prev = out[y - 1] if y else zero
        if f == 0:
            out[y] = line
        elif f == 2:
            out[y] = (line + prev) & 255
        else:
            cur = np.zeros(w * bpp, np.int64)
            for x in range(w * bpp):
                a = cur[x - bpp] if x >= bpp else 0
                b = prev[x]
                c = prev[x - bpp] if x >= bpp else 0
                if f == 1:
                    q = a
                elif f == 3:
                    q = (a + b) // 2
                else:
                    pa, pb, pc = abs(b - c), abs(a - c), abs(a + b - 2 * c)
                    q = a if pa <= pb and pa <= pc else (b if pb <= pc else c)
                cur[x] = (line[x] + q) & 255
            out[y] = cur
    return out.reshape(h, w, bpp)


def ref_apng_spec(path, frame):
    """APNG frame `frame` composited from the chunks as the APNG specification says (dispose
    before the next frame, "previous" on the first frame as "background", straight-alpha
    "over" in float, the 8-bit canvas rounded after each frame). 8-bit RGBA, non-interlaced
    only."""
    import struct
    import zlib

    b = path.read_bytes()
    i, frames, cur, W, H = 8, [], None, 0, 0
    while i + 8 <= len(b):
        n = struct.unpack(">I", b[i:i + 4])[0]
        t, d = b[i + 4:i + 8], b[i + 8:i + 8 + n]
        if t == b"IHDR":
            W, H, depth, ct, _, _, il = struct.unpack(">IIBBBBB", d[:13])
            if (depth, ct, il) != (8, 6, 0):
                raise ValueError("apng-spec: 8-bit RGBA, non-interlaced only")
        elif t == b"fcTL":
            _, w, h, x, y, _, _, dop, bop = struct.unpack(">IIIIIHHBB", d[:26])
            cur = dict(w=w, h=h, x=x, y=y, dop=dop, bop=bop, data=b"")
            frames.append(cur)
        elif t == b"IDAT" and cur is not None:
            cur["data"] += d
        elif t == b"fdAT" and cur is not None:
            cur["data"] += d[4:]
        i += 12 + n
    if frame >= len(frames):
        raise ValueError("apng-spec: no such frame")
    canvas = np.zeros((H, W, 4))
    for k, f in enumerate(frames[:frame + 1]):
        img = _png_unfilter(zlib.decompress(f["data"]), f["w"], f["h"], 4) / 255.0
        sl = (slice(f["y"], f["y"] + f["h"]), slice(f["x"], f["x"] + f["w"]))
        saved = canvas[sl].copy()
        if f["bop"] == 0 or k == 0:
            canvas[sl] = img
        else:
            dst = canvas[sl]
            sa, da = img[..., 3:], dst[..., 3:]
            oa = sa + da * (1 - sa)
            oc = (img[..., :3] * sa + dst[..., :3] * da * (1 - sa)) / np.maximum(oa, 1e-12)
            canvas[sl] = np.concatenate([np.where(oa > 0, oc, 0), oa], -1)
        # The canvas is an 8-bit RGBA buffer: each frame's result is rounded there.
        canvas = np.floor(canvas * 255 + 0.5) / 255
        if k == frame:
            return canvas.astype(np.float32)
        dop = 1 if (k == 0 and f["dop"] == 2) else f["dop"]
        if dop == 1:
            canvas[sl] = 0
        elif dop == 2:
            canvas[sl] = saved
    raise ValueError("apng-spec: unreachable")


def ref_ffmpeg_apng(path, frame):
    """FFmpeg's APNG demuxer and decoder (its own dispose and blend code), frame `frame`."""
    w, h = ref_pillow(path).shape[1::-1]
    r = subprocess.run(["ffmpeg", "-v", "error", "-f", "apng", "-i", str(path), "-vf",
                        f"select=eq(n\\,{frame})", "-frames:v", "1", "-f", "rawvideo",
                        "-pix_fmt", "rgba64le", "-"], capture_output=True, timeout=120)
    if r.returncode or not r.stdout:
        raise ValueError("ffmpeg: " + r.stderr.decode(errors="replace")[:200])
    a = np.frombuffer(r.stdout, "<u2")[: w * h * 4].reshape(h, w, 4)
    return a.astype(np.float32) / 65535


def ref_ffmpeg(path, pix=None, alpha=True, frame=0):
    p = subprocess.run(
        ["ffprobe", "-v", "error", "-select_streams", "v:0", "-show_entries", "stream=width,height,pix_fmt", "-of", "csv=p=0", str(path)],
        capture_output=True,
        text=True,
        timeout=60,
    )
    fields = p.stdout.strip().split(",")
    w, h = int(fields[0]), int(fields[1])
    native = fields[2] if len(fields) > 2 else ""
    if pix is None:
        # Float output in the stream's own layout: converting to another one clamps to 0..1.
        pix = "grayf32le" if native.startswith("gray") else ("gbrapf32le" if "a" in native.replace("gbr", "") else "gbrpf32le")
    sel = ["-vf", f"select=eq(n\\,{frame})"] if frame else []
    # -noautorotate: the stored orientation (FFmpeg 7+ turns EXIF orientation into a display
    # matrix and would apply it).
    r = subprocess.run(["ffmpeg", "-v", "error", "-noautorotate", "-i", str(path), *sel, "-frames:v", "1", "-f", "rawvideo", "-pix_fmt", pix, "-"], capture_output=True, timeout=120)
    if r.returncode or not r.stdout:
        raise ValueError("ffmpeg: " + r.stderr.decode(errors="replace")[:200])
    if pix == "grayf32le":
        a = np.frombuffer(r.stdout, "<f4")[: w * h].reshape(h, w, 1).copy()
    elif pix in ("gbrapf32le", "gbrpf32le"):
        n = 4 if pix == "gbrapf32le" else 3
        a = np.frombuffer(r.stdout, "<f4")[: w * h * n].reshape(n, h, w)
        a = np.stack([a[2], a[0], a[1]] + ([a[3]] if n == 4 else []), -1)  # G B R (A) planes
    else:  # rgba64le
        u = np.frombuffer(r.stdout, "<u2")[: w * h * 4].reshape(h, w, 4)
        # swscale widens 8-bit samples by a shift (255 -> 65280), not by 257.
        a = (u >> 8).astype(np.float32) / 255 if not (u & 0xFF).any() else u.astype(np.float32) / 65535
    return a if alpha else a[..., :3]


def ref_magick(path):
    r = subprocess.run(["convert", str(path) + "[0]", "-depth", "16", "png:-"], capture_output=True, timeout=120)
    if r.returncode or not r.stdout:
        raise ValueError("magick: " + r.stderr.decode(errors="replace")[:200])
    return ref_libpng_bytes(r.stdout)


def ref_libpng_bytes(b):
    import imagecodecs

    a = imagecodecs.png_decode(b)
    if a.ndim == 2:
        a = a[..., None]
    return a.astype(np.float32) / (65535 if a.dtype == np.uint16 else 255)


def ref_jxl(path):
    import imagecodecs

    a = imagecodecs.jpegxl_decode(Path(path).read_bytes())
    if a.ndim == 4:
        a = a[0]  # animation: the first frame
    if a.ndim == 2:
        a = a[..., None]
    if a.dtype.kind == "f":
        return a.astype(np.float32)
    return a.astype(np.float32) / (65535 if a.dtype == np.uint16 else 255)


def ref_avif(path, frame=0, page=0):
    """libavif (imagecodecs): the primary item, or frame `frame` of an AVIS sequence (earlier
    versions compared the frame rows against frame 0, and a whole sequence came back as one
    array). It reads no other items."""
    import imagecodecs

    if page:
        raise ValueError("libavif reads only the primary item")
    b = Path(path).read_bytes()
    a = imagecodecs.avif_decode(b, index=frame)
    if a.ndim == 2:
        a = a[..., None]
    if a.dtype == np.uint16:
        bits = 16
        mx = int(a.max())
        for cand in (10, 12):
            if mx < 2**cand:
                bits = cand
                break
        return heif_transforms(b, a.astype(np.float32) / (2**bits - 1))
    return heif_transforms(b, a.astype(np.float32) / 255)


def heif_transforms(b, a):
    """libavif hands out the coded picture: apply the file's clap, irot and imir (first
    occurrence of each; clap, then irot, then imir as the HEIF order) as a viewer must."""
    import struct

    i = b.find(b"clap")
    if i > 0:
        wn, wd, hn, hd, xn, xd, yn, yd = struct.unpack(">IIIIiIiI", b[i + 4 : i + 36])
        if wd and hd and xd and yd:
            h, w = a.shape[:2]
            cw, ch = wn / wd, hn / hd
            x0 = int(round((w - 1) / 2 + xn / xd - (cw - 1) / 2))
            y0 = int(round((h - 1) / 2 + yn / yd - (ch - 1) / 2))
            if 0 <= x0 and 0 <= y0 and x0 + cw <= w and y0 + ch <= h:
                a = a[y0 : y0 + int(ch), x0 : x0 + int(cw)]
    i = b.find(b"irot")
    if i > 0:
        a = np.rot90(a, k=b[i + 4] & 3)  # anticlockwise quarter turns
    i = b.find(b"imir")
    if i > 0:
        # As libheif and we read it (the AV1 AVIF test files then show upright): axis 1
        # mirrors left to right, 0 top to bottom.
        a = a[:, ::-1] if b[i + 4] & 1 else a[::-1]
    return np.ascontiguousarray(a)


def ref_heif_dec(path, page=0, frame=0):
    """heif-dec (libheif): the primary item, or with several top-level images (written as
    `<name>-<n>.png`, in item order as our pages) top-level image `page`."""
    if frame:
        raise ValueError("heif-dec: sequence frames not read here")
    out = Path(os.environ.get("TMPDIR", ".")) / f"heifdec-{os.getpid()}.png"
    r = subprocess.run(["heif-dec", str(path), str(out)], capture_output=True, timeout=120)
    try:
        many = out.parent / f"{out.stem}-{page + 1}.png"
        got = many if many.exists() else (out if page == 0 else None)
        if r.returncode or got is None or not got.exists():
            raise ValueError("heif-dec failed")
        return ref_libpng(got)
    finally:
        for f in out.parent.glob(out.stem + "*"):
            f.unlink()


def ref_pillow_heif(path, page=0):
    import pillow_heif

    hf = pillow_heif.open_heif(str(path), convert_hdr_to_8bit=False)
    if page:
        hf = hf[page]
    a = np.asarray(hf)
    if a.ndim == 2:
        a = a[..., None]
    if a.dtype == np.uint16:
        # pillow-heif shifts 10/12-bit samples into 16 bits (1023 << 6 is white).
        bits = int(getattr(hf, "bit_depth", 16) or 16)
        return a.astype(np.float32) / (((1 << bits) - 1) << (16 - bits))
    return a.astype(np.float32) / 255


def ref_pillow_at(path, frame=0):
    """Pillow at frame or page `frame` (GIF and WebP composited, MPO images)."""
    from PIL import Image

    im = Image.open(path)
    if frame:
        im.seek(frame)
    im.load()
    return pil_float(im)


def ref_oiio(path, subimage=0):
    """OpenImageIO's reader (own BMP, PSD, Radiance, PNM/PFM, Cineon code), straight alpha."""
    import OpenImageIO as oiio

    cfg = oiio.ImageSpec()
    cfg.attribute("oiio:UnassociatedAlpha", 1)
    i = oiio.ImageInput.open(str(path), cfg)
    if not i:
        raise ValueError("oiio: " + oiio.geterror()[:120])
    try:
        if subimage and not i.seek_subimage(subimage, 0):
            raise ValueError(f"oiio: no subimage {subimage}")
        a = i.read_image(format="float")
    finally:
        i.close()
    if a is None:
        raise ValueError("oiio: no pixels")
    a = np.asarray(a, np.float32)
    return a[..., None] if a.ndim == 2 else a


def ref_jpeg8(path):
    """libjpeg-turbo 3 (imagecodecs): 12-bit, arithmetic and lossless JPEG."""
    import imagecodecs

    a = imagecodecs.jpeg8_decode(Path(path).read_bytes())
    a = a[..., None] if a.ndim == 2 else a
    return a.astype(np.float32) / (2 ** jpeg_precision(path) - 1)


def ref_jpegsof3(path):
    """An independent lossless JPEG (SOF3) decoder."""
    import imagecodecs

    a = imagecodecs.jpegsof3_decode(Path(path).read_bytes())
    a = a[..., None] if a.ndim == 2 else a
    return a.astype(np.float32) / (2 ** jpeg_precision(path) - 1)


def jpeg_precision(path):
    b = Path(path).read_bytes()
    i = 2
    while i + 4 < len(b):
        if b[i] != 0xFF:
            i += 1
            continue
        m = b[i + 1]
        if 0xC0 <= m <= 0xCF and m not in (0xC4, 0xC8, 0xCC):
            return b[i + 4]
        if m in (0xD8, 0x01) or 0xD0 <= m <= 0xD7:
            i += 2
            continue
        i += 2 + int.from_bytes(b[i + 2 : i + 4], "big")
    return 8


def jpeg_max_sampling(path):
    """The largest luma-to-chroma sampling ratio in either direction (1 without chroma)."""
    b = Path(path).read_bytes()
    i = 2
    while i + 4 < len(b):
        if b[i] != 0xFF:
            i += 1
            continue
        m = b[i + 1]
        if 0xC0 <= m <= 0xCF and m not in (0xC4, 0xC8, 0xCC):
            n = b[i + 9]
            f = [(b[i + 11 + 3 * k] >> 4, b[i + 11 + 3 * k] & 15) for k in range(n)]
            if n < 3:
                return 1
            hmax, vmax = max(h for h, _ in f), max(v for _, v in f)
            return max(max(hmax // max(h, 1), vmax // max(v, 1)) for h, v in f)
        if m in (0xD8, 0x01) or 0xD0 <= m <= 0xD7:
            i += 2
            continue
        i += 2 + int.from_bytes(b[i + 2 : i + 4], "big")
    return 1


def jp2_icc(path):
    """The ICC profile of a JP2's first colour box (method 2 or 3), as JP2 readers use it."""
    import struct

    b = Path(path).read_bytes()
    i = b.find(b"jp2h")
    if i < 4:
        return None
    end = i - 4 + struct.unpack(">I", b[i - 4 : i])[0]
    o = i + 4
    while o + 8 <= end:
        n, t = struct.unpack(">I4s", b[o : o + 8])
        if n < 8:
            return None
        if t == b"colr":
            return b[o + 11 : o + n] if b[o + 8] in (2, 3) else None
        o += n
    return None


def ref_openjpeg(path):
    import imagecodecs

    a = imagecodecs.jpeg2k_decode(Path(path).read_bytes())
    return j2k_float(path, a)


def ref_openjph(path):
    import imagecodecs

    a = imagecodecs.htj2k_decode(Path(path).read_bytes())
    return j2k_float(path, a)


def j2k_precision(path):
    """The first component's precision from SIZ (JP2 or raw codestream)."""
    b = Path(path).read_bytes()
    i = b.find(b"\xff\x51")
    if i < 0 or i + 41 > len(b):
        return None
    return (b[i + 40] & 0x7F) + 1, bool(b[i + 40] & 0x80)


def j2k_float(path, a):
    a = a[..., None] if a.ndim == 2 else a
    pr = j2k_precision(path)
    bits, signed = pr if pr else (8 * a.dtype.itemsize, False)
    if signed or a.dtype.kind == "i":
        return ((a.astype(np.float64) + 2 ** (bits - 1)) / (2**bits - 1)).astype(np.float32)
    return (a.astype(np.float64) / (2**bits - 1)).astype(np.float32)


def ref_psdtools(path):
    """psd-tools: the merged image as stored (no compositing of layers)."""
    from psd_tools import PSDImage

    psd = PSDImage.open(str(path))
    mode = int(psd.color_mode)
    if mode in (4, 9):
        raise ValueError(f"psd-tools: colour mode {mode} not converted here")
    if psd.has_preview():
        a = psd.numpy()
    else:
        # No merged image (Maximize compatibility off): psd-tools' numpy() is blank white,
        # so its composite of the layers is the reference, as ours is built from them.
        a = psd.composite(ignore_preview=True)
        a = None if a is None else np.asarray(a.convert("RGBA"), np.float32) / 255.0
    if a is None:
        raise ValueError("psd-tools: no merged image")
    a = np.asarray(a, np.float32)
    if a.ndim == 2:
        a = a[..., None]
    if mode == 7:
        a = a[..., :3]
    return a


def ref_lcms_psd(path):
    """CMYK with its embedded profile, or Lab (D50), to sRGB through LittleCMS (relative
    colorimetric with black-point compensation); compared with the lossy tolerance (CLUT
    interpolation differs)."""
    from PIL import Image, ImageCms
    from psd_tools import PSDImage

    psd = PSDImage.open(str(path))
    mode = int(psd.color_mode)
    v = np.asarray(psd.numpy(), np.float64)
    srgb = ImageCms.createProfile("sRGB")
    flags = ImageCms.Flags.BLACKPOINTCOMPENSATION
    if mode == 4:
        icc = psd.image_resources.get_data(1039)
        if not icc:
            raise ValueError("lcms: untagged CMYK (a generic profile is a design choice)")
        ink = np.round((1 - v[..., :4]) * 255).astype("u1")  # PSD stores 255 as no ink
        t = ImageCms.buildTransform(ImageCms.ImageCmsProfile(io.BytesIO(icc)), srgb, "CMYK", "RGB", renderingIntent=1, flags=flags)
        rgb = np.asarray(ImageCms.applyTransform(Image.fromarray(np.ascontiguousarray(ink), "CMYK"), t))
    elif mode == 9:
        # psd-tools: L 0..255, a and b offset by 128; Pillow's LAB mode takes a and b signed.
        q = np.round(v[..., :3] * 255).astype(np.int32)
        lab = np.concatenate([q[..., :1], q[..., 1:] - 128], -1).astype(np.int8).view("u1")
        t = ImageCms.buildTransform(ImageCms.createProfile("LAB"), srgb, "LAB", "RGB", renderingIntent=1, flags=flags)
        rgb = np.asarray(ImageCms.applyTransform(Image.fromarray(np.ascontiguousarray(lab), "LAB"), t))
    else:
        raise ValueError("lcms: not CMYK or Lab")
    return rgb.astype(np.float32) / 255, {"lossy": True}


def ref_psd_layer(path, label, nth=0):
    """A PSD layer page as specular-decode draws it (the layer alone on the canvas with its
    transparency and opacity, no blending or effects), from psd-tools' layer pixels. `label`
    is `<place from the top>:<name>`, `nth` which of the layers of that name; layers with a user mask, fill opacity or a colour mode
    other than RGB/grey are not modelled here."""
    from psd_tools import PSDImage

    psd = PSDImage.open(str(path))
    if int(psd.color_mode) not in (1, 3) or psd.depth == 32:
        raise ValueError("psd-tools layer: colour mode / depth not modelled")
    name = label.split(":", 1)[1]
    cands = [l for l in reversed(list(psd.descendants())) if not l.is_group() and l.name == name and l.has_pixels()]
    if not cands:
        raise ValueError("psd-tools layer: not found")
    if nth >= len(cands):
        raise ValueError("psd-tools layer: not found")
    lay = cands[nth]
    if lay.has_mask() or lay.has_vector_mask():
        raise ValueError("psd-tools layer: masks not modelled")
    if getattr(lay, "fill_opacity", 255) not in (None, 255):
        raise ValueError("psd-tools layer: fill opacity not modelled")
    px = lay.numpy()
    if px is None:
        raise ValueError("psd-tools layer: no pixels")
    px = as_rgba(np.asarray(px, np.float32))
    out = np.zeros((psd.height, psd.width, 4), np.float32)
    x0, y0 = lay.left, lay.top
    h, w = px.shape[:2]
    sx0, sy0 = max(0, -x0), max(0, -y0)
    dx0, dy0 = max(0, x0), max(0, y0)
    dx1, dy1 = min(psd.width, x0 + w), min(psd.height, y0 + h)
    if dx1 > dx0 and dy1 > dy0:
        out[dy0:dy1, dx0:dx1] = px[sy0 : sy0 + dy1 - dy0, sx0 : sx0 + dx1 - dx0]
    out[..., 3] *= lay.opacity / 255
    return out


def ref_magick_frame0(path):
    return ref_magick(path)


def gifsuite_expected(path, frame=0):
    """The pygif test suite's expected frame (from <name>.conf: the frames a player shows,
    with images that have no delay combined, the background transparent), canvas size."""
    import configparser

    conf = path.with_suffix(".conf")
    if not conf.exists():
        raise ValueError("no test description")
    cp = configparser.ConfigParser(inline_comment_prefixes=("#",))
    cp.read_string(conf.read_text())
    frames = [f.strip() for f in cp["config"].get("frames", "").split(",") if f.strip()]
    if frame >= len(frames) or "pixels" not in cp[frames[frame]]:
        raise ValueError(f"the suite lists {len(frames)} frame(s)")
    p = path.with_name(cp[frames[frame]]["pixels"])
    w, h = int(cp["config"]["width"]), int(cp["config"]["height"])
    a = np.frombuffer(p.read_bytes(), "u1")
    if a.size != w * h * 4:
        raise ValueError("expected frame size")
    return a.reshape(h, w, 4).astype(np.float32) / 255


def bmp_compression(path):
    b = Path(path).read_bytes()
    return int.from_bytes(b[30:34], "little") if len(b) >= 34 and int.from_bytes(b[14:18], "little") >= 40 else 0


def bmpsuite_expected(path):
    """BMP Suite's expected rendering: html/bmpsuite.html pairs each file with a PNG (the
    first picture of its row is the correct one)."""
    import re

    html = path.parent.parent / "html" / "bmpsuite.html"
    if not html.exists():
        raise ValueError("no BMP Suite page")
    rel = f"{path.parent.name}/{path.name}"
    s = html.read_text(errors="replace")
    m = re.search(r"<td class=\w+>" + re.escape(rel) + r"</td>.*?<img src=\"([^\"]+\.png)\"", s, re.S)
    if not m:
        raise ValueError("not in the BMP Suite page")
    return ref_libpng(html.parent / m.group(1))


def cineon_density(path):
    """Density per 10-bit code from the first channel's minimum/maximum data value and
    quantity (Cineon 4.5, offsets 208-223), else the Kodak default 0.002."""
    import struct

    b = Path(path).read_bytes()
    e = ">" if b[:4] == b"\x80\x2a\x5f\xd7" else "<"
    bits = b[198]
    mind, minq, maxd, maxq = struct.unpack(e + "ffff", b[208:224])
    if all(np.isfinite([mind, minq, maxd, maxq])) and maxd > mind:
        d = (maxq - minq) / (maxd - mind) * (((1 << bits) - 1) / 1023)
        if 0.0005 <= d < 0.01:
            return d
    return 0.002


def cineon_codes_to_linear(a, density=0.002):
    """A reader's normalised code values to linear light by the Kodak printing-density
    conversion (white 685, black 95, negative gamma 0.6)."""
    c = a.astype(np.float64) * 1023.0
    k = density / 0.6
    off = 10 ** ((95 - 685) * k)
    return ((10 ** ((c - 685) * k) - off) / (1 - off)).astype(np.float32)


def ref_magick_codes(path):
    """ImageMagick's code values (colourspace reset, so no log conversion)."""
    r = subprocess.run(["convert", str(path) + "[0]", "-set", "colorspace", "sRGB", "-depth", "16", "png:-"], capture_output=True, timeout=120)
    if r.returncode or not r.stdout:
        raise ValueError("magick: " + r.stderr.decode(errors="replace")[:200])
    return ref_libpng_bytes(r.stdout)


def references5(path, group, frame, page, out, tryref):
    """The extra set's groups: two independent decoders each where they exist."""
    ext = path.suffix.lower()
    k = frame or page
    if group == "jpeg":
        if k:
            tryref("pillow", ref_pillow_at, path, k)
            return
        tryref("pillow", ref_pillow, path)
        tryref("ffmpeg", ref_ffmpeg, path, None if group == "hdr" else "rgba64le")
        b = path.read_bytes()
        if b.find(b"\xff\xc3") > 0 or b.find(b"\xff\xcb") > 0:
            tryref("jpegsof3", ref_jpegsof3, path)
        tryref("libjpeg-turbo3", ref_jpeg8, path)
    elif group in ("webp", "gif"):
        tryref("pillow", ref_pillow_at, path, k)
        tryref("ffmpeg", ref_ffmpeg, path, "rgba64le", True, k)
        if group == "webp" and path.read_bytes().find(b"ANIM", 0, 4096) > 0:
            # FFmpeg reads no animated WebP; OpenImageIO composites with libwebp (as Pillow).
            tryref("oiio", ref_oiio, path, k)
        if group == "gif":
            tryref("gif-suite", gifsuite_expected, path, k)
            if not k:
                tryref("magick", ref_magick, path)
    elif group == "bmp":
        tryref("pillow", ref_pillow, path)
        tryref("magick", ref_magick, path)
        tryref("oiio", ref_oiio, path)
        if "bmpsuite" in str(path) and bmp_compression(path) != 4:
            # (Not for BI_JPEG: the suite's picture is the original before JPEG coding.)
            tryref("bmp-suite", bmpsuite_expected, path)
    elif group == "jp2":
        if ext in (".j2c", ".jph") or b"\xff\x50" in path.read_bytes()[:4096]:
            tryref("openjph", ref_openjph, path)
        tryref("openjpeg", ref_openjpeg, path)
        tryref("ffmpeg", ref_ffmpeg, path, None if group == "hdr" else "rgba64le")
    elif group == "psd":
        if k:
            return
        if psd_mode(path) in (4, 9):
            tryref("lcms", ref_lcms_psd, path)
        tryref("psd-tools", ref_psdtools, path)
        tryref("magick", ref_magick_frame0, path)
        tryref("oiio", ref_oiio, path)
    elif group == "hdr":
        tryref("oiio", ref_oiio, path)
        tryref("ffmpeg", ref_ffmpeg, path, None if group == "hdr" else "rgba64le")
        if ext == ".hdr":
            import imagecodecs

            tryref("imagecodecs-rgbe", lambda p: imagecodecs.rgbe_decode(p.read_bytes()), path)
    elif group == "film":
        tryref("oiio", ref_oiio, path)
        tryref("magick", ref_magick_codes, path)


def references(path, group, frame=0, page=0):
    """[(name, array)] plus metadata: the format's reference decoders that worked."""
    out, meta, errs = [], {}, []

    def tryref(name, fn, *a):
        try:
            v = fn(*a)
            if isinstance(v, tuple):
                v, m = v
                meta.update(m)
            out.append((name, np.asarray(v, np.float32)))
        except Exception as e:  # noqa: BLE001
            errs.append(f"{name}: {str(e)[:120]}")

    ext = path.suffix.lower()
    if group in GROUPS5:
        references5(path, group, frame, page, out, tryref)
    elif group == "tiff" or ext in (".tif", ".tiff"):
        tryref("tifffile", ref_tifffile, path)
        tryref("pillow", ref_pillow, path)
    elif group == "png" or ext == ".png":
        if b"acTL" in path.read_bytes()[:4096]:
            # APNG frames, composited from the chunks as the specification says
            # (Pillow pastes "over" frames with their alpha as the mask, which darkens
            # translucent alpha; FFmpeg rounds at 8 bits); frame 0 is the first frame of the
            # animation, also when the default image is not part of it.
            tryref("apng-spec", ref_apng_spec, path, frame)
            tryref("ffmpeg", ref_ffmpeg_apng, path, frame)
            # (Pillow's frame 0 is the default image, also when it is not in the animation.)
            tryref("pillow", ref_pillow_at, path, frame)
        else:
            tryref("libpng", ref_libpng, path)
            tryref("pillow", ref_pillow, path)
    elif group == "exr" or ext == ".exr":
        tryref("openexr", ref_exr_openexr, path)
        tryref("ffmpeg", ref_ffmpeg, path)
        # Deep files: neither reads them; OpenImageIO flattens them.
        tryref("oiio-deep", ref_exr_oiio_deep, path)
    elif group == "jxl" or ext == ".jxl":
        tryref("libjxl", ref_jxl, path)
        conf = path.parent / "reference_image.npy"
        if conf.exists():
            tryref("conformance", lambda p: np.load(p)[0], conf)
    elif group == "avif" or ext == ".avif":
        tryref("libavif", ref_avif, path, frame, page)
        # libheif: independent of libavif's colour code (the range of files whose colr box
        # and AV1 header disagree, Microsoft's test files: libavif takes the box's).
        tryref("heif-dec", ref_heif_dec, path, page, frame)
        if not (frame or page):
            tryref("pillow", ref_pillow, path)
    elif group == "heic" or ext in (".heic", ".heif"):
        # Pages: the top-level images in item order (every page was compared with
        # the primary image).
        tryref("pillow-heif", ref_pillow_heif, path, page)
        tryref("heif-dec", ref_heif_dec, path, page)
    elif group == "dpx" or ext == ".dpx":
        tryref("ffmpeg", ref_ffmpeg, path, "rgba64le")
        tryref("magick", ref_magick, path)
    return out, meta, errs


# --------------------------------------------------------------------------- comparison


def as_rgba(a):
    if a.shape[-1] == 1:
        return np.concatenate([a, a, a, np.ones_like(a)], -1)
    if a.shape[-1] == 2:
        return np.concatenate([a[..., :1]] * 3 + [a[..., 1:]], -1)
    if a.shape[-1] == 3:
        return np.concatenate([a, np.ones_like(a[..., :1])], -1)
    return a[..., :4]


ORIENT = {
    1: lambda a: a,
    2: lambda a: a[:, ::-1],
    3: lambda a: a[::-1, ::-1],
    4: lambda a: a[::-1],
    5: lambda a: a.transpose(1, 0, 2),
    6: lambda a: a.transpose(1, 0, 2)[:, ::-1],
    7: lambda a: a.transpose(1, 0, 2)[::-1, ::-1],
    8: lambda a: a.transpose(1, 0, 2)[::-1],
}


# Per file (set by check_one): float data compares relative to max(1, |reference|), and colour
# only where alpha reaches ALPHA_MIN (un-premultiplied colour under a 1e-5 alpha is noise).
REL = False
ALPHA_MIN = 0.0


def diff_stats(ours, ref, alpha_mask=True):
    """Errors over colour where the reference is visible, and over alpha."""
    o, r = as_rgba(ours), as_rgba(ref)
    vis = (r[..., 3] > ALPHA_MIN) if alpha_mask else np.ones(r.shape[:2], bool)
    # Non-finite reference samples (NaN, inf) are only checked for kind.
    vis = vis & np.isfinite(r[..., :3]).all(-1)
    d = o - r
    if REL:
        with np.errstate(invalid="ignore"):
            d = d / np.maximum(1.0, np.abs(r))
    dc = np.abs(d[..., :3])[vis] if vis.any() else np.zeros((1, 3))
    da = np.abs(d[..., 3])
    fin = np.isfinite(dc)
    dc = np.where(fin, dc, 0)
    nan_mismatch = int((np.isnan(o[..., :3]) & np.isfinite(r[..., :3])).sum())
    return {
        "max": float(dc.max()) if dc.size else 0.0,
        "mean": float(dc.mean()) if dc.size else 0.0,
        "p999": float(np.quantile(dc[:: max(1, dc.shape[0] // 1_000_000)], 0.999)) if dc.size else 0.0,
        "amax": float(da.max()),
        "bias": [float(np.nan_to_num(d[..., c][vis]).mean()) if vis.any() else 0.0 for c in range(3)],
        "nan": nan_mismatch,
    }


def tolerance(group, name, ours_bits, ref, lossy, hdr, src_bits=16):
    if lossy and hdr:
        # Lossy float codecs (PXR24 on 32-bit floats, B44, DWA): relative, decoder rounding.
        return {"mean": 2e-3, "p999": 2e-2, "max": 1.0, "rel": True}
    if lossy:
        # Decoders differ in IDCT and chroma upsampling (libavif and libheif by 8/255 at the
        # 99.9th percentile on the AV1 AVIF test files).
        return {"mean": 1.0 / 255, "p999": 10.0 / 255, "max": 1.0}
    if hdr:
        return {"mean": 1e-3, "p999": 2e-3, "max": 4e-3, "rel": True}
    q = 0.5 / (2 ** min(ours_bits, 16) - 1) + 0.5 / 65535 + 1e-6
    if 1 < src_bits < min(ours_bits, 16):
        # Widened samples: references may round the widening differently (one source step).
        q = max(q, 1.01 / (2**src_bits - 1))
    return {"mean": q, "p999": q, "max": q}


def passes(st, tol):
    return st["max"] <= tol["max"] and st["mean"] <= tol["mean"] and st["p999"] <= tol["p999"] and st["amax"] <= max(tol["max"], 1.5 / 255) and st["nan"] == 0


def thin(a, limit=250_000):
    """A stride sample of a large image (classification only: speed)."""
    h, w = a.shape[:2]
    k = max(1, int(np.ceil(np.sqrt(h * w / limit))))
    return a[::k, ::k] if k > 1 else a


def classify(ours, ref, tol):
    """Why `ours` differs from `ref`: the first transformation that makes them agree."""
    notes = []
    o, r = as_rgba(ours), as_rgba(ref)
    if o.shape[0] * o.shape[1] > 250_000 and o.shape[:2] == r.shape[:2]:
        o, r = thin(o), thin(r)
    if o.shape[:2] != r.shape[:2]:
        if o.shape[:2] == r.shape[1::-1]:
            for k in (5, 6, 7, 8):
                if passes(diff_stats(ORIENT[k](o), r), tol):
                    return [f"orientation (ours needs EXIF {k})"]
            return ["transposed size"]
        oh, ow = o.shape[:2]
        rh, rw = r.shape[:2]
        if max(abs(oh - rh), abs(ow - rw)) > 16 or oh * ow > 4_000_000:
            return [f"size ours {ow}x{oh} vs {rw}x{rh}"]
        if oh >= rh and ow >= rw:
            for y in range(oh - rh + 1):
                for x in range(ow - rw + 1):
                    if passes(diff_stats(o[y : y + rh, x : x + rw], r), tol):
                        return [f"size: ours {ow}x{oh} contains the reference at +{x},+{y} (crop/window)"]
        elif rh >= oh and rw >= ow:
            for y in range(rh - oh + 1):
                for x in range(rw - ow + 1):
                    if passes(diff_stats(o, r[y : y + oh, x : x + ow]), tol):
                        return [f"size: reference {rw}x{rh} contains ours at +{x},+{y} (crop/window)"]
        return [f"size ours {ow}x{oh} vs {rw}x{rh}"]
    for k in range(2, 9):
        oo = ORIENT[k](o)
        if oo.shape == r.shape and passes(diff_stats(oo, r), tol):
            return [f"orientation (EXIF {k} away)"]
    for perm, nm in (((2, 1, 0), "BGR"), ((1, 0, 2), "GRB"), ((0, 2, 1), "RBG"), ((1, 2, 0), "GBR"), ((2, 0, 1), "BRG")):
        oo = o.copy()
        oo[..., :3] = o[..., list(perm)]
        if passes(diff_stats(oo, r), tol):
            return [f"channel swap ({nm})"]
    inv = o.copy()
    inv[..., :3] = 1 - o[..., :3]
    if passes(diff_stats(inv, r), tol):
        return ["inverted"]
    for dy in range(-2, 3) if min(o.shape[:2]) > 6 else ():
        for dx in range(-2, 3):
            if dx == dy == 0:
                continue
            oo = np.roll(o, (dy, dx), (0, 1))
            sl = (slice(2, -2), slice(2, -2))
            if passes(diff_stats(oo[sl], r[sl]), tol):
                return [f"shift ({dx:+d},{dy:+d}) px"]
    a = r[..., 3:4]
    if (a < 0.999).any():
        pm = o.copy()
        pm[..., :3] *= o[..., 3:4]
        if passes(diff_stats(pm, r), tol):
            return ["ours is premultiplied (reference straight)"]
        un = o.copy()
        un[..., :3] = np.where(o[..., 3:4] > 0, o[..., :3] / np.maximum(o[..., 3:4], 1e-9), 0)
        if passes(diff_stats(un, r), tol):
            return ["ours is straight where the reference is premultiplied (or un-premultiplied twice)"]
    vis = r[..., 3] > 0
    x = o[..., :3][vis].ravel()
    y = r[..., :3][vis].ravel()
    ok = np.isfinite(x) & np.isfinite(y)
    x, y = x[ok], y[ok]
    if x.size > 16 and np.ptp(x) > 0:
        A = np.vstack([x, np.ones_like(x)]).T
        (s, b), *_ = np.linalg.lstsq(A, y, rcond=None)
        res = np.abs(A @ [s, b] - y)
        if res.mean() <= tol["mean"] * 2 and (abs(s - 1) > 1e-3 or abs(b) > 1e-4):
            notes.append(f"range: reference = {s:.5f} x ours {b:+.5f}")
        pos = (x > 1e-3) & (y > 1e-3) & (x < 1) & (y < 1)
        if not notes and pos.sum() > 16:
            g = np.median(np.log(y[pos]) / np.log(x[pos]))
            if abs(g - 1) > 0.02 and np.abs(np.clip(x, 0, 1) ** g - y).mean() <= tol["mean"] * 2:
                notes.append(f"gamma: reference = ours^{g:.3f}")
    d = np.abs(o[..., :3] - r[..., :3]).max(-1)
    d = np.nan_to_num(d)
    big = d > tol["max"]
    if big.any():
        edge = np.zeros_like(big)
        edge[:2], edge[-2:], edge[:, :2], edge[:, -2:] = True, True, True, True
        if big[~edge].sum() == 0:
            notes.append("edge-only (outer 2 px)")
        else:
            gy, gx = np.gradient(np.nan_to_num(r[..., :3]).mean(-1))
            grad = np.hypot(gx, gy)
            hi = grad > np.quantile(grad, 0.9)
            if big[~hi].sum() <= 0.01 * big.sum():
                notes.append("at strong edges only (filter/upsampling)")
            ys, xs = np.nonzero(big)
            notes.append(f"{big.sum()} px off, first at ({xs[0]},{ys[0]})")
    return notes or ["unclassified"]


def systematic(st, tol):
    """A consistent signed error, though within tolerance: flagged."""
    b = np.array(st["bias"])
    lim = max(tol["mean"] * 0.25, 0.02 / 255)
    if np.abs(b).max() > lim and np.abs(b).max() > 0.5 * st["mean"] and st["mean"] > 0:
        return "systematic bias " + ",".join(f"{v * 255:+.2f}" for v in b) + " /255"
    return None


def agree(refs, tol):
    if len(refs) < 2:
        return None
    (n1, a1), (n2, a2) = refs[0], refs[1]
    if as_rgba(a1).shape != as_rgba(a2).shape:
        return f"{n1} {a1.shape[1]}x{a1.shape[0]} vs {n2} {a2.shape[1]}x{a2.shape[0]}"
    st = diff_stats(a1, a2)
    if passes(st, tol):
        return None
    return f"{n1} vs {n2}: max {st['max'] * 255:.2f} mean {st['mean'] * 255:.3f} /255"


def exif_facts(paths):
    """exiftool's orientation and size per file (one run)."""
    if not shutil.which("exiftool") or not paths:
        return {}
    out = {}
    for i in range(0, len(paths), 200):
        r = subprocess.run(
            ["exiftool", "-j", "-n", "-q", "-q", "-Orientation", "-ImageWidth", "-ImageHeight", "-ICC_Profile:ProfileDescription", "-Rotation", "-Mirror"] + [str(p) for p in paths[i : i + 200]],
            capture_output=True,
            text=True,
            timeout=300,
        )
        try:
            for e in json.loads(r.stdout or "[]"):
                out[e["SourceFile"]] = e
        except json.JSONDecodeError:
            pass
    return out


def srgb_encode(v):
    v = np.clip(v, 0, 1)
    return np.where(v <= 0.0031308, 12.92 * v, 1.055 * np.power(v, 1 / 2.4) - 0.055)


def png_decode_exponent(path):
    """A PNG's gAMA decoding exponent when it is the only colour description and not about
    1/2.2 (the viewer then treats it as a profile); else None."""
    import struct

    b = path.read_bytes()
    i, gama, other = 8, None, False
    while i + 8 <= len(b):
        n = struct.unpack(">I", b[i : i + 4])[0]
        t = b[i + 4 : i + 8]
        if t == b"gAMA" and n == 4:
            gama = struct.unpack(">I", b[i + 8 : i + 12])[0]
        elif t in (b"sRGB", b"iCCP", b"cICP"):
            other = True
        elif t == b"IDAT":
            break
        i += 12 + n
    if other or not gama:
        return None
    e = 100_000 / gama
    return None if abs(e - 2.2) < 0.05 else e


def dpx_transfer(path):
    try:
        return path.read_bytes()[801]
    except IndexError:
        return None


def display_ref(ref, orientation, icc, transfer=None, gray_icc=False):
    """The reference as the viewer should show it: decoded by the file's transfer (`linear`,
    or a PNG gAMA exponent), ICC to sRGB, oriented, 8-bit."""
    from PIL import Image, ImageCms

    a = as_rgba(np.nan_to_num(ref.astype(np.float32)))
    rgb = a[..., :3]
    if transfer == "linear":
        rgb = srgb_encode(rgb)
    elif isinstance(transfer, float):
        rgb = srgb_encode(np.power(np.clip(rgb, 0, 1), transfer))
    rgb8 = np.clip(np.round(rgb * 255), 0, 255).astype("u1")
    if icc:
        try:
            src = ImageCms.ImageCmsProfile(io.BytesIO(icc))
            if src.profile.xcolor_space.strip() == "RGB":
                im = ImageCms.profileToProfile(Image.fromarray(rgb8), src, ImageCms.createProfile("sRGB"), renderingIntent=0, outputMode="RGB")
                rgb8 = np.asarray(im)
            elif gray_icc and src.profile.xcolor_space.strip() == "GRAY":
                im = ImageCms.profileToProfile(Image.fromarray(np.ascontiguousarray(rgb8[..., 0]), "L"), src, ImageCms.createProfile("sRGB"), renderingIntent=0, outputMode="RGB")
                rgb8 = np.asarray(im)
        except Exception:  # noqa: BLE001
            pass
    out = np.concatenate([rgb8.astype(np.float32) / 255, a[..., 3:4]], -1)
    return ORIENT.get(orientation or 1, ORIENT[1])(out)


def load_png(p):
    a = ref_libpng(p)
    return as_rgba(a)


def unpremultiply(a):
    """Associated (OpenEXR's convention) to straight alpha."""
    if a.shape[-1] != 4 and a.shape[-1] != 2:
        return a
    al = a[..., -1:]
    with np.errstate(divide="ignore", invalid="ignore"):
        c = np.where(al > 0, a[..., :-1] / al, 0)
    return np.concatenate([c, al], -1).astype(np.float32)


def lossy5(group, rel, has_truth):
    """The extra set: which files compare with the lossy tolerance."""
    n = rel.lower()
    if group == "jpeg":
        return "lossless" not in n
    if group == "webp":
        return not has_truth and not ("lossless" in Path(n).name and "near" not in n)
    if group == "jp2":
        return not has_truth and "lossless" not in n
    if group == "bmp":
        return "jpeg" in Path(n).name
    return False


def psd_icc(path):
    try:
        from psd_tools import PSDImage

        return PSDImage.open(str(path)).image_resources.get_data(1039)
    except Exception:  # noqa: BLE001
        return None


def psd_mode(path):
    try:
        return int.from_bytes(path.read_bytes()[24:26], "big")
    except (OSError, ValueError):
        return -1


def psd_depth(path):
    try:
        return int.from_bytes(path.read_bytes()[22:24], "big")
    except (OSError, ValueError):
        return 0


def webp_planes_check(path, raw, layout):
    """Our decoded Y, U and V planes against `dwebp -yuv` (VP8 decoding is bit-exact):
    None when there is nothing to compare, else (equal, note). The planes are
    VP8's own (layout Yuv, studio range); a YCbCr raster had them range-expanded."""
    planes = Path(raw).with_suffix(".planes")
    if not planes.exists() or not shutil.which("dwebp"):
        return None
    out = Path(os.environ.get("TMPDIR", ".")) / f"dwebp-{os.getpid()}.yuv"
    try:
        r = subprocess.run(["dwebp", "-quiet", "-yuv", str(path), "-o", str(out)], capture_output=True, timeout=60)
        if r.returncode or not out.exists():
            return None
        want = np.frombuffer(out.read_bytes(), "u1")
    finally:
        out.unlink(missing_ok=True)
    got = np.frombuffer(planes.read_bytes(), "u1")
    if got.size != want.size:
        return False, f"planes: {got.size} samples, dwebp {want.size}"
    expanded = layout == "YCbCr"
    if expanded:
        # Earlier specular-webp handed out full-range (JFIF) planes, each sample rounded
        # half up from (Y - 16) * 255/219 and (C - 128) * 255/224 + 128.
        from PIL import Image

        with Image.open(path) as im:
            w, h = im.size
        ny = w * h
        v = want.astype(np.float64)
        want = np.concatenate([np.floor((v[:ny] - 16) * 255 / 219 + 0.5), np.floor((v[ny:] - 128) * 255 / 224 + 128.5)])
        want = np.clip(want, 0, 255)
    d = np.abs(got.astype(int) - want.astype(int))
    if not d.any():
        return True, "Y/U/V planes equal dwebp -yuv" + (" (range expanded)" if expanded else "")
    return False, f"planes: {int((d > 0).sum())} of {d.size} samples differ from dwebp -yuv, max {int(d.max())}, first at sample {int(np.argmax(d > 0))}"


def jpeg_planes_check(path, raw, w, h):
    """Our Y, Cb and Cr planes against libjpeg-turbo's (imagecodecs, YCbCr out, no fancy
    upsampling, so each chroma sample repeats over its block): None when there is nothing to
    compare, else (equal within one code of IDCT rounding, note)."""
    import imagecodecs

    planes = Path(raw).with_suffix(".planes")
    if not planes.exists():
        return None
    try:
        full = imagecodecs.jpeg8_decode(Path(path).read_bytes(), outcolorspace="YCBCR", fancyupsampling=False)
    except Exception:  # noqa: BLE001
        return None
    if full.ndim != 3 or full.shape[:2] != (h, w):
        return None
    got = np.frombuffer(planes.read_bytes(), "u1").astype(int)
    hs = jpeg_sampling(path)
    if not hs:
        return None
    hmax, vmax = max(a for a, _ in hs), max(b for _, b in hs)
    want, off = [], 0
    for c, (sh, sv) in enumerate(hs[:3]):
        cw, ch = -(-w * sh // hmax), -(-h * sv // vmax)
        want.append(full[:: vmax // sv, :: hmax // sh, c][:ch, :cw].astype(int).ravel())
        off += cw * ch
    want = np.concatenate(want)
    if got.size != want.size:
        return False, f"planes: {got.size} samples, libjpeg-turbo {want.size}"
    d = np.abs(got - want)
    if d.max() <= 1:
        return True, f"Y/Cb/Cr planes equal libjpeg-turbo's (max {int(d.max())}, {int((d > 0).sum())} samples one code off)"
    return False, f"planes: {int((d > 1).sum())} of {d.size} samples differ from libjpeg-turbo's by more than one code, max {int(d.max())}"


def jpeg_sampling(path):
    """[(h, v)] sampling factors per component from the frame header."""
    b = Path(path).read_bytes()
    i = 2
    while i + 4 < len(b):
        if b[i] != 0xFF:
            i += 1
            continue
        m = b[i + 1]
        if 0xC0 <= m <= 0xCF and m not in (0xC4, 0xC8, 0xCC):
            n = b[i + 9]
            return [(b[i + 11 + 3 * k] >> 4, b[i + 11 + 3 * k] & 15) for k in range(n)]
        if m in (0xD8, 0x01) or 0xD0 <= m <= 0xD7:
            i += 2
            continue
        i += 2 + int.from_bytes(b[i + 2 : i + 4], "big")
    return None


def check_one(r, root, ex):
    """One manifest row: (format, file, result, details, flagged)."""
    rel = r[0]
    frame = page = 0
    base_rel = rel
    if "#" in rel:
        base_rel, k = rel.rsplit("#", 1)
        frame = int(k)
    elif "@" in rel:
        base_rel, k = rel.rsplit("@", 1)
        page = int(k)
    path = root / base_rel
    group = Path(rel).parts[0]
    fmt = f"{GROUP_FMT.get(group, group.upper())}/{Path(rel).parts[1] if len(Path(rel).parts) > 2 else 'misc'}"
    detail, flagged = [], 0
    refs, meta, errs = references(path, group, frame, page)
    if group == "psd" and page and len(r) > 12 and r[12].startswith("layer="):
        try:
            label = r[12][len("layer=") :]
            name = label.split(":", 1)[1]
            nth = sum(1 for q, l in PAGE_LABELS.get(base_rel, {}).items() if q < page and l == name)
            refs.append(("psd-tools", ref_psd_layer(path, label, nth)))
        except Exception as e:  # noqa: BLE001
            errs.append(str(e)[:120])
    truth = Path(str(path) + ".npy") if not (frame or page) else Path("/nonexistent")
    tmeta = {}
    # A test suite's expected output outranks the decoders (like the generator's array).
    refs.sort(key=lambda x: x[0] not in EXACT)
    if truth.exists():
        refs.insert(0, ("generator", np.load(truth)))
        jm = Path(str(path) + ".json")
        if jm.exists():
            tmeta = json.loads(jm.read_text())
    if group in GROUPS5:
        lossy = lossy5(group, rel, truth.exists())
    else:
        lossy = any(h in rel.lower() for h in LOSSY_HINTS) or (group == "heic") or (group == "avif" and "lossless" not in rel) or (group == "jxl" and not truth.exists() and "lossless" not in rel)
    hdr = group in ("exr", "hdr", "film") or bool(tmeta.get("hdr"))
    if r[1] != "ok":
        if not refs:
            return fmt, rel, "skip", [f"neither we nor a reference decode it ({r[1][4:80]}; {'; '.join(errs)[:160]})"], 0
        return fmt, rel, "fail", [f"ours: {r[1][4:200]}; references decode it ({', '.join(n for n, _ in refs)})"], 0
    if not refs:
        return fmt, rel, "skip", ["no reference decodes it: " + "; ".join(errs)[:200]], 0
    global REL, ALPHA_MIN
    REL, ALPHA_MIN = hdr, (1e-3 if group == "exr" else 0.0)
    w, h, layout, bits, orient, has_icc, trunc, fmtname, raw, shown = r[2:12]
    w, h, bits, orient = int(w), int(h), int(bits), int(orient)
    ours = np.fromfile(raw, "<f4").reshape(h, w, 4)
    if group == "film":
        # References read code values; specular-film shows them as linear light.
        dens = cineon_density(path)
        refs = [(n, a if n == "generator" else cineon_codes_to_linear(a, dens)) for n, a in refs]
    if group == "psd" and psd_depth(path) == 32:
        # 32-bit documents are linear; the merged image is shown through the sRGB curve.
        refs = [(n, np.concatenate([srgb_encode(as_rgba(a)[..., :3]), as_rgba(a)[..., 3:]], -1).astype(np.float32)) for n, a in refs]
    if group in ("hdr", "film") and layout.startswith("Rgba16F"):
        with np.errstate(over="ignore"):
            refs = [(n, a.astype(np.float16).astype(np.float32)) for n, a in refs]
    ours_bits = 8 if layout in ("Gray", "Rgb", "Rgba", "YCbCr", "Yuv") else 16
    if layout.startswith("Rgba16F"):
        ours_bits = 11
    if group in GROUPS5 and ours_bits == 8 and not hdr and not lossy:
        # An exact 8-bit decoder rounds to the nearest code: references from wider or derived
        # samples (32-bit floats, 10-bit fields, un-matted colour) go on that grid first.
        refs = [(n, (np.round(np.nan_to_num(a) * 255) / 255).astype(np.float32)) for n, a in refs]
    if group == "exr":
        # OpenEXR colour is associated with alpha; ours is straight. Our half-float raster
        # holds what a half holds (a float file's 1e30 is +inf there).
        refs = [(n, unpremultiply(a)) for n, a in refs]
        if layout.startswith("Rgba16F"):
            with np.errstate(over="ignore"):
                refs = [(n, a.astype(np.float16).astype(np.float32)) for n, a in refs]
        if meta.get("dw") and meta["dw"] != meta.get("disp"):
            # We show the data window (exr_dec.rs: a design decision); FFmpeg the display
            # window. Compare with OpenEXR's data window only.
            refs = [(n, a) for n, a in refs if n not in ("ffmpeg",)]
            detail.append("data window shown (display window differs)")
    if group in ("jxl", "avif", "heic") and orient != 1:
        # libjxl and heif-dec hand out pixels already oriented (libavif does not; where the
        # two disagree ours, oriented, matches heif-dec).
        ours = ORIENT[orient](ours)
    lossy = lossy or bool(meta.get("lossy"))
    tol = tolerance(group, rel, min(ours_bits, 16), refs[0][1], lossy, hdr, bits)
    if group == "psd" and not lossy and not hdr and (as_rgba(refs[0][1])[..., 3] < 1).any():
        # Un-matted colour (from white-matted samples) is a quotient: decoders round it a
        # code apart (psd-tools and ImageMagick differ the same way).
        tol = {k: max(v, 1.01 / 255) for k, v in tol.items()}
    dis = agree([x for x in refs if x[0] not in EXACT], tol)
    if dis:
        detail.append(f"oracle disagreement: {dis}")
    best = None
    for n, a in refs:
        if tmeta.get("cmyk_naive") and n == "generator":
            continue
        st = diff_stats(ours, a) if as_rgba(a).shape == ours.shape else None
        ok = st is not None and passes(st, tol)
        if best is None or (ok and not best[2]):
            best = (n, st, ok, a)
        if ok and (n in EXACT or not dis):
            break
    n, st, ok, a = best
    if ok:
        res = "pass"
        s = systematic(st, tol)
        if s and not lossy:
            detail.append(f"{s} vs {n}")
            flagged += 1
        if dis:
            detail.append(f"ours = {n}")
    else:
        res = "fail"
        notes = classify(ours, a, tol)
        if st:
            detail.append(f"vs {n}: max {st['max'] * 255:.2f} mean {st['mean'] * 255:.3f} p99.9 {st['p999'] * 255:.2f} alpha {st['amax'] * 255:.1f} /255; " + "; ".join(notes))
        else:
            detail.append(f"vs {n}: " + "; ".join(notes))
    if group == "webp" and layout == "YCbCr" and not (frame or page):
        # Range-expanded planes (older builds): the planes decide.
        pc = webp_planes_check(path, raw, layout)
        if pc is not None:
            detail.append(pc[1])
            res = "pass" if pc[0] else "fail"
    if group == "webp" and layout == "Yuv" and not (frame or page):
        # VP8's own planes must equal libwebp's, and the RGB above (the viewer's conversion)
        # must pass against libwebp's RGB too.
        pc = webp_planes_check(path, raw, layout)
        if pc is not None:
            detail.append(pc[1])
            if not pc[0]:
                res = "fail"
    if group == "jpeg" and layout == "YCbCr" and not (frame or page) and res == "fail":
        # The same for JPEG: chroma reconstruction of tiny or 4x-subsampled pictures differs
        # between decoders (libjpeg replicates, FFmpeg filters), the planes do not.
        pc = jpeg_planes_check(path, raw, w, h)
        if pc is not None:
            detail.append(pc[1] + "; shown not checked (chroma upsampling convention)")
            res = "pass" if pc[0] else "fail"
            shown = "-" if pc[0] else shown
    # Metadata: orientation against exiftool (EXIF/TIFF) or the generator (HEIF irot/imir).
    e = ex.get(str(path), {}) if not (frame or page) else {}
    eo = e.get("Orientation") if group not in ("avif", "heic", "jxl") else None
    want_o = tmeta.get("orientation", eo if isinstance(eo, int) else None)
    if isinstance(want_o, int) and want_o != orient and 1 <= want_o <= 8:
        detail.append(f"Info orientation {orient}, exiftool/generator {want_o}")
        res = "fail"
    # The displayed picture (SDR formats whose colour we can model here).
    if group == "jpeg" and jpeg_max_sampling(path) > 2:
        detail.append("shown not checked: the viewer filters 4x-subsampled chroma bilinearly, libjpeg replicates it")
    elif shown != "-" and not hdr and res == "pass" and group not in ("avif", "heic", "jxl") and not (frame or page):
        icc = None
        try:
            from PIL import Image

            with Image.open(path) as im:
                icc = im.info.get("icc_profile")
        except Exception:  # noqa: BLE001
            pass
        if icc is None and group == "jp2":
            icc = jp2_icc(path)
        if group == "psd":
            # Pillow opens no PSB; 32-bit documents carry a linear profile, and the reference
            # was put through the sRGB curve already (a linear-sRGB profile is assumed).
            icc = None if psd_depth(path) == 32 else psd_icc(path)
        transfer = None
        if group == "png":
            transfer = png_decode_exponent(path)
        elif group == "dpx" and dpx_transfer(path) == 2:
            transfer = "linear"
        elif layout.startswith("Rgba16F"):
            # Float TIFF is scene-linear HDR, like OpenEXR (tiff_dec.rs).
            transfer = "linear"
        dref = display_ref(a, want_o or 1, icc if has_icc == "1" else None, transfer, gray_icc=group in GROUPS5)
        sh = load_png(shown)
        if sh.shape[:2] != dref.shape[:2]:
            detail.append(f"shown {sh.shape[1]}x{sh.shape[0]}, expected {dref.shape[1]}x{dref.shape[0]}")
            res = "fail"
        else:
            vis = dref[..., 3] > 0.5
            dd = np.abs(sh[..., :3] - dref[..., :3])[vis]
            if dd.size and (dd.mean() > 2.5 / 255 or np.quantile(dd, 0.999) > 12 / 255):
                nn = classify(sh[..., :3], dref[..., :3], {"mean": 2.5 / 255, "p999": 12 / 255, "max": 1.0})
                detail.append(f"shown: mean {dd.mean() * 255:.2f} p99.9 {np.quantile(dd, 0.999) * 255:.1f} /255 vs reference display; " + "; ".join(nn))
                res = "fail"
    return fmt, rel, res, detail, flagged


def check(root, manifest):
    root = Path(root)
    rows = [l.rstrip("\n").split("\t") for l in open(manifest) if l.strip()]
    ex = exif_facts(sorted({root / r[0].split("#")[0].split("@")[0] for r in rows}))
    for r in rows:
        if "@" in r[0] and len(r) > 12 and r[12].startswith("layer="):
            b, q = r[0].rsplit("@", 1)
            PAGE_LABELS.setdefault(b, {})[int(q)] = r[12].split(":", 1)[1]
    totals = {}
    flagged = 0
    unexpected = 0
    for r in rows:
        try:
            fmt, rel, res, detail, fl = check_one(r, root, ex)
        except Exception as e:  # noqa: BLE001 - one file's trouble is reported, not fatal
            rel = r[0]
            fmt = f"{GROUP_FMT.get(Path(rel).parts[0], 'MISC')}/{Path(rel).parts[1] if len(Path(rel).parts) > 2 else 'misc'}"
            tb = traceback.extract_tb(e.__traceback__)[-1]
            res, detail, fl = "skip", [f"oracle script error: {type(e).__name__}: {str(e)[:120]} (line {tb.lineno})"], 0
        flagged += fl
        known = next((v for k, v in KNOWN.items() if k in rel), None)
        joined = " ".join(detail)
        known = known or next((v for k, v in KNOWN_ERRORS.items() if k in joined and "ours:" in joined), None)
        known = known or next((v for k, v in KNOWN_DETAILS.items() if k in joined), None)
        if res == "fail" and known:
            detail.append(f"known: {known}")
        elif res == "fail":
            unexpected += 1
        totals.setdefault(fmt, [0, 0, 0])[{"pass": 0, "fail": 1, "skip": 2}[res]] += 1
        print(f"ORACLE\t{fmt}\t{rel}\t{res}", flush=True)
        if detail:
            print("#\t" + rel + "\t" + " | ".join(detail), flush=True)
    print("# summary (format: pass/fail/skip)")
    for k in sorted(totals):
        p, f, s = totals[k]
        print(f"#\t{k}\t{p}/{f}/{s}")
    print(f"# systematic flags under tolerance: {flagged}")
    print(f"#\tunexpected\t{unexpected}")


def main():
    if len(sys.argv) >= 3 and sys.argv[1] == "gen":
        gen(sys.argv[2])
    elif len(sys.argv) >= 3 and sys.argv[1] == "gen-extra":
        want = set(sys.argv[3:])
        gen(sys.argv[2], tuple(f for f in GEN_EXTRA if not want or f.__name__[4:] in want))
    elif len(sys.argv) >= 4 and sys.argv[1] == "check":
        check(sys.argv[2], sys.argv[3])
    else:
        print(__doc__, file=sys.stderr)
        sys.exit(2)


if __name__ == "__main__":
    main()
