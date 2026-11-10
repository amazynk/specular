#!/usr/bin/env python3
"""Oracle for the fmt500d formats: writes generated files and Pillow's (and other
oracles') decodes beside the fetched corpus (tools/corpus/fetch-fmt500d.sh).

    tools/corpus/fetch-fmt500d.sh
    $CARGO_TARGET_DIR/venv/bin/python -I tools/corpus/fmt500d-oracle.py $CARGO_TARGET_DIR/corpus/fmt500d
    SPECULAR_FMT500D_CORPUS=$CARGO_TARGET_DIR/corpus/fmt500d \\
        cargo test -p specular-legacy2 --test oracle_fmt500d -- --ignored --nocapture

Needs Pillow 13 and numpy (and texture2ddecoder, etcpak for the PKM files) in the venv.
Each `NAME.rgba` is RGBA8, row by row; each `NAME.f64` little-endian float64 values; each
`NAME.size` "width height".
"""
import os
import struct
import sys

import numpy as np
from PIL import Image, GimpGradientFile, GimpPaletteFile, GdImageFile

out = sys.argv[1]
gen = os.path.join(out, "gen")
os.makedirs(gen, exist_ok=True)


def put_rgba(name, im):
    im = im.convert("RGBA")
    with open(name + ".rgba", "wb") as f:
        f.write(im.tobytes())
    with open(name + ".size", "w") as f:
        f.write("%d %d" % im.size)


def put_f64(name, im):
    a = np.asarray(im, dtype=np.float64)
    with open(name + ".f64", "wb") as f:
        f.write(a.astype("<f8").tobytes())
    with open(name + ".size", "w") as f:
        f.write("%d %d" % im.size)


def p(n):
    return os.path.join(out, n)


hopper = Image.open(p("hopper.png")).convert("RGB")

# Rasters Pillow opens directly.
for n in ["hopper.msp", "hopper.pxr", "hopper.im", "bw_gradient.imt", "hopper.p7",
          "gbr.gbr"]:
    im = Image.open(p(n))
    im.load()
    put_rgba(p(n), im)
im = GdImageFile.open(p("hopper.gd"))
im.load()
put_rgba(p("hopper.gd"), im)

# Measurements: values, not display levels.
for n in ["hopper.spider", "cmx3g8_wv_1998.260_0745_mcidas.ara"]:
    im = Image.open(p(n))
    im.load()
    put_f64(p(n), im)

# LabEye IM in each mode Pillow writes.
for mode in ["1", "L", "P", "LA", "RGB", "RGBA", "CMYK", "YCbCr", "I", "F", "I;16", "I;16B"]:
    if mode in ("I", "F"):
        src = hopper.convert("L").convert(mode)
        src = src.point(lambda v: v * 3 - 100)
    elif mode in ("I;16", "I;16B"):
        src = hopper.convert("L").convert("I").point(lambda v: v * 257).convert(mode)
    elif mode == "P":
        src = hopper.quantize(64)
    else:
        src = hopper.convert(mode)
    name = os.path.join(gen, "hopper_%s.im" % mode.replace(";", "_"))
    src.save(name)
    back = Image.open(name)
    back.load()
    if mode in ("I", "F"):
        put_f64(name, back)
    elif mode in ("I;16", "I;16B"):
        put_f64(name, back)
    else:
        put_rgba(name, back)

# A two-image IM file (Pillow writes one image; the second frame is appended by hand).
one = open(os.path.join(gen, "hopper_RGB.im"), "rb").read()
flipped = hopper.transpose(Image.Transpose.ROTATE_180)
planes = np.asarray(flipped).transpose(0, 2, 1).copy()  # rows of R, G, B lines
two = one[:512].replace(b"(no of images): 1", b"(no of images): 2")
with open(os.path.join(gen, "two.im"), "wb") as f:
    f.write(two + one[512:] + planes.tobytes())
back = Image.open(os.path.join(gen, "two.im"))
back.seek(1)
put_rgba(os.path.join(gen, "two.im.1"), back)

# Palettes and gradients: Pillow's parsed colours.
for n in ["full_gimp_palette.gpl", "custom_gimp_palette.gpl"]:
    with open(p(n), "rb") as f:
        pal, _ = GimpPaletteFile.GimpPaletteFile(f).getpalette()
    with open(p(n) + ".rgb", "wb") as f:
        f.write(pal)
# A well-formed gradient: contiguous segments, one per blending function Pillow knows.
# (Pillow's own test gradients overlap and leave 0-1, where readers disagree.)
segs = []
cols = [(1, 0, 0, 1), (0, 1, 0, 0.5), (0, 0, 1, 1), (1, 1, 0, 1), (0.2, 0.4, 0.6, 0.8),
        (1, 1, 1, 1)]
