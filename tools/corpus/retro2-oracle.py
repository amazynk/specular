#!/usr/bin/env python3
"""Photo CD oracle for specular-retro2: writes a generated Photo CD image pack and the
Base image as decoded by Pillow and by ImageMagick (raw 768x512 RGB), into the folder given.

    python3 -I tools/corpus/retro2-oracle.py $CARGO_TARGET_DIR/corpus/retro2
    SPECULAR_RETRO2_CORPUS=$CARGO_TARGET_DIR/corpus/retro2 \
        cargo test -p specular-retro2 --test oracle -- --ignored --nocapture
"""
import os
import subprocess
import sys

from PIL import Image

SECTOR = 2048


def pack(w, h, y_of, c_of):
    """PhotoYCC 4:2:0 in Photo CD's order: two luma lines, then C1 and C2 lines."""
    out = bytearray()
    for py in range(h // 2):
        for dy in range(2):
            out += bytes(y_of(x, py * 2 + dy, w, h) for x in range(w))
        out += bytes(c_of(0, x, py, w, h) for x in range(w // 2))
        out += bytes(c_of(1, x, py, w, h) for x in range(w // 2))
    return out


def y_of(x, y, w, h):
    return (x * 255 // (w - 1) + y * 64 // h) % 256


def c_of(which, x, y, w, h):
    if which == 0:
        return 156 + ((x * 80 // (w // 2)) - 40)
    return 137 + ((y * 80 // (h // 2)) - 40)


def main():
    out = sys.argv[1]
    os.makedirs(out, exist_ok=True)
    data = bytearray(96 * SECTOR + 768 * 512 * 3 // 2)
    data[SECTOR:SECTOR + 7] = b"PCD_IPI"
    for (w, h, sector) in ((192, 128, 4), (384, 256, 23), (768, 512, 96)):
        p = pack(w, h, y_of, c_of)
        data[sector * SECTOR:sector * SECTOR + len(p)] = p
    path = os.path.join(out, "synthetic.pcd")
    with open(path, "wb") as f:
        f.write(data)
    im = Image.open(path)
    im.load()
    with open(os.path.join(out, "synthetic.pil.rgb"), "wb") as f:
        f.write(im.convert("RGB").tobytes())
    env = dict(os.environ, MAGICK_HOME=out)
    raw = subprocess.run(
        ["convert", path, "-depth", "8", "rgb:-"], capture_output=True, env=env
    )
    if raw.returncode == 0 and len(raw.stdout) == 768 * 512 * 3:
        with open(os.path.join(out, "synthetic.im.rgb"), "wb") as f:
            f.write(raw.stdout)
    else:
        print("ImageMagick did not decode Base:", raw.stderr[:200])
    print("wrote", out)


main()
