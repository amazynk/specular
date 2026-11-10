#!/usr/bin/env python3
"""Writes the small diffraction-detector fixtures for the specular-xtal tests.

    make_samples.py OUTDIR

Each file is written here from the format descriptions (not with the crate under test);
the expected values are then read back by fabio and the geometry checked with pyFAI
(gen/oracle.py), not taken from this script.

Pixel values follow v(x, y, k) = (37x + 101y + 211k) % 1009 + 3 (frame k), with features
each format needs: Pilatus gaps (-1) and bad pixels (-2), large jumps for every
byte-offset width, Bruker overflows and underflows, R-AXIS high-bit pixels.
"""

import bz2
import gzip
import os
import struct
import sys
import zlib

import numpy as np


def ramp(w, h, k=0):
    y, x = np.mgrid[0:h, 0:w]
    return ((37 * x + 101 * y + 211 * k) % 1009 + 3).astype(np.int64)


# --- CBF ---------------------------------------------------------------------------------------


def byte_offset(values):
    out = bytearray()
    prev = 0
    for v in values.ravel().tolist():
        d = v - prev
        prev = v
        if -127 <= d <= 127:
            out += struct.pack("<b", d)
        elif -32767 <= d <= 32767:
            out += b"\x80" + struct.pack("<h", d)
        elif -(2**31 - 1) <= d <= 2**31 - 1:
            out += b"\x80" + struct.pack("<h", -32768) + struct.pack("<i", d)
        else:
            out += b"\x80" + struct.pack("<h", -32768) + struct.pack("<i", -(2**31)) + struct.pack("<q", d)
    return bytes(out)


def cbf(path, data, header_lines, conversions="x-CBF_BYTE_OFFSET", etype="signed 32-bit integer",
        raw=None, convention="PILATUS_1.2", start=b"###CBF: VERSION 1.5, CBFlib v0.7.8\r\n"):
    h, w = data.shape
    binary = byte_offset(data) if raw is None else raw
    t = start.decode() + "\r\ndata_fixture\r\n\r\n"
    if convention:
        t += f'_array_data.header_convention "{convention}"\r\n'
    if header_lines:
        t += "_array_data.header_contents\r\n;\r\n" + "".join(l + "\r\n" for l in header_lines) + ";\r\n"
    t += "\r\n_array_data.data\r\n;\r\n--CIF-BINARY-FORMAT-SECTION--\r\n"
    t += f'Content-Type: application/octet-stream;\r\n     conversions="{conversions}"\r\n'
    t += "Content-Transfer-Encoding: BINARY\r\n"
    t += f"X-Binary-Size: {len(binary)}\r\nX-Binary-ID: 1\r\n"
    t += f'X-Binary-Element-Type: "{etype}"\r\nX-Binary-Element-Byte-Order: LITTLE_ENDIAN\r\n'
    t += f"X-Binary-Number-of-Elements: {w * h}\r\n"
    t += f"X-Binary-Size-Fastest-Dimension: {w}\r\nX-Binary-Size-Second-Dimension: {h}\r\n"
    t += "X-Binary-Size-Padding: 4095\r\n\r\n"
    body = t.encode() + b"\x0c\x1a\x04\xd5" + binary + b"\0" * 4095
    body += b"\r\n--CIF-BINARY-FORMAT-SECTION----\r\n;\r\n\r\n"
    open(path, "wb").write(body)


