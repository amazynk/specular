#!/usr/bin/env python3
"""Writes HDF5 / NetCDF / EMD test fixtures and their expected values.

Usage: make_fixtures.py OUT_DIR

For every fixture NAME.h5 (or .nc / .emd) it writes NAME.json:

  {"datasets": {path: {"shape": [...], "bin": "NAME.N.bin"}},
   "attrs": {path: {name: value}}}

where each .bin holds the dataset's values as little-endian float64, row-major (exactly as
numpy converts them). The Rust tests compare their reads against these, bit for bit.

Needs numpy and h5py; hdf5plugin (LZ4, Zstandard, bitshuffle, Blosc) and netCDF4 are used
when installed, and their fixtures skipped otherwise.
"""

import json
import os
import sys

import numpy as np
import h5py

try:
    import hdf5plugin
except ImportError:  # pragma: no cover
    hdf5plugin = None
try:
    import netCDF4
except ImportError:  # pragma: no cover
    netCDF4 = None

OUT = sys.argv[1]
os.makedirs(OUT, exist_ok=True)
rng = np.random.default_rng(1234)


def smooth(shape, dtype, scale=1000.0):
    """Compressible but non-trivial data."""
    idx = np.indices(shape).astype(np.float64)
    v = np.zeros(shape)
    for k, a in enumerate(idx):
        v += np.sin(a * (0.05 + 0.03 * k)) * scale / (k + 1)
    v += rng.normal(0, scale * 0.01, shape)
    if np.issubdtype(np.dtype(dtype), np.integer):
        info = np.iinfo(dtype)
        v = np.clip(np.round(v + (info.min + info.max) / 2.0), info.min, info.max)
    return v.astype(dtype)


def attr_json(v):
    if isinstance(v, bytes):
        return v.decode("utf-8", "replace")
    if isinstance(v, str):
        return v
    a = np.asarray(v)
    if a.dtype.kind in "SUO":
        return [x.decode() if isinstance(x, bytes) else str(x) for x in a.ravel()]
    if a.dtype.kind in "iufb":
        return [float(x) for x in a.ravel()]
    return None


def manifest(path, name):
    """Records every numeric dataset (and the attributes) of a written HDF5 file."""
    out = {"datasets": {}, "attrs": {}}
    n = [0]
    with h5py.File(path, "r") as f:
        def visit(p, obj):
            full = "/" + p
            attrs = {}
            for k in obj.attrs:
                try:
                    j = attr_json(obj.attrs[k])
                except Exception:
                    j = None
                if j is not None:
                    attrs[k] = j
            if attrs:
                out["attrs"][full] = attrs
            if isinstance(obj, h5py.Dataset) and obj.dtype.kind in "iuf" and obj.shape is not None:
                data = obj[()]
                b = f"{name}.{n[0]}.bin"
                n[0] += 1
                np.asarray(data, dtype="<f8").tofile(os.path.join(OUT, b))
                out["datasets"][full] = {"shape": list(obj.shape), "bin": b}
            elif isinstance(obj, h5py.Dataset) and obj.dtype.names and obj.shape is not None:
                # A compound dataset: each numeric field as "path[field]".
                data = obj[()]
                for fname, values in compound_fields(data):
                    b = f"{name}.{n[0]}.bin"
                    n[0] += 1
                    np.asarray(values, dtype="<f8").tofile(os.path.join(OUT, b))
                    out["datasets"][f"{full}[{fname}]"] = {"shape": list(obj.shape), "bin": b}
            elif isinstance(obj, h5py.Dataset) and obj.dtype.kind == "c":
                # h5py reads a compound of two floats named r and i as complex.
                data = obj[()]
                for fname, values in [("r", data.real), ("i", data.imag)]:
                    b = f"{name}.{n[0]}.bin"
                    n[0] += 1
                    np.asarray(values, dtype="<f8").tofile(os.path.join(OUT, b))
                    out["datasets"][f"{full}[{fname}]"] = {"shape": list(obj.shape), "bin": b}
        f.visititems(visit)
        root = {}
        for k in f.attrs:
            j = attr_json(f.attrs[k])
            if j is not None:
                root[k] = j
        if root:
            out["attrs"]["/"] = root
    with open(os.path.join(OUT, name + ".json"), "w") as fh:
        json.dump(out, fh)


def compound_fields(data, prefix=""):
    """(name, values) of every numeric field of a compound array, nested ones as a.b and
    small arrays element by element as a[i]."""
    out = []
    for fname in data.dtype.names:
        v = data[fname]
        dt = data.dtype.fields[fname][0]
        full = prefix + fname
        if dt.names:
            out += compound_fields(v, full + ".")
        elif dt.subdtype is not None:
            base, shape = dt.subdtype
            k = int(np.prod(shape))
            if base.kind in "iuf" and k <= 16:
                flat = v.reshape(v.shape[: v.ndim - len(shape)] + (k,))
                for i in range(k):
                    out.append((f"{full}[{i}]", flat[..., i]))
        elif dt.kind in "iuf":
            out.append((full, v))
    return out


