#!/usr/bin/env python3
"""Synthesise the source images of the FLIF test corpus (PNG and 16-bit / odd-depth PNM).

Usage: flif-generate.py OUT_DIR [PHOTO.png ...]

Writes OUT_DIR/src/*.png|*.pnm and OUT_DIR/src/anim-*/NNN.png (animation frames). Photos
given on the command line are cropped and reduced into a few variants. Deterministic (fixed
seeds). Needs numpy and Pillow (the oracle venv).
"""
import os
import sys

import numpy as np
from PIL import Image

OUT = sys.argv[1]
PHOTOS = sys.argv[2:]
SRC = os.path.join(OUT, "src")
os.makedirs(SRC, exist_ok=True)
rng = np.random.default_rng(1234)


def save_png(name, a, mode=None):
    Image.fromarray(a, mode).save(os.path.join(SRC, name + ".png"))


def save_pnm(name, a, maxval):
    """16-bit (or odd maxval) PGM/PPM, big-endian samples."""
    h, w = a.shape[:2]
    c = 1 if a.ndim == 2 else a.shape[2]
    magic = b"P5" if c == 1 else b"P6"
    with open(os.path.join(SRC, name + (".pgm" if c == 1 else ".ppm")), "wb") as f:
        f.write(magic + b"\n%d %d\n%d\n" % (w, h, maxval))
        dt = ">u2" if maxval > 255 else "u1"
        f.write(a.astype(dt).tobytes())


def gradient(w, h, c, depth=8):
    y, x = np.mgrid[0:h, 0:w]
    m = (1 << depth) - 1
    chans = [
        (x * m // max(w - 1, 1)),
        (y * m // max(h - 1, 1)),
        ((x + y) * m // max(w + h - 2, 1)),
        (np.abs(x - w / 2) * 2 * m // max(w, 1)),
    ]
    a = np.stack(chans[:c], axis=-1)
    noise = rng.integers(-(m // 40) - 1, m // 40 + 2, a.shape)
    a = np.clip(a + noise, 0, m)
    return a[..., 0] if c == 1 else a


# Smooth images with noise, every channel count, 8 and 16 bit.
save_png("grad-rgb8", gradient(320, 200, 3).astype(np.uint8))
save_png("grad-grey8", gradient(257, 129, 1).astype(np.uint8))
rgba = gradient(240, 160, 4).astype(np.uint8)
rgba[40:90, 30:120, 3] = 0  # invisible pixels
rgba[100:150, 150:230, 3] = rng.integers(0, 3, (50, 80)) * 127
save_png("grad-rgba8", rgba)
save_pnm("grad-rgb16", gradient(200, 150, 3, 16), 65535)
save_pnm("grad-grey16", gradient(181, 97, 1, 16), 65535)
save_pnm("grad-rgb10", gradient(160, 120, 3, 10), 1023)
save_pnm("grad-grey12", gradient(150, 100, 1, 12), 4095)
save_pnm("grad-grey1", (gradient(64, 48, 1, 1)).astype(np.uint8), 1)
a16 = gradient(128, 96, 4, 16).astype(np.uint16)
a16[10:40, 10:60, 3] = 0
# 16-bit RGBA as PAM (Pillow cannot write it).
with open(os.path.join(SRC, "grad-rgba16.pam"), "wb") as f:
    f.write(b"P7\nWIDTH 128\nHEIGHT 96\nDEPTH 4\nMAXVAL 65535\nTUPLTYPE RGB_ALPHA\nENDHDR\n")
    f.write(a16.astype(">u2").tobytes())

# Few colours: palettes and colour buckets.
pal = rng.integers(0, 256, (12, 3), dtype=np.uint8)
idx = (np.add.outer(np.arange(150) // 13, np.arange(210) // 17)) % 12
save_png("palette-rgb8", pal[idx])
pala = np.concatenate([pal, rng.integers(0, 2, (12, 1), dtype=np.uint8) * 255], axis=1)
save_png("palette-rgba8", pala[idx])
pal2 = rng.integers(0, 256, (700, 3), dtype=np.uint8)
idx2 = rng.integers(0, 700, (120, 160))
save_png("sparse-rgb8", pal2[idx2])
two = (np.indices((90, 70)).sum(axis=0) % 7 == 0).astype(np.uint8) * 255
save_png("bilevel-grey8", two)
save_png("const-rgb8", np.full((40, 30, 3), (12, 200, 99), np.uint8))

# Small colour-bucket and metadata sources (the committed fixtures are encoded from these).
yy, xx = np.mgrid[0:32, 0:48]
cb = np.stack([xx * 5, yy * 7, (xx + yy) * 3], -1).astype(np.uint8)
save_png("cb-src", cb)
icc_path = "/usr/share/color/icc/sRGB-v2-micro.icc"
if os.path.exists(icc_path):
    exif = Image.Exif()
    exif[0x010F] = "SpecularTest"
    Image.fromarray(cb[:8, :16]).save(
        os.path.join(SRC, "meta-src.png"), icc_profile=open(icc_path, "rb").read(), exif=exif.tobytes()
    )

# Odd sizes.
for w, h in [(1, 1), (1, 17), (23, 1), (3, 2), (2, 3), (33, 65)]:
    save_png(f"tiny-{w}x{h}", gradient(w, h, 3).astype(np.uint8))
save_png("tiny-rgba-5x4", gradient(5, 4, 4).astype(np.uint8))

# Noise (worst case for prediction).
save_png("noise-rgb8", rng.integers(0, 256, (96, 128, 3), dtype=np.uint8))

# Photos: crops and a larger bench image.
for i, p in enumerate(PHOTOS):
    im = Image.open(p).convert("RGB")
    a = np.asarray(im)
    save_png(f"photo{i}", a[: min(a.shape[0], 512), : min(a.shape[1], 768)])
    save_png(f"photo{i}-grey", np.asarray(im.convert("L"))[:300, :400])
    ra = np.dstack([a[:256, :256], (np.indices((256, 256))[1] % 64 * 4).astype(np.uint8)])
    save_png(f"photo{i}-rgba", ra.astype(np.uint8))
if PHOTOS:
    # About 3 MP for timings: the first photo tiled.
    a = np.asarray(Image.open(PHOTOS[0]).convert("RGB"))
    big = np.tile(a, (3, 3, 1))[:1536, :2048]
    save_png("bench-3mp", big)

# Animations: a moving square over a gradient, with transparency.
for name, c in [("anim-rgb", 3), ("anim-rgba", 4), ("anim-grey", 1)]:
    d = os.path.join(SRC, name)
    os.makedirs(d, exist_ok=True)
    base = gradient(96, 64, max(c, 3)).astype(np.uint8)
    outs = []
    for f in range(6):
        a = base.copy()
        x = 8 + f * 9
        a[20:40, x : x + 16, :3] = (255 - f * 30, f * 40, 128)
        if c == 4:
            a[..., 3] = 255
            a[0:10, :, 3] = 0
            a[50:60, f * 10 : f * 10 + 20, 3] = 0
        out = a[..., 0] if c == 1 else (a[..., :3] if c == 3 else a)
        if f == 3:
            out = outs[2]  # a duplicate frame
        outs.append(out)
        Image.fromarray(np.ascontiguousarray(out)).save(os.path.join(d, f"{f:03d}.png"))
print("generated", len(os.listdir(SRC)), "sources in", SRC)
