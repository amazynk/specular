"""Dumps every GRIB message of the files given as ecCodes reads them, for tests/oracle.rs.

usage: oracle_dump.py OUT_PREFIX FILE...

Writes OUT_PREFIX.txt, one line per message (file index, then key=value pairs separated by
tabs), and OUT_PREFIX.bin, the message's values, latitudes and longitudes as little-endian
float64 in that order (missing values as NaN).
"""

import sys

import eccodes
import numpy as np

MISSING = 9.999e20
KEYS = [
    "edition",
    "shortName",
    "typeOfLevel",
    "level",
    "validityDate",
    "validityTime",
    "numberOfDataPoints",
    "gridType",
    "Ni",
    "Nj",
    "perturbationNumber",
    "packingType",
    "offset",
    "centre",
]


def main():
    prefix, files = sys.argv[1], sys.argv[2:]
    with open(prefix + ".txt", "w") as txt, open(prefix + ".bin", "wb") as bin_:
        for fi, path in enumerate(files):
            with open(path, "rb") as f:
                while True:
                    h = eccodes.codes_grib_new_from_file(f)
                    if h is None:
                        break
                    try:
                        dump(fi, h, txt, bin_)
                    finally:
                        eccodes.codes_release(h)


def dump(fi, h, txt, bin_):
    parts = [str(fi)]
    for k in KEYS:
        try:
            v = eccodes.codes_get_string(h, k)
        except Exception:
            v = ""
        parts.append(f"{k}={v}")
    eccodes.codes_set_double(h, "missingValue", MISSING)
    vals = eccodes.codes_get_double_array(h, "values")
    vals = np.where(vals == MISSING, np.nan, vals)
    try:
        lats = eccodes.codes_get_double_array(h, "latitudes")
        lons = eccodes.codes_get_double_array(h, "longitudes")
    except Exception:
        lats = np.full(len(vals), np.nan)
        lons = np.full(len(vals), np.nan)
    parts.append(f"count={len(vals)}")
    txt.write("\t".join(parts) + "\n")
    for a in (vals, lats, lons):
        bin_.write(np.asarray(a, dtype="<f8").tobytes())


if __name__ == "__main__":
    main()
