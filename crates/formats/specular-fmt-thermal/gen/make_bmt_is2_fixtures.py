"""Write the small Testo .bmt and Fluke .is2 test fixtures and their expected readings.

Usage: python3 -I gen/make_bmt_is2_fixtures.py tests/data

Standard library only. The temperatures in bmt_is2_expected.tsv are computed here, apart
from the Rust reader:
  * Testo: count * 1102 / 65535 - 101 (degrees Celsius), per Testo's published sample reader;
  * Fluke: the object signal (count - (1 - e) * counts(background)) / (e * tau), inverted
    through the quadratic curve that covers it, held to the table's range.
Every pixel is listed. Fixtures that must be refused are written too.
"""

import io
import json
import math
import struct
import sys
import zipfile
from pathlib import Path

OUT = Path(sys.argv[1])
rows = ["# kind\tfile\tfield\tvalue..."]


def tiny_jpeg(w, h):
    """SOI, a baseline frame header and EOI: enough to state a size (not decodable)."""
    sof = b"\xff\xc0" + struct.pack(">HBHHB", 11, 8, h, w, 1) + b"\x01\x11\x00"
    return b"\xff\xd8" + sof + b"\xff\xd9"


# ---------------------------------------------------------------- Testo .bmt
def bmt(w, h, counts, preview=True):
    items = []  # (group path, name, type, bytes)

    def f32(v):
        return struct.pack("<f", v)

    def string(s, size):
        b = struct.pack("<I", len(s)) + s.encode()
        return b + b"\0" * (size - len(b))

    def kelvin(c):
        return f32(c + 273.15)

    items += [
        ("ActiveMeasRange", "Bottom", "Temperature&lt;MarkedFloat&gt;", kelvin(-30)),
        ("ActiveMeasRange", "Top", "Temperature&lt;MarkedFloat&gt;", kelvin(100)),
        ("DeviceInfo", "DeviceName", "string", string("testo 872", 13)),
        ("DeviceInfo", "FirmwareVersion", "Version", struct.pack("<3I", 1, 9, 0)),
        ("Environment", "EmissivityValue", "float", f32(0.9)),
        ("Environment", "Humidity", "Relation&lt;MarkedFloat&gt;", f32(42.5)),
        ("Environment", "ReflectedTemperature", "Temperature&lt;MarkedFloat&gt;", kelvin(21)),
        ("Environment", "Temperature", "Temperature&lt;MarkedFloat&gt;", kelvin(19.5)),
        ("FileInfo", "DateTime", "uint64", struct.pack("<Q", 1515246692)),
    ]
    mat = struct.pack("<6I", 2, h, w, 2, 1, 2) + struct.pack(f"<{w * h}H", *counts)
    vis = tiny_jpeg(16, 12)
    items += [
        ("Images", "Ir", "cvmat", mat),
        ("Images", "Vis", "vecuint8", struct.pack("<I", len(vis)) + vis),
    ]
    colours = bytes([0, 0, 0, 128, 0, 128, 255, 128, 0, 255, 255, 255])
    items += [
        ("Palette", "ActivePalette", "string", string("Ironbow", 11)),
        ("Palette", "Colors", "vecuint8", struct.pack("<I", len(colours)) + colours),
        (None, "Serial", "uint64", struct.pack("<Q", 61823573)),
    ]
    xml = ['\n<group name="BmtMetaData">']
    data = b""
    open_group = None
    for g, name, ty, b in items:
        if g != open_group:
            if open_group is not None:
                xml.append("    </group>")
            if g is not None:
                xml.append(f'    <group name="{g}">')
            open_group = g
        xml.append(f'        <item name="{name}" type="{ty}" size="{len(b)}" />')
        data += b
    if open_group is not None:
        xml.append("    </group>")
    xml.append("</group>")
    xml = "\n".join(xml).encode()
    tofo = (
        f'<ToFo version="1.0.1">\n    <xml size="{len(xml)}" />\n'
        f'    <data size="{len(data)}" endianness="little" />\n</ToFo>'
    ).encode()
    head = tiny_jpeg(w, h) if preview else b""
    return head + tofo + xml + b"\n" + data


w, h = 8, 6
counts = [6900 + 37 * i for i in range(w * h)]
counts[5] = 0
counts[6] = 65535
(OUT / "bmt_872.bmt").write_bytes(bmt(w, h, counts))
(OUT / "bmt_bare.bmt").write_bytes(bmt(w, h, counts, preview=False))
(OUT / "bmt_old.bmt").write_bytes(b"BM" + struct.pack("<I", 4000) + b"\0" * 60)
for name in ("bmt_872.bmt", "bmt_bare.bmt"):
    rows.append(f"bmt\t{name}\tsize\t{w}\t{h}")
    rows.append(f"bmt\t{name}\tcond\t0.9\t21\t19.5\t0.425")
    for i, c in enumerate(counts):
        rows.append(f"bmt\t{name}\tpx\t{i}\t{c}\t{c * 1102 / 65535 - 101:.6f}")


# ---------------------------------------------------------------- Fluke .is2
def varint(v):
    out = b""
    while True:
        b = v & 0x7F
        v >>= 7
        if v:
            out += bytes([b | 0x80])
        else:
            return out + bytes([b])


def fint(n, v):
    return varint(n << 3) + varint(v)


def ff32(n, v):
    return varint(n << 3 | 5) + struct.pack("<f", v)


def fbytes(n, b):
    return varint(n << 3 | 2) + varint(len(b)) + b


def fstr(n, s):
    return fbytes(n, s.encode())


