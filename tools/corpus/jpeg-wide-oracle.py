#!/usr/bin/env python3
"""Oracle for specular-jpeg's full-precision decode (`decode_wide`, `tests/wide.rs`).

Decodes every NAME.jpg in DIR without a colour transform with two decoders, run here as
separate tool processes (never linked):
  * libjpeg-turbo 3 through imagecodecs (islow IDCT, like ours: must match exactly), where it
    can (no hierarchical mode): NAME.turbo.u16 / NAME.turbo.txt;
  * pylibjpeg-libjpeg (Thomas Richter's reference libjpeg, GPL-3; every T.81 process, its own
    integer IDCT, so DCT samples may differ by rounding): NAME.ref.u16 / NAME.ref.txt.
The .u16 files hold little-endian samples, interleaved; the .txt files "w h channels".

  cargo test -p specular-jpeg --test wide -- --ignored write_oracle_streams
  python -I tools/corpus/jpeg-wide-oracle.py tools/corpus/out/jpeg-wide
  cargo test -p specular-jpeg --test wide matches_reference_libjpeg_oracle -- --nocapture

Needs: pip install numpy imagecodecs pylibjpeg-libjpeg (in a venv under the target directory).
"""
import sys
from pathlib import Path

import imagecodecs
import numpy as np
from libjpeg import decode


def turbo(b: bytes) -> np.ndarray:
    try:
        return imagecodecs.jpeg8_decode(b, colorspace="YCbCr", outcolorspace="YCbCr")
    except Exception:  # noqa: BLE001 - greyscale: no colour space to keep
        return imagecodecs.jpeg8_decode(b)


def write(p: Path, tag: str, a: np.ndarray) -> None:
    h, w = a.shape[:2]
    ch = 1 if a.ndim == 2 else a.shape[2]
    a.astype("<u2").tofile(p.with_suffix(f".{tag}.u16"))
    p.with_suffix(f".{tag}.txt").write_text(f"{w} {h} {ch}\n")
    print(f"{p.name} {tag}: {w}x{h}x{ch} max {int(a.max())}")


def main(d: Path) -> None:
    for p in sorted(d.glob("*.jpg")):
        b = p.read_bytes()
        for tag, dec in (("turbo", turbo), ("ref", lambda b: decode(b, colour_transform=0))):
            try:
                write(p, tag, dec(b))
            except Exception as e:  # noqa: BLE001 - report and go on
                print(f"{p.name} {tag}: oracle failed: {e}")


if __name__ == "__main__":
    main(Path(sys.argv[1]))
