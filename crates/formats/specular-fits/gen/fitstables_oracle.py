#!/usr/bin/env python3
"""Oracle values for the FITS table corpus (tools/corpus/fetch-fitstables.sh): writes
expected.tsv into the corpus directory given as the only argument.

Lines (tab-separated), checked by tests/tables_corpus.rs:
    cell  FILE HDU ROW COL VALUE...      astropy.io.fits table cells (sampled rows)
    mol   FILE FIELD W I J VALUE         Mollweide raster samples (astropy.wcs + astropy-healpix)
    credible FILE LEVEL THRESHOLD        credible-region density thresholds (numpy cumsum)
    grid  FILE BASE X0 Y0 W H            event image grid
    hist  FILE FACTOR BAND SUM PRINT     numpy.histogram2d count and fingerprint per binning
    wcs   FILE I J RA DEC                event image WCS at bin centres (astropy.wcs)
    pos   FILE RA DEC                    catalogue positions (astropy.io.votable)

Run with a Python that has astropy, astropy-healpix and numpy, as `python -I`.
"""

import math
import pathlib
import sys

import numpy as np
from astropy import units as u
from astropy.io import fits
from astropy.io.votable import parse
from astropy.wcs import WCS
from astropy_healpix import HEALPix, uniq_to_level_ipix

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from table_fixtures import mollweide_header, num  # noqa: E402

DIR = pathlib.Path(sys.argv[1])
MOL_WIDTH = 1024


def cells(name, rows_per_hdu=6):
    lines = []
    with fits.open(DIR / name) as f:
        for hi, hdu in enumerate(f):
            if not isinstance(hdu, (fits.BinTableHDU, fits.TableHDU)) or len(hdu.data) == 0:
                continue
            d = hdu.data
            n = len(d)
            rows = sorted({0, n - 1, *np.linspace(0, n - 1, rows_per_hdu).astype(int).tolist()})
            for ci, cname in enumerate(d.columns.names):
                col = d.columns[ci]
                for r in rows:
                    v = d[cname][r]
                    vals = [num(x) for x in np.ravel(v)] if isinstance(v, np.ndarray) else [num(v)]
                    if col.null is not None and not isinstance(hdu, fits.TableHDU) and vals == [num(col.null)]:
                        vals = ["null"]
                    lines.append("\t".join(["cell", name, str(hi), str(r), str(ci)] + vals))
    return lines


