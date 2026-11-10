#!/usr/bin/env python3
"""orix oracle for the EBSD readers.

For every .ang / .ctf under SRC, orix (0.15) reads the map and this writes, under DEST with the
same relative path:

  <rel>.txt   points N / stride S / file-order sample of points kept / error text
  <rel>.f32   per kept point, 15 little-endian float32:
                x, y, laue (Laue group number 1..11 of the point's phase as orix knows it,
                0 without one, -1 not indexed), φ1, Φ, φ2 (radians, orix's Euler angles),
                h (3: the sample-Z crystal direction folded into the standard sector by
                orix), key (3: the TSL-style key of specular-ebsd, evaluated here with numpy
                from orix's h — see `key` below), orix (3: orix's own IPFColorKeyTSL colour)

The Rust corpus test compares orientations (rotation angle between orix's and ours), the
folded direction (angle) and the colour (per channel) at each point's pixel, and reports how
far orix's own key (MTEX's HSV construction, a different key) is from ours.

Usage: python -I gen/oracle.py SRC DEST [MAX_POINTS]
"""

import os
import sys

import numpy as np
from orix import io as oio
from orix.plot import IPFColorKeyTSL
from orix.quaternion import Orientation, Rotation
from orix.vector import Vector3d

LAUE = ["-1", "2/m", "mmm", "4/m", "4/mmm", "-3", "-3m", "6/m", "6/mmm", "m-3", "m-3m"]


def az(deg):
    a = np.radians(deg)
    return [np.cos(a), np.sin(a), 0.0]


R2, R3 = 1 / np.sqrt(2), 1 / np.sqrt(3)
Z = [0.0, 0.0, 1.0]
CORNERS = {
    "m-3m": [Z, [R2, 0, R2], [R3, R3, R3]],
    "6/mmm": [Z, az(0), az(30)],
    "6/m": [Z, az(0), az(60)],
    "4/mmm": [Z, az(0), az(45)],
    "4/m": [Z, az(0), az(90)],
    "mmm": [Z, az(0), az(90)],
    "-3m": [Z, az(-30), az(30)],
    "-3": [Z, az(0), az(120)],
}


def key(laue, h):
    """The specular-ebsd key: weight of each corner = angular distance from the opposite edge
    over the corner's own, square-rooted, scaled so the largest is one; grey without a key."""
    if laue not in CORNERS:
        return np.full(h.shape, 0.5)
    c = np.array(CORNERS[laue], dtype=float)
    out = np.zeros(h.shape)
    for i in range(3):
        n = np.cross(c[(i + 1) % 3], c[(i + 2) % 3])
        n /= np.linalg.norm(n)
        if n @ c[i] < 0:
            n = -n
        reach = np.arcsin(np.clip(n @ c[i], -1, 1))
        d = np.arcsin(np.clip(h @ n, -1, 1)) / reach
        out[:, i] = np.sqrt(np.clip(d, 0, 1))
    m = out.max(axis=1, keepdims=True)
    m[m <= 0] = 1
    return out / m


def one(path, dest, max_points):
    xmap = oio.load(path)
    n = xmap.size
    stride = max(1, -(-n // max_points))
    keep = np.arange(0, n, stride)
    x = np.asarray(xmap.x, dtype=float)[keep]
    y = np.asarray(xmap.y, dtype=float)[keep]
    pid = np.asarray(xmap.phase_id)[keep]
    rot = Rotation(xmap.rotations.data[keep])
    euler = rot.to_euler()
    rec = np.zeros((keep.size, 15), dtype=np.float32)
    rec[:, 0], rec[:, 1] = x, y
    rec[:, 3:6] = euler
    rec[:, 2] = -1
    for phase_id, phase in xmap.phases:
        sel = pid == phase_id
        if phase_id < 0 or not sel.any():
            continue
        pg = phase.point_group
        if pg is None:
            rec[sel, 2] = 0
            continue
        laue = pg.laue.name
        rec[sel, 2] = LAUE.index(laue) + 1 if laue in LAUE else 0
        v = rot[sel] * Vector3d.zvector()
        h = Vector3d(v).in_fundamental_sector(pg.laue).unit.data
        rec[sel, 6:9] = h
        rec[sel, 9:12] = key(laue, h)
        ori = Orientation(rot[sel].data, symmetry=pg)
        rec[sel, 12:15] = IPFColorKeyTSL(pg).orientation2color(ori)
    os.makedirs(os.path.dirname(dest), exist_ok=True)
    rec.astype("<f4").tofile(dest + ".f32")
    with open(dest + ".txt", "w") as f:
        f.write(f"points {n}\nstride {stride}\nkept {keep.size}\n")
        for phase_id, phase in xmap.phases:
            pg = phase.point_group.name if phase.point_group is not None else "-"
            f.write(f"phase {phase_id} {pg} {phase.name}\n")


def main():
    src, dest = sys.argv[1], sys.argv[2]
    max_points = int(sys.argv[3]) if len(sys.argv) > 3 else 250_000
    for root, dirs, files in os.walk(src):
        dirs[:] = sorted(d for d in dirs if d != "expected")
        for name in sorted(files):
            if not name.lower().endswith((".ang", ".ctf")):
                continue
            path = os.path.join(root, name)
            rel = os.path.relpath(path, src)
            out = os.path.join(dest, rel)
            try:
                one(path, out, max_points)
                print(f"  {rel}")
            except Exception as e:  # noqa: BLE001 - recorded for the test to skip
                os.makedirs(os.path.dirname(out), exist_ok=True)
                with open(out + ".txt", "w") as f:
                    f.write(f"error {type(e).__name__}: {e}\n")
                print(f"  {rel}: orix fails: {e}")


if __name__ == "__main__":
    main()
