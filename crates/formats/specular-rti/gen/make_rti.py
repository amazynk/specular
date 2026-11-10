#!/usr/bin/env python3
"""Writes the synthetic RTI fixtures of specular-rti's tests (tests/data), each under 200 KB.

    python3 -I make_rti.py DIR

Every fixture is the same small embossed surface (a height field of bumps, a ridge and a
ring, with a colour pattern) lit by a Lambertian model at sampled lights, fitted to each
basis by least squares and quantised the way the format says:

  ptm_lrgb.ptm        PTM_1.2 PTM_FORMAT_LRGB, uncompressed
  ptm_rgb.ptm         PTM_1.2 PTM_FORMAT_RGB, uncompressed
  ptm11_lrgb.ptm      PTM_1.1 LRGB (coefficients and colour interleaved per pixel)
  ptm_lut.ptm         PTM_FORMAT_PTM_LUT (luminance coefficients in a table, colour after)
  ptm_clut.ptm        PTM_FORMAT_PTM_C_LUT (coefficients and colour in a 300-entry table)
  ptm_jpeg_lrgb.ptm   PTM_FORMAT_JPEG_LRGB: intra, predicted and inverted-reference planes,
                      a shuffled decoding order and side information
  ptm_jpeg_rgb.ptm    PTM_FORMAT_JPEG_RGB, likewise with eighteen planes
  hsh2.rti hsh3.rti hsh4.rti   CHI .rti HSH with 4, 9 and 16 terms
  legacy.hsh          the old binary .hsh header (order 3)
  relight_ptm_lrgb/   Relight folders (info.json and plane_N.jpg): PTM in LRGB,
  relight_hsh_rgb/      HSH (9 terms) in RGB,
  relight_sh_rgb/       SH (4 terms) in RGB,
  relight_rbf_mrgb/     RBF over 20 lights in MRGB with the basis in the JSON,
  relight_bln_mycc/     bilinear 6×6 in MYCC with materials.bin

Only numpy and Pillow (for the JPEG encoder) are needed; the RNG seed is fixed.
"""

import io
import json
import math
import os
import sys

import numpy as np
from PIL import Image

W, H = 48, 32
RNG = np.random.default_rng(20261011)


def surface(w=W, h=H):
    """Normals (h, w, 3) with y up and colours (h, w, 3) in 0..1."""
    ys, xs = np.mgrid[0:h, 0:w].astype(np.float64)
    z = np.zeros((h, w))
    for cx, cy, r, a in [(12, 10, 5, 4.0), (34, 20, 7, -3.0), (24, 26, 4, 2.5)]:
        z += a * np.exp(-((xs - cx) ** 2 + (ys - cy) ** 2) / (2 * r * r))
    z += 1.5 * np.exp(-((xs - ys * 1.2 - 4) ** 2) / 6.0)          # ridge
    rr = np.hypot(xs - 38, ys - 8)
    z += 1.2 * np.exp(-((rr - 4.5) ** 2) / 1.5)                     # ring
    gx = np.gradient(z, axis=1)
    gy = -np.gradient(z, axis=0)                                     # rows run down
    n = np.stack([-gx, -gy, np.ones_like(z)], axis=-1)
    n /= np.linalg.norm(n, axis=-1, keepdims=True)
    col = np.stack([0.55 + 0.35 * np.sin(xs / 7.0),
                    0.5 + 0.3 * np.cos(ys / 5.0),
                    0.45 + 0.3 * np.sin((xs + ys) / 9.0)], axis=-1)
    return n, np.clip(col, 0.05, 0.95)


def lights(count=40):
    """Sampled light directions on the upper hemisphere (count, 3)."""
    out = []
    for i in range(count):
        el = math.radians(15 + 75 * ((i * 0.618034) % 1.0))
        az = 2 * math.pi * i / count * 3.0
        out.append([math.cos(el) * math.cos(az), math.cos(el) * math.sin(az), math.sin(el)])
    return np.array(out)