def compact(f, name, data):
    space = h5py.h5s.create_simple(data.shape)
    dcpl = h5py.h5p.create(h5py.h5p.DATASET_CREATE)
    dcpl.set_layout(h5py.h5d.COMPACT)
    tid = h5py.h5t.py_create(data.dtype)
    dsid = h5py.h5d.create(f.id, name.encode(), tid, space, dcpl=dcpl)
    dsid.write(h5py.h5s.ALL, h5py.h5s.ALL, data)


def implicit(f, name, data, chunks):
    space = h5py.h5s.create_simple(data.shape)
    dcpl = h5py.h5p.create(h5py.h5p.DATASET_CREATE)
    dcpl.set_chunk(chunks)
    dcpl.set_alloc_time(h5py.h5d.ALLOC_TIME_EARLY)
    tid = h5py.h5t.py_create(data.dtype)
    dsid = h5py.h5d.create(f.id, name.encode(), tid, space, dcpl=dcpl)
    dsid.write(h5py.h5s.ALL, h5py.h5s.ALL, data)


def fixture(name, libver, build, ext=".h5", **kw):
    path = os.path.join(OUT, name + ext)
    with h5py.File(path, "w", libver=libver, **kw) as f:
        build(f)
    manifest(path, name)


# 1. Earliest format: superblock 0, symbol-table groups, v1 object headers, B-tree v1 chunks.
def old(f):
    f.attrs["title"] = "old style"
    f.attrs["answer"] = np.int32(42)
    f.attrs["scales"] = np.array([1.5, -2.25, 3.0])
    f.attrs.create("fixed", np.bytes_("fixed string"))
    f.attrs["vlen"] = "variable string"
    f.create_dataset("contig_f32", data=smooth((37, 53), np.float32))
    f.create_dataset("contig_i16be", data=smooth((20, 30), ">i2"))
    f.create_dataset("u8", data=smooth((16, 16), np.uint8))
    compact(f, "compact_u16", smooth((8, 12), np.uint16))
    g = f.create_group("chunked")
    g.create_dataset("deflate", data=smooth((100, 130), np.float32), chunks=(32, 32),
                     compression="gzip", shuffle=True, fletcher32=True)
    d = g.create_dataset("sparse", shape=(64, 64), dtype=np.int32, chunks=(16, 16),
                         fillvalue=-7)
    d[0:16, 16:48] = smooth((16, 32), np.int32)
    g.create_dataset("cube", data=smooth((5, 40, 30), np.uint16), chunks=(2, 16, 16),
                     compression="lzf")
    many = f.create_group("many")
    for i in range(70):
        many.create_dataset(f"d{i:03}", data=np.full((2, 2), i, dtype=np.int8))
    for dt in ["<f2", ">f8", "<i8", ">u4", "<u8", "i1"]:
        f.create_dataset("types/" + dt.replace("<", "le_").replace(">", "be_"),
                         data=smooth((9, 11), dt, 100.0))


fixture("old", "earliest", old)


# 2. Latest format: superblock 3, v2 object headers, every v4 chunk index, dense groups.
def new(f):
    f.attrs["title"] = "new style"
    for i in range(12):
        f.attrs[f"a{i:02}"] = np.float64(i) / 4
    f.attrs["big"] = np.arange(20000, dtype=np.int32)
    f.create_dataset("single", data=smooth((30, 40), np.float32), chunks=(30, 40))
    f.create_dataset("single_filtered", data=smooth((30, 40), np.float32), chunks=(30, 40),
                     compression="gzip")
    implicit(f, "implicit", smooth((50, 60), np.int16), (16, 16))
    f.create_dataset("fixed", data=smooth((70, 90), np.float64), chunks=(16, 16),
                     compression="gzip", shuffle=True)
    f.create_dataset("fixed_paged", data=smooth((1100, 1200), np.uint8), chunks=(32, 32),
                     compression="gzip")
    f.create_dataset("fixed_unfiltered", data=smooth((40, 40), np.int32), chunks=(8, 8))
    ea = f.create_dataset("earray", shape=(0, 24), maxshape=(None, 24), dtype=np.float32,
                          chunks=(1, 8), compression="gzip")
    ea.resize((600, 24))
    ea[...] = smooth((600, 24), np.float32)
    eb = f.create_dataset("earray_last", shape=(20, 0), maxshape=(20, None), dtype=np.int16,
                          chunks=(4, 3))
    eb.resize((20, 50))
    eb[...] = smooth((20, 50), np.int16)
    bt = f.create_dataset("btree2", shape=(10, 10), maxshape=(None, None), dtype=np.float32,
                          chunks=(4, 4), compression="gzip")
    bt.resize((70, 90))
    bt[...] = smooth((70, 90), np.float32)
    bu = f.create_dataset("btree2_raw", shape=(10, 10), maxshape=(None, None), dtype=np.uint16,
                          chunks=(3, 5))
    bu.resize((41, 33))
    bu[...] = smooth((41, 33), np.uint16)
    g = f.create_group("dense")
    for i in range(40):
        g.create_dataset(f"m{i:02}", data=np.full((3, 2), i, dtype=np.float32))
    f.create_dataset("rgb", data=smooth((24, 32, 3), np.uint8))


