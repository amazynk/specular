#!/usr/bin/env python3
"""An independent reading of RTI files, written from the format descriptions (the PTM 1.2
specification, CHI's .rti/.hsh layout and CNR-ISTI Relight's relight-core.js), for
specular-rti's tests.

    python3 -I oracle.py DIR OUT.tsv

Every .ptm, .rti and .hsh file in DIR, and every subfolder holding a Relight info.json, is
read with numpy (JPEG planes with Pillow) and written as tab-separated rows:

  file   NAME  CONTAINER  WIDTH  HEIGHT  PLANES
  plane  NAME  P  SCALE  OFFSET  WSUM  SUM  S0,S1,…   one row per coefficient plane P in
         the format's own order; the plane's value is byte · SCALE + OFFSET; WSUM (Σ byte_i ·
         ((i · 2654435761 + 1) mod 2³²) mod 2⁶⁴) and SUM are over all its bytes (rows
         top-down), S… are the bytes at 64 evenly spaced pixels (index i · n / 64)
  relit  NAME  LX LY LZ  R0,G0,B0,R1,…   the relit colour (display units, not clamped) at
         the same 64 pixels for the light (LX, LY, LZ), x right, y up, z to the viewer
  error  NAME  MESSAGE                    a file the oracle cannot read

Plane order and units:
  PTM LRGB  a0..a5 then R, G, B;  PTM RGB  a0..a5 of red, of green, of blue.
            Coefficients are (byte − bias) · scale / 255 so that the luminance polynomial is
            in 0..1; colours byte / 255.  Light: L = a0 u² + a1 v² + a2 uv + a3 u + a4 v + a5;
            LRGB out = colour · L, RGB out = L per channel.
  HSH       term k of channel c is plane 3k + c; value = byte / 255 · scale + bias (.hsh:
            byte / 255 · (max − min) + min).  Weights: real hemispherical harmonics from the
            Condon–Shortley associated Legendre functions of 2 cos θ − 1, the m ≠ 0 terms
            negated (RTIViewer's sign), index l² + l − m, θ at most π/2 − 0.04.
  Relight   plane p is channel p % 3 of plane_{p // 3}.jpg; value = (byte / 255 − bias) ·
            scale.  rgb: out_c = Σ_t w_t · plane(3t + c); lrgb: planes 0..2 are the colour,
            out = colour · Σ_t w_t · plane(3 + t).  mrgb/mycc: weights interpolated from the
            light table (rbf: Gaussian, normalised, weights ≤ 0.001 dropped, renormalised;
            bilinear: the octahedral map); mycc converts luma/chroma with g = y + b/2,
            b' = y − b/2 − a/2, r = b' + a.
"""

import io
import json
import math
import os
import sys

import numpy as np
from PIL import Image

LIGHTS = [(0.0, 0.0, 1.0), (0.5, 0.3, None), (-0.6, -0.2, None), (0.1, -0.8, None)]
SAMPLES = 64


def unit(x, y, z):
    if z is None:
        z = math.sqrt(max(1 - x * x - y * y, 0.0))
    v = np.array([x, y, z], np.float64)
    return v / np.linalg.norm(v)


def wsum(b):
    """Σ byte_i · ((i · 2654435761 + 1) mod 2³²) mod 2⁶⁴ over the plane's bytes."""
    i = np.arange(b.size, dtype=np.uint64)
    wt = (i * np.uint64(2654435761) + np.uint64(1)) & np.uint64(0xFFFFFFFF)
    return int((b.astype(np.uint64) * wt).sum(dtype=np.uint64))


PICK = np.zeros(0, np.int64)


def S(a):
    """The sampled pixels (PICK, raster order) of an (h, w, …) array."""
    return a.reshape((-1,) + a.shape[2:])[PICK]


# ---------------------------------------------------------------- readers
# Each returns (container, w, h, planes, shade) where planes is a list of
# (bytes (h, w) uint8, scale, offset) and shade(light) gives the (h, w, 3) colour.

