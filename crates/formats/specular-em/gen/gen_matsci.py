"""Writes the TIA and Bruker BCF fixtures in tests/data/matsci (each well under 200 KB).

    python3 gen/gen_matsci.py
then the expected values with the RosettaSciIO oracle:
    <venv>/bin/python -I gen/oracle_matsci.py tests/data/matsci

* tia_series_1.ser + tia_series.emi: a TEM image series (3 frames of 32 x 24 u16, version
  0x0210, 32-bit offsets).
* tia_si_1.ser + tia_si.emi: a STEM spectrum image (4 x 3 positions scanned x fastest, 64
  u32 channels, version 0x0220, 64-bit offsets).
* tia_line_1.ser: a 5-point f32 line profile without an .emi.
* bcf_plain.bcf: uncompressed, a 6 x 4 map with 16-bit, 12-bit and instructed pixels (every
  bunch width), extra pulses, two image planes (one blank) and missing pixels.
* bcf_zlib.bcf: the same map AACS-compressed in small blocks, file version 2 with an
  overview image, pointer tables chained over several chunks.
"""

import base64
import pathlib
import random
import struct
import zlib

OUT = pathlib.Path(__file__).resolve().parent.parent / "tests" / "data" / "matsci"

# --- TIA ----------------------------------------------------------------------------------


def ser(path, version, data_type, dims, elements, tags):
    wide = version == 0x220
    head = struct.pack("<HHHIIII", 0x4949, 0x0197, version, data_type, 0x4142, len(elements), len(elements))
    off_fmt = "<Q" if wide else "<I"
    body = bytearray(head)
    table_at = len(body)
    body += struct.pack(off_fmt, 0)
    body += struct.pack("<I", len(dims))
    for size, off, delta, element, desc, units in dims:
        body += struct.pack("<Iddi", size, off, delta, element)
        body += struct.pack("<I", len(desc)) + desc.encode()
        body += struct.pack("<I", len(units)) + units.encode()
    offs, tagoffs = [], []
    for e, (x, y, t) in zip(elements, tags):
        offs.append(len(body))
        body += e
        tagoffs.append(len(body))
        body += struct.pack("<HHIdd", 0x4142, 0, t, x, y)
    table = len(body)
    body[table_at : table_at + (8 if wide else 4)] = struct.pack(off_fmt, table)
    for o in offs + tagoffs:
        body += struct.pack(off_fmt, o)
    path.write_bytes(bytes(body))


def emi(path, objects):
    blob = bytearray(b"\x00" * 64)
    for o in objects:
        blob += b"\x01\x02" + o.encode() + b"\x00" * 7
    path.write_bytes(bytes(blob))


def object_info(mode, kv, mag, camera=None):
    data = "".join(
        f"<Data><Label>{l}</Label><Value>{v}</Value><Unit>{u}</Unit></Data>"
        for l, v, u in [
            ("Microscope", "Microscope Tecnai F20 Twin", ""),
            ("Gun type", "FEG", ""),
            ("High tension", kv, "kV"),
            ("Mode", mode, ""),
            ("Magnification", mag, "x"),
            ("Stage X", "12.5", "um"),
        ]
    )
    acq = f"<DwellTimePath>0.002</DwellTimePath><Binning>1</Binning>"
    if camera:
        acq += f"<CameraNamePath>{camera}</CameraNamePath>"
    return (
        "<ObjectInfo><Uuid>0a1b</Uuid><AcquireDate>Sun Feb 21 17:50:18 2016</AcquireDate>"
        f"<Manufacturer>FEI Company</Manufacturer><AcquireInfo>{acq}</AcquireInfo>"
        "<ExperimentalConditions><MicroscopeConditions>"
        f"<AcceleratingVoltage>{float(kv) * 1e3}</AcceleratingVoltage>"
        "</MicroscopeConditions></ExperimentalConditions>"
        f"<ExperimentalDescription><Root>{data}</Root></ExperimentalDescription></ObjectInfo>"
    )