fixture("new", "latest", new)

# 3. A user block before the superblock.
fixture("userblock", "earliest",
        lambda f: f.create_dataset("img", data=smooth((20, 25), np.float32)),
        userblock_size=1024)


# 4. Plugin filters.
def plugins(f):
    base = smooth((96, 80), np.float32)
    f.create_dataset("lzf", data=base, chunks=(32, 32), compression="lzf")
    if hdf5plugin is None:
        return
    f.create_dataset("lz4", data=base, chunks=(32, 32), **hdf5plugin.LZ4())
    f.create_dataset("zstd", data=base, chunks=(32, 32), **hdf5plugin.Zstd())
    f.create_dataset("bshuf_none", data=base, chunks=(32, 32), **hdf5plugin.Bitshuffle(cname="none"))
    f.create_dataset("bshuf_lz4", data=base, chunks=(32, 32), **hdf5plugin.Bitshuffle(cname="lz4"))
    f.create_dataset("bshuf_zstd", data=base, chunks=(32, 32), **hdf5plugin.Bitshuffle(cname="zstd"))
    u16 = smooth((300, 257), np.uint16)
    f.create_dataset("bshuf_u16", data=u16, chunks=(100, 100), **hdf5plugin.Bitshuffle(cname="lz4"))
    for cname in ["blosclz", "lz4", "lz4hc", "zlib", "zstd"]:
        for sh, sname in [(hdf5plugin.Blosc.NOSHUFFLE, "noshuf"), (hdf5plugin.Blosc.SHUFFLE, "shuf"),
                          (hdf5plugin.Blosc.BITSHUFFLE, "bitshuf")]:
            f.create_dataset(f"blosc_{cname}_{sname}", data=base, chunks=(32, 32),
                             **hdf5plugin.Blosc(cname=cname, clevel=5, shuffle=sh))
            f.create_dataset(f"blosc_{cname}_{sname}_u16", data=u16, chunks=(100, 100),
                             **hdf5plugin.Blosc(cname=cname, clevel=9, shuffle=sh))
    # Large chunks: several Blosc blocks, BloscLZ long matches.
    big = np.tile(smooth((64, 64), np.float32), (8, 8))
    f.create_dataset("blosc_big", data=big, chunks=(512, 512),
                     **hdf5plugin.Blosc(cname="blosclz", clevel=9, shuffle=hdf5plugin.Blosc.SHUFFLE))
    f.create_dataset("blosc_big_noshuf", data=big, chunks=(512, 512),
                     **hdf5plugin.Blosc(cname="blosclz", clevel=9, shuffle=hdf5plugin.Blosc.NOSHUFFLE))


fixture("plugins", "earliest", plugins)


# 4b. N-bit, scale-offset and Blosc2.
def nbit(f, name, data, prec, offset, chunks):
    tid = h5py.h5t.py_create(data.dtype).copy()
    tid.set_precision(prec)
    tid.set_offset(offset)
    space = h5py.h5s.create_simple(data.shape)
    dcpl = h5py.h5p.create(h5py.h5p.DATASET_CREATE)
    dcpl.set_chunk(chunks)
    dcpl.set_filter(h5py.h5z.FILTER_NBIT, h5py.h5z.FLAG_OPTIONAL, ())
    dsid = h5py.h5d.create(f.id, name.encode(), tid, space, dcpl=dcpl)
    dsid.write(h5py.h5s.ALL, h5py.h5s.ALL, data)