def lambert(n, col, ls):
    """Observations (h, w, L, 3) in 0..1."""
    d = np.clip(np.einsum("hwk,lk->hwl", n, ls), 0, None) * 0.9 + 0.05
    return d[..., None] * col[:, :, None, :]


def ptm_terms(ls, relight=False):
    x, y = ls[:, 0], ls[:, 1]
    if relight:
        return np.stack([np.ones_like(x), x, y, x * x, x * y, y * y], axis=1)
    return np.stack([x * x, y * y, x * y, x, y, np.ones_like(x)], axis=1)


def hsh_terms(ls, n):
    phi = np.arctan2(ls[:, 1], ls[:, 0])
    phi = np.where(phi < 0, phi + 2 * np.pi, phi)
    theta = np.minimum(np.arccos(np.clip(ls[:, 2], -1, 1)), np.pi / 2 - 0.04)
    c = np.cos(theta)
    c2 = c * c
    s = np.sqrt(np.maximum(c - c2, 0))
    pi = np.pi
    t = [np.full_like(c, 1 / math.sqrt(2 * pi)),
         math.sqrt(6 / pi) * np.cos(phi) * s,
         math.sqrt(3 / (2 * pi)) * (2 * c - 1),
         math.sqrt(6 / pi) * np.sin(phi) * s,
         math.sqrt(30 / pi) * np.cos(2 * phi) * (c2 - c),
         math.sqrt(30 / pi) * np.cos(phi) * (2 * c - 1) * s,
         math.sqrt(5 / (2 * pi)) * (1 - 6 * c + 6 * c2),
         math.sqrt(30 / pi) * np.sin(phi) * (2 * c - 1) * s,
         math.sqrt(30 / pi) * np.sin(2 * phi) * (c2 - c),
         2 * math.sqrt(35 / pi) * np.cos(3 * phi) * s ** 3,
         math.sqrt(210 / pi) * np.cos(2 * phi) * (2 * c - 1) * (c2 - c),
         2 * math.sqrt(21 / pi) * np.cos(phi) * s * (1 - 5 * c + 5 * c2),
         math.sqrt(7 / (2 * pi)) * (-1 + 12 * c - 30 * c2 + 20 * c2 * c),
         2 * math.sqrt(21 / pi) * np.sin(phi) * s * (1 - 5 * c + 5 * c2),
         math.sqrt(210 / pi) * np.sin(2 * phi) * (2 * c - 1) * (c2 - c),
         2 * math.sqrt(35 / pi) * np.sin(3 * phi) * s ** 3]
    return np.stack(t[:n], axis=1)


def sh_terms(ls, n):
    x, y, z = ls[:, 0], ls[:, 1], ls[:, 2]
    a = 0.5 * math.sqrt(3 / math.pi)
    b = 0.5 * math.sqrt(15 / math.pi)
    t = [np.full_like(x, 0.5 / math.sqrt(math.pi)), a * x, a * z, a * y, b * x * y, b * x * z,
         0.5 * math.sqrt(5 / math.pi) * (3 * z * z - 1), b * y * z, 0.5 * b * (y * y - x * x)]
    return np.stack(t[:n], axis=1)


def fit(obs, basis):
    """Least-squares coefficients (h, w, terms) of obs (h, w, L) on basis (L, terms)."""
    sol, *_ = np.linalg.lstsq(basis, obs.reshape(-1, obs.shape[2]).T, rcond=None)
    return sol.T.reshape(obs.shape[0], obs.shape[1], basis.shape[1])


def ptm_header(version, fmt, scale, bias, w=W, h=H):
    return (f"{version}\n{fmt}\n{w}\n{h}\n" + " ".join(f"{s:.6f}" for s in scale) + " \n"
            + " ".join(str(b) for b in bias) + " \n").encode()


