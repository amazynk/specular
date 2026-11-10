#!/usr/bin/env python3
"""Writes the FITS table, HEALPix, event-list and catalogue fixtures of specular-fits, with
their oracle values, into tests/data/tables/.

Fixtures (small, committed):
    alltypes.fits      BINTABLE of every TFORM type (L X B I J K A E D C M, P and Q arrays,
                       TSCAL / TZERO, unsigned offsets, TNULL, TDIM), an ASCII TABLE, a
                       second BINTABLE
    ring8.fits         HEALPix RING, Nside 8, implicit, I / Q / U, galactic
    nest16.fits        HEALPix NESTED, Nside 16, implicit, 1024 pixels per row, equatorial
    partial8.fits      HEALPix RING, Nside 8, explicit (PIXEL column), partial sky
    flatprob.fits      HEALPix NESTED, Nside 32, PROB / DISTMU (a flat GW sky map)
    moc.fits           multi-order UNIQ / PROBDENSITY / DISTMU (a LIGO multi-order sky map)
    chandra.fits       events: float X / Y, ENERGY in eV, column WCS
    xmm.fits           events: integer X / Y, PI in eV (TELESCOP XMM)
    cat.vot, cat2.vot  VOTable TABLEDATA and BINARY2 catalogues; cat.csv, cat.fits
Oracles (tests/data/tables/*.tsv), from astropy.io.fits, astropy-healpix, astropy.wcs,
numpy.histogram2d, astropy.io.votable and astropy.coordinates.

Run with a Python that has astropy, astropy-healpix and numpy (the lane used a venv under
target/):
    target/fitstables/venv/bin/python crates/formats/specular-fits/gen/table_fixtures.py
"""

import math
import pathlib

import numpy as np
from astropy import units as u
from astropy.coordinates import Angle
from astropy.io import fits
from astropy.io.votable import from_table, parse_single_table, writeto
from astropy.table import Table
from astropy.wcs import WCS
from astropy_healpix import HEALPix, healpy as hp
from astropy_healpix import level_ipix_to_uniq, uniq_to_level_ipix

OUT = pathlib.Path(__file__).resolve().parent.parent / "tests" / "data" / "tables"
OUT.mkdir(parents=True, exist_ok=True)
RNG = np.random.default_rng(20261011)


def num(v):
    """A value as text the Rust side parses exactly."""
    if isinstance(v, (bool, np.bool_)):
        return "T" if v else "F"
    if isinstance(v, (int, np.integer)):
        return str(int(v))
    if isinstance(v, (float, np.floating)):
        f = float(v)
        return "nan" if math.isnan(f) else repr(f)
    if isinstance(v, (complex, np.complexfloating)):
        return f"({num(v.real)}, {num(v.imag)})"
    if isinstance(v, (bytes, np.bytes_)):
        v = v.decode()
    return str(v).rstrip().replace("\t", " ").replace("\n", " ")


# --- every TFORM ---------------------------------------------------------------------------

