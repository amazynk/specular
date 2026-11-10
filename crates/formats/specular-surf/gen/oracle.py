#!/usr/bin/env python3
"""Writes the oracles' expected height maps and calibrations for areal topography files.

    oracle.py ROOT [OUT]

Walks ROOT/surface for .x3p, .dat, .datx, .vk4, .plu, .al3d, .sur and .pro files and writes,
for each file REL (relative to ROOT), OUT/REL.txt and OUT/REL.f32 describing the first page
the reader should give. OUT defaults to ROOT/expected.

Oracles: RosettaSciIO's digitalsurf reader for .sur / .pro; h5py for Zygo .datx; surfalize
for .al3d, .dat, .plu, .vk4 and the .x3p files it reads; for other .x3p files (main.xml in a
folder, which surfalize does not find, or a date it cannot parse) a small zipfile + xml.etree + numpy reader here.

The .txt is tab-separated lines:
    tool NAME           the oracle used
    size W H P          width, height, plane count
    step DX DY          pixel pitch in metres (0 when the oracle gives none)
    zunit U             the unit of the expected values of planes the reader gives in metres
                        (they are compared after converting U to metres); "m" when already so
    rescaled WHY        the oracle's values were rescaled (surfalize reads MetroPro phase
                        as 4096 counts per wave; the header's phase resolution is applied)
    tail N              the last N values are not compared (surfalize reads Sensofar PLu
                        heights from 8 bytes before the data, so its plane is shifted by two)
    spectral            the file holds spectra, which the reader refuses
RosettaSciIO rounds the layers of Digital Surf colour files to integers; non-height planes
of its files are compared after rounding the reader's values.
    error MESSAGE       the oracle could not read the file
The .f32 holds the P planes as little-endian float32, top row (file row 0) first, NaN where
the oracle marks a point invalid.

Run with python -I; the files are untrusted data and are only read.
"""

import os
import struct
import sys
import warnings
import xml.etree.ElementTree as ET
import zipfile

import numpy as np

warnings.simplefilter("ignore")

EXTS = (".x3p", ".dat", ".datx", ".vk4", ".plu", ".al3d", ".sur", ".pro")


def write(out, rel, tool, planes, step, zunit="m", extra=()):
    """planes: array (P, H, W)."""
    base = os.path.join(out, rel)
    os.makedirs(os.path.dirname(base), exist_ok=True)
    p, h, w = planes.shape
    with open(base + ".txt", "w") as f:
        f.write(f"tool\t{tool}\n")
        f.write(f"size\t{w}\t{h}\t{p}\n")
        f.write(f"step\t{step[0]!r}\t{step[1]!r}\n")
        f.write(f"zunit\t{zunit}\n")
        for line in extra:
            f.write("\t".join(str(x) for x in line) + "\n")
    with open(base + ".f32", "wb") as f:
        f.write(np.ascontiguousarray(planes).astype("<f4").tobytes())


def error(out, rel, tool, msg):
    base = os.path.join(out, rel)
    os.makedirs(os.path.dirname(base), exist_ok=True)
    with open(base + ".txt", "w") as f:
        f.write(f"tool\t{tool}\n")
        f.write(f"error\t{type(msg).__name__}: {msg}".replace("\n", " ") + "\n")
    if os.path.exists(base + ".f32"):
        os.remove(base + ".f32")


def clean(s):
    return s.replace("\x00", "").strip()


def digitalsurf(path, out, rel):
    from rsciio.digitalsurf import file_reader

    s = file_reader(path)[0]
    data = np.asarray(s["data"], dtype=np.float64)
    axes = s["axes"]
    if any("Wavelength" in clean(a.get("name", "")) for a in axes):
        write(out, rel, "rsciio", np.zeros((1, 1, 1)), (0, 0), extra=[("spectral",)])
        return
    quantity = clean(s["metadata"].get("Signal", {}).get("quantity", ""))
    zunit = quantity[quantity.find("(") + 1 : quantity.rfind(")")] if "(" in quantity else ""
    zunit = clean(zunit)
    named = [a for a in axes if clean(a.get("name", "")) in ("X", "Y")]
    # RosettaSciIO reads a surface typed as a profile as one long row; the header's x
    # size reshapes it.
    if data.ndim == 1 and len(axes) == 1 and axes[0]["size"] != data.size:
        data = data.reshape(-1, axes[0]["size"])
    if data.ndim == 1:
        data = data.reshape(1, -1)
    if data.ndim == 2:
        data = data[None]
    lat = {clean(a.get("name", "")): a for a in named}
    if len(axes) == 1:
        lat = {"X": axes[0]}

    def step(k):
        a = lat.get(k)
        if a is None:
            return 0.0
        unit = clean(a.get("units", ""))
        m = METRES.get(unit)
        return float(a["scale"]) * m if m else float(a["scale"])

    write(out, rel, "rsciio", data, (step("X"), step("Y")), zunit or "-")


METRES = {"m": 1.0, "mm": 1e-3, "um": 1e-6, "µm": 1e-6, "nm": 1e-9, "pm": 1e-12}


def metropro_resolution(path):
    """Counts per wave of a MetroPro file's phase (header field phase_res)."""
    with open(path, "rb") as f:
        h = f.read(220)
    fmt = struct.unpack_from(">h", h, 4)[0]
    res = struct.unpack_from(">h", h, 218)[0]
    return {0: 4096, 1: 32768}.get(res, 131072 if fmt >= 2 and res == 2 else 4096)


