#!/usr/bin/env python3
"""Writes the oracles' expected planes and calibrations for EM and SPM files.

    oracle.py ROOT [OUT]

Walks ROOT for .dm3 / .dm4 (read with RosettaSciIO), .mrc / .mrcs / .map / .ccp4 (mrcfile;
maps stored in another axis order are put in x, y, z order and checked against gemmi), .gwy
(gwyfile), .ibw (igor2) and Gaussian .cube (ASE; z sections of y rows) files and writes,
for image K of each file REL (images without thumbnails for DM, channels by number for
Gwyddion, else 0), OUT/REL.K.txt and OUT/REL.K.f32.
OUT defaults to ROOT/expected; files under OUT itself are skipped.

The .txt is tab-separated lines:
    size W H P          width, height, plane count
    planes I...         the planes stored in the .f32 (first, second, middle, last)
    axis D SCALE OFFSET UNIT     per dimension, x first: value = OFFSET + index * SCALE
    voxel X Y Z         MRC voxel size (Å)
    origin X Y Z        the first grid point (Å) of a map in another axis order: ORIGIN, else
                        the start indices times the voxel size
    unit U              value unit (Gwyddion, Igor)
    title T             channel title (Gwyddion)
    label I NAME        plane label (Igor layer labels)
    complex             values are magnitudes
    error MESSAGE       the oracle could not read the file
The .f32 holds the stored planes as little-endian float32, each top row (file row 0) first.

Colour DM images give three planes per section (red, green, blue); complex values are
np.abs; MRC modes mrcfile cannot read (3 and 101) are read from mrcfile's parsed header with
numpy; Gwyddion 1 (GWYO) files, which gwyfile cannot read (it wants GWYP and UTF-8), are read
by the small object reader here. Run with python -I; the files are untrusted data and are only read.
"""

import os
import struct
import sys
import warnings

import numpy as np

warnings.simplefilter("ignore")


