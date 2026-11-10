"""Array files and their expected values, made and read by the upstream libraries (oracles).

    python -I make_samples.py OUT_DIR [--small]

Writes NumPy .npy / .npz (numpy), MATLAB level 4 and 5 .mat (scipy.io.savemat, compressed and
not) and Zarr v2 / v3 stores (zarr-python: folders and zips, every codec we read) into OUT_DIR,
and for each one `expected/<name>.json`: the arrays a reader should list, each with its shape
and its values as little-endian float32 in C order of the logical shape (complex: magnitude;
`<name>.<k>.re.f32` holds the real part too), the arrays it should refuse, and char texts.

--small makes the committed fixture set (a few KB each); without it, a larger corpus with more
shapes, sizes and random seeds.
"""

import json
import os
import shutil
import sys
import warnings

import numpy as np

warnings.filterwarnings("ignore")

OUT = sys.argv[1]
SMALL = "--small" in sys.argv
EXP = os.path.join(OUT, "expected")
os.makedirs(EXP, exist_ok=True)
rng = np.random.default_rng(1234)


def values_of(a):
    a = np.asarray(a)
    if a.dtype.kind == "c":
        return np.abs(a).astype("<f4"), a.real.astype("<f4")
    if a.dtype.kind in "Mm":
        a = a.astype(np.int64)
    return a.astype("<f4"), None


def record(name, kind, arrays, refused=(), texts=None):
    """arrays: list of (path, ndarray in logical shape)."""
    out = {"kind": kind, "arrays": [], "refused": sorted(refused), "texts": texts or {}}
    # Every array's values in one file: magnitude (or the value), then the real part of
    # complex arrays; offsets in values.
    chunks, at = [], 0
    for path, a in arrays:
        a = np.asarray(a)
        mag, re = values_of(a)
        e = {"name": path, "shape": list(a.shape), "offset": at, "complex": re is not None}
        chunks.append(np.ascontiguousarray(mag).reshape(-1))
        at += mag.size
        if re is not None:
            e["re_offset"] = at
            chunks.append(np.ascontiguousarray(re).reshape(-1))
            at += re.size
        out["arrays"].append(e)
    if chunks:
        np.concatenate(chunks).astype("<f4").tofile(os.path.join(EXP, name + ".f32"))
    with open(os.path.join(EXP, name + ".json"), "w") as f:
        json.dump(out, f, indent=1, sort_keys=True)


def sample(dtype, shape):
    dt = np.dtype(dtype)
    if dt.kind == "b":
        return rng.integers(0, 2, shape).astype(dt)
    if dt.kind in "iu":
        info = np.iinfo(dt)
        lo, hi = max(info.min, -30000), min(info.max, 30000)
        return rng.integers(lo, hi, shape, endpoint=True).astype(dt)
    if dt.kind == "f":
        return (rng.standard_normal(shape) * 100).astype(dt)
    if dt.kind == "c":
        return (rng.standard_normal(shape) + 1j * rng.standard_normal(shape)).astype(dt)
    if dt.kind in "Mm":
        return rng.integers(0, 10**6, shape).astype(dt)
    raise ValueError(dtype)


# ----- NumPy ------------------------------------------------------------------------------