def pilatus(out):
    w, h = 97, 61
    a = ramp(w, h) * 37
    a[:, 40:43] = -1  # module gap
    a[25, :] = -1
    a[10, 10] = -2  # bad pixel
    a[5, 5] = 1_048_500  # beyond 16-bit deltas
    a[5, 6] = 3_000_000_000 - 2**31  # still i32, big jump down
    a[6, 0] = 2_000_000_000  # 32-bit delta
    lines = [
        "# Detector: PILATUS3 6M, S/N 60-0123, Fixture",
        "# 2024-03-04T05:06:07.890",
        "# Pixel_size 172e-6 m x 172e-6 m",
        "# Silicon sensor, thickness 0.001000 m",
        "# Exposure_time 0.0990000 s",
        "# Exposure_period 0.1000000 s",
        "# Tau = 1.991e-07 s",
        "# Count_cutoff 1048500 counts",
        "# Threshold_setting: 6342 eV",
        "# N_excluded_pixels = 1",
        "# Excluded_pixels: badpix_mask.tif",
        "# Wavelength 0.97950 A",
        "# Detector_distance 0.25000 m",
        "# Beam_xy (48.30, 29.70) pixels",
        "# Start_angle 12.0000 deg.",
        "# Angle_increment 0.1000 deg.",
        "# Detector_2theta 0.0000 deg.",
        "# Phi 12.0000 deg.",
        "# Omega 0.0000 deg.",
        "# Chi 0.0000 deg.",
        "# Kappa 0.0000 deg.",
        "# Oscillation_axis X, CW",
        "# Image_path: /data/visitor/a-very-long-directory-name-that-wraps-acr\\",
        "oss-two-lines/",
    ]
    cbf(os.path.join(out, "pilatus.cbf"), a, lines)
    # Uncompressed 16-bit, no `###CBF` line, no detector header.
    b = ramp(33, 21).astype(np.uint16) * 50
    cbf(os.path.join(out, "none_u16.cbf"), b, [], conversions="x-CBF_NONE",
        etype="unsigned 16-bit integer", raw=b.astype("<u2").tobytes(), convention=None, start=b"")
    # A gzipped miniCBF (`.cbf.gz`).
    c = ramp(31, 17) - 100
    cbf(os.path.join(out, "tmp.cbf"), c, ["# Detector: PILATUS 100K, S/N 1-0001", "# Wavelength 1.5418 A",
                                          "# Detector_distance 0.100 m", "# Beam_xy (15.5, 8.5) pixels",
                                          "# Pixel_size 172e-6 m x 172e-6 m"])
    raw = open(os.path.join(out, "tmp.cbf"), "rb").read()
    os.remove(os.path.join(out, "tmp.cbf"))
    with open(os.path.join(out, "small.cbf.gz"), "wb") as f:
        f.write(gzip.compress(raw, mtime=0))


# --- EDF ---------------------------------------------------------------------------------------


def edf_header(items, pad=512):
    t = "{\n" + "".join(f"{k} = {v} ;\n" for k, v in items)
    n = len(t) + 2
    t += " " * ((-n) % pad) + "}\n"
    return t.encode()


def edf(out):
    w, h = 40, 30
    frames = []
    for k in range(3):
        a = (ramp(w, h, k) * 0.25).astype(">f4")
        a[3, 4] = -1.0  # Dummy
        frames.append(a)
    body = b""
    for k, a in enumerate(frames):
        items = [("HeaderID", f"EH:{k + 1:06d}:000000:000000"), ("Image", k + 1),
                 ("ByteOrder", "HighByteFirst"), ("DataType", "FloatValue"),
                 ("Dim_1", w), ("Dim_2", h), ("Size", a.nbytes)]
        if k == 0:
            items += [("Title", "SAXS fixture"), ("Date", "Mon Mar  4 05:06:07 2024"),
                      ("Dummy", "-1"), ("DDummy", "0.1"), ("Offset_1", 0), ("Offset_2", 0),
                      ("Center_1", 21.5), ("Center_2", 12.25), ("PSize_1", "0.000172"),
                      ("PSize_2", "0.000172"), ("SampleDistance", "1.5"), ("WaveLength", "1.0e-10")]
        else:
            items += [("Dummy", "-1"), ("DDummy", "0.1")]
        body += edf_header(items) + a.tobytes()
    open(os.path.join(out, "saxs.edf"), "wb").write(body)
    # The same file gzipped whole.
    with open(os.path.join(out, "saxs.edf.gz"), "wb") as f:
        f.write(gzip.compress(body, mtime=0))

    # Compressed blocks: gzip (two frames) and zlib, 16-bit, Oxford keys.
    w, h = 64, 48
    body = b""
    for k in range(2):
        a = ramp(w, h, k).astype("<u2")
        blk = gzip.compress(a.tobytes(), mtime=0)
        body += edf_header([("Image", k + 1), ("ByteOrder", "LowByteFirst"), ("DataType", "UnsignedShort"),
                            ("Dim_1", w), ("Dim_2", h), ("Size", len(blk)), ("Compression", "GzipCompression"),
                            ("Distance in mm", "60.0"), ("Beam center x", "30.5"), ("Beam center y", "20.5"),
                            ("Real pixel size x (mm)", "0.06"), ("Real pixel size y (mm)", "0.06"),
                            ("Wavelength alpha1", "0.70930")]) + blk
    open(os.path.join(out, "gzip.edf"), "wb").write(body)
    a = (ramp(w, h) * 1000).astype("<i4")
    blk = zlib.compress(a.tobytes())
    body = edf_header([("Image", 1), ("ByteOrder", "LowByteFirst"), ("DataType", "SignedInteger"),
                       ("Dim_1", w), ("Dim_2", h), ("Size", len(blk)), ("Compression", "ZCompression")]) + blk
    open(os.path.join(out, "zlib.edf"), "wb").write(body)
    a = ramp(w, h).astype("<u4")
    blk = bz2.compress(a.tobytes())
    body = edf_header([("Image", 1), ("ByteOrder", "LowByteFirst"), ("DataType", "UnsignedInteger"),
                       ("Dim_1", w), ("Dim_2", h), ("Size", len(blk)), ("Compression", "BzipCompression")]) + blk
    open(os.path.join(out, "bzip2.edf"), "wb").write(body)


