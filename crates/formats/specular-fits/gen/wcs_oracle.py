#!/usr/bin/env python3
"""Pins astropy.wcs (wcslib) pixel -> world values for specular-fits' WCS tests.

Writes tests/data/wcs_oracle.tsv:

    case  <name>
    card  <KEY>  <value as in a FITS card>
    pix   <p1,p2[,p3]>  <w1,w2[,w3]>        (FITS 1-based pixels; 'nan' where undefined)

Run with a Python that has astropy (the lane used a venv under target/):
    target/astrowcs/venv/bin/python crates/formats/specular-fits/gen/wcs_oracle.py
"""

import math
import pathlib
import sys

import astropy
import numpy as np
from astropy.io import fits
from astropy.wcs import WCS

OUT = pathlib.Path(__file__).resolve().parent.parent / "tests" / "data" / "wcs_oracle.tsv"


def cel(code, lon="RA", lat="DEC", crval=(83.633, 22.0145), cdelt=(-0.002, 0.002), size=(200, 160), **extra):
    lon_t = (lon + "-" * 4)[:4] + "-" + code
    lat_t = (lat + "-" * 4)[:4] + "-" + code
    h = {
        "NAXIS1": size[0],
        "NAXIS2": size[1],
        "CTYPE1": lon_t,
        "CTYPE2": lat_t,
        "CRVAL1": crval[0],
        "CRVAL2": crval[1],
        "CRPIX1": size[0] / 2 + 0.5,
        "CRPIX2": size[1] / 2 + 0.5,
        "CDELT1": cdelt[0],
        "CDELT2": cdelt[1],
    }
    h.update(extra)
    return h


