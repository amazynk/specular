#!/usr/bin/env python3
"""A generated photo library for the real-use runs (tools/realuse/run.py, scenarios 1 and 6).

Every file is a pure function of its index, so each search count is known in advance
(`expect()` below). Per unique photo `i` (0 <= i < unique):

- a 16 x 16 greyscale baseline JPEG from our own DC-only encoder: four 8 x 8 blocks whose
  levels are the base-64 digits of i, so no two photos share their pixels (about 1 KB);
- its own EXIF: make and model, ISO, date, exposure, f-number, focal length and its 35 mm
  equivalent, flash, white balance and light source, exposure compensation, program,
  metering, lens make, model and serial, body serial, software, artist, copyright,
  description, and GPS position and altitude;
- every 10th has an XMP sidecar (rating and keywords);
- every 50th has a raw twin: a 64 x 48 DNG with the same stem (RAW+JPEG pairs);
- the first `unique * links_per // 10` ... see LINK_* below: hard links in known groups
  (byte-identical duplicates under other names, in other folders).

Folders nest 30 deep along one branch (deep/d01/d02/…/d30) plus yyyy/dNNN folders of 100.

    python3 -I tools/realuse/gen_library.py OUT [--unique 25000] [--links 75000]

Stops when the disk has less than 8 GB free. Prints a JSON summary; `expect(query)` is used
by run.py.
"""

import argparse
import json
import os
import shutil
import struct
import sys

CAMERAS = [
    ("Canon", "Canon EOS R5"),
    ("NIKON CORPORATION", "NIKON Z 8"),
    ("SONY", "ILCE-7M4"),
    ("FUJIFILM", "X-T5"),
    ("Google", "Pixel 8"),
]
LENSES = ["RF24-70mm F2.8 L IS USM", "NIKKOR Z 50mm f/1.8 S", "FE 85mm F1.8",
          "XF16-55mmF2.8 R LM WR"]
FOCAL35 = [16, 24, 28, 35, 50, 85, 135]
ARTISTS = ["Alex Doe", "Sam Roe", "Kim Poe"]
KEYWORDS = ["beach", "mountain", "family", "city"]
MIN_FREE = 8 << 30
# Hard links: unique photo i < LINKED has (i % 5) + 1 links (1..5, mean 3), until the link
# budget is spent.

# --- what photo i is ----------------------------------------------------------------------------