def packers(f):
    u16 = smooth((60, 70), np.uint16) % 4096
    nbit(f, "nbit_u16_12", u16, 12, 0, (32, 32))
    nbit(f, "nbit_u16_12_off3", u16.astype(np.uint16) & 0x0FFF, 12, 3, (25, 70))
    i32 = (smooth((40, 50), np.int32) % 2000).astype(np.int32) - 1000
    nbit(f, "nbit_i32_be", i32.astype(">i4"), 32, 0, (40, 50))
    u8 = smooth((33, 47), np.uint8) & 0x3F
    nbit(f, "nbit_u8_6", u8, 6, 0, (16, 16))
    so_i = smooth((80, 90), np.int16)
    f.create_dataset("so_i16", data=so_i, chunks=(32, 32), scaleoffset=0)
    f.create_dataset("so_i16_fill", data=so_i, chunks=(32, 32), scaleoffset=0, fillvalue=7)
    f.create_dataset("so_u32_bits", data=smooth((50, 60), np.uint32), chunks=(20, 20), scaleoffset=12)
    f.create_dataset("so_i64", data=smooth((30, 40), np.int64), chunks=(30, 40), scaleoffset=0)
    f.create_dataset("so_const", data=np.full((20, 30), 5, np.int32), chunks=(20, 30), scaleoffset=0)
    f.create_dataset("so_f32_d2", data=smooth((64, 48), np.float32), chunks=(32, 32), scaleoffset=2)
    f.create_dataset("so_f64_d3", data=smooth((40, 30), np.float64), chunks=(16, 30), scaleoffset=3)
    f.create_dataset("so_f32_be", data=smooth((40, 30), np.float32).astype(">f4"), chunks=(40, 30),
                     scaleoffset=1)
    if hdf5plugin is None:
        return
    base = smooth((96, 80), np.float32)
    u16b = smooth((300, 257), np.uint16)
    vol = smooth((12, 70, 90), np.int16)
    for cname in ["blosclz", "lz4", "lz4hc", "zlib", "zstd"]:
        for sh, sname in [(hdf5plugin.Blosc2.NOFILTER, "nofilt"), (hdf5plugin.Blosc2.SHUFFLE, "shuf"),
                          (hdf5plugin.Blosc2.BITSHUFFLE, "bitshuf")]:
            opts = dict(cname=cname, clevel=5, filters=sh)
            f.create_dataset(f"blosc2_{cname}_{sname}", data=base, chunks=(32, 32), **hdf5plugin.Blosc2(**opts))
            f.create_dataset(f"blosc2_{cname}_{sname}_u16", data=u16b, chunks=(100, 100),
                             **hdf5plugin.Blosc2(**opts))
            f.create_dataset(f"blosc2_{cname}_{sname}_vol", data=vol, chunks=(5, 64, 64),
                             **hdf5plugin.Blosc2(**opts))
            f.create_dataset(f"blosc2_{cname}_{sname}_1d", data=base.ravel(), chunks=(3001,),
                             **hdf5plugin.Blosc2(**opts))
    # Big chunks: several Blosc2 blocks per chunk, blocks smaller than the chunk.
    big = np.tile(smooth((64, 64), np.float32), (8, 8))
    f.create_dataset("blosc2_big", data=big, chunks=(512, 512),
                     **hdf5plugin.Blosc2(cname="lz4", clevel=9, filters=hdf5plugin.Blosc2.SHUFFLE))
    f.create_dataset("blosc2_zeros", data=np.zeros((64, 64), np.float32), chunks=(32, 64),
                     **hdf5plugin.Blosc2(cname="zstd", clevel=5))
    f.create_dataset("blosc2_f64", data=smooth((50, 70), np.float64), chunks=(50, 70),
                     **hdf5plugin.Blosc2(cname="zlib", clevel=5, filters=hdf5plugin.Blosc2.SHUFFLE))


    # Delta: blocks XORed with the chunk's first block (several blocks per chunk).
    for dt in [np.float32, np.uint16, np.int32, np.float64, np.uint8]:
        name = np.dtype(dt).name
        d = smooth((256, 300), dt)
        for sh, sname in [(hdf5plugin.Blosc2.DELTA, "delta")]:
            f.create_dataset(f"blosc2_{name}_{sname}", data=d, chunks=(256, 300),
                             **hdf5plugin.Blosc2(cname="lz4", clevel=5, filters=sh))
        f.create_dataset(f"blosc2_{name}_delta_small", data=d, chunks=(13, 7),
                         **hdf5plugin.Blosc2(cname="zstd", clevel=5, filters=hdf5plugin.Blosc2.DELTA))


fixture("packers", "earliest", packers)


# 4d. Compound datasets: each numeric field read on its own (also nested compounds, small
# numeric arrays and big-endian members); old (version 1/2) and new (version 3) encodings.
def compound(f):
    inner = np.dtype([("a", np.uint8), ("b", ">f8")])
    dt = np.dtype([("x", np.float32), ("y", np.int16), ("name", "S5"), ("pos", inner),
                   ("arr", np.int32, (3,)), ("z", ">u4")])
    v = np.zeros((30, 40), dt)
    v["x"] = smooth((30, 40), np.float32)
    v["y"] = smooth((30, 40), np.int16)
    v["name"] = b"abc"
    v["pos"]["a"] = smooth((30, 40), np.uint8)
    v["pos"]["b"] = smooth((30, 40), np.float64)
    for i in range(3):
        v["arr"][..., i] = smooth((30, 40), np.int32) + i
    v["z"] = smooth((30, 40), np.uint32)
    f.create_dataset("table", data=v)
    f.create_dataset("chunked", data=v, chunks=(16, 16), compression="gzip", shuffle=True)
    cplx = np.zeros((20, 25), np.complex64)
    cplx.real = smooth((20, 25), np.float32)
    cplx.imag = -smooth((20, 25), np.float32)
    f.create_dataset("complex", data=cplx.view(np.dtype([("r", np.float32), ("i", np.float32)])))
    f.create_dataset("rows", data=v[0])


fixture("compound_old", "earliest", compound)
fixture("compound_new", "latest", compound)


