#!/usr/bin/env python3
"""Writes the small synthetic AFM fixtures of specular-spm into tests/data/afm.

Nanoscope (current and old header style), Nanonis SXM, JPK TIFF and NT-MDT (a scanned and an
MDA frame). The values are deterministic; tests/data/afm/expected comes from oracle_afm.py.

    python3 gen/gen_afm.py [OUT_DIR]
"""

import struct
import sys
from pathlib import Path

import numpy as np

OUT = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(__file__).resolve().parent.parent / "tests/data/afm"


def ramp(w, h, seed, lo, hi, dtype):
    y, x = np.mgrid[0:h, 0:w]
    v = np.sin(x * 0.7 + seed) * np.cos(y * 0.5 - seed) + (x - y * 2) / (w + h)
    v = (v - v.min()) / (v.max() - v.min())
    return np.round(lo + v * (hi - lo)).astype(dtype)


# --- Nanoscope ---------------------------------------------------------------------------

def nanoscope(path, sections, old=False, head=8192):
    """sections: list of (fields, data array) — fields are the image list's lines."""
    lines = ["\\*File list", "\\Version: 0x09300201" if not old else "\\Version: 0x05310001",
             f"\\Data length: {head}", "\\Date: 01:02:03 PM Mon Jan 02 2023",
             "\\*Equipment list", "\\Microscope: MultiMode",
             "\\*Scanner list", "\\@Sens. Zscan: V 15.06800 nm/V",
             "\\@Sens. ZsensSens: V 6131.568 nm/V" if not old else "\\@Sens. Phase: V 18.00000 º/V",
             "\\*Ciao scan list", "\\Operating mode: Tapping" if not old else "\\Operating mode: Contact"]
    if old:
        lines.append("\\Scan size: 15000 nm")
    offset = head
    blobs = []
    for fields, data in sections:
        lines.append("\\*Ciao image list")
        lines.append(f"\\Data offset: {offset}")
        lines.append(f"\\Data length: {data.nbytes}")
        lines.append(f"\\Bytes/pixel: {data.itemsize}")
        lines.append(f"\\Samps/line: {data.shape[1]}")
        lines.append(f"\\Number of lines: {data.shape[0]}")
        lines += fields
        blobs.append(data.astype("<" + data.dtype.str[1:]).tobytes())
        offset += data.nbytes
    lines.append("\\*File list end")
    text = "\r\n".join(lines).encode("latin-1") + b"\r\n\x1a"
    assert len(text) <= head
    path.write_bytes(text + b"\0" * (head - len(text)) + b"".join(blobs))


def gen_nanoscope():
    w, h = 16, 12
    z = ramp(w, h, 0.3, -30000, 30000, np.int16)
    zr = ramp(w, h, 0.4, -20000, 25000, np.int16)
    pf = ramp(8, 8, 1.1, -2_000_000_000, 2_000_000_000, np.int32)
    common = ["\\Aspect Ratio: 1:1", "\\Scan Size: 378.906 284.180 nm"]
    nanoscope(OUT / "height.spm", [
        (common + ["\\Line Direction: Trace", '\\@2:Image Data: S [ZSensor] "Height Sensor"',
                   "\\@2:Z scale: V [Sens. ZsensSens] (0.00000000572205 V/LSB) 0.024576 V",
                   "\\@2:Z offset: V [Sens. ZsensSens] (0.00000000572205 V/LSB) 0 V"], z),
        (common + ["\\Line Direction: Retrace", '\\@2:Image Data: S [ZSensor] "Height Sensor"',
                   "\\@2:Z scale: V [Sens. ZsensSens] (0.00000000572205 V/LSB) 0.024576 V",
                   "\\@2:Z offset: V [Sens. ZsensSens] (0.00000000572205 V/LSB) 0 V"], zr),
        (["\\Aspect Ratio: 1:1", "\\Scan Size: 2 2 ~m", "\\Line Direction: Trace",
          '\\@2:Image Data: S [Height] "Peak Force Error"',
          "\\@2:Z scale: V [Sens. Zscan] (0.000000004 V/LSB) 17.17987 V",
          "\\@2:Z offset: V [Sens. Zscan] (0.000000004 V/LSB) 0 V"], pf),
    ])
    nanoscope(OUT / "old.001", [
        (["\\Aspect ratio: 1:1", "\\Scan size: 15 15 ~m", "\\Line direction: Trace",
          '\\@2:Image Data: S [Height] "Height"',
          "\\@2:Z scale: V [Sens. Zscan] (0.006713765 V/LSB) 439.9866 V",
          "\\@2:Z offset: V [Sens. Zscan] (0.006713765 V/LSB)       0 V"],
         ramp(w, w, 2.0, -32000, 32000, np.int16)),
        (["\\Aspect ratio: 1:1", "\\Line direction: Retrace",
          '\\@2:Image Data: S [Phase] "Phase"',
          "\\@2:Z scale: V [Sens. Phase] (0.006713765 V/LSB) 20.00000 V",
          "\\@2:Z offset: V [Sens. Phase] (0.006713765 V/LSB)       0 V"],
         ramp(w, w, 2.5, -32000, 32000, np.int16)),
    ], old=True)