def npy_files():
    dtypes = ["|b1", "|i1", "|u1", "<i2", ">i2", "<u2", ">u2", "<i4", ">i4", "<u4", ">u4",
              "<i8", ">i8", "<u8", ">u8", "<f2", ">f2", "<f4", ">f4", "<f8", ">f8", "<c8",
              ">c8", "<c16", ">c16", "<M8[ns]", ">m8[s]"]
    shapes = [(7,), (5, 6), (3, 4, 5)] if SMALL else [(1,), (13,), (5, 6), (1, 9), (33, 17),
                                                       (3, 4, 5), (2, 3, 4, 5), (64, 48, 3)]
    n = 0
    for dt in dtypes:
        for shape in shapes:
            if SMALL and dt not in ("|u1", ">i2", "<f4", ">f8", "<c8", "<M8[ns]", "|b1", "<f2"):
                continue
            for fortran in (False, True):
                if SMALL and fortran and len(shape) < 2:
                    continue
                a = sample(dt, shape)
                if fortran:
                    a = np.asfortranarray(a)
                for ver in ((1, 0), (2, 0), (3, 0)):
                    if SMALL and ver != (1, 0) and not (dt == "<f4" and len(shape) == 2):
                        continue
                    if not SMALL and ver != (1, 0) and n % 3:
                        n += 1
                        continue
                    name = f"a{n:04d}_{dt.replace('<', 'le').replace('>', 'be').replace('|', '')}" \
                           f"{'_F' if fortran else ''}_v{ver[0]}.npy"
                    name = name.replace("[", "_").replace("]", "")
                    with open(os.path.join(OUT, name), "wb") as f:
                        np.lib.format.write_array(f, a, version=ver)
                    record(name, "npy", [("", a)])
                    n += 1
    # Scalars, empties and refusals.
    for name, a in [("scalar.npy", np.float64(3.5)), ("empty.npy", np.zeros((0, 4), "<i2"))]:
        np.save(os.path.join(OUT, name), a)
        record(name, "npy", [("", np.asarray(a))])
    for name, a in [("object.npy", np.array([{"a": 1}, None], dtype=object)),
                    ("text.npy", np.array(["ab", "cd"])),
                    ("record.npy", np.zeros(3, dtype=[("x", "<i4"), ("y", "<f8")])),
                    ("longdouble.npy", np.zeros(3, dtype=np.longdouble))]:
        np.save(os.path.join(OUT, name), a, allow_pickle=True)
        record(name, "npy", [], refused=[""])


def npz_files():
    sets = 2 if SMALL else 12
    for i in range(sets):
        arrays = {"image": sample("<u2", (24, 32)), "stack": sample("<f4", (3, 8, 10)),
                  "line": sample("<f8", (50,)), "cplx": sample("<c16", (6, 7))}
        if i % 2:
            arrays["obj"] = np.array([1, "x"], dtype=object)
        for compressed in (False, True):
            name = f"set{i:02d}{'_z' if compressed else ''}.npz"
            (np.savez_compressed if compressed else np.savez)(os.path.join(OUT, name), **arrays)
            ok = [(k, v) for k, v in arrays.items() if v.dtype != object]
            record(name, "npz", ok, refused=[k for k, v in arrays.items() if v.dtype == object])


# ----- MATLAB -----------------------------------------------------------------------------

def mat_expect(d, prefix=""):
    """Walks loadmat's output the way the reader names variables and elements."""
    arrays, refused, texts = [], [], {}

    def sub(shape, idx):
        if sum(1 for s in shape if s != 1) <= 1:
            return str(int(np.ravel_multi_index(idx, shape, order="F")) + 1)
        return ",".join(str(i + 1) for i in idx)

    def walk(path, v):
        import scipy.io.matlab as sm
        import scipy.sparse
        if scipy.sparse.issparse(v):
            arrays.append((path, v.toarray()))
            return
        if isinstance(v, (sm.MatlabFunction, sm.MatlabOpaque)):
            refused.append(path)
            return
        if v is None:  # a field of an empty struct
            return
        v = np.asarray(v)
        if v.dtype.names:  # struct array
            if v.size == 1:
                for f in v.dtype.names:
                    walk(f"{path}.{f}", v.reshape(-1)[0][f])
            else:
                for idx in np.ndindex(*v.shape[::-1]):
                    idx = idx[::-1]
                    for f in v.dtype.names:
                        walk(f"{path}({sub(v.shape, idx)}).{f}", v[idx][f])
            return
        if v.dtype == object:  # cell
            for idx in np.ndindex(*v.shape[::-1]):
                idx = idx[::-1]
                walk(f"{path}{{{sub(v.shape, idx)}}}", v[idx])
            return
        if v.dtype.kind in "US":
            if v.size:
                texts[path] = "\n".join(str(s).rstrip() for s in v.reshape(-1))
            return
        if v.size == 0:
            return
        arrays.append((path, v))

    for k in sorted(d):
        if k.startswith("__"):
            continue
        walk(prefix + k, d[k])
    return arrays, refused, texts


