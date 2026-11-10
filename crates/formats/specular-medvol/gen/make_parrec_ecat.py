#!/usr/bin/env python3
"""Writes the small Philips PAR/REC and ECAT 7 samples in ../tests/data and their oracle,
../tests/data/expected-parrec-ecat.tsv (same rows as expected.tsv), read back with nibabel.

The files are written here byte by byte, laid out as nibabel's documentation of the formats
describes; nibabel then reads them as the oracle. Nothing here is used at run time. Run it
with -I from a venv that has nibabel and numpy:

    python3 -I crates/formats/specular-medvol/gen/make_parrec_ecat.py
"""

import os
import struct
import sys

import nibabel as nib
import numpy as np
from nibabel import ecat, parrec

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "..", "tests", "data")
RAS_TO_LPS = np.diag([-1.0, -1.0, 1.0, 1.0])

rows = []


def fmt(x):
    return repr(float(x))


def record(name, kind, dtype, orient, vols):
    rows.append(f"file\t{name}\t{kind}\t{dtype}\t{orient}\t{vols}")


def geom(name, affine_lps, shape):
    nx, ny, nz = (list(shape) + [1, 1, 1])[:3]
    pts = [(i, j, k) for i in (0, nx - 1) for j in (0, ny - 1) for k in (0, nz - 1)]
    pts.append((min(1, nx - 1), min(1, ny - 1), min(1, nz - 1)))
    for p in sorted(set(pts)):
        x = affine_lps @ np.array([p[0], p[1], p[2], 1.0])
        rows.append(f"geom\t{name}\t{p[0]}\t{p[1]}\t{p[2]}\t{fmt(x[0])}\t{fmt(x[1])}\t{fmt(x[2])}")


def vals(name, d):
    d = np.asarray(d, dtype=np.float64)
    if d.ndim == 3:
        d = d[..., np.newaxis]
    nx, ny, nz, nv = d.shape
    for v in range(nv):
        for k in range(nz):
            for j in range(ny):
                for i in range(nx):
                    rows.append(f"val\t{name}\t{i}\t{j}\t{k}\t{v}\t0\t{fmt(d[i, j, k, v])}")


# --- PAR/REC -------------------------------------------------------------------------------

GENERAL = """.    Patient name                       :   sample
.    Examination name                   :   specular
.    Protocol name                      :   {protocol}
.    Examination date/time              :   2020.03.04 / 10:11:12
.    Series Type                        :   Image   MRSERIES
.    Acquisition nr                     :   1
.    Reconstruction nr                  :   1
.    Scan Duration [sec]                :   10
.    Max. number of cardiac phases      :   1
.    Max. number of echoes              :   1
.    Max. number of slices/locations    :   {slices}
.    Max. number of dynamics            :   {dynamics}
.    Max. number of mixes               :   1
.    Patient position                   :   Head First Supine
.    Preparation direction              :   Anterior-Posterior
.    Technique                          :   FEEPI
.    Scan resolution  (x, y)            :   {w}  {h}
.    Scan mode                          :   MS
.    Repetition time [ms]               :   2000.000
.    FOV (ap,fh,rl) [mm]                :   240.000  70.000  240.000
.    Water Fat shift [pixels]           :   11.050
.    Angulation midslice(ap,fh,rl)[degr]:   {ang}
.    Off Centre midslice(ap,fh,rl) [mm] :   {off}
.    Flow compensation <0=no 1=yes> ?   :   0
.    Presaturation     <0=no 1=yes> ?   :   0
.    Phase encoding velocity [cm/sec]   :   0.000000  0.000000  0.000000
.    MTC               <0=no 1=yes> ?   :   0
.    SPIR              <0=no 1=yes> ?   :   1
.    EPI factor        <0,1=no EPI>     :   39
.    Dynamic scan      <0=no 1=yes> ?   :   {dyn}
.    Diffusion         <0=no 1=yes> ?   :   0
.    Diffusion echo time [ms]           :   0.0000
"""

V41 = """.    Max. number of diffusion values    :   1
.    Max. number of gradient orients    :   1
"""
V42 = """.    Number of label types   <0=no ASL> :   0
"""


def par_line(version, sl, dyn, idx, bits, w, h, ri, rs, orient, thick, gap, spacing):
    v = [sl, 1, dyn, 1, 0, 2, idx, bits, 100, w, h, ri, rs, 0.004, 1070, 1860,
         0.0, 0.0, 0.0, 0.0, 0.0, 0.0, thick, gap, 0, orient, 0, 2, spacing[0], spacing[1],
         30.0, 0.0, 0.0, 0.0, 1, 90.0, 0, 0, 0, 39, 0.0]
    if version in ("V4.1", "V4.2"):
        v += [1, 1, 0, 0, 0.0, 0.0, 0.0]
    if version == "V4.2":
        v += [1]
    return " ".join(str(x) for x in v)