def pick(p):
    return sorted({i for i in (0, 1, p // 2, p - 1) if 0 <= i < p})


def write(out, rel, k, planes, lines):
    """planes: array (P, H, W)."""
    base = os.path.join(out, f"{rel}.{k}")
    os.makedirs(os.path.dirname(base), exist_ok=True)
    p, h, w = planes.shape
    keep = pick(p)
    with open(base + ".txt", "w") as f:
        f.write(f"size\t{w}\t{h}\t{p}\n")
        f.write("planes\t" + "\t".join(map(str, keep)) + "\n")
        for line in lines:
            f.write("\t".join(str(x) for x in line) + "\n")
    with open(base + ".f32", "wb") as f:
        f.write(np.ascontiguousarray(planes[keep]).astype("<f4").tobytes())


def error(out, rel, msg):
    base = os.path.join(out, f"{rel}.0")
    os.makedirs(os.path.dirname(base), exist_ok=True)
    with open(base + ".txt", "w") as f:
        f.write(f"error\t{type(msg).__name__}: {msg}".replace("\n", " ") + "\n")
    if os.path.exists(base + ".f32"):
        os.remove(base + ".f32")


def to_planes(a):
    a = np.asarray(a)
    if a.ndim == 1:
        return a.reshape(1, 1, -1)
    return a.reshape(-1, a.shape[-2], a.shape[-1])


def values(a):
    """Magnitudes for complex data, float32 for the rest."""
    lines = []
    if np.iscomplexobj(a):
        lines.append(("complex",))
        a = np.abs(a)
    return a.astype(np.float32), lines


def dm(path, rel, out):
    from rsciio.digitalmicrograph import file_reader

    sigs = file_reader(path)
    for k, s in enumerate(sigs):
        d = s["data"]
        lines = []
        if d.dtype.names:
            # Colour: sections of R, G, B planes.
            rgb = np.stack([d["R"], d["G"], d["B"]], axis=-1).astype(np.float32)
            if rgb.ndim == 2:
                rgb = rgb.reshape(1, 1, rgb.shape[0], 3)
            sec = rgb.reshape(-1, rgb.shape[-3], rgb.shape[-2], 3)
            planes = np.moveaxis(sec, -1, 1).reshape(-1, sec.shape[1], sec.shape[2])
        else:
            v, lines = values(d)
            planes = to_planes(v)
        for i, ax in enumerate(reversed(s["axes"])):
            lines.append(("axis", i, repr(float(ax.get("scale", 1.0))), repr(float(ax.get("offset", 0.0))), ax.get("units") or ""))
        write(out, rel, k, planes, lines)


def mrc(path, rel, out):
    import mrcfile

    with mrcfile.open(path, permissive=True, header_only=True) as m:
        h = m.header
        mode = int(h.mode)
        voxel = m.voxel_size
        nx, ny, nz = int(h.nx), int(h.ny), int(h.nz)
        bo = h.mode.dtype.byteorder
        le = bo == "<" or (bo in ("=", "|") and sys.byteorder == "little")
        offset = 1024 + int(h.nsymbt)
        order = (int(h.mapc), int(h.mapr), int(h.maps))
        start = (int(h.nxstart), int(h.nystart), int(h.nzstart))
        origin = (float(h.origin.x), float(h.origin.y), float(h.origin.z))
    lines = [("voxel", repr(float(voxel.x)), repr(float(voxel.y)), repr(float(voxel.z)))]
    if sorted(order) == [1, 2, 3] and order != (1, 2, 3):
        # A crystallographic map in another axis order: mrcfile's samples (sections, rows,
        # columns) put in z, y, x order by MAPC/MAPR/MAPS, checked against gemmi's own
        # reordering (gemmi reads little-endian maps only).
        import gemmi

        with mrcfile.open(path, permissive=True) as mf:
            stored = np.asarray(mf.data, dtype=np.float32)
        # Stored numpy axes are (sections, rows, columns) = format axes order[2], [1], [0].
        where = {order[2]: 0, order[1]: 1, order[0]: 2}
        planes = np.ascontiguousarray(np.transpose(stored, (where[3], where[2], where[1])))
        if le:
            m = gemmi.read_ccp4_map(path)
            m.setup(float("nan"), gemmi.MapSetup.ReorderOnly)
            xyz = np.array(m.grid, copy=True)  # (x, y, z)
            if not np.array_equal(np.transpose(xyz, (2, 1, 0)), planes):
                raise ValueError("gemmi and mrcfile disagree on the reordered map")
        if not any(origin):
            v = (float(voxel.x), float(voxel.y), float(voxel.z))
            o = [0.0, 0.0, 0.0]
            for k in range(3):
                o[order[k] - 1] = start[k] * v[order[k] - 1]
            origin = tuple(o)
        lines.append(("origin", *(repr(x) for x in origin)))
        write(out, rel, 0, planes, lines)
        return
    if mode in (3, 101):
        raw = open(path, "rb").read()[offset:]
        if mode == 3:
            a = np.frombuffer(raw, dtype=("<" if le else ">") + "i2", count=nx * ny * nz * 2).astype(np.float32)
            a = a.reshape(nz, ny, nx, 2)
            planes = np.hypot(a[..., 0], a[..., 1]).astype(np.float32)
            lines.append(("complex",))
        else:
            row = (nx + 1) // 2
            b = np.frombuffer(raw, dtype=np.uint8, count=row * ny * nz).reshape(nz, ny, row)
            nib = np.stack([b & 15, b >> 4], axis=-1).reshape(nz, ny, row * 2)[..., :nx]
            planes = nib.astype(np.float32)
        write(out, rel, 0, planes, lines)
        return
    with mrcfile.open(path, permissive=True) as m:
        v, extra = values(m.data)
    write(out, rel, 0, to_planes(v), lines + extra)


def cube(path, rel, out):
    """A Gaussian cube file's first value per point, read by ASE, as z sections of (y, x)."""
    from ase.io.cube import read_cube_data

    data, _atoms = read_cube_data(path)
    if data.ndim == 4:
        data = data[..., 0]
    write(out, rel, 0, np.transpose(np.asarray(data, dtype=np.float64), (2, 1, 0)).astype(np.float32), [])


class Obj(dict):
    """A Gwyddion object read by gwy_object (gwyfile only reads UTF-8 strings)."""

    name = ""


def gwy_object(raw, pos):
    """A serialized object at pos: (object, end). Strings are UTF-8, else Latin-1."""
    def cstr(p):
        z = raw.index(b"\0", p)
        b = raw[p:z]
        try:
            return b.decode("utf-8"), z + 1
        except UnicodeDecodeError:
            return b.decode("latin-1"), z + 1

    name, pos = cstr(pos)
    (size,) = struct.unpack_from("<I", raw, pos)
    pos += 4
    end = pos + size
    obj = Obj()
    obj.name = name
    fixed = {"b": ("<?", 1), "c": ("<B", 1), "i": ("<i", 4), "q": ("<q", 8), "d": ("<d", 8)}
    arrays = {"C": "<u1", "I": "<i4", "Q": "<i8", "D": "<f8"}
    while pos < end:
        key, pos = cstr(pos)
        t = chr(raw[pos])
        pos += 1
        if t in fixed:
            fmt, n = fixed[t]
            (obj[key],) = struct.unpack_from(fmt, raw, pos)
            pos += n
        elif t == "s":
            obj[key], pos = cstr(pos)
        elif t == "o":
            obj[key], pos = gwy_object(raw, pos)
        elif t in arrays:
            (n,) = struct.unpack_from("<I", raw, pos)
            pos += 4
            a = np.frombuffer(raw, dtype=arrays[t], count=n, offset=pos)
            obj[key] = a
            pos += a.nbytes
        else:
            raise ValueError(f"component type {t!r}")
    return obj, end


def gwy_old_container(raw):
    """Gwyddion 1: GWYO, "GwyContainer", size, then (GLib type, key, value) items."""
    pos = 4 + len(b"GwyContainer\0")
    (size,) = struct.unpack_from("<I", raw, pos)
    pos += 4
    end = pos + size
    items = {}
    sizes = {12: 1, 16: 1, 20: 1, 24: 4, 28: 4, 48: 4, 52: 4, 32: 8, 36: 8, 40: 8, 44: 8, 56: 4, 60: 8}
    while pos < end:
        (t,) = struct.unpack_from("<I", raw, pos)
        pos += 4
        z = raw.index(b"\0", pos)
        key = raw[pos:z].decode("latin-1")
        pos = z + 1
        if t == 64:
            z = raw.index(b"\0", pos)
            items[key] = raw[pos:z].decode("latin-1")
            pos = z + 1
        elif t == 80:
            items[key], pos = gwy_object(raw, pos)
        elif t in sizes:
            pos += sizes[t]
        else:
            break
    return items


def gwy(path, rel, out):
    from gwyfile.objects import GwyObject

    raw = open(path, "rb").read()
    if raw[:4] == b"GWYP":
        root = GwyObject.frombuffer(raw[4:])
    elif raw[:4] == b"GWYO":
        root = gwy_old_container(raw)
    else:
        raise ValueError("not a Gwyddion file")
    ids = []
    for key in root.keys():
        parts = key.split("/")
        if len(parts) == 3 and parts[0] == "" and parts[2] == "data" and parts[1].isdigit():
            if getattr(root[key], "name", "") == "GwyDataField":
                ids.append(int(parts[1]))
    for k, i in enumerate(sorted(ids)):
        df = root[f"/{i}/data"]
        xres, yres = df["xres"], df["yres"]
        data = np.asarray(df["data"]).reshape(yres, xres).astype(np.float32)
        def unit(name):
            u = df.get(name)
            if u is None:
                return ""
            s = u.get("unitstr", "")
            return s.decode("latin-1") if isinstance(s, bytes) else s
        lines = [
            ("axis", 0, repr(df.get("xreal", 1.0) / xres), repr(float(df.get("xoff", 0.0))), unit("si_unit_xy")),
            ("axis", 1, repr(df.get("yreal", 1.0) / yres), repr(float(df.get("yoff", 0.0))), unit("si_unit_xy")),
            ("unit", unit("si_unit_z")),
        ]
        title = root.get(f"/{i}/data/title")
        if title:
            lines.append(("title", title))
        write(out, rel, k, data.reshape(1, yres, xres), lines)


def ibw(path, rel, out):
    from igor2 import binarywave as bw

    w = bw.load(path)
    wave = w["wave"]
    wh = wave["wave_header"]
    d = np.asarray(wave["wData"])
    if d.dtype.kind in "SUO" or d.size == 0:
        raise ValueError(f"not numeric ({d.dtype}, {d.size} points)")
    v, lines = values(d)
    # wData is indexed [x, y, z, t] (x fastest in the file).
    planes = to_planes(v.T) if v.ndim > 1 else v.reshape(1, 1, -1)
    dec = lambda b: b.split(b"\0")[0].decode("latin-1") if isinstance(b, bytes) else str(b)
    if w["version"] == 5:
        units = wh["dimUnits"]
        ext = wave.get("dimension_units") or b""
        n = [x for x in wh["nDim"] if x > 0]
        for i in range(len(n)):
            u = dec(b"".join(bytes(x) for x in units[i]))
            lines.append(("axis", i, repr(float(wh["sfA"][i])), repr(float(wh["sfB"][i])), u))
        dunit = dec(wave.get("data_units") or b"") or dec(bytes(wh["dataUnits"]))
        # igor2 drops the NULs inside each 32-byte label instead of ending the label at the
        # first one (stale bytes after it show up), so the layer labels are taken from the
        # file: they follow the data, formula, note and units; entry 0 is the dimension's own.
        bh = wave["bin_header"]
        pos = 64 + bh["wfmSize"] + bh["formulaSize"] + bh["noteSize"] + bh["dataEUnitsSize"]
        pos += int(sum(bh["dimEUnitsSize"])) + int(bh["dimLabelsSize"][0]) + int(bh["dimLabelsSize"][1])
        raw = open(path, "rb").read()[pos : pos + int(bh["dimLabelsSize"][2])]
        for j in range(1, len(raw) // 32):
            name = dec(raw[32 * j : 32 * j + 32])
            if name:
                lines.append(("label", j - 1, name))
        del ext
    else:
        lines.append(("axis", 0, repr(float(wh["hsA"])), repr(float(wh["hsB"])), dec(bytes(wh["xUnits"]))))
        dunit = dec(bytes(wh["dataUnits"]))
    lines.append(("unit", dunit))
    write(out, rel, 0, planes, lines)


READERS = {".dm3": dm, ".dm4": dm, ".mrc": mrc, ".mrcs": mrc, ".map": mrc, ".ccp4": mrc, ".cube": cube, ".gwy": gwy, ".ibw": ibw}


def main():
    if len(sys.argv) not in (2, 3):
        sys.exit(__doc__)
    root = os.path.abspath(sys.argv[1])
    out = os.path.abspath(sys.argv[2]) if len(sys.argv) == 3 else os.path.join(root, "expected")
    n = bad = 0
    for dirpath, dirs, files in os.walk(root):
        dirs[:] = sorted(x for x in dirs if os.path.join(dirpath, x) != out and not x.startswith("."))
        for name in sorted(files):
            reader = READERS.get(os.path.splitext(name)[1].lower())
            if not reader:
                continue
            path = os.path.join(dirpath, name)
            rel = os.path.relpath(path, root)
            n += 1
            try:
                reader(path, rel, out)
            except Exception as e:  # noqa: BLE001 - recorded for the tests
                bad += 1
                error(out, rel, e)
    print(f"oracle: {n} files, {bad} the oracles could not read")


if __name__ == "__main__":
    main()
