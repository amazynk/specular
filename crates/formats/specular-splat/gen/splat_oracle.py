#!/usr/bin/env python3
"""Oracle facts and reference renderings for the splat corpus (tools/corpus/models/splats).

Run with `python -I` from a venv with numpy and plyfile (see tools/corpus/fetch-models.sh):

    python -I crates/formats/specular-splat/gen/splat_oracle.py tools/corpus/models

Writes OUT/oracle/splats.tsv: per file its splat count, harmonic degree, the bounds of the
centres (file frame) and the first three centres; for the PLY files also a reference
rendering (OUT/oracle/<name>.rgba, straight RGBA8, 192×144) of an orthographic view whose
camera (viewer frame: z up) is written beside it. The renderer is written from the 3DGS
paper's equations (EWA splatting with a 0.3 px² low-pass, harmonics to degree 3, sorted
front-to-back blending), independently of the Rust code; plyfile reads the files.
"""
import gzip
import math
import os
import struct
import sys

import numpy as np
from plyfile import PlyData

C0 = 0.28209479177387814
C1 = 0.4886025119029199
C2 = [1.0925484305920792, -1.0925484305920792, 0.31539156525252005, -1.0925484305920792,
      0.5462742152960396]
C3 = [-0.5900435899266435, 2.890611442640554, -0.4570457994644658, 0.3731763325901154,
      -0.4570457994644658, 1.445305721320277, -0.5900435899266435]
W, H = 192, 144


def sh_color(dc, rest, d):
    """dc (n,3), rest (n,K,3) channel-last, d (3,) unit direction in the file frame."""
    x, y, z = d
    basis = [-C1 * y, C1 * z, -C1 * x,
             C2[0] * x * y, C2[1] * y * z, C2[2] * (2 * z * z - x * x - y * y), C2[3] * x * z,
             C2[4] * (x * x - y * y),
             C3[0] * y * (3 * x * x - y * y), C3[1] * x * y * z,
             C3[2] * y * (4 * z * z - x * x - y * y),
             C3[3] * z * (2 * z * z - 3 * x * x - 3 * y * y),
             C3[4] * x * (4 * z * z - x * x - y * y), C3[5] * z * (x * x - y * y),
             C3[6] * x * (x * x - 3 * y * y)]
    c = 0.5 + C0 * dc
    k = rest.shape[1]
    for j in range(k):
        c = c + basis[j] * rest[:, j, :]
    return np.maximum(c, 0.0)


def quat_to_mat(q):
    q = q / np.linalg.norm(q, axis=1, keepdims=True)
    w, x, y, z = q[:, 0], q[:, 1], q[:, 2], q[:, 3]
    r = np.empty((len(q), 3, 3))
    r[:, 0, 0] = 1 - 2 * (y * y + z * z)
    r[:, 0, 1] = 2 * (x * y - w * z)
    r[:, 0, 2] = 2 * (x * z + w * y)
    r[:, 1, 0] = 2 * (x * y + w * z)
    r[:, 1, 1] = 1 - 2 * (x * x + z * z)
    r[:, 1, 2] = 2 * (y * z - w * x)
    r[:, 2, 0] = 2 * (x * z - w * y)
    r[:, 2, 1] = 2 * (y * z + w * x)
    r[:, 2, 2] = 1 - 2 * (x * x + y * y)
    return r


def render(pos, scale, rot, dc, rest, opacity, cam):
    """Orthographic: cam = (right, down, forward, target, focal) in the file frame."""
    right, down, fwd, target, focal = cam
    R = quat_to_mat(rot)
    M = R * scale[:, None, :]
    cov = M @ np.transpose(M, (0, 2, 1))
    Wv = np.stack([right, down, fwd])
    cv = Wv @ cov @ Wv.T
    a = cv[:, 0, 0] * focal * focal + 0.3
    b = cv[:, 0, 1] * focal * focal
    c = cv[:, 1, 1] * focal * focal + 0.3
    det = a * c - b * b
    d = pos - target
    px = W / 2 + focal * (d @ right)
    py = H / 2 + focal * (d @ down)
    depth = d @ fwd
    col = sh_color(dc, rest, fwd)
    mid = 0.5 * (a + c)
    lam = mid + np.sqrt(np.maximum(mid * mid - det, 0.1))
    rad = np.ceil(3 * np.sqrt(lam))
    order = np.argsort(depth, kind="stable")
    T = np.ones((H, W))
    C = np.zeros((H, W, 3))
    ys, xs = np.mgrid[0:H, 0:W]
    xs = xs + 0.5
    ys = ys + 0.5
    for i in order:
        if not (det[i] > 0):
            continue
        r = rad[i]
        x0, x1 = int(max(px[i] - r, 0)), int(min(px[i] + r + 1, W))
        y0, y1 = int(max(py[i] - r, 0)), int(min(py[i] + r + 1, H))
        if x0 >= x1 or y0 >= y1:
            continue
        dx = xs[y0:y1, x0:x1] - px[i]
        dy = ys[y0:y1, x0:x1] - py[i]
        ia, ib, ic = c[i] / det[i], -b[i] / det[i], a[i] / det[i]
        power = -0.5 * (ia * dx * dx + ic * dy * dy) - ib * dx * dy
        alpha = np.minimum(0.99, opacity[i] * np.exp(np.minimum(power, 0)))
        alpha[(power > 0) | (alpha < 1 / 255)] = 0
        t = T[y0:y1, x0:x1]
        C[y0:y1, x0:x1] += (alpha * t)[..., None] * col[i]
        T[y0:y1, x0:x1] = t * (1 - alpha)
    A = 1 - T
    out = np.zeros((H, W, 4), np.uint8)
    rgb = np.where(A[..., None] > 0, C / np.maximum(A[..., None], 1e-12), 0)
    out[..., :3] = np.round(np.clip(rgb, 0, 1) * 255)
    out[..., 3] = np.round(np.clip(A, 0, 1) * 255)
    return out


