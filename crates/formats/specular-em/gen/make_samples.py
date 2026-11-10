#!/usr/bin/env python3
"""Writes EM and SPM sample files for the specular-em and specular-spm tests.

    make_samples.py fixtures OUTDIR   the tiny committed fixtures (tests/data of both crates:
                                      OUTDIR/em and OUTDIR/spm)
    make_samples.py big OUTDIR        large files for open-time measurements (not committed)
    make_samples.py shots OUTDIR      the app's screenshot-test images (app/specular/tests/data/
                                      science: a micrograph, an MRC stack, an AFM height map)

Needs numpy, mrcfile and gwyfile. DigitalMicrograph files, Igor binary waves, Gwyddion 1
(GWYO) files and the MRC modes mrcfile cannot write (3 and 101) come from the small writers
below, written from the format descriptions; the expected values are then read back by the
oracles (gen/oracle.py), not taken from here.

Values follow v(x, y, z) = (7x + 13y + 31z) % 97 - 40 (shifted to be non-negative for
unsigned types, scaled by 0.25 for floats); complex samples use that as the real part and
(3x + 5y) % 11 - 5 as the imaginary part.
"""

import os
import struct
import sys

import numpy as np


def ramp(w, h, d=1):
    z, y, x = np.meshgrid(np.arange(d), np.arange(h), np.arange(w), indexing="ij")
    return ((7 * x + 13 * y + 31 * z) % 97 - 40).astype(np.float64), ((3 * x + 5 * y) % 11 - 5).astype(np.float64)


def typed(a, im, dtype):
    dt = np.dtype(dtype)
    if dt.kind == "c":
        return (a + 1j * im).astype(dt)
    if dt.kind == "f":
        return (a * 0.25).astype(dt)
    if dt.kind == "u":
        return (a + 40).astype(dt)
    if dt.kind == "b":
        return (a > 0)
    return a.astype(dt)


# --- DigitalMicrograph -----------------------------------------------------------------------


