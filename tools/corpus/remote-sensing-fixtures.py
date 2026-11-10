#!/usr/bin/env python3
"""Generate the remote-sensing fixtures and their GDAL oracle.

Usage: remote-sensing-fixtures.py OUT_DIR

Writes, under OUT_DIR/gen:
  * ENVI rasters of every data type (1 2 3 4 5 6 9 12 13 14 15), each interleave and both
    byte orders, written by hand (raw bytes + .hdr) so GDAL reads them independently;
  * a 24-band hyperspectral ENVI cube with wavelengths, fwhm, bbl and default bands;
  * complex ENVI (types 6 and 9), complex GeoTIFFs (CInt16, CFloat32, CFloat64, two bands)
    and SICD-style NITF files (I and Q bands, 16-bit integer and 32-bit float), written
    through GDAL (rasterio);
and OUT_DIR/expected/remote-sensing.jsonl: per file, what GDAL reads, as values at a grid of points
for each plane the viewer offers (complex: magnitude, real, imaginary, phase and dB at
each multilook factor). GDAL autotest files fetched next to gen/ (OUT_DIR/gdal) are added
to the oracle too when present.

Run with the oracle venv (numpy, rasterio): nothing here touches the network.
"""
import json
import os
import sys

import numpy as np
import rasterio
from rasterio.transform import Affine

OUT = sys.argv[1]
GEN = os.path.join(OUT, "gen")
os.makedirs(GEN, exist_ok=True)
os.makedirs(os.path.join(OUT, "expected"), exist_ok=True)

ENVI_TYPES = {
    1: np.uint8,
    2: np.int16,
    3: np.int32,
    4: np.float32,
    5: np.float64,
    6: np.complex64,
    9: np.complex128,
    12: np.uint16,
    13: np.uint32,
    14: np.int64,
    15: np.uint64,
}
LOOKS = [(1, 1), (2, 2), (3, 3), (4, 4), (5, 5), (1, 2), (2, 1), (1, 4), (4, 1)]
rng = np.random.default_rng(20261025)


def grid_points(w, h, n=12):
    xs = sorted({int(round(v)) for v in np.linspace(0, w - 1, min(n, w))})
    ys = sorted({int(round(v)) for v in np.linspace(0, h - 1, min(n, h))})
    return [(x, y) for y in ys for x in xs]


def num(v):
    v = float(v)
    return None if not np.isfinite(v) else v


def write_envi(path, arr, interleave, big_endian, extra=""):
    """arr: (bands, rows, cols)."""
    code = next(k for k, t in ENVI_TYPES.items() if np.dtype(t) == arr.dtype)
    b, h, w = arr.shape
    order = {"bsq": (0, 1, 2), "bil": (1, 0, 2), "bip": (1, 2, 0)}[interleave]
    data = np.ascontiguousarray(arr.transpose(order))
    data = data.astype(data.dtype.newbyteorder(">" if big_endian else "<"))
    data.tofile(path)
    with open(path[: -len(".img")] + ".hdr", "w") as f:
        f.write(
            "ENVI\ndescription = {remote-sensing fixture}\n"
            f"samples = {w}\nlines = {h}\nbands = {b}\nheader offset = 0\n"
            f"file type = ENVI Standard\ndata type = {code}\ninterleave = {interleave}\n"
            f"byte order = {1 if big_endian else 0}\n{extra}"
        )


def looked(z, fx, fy):
    """Block averages of a complex plane, replicated over each block."""
    h, w = z.shape
    re = np.full((h, w), np.nan)
    im = np.full((h, w), np.nan)
    it = np.full((h, w), np.nan)
    n = np.zeros((h, w))
    for y0 in range(0, h, fy):
        for x0 in range(0, w, fx):
            blk = z[y0 : y0 + fy, x0 : x0 + fx].astype(np.complex128)
            ok = np.isfinite(blk.real) & np.isfinite(blk.imag)
            if ok.sum() == 0:
                continue
            s = blk[ok]
            cell = (slice(y0, y0 + fy), slice(x0, x0 + fx))
            re[cell] = s.real.sum()
            im[cell] = s.imag.sum()
            it[cell] = (np.abs(s) ** 2).sum()
            n[cell] = ok.sum()
    with np.errstate(divide="ignore", invalid="ignore"):
        mag = np.sqrt(it / n)
        intensity = it / n
        db = np.where(intensity > 0, 10 * np.log10(intensity), np.nan)
        return [mag, re / n, im / n, np.arctan2(im, re), db]


