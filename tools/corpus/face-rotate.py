#!/usr/bin/env python3
"""Rotated copies of the face test photos, for the rotation checks of People
(crates/specular-ai/tests/faces.rs `real_faces_found_in_turned_and_exif_photos`).

Usage: python -I tools/corpus/face-rotate.py GROUP_DIR OUT_DIR [COUNT]

For the first COUNT (default 12) JPEGs of GROUP_DIR (tools/corpus/fetch-faces.sh's group
photos), shrunk to at most 1600 px: NN.jpg (upright, no orientation tag), NN-exif6.jpg,
NN-exif3.jpg and NN-exif8.jpg (the pixels stored turned, with the EXIF orientation that
shows them upright again), and NN-roll1.jpg / NN-roll2.jpg / NN-roll3.jpg (the picture
itself turned a quarter clockwise, upside down, a quarter back, no tag: faces lying on their
side or upside down).
"""

import os
import sys

from PIL import Image


def main():
    src, out = sys.argv[1], sys.argv[2]
    count = int(sys.argv[3]) if len(sys.argv) > 3 else 12
    os.makedirs(out, exist_ok=True)
    names = sorted(n for n in os.listdir(src) if n.lower().endswith(".jpg"))[:count]
    for n in names:
        im = Image.open(os.path.join(src, n)).convert("RGB")
        im.thumbnail((1600, 1600))
        base = os.path.splitext(n)[0]
        im.save(os.path.join(out, base + ".jpg"), quality=92)
        # Stored turned; the tag turns it back (6: show turned 90° clockwise, 3: 180°,
        # 8: 90° counter-clockwise).
        for tag, stored in [(6, Image.Transpose.ROTATE_90), (3, Image.Transpose.ROTATE_180),
                            (8, Image.Transpose.ROTATE_270)]:
            exif = Image.Exif()
            exif[0x0112] = tag
            im.transpose(stored).save(os.path.join(out, f"{base}-exif{tag}.jpg"), quality=92,
                                      exif=exif.tobytes())
        for q, t in [(1, Image.Transpose.ROTATE_270), (2, Image.Transpose.ROTATE_180),
                     (3, Image.Transpose.ROTATE_90)]:
            im.transpose(t).save(os.path.join(out, f"{base}-roll{q}.jpg"), quality=92)


if __name__ == "__main__":
    main()