# 4e. Virtual datasets mapping from this file: a strided source, part of a space (the rest
# the fill value), a mosaic of tiles and a transposed-shape stack.
def virtual_local(f):
    f.create_dataset("src", data=smooth((40, 60), np.float32))
    for k in range(4):
        f.create_dataset(f"tile{k}", data=smooth((20, 30), np.int16) + 100 * k)
    lay = h5py.VirtualLayout(shape=(20, 60), dtype=np.float32)
    lay[:, :] = h5py.VirtualSource(f["src"])[::2, :]
    f.create_virtual_dataset("strided", lay, fillvalue=-1)
    lay = h5py.VirtualLayout(shape=(50, 70), dtype=np.float32)
    lay[5:45, 3:63] = h5py.VirtualSource(f["src"])
    f.create_virtual_dataset("part", lay, fillvalue=-2.5)
    lay = h5py.VirtualLayout(shape=(40, 60), dtype=np.int16)
    for k in range(4):
        y, x = divmod(k, 2)
        lay[y * 20:(y + 1) * 20, x * 30:(x + 1) * 30] = h5py.VirtualSource(f[f"tile{k}"])
    f.create_virtual_dataset("mosaic", lay, fillvalue=0)
    lay = h5py.VirtualLayout(shape=(4, 20, 30), dtype=np.int16)
    for k in range(4):
        lay[3 - k] = h5py.VirtualSource(f[f"tile{k}"])
    f.create_virtual_dataset("stack", lay)
    # A source block whose rows land strided in the virtual space.
    lay = h5py.VirtualLayout(shape=(40, 30), dtype=np.int16)
    lay[1::2, :] = h5py.VirtualSource(f["tile1"])
    f.create_virtual_dataset("interleaved", lay, fillvalue=9)


fixture("virtual_local", "latest", virtual_local)
fixture("virtual_local_v110", "v110", virtual_local)


# 4f. A virtual dataset stacking frames kept in other files (one of them missing).
VDS_FRAMES = [smooth((25, 35), np.uint16) + 1000 * k for k in range(3)]
for k in range(3):
    with h5py.File(os.path.join(OUT, f"vds_part{k}.h5"), "w") as f:
        f.create_dataset("data", data=VDS_FRAMES[k])
with h5py.File(os.path.join(OUT, "vds_master.h5"), "w", libver="latest") as f:
    lay = h5py.VirtualLayout(shape=(4, 25, 35), dtype=np.uint16)
    for k in range(4):
        lay[k] = h5py.VirtualSource(f"vds_part{k}.h5", "data", shape=(25, 35))
    f.create_virtual_dataset("frames", lay, fillvalue=3)
want = np.concatenate([np.stack(VDS_FRAMES), np.full((1, 25, 35), 3, np.uint16)])
want.astype("<f8").tofile(os.path.join(OUT, "vds_master.bin"))


# 4c. SZIP (filter 4, libaec in h5py's wheels): both coding methods, every sample width,
# scanlines that don't fill a block, constant runs (zero blocks), noise (uncompressed blocks).
def szip_sets(f):
    if not h5py.h5z.filter_avail(h5py.h5z.FILTER_SZIP):
        return
    for dt in [np.uint8, np.int8, np.uint16, np.int16, np.uint32, np.int32, np.int64,
               np.float32, np.float64]:
        name = np.dtype(dt).name
        base = smooth((70, 90), dt)
        base[10:30, :] = base[10, 0]
        f.create_dataset(f"nn_{name}", data=base, chunks=(35, 45), compression="szip",
                         compression_opts=("nn", 32))
        f.create_dataset(f"ec_{name}", data=base, chunks=(70, 90), compression="szip",
                         compression_opts=("ec", 16))
    for ppb in [8, 16, 32]:
        # 10 bits of noise: split-sample blocks with large k (incompressible chunks would be
        # stored unfiltered).
        noisy = rng.integers(0, 1024, (40, 37)).astype(np.uint16)
        f.create_dataset(f"noise_{ppb}", data=noisy, chunks=(20, 37), compression="szip",
                         compression_opts=("nn", ppb))
    f.create_dataset("zeros", data=np.zeros((64, 128), np.int16), chunks=(64, 128),
                     compression="szip", compression_opts=("nn", 32))
    big = np.tile(smooth((64, 64), np.float32), (2, 9))
    f.create_dataset("odd_rows", data=big[:, :555], chunks=(128, 555), compression="szip",
                     compression_opts=("nn", 32))
    f.create_dataset("be_u16", data=smooth((50, 60), np.dtype(">u2")), chunks=(25, 60),
                     compression="szip", compression_opts=("nn", 16))


fixture("szip", "earliest", szip_sets)


# 5. EMD, Berkeley/NCEM convention.
def berkeley(f):
    f.attrs["version_major"] = 0
    f.attrs["version_minor"] = 2
    g = f.create_group("data/haadf")
    g.attrs["emd_group_type"] = 1
    img = smooth((48, 64), np.float32)
    g.create_dataset("data", data=img)
    d1 = g.create_dataset("dim1", data=np.arange(48) * 0.25)
    d1.attrs["name"] = np.bytes_("y")
    d1.attrs["units"] = np.bytes_("[n_m]")
    d2 = g.create_dataset("dim2", data=np.arange(64) * 0.25)
    d2.attrs["name"] = np.bytes_("x")
    d2.attrs["units"] = np.bytes_("[n_m]")
    m = f.create_group("microscope")
    m.attrs["voltage"] = 300.0
    m.attrs["name"] = "TEAM 0.5"


