#!/usr/bin/env python3
"""Writes the small industrial CT samples in ../tests/data and their oracles:

* ../tests/data/expected-indct.tsv: the same rows as expected.tsv (file, geom, val, error),
  read back with independent readers: txrm2tiff (over olefile) for Zeiss Xradia files, a
  small Amira reader below, PIL and tifffile for SkyScan slices, numpy for raw volumes.
* ../tests/data/expected-indct-extra.tsv: what tests/indct.rs checks beyond those rows:
  `layout` + `val` rows for a headerless volume read with a given layout, `param` rows (facts
  olefile reads from the Xradia streams, or that the INI headers state), `guess` rows
  (the layout a raw file's name and size spell), and `cfb` rows (every stream olefile
  lists in each compound file, its size and FNV-1a 64 hash) for specular-cfb's tests.

Xradia files are written by the compound-file writer below (version 3 with 512-byte
sectors, or version 4 with 4096-byte ones; small streams in the mini stream), laid out as
[MS-CFB] describes. Nothing here is used at run time. Run it with -I from a venv that has
numpy, olefile, txrm2tiff, tifffile and pillow:

    python3 -I crates/formats/specular-medvol/gen/make_indct.py
"""

import os
import struct
import zlib

import numpy as np
import olefile
import tifffile
from PIL import Image
from txrm2tiff.txrm import open_txrm

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "..", "tests", "data")

rows = []
extra = []


def fmt(x):
    return repr(float(x))


def record(name, kind, dtype, orient, vols):
    rows.append(f"file\t{name}\t{kind}\t{dtype}\t{orient}\t{vols}")


def geom(name, affine, shape):
    nx, ny, nz = shape
    pts = {(i, j, k) for i in (0, nx - 1) for j in (0, ny - 1) for k in (0, nz - 1)}
    pts.add((min(1, nx - 1), min(1, ny - 1), min(1, nz - 1)))
    for p in sorted(pts):
        x = np.asarray(affine) @ np.array([p[0], p[1], p[2], 1.0])
        rows.append(f"geom\t{name}\t{p[0]}\t{p[1]}\t{p[2]}\t{fmt(x[0])}\t{fmt(x[1])}\t{fmt(x[2])}")


def vals(name, d, out=None):
    """d indexed [i, j, k, (volume or component)], i the fastest stored axis."""
    out = rows if out is None else out
    d = np.asarray(d, dtype=np.float64)
    comp = False
    if d.ndim == 5:
        comp = True
        d = d[..., 0, :]
    if d.ndim == 3:
        d = d[..., np.newaxis]
    nx, ny, nz, nv = d.shape
    every = [(i, j, k, v) for v in range(nv) for k in range(nz) for j in range(ny)
             for i in range(nx)]
    if len(every) > 400:
        # Large samples: the corners and 400 voxels spread at random, to keep this small.
        pick = np.random.default_rng(len(every)).choice(len(every), 400, replace=False)
        corners = {(i, j, k, v) for i in (0, nx - 1) for j in (0, ny - 1)
                   for k in (0, nz - 1) for v in (0, nv - 1)}
        every = sorted(corners | {every[x] for x in pick}, key=lambda p: p[::-1])
    for i, j, k, v in every:
        vol, c = (0, v) if comp else (v, 0)
        out.append(f"val\t{name}\t{i}\t{j}\t{k}\t{vol}\t{c}\t{fmt(d[i, j, k, v])}")


def values_for(dtype, shape, seed):
    r = np.random.default_rng(seed)
    dt = np.dtype(dtype)
    if dt.kind == "f":
        return (r.standard_normal(shape) * 1000).astype(dt)
    info = np.iinfo(dt)
    a = r.integers(info.min, info.max, size=shape, endpoint=True, dtype=np.int64).astype(dt)
    flat = a.reshape(-1)
    flat[0] = info.min
    flat[-1] = info.max
    return a


