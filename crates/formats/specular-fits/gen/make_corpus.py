#!/usr/bin/env python3
"""FITS dev corpus for specular-fits (needs numpy and astropy; run by tools/corpus/fetch-sci.sh).

    make_corpus.py generate DIR   writes generated FITS files into DIR/generated
    make_corpus.py big DIR        writes DIR/generated/big_f32_4096.fits (64 MB, speed test)
    make_corpus.py stats DIR      astropy oracle: DIR/expected.tsv for every *.fits / *.fit /
                                  *.fts / *.fz below DIR (one line per image plane)

expected.tsv columns (tab-separated), planes in display order (top row first, i.e. FITS row
NAXIS2 first), plane = linear index over the axes past the second (axis 3 fastest):

    file hdu plane width height finite sum min max zlo zhi pixels

`pixels` is a space-separated list of x,y=value (display coordinates; 'nan' for blank).
zlo / zhi are astropy's ZScaleInterval limits ('-' when the plane is too large or empty).
"""

import math
import pathlib
import sys

import numpy as np
from astropy.io import fits
from astropy.visualization import ZScaleInterval

EXTS = (".fits", ".fit", ".fts", ".fz")
MAX_STATS_PLANES = 64
ZSCALE_MAX = 4096 * 4096


def wcs_cards(h, nx, ny, crval=(83.633, 22.0145), scale=1.0 / 3600):
    h["CTYPE1"] = "RA---TAN"
    h["CTYPE2"] = "DEC--TAN"
    h["CRVAL1"] = crval[0]
    h["CRVAL2"] = crval[1]
    h["CRPIX1"] = nx / 2 + 0.5
    h["CRPIX2"] = ny / 2 + 0.5
    h["CDELT1"] = -scale
    h["CDELT2"] = scale
    h["RADESYS"] = "ICRS"


def sky(rng, ny, nx, sky=1000.0, stars=40):
    """A star field: noisy sky plus Gaussian stars, float32."""
    y, x = np.mgrid[0:ny, 0:nx]
    img = rng.normal(sky, 12.0, (ny, nx))
    for _ in range(stars):
        cx, cy = rng.uniform(0, nx), rng.uniform(0, ny)
        amp = rng.uniform(200, 30000)
        s = rng.uniform(1.0, 3.0)
        img += amp * np.exp(-((x - cx) ** 2 + (y - cy) ** 2) / (2 * s * s))
    return img.astype(np.float32)


