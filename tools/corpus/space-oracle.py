#!/usr/bin/env python3
"""Planetary corpus oracle (tools/corpus/fetch-space.sh): writes synthetic VICAR, PDS3 and
PDS4 files and fpack files, then the expected values of every image in the corpus.

Usage: space-oracle.py OUT

  OUT/synth/    VICAR (every FORMAT, ORG, INTFMT/REALFMT incl. VAX, prefixes, binary headers,
                BASIC and BASIC2 compression written by GDAL,
                an EOL label), PDS3 (every SAMPLE_TYPE family, storage order, prefixes,
                scaling, missing constants incl. hex bit patterns, a detached label) and
                PDS4 (every element type, axis order, special constants, scaling) files,
                written here from arrays whose values are known (independently of the Rust
                writers)
  OUT/fpack/    tile-compressed FITS written by astropy (RICE, GZIP 1/2, HCOMPRESS, PLIO,
                quantised floats with dithering, BZERO unsigned)
  OUT/expected/ per image file: <rel>.json (size, bands, the source of the values and, for
                synthetic files, whether GDAL agrees with the known values) and <rel>.f32
                (the values: bands one after another, little-endian float32, no-data as NaN,
                every `step`-th row and column of large images)

The values are the known ones for synthetic files, GDAL's (rasterio, scaled, its mask as
NaN) for downloaded ones, and astropy's for fpack files. Only reads the corpus: nothing in it
is executed.
"""

import json
import os
import struct
import sys

import numpy as np
import rasterio
import rasterio.shutil
from astropy.io import fits

OUT = sys.argv[1]
SYN = os.path.join(OUT, "synth")
FZ = os.path.join(OUT, "fpack")
EXP = os.path.join(OUT, "expected")
MAX_PIXELS = 1 << 20
RNG = np.random.default_rng(20261019)


# ------------------------------------------------------------------------- encodings

def vax_f32(a):
    """float32 values as VAX F bytes (each 32-bit word pair little-endian, high word first)."""
    a = np.asarray(a, dtype=np.float32)
    bits = a.view(np.uint32).astype(np.uint64)
    sign = bits >> 31
    exp = (bits >> 23) & 0xFF
    frac = bits & 0x7FFFFF
    x = (sign << 31) | ((exp + 2) << 23) | frac
    x = np.where(a == 0, 0, x).astype(np.uint32)
    hi = (x >> 16).astype("<u2")
    lo = (x & 0xFFFF).astype("<u2")
    return np.stack([hi, lo], axis=-1).tobytes()


def vax_f64(a, g=False):
    """float64 values as VAX D (or G) bytes: four little-endian words, most significant first."""
    a = np.asarray(a, dtype=np.float64)
    bits = a.view(np.uint64)
    sign = bits >> np.uint64(63)
    exp = (bits >> np.uint64(52)) & np.uint64(0x7FF)
    frac = bits & np.uint64((1 << 52) - 1)
    if g:
        x = (sign << np.uint64(63)) | ((exp + np.uint64(2)) << np.uint64(52)) | frac
    else:
        e = (exp.astype(np.int64) - 1023 + 129).astype(np.uint64) & np.uint64(0xFF)
        x = (sign << np.uint64(63)) | (e << np.uint64(55)) | (frac << np.uint64(3))
    x = np.where(a == 0, np.uint64(0), x)
    words = [((x >> np.uint64(s)) & np.uint64(0xFFFF)).astype("<u2") for s in (48, 32, 16, 0)]
    return np.stack(words, axis=-1).tobytes()


