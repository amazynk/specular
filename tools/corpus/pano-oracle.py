#!/usr/bin/env python3
"""Merge oracle: OpenCV's exposure fusion, response recovery and panorama
stitcher against ours. Outputs only: run in a venv under the target dir with
opencv-python-headless and numpy (Apache-2.0 / BSD). Never shipped, never committed
outputs.

  python3 -m venv $CARGO_TARGET_DIR/venv-pano
  PIP_CACHE_DIR=$CARGO_TARGET_DIR/pip-cache $CARGO_TARGET_DIR/venv-pano/bin/pip install \
      opencv-python-headless numpy

Images in and out are raw files: `.rgb8` is u32 width, u32 height (little-endian), then
RGB8 rows; `.f32` is u32 width, u32 height, then little-endian float32 values.

  pano-oracle.py mertens OUT.f32 F1.rgb8 F2.rgb8 ...
      cv2.createMergeMertens() (exponents 1, 1, 1) of the frames: RGB floats, W × H × 3.
  pano-oracle.py mertens-paper OUT.f32 F1.rgb8 F2.rgb8 ...
      the paper's method from cv2's primitives (grey, Laplacian, pyrDown, pyrUp), as above.
  pano-oracle.py debevec OUT.f32 T1,T2,... F1.rgb8 ...
      cv2.createCalibrateDebevec() (70 samples, lambda 10, grid): the response, 256 × 3
      RGB floats (header 256, 3).
  pano-oracle.py robertson OUT.f32 T1,T2,... F1.rgb8 ...
      cv2.createCalibrateRobertson() (30 iterations, threshold 0.01): as debevec.
  pano-oracle.py stitch OUT.json IMG1 IMG2 ...
      cv2.Stitcher (PANORAMA mode) on image files (any format cv2 reads, or .rgb8): each
      camera's focal length, principal point and rotation R (3 × 3), and the panorama's
      size. Status other than OK is written as {"status": N}.
(then: SPECULAR_PANO_ORACLE_PY=$CARGO_TARGET_DIR/venv-pano/bin/python cargo test
       -p specular-develop --lib merge:: -- --ignored oracle --nocapture)
"""

import json
import struct
import sys

import cv2
import numpy as np


def read_rgb8(path):
    with open(path, "rb") as f:
        w, h = struct.unpack("<II", f.read(8))
        return np.frombuffer(f.read(), np.uint8).reshape(h, w, 3)


def write_f32(path, w, h, values):
    with open(path, "wb") as f:
        f.write(struct.pack("<II", w, h))
        f.write(np.ascontiguousarray(values, dtype="<f4").tobytes())


def load_bgr(path):
    if path.endswith(".rgb8"):
        return np.ascontiguousarray(read_rgb8(path)[..., ::-1])
    img = cv2.imread(path, cv2.IMREAD_COLOR)
    if img is None:
        raise SystemExit(f"cannot read {path}")
    return img


def mertens(out, files):
    imgs = [load_bgr(p) for p in files]
    res = cv2.createMergeMertens(1.0, 1.0, 1.0).process(imgs)
    h, w, _ = res.shape
    write_f32(out, w, h, res[..., ::-1])


def mertens_paper(out, files):
    """The paper's method built from cv2's own primitives (cvtColor, Laplacian, pyrDown,
    pyrUp): it separates differences of the pyramid kernels from MergeMertens' internals."""
    fl = [load_bgr(p).astype(np.float32) / 255 for p in files]
    h, w, _ = fl[0].shape
    ws = []
    for f in fl:
        g = cv2.cvtColor(f, cv2.COLOR_BGR2GRAY)
        con = np.abs(cv2.Laplacian(g, cv2.CV_32F))
        mu = f.mean(-1, keepdims=True)
        sat = np.sqrt(((f - mu) ** 2).sum(-1))
        we = np.prod(np.exp(-((f - 0.5) ** 2) / 0.08), -1)
        ws.append(con * sat * we + 1e-12)
    s = sum(ws)
    ws = [x / s for x in ws]
    lv = int(np.log(min(h, w)) / np.log(2))
    res = None
    for f, wt in zip(fl, ws):
        ip, wp = [f], [wt]
        for _ in range(lv):
            ip.append(cv2.pyrDown(ip[-1]))
            wp.append(cv2.pyrDown(wp[-1]))
        for l in range(lv):
            ip[l] = ip[l] - cv2.pyrUp(ip[l + 1], dstsize=(ip[l].shape[1], ip[l].shape[0]))
        prod = [i * x[..., None] for i, x in zip(ip, wp)]
        res = prod if res is None else [a + b for a, b in zip(res, prod)]
    for l in range(lv, 0, -1):
        res[l - 1] = res[l - 1] + cv2.pyrUp(res[l], dstsize=(res[l - 1].shape[1], res[l - 1].shape[0]))
    write_f32(out, w, h, res[0][..., ::-1])


def calibrate(out, times, files, which):
    imgs = [load_bgr(p) for p in files]
    t = np.array([float(x) for x in times.split(",")], np.float32)
    if which == "debevec":
        cal = cv2.createCalibrateDebevec(70, 10.0, False)
    else:
        cal = cv2.createCalibrateRobertson(30, 0.01)
    resp = cal.process(imgs, t).reshape(256, 3)
    write_f32(out, 256, 3, resp[:, ::-1])


def stitch(out, files):
    imgs = [load_bgr(p) for p in files]
    s = cv2.Stitcher.create(cv2.Stitcher_PANORAMA)
    status = s.estimateTransform(imgs)
    if status != cv2.Stitcher_OK:
        with open(out, "w") as f:
            json.dump({"status": int(status)}, f)
        return
    cams = s.cameras()
    status, pano = s.composePanorama()
    result = {
        "status": int(status),
        "indices": [int(i) for i in s.component()],
        "cameras": [
            {
                "focal": float(c.focal),
                "aspect": float(c.aspect),
                "ppx": float(c.ppx),
                "ppy": float(c.ppy),
                "R": np.asarray(c.R, np.float64).tolist(),
            }
            for c in cams
        ],
        "size": [int(pano.shape[1]), int(pano.shape[0])] if pano is not None else None,
    }
    with open(out, "w") as f:
        json.dump(result, f)


def main():
    if len(sys.argv) < 3:
        raise SystemExit(__doc__)
    cmd, out = sys.argv[1], sys.argv[2]
    if cmd == "mertens":
        mertens(out, sys.argv[3:])
    elif cmd == "mertens-paper":
        mertens_paper(out, sys.argv[3:])
    elif cmd in ("debevec", "robertson"):
        calibrate(out, sys.argv[3], sys.argv[4:], cmd)
    elif cmd == "stitch":
        stitch(out, sys.argv[3:])
    else:
        raise SystemExit(__doc__)


if __name__ == "__main__":
    main()