def fnv(b):
    h = 0xCBF29CE484222325
    for x in b:
        h = ((h ^ x) * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
    return h


def write(name, data):
    with open(os.path.join(OUT, name), "wb") as f:
        f.write(data)


# ---------------------------------------------------------------------------------------
# Compound File Binary writer ([MS-CFB]).

END = 0xFFFFFFFE
FREE = 0xFFFFFFFF
FATSECT = 0xFFFFFFFD
NOSTREAM = 0xFFFFFFFF


def cfb_bytes(streams, version=3):
    """streams: {"Storage/Stream": bytes}, in any order."""
    ss = 512 if version == 3 else 4096
    root = {"name": "Root Entry", "type": 5, "children": {}, "data": None}
    nodes = [root]
    for path, data in streams.items():
        cur = root
        parts = path.split("/")
        for i, part in enumerate(parts):
            last = i == len(parts) - 1
            if part not in cur["children"]:
                n = {"name": part, "type": 2 if last else 1, "children": {},
                     "data": data if last else None}
                nodes.append(n)
                cur["children"][part] = n
            cur = cur["children"][part]
    for i, n in enumerate(nodes):
        n["id"] = i
        n["left"] = n["right"] = NOSTREAM

    def key(n):
        return (len(n["name"]), n["name"].upper())

    def tree(lst):
        if not lst:
            return NOSTREAM
        m = len(lst) // 2
        lst[m]["left"] = tree(lst[:m])
        lst[m]["right"] = tree(lst[m + 1:])
        return lst[m]["id"]

    for n in nodes:
        n["child"] = tree(sorted(n["children"].values(), key=key))

    # The mini stream: streams under the cutoff in 64-byte mini sectors.
    mini = bytearray()
    minifat = []
    big = []
    for n in nodes:
        if n["type"] != 2:
            continue
        d = n["data"]
        if len(d) < 4096:
            count = -(-len(d) // 64)
            n["start"] = len(minifat) if count else END
            for s in range(count):
                minifat.append(len(minifat) + 1 if s < count - 1 else END)
            mini += d + bytes(count * 64 - len(d))
        else:
            big.append(n)

    def secs(nbytes):
        return -(-nbytes // ss)

    ndir = secs(len(nodes) * 128)
    nminifat = secs(len(minifat) * 4)
    nmini = secs(len(mini))
    nbig = [secs(len(n["data"])) for n in big]
    other = ndir + nminifat + nmini + sum(nbig)
    nfat = 1
    while nfat * (ss // 4) < nfat + other:
        nfat += 1
    assert nfat <= 109
    fat = []
    sectors = []

    def chain(count, payload):
        start = len(fat)
        for s in range(count):
            fat.append(start + s + 1 if s < count - 1 else END)
        payload = bytes(payload) + bytes(count * ss - len(payload))
        sectors.extend(payload[s * ss:(s + 1) * ss] for s in range(count))
        return start if count else END

    fat.extend([FATSECT] * nfat)
    sectors.extend([None] * nfat)
    dir_start = len(fat)
    dir_placeholder = len(sectors)
    fat.extend(dir_start + s + 1 if s < ndir - 1 else END for s in range(ndir))
    sectors.extend([None] * ndir)
    minifat_bytes = b"".join(struct.pack("<I", x) for x in minifat)
    minifat_start = chain(nminifat, minifat_bytes)
    root["start"] = chain(nmini, mini) if mini else END
    root["data"] = mini
    for n, count in zip(big, nbig):
        n["start"] = chain(count, n["data"])
    fat_bytes = b"".join(struct.pack("<I", x) for x in fat)
    fat_bytes += struct.pack("<I", FREE) * (nfat * ss // 4 - len(fat))
    for s in range(nfat):
        sectors[s] = fat_bytes[s * ss:(s + 1) * ss]

    d = bytearray()
    for n in nodes:
        name = n["name"].encode("utf-16-le") + b"\0\0"
        e = bytearray(128)
        e[0:len(name)] = name
        size = len(n["data"]) if n["data"] is not None else 0
        struct.pack_into("<HBBIII", e, 64, len(name), n["type"], 1, n["left"], n["right"],
                         n["child"])
        struct.pack_into("<IQ", e, 116, n.get("start", END) if size or n["type"] == 5 else 0,
                         size)
        if n["type"] == 1:
            struct.pack_into("<IQ", e, 116, 0, 0)
        d += e
    while len(d) < ndir * ss:
        e = bytearray(128)
        struct.pack_into("<III", e, 68, NOSTREAM, NOSTREAM, NOSTREAM)
        d += e
    for s in range(ndir):
        sectors[dir_placeholder + s] = bytes(d[s * ss:(s + 1) * ss])

    h = bytearray(512)
    h[0:8] = bytes.fromhex("D0CF11E0A1B11AE1")
    struct.pack_into("<HHHHH", h, 24, 0x3E, version, 0xFFFE, 9 if version == 3 else 12, 6)
    struct.pack_into("<IIIIIIIII", h, 40, ndir if version == 4 else 0, nfat, dir_start, 0,
                     4096, minifat_start if nminifat else END, nminifat, END, 0)
    difat = list(range(nfat)) + [FREE] * (109 - nfat)
    struct.pack_into("<109I", h, 76, *difat)
    head = bytes(h) + bytes(ss - 512)
    return head + b"".join(sectors)


# ---------------------------------------------------------------------------------------
# Zeiss Xradia.

XRM_TYPES = {np.dtype("i1"): 2, np.dtype("u1"): 3, np.dtype("<i2"): 4, np.dtype("<u2"): 5,
             np.dtype("<i4"): 6, np.dtype("<u4"): 7, np.dtype("<f4"): 10, np.dtype("<f8"): 11}


def f32(*v):
    return struct.pack(f"<{len(v)}f", *v)


def i32(v):
    return struct.pack("<i", v)


def xradia(name, images, *, dtype_stream=True, angles=None, pixel_um=1.0, mosaic=None,
           facts=None, version=3):
    """images: [n, rows, cols] as stored (row 0 the bottom of the vendor's view)."""
    n, h, w = images.shape
    s = {
        "Version": f32(5.0),
        "ImageInfo/ImageWidth": i32(w),
        "ImageInfo/ImageHeight": i32(h),
        "ImageInfo/ImagesTaken": struct.pack("<I", n),
        "ImageInfo/NoOfImages": i32(n),
        "ImageInfo/PixelSize": f32(pixel_um),
        "ImageInfo/Angles": f32(*(angles if angles is not None else [0.0] * n)),
    }
    if dtype_stream:
        s["ImageInfo/DataType"] = i32(XRM_TYPES[images.dtype])
    # 0 × 0 for a single field, as the instrument writes it.
    s["ImageInfo/MosiacColumns"] = i32(mosaic[0] if mosaic else 0)
    s["ImageInfo/MosiacRows"] = i32(mosaic[1] if mosaic else 0)
    for k, v in (facts or {}).items():
        s[k] = v
    for m in range(1, n + 1):
        s[f"ImageData{-(-m // 100)}/Image{m}"] = images[m - 1].astype(images.dtype).tobytes()
    data = cfb_bytes(s, version)
    write(name, data)
    return data


def xradia_case(name, images, oracle_dtype, **kw):
    xradia(name, images, **kw)
    path = os.path.join(OUT, name)
    # As bytes: txrm2tiff checks a path only for the .txrm and .xrm suffixes.
    t = open_txrm(open(path, "rb").read())
    # txrm2tiff's default output: as XRM Data Explorer shows it, [image, row, column].
    out = np.asarray(t.get_output(load=True, flip=False))
    t.close()
    n_, h, w = out.shape
    px = kw.get("pixel_um", 1.0) / 1000.0
    record(name, "Zeiss Xradia TXM", oracle_dtype, "object", 1)
    affine = [[px, 0, 0, 0], [0, -px, 0, (h - 1) * px], [0, 0, px, 0]]
    geom(name, affine, (w, h, n_))
    # Stored row j is the vendor's row h-1-j.
    stored = out[:, ::-1, :]
    vals(name, np.transpose(stored, (2, 1, 0)))
    # Every stream olefile lists, for specular-cfb's reader.
    ole = olefile.OleFileIO(path)
    for entry in ole.listdir(streams=True, storages=False):
        p = "/".join(entry)
        b = ole.openstream(entry).read()
        extra.append(f"cfb\t{name}\t{p}\t{len(b)}\t{fnv(b):016x}")
    ole.close()
    return path


def param_rows(name, pairs):
    for k, v in pairs:
        extra.append(f"param\t{name}\t{k}\t{v}")


def xradia_cases():
    r = np.random.default_rng(7)
    # A reconstruction: uint16 slices, angles all the same.
    a = values_for("<u2", (6, 10, 12), 1)
    p = xradia_case("txm_u16.txm", a, "uint16", pixel_um=2.5, facts={
        "ImageInfo/Voltage": f32(60.0),
        "ImageInfo/Current": f32(83.0),
        "ImageInfo/ObjectiveName": b"4X\0",
        "ImageInfo/Date": b"03/14/2024 10:11:12\0",
    })
    ole = olefile.OleFileIO(p)
    v = np.frombuffer(ole.openstream("ImageInfo/Voltage").read(), "<f4")[0]
    c = np.frombuffer(ole.openstream("ImageInfo/Current").read(), "<f4")[0]
    obj = ole.openstream("ImageInfo/ObjectiveName").read().split(b"\0")[0].decode()
    ole.close()
    param_rows("txm_u16.txm", [("content", "reconstruction"), ("kv", fmt(v)),
                               ("current", fmt(c)), ("objective", obj)])
    # Projections: float32 images (4096 bytes: outside the mini stream), turning angles.
    a = (r.standard_normal((3, 32, 32)) * 100).astype("<f4")
    angles = [-60.0, 0.0, 60.0]
    p = xradia_case("txrm_f32.txrm", a, "float32", angles=angles, pixel_um=12.0, facts={
        "ImageInfo/ExpTimes": f32(1.5, 1.5, 1.5),
        "ImageInfo/StoRADistance": f32(-25.0),
        "ImageInfo/DtoRADistance": f32(150.0),
        "ImageInfo/OpticalMagnification": f32(3.92),
        "ImageInfo/CameraBinning": i32(2),
    })
    t = open_txrm(open(p, "rb").read())
    exps = t.exposures
    ang = t.image_info["Angles"]
    t.close()
    param_rows("txrm_f32.txrm", [
        ("content", "projections"), ("projections", "3"),
        ("angles", f"{min(ang):g}° – {max(ang):g}°"), ("exposure", fmt(exps[0] * 1000.0)),
        ("sod", "25.0"), ("odd", "150.0"), ("binning", "2"),
    ])
    # No DataType stream: told from the image length (uint16), as txrm2tiff falls back.
    a = values_for("<u2", (2, 6, 8), 3)
    xradia_case("xrm_nodtype.xrm", a, "uint16", dtype_stream=False)
    # More than 100 images (ImageData2), in a version 4 file (4096-byte sectors).
    a = values_for("u1", (105, 3, 4), 4)
    xradia_case("txm_many.txm", a, "uint8", version=4, pixel_um=0.5)
    # A mosaic of 2 bands of 3 tiles.
    a = values_for("<u2", (6, 4, 5), 5)
    xradia_case("mosaic.xrm", a, "uint16", mosaic=(2, 3))


# ---------------------------------------------------------------------------------------
# Amira: written here, read back by the small reader below (from the text alone).

AM_TYPES = {"byte": "u1", "sbyte": "i1", "short": "i2", "ushort": "u2", "int": "i4",
            "uint": "u4", "float": "f4", "double": "f8"}


def byte_rle_encode(b):
    out = bytearray()
    i = 0
    while i < len(b):
        j = i
        while j < len(b) and j - i < 127 and b[j] == b[i]:
            j += 1
        if j - i >= 3:
            out += bytes([j - i, b[i]])
            i = j
            continue
        j = i
        while j < len(b) and j - i < 127 and not (j + 2 < len(b) and b[j] == b[j + 1] == b[j + 2]):
            j += 1
        out += bytes([0x80 | (j - i)]) + b[i:j]
        i = j
    return bytes(out)


def amira_write(name, data, typ, *, coding="BINARY-LITTLE-ENDIAN", bbox, unit=None,
                comp=None, compress=None, lead=None):
    """data indexed [z, y, x(, c)]."""
    nz, ny, nx = data.shape[:3]
    t = f"{typ}[{comp}]" if comp else typ
    big = coding == "BINARY"
    if coding == "ASCII":
        body = ("\n".join(" ".join(repr(x.item()) if data.dtype.kind == "f" else str(x)
                                   for x in row) for row in data.reshape(-1, 6))).encode()
    else:
        dt = np.dtype(AM_TYPES[typ]).newbyteorder(">" if big else "<")
        body = data.astype(dt).tobytes()
    label = ""
    if compress == "rle":
        body = byte_rle_encode(body)
        label = f"(HxByteRLE,{len(body)})"
    elif compress == "zip":
        body = zlib.compress(body)
        label = f"(HxZip,{len(body)})"
    units = f'    Units {{\n        Coordinates "{unit}"\n    }}\n' if unit else ""
    head = (f"# AmiraMesh {coding} 2.1\n\n\ndefine Lattice {nx} {ny} {nz}\n\n"
            f"Parameters {{\n{units}"
            f'    Content "{nx}x{ny}x{nz} {t}, uniform coordinates",\n'
            f"    BoundingBox {' '.join(repr(float(x)) for x in bbox)},\n"
            f'    CoordType "uniform"\n}}\n\n')
    sections = ""
    pre = b""
    n = 1
    if lead is not None:
        # A section before the image's, of a type that is not read (skipped over).
        sections += f"Lattice {{ {lead[0]} Labels }} @{n}\n"
        pre = f"@{n}\n".encode() + lead[1] + b"\n\n"
        n += 1
    sections += f"Lattice {{ {t} Data }} @{n}{label}\n\n"
    out = (head + sections + "# Data section follows\n").encode() + pre
    out += f"@{n}\n".encode() + body + b"\n"
    write(name, out)


def amira_read(path):
    """A minimal AmiraMesh reader: the image lattice as [x, y, z(, c)], spacing, origin."""
    raw = open(path, "rb").read()
    first = raw.split(b"\n", 1)[0].decode()
    ascii_ = "ASCII" in first
    big = "BINARY" in first and "LITTLE" not in first
    text = raw.decode("utf-8", "replace")
    lines = text.split("\n")
    size = bbox = None
    unit = 1.0
    sects = []
    for ln in lines:
        s = ln.strip()
        if s.startswith("@"):
            break
        if s.startswith("define Lattice"):
            size = [int(x) for x in s.split()[2:]]
        elif s.startswith("BoundingBox"):
            bbox = [float(x) for x in s.rstrip(",").split()[1:]]
        elif s.startswith("Coordinates"):
            unit = {"mm": 1.0, "µm": 1e-3, "um": 1e-3, "nm": 1e-6, "m": 1000.0}[s.split('"')[1]]
        elif s.startswith("Lattice {"):
            decl = s[s.index("{") + 1:s.index("}")].split()[0]
            at = s[s.index("}") + 1:].strip()
            sid = int(at[1:].split("(")[0])
            enc = None
            if "(" in at:
                e, ln_ = at[at.index("(") + 1:-1].split(",")
                enc = (e, int(ln_))
            typ, comp = (decl.split("[")[0], int(decl.split("[")[1][:-1])) if "[" in decl \
                else (decl, 1)
            sects.append((sid, typ, comp, enc))
    nx, ny, nz = size
    sid, typ, comp, enc = next(x for x in sects if x[1] in AM_TYPES)
    count = nx * ny * nz * comp
    # Walk the markers to the chosen section.
    pos = 0
    while True:
        m = raw.index(b"\n@", pos) + 1
        nl = raw.index(b"\n", m)
        if int(raw[m + 1:nl]) == sid:
            start = nl + 1
            break
        pos = nl
    dt = np.dtype(AM_TYPES[typ]).newbyteorder(">" if big else "<")
    if ascii_:
        toks = raw[start:].split()[:count]
        a = np.array([float(x) for x in toks]).astype(dt)
    elif enc and enc[0] == "HxByteRLE":
        src = raw[start:start + enc[1]]
        out = bytearray()
        i = 0
        while len(out) < count * dt.itemsize:
            c = src[i]
            i += 1
            if c & 0x80:
                out += src[i:i + (c & 0x7F)]
                i += c & 0x7F
            else:
                out += bytes([src[i]]) * c
                i += 1
        a = np.frombuffer(bytes(out[:count * dt.itemsize]), dt)
    elif enc and enc[0] == "HxZip":
        # A length of 0: the stream runs to its end.
        z = zlib.decompressobj().decompress(raw[start:start + enc[1]] if enc[1] else raw[start:])
        a = np.frombuffer(z, dt)[:count]
    else:
        a = np.frombuffer(raw[start:start + count * dt.itemsize], dt)
    a = a.reshape(nz, ny, nx, comp) if comp > 1 else a.reshape(nz, ny, nx)
    a = np.moveaxis(a, (0, 1, 2), (2, 1, 0))  # [x, y, z(, c)]
    sp = [(bbox[2 * i + 1] - bbox[2 * i]) / (size[i] - 1) * unit if size[i] > 1 else 1.0
          for i in range(3)]
    org = [bbox[2 * i] * unit for i in range(3)]
    return a, sp, org, typ, comp


def amira_case(name, data, typ, oracle_dtype, **kw):
    amira_write(name, data, typ, **kw)
    a, sp, org, typ_, comp = amira_read(os.path.join(OUT, name))
    colour = typ_ == "byte" and comp in (3, 4)
    vols = 1 if colour or comp == 1 else comp
    record(name, "Amira", oracle_dtype, "object", vols)
    affine = [[sp[0], 0, 0, org[0]], [0, -sp[1], 0, -org[1]], [0, 0, sp[2], org[2]]]
    geom(name, affine, a.shape[:3])
    if colour:
        vals(name, a[..., np.newaxis, :3])
    else:
        vals(name, a)


def amira_cases():
    amira_case("am_le.am", values_for("<u2", (4, 5, 7), 11), "ushort", "uint16",
               bbox=[10, 13, 2, 4, 0, 1.5], unit="µm")
    amira_case("am_be.am", values_for(">f4", (3, 4, 6), 12), "float", "float32",
               coding="BINARY", bbox=[0, 5, 0, 3, 0, 2])
    amira_case("am_ascii.am", values_for("<i2", (3, 4, 5), 13), "short", "int16",
               coding="ASCII", bbox=[-1, 1, -1, 2, 0, 4])
    # Long runs and short literals, for HxByteRLE.
    d = np.zeros((5, 7, 9), np.uint8)
    d[1:4, 2:6, 1:8] = 200
    d[2] += values_for("u1", (7, 9), 14) // 8
    amira_case("am_rle.am", d, "byte", "uint8", bbox=[0, 8, 0, 6, 0, 4], compress="rle")
    amira_case("am_zip.am", values_for("<i4", (3, 4, 5), 15), "int", "int32",
               bbox=[0, 4, 0, 3, 0, 2], compress="zip", unit="mm")
    amira_case("am_rgb.am", values_for("u1", (2, 3, 4, 3), 16), "byte", "uint8",
               bbox=[0, 3, 0, 2, 0, 1], comp=3)
    # Two interleaved components, shown as two volumes, after a section not read.
    amira_case("am_vec.am", values_for("<f4", (2, 3, 4, 2), 17), "float", "float32",
               bbox=[0, 3, 0, 2, 0, 1], comp=2, lead=("string", b"abc"))


# ---------------------------------------------------------------------------------------
# Bruker SkyScan: a folder of slices and the reconstruction log.

LOG = """[System]
Scanner=SkyScan1272
Instrument S/N=161002
Software Version=1.1.19
[Acquisition]
Number of Files=  {nfiles}
Source Voltage (kV)=  70
Source Current (uA)=142
Image Pixel Size (um)=   12.500000
Object to Source (mm)=93.100
Camera to Source (mm)=279.11
Filter=Al 0.5mm
Exposure (ms)=  1200
Rotation Step (deg)=0.400
Study Date and Time=11 Mar 2024  14h:07m:28s
[Reconstruction]
Reconstruction Program=NRecon
Program Version=Version: 1.7.4.6
Dataset Prefix=sample
Result File Type={rtype}
Filename Prefix={prefix}
First Section={first}
Last Section={last}
Section to Section Step=1
Pixel Size (um)=12.50000
Minimum for CS to Image Conversion={lo}
Maximum for CS to Image Conversion={hi}
Ring Artifact Correction=8
Beam Hardening Correction (%)=30
Smoothing=1
Time and Date=Mar 11, 2024  15:01:47
"""


def skyscan_case(folder, ext, dtype, lo, hi):
    d = os.path.join(OUT, folder)
    os.makedirs(d, exist_ok=True)
    for f in os.listdir(d):
        os.remove(os.path.join(d, f))
    prefix = "sample_rec"
    first, last = 10, 14
    w, h = 10, 8
    log = LOG.format(nfiles=900, rtype=ext.upper(), prefix=prefix, first=first, last=last,
                     lo=lo, hi=hi)
    with open(os.path.join(d, prefix + ".log"), "w", newline="\r\n") as f:
        f.write(log)
    # One section before the range, a spot image and the slices.
    for sec in range(first - 1, last + 1):
        a = values_for(dtype, (h, w), 100 + sec)
        path = os.path.join(d, f"{prefix}{sec:08d}.{ext}")
        if ext == "tif":
            tifffile.imwrite(path, a)
        elif dtype == "u1":
            Image.fromarray(a, "L").save(path)
        else:
            Image.fromarray(a.astype("<u2")).save(path)
    Image.fromarray(np.zeros((4, 4), np.uint8), "L").save(os.path.join(d, f"{prefix}_spr.{ext}"))
    # The oracle: the slices read back by PIL / tifffile.
    stack = []
    for sec in range(first, last + 1):
        path = os.path.join(d, f"{prefix}{sec:08d}.{ext}")
        a = tifffile.imread(path) if ext == "tif" else np.asarray(Image.open(path))
        stack.append(np.asarray(a, np.float64))
    s = np.stack(stack)  # [z, y, x]
    top = 255.0 if dtype == "u1" else 65535.0
    if dtype in ("u1", "<u2"):
        s = s * ((hi - lo) / top) + lo
    name = f"{folder}/{prefix}.log"
    names = {"u1": "uint8", "<u2": "uint16", "<f4": "float32"}
    record(name, "Bruker SkyScan", names[dtype], "object", 1)
    px = 0.0125
    geom(name, [[px, 0, 0, 0], [0, px, 0, 0], [0, 0, px, 0]], (w, h, last - first + 1))
    vals(name, np.transpose(s, (2, 1, 0)))
    return name


def skyscan_cases():
    n = skyscan_case("skyscan_bmp", "bmp", "u1", -0.005, 0.12)
    param_rows(n, [("content", "reconstruction"), ("instrument", "SkyScan1272"),
                   ("kv", "70.0"), ("current", "142.0"), ("sod", "93.1"),
                   ("beam-hardening", "30.0"), ("projections", "900.0"),
                   ("software", "NRecon 1.7.4.6")])
    skyscan_case("skyscan_png", "png", "<u2", 0.0, 0.06)
    skyscan_case("skyscan_tif", "tif", "<f4", 0.0, 0.06)


# ---------------------------------------------------------------------------------------
# Nikon X-Tek and Volume Graphics: INI headers beside raw volumes; numpy reads them back.

def xtek_cases():
    a = values_for("<f4", (4, 5, 6), 21)  # [z, y, x]
    write("xtek_f32.vol", a.tobytes())
    hdr = ("[XTekCT]\r\nName=xtek_f32\r\nVoxelsX=6\r\nVoxelsY=5\r\nVoxelsZ=4\r\n"
           "VoxelSizeX=0.05\r\nVoxelSizeY=0.05\r\nVoxelSizeZ=0.06\r\nOffsetX=1.5\r\n"
           "OffsetY=0\r\nOffsetZ=-2\r\nUnits=mm\r\nXraykV=160\r\nXrayuA=62\r\n"
           "Projections=1571\r\nSrcToObject=180.5\r\nSrcToDetector=1100\r\n"
           "Filter_Material=Copper\r\n")
    write("xtek_f32.xtekct", hdr.encode())
    back = np.fromfile(os.path.join(OUT, "xtek_f32.vol"), "<f4").reshape(4, 5, 6)
    aff = [[0.05, 0, 0, 1.5], [0, 0.05, 0, 0], [0, 0, 0.06, -2]]
    for name in ("xtek_f32.xtekct", "xtek_f32.vol"):
        record(name, "Nikon X-Tek", "float32", "object", 1)
        geom(name, aff, (6, 5, 4))
        vals(name, np.transpose(back, (2, 1, 0)))
    param_rows("xtek_f32.xtekct", [("kv", "160.0"), ("current", "62.0"),
                                   ("projections", "1571.0"), ("sod", "180.5"),
                                   ("sdd", "1100.0"), ("filter", "Copper")])
    # An .xtekVolume of 16-bit samples, the type told from the file's size, units in µm.
    a = values_for("<u2", (3, 4, 5), 22)
    write("xtek_u16.vol", a.tobytes())
    hdr = ("[XTekCT]\nVoxelsX=5\nVoxelsY=4\nVoxelsZ=3\nVoxelSizeX=20\nVoxelSizeY=20\n"
           "VoxelSizeZ=25\nUnits=um\n")
    write("xtek_u16.xtekVolume", hdr.encode())
    back = np.fromfile(os.path.join(OUT, "xtek_u16.vol"), "<u2").reshape(3, 4, 5)
    record("xtek_u16.xtekVolume", "Nikon X-Tek", "uint16", "object", 1)
    geom("xtek_u16.xtekVolume", [[0.02, 0, 0, 0], [0, 0.02, 0, 0], [0, 0, 0.025, 0]], (5, 4, 3))
    vals("xtek_u16.xtekVolume", np.transpose(back, (2, 1, 0)))


def vgi_cases():
    a = values_for(">u2", (3, 4, 6), 31)
    write("vgi_u16.raw", b"\xAA" * 16 + a.tobytes())
    hdr = ("{volume1}\n[representation]\nsize = 6 4 3\ndatatype = unsigned integer\n"
           "datarange = 0 65535\nbitsperelement = 16\n[file1]\nSkipHeader = 16\n"
           "FileFormat = raw\nSize = 6 4 3\nName = C:\\exports\\vgi_u16.raw\n"
           "Datatype = unsigned integer\ndatarange = 0 65535\nBitsPerElement = 16\n"
           "Endian = big\n{volumeprimitive12}\n[geometry]\nstatus = visible\n"
           "relativeposition = 0 0 0\nposition = 0 0 0\nresolution = 0.02 0.02 0.03\n"
           "scale = 1 1 1\ncenter = 0 0 0\nunit = mm\n[description]\ntext = test part\n")
    write("vgi_u16.vgi", hdr.encode())
    back = np.frombuffer(open(os.path.join(OUT, "vgi_u16.raw"), "rb").read()[16:], ">u2")
    back = back.reshape(3, 4, 6)
    for name in ("vgi_u16.vgi", "vgi_u16.raw"):
        record(name, "Volume Graphics VGI", "uint16", "object", 1)
        geom(name, [[0.02, 0, 0, 0], [0, 0.02, 0, 0], [0, 0, 0.03, 0]], (6, 4, 3))
        vals(name, np.transpose(back, (2, 1, 0)))


def raw_cases():
    # A headerless volume: refused until its layout is given, then read with it.
    a = values_for("<u2", (6, 8, 10), 41)
    name = "part_10x8x6_uint16.raw"
    write(name, b"\0" * 32 + a.tobytes())
    rows.append(f"error\t{name}\theaderless")
    back = np.fromfile(os.path.join(OUT, name), "<u2", offset=32).reshape(6, 8, 10)
    extra.append(f"layout\t{name}\t10\t8\t6\tuint16\t0\t32\t0.1\t0.1\t0.2")
    extra.append(f"guess\t{name}\t10\t8\t6\tuint16\t32")
    vals(name, np.transpose(back, (2, 1, 0)), extra)
    # A big-endian float cube with nothing in its name: 256 bytes are 4³ float32 samples
    # (no uint16 cube fills them), the first guess.
    c = values_for(">f4", (4, 4, 4), 42)
    write("cube.vol", c.tobytes())
    extra.append("guess\tcube.vol\t4\t4\t4\tfloat32\t0")
    extra.append("layout\tcube.vol\t4\t4\t4\tfloat32\t1\t0\t1\t1\t1")
    back = np.frombuffer(open(os.path.join(OUT, "cube.vol"), "rb").read(), ">f4")
    vals("cube.vol", np.transpose(back.reshape(4, 4, 4), (2, 1, 0)), extra)


def main():
    import txrm2tiff
    import PIL
    xradia_cases()
    amira_cases()
    skyscan_cases()
    xtek_cases()
    vgi_cases()
    raw_cases()
    head = (f"# Written by gen/make_indct.py (txrm2tiff {getattr(txrm2tiff, '__version__', '?')}, "
            f"olefile {olefile.__version__}, tifffile {tifffile.__version__}, "
            f"Pillow {PIL.__version__}, numpy {np.__version__}); do not edit.\n")
    with open(os.path.join(OUT, "expected-indct.tsv"), "w") as f:
        f.write(head + "\n".join(rows) + "\n")
    with open(os.path.join(OUT, "expected-indct-extra.tsv"), "w") as f:
        f.write(head + "\n".join(extra) + "\n")


if __name__ == "__main__":
    main()