records = []


def oracle_real(path, page=0, bands=None, composite=None, spectrum=None, nodata=None):
    with rasterio.open(path) as ds:
        a = ds.read().astype(np.float64)
        w, h = ds.width, ds.height
        crs = ds.crs.to_epsg() if ds.crs else None
        tr = list(ds.transform)[:6]
    nd = nodata
    pts = grid_points(w, h)
    planes = []
    order = composite if composite is not None else range(a.shape[0])
    for k, b in enumerate(order):
        v = a[b]
        planes.append(
            {
                "index": [k] if a.shape[0] > 1 else [],
                "points": [
                    [x, y, None if (nd is not None and v[y, x] == nd) else num(v[y, x])]
                    for x, y in pts
                ],
            }
        )
    rec = {
        "file": os.path.relpath(path, OUT),
        "w": w,
        "h": h,
        "page": page,
        "epsg": crs,
        "transform": tr,
        "planes": planes,
    }
    if spectrum is not None:
        good = spectrum
        rec["spectra"] = [
            [x, y, [None if not good[b] else num(a[b, y, x]) for b in range(a.shape[0])]]
            for x, y in pts[::7]
        ]
    records.append(rec)


def oracle_complex(path, band_axis):
    with rasterio.open(path) as ds:
        a = ds.read()
        w, h = ds.width, ds.height
        crs = ds.crs.to_epsg() if ds.crs else None
        tr = list(ds.transform)[:6]
        dt = ds.dtypes[0]
    if not np.iscomplexobj(a):
        # I and Q bands (SICD): pairs of real bands.
        a = a[0::2].astype(np.float64) + 1j * a[1::2].astype(np.float64)
    pts = grid_points(w, h)
    planes = []
    for b in range(a.shape[0]):
        for li, (fx, fy) in enumerate(LOOKS):
            if li not in (0, 1, 5, 8):
                continue
            parts = looked(a[b], fx, fy)
            for p, v in enumerate(parts):
                idx = ([b] if band_axis else []) + [p, li]
                planes.append(
                    {"index": idx, "points": [[x, y, num(v[y, x])] for x, y in pts]}
                )
    records.append(
        {
            "file": os.path.relpath(path, OUT),
            "w": w,
            "h": h,
            "page": 0,
            "epsg": crs,
            "transform": tr,
            "gdal_type": dt,
            "complex": True,
            "planes": planes,
        }
    )


# ENVI: every type, interleave and byte order (3 bands, 13 × 7: odd sizes catch strides).
for code, t in ENVI_TYPES.items():
    for il in ("bsq", "bil", "bip"):
        for be in (False, True):
            if np.dtype(t).itemsize == 1 and be:
                continue
            shape = (3, 7, 13)
            if np.issubdtype(t, np.complexfloating):
                arr = (rng.normal(0, 50, shape) + 1j * rng.normal(0, 50, shape)).astype(t)
            elif np.issubdtype(t, np.floating):
                arr = rng.normal(0, 1000, shape).astype(t)
            else:
                info = np.iinfo(t)
                lo, hi = max(info.min, -(2**40)), min(info.max, 2**40)
                arr = rng.integers(lo, hi, shape, endpoint=True).astype(t)
            p = os.path.join(GEN, f"envi_t{code}_{il}_{'be' if be else 'le'}.img")
            write_envi(p, arr, il, be)
            if np.issubdtype(t, np.complexfloating):
                oracle_complex(p, True)
            else:
                oracle_real(p)

# A hyperspectral cube: 24 bands 400–1000 nm, two bad bands, default bands, map info.
nb, h, w = 24, 40, 33
wl = np.linspace(400, 1000, nb)
yy, xx = np.mgrid[0:h, 0:w]
cube = np.stack(
    [(1000 + 30 * k + 7 * xx + 3 * yy + rng.integers(0, 20, (h, w))) for k in range(nb)]
).astype(np.uint16)
bbl = [1] * nb
bbl[5] = 0
bbl[17] = 0
hyper = os.path.join(GEN, "hyper_bil.img")
extra = (
    "wavelength units = Nanometers\n"
    "wavelength = {" + ", ".join(f"{v:.2f}" for v in wl) + "}\n"
    "fwhm = {" + ", ".join("10.0" for _ in wl) + "}\n"
    "bbl = {" + ", ".join(str(v) for v in bbl) + "}\n"
    "default bands = {10, 6, 2}\n"
    "map info = {UTM, 1.000, 1.000, 500000.000, 4200000.000, 3.0, 3.0, 33, North, WGS-84, units=Meters}\n"
)
write_envi(hyper, cube, "bil", False, extra)
oracle_real(hyper, page=0, composite=[9, 5, 1])
oracle_real(hyper, page=1, spectrum=[bool(v) for v in bbl])
# Without default bands: the composite comes from the wavelengths nearest 640/550/460 nm.
hyper2 = os.path.join(GEN, "hyper_nodefault_bip.img")
write_envi(hyper2, cube, "bip", True, extra.replace("default bands = {10, 6, 2}\n", ""))
near = [int(np.argmin(np.abs(wl - t) + np.where(np.array(bbl) == 0, 1e9, 0))) for t in (640, 550, 460)]
oracle_real(hyper2, page=0, composite=near)