CASES = {
    "tan": cel("TAN", RADESYS="ICRS"),
    "tan_cd": {
        **{k: v for k, v in cel("TAN").items() if not k.startswith("CDELT")},
        "CD1_1": -1.2e-4, "CD1_2": 3.1e-5, "CD2_1": 2.9e-5, "CD2_2": 1.25e-4,
        "RADESYS": "FK5", "EQUINOX": 2000.0,
    },
    "tan_pc": cel("TAN", PC1_1=0.866, PC1_2=-0.5, PC2_1=0.5, PC2_2=0.866),
    "tan_crota": cel("TAN", CROTA2=30.0),
    "tan_pole": cel("TAN", crval=(10.0, 89.5), cdelt=(-0.01, 0.01)),
    "tan_lonpole": cel("TAN", LONPOLE=170.0),
    "sin": cel("SIN", cdelt=(-0.3, 0.3)),
    "arc": cel("ARC", cdelt=(-0.5, 0.5)),
    "stg": cel("STG", cdelt=(-0.5, 0.5)),
    "zea": cel("ZEA", lon="GLON", lat="GLAT", crval=(120.0, -30.0), cdelt=(-0.8, 0.8)),
    "car": cel("CAR", lon="GLON", lat="GLAT", crval=(0.0, 0.0), cdelt=(-1.0, 1.0), size=(360, 180)),
    "car_offset": cel("CAR", crval=(150.0, 30.0), cdelt=(-0.05, 0.05)),
    "mer": cel("MER", crval=(40.0, 0.0), cdelt=(-0.2, 0.2)),
    "cea": cel("CEA", lon="ELON", lat="ELAT", crval=(0.0, 0.0), cdelt=(-0.5, 0.5)),
    "ait": cel("AIT", lon="GLON", lat="GLAT", crval=(0.0, 0.0), cdelt=(-0.9, 0.9), size=(400, 200)),
    "sip": {
        **{k: v for k, v in cel("TAN").items() if not k.startswith("CDELT")},
        "CTYPE1": "RA---TAN-SIP", "CTYPE2": "DEC--TAN-SIP",
        "CD1_1": -2.8e-4, "CD1_2": 1.0e-6, "CD2_1": 1.2e-6, "CD2_2": 2.8e-4,
        "A_ORDER": 2, "A_0_2": 2.1e-6, "A_1_1": -3.0e-6, "A_2_0": 4.4e-6,
        "B_ORDER": 2, "B_0_2": -1.5e-6, "B_1_1": 2.6e-6, "B_2_0": 1.1e-6,
    },
    "cube": {
        **cel("TAN"),
        "NAXIS3": 12, "CTYPE3": "FREQ", "CUNIT3": "Hz",
        "CRVAL3": 1.4204e9, "CRPIX3": 1.0, "CDELT3": 2.5e5,
    },
    # Paper II projections not above, with parameters where they take them.
    "azp": cel("AZP", cdelt=(-0.3, 0.3), PV2_1=2.0, PV2_2=30.0),
    "szp": cel("SZP", cdelt=(-0.3, 0.3), PV2_1=2.0, PV2_2=180.0, PV2_3=60.0),
    "sin_slant": cel("SIN", cdelt=(-0.25, 0.25), PV2_1=0.1, PV2_2=-0.2),
    "zpn": cel("ZPN", cdelt=(-0.5, 0.5), PV2_0=0.02, PV2_1=0.98, PV2_3=-0.04),
    "air": cel("AIR", cdelt=(-0.5, 0.5), PV2_1=45.0),
    "ncp": cel("NCP", crval=(200.0, 60.0), cdelt=(-0.1, 0.1)),
    "cyp": cel("CYP", lon="GLON", lat="GLAT", crval=(30.0, 10.0), cdelt=(-0.8, 0.8), PV2_1=1.0, PV2_2=0.7071),
    "cea_l": cel("CEA", crval=(10.0, 0.0), cdelt=(-0.6, 0.6), PV2_1=0.75),
    "sfl": cel("SFL", crval=(0.0, 0.0), cdelt=(-0.9, 0.9), size=(400, 200)),
    "gls": cel("GLS", crval=(60.0, 20.0), cdelt=(-0.3, 0.3)),
    "par": cel("PAR", lon="GLON", lat="GLAT", crval=(0.0, 0.0), cdelt=(-0.9, 0.9), size=(400, 200)),
    "mol": cel("MOL", lon="GLON", lat="GLAT", crval=(0.0, 0.0), cdelt=(-0.9, 0.9), size=(400, 200)),
    "ait_off": cel("AIT", crval=(266.4, -28.9), cdelt=(-0.4, 0.4)),
    "cop": cel("COP", crval=(30.0, 40.0), cdelt=(-0.5, 0.5), PV2_1=45.0, PV2_2=25.0),
    "coe": cel("COE", crval=(30.0, 40.0), cdelt=(-0.5, 0.5), PV2_1=45.0, PV2_2=25.0),
    "cod": cel("COD", crval=(30.0, 40.0), cdelt=(-0.5, 0.5), PV2_1=45.0, PV2_2=25.0),
    "coo": cel("COO", crval=(30.0, 40.0), cdelt=(-0.5, 0.5), PV2_1=45.0, PV2_2=25.0),
    "coe_south": cel("COE", crval=(30.0, -40.0), cdelt=(-0.5, 0.5), PV2_1=-45.0, PV2_2=25.0),
    "bon": cel("BON", crval=(0.0, 0.0), cdelt=(-1.0, 1.0), size=(300, 200), PV2_1=45.0),
    "pco": cel("PCO", crval=(0.0, 0.0), cdelt=(-0.8, 0.8), size=(300, 200)),
    "tsc": cel("TSC", lon="GLON", lat="GLAT", crval=(0.0, 0.0), cdelt=(-1.0, 1.0), size=(360, 270)),
    "csc": cel("CSC", lon="GLON", lat="GLAT", crval=(0.0, 0.0), cdelt=(-1.0, 1.0), size=(360, 270)),
    "qsc": cel("QSC", lon="GLON", lat="GLAT", crval=(0.0, 0.0), cdelt=(-1.0, 1.0), size=(360, 270)),
    "qsc_rot": cel("QSC", crval=(120.0, 35.0), cdelt=(-0.3, 0.3)),
    "hpx": cel("HPX", lon="GLON", lat="GLAT", crval=(0.0, 0.0), cdelt=(-1.0, 1.0), size=(360, 180)),
    "hpx_k4": cel("HPX", crval=(0.0, 0.0), cdelt=(-1.0, 1.0), size=(360, 180), PV2_1=4.0, PV2_2=4.0),
    "hpx_h3": cel("HPX", crval=(0.0, 0.0), cdelt=(-1.0, 1.0), size=(360, 180), PV2_1=3.0, PV2_2=2.0),
    "xph": cel("XPH", lon="GLON", lat="GLAT", crval=(0.0, 90.0), cdelt=(-1.0, 1.0), size=(260, 260)),
    # Fiducial point and pole handling.
    "car_fiducial": cel("CAR", crval=(40.0, 20.0), cdelt=(-0.5, 0.5), PV1_0=1, PV1_1=10.0, PV1_2=15.0),
    "cea_latpole": cel("CEA", crval=(40.0, 50.0), cdelt=(-0.2, 0.2), LONPOLE=20.0, LATPOLE=-30.0),
    "sin_fiducial": cel("SIN", crval=(40.0, 20.0), cdelt=(-0.1, 0.1), PV1_1=5.0, PV1_2=80.0),
    # Units, PC over CD, SIP without the suffix, TPV.
    "tan_arcsec": cel("TAN", cdelt=(-7.2, 7.2), CUNIT1="arcsec", CUNIT2="arcsec", CRVAL1=301320.0, CRVAL2=79200.0),
    "tan_pc_cd": cel("TAN", PC1_1=0.9, PC1_2=0.1, PC2_1=-0.1, PC2_2=0.9, CD1_1=-3e-4, CD2_2=3e-4),
    "sip_nosuffix": {
        **{k: v for k, v in cel("TAN").items() if not k.startswith("CDELT")},
        "CD1_1": -2.8e-4, "CD1_2": 0.0, "CD2_1": 0.0, "CD2_2": 2.8e-4,
        "A_ORDER": 2, "A_2_0": 4.4e-6, "B_ORDER": 2, "B_0_2": -1.5e-6,
    },
    "tpv": {
        **{k: v for k, v in cel("TPV", size=(400, 300)).items() if not k.startswith("CDELT")},
        "CD1_1": -7.3e-5, "CD1_2": 1.0e-7, "CD2_1": -2.0e-7, "CD2_2": 7.3e-5,
        "PV1_0": 1.2e-4, "PV1_1": 1.0012, "PV1_2": 3.1e-4, "PV1_4": -2.6e-3, "PV1_5": 1.7e-3,
        "PV1_6": 9.0e-4, "PV1_7": -1.1e-2, "PV1_8": 2.0e-4, "PV1_9": -9.5e-3, "PV1_10": 4.0e-4,
        "PV1_11": 3.0e-3,
        "PV2_0": -8.0e-5, "PV2_1": 0.9989, "PV2_2": -2.2e-4, "PV2_4": 1.4e-3, "PV2_5": -2.1e-3,
        "PV2_6": 6.0e-4, "PV2_7": -1.0e-2, "PV2_8": 3.0e-4, "PV2_9": -8.0e-3, "PV2_10": 2.0e-4,
        "PV2_12": 1.0e-3,
    },
    "cube_log": {
        **cel("TAN"),
        "NAXIS3": 12, "CTYPE3": "WAVE-LOG", "CUNIT3": "m",
        "CRVAL3": 5.0e-7, "CRPIX3": 3.0, "CDELT3": 1.0e-10,
    },
    # HST-style: SIP with D2IM and CPDIS lookup tables.
    "lookup": {
        **{k: v for k, v in cel("TAN", size=(256, 200)).items() if not k.startswith("CDELT")},
        "CTYPE1": "RA---TAN-SIP", "CTYPE2": "DEC--TAN-SIP",
        "CD1_1": -1.1e-5, "CD1_2": 2.0e-7, "CD2_1": 3.0e-7, "CD2_2": 1.1e-5,
        "A_ORDER": 2, "A_2_0": 3.0e-6, "A_1_1": -1.0e-6, "B_ORDER": 2, "B_0_2": 2.0e-6,
        "D2IMDIS1": "LOOKUP", "D2IM1": ["EXTVER: 1", "NAXES: 1", "AXIS.1: 1"],
        "D2IMDIS2": "LOOKUP", "D2IM2": ["EXTVER: 2", "NAXES: 1", "AXIS.2: 1"],
        "CPDIS1": "LOOKUP", "DP1": ["NAXES: 2", "AXIS.1: 1", "AXIS.2: 2", "EXTVER: 1"],
        "CPDIS2": "LOOKUP", "DP2": ["NAXES: 2", "AXIS.1: 1", "AXIS.2: 2", "EXTVER: 2"],
    },
    "linear": {
        "NAXIS1": 50, "NAXIS2": 40, "CTYPE1": "WAVE", "CTYPE2": "OFFSET",
        "CRVAL1": 5000.0, "CRPIX1": 1.0, "CDELT1": 2.0,
        "CRVAL2": -10.0, "CRPIX2": 20.0, "CDELT2": 0.5,
    },
    # RA / DEC with no projection code: linear axes in wcslib.
    "plain_radec": {
        "NAXIS1": 30, "NAXIS2": 20, "CTYPE1": "RA", "CTYPE2": "DEC",
        "CRVAL1": 83.8, "CRPIX1": 1.0, "CDELT1": -0.01,
        "CRVAL2": -5.4, "CRPIX2": 1.0, "CDELT2": 0.01,
    },
    # Pre-2012 SCAMP: -TAN with TPV's polynomial is TPV.
    "scamp_tan": {
        **{k: v for k, v in cel("TAN").items() if not k.startswith("CDELT")},
        "CD1_1": -7.3e-5, "CD1_2": 0.0, "CD2_1": 0.0, "CD2_2": 7.3e-5,
        "PV1_0": 0.0, "PV1_1": 1.0, "PV1_2": 0.0, "PV1_4": 2.0e-3, "PV1_5": -1.5e-3, "PV1_7": -9.0e-3,
        "PV2_0": 0.0, "PV2_1": 1.0, "PV2_2": 0.0, "PV2_4": -1.2e-3, "PV2_5": 1.9e-3, "PV2_10": 8.0e-3,
    },
    # -TAN-SIP with SCAMP's PVs beside it: the PVs are dropped (astropy's fix).
    "sip_with_pv": {
        **{k: v for k, v in cel("TAN").items() if not k.startswith("CDELT")},
        "CTYPE1": "RA---TAN-SIP", "CTYPE2": "DEC--TAN-SIP",
        "CD1_1": -2.8e-4, "CD1_2": 0.0, "CD2_1": 0.0, "CD2_2": 2.8e-4,
        "LONPOLE": 180.0, "LATPOLE": 0.0,
        "A_ORDER": 2, "A_2_0": 3.0e-6, "B_ORDER": 2, "B_0_2": 2.0e-6,
        "PV1_0": 0.0, "PV1_1": 1.0, "PV1_2": 0.0, "PV1_4": 8.1e-4, "PV1_5": 6.1e-4,
    },
    # Digitized Sky Survey plate solution, with the r² / r⁴ terms switched on.
    "dss": {
        "NAXIS1": 100, "NAXIS2": 100,
        "PLTRAH": 14, "PLTRAM": 37, "PLTRAS": 46.88253, "PLTDECSN": "-",
        "PLTDECD": 60, "PLTDECM": 12, "PLTDECS": 59.28761, "PLTSCALE": 67.2,
        "CNPIX1": 8860, "CNPIX2": 1708, "XPIXELSZ": 25.28445, "YPIXELSZ": 25.28445,
        "PPO3": 176616.98338150, "PPO6": 177193.56115606,
        **{f"AMDX{i + 1}": v for i, v in enumerate([
            67.226158492105, -0.23024540155842, 1.0901493794544, 1.1835555135256e-06,
            5.5613287069447e-06, -3.8419945268889e-05, 4.0e-05, 2.2770722559048e-06,
            1.0335160125448e-07, 2.3321693523829e-06, 1.8565977703272e-07, 3.0e-06, 2.0e-09])},
        **{f"AMDY{i + 1}": v for i, v in enumerate([
            67.23331978069, 0.22606735154764, -0.52719954923422, -7.3037555803828e-06,
            1.9544740562734e-05, -5.6277730545338e-06, -3.0e-05, 2.2086510085905e-06,
            1.0292330494988e-08, 2.1738798605975e-06, -1.5811855244111e-08, -2.0e-06, 1.0e-09])},
    },
}