def mat_files():
    import scipy.io
    import scipy.sparse
    count = 2 if SMALL else 16
    for i in range(count):
        cls = ["<f8", "<f4", "|i1", "|u1", "<i2", "<u2", "<i4", "<u4", "<i8", "<u8"]
        dvars = {}
        for j, dt in enumerate(cls if not SMALL else cls[:4]):
            shape = [(4, 5), (6,), (3, 4, 2), (2, 3, 2, 2)][(i + j) % 4]
            dvars[f"v_{dt[1:]}"] = sample(dt, shape)
        dvars["flag"] = sample("|b1", (3, 5))
        dvars["z"] = sample("<c16", (4, 3))
        dvars["zs"] = sample("<c8", (2, 5))
        dvars["name"] = "hello"
        dvars["s"] = {"a": sample("<f8", (2, 3)), "inner": {"b": sample("<i4", (3,)), "t": "deep"}}
        dvars["c"] = np.array([sample("<f8", (2, 2)), "txt", sample("|u1", (3, 1))], dtype=object)
        sa = np.zeros((1, 2), dtype=[("p", object), ("q", object)])
        sa[0, 0] = (sample("<f8", (2, 2)), sample("<i2", (3,)))
        sa[0, 1] = (sample("<f8", (1, 4)), sample("<i2", (2, 2)))
        dvars["sa"] = sa
        dvars["sp"] = scipy.sparse.random(6, 5, density=0.3, format="csc", random_state=i) * 10
        dvars["img"] = sample("|u1", (16, 20, 3))
        for compress in (False, True):
            name = f"m{i:02d}{'_z' if compress else ''}.mat"
            scipy.io.savemat(os.path.join(OUT, name), dvars, do_compression=compress,
                             long_field_names=True)
            d = scipy.io.loadmat(os.path.join(OUT, name), squeeze_me=False,
                                 struct_as_record=True, chars_as_strings=True)
            arrays, refused, texts = mat_expect(d)
            record(name, "mat", arrays, refused, texts)
        # Level 4: numeric matrices, text and sparse.
        v4 = {"A": sample("<f8", (5, 4)), "B": sample("<f8", (1, 7)),
              "Z": sample("<c16", (3, 3)), "S": scipy.sparse.random(4, 6, density=0.4,
                                                                     format="csc",
                                                                     random_state=i) * 3}
        name = f"m{i:02d}_v4.mat"
        scipy.io.savemat(os.path.join(OUT, name), v4, format="4")
        d = scipy.io.loadmat(os.path.join(OUT, name), squeeze_me=False)
        arrays, refused, texts = mat_expect(d)
        record(name, "mat", arrays, refused, texts)


# ----- Zarr -------------------------------------------------------------------------------