# Complex GeoTIFFs and SICD-style NITF through GDAL.
geo = dict(crs="EPSG:32633", transform=Affine(2.5, 0, 400000, 0, -2.5, 5000000))
for dt, name in (("complex_int16", "cint16"), ("complex64", "cfloat32"), ("complex128", "cfloat64")):
    shape = (2, 45, 38)
    z = rng.normal(0, 300, shape) + 1j * rng.normal(0, 300, shape)
    if dt == "complex_int16":
        # rasterio writes CInt16 from an int16 pair view.
        p = os.path.join(GEN, f"sar_{name}.tif")
        with rasterio.open(
            p, "w", driver="GTiff", width=38, height=45, count=2, dtype="complex_int16", **geo
        ) as ds:
            ds.write(np.round(z).astype(np.complex64))
    else:
        p = os.path.join(GEN, f"sar_{name}.tif")
        with rasterio.open(
            p, "w", driver="GTiff", width=38, height=45, count=2, dtype=dt, **geo
        ) as ds:
            ds.write(z.astype(dt))
    oracle_complex(p, True)

for dt, name in (("float32", "re32f"), ("int16", "re16i")):
    h, w = 37, 51
    z = rng.normal(0, 400, (h, w)) + 1j * rng.normal(0, 400, (h, w))
    z[3:6, 10:13] = 0  # zero intensity: dB undefined
    iq = np.stack([z.real, z.imag]).astype(dt)
    p = os.path.join(GEN, f"sicd_{name}.ntf")
    with rasterio.open(
        p,
        "w",
        driver="NITF",
        width=w,
        height=h,
        count=2,
        dtype=dt,
        ICAT="SAR",
        ISUBCAT="I,Q",
        IREP="NODISPLY",
        IC="NC",
    ) as ds:
        ds.write(iq)
    oracle_complex(p, False)

# GDAL autotest files (fetched by fetch-remote-sensing.sh) GDAL reads.
gd = os.path.join(OUT, "gdal")
if os.path.isdir(gd):
    for f in sorted(os.listdir(gd)):
        p = os.path.join(gd, f)
        if f.endswith(".ntf"):
            oracle_complex(p, False)
        elif f.endswith((".img", ".dat")) and os.path.exists(p.rsplit(".", 1)[0] + ".hdr"):
            try:
                with rasterio.open(p) as ds:
                    n = ds.count
                    complex_ = np.dtype(ds.dtypes[0]).kind == "c"
            except Exception:
                continue
            if complex_:
                oracle_complex(p, n > 1)
            elif n <= 3:
                oracle_real(p)

with open(os.path.join(OUT, "expected", "remote-sensing.jsonl"), "w") as f:
    for r in records:
        f.write(json.dumps(r) + "\n")
print(f"{len(records)} oracle records")

# --------------------------------------------------------------------------- Sentinel-2
S2_BANDS = [
    ("B01", 60), ("B02", 10), ("B03", 10), ("B04", 10), ("B05", 20), ("B06", 20),
    ("B07", 20), ("B08", 10), ("B8A", 20), ("B09", 60), ("B10", 60), ("B11", 20), ("B12", 20),
]
VIEWS = {
    "TrueColour": ["B04", "B03", "B02"],
    "FalseColour": ["B08", "B04", "B03"],
    "Swir": ["B12", "B8A", "B04"],
    "Agriculture": ["B11", "B08", "B02"],
}