# Lookup tables of the "lookup" case: (EXTNAME, EXTVER) -> (data rows in FITS order,
# CRPIX, CRVAL, CDELT).
def tables():
    j, i = np.mgrid[0:13, 0:17].astype(np.float64)
    t = {
        ("D2IMARR", 1): (np.sin(np.arange(64) / 5.0)[None, :] * 0.3, (1.0, 1.0), (0.0, 0.0), (4.0, 1.0)),
        ("D2IMARR", 2): ((np.cos(np.arange(50) / 4.0) * 0.2)[None, :], (1.0, 1.0), (0.0, 0.0), (4.0, 1.0)),
        ("WCSDVARR", 1): (0.05 * np.sin(i / 3.0) * np.cos(j / 2.0) + 0.01 * i, (1.0, 1.0), (0.0, 0.0), (16.0, 16.0)),
        ("WCSDVARR", 2): (0.04 * np.cos(i / 4.0 + j / 3.0) - 0.02 * j, (1.0, 1.0), (0.0, 0.0), (16.0, 16.0)),
    }
    # D2IM axis 2's table is a 1-D row too, along y (D2IM2.AXIS.2 = 1: transposed).
    return {k: (v[0].astype(np.float32), *v[1:]) for k, v in t.items()}


def fmt_value(v):
    if isinstance(v, str):
        return "'" + v + "'"
    if isinstance(v, bool):
        return "T" if v else "F"
    if isinstance(v, int):
        return str(v)
    return repr(float(v)).upper()