fixture("berkeley", "earliest", berkeley, ext=".emd")


# 6. EMD, Velox convention.
def velox(f):
    f.create_dataset("Version", data=np.array([b'{"format": "Velox", "version": "2"}']))
    g = f.create_group("Data/Image/6d4b9f1c2b6e4ad1a0b8e1f2c3d4e5f6")
    frames = smooth((40, 56, 2), np.uint16)
    g.create_dataset("Data", data=frames, chunks=(40, 56, 1))
    meta = {
        "BinaryResult": {"PixelSize": {"width": "2.5e-10", "height": "2.5e-10"},
                         "Offset": {"x": "-7e-9", "y": "-5e-9"},
                         "PixelUnitX": "m", "PixelUnitY": "m", "Detector": "HAADF"},
        "Instrument": {"InstrumentModel": "Talos F200X"},
        "Acquisition": {"AcquisitionStartDatetime": {"DateTime": "1700000000"}},
        "Optics": {"AccelerationVoltage": "200000"},
    }
    raw = json.dumps(meta).encode()
    m = np.zeros((len(raw) + 16, 2), dtype=np.uint8)
    m[: len(raw), 0] = np.frombuffer(raw, dtype=np.uint8)
    m[: len(raw), 1] = np.frombuffer(raw, dtype=np.uint8)
    g.create_dataset("Metadata", data=m)
    # The acquiring STEM detector segment names the image (variable-length JSON string).
    op = {"dataPath": "/Data/Image/6d4b9f1c2b6e4ad1a0b8e1f2c3d4e5f6", "detector": "HAADF-A"}
    f.create_dataset("Operations/StemInputOperation/0a1b2c", data=[json.dumps(op)],
                     dtype=h5py.string_dtype())


fixture("velox", "earliest", velox, ext=".emd")


# 8. NeXus: the @default chain (NXroot -> NXentry -> NXdata -> signal and axes), with a
# larger decoy dataset named "data" that the plain ranking would put first.
def nexus(f):
    f.attrs["NX_class"] = "NXroot"
    f.attrs["default"] = "scan"
    decoy = f.create_group("other")
    decoy.attrs["NX_class"] = "NXentry"
    decoy.create_dataset("data", data=smooth((64, 80), np.float32))
    e = f.create_group("scan")
    e.attrs["NX_class"] = "NXentry"
    e.attrs["default"] = "plot"
    g = e.create_group("plot")
    g.attrs["NX_class"] = "NXdata"
    g.attrs["signal"] = "counts"
    g.attrs["axes"] = np.array(["energy", ".", "x"], dtype=h5py.string_dtype())
    g.attrs["energy_indices"] = np.int32(0)
    g.attrs["x_indices"] = np.int32(2)
    c = g.create_dataset("counts", data=smooth((3, 20, 30), np.int32))
    c.attrs["units"] = "counts"
    c.attrs["long_name"] = "detector counts"
    en = g.create_dataset("energy", data=np.array([8.0, 8.5, 9.0]))
    en.attrs["units"] = "keV"
    g.create_dataset("x", data=np.linspace(-1.0, 1.0, 30))
    g.create_dataset("y", data=np.linspace(0.0, 2.0, 20))


fixture("nexus", "earliest", nexus, ext=".nxs")


# 9. An Eiger-like master file whose frames live in data files behind external links, and
# a master in a subfolder whose links leave it.
EIGER_FRAMES = [smooth((3, 40, 50), np.uint32), smooth((2, 40, 50), np.uint32) + 7]


def eiger_data(k):
    def build(f):
        g = f.create_group("entry/data")
        g.create_dataset("data", data=EIGER_FRAMES[k], chunks=(1, 40, 50), compression="gzip")
    return build


def eiger_master(prefix):
    def build(f):
        f.attrs["default"] = "entry"
        e = f.create_group("entry")
        e.attrs["NX_class"] = "NXentry"
        e.attrs["default"] = "data"
        d = e.create_group("data")
        d.attrs["NX_class"] = "NXdata"
        d["data_000001"] = h5py.ExternalLink(prefix + "eiger_data_000001.h5", "/entry/data/data")
        d["data_000002"] = h5py.ExternalLink(prefix + "eiger_data_000002.h5", "/entry/data/data")
        det = e.create_group("instrument/detector")
        det.attrs["NX_class"] = "NXdetector"
        det.create_dataset("description", data=np.bytes_("Dectris EIGER2 Si 500K"))
        det.create_dataset("x_pixel_size", data=7.5e-05)
    return build


