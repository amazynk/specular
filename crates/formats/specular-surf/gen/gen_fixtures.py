#!/usr/bin/env python3
"""Writes the synthetic areal topography fixtures in tests/data/surface.

    gen_fixtures.py [OUT]    (default: tests/data/surface next to this script's crate)

Each file holds one small made-up surface (a tilted ripple with a few non-measured points)
in the layout the format's documentation and public sample files show; gen/oracle.py then
reads them with the reference tools into tests/data/expected. Needs numpy and h5py.
"""

import io
import os
import struct
import sys
import zipfile
import zlib

import numpy as np

W, H = 48, 32


def surface(w=W, h=H):
    """Heights in metres and a mask of non-measured points."""
    y, x = np.mgrid[0:h, 0:w].astype(np.float64)
    z = 1e-6 * (np.sin(x / 5.0) * np.cos(y / 7.0) + 0.02 * x - 0.01 * y) + 2e-6
    bad = np.zeros((h, w), bool)
    bad[3:6, 10:14] = True
    bad[h - 1, 0] = True
    return z, bad


def intensity(w=W, h=H):
    y, x = np.mgrid[0:h, 0:w]
    return ((x * 5 + y * 3) % 256).astype(np.int64)


def x3p(path, folder, dtype):
    z, bad = surface()
    inc = 1e-9 if dtype == "I" else 1.0
    if dtype == "I":
        data = np.round(z / inc).astype("<i2").tobytes()
    else:
        data = z.astype("<f4").tobytes()
    valid = np.packbits((~bad).ravel(), bitorder="little").tobytes()
    zinc = "1e-09" if dtype == "I" else "1.0"
    xml = f"""<?xml version="1.0" encoding="UTF-8"?>
<p:ISO5436_2 xmlns:p="http://www.opengps.eu/2008/ISO5436_2">
  <Record1>
    <Revision>ISO5436 - 2000</Revision>
    <FeatureType>SUR</FeatureType>
    <Axes>
      <CX><AxisType>I</AxisType><DataType>D</DataType><Increment>2e-06</Increment><Offset>0.0</Offset></CX>
      <CY><AxisType>I</AxisType><DataType>D</DataType><Increment>3e-06</Increment><Offset>0.0</Offset></CY>
      <CZ><AxisType>A</AxisType><DataType>{dtype}</DataType><Increment>{zinc}</Increment><Offset>0.0</Offset></CZ>
    </Axes>
  </Record1>
  <Record2>
    <Date>2026-10-11T12:00:00</Date>
    <Creator>specular fixtures</Creator>
    <Instrument><Manufacturer>Specular</Manufacturer><Model>Synthetic</Model><Serial>0</Serial><Version>1</Version></Instrument>
    <CalibrationDate>2026-10-11T12:00:00</CalibrationDate>
    <ProbingSystem><Type>Software</Type><Identification>gen_fixtures.py</Identification></ProbingSystem>
    <Comment>Synthetic ripple</Comment>
  </Record2>
  <Record3>
    <MatrixDimension><SizeX>{W}</SizeX><SizeY>{H}</SizeY><SizeZ>1</SizeZ></MatrixDimension>
    <DataLink>
      <PointDataLink>bindata/data.bin</PointDataLink>
      <ValidPointsLink>bindata/valid.bin</ValidPointsLink>
    </DataLink>
  </Record3>
  <Record4><ChecksumFile>md5checksum.hex</ChecksumFile></Record4>
</p:ISO5436_2>
"""
    with zipfile.ZipFile(path, "w", zipfile.ZIP_DEFLATED) as zf:
        zf.writestr(folder + "main.xml", xml)
        zf.writestr(folder + "bindata/data.bin", data)
        zf.writestr(folder + "bindata/valid.bin", valid)
        zf.writestr(folder + "md5checksum.hex", "0" * 32 + " *main.xml\n")


def metropro(path):
    z, bad = surface()
    size = 834
    S, wl, R = 0.5, 6.328e-7, 32768
    phase = np.round(z * R / (S * wl)).astype(np.int64)
    phase[bad] = 2147483640
    h = bytearray(size)
    struct.pack_into(">IhI", h, 0, 0x881B036F, 1, size)
    h[12:36] = b"Sun Oct 11 12:00:00 2026"
    struct.pack_into(">hhHHHhi", h, 48, 0, 0, W, H, 1, 255, W * H * 2)
    struct.pack_into(">hhHHi", h, 64, 0, 0, W, H, W * H * 4)
    h[80:96] = b"synthetic ripple"
    struct.pack_into(">fffff", h, 164, S, wl, 0.3, 1.0, 20.0)
    struct.pack_into(">f", h, 184, 2e-6)
    struct.pack_into(">h", h, 218, 1)
    struct.pack_into(">hh", h, 234, W, H)
    h[246:252] = b" 20X  "
    body = intensity().astype(">u2").tobytes() + phase.astype(">i4").tobytes()
    open(path, "wb").write(bytes(h) + body)


