#!/usr/bin/env python3
"""Writes the small DeltaVision samples in ../tests/data/dv and their oracle,
../tests/data/dv/expected-dv.tsv, read back with the `mrc` package (Priism/DeltaVision).

The files are written here byte by byte from the published Priism header layout; the `mrc`
package then reads them as the oracle: its section order (shape and axis letters), pixel
sizes, wavelengths, extended-header time stamps and every value. Nothing here is used at run
time. Run it with -I from a venv that has mrc and numpy:

    python3 -I crates/formats/specular-em/gen/make_dv.py

Rows (tab-separated):
    file NAME NZ NW NT SEQUENCE PX PY PZ
    wave NAME W NM
    time NAME T SECONDS            (time stamp of z 0, w 0, time point T)
    val NAME Z W T X Y VALUE       (Y counted from the file's first row: the bottom one)
    error NAME WORDS
"""

import os
import struct
import sys

import mrc
import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "..", "tests", "data", "dv")
rows = []
MODES = {0: "u1", 1: "i2", 2: "f4", 6: "u2", 7: "i4", 4: "c8"}


def write(name, nx, ny, nz, nw, nt, mode, seq, big=False, nfloats=14, times=None, title=None):
    e = ">" if big else "<"
    nsec = nz * nw * nt
    h = bytearray(1024)
    struct.pack_into(e + "4i", h, 0, nx, ny, nsec, mode)
    struct.pack_into(e + "3i", h, 28, 1, 1, 1)
    struct.pack_into(e + "3f", h, 40, 0.0645, 0.0645, 0.2)
    struct.pack_into(e + "3f", h, 52, 90.0, 90.0, 90.0)
    struct.pack_into(e + "3i", h, 64, 1, 2, 3)
    ext = bytearray()
    if nfloats:
        for k in range(nsec):
            fl = [0.0] * nfloats
            # Section k's (z, w, t), from the sequence.
            if seq == 0:
                z, t, w = k % nz, k // nz % nt, k // (nz * nt)
            elif seq == 1:
                w, z, t = k % nw, k // nw % nz, k // (nw * nz)
            else:
                z, w, t = k % nz, k // nz % nw, k // (nz * nw)
            fl[1] = (times or [0.0] * nt)[t] + 0.01 * z
            fl[8] = 0.05
            fl[10] = 488.0 + 100 * w
            fl[11] = 525.0 + 100 * w
            ext += struct.pack(e + f"{nfloats}f", *fl)
        ext += bytes((-len(ext)) % 1024)
    struct.pack_into(e + "i", h, 92, len(ext))
    struct.pack_into(e + "h", h, 96, -16224)
    struct.pack_into(e + "hh", h, 128, 0, nfloats)
    struct.pack_into(e + "hh", h, 180, nt, seq)
    waves = [488, 561, 640, 405, 700][:nw]
    struct.pack_into(e + "h", h, 196, nw)
    struct.pack_into(e + f"{len(waves)}h", h, 198, *waves)
    struct.pack_into(e + "3f", h, 208, 1.5, -2.0, 3.0)
    struct.pack_into(e + "i", h, 220, 1)
    title = title or name
    h[224:224 + len(title)] = title.encode()
    r = np.random.default_rng(len(title) * 31 + mode)
    dt = np.dtype(e + MODES[mode])
    if dt.kind == "f":
        data = (r.standard_normal((nsec, ny, nx)) * 100).astype(dt)
    elif dt.kind == "c":
        data = (r.standard_normal((nsec, ny, nx)) + 0j).astype(dt)
    else:
        info = np.iinfo(dt)
        data = r.integers(info.min, info.max, size=(nsec, ny, nx), endpoint=True).astype(dt)
    with open(os.path.join(OUT, name), "wb") as f:
        f.write(bytes(h) + bytes(ext) + data.tobytes())


def oracle(name, read=None):
    """`read`: the file the package reads, when not `name` (the package cannot read big-endian
    files under numpy 2: a little-endian twin with the same values is read instead)."""
    m = mrc.Mrc(os.path.join(OUT, read or name))
    h = m.hdr
    nt, nw = max(1, int(h.NumTimes)), max(1, int(h.NumWaves))
    nz = int(h.Num[2]) // nt // nw
    letters = m.axisOrderStr()
    d = np.asarray(m.data)
    if nt == nw == 1:
        letters = "zyx"
    rows.append(
        f"file\t{name}\t{nz}\t{nw}\t{nt}\t{int(h.ImgSequence)}\t"
        f"{float(h.d[0])!r}\t{float(h.d[1])!r}\t{float(h.d[2])!r}"
    )
    for w in range(nw):
        rows.append(f"wave\t{name}\t{w}\t{int(h.wave[w])}")
    ext = m.extended_header
    if ext is not None and nt > 1:
        ts = m.extFloats["timeStampSeconds"] if m.dv else None
        if ts is not None:
            # Section of (z 0, w 0, t): read the order back from the data's letters.
            for t in range(nt):
                idx = {"z": 0, "w": 0, "t": t}
                # The flat section number in file order (first letter slowest in numpy).
                lead = letters[:-2]
                sec = 0
                for c in lead:
                    n = {"z": nz, "w": nw, "t": nt}[c]
                    sec = sec * n + idx[c]
                rows.append(f"time\t{name}\t{t}\t{float(ts.reshape(-1)[sec])!r}")
    lead = letters[:-2]
    for index in np.ndindex(*d.shape[:-2]):
        pos = dict(zip(lead, index))
        plane = d[index]
        z, w, t = pos.get("z", 0), pos.get("w", 0), pos.get("t", 0)
        ny, nx = plane.shape
        for y in range(ny):
            for x in range(nx):
                rows.append(f"val\t{name}\t{z}\t{w}\t{t}\t{x}\t{y}\t{float(plane[y, x])!r}")
    del m


def main():
    os.makedirs(OUT, exist_ok=True)
    write("ztw.dv", 5, 4, 3, 2, 2, 6, 0, times=[0.0, 2.5])
    oracle("ztw.dv")
    write("wzt-be.dv", 4, 3, 2, 3, 2, 1, 1, big=True, times=[1.0, 4.0])
    write("twin.dv", 4, 3, 2, 3, 2, 1, 1, times=[1.0, 4.0], title="wzt-be.dv")
    oracle("wzt-be.dv", read="twin.dv")
    os.remove(os.path.join(OUT, "twin.dv"))
    write("zwt-float.dv", 3, 3, 2, 2, 2, 2, 2, times=[0.0, 1.0])
    oracle("zwt-float.dv")
    write("zstack-u8.dv", 6, 2, 4, 1, 1, 0, 0, nfloats=0)
    oracle("zstack-u8.dv")
    write("complex.dv", 2, 2, 1, 1, 1, 4, 0, nfloats=0)
    rows.append("error\tcomplex.dv\tcomplex")
    with open(os.path.join(OUT, "expected-dv.tsv"), "w") as f:
        f.write(f"# Written by gen/make_dv.py (mrc {mrc.__version__}); do not edit.\n")
        f.write("\n".join(rows) + "\n")
    print(f"{len(rows)} rows", file=sys.stderr)


if __name__ == "__main__":
    main()