def zarr_files():
    import numcodecs
    import zarr
    import zarr.codecs as zc
    from zarr.storage import ZipStore

    def put(name, fn, zip_too=False):
        path = os.path.join(OUT, name)
        shutil.rmtree(path, ignore_errors=True)
        # The committed set keeps these many-chunk stores as zips only.
        if not SMALL:
            root = zarr.open_group(path, mode="w", zarr_format=fn.zarr_format)
            arrays = fn(root)
            record(name, "zarr", arrays)
        if zip_too:
            zp = path + ".zip"
            if os.path.exists(zp):
                os.remove(zp)
            store = ZipStore(zp, mode="w")
            root = zarr.open_group(store, mode="w", zarr_format=fn.zarr_format)
            arrays = fn(root)
            store.close()
            record(name + ".zip", "zarr", arrays)

    cases = []

    def case(fmt):
        def deco(f):
            f.zarr_format = fmt
            cases.append(f)
            return f
        return deco

    shapes = [(10, 12), (3, 9, 11)] if SMALL else [(10, 12), (3, 9, 11), (2, 2, 17, 13), (40,),
                                                   (130, 70)]

    @case(2)
    def v2_compressors(root):
        out = []
        comps = [None, numcodecs.Blosc("lz4", 5, numcodecs.Blosc.SHUFFLE),
                 numcodecs.Blosc("zstd", 3, numcodecs.Blosc.BITSHUFFLE),
                 numcodecs.Blosc("blosclz", 5, numcodecs.Blosc.NOSHUFFLE),
                 numcodecs.Blosc("zlib", 5, numcodecs.Blosc.SHUFFLE), numcodecs.Zstd(3),
                 numcodecs.GZip(5), numcodecs.Zlib(5), numcodecs.LZ4()]
        for k, c in enumerate(comps):
            for s, shape in enumerate(shapes):
                dt = ["<u2", "<f4", ">i4", "<f8", "|u1"][(k + s) % 5]
                a = sample(dt, shape)
                chunks = tuple(max(1, d // 3 + 1) for d in shape)
                name = f"c{k}_{s}"
                z = root.create_array(name, shape=shape, dtype=a.dtype, chunks=chunks,
                                      compressors=c, fill_value=0,
                                      order="F" if (k + s) % 2 else "C")
                z[...] = a
                out.append((name, a))
        # Filters, a nested group, missing chunks with a fill value, '/' separators.
        g = root.create_group("nested").create_group("deeper")
        a = sample("<i4", (12, 9))
        z = g.create_array("delta", shape=a.shape, dtype=a.dtype, chunks=(5, 4),
                           filters=[numcodecs.Delta(dtype="<i4")], compressors=numcodecs.Zlib(),
                           fill_value=0)
        z[...] = a
        out.append(("nested/deeper/delta", a))
        a = sample("<f4", (12, 9))
        z = g.create_array("shuffle", shape=a.shape, dtype=a.dtype, chunks=(5, 4),
                           filters=[numcodecs.Shuffle(elementsize=4)], compressors=None,
                           fill_value=0)
        z[...] = a
        out.append(("nested/deeper/shuffle", a))
        z = root.create_array("sparse", shape=(9, 10), dtype="<f4", chunks=(4, 4),
                              fill_value=-7.5, chunk_key_encoding={"name": "v2",
                                                                   "separator": "/"})
        part = sample("<f4", (4, 4))
        z[:4, :4] = part
        a = np.full((9, 10), -7.5, "<f4")
        a[:4, :4] = part
        out.append(("sparse", a))
        for dt in ["|b1", "<f2", "<c8", ">c16", "<i8", "<u8", "|i1"]:
            a = sample(dt, (6, 7))
            name = "t_" + dt[1:]
            z = root.create_array(name, shape=a.shape, dtype=a.dtype, chunks=(4, 4),
                                  fill_value=0)
            z[...] = a
            out.append((name, a))
        return out

    @case(3)
    def v3_codecs(root):
        out = []
        sets = [
            [zc.BytesCodec()],
            [zc.BytesCodec(endian="big"), zc.ZstdCodec(level=3)],
            [zc.BytesCodec(), zc.GzipCodec(level=5)],
            [zc.BytesCodec(), zc.BloscCodec(cname="lz4", clevel=5, shuffle="shuffle")],
            [zc.BytesCodec(), zc.BloscCodec(cname="zstd", clevel=3, shuffle="bitshuffle")],
            [zc.TransposeCodec(order=(1, 0)), zc.BytesCodec(), zc.Crc32cCodec()],
        ]
        for k, codecs in enumerate(sets):
            a = sample(["<f4", "<u2", "<i8", "<f8", "|u1", "<i2"][k], (14, 11))
            name = f"k{k}"
            transpose = [c for c in codecs if isinstance(c, zc.TransposeCodec)]
            ser = [c for c in codecs if isinstance(c, zc.BytesCodec)][0]
            comp = [c for c in codecs if not isinstance(c, (zc.TransposeCodec, zc.BytesCodec))]
            z = root.create_array(name, shape=a.shape, dtype=a.dtype, chunks=(5, 4),
                                  filters=transpose, serializer=ser, compressors=comp,
                                  fill_value=0, dimension_names=["row", "col"])
            z[...] = a
            out.append((name, a))
        for s, shape in enumerate(shapes):
            a = sample("<f4", shape)
            name = f"shard{s}"
            chunks = tuple(max(1, d // 4) for d in shape)
            shards = tuple(c * 2 for c in chunks)
            z = root.create_array(name, shape=shape, dtype=a.dtype, chunks=chunks,
                                  shards=shards, compressors=zc.ZstdCodec(), fill_value=0)
            z[...] = a
            out.append((name, a))
        z = root.create_group("g").create_array("missing", shape=(8, 8), dtype="<i2",
                                                chunks=(3, 3), fill_value=-3)
        z[0:3, 0:3] = 5
        a = np.full((8, 8), -3, "<i2")
        a[0:3, 0:3] = 5
        out.append(("g/missing", a))
        z = root.create_array("nanfill", shape=(5, 5), dtype="<f8", chunks=(2, 2),
                              fill_value=float("nan"))
        z[0:2, 0:2] = 1.5
        a = np.full((5, 5), np.nan)
        a[0:2, 0:2] = 1.5
        out.append(("nanfill", a))
        for dt in ["bool", "float16", "complex64", "complex128", "uint64", "int8"]:
            a = sample(np.dtype(dt).str, (6, 7))
            z = root.create_array("t_" + dt, shape=a.shape, dtype=a.dtype, chunks=(4, 4),
                                  fill_value=0)
            z[...] = a
            out.append(("t_" + dt, a))
        return out

    for f in cases:
        put(f"{f.__name__}.zarr", f, zip_too=True)
    # Consolidated metadata, both versions.
    for fmt in (2, 3):
        path = os.path.join(OUT, f"consolidated_v{fmt}.zarr")
        shutil.rmtree(path, ignore_errors=True)
        root = zarr.open_group(path, mode="w", zarr_format=fmt)
        a = sample("<f4", (6, 8))
        root.create_group("grp").create_array("x", shape=a.shape, dtype=a.dtype, chunks=(3, 3))[...] = a
        b = sample("<u1", (4, 5, 3))
        root.create_array("rgb", shape=b.shape, dtype=b.dtype, chunks=(4, 5, 3))[...] = b
        zarr.consolidate_metadata(path)
        record(os.path.basename(path), "zarr", [("grp/x", a), ("rgb", b)])
    # A root array, v2 and v3.
    for fmt in (2, 3):
        path = os.path.join(OUT, f"root_v{fmt}.zarr")
        shutil.rmtree(path, ignore_errors=True)
        a = sample("<i2", (7, 9))
        z = zarr.create_array(path, shape=a.shape, dtype=a.dtype, chunks=(4, 4),
                              zarr_format=fmt)
        z[...] = a
        record(os.path.basename(path), "zarr", [("", a)])


def scipy_mat_files():
    """The MATLAB-written test files shipped with scipy (levels 4 and 5, both byte orders,
    MATLAB 4.2 to 8), with what scipy.io.loadmat reads from them; files scipy cannot read
    are recorded as broken (the reader must refuse them without panicking)."""
    import scipy.io
    src = os.path.join(os.path.dirname(scipy.io.__file__), "matlab", "tests", "data")
    for fn in sorted(os.listdir(src)):
        if not fn.endswith(".mat"):
            continue
        name = "scipy_" + fn
        shutil.copy(os.path.join(src, fn), os.path.join(OUT, name))
        try:
            if "duplicate_fieldnames" in fn:
                raise ValueError("scipy renames duplicate field names; not comparable")
            d = scipy.io.loadmat(os.path.join(OUT, name), squeeze_me=False,
                                 struct_as_record=True, chars_as_strings=True)
        except Exception as e:  # noqa: BLE001 (any failure marks the file broken)
            with open(os.path.join(EXP, name + ".json"), "w") as f:
                json.dump({"kind": "mat", "broken": str(e)[:200], "arrays": [], "refused": [],
                           "texts": {}}, f, indent=1)
            continue
        arrays, refused, texts = mat_expect(d)
        record(name, "mat", arrays, refused, texts)


if "--scipy-data" in sys.argv:
    scipy_mat_files()
else:
    npy_files()
    npz_files()
    mat_files()
    zarr_files()
print("written to", OUT)