def datx(path):
    import h5py

    z, bad = surface()
    nm = z * 1e9
    nm[bad] = np.finfo(np.float64).max
    s = h5py.string_dtype()
    with h5py.File(path, "w") as f:
        a = f.create_group("Attributes")
        a.attrs["File Layout Version"] = np.array([1], np.int32)
        g = a.create_group("{00000000-0000-0000-0000-000000000001}")
        g.attrs["Data Context.Lateral Resolution:Value"] = np.array([2e-6])
        g.attrs.create("Data Context.Lateral Resolution:Unit", np.array(["Meters"], object), dtype=s)
        g.attrs.create("Data Context.Data Attributes.Instrument", np.array(["Synthetic"], object), dtype=s)
        d = f.create_dataset("Data/Surface/{00000000-0000-0000-0000-000000000002}", data=nm)
        d.attrs.create("Unit", np.array(["NanoMeters"], object), dtype=s)
        d.attrs["No Data"] = np.array([np.finfo(np.float64).max])
        d.attrs["Interferometric Scale Factor"] = np.array([0.5])
        d.attrs["Wavelength"] = np.array([6.328e-7])
        d.attrs["Obliquity Factor"] = np.array([1.0])
        i = intensity().astype(np.int32)
        i[0, 0] = 2147483647
        d = f.create_dataset("Data/Intensity/{00000000-0000-0000-0000-000000000003}", data=i)
        d.attrs.create("Unit", np.array(["Counts"], object), dtype=s)
        d.attrs["No Data"] = np.array([2147483647], np.int32)


def vk4(path):
    z, _ = surface()
    pm_per_digit = 100
    raw = np.round(z / (pm_per_digit * 1e-12)).astype("<u4")
    light = (intensity() * 200).astype("<u2")
    rgb = np.stack([intensity(), 255 - intensity(), np.full((H, W), 40)], -1).astype(np.uint8)
    out = bytearray(b"VK4_\x00\x01\x02\x01" + bytes(4) + bytes(4 * 17))
    table = {}

    def block(k, data):
        table[k] = len(out)
        out.extend(data)

    setting = [0] * 152
    setting[0] = 608
    setting[1:7] = [2026, 10, 11, 12, 0, 0]
    setting[42], setting[43], setting[44] = 2_000_000, 3_000_000, pm_per_digit
    block(0, struct.pack("<152I", *setting))
    colour = struct.pack("<5I", W, H, 24, 0, W * H * 3) + rgb[..., ::-1].tobytes()
    block(1, colour)
    block(2, colour)
    palette = bytes(768)
    block(3, struct.pack("<7I", W, H, 16, 0, W * H * 2, 0, 65535) + palette + light.tobytes())
    block(6, struct.pack("<7I", W, H, 32, 0, W * H * 4, 0, int(raw.max())) + palette + raw.tobytes())
    title = "ripple".encode("utf-16-le")
    lens = "20x".encode("utf-16-le")
    block(16, struct.pack("<I", 6) + title + struct.pack("<I", 3) + lens)
    for k, at in table.items():
        struct.pack_into("<I", out, 12 + 4 * k, at)
    open(path, "wb").write(bytes(out))


def plu(path):
    z, bad = surface()
    um = (z * 1e6).astype("<f4")
    um[bad] = 1000001.0
    h = bytearray(500)
    h[0:24] = b"Sun Oct 11 12:00:00 2026"
    h[132:148] = b"synthetic ripple"
    struct.pack_into("<II", h, 388, H, W)
    struct.pack_into("<ffff", h, 400, 1.0, 2.0, 2.0, 0.0)
    rgb = np.stack([intensity()] * 3, -1).astype(np.uint8)
    body = struct.pack("<II", H, W) + um.tobytes() + struct.pack("<ff", 1000001.0, 1000001.0)
    open(path, "wb").write(bytes(h) + body + rgb.tobytes() + bytes(64))


