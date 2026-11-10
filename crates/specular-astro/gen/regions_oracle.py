"""Oracle for specular-astro's DS9 region reader and writer (development only; not run by CI).

Writes `tests/data/regions/` fixtures checked by `regions::tests`:
- `sample.hdr`: a rotated TAN WCS (one `KEY = value` card per line);
- `sample.reg`: regions in fk5, icrs, galactic and image coordinates;
- `sample.oracle.tsv`: each region placed on the image by astropy-regions
  (`Regions.parse(..., format="ds9")`, `to_pixel(WCS)`), in display pixels (top-down,
  the first pixel spanning 0-1): kind, centre or vertices, sizes and angle;
- `sample.pyregion.tsv`: the same through pyregion (`as_imagecoord`), as a second opinion.
- `physical.hdr`, `physical.reg`, `physical.pyregion.tsv`: an image cut out and binned 2×2
  (`LTM1_1 = LTM2_2 = 0.5`, `LTV1`, `LTV2`; the same transform as the `L` alternate WCS,
  which is what pyregion reads), regions in DS9's `physical` system, placed by pyregion.
  astropy-regions (0.13) has no frame for `physical` and skips those regions.

With `--written FILE` it instead parses a region file our writer made (fk5) and prints the
shapes in display pixels in the same layout, for `regions::tests::written_file_reads_back`.

Usage: python -I regions_oracle.py OUT_DIR [--written FILE]
"""

import sys
from pathlib import Path

import numpy as np
from astropy.coordinates import SkyCoord
from astropy.io import fits
from astropy.wcs import WCS
import astropy.units as u
from regions import Regions

W, H = 200, 150
ROT = np.radians(25.0)
SCALE = 1.0 / 3600.0
CARDS = [
    ("NAXIS", 2),
    ("NAXIS1", W),
    ("NAXIS2", H),
    ("CTYPE1", "'RA---TAN'"),
    ("CTYPE2", "'DEC--TAN'"),
    ("CRVAL1", 150.0),
    ("CRVAL2", 2.2),
    ("CRPIX1", 100.5),
    ("CRPIX2", 75.5),
    ("CD1_1", -SCALE * np.cos(ROT)),
    ("CD1_2", SCALE * np.sin(ROT)),
    ("CD2_1", SCALE * np.sin(ROT)),
    ("CD2_2", SCALE * np.cos(ROT)),
    ("RADESYS", "'ICRS'"),
]


def header():
    h = fits.Header()
    for k, v in CARDS:
        h[k] = v.strip("'") if isinstance(v, str) else v
    return h


def region_text():
    g = SkyCoord(150.0021 * u.deg, 2.2008 * u.deg, frame="icrs").galactic
    return "\n".join(
        [
            "# Region file format: DS9 version 4.1",
            'global color=green dashlist=8 3 width=1 font="helvetica 10 normal roman" select=1 highlite=1 dash=0 fixed=0 edit=1 move=1 delete=1 include=1 source=1',
            "fk5",
            "circle(150.0012,2.2003,12.5\") # color=red text={A}",
            "ellipse(150.0,2.2,20\",10\",30) # width=2",
            "box(149.995,2.205,30\",15\",45) # dash=1",
            "polygon(149.99,2.19,150.0,2.21,150.01,2.195)",
            "point(150.003,2.198) # point=cross",
            "line(149.99,2.20,150.01,2.20) # line=1 0",
            "-circle(10:00:00.5,+02:12:00,5\")",
            "annulus(150.0,2.2,5\",10\")",
            "# text(150.002,2.203) text={Label}",
            "icrs",
            "ellipse(10:00:00.2,+02:12:01.5,0.5',0.25',-15)",
            "galactic",
            f"circle({g.l.deg:.7f},{g.b.deg:.7f},8\")",
            f"ellipse({g.l.deg:.7f},{g.b.deg:.7f},16\",6\",40)",
            "image",
            "circle(100,75,10)",
            "ellipse(50.5,60.25,8,4,20)",
            "box(120,30,16,8,-10)",
            "polygon(10,10,30,10,20,25)",
            "point(80,90) # point=x 9",
            "line(20,20,60,40)",
            "",
        ]
    )


# Binned 2×2 and cut out at (21, 41) of the original (1-based): physical = LTM·image + LTV.
LTM = 0.5
LTV = (-10.25, -20.75)
PHYSICAL_CARDS = CARDS + [
    ("LTM1_1", LTM),
    ("LTM2_2", LTM),
    ("LTV1", LTV[0]),
    ("LTV2", LTV[1]),
    ("WCSNAMEL", "'PHYSICAL'"),
    ("CTYPE1L", "'X'"),
    ("CTYPE2L", "'Y'"),
    ("CRPIX1L", 0.0),
    ("CRPIX2L", 0.0),
    ("CRVAL1L", LTV[0]),
    ("CRVAL2L", LTV[1]),
    ("CDELT1L", LTM),
    ("CDELT2L", LTM),
]