def fmt(x):
    return "nan" if not math.isfinite(x) else repr(float(x))


def main():
    lines = [f"# astropy {astropy.__version__} (wcslib) pixel -> world; regenerate with gen/wcs_oracle.py"]
    for name, cards in CASES.items():
        h = fits.Header()
        naxis = 3 if "NAXIS3" in cards else 2
        h["NAXIS"] = naxis
        for k, v in cards.items():
            for one in v if isinstance(v, list) else [v]:
                h.append((k, one))
        lines.append(f"case\t{name}")
        lines.append(f"card\tNAXIS\t{naxis}")
        for k, v in cards.items():
            for one in v if isinstance(v, list) else [v]:
                lines.append(f"card\t{k}\t{fmt_value(one)}")
        if name == "lookup":
            hdus = [fits.PrimaryHDU(header=h)]
            for (ext, ver), (data, crpix, crval, cdelt) in tables().items():
                e = fits.ImageHDU(data=data)
                e.header["EXTNAME"] = ext
                e.header["EXTVER"] = ver
                for a in (1, 2):
                    e.header[f"CRPIX{a}"] = crpix[a - 1]
                    e.header[f"CRVAL{a}"] = crval[a - 1]
                    e.header[f"CDELT{a}"] = cdelt[a - 1]
                hdus.append(e)
                ny, nx = data.shape
                lines.append(
                    f"table\t{ext}\t{ver}\t{nx}\t{ny}\t{crpix[0]},{crpix[1]}\t{crval[0]},{crval[1]}"
                    f"\t{cdelt[0]},{cdelt[1]}\t" + ",".join(repr(float(x)) for x in data.ravel())
                )
            hl = fits.HDUList(hdus)
            w = WCS(hdus[0].header, fobj=hl)
            assert w.det2im1 is not None and w.cpdis1 is not None and w.sip is not None
        else:
            try:
                w = WCS(h)
            except Exception as e:
                sys.exit(f"{name}: {e}")
        nx, ny = cards["NAXIS1"], cards["NAXIS2"]
        xs = np.linspace(1, nx, 6)
        ys = np.linspace(1, ny, 5)
        for x in xs:
            for y in ys:
                pix = [x, y] + ([1 + (int(x + y) % cards["NAXIS3"])] if naxis == 3 else [])
                world = w.all_pix2world(np.array([pix]), 1)[0]
                lines.append(
                    "pix\t" + ",".join(fmt(p) for p in pix) + "\t" + ",".join(fmt(v) for v in world)
                )
    OUT.write_text("\n".join(lines) + "\n")
    print(f"wrote {OUT}", file=sys.stderr)