def ptm_quant(coefs):
    """Per-coefficient float scale and integer bias, and the raw bytes."""
    scale, bias, raw = [], [], np.zeros(coefs.shape, np.uint8)
    for k in range(6):
        lo, hi = coefs[..., k].min(), coefs[..., k].max()
        s = max((hi - lo) / 255.0, 1e-3)
        s = float(f"{s:.6f}")
        b = int(round(-lo / s))
        raw[..., k] = np.clip(np.round(coefs[..., k] / s + b), 0, 255)
        scale.append(s)
        bias.append(b)
    return scale, bias, raw


def ptm_fixtures(out):
    n, col = surface()
    ls = lights()
    obs = lambert(n, col, ls)
    lum = obs.mean(axis=-1) / np.maximum(col.mean(axis=-1), 1e-6)[..., None]
    lcoef = fit(lum * 255.0, ptm_terms(ls))
    rgb = np.clip(np.round(col * 255), 0, 255).astype(np.uint8)
    scale, bias, raw = ptm_quant(lcoef)
    flip = lambda a: a[::-1]  # noqa: E731 - PTM rows are bottom-up
    with open(os.path.join(out, "ptm_lrgb.ptm"), "wb") as f:
        f.write(ptm_header("PTM_1.2", "PTM_FORMAT_LRGB", scale, bias))
        f.write(flip(raw).tobytes())
        f.write(flip(rgb).tobytes())
    with open(os.path.join(out, "ptm11_lrgb.ptm"), "wb") as f:
        f.write(ptm_header("PTM_1.1", "PTM_FORMAT_LRGB", scale, bias))
        f.write(np.concatenate([flip(raw), flip(rgb)], axis=-1).tobytes())

    # Lookup tables: quantise the luminance coefficients to a few levels so entries repeat.
    q = (raw // 32) * 32 + 16
    keys, idx = np.unique(q.reshape(-1, 6), axis=0, return_inverse=True)
    idx = idx.reshape(H, W)
    with open(os.path.join(out, "ptm_lut.ptm"), "wb") as f:
        f.write(ptm_header("PTM_1.2", "PTM_FORMAT_PTM_LUT", scale, bias))
        f.write(f"nentries {len(keys)}\n".encode())
        f.write(keys.astype(np.uint8).tobytes())
        f.write(flip(idx).astype(np.uint16 if len(keys) > 256 else np.uint8).tobytes())
        f.write(flip(rgb).tobytes())
    full = np.concatenate([q, (rgb // 64) * 64 + 32], axis=-1).reshape(-1, 9)
    keys, idx = np.unique(full, axis=0, return_inverse=True)
    while len(keys) <= 256:   # make the table need two-byte indices
        keys = np.concatenate([keys, RNG.integers(0, 256, (300 - len(keys), 9))])
    idx = idx.reshape(H, W)
    with open(os.path.join(out, "ptm_clut.ptm"), "wb") as f:
        f.write(ptm_header("PTM_1.2", "PTM_FORMAT_PTM_C_LUT", scale, bias))
        f.write(f"{len(keys)}\n".encode())
        f.write(keys.astype(np.uint8).tobytes())
        f.write(flip(idx).astype("<u2").tobytes())

    # Per-channel coefficients.
    ccoef = np.stack([fit(obs[..., c] * 255.0, ptm_terms(ls)) for c in range(3)], axis=2)
    lo = ccoef.min(axis=(0, 1, 2))
    hi = ccoef.max(axis=(0, 1, 2))
    rscale = [float(f"{max((h - l) / 255.0, 1e-3):.6f}") for l, h in zip(lo, hi)]
    rbias = [int(round(-l / s)) for l, s in zip(lo, rscale)]
    rraw = np.clip(np.round(ccoef / np.array(rscale) + np.array(rbias)), 0, 255).astype(np.uint8)
    with open(os.path.join(out, "ptm_rgb.ptm"), "wb") as f:
        f.write(ptm_header("PTM_1.2", "PTM_FORMAT_RGB", rscale, rbias))
        for c in range(3):
            f.write(flip(rraw[:, :, c, :]).tobytes())

    # JPEG: planes in the spec's order (top-down images), each predicted or intra.
    lplanes = [raw[..., k] for k in range(6)] + [rgb[..., c] for c in range(3)]
    #            a0  a1  a2  a3  a4  a5   r  g  b
    refs = [-1, 0, 1, -1, 3, 4, -1, 6, 7]
    xform = [0, 0, 1, 0, 0, 1, 0, 0, 1]
    order = [3, 4, 5, 6, 7, 8, 0, 1, 2]
    jpeg_ptm(os.path.join(out, "ptm_jpeg_lrgb.ptm"), "PTM_FORMAT_JPEG_LRGB", scale, bias,
             lplanes, refs, xform, order, 90)
    rplanes = [rraw[:, :, c, k] for c in range(3) for k in range(6)]
    refs = [-1, 0, 1, 2, -1, 4] + [0, 1, 2, 3, 4, 5] + [6, 7, 8, 9, 10, 11]
    xform = [0, 0, 1, 0, 0, 0] + [0] * 6 + [0, 1, 0, 0, 0, 1]
    order = list(range(18))
    jpeg_ptm(os.path.join(out, "ptm_jpeg_rgb.ptm"), "PTM_FORMAT_JPEG_RGB", rscale, rbias,
             rplanes, refs, xform, order, 75)


def jpeg_gray(a, quality):
    buf = io.BytesIO()
    Image.fromarray(a.astype(np.uint8), "L").save(buf, "JPEG", quality=quality)
    return buf.getvalue()


def unjpeg(b):
    return np.asarray(Image.open(io.BytesIO(b)).convert("L"), dtype=np.int32)


def jpeg_ptm(path, fmt, scale, bias, planes, refs, xform, order, quality):
    """Closed-loop prediction against the decoded references, side information for every
    pixel off by more than 24 (and three forced records per predicted plane)."""
    n = len(planes)
    rec = [None] * n
    streams, sides = [None] * n, [None] * n
    for rank in range(n):
        i = order.index(rank)
        cur = planes[i].astype(np.int32)
        if refs[i] < 0:
            base = None
            resid = cur
        else:
            base = rec[refs[i]]
            if xform[i] & 1:
                base = 255 - base
            resid = (cur - base + 128) % 256
        js = jpeg_gray(resid, quality)
        dec = unjpeg(js)
        got = dec if base is None else (base + dec - 128) % 256
        bad = np.abs(got - cur) > 24
        if base is not None:
            bad.flat[[5, 77, W * H - 1]] = True
        side = bytearray()
        for p in np.flatnonzero(bad):
            side += int(p).to_bytes(4, "big") + bytes([int(cur.flat[p])])
            got.flat[p] = cur.flat[p]
        rec[i] = got
        streams[i], sides[i] = js, bytes(side)
    hdr = ptm_header("PTM_1.2", fmt, scale, bias)
    motion = [0] * (2 * n)
    hdr += (f"{quality}\n" + " ".join(map(str, xform)) + "\n" + " ".join(map(str, motion))
            + "\n" + " ".join(map(str, order)) + "\n" + " ".join(map(str, refs)) + "\n"
            + " ".join(str(len(s)) for s in streams) + "\n"
            + " ".join(str(len(s)) for s in sides) + "\n").encode()
    with open(path, "wb") as f:
        f.write(hdr)
        for s, si in zip(streams, sides):
            f.write(s)
            f.write(si)


def hsh_fixtures(out):
    n, col = surface()
    ls = lights()
    obs = lambert(n, col, ls)
    for order in (2, 3, 4):
        terms = order * order
        coef = np.stack([fit(obs[..., c], hsh_terms(ls, terms)) for c in range(3)], axis=2)
        lo = coef.min(axis=(0, 1, 2))
        hi = coef.max(axis=(0, 1, 2))
        span = np.maximum(hi - lo, 1e-4).astype(np.float32)
        lo = lo.astype(np.float32)
        raw = np.clip(np.round((coef - lo) / span * 255), 0, 255).astype(np.uint8)
        if order == 3:
            with open(os.path.join(out, "legacy.hsh"), "wb") as f:
                f.write(b"#HSH1.2\n#synthetic fixture\n")
                f.write(np.array([W, H, 3, order], "<i4").tobytes())
                f.write(lo.astype("<f4").tobytes())
                f.write((lo + span).astype("<f4").tobytes())
                f.write(raw.tobytes())
        with open(os.path.join(out, f"hsh{order}.rti"), "wb") as f:
            f.write(b"#HSH1.2\n#synthetic fixture for specular-rti\n3\n")
            f.write(f"{W} {H} 3\n{terms} 2 1\n".encode())
            f.write(span.astype("<f4").tobytes())
            f.write(lo.astype("<f4").tobytes())
            f.write(raw.tobytes())


def jpeg_rgb(a, path, quality=95):
    Image.fromarray(a.astype(np.uint8), "RGB").save(path, "JPEG", quality=quality,
                                                    subsampling=0)


def relight_planes(folder, planes, quality=95):
    """planes: list of (h, w) float arrays; returns (scale, bias) and writes the JPEGs."""
    scale, bias, q = [], [], []
    for p in planes:
        lo, hi = float(p.min()), float(p.max())
        s = max(hi - lo, 1e-4)
        b = -lo / s
        scale.append(s)
        bias.append(b)
        q.append(np.clip(np.round((p / s + b) * 255), 0, 255))
    while len(q) % 3:
        q.append(np.zeros((H, W)))
    for j in range(len(q) // 3):
        jpeg_rgb(np.stack(q[3 * j:3 * j + 3], axis=-1), os.path.join(folder, f"plane_{j}.jpg"),
                 quality)
    return scale, bias


def relight_fixtures(out):
    n, col = surface()
    ls = lights()
    obs = lambert(n, col, ls)
    flat_lights = [round(float(v), 6) for v in ls.reshape(-1)]

    def write(name, info):
        d = os.path.join(out, name)
        os.makedirs(d, exist_ok=True)
        with open(os.path.join(d, "info.json"), "w") as f:
            json.dump(info, f, indent=1)
        return d

    # PTM / LRGB: colour plane, then the six luminance terms in Relight's order.
    d = write("relight_ptm_lrgb", {})
    lum = obs.mean(axis=-1) / np.maximum(col.mean(axis=-1), 1e-6)[..., None]
    lc = fit(lum, ptm_terms(ls, relight=True))
    planes = [col[..., c] for c in range(3)] + [lc[..., k] for k in range(6)]
    s, b = relight_planes(d, planes)
    write("relight_ptm_lrgb", {"width": W, "height": H, "format": "jpg", "type": "ptm",
                               "colorspace": "lrgb", "nplanes": 9, "quality": 95,
                               "materials": [{"scale": s, "bias": b}], "lights": flat_lights})

    for kind, terms, fn in (("hsh", 9, hsh_terms), ("sh", 4, sh_terms)):
        name = f"relight_{kind}_rgb"
        d = write(name, {})
        coef = [fit(obs[..., c], fn(ls, terms)) for c in range(3)]
        planes = [coef[c][..., t] for t in range(terms) for c in range(3)]
        s, b = relight_planes(d, planes)
        write(name, {"width": W, "height": H, "format": "jpg", "type": kind,
                     "colorspace": "rgb", "nplanes": 3 * terms, "quality": 95,
                     "materials": [{"scale": s, "bias": b}], "lights": flat_lights})

    # RBF / MRGB: PCA of the observations at 20 lights.
    rl = lights(20)
    o = lambert(n, col, rl).reshape(H * W, -1)          # (pixels, lights*3), light-major
    np_ = 9
    mean, comps, coefs = pca(o, np_)
    basis, rng = pack_basis(mean, comps, lambda p, l, k: (p, l, k), np_, len(rl))
    d = write("relight_rbf_mrgb", {})
    s, b = relight_planes(d, [coefs[:, p].reshape(H, W) for p in range(np_)])
    write("relight_rbf_mrgb", {"width": W, "height": H, "format": "jpg", "type": "rbf",
                               "colorspace": "mrgb", "nplanes": np_, "quality": 95,
                               "sigma": 0.125, "lights": [round(float(v), 6)
                                                          for v in rl.reshape(-1)],
                               "materials": [{"scale": s, "bias": b, "range": rng}],
                               "basis": basis})

    # Bilinear / MYCC: luma and two chroma planes in the first jpeg, luma only after.
    res = 6
    bl = octa_lights(res)
    rgb = lambert(n, col, bl).reshape(H * W, len(bl), 3)
    t = np.array([[1.0, 0.5, -0.5], [1.0, 0.0, 0.5], [1.0, -0.5, -0.5]])
    ycc = np.einsum("rk,plk->plr", np.linalg.inv(t), rgb)
    ny = [7, 1, 1]
    my, cy, ky = pca(ycc[..., 0], ny[0])
    ma, ca, ka = pca(ycc[..., 1], ny[1])
    mb, cb, kb = pca(ycc[..., 2], ny[2])
    # planes: jpeg 0 = (Y0, A0, B0); jpegs 1.. = Y1..Y6
    planes = [ky[:, 0], ka[:, 0], kb[:, 0]] + [ky[:, p] for p in range(1, ny[0])]
    comps = [(cy[0], 0), (ca[0], 1), (cb[0], 2)] + [(cy[p], 0) for p in range(1, ny[0])]
    nplanes = len(planes)
    mean = np.stack([my, ma, mb], axis=-1).reshape(-1)    # (lights*3)
    tab = np.zeros((nplanes + 1, len(bl), 3))
    tab[0] = np.clip(mean.reshape(len(bl), 3), 0, 1)
    rng = []
    for p, (c, ch) in enumerate(comps):
        r = 127.0 / max(np.abs(c).max(), 1e-6)
        rng.append(round(float(r), 4))
        tab[p + 1, :, ch] = c
    bytes_ = np.zeros_like(tab, dtype=np.uint8)
    bytes_[0] = np.clip(np.round(tab[0] * 255), 0, 255)
    for p in range(nplanes):
        bytes_[p + 1] = np.clip(np.round(tab[p + 1] * rng[p] + 127), 0, 255)
    d = write("relight_bln_mycc", {})
    s, b = relight_planes(d, [p.reshape(H, W) for p in planes])
    with open(os.path.join(d, "materials.bin"), "wb") as f:
        f.write(bytes_.tobytes())
    write("relight_bln_mycc", {"width": W, "height": H, "format": "jpg", "type": "bilinear",
                               "colorspace": "mycc", "yccplanes": ny, "resolution": res,
                               "quality": 95,
                               "materials": [{"scale": s, "bias": b, "range": rng}]})


def pca(x, k):
    mean = x.mean(axis=0)
    u, sv, vt = np.linalg.svd(x - mean, full_matrices=False)
    comps = vt[:k]
    return mean, comps, (x - mean) @ comps.T


def pack_basis(mean, comps, _idx, np_, nl):
    rng = [round(float(127.0 / max(np.abs(c).max(), 1e-6)), 4) for c in comps]
    tab = np.zeros((np_ + 1, nl * 3), np.uint8)
    tab[0] = np.clip(np.round(mean * 255), 0, 255)
    for p in range(np_):
        tab[p + 1] = np.clip(np.round(comps[p] * rng[p] + 127), 0, 255)
    return [int(v) for v in tab.reshape(-1)], rng


def octa_lights(res):
    out = []
    for j in range(res):
        for i in range(res):
            X, Y = 2 * i / (res - 1) - 1, 2 * j / (res - 1) - 1
            x, y = (X - Y) / 2, (X + Y) / 2
            z = max(1 - abs(x) - abs(y), 0.0)
            v = np.array([x, y, z])
            out.append(v / max(np.linalg.norm(v), 1e-9))
    return np.array(out)


def main():
    out = sys.argv[1]
    os.makedirs(out, exist_ok=True)
    ptm_fixtures(out)
    hsh_fixtures(out)
    relight_fixtures(out)


if __name__ == "__main__":
    main()