# --- Nanonis SXM -------------------------------------------------------------------------

def gen_sxm():
    w, h = 12, 10
    chans = [("Z", "m", 1e-9), ("Current", "A", 1e-11)]
    head = [":NANONIS_VERSION:", "2", ":SCANIT_TYPE:", "              FLOAT            MSBFIRST",
            ":REC_DATE:", " 06.06.2019", ":REC_TIME:", "15:12:03", ":ACQ_TIME:", "        59.1",
            ":SCAN_PIXELS:", f"       {w}       {h}", ":SCAN_TIME:", "  1.152E-1  1.152E-1", ":SCAN_RANGE:", "           6.000000E-9           5.000000E-9",
            ":SCAN_OFFSET:", "             -3.163728E-9         -1.225518E-7",
            ":SCAN_ANGLE:", "            0.000E+0", ":SCAN_DIR:", "up", ":BIAS:", "            -2.000E-2",
            ":COMMENT:", "synthetic fixture", ":DATA_INFO:",
            "\tChannel\tName\tUnit\tDirection\tCalibration\tOffset"]
    for i, (n, u, _) in enumerate(chans):
        head.append(f"\t{i}\t{n}\t{u}\tboth\t1.000E+0\t0.000E+0")
    head += ["", ":SCANIT_END:", "", "", ""]
    data = b""
    for i, (_, _, s) in enumerate(chans):
        for d in range(2):
            v = ramp(w, h, i + d * 0.5, -1, 1, np.float64) * s
            v = v + np.arange(w * h).reshape(h, w) * s * 1e-3
            data += v.astype(">f4").tobytes()
    (OUT / "scan.sxm").write_bytes("\n".join(head).encode("latin-1") + b"\x1a\x04" + data)


# --- JPK ---------------------------------------------------------------------------------

class Tiff:
    """A big-endian classic TIFF writer: IFDs of (tag, type, values)."""

    TYPES = {1: "B", 2: "s", 3: "H", 4: "I", 8: "h", 9: "i", 12: "d"}

    def __init__(self):
        self.ifds = []

    def write(self, path):
        out = bytearray(b"MM\0*\0\0\0\0")
        # Layout: header, then per IFD its strips, the IFD and its out-of-line values.
        prev_next = 4
        for tags, strips in self.ifds:
            # Place strips first so their offsets are known.
            strip_offsets = []
            for s in strips:
                strip_offsets.append(len(out))
                out += s
                out += b"\0" * 6  # a gap: strips need not be contiguous
            if len(out) % 2:
                out += b"\0"
            tags = list(tags)
            if strips:
                tags.append((273, 4, strip_offsets))
                tags.append((279, 4, [len(s) for s in strips]))
            tags.sort(key=lambda t: t[0])
            ifd_at = len(out)
            struct.pack_into(">I", out, prev_next, ifd_at)
            body = bytearray(struct.pack(">H", len(tags)))
            extra_at = ifd_at + 2 + 12 * len(tags) + 4
            extra = bytearray()
            for tag, ty, vals in tags:
                if ty == 2:
                    raw = vals.encode() + b"\0"
                    count = len(raw)
                else:
                    if not isinstance(vals, list):
                        vals = [vals]
                    raw = struct.pack(">" + self.TYPES[ty] * len(vals), *vals)
                    count = len(vals)
                if len(raw) <= 4:
                    body += struct.pack(">HHI", tag, ty, count) + raw.ljust(4, b"\0")
                else:
                    body += struct.pack(">HHII", tag, ty, count, extra_at + len(extra))
                    extra += raw
                    if len(extra) % 2:
                        extra += b"\0"
            prev_next = len(out) + len(body)
            body += b"\0\0\0\0"
            out += body + extra
        Path(path).write_bytes(bytes(out))


def calibration_slots(slots, shown):
    tags = [(0x8080, 9, len(slots)), (0x8081, 2, shown)]
    for n, (name, unit, kind, mult, off) in enumerate(slots):
        b = 0x30 * n
        tags += [(0x8090 + b, 2, name), (0x8091 + b, 2, "RelativeCalibration" if n else "AbsoluteCalibration"),
                 (0x80A0 + b, 2, name.capitalize()), (0x80A1 + b, 2, "SignedInteger"),
                 (0x80A3 + b, 2, kind)]
        if unit:
            tags.append((0x80A2 + b, 2, unit))
        if kind == "LinearScaling":
            tags += [(0x80A4 + b, 12, mult), (0x80A5 + b, 12, off)]
    return tags


def image_tags(w, h, bits=32, fmt=2, rows=None):
    return [(256, 3, w), (257, 3, h), (258, 3, bits), (259, 3, 1), (262, 3, 1), (277, 3, 1),
            (278, 3, rows or h), (284, 3, 1), (339, 3, fmt)]


