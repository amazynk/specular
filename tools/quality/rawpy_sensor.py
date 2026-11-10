#!/usr/bin/env python3
"""Unpacked sensor values and levels of one RAW file from rawpy (LibRaw), for the
`raw_more` oracle test of specular-raw (`cargo test -p specular-raw --test raw_more -- --ignored`).

    $CARGO_TARGET_DIR/venv/bin/python -I tools/quality/rawpy_sensor.py FILE OUT_PREFIX

Writes OUT_PREFIX.raw: u32 LE height, width, channels, then the raw_image samples as u16 LE
(row-major, channels interleaved; X3F has three per pixel), and OUT_PREFIX.dev: u32 LE height,
width, then a half-size linear develop (camera white balance, sRGB primaries, gamma 1, no
auto-brightness, no flip) as u16 LE RGB. Prints one line of `key=value` facts on stdout.
Test-only: rawpy is an oracle, never a dependency of the viewer.
"""

import struct
import sys

import numpy as np
import rawpy


def main():
    path, out = sys.argv[1], sys.argv[2]
    with rawpy.imread(path) as r:
        raw = np.ascontiguousarray(r.raw_image, dtype="<u2")
        shape = raw.shape + (1,) if raw.ndim == 2 else raw.shape
        with open(out + ".raw", "wb") as f:
            f.write(struct.pack("<3I", *shape))
            f.write(raw.tobytes())
        s = r.sizes
        pat = r.raw_pattern
        facts = {
            "black": ",".join(str(int(v)) for v in r.black_level_per_channel),
            "white": int(r.white_level),
            "pattern": "" if pat is None else ",".join(str(int(v)) for v in pat.flatten()),
            "pattern_w": 0 if pat is None else pat.shape[1],
            "desc": r.color_desc.decode("latin1"),
            "crop": f"{s.left_margin},{s.top_margin},{s.width},{s.height}",
            "flip": s.flip,
            "wb": ",".join(f"{v:.5g}" for v in r.camera_whitebalance),
        }
        try:
            rgb = r.postprocess(
                half_size=True,
                use_camera_wb=True,
                gamma=(1, 1),
                no_auto_bright=True,
                output_bps=16,
                user_flip=0,
                output_color=rawpy.ColorSpace.sRGB,
            )
            rgb = np.ascontiguousarray(rgb, dtype="<u2")
            with open(out + ".dev", "wb") as f:
                f.write(struct.pack("<2I", rgb.shape[0], rgb.shape[1]))
                f.write(rgb.tobytes())
        except Exception as e:  # noqa: BLE001 (any LibRaw failure only skips the develop check)
            facts["develop_error"] = str(e).replace(" ", "_")
    print(" ".join(f"{k}={v}" for k, v in facts.items()))


if __name__ == "__main__":
    main()