for k in range(5):
    l, r = k / 5, (k + 1) / 5
    m = l + (r - l) * (0.3 + 0.1 * k)
    a, b = cols[k], cols[k + 1]
    segs.append("%f %f %f %s %s %d 0" % (l, m, r, " ".join("%f" % x for x in a),
                                       " ".join("%f" % x for x in b), k))
with open(os.path.join(gen, "blends.ggr"), "w") as f:
    f.write("GIMP Gradient\nName: Blends\n5\n" + "\n".join(segs) + "\n")
for n in ["gimp_gradient.ggr", "gimp_gradient_with_name.ggr", "gen/blends.ggr"]:
    with open(p(n), "rb") as f:
        pal, mode = GimpGradientFile.GimpGradientFile(f).getpalette(256)
    assert mode == "RGBA"
    with open(p(n) + ".rgba256", "wb") as f:
        f.write(pal)

# MSP version 1 written by Pillow from a bilevel image.
hopper.convert("1").save(os.path.join(gen, "hopper1.msp"))
put_rgba(os.path.join(gen, "hopper1.msp"), Image.open(os.path.join(gen, "hopper1.msp")))

# PGX: the samples as written (OpenJPEG's own reference output; read by numpy here).
with open(p("c1p0_01_0.pgx"), "rb") as f:
    d = f.read()
nl = d.index(b"\n")
hdr = d[:nl].decode().split()
w, h = int(hdr[-2]), int(hdr[-1])
a = np.frombuffer(d[nl + 1:nl + 1 + w * h], dtype=np.uint8).reshape(h, w)
with open(p("c1p0_01_0.pgx") + ".f64", "wb") as f:
    f.write(a.astype("<f8").tobytes())
with open(p("c1p0_01_0.pgx") + ".size", "w") as f:
    f.write("%d %d" % (w, h))

# CALS: hopper_g4.tif's G4 strip behind a CALS header, against Pillow's TIFF decode.
tif = Image.open(p("hopper_g4.tif"))
tif.load()
assert tif.tag_v2.get(259) == 4, "expected a G4 TIFF"
offs = tif.tag_v2[273]
counts = tif.tag_v2[279]
assert len(offs) == 1, "one strip expected"
with open(p("hopper_g4.tif"), "rb") as f:
    raw = f.read()
g4 = raw[offs[0]:offs[0] + counts[0]]
w, h = tif.size
header = ""
for rec in ["srcdocid: NONE", "dstdocid: NONE", "txtfilid: NONE", "figid: NONE",
            "srcgph: NONE", "doccls: NONE", "rtype: 1", "rorient: 000,270",
            "rpelcnt: %06d,%06d" % (w, h), "rdensty: 0200", "notes: NONE"]:
    header += rec.ljust(128)
header = header.ljust(2048)
with open(os.path.join(gen, "hopper.cal"), "wb") as f:
    f.write(header.encode() + g4)
put_rgba(os.path.join(gen, "hopper.cal"), tif)

# PKM: ETC1 and ETC2 written by etcpak, decoded by texture2ddecoder.
try:
    import etcpak
    import texture2ddecoder as t2d
    rgba = np.asarray(hopper.convert("RGBA").resize((64, 32)))
    for ver, fn, dec, fmt in [
        (b"10", etcpak.compress_etc1_rgb, t2d.decode_etc1, 0),
        (b"20", etcpak.compress_etc2_rgb, t2d.decode_etc2, 1),
        (b"20", etcpak.compress_etc2_rgba, t2d.decode_etc2a8, 3),
    ]:
        blocks = fn(rgba.tobytes(), 64, 32)
        pkm = b"PKM " + ver + struct.pack(">HHHHH", fmt, 64, 32, 64, 32) + blocks
        name = os.path.join(gen, "hopper_%s_%d.pkm" % (ver.decode(), fmt))
        with open(name, "wb") as f:
            f.write(pkm)
        bgra = np.frombuffer(dec(blocks, 64, 32), dtype=np.uint8).reshape(32, 64, 4)
        put_rgba(name, Image.fromarray(bgra[:, :, [2, 1, 0, 3]].copy(), "RGBA"))
except ImportError as e:
    print("PKM skipped:", e)

# Firaxis textures.
for n in ["ftex_dxt1.ftc", "ftex_uncompressed.ftu"]:
    im = Image.open(p(n))
    im.load()
    put_rgba(p(n), im)