def read_ptm(data):
    pos = 0
    toks = []

    def tok():
        nonlocal pos
        while data[pos:pos + 1].isspace():
            pos += 1
        s = pos
        while pos < len(data) and not data[pos:pos + 1].isspace():
            pos += 1
        return data[s:pos].decode("ascii")

    version, fmt = tok(), tok()
    w, h = int(tok()), int(tok())
    scale = [float(tok()) for _ in range(6)]
    bias = [int(tok()) for _ in range(6)]
    n = w * h

    def body_start():
        nonlocal pos
        while data[pos:pos + 1] in (b" ", b"\r", b"\t"):
            pos += 1
        if data[pos:pos + 1] == b"\n":
            pos += 1
        return pos

    def up(a):            # bottom-up rows to top-down (h, w, ...)
        return a.reshape((h, w) + a.shape[1:])[::-1]

    coef = np.zeros((h, w, 6), np.uint8)
    rgb = np.zeros((h, w, 3), np.uint8)
    ccoef = None
    if fmt == "PTM_FORMAT_LRGB":
        s = body_start()
        if version == "PTM_1.1":
            px = np.frombuffer(data, np.uint8, n * 9, s).reshape(n, 9)
            coef, rgb = up(px[:, :6]), up(px[:, 6:])
        else:
            coef = up(np.frombuffer(data, np.uint8, n * 6, s).reshape(n, 6))
            rgb = up(np.frombuffer(data, np.uint8, n * 3, s + n * 6).reshape(n, 3))
    elif fmt == "PTM_FORMAT_RGB":
        s = body_start()
        ccoef = [up(np.frombuffer(data, np.uint8, n * 6, s + c * n * 6).reshape(n, 6))
                 for c in range(3)]
    elif fmt in ("PTM_FORMAT_PTM_LUT", "PTM_FORMAT_PTM_C_LUT"):
        t = tok()
        if t.lower() == "nentries":
            t = tok()
        entries = int(t)
        s = body_start()
        per = 9 if fmt.endswith("C_LUT") else 6
        lut = np.frombuffer(data, np.uint8, entries * per, s).reshape(entries, per)
        s += entries * per
        dt = np.dtype("<u2") if entries > 256 else np.dtype(np.uint8)
        idx = np.frombuffer(data, dt, n, s).astype(np.int64)
        s += n * dt.itemsize
        px = lut[idx]
        coef = up(px[:, :6])
        rgb = up(px[:, 6:9]) if per == 9 else up(np.frombuffer(data, np.uint8, n * 3, s)
                                                  .reshape(n, 3))
    elif fmt in ("PTM_FORMAT_JPEG_LRGB", "PTM_FORMAT_JPEG_RGB"):
        k = 9 if fmt.endswith("LRGB") else 18
        q = int(tok())
        xform = [int(tok()) for _ in range(k)]
        motion = [int(tok()) for _ in range(2 * k)]
        order = [int(tok()) for _ in range(k)]
        refs = [int(tok()) for _ in range(k)]
        sizes = [int(tok()) for _ in range(k)]
        sides = [int(tok()) for _ in range(k)]
        s = body_start()
        chunks = []
        for i in range(k):
            chunks.append((data[s:s + sizes[i]], data[s + sizes[i]:s + sizes[i] + sides[i]]))
            s += sizes[i] + sides[i]
        out = [None] * k
        for rank in range(k):
            i = order.index(rank)
            if xform[i] & 2 and (motion[i] or motion[k + i]):
                raise ValueError("motion compensation")
            p = np.asarray(Image.open(io.BytesIO(chunks[i][0])).convert("L"), np.int32)
            if refs[i] >= 0:
                base = out[refs[i]].astype(np.int32)
                if xform[i] & 1:
                    base = 255 - base
                p = (base + p - 128) % 256
            flat = p.reshape(-1)
            side = chunks[i][1]
            for r in range(0, len(side) - 4, 5):
                at = int.from_bytes(side[r:r + 4], "big")
                if at < n:
                    flat[at] = side[r + 4]
            out[i] = p.astype(np.uint8)
        if k == 9:
            coef = np.stack(out[:6], -1)
            rgb = np.stack(out[6:], -1)
        else:
            ccoef = [np.stack(out[6 * c:6 * c + 6], -1) for c in range(3)]
        _ = q
    else:
        raise ValueError(f"PTM format {fmt}")

    sc = [s_ / 255.0 for s_ in scale]
    off = [-b * s_ / 255.0 for b, s_ in zip(bias, scale)]

    def poly(c, l):
        u, v = l[0], l[1]
        a = [c[..., i].astype(np.float64) * sc[i] + off[i] for i in range(6)]
        return a[0] * u * u + a[1] * v * v + a[2] * u * v + a[3] * u + a[4] * v + a[5]

    if ccoef is None:
        planes = [(coef[..., i], sc[i], off[i]) for i in range(6)]
        planes += [(rgb[..., c], 1 / 255.0, 0.0) for c in range(3)]

        def shade(l):
            return S(rgb).astype(np.float64) / 255.0 * poly(S(coef), l)[..., None]
    else:
        planes = [(ccoef[c][..., i], sc[i], off[i]) for c in range(3) for i in range(6)]

        def shade(l):
            return np.stack([poly(S(ccoef[c]), l) for c in range(3)], -1)
    return "PTM", w, h, planes, shade