def generate(out):
    g = out / "generated"
    g.mkdir(parents=True, exist_ok=True)
    rng = np.random.default_rng(20261011)
    nx, ny = 300, 200
    field = sky(rng, ny, nx)

    def write(name, hdus):
        fits.HDUList(hdus).writeto(g / name, overwrite=True)

    # Every BITPIX, with WCS.
    for name, data in [
        ("u8.fits", np.clip(field / 120, 0, 255).astype(np.uint8)),
        ("i16.fits", np.clip(field - 1000, -32768, 32767).astype(np.int16)),
        ("i32.fits", (field * 10).astype(np.int32)),
        ("i64.fits", (field.astype(np.int64) * 100000)),
        ("f32.fits", field),
        ("f64.fits", field.astype(np.float64) / 7.0),
    ]:
        h = fits.PrimaryHDU(data)
        wcs_cards(h.header, nx, ny)
        h.header["OBJECT"] = "Generated star field"
        h.header["TELESCOP"] = "Specular test"
        write(name, [h])

    # Unsigned 16-bit (BZERO 32768), BSCALE, BLANK.
    u16 = np.clip(field * 2, 0, 65535).astype(np.uint16)
    write("u16.fits", [fits.PrimaryHDU(u16)])
    h = fits.PrimaryHDU(np.clip(field - 1000, -30000, 30000).astype(np.int16))
    h.header["BSCALE"] = 0.25
    h.header["BZERO"] = 100.0
    h.header["BLANK"] = -32768
    h.data[5:9, 10:20] = -32768
    write("i16_scaled_blank.fits", [h])
    nanf = field.copy()
    nanf[50:60, 50:70] = np.nan
    write("f32_nan.fits", [fits.PrimaryHDU(nanf)])

    # A long-string header (CONTINUE).
    h = fits.PrimaryHDU(field[:64, :64])
    h.header["HISTORY"] = "made by make_corpus.py"
    h.header["LONGSTR"] = "This value is long enough to need CONTINUE cards: " + "abcdefghij" * 12
    write("continue.fits", [h])

    # Cubes: spectral cube, and a 4-D hypercube.
    cube = np.stack([field[:64, :80] * (1 + 0.1 * k) for k in range(12)])
    h = fits.PrimaryHDU(cube)
    wcs_cards(h.header, 80, 64)
    h.header["CTYPE3"] = "FREQ"
    h.header["CUNIT3"] = "Hz"
    h.header["CRVAL3"] = 1.4204e9
    h.header["CRPIX3"] = 1.0
    h.header["CDELT3"] = 2.5e5
    write("cube_freq.fits", [h])
    hyper = rng.normal(0, 1, (2, 3, 32, 48)).astype(np.float32)
    write("hyper4d.fits", [fits.PrimaryHDU(hyper)])

    # Multi-extension: empty primary, SCI / ERR / DQ.
    sci = fits.ImageHDU(field, name="SCI")
    wcs_cards(sci.header, nx, ny)
    write(
        "mef.fits",
        [
            fits.PrimaryHDU(),
            sci,
            fits.ImageHDU(np.sqrt(np.abs(field)), name="ERR"),
            fits.ImageHDU((field > 5000).astype(np.int16), name="DQ"),
        ],
    )
    # Three filters as extensions (channel compositing).
    write(
        "rgb_ext.fits",
        [fits.PrimaryHDU()]
        + [fits.ImageHDU(field * f, name=n) for f, n in [(1.0, "R"), (0.8, "G"), (0.6, "B")]],
    )

    # Tile compression.
    i16 = np.clip(field - 1000, -32768, 32767).astype(np.int16)
    i32 = (field * 10).astype(np.int32)
    comp = [
        ("rice_i16.fits", i16, dict(compression_type="RICE_1")),
        ("rice_i32.fits", i32, dict(compression_type="RICE_1")),
        ("rice_u8.fits", np.clip(field / 120, 0, 255).astype(np.uint8), dict(compression_type="RICE_1")),
        ("rice_u16.fits", u16, dict(compression_type="RICE_1")),
        ("rice_f32_nodither.fits", field, dict(compression_type="RICE_1", quantize_method=-1)),
        ("rice_f32_dither1.fits", nanf, dict(compression_type="RICE_1", quantize_method=1, dither_seed=17)),
        ("rice_f32_dither2.fits", field * (field > 1100), dict(compression_type="RICE_1", quantize_method=2, dither_seed=3)),
        ("rice_tiles.fits", i16, dict(compression_type="RICE_1", tile_shape=(64, 50))),
        ("gzip1_i16.fits", i16, dict(compression_type="GZIP_1")),
        ("gzip1_f32_lossless.fits", field, dict(compression_type="GZIP_1", quantize_level=0.0)),
        ("gzip2_f32_lossless.fits", field, dict(compression_type="GZIP_2", quantize_level=0.0)),
        ("gzip2_i32.fits", i32, dict(compression_type="GZIP_2")),
        ("gzip1_f64_lossless.fits", field.astype(np.float64), dict(compression_type="GZIP_1", quantize_level=0.0)),
        ("hcompress_i16.fits", i16, dict(compression_type="HCOMPRESS_1")),
        ("hcompress_i32_lossy.fits", i32, dict(compression_type="HCOMPRESS_1", hcomp_scale=2.5)),
        ("hcompress_f32_smooth.fits", field, dict(compression_type="HCOMPRESS_1", hcomp_scale=2.0, hcomp_smooth=True)),
        ("plio_i16.fits", (field > 1500).astype(np.int16) * 3, dict(compression_type="PLIO_1")),
        ("nocompress_f32.fits", field, dict(compression_type="NOCOMPRESS")),
    ]
    for name, data, kw in comp:
        c = fits.CompImageHDU(data, **kw)
        if name.startswith("rice_f32"):
            wcs_cards(c.header, nx, ny)
        write(name, [fits.PrimaryHDU(), c])
    # A compressed cube (tiles one plane each) and an .fz name.
    write("rice_cube.fits", [fits.PrimaryHDU(), fits.CompImageHDU(cube.astype(np.float32), compression_type="RICE_1")])
    write("star.fits.fz", [fits.PrimaryHDU(), fits.CompImageHDU(field, compression_type="RICE_1")])