def alltypes():
    n = 6
    cols = [
        fits.Column("L", "L", array=np.array([True, False, True, False, True, True])),
        fits.Column("X", "11X", array=RNG.integers(0, 2, (n, 11)).astype(bool)),
        fits.Column("B", "B", array=np.array([0, 1, 127, 128, 255, 9], np.uint8), null=9),
        fits.Column("I", "I", array=np.array([-32768, -1, 0, 1, 32767, 5], np.int16)),
        fits.Column("J", "J", array=np.array([-2**31, -1, 0, 7, 2**31 - 1, -99], np.int32), null=-99),
        fits.Column("K", "K", array=np.array([-2**63, -1, 0, 2**53 + 1, 2**63 - 1, 3], np.int64)),
        fits.Column("A", "8A", array=np.array(["", "a", "hello", "eight ch", "x y", "Zz"])),
        fits.Column("E", "E", array=np.array([0, -1.5, 3.25e-30, np.nan, np.inf, 1e30], np.float32)),
        fits.Column("D", "D", array=np.array([0, -1.5, 1e-300, np.nan, math.pi, -1e300])),
        fits.Column("C", "C", array=np.array([1 + 2j, -1j, 0, 3.5, 1e-3 + 1e3j, 2], np.complex64)),
        fits.Column("M", "M", array=np.array([1 + 2j, -1j, 0, 3.5, 1e-3 + 1e3j, math.e], np.complex128)),
        fits.Column("U16", "I", bzero=32768, array=np.array([0, 1, 32768, 65535, 100, 7], np.uint16)),
        fits.Column("U32", "J", bzero=2**31, array=np.array([0, 1, 2**31, 2**32 - 1, 100, 7], np.uint32)),
        fits.Column("U64", "K", bzero=2**63, array=np.array([0, 1, 2**63, 2**64 - 1, 100, 7], np.uint64)),
        # Stored raw; TSCAL / TZERO set on the header below.
        fits.Column("SCALED", "I", array=np.array([20, 22, 16, 221, 0, -26], np.int16)),
        fits.Column("VEC", "6E", dim="(3,2)", array=RNG.normal(size=(n, 2, 3)).astype(np.float32)),
        fits.Column("PJ", "PJ()", array=np.array([np.arange(k, dtype=np.int32) * 3 - 1 for k in range(n)], dtype=object)),
        fits.Column("QD", "QD()", array=np.array([RNG.normal(size=(k + 1) % 4) for k in range(n)], dtype=object)),
        fits.Column("PE", "PE()", array=np.array([RNG.normal(size=2 * k).astype(np.float32) for k in range(n)], dtype=object)),
        fits.Column("PI", "PI()", array=np.array([np.array([k, -k, 7][: k % 3 + 1], np.int16) for k in range(n)], dtype=object)),
    ]
    t1 = fits.BinTableHDU.from_columns(cols, name="ALL")
    k = t1.columns.names.index("SCALED") + 1
    t1.header.set(f"TSCAL{k}", 0.5, after=f"TFORM{k}")
    t1.header.set(f"TZERO{k}", -10.0, after=f"TSCAL{k}")
    a = fits.TableHDU.from_columns(
        [
            fits.Column("NAME", "A6", array=np.array(["M 31", "NGC 1", "x"])),
            fits.Column("N", "I5", array=np.array([1, -20, 300])),
            fits.Column("F", "F9.3", array=np.array([1.5, -2.25, 1000.125])),
            fits.Column("E", "E12.4", array=np.array([1.5e10, -2.5e-3, 0.0])),
            fits.Column("D", "D20.12", array=np.array([math.pi, -math.e, 1e100])),
        ],
        name="ASCII",
    )
    t3 = fits.BinTableHDU.from_columns(
        [fits.Column("V", "J", array=np.arange(4, dtype=np.int32))], name="SECOND"
    )
    path = OUT / "alltypes.fits"
    fits.HDUList([fits.PrimaryHDU(), t1, a, t3]).writeto(path, overwrite=True)
    lines = []
    with fits.open(path) as f:
        for hi, hdu in enumerate(f):
            if not isinstance(hdu, (fits.BinTableHDU, fits.TableHDU)):
                continue
            d = hdu.data
            for ci, name in enumerate(d.columns.names):
                col = d.columns[ci]
                for r in range(len(d)):
                    v = d[name][r]
                    null = col.null
                    if isinstance(v, np.ndarray):
                        vals = [num(x) for x in np.ravel(v)]
                    else:
                        vals = [num(v)]
                    # TNULL: astropy keeps the stored integer; the oracle marks it.
                    if null is not None and not isinstance(hdu, fits.TableHDU) and vals == [num(null)]:
                        vals = ["null"]
                    lines.append("\t".join(["cell", str(hi), str(r), str(ci)] + vals))
    (OUT / "alltypes.tsv").write_text("\n".join(lines) + "\n")


# --- HEALPix -------------------------------------------------------------------------------

def sky_field(lon, lat, seed):
    r = np.random.default_rng(seed)
    a, b, c = r.normal(size=3)
    return (np.sin(np.radians(lat) * 3 + a) * np.cos(np.radians(lon) * 2 + b) + 0.1 * c).astype(np.float32)