# --- SMV ---------------------------------------------------------------------------------------


def smv(path, items, data):
    body = "".join(f"{k}={v};\n" for k, v in items) + "}\n\f\n"
    n = 512
    while len(body) + 21 > n:
        n += 512
    b = (f"{{\nHEADER_BYTES={n:5d};\n" + body).encode().ljust(n, b" ")
    open(path, "wb").write(b + data)


def smvs(out):
    w, h = 64, 48
    a = (ramp(w, h) * 60).astype(">u2")
    smv(os.path.join(out, "adsc.img"),
        [("DIM", 2), ("BYTE_ORDER", "big_endian"), ("TYPE", "unsigned_short"), ("SIZE1", w), ("SIZE2", h),
         ("PIXEL_SIZE", "0.102588"), ("BIN", "2x2"), ("ADC", "slow"), ("DETECTOR_SN", 901),
         ("DATE", "Mon Mar  4 05:06:07 2024"), ("TIME", "29.945155"), ("DISTANCE", "200.000000"),
         ("TWOTHETA", "0.00"), ("PHI", "85.000000"), ("OSC_START", "85.000000"), ("OSC_RANGE", "1.000000"),
         ("WAVELENGTH", "0.979381"), ("BEAM_CENTER_X", "2.500000"), ("BEAM_CENTER_Y", "3.250000"),
         ("CCD_IMAGE_SATURATION", "65535")],
        a.tobytes())
    a = (ramp(w, h) * 1000 - 5000).astype("<i4")
    smv(os.path.join(out, "dtrek.img"),
        [("BYTE_ORDER", "little_endian"), ("COMPRESSION", "None"), ("DETECTOR_NAMES", "PXD_"),
         ("Data_type", "long int"), ("DIM", 2), ("DTREK_DATE_TIME", "04-Mar-2024 05:06:07"),
         ("PXD_DETECTOR_DESCRIPTION", "HyPix-3000"),
         ("PXD_GONIO_NAMES", "RotAboutBeam 2Theta RotY XShift YShift Distance"),
         ("PXD_GONIO_VALUES", "0.0000 0.0000 0.0000 0.0000 0.0000 120.5000"),
         ("PXD_SPATIAL_BEAM_POSITION", "31.7 22.5"), ("PXD_SPATIAL_DISTORTION_INFO", "31.7 22.5 0.1 0.1"),
         ("ROTATION", "10.000 10.500 0.500 5.4e+003 0.000 0.000 0.000 100.000 0.000 0.000"),
         ("SATURATED_VALUE", "2147483648.0000"), ("SIZE1", w), ("SIZE2", h), ("WAVELENGTH", "1.0 1.541870")],
        a.tobytes())


# --- Bruker ------------------------------------------------------------------------------------