def legendre(l, m, x):
    """Associated Legendre P_l^m(x), m ≥ 0, with the Condon–Shortley phase."""
    pmm = np.ones_like(x)
    if m > 0:
        somx2 = np.sqrt(np.maximum((1 - x) * (1 + x), 0))
        fact = 1.0
        for _ in range(m):
            pmm = -pmm * fact * somx2
            fact += 2
    if l == m:
        return pmm
    pmmp1 = x * (2 * m + 1) * pmm
    if l == m + 1:
        return pmmp1
    for ll in range(m + 2, l + 1):
        pll = ((2 * ll - 1) * x * pmmp1 - (ll + m - 1) * pmm) / (ll - m)
        pmm, pmmp1 = pmmp1, pll
    return pmmp1


def hsh(terms, light):
    x, y, z = light
    theta = min(math.acos(max(min(z, 1.0), -1.0)), math.pi / 2 - 0.04)
    phi = math.atan2(y, x)
    if phi < 0:
        phi += 2 * math.pi
    xs = np.array(2 * math.cos(theta) - 1)
    out = np.zeros(terms)
    order = int(round(math.sqrt(terms)))
    for l in range(order):
        for m in range(-l, l + 1):
            am = abs(m)
            k = math.sqrt((2 * l + 1) / (2 * math.pi) * math.factorial(l - am)
                          / math.factorial(l + am))
            p = float(legendre(l, am, xs))
            if m == 0:
                v = k * p
            elif m > 0:
                v = -math.sqrt(2) * k * p * math.cos(m * phi)
            else:
                v = -math.sqrt(2) * k * p * math.sin(am * phi)
            out[l * l + l - m] = v
    return out


def read_hsh(data):
    pos = 0
    while data[pos:pos + 1] == b"#":
        pos = data.index(b"\n", pos) + 1
    legacy = False
    try:
        lines = data[pos:pos + 400].split(b"\n")
        kind = [int(t) for t in lines[0].split()]
        size = [int(t) for t in lines[1].split()]
        basis = [int(t) for t in lines[2].split()]
        assert len(kind) == 1 and len(size) == 3 and len(basis) == 3
        if kind[0] != 3:
            raise ValueError(f"RTI type {kind[0]}")
        w, h, bands = size
        terms = {1: 4, 4: 4, 2: 9, 9: 9, 16: 16}[basis[0]]
        pos += sum(len(x) + 1 for x in lines[:3])
    except (ValueError, IndexError, AssertionError, UnicodeDecodeError) as e:
        if isinstance(e, ValueError) and str(e).startswith("RTI type"):
            raise
        legacy = True
        w, h, bands, order = np.frombuffer(data, "<i4", 4, pos)
        w, h, bands, terms = int(w), int(h), int(bands), int(order) ** 2
        pos += 16
    a = np.frombuffer(data, "<f4", terms, pos).astype(np.float64)
    b = np.frombuffer(data, "<f4", terms, pos + 4 * terms).astype(np.float64)
    pos += 8 * terms
    if legacy:
        sc, off = (b - a) / 255.0, a
    else:
        sc, off = a / 255.0, b
    px = np.frombuffer(data, np.uint8, w * h * bands * terms, pos).reshape(h, w, bands, terms)
    planes = [(px[:, :, c, k], float(np.float32(sc[k])), float(off[k]))
              for k in range(terms) for c in range(3)]

    def shade(l):
        wt = hsh(terms, l)
        v = S(px).astype(np.float64) * sc + off
        return np.einsum("nck,k->nc", v, wt)
    return "HSH" if legacy else "RTI (HSH)", w, h, planes, shade


