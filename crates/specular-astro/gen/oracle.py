"""Oracle for specular-astro's star measurements (development only; not run by CI).

For every `*.stars.tsv` next to its FITS file (written by the ignored test
`specular-decode --test astro_metrics oracle_dump`), each star is measured again here with
astropy and photutils, independently of our code:

- FWHM: an astropy Moffat2D + constant fitted by Levenberg-Marquardt to a cutout
  (circular, so it is compared with our geometric-mean FWHM);
- HFD: twice the radius holding half the light of a circular aperture (photutils exact
  aperture photometry, bisection), the background the sigma-clipped median of an annulus;
- eccentricity: from the second moments of the background-subtracted cutout (photutils
  data_properties), for elongated fields.

Prints, per file, the stars compared and the median and 90th-percentile relative
differences, and the count of stars beyond 5 % (FWHM, HFD) or 0.05 (eccentricity).

Usage: python -I oracle.py DIR   (numpy, astropy, photutils; a venv under CARGO_TARGET_DIR)
"""

import sys
from pathlib import Path

import numpy as np
from astropy.io import fits
from astropy.modeling import fitting, models
from astropy.stats import sigma_clipped_stats
from photutils.aperture import CircularAnnulus, CircularAperture, aperture_photometry
from photutils.morphology import data_properties

MAX_STARS = 200


def load(path):
    data = fits.getdata(path).astype(np.float64)
    if data.ndim > 2:
        data = data[0]
    return data


def stars(tsv):
    rows = np.genfromtxt(tsv, delimiter="\t", names=True)
    return np.atleast_1d(rows)


def half_flux_diameter(data, x, y, radius):
    ann = CircularAnnulus((x, y), r_in=radius + 2, r_out=radius + 6)
    mask = ann.to_mask(method="center")
    vals = mask.get_values(data)
    _, bg, _ = sigma_clipped_stats(vals, sigma=3.0)
    total = aperture_photometry(data - bg, CircularAperture((x, y), r=radius))["aperture_sum"][0]
    if total <= 0:
        return np.nan
    lo, hi = 0.05, radius
    for _ in range(40):
        mid = 0.5 * (lo + hi)
        f = aperture_photometry(data - bg, CircularAperture((x, y), r=mid))["aperture_sum"][0]
        if f < total / 2:
            lo = mid
        else:
            hi = mid
    return lo + hi  # 2 * radius


def moffat_fwhm(data, x, y, fwhm0):
    r = int(max(6, round(2.5 * fwhm0)))
    xi, yi = int(round(x)), int(round(y))
    if xi - r < 0 or yi - r < 0 or xi + r >= data.shape[1] or yi + r >= data.shape[0]:
        return np.nan
    cut = data[yi - r : yi + r + 1, xi - r : xi + r + 1]
    yy, xx = np.mgrid[yi - r : yi + r + 1, xi - r : xi + r + 1]
    bg = np.median(np.concatenate([cut[0], cut[-1], cut[:, 0], cut[:, -1]]))
    amp = cut.max() - bg
    alpha0 = fwhm0 / (2 * np.sqrt(2 ** (1 / 3.0) - 1))
    m = models.Moffat2D(amplitude=amp, x_0=x, y_0=y, gamma=alpha0, alpha=3.0) + models.Const2D(bg)
    m[0].alpha.bounds = (1.01, 60)
    fit = fitting.LevMarLSQFitter()
    try:
        out = fit(m, xx, yy, cut, maxiter=400)
    except Exception:
        return np.nan
    return float(out[0].fwhm)


def moment_ecc(data, x, y, fwhm0, bg):
    r = int(max(5, round(2.0 * fwhm0)))
    xi, yi = int(round(x)), int(round(y))
    cut = data[yi - r : yi + r + 1, xi - r : xi + r + 1] - bg
    cut = np.where(cut > 0, cut, 0)
    p = data_properties(cut)
    return float(p.eccentricity.value if hasattr(p.eccentricity, "value") else p.eccentricity)


def summary(name, ours, theirs, tol, absolute=False):
    ours, theirs = np.asarray(ours), np.asarray(theirs)
    ok = np.isfinite(ours) & np.isfinite(theirs)
    if not ok.any():
        print(f"  {name}: no stars compared")
        return
    d = ours[ok] - theirs[ok]
    if not absolute:
        d = d / theirs[ok]
    a = np.abs(d)
    unit = "" if absolute else " (relative)"
    print(
        f"  {name}: {ok.sum()} stars, median {np.median(d):+.4f}, p90 |d| {np.percentile(a, 90):.4f}{unit},"
        f" beyond {tol}: {(a > tol).sum()}"
    )


def main(root):
    for tsv in sorted(Path(root).glob("*.stars.tsv")):
        fit_path = tsv.with_name(tsv.name.replace(".stars.tsv", ".fits"))
        data = load(fit_path)
        h = data.shape[0]
        st = stars(tsv)
        # Brightest first (as measured); our rows are displayed top-down: FITS rows bottom-up.
        st = st[np.argsort(-st["amp"])][:MAX_STARS]
        fw_o, fw_t, hf_o, hf_t, ec_o, ec_t = [], [], [], [], [], []
        for s in st:
            x, y = float(s["x"]), float(h - 1 - s["y"])
            fw_o.append(s["fwhm"])
            fw_t.append(moffat_fwhm(data, x, y, s["fwhm"]))
            radius = float(np.clip(round(5 * s["fwhm"] / 2.3548), 4, 32))
            hf_o.append(s["hfd"])
            hf_t.append(half_flux_diameter(data, x, y, radius))
            ec_o.append(s["ecc"])
            ec_t.append(moment_ecc(data, x, y, s["fwhm"], s["bg"]))
        print(f"{fit_path.name}: {len(st)} stars")
        summary("FWHM vs astropy Moffat2D", fw_o, fw_t, 0.05)
        summary("HFD vs photutils apertures", hf_o, hf_t, 0.05)
        summary("eccentricity vs photutils moments", ec_o, ec_t, 0.05, absolute=True)


if __name__ == "__main__":
    main(sys.argv[1])