def bruker_header(recs, blocks=5):
    b = b""
    for k, v in recs:
        b += f"{k:<7}:{v}".ljust(80)[:80].encode()
    b = b.ljust(blocks * 512, b".")
    return b


def pad16(b):
    return b + b"\0" * ((-len(b)) % 16)


def bruker(out):
    w = h = 64
    a = ramp(w, h) * 3
    a[2, 63] = 145942
    a[40, 7] = 70000
    recs = [("FORMAT", "86"), ("VERSION", "9"), ("HDRBLKS", "5"), ("TYPE", "Scan frame"), ("SITE", "Fixture"),
            ("MODEL", "PLATFORM"), ("SAMPLE", "Cr8F8"), ("TITLE", "fixture frame"), ("NOVERFL", "2"),
            ("CREATED", "12/12/03   11:45:47"), ("CUMULAT", "0.992"), ("RANGE", "0.2"), ("START", "84.8"),
            ("INCREME", "-0.2"), ("ANGLES", "0.0 -90.0 84.8 54.92"), ("NPIXELB", "2"), ("NROWS", h),
            ("NCOLS", w), ("WORDORD", "0"), ("LONGORD", "0"), ("CENTER", "260.61 240.85"),
            ("DISTANC", "8.9"), ("COMPRES", "NONE"), ("LINEAR", "1.0 0.0"),
            ("WAVELEN", "0.5166 0.0 0.0"), ("AXIS", "3"), ("DETTYPE", "CCD-PXL-L6500F 32.000000 1.600000")]
    data = np.where(a > 65535, 65535, a).astype("<u2").tobytes()
    ovf = b""
    for y, x in ((2, 63), (40, 7)):
        ovf += f"{a[y, x]:9d}{y * w + x:7d}".encode()
    open(os.path.join(out, "frame86.0001"), "wb").write(bruker_header(recs) + data + ovf)

    # Version 100: 1-byte pixels, underflow, 16-bit and 32-bit overflow tables, baseline 32.
    base = 32
    a = ramp(w, h) % 200 - 40  # some below the baseline
    a[0, 0] = 300
    a[1, 1] = 100_000
    a[63, 63] = -1000
    under, o1, o2, flat = [], [], [], []
    for t in a.ravel().tolist():
        s = t - base  # stored above the baseline
        if s <= 0:
            flat.append(0)
            under.append(t)
        elif s < 255:
            flat.append(s)
        elif s < 65535:
            flat.append(255)
            o1.append(s)
        else:
            flat.append(255)
            o1.append(65535)
            o2.append(s)
    recs = [("FORMAT", "100"), ("VERSION", "18"), ("HDRBLKS", "5"), ("TYPE", "Phi Scan Image"),
            ("SITE", "Fixture"), ("MODEL", "D85 with FIXED CHI"), ("NOVERFL", f"{len(under)} {len(o1)} {len(o2)}"),
            ("CREATED", "09/12/14 16:25:21"), ("CUMULAT", "15.0"), ("RANGE", "10.0"), ("START", "279.99"),
            ("INCREME", "-10.0"), ("ANGLES", "0.0 0.0 279.99 54.736"), ("NPIXELB", "1 2"), ("NROWS", f"{h} 2"),
            ("NCOLS", f"{w} 2"), ("CENTER", "253.0 257.5 253.0 257.5"), ("DISTANC", "5.02 5.02"),
            ("COMPRES", "none"), ("WAVELEN", "0.71073 0.7093 0.71359 0.63229"), ("AXIS", "3"),
            ("DETTYPE", "CCD-LDI-APEX2 83.333 0.0 0 0.254 0.0508 1"), ("NEXP", f"2 599 {base} 0 1")]
    body = bruker_header(recs) + pad16(np.array(flat, "<u1").tobytes())
    body += pad16(np.array(under, "<i2").tobytes()) + pad16(np.array(o1, "<u2").tobytes())
    body += pad16(np.array(o2, "<i4").tobytes())
    open(os.path.join(out, "frame100.sfrm"), "wb").write(body)


# --- MarCCD ------------------------------------------------------------------------------------