def synth_product(level, root, n10=60):
    """A small SAFE product: native-resolution band files only (so the reader resamples),
    15-bit lossless JPEG 2000 through GDAL, metadata in the real layout."""
    tile = "T33TUN"
    gran = f"GRANULE/{level}_{tile}_A000001_20200101T000000"
    os.makedirs(os.path.join(root, gran), exist_ok=True)
    sizes = {10: n10, 20: n10 // 2, 60: n10 // 6}
    files = []
    crs = "EPSG:32633"
    ul = (300000.0, 5300040.0)
    for name, res in S2_BANDS:
        if level == "L2A" and name == "B10":
            continue
        n = sizes[res]
        yy, xx = np.mgrid[0:n, 0:n]
        k = S2_BANDS.index((name, res))
        dn = (1000 + 97 * k + 13 * xx + 7 * yy + rng.integers(0, 50, (n, n))).astype(np.uint16)
        dn[: max(1, n // 10), : max(1, n // 10)] = 0  # no data in a corner
        if level == "L2A":
            rel = f"{gran}/IMG_DATA/R{res}m/{tile}_20200101T000000_{name}_{res}m"
        else:
            rel = f"{gran}/IMG_DATA/{tile}_20200101T000000_{name}"
        write_jp2(os.path.join(root, rel + ".jp2"), [dn], res, ul, crs)
        files.append(rel)
    if level == "L2A":
        n = sizes[20]
        scl = rng.integers(0, 12, (n, n)).astype(np.uint8)
        rel = f"{gran}/IMG_DATA/R20m/{tile}_20200101T000000_SCL_20m"
        write_jp2(os.path.join(root, rel + ".jp2"), [scl], 20, ul, crs, bits=8)
        files.append(rel)
        cld = rng.integers(0, 101, (n, n)).astype(np.uint8)
        write_jp2(os.path.join(root, gran, "QI_DATA", "MSK_CLDPRB_20m.jp2"), [cld], 20, ul, crs, bits=8)
    else:
        n = sizes[60]
        masks = [(rng.random((n, n)) < p).astype(np.uint8) for p in (0.2, 0.2, 0.1)]
        write_jp2(os.path.join(root, gran, "QI_DATA", "MSK_CLASSI_B00.jp2"), masks, 60, ul, crs, bits=8)
    off = "BOA_ADD_OFFSET" if level == "L2A" else "RADIO_ADD_OFFSET"
    q = "BOA_QUANTIFICATION_VALUE" if level == "L2A" else "QUANTIFICATION_VALUE"
    lv = "2A" if level == "L2A" else "1C"
    product = (
        f'<?xml version="1.0" encoding="UTF-8"?>\n<n1:Level-{lv}_User_Product xmlns:n1="x">\n'
        f"<n1:General_Info><Product_Info><PRODUCT_START_TIME>2020-01-01T00:00:00Z</PRODUCT_START_TIME>"
        f"<PROCESSING_LEVEL>Level-{lv}</PROCESSING_LEVEL><PRODUCT_TYPE>S2MSI{lv}</PRODUCT_TYPE>"
        f"<PROCESSING_BASELINE>05.00</PROCESSING_BASELINE><Datatake><SPACECRAFT_NAME>Sentinel-2B</SPACECRAFT_NAME></Datatake>"
        "<Product_Organisation><Granule_List><Granule>"
        + "".join(f"<IMAGE_FILE>{f}</IMAGE_FILE>" for f in files)
        + "</Granule></Granule_List></Product_Organisation></Product_Info>"
        f"<Product_Image_Characteristics><QUANTIFICATION_VALUES_LIST><{q}>10000</{q}></QUANTIFICATION_VALUES_LIST>"
        f"<{off}_VALUES_LIST>"
        + "".join(f'<{off} band_id="{i}">-1000</{off}>' for i in range(13))
        + f"</{off}_VALUES_LIST></Product_Image_Characteristics></n1:General_Info>"
        f"<n1:Quality_Indicators_Info><Cloud_Coverage_Assessment>3.5</Cloud_Coverage_Assessment></n1:Quality_Indicators_Info>"
        f"</n1:Level-{lv}_User_Product>\n"
    )
    with open(os.path.join(root, f"MTD_MSIL{lv}.xml"), "w") as f:
        f.write(product)
    tl = (
        f'<?xml version="1.0"?><n1:Level-{lv}_Tile_ID xmlns:n1="x"><n1:General_Info>'
        f"<TILE_ID>S2B_OPER_MSI_{level}_TL_{tile}</TILE_ID><SENSING_TIME>2020-01-01T00:00:01Z</SENSING_TIME>"
        "</n1:General_Info><n1:Geometric_Info><Tile_Geocoding>"
        "<HORIZONTAL_CS_NAME>WGS84 / UTM zone 33N</HORIZONTAL_CS_NAME><HORIZONTAL_CS_CODE>EPSG:32633</HORIZONTAL_CS_CODE>"
        + "".join(
            f'<Size resolution="{r}"><NROWS>{n}</NROWS><NCOLS>{n}</NCOLS></Size>'
            for r, n in sizes.items()
        )
        + "".join(
            f'<Geoposition resolution="{r}"><ULX>{ul[0]:.0f}</ULX><ULY>{ul[1]:.0f}</ULY><XDIM>{r}</XDIM><YDIM>-{r}</YDIM></Geoposition>'
            for r in sizes
        )
        + f"</Tile_Geocoding></n1:Geometric_Info></n1:Level-{lv}_Tile_ID>\n"
    )
    with open(os.path.join(root, gran, "MTD_TL.xml"), "w") as f:
        f.write(tl)


def write_jp2(path, comps, res, ul, crs, bits=15):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    n = comps[0].shape[0]
    with rasterio.open(
        path,
        "w",
        driver="JP2OpenJPEG",
        width=n,
        height=n,
        count=len(comps),
        dtype=comps[0].dtype,
        crs=crs,
        transform=Affine(res, 0, ul[0], 0, -res, ul[1]),
        QUALITY=100,
        REVERSIBLE="YES",
        NBITS=bits,
        RESOLUTIONS=2,
    ) as ds:
        ds.write(np.stack(comps))


def s2_meta(root):
    import re

    name = next(f for f in os.listdir(root) if f.startswith("MTD_MSIL"))
    text = open(os.path.join(root, name)).read()
    level = "L2A" if "MSIL2A" in name else "L1C"
    files = re.findall(r"<IMAGE_FILE>([^<]+)</IMAGE_FILE>", text)
    tag = "BOA_ADD_OFFSET" if level == "L2A" else "RADIO_ADD_OFFSET"
    offs = {int(i): float(v) for i, v in re.findall(rf'<{tag} band_id="(\d+)">([^<]+)<', text)}
    qtag = "BOA_QUANTIFICATION_VALUE" if level == "L2A" else "QUANTIFICATION_VALUE"
    q = float(re.search(rf"<{qtag}[^>]*>([^<]+)<", text).group(1))
    gran = "/".join(files[0].split("/")[:2])
    return level, files, offs, q, gran


def band_file(files, band, r):
    native = dict(S2_BANDS).get(band, 20)
    found = []
    for f in files:
        parts = f.rsplit("/", 1)[-1].split("_")
        if parts[-1] == band:
            found.append((f, native))
        elif parts[-1].endswith("m") and parts[-1][:-1].isdigit() and parts[-2] == band:
            found.append((f, int(parts[-1][:-1])))
    if not found:
        return None
    found.sort(key=lambda t: (0, 0) if t[1] == r else ((1, r - t[1]) if t[1] < r else (2, t[1] - r)))
    return found[0]


def resample(a, sr, r, size, categorical=False):
    if sr == r:
        return a
    if sr < r:
        k = r // sr
        if categorical:
            return a[::k, ::k][: size, : size]
        h = a.shape[0] // k
        b = a[: h * k, : h * k].reshape(h, k, h, k)
        with np.errstate(invalid="ignore"):
            return np.nanmean(b, axis=(1, 3))
    k = sr // r
    return np.repeat(np.repeat(a, k, axis=0), k, axis=1)[:size, :size]


def s2_oracle(root):
    level, files, offs, q, gran = s2_meta(root)
    with open(os.path.join(root, gran, "MTD_TL.xml")) as f:
        tl = f.read()
    import re

    sizes = {int(r): int(n) for r, n in re.findall(r'<Size resolution="(\d+)">\s*<NROWS>(\d+)<', tl)}
    out = []

    def band_plane(band, r):
        bf = band_file(files, band, r)
        if bf is None:
            return None
        p = os.path.join(root, bf[0] + ".jp2")
        if not os.path.exists(p):
            return None
        with rasterio.open(p) as ds:
            dn = ds.read(1).astype(np.float64)
            tr = list(ds.transform)[:6]
            epsg = ds.crs.to_epsg() if ds.crs else None
        k = [b for b, _ in S2_BANDS].index(band)
        v = np.where(dn == 0, np.nan, (dn + offs.get(k, 0.0)) / q)
        return resample(v, bf[1], r, sizes[r]), tr, epsg, bf[1]

    for r in (60, 20, 10):
        if r not in sizes:
            continue
        for view, bands in VIEWS.items():
            planes = []
            geo = None
            for k, b in enumerate(bands):
                got = band_plane(b, r)
                if got is None:
                    continue
                v, tr, epsg, fr = got
                if geo is None:
                    t = list(tr)
                    t[0], t[4] = float(r), -float(r)
                    geo = (t, epsg)
                pts = grid_points(sizes[r], sizes[r])
                planes.append({"index": [k], "points": [[x, y, num(v[y, x])] for x, y in pts]})
            if planes:
                out.append({"product": os.path.relpath(root, OUT), "view": view, "metres": r,
                            "w": sizes[r], "epsg": geo[1], "transform": geo[0], "planes": planes})
        # Scene classification and cloud probability.
        if level == "L2A":
            bf = band_file(files, "SCL", r)
            p = os.path.join(root, bf[0] + ".jp2") if bf else ""
            if os.path.exists(p):
                with rasterio.open(p) as ds:
                    a = ds.read(1).astype(np.float64)
                a = np.where(a == 0, np.nan, a)
                v = resample(a, bf[1], r, sizes[r], categorical=True)
                pts = grid_points(sizes[r], sizes[r])
                out.append({"product": os.path.relpath(root, OUT), "view": "SceneClass", "metres": r,
                            "w": sizes[r], "planes": [{"index": [], "points": [[x, y, num(v[y, x])] for x, y in pts]}]})
            fr = 60 if (r == 60 and os.path.exists(os.path.join(root, gran, "QI_DATA", "MSK_CLDPRB_60m.jp2"))) else 20
            p = os.path.join(root, gran, "QI_DATA", f"MSK_CLDPRB_{fr}m.jp2")
            if os.path.exists(p):
                with rasterio.open(p) as ds:
                    a = ds.read(1).astype(np.float64)
                v = resample(a, fr, r, sizes[r])
                pts = grid_points(sizes[r], sizes[r])
                out.append({"product": os.path.relpath(root, OUT), "view": "CloudProbability", "metres": r,
                            "w": sizes[r], "planes": [{"index": [], "points": [[x, y, num(v[y, x])] for x, y in pts]}]})
        else:
            p = os.path.join(root, gran, "QI_DATA", "MSK_CLASSI_B00.jp2")
            if os.path.exists(p):
                with rasterio.open(p) as ds:
                    m = ds.read().astype(np.float64)
                c = np.where(m[0] != 0, 1.0, np.where(m[1] != 0, 2.0, np.where(m[2] != 0, 3.0, 0.0)))
                v = resample(c, 60, r, sizes[r], categorical=True)
                pts = grid_points(sizes[r], sizes[r])
                out.append({"product": os.path.relpath(root, OUT), "view": "SceneClass", "metres": r,
                            "w": sizes[r], "planes": [{"index": [], "points": [[x, y, num(v[y, x])] for x, y in pts]}]})
    return out


synth = os.path.join(GEN, "s2")
for level in ("L2A", "L1C"):
    root = os.path.join(synth, f"S2B_MSI{level}_20200101T000000_N0500_R001_T33TUN_20200101T000000.SAFE")
    if not os.path.isdir(root):
        synth_product(level, root)
s2_records = []
for base in (synth, os.path.join(OUT, "s2")):
    if os.path.isdir(base):
        for d in sorted(os.listdir(base)):
            if d.endswith(".SAFE"):
                s2_records += s2_oracle(os.path.join(base, d))
# A zipped product: the synthetic L2A one.
import zipfile

zp = os.path.join(GEN, "s2_l2a.zip")
if not os.path.exists(zp):
    src = os.path.join(synth, "S2B_MSIL2A_20200101T000000_N0500_R001_T33TUN_20200101T000000.SAFE")
    with zipfile.ZipFile(zp, "w", zipfile.ZIP_DEFLATED) as z:
        for dp, _, fs in os.walk(src):
            for f in fs:
                full = os.path.join(dp, f)
                z.write(full, os.path.relpath(full, synth))
with open(os.path.join(OUT, "expected", "s2.jsonl"), "w") as f:
    for r in s2_records:
        f.write(json.dumps(r) + "\n")
print(f"{len(s2_records)} Sentinel-2 oracle records")