def write_map(path, nside, order, cols, coordsys, extra=()):
    hdu = fits.BinTableHDU.from_columns(cols)
    h = hdu.header
    h["PIXTYPE"] = "HEALPIX"
    h["ORDERING"] = order
    if nside:
        h["NSIDE"] = nside
    if coordsys:
        h["COORDSYS"] = coordsys
    for k, v in extra:
        h[k] = v
    fits.HDUList([fits.PrimaryHDU(), hdu]).writeto(path, overwrite=True)


def healpix_maps():
    hp_lines = []
    # ring8: implicit I/Q/U, one pixel per row; a few UNSEEN.
    nside = 8
    hpx = HEALPix(nside, "ring")
    lon, lat = hpx.healpix_to_lonlat(np.arange(hpx.npix))
    lon, lat = lon.deg, lat.deg
    iq = [sky_field(lon, lat, s) for s in (1, 2, 3)]
    iq[0][[5, 100, 700]] = -1.6375e30
    write_map(OUT / "ring8.fits", nside, "RING",
              [fits.Column(n, "E", array=v, unit="K_CMB") for n, v in zip(("I_STOKES", "Q_STOKES", "U_STOKES"), iq)],
              "G", [("INDXSCHM", "IMPLICIT"), ("OBJECT", "FULLSKY")])
    # nest16: Planck-like 1024E cells.
    nside = 16
    hpx = HEALPix(nside, "nested")
    lon, lat = hpx.healpix_to_lonlat(np.arange(hpx.npix))
    v = sky_field(lon.deg, lat.deg, 4)
    write_map(OUT / "nest16.fits", nside, "NESTED",
              [fits.Column("TEMPERATURE", "1024E", array=v.reshape(-1, 1024), unit="K")],
              "C", [("INDXSCHM", "IMPLICIT")])
    # partial8: explicit pixels, ring.
    nside = 8
    hpx = HEALPix(nside, "ring")
    pix = np.sort(RNG.choice(hpx.npix, 300, replace=False)).astype(np.int32)
    lon, lat = hpx.healpix_to_lonlat(pix)
    write_map(OUT / "partial8.fits", nside, "RING",
              [fits.Column("PIXEL", "J", array=pix), fits.Column("SIGNAL", "E", array=sky_field(lon.deg, lat.deg, 5))],
              "C", [("INDXSCHM", "EXPLICIT"), ("OBJECT", "PARTIAL")])
    # flatprob: a Gaussian blob, normalised.
    nside = 32
    hpx = HEALPix(nside, "nested")
    lon, lat = hpx.healpix_to_lonlat(np.arange(hpx.npix))
    d = blob(lon.deg, lat.deg)
    prob = d / d.sum()
    write_map(OUT / "flatprob.fits", nside, "NESTED",
              [fits.Column("PROB", "D", array=prob, unit="pix-1"),
               fits.Column("DISTMU", "D", array=100 + lat.deg, unit="Mpc")],
              "C", [("INDXSCHM", "IMPLICIT"), ("OBJECT", "S0001")])
    # moc: refine the blob from order 2 to 6.
    uniq, dens = [], []
    def refine(level, ipix):
        nsd = 2**level
        lo, la = hp.pix2ang(nsd, ipix, nest=True, lonlat=True)
        dd = blob(np.atleast_1d(lo), np.atleast_1d(la))[0]
        if level < 6 and dd > 1e-3:
            for c in range(4):
                refine(level + 1, ipix * 4 + c)
        else:
            uniq.append(int(level_ipix_to_uniq(level, ipix)))
            dens.append(dd)
    for p in range(12 * 4**2):
        refine(2, p)
    uniq = np.array(uniq, np.int64)
    dens = np.array(dens)
    lev, ip = uniq_to_level_ipix(uniq)
    area = 4 * np.pi / (12 * 4.0**lev)
    dens = dens / (dens * area).sum()
    order = np.argsort(-dens, kind="stable")
    write_map(OUT / "moc.fits", None, "NUNIQ",
              [fits.Column("UNIQ", "K", array=uniq[order]),
               fits.Column("PROBDENSITY", "D", array=dens[order], unit="sr-1"),
               fits.Column("DISTMU", "D", array=np.full(len(uniq), 250.0), unit="Mpc")],
              "C", [("INDXSCHM", "EXPLICIT"), ("MOCORDER", 6)])
    hp_lines += oracle_maps()
    hp_lines += oracle_pixels()
    (OUT / "healpix.tsv").write_text("\n".join(hp_lines) + "\n")