def surfalize_read(path, out, rel, tail=0):
    import surfalize

    s = surfalize.Surface.load(path)
    data = np.asarray(s.data, dtype=np.float64) * 1e-6
    extra = []
    if path.lower().endswith(".dat"):
        # surfalize divides MetroPro phase by 4096 counts per wave whatever the header's
        # phase resolution says; the header's resolution is applied here.
        r = metropro_resolution(path)
        if r != 4096:
            data = data * 4096 / r
            extra.append(("rescaled", f"phase resolution {r}"))
    if tail:
        flat = data.ravel()
        data = np.concatenate([flat[tail:], np.full(tail, np.nan)]).reshape(data.shape)
        extra.append(("tail", tail))
    write(out, rel, "surfalize", data[None], (s.step_x * 1e-6, s.step_y * 1e-6), "m", extra)


def x3p_numpy(path, out, rel):
    z = zipfile.ZipFile(path)
    names = [n for n in z.namelist() if not n.startswith("__MACOSX")]
    main = next(n for n in names if n == "main.xml" or n.endswith("/main.xml"))
    folder = main[: -len("main.xml")]
    root = ET.fromstring(z.read(main))

    def find(p):
        e = root.find(p)
        return None if e is None or e.text is None else e.text.strip()

    w = int(find("Record3/MatrixDimension/SizeX"))
    h = int(find("Record3/MatrixDimension/SizeY"))
    dtype = find("Record1/Axes/CZ/DataType") or "D"
    inc = float(find("Record1/Axes/CZ/Increment") or 1)
    off = float(find("Record1/Axes/CZ/Offset") or 0)
    link = find("Record3/DataLink/PointDataLink")
    np_type = {"I": "<i2", "L": "<i4", "F": "<f4", "D": "<f8"}[dtype]
    raw = np.frombuffer(z.read(folder + link)[: w * h * np.dtype(np_type).itemsize], np_type, w * h).astype(np.float64)
    # Integer heights are counts of the increment; float heights are metres as stored.
    zs = raw * inc + off if dtype in "IL" else raw + off
    valid = find("Record3/DataLink/ValidPointsLink")
    if valid:
        bits = np.unpackbits(np.frombuffer(z.read(folder + valid), np.uint8), bitorder="little")
        zs[bits[: w * h] == 0] = np.nan
    dx = float(find("Record1/Axes/CX/Increment") or 0)
    dy = float(find("Record1/Axes/CY/Increment") or 0)
    write(out, rel, "numpy", zs.reshape(1, h, w), (dx, dy))


DATX_UNITS = {"Meters": 1.0, "MilliMeters": 1e-3, "MicroMeters": 1e-6, "NanoMeters": 1e-9}


def datx(path, out, rel):
    import h5py

    f = h5py.File(path, "r")
    name = sorted(f["Data/Surface"].keys())[0]
    d = f["Data/Surface"][name]
    a = d[()].astype(np.float64)
    unit = d.attrs["Unit"][0]
    unit = unit.decode() if isinstance(unit, bytes) else str(unit)
    if unit == "Fringes":
        k = (
            d.attrs["Wavelength"][0]
            * d.attrs["Interferometric Scale Factor"][0]
            * d.attrs["Obliquity Factor"][0]
        )
    else:
        k = DATX_UNITS[unit]
    a[a == d.attrs["No Data"][0]] = np.nan
    a = a * k
    planes = [a]
    for iname in sorted(f["Data/Intensity"].keys()) if "Data/Intensity" in f else []:
        i = f["Data/Intensity"][iname]
        if i.shape == a.shape:
            v = i[()].astype(np.float64)
            v[v == i.attrs["No Data"][0]] = np.nan
            planes.append(v)
    step = 0.0
    for g in f["Attributes"].values():
        if "Data Context.Lateral Resolution:Value" in g.attrs:
            step = float(g.attrs["Data Context.Lateral Resolution:Value"][0])
    write(out, rel, "h5py", np.stack(planes), (step, step), "-" if len(planes) > 1 else "m")


def one(path, out, rel):
    ext = os.path.splitext(path)[1].lower()
    if ext in (".sur", ".pro"):
        tool, fn = "rsciio", lambda: digitalsurf(path, out, rel)
    elif ext == ".datx":
        tool, fn = "h5py", lambda: datx(path, out, rel)
    elif ext == ".x3p":
        names = zipfile.ZipFile(path).namelist()
        if "main.xml" in names:
            tool, fn = "surfalize", lambda: surfalize_read(path, out, rel)
        else:
            tool, fn = "numpy", lambda: x3p_numpy(path, out, rel)
    elif ext == ".plu":
        tool, fn = "surfalize", lambda: surfalize_read(path, out, rel, tail=2)
    else:
        tool, fn = "surfalize", lambda: surfalize_read(path, out, rel)
    try:
        fn()
    except Exception as e:  # noqa: BLE001 - every oracle failure is recorded
        if ext != ".x3p" or tool == "numpy":
            error(out, rel, tool, e)
            return
        # surfalize refuses some valid X3P headers (dates it cannot parse).
        try:
            x3p_numpy(path, out, rel)
        except Exception as e2:  # noqa: BLE001
            error(out, rel, "numpy", e2)


def main():
    root = os.path.abspath(sys.argv[1])
    out = os.path.abspath(sys.argv[2]) if len(sys.argv) > 2 else os.path.join(root, "expected")
    n = 0
    for d, _, files in sorted(os.walk(os.path.join(root, "surface"))):
        if os.path.abspath(d).startswith(out):
            continue
        for name in sorted(files):
            if not name.lower().endswith(EXTS):
                continue
            path = os.path.join(d, name)
            one(path, out, os.path.relpath(path, root))
            n += 1
    print(f"surface oracle: {n} files")


if __name__ == "__main__":
    main()
