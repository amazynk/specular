"""Oracle for specular-astro's HiPS tile painting (development only; not run by CI).

Independently of our tile-pixel rule (`hips::locate`), paints a made-up HiPS into a TAN view
the way the `hips` package does: each output pixel's tile from astropy-healpix, then the
position inside the tile through a projective transform fitted to the tile's four corners
(north, west, south and east, told apart by their coordinates), with the corners mapped to
the tile image's corners in FITS orientation (north top-right of the bottom-up array, east
bottom-left) and the rows then flipped to the top-down order of JPEG and PNG tiles.

The tiles are order 6, 8 × 8 pixels; pixel (column c, row r, top-down) of tile n is
RGB (32c + 16, 32r + 16, n mod 251).

Writes `tests/data/hips/view.tsv` (x, y top-down, R, G, B per output pixel, `-` where the
view has no sky) for `hips::tests::painting_matches_corner_fit`. A projective fit is not
exact inside a tile, so a few pixels on tile-pixel boundaries may differ; a mirrored or
transposed rule would differ almost everywhere.

Usage: python -I hips_oracle.py OUT_DIR
"""

import sys
from pathlib import Path

import numpy as np
from astropy.io import fits
from astropy.wcs import WCS
import astropy.units as u
from astropy_healpix import HEALPix

ORDER, WIDTH = 6, 8
W = H = 48
CARDS = [
    ("NAXIS", 2),
    ("NAXIS1", W),
    ("NAXIS2", H),
    ("CTYPE1", "RA---TAN"),
    ("CTYPE2", "DEC--TAN"),
    ("CRVAL1", 40.0),
    ("CRVAL2", 30.0),
    ("CRPIX1", 24.5),
    ("CRPIX2", 24.5),
    ("CDELT1", -0.05),
    ("CDELT2", 0.05),
    ("RADESYS", "ICRS"),
]


def projective(src, dst):
    """3x3 homography mapping src points (4, 2) to dst points."""
    a = []
    b = []
    for (x, y), (u_, v) in zip(src, dst):
        a.append([x, y, 1, 0, 0, 0, -u_ * x, -u_ * y])
        b.append(u_)
        a.append([0, 0, 0, x, y, 1, -v * x, -v * y])
        b.append(v)
    h = np.linalg.solve(np.array(a, float), np.array(b, float))
    return np.append(h, 1.0).reshape(3, 3)


def main():
    out = Path(sys.argv[1])
    out.mkdir(parents=True, exist_ok=True)
    h = fits.Header()
    for k, v in CARDS:
        h[k] = v
    wcs = WCS(h)
    hp = HEALPix(nside=2**ORDER, order="nested")
    rows = []
    cache = {}
    for yd in range(H):
        y0 = H - 1 - yd
        for x in range(W):
            sky = wcs.pixel_to_world(x, y0)
            lon, lat = sky.ra, sky.dec
            n = int(hp.lonlat_to_healpix(lon, lat))
            if n not in cache:
                blon, blat = hp.boundaries_lonlat(n, step=1)
                blon, blat = blon.to_value(u.deg)[0], blat.to_value(u.deg)[0]
                clon, clat = hp.healpix_to_lonlat(n)
                clon = clon.to_value(u.deg)
                north = int(np.argmax(blat))
                south = int(np.argmin(blat))
                rest = [i for i in range(4) if i not in (north, south)]
                d = [((blon[i] - clon + 180) % 360) - 180 for i in rest]
                east = rest[int(np.argmax(d))]
                west = rest[int(np.argmin(d))]
                px = []
                for i in (north, west, south, east):
                    from astropy.coordinates import SkyCoord

                    c = SkyCoord(blon[i] * u.deg, blat[i] * u.deg, frame="icrs")
                    px.append(wcs.world_to_pixel(c))
                e = WIDTH - 0.5
                dst = [(e, -0.5), (e, e), (-0.5, e), (-0.5, -0.5)]
                cache[n] = projective(np.array(px, float), dst)
            m = cache[n]
            v = m @ np.array([x, y0, 1.0])
            col, frow = v[0] / v[2], v[1] / v[2]
            c = int(np.clip(np.floor(col + 0.5), 0, WIDTH - 1))
            fr = int(np.clip(np.floor(frow + 0.5), 0, WIDTH - 1))
            r = WIDTH - 1 - fr
            rows.append(f"{x}\t{yd}\t{32 * c + 16}\t{32 * r + 16}\t{n % 251}\n")
    (out / "view.tsv").write_text("".join(rows))
    (out / "view.hdr").write_text(
        "".join(
            f"{k} = '{v}'\n" if isinstance(v, str) else f"{k} = {float(v)!r}\n" for k, v in CARDS
        )
    )


if __name__ == "__main__":
    main()