def al3d(path):
    z, bad = surface()
    zz = z.astype("<f4")
    invalid = 3.0e15
    zz[bad] = invalid
    tags = [
        ("Version", "1"),
        ("TagCount", "9"),
        ("Cols", str(W)),
        ("Rows", str(H)),
        ("IconOffset", "0"),
        ("DepthImageOffset", "@"),
        ("InvalidPixelValue", repr(invalid)),
        ("PixelSizeXMeter", "2e-06"),
        ("PixelSizeYMeter", "3e-06"),
        ("NumberOfPlanes", "0"),
        ("TextureImageOffset", "0"),
    ]
    head = b"AliconaImaging\x00\r\n"
    offset = len(head) + 52 * len(tags) + 256
    out = bytearray(head)
    for k, v in tags:
        v = str(offset) if v == "@" else v
        out += k.encode().ljust(20, b"\0") + v.encode().ljust(30, b"\0") + b"\r\n"
    out += b"synthetic ripple".ljust(256, b"\0")
    out += zz.tobytes()
    open(path, "wb").write(bytes(out))


def sur_header(code, kind, bits, w, h, zmin, zmax, dz, zunit, special, zoff, name, comment):
    b = bytearray(512)
    b[0:12] = code
    struct.pack_into("<hhhh", b, 12, 0, 1, 1, kind)
    b[20:20 + len(name)] = name
    struct.pack_into("<hhhh", b, 84, 0, special, 1, 0)
    struct.pack_into("<f", b, 90, 0.0)
    struct.pack_into("<h", b, 98, bits)
    struct.pack_into("<iiiii", b, 100, zmin, zmax, w, h, w * h)
    struct.pack_into("<fff", b, 120, 2e-3, 3e-3, dz)
    for k, axis in enumerate([b"X", b"Y", b"Z"]):
        b[132 + 16 * k:132 + 16 * k + len(axis)] = axis
    for k, unit in enumerate([b"mm", b"mm", zunit]):
        b[180 + 16 * k:180 + 16 * k + len(unit)] = unit
        b[228 + 16 * k:228 + 16 * k + len(unit)] = unit
    struct.pack_into("<fff", b, 276, 1.0, 1.0, 1.0)
    struct.pack_into("<hhhhhhh", b, 306, 0, 0, 12, 11, 10, 2026, 0)
    struct.pack_into("<hh", b, 334, len(comment), 0)
    struct.pack_into("<fff", b, 466, 0.0, 0.0, zoff)
    return bytes(b) + comment


def sur(path, compressed):
    z, bad = surface()
    dz = 1e-6  # µm per count
    zmin = -1000
    raw = np.round(z * 1e6 / dz).astype(np.int64) - 2_000_000 + zmin
    raw = raw - raw.min() + zmin
    if compressed:
        raw[bad] = zmin - 2
    zmax = int(raw.max())
    code = b"DSCOMPRESSED" if compressed else b"DIGITAL SURF"
    name = b"* ripple *"

    def points(a, fmt):
        data = a.astype(fmt).tobytes()
        if not compressed:
            return data
        # One stream, as Digital Surf writes files of this size.
        parts = [data]
        out = struct.pack("<I", len(parts))
        for p in parts:
            c = zlib.compress(p)
            out += struct.pack("<II", len(p), len(c)) + c
        return out

    out = sur_header(code, 2, 32, W, H, zmin, zmax, dz, b"um", int(compressed), 1.5, name, b"ripple")
    out += points(raw, "<i4")
    if compressed:
        i = intensity()
        out += sur_header(code, 10, 16, W, H, 0, 255, 1.0, b"", 0, 0.0, b"", b"")
        out += points(i, "<i2")
    open(path, "wb").write(out)


def main():
    here = os.path.dirname(os.path.abspath(__file__))
    out = sys.argv[1] if len(sys.argv) > 1 else os.path.join(here, "..", "tests", "data", "surface")
    os.makedirs(out, exist_ok=True)
    x3p(os.path.join(out, "ripple_int16.x3p"), "", "I")
    x3p(os.path.join(out, "ripple_folder.x3p"), "ripple/", "F")
    metropro(os.path.join(out, "ripple.dat"))
    datx(os.path.join(out, "ripple.datx"))
    vk4(os.path.join(out, "ripple.vk4"))
    plu(os.path.join(out, "ripple.plu"))
    al3d(os.path.join(out, "ripple.al3d"))
    sur(os.path.join(out, "ripple.sur"), False)
    sur(os.path.join(out, "ripple_compressed.sur"), True)
    for name in sorted(os.listdir(out)):
        size = os.path.getsize(os.path.join(out, name))
        assert size < 200_000, name
        print(f"{size:8d} {name}")


if __name__ == "__main__":
    main()