def tia(rng):
    w, h = 32, 24
    frames = []
    for k in range(3):
        px = [(x * 7 + y * 131 + k * 1000 + rng.randrange(50)) & 0xFFFF for y in range(h) for x in range(w)]
        e = struct.pack("<ddiddiHII", -1.6e-8, 5e-10, 0, -1.2e-8, 5e-10, 0, 2, w, h)
        frames.append(e + struct.pack(f"<{w * h}H", *px))
    ser(
        OUT / "tia_series_1.ser",
        0x210,
        0x4122,
        [(3, 0.0, 1.0, 0, "Number", "")],
        frames,
        [(0.0, 0.0, k) for k in range(3)],
    )
    emi(OUT / "tia_series.emi", [object_info(" TEM Bright field", "200", "38000")])

    nx, ny, c = 4, 3, 64
    spectra, tags = [], []
    for y in range(ny):
        for x in range(nx):
            v = [rng.randrange(1000) if (ch + x + y) % 5 else 0 for ch in range(c)]
            e = struct.pack("<ddiHI", 120.0, 2.5, 0, 3, c) + struct.pack(f"<{c}I", *v)
            spectra.append(e)
            tags.append((1e-9 * (x - 2), -1e-9 * (y - 1), x + y * nx))
    ser(
        OUT / "tia_si_1.ser",
        0x220,
        0x4120,
        [(nx, -2e-9, 1e-9, 0, "Position", "meters"), (ny, 1e-9, -1e-9, 0, "Position", "meters")],
        spectra,
        tags,
    )
    emi(OUT / "tia_si.emi", [object_info("STEM nP SA Zoom Image", "300", "910000")])

    line = []
    for k in range(5):
        v = [float((ch * k) % 17) / 4 for ch in range(40)]
        line.append(struct.pack("<ddiHI", -20.0, 0.2, 0, 7, 40) + struct.pack("<40f", *v))
    ser(
        OUT / "tia_line_1.ser",
        0x210,
        0x4120,
        [(5, 0.0, 3e-10, 0, "Position", "meters")],
        line,
        [(3e-10 * k, 0.0, k) for k in range(5)],
    )


# --- BCF ----------------------------------------------------------------------------------

CHUNK = 0x400 + 32


