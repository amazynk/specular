#!/usr/bin/env python3
"""Linear camera-space data of one RAW file from rawpy (LibRaw), for the `raw_cameras` oracle
test of specular-raw (`cargo test -p specular-raw --test raw_cameras -- --ignored`).

    $CARGO_TARGET_DIR/venv/bin/python -I tools/quality/rawpy_linear.py FILE OUT_PREFIX

Nothing here is a look: no white balance, no colour matrix, gamma 1, no auto-brightness, no
flip, LibRaw's maximum adjustment off, highlights clipped. Writes

  OUT_PREFIX.raw   u32 LE height, width, channels, then raw_image as u16 LE;
  OUT_PREFIX.half  u32 LE height, width, then the half-size develop (one pixel per 2x2 sensor
                   block, no interpolation) as f32 LE RGB, camera space, white = 65535;
  OUT_PREFIX.full  the same for the full-size demosaic (LibRaw's default: AHD for Bayer,
                   its 3-pass for X-Trans), reduced to 2x2 block means.

Prints one line of `key=value` facts (levels, the largest visible sample and its count,
as-shot white balance, LibRaw's rgb_cam and XYZ-to-camera matrices). Test-only: rawpy is
an oracle, never a dependency of the viewer.
"""

import struct
import sys

import numpy as np
import rawpy


# Linear sRGB (D65) -> XYZ, as LibRaw's xyz_rgb.
XYZ_RGB = np.array(
    [[0.412453, 0.357580, 0.180423], [0.212671, 0.715160, 0.072169], [0.019334, 0.119193, 0.950227]]
)


def blocks(a, k):
    h, w = a.shape[0] // k * k, a.shape[1] // k * k
    a = a[:h, :w].astype(np.float64)
    return a.reshape(h // k, k, w // k, k, a.shape[2]).mean(axis=(1, 3))


def write(path, a):
    a = np.ascontiguousarray(a, dtype="<f4")
    with open(path, "wb") as f:
        f.write(struct.pack("<2I", a.shape[0], a.shape[1]))
        f.write(a.tobytes())


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
        num = lambda v: ",".join(f"{float(x):.6g}" for x in np.asarray(v).flatten())  # noqa: E731
        facts = {
            "black": num(r.black_level_per_channel),
            "white": int(r.white_level),
            "pattern": "" if pat is None else num(pat),
            "pattern_w": 0 if pat is None else pat.shape[1],
            "desc": r.color_desc.decode("latin1"),
            "crop": f"{s.left_margin},{s.top_margin},{s.width},{s.height}",
            "wb": num(r.camera_whitebalance),
            "xyz_cam": num(np.asarray(r.rgb_xyz_matrix)[:3, :3]),
        }
        # The largest visible sample and how many samples have it: where the data clips
        # (LibRaw's default develop lowers its maximum to it, adjust_maximum_thr 0.75).
        vis = r.raw_image_visible
        facts["data_max"] = int(vis.max())
        facts["data_max_n"] = int(np.count_nonzero(vis == vis.max()))
        common = dict(
            use_camera_wb=False,
            use_auto_wb=False,
            user_wb=[1.0, 1.0, 1.0, 1.0],
            gamma=(1, 1),
            no_auto_bright=True,
            output_bps=16,
            user_flip=0,
            output_color=rawpy.ColorSpace.raw,
            adjust_maximum_thr=0.0,
            highlight_mode=rawpy.HighlightMode.Clip,
        )
        # rawpy does not expose LibRaw's rgb_cam: derive it from rgb_xyz_matrix the way
        # LibRaw's cam_xyz_coeff does (camera <- sRGB rows normalised to 1, pseudo-inverse).
        xyz_cam = np.asarray(r.rgb_xyz_matrix)[:3, :3].astype(np.float64)
        if np.abs(xyz_cam).sum() > 0:
            cam_rgb = xyz_cam @ XYZ_RGB
            cam_rgb /= cam_rgb.sum(axis=1, keepdims=True)
            facts["rgb_cam"] = num(np.linalg.pinv(cam_rgb))
        for name, half, k in (("half", True, 1), ("full", False, 2)):
            try:
                rgb = r.postprocess(half_size=half, **common)
                write(f"{out}.{name}", blocks(rgb, k))
            except Exception as e:  # noqa: BLE001 (a LibRaw failure only skips this check)
                facts[f"{name}_error"] = str(e).replace(" ", "_")
    print(" ".join(f"{k}={v}" for k, v in facts.items()))


if __name__ == "__main__":
    main()