class Dm:
    """A DM3 / DM4 writer: tags are (name, value) where value is a list (a group) or a
    (typewords, bytes) pair."""

    def __init__(self, dm4, little):
        self.dm4, self.little = dm4, little
        self.e = "<" if little else ">"

    def word(self, v):
        return struct.pack(">Q" if self.dm4 else ">I", v)

    def group(self, tags):
        b = bytes([0, 1]) + self.word(len(tags))
        for name, v in tags:
            n = name.encode("latin-1")
            if isinstance(v, list):
                body = self.group(v)
                kind = 20
            else:
                info, payload = v
                body = b"%%%%" + self.word(len(info)) + b"".join(self.word(i) for i in info) + payload
                kind = 21
            b += bytes([kind]) + struct.pack(">H", len(n)) + n
            if self.dm4:
                b += struct.pack(">Q", len(body))
            b += body
        return b

    def u32(self, v):
        return ([5], struct.pack(self.e + "I", v))

    def f32(self, v):
        return ([6], struct.pack(self.e + "f", v))

    def f64(self, v):
        return ([7], struct.pack(self.e + "d", v))

    def units(self, s):
        return ([20, 4, len(s)], s.encode("utf-16-le" if self.little else "utf-16-be"))

    def string(self, s):
        b = s.encode("latin-1")
        return ([18, len(b)], b)

    def array(self, a, code):
        a = np.ascontiguousarray(a)
        b = a.astype(a.dtype.newbyteorder(self.e)).tobytes()
        size = {2: 2, 4: 2, 5: 4, 6: 4, 7: 8, 10: 1, 9: 1, 8: 1}[code]
        return ([20, code, len(b) // size], b)

    def file(self, root):
        body = self.group(root)
        head = struct.pack(">I", 4 if self.dm4 else 3)
        head += struct.pack(">Q" if self.dm4 else ">I", len(body) + 24)
        head += struct.pack(">I", 1 if self.little else 0)
        return head + body + b"\0" * 8


DM_TYPES = {
    1: ("i2", 2),
    2: ("f4", 6),
    3: ("c8", 7),
    6: ("u1", 10),
    7: ("i4", 5),
    9: ("i1", 9),
    10: ("u2", 4),
    11: ("u4", 5),
    12: ("f8", 7),
    14: ("bool", 8),
}


def dm_image(dm, data_type, data, dims, cals, tags=None, name=None):
    calib = [
        ("Brightness", [("Origin", dm.f32(0.0)), ("Scale", dm.f32(1.0)), ("Units", dm.units("counts"))]),
        (
            "Dimension",
            [
                ("", [("Origin", dm.f32(o)), ("Scale", dm.f32(s)), ("Units", dm.units(u))])
                for (o, s, u) in cals
            ],
        ),
    ]
    image_data = [
        ("Calibrations", calib),
        ("Data", data),
        ("DataType", dm.u32(data_type)),
        ("Dimensions", [("", dm.u32(d)) for d in dims]),
        ("PixelDepth", dm.u32(4)),
    ]
    g = [("ImageData", image_data), ("ImageTags", tags or [])]
    if name:
        g.append(("Name", dm.units(name)))
    return g


def dm_sample(path, dm4, little, data_type, w, h, d=1, rgb=False):
    dm = Dm(dm4, little)
    a, im = ramp(w, h, d)
    if rgb:
        bgra = np.zeros((d, h, w, 4), np.uint8)
        bgra[..., 2] = (a + 40).astype(np.uint8)  # red
        bgra[..., 1] = (a + 50).astype(np.uint8)
        bgra[..., 0] = (a + 60).astype(np.uint8)
        bgra[..., 3] = 255
        data = ([20, 5, w * h * d], bgra.tobytes())
    else:
        dtype, code = DM_TYPES[data_type]
        v = typed(a, im, dtype)
        if dtype == "bool":
            v = v.astype(np.uint8)
        data = dm.array(v, code)
    dims = [w, h] + ([d] if d > 1 else [])
    cals = [(2.0, 0.5, "nm"), (0.0, 0.5, "nm"), (0.0, 1.5, "eV")][: len(dims)]
    tags = [
        (
            "Microscope Info",
            [
                ("Voltage", dm.f64(300000.0)),
                ("Indicated Magnification", dm.f64(88000.0)),
                ("Microscope", dm.string("Synthetic TEM")),
                ("Operation Mode", dm.string("IMAGING")),
            ],
        ),
        (
            "DataBar",
            [
                ("Exposure Time (s)", dm.f64(0.5)),
                ("Acquisition Date", dm.string("1/2/2026")),
                ("Acquisition Time", dm.string("3:04:05 PM")),
                ("Device Name", dm.string("Synthetic camera")),
            ],
        ),
    ]
    thumb = dm_image(dm, 6, dm.array(np.zeros((2, 2), np.uint8), 10), [2, 2], [(0, 1, ""), (0, 1, "")])
    root = [
        ("ApplicationBounds", [("", dm.u32(0))]),
        ("ImageList", [("", thumb), ("", dm_image(dm, 23 if rgb else data_type, data, dims, cals, tags, "sample"))]),
        ("Thumbnails", [("", [("ImageIndex", dm.u32(0))])]),
    ]
    with open(path, "wb") as f:
        f.write(dm.file(root))


# --- MRC -------------------------------------------------------------------------------------


def mrc_header(nx, ny, nz, mode, little=True, voxel=(1.5, 1.5, 2.0), nsymbt=0, exttyp=b"\0\0\0\0", ispg=0):
    e = "<" if little else ">"
    h = bytearray(1024)
    struct.pack_into(e + "4i", h, 0, nx, ny, nz, mode)
    struct.pack_into(e + "3i", h, 28, nx, ny, nz)
    struct.pack_into(e + "3f", h, 40, nx * voxel[0], ny * voxel[1], nz * voxel[2])
    struct.pack_into(e + "3i", h, 64, 1, 2, 3)
    struct.pack_into(e + "2i", h, 88, ispg, nsymbt)
    h[104:108] = exttyp
    struct.pack_into(e + "i", h, 108, 20140)
    h[208:212] = b"MAP "
    h[212:214] = b"\x44\x44" if little else b"\x11\x11"
    struct.pack_into(e + "i", h, 220, 1)
    h[224:224 + 18] = b"specular synthetic"
    return bytes(h)


def mrc_samples(out):
    import mrcfile

    w, h, d = 7, 5, 3
    a, im = ramp(w, h, d)
    for mode, dtype in [(0, "i1"), (1, "i2"), (2, "f4"), (4, "c8"), (6, "u2"), (12, "f2")]:
        p = os.path.join(out, f"mode{mode}.mrc")
        with mrcfile.new(p, overwrite=True) as m:
            m.set_data(typed(a, im, dtype))
            m.voxel_size = (1.5, 1.5, 2.0)
    with mrcfile.new(os.path.join(out, "stack.mrcs"), overwrite=True) as m:
        m.set_data(typed(a, im, "f4"))
        m.set_image_stack()
        m.voxel_size = (0.8, 0.8, 1.0)
    # Big-endian, written by hand.
    with open(os.path.join(out, "mode1-be.mrc"), "wb") as f:
        f.write(mrc_header(w, h, d, 1, little=False))
        f.write(typed(a, im, ">i2").tobytes())
    # Mode 3 (complex int16) and 101 (4-bit, rows padded to a byte), by hand.
    with open(os.path.join(out, "mode3.mrc"), "wb") as f:
        f.write(mrc_header(w, h, d, 3))
        c = np.stack([a, im], axis=-1).astype("<i2")
        f.write(c.tobytes())
    with open(os.path.join(out, "mode101.mrc"), "wb") as f:
        f.write(mrc_header(w, h, d, 101))
        v = ((a + 40).astype(np.uint8) % 16)
        rows = []
        for z in range(d):
            for y in range(h):
                row = list(v[z, y]) + ([0] if w % 2 else [])
                rows.append(bytes(row[i] | (row[i + 1] << 4) for i in range(0, len(row), 2)))
        f.write(b"".join(rows))
    # An FEI2 extended header (one 768-byte record per section).
    rec = bytearray(768)
    struct.pack_into("<iii", rec, 0, 768, 2, 0)
    struct.pack_into("<d", rec, 12, 46000.5)
    rec[20:36] = b"Synthetic Krios\0"
    struct.pack_into("<d", rec, 84, 300000.0)
    struct.pack_into("<dd", rec, 156, 1.1e-10, 1.1e-10)
    struct.pack_into("<i", rec, 244, 1)
    struct.pack_into("<d", rec, 289, 105000.0)
    struct.pack_into("<d", rec, 419, 2.5)
    rec[435:451] = b"Falcon synthetic"
    with open(os.path.join(out, "fei2.mrc"), "wb") as f:
        f.write(mrc_header(w, h, d, 2, nsymbt=768 * d, exttyp=b"FEI2"))
        f.write(bytes(rec) * d)
        f.write(typed(a, im, "<f4").tobytes())


def ccp4_map(path, xyz, mode, order, little=True, start=(0, 0, 0), ispg=19, symops=("X,Y,Z", "-X+1/2,-Y,Z+1/2"),
             cell_angles=(90.0, 90.0, 90.0), voxel=(0.8, 0.9, 1.1)):
    """A crystallographic map written from the CCP4 format description: `xyz` (z, y, x) values
    stored with columns, rows and sections along `order` (MAPC, MAPR, MAPS: 1 x, 2 y, 3 z),
    an 80-character record per symmetry operator after the header."""
    e = "<" if little else ">"
    sz, sy, sx = xyz.shape
    size = {1: sx, 2: sy, 3: sz}
    nc, nr, ns = size[order[0]], size[order[1]], size[order[2]]
    sym = b"".join(op.ljust(80).encode() for op in symops)
    h = bytearray(1024)
    struct.pack_into(e + "4i", h, 0, nc, nr, ns, mode)
    struct.pack_into(e + "3i", h, 16, *start)
    struct.pack_into(e + "3i", h, 28, sx, sy, sz)
    struct.pack_into(e + "3f", h, 40, sx * voxel[0], sy * voxel[1], sz * voxel[2])
    struct.pack_into(e + "3f", h, 52, *cell_angles)
    struct.pack_into(e + "3i", h, 64, *order)
    struct.pack_into(e + "3f", h, 76, float(xyz.min()), float(xyz.max()), float(xyz.mean()))
    struct.pack_into(e + "2i", h, 88, ispg, len(sym))
    h[208:212] = b"MAP "
    h[212:214] = b"\x44\x41" if little else b"\x11\x11"
    struct.pack_into(e + "f", h, 216, float(xyz.std()))
    struct.pack_into(e + "i", h, 220, 1)
    h[224:224 + 22] = b"specular synthetic map"
    # Stored index (s, r, c) holds xyz at the axes `order` names.
    axis = {1: 2, 2: 1, 3: 0}  # format axis -> numpy axis of xyz (z, y, x)
    stored = np.transpose(xyz, (axis[order[2]], axis[order[1]], axis[order[0]]))
    dt = {0: "i1", 1: "i2", 2: "f4", 6: "u2"}[mode]
    with open(path, "wb") as f:
        f.write(bytes(h) + sym + np.ascontiguousarray(stored).astype(e + dt).tobytes())


def ccp4_samples(out):
    """CCP4 maps in other axis orders and modes, with symmetry records and start indices."""
    d, h, w = 6, 5, 4
    z, y, x = np.meshgrid(np.arange(d), np.arange(h), np.arange(w), indexing="ij")
    v = (11 * x + 5 * y + 3 * z) % 50 - 20
    ccp4_map(os.path.join(out, "zxy.map"), v * 0.5, 2, (3, 1, 2), start=(-2, 3, 1))
    ccp4_map(os.path.join(out, "yzx-be.map"), v, 1, (2, 3, 1), little=False, start=(4, -1, 0))
    ccp4_map(os.path.join(out, "mode0.ccp4"), v, 0, (1, 2, 3), start=(1, 2, 3), symops=("X,Y,Z",), ispg=1)
    ccp4_map(os.path.join(out, "zyx-u16.map"), v + 20, 6, (3, 2, 1), cell_angles=(90.0, 100.0, 90.0), ispg=4,
             symops=("X,Y,Z", "-X,Y+1/2,-Z"))


# --- Gwyddion --------------------------------------------------------------------------------


def gwy_samples(out):
    from gwyfile.objects import GwyContainer, GwyDataField, GwySIUnit

    w, h = 9, 6
    a, _ = ramp(w, h, 2)
    obj = GwyContainer()
    obj["/0/data"] = GwyDataField(
        (a[0] * 1e-9).astype(np.float64),
        xreal=4.5e-6,
        yreal=3e-6,
        xoff=1e-6,
        si_unit_xy=GwySIUnit(unitstr="m"),
        si_unit_z=GwySIUnit(unitstr="m"),
    )
    obj["/0/data/title"] = "Height"
    obj["/1/data"] = GwyDataField(
        (a[1] * 0.01 + 0.5 * np.arange(w)[None, :] + 0.25 * np.arange(h)[:, None]).astype(np.float64),
        xreal=4.5e-6,
        yreal=3e-6,
        si_unit_xy=GwySIUnit(unitstr="m"),
        si_unit_z=GwySIUnit(unitstr="V"),
    )
    obj["/1/data/title"] = "Amplitude"
    meta = GwyContainer()
    meta["Scan rate"] = "1 Hz"
    obj["/0/meta"] = meta
    obj.tofile(os.path.join(out, "channels.gwy"))
    # Gwyddion 1: a GWYO container (GLib type, key, value items).
    field = GwyDataField(
        a[0].astype(np.float64), xreal=2e-6, yreal=1e-6,
        si_unit_xy=GwySIUnit(unitstr="m"), si_unit_z=GwySIUnit(unitstr="m"),
    ).serialize()
    items = struct.pack("<I", 64) + b"/0/base/palette\0Gray\0"
    items += struct.pack("<I", 80) + b"/0/data\0" + field
    items += struct.pack("<I", 20) + b"/0/show\0" + b"\x01"
    with open(os.path.join(out, "old.gwy"), "wb") as f:
        f.write(b"GWYOGwyContainer\0" + struct.pack("<I", len(items)) + items)


# --- Igor ------------------------------------------------------------------------------------


IBW_TYPES = {"f4": 2, "f8": 4, "i1": 8, "i2": 0x10, "i4": 0x20, "u1": 0x48, "u2": 0x50, "u4": 0x60, "c8": 3}


def ibw5(path, data, dtype, little=True, labels=None, note=b"", units=("m", "m", "", "")):
    """data is (layers, y, x); Igor stores x fastest."""
    e = "<" if little else ">"
    nx, ny, nz = data.shape[2], data.shape[1], data.shape[0]
    raw = data.astype(np.dtype(dtype).newbyteorder(e)).tobytes()
    lab = b""
    lab_sizes = [0, 0, 0, 0]
    if labels:
        entries = [b""] + [s.encode() for s in labels]
        lab = b"".join(x.ljust(32, b"\0") for x in entries)
        lab_sizes[2] = len(lab)
    bh = struct.pack(e + "hhi", 5, 0, 320 + len(raw))
    bh += struct.pack(e + "iii", 0, len(note), 0)
    bh += struct.pack(e + "4i", 0, 0, 0, 0)
    bh += struct.pack(e + "4i", *lab_sizes)
    bh += struct.pack(e + "iii", 0, 0, 0)
    assert len(bh) == 64
    wh = bytearray(320)
    struct.pack_into(e + "i", wh, 12, nx * ny * nz)
    struct.pack_into(e + "h", wh, 16, IBW_TYPES[dtype])
    wh[28:32] = b"wave"
    struct.pack_into(e + "4i", wh, 68, nx, ny, nz if nz > 1 else 0, 0)
    struct.pack_into(e + "4d", wh, 84, 2e-8, 2e-8, 1.0, 1.0)
    struct.pack_into(e + "4d", wh, 116, 0.0, 1e-7, 0.0, 0.0)
    wh[148:148 + len(units[0])] = units[0].encode()
    wh[152:152 + len(units[1])] = units[1].encode()
    wh[156:156 + len(units[1])] = units[1].encode()
    with open(path, "wb") as f:
        f.write(bh + bytes(wh) + raw + note + lab)


def ibw_samples(out):
    w, h, d = 8, 5, 3
    a, im = ramp(w, h, d)
    note = b"ScanSize: 1.6e-07\rImagingMode: AC Mode\rScanRate: 1.0\rSpringConstant: 2.1\r"
    ibw5(os.path.join(out, "height.ibw"), typed(a, im, "f4"), "f4", labels=["HeightRetrace", "AmplitudeRetrace", "PhaseRetrace"], note=note)
    ibw5(os.path.join(out, "be-i16.ibw"), typed(a, im, "i2"), "i2", little=False)
    ibw5(os.path.join(out, "u8.ibw"), typed(a, im, "u1")[:1], "u1")
    ibw5(os.path.join(out, "complex.ibw"), typed(a, im, "c8")[:1], "c8")
    # Version 2, big-endian, one dimension.
    v = typed(a[0, 0], im[0, 0], ">f4")
    raw = v.tobytes()
    wh = bytearray(110)
    struct.pack_into(">h", wh, 0, 2)
    wh[6:10] = b"line"
    wh[34:35] = b"V"
    wh[38:39] = b"s"
    struct.pack_into(">i", wh, 42, len(v))
    struct.pack_into(">dd", wh, 48, 0.5, -1.0)
    n2 = b"Comment: version 2\r"
    bh = struct.pack(">hiiih", 2, 110 + len(raw) + 16, len(n2), 0, 0)
    with open(os.path.join(out, "v2.ibw"), "wb") as f:
        f.write(bh + bytes(wh) + raw + b"\0" * 16 + n2)


# --- big files -------------------------------------------------------------------------------


def big(out):
    import mrcfile
    from gwyfile.objects import GwyContainer, GwyDataField, GwySIUnit

    os.makedirs(out, exist_ok=True)
    rng = np.random.default_rng(7)
    img = rng.normal(1000, 50, (4096, 4096)).astype(np.float32)
    dm = Dm(True, True)
    root = [("ImageList", [("", dm_image(dm, 2, dm.array(img, 6), [4096, 4096], [(0, 0.1, "nm"), (0, 0.1, "nm")]))])]
    with open(os.path.join(out, "big-4096-f32.dm4"), "wb") as f:
        f.write(dm.file(root))
    with mrcfile.new(os.path.join(out, "big-stack-u16.mrcs"), overwrite=True) as m:
        m.set_data(rng.integers(0, 4000, (8, 4096, 4096), dtype=np.uint16))
        m.set_image_stack()
    obj = GwyContainer()
    for i in range(2):
        obj[f"/{i}/data"] = GwyDataField(
            rng.normal(0, 1e-9, (2048, 2048)), xreal=1e-5, yreal=1e-5,
            si_unit_xy=GwySIUnit(unitstr="m"), si_unit_z=GwySIUnit(unitstr="m"),
        )
    obj.tofile(os.path.join(out, "big-2048x2.gwy"))
    ibw5(os.path.join(out, "big-2048x4.ibw"), rng.normal(0, 1e-9, (4, 2048, 2048)).astype(np.float32), "f4",
         labels=["HeightRetrace", "AmplitudeRetrace", "PhaseRetrace", "ZSensorRetrace"])


def blobs(rng, w, h, n, sigma, amp):
    """`n` Gaussian blobs at random places."""
    y, x = np.mgrid[0:h, 0:w].astype(np.float64)
    v = np.zeros((h, w))
    for cx, cy, s, a in zip(rng.uniform(0, w, n), rng.uniform(0, h, n), rng.uniform(0.6, 1.4, n) * sigma, rng.uniform(0.5, 1.0, n) * amp):
        v += a * np.exp(-((x - cx) ** 2 + (y - cy) ** 2) / (2 * s * s))
    return v


def shots(out):
    import mrcfile

    os.makedirs(out, exist_ok=True)
    rng = np.random.default_rng(11)
    w, h = 128, 96
    # A bright-field micrograph: dark particles on a noisy carbon film.
    film = 9000 + rng.normal(0, 250, (h, w)) - blobs(rng, w, h, 14, 4.0, 5000)
    dm = Dm(False, True)
    tags = [
        ("Microscope Info", [
            ("Voltage", dm.f64(200000.0)),
            ("Indicated Magnification", dm.f64(150000.0)),
            ("Microscope", dm.string("Synthetic TEM")),
            ("Operation Mode", dm.string("IMAGING")),
        ]),
        ("DataBar", [
            ("Exposure Time (s)", dm.f64(1.0)),
            ("Device Name", dm.string("Synthetic camera")),
        ]),
    ]
    img = dm_image(dm, 1, dm.array(film.astype("<i2"), 2), [w, h], [(0, 0.25, "nm"), (0, 0.25, "nm")], tags, "Particles")
    with open(os.path.join(out, "particles.dm3"), "wb") as f:
        f.write(dm.file([("ImageList", [("", img)])]))
    # A particle stack: three noisy 8-bit sections.
    stack = np.stack([np.clip(blobs(rng, w, h, 6, 7.0, 90) + rng.normal(-40, 12, (h, w)), -128, 127) for _ in range(3)])
    with mrcfile.new(os.path.join(out, "stack.mrcs"), overwrite=True) as m:
        m.set_data(stack.astype(np.int8))
        m.set_image_stack()
        m.voxel_size = (1.06, 1.06, 1.0)
    # An AFM height map (metres): a tilted sample with islands, rows bottom-up as Igor stores.
    y, x = np.mgrid[0:h, 0:w].astype(np.float64)
    height = 4e-9 * x / w + 9e-9 * y / h + blobs(rng, w, h, 20, 3.5, 1.5e-9) + rng.normal(0, 5e-11, (h, w))
    note = b"ScanSize: 2.56e-06\rImagingMode: AC Mode\rScanRate: 1.0\r"
    ibw5(os.path.join(out, "height.ibw"), height[None], "f4", note=note)


def fixtures(out):
    em, spm = os.path.join(out, "em"), os.path.join(out, "spm")
    os.makedirs(em, exist_ok=True)
    os.makedirs(spm, exist_ok=True)
    for t in DM_TYPES:
        dm_sample(os.path.join(em, f"type{t}.dm3"), False, True, t, 6, 4)
    dm_sample(os.path.join(em, "stack.dm4"), True, True, 2, 6, 4, 3)
    dm_sample(os.path.join(em, "be.dm3"), False, False, 1, 6, 4, 2)
    dm_sample(os.path.join(em, "rgb.dm4"), True, True, 23, 5, 3, rgb=True)
    mrc_samples(em)
    ccp4_samples(em)
    gwy_samples(spm)
    ibw_samples(spm)


if __name__ == "__main__":
    if len(sys.argv) != 3 or sys.argv[1] not in ("fixtures", "big", "shots"):
        sys.exit(__doc__)
    if sys.argv[1] == "big":
        big(sys.argv[2])
    elif sys.argv[1] == "shots":
        shots(sys.argv[2])
    else:
        fixtures(sys.argv[2])
