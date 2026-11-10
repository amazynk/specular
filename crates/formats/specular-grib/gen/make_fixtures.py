#!/usr/bin/env python3
"""Writes the GRIB test fixtures with ecCodes (the `eccodes` wheel).

    python make_fixtures.py <out-dir>

Each file holds a few small synthetic messages covering one feature: packings, grids,
scanning modes, bitmaps, product templates and editions. The checked-in copies live in
../tests/data; regenerate them with `python make_fixtures.py ../tests/data`. The tests compare
the decoder against ecCodes reading these same files (tests/oracle.rs), so the values written
here need not be checked in anywhere else.
"""

import math
import os
import sys

import eccodes as ec
import numpy as np

OUT = sys.argv[1] if len(sys.argv) > 1 else "."
os.makedirs(OUT, exist_ok=True)
RNG = np.random.default_rng(42)


def field(h, base=270.0, amp=30.0, noise=0.3):
    """A smooth synthetic temperature-like field on the handle's grid."""
    # A fresh grid needs the data section resized before ecCodes iterates over it.
    ni = ec.codes_get(h, "Nx") if ec.codes_is_defined(h, "Nx") else None
    reduced = ec.codes_get(h, "gridType").startswith("reduced")
    n = ni * ec.codes_get(h, "Ny") if ni is not None and not reduced else None
    if n is None:
        n = int(np.sum(ec.codes_get_array(h, "pl")))
    if ec.codes_get_size(h, "values") != n:
        ec.codes_set_values(h, np.arange(n, dtype=float))
    lats = np.array(ec.codes_get_array(h, "latitudes"))
    lons = np.array(ec.codes_get_array(h, "longitudes"))
    v = base + amp * np.cos(np.radians(lats)) ** 2 + 4.0 * np.sin(np.radians(lons) * 3.0)
    v += 2.0 * np.sin(np.radians(lats) * 5.0) * np.cos(np.radians(lons) * 2.0)
    v += RNG.normal(0.0, noise, v.shape)
    return v


def new(sample, **keys):
    h = ec.codes_grib_new_from_samples(sample)
    for k, v in keys.items():
        if isinstance(v, (list, tuple)):
            ec.codes_set_array(h, k, v)
        else:
            ec.codes_set(h, k, v)
    return h


def regular_ll(h, ni, nj, lat1=90.0, lon1=0.0, di=None, dj=None, jpos=0, ineg=0):
    di = di if di is not None else 360.0 / ni
    dj = dj if dj is not None else (2 * lat1) / (nj - 1)
    lat2 = lat1 - (nj - 1) * dj if not jpos else lat1 + (nj - 1) * dj
    lon2 = lon1 + (ni - 1) * di if not ineg else lon1 - (ni - 1) * di
    ec.codes_set(h, "Ni", ni)
    ec.codes_set(h, "Nj", nj)
    ec.codes_set(h, "iScansNegatively", ineg)
    ec.codes_set(h, "jScansPositively", jpos)
    ec.codes_set(h, "latitudeOfFirstGridPointInDegrees", lat1)
    ec.codes_set(h, "longitudeOfFirstGridPointInDegrees", lon1)
    ec.codes_set(h, "latitudeOfLastGridPointInDegrees", lat2)
    ec.codes_set(h, "longitudeOfLastGridPointInDegrees", lon2)
    ec.codes_set(h, "iDirectionIncrementInDegrees", di)
    ec.codes_set(h, "jDirectionIncrementInDegrees", dj)


def write(name, handles):
    with open(os.path.join(OUT, name), "wb") as f:
        for h in handles:
            ec.codes_write(h, f)
            ec.codes_release(h)


def ll2(**keys):
    h = new("regular_ll_pl_grib2", **keys)
    regular_ll(h, 72, 37)
    return h


def set_values(h, v, packing=None, bits=None, missing=None):
    if missing is not None:
        ec.codes_set(h, "bitmapPresent", 1)
        ec.codes_set(h, "missingValue", 9999.0)
        v = np.where(missing, 9999.0, v)
    if bits is not None:
        ec.codes_set(h, "bitsPerValue", bits)
    ec.codes_set_values(h, v)
    if packing is not None:
        ec.codes_set(h, "packingType", packing)