# The viewer's frame from COLMAP's (y down): (x, y, z) -> (x, z, -y).
TO_CANON = np.array([[1, 0, 0], [0, 0, 1], [0, -1, 0]], float)


def iso_camera(pos):
    """The orthographic iso view of the 0.5-99.5 percentile box, in the viewer's frame."""
    p = pos @ TO_CANON.T
    lo = np.percentile(p, 0.5, axis=0)
    hi = np.percentile(p, 99.5, axis=0)
    s2, s3, s6 = 1 / math.sqrt(2), 1 / math.sqrt(3), 1 / math.sqrt(6)
    right = np.array([s2, s2, 0])
    down = np.array([s6, -s6, -2 * s6])
    fwd = np.array([-s3, s3, -s3])
    centre = (lo + hi) / 2
    corners = np.array([[lo[0] if i & 1 == 0 else hi[0], lo[1] if i & 2 == 0 else hi[1],
                         lo[2] if i & 4 == 0 else hi[2]] for i in range(8)]) - centre
    u, v = corners @ right, corners @ down
    focal = 0.92 * min(W / (u.max() - u.min()), H / (v.max() - v.min()))
    target = centre + right * (u.max() + u.min()) / 2 + down * (v.max() + v.min()) / 2
    return right, down, fwd, target, focal


def fmt(v):
    return ",".join(f"{x:.6g}" for x in v)


def main(root):
    sd = os.path.join(root, "splats")
    od = os.path.join(root, "oracle")
    os.makedirs(od, exist_ok=True)
    rows = []
    for name in sorted(os.listdir(sd)):
        path = os.path.join(sd, name)
        if name.endswith(".ply"):
            ply = PlyData.read(path)
            v = ply["vertex"]
            props = [p.name for p in v.properties]
            if "opacity" not in props:
                rows.append((name, "kind", "not-splats"))
                continue
            pos = np.stack([v["x"], v["y"], v["z"]], 1).astype(float)
            nrest = len([p for p in props if p.startswith("f_rest_")])
            per = nrest // 3
            deg = 3 if per >= 15 else 2 if per >= 8 else 1 if per >= 3 else 0
            k = [0, 3, 8, 15][deg]
            rows.append((name, "count", str(len(pos))))
            rows.append((name, "degree", str(deg)))
            scale = np.exp(np.stack([v[f"scale_{i}"] for i in range(3)], 1).astype(float))
            rot = np.stack([v[f"rot_{i}"] for i in range(4)], 1).astype(float)
            dc = np.stack([v[f"f_dc_{i}"] for i in range(3)], 1).astype(float)
            rest = np.zeros((len(pos), k, 3))
            for ch in range(3):
                for j in range(k):
                    rest[:, j, ch] = v[f"f_rest_{ch * per + j}"]
            op = 1 / (1 + np.exp(-np.asarray(v["opacity"], float)))
            ok = np.isfinite(pos).all(1) & np.isfinite(scale).all(1) & np.isfinite(rot).all(1)
            right, down, fwd, target, focal = iso_camera(pos[ok])
            # The camera in the file frame for the renderer (TO_CANON is orthonormal).
            back = TO_CANON.T
            cam = (back @ right, back @ down, back @ fwd, back @ target, focal)
            img = render(pos[ok], scale[ok], rot[ok], dc[ok], rest[ok], op[ok], cam)
            stem = os.path.splitext(name)[0]
            img.tofile(os.path.join(od, stem + ".rgba"))
            rows.append((name, "camera", ";".join(
                [fmt(right), fmt(down), fmt(fwd), fmt(target), f"{focal:.9g}", f"{W}x{H}"])))
        elif name.endswith(".splat"):
            b = np.fromfile(path, np.uint8)
            n = len(b) // 32
            pos = b[: n * 32].reshape(n, 32)[:, :12].copy().view("<f4").reshape(n, 3)
            pos = pos.astype(float)
            rows.append((name, "count", str(n)))
            rows.append((name, "degree", "0"))
        elif name.endswith(".spz"):
            raw = gzip.open(path).read()
            magic, ver, n, deg, frac = struct.unpack_from("<IIIBB", raw)
            rows.append((name, "count", str(n)))
            rows.append((name, "degree", str(deg)))
            p = np.frombuffer(raw[16: 16 + n * 9], np.uint8).reshape(n, 3, 3).astype(np.int64)
            v24 = p[:, :, 0] | (p[:, :, 1] << 8) | (p[:, :, 2] << 16)
            v24 = np.where(v24 & 0x800000, v24 - (1 << 24), v24)
            pos = v24 / float(1 << frac)
        else:
            continue
        ok = np.isfinite(pos).all(1)
        rows.append((name, "min", fmt(pos[ok].min(0))))
        rows.append((name, "max", fmt(pos[ok].max(0))))
        rows.append((name, "first", ";".join(fmt(p) for p in pos[:3])))
    with open(os.path.join(od, "splats.tsv"), "w") as f:
        for r in rows:
            f.write("\t".join(r) + "\n")
    print(f"splats: {len(rows)} facts")


if __name__ == "__main__":
    main(sys.argv[1])