def write_par(name, version, order, w, h, bits, scales, orient, ang, off, slices, dynamics,
              thick=4.0, gap=1.0, spacing=(1.5, 2.0)):
    """`order`: (slice, dynamic) per stored image; `scales`: (RI, RS) per stored image."""
    text = "# === DATA DESCRIPTION FILE ======================================================\n#\n"
    text += f"# CLINICAL TRYOUT             Research image export tool     {version}\n#\n"
    text += "# === GENERAL INFORMATION ========================================================\n#\n"
    text += GENERAL.format(protocol=name, slices=slices, dynamics=dynamics, w=w, h=h,
                           ang="  ".join(f"{a:.3f}" for a in ang),
                           off="  ".join(f"{a:.3f}" for a in off), dyn=int(dynamics > 1))
    if version in ("V4.1", "V4.2"):
        text += V41
    if version == "V4.2":
        text += V42
    text += "#\n# === IMAGE INFORMATION ==========================================================\n#\n\n"
    for i, ((sl, dyn), (ri, rs)) in enumerate(zip(order, scales)):
        text += par_line(version, sl, dyn, i, bits, w, h, ri, rs, orient, thick, gap, spacing) + "\n"
    text += "\n# === END OF DATA DESCRIPTION FILE ===============================================\n"
    with open(os.path.join(OUT, name + ".PAR"), "w") as f:
        f.write(text)
    r = np.random.default_rng(len(name) * 7 + bits)
    dt = np.uint8 if bits == 8 else np.uint16
    data = r.integers(0, np.iinfo(dt).max, size=(len(order), h, w), endpoint=True).astype("<" + dt().dtype.str[1:])
    data.reshape(-1)[0] = np.iinfo(dt).max
    with open(os.path.join(OUT, name + ".REC"), "wb") as f:
        f.write(data.tobytes())


def par_case(name, truncated=False):
    img = parrec.load(os.path.join(OUT, name + ".PAR"), permit_truncated=truncated)
    d = img.get_fdata(dtype=np.float64)
    vols = d.shape[3] if d.ndim > 3 else 1
    sl = img.dataobj._slice_scaling
    uniform = sl is None or (np.all(sl[0] == sl[0].flat[0]) and np.all(sl[1] == sl[1].flat[0]))
    dtype = img.get_data_dtype().name if uniform else "float32"
    record(name + ".PAR", "Philips PAR/REC", dtype, "parrec", vols)
    geom(name + ".PAR", (RAS_TO_LPS @ img.affine)[:3, :], img.shape)
    vals(name + ".PAR", d)


def parrec_cases():
    # Transverse, angulated, two dynamics, every image scaled differently.
    order = [(s, d) for d in (1, 2) for s in (1, 2, 3)]
    scales = [(0.5 * i, 1.0 + 0.25 * i) for i in range(len(order))]
    write_par("par_tra", "V4.2", order, 5, 4, 16, scales, 1, (-13.265, 4.0, 2.5),
              (2.508, 30.339, -16.032), 3, 2)
    par_case("par_tra")
    # Sagittal, 8-bit, slices stored out of order, one scaling.
    order = [(3, 1), (1, 1), (2, 1), (4, 1)]
    write_par("par_sag", "V4.1", order, 4, 6, 8, [(-10.0, 2.0)] * 4, 2, (0.0, 0.0, 7.5),
              (0.0, -3.0, 12.0), 4, 1, thick=3.0, gap=0.0, spacing=(1.25, 1.25))
    par_case("par_sag")
    # Coronal, version 4, two full dynamics and a third cut short.
    order = [(s, d) for d in (1, 2) for s in (1, 2)] + [(1, 3)]
    write_par("par_cor", "V4", order, 3, 5, 16, [(0.0, 1.0)] * 5, 3, (10.0, -5.0, 0.0),
              (1.0, 2.0, 3.0), 2, 3)
    par_case("par_cor", truncated=True)
    # Version 3: refused.
    with open(os.path.join(OUT, "par_v3.PAR"), "w") as f:
        f.write("# === DATA DESCRIPTION FILE ===\n#\n# Research image export tool     V3\n#\n"
                ".    Max. number of slices/locations    :   1\n#\n"
                "1 1 1 1 0 2 0 16 100 2 2 0 1 1 1 1\n")
    open(os.path.join(OUT, "par_v3.REC"), "wb").write(bytes(8))
    rows.append("error\tpar_v3.PAR\tonly versions 4 to 4.2")


# --- ECAT 7 --------------------------------------------------------------------------------

