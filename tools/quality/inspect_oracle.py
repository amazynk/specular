#!/usr/bin/env python3
"""Oracle for the radiograph and crack probes (specular-measure `inspect`).

Generates synthetic 16-bit radiographs with numpy (a weld bead with known noise, a duplex-wire
IQI with pairs at a known pitch, a single-wire IQI, a step wedge for density, and a photo of
a dark crack of known width), then recomputes every probe independently with numpy/scipy:
profiles by scipy.ndimage.map_coordinates (order 1, edge extended), SNR from the region,
the duplex dips by the documented steps, FWHM for wires and cracks. Also checks the ground
truth (noise σ, first unresolved pair of the generator, wire number, crack width).

    python3 -I tools/quality/inspect_oracle.py gen <dir>
    INSPECT_ORACLE_DIR=<dir> cargo test -p specular-measure --test inspect_oracle -- --ignored
    python3 -I tools/quality/inspect_oracle.py check <dir>
"""
import json
import pathlib
import sys

import numpy as np
from scipy import ndimage

DUPLEX = [0.80, 0.63, 0.50, 0.40, 0.32, 0.25, 0.20, 0.16, 0.13, 0.10, 0.080, 0.063, 0.050,
          0.040, 0.032]
WIRES = [3.20, 2.50, 2.00, 1.60, 1.25, 1.00, 0.80, 0.63, 0.50, 0.40, 0.32, 0.25, 0.20, 0.16,
         0.125, 0.100, 0.080, 0.063, 0.050]
MM = 0.02  # mm per pixel of the radiographs (20 µm)
TOL = 1e-6


def chord(x, d):
    r = d / 2.0
    return np.where(np.abs(x) < r, 2.0 * np.sqrt(np.clip(r * r - x * x, 0, None)), 0.0)