def maps(name, fields):
    lines = []
    w = MOL_WIDTH
    with fits.open(DIR / name) as f:
        hdu = f[1]
        h, d = hdu.header, hdu.data
        coordsys = {"GALACTIC": "G"}.get(h.get("COORDSYS", "C"), h.get("COORDSYS", "C"))
        wcs = WCS(mollweide_header(coordsys, w))
        ii, jj = np.meshgrid(np.arange(w), np.arange(w // 2))
        ii, jj = ii.ravel(), jj.ravel()
        world = wcs.all_pix2world(ii + 1.0, (w // 2) - jj + 0.0, 1)
        ok = np.isfinite(world[0]) & np.isfinite(world[1])
        s2 = math.sqrt(2)
        x = (ii + 0.5 - w / 2) * (4 * s2 / w)
        y = (w / 4 - (jj + 0.5)) * (2 * s2 / (w // 2))
        ok &= (x * x / 8 + y * y / 2) <= 1
        sel = np.flatnonzero(ok)[:: max(1, ok.sum() // 1500)]
        lon = world[0][sel] * u.deg
        lat = world[1][sel] * u.deg
        for field in fields:
            if h["ORDERING"] == "NUNIQ":
                lev, ip = uniq_to_level_ipix(d["UNIQ"])
                mx = lev.max()
                start = ip * 4 ** (mx - lev)
                order = np.argsort(start)
                p = HEALPix(2 ** int(mx), "nested").lonlat_to_healpix(lon, lat)
                k = np.searchsorted(start[order], p, side="right") - 1
                vals = d.field(field + 1)[order][k]
            else:
                hpx = HEALPix(h["NSIDE"], "nested" if h["ORDERING"].startswith("NEST") else "ring")
                p = hpx.lonlat_to_healpix(lon, lat)
                vals = np.ravel(d.field(field))[p].astype(np.float64)
            # UNSEEN and non-finite values (BAYESTAR writes DISTMU = inf) are no data.
            vals = np.where(~np.isfinite(vals) | (np.abs(vals - (-1.6375e30)) < 1e25), np.nan, vals)
            for i, j, v in zip(ii[sel], jj[sel], vals):
                lines.append(f"mol\t{name}\t{field}\t{w}\t{i}\t{j}\t{num(np.float32(v))}")
        if "PROB" in d.columns.names or "PROBDENSITY" in d.columns.names:
            if h["ORDERING"] == "NUNIQ":
                lev, _ = uniq_to_level_ipix(d["UNIQ"])
                dens = d["PROBDENSITY"]
                area = 4 * np.pi / (12 * 4.0 ** lev)
            else:
                dens = d["PROB"]
                area = np.ones_like(dens)
            keep = dens > 0
            dens, area = dens[keep], area[keep]
            o = np.argsort(-dens, kind="stable")
            cum = np.cumsum(dens[o] * area[o])
            for level in (0.5, 0.9):
                k = np.searchsorted(cum, level * cum[-1], side="left")
                lines.append(f"credible\t{name}\t{level}\t{num(dens[o][k])}")
    return lines


def event_grid(h, d, xcol, ycol):
    """The base bin and edges, as specular-fits documents them; TNULL positions dropped and
    a string TLMINn (XMM SAS) read as a number."""
    ci = d.columns.names.index(xcol) + 1
    cj = d.columns.names.index(ycol) + 1
    xs, ys = d[xcol].astype(np.float64), d[ycol].astype(np.float64)
    ok = np.isfinite(xs) & np.isfinite(ys)
    for c, v in ((ci, xs), (cj, ys)):
        if f"TNULL{c}" in h:
            ok &= v != h[f"TNULL{c}"]
    ext = max(xs[ok].max() - xs[ok].min(), ys[ok].max() - ys[ok].min(), 1.0)
    base = max(2.0 ** math.ceil(math.log2(ext / 4096)), 1.0)

    def origin(c, mn):
        t = h.get(f"TLMIN{c}")
        t = float(t) if t is not None else None
        return t + math.floor((mn - t) / base) * base if t is not None and t <= mn else math.floor(mn / base) * base
    x0, y0 = origin(ci, xs[ok].min()), origin(cj, ys[ok].min())
    w = int(math.floor((xs[ok].max() - x0) / base)) + 1
    hh = int(math.floor((ys[ok].max() - y0) / base)) + 1
    return xs, ys, ok, base, x0, y0, w, hh, ci, cj


def events(name, xcol, ycol, energy, scale):
    lines = []
    with fits.open(DIR / name) as f:
        hdu = f[1]
        h, d = hdu.header, hdu.data
        xs, ys, ok, base, x0, y0, w, hh, ci, cj = event_grid(h, d, xcol, ycol)
        lines.append(f"grid\t{name}\t{base!r}\t{x0!r}\t{y0!r}\t{w}\t{hh}")
        ev = d[energy].astype(np.float64) * scale
        ec = d.columns.names.index(energy) + 1
        if f"TNULL{ec}" in h:
            ev[d[energy] == h[f"TNULL{ec}"]] = np.nan
        for band, (lo, hi) in enumerate([(500, 1200), (1200, 2000), (2000, 7000), (None, None)]):
            sel = ok.copy() if lo is None else ok & (ev >= lo) & (ev < hi)
            for fi, k in enumerate((1, 2, 4, 8)):
                b = base * k
                cw, ch = -(-w // k), -(-hh // k)
                hist, _, _ = np.histogram2d(xs[sel], ys[sel], bins=[cw, ch],
                                            range=[[x0, x0 + cw * b], [y0, y0 + ch * b]])
                ii, jj = np.indices(hist.shape)
                fp = int((hist.astype(np.int64) * ((ii * 7919 + jj * 104729) % 1000003)).sum())
                lines.append(f"hist\t{name}\t{fi}\t{band}\t{int(hist.sum())}\t{fp}")
        wcs = WCS(h, keysel=["pixel"], colsel=[ci, cj])
        for i, j in [(0, 0), (w - 1, hh - 1), (w // 2, hh // 2), (w // 3, hh // 5), (w // 5, hh - 2)]:
            ra, dec = wcs.all_pix2world(x0 + (i + 0.5) * base, y0 + (j + 0.5) * base, 1)
            lines.append(f"wcs\t{name}\t{i}\t{j}\t{float(ra)!r}\t{float(dec)!r}")
    return lines


def votable(name, ra, dec):
    for t in parse(str(DIR / name)).iter_tables():
        names = t.array.dtype.names
        if ra in names and dec in names:
            return [f"pos\t{name}\t{float(r[ra])!r}\t{float(r[dec])!r}" for r in t.array]
    raise SystemExit(f"{name}: no table with {ra}/{dec}")


if __name__ == "__main__":
    out = []
    for n in ("planck-freefree-0256.fits", "S200115j-multiorder.fits", "S200115j-flat.fits",
              "chandra-635-evt2.fits", "xmm-0123700101-m1.fits"):
        out += cells(n)
    out += maps("planck-freefree-0256.fits", [0, 3])
    out += maps("S200115j-multiorder.fits", [0, 1])
    out += maps("S200115j-flat.fits", [0, 1])
    out += events("chandra-635-evt2.fits", "x", "y", "energy", 1.0)
    out += events("xmm-0123700101-m1.fits", "X", "Y", "PI", 1.0)
    out += votable("vizier.vot", "_RAJ2000", "_DEJ2000")
    out += votable("gaia-binary2.vot", "alpha", "delta")
    (DIR / "expected.tsv").write_text("\n".join(out) + "\n")
    print(f"wrote {DIR / 'expected.tsv'}: {len(out)} lines")
