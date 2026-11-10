#!/usr/bin/env python3
"""HDF5 files with shared object-header messages kept in the SOHM table (superblock v2,
extension with a Shared Message Table), for specular-hdf5's SOHM tests.

Usage: hdf5-sohm.py OUT_DIR

The file-creation property list (H5Pset_shared_mesg_nindexes / _index / _phase_change) is
set through ctypes on the system libhdf5 (h5py does not expose those calls); h5py then fills
the file in r+ mode, so its datatypes, dataspaces, fill values, filter pipelines and
attributes go into the shared-message heaps. Each case gets NAME.json with every dataset's
values and attributes as h5py reads them back (the oracle). Needs numpy and h5py.
"""

import ctypes
import ctypes.util
import json
import os
import sys

import h5py
import numpy as np

# H5Opublic.h: one bit per message type id (dataspace 1, datatype 3, fill 5, pipeline 11,
# attribute 12).
H5O_SHMESG_SDSPACE, H5O_SHMESG_DTYPE, H5O_SHMESG_FILL = 1 << 1, 1 << 3, 1 << 5
H5O_SHMESG_PLINE, H5O_SHMESG_ATTR = 1 << 11, 1 << 12
H5O_SHMESG_ALL = 0x182A


def hdf5():
    name = ctypes.util.find_library("hdf5_serial") or ctypes.util.find_library("hdf5")
    lib = ctypes.CDLL(name or "libhdf5_serial.so.310")
    lib.H5open()
    return lib


def create(path, indexes, list_max, btree_min):
    """`indexes`: (message type flags, minimum message size) per index."""
    lib = hdf5()
    hid = ctypes.c_int64
    fcpl_cls = hid.in_dll(lib, "H5P_CLS_FILE_CREATE_ID_g").value
    lib.H5Pcreate.restype = hid
    fcpl = lib.H5Pcreate(hid(fcpl_cls))
    if lib.H5Pset_shared_mesg_nindexes(hid(fcpl), ctypes.c_uint(len(indexes))) < 0:
        raise RuntimeError("H5Pset_shared_mesg_nindexes")
    for i, (flags, minsize) in enumerate(indexes):
        if lib.H5Pset_shared_mesg_index(hid(fcpl), ctypes.c_uint(i), ctypes.c_uint(flags),
                                        ctypes.c_uint(minsize)) < 0:
            raise RuntimeError("H5Pset_shared_mesg_index")
    if lib.H5Pset_shared_mesg_phase_change(hid(fcpl), ctypes.c_uint(list_max),
                                           ctypes.c_uint(btree_min)) < 0:
        raise RuntimeError("H5Pset_shared_mesg_phase_change")
    lib.H5Fcreate.restype = hid
    f = lib.H5Fcreate(path.encode(), ctypes.c_uint(2), hid(fcpl), hid(0))  # H5F_ACC_TRUNC
    if f < 0:
        raise RuntimeError("H5Fcreate")
    lib.H5Fclose(hid(f))
    lib.H5Pclose(hid(fcpl))


def fill(path, n_sets):
    rng = np.random.default_rng(len(path))
    with h5py.File(path, "r+") as f:
        g = f.create_group("g")
        for k in range(n_sets):
            # The same datatype, dataspace and pipeline again and again: shared.
            d = g.create_dataset(f"d{k}", data=(np.arange(60).reshape(6, 10) * (k + 1))
                                 .astype("<f8"), chunks=(3, 5), compression="gzip",
                                 fillvalue=-1.0)
            d.attrs["units"] = "m"
            d.attrs["scale"] = np.float32(0.5 * k)
            d.attrs["vec"] = np.arange(5, dtype="<i4") + k
        f.create_dataset("ints", data=rng.integers(0, 1000, (7, 9), dtype="<i2"))
        f.attrs["title"] = "SOHM test"


def dump(path):
    out = {}
    with h5py.File(path, "r") as f:
        def visit(name, o):
            attrs = {k: (v.tolist() if hasattr(v, "tolist") else
                         v.decode() if isinstance(v, bytes) else v) for k, v in o.attrs.items()}
            if isinstance(o, h5py.Dataset):
                out[name] = {"shape": list(o.shape), "data": o[()].ravel().tolist(),
                             "attrs": attrs}
            else:
                out[name] = {"attrs": attrs}
        visit("/", f)
        f.visititems(visit)
    with open(path[:-3] + ".json", "w") as fh:
        json.dump(out, fh)


def main():
    out = sys.argv[1]
    os.makedirs(out, exist_ok=True)
    cases = {
        # Every message type in one index, kept as a list.
        "sohm_list": ([(H5O_SHMESG_ALL, 0)], 50, 40),
        # Every message type in one index that turns into a B-tree (version 2) at once.
        "sohm_btree": ([(H5O_SHMESG_ALL, 0)], 0, 0),
        # One index per message type.
        "sohm_split": ([(H5O_SHMESG_SDSPACE, 0), (H5O_SHMESG_DTYPE, 0), (H5O_SHMESG_FILL, 0),
                        (H5O_SHMESG_PLINE, 0), (H5O_SHMESG_ATTR, 0)], 50, 40),
    }
    for name, (indexes, lmax, bmin) in cases.items():
        p = os.path.join(out, f"{name}.h5")
        create(p, indexes, lmax, bmin)
        fill(p, 12)
        dump(p)
        print(p)
    for name in ("h5stat_tsohm", "h5repack_szip"):
        p = os.path.join(out, f"{name}.h5")
        if os.path.exists(p):
            dump(p)


if __name__ == "__main__":
    main()
