#!/usr/bin/env python3
"""Type 1 oracle for specular-font: copies Type 1 fonts from the system's TeX and URW trees
into the folder given and writes, per font, fontTools' glyph metrics: one line per glyph,
`name advance xmin ymin xmax ymax` (bounds of the outline's points, `-` for an empty glyph).

    python3 -I tools/corpus/type1-oracle.py $CARGO_TARGET_DIR/corpus/type1
    SPECULAR_TYPE1_CORPUS=$CARGO_TARGET_DIR/corpus/type1 \
        cargo test -p specular-font --test type1 -- --ignored --nocapture
"""
import os
import shutil
import subprocess
import sys

from fontTools.pens.basePen import BasePen
from fontTools.t1Lib import T1Font

FONTS = ["cmr10.pfb", "cmmi10.pfb", "cmsy10.pfb", "cmbx12.pfb", "cmtt10.pfb",
         "utmr8a.pfb", "uhvb8a.pfb", "pzcmi8a.pfb", "usyr.pfb"]


class PointBounds(BasePen):
    """Bounds of every on- and off-curve point (as our outline boxes are)."""

    def __init__(self):
        super().__init__(None)
        self.b = None

    def _add(self, p):
        x, y = p
        if self.b is None:
            self.b = [x, y, x, y]
        else:
            self.b = [min(self.b[0], x), min(self.b[1], y), max(self.b[2], x), max(self.b[3], y)]

    def _moveTo(self, p):
        self._add(p)

    def _lineTo(self, p):
        self._add(p)

    def _curveToOne(self, a, b, c):
        for p in (a, b, c):
            self._add(p)

    def _closePath(self):
        pass


def main():
    out = sys.argv[1]
    os.makedirs(out, exist_ok=True)
    for name in FONTS:
        p = subprocess.run(["kpsewhich", name], capture_output=True, text=True).stdout.strip()
        if not p:
            print("not found:", name)
            continue
        dst = os.path.join(out, name)
        shutil.copy(p, dst)
        f = T1Font(dst)
        gs = f.getGlyphSet()
        lines = []
        for g in sorted(gs.keys()):
            pen = PointBounds()
            gs[g].draw(pen)
            w = gs[g].width
            b = pen.b
            lines.append(f"{g} {w:g} " + (" ".join(f"{v:g}" for v in b) if b else "-"))
        with open(dst + ".glyphs", "w") as fh:
            fh.write("\n".join(lines) + "\n")
        print(name, len(lines), "glyphs")
        # The same font as PFA (hex eexec section), when t1utils is installed.
        if name == "cmr10.pfb" and shutil.which("t1ascii"):
            pfa = os.path.join(out, "cmr10-ascii.pfa")
            subprocess.run(["t1ascii", dst, pfa], check=True)
            shutil.copy(dst + ".glyphs", os.path.join(out, "cmr10-ascii.pfb.glyphs"))


main()