def blob(lon, lat):
    # Two lobes, like an arc of a GW localisation.
    def g(l0, b0, s):
        c = np.cos(np.radians(lat)) * np.cos(np.radians(b0)) * np.cos(np.radians(lon - l0)) + np.sin(
            np.radians(lat)) * np.sin(np.radians(b0))
        return np.exp((np.clip(c, -1, 1) - 1) / np.radians(s) ** 2)
    return g(60, 20, 6) + 0.6 * g(80, 5, 4)


def mollweide_header(coordsys, w):
    h = w // 2
    r0 = 180 / math.pi
    c1, c2 = {"C": ("RA---MOL", "DEC--MOL"), "G": ("GLON-MOL", "GLAT-MOL")}[coordsys]
    hd = fits.Header()
    hd["NAXIS"] = 2
    hd["NAXIS1"] = w
    hd["NAXIS2"] = h
    hd["CTYPE1"], hd["CTYPE2"] = c1, c2
    hd["CRPIX1"], hd["CRPIX2"] = w / 2 + 0.5, h / 2 + 0.5
    hd["CDELT1"], hd["CDELT2"] = -4 * math.sqrt(2) * r0 / w, 2 * math.sqrt(2) * r0 / h
    hd["CRVAL1"] = hd["CRVAL2"] = 0.0
    return hd