def physical_text():
    return "\n".join(
        [
            "# Region file format: DS9 version 4.1",
            "physical",
            "circle(30,20,6)",
            "ellipse(25.5,12.25,4,2,20)",
            "box(40,5,8,4,-10)",
            "polygon(-5,-10,5,-10,0,-2)",
            "point(15,25) # point=x 9",
            "line(-5,-5,15,5)",
            "",
        ]
    )


def disp(x, y):
    """0-based pixel (astropy) to display pixels."""
    return x + 0.5, H - 0.5 - y


def rows_regions(text, wcs):
    out = []
    for r in Regions.parse(text, format="ds9"):
        p = r.to_pixel(wcs) if hasattr(r, "to_pixel") else r
        name = type(p).__name__
        if name == "CirclePixelRegion":
            c = disp(p.center.x, p.center.y)
            out.append(["circle", c[0], c[1], p.radius, p.radius, 0.0])
        elif name == "CircleAnnulusPixelRegion":
            c = disp(p.center.x, p.center.y)
            out.append(["annulus", c[0], c[1], p.inner_radius, p.outer_radius, 0.0])
        elif name == "EllipsePixelRegion":
            c = disp(p.center.x, p.center.y)
            out.append(["ellipse", c[0], c[1], p.width / 2, p.height / 2, p.angle.to_value(u.deg)])
        elif name == "RectanglePixelRegion":
            c = disp(p.center.x, p.center.y)
            out.append(["box", c[0], c[1], p.width, p.height, p.angle.to_value(u.deg)])
        elif name == "PolygonPixelRegion":
            v = [disp(x, y) for x, y in zip(p.vertices.x, p.vertices.y)]
            out.append(["polygon"] + [q for xy in v for q in xy])
        elif name == "PointPixelRegion":
            c = disp(p.center.x, p.center.y)
            out.append(["point", c[0], c[1]])
        elif name == "LinePixelRegion":
            a, b = disp(p.start.x, p.start.y), disp(p.end.x, p.end.y)
            out.append(["line", a[0], a[1], b[0], b[1]])
        elif name == "TextPixelRegion":
            c = disp(p.center.x, p.center.y)
            out.append(["text", c[0], c[1]])
        else:
            out.append(["unknown", name])
        out[-1].insert(1, int(bool(r.meta.get("include", True))))
    return out


def rows_pyregion(text, hdr):
    import pyregion

    out = []
    for s in pyregion.parse(text).as_imagecoord(hdr):
        c = s.coord_list
        if s.name in ("circle", "ellipse", "box", "point", "annulus"):
            x, y = disp(c[0] - 1, c[1] - 1)
            rest = list(c[2:])
            out.append([s.name, int(not s.exclude), x, y] + rest)
        elif s.name in ("polygon", "line"):
            v = [disp(c[i] - 1, c[i + 1] - 1) for i in range(0, len(c), 2)]
            out.append([s.name, int(not s.exclude)] + [q for xy in v for q in xy])
        else:
            out.append([s.name, int(not s.exclude)])
    return out


def fmt(rows):
    return "".join(
        "\t".join(v if isinstance(v, str) else f"{float(v):.9f}" for v in r) + "\n" for r in rows
    )


def main():
    out = Path(sys.argv[1])
    out.mkdir(parents=True, exist_ok=True)
    hdr = header()
    wcs = WCS(hdr)
    if "--written" in sys.argv:
        text = Path(sys.argv[sys.argv.index("--written") + 1]).read_text()
        sys.stdout.write(fmt(rows_regions(text, wcs)))
        return
    text = region_text()
    (out / "sample.hdr").write_text(
        "".join(f"{k} = {float(v)!r}\n" if not isinstance(v, str) else f"{k} = {v}\n" for k, v in CARDS)
    )
    (out / "sample.reg").write_text(text)
    (out / "sample.oracle.tsv").write_text(fmt(rows_regions(text, wcs)))
    try:
        (out / "sample.pyregion.tsv").write_text(fmt(rows_pyregion(text, hdr)))
    except ImportError:
        print("pyregion not installed: no second opinion", file=sys.stderr)
    phdr = fits.Header()
    for k, v in PHYSICAL_CARDS:
        phdr[k] = v.strip("'") if isinstance(v, str) else v
    ptext = physical_text()
    (out / "physical.hdr").write_text(
        "".join(
            f"{k} = {float(v)!r}\n" if not isinstance(v, str) else f"{k} = {v}\n"
            for k, v in PHYSICAL_CARDS
        )
    )
    (out / "physical.reg").write_text(ptext)
    (out / "physical.pyregion.tsv").write_text(fmt(rows_pyregion(ptext, phdr)))


if __name__ == "__main__":
    main()