# Aseprite sprites: every frame composited by aseprite-py (Apache-2.0, run only; Normal
# blending only), and a summary line per sprite (size, frames, layers, tags).
try:
    import aseprite
    import glob
    ase_out = os.path.join(gen, "ase")
    os.makedirs(ase_out, exist_ok=True)
    for path in sorted(glob.glob(os.path.join(out, "*.aseprite"))):
        name = os.path.basename(path)
        spr = aseprite.Sprite.open(path)
        w, h = spr.size
        with open(os.path.join(ase_out, name + ".txt"), "w") as f:
            f.write("%d %d %d %d %d\n" % (w, h, len(spr.frames), len(spr.layers), len(spr.tags)))
            for l in spr.layers:
                f.write("layer %s|%d|%d\n" % (l.name, int(l.visible), l.child_level))
            for fr in spr.frames:
                f.write("duration %d\n" % fr.duration_ms)
        for k in range(len(spr.frames)):
            with open(os.path.join(ase_out, "%s.%d.rgba" % (name, k)), "wb") as f:
                f.write(spr.flatten(k))
except ImportError as e:
    print("Aseprite skipped:", e)

# QOA: a small encoder written here from the specification (no QOA encoder wheel builds
# for this Python, FFmpeg only decodes it), then FFmpeg's native decoder as the oracle.
import math
import subprocess

QOA_SCALE = [round(pow(s + 1, 2.75)) for s in range(16)]
QOA_DQ = [0.75, -0.75, 2.5, -2.5, 4.5, -4.5, 7, -7]


def qoa_dequant(sf, q):
    v = QOA_SCALE[sf] * QOA_DQ[q]
    return int(math.floor(abs(v) + 0.5)) * (1 if v >= 0 else -1)


def qoa_encode(chans, rate):
    ch, n = len(chans), len(chans[0])
    out = bytearray(b"qoaf" + struct.pack(">I", n))
    lms = [([0, 0, 0, 0], [0, 0, -(1 << 13), 1 << 14]) for _ in range(ch)]
    for f0 in range(0, n, 5120):
        fl = min(5120, n - f0)
        slices = (fl + 19) // 20
        size = 8 + 16 * ch + 8 * slices * ch
        out += bytes([ch]) + rate.to_bytes(3, "big") + struct.pack(">HH", fl, size)
        for h, w in lms:
            out += struct.pack(">4h", *h) + struct.pack(">4h", *[max(-32768, min(32767, x)) for x in w])
        for s in range(slices):
            for c in range(ch):
                best = None
                for sf in range(16):
                    h, w = list(lms[c][0]), list(lms[c][1])
                    err, codes = 0, []
                    for j in range(min(20, fl - s * 20)):
                        x = chans[c][f0 + s * 20 + j]
                        pred = sum(a * b for a, b in zip(w, h)) >> 13
                        r = x - pred
                        q = min(range(8), key=lambda q: abs(qoa_dequant(sf, q) - r))
                        dq = qoa_dequant(sf, q)
                        v = max(-32768, min(32767, pred + dq))
                        err += (x - v) ** 2
                        d = dq >> 4
                        w = [wi + (-d if hi < 0 else d) for wi, hi in zip(w, h)]
                        h = h[1:] + [v]
                        codes.append(q)
                    if best is None or err < best[0]:
                        best = (err, sf, codes, h, w)
                _, sf, codes, h, w = best
                bits = sf << 60
                for j, q in enumerate(codes):
                    bits |= q << (57 - 3 * j)
                out += struct.pack(">Q", bits)
                lms[c] = (h, w)
    return bytes(out)


rate = 22050
n = rate // 3
left = [int(12000 * math.sin(2 * math.pi * 440 * i / rate)) for i in range(n)]
right = [int(9000 * math.sin(2 * math.pi * 660 * i / rate) + 2000 * math.sin(i * 0.37)) for i in range(n)]
qoa_path = os.path.join(gen, "tones.qoa")
with open(qoa_path, "wb") as f:
    f.write(qoa_encode([left, right], rate))
subprocess.run(["ffmpeg", "-v", "error", "-y", "-i", qoa_path, "-f", "s16le", "-acodec",
                "pcm_s16le", qoa_path + ".s16"], check=True)

# APV: FFmpeg's native decoder of the FATE sample.
subprocess.run(["ffmpeg", "-v", "error", "-y", "-i", p("profile_422-10.apv"), "-f", "rawvideo",
                "-pix_fmt", "yuv422p10le", os.path.join(gen, "profile_422-10.yuv")], check=True)

print("wrote", out)