def read_header(path):
    """The header (and HDU list, for lookup tables) of a corpus file, or None."""
    name = path.name.lower()
    if name.endswith(".hdr"):
        raw = path.read_bytes()
        for sep in ("", "\n"):
            try:
                return fits.Header.fromstring(raw.decode("latin-1"), sep=sep), None
            except Exception:
                pass
        return None
    hl = fits.open(path)
    for hdu in hl:
        h = hdu.header
        if any(k.startswith("CTYPE") for k in h):
            return h, hl
    return None


def sized(h):
    """`h` with an image size: header-only files (NAXIS = 0, or none) get one covering twice
    CRPIX on each axis (at least 100 pixels). `None` for fewer than two WCS axes."""
    naxis = [h.get(f"NAXIS{i}") for i in range(1, h.get("NAXIS", 0) + 1)]
    if len(naxis) >= 2 and all(isinstance(n, int) and n >= 1 for n in naxis):
        return h
    n = h.get("WCSAXES") or max(
        (int(k[5:]) for k in h if k.startswith("CTYPE") and k[5:].isdigit()), default=0
    )
    if n < 2:
        return None
    h = h.copy()
    for k in [k for k in h if k == "NAXIS" or (k.startswith("NAXIS") and k[5:].isdigit())]:
        del h[k]
    h["NAXIS"] = n
    for i in range(1, n + 1):
        crpix = float(h.get(f"CRPIX{i}", 1.0))
        h[f"NAXIS{i}"] = int(min(8192, max(100, round(2 * crpix)))) if i <= 2 else 1
    return h