def encode(values, kind, bytes_, order, real="ieee"):
    """values (any shape) as `kind` ('u', 'i', 'f', 'c'), `bytes_` bytes, `order` ('>' or '<'),
    reals as 'ieee', 'vax' or 'vaxg'."""
    if kind in "ui":
        return np.asarray(values).astype(f"{order}{kind}{bytes_}").tobytes()
    if kind == "f":
        if real == "vax":
            return vax_f32(values) if bytes_ == 4 else vax_f64(values)
        if real == "vaxg":
            return vax_f64(values, g=True)
        return np.asarray(values).astype(f"{order}f{bytes_}").tobytes()
    # complex: (re, im) pairs; values are magnitudes, stored as (v, 0)
    v = np.asarray(values, dtype=np.float64)
    pairs = np.stack([v, np.zeros_like(v)], axis=-1)
    return encode(pairs, "f", bytes_ // 2, order, real)


def make_values(kind, bytes_, shape):
    if kind == "u":
        hi = min(2 ** (8 * bytes_) - 1, 60000)
        return RNG.integers(0, hi, size=shape).astype(np.float64)
    if kind == "i":
        lim = min(2 ** (8 * bytes_ - 1) - 1, 30000)
        return RNG.integers(-lim, lim, size=shape).astype(np.float64)
    if kind == "c":
        return np.abs(RNG.normal(0, 500, size=shape)).astype(np.float32).astype(np.float64)
    v = RNG.normal(0, 1000, size=shape)
    return v.astype(np.float32).astype(np.float64) if bytes_ == 4 else v


# ------------------------------------------------------------------------------ VICAR

VICAR_FORMATS = {  # FORMAT: (kind, bytes)
    "BYTE": ("u", 1), "HALF": ("i", 2), "FULL": ("i", 4),
    "REAL": ("f", 4), "DOUB": ("f", 8), "COMP": ("c", 8),
}


def write_vicar(path, fmt, org, intfmt, realfmt, ns, nl, nb, nbb=0, nlb=0, eol=False):
    kind, nbytes = VICAR_FORMATS[fmt]
    vals = make_values(kind, nbytes, (nb, nl, ns))
    order = ">" if (intfmt == "HIGH" if kind in "ui" else realfmt == "IEEE") else "<"
    real = "vax" if realfmt == "VAX" else "ieee"
    line_items = ns if org == "BSQ" else ns * nb
    recsize = nbb + line_items * nbytes
    n1, n2, n3 = {"BSQ": (ns, nl, nb), "BIL": (ns, nb, nl), "BIP": (nb, ns, nl)}[org]
    items = (f"FORMAT='{fmt}'  TYPE='IMAGE'  BUFSIZ=20480  DIM=3  EOL={int(eol)}  "
             f"RECSIZE={recsize}  ORG='{org}'  NL={nl}  NS={ns}  NB={nb}  N1={n1}  N2={n2}  "
             f"N3={n3}  N4=0  NBB={nbb}  NLB={nlb}  HOST='X86-64-LINX'  INTFMT='{intfmt}'  "
             f"REALFMT='{realfmt}'  BHOST='X86-64-LINX'  BINTFMT='LOW'  BREALFMT='RIEEE'  "
             f"BLTYPE=''  PROPERTY='IDENTIFICATION'  TARGET_NAME='TITAN'  "
             f"INSTRUMENT_NAME='SYNTHETIC CAMERA'  ")
    size = -(-(len(items) + 24) // recsize) * recsize
    label = f"LBLSIZE={size:<12d}{items}".encode().ljust(size, b"\0")
    out = bytearray(label)
    out += bytes([0xAA]) * (nlb * recsize)
    prefix = bytes([0x55]) * nbb
    for y in range(nl):
        if org == "BSQ":
            continue
        out += prefix
        row = vals[:, y, :]  # (nb, ns)
        data = row.T if org == "BIP" else row  # BIP: sample-major
        out += encode(data, kind, nbytes, order, real)
    if org == "BSQ":
        for b in range(nb):
            for y in range(nl):
                out += prefix
                out += encode(vals[b, y, :], kind, nbytes, order, real)
    if eol:
        e = "PROPERTY='MAP'  MAP_PROJECTION_TYPE='SIMPLE_CYLINDRICAL'  MAP_SCALE=1.5  "
        esize = -(-(len(e) + 24) // recsize) * recsize
        out += f"LBLSIZE={esize:<12d}{e}".encode().ljust(esize, b"\0")
    with open(path, "wb") as f:
        f.write(out)
    return vals


# ------------------------------------------------------------------------------- PDS3

PDS3_TYPES = [  # SAMPLE_TYPE, bits, kind, order, real, gdal_reads_it
    ("UNSIGNED_INTEGER", 8, "u", ">", "ieee", True),
    ("MSB_UNSIGNED_INTEGER", 16, "u", ">", "ieee", True),
    ("LSB_UNSIGNED_INTEGER", 16, "u", "<", "ieee", True),
    # GDAL reads 16-bit UNSIGNED_INTEGER as LSB; the standard makes it MSB.
    ("UNSIGNED_INTEGER", 16, "u", ">", "ieee", False),
    ("MSB_INTEGER", 16, "i", ">", "ieee", True),
    ("LSB_INTEGER", 16, "i", "<", "ieee", True),
    ("PC_INTEGER", 16, "i", "<", "ieee", True),
    ("VAX_INTEGER", 16, "i", "<", "ieee", True),
    # GDAL reads every 32-bit sample as a real.
    ("MSB_INTEGER", 32, "i", ">", "ieee", False),
    ("LSB_UNSIGNED_INTEGER", 32, "u", "<", "ieee", False),
    ("IEEE_REAL", 32, "f", ">", "ieee", True),
    ("PC_REAL", 32, "f", "<", "ieee", True),
    ("VAX_REAL", 32, "f", "<", "vax", True),
    ("IEEE_REAL", 64, "f", ">", "ieee", True),
    ("PC_REAL", 64, "f", "<", "ieee", True),
    ("VAX_REAL", 64, "f", "<", "vax", False),
    ("VAXG_REAL", 64, "f", "<", "vaxg", False),
    ("IEEE_COMPLEX", 64, "c", ">", "ieee", False),
    ("PC_COMPLEX", 128, "c", "<", "ieee", False),
]
STORAGE = {"BAND_SEQUENTIAL": "bsq", "LINE_INTERLEAVED": "bil", "SAMPLE_INTERLEAVED": "bip"}


def pds3_label(pointer, ns, nl, nb, st, bits, storage, extra, record=512, label_records=None):
    lines = [
        "PDS_VERSION_ID = PDS3",
        "RECORD_TYPE = FIXED_LENGTH",
        f"RECORD_BYTES = {record}",
    ]
    if label_records is not None:
        lines.append(f"LABEL_RECORDS = {label_records}")
    lines += [
        f"^IMAGE = {pointer}",
        'MISSION_NAME = "SYNTHETIC"',
        'INSTRUMENT_HOST_NAME = "TEST ORBITER"',
        'INSTRUMENT_NAME = "TEST CAMERA"',
        "TARGET_NAME = MARS",
        "START_TIME = 2020-01-01T00:00:00.000",
        "OBJECT = IMAGE",
        f"  LINES = {nl}",
        f"  LINE_SAMPLES = {ns}",
        f"  BANDS = {nb}",
        f"  SAMPLE_TYPE = {st}",
        f"  SAMPLE_BITS = {bits}",
        f"  BAND_STORAGE_TYPE = {storage}",
    ] + [f"  {e}" for e in extra] + ["END_OBJECT = IMAGE", "END", ""]
    return "\r\n".join(lines)


def pds3_data(vals, kind, nbytes, order, real, storage, prefix):
    nb, nl, ns = vals.shape
    out = bytearray()
    pre = bytes([0x33]) * prefix
    if storage == "bsq":
        for b in range(nb):
            for y in range(nl):
                out += pre + encode(vals[b, y], kind, nbytes, order, real)
    else:
        for y in range(nl):
            row = vals[:, y, :]
            out += pre + encode(row.T if storage == "bip" else row, kind, nbytes, order, real)
    return bytes(out)


def write_pds3(path, st, bits, kind, order, real, storage, ns=29, nl=17, nb=3,
               prefix=0, scale=None, missing=None, hex_missing=False):
    nbytes = bits // 8
    vals = make_values(kind, nbytes, (nb, nl, ns))
    extra = []
    if prefix:
        extra.append(f"LINE_PREFIX_BYTES = {prefix}")
    truth = vals.copy()
    if missing is not None:
        vals[0, 1, 2] = missing
        vals[nb - 1, nl - 1, 0] = missing
        truth[vals == missing] = np.nan
        if hex_missing:
            h = struct.unpack(">I", struct.pack(">f", missing))[0]
            extra.append(f'MISSING_CONSTANT = 16#{h:08X}#')
        else:
            extra.append(f"MISSING_CONSTANT = {missing:g}")
    if scale is not None:
        extra += [f"SCALING_FACTOR = {scale[0]}", f"OFFSET = {scale[1]}"]
        truth = truth * scale[0] + scale[1]
    data = pds3_data(vals, kind, nbytes, order, real, STORAGE[storage], prefix)
    record = 512
    n = 1
    while True:
        text = pds3_label(n + 1, ns, nl, nb, st, bits, storage, extra, record, n).encode()
        if len(text) <= n * record:
            break
        n += 1
    with open(path, "wb") as f:
        f.write(text.ljust(n * record, b" ") + data)
    return truth


def write_pds3_detached(stem, ns=31, nl=11):
    vals = make_values("i", 2, (1, nl, ns))
    data = encode(vals[0], "i", 2, "<", "ieee")
    with open(stem + ".DAT", "wb") as f:
        f.write(b"\x00" * 100 + data)
    label = pds3_label(f'("{os.path.basename(stem)}.DAT", 101 <BYTES>)', ns, nl, 1,
                       "LSB_INTEGER", 16, "BAND_SEQUENTIAL", [], record=1)
    with open(stem + ".lbl", "w") as f:
        f.write(label)
    return vals


# ------------------------------------------------------------------------------- PDS4

PDS4_TYPES = [  # data_type, kind, bytes, order
    ("UnsignedByte", "u", 1, ">"), ("SignedByte", "i", 1, ">"),
    ("UnsignedMSB2", "u", 2, ">"), ("UnsignedLSB2", "u", 2, "<"),
    ("SignedMSB2", "i", 2, ">"), ("SignedLSB2", "i", 2, "<"),
    ("UnsignedMSB4", "u", 4, ">"), ("SignedLSB4", "i", 4, "<"),
    ("IEEE754MSBSingle", "f", 4, ">"), ("IEEE754LSBSingle", "f", 4, "<"),
    ("IEEE754MSBDouble", "f", 8, ">"), ("IEEE754LSBDouble", "f", 8, "<"),
    ("ComplexLSB8", "c", 8, "<"), ("ComplexMSB16", "c", 16, ">"),
]
PDS4_ORDERS = {"bsq": ("Band", "Line", "Sample"), "bip": ("Line", "Sample", "Band"),
               "bil": ("Line", "Band", "Sample")}


def write_pds4(stem, dtype, kind, nbytes, order, axes, ns=23, nl=13, nb=2, offset=64,
               scale=None, missing=None, cls=None):
    vals = make_values(kind, nbytes, (nb, nl, ns))
    truth = vals.copy()
    if missing is not None:
        vals[0, 0, 1] = missing
        truth[vals == missing] = np.nan
    if scale is not None:
        truth = truth * scale[0] + scale[1]
    if axes == "2d":
        names, dims, arr = ("Line", "Sample"), (nl, ns), vals[0]
        truth = truth[:1]
    else:
        names = PDS4_ORDERS[axes]
        pos = {"Band": 0, "Line": 1, "Sample": 2}
        arr = np.transpose(vals, [pos[n] for n in names])
        dims = arr.shape
    cls = cls or ("Array_2D_Image" if axes == "2d" else "Array_3D_Image")
    data = encode(arr, kind, nbytes, order)
    name = os.path.basename(stem)
    with open(stem + ".dat", "wb") as f:
        f.write(b"\x11" * offset + data)
    axes_xml = "".join(
        f"<Axis_Array><axis_name>{n}</axis_name><elements>{d}</elements>"
        f"<sequence_number>{i + 1}</sequence_number></Axis_Array>"
        for i, (n, d) in enumerate(zip(names, dims)))
    el = f"<data_type>{dtype}</data_type>"
    if scale is not None:
        el += f"<scaling_factor>{scale[0]}</scaling_factor><value_offset>{scale[1]}</value_offset>"
    sc = ""
    if missing is not None:
        sc = f"<Special_Constants><missing_constant>{missing:g}</missing_constant></Special_Constants>"
    xml = f"""<?xml version="1.0" encoding="UTF-8"?>
<Product_Observational xmlns="http://pds.nasa.gov/pds4/pds/v1">
  <Identification_Area>
    <logical_identifier>urn:nasa:pds:synthetic:data:{name.lower()}</logical_identifier>
    <version_id>1.0</version_id><title>Synthetic {dtype} {axes}</title>
    <information_model_version>1.15.0.0</information_model_version>
    <product_class>Product_Observational</product_class>
  </Identification_Area>
  <Observation_Area>
    <Time_Coordinates><start_date_time>2021-02-18T20:55:00Z</start_date_time>
    <stop_date_time>2021-02-18T20:56:00Z</stop_date_time></Time_Coordinates>
    <Investigation_Area><name>Synthetic Mission</name><type>Mission</type></Investigation_Area>
    <Observing_System><Observing_System_Component><name>Test Rover</name><type>Host</type>
    </Observing_System_Component><Observing_System_Component><name>Test Camera</name>
    <type>Instrument</type></Observing_System_Component></Observing_System>
    <Target_Identification><name>Mars</name><type>Planet</type></Target_Identification>
  </Observation_Area>
  <File_Area_Observational>
    <File><file_name>{name}.dat</file_name></File>
    <{cls}><offset unit="byte">{offset}</offset><axes>{len(names)}</axes>
      <axis_index_order>Last Index Fastest</axis_index_order>
      <Element_Array>{el}</Element_Array>{axes_xml}{sc}
    </{cls}>
  </File_Area_Observational>
</Product_Observational>
"""
    with open(stem + ".xml", "w") as f:
        f.write(xml)
    return truth


# ------------------------------------------------------------------------------ fpack

def write_fpack():
    os.makedirs(FZ, exist_ok=True)
    known = {}
    y, x = np.mgrid[0:300, 0:217]
    smooth = (1000 + 800 * np.sin(x / 17.0) * np.cos(y / 23.0)).astype(np.float64)
    cases = [
        ("rice_i16", smooth.astype(np.int16), "RICE_1", {}),
        ("rice_i32", (smooth * 1000).astype(np.int32), "RICE_1", {}),
        ("rice_u8", (smooth / 8).astype(np.uint8), "RICE_1", {}),
        ("gzip1_i16", smooth.astype(np.int16), "GZIP_1", {}),
        ("gzip2_i32", (smooth * 50).astype(np.int32), "GZIP_2", {}),
        ("hcompress_i16", smooth.astype(np.int16), "HCOMPRESS_1", {}),
        ("plio_i16", (smooth / 4).astype(np.int16), "PLIO_1", {}),
        ("rice_u16_bzero", (smooth * 30).astype(np.uint16), "RICE_1", {}),
        ("rice_f32_dither", (smooth + RNG.normal(0, 3, smooth.shape)).astype(np.float32),
         "RICE_1", {"quantize_level": 16.0, "quantize_method": 1}),
        ("gzip2_f32_lossless", (smooth / 7).astype(np.float32), "GZIP_2",
         {"quantize_level": 0.0}),
        ("rice_tiles_i16", smooth.astype(np.int16), "RICE_1", {"tile_shape": (32, 50)}),
    ]
    for name, data, ctype, kw in cases:
        hdu = fits.CompImageHDU(data=data, compression_type=ctype, **kw)
        hdu.header["OBJECT"] = name
        path = os.path.join(FZ, f"{name}.fits.fz" if name.startswith("rice_t") else f"{name}.fz")
        fits.HDUList([fits.PrimaryHDU(), hdu]).writeto(path, overwrite=True)
        with fits.open(path) as f:
            known[os.path.relpath(path, OUT)] = np.asarray(f[1].data, dtype=np.float64)[None]
    return known


# ----------------------------------------------------------------------------- driver

def synth():
    os.makedirs(SYN, exist_ok=True)
    known = {}
    for fmt in VICAR_FORMATS:
        for org in ("BSQ", "BIL", "BIP"):
            for intfmt, realfmt in (("LOW", "RIEEE"), ("HIGH", "IEEE"), ("LOW", "VAX")):
                if fmt in ("BYTE", "HALF", "FULL") and realfmt == "VAX" and intfmt == "LOW":
                    pass
                nbb = 0 if org == "BIL" else 6
                name = f"vicar_{fmt}_{org}_{intfmt}_{realfmt}.vic".lower()
                p = os.path.join(SYN, name)
                v = write_vicar(p, fmt, org, intfmt, realfmt, 21, 13, 3, nbb=nbb, nlb=2)
                known[os.path.relpath(p, OUT)] = v
    # BASIC and BASIC2 compressed VICAR (one integer band), written by GDAL's encoder.
    y, x = np.mgrid[0:37, 0:53]
    for dtype in ("uint8", "int16", "int32"):
        for comp in ("BASIC", "BASIC2"):
            v = (np.sin(x / 5.0) * 40 + y * 3 + RNG.integers(0, 3, x.shape)).astype(dtype)
            if dtype == "uint8":
                v = np.where((x > 20) & (x < 40), 77, v).astype(dtype)  # a long run
            vals = v[None]
            p = os.path.join(SYN, f"vicar_{comp}_{dtype}.vic".lower())
            # Through CreateCopy (writing a BASIC file block by block clobbers its label).
            tif = p + ".tmp.tif"
            with rasterio.open(tif, "w", driver="GTiff", width=53, height=37, count=1,
                               dtype=dtype) as d:
                d.write(vals)
            rasterio.shutil.copy(tif, p, driver="VICAR", COMPRESS=comp)
            os.remove(tif)
            known[os.path.relpath(p, OUT)] = vals.astype(np.float64)
    p = os.path.join(SYN, "vicar_eol.img")
    known[os.path.relpath(p, OUT)] = write_vicar(p, "HALF", "BSQ", "LOW", "RIEEE", 40, 30, 1,
                                                 eol=True)
    for st, bits, kind, order, real, _gdal in PDS3_TYPES:
        for storage in STORAGE:
            name = f"pds3_{st}_{bits}_{STORAGE[storage]}.img".lower()
            p = os.path.join(SYN, name)
            missing = None if kind == "c" else (7.0 if kind in "ui" else -1.25)
            scale = (0.5, 10.0) if kind in "ui" else None
            known[os.path.relpath(p, OUT)] = write_pds3(
                p, st, bits, kind, order, real, storage, prefix=12 if kind != "c" else 0,
                scale=scale, missing=missing)
    p = os.path.join(SYN, "pds3_hex_missing.img")
    known[os.path.relpath(p, OUT)] = write_pds3(p, "IEEE_REAL", 32, "f", ">", "ieee",
                                                "BAND_SEQUENTIAL", nb=1,
                                                missing=float(np.float32(-3.4028226550889045e+38)),
                                                hex_missing=True)
    stem = os.path.join(SYN, "pds3_detached")
    known[os.path.relpath(stem + ".lbl", OUT)] = write_pds3_detached(stem)
    for dtype, kind, nbytes, order in PDS4_TYPES:
        for axes in ("2d", "bsq", "bil", "bip"):
            stem = os.path.join(SYN, f"pds4_{dtype}_{axes}".lower())
            missing = None if kind == "c" else (5.0 if kind in "ui" else -2.5)
            scale = (2.0, -1.0) if kind in "ui" else None
            cls = "Array_3D_Spectrum" if axes == "bip" else None
            known[os.path.relpath(stem + ".xml", OUT)] = write_pds4(
                stem, dtype, kind, nbytes, order, axes, scale=scale, missing=missing, cls=cls)
    return known


def gdal_read(path):
    """GDAL's bands (scaled, mask as NaN) and facts, or the error."""
    try:
        with rasterio.open(path) as d:
            bands = []
            for i in range(1, d.count + 1):
                a = d.read(i, masked=True)
                v = np.asarray(a.astype(np.float64).filled(np.nan)) if np.ma.isMaskedArray(a) \
                    else np.asarray(a, dtype=np.float64)
                if np.iscomplexobj(a):
                    v = np.abs(np.asarray(a.filled(np.nan)))
                s, o = d.scales[i - 1], d.offsets[i - 1]
                bands.append(v * s + o)
            return np.stack(bands), {"driver": d.driver, "nodata": d.nodata,
                                     "scales": list(d.scales), "offsets": list(d.offsets)}
    except Exception as e:  # noqa: BLE001 (the oracle records any failure)
        return None, {"error": str(e)[:300]}


def agree(a, b):
    """Pixels where a and b differ (NaN equal NaN; reals within 1e-6 relative)."""
    if a.shape != b.shape:
        return -1
    both = np.isnan(a) & np.isnan(b)
    close = np.isclose(a, b, rtol=1e-6, atol=1e-9)
    return int(np.count_nonzero(~(both | close)))


def clean(v):
    """JSON without NaN or infinities (null instead)."""
    if isinstance(v, float) and not np.isfinite(v):
        return None
    if isinstance(v, dict):
        return {k: clean(x) for k, x in v.items()}
    if isinstance(v, (list, tuple)):
        return [clean(x) for x in v]
    return v


def dump(rel, values, meta):
    nb, h, w = values.shape
    step = 1
    while (h // step) * (w // step) * nb > MAX_PIXELS:
        step += 1
    sub = values[:, ::step, ::step].astype("<f4")
    path = os.path.join(EXP, rel)
    os.makedirs(os.path.dirname(path), exist_ok=True)
    sub.tofile(path + ".f32")
    meta.update({"width": w, "height": h, "bands": nb, "step": step,
                 "sub_width": sub.shape[2], "sub_height": sub.shape[1]})
    with open(path + ".json", "w") as f:
        json.dump(clean(meta), f, indent=1)


# Files GDAL cannot open that hold the same image as one it can (the oracle path).
ORACLE_ALIAS = {
    # An SFDU-labelled PDS3 label (GDAL only knows PDS_VERSION_ID): its VICAR image.
    "nasa/galileo/2000r.lbl": "nasa/galileo/2000r.img",
}
SKIP_SUFFIX = (".json", ".f32", ".part", ".tsv", ".tif", ".fit", ".zip", ".ZIP", ".raw", ".dat",
               ".DAT")


def main():
    os.makedirs(EXP, exist_ok=True)
    known = synth()
    known.update(write_fpack())
    summary = {"synthetic": 0, "gdal_agrees": 0, "gdal_disagrees": [], "gdal_fails": []}
    for root, _dirs, files in os.walk(OUT):
        if root.startswith(EXP):
            continue
        for fn in sorted(files):
            rel = os.path.relpath(os.path.join(root, fn), OUT)
            if fn.startswith(".") or fn.endswith(SKIP_SUFFIX):
                continue
            if rel.startswith("fpack/"):
                dump(rel, known[rel], {"source": "astropy", "flip_y": True})
                continue
            oracle = ORACLE_ALIAS.get(rel, rel)
            g, gmeta = gdal_read(os.path.join(OUT, oracle))
            meta = {"gdal": gmeta, "oracle_path": oracle}
            if rel in known:
                truth = known[rel]
                summary["synthetic"] += 1
                if g is not None:
                    n = agree(truth, g)
                    meta["gdal_mismatches"] = n
                    if n == 0:
                        summary["gdal_agrees"] += 1
                    else:
                        summary["gdal_disagrees"].append(rel)
                else:
                    summary["gdal_fails"].append(rel)
                meta["source"] = "known"
                dump(rel, truth, meta)
            elif g is not None:
                meta["source"] = "gdal"
                dump(rel, g, meta)
            else:
                path = os.path.join(EXP, rel)
                os.makedirs(os.path.dirname(path), exist_ok=True)
                with open(path + ".json", "w") as f:
                    json.dump(clean({"source": "none", **meta}), f, indent=1)
    with open(os.path.join(EXP, "SUMMARY.json"), "w") as f:
        json.dump(summary, f, indent=1)
    print(f"  oracle: {summary['synthetic']} synthetic files, GDAL agrees on "
          f"{summary['gdal_agrees']}, disagrees on {len(summary['gdal_disagrees'])}, "
          f"cannot open {len(summary['gdal_fails'])}")


if __name__ == "__main__":
    main()