# Simple packing, several parameters, levels and steps (the slider axes).
hs = []
for step in (0, 6):
    for lev in (1000, 850, 500):
        for short, base in (("t", 270.0), ("gh", 5000.0)):
            h = ll2(shortName=short, level=lev, stepRange=str(step), dataDate=20240115, dataTime=1200)
            ec.codes_set(h, "bitsPerValue", 12)
            ec.codes_set(h, "decimalScaleFactor", 0)
            ec.codes_set_values(h, field(h, base, 30.0 + lev / 100.0 + step))
            hs.append(h)
write("simple_axes.grib2", hs)

# Every packing GRIB2 has, on the same field, one with a bitmap of missing points.
hs = []
for packing, bits in (
    ("grid_simple", 16),
    ("grid_complex", 12),
    ("grid_complex_spatial_differencing", 14),
    ("grid_jpeg", 16),
    ("grid_png", 12),
    ("grid_ccsds", 16),
    ("grid_ieee", 32),
):
    for with_bitmap in (False, True):
        h = ll2(shortName="t", level=850)
        v = field(h)
        miss = None
        if with_bitmap:
            lats = np.array(ec.codes_get_array(h, "latitudes"))
            lons = np.array(ec.codes_get_array(h, "longitudes"))
            miss = (lats > 30) & (lats < 60) & (lons > 100) & (lons < 200)
        if packing == "grid_ieee":
            ec.codes_set(h, "packingType", packing)
            ec.codes_set(h, "precision", 1)
            if miss is not None:
                ec.codes_set(h, "bitmapPresent", 1)
                ec.codes_set(h, "missingValue", 9999.0)
                v = np.where(miss, 9999.0, v)
            ec.codes_set_values(h, v)
        else:
            set_values(h, v, None, bits, miss)
            ec.codes_set(h, "packingType", packing)
        hs.append(h)
# Second-order spatial differencing, decimal scaling and a constant field.
h = ll2(shortName="t", level=700)
ec.codes_set(h, "bitsPerValue", 16)
ec.codes_set_values(h, field(h))
ec.codes_set(h, "packingType", "grid_complex_spatial_differencing")
ec.codes_set(h, "orderOfSpatialDifferencing", 2)
ec.codes_set_values(h, field(h))
hs.append(h)
h = ll2(shortName="r", level=700)
ec.codes_set(h, "decimalScaleFactor", 1)
ec.codes_set(h, "bitsPerValue", 10)
ec.codes_set_values(h, np.clip(field(h, 60.0, 30.0, 2.0), 0, 100))
hs.append(h)
h = ll2(shortName="t", level=200)
ec.codes_set_values(h, np.full(72 * 37, 216.5))
hs.append(h)
# CCSDS with other bit widths.
for bits in (8, 24):
    h = ll2(shortName="t", level=300)
    ec.codes_set(h, "bitsPerValue", bits)
    ec.codes_set_values(h, field(h))
    ec.codes_set(h, "packingType", "grid_ccsds")
    hs.append(h)
write("packings.grib2", hs)

# Grids and scanning modes.
hs = []
for jpos, ineg in ((1, 0), (0, 1)):
    h = new("regular_ll_sfc_grib2", shortName="2t")
    regular_ll(h, 36, 19, lat1=-90.0 if jpos else 90.0, lon1=350.0 if ineg else 0.0,
               di=10.0, dj=10.0, jpos=jpos, ineg=ineg)
    ec.codes_set_values(h, field(h))
    hs.append(h)
