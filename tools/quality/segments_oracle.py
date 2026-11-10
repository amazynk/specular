#!/usr/bin/env python3
"""Oracle for the segments' STL export (specular-render-volume `label_surface`, smoothing and
decimation): trimesh reads every STL and checks it is watertight and consistently wound, and
its volume is compared with the voxel count x voxel volume and with scikit-image's marching
cubes on the same mask.

    SPECULAR_ORACLE_DIR=<dir> cargo test -p specular-render-volume dump_for_oracle -- --ignored
    <venv>/bin/python -I tools/quality/segments_oracle.py <dir>
"""
import json
import pathlib
import sys

import numpy as np
import trimesh
from skimage import measure


def main(d: pathlib.Path) -> int:
    bad = 0
    for meta in sorted(d.glob("label*.json")):
        stem = meta.stem
        info = json.loads(meta.read_text())
        w, h, depth = info["dims"]
        vx = info["voxel"]
        mask = np.fromfile(d / f"{stem}.mask", dtype=np.uint8).reshape(depth, h, w)
        voxels = float(mask.sum()) * vx[0] * vx[1] * vx[2]
        padded = np.pad(mask, 1).astype(np.float32)
        verts, faces, _, _ = measure.marching_cubes(padded, 0.5, spacing=(vx[2], vx[1], vx[0]))
        mc = abs(trimesh.Trimesh(verts, faces).volume)
        print(f"{stem}: voxels {voxels:.1f} mm3, skimage marching cubes {mc:.1f} mm3")
        for kind in ("raw", "smooth", "decimated"):
            m = trimesh.load(d / f"{stem}-{kind}.stl", process=True)
            ok = m.is_watertight and m.is_winding_consistent and m.volume > 0
            rel_vox = (m.volume - voxels) / voxels
            rel_mc = (m.volume - mc) / mc
            print(
                f"  {kind:9} {len(m.faces):6} triangles  watertight={m.is_watertight} "
                f"winding={m.is_winding_consistent} volume {m.volume:.1f} "
                f"({rel_vox:+.2%} vs voxels, {rel_mc:+.2%} vs skimage)"
            )
            if not ok or abs(rel_vox) > 0.03:
                bad += 1
    print("mismatches:", bad)
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main(pathlib.Path(sys.argv[1])))