def facts(i):
    make, model = CAMERAS[i % 5]
    return {
        "make": make,
        "model": model,
        "iso": 100 << (i % 6),
        "flash": 0x01 if i % 3 == 0 else 0x10,
        "focal35": FOCAL35[i % 7],
        "focal": FOCAL35[i % 7] * 2 // 3,
        "lens": LENSES[i % 4],
        "wb": i % 2,
        "light": [1, 10, 3][i % 3],
        "ev3": (i % 5) - 2,          # exposure compensation in thirds
        "program": [1, 2, 3, 4][i % 4],
        "metering": [2, 3, 5][i % 3],
        "serial": f"SN{i % 1000:05d}",
        "lens_serial": f"LS{i % 97:04d}",
        "software": f"Firmware 1.{i % 3}",
        "artist": ARTISTS[i % 3],
        "copyright": f"(c) {ARTISTS[i % 3]}",
        "description": f"Frame {i}",
        "altitude": (i % 50) * 10,
        "lat": 40 + (i % 90) / 100.0,
        "lon": 10 + (i % 70) / 100.0,
        "date": "%04d:%02d:%02d %02d:%02d:%02d" % (2016 + i // 12500, 1 + (i // 1000) % 12,
                                                   1 + (i // 40) % 28, (i // 7) % 24, i % 60,
                                                   (i * 7) % 60),
        "sidecar": i % 10 == 0,
        "rating": (i // 10) % 6,
        "keyword": KEYWORDS[(i // 10) % 4],
        "raw": i % 50 == 0,
    }


def links_of(i, unique, budget):
    """How many hard links photo i has (the first photos get 1..5 until `budget` is spent)."""
    n, spent = 0, 0
    # Closed form: photo k has (k % 5) + 1 links; a run of 5 photos has 15.
    full = budget // 15
    if i < full * 5:
        return (i % 5) + 1
    rest = budget - full * 15
    for k in range(full * 5, min(unique, full * 5 + 5)):
        n = min((k % 5) + 1, rest - spent)
        if k == i:
            return max(n, 0)
        spent += max(n, 0)
    return 0


def folder(i):
    if i % 1000 == 7:
        # A few photos along one deep branch.
        depth = 1 + (i // 1000) % 30
        return os.path.join("deep", *[f"d{k:02d}" for k in range(1, depth + 1)])
    return os.path.join(str(2016 + i // 12500), f"d{(i // 100) % 125:03d}")


# --- EXIF and TIFF ------------------------------------------------------------------------------

TYPES = {"byte": (1, 1), "ascii": (2, 1), "short": (3, 2), "long": (4, 4), "rational": (5, 8),
         "undef": (7, 1), "srational": (10, 8)}


def _value(kind, v):
    if kind == "ascii":
        return v.encode("ascii") + b"\0"
    if kind == "undef":
        return bytes(v)
    if kind == "byte":
        return bytes(v)
    fmt = {"short": "<H", "long": "<I"}
    if kind in fmt:
        return b"".join(struct.pack(fmt[kind], x) for x in v)
    if kind == "rational":
        return b"".join(struct.pack("<II", a, b) for a, b in v)
    if kind == "srational":
        return b"".join(struct.pack("<ii", a, b) for a, b in v)
    raise ValueError(kind)


def _count(kind, v):
    if kind == "ascii":
        return len(v) + 1
    return len(v)


def tiff(ifds, data=b""):
    """A little-endian TIFF: `ifds` is a list of entry lists [(tag, kind, value)], chained
    as sub-IFDs by the special kinds "ifd:N" (a LONG pointing at ifds[N]). `data` is placed
    right after the header; tags with kind "dataoff" get its offset."""
    out = bytearray(b"II*\0\0\0\0\0")
    data_at = len(out)
    out += data
    if len(out) % 2:
        out += b"\0"
    # Lay out each IFD then its values.
    offsets = [0] * len(ifds)
    sizes = [2 + 12 * len(e) + 4 for e in ifds]
    pos = len(out)
    for k, e in enumerate(ifds):
        offsets[k] = pos
        pos += sizes[k]
        for tag, kind, v in e:
            if kind.startswith("ifd:") or kind == "dataoff":
                continue
            n = len(_value(kind, v))
            if n > 4:
                pos += n + (n % 2)
    blob = bytearray()
    vpos = None
    for k, e in enumerate(ifds):
        e = sorted(e, key=lambda x: x[0])
        head = bytearray(struct.pack("<H", len(e)))
        vals = bytearray()
        vstart = offsets[k] + sizes[k]
        for tag, kind, v in e:
            if kind.startswith("ifd:"):
                head += struct.pack("<HHII", tag, 4, 1, offsets[int(kind[4:])])
                continue
            if kind == "dataoff":
                head += struct.pack("<HHII", tag, 4, 1, data_at + v)
                continue
            code, _ = TYPES[kind]
            raw = _value(kind, v)
            cnt = _count(kind, v) if kind not in ("rational", "srational") else len(v)
            if len(raw) <= 4:
                head += struct.pack("<HHI", tag, code, cnt) + raw.ljust(4, b"\0")
            else:
                head += struct.pack("<HHII", tag, code, cnt, vstart + len(vals))
                vals += raw
                if len(vals) % 2:
                    vals += b"\0"
        head += struct.pack("<I", 0)
        blob += head + vals
        vpos = vstart + len(vals)
    out += blob
    out[4:8] = struct.pack("<I", offsets[0])
    assert vpos is None or len(out) == vpos, (len(out), vpos)
    return bytes(out)


def exif_block(f):
    ev = (f["ev3"], 3)
    ifd0 = [
        (0x010E, "ascii", f["description"]),
        (0x010F, "ascii", f["make"]),
        (0x0110, "ascii", f["model"]),
        (0x0131, "ascii", f["software"]),
        (0x013B, "ascii", f["artist"]),
        (0x8298, "ascii", f["copyright"]),
        (0x8769, "ifd:1", None),
        (0x8825, "ifd:2", None),
    ]
    exif = [
        (0x829A, "rational", [(1, 125)]),
        (0x829D, "rational", [(28, 10)]),
        (0x8822, "short", [f["program"]]),
        (0x8827, "short", [f["iso"]]),
        (0x9003, "ascii", f["date"]),
        (0x9204, "srational", [ev]),
        (0x9207, "short", [f["metering"]]),
        (0x9208, "short", [f["light"]]),
        (0x9209, "short", [f["flash"]]),
        (0x920A, "rational", [(f["focal"], 1)]),
        (0xA402, "short", [0]),
        (0xA403, "short", [f["wb"]]),
        (0xA405, "short", [f["focal35"]]),
        (0xA431, "ascii", f["serial"]),
        (0xA433, "ascii", f["make"]),
        (0xA434, "ascii", f["lens"]),
        (0xA435, "ascii", f["lens_serial"]),
    ]

    def dms(x):
        d = int(x)
        m = int((x - d) * 60)
        s = round(((x - d) * 60 - m) * 60 * 100)
        return [(d, 1), (m, 1), (s, 100)]

    gps = [
        (0x0000, "byte", [2, 3, 0, 0]),
        (0x0001, "ascii", "N"),
        (0x0002, "rational", dms(f["lat"])),
        (0x0003, "ascii", "E"),
        (0x0004, "rational", dms(f["lon"])),
        (0x0005, "byte", [0]),
        (0x0006, "rational", [(f["altitude"], 1)]),
    ]
    return tiff([ifd0, exif, gps])


# --- the JPEG -----------------------------------------------------------------------------------

# Our own tiny tables: 12 DC categories, each a 4-bit code; AC: EOB as "00" (and ZRL "01").
DC_BITS = [0, 0, 0, 12] + [0] * 12
DC_VALS = list(range(12))
AC_BITS = [0, 2] + [0] * 14
AC_VALS = [0x00, 0xF0]


class Bits:
    def __init__(self):
        self.out = bytearray()
        self.acc = 0
        self.n = 0

    def put(self, v, n):
        for k in range(n - 1, -1, -1):
            self.acc = (self.acc << 1) | ((v >> k) & 1)
            self.n += 1
            if self.n == 8:
                self.out.append(self.acc)
                if self.acc == 0xFF:
                    self.out.append(0)
                self.acc = 0
                self.n = 0

    def flush(self):
        while self.n:
            self.put(1, 1)
        return bytes(self.out)


def levels(i):
    """The four blocks' grey levels (2..254) from the base-64 digits of i."""
    return [2 + 4 * ((i >> (6 * k)) & 63) for k in range(4)]


def jpeg(i, exif):
    seg = bytearray(b"\xFF\xD8")
    app1 = b"Exif\0\0" + exif
    seg += b"\xFF\xE1" + struct.pack(">H", len(app1) + 2) + app1
    seg += b"\xFF\xDB" + struct.pack(">H", 67) + b"\x00" + b"\x01" * 64
    seg += b"\xFF\xC0" + struct.pack(">HBHHB", 11, 8, 16, 16, 1) + b"\x01\x11\x00"
    dht = b"\x00" + bytes(DC_BITS) + bytes(DC_VALS) + b"\x10" + bytes(AC_BITS) + bytes(AC_VALS)
    seg += b"\xFF\xC4" + struct.pack(">H", len(dht) + 2) + dht
    seg += b"\xFF\xDA" + struct.pack(">HB", 8, 1) + b"\x01\x00" + b"\x00\x3F\x00"
    bits = Bits()
    prev = 0
    for lv in levels(i):
        dc = (lv - 128) * 8
        diff = dc - prev
        prev = dc
        size = abs(diff).bit_length()
        bits.put(size, 4)  # category code: 4 bits, value = category
        if size:
            v = diff if diff > 0 else diff + (1 << size) - 1
            bits.put(v, size)
        bits.put(0b00, 2)  # EOB
    seg += bits.flush()
    seg += b"\xFF\xD9"
    return bytes(seg)


# --- the DNG ------------------------------------------------------------------------------------


def dng(i, f):
    w, h = 64, 48
    lv = levels(i)
    px = bytearray()
    for y in range(h):
        for x in range(w):
            q = (y >= h // 2) * 2 + (x >= w // 2)
            px += struct.pack("<H", 64 + lv[q] * 12)
    ifd0 = [
        (0x00FE, "long", [0]),
        (0x0100, "long", [w]),
        (0x0101, "long", [h]),
        (0x0102, "short", [16]),
        (0x0103, "short", [1]),
        (0x0106, "short", [32803]),
        (0x010F, "ascii", f["make"]),
        (0x0110, "ascii", f["model"]),
        (0x0111, "dataoff", 0),
        (0x0112, "short", [1]),
        (0x0115, "short", [1]),
        (0x0116, "long", [h]),
        (0x0117, "long", [len(px)]),
        (0x828D, "short", [2, 2]),
        (0x828E, "byte", [0, 1, 1, 2]),
        (0x8769, "ifd:1", None),
        (0xC612, "byte", [1, 4, 0, 0]),
        (0xC614, "ascii", f["model"]),
        (0xC61A, "long", [64]),
        (0xC61D, "long", [4095]),
        (0xC621, "srational", [(10000, 10000), (0, 10000), (0, 10000), (0, 10000),
                               (10000, 10000), (0, 10000), (0, 10000), (0, 10000),
                               (10000, 10000)]),
        (0xC628, "rational", [(1, 2), (1, 1), (2, 3)]),
        (0xC65A, "short", [21]),
    ]
    exif = [
        (0x8827, "short", [f["iso"]]),
        (0x9003, "ascii", f["date"]),
    ]
    return tiff([ifd0, exif], bytes(px))


SIDECAR = ('<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF '
           'xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"><rdf:Description rdf:about="" '
           'xmlns:xmp="http://ns.adobe.com/xap/1.0/" xmlns:dc="http://purl.org/dc/elements/1.1/" '
           'xmp:Rating="{rating}"><dc:subject><rdf:Bag><rdf:li>Holiday</rdf:li>'
           '<rdf:li>{keyword}</rdf:li></rdf:Bag></dc:subject></rdf:Description></rdf:RDF>'
           '</x:xmpmeta>')


# --- the counts ---------------------------------------------------------------------------------


def expect(pred, unique, links):
    """Photos (paths, links included) whose facts satisfy `pred`; links have no sidecar."""
    n = 0
    for i in range(unique):
        f = facts(i)
        if pred(f, True):
            n += 1
        k = links_of(i, unique, links)
        if k and pred(f, False):
            n += k
    return n


def generate(out, unique, links):
    os.makedirs(out, exist_ok=True)
    linkdir = os.path.join(out, "copies")
    written, made_links, raws, sidecars = 0, 0, 0, 0
    for i in range(unique):
        if i % 1000 == 0 and shutil.disk_usage(out).free < MIN_FREE:
            sys.exit(f"gen_library: less than 8 GB free on disk at photo {i}; stopped")
        f = facts(i)
        d = os.path.join(out, folder(i))
        os.makedirs(d, exist_ok=True)
        p = os.path.join(d, f"IMG_{i:06d}.jpg")
        if not os.path.exists(p):
            with open(p, "wb") as fh:
                fh.write(jpeg(i, exif_block(f)))
        written += 1
        if f["sidecar"]:
            x = os.path.join(d, f"IMG_{i:06d}.xmp")
            if not os.path.exists(x):
                with open(x, "w") as fh:
                    fh.write(SIDECAR.format(rating=f["rating"], keyword=f["keyword"]))
            sidecars += 1
        if f["raw"]:
            r = os.path.join(d, f"IMG_{i:06d}.dng")
            if not os.path.exists(r):
                with open(r, "wb") as fh:
                    fh.write(dng(i, f))
            raws += 1
        for k in range(links_of(i, unique, links)):
            ld = os.path.join(linkdir, f"g{(i // 100):03d}")
            os.makedirs(ld, exist_ok=True)
            lp = os.path.join(ld, f"COPY_{i:06d}_{k}.jpg")
            if not os.path.exists(lp):
                os.link(p, lp)
            made_links += 1
    return {"unique": written, "links": made_links, "raws": raws, "sidecars": sidecars,
            "images": written + made_links + raws}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("out")
    ap.add_argument("--unique", type=int, default=25000)
    ap.add_argument("--links", type=int, default=75000)
    a = ap.parse_args()
    s = generate(a.out, a.unique, a.links)
    with open(os.path.join(a.out, "generated.json"), "w") as fh:
        json.dump({"unique": a.unique, "links": a.links, **s}, fh)
    print(json.dumps(s))


if __name__ == "__main__":
    main()