def oracle_maps():
    """Mollweide raster samples (from astropy.wcs + astropy-healpix) and credible levels."""
    lines = []
    for name, field, w in [("ring8.fits", 0, 512), ("ring8.fits", 2, 512), ("nest16.fits", 0, 512),
                           ("partial8.fits", 0, 512), ("flatprob.fits", 0, 512), ("moc.fits", 0, 1024)]:
        with fits.open(OUT / name) as f:
            hdu = f[1]
            h = hdu.header
            d = hdu.data
            coordsys = h.get("COORDSYS", "C")
            wcs = WCS(mollweide_header(coordsys, w))
            ii, jj = np.meshgrid(np.arange(w), np.arange(w // 2))
            ii, jj = ii.ravel(), jj.ravel()
            # Display row j is FITS row h - j.
            world = wcs.all_pix2world(ii + 1.0, (w // 2) - jj + 0.0, 1)
            ok = np.isfinite(world[0]) & np.isfinite(world[1])
            # Inside the ellipse only.
            s2 = math.sqrt(2)
            x = (ii + 0.5 - w / 2) * (4 * s2 / w)
            y = (w / 4 - (jj + 0.5)) * (2 * s2 / (w // 2))
            ok &= (x * x / 8 + y * y / 2) <= 1
            sel = np.flatnonzero(ok)[:: max(1, ok.sum() // 800)]
            lon = world[0][sel] * u.deg
            lat = world[1][sel] * u.deg
            if h["ORDERING"] == "NUNIQ":
                uniq = d["UNIQ"]
                lev, ip = uniq_to_level_ipix(uniq)
                mx = lev.max()
                start = ip * 4 ** (mx - lev)
                order = np.argsort(start)
                p = HEALPix(2**int(mx), "nested").lonlat_to_healpix(lon, lat)
                k = np.searchsorted(start[order], p, side="right") - 1
                vals = d.field(field + 1)[order][k]
            else:
                nside = h["NSIDE"]
                hpx = HEALPix(nside, "nested" if h["ORDERING"].startswith("NEST") else "ring")
                p = hpx.lonlat_to_healpix(lon, lat)
                if h.get("INDXSCHM") == "EXPLICIT":
                    pix = d["PIXEL"]
                    col = d.field(field + 1)
                    lookup = dict(zip(pix.tolist(), col.tolist()))
                    vals = np.array([lookup.get(int(q), np.nan) for q in p])
                else:
                    flat = np.ravel(d.field(field))
                    vals = flat[p].astype(np.float64)
            vals = np.where(np.abs(vals - (-1.6375e30)) < 1e25, np.nan, vals)
            for i, j, v in zip(ii[sel], jj[sel], vals):
                lines.append(f"mol\t{name}\t{field}\t{w}\t{i}\t{j}\t{num(np.float32(v))}")
            if name in ("flatprob.fits", "moc.fits"):
                if h["ORDERING"] == "NUNIQ":
                    lev, _ = uniq_to_level_ipix(d["UNIQ"])
                    dens = d["PROBDENSITY"]
                    area = 4 * np.pi / (12 * 4.0**lev)
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


def oracle_pixels():
    lines = []
    r = np.random.default_rng(7)
    for nside in (1, 2, 8, 64, 1024, 2**20, 2**29):
        lon = r.uniform(0, 360, 200)
        lat = np.degrees(np.arcsin(r.uniform(-1, 1, 200)))
        # Near the poles and on the cap boundary.
        lat[:20] = 90 - 10.0 ** r.uniform(-9, 0, 20)
        lat[20:40] = -lat[:20]
        lat[40:50] = np.degrees(np.arcsin(2 / 3)) + r.normal(0, 1e-7, 10)
        theta = np.radians(90 - lat)
        phi = np.radians(lon)
        for nest in (False, True):
            p = hp.ang2pix(nside, theta, phi, nest=nest)
            for t, f, q in zip(theta, phi, p):
                lines.append(f"ang2pix\t{nside}\t{'nest' if nest else 'ring'}\t{float(t)!r}\t{float(f)!r}\t{q}")
            pix = r.integers(0, 12 * nside * nside, 100, dtype=np.int64)
            pix[:4] = [0, 12 * nside * nside - 1, 2 * nside * (nside - 1), 6 * nside * nside]
            t2, f2 = hp.pix2ang(nside, pix, nest=nest)
            for q, t, f in zip(pix, t2, f2):
                lines.append(f"pix2ang\t{nside}\t{'nest' if nest else 'ring'}\t{q}\t{float(t)!r}\t{float(f)!r}")
    lev = r.integers(0, 30, 200)
    ip = np.array([r.integers(0, 12 * 4**int(l)) for l in lev], np.int64)
    for l, q, un in zip(lev, ip, level_ipix_to_uniq(lev, ip)):
        lines.append(f"uniq\t{l}\t{q}\t{un}")
    return lines


# --- events --------------------------------------------------------------------------------

def events():
    lines = []
    n = 6000
    # Chandra-like: float sky pixels around 4096.5, ENERGY in eV.
    x = np.concatenate([RNG.normal(4096.5, 40, n // 2), RNG.uniform(3800, 4400, n // 2)]).astype(np.float32)
    y = np.concatenate([RNG.normal(4100.0, 25, n // 2), RNG.uniform(3900, 4300, n // 2)]).astype(np.float32)
    e = RNG.uniform(300, 9000, n).astype(np.float32)
    cols = [
        fits.Column("TIME", "D", array=np.sort(RNG.uniform(0, 1e4, n)), unit="s"),
        fits.Column("X", "E", array=x, unit="pixel"),
        fits.Column("Y", "E", array=y, unit="pixel"),
        fits.Column("ENERGY", "E", array=e, unit="eV"),
        fits.Column("PI", "J", array=(e / 14.6).astype(np.int32), unit="chan"),
    ]
    t = fits.BinTableHDU.from_columns(cols, name="EVENTS")
    h = t.header
    h["TELESCOP"] = "CHANDRA"
    h["INSTRUME"] = "ACIS"
    h["HDUCLAS1"] = "EVENTS"
    h["RADESYS"] = "ICRS"
    h["EQUINOX"] = 2000.0
    for k, (crval, cdelt, ct) in {2: (83.63308, -1.366666e-4, "RA---TAN"), 3: (22.0145, 1.366666e-4, "DEC--TAN")}.items():
        h[f"TCTYP{k}"] = ct
        h[f"TCRVL{k}"] = crval
        h[f"TCRPX{k}"] = 4096.5
        h[f"TCDLT{k}"] = cdelt
        h[f"TCUNI{k}"] = "deg"
        h[f"TLMIN{k}"] = 0.5
        h[f"TLMAX{k}"] = 8192.5
    fits.HDUList([fits.PrimaryHDU(), t]).writeto(OUT / "chandra.fits", overwrite=True)
    lines += oracle_events("chandra.fits", ("ENERGY", 1.0))
    # XMM-like: integer sky pixels (0.05"), PI in eV.
    x = np.round(RNG.normal(25000, 3000, n)).astype(np.int32)
    y = np.round(RNG.normal(26000, 2500, n)).astype(np.int32)
    pi = RNG.integers(150, 12000, n).astype(np.int32)
    cols = [
        fits.Column("TIME", "D", array=np.sort(RNG.uniform(0, 1e4, n))),
        fits.Column("X", "J", array=x),
        fits.Column("Y", "J", array=y),
        fits.Column("PI", "J", array=pi),
    ]
    t = fits.BinTableHDU.from_columns(cols, name="EVENTS")
    h = t.header
    h["TELESCOP"] = "XMM"
    h["INSTRUME"] = "EPN"
    h["RADECSYS"] = "FK5"
    h["EQUINOX"] = 2000.0
    for k, (crval, cdelt, ct) in {2: (10.68, -1.388888e-5, "RA---TAN"), 3: (41.27, 1.388888e-5, "DEC--TAN")}.items():
        h[f"TCTYP{k}"] = ct
        h[f"TCRVL{k}"] = crval
        h[f"TCRPX{k}"] = 25921.0
        h[f"TCDLT{k}"] = cdelt
        h[f"TLMIN{k}"] = 3649
        h[f"TLMAX{k}"] = 48106
    fits.HDUList([fits.PrimaryHDU(), t]).writeto(OUT / "xmm.fits", overwrite=True)
    lines += oracle_events("xmm.fits", ("PI", 1.0))
    (OUT / "events.tsv").write_text("\n".join(lines) + "\n")


def event_grid(h, d, xcol, ycol):
    """The base bin and edges, as specular-fits documents them."""
    xs, ys = d[xcol].astype(np.float64), d[ycol].astype(np.float64)
    ext = max(xs.max() - xs.min(), ys.max() - ys.min(), 1.0)
    base = max(2.0 ** math.ceil(math.log2(ext / 4096)), 1.0)
    def origin(c, mn):
        t = h.get(f"TLMIN{c}")
        return t + math.floor((mn - t) / base) * base if t is not None and t <= mn else math.floor(mn / base) * base
    ci = d.columns.names.index(xcol) + 1
    cj = d.columns.names.index(ycol) + 1
    x0, y0 = origin(ci, xs.min()), origin(cj, ys.min())
    w = int(math.floor((xs.max() - x0) / base)) + 1
    hh = int(math.floor((ys.max() - y0) / base)) + 1
    return xs, ys, base, x0, y0, w, hh, ci, cj


def oracle_events(name, energy):
    lines = []
    with fits.open(OUT / name) as f:
        hdu = f[1]
        h, d = hdu.header, hdu.data
        xs, ys, base, x0, y0, w, hh, ci, cj = event_grid(h, d, "X", "Y")
        lines.append(f"grid\t{name}\t{base!r}\t{x0!r}\t{y0!r}\t{w}\t{hh}")
        ev = d[energy[0]].astype(np.float64) * energy[1]
        for band, (lo, hi) in enumerate([(500, 1200), (1200, 2000), (2000, 7000), (None, None)]):
            sel = np.ones(len(xs), bool) if lo is None else (ev >= lo) & (ev < hi)
            for fi, k in enumerate((1, 2, 4, 8)):
                b = base * k
                cw, ch = -(-w // k), -(-hh // k)
                hist, _, _ = np.histogram2d(xs[sel], ys[sel], bins=[cw, ch],
                                            range=[[x0, x0 + cw * b], [y0, y0 + ch * b]])
                # Every bin of two binnings; a fingerprint of the others.
                if (fi, band) in ((0, 3), (1, 0)):
                    for i, j in np.argwhere(hist > 0):
                        lines.append(f"bin\t{name}\t{fi}\t{band}\t{i}\t{j}\t{int(hist[i, j])}")
                ii, jj = np.indices(hist.shape)
                fp = int((hist.astype(np.int64) * ((ii * 7919 + jj * 104729) % 1000003)).sum())
                lines.append(f"hist\t{name}\t{fi}\t{band}\t{int(hist.sum())}\t{fp}")
        # The event-pixel WCS (astropy, from the column keywords) at bin centres.
        wcs = WCS(h, keysel=["pixel"], colsel=[ci, cj])
        for i, j in [(0, 0), (w - 1, hh - 1), (w // 2, hh // 3), (w // 5, hh - 2)]:
            px = x0 + (i + 0.5) * base
            py = y0 + (j + 0.5) * base
            ra, dec = wcs.all_pix2world(px, py, 1)
            lines.append(f"wcs\t{name}\t{i}\t{j}\t{float(ra)!r}\t{float(dec)!r}")
    return lines


# --- catalogues ----------------------------------------------------------------------------

def catalogues():
    n = 40
    ra = RNG.uniform(83.5, 83.8, n)
    dec = RNG.uniform(21.9, 22.1, n)
    t = Table()
    t["id"] = [f"src{k}" for k in range(n)]
    t["RA_ICRS"] = ra * u.deg
    t["DE_ICRS"] = dec * u.deg
    t["Gmag"] = RNG.uniform(10, 20, n).astype(np.float32) * u.mag
    t["flag"] = RNG.integers(0, 2, n).astype(bool)
    t["RA_ICRS"].meta = {}
    vt = from_table(t)
    tab = vt.get_first_table()
    tab.get_field_by_id("RA_ICRS").ucd = "pos.eq.ra;meta.main"
    tab.get_field_by_id("DE_ICRS").ucd = "pos.eq.dec;meta.main"
    writeto(vt, str(OUT / "cat.vot"))
    tab.format = "binary2"
    writeto(vt, str(OUT / "cat2.vot"))
    # CSV with sexagesimal positions.
    rows = ["name,RA,Dec,mag"]
    for k in range(n):
        rows.append(f"s{k},{Angle(ra[k], u.deg).to_string(u.hour, sep=':', precision=4)},"
                    f"{Angle(dec[k], u.deg).to_string(u.deg, sep=':', precision=3, alwayssign=True)},{k}")
    (OUT / "cat.csv").write_text("\n".join(rows) + "\n")
    fits.HDUList([fits.PrimaryHDU(), fits.BinTableHDU(t, name="CAT")]).writeto(OUT / "cat.fits", overwrite=True)
    lines = []
    for name in ("cat.vot", "cat2.vot", "cat.fits"):
        tt = parse_single_table(str(OUT / name)).to_table() if name.endswith(".vot") else Table.read(OUT / name)
        for r in tt:
            lines.append(f"pos\t{name}\t{float(r['RA_ICRS'])!r}\t{float(r['DE_ICRS'])!r}")
    for k in range(n):
        line = rows[k + 1].split(",")
        lines.append(f"pos\tcat.csv\t{float(Angle(line[1], u.hour).deg)!r}\t{float(Angle(line[2], u.deg).deg)!r}")
    (OUT / "catalogue.tsv").write_text("\n".join(lines) + "\n")


if __name__ == "__main__":
    alltypes()
    healpix_maps()
    events()
    catalogues()
    print("wrote", OUT)