def gen_jpk():
    w, h = 12, 10
    t = Tiff()
    thumb = np.zeros((4, 4), np.uint8)
    t.ifds.append((image_tags(4, 4, 8, 1) + [
        (0x8000, 2, "6.0.41"), (0x8003, 2, "2017-01-16 16:00:32.334 GMT"), (0x8004, 2, "Synthetic"),
        (0x8030, 2, "contact"), (0x8040, 12, 2.5e-6), (0x8041, 12, 1.5e-6), (0x8042, 12, 3.0e-7),
        (0x8043, 12, 2.5e-7), (0x8046, 9, w), (0x8047, 9, h), (0x804B, 2, "bottomUp"),
        (0x8050, 2, "thumbnail")], [thumb.tobytes()]))
    height = ramp(w, h, 0.1, -2_000_000_000, 2_000_000_000, np.int32)
    rows = height.astype(">i4").tobytes()
    # Height: one strip, read in place.
    tags = image_tags(w, h) + [(0x8050, 2, "height"), (0x8051, 8, 0), (0x8052, 2, "Height")]
    tags += calibration_slots([("raw", "", "NullScaling", 0, 0),
                               ("volts", "V", "LinearScaling", 2.3283064370807974e-08, 50.0),
                               ("nominal", "m", "LinearScaling", -1.5133991841025183e-15, 3.25e-06),
                               ("calibrated", "m", "LinearScaling", -1.1577216212539286e-15, 2.486e-06)],
                              "calibrated")
    t.ifds.append((tags, [rows]))
    err = ramp(w, h, 0.9, -1_000_000_000, 1_000_000_000, np.int32).astype(">i4").tobytes()
    tags = image_tags(w, h, rows=4) + [(0x8050, 2, "vDeflection"), (0x8051, 8, 1), (0x8052, 2, "Vertical Deflection")]
    tags += calibration_slots([("raw", "", "NullScaling", 0, 0),
                               ("volts", "V", "LinearScaling", 5.2e-09, -0.1)], "volts")
    # Three strips with gaps between them: gathered.
    t.ifds.append((tags, [err[:192], err[192:384], err[384:]]))
    t.write(OUT / "image.jpk")


# --- NT-MDT ------------------------------------------------------------------------------

def frame(ty, body, var_size, date=(2002, 6, 27, 17, 3, 59)):
    return struct.pack("<IHH6HH", 22 + len(body), ty, 0x0106, *date, var_size) + body


def scanned_frame(w, h, data):
    v = struct.pack("<ffh", 10.0, 672.0458984375, 0) + struct.pack("<ffh", -5.0, 500.0, 0)
    v += struct.pack("<ffh", 3.0, 0.7483410835266113, -1)
    v += struct.pack("<BBHHHfHBBBBBBfffBBiiB", 0, 1, w, h, 0, 672.0, 0, 1, 0, 45, 0, 1, 1, 1.0, 1.0, 0.25, 1, 0,
                     255, 255, 1)
    v = v.ljust(442, b"\0")
    body = v + struct.pack("<4H", 0, w, h, 0) + data.astype("<i2").tobytes()
    title = "Topography".encode("cp1251")
    body += struct.pack("<I", len(title)) + title + struct.pack("<I", 0)
    return frame(0, body, 442)


def calibration(name, unit, bias, scale, lo, hi, dtype):
    fixed = 84
    nb, ub = name.encode(), unit.encode()
    return struct.pack("<5IQdQddQQiI", fixed + len(nb) + len(ub), fixed, len(nb), 0, len(ub), 0, 0.0, 0,
                       bias, scale, lo, hi, dtype, 0) + nb + ub


def mda_frame(w, h, data):
    title = b"Height MDA"
    head = struct.pack("<II", 76, 0) + b"\0" * 32 + b"\0" * 4
    struct_ = calibration("X", "um", -1.0, 0.25, 0, w - 1, 0) + calibration("Y", "um", 2.0, 0.5, 0, h - 1, 0)
    struct_ += calibration("Height", "nm", 1.5, 1.0, 0, 0, -5892)
    struct_ = struct.pack("<IQII", 0, w * h, 4, 2) + struct.pack("<I", 1) + struct_
    data_offset = 76 + len(title) + len(struct_)
    raw = data.astype("<f4").tobytes()
    head += struct.pack("<8I", len(title), 0, 0, 0, 0, 0, data_offset, len(raw))
    body = head + title + struct_ + raw
    assert len(head) == 76
    return frame(106, body, 0)


def gen_mdt():
    w, h = 12, 10
    frames = [scanned_frame(w, h, ramp(w, h, 0.2, -30000, 30000, np.int16)),
              frame(3, b"some text frame", 0),
              mda_frame(8, 6, ramp(8, 6, 0.6, 0, 1000, np.float64).astype(np.float32) / 7)]
    body = b"".join(frames)
    head = bytes([0x01, 0xB0, 0x93, 0xFF]) + struct.pack("<I", 33 + len(body)) + b"\0" * 4
    head += struct.pack("<H", len(frames) - 1)
    head = head.ljust(33, b"\0")
    (OUT / "frames.mdt").write_bytes(head + body)


if __name__ == "__main__":
    OUT.mkdir(parents=True, exist_ok=True)
    gen_nanoscope()
    gen_sxm()
    gen_jpk()
    gen_mdt()
    for p in sorted(OUT.iterdir()):
        if p.is_file():
            print(p.name, p.stat().st_size)