# A regional lat/lon window crossing the antimeridian.
h = new("regular_ll_sfc_grib2", shortName="msl")
regular_ll(h, 40, 25, lat1=70.0, lon1=150.0, di=2.0, dj=2.0)
ec.codes_set_values(h, field(h, 101000.0, 2000.0, 10.0))
hs.append(h)
h = new("regular_gg_sfc_grib2", shortName="2t", bitsPerValue=12)
ec.codes_set_values(h, field(h))
hs.append(h)
h = new("reduced_gg_pl_32_grib2", shortName="t", bitsPerValue=12)
ec.codes_set_values(h, field(h))
hs.append(h)
h = new("rotated_ll_sfc_grib2", shortName="2t")
ec.codes_set(h, "latitudeOfSouthernPoleInDegrees", -40.0)
ec.codes_set(h, "longitudeOfSouthernPoleInDegrees", 10.0)
ec.codes_set(h, "angleOfRotationInDegrees", 0.0)
regular_ll(h, 40, 30, lat1=-15.0, lon1=-20.0, di=1.0, dj=1.0, jpos=1)
ec.codes_set_values(h, field(h))
hs.append(h)
h = new("polar_stereographic_sfc_grib2", shortName="2t")
ec.codes_set(h, "Nx", 45)
ec.codes_set(h, "Ny", 40)
ec.codes_set(h, "DxInMetres", 150000)
ec.codes_set(h, "DyInMetres", 150000)
ec.codes_set(h, "latitudeOfFirstGridPointInDegrees", 30.0)
ec.codes_set(h, "longitudeOfFirstGridPointInDegrees", 250.0)
ec.codes_set(h, "orientationOfTheGridInDegrees", 255.0)
ec.codes_set(h, "LaDInDegrees", 60.0)
ec.codes_set(h, "jScansPositively", 1)
ec.codes_set_values(h, field(h))
hs.append(h)
h = new("polar_stereographic_sfc_grib2", shortName="2t")
ec.codes_set(h, "Nx", 30)
ec.codes_set(h, "Ny", 30)
ec.codes_set(h, "DxInMetres", 200000)
ec.codes_set(h, "DyInMetres", 200000)
ec.codes_set(h, "projectionCentreFlag", 128)
ec.codes_set(h, "latitudeOfFirstGridPointInDegrees", -50.0)
ec.codes_set(h, "longitudeOfFirstGridPointInDegrees", 225.0)
ec.codes_set(h, "orientationOfTheGridInDegrees", 0.0)
ec.codes_set(h, "LaDInDegrees", -71.0)
# ecCodes walks polar stereographic rows towards +y whatever the scanning mode says, so the
# fixtures scan +j (as NCEP's polar grids do) to compare like with like.
ec.codes_set(h, "jScansPositively", 1)
ec.codes_set_values(h, field(h))
hs.append(h)
h = new("regular_ll_sfc_grib2", shortName="2t")
ec.codes_set(h, "gridType", "lambert")
ec.codes_set(h, "Nx", 50)
ec.codes_set(h, "Ny", 40)
ec.codes_set(h, "DxInMetres", 50000)
ec.codes_set(h, "DyInMetres", 50000)
ec.codes_set(h, "latitudeOfFirstGridPointInDegrees", 25.0)
ec.codes_set(h, "longitudeOfFirstGridPointInDegrees", 250.0)
ec.codes_set(h, "LoVInDegrees", 265.0)
ec.codes_set(h, "LaDInDegrees", 40.0)
ec.codes_set(h, "Latin1InDegrees", 33.0)
ec.codes_set(h, "Latin2InDegrees", 45.0)
ec.codes_set(h, "latitudeOfSouthernPoleInDegrees", -90.0)
ec.codes_set(h, "longitudeOfSouthernPoleInDegrees", 0.0)
ec.codes_set(h, "jScansPositively", 1)
ec.codes_set_values(h, field(h))
hs.append(h)
write("grids.grib2", hs)

# Product templates: ensemble members, accumulations and height/surface levels.
hs = []
h = ll2(shortName="t", level=500, productDefinitionTemplateNumber=1, perturbationNumber=3,
        numberOfForecastsInEnsemble=10)
ec.codes_set_values(h, field(h))
hs.append(h)
h = new("regular_ll_sfc_grib2", shortName="tp", productDefinitionTemplateNumber=8)
regular_ll(h, 72, 37)
ec.codes_set(h, "stepRange", "0-12")
ec.codes_set_values(h, np.abs(field(h, 0.0, 0.01, 0.002)))
hs.append(h)
h = new("regular_ll_sfc_grib2", shortName="10u", typeOfLevel="heightAboveGround", level=10)
regular_ll(h, 72, 37)
ec.codes_set_values(h, field(h, 0.0, 10.0, 1.0))
hs.append(h)
h = ll2(shortName="t", level=850, stepUnits="m", forecastTime=90)
ec.codes_set_values(h, field(h))
hs.append(h)
write("products.grib2", hs)