def radiograph(seed, sigma_blur_mm, noise):
    """A weld radiograph: 4200 × 700 px. Returns the image, the cases' probes, and truth."""
    rng = np.random.default_rng(seed)
    h, w = 700, 4200
    y, x = np.mgrid[0:h, 0:w] + 0.5
    # Steel plate with a weld bead across the middle (thicker → darker), in mm.
    t = 8.0 + 2.0 * np.exp(-(((y - 350) / 90.0) ** 2))
    # Duplex IQI along x at y 80–180: pairs at a 5 mm pitch from x = 4 mm.
    xm, ym = x * MM, y * MM
    band = (y > 80) & (y < 180)
    for i, d in enumerate(DUPLEX):
        c = 4.0 + 5.0 * i
        t = t + band * (chord(xm - (c - d), d) + chord(xm - (c + d), d))
    # Single-wire IQI W10–W16 across y at 500–650 (vertical wires every 5 mm from x = 10 mm).
    wires = list(range(10, 17))
    vband = (y > 500) & (y < 650)
    for i, n in enumerate(wires):
        c = 10.0 + 5.0 * i
        t = t + vband * chord(xm - c, WIRES[n - 1])
    # A step wedge at x 3700–4100: four steps of 1 mm less steel, 100 px each, rows 300–400.
    wedge = (y > 300) & (y < 400) & (x > 3700) & (x < 4100)
    step = np.clip(((x - 3700) // 100).astype(int), 0, 3)
    t = t - wedge * (1.0 + step)
    i0 = 60000.0
    clean = ndimage.gaussian_filter(i0 * np.exp(-0.15 * t), sigma_blur_mm / MM, mode="nearest")
    img = clean + rng.normal(0.0, noise, clean.shape)
    img = np.clip(np.rint(img), 0, 65535).astype(np.uint16)
    # Ground truth: the first pair whose dip is below 20 % on the noise-free image, read at
    # the pairs' known places (pixel sampling and blur included, no grouping).
    row = clean[130]
    s = smooth(np.median(row) - row, 3)
    truth_pair = None
    for n, d in enumerate(DUPLEX, 1):
        c, dp = (4.0 + 5.0 * (n - 1)) / MM - 0.5, d / MM
        lo, mid, hi = int(np.floor(c - 2 * dp)), int(round(c)), int(np.ceil(c + 2 * dp))
        pa = lo + int(np.argmax(s[lo:mid + 1]))
        pb = mid + int(np.argmax(s[mid:hi + 1]))
        a, b, cc = s[pa], s[pb], s[pa:pb + 1].min()
        if pa == pb or (a + b - 2 * cc) / (a + b) * 100 < 20:
            truth_pair = n
            break
    probes = [
        {"kind": "profile", "a": [100.5, 350.5], "b": [100.5, 650.25], "width": 1},
        {"kind": "profile", "a": [10.0, 130.0], "b": [900.0, 133.0], "width": 9},
        {"kind": "snr", "rect": [1200.0, 340.0, 1210.0, 345.0], "srb_um": 40.0},
        {"kind": "snr", "rect": [2300.0, 600.0, 2330.0, 690.0], "srb_um": None},
        {"kind": "duplex", "a": [60.0, 130.0], "b": [(4.0 + 5.0 * 14 + 2.5) / MM, 130.0]},
        {"kind": "density", "grey": [float(img[350, 3750]), float(img[350, 4050])],
         "density": [3.5, 1.2], "at": [[3850.5, 350.5], [3950.5, 350.5], [3790.25, 333.75]]},
    ]
    for i, n in enumerate(wires[:4]):
        c = (10.0 + 5.0 * i) / MM
        probes.append({"kind": "wire", "a": [c - 40.0, 575.0], "b": [c + 40.0, 575.0]})
    truth = {"noise": noise, "first_unresolved": truth_pair, "wires": wires[:4],
             "blur": sigma_blur_mm}
    return img, probes, truth


def crack_photo(seed, width_px):
    """A grey photo of a slanted dark crack of known width on concrete-like texture."""
    rng = np.random.default_rng(seed)
    h, w = 400, 600
    y, x = np.mgrid[0:h, 0:w] + 0.5
    # Signed distance to the line through (100, 50) and (500, 350).
    a, b = np.array([100.0, 50.0]), np.array([500.0, 350.0])
    n = np.array([-(b - a)[1], (b - a)[0]]) / np.linalg.norm(b - a)
    dist = (x - a[0]) * n[0] + (y - a[1]) * n[1]
    # Anti-aliased band: coverage of each pixel by |dist| <= width/2 (sampled 8 × 8).
    cov = np.zeros((h, w))
    for sy in range(8):
        for sx in range(8):
            dd = (x - 0.5 + (sx + 0.5) / 8 - a[0]) * n[0] + (y - 0.5 + (sy + 0.5) / 8 - a[1]) * n[1]
            cov += (np.abs(dd) <= width_px / 2.0)
    cov /= 64.0
    img = 40000.0 - 30000.0 * cov + rng.normal(0, 300.0, (h, w))
    img = np.clip(np.rint(img), 0, 65535).astype(np.uint16)
    probes = [{"kind": "crack", "line": [[120.0, 65.0], [300.0, 200.0], [480.0, 335.0]],
               "n": 10, "half": 20.0}]
    return img, probes, {"crack_px": width_px}


def gen(d):
    d.mkdir(parents=True, exist_ok=True)
    cases = []
    truths = {}
    specs = [("weld-sharp", radiograph, (1, 0.03, 150.0)),
             ("weld-blurred", radiograph, (2, 0.07, 250.0)),
             ("weld-soft", radiograph, (3, 0.12, 200.0)),
             ("crack-4px", crack_photo, (4, 4.0)),
             ("crack-9px", crack_photo, (5, 9.0))]
    for name, f, args in specs:
        img, probes, truth = f(*args)
        img.astype("<u2").tofile(d / f"{name}.u16")
        cases.append({"name": name, "w": img.shape[1], "h": img.shape[0],
                      "file": f"{name}.u16", "mm_per_px": MM, "probes": probes})
        truths[name] = truth
    (d / "cases.json").write_text(json.dumps(cases, indent=1))
    (d / "truth.json").write_text(json.dumps(truths, indent=1))
    print(f"wrote {len(cases)} images to {d}")


# ----- the independent computations -----------------------------------------------------

def sample(img, pts):
    pts = np.asarray(pts, float)
    return ndimage.map_coordinates(img.astype(float), [pts[:, 1] - 0.5, pts[:, 0] - 0.5],
                                   order=1, mode="nearest")


def profile(img, a, b, width):
    a, b = np.asarray(a, float), np.asarray(b, float)
    L = np.linalg.norm(b - a)
    n = max(1, int(np.ceil(L)))
    nrm = np.array([-(b - a)[1], (b - a)[0]]) / L
    t = np.arange(n + 1) / n
    c = a[None, :] + t[:, None] * (b - a)[None, :]
    offs = np.arange(width) - (width - 1) / 2.0
    vals = np.mean([sample(img, c + o * nrm[None, :]) for o in offs], axis=0)
    return vals, L / n


def snr_region(rect, iw, ih):
    x0, y0, x1, y1 = min(rect[0], rect[2]), min(rect[1], rect[3]), max(rect[0], rect[2]), max(rect[1], rect[3])
    mw, mh = (55, 20) if x1 - x0 >= y1 - y0 else (20, 55)

    def fit(a, b, m, size):
        k = max(int(np.floor(max(b - a, 0) + 0.5)), m)
        s = np.floor((a + b) / 2 - k / 2 + 0.5)  # rounds half up (the region centres are not halves here)
        s = int(min(max(s, 0), size - k))
        return s, s + k
    rx = fit(x0, x1, mw, iw)
    ry = fit(y0, y1, mh, ih)
    return rx[0], ry[0], rx[1], ry[1]


def smooth(v, k):
    """Centred moving average, fewer samples at the ends."""
    h = k // 2
    c = np.concatenate([[0.0], np.cumsum(v)])
    i = np.arange(len(v))
    a, b = np.maximum(i - h, 0), np.minimum(i + h, len(v) - 1)
    return (c[b + 1] - c[a]) / (b - a + 1)


def wire_signal(v, k):
    v = smooth(np.asarray(v, float), k)
    b = np.median(v)
    sign = -1.0 if b - v.min() >= v.max() - b else 1.0
    return sign * (v - b)


def duplex(v, mm):
    s = wire_signal(v, 3)
    noise = 1.4826 * np.median(np.abs(s - np.median(s)))
    thr = max(3 * noise, 0.02 * s.max())
    above = np.concatenate([[False], s > thr, [False]])
    edges = np.flatnonzero(np.diff(above.astype(int)))
    runs = [(a, b) for a, b in zip(edges[::2], edges[1::2] - 1) if s[a:b + 1].max() >= 2 * thr]
    groups, single, prev = [], False, 0
    for a, bb in runs:
        wdt = bb - a + 1
        if groups and single:
            gap = a - groups[-1][1] - 1
            lo, hi = min(prev, wdt), max(prev, wdt)
            if 2 * gap < 3 * lo and hi <= 2 * lo:
                groups[-1] = (groups[-1][0], bb)
                single = False
                continue
        groups.append((a, bb))
        prev, single = wdt, True
    if len(groups) > 1 and groups[-1][1] - groups[-1][0] > groups[0][1] - groups[0][0]:
        groups.reverse()
    dips, first = [], None
    for i, (a, bb) in enumerate(groups):
        mid = (a + bb) // 2
        if mid > a:
            pa = a + int(np.argmax(s[a:mid + 1]))
            pb = mid + 1 + int(np.argmax(s[mid + 1:bb + 1]))
        else:
            pa, pb = a, bb
        A, B = s[pa], s[pb]
        C = s[min(pa, pb):max(pa, pb) + 1].min()
        dip = min(max((A + B - 2 * C) / (A + B) * 100, 0.0), 100.0) if A + B > 0 else 0.0
        res = dip >= 20 and pa != pb
        if res and first is None:
            first = (i, abs(pb - pa) * mm / 2)
        dips.append((dip, res))
    i0, d0 = first if first else (0, (groups[0][1] - groups[0][0] + 1) * mm / 3)
    n0 = 1 + int(np.argmin([abs(np.log(t / d0)) for t in DUPLEX])) - i0
    numbers = [n0 + i if 1 <= n0 + i <= len(DUPLEX) else None for i in range(len(groups))]
    fu = next((i for i, (_, r) in enumerate(dips) if not r), None)
    if fu is None and numbers and numbers[-1] is not None and numbers[-1] < len(DUPLEX):
        numbers.append(numbers[-1] + 1)
        dips.append((0.0, False))
        fu = len(dips) - 1
    return [d for d, _ in dips], numbers, (numbers[fu] if fu is not None else None)


def isrb(dips, numbers, fu_number, mm):
    """iSRb (mm), ISO 19232-5:2018 / ASTM E2002-15 7.2.3: a second-order polynomial through
    the dips of the last two resolved pairs and up to two unresolved pairs above 0 % (else the
    first unresolved pair) over the wire diameter, crossed with 20 % between the last resolved
    and first unresolved diameters; linear with two points or no crossing; at least `mm`."""
    if fu_number is None:
        return None
    fu = numbers.index(fu_number)
    if fu == 0:
        return None

    def pt(i):
        return (DUPLEX[numbers[i] - 1], dips[i]) if numbers[i] is not None else None

    above = [p for p in (pt(i) for i in range(fu - 1, max(fu - 3, -1), -1)) if p]
    below = [p for p in (pt(i) for i in range(fu, min(fu + 2, len(dips))))
             if p and 0 < p[1] < 20]
    if not below and pt(fu):
        below = [pt(fu)]
    if not above or not pt(fu):
        return None
    (xh, yh), (xl, yl) = above[0], pt(fu)
    lin = xl + (xh - xl) * (20 - yl) / (yh - yl) if yh > yl else xl
    pts = np.array(above + below)
    x = lin
    if len(pts) >= 3:
        c = np.polyfit(pts[:, 0], pts[:, 1], 2)
        c[-1] -= 20
        r = [z.real for z in np.roots(c)
             if abs(z.imag) < 1e-12 and xl - 1e-9 <= z.real <= xh + 1e-9]
        if r:
            x = r[0]
    return max(min(max(x, xl), xh), mm)


def fwhm(s, peak):
    half = s[peak] / 2
    l = peak
    while l > 0 and s[l - 1] > half:
        l -= 1
    r = peak
    while r + 1 < len(s) and s[r + 1] > half:
        r += 1
    if l == 0 or r + 1 >= len(s):
        return None
    left = (l - 1) + (half - s[l - 1]) / (s[l] - s[l - 1])
    right = r + (s[r] - half) / (s[r] - s[r + 1])
    return left, right


def wire(v, mm):
    s = wire_signal(v, 5)
    l, r = fwhm(s, int(np.argmax(s)))
    f = (r - l) * mm
    dia = f / (np.sqrt(3) / 2)
    k = int(np.argmin([abs(np.log(t / dia)) for t in WIRES]))
    return f, k + 1


def crack(img, line, n, half):
    line = np.asarray(line, float)
    seg = np.diff(line, axis=0)
    lens = np.linalg.norm(seg, axis=1)
    cum = np.concatenate([[0], np.cumsum(lens)])
    steps = int(round(half / 0.25))
    out = []
    for k in range(n):
        at = cum[-1] * (k + 0.5) / n
        i = min(int(np.searchsorted(cum, at, side="left")) - 1, len(seg) - 1)
        i = max(i, 0)
        t = (at - cum[i]) / lens[i]
        c = line[i] + t * seg[i]
        nrm = np.array([-seg[i][1], seg[i][0]]) / lens[i]
        offs = np.arange(-steps, steps + 1) * 0.25
        cut = sample(img, c[None, :] + offs[:, None] * nrm[None, :])
        m = len(cut)
        q = m // 4
        bg = np.median(np.concatenate([cut[:q], cut[m - q:]]))
        lo = q + int(np.argmin(cut[q:m - q]))
        s = bg - cut
        f = fwhm(s, lo)
        if f is None:
            continue
        out.append((f[1] - f[0]) * 0.25)
    return out


def check(d):
    cases = json.loads((d / "cases.json").read_text())
    truths = json.loads((d / "truth.json").read_text())
    rust = {r["name"]: r["results"] for r in json.loads((d / "rust.json").read_text())}
    compared = mismatches = 0
    truth_fail = 0

    def cmp(what, a, b, tol=TOL):
        nonlocal compared, mismatches
        a, b = np.atleast_1d(np.asarray(a, float)), np.atleast_1d(np.asarray(b, float))
        compared += a.size
        if a.shape != b.shape:
            mismatches += max(a.size, b.size)
            print(f"  MISMATCH {what}: shapes {a.shape} vs {b.shape}")
            return
        bad = ~np.isclose(a, b, rtol=tol, atol=tol)
        if bad.any():
            mismatches += int(bad.sum())
            print(f"  MISMATCH {what}: {int(bad.sum())} of {a.size}, max diff {np.abs(a - b).max():.3g}")

    for case in cases:
        name = case["name"]
        img = np.fromfile(d / case["file"], dtype="<u2").reshape(case["h"], case["w"])
        mm = case["mm_per_px"]
        truth = truths[name]
        print(name)
        for p, r in zip(case["probes"], rust[name]):
            k = p["kind"]
            if k == "profile":
                v, step = profile(img, p["a"], p["b"], p["width"])
                cmp("profile", r["values"], v)
                cmp("step", r["step"], step)
            elif k == "snr":
                reg = snr_region(p["rect"], case["w"], case["h"])
                cmp("snr region", r["region"], reg, 0)
                x0, y0, x1, y1 = reg
                block = img[y0:y1, x0:x1].astype(float)
                mean, sd = block.mean(), block.std(ddof=1)
                cmp("snr mean", r["mean"], mean)
                cmp("snr sd", r["sd"], sd)
                cmp("snr", r["snr"], mean / sd)
                if p["srb_um"]:
                    cmp("snrn", r["snrn"], mean / sd * 88.6 / p["srb_um"])
                # Truth: the region is flat weld/plate; σ within 15 % of the generator's.
                if abs(sd - truth["noise"]) > 0.15 * truth["noise"] + 30:
                    print(f"  TRUTH: σ {sd:.1f} vs generated {truth['noise']}")
                    truth_fail += 1
            elif k == "duplex":
                v, _ = profile(img, p["a"], p["b"], 21)
                a, b = np.asarray(p["a"]), np.asarray(p["b"])
                step_mm = mm * np.linalg.norm(b - a) / (len(v) - 1)
                dips, numbers, fu = duplex(v, step_mm)
                cmp("dips", r["dips"], dips, 1e-6)
                if numbers != r["numbers"] or fu != r["first_unresolved"]:
                    mismatches += 1
                    print(f"  MISMATCH duplex numbers {r['numbers']} / {r['first_unresolved']} vs {numbers} / {fu}")
                compared += 1
                want = isrb(dips, numbers, fu, r["step_mm"])
                if (want is None) != (r["isrb_mm"] is None):
                    mismatches += 1
                    print(f"  MISMATCH iSRb {r['isrb_mm']} vs {want}")
                elif want is not None:
                    cmp("isrb", r["isrb_mm"], want, 1e-6)
                print(f"  duplex: first unresolved D{fu} (generator: D{truth['first_unresolved']}), "
                      f"iSRb {want}")
                if fu is None or abs(fu - truth["first_unresolved"]) > 1:
                    truth_fail += 1
                    print("  TRUTH: first unresolved pair off by more than one")
            elif k == "wire":
                v, _ = profile(img, p["a"], p["b"], 21)
                a, b = np.asarray(p["a"]), np.asarray(p["b"])
                step_mm = mm * np.linalg.norm(b - a) / (len(v) - 1)
                f, num = wire(v, step_mm)
                cmp("wire fwhm", r["fwhm"], f)
                compared += 1
                if num != r["number"]:
                    mismatches += 1
                    print(f"  MISMATCH wire number {r['number']} vs {num}")
                want = truth["wires"][[q for q in case["probes"] if q["kind"] == "wire"].index(p)]
                # The blur widens a wire thinner than it: checked only where it does not.
                if 2.355 * truth["blur"] >= WIRES[want - 1]:
                    print(f"  wire W{want}: blur-limited, read as W{num} (not checked)")
                elif abs(num - want) > 1:
                    truth_fail += 1
                    print(f"  TRUTH: wire W{num} vs placed W{want}")
            elif k == "crack":
                px = crack(img, p["line"], p["n"], p["half"])
                cmp("crack px", r["px"], px)
                mmw = np.asarray(px) * mm
                cmp("crack min/mean/max", [r["min"], r["mean"], r["max"]],
                    [mmw.min(), mmw.mean(), mmw.max()])
                if abs(np.mean(px) - truth["crack_px"]) > 0.6:
                    truth_fail += 1
                    print(f"  TRUTH: crack {np.mean(px):.2f} px vs {truth['crack_px']}")
            elif k == "density":
                g0, g1 = p["grey"]
                d0, d1 = p["density"]
                vals = sample(img, p["at"])
                cmp("density", r["density"], d0 + (vals - g0) * (d1 - d0) / (g1 - g0))
    print(f"{compared} values compared, {mismatches} mismatches; {truth_fail} ground-truth misses")
    return 1 if mismatches or truth_fail else 0


if __name__ == "__main__":
    cmd, d = sys.argv[1], pathlib.Path(sys.argv[2])
    sys.exit(gen(d) if cmd == "gen" else check(d))