def big(out):
    g = out / "generated"
    g.mkdir(parents=True, exist_ok=True)
    rng = np.random.default_rng(4096)
    data = rng.normal(1000, 30, (4096, 4096)).astype(np.float32)
    fits.PrimaryHDU(data).writeto(g / "big_f32_4096.fits", overwrite=True)


def fmt(v):
    v = float(v)
    return "nan" if not math.isfinite(v) else repr(v)


def stats(out):
    lines = ["file\thdu\tplane\twidth\theight\tfinite\tsum\tmin\tmax\tzlo\tzhi\tpixels"]
    files = sorted(p for p in out.rglob("*") if p.is_file() and p.name.lower().endswith(EXTS))
    for p in files:
        rel = p.relative_to(out).as_posix()
        try:
            hl = fits.open(p, memmap=False, do_not_scale_image_data=False)
            hl.verify("silentfix")
        except Exception as e:  # noqa: BLE001 - astropy can't read it; nothing to compare
            print(f"skip {rel}: {e}", file=sys.stderr)
            continue
        with hl:
            for i, hdu in enumerate(hl):
                if not (isinstance(hdu, (fits.PrimaryHDU, fits.ImageHDU, fits.CompImageHDU))):
                    continue
                if getattr(hdu, "is_image", True) is False:
                    continue
                try:
                    data = hdu.data
                except Exception as e:  # noqa: BLE001
                    print(f"skip {rel}[{i}]: {e}", file=sys.stderr)
                    continue
                if data is None or data.ndim < 2 or data.size == 0 or data.dtype.kind not in "uif":
                    continue
                if isinstance(hdu, fits.PrimaryHDU) and hdu.header.get("GROUPS"):
                    continue
                ny, nx = data.shape[-2:]
                planes = data.reshape(-1, ny, nx)
                # The HDU index as our parser counts it: compressed images are the table HDU.
                for k in range(min(len(planes), MAX_STATS_PLANES)):
                    plane = np.asarray(planes[k], dtype=np.float64)
                    disp = plane[::-1]  # top row first
                    fin = np.isfinite(disp)
                    n = int(fin.sum())
                    if n:
                        s, lo, hi = disp[fin].sum(), disp[fin].min(), disp[fin].max()
                    else:
                        s, lo, hi = 0.0, float("nan"), float("nan")
                    if 0 < n and plane.size <= ZSCALE_MAX:
                        zl, zh = ZScaleInterval().get_limits(np.asarray(planes[k]))
                        z = (fmt(zl), fmt(zh))
                    else:
                        z = ("-", "-")
                    pts = []
                    for fx, fy in [(0, 0), (1, 0), (0, 1), (0.5, 0.5), (0.25, 0.75), (0.9, 0.1), (0.33, 0.66), (1, 1)]:
                        x = min(int(fx * nx), nx - 1)
                        y = min(int(fy * ny), ny - 1)
                        pts.append(f"{x},{y}={fmt(disp[y, x])}")
                    lines.append(
                        "\t".join(
                            [rel, str(i), str(k), str(nx), str(ny), str(n), fmt(s), fmt(lo), fmt(hi), *z, " ".join(pts)]
                        )
                    )
    (out / "expected.tsv").write_text("\n".join(lines) + "\n")
    print(f"{len(lines) - 1} planes from {len(files)} files -> {out / 'expected.tsv'}", file=sys.stderr)


def main():
    if len(sys.argv) != 3 or sys.argv[1] not in ("generate", "big", "stats"):
        sys.exit(__doc__)
    out = pathlib.Path(sys.argv[2]).resolve()
    {"generate": generate, "big": big, "stats": stats}[sys.argv[1]](out)


if __name__ == "__main__":
    main()