def marccd(out):
    w = h = 64
    a = (ramp(w, h) * 7).astype("<u2")
    tiff = bytearray(4096)
    entries = [(256, 4, 1, w), (257, 4, 1, h), (258, 3, 1, 16), (259, 3, 1, 1), (262, 3, 1, 1),
               (273, 4, 1, 4096), (277, 3, 1, 1), (278, 4, 1, h), (279, 4, 1, w * h * 2)]
    tiff[0:8] = b"II*\0" + struct.pack("<I", 8)
    ifd = struct.pack("<H", len(entries))
    for tag, typ, n, v in entries:
        val = struct.pack("<HH", v, 0) if typ == 3 else struct.pack("<I", v)
        ifd += struct.pack("<HHI", tag, typ, n) + val
    ifd += struct.pack("<I", 0)
    tiff[8:8 + len(ifd)] = ifd

    def put(at, fmt, v):
        tiff[at:at + struct.calcsize(fmt)] = struct.pack(fmt, v)

    tiff[1028:1031] = b"MMX"
    for at, v in [(1052, 1234), (1056, 1234), (1104, w), (1108, h), (1112, 2), (1124, 0), (1128, 65535),
                  (1664, 100_000), (1668, 30_500), (1672, 33_250), (1676, 1000), (1680, 1000),
                  (1692, 0), (1696, 0), (1700, 0), (1704, 0), (1708, 15_000), (1720, 100_000),
                  (1756, 4), (1760, 1000), (1792, 0), (1796, 79_092), (1800, 79_092), (1932, 103_318)]:
        put(at, "<i", v)
    tiff[2048:2048 + 13] = b"fixture title"
    tiff[2368:2368 + 15] = b"030405062024.07"
    open(os.path.join(out, "frame.mccd"), "wb").write(bytes(tiff) + a.tobytes())


# --- R-AXIS ------------------------------------------------------------------------------------


