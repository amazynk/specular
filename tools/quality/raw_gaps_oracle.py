#!/usr/bin/env python3
"""Four-plane DNG colour (camera raw gaps) against LibRaw.

The synthetic CMYG DNGs of `crates/formats/specular-raw/tests/dng_cfa_gaps.rs` carry their
own 4 x 3 ColorMatrix and a four-value AsShotNeutral. Specular converts them by that matrix;
LibRaw (rawpy) does too. Both developed outputs (linear sRGB, camera white balance, no
auto-brightness) must show the same colour: the centre pixel's channel ratios are compared.

    SPECULAR_RAWGAPS_DUMP=<dir> cargo test -p specular-raw --test dng_cfa_gaps \\
        dump_oracle_inputs -- --ignored
    <venv>/bin/python -I tools/quality/raw_gaps_oracle.py <dir>

Exit status 1 when a ratio differs by more than 1 %.
"""

import sys
from pathlib import Path

import numpy as np
import rawpy


def main() -> int:
    d = Path(sys.argv[1])
    bad = 0
    for line in (d / "specular.txt").read_text().splitlines():
        name, *ours = line.split()
        ours = np.array([float(v) for v in ours])
        with rawpy.imread(str(d / name)) as r:
            rgb = r.postprocess(
                use_camera_wb=True,
                no_auto_bright=True,
                gamma=(1, 1),
                output_bps=16,
                output_color=rawpy.ColorSpace.sRGB,
                user_flip=0,
            ).astype(np.float64)
        h, w, _ = rgb.shape
        theirs = rgb[h // 2 - 4 : h // 2 + 4, w // 2 - 4 : w // 2 + 4].reshape(-1, 3).mean(0)
        if theirs[1] <= 0:
            # LibRaw takes CFAPattern values as colour codes, not as indices into
            # CFAPlaneColor (DNG 1.7): planes whose codes differ from their indices (CMYG)
            # come out without green there.
            print(f"{name}: not comparable (LibRaw reads the pattern as colour codes)")
            continue
        a, b = ours / ours[1], theirs / theirs[1]
        diff = np.abs(a - b).max()
        bad += diff > 0.01
        print(f"{name}: specular r/g b/g {a[0]:.4f} {a[2]:.4f}  libraw {b[0]:.4f} {b[2]:.4f}  max diff {diff:.4f}")
    print(f"mismatches: {bad}")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
