#!/usr/bin/env python3
"""LibRaw (rawpy) renderings of one RAW file for the `rawrest` oracle test of specular-raw
(`cargo test -p specular-raw --test rawrest -- --ignored`).

    $CARGO_TARGET_DIR/venv/bin/python -I tools/quality/rawrest_oracle.py FILE OUT_PREFIX MODE

MODE:
  raw   OUT_PREFIX.raw: the unpacked raw_image (before LibRaw's sensor corrections).
  cfa   OUT_PREFIX.cfa: a full-size bilinear develop in camera colour (no white balance,
        gamma 1, no auto-brightness, no flip). Each photosite keeps its own filter's value in
        its channel, so the black-subtracted, corrected sensor values (Phase One black tables,
        flat fields, defects; RADC; SRF) can be read back at the photosites, up to LibRaw's
        scale to 16 bits.
  dev   OUT_PREFIX.dev: a full-size linear develop (camera white balance where the file has
        one, else LibRaw's daylight default; sRGB primaries, gamma 1, no auto-brightness, no
        flip).
  half  OUT_PREFIX.half: the same at half size (skips LibRaw's Phase One corrections).

Each output: u32 LE height, width, channels, then u16 LE samples, row-major, channels
interleaved. Prints one line of `key=value` facts on stdout. Test-only: rawpy is an oracle,
never a dependency of the viewer.
"""

import struct
import sys

import numpy as np
import rawpy


def write(path, a):
    a = np.ascontiguousarray(a, dtype="<u2")
    shape = a.shape + (1,) if a.ndim == 2 else a.shape
    with open(path, "wb") as f:
        f.write(struct.pack("<3I", *shape))
        f.write(a.tobytes())


def main():
    path, out, mode = sys.argv[1], sys.argv[2], sys.argv[3]
    with rawpy.imread(path) as r:
        s = r.sizes
        pat = r.raw_pattern
        facts = {
            "black": ",".join(str(int(v)) for v in r.black_level_per_channel),
            "white": int(r.white_level),
            "pattern": "" if pat is None else ",".join(str(int(v)) for v in pat.flatten()),
            "pattern_w": 0 if pat is None else pat.shape[1],
            "desc": r.color_desc.decode("latin1"),
            "crop": f"{s.left_margin},{s.top_margin},{s.width},{s.height}",
            "raw": f"{s.raw_width},{s.raw_height}",
            "flip": s.flip,
            "wb": ",".join(f"{v:.5g}" for v in r.camera_whitebalance),
            "daylight": ",".join(f"{v:.5g}" for v in r.daylight_whitebalance),
        }
        if mode == "raw":
            write(out + ".raw", r.raw_image)
        elif mode == "cfa":
            img = r.postprocess(
                demosaic_algorithm=rawpy.DemosaicAlgorithm.LINEAR,
                output_color=rawpy.ColorSpace.raw,
                user_wb=[1.0, 1.0, 1.0, 1.0],
                gamma=(1, 1),
                no_auto_bright=True,
                output_bps=16,
                user_flip=0,
                highlight_mode=rawpy.HighlightMode.Clip,
            )
            write(out + ".cfa", img)
        elif mode in ("dev", "half"):
            has_wb = any(v > 0 for v in r.camera_whitebalance)
            img = r.postprocess(
                half_size=mode == "half",
                use_camera_wb=has_wb,
                gamma=(1, 1),
                no_auto_bright=True,
                output_bps=16,
                user_flip=0,
                output_color=rawpy.ColorSpace.sRGB,
            )
            write(out + "." + mode, img)
        else:
            raise SystemExit(f"unknown mode {mode}")
    print(" ".join(f"{k}={v}" for k, v in facts.items()))


if __name__ == "__main__":
    main()