def raxis(out):
    w, h = 64, 32
    ratio = 32.0
    a = ramp(w, h) * 40
    stored = np.where(a > 32767, (a // ratio).astype(np.int64) | 0x8000, a).astype(">u2")
    hdr = bytearray(2048)

    def put(at, fmt, v):
        hdr[at:at + struct.calcsize(fmt)] = struct.pack(fmt, v)

    hdr[0:10] = b"R-AXIS4   "
    hdr[10:20] = b"IV++      "
    hdr[20:40] = b"fixture crystal".ljust(20)
    hdr[172:184] = b"2024-03-04  "
    hdr[288:292] = b"CU  "
    put(292, ">f", 1.5418)
    put(344, ">f", 100.0)
    put(448, ">i", 0)
    put(524, ">f", 0.0)
    put(528, ">f", 1.0)
    put(536, ">f", 5.0)
    put(540, ">f", 31.5)
    put(544, ">f", 15.5)
    put(768, ">i", w)
    put(772, ">i", h)
    put(776, ">f", 0.1)
    put(780, ">f", 0.1)
    put(784, ">i", w * 2)
    put(788, ">i", h)
    put(800, ">f", ratio)
    hdr[812:822] = b"IBM       "
    hdr[822:832] = b"R-AXIS    "
    # Header padded to one record (w * 2 bytes) multiple, as the readers write it.
    open(os.path.join(out, "plate.img"), "wb").write(bytes(hdr) + stored.tobytes())


def edf_more(out):
    """Volumes (Dim_3), every data type, byte-offset blocks and binning."""
    w, h, d = 24, 16, 3
    vol = np.stack([ramp(w, h, k) - 500 for k in range(d)]).astype(">i2")
    body = edf_header([("HeaderID", "EH:000001:000000:000000"), ("Image", 1), ("ByteOrder", "HighByteFirst"),
                       ("DataType", "SignedShort"), ("Dim_1", w), ("Dim_2", h), ("Dim_3", d),
                       ("Size", vol.nbytes), ("BSize_1", 2), ("BSize_2", 2), ("Title", "volume")])
    open(os.path.join(out, "volume.edf"), "wb").write(body + vol.tobytes())
    vol = np.stack([ramp(w, h, k) for k in range(d)]).astype("<u2")
    blk = gzip.compress(vol.tobytes(), mtime=0)
    body = edf_header([("Image", 1), ("ByteOrder", "LowByteFirst"), ("DataType", "UnsignedShort"),
                       ("Dim_1", w), ("Dim_2", h), ("Dim_3", d), ("Size", len(blk)),
                       ("Compression", "GzipCompression")])
    open(os.path.join(out, "volume_gz.edf"), "wb").write(body + blk)
    # One frame per data type, alternating byte orders.
    types = [("SignedByte", "i1"), ("UnsignedByte", "u1"), ("SignedShort", "i2"), ("Unsigned16", "u2"),
             ("SignedInteger", "i4"), ("UnsignedLong", "u4"), ("Signed64", "i8"), ("Unsigned64", "u8"),
             ("FloatIEEE32", "f4"), ("DoubleValue", "f8")]
    body = b""
    for k, (name, t) in enumerate(types):
        big = k % 2 == 1
        a = ramp(w, h, k)
        if t[0] == "i":
            a = a - 120 if t == "i1" else a * (k + 1) - 700
        if t == "u1":
            a = a % 256
        if t == "i1":
            a = np.clip(a, -128, 127)
        if t[0] == "f":
            a = a * 0.125 - 33.0
        a = a.astype((">" if big else "<") + t)
        body += edf_header([("Image", k + 1), ("ByteOrder", "HighByteFirst" if big else "LowByteFirst"),
                            ("DataType", name), ("Dim_1", w), ("Dim_2", h), ("Size", a.nbytes)]) + a.tobytes()
    open(os.path.join(out, "types.edf"), "wb").write(body)
    a = ramp(w, h) * 50
    a[2, 3] = 70000
    a[2, 4] = -70000
    blk = byte_offset(a.astype(np.int64))
    body = edf_header([("Image", 1), ("ByteOrder", "LowByteFirst"), ("DataType", "SignedInteger"),
                       ("Dim_1", w), ("Dim_2", h), ("Size", len(blk)), ("Compression", "BYTE_OFFSET")])
    open(os.path.join(out, "offset.edf"), "wb").write(body + blk)


def compress_z(path, bits):
    """`path` + `.Z` by the system's compress(1) at `bits` bits (the oracle reads it back)."""
    import subprocess

    data = subprocess.run(["compress", "-c", f"-b{bits}", path], check=True, capture_output=True).stdout
    open(path + ".Z", "wb").write(data)


def smv_more(out):
    """32-bit ADSC, and whole files compressed by compress(1) (one with table clears)."""
    w, h = 40, 30
    a = (ramp(w, h) * 70000).astype("<u4")
    smv(os.path.join(out, "adsc32.img"),
        [("DIM", 2), ("BYTE_ORDER", "little_endian"), ("TYPE", "unsigned_long"),
         ("Data_type", "unsigned long int"), ("SIZE1", w), ("SIZE2", h),
         ("PIXEL_SIZE", "0.172"), ("BIN", "none"), ("DISTANCE", "150.5"), ("WAVELENGTH", "1.0332"),
         ("BEAM_CENTER_X", "2.58"), ("BEAM_CENTER_Y", "3.44"), ("TWOTHETA", "0")],
        a.tobytes())
    compress_z(os.path.join(out, "adsc32.img"), 12)
    # Noise past compress's 10000-byte ratio check, so the 10-bit table is cleared
    # (ncompress 5.0 writes broken 9-bit streams).
    rng = np.random.default_rng(7)
    w, h = 128, 96
    a = rng.integers(0, 65535, size=(h, w)).astype(">u2")
    path = os.path.join(out, "noise.img")
    smv(path, [("DIM", 2), ("BYTE_ORDER", "big_endian"), ("TYPE", "unsigned_short"), ("SIZE1", w),
               ("SIZE2", h), ("PIXEL_SIZE", "0.1"), ("DISTANCE", "100"), ("WAVELENGTH", "1.0"),
               ("BEAM_CENTER_X", "1.5"), ("BEAM_CENTER_Y", "2.0")], a.tobytes())
    compress_z(path, 10)
    os.remove(path)


def main():
    out = sys.argv[1]
    os.makedirs(out, exist_ok=True)
    pilatus(out)
    edf(out)
    edf_more(out)
    smvs(out)
    smv_more(out)
    bruker(out)
    marccd(out)
    raxis(out)


if __name__ == "__main__":
    main()