def main_header(big, file_type, orient, calib, nframes, planes):
    e = ">" if big else "<"
    b = bytearray(512)
    b[0:14] = b"MATRIX72v".ljust(14, b"\0")
    b[14:46] = b"sample.v".ljust(32, b"\0")
    struct.pack_into(e + "HHH", b, 46, 74, 962, file_type)
    struct.pack_into(e + "I", b, 62, 1583316672)
    b[66:74] = b"F-18".ljust(8, b"\0")
    b[78:110] = b"FDG".ljust(32, b"\0")
    struct.pack_into(e + "f", b, 144, calib)
    b[296:328] = b"brain study".ljust(32, b"\0")
    struct.pack_into(e + "H", b, 330, orient)
    struct.pack_into(e + "HH", b, 352, planes, nframes)
    b[466:498] = b"Bq/cc".ljust(32, b"\0")
    return bytes(b)


def sub_header(big, dtype, shape, offset, scale, pixel):
    e = ">" if big else "<"
    b = bytearray(512)
    struct.pack_into(e + "HHHHH", b, 0, dtype, 3, *shape)
    struct.pack_into(e + "fff", b, 10, *offset)
    struct.pack_into(e + "f", b, 26, scale)
    struct.pack_into(e + "fff", b, 34, *pixel)
    return bytes(b)


NP = {1: "u1", 2: "i2", 3: "i4", 5: "f4", 6: "i2", 7: "i4", 4: "f4"}


def write_ecat(name, big=True, dtype=6, file_type=7, orient=3, calib=2.0, ids=(1,),
               scales=(0.5,), shape=(5, 4, 3), offset=(0.0, 0.0, 0.0), pixel=(0.2, 0.25, 0.3)):
    e = ">" if big else "<"
    r = np.random.default_rng(len(name) + dtype)
    nx, ny, nz = shape
    frames = []
    for _ in ids:
        if NP[dtype].startswith("f"):
            a = (r.standard_normal((nz, ny, nx)) * 100).astype(e + "f4")
        else:
            info = np.iinfo(np.dtype(NP[dtype]))
            a = r.integers(info.min, info.max, size=(nz, ny, nx), endpoint=True).astype(e + NP[dtype])
        frames.append(a.tobytes())
    out = bytearray(main_header(big, file_type, orient, calib, len(ids), nz))
    mlist = bytearray(512)
    struct.pack_into(e + "iiii", mlist, 0, 31 - len(ids), 2, 0, len(ids))
    blk = 3
    subs = bytearray()
    for n, (mid, sc, data) in enumerate(zip(ids, scales, frames)):
        nblocks = (len(data) + 511) // 512
        struct.pack_into(e + "iiii", mlist, 16 * (n + 1), mid, blk, blk + nblocks, 1)
        subs += sub_header(big, dtype, shape, offset, sc, pixel)
        subs += data.ljust(nblocks * 512, b"\0")
        blk += 1 + nblocks
    out += mlist + subs
    with open(os.path.join(OUT, name), "wb") as f:
        f.write(out)


def ecat_case(name, dtype):
    img = ecat.load(os.path.join(OUT, name))
    d = img.get_fdata(dtype=np.float64)
    vols = d.shape[3]
    record(name, "ECAT 7", dtype, "ecat", vols)
    geom(name, (RAS_TO_LPS @ img.affine)[:3, :], img.shape)
    vals(name, d)


def ecat_cases():
    write_ecat("ecat_i16.v")
    ecat_case("ecat_i16.v", "int16")
    # Three frames stored out of order, each with its own scale; feet first prone; offsets.
    write_ecat("ecat_frames.v", ids=(16842755, 16842753, 16842754), scales=(0.5, 1.5, 3.0),
               orient=0, offset=(1.5, -2.0, 0.5), calib=1.25)
    ecat_case("ecat_frames.v", "float32")
    write_ecat("ecat_le_f32.v", big=False, dtype=5, orient=8, scales=(2.0,), calib=1.0)
    ecat_case("ecat_le_f32.v", "float32")
    write_ecat("ecat_u8.v", dtype=1, file_type=6, orient=1, scales=(1.0,), calib=1.0,
               shape=(3, 3, 2))
    ecat_case("ecat_u8.v", "uint8")
    write_ecat("ecat_sino.v", file_type=11)
    rows.append("error\tecat_sino.v\tsinogram")
    write_ecat("ecat_vax.v", dtype=4)
    rows.append("error\tecat_vax.v\tVAX floating point")


def main():
    os.makedirs(OUT, exist_ok=True)
    parrec_cases()
    ecat_cases()
    with open(os.path.join(OUT, "expected-parrec-ecat.tsv"), "w") as f:
        f.write("# Written by gen/make_parrec_ecat.py (nibabel %s); do not edit.\n" % nib.__version__)
        f.write("\n".join(rows) + "\n")
    print(f"{len(rows)} rows", file=sys.stderr)


if __name__ == "__main__":
    main()