def corpus(directory):
    """astropy pixel -> world over a grid of every header in DIRECTORY (the astropy WCS test
    data fetched by tools/corpus/fetch-astrowcs.sh), written to DIRECTORY/expected.tsv. Cards
    are the raw value fields; files astropy itself refuses are listed as skipped."""
    import warnings

    warnings.simplefilter("ignore")
    lines = [f"# astropy {astropy.__version__} (wcslib) pixel -> world over the corpus"]
    done = skipped = 0
    for path in sorted(directory.iterdir()):
        if not path.name.lower().endswith((".hdr", ".fits", ".fits.gz", ".fit")):
            continue
        try:
            got = read_header(path)
            if got is None:
                raise ValueError("no image header with CTYPE")
            h, hl = got
            h = sized(h)
            if h is None:
                raise ValueError("fewer than two WCS axes")
            naxis = [h[f"NAXIS{i}"] for i in range(1, h["NAXIS"] + 1)]
            w = WCS(h, fobj=hl)
            pts = []
            for x in np.linspace(1, naxis[0], 6):
                for y in np.linspace(1, naxis[1], 5):
                    pts.append([x, y] + [1.0] * (w.naxis - 2))
            world = w.all_pix2world(np.array(pts), 1)
            # wcslib puts spectral axes in SI (km/s -> m/s, nm -> m); Specular keeps the
            # header's unit, so astropy's values go back to it.
            from astropy import units as u

            for i in range(w.naxis):
                if i in (w.wcs.lng, w.wcs.lat):
                    continue
                orig = str(h.get(f"CUNIT{i + 1}", "")).strip()
                cur = str(w.wcs.cunit[i]).strip()
                if orig and cur and orig != cur:
                    world[:, i] *= u.Unit(cur).to(u.Unit(orig))
        except Exception as e:
            lines.append(f"# skipped {path.name}: {type(e).__name__}: {str(e).splitlines()[0][:120]}")
            skipped += 1
            continue
        lines.append(f"case\t{path.name}")
        for card in h.cards:
            image = card.image
            key = image[:8].strip()
            if image[8:10] != "= " or key in ("COMMENT", "HISTORY", "", "CONTINUE"):
                continue
            lines.append(f"card\t{key}\t{image[10:].rstrip()}")
        if hl is not None:
            for hdu in hl[1:]:
                ext = hdu.header.get("EXTNAME")
                if ext not in ("D2IMARR", "WCSDVARR"):
                    continue
                data = np.asarray(hdu.data, dtype=np.float64)
                if data.ndim == 1:
                    data = data[None, :]
                ny, nx = data.shape
                g = lambda k, a: float(hdu.header.get(f"{k}{a}", 1.0 if k != "CRVAL" else 0.0))
                lines.append(
                    f"table\t{ext}\t{hdu.header.get('EXTVER', 1)}\t{nx}\t{ny}"
                    f"\t{g('CRPIX', 1)},{g('CRPIX', 2)}\t{g('CRVAL', 1)},{g('CRVAL', 2)}"
                    f"\t{g('CDELT', 1)},{g('CDELT', 2)}\t" + ",".join(repr(float(x)) for x in data.ravel())
                )
        for p, v in zip(pts, world):
            lines.append("pix\t" + ",".join(fmt(c) for c in p) + "\t" + ",".join(fmt(c) for c in v))
        done += 1
    (directory / "expected.tsv").write_text("\n".join(lines) + "\n")
    print(f"wcs corpus: {done} files, {skipped} skipped -> {directory / 'expected.tsv'}", file=sys.stderr)


if __name__ == "__main__":
    if len(sys.argv) == 3 and sys.argv[1] == "corpus":
        corpus(pathlib.Path(sys.argv[2]))
    else:
        main()