def container(files):
    """SFS container with EDSDatabase/<name> files; pointer tables chain across chunks."""
    usable = CHUNK - 32
    chunks = []  # (next chunk in a chain or None, data); entry k is chunk k + 1

    def chain(data):
        pieces = [data[i : i + usable] for i in range(0, len(data), usable)] or [b""]
        first = len(chunks) + 1
        for k, p in enumerate(pieces):
            chunks.append((first + k + 1 if k + 1 < len(pieces) else 0, p))
        return first

    def item(table, size, parent, is_dir, name):
        r = struct.pack("<iQQQQIi", table, size, 0, 0, 0, 0, parent)
        r += b"\x00" * 176 + struct.pack("<?3x", is_dir) + name.encode().ljust(256, b"\x00") + b"\x00" * 32
        assert len(r) == 0x200
        return r

    items = [item(0, 0, -1, True, "EDSDatabase")]
    for name, data in files:
        first = len(chunks) + 1
        n = -(-len(data) // usable)
        for k in range(n):
            chunks.append((None, data[k * usable : (k + 1) * usable]))
        table = b"".join(struct.pack("<I", first + k) for k in range(n))
        items.append(item(chain(table), len(data), 0, False, name))
    tree = chain(b"".join(items))
    b = bytearray(CHUNK * (len(chunks) + 1) + 0x118)
    b[0:8] = b"AAMVHFSS"
    b[0x124:0x12C] = struct.pack("<fI", 2.6, CHUNK)
    b[0x140:0x14C] = struct.pack("<III", tree, len(items), len(chunks) + 1)
    for k, (nxt, data) in enumerate(chunks):
        at = (k + 1) * CHUNK
        if nxt is not None:
            b[at + 0x118 : at + 0x11C] = struct.pack("<I", nxt)
        b[at + 0x138 : at + 0x138 + len(data)] = data
    return bytes(b)


def aacs(data, block):
    blocks = [data[i : i + block] for i in range(0, len(data), block)]
    out = bytearray(struct.pack("<4sIII", b"AACS", block, 0, len(blocks)))
    out += b"\x00" * (0x80 - len(out))
    for blk in blocks:
        z = zlib.compress(blk, 6)
        out += struct.pack("<IIII", len(z), len(blk), 0, len(z) + 16) + z
    return bytes(out)


def record(x, channels, flag, pulses, data, extra=()):
    tail = 4 if flag > 1 else 0
    r = struct.pack("<IHHIHHHI", x, channels, channels, 0x80, flag, len(data), pulses, len(data) + tail)
    r += data
    if flag > 1:
        r += struct.pack("<I", 2 * len(extra)) + struct.pack(f"<{len(extra)}H", *extra)
    return r


def pack12(ch):
    out = bytearray()
    for i in range(0, len(ch), 4):
        c = list(ch[i : i + 4]) + [0] * (4 - len(ch[i : i + 4]))
        out += bytes(
            [
                ((c[0] & 0xF) << 4) | (c[1] >> 8),
                c[0] >> 4,
                c[2] >> 4,
                c[1] & 0xFF,
                c[3] & 0xFF,
                ((c[2] & 0xF) << 4) | (c[3] >> 8),
            ]
        )
    return bytes(out)


def bunch(width, gain, values):
    out = struct.pack("<BB", width, len(values))
    if width == 0:
        return out
    out += struct.pack({1: "<B", 2: "<H", 4: "<I", 8: "<Q"}[width], gain)
    if width == 1:
        vs = list(values) + [0]
        out += bytes(vs[i] | (vs[i + 1] << 4) for i in range(0, len(values), 2))
    elif width == 2:
        out += bytes(values)
    elif width == 4:
        out += struct.pack(f"<{len(values)}H", *values)
    else:
        out += struct.pack(f"<{len(values)}I", *values)
    return out


def hypermap(rng, w, h, channels):
    s = bytearray(0x1A0)
    s[0:8] = struct.pack("<ii", h, w)
    for y in range(h):
        recs = []
        for x in range(w):
            if (x + 2 * y) % 7 == 3:
                continue  # not acquired
            kind = (x + y) % 4
            if kind == 0:
                p = [rng.randrange(channels) for _ in range(rng.randrange(1, 30))]
                recs.append(record(x, channels, 0, len(p), struct.pack(f"<{len(p)}H", *p)))
            elif kind == 1:
                p = [rng.randrange(channels) for _ in range(rng.randrange(1, 30))]
                recs.append(record(x, channels, 1, len(p), pack12(p)))
            else:
                # Instructed: nibbles, a gap, bytes, 16-bit values, one 32-bit value; the
                # cython reader steps 32-bit values by two bytes, so one is used per bunch.
                d = bunch(1, rng.randrange(3), [rng.randrange(16) for _ in range(5)])
                d += bunch(0, 0, [0] * 3)
                d += bunch(2, 7, [rng.randrange(256) for _ in range(4)])
                d += bunch(4, 100, [rng.randrange(60000) for _ in range(3)])
                d += bunch(8, 5, [70000 + rng.randrange(1000)])
                used = 5 + 3 + 4 + 3 + 1
                d += bunch(0, 0, [0] * (channels - used))
                extra = [rng.randrange(channels) for _ in range(kind)] if kind == 3 else []
                recs.append(record(x, channels, 2 + kind, len(extra), d, extra))
        s += struct.pack("<I", len(recs)) + b"".join(recs)
    return bytes(s)


def header(w, h, channels, version, image, overview=None):
    def img(width, height, planes):
        ps = "".join(
            f"<Plane{i}>{f'<Description>{d}</Description>' if d else ''}"
            f"<Data>{base64.b64encode(struct.pack(f'<{len(v)}H', *v)).decode()}</Data></Plane{i}>"
            for i, (d, v) in enumerate(planes)
        )
        return (
            f"<ClassInstance Type=\"TRTImageData\"><Width>{width}</Width><Height>{height}</Height>"
            f"<ItemSize>2</ItemSize><PlaneCount>{len(planes)}</PlaneCount>{ps}</ClassInstance>"
        )

    total = [w * h * 0x20000 if ch == 0 else (ch * 37) % 500 for ch in range(channels)]
    det_layers = base64.b64encode(zlib.compress(b'<DetLayers><Layer0 Atom="5" Thickness="1E-2"/></DetLayers>')).decode()
    ov = ""
    if overview is not None:
        ov = (
            "<ClassInstance Type=\"TRTContainerClass\"><ChildClassInstances>"
            "<ClassInstance Type=\"TRTContainerClass\" Name=\"OverviewImages\"><ChildClassInstances>"
            f"{img(*overview)}</ChildClassInstances></ClassInstance></ChildClassInstances></ClassInstance>"
        )
    x = (
        '<?xml version="1.0" encoding="WINDOWS-1252" standalone="yes"?><TRTSpectrumDatabase><RTHeader/>'
        '<ClassInstance Type="TRTSpectrumDatabase" Name="Map data">'
        f"<Header><Date>12.9.2018</Date><Time>13:9:11</Time><FileVersion>{version}</FileVersion></Header>"
        '<ClassInstance Type="TRTSEMData"><HV>20</HV><WD>8,42</WD><Mag>3138,54272</Mag>'
        "<DX>0,966497572879044</DX><DY>0,866497572879043</DY></ClassInstance>"
        '<ClassInstance Type="TRTSEMStageData"><X>67358,85</X><Y>62116,44</Y></ClassInstance>'
        '<ClassInstance Type="TRTDSPConfiguration"><ImageWidth>6</ImageWidth><PixelAverage>2</PixelAverage>'
        "<LineAverage>1</LineAverage><PixelTime>1</PixelTime></ClassInstance>"
        f"{img(w, h, image)}{ov}"
        f"<LineCounter>{','.join(['1'] * h)}</LineCounter><ChCount>{channels}</ChCount><DetectorCount>1</DetectorCount>"
        '<SpectrumData0><ClassInstance Type="TRTSpectrum" Name="Map data"><TRTHeaderedClass>'
        '<ClassInstance Type="TRTSpectrumHardwareHeader"><Amplification>2E4</Amplification>'
        "<RealTime>1000</RealTime><LifeTime>900</LifeTime></ClassInstance>"
        f'<ClassInstance Type="TRTDetectorHeader"><Type>XFlash 6|10</Type><DetLayers>{det_layers}</DetLayers></ClassInstance>'
        '<ClassInstance Type="TRTESMAHeader"><PrimaryEnergy>2E1</PrimaryEnergy><ElevationAngle>3.5E1</ElevationAngle>'
        "</ClassInstance></TRTHeaderedClass>"
        '<ClassInstance Type="TRTSpectrumHeader"><Date>12.9.2018</Date><Time>13:9:10</Time>'
        f"<ChannelCount>{channels}</ChannelCount><CalibAbs>-4.7095867E-1</CalibAbs><CalibLin>9.997E-3</CalibLin>"
        f"</ClassInstance><Channels>{','.join(map(str, total))}</Channels></ClassInstance></SpectrumData0>"
        "</ClassInstance></TRTSpectrumDatabase>"
    )
    return x.encode("cp1252")


def bcf(rng):
    w, h, channels = 6, 4, 64
    se = [(x * 300 + y * 40) for y in range(h) for x in range(w)]
    image = [("SE µm", se), ("", [0] * (w * h))]
    m = hypermap(rng, w, h, channels)
    (OUT / "bcf_plain.bcf").write_bytes(container([("HeaderData", header(w, h, channels, 1, image)), ("SpectrumData0", m)]))
    ow, oh = 11, 8
    overview = (ow, oh, [("BSE", [(x * y) & 0xFFFF for y in range(oh) for x in range(ow)])])
    hdr = header(w, h, channels, 2, image, overview)
    (OUT / "bcf_zlib.bcf").write_bytes(
        container([("HeaderData", aacs(hdr, 4096)), ("SpectrumData0", aacs(m, 1000))])
    )


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    rng = random.Random(7)
    tia(rng)
    bcf(rng)
    for p in sorted(OUT.iterdir()):
        assert p.stat().st_size < 200_000, p
        print(p.name, p.stat().st_size)


if __name__ == "__main__":
    main()