fixture("eiger_data_000001", "earliest", eiger_data(0))
fixture("eiger_data_000002", "earliest", eiger_data(1))
fixture("eiger_master", "earliest", eiger_master(""))
os.makedirs(os.path.join(OUT, "escape"), exist_ok=True)
with h5py.File(os.path.join(OUT, "escape", "eiger_master.h5"), "w", libver="earliest") as f:
    eiger_master("../")(f)


# 10. Imaris 5.5: resolution levels, time points and channels; blocks padded past the image.
def ims_text(v):
    return np.frombuffer(str(v).encode(), dtype="S1")


IMS = {}


def imaris(f):
    f.attrs["ImarisDataSet"] = ims_text("ImarisDataSet")
    f.attrs["ImarisVersion"] = ims_text("5.5.0")
    f.attrs["DataSetDirectoryName"] = ims_text("DataSet")
    f.attrs["DataSetInfoDirectoryName"] = ims_text("DataSetInfo")
    img = f.create_group("DataSetInfo/Image")
    for k, v in {"X": 50, "Y": 40, "Z": 6, "Unit": "um", "ExtMin0": 0, "ExtMin1": 0,
                 "ExtMin2": 0, "ExtMax0": 25, "ExtMax1": 20, "ExtMax2": 12,
                 "Name": "cells"}.items():
        img.attrs[k] = ims_text(v)
    for c, (name, colour) in enumerate([("DAPI", "0 0 1"), ("GFP", "0 1 0")]):
        ch = f.create_group(f"DataSetInfo/Channel {c}")
        ch.attrs["Name"] = ims_text(name)
        ch.attrs["Color"] = ims_text(colour)
    t = f.create_group("DataSetInfo/TimeInfo")
    t.attrs["DatasetTimePoints"] = ims_text(2)
    t.attrs["TimePoint1"] = ims_text("2024-05-01 10:00:00.000")
    t.attrs["TimePoint2"] = ims_text("2024-05-01 10:00:30.000")
    sizes = [(6, 40, 50, (8, 48, 64)), (3, 20, 25, (4, 32, 32))]
    for r, (z, y, x, stored) in enumerate(sizes):
        for tp in range(2):
            for c in range(2):
                v = smooth((z, y, x), np.uint16) + 100 * (c + 2 * tp)
                full = np.zeros(stored, np.uint16)
                full[:z, :y, :x] = v
                IMS[(r, tp, c)] = v
                g = f.create_group(f"DataSet/ResolutionLevel {r}/TimePoint {tp}/Channel {c}")
                g.attrs["ImageSizeX"] = ims_text(x)
                g.attrs["ImageSizeY"] = ims_text(y)
                g.attrs["ImageSizeZ"] = ims_text(z)
                g.attrs["HistogramMin"] = ims_text(int(v.min()))
                g.attrs["HistogramMax"] = ims_text(int(v.max()))
                g.create_dataset("Data", data=full, chunks=(min(stored[0], 4), 16, 32),
                                 compression="gzip")
                g.create_dataset("Histogram", data=np.ones(256, np.uint64))


fixture("imaris", "earliest", imaris, ext=".ims")


# 11. BAG (bathymetry): elevation and uncertainty with the 1e6 no-data value, rows south
# first, and the ISO 19115 metadata with the grid's corners.
BAG_XML = """<?xml version="1.0" encoding="UTF-8"?>
<gmi:MI_Metadata xmlns:gmi="http://www.isotc211.org/2005/gmi" xmlns:gmd="http://www.isotc211.org/2005/gmd" xmlns:gco="http://www.isotc211.org/2005/gco" xmlns:gml="http://www.opengis.net/gml/3.2">
 <gmd:spatialRepresentationInfo><gmd:MD_Georectified>
  <gmd:axisDimensionProperties><gmd:MD_Dimension>
   <gmd:dimensionName><gmd:MD_DimensionNameTypeCode codeListValue="row">row</gmd:MD_DimensionNameTypeCode></gmd:dimensionName>
   <gmd:dimensionSize><gco:Integer>30</gco:Integer></gmd:dimensionSize>
   <gmd:resolution><gco:Measure uom="Metres">4</gco:Measure></gmd:resolution>
  </gmd:MD_Dimension></gmd:axisDimensionProperties>
  <gmd:axisDimensionProperties><gmd:MD_Dimension>
   <gmd:dimensionName><gmd:MD_DimensionNameTypeCode codeListValue="column">column</gmd:MD_DimensionNameTypeCode></gmd:dimensionName>
   <gmd:dimensionSize><gco:Integer>40</gco:Integer></gmd:dimensionSize>
   <gmd:resolution><gco:Measure uom="Metres">2</gco:Measure></gmd:resolution>
  </gmd:MD_Dimension></gmd:axisDimensionProperties>
  <gmd:cornerPoints><gml:Point gml:id="id1"><gml:coordinates cs="," decimal="." ts=" ">500000,4000000 500078,4000116</gml:coordinates></gml:Point></gmd:cornerPoints>
 </gmd:MD_Georectified></gmd:spatialRepresentationInfo>
 <gmd:referenceSystemInfo><gmd:MD_ReferenceSystem><gmd:referenceSystemIdentifier><gmd:RS_Identifier>
  <gmd:code><gco:CharacterString>32618</gco:CharacterString></gmd:code>
  <gmd:codeSpace><gco:CharacterString>EPSG</gco:CharacterString></gmd:codeSpace>
 </gmd:RS_Identifier></gmd:referenceSystemIdentifier></gmd:MD_ReferenceSystem></gmd:referenceSystemInfo>
</gmi:MI_Metadata>
"""