CURVES = [(-20.0, 0.0, 3000.0, 52.9, 0.2835), (0.0, 40.0, 3000.0, 52.5, 0.3259),
          (40.0, 120.0, 2841.3, 55.38, 0.315)]
RANGE = (-10.0, 80.0)


def f32r(v):
    return struct.unpack("<f", struct.pack("<f", v))[0]


CURVES = [tuple(f32r(x) for x in c) for c in CURVES]


def cal_table(key):
    t = fint(1, key[0]) + fint(2, key[1]) + ff32(3, RANGE[0]) + ff32(4, RANGE[1])
    for t0, t1, c, b, a in CURVES:
        t += fbytes(9, ff32(1, t0) + ff32(2, t1) + ff32(3, c) + ff32(4, b) + ff32(5, a))
    return t


def counts_at(t):
    for t0, t1, c, b, a in CURVES:
        if t0 <= t < t1:
            return a * t * t + b * t + c
    t0, t1, c, b, a = CURVES[0] if t < CURVES[0][0] else CURVES[-1]
    return a * t * t + b * t + c


def temp_of(s):
    pick = None
    for i, (t0, t1, c, b, a) in enumerate(CURVES):
        if a * t0 * t0 + b * t0 + c <= s < a * t1 * t1 + b * t1 + c:
            pick = i
    if pick is None:
        t0, t1, c, b, a = CURVES[0]
        pick = 0 if s < a * t0 * t0 + b * t0 + c else len(CURVES) - 1
    t0, t1, c, b, a = CURVES[pick]
    return (-b + math.sqrt(b * b - 4 * a * (c - s))) / (2 * a)


def fluke_celsius(count, e, bg, tau):
    s = (count - (1 - e) * counts_at(bg)) / (e * tau)
    return min(max(temp_of(s), RANGE[0]), RANGE[1])


EPS, BG, TAU = f32r(0.95), 22.0, 1.0


def is2(w, h, counts, with_json, tables=((0, 0),), image_key=(0, 0), ir=True):
    z = io.BytesIO()
    with zipfile.ZipFile(z, "w", zipfile.ZIP_DEFLATED) as zf:
        zf.writestr("Thumbnails/0050003C.jpg", tiny_jpeg(80, 60))
        ci = fint(1, 5) + fint(4, 1280) + fint(5, 960) + fstr(22, "Fluke Thermography")
        ci += fstr(23, "G13080059") + fstr(24, "M13080110")
        zf.writestr("CameraInfo.gpbenc", ci)
        cal = fint(1, 20130306)
        for k in tables:
            cal += fbytes(3, cal_table(k))
        zf.writestr("CalibrationData.gpbenc", cal)
        info = fint(1, image_key[0]) + fint(2, image_key[1]) + fint(4, 2 * w) + fint(5, 2 * (h + 1))
        info += ff32(10, EPS) + ff32(11, BG) + ff32(12, TAU)
        zf.writestr("Images/Main/IRImageInfo.gpbenc", info)
        if ir:
            header = [0x5201] + [0] * (w - 1)
            zf.writestr("Images/Main/IR.data", struct.pack(f"<{w * (h + 1)}H", *header, *counts))
        zf.writestr("Images/Main/00200018.jpg", tiny_jpeg(32, 24))
        zf.writestr("Images/Main/00400030.jpg", tiny_jpeg(64, 48))
        if with_json:
            props = {
                "IRPROP_THERMAL_IMAGER_MODEL": '"Ti400"',
                "IRPROP_THERMAL_IMAGER_SN": '"G13080059"',
                "IRPROP_IR_SENSOR_WIDTH": str(w),
                "IRPROP_IR_SENSOR_HEIGHT": str(h),
                "IRPROP_THERMAL_IMAGE_CAPTURE_DATE_TIME": "2013-09-04 00:00:48",
                "IRPROP_THERMAL_IMAGE_EMISSIVITY": ".9",
            }
            zf.writestr("ImageProperties.json", b"\xff\xfe" + json.dumps(props).encode("utf-16-le"))
    return z.getvalue()


w, h = 10, 4
counts = [int(counts_at(-15 + 2.5 * i)) for i in range(w * h)]
(OUT / "is2_json.is2").write_bytes(is2(w, h, counts, True))
(OUT / "is2_bare.is2").write_bytes(is2(w, h, counts, False, tables=((0, 0), (1, 0)), image_key=(1, 0)))
(OUT / "is2_noir.is2").write_bytes(is2(w, h, counts, False, ir=False))
(OUT / "is2_ambiguous.is2").write_bytes(is2(w, h, counts, False, tables=((0, 0), (1, 0)), image_key=(2, 0)))
(OUT / "is2_old.is2").write_bytes(bytes([0, 1, 0, 1, 0, 0, 0, 0, 0, 0x16, 0xFF, 0xFF]) + b"\0" * 200)
for name in ("is2_json.is2", "is2_bare.is2"):
    rows.append(f"is2\t{name}\tsize\t{w}\t{h}")
    rows.append(f"is2\t{name}\tcond\t{EPS:.9f}\t{BG}\t{TAU}")
    for i, c in enumerate(counts):
        rows.append(f"is2\t{name}\tpx\t{i}\t{c}\t{fluke_celsius(c, EPS, BG, TAU):.6f}")
    # Another emissivity and background: the curves recompute.
    for i, c in enumerate(counts):
        rows.append(f"is2\t{name}\tpx2\t{i}\t{c}\t{fluke_celsius(c, 0.7, 35.0, TAU):.6f}")

(OUT / "bmt_is2_expected.tsv").write_text("\n".join(rows) + "\n")
