"""Pillow's reading of the FlashPix files of the raw400 corpus, for the specular-decode
`raw_old_cameras` test: <corpus>/oracle/<name>.rgba (RGBA8) and <name>.rgba.txt (width height).

    $CARGO_TARGET_DIR/venv/bin/python -I tools/corpus/fpx-oracle.py tools/corpus/raw400

Pillow (and olefile) are oracles only.
"""
import glob
import os
import sys

from PIL import Image

corp = sys.argv[1]
out = os.path.join(corp, "oracle")
os.makedirs(out, exist_ok=True)
for p in sorted(glob.glob(os.path.join(corp, "telparia", "fpx", "*"))):
    rel = os.path.relpath(p, corp).replace("/", "__")
    try:
        im = Image.open(p)
        if im.format != "FPX":
            continue
        im = im.convert("RGBA")
    except Exception:
        continue
    with open(os.path.join(out, rel + ".rgba"), "wb") as f:
        f.write(im.tobytes())
    with open(os.path.join(out, rel + ".rgba.txt"), "w") as f:
        f.write("%d %d\n" % im.size)