def bag(f):
    g = f.create_group("BAG_root")
    g.attrs["Bag Version"] = np.bytes_("1.6.2")
    g.create_dataset("metadata", data=np.frombuffer(BAG_XML.encode(), dtype="S1"),
                     maxshape=(None,), chunks=(1024,))
    elev = -20.0 - smooth((30, 40), np.float32, 5.0)
    elev[0:3, 0:4] = 1e6
    unc = np.abs(smooth((30, 40), np.float32, 0.5)) + 0.1
    unc[0:3, 0:4] = 1e6
    e = g.create_dataset("elevation", data=elev.astype(np.float32), chunks=(10, 40),
                         compression="gzip", fillvalue=np.float32(1e6))
    e.attrs["Minimum Elevation Value"] = np.float32(elev[elev < 1e5].min())
    e.attrs["Maximum Elevation Value"] = np.float32(elev[elev < 1e5].max())
    g.create_dataset("uncertainty", data=unc.astype(np.float32), chunks=(10, 40),
                     compression="gzip", fillvalue=np.float32(1e6))


fixture("bag", "earliest", bag, ext=".bag")


# 7. NetCDF.
def nc_manifest(path, name):
    """Expected values of every variable, scaled and with fill values as NaN."""
    out = {"datasets": {}, "attrs": {}}
    with netCDF4.Dataset(path) as ds:
        ds.set_auto_maskandscale(True)
        for i, (vname, v) in enumerate(ds.variables.items()):
            if v.dtype.kind not in "iuf":
                continue
            a = np.ma.filled(np.ma.asarray(v[...]).astype(np.float64), np.nan)
            b = f"{name}.{i}.bin"
            np.asarray(a, dtype="<f8").tofile(os.path.join(OUT, b))
            out["datasets"][vname] = {"shape": list(v.shape), "bin": b,
                                      "dims": list(v.dimensions)}
            out["attrs"][vname] = {k: attr_json(v.getncattr(k)) for k in v.ncattrs()}
        out["attrs"]["/"] = {k: attr_json(ds.getncattr(k)) for k in ds.ncattrs()}
    with open(os.path.join(OUT, name + ".json"), "w") as fh:
        json.dump(out, fh)


def nc(name, fmt):
    path = os.path.join(OUT, name + ".nc")
    with netCDF4.Dataset(path, "w", format=fmt) as ds:
        ds.title = "test " + fmt
        ds.createDimension("time", None)
        ds.createDimension("lat", 18)
        ds.createDimension("lon", 24)
        lat = ds.createVariable("lat", "f4", ("lat",))
        lat.units = "degrees_north"
        lat[:] = np.linspace(-85, 85, 18)
        lon = ds.createVariable("lon", "f8", ("lon",))
        lon.units = "degrees_east"
        lon[:] = np.linspace(0, 345, 24)
        t = ds.createVariable("time", "i4", ("time",))
        t.units = "days since 2000-01-01"
        t[:] = [0, 31, 59]
        tas = ds.createVariable("tas", "i2", ("time", "lat", "lon"), fill_value=-999)
        tas.scale_factor = 0.01
        tas.add_offset = 273.15
        tas.units = "K"
        v = smooth((3, 18, 24), np.float32, 20.0) + 10
        v[0, 2, 3] = np.nan
        tas[:] = np.ma.masked_invalid(v)
        elev = ds.createVariable("elev", "f4", ("lat", "lon"))
        elev[:] = smooth((18, 24), np.float32)
        if fmt == "NETCDF3_64BIT_DATA":
            u = ds.createVariable("u64", "u8", ("lat", "lon"))
            u[:] = smooth((18, 24), np.uint64)
            b = ds.createVariable("i8", "i1", ("lat", "lon"))
            b[:] = smooth((18, 24), np.int8)
        if fmt == "NETCDF4":
            c = ds.createVariable("chunked", "f4", ("time", "lat", "lon"), zlib=True,
                                  chunksizes=(1, 9, 12))
            c[:] = smooth((3, 18, 24), np.float32)
    nc_manifest(path, name)


if netCDF4 is not None:
    nc("cdf1", "NETCDF3_CLASSIC")
    nc("cdf2", "NETCDF3_64BIT_OFFSET")
    nc("cdf5", "NETCDF3_64BIT_DATA")
    nc("nc4", "NETCDF4")
    nc("nc4classic", "NETCDF4_CLASSIC")

print("ok")