def sh(terms, l):
    x, y, z = l
    a = 0.5 * math.sqrt(3 / math.pi)
    b = 0.5 * math.sqrt(15 / math.pi)
    return np.array([0.5 / math.sqrt(math.pi), a * x, a * z, a * y, b * x * y, b * x * z,
                     0.5 * math.sqrt(5 / math.pi) * (3 * z * z - 1), b * y * z,
                     0.5 * b * (y * y - x * x)][:terms])


def read_relight(folder):
    info = json.load(open(os.path.join(folder, "info.json")))
    w, h = int(info["width"]), int(info["height"])
    cs, kind = info["colorspace"], info["type"]
    np_ = sum(info["yccplanes"]) if cs == "mycc" else int(info["nplanes"])
    mat = info["materials"][0]
    jp = []
    for j in range((np_ + 2) // 3):
        im = Image.open(os.path.join(folder, f"plane_{j}.jpg")).convert("RGB")
        jp.append(np.asarray(im, np.uint8))
    raw = [jp[p // 3][..., p % 3] for p in range(np_)]
    sc = [float(np.float32(s)) / 255.0 for s in mat["scale"][:np_]]
    off = [-float(np.float32(b)) * float(np.float32(s))
           for b, s in zip(mat["bias"][:np_], mat["scale"][:np_])]
    planes = [(raw[p], sc[p], off[p]) for p in range(np_)]

    def val(p):
        return S(raw[p]).astype(np.float64) * sc[p] + off[p]

    if kind in ("ptm", "hsh", "sh"):
        def weights(l, n):
            if kind == "ptm":
                x, y = l[0], l[1]
                return np.array([1, x, y, x * x, x * y, y * y])[:n]
            return hsh(n, l) if kind == "hsh" else sh(n, l)
        if cs == "lrgb":
            def shade(l):
                wt = weights(l, np_ - 3)
                lum = sum(wt[t] * val(3 + t) for t in range(np_ - 3))
                return np.stack([val(c) for c in range(3)], -1) * lum[..., None]
        else:
            def shade(l):
                wt = weights(l, np_ // 3)
                return np.stack([sum(wt[t] * val(3 * t + c) for t in range(np_ // 3))
                                 for c in range(3)], -1)
        return "Relight", w, h, planes, shade

    if "basis" in info:
        bb = np.array(info["basis"], np.float64)
    else:
        bb = np.fromfile(os.path.join(folder, "materials.bin"), np.uint8).astype(np.float64)
    rng = [float(np.float32(r)) for r in mat["range"]]
    if kind == "rbf":
        lights = np.array(info["lights"], np.float64).reshape(-1, 3)
        nd = len(lights)
    else:
        res = int(info["resolution"])
        nd = res * res
    bb = bb[:(np_ + 1) * nd * 3].reshape(np_ + 1, nd, 3)
    base = np.empty_like(bb)
    base[0] = bb[0] / 255.0
    for p in range(np_):
        base[p + 1] = (bb[p + 1] - 127.0) / rng[p]
    base = base.astype(np.float32).astype(np.float64)

    def lweights(l):
        if kind == "rbf":
            sigma = float(np.float32(info["sigma"]))
            d2 = ((lights.astype(np.float32).astype(np.float64) - l) ** 2).sum(1)
            wt = np.exp(-d2 / (sigma * sigma))
            wt /= wt.sum()
            keep = wt > 0.001
            wt = np.where(keep, wt, 0) / wt[keep].sum()
            return np.einsum("i,pik->pk", wt, base)
        s = abs(l[0]) + abs(l[1]) + abs(l[2])
        x = ((l[0] + l[1]) / s + 1) / 2 * (res - 1)
        y = ((l[1] - l[0]) / s + 1) / 2 * (res - 1)
        sx = min(max(math.floor(x), 0), res - 2)
        sy = min(max(math.floor(y), 0), res - 2)
        dx, dy = x - sx, y - sy
        out = np.zeros((np_ + 1, 3))
        for cx, cy, wt in ((sx, sy, (1 - dx) * (1 - dy)), (sx + 1, sy, dx * (1 - dy)),
                           (sx, sy + 1, (1 - dx) * dy), (sx + 1, sy + 1, dx * dy)):
            out += wt * base[:, cx + cy * res]
        return out

    def shade(l):
        lw = lweights(l)
        c = np.zeros((len(PICK), 3)) + lw[0]
        if cs == "mrgb":
            for p in range(np_):
                c += val(p)[..., None] * lw[p + 1]
            return c
        chroma = info["yccplanes"][1]
        for p in range(np_):
            j, ch = p // 3, p % 3
            if j < chroma:
                c[..., ch] += val(p) * lw[p + 1][ch]
            else:
                c[..., 0] += val(p) * lw[p + 1][0]
        yy, a, b = c[..., 0], c[..., 1], c[..., 2]
        g = yy + b / 2
        bl = yy - b / 2 - a / 2
        return np.stack([bl + a, g, bl], -1)
    return "Relight", w, h, planes, shade


# ---------------------------------------------------------------- output

def rows(name, reader, arg):
    try:
        container, w, h, planes, shade = reader(arg)
    except Exception as e:  # noqa: BLE001 - every failure is a row
        return [f"error\t{name}\t{type(e).__name__}: {e}"]
    n = w * h
    idx = [i * n // SAMPLES for i in range(SAMPLES)]
    global PICK
    PICK = np.array(idx, np.int64)
    out = [f"file\t{name}\t{container}\t{w}\t{h}\t{len(planes)}"]
    for p, (b, s, o) in enumerate(planes):
        flat = np.ascontiguousarray(b).reshape(-1)
        out.append(f"plane\t{name}\t{p}\t{s:.9g}\t{o:.9g}\t{wsum(flat):016x}\t"
                   f"{int(flat.sum(dtype=np.int64))}\t" + ",".join(str(int(flat[i])) for i in idx))
    for lx, ly, lz in LIGHTS:
        l = unit(lx, ly, lz)
        img = shade(l).reshape(-1, 3)
        vals = ",".join(f"{v:.6g}" for px in img for v in px)
        out.append(f"relit\t{name}\t{l[0]:.9g}\t{l[1]:.9g}\t{l[2]:.9g}\t{vals}")
    return out


def main():
    src, dst = sys.argv[1], sys.argv[2]
    lines = ["# specular-rti oracle: python3 -I gen/oracle.py " + os.path.basename(src)]
    for name in sorted(os.listdir(src)):
        path = os.path.join(src, name)
        if os.path.isdir(path) and os.path.exists(os.path.join(path, "info.json")):
            lines += rows(name, read_relight, path)
        elif name.lower().endswith(".ptm"):
            lines += rows(name, read_ptm, open(path, "rb").read())
        elif name.lower().endswith((".rti", ".hsh")):
            lines += rows(name, read_hsh, open(path, "rb").read())
    with open(dst, "w") as f:
        f.write("\n".join(lines) + "\n")


if __name__ == "__main__":
    main()