# GRIB edition 1: simple packing, ECMWF table 128, bitmaps, grids.
hs = []
for short, lev, base in (("t", 850, 270.0), ("z", 500, 55000.0), ("u", 250, 0.0)):
    h = new("regular_ll_pl_grib1", shortName=short, level=lev, dataDate=20230701, dataTime=600,
            stepRange="12")
    regular_ll(h, 72, 37)
    ec.codes_set(h, "bitsPerValue", 16)
    ec.codes_set_values(h, field(h, base, 30.0))
    hs.append(h)
h = new("regular_ll_sfc_grib1", shortName="2t")
regular_ll(h, 72, 37)
lats = np.array(ec.codes_get_array(h, "latitudes"))
v = field(h)
ec.codes_set(h, "bitmapPresent", 1)
ec.codes_set(h, "missingValue", 9999.0)
ec.codes_set_values(h, np.where(lats < -60, 9999.0, v))
hs.append(h)
h = new("regular_ll_sfc_grib1", shortName="sst", decimalScaleFactor=2, bitsPerValue=16)
regular_ll(h, 72, 37)
ec.codes_set_values(h, field(h, 285.0, 15.0, 0.1))
hs.append(h)
h = new("reduced_gg_pl_48_grib1", shortName="2t", bitsPerValue=12)
ec.codes_set_values(h, field(h))
hs.append(h)
h = new("regular_gg_pl_grib1", shortName="t", bitsPerValue=12)
ec.codes_set_values(h, field(h))
hs.append(h)
h = new("rotated_ll_sfc_grib1", shortName="2t")
ec.codes_set(h, "latitudeOfSouthernPoleInDegrees", -30.0)
ec.codes_set(h, "longitudeOfSouthernPoleInDegrees", -15.0)
regular_ll(h, 30, 25, lat1=-12.0, lon1=-15.0, di=1.0, dj=1.0, jpos=1)
ec.codes_set_values(h, field(h))
hs.append(h)
h = new("polar_stereographic_sfc_grib1", shortName="2t")
ec.codes_set_values(h, field(h))
hs.append(h)
h = new("regular_ll_sfc_grib1", shortName="2t")
ec.codes_set(h, "gridType", "lambert")
ec.codes_set(h, "Nx", 40)
ec.codes_set(h, "Ny", 30)
ec.codes_set(h, "DxInMetres", 60000)
ec.codes_set(h, "DyInMetres", 60000)
ec.codes_set(h, "latitudeOfFirstGridPointInDegrees", 30.0)
ec.codes_set(h, "longitudeOfFirstGridPointInDegrees", -110.0)
ec.codes_set(h, "LoVInDegrees", -95.0)
ec.codes_set(h, "Latin1InDegrees", 30.0)
ec.codes_set(h, "Latin2InDegrees", 60.0)
ec.codes_set(h, "jScansPositively", 1)
ec.codes_set_values(h, field(h))
hs.append(h)
write("edition1.grib1", hs)

# Both editions in one file, with junk between messages (as some archives carry).
with open(os.path.join(OUT, "mixed.grb"), "wb") as f:
    h = ll2(shortName="t", level=500)
    ec.codes_set_values(h, field(h))
    ec.codes_write(h, f)
    ec.codes_release(h)
    f.write(b"\0" * 7 + b"junk")
    h = new("regular_ll_sfc_grib1", shortName="msl")
    regular_ll(h, 36, 19)
    ec.codes_set_values(h, field(h, 101000.0, 2000.0, 10.0))
    ec.codes_write(h, f)
    ec.codes_release(h)

for n in sorted(os.listdir(OUT)):
    if n.endswith((".grib1", ".grib2", ".grb")):
        print(n, os.path.getsize(os.path.join(OUT, n)))
