#!/usr/bin/env python3
"""Write the synthetic LIF / LOF / ND2 fixtures under ../tests/data.

Hand-written writers, following the published structure of the formats (the Leica
"LIF, XLEF, XLLF, LOF" file format description and the BSD-licensed `nd2` package's
documented layout); the files are then read back by the oracles (`oracle.py`: liffile,
readlif, nd2). Pixel values are deterministic so the expected JSON is reproducible.

    python make_fixtures.py [outdir]

Needs numpy and (for the legacy ND2) imagecodecs' JPEG 2000 encoder.
"""

from __future__ import annotations

import struct
import sys
import zlib
from pathlib import Path

import numpy as np

OUT = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(__file__).resolve().parent.parent / "tests" / "data"


def ramp(shape, seed, maxval, dtype):
    """Deterministic, structured values: gradients plus a hashed texture."""
    idx = np.indices(shape).astype(np.int64)
    v = seed * 7919
    for k, a in enumerate(idx):
        v = v + a * (31 + 17 * k + seed)
    v = (v * 2654435761 + (v >> 3)) % (maxval + 1)
    return v.astype(dtype)


# --------------------------------------------------------------------------- LIF


def u16s(s: str) -> bytes:
    return s.encode("utf-16-le")


def lif_header(xml: str) -> bytes:
    body = u16s(xml)
    return struct.pack("<IIBI", 0x70, len(body) + 5, 0x2A, len(xml)) + body


def lif_block(block_id: str, data: bytes, version: int) -> bytes:
    if version == 2:
        head = struct.pack("<IIBQBI", 0x70, 0, 0x2A, len(data), 0x2A, len(block_id))
    else:
        head = struct.pack("<IIBIBI", 0x70, 0, 0x2A, len(data), 0x2A, len(block_id))
    return head + u16s(block_id) + data


def dim_xml(dim_id, n, origin, length, unit, bytes_inc):
    return (
        f'<DimensionDescription DimID="{dim_id}" NumberOfElements="{n}" Origin="{origin!r}" '
        f'Length="{length!r}" Unit="{unit}" BitInc="0" BytesInc="{bytes_inc}"/>'
    )


def chan_xml(dtype, res, lut, bytes_inc, tag=0, dye=None, lo=0.0, hi=255.0):
    prop = (
        f"<ChannelProperty><Key>DyeName</Key><Value>Leica/{dye}</Value></ChannelProperty>"
        if dye
        else ""
    )
    return (
        f'<ChannelDescription DataType="{dtype}" ChannelTag="{tag}" Resolution="{res}" '
        f'NameOfMeasuredQuantity="" Min="{lo!r}" Max="{hi!r}" Unit="" LUTName="{lut}" '
        f'IsLUTInverted="0" BytesInc="{bytes_inc}" BitInc="0">{prop}</ChannelDescription>'
    )


def image_element(name, block_id, size, dims, chans, children="", hardware=""):
    return (
        f'<Element Name="{name}" Visibility="1" CopyOption="1" UniqueID="u-{name}">'
        f'<Data><Image TextDescription=""><ImageDescription>'
        f'<Channels>{"".join(chans)}</Channels><Dimensions>{"".join(dims)}</Dimensions>'
        f"</ImageDescription>{hardware}</Image></Data>"
        f'<Memory Size="{size}" MemoryBlockID="{block_id}"/>'
        f"<Children>{children}</Children></Element>"
    )


def folder_element(name, block_id, children):
    return (
        f'<Element Name="{name}" Visibility="1" CopyOption="1" UniqueID="u-{name}">'
        f'<Data><Experiment Path="x"/></Data><Memory Size="0" MemoryBlockID="{block_id}"/>'
        f"<Children>{children}</Children></Element>"
    )


def series_planar(nc, x, y, z, bits, seed):
    """Channels as separate planes: X fastest, then Y, channel, Z."""
    dt = np.uint16 if bits > 8 else np.uint8
    bs = np.dtype(dt).itemsize
    arr = ramp((z, nc, y, x), seed, (1 << bits) - 1, dt)
    dims = [
        dim_xml(1, x, 0.0, 1.5e-7 * (x - 1), "m", bs),
        dim_xml(2, y, 2e-6, 1.5e-7 * (y - 1), "m", bs * x),
        dim_xml(3, z, 0.0, 5e-7 * (z - 1), "m", bs * x * y * nc),
    ]
    return arr.tobytes(), dims, bs * x * y


def series_interleaved(nc, x, y, extra, seed, bits=8):
    """Channels interleaved per pixel; `extra` = [(DimID, n), ...] after Y."""
    dt = np.uint16 if bits > 8 else np.uint8
    bs = np.dtype(dt).itemsize
    shape = tuple(n for _, n in reversed(extra)) + (y, x, nc)
    arr = ramp(shape, seed, (1 << bits) - 1, dt)
    dims = [dim_xml(1, x, 0.0, 2e-7 * (x - 1), "m", bs * nc), dim_xml(2, y, 0.0, 2e-7 * (y - 1), "m", bs * nc * x)]
    inc = bs * nc * x * y
    for dim_id, n in extra:
        length = 1.0 * (n - 1) if dim_id == 4 else float(n - 1)
        dims.append(dim_xml(dim_id, n, 0.0, length, "s" if dim_id == 4 else "", inc))
        inc *= n
    return arr.tobytes(), dims


def series_planes(nc, x, y, extra, seed, bits=8):
    """Channels as planes (Leica confocal layout): X, Y, channel, then `extra` dims."""
    dt = np.uint16 if bits > 8 else np.uint8
    bs = np.dtype(dt).itemsize
    shape = tuple(n for _, n in reversed(extra)) + (nc, y, x)
    arr = ramp(shape, seed, (1 << bits) - 1, dt)
    dims = [dim_xml(1, x, 0.0, 2e-7 * (x - 1), "m", bs), dim_xml(2, y, 0.0, 2e-7 * (y - 1), "m", bs * x)]
    inc = bs * nc * x * y
    for dim_id, n in extra:
        length = 1.0 * (n - 1) if dim_id == 4 else float(n - 1)
        dims.append(dim_xml(dim_id, n, 0.0, length, "s" if dim_id == 4 else "", inc))
        inc *= n
    return arr.tobytes(), dims, bs * x * y


def write_lif():
    blocks = []
    # Series 1: three 12-bit channels in planes, a Z stack.
    d1, dims1, cstep = series_planar(3, 20, 16, 3, 12, 1)
    ch1 = [
        chan_xml(0, 12, lut, i * cstep, dye=dye, hi=4095.0)
        for i, (lut, dye) in enumerate([("Green", "FITC"), ("Red", "TRITC"), ("Blue", "DAPI")])
    ]
    hw = (
        '<Attachment Name="HardwareSetting"><ATLConfocalSettingDefinition '
        'ObjectiveName="HC PL APO 63x/1.40 OIL" NumericalAperture="1.4" Magnification="63" '
        'Pinhole="9.5e-05" Zoom="2"/></Attachment>'
    )
    blocks.append(("MemBlock_101", d1))
    e1 = image_element("Series001", "MemBlock_101", len(d1), dims1, ch1, hardware=hw)
    # Series 2: six 8-bit channels as planes, two time points, two tiles.
    luts = ["Blue", "Green", "Red", "Cyan", "Magenta", "Yellow"]
    dyes = ["DAPI", "GFP", "mCherry", "CFP", None, "YFP"]
    d2, dims2, cstep2 = series_planes(6, 24, 16, [(4, 2), (10, 2)], 2)
    ch2 = [chan_xml(0, 8, luts[i], i * cstep2, dye=dyes[i]) for i in range(6)]
    blocks.append(("MemBlock_102", d2))
    e2 = image_element("Mosaic 6ch", "MemBlock_102", len(d2), dims2, ch2)
    # Series 3: a colour camera image (R, G, B tags), interleaved.
    d3, dims3 = series_interleaved(3, 18, 12, [], 3)
    ch3 = [chan_xml(0, 8, lut, i, tag=i + 1) for i, lut in enumerate(["Red", "Green", "Blue"])]
    blocks.append(("MemBlock_103", d3))
    e3 = image_element("Camera RGB", "MemBlock_103", len(d3), dims3, ch3)
    # Series 4: one float channel.
    f = (ramp((8, 10), 4, 1000, np.int64).astype(np.float32) / 7.0 - 30.0).astype("<f4")
    d4 = f.tobytes()
    dims4 = [dim_xml(1, 10, 0.0, 9e-6, "m", 4), dim_xml(2, 8, 0.0, 7e-6, "m", 40)]
    ch4 = [chan_xml(1, 32, "Gray", 0, hi=1.0)]
    blocks.append(("MemBlock_104", d4))
    e4 = image_element("Ratio", "MemBlock_104", len(d4), dims4, ch4)
    folder = folder_element("Folder", "MemBlock_100", e2 + e3)
    project = folder_element("Project", "MemBlock_99", e1 + folder + e4)
    xml = f'<LMSDataContainerHeader Version="2">{project}</LMSDataContainerHeader>'
    out = bytearray(lif_header(xml))
    blocks = [("MemBlock_99", b""), ("MemBlock_100", b"")] + blocks
    for bid, data in blocks:
        out += lif_block(bid, data, 2)
    (OUT / "multi.lif").write_bytes(out)

    # Version 1 block headers: one 8-bit grey series.
    d, dims, _ = series_planar(1, 16, 12, 1, 8, 5)
    dims = dims[:2]
    e = image_element("Only", "MemBlock_7", len(d), dims, [chan_xml(0, 8, "Gray", 0)])
    xml = f'<LMSDataContainerHeader Version="1">{folder_element("v1", "MemBlock_6", e)}</LMSDataContainerHeader>'
    out = bytearray(lif_header(xml)) + lif_block("MemBlock_6", b"", 1) + lif_block("MemBlock_7", d, 1)
    (OUT / "v1.lif").write_bytes(out)


def write_lof():
    """A LOF: header text, one memory block, then a bare <Data> XML (older writers)."""
    d, dims, cstep = series_planar(2, 14, 10, 2, 16, 6)
    chans = [chan_xml(0, 16, "Red", 0, hi=65535.0), chan_xml(0, 16, "Green", cstep, hi=65535.0)]
    xml = (
        f'<Data><Image TextDescription=""><ImageDescription><Channels>{"".join(chans)}</Channels>'
        f'<Dimensions>{"".join(dims)}</Dimensions></ImageDescription></Image></Data>'
        f'<Memory Size="{len(d)}" MemoryBlockID="MemBlock_0"/>'
    )
    out = bytearray(lif_header("LMS_Object_File"))
    out += struct.pack("<BIBI", 0x2A, 2, 0x2A, 1)
    out += struct.pack("<BQ", 0x2A, len(d)) + d
    out += lif_header(xml)
    (OUT / "single.lof").write_bytes(out)


# --------------------------------------------------------------------------- ND2

U32, I32, U64, F64, BOOL, STR, BYTES = "u32", "i32", "u64", "f64", "bool", "str", "bytes"


class T:
    """A typed scalar for the CLX writers."""

    def __init__(self, kind, value):
        self.kind, self.value = kind, value


def lite(name: str, v) -> bytes:
    """One CLX Lite record."""
    nm = (name + "\0").encode("utf-16-le")
    head = lambda t: bytes([t, len(nm) // 2]) + nm
    if isinstance(v, T):
        k, x = v.kind, v.value
        if k == BOOL:
            return head(1) + struct.pack("<B", int(x))
        if k == I32:
            return head(2) + struct.pack("<i", x)
        if k == U32:
            return head(3) + struct.pack("<I", x)
        if k == U64:
            return head(5) + struct.pack("<Q", x)
        if k == F64:
            return head(6) + struct.pack("<d", x)
        if k == STR:
            return head(8) + (x + "\0").encode("utf-16-le")
        if k == BYTES:
            return head(9) + struct.pack("<Q", len(x)) + bytes(x)
    if isinstance(v, bool):
        return lite(name, T(BOOL, v))
    if isinstance(v, int):
        return lite(name, T(U32 if v >= 0 else I32, v))
    if isinstance(v, float):
        return lite(name, T(F64, v))
    if isinstance(v, str):
        return lite(name, T(STR, v))
    if isinstance(v, (list, dict)):
        items = list(v.items()) if isinstance(v, dict) else [("", x) for x in v]
        children = [lite(k, x) for k, x in items]
        h = bytes([11, len(nm) // 2]) + nm
        body = b"".join(children)
        length = len(h) + 12 + len(body)
        offsets, pos = [], len(h) + 12
        for c in children:
            offsets.append(pos)
            pos += len(c)
        return h + struct.pack("<IQ", len(items), length) + body + struct.pack(f"<{len(items)}Q", *offsets)
    raise TypeError(type(v))


def xml_variant(name: str, v, top=True) -> str:
    """The XML form of the same value (version 2 chunks)."""
    from xml.sax.saxutils import quoteattr

    tag = name or "no_name"
    if isinstance(v, T) or not isinstance(v, (list, dict)):
        if not isinstance(v, T):
            v = T(BOOL if isinstance(v, bool) else U32 if isinstance(v, int) and v >= 0 else I32 if isinstance(v, int) else F64 if isinstance(v, float) else STR, v)
        rt = {BOOL: "bool", I32: "lx_int32", U32: "lx_uint32", U64: "lx_uint64", F64: "double", STR: "CLxStringW"}[v.kind]
        val = ("true" if v.value else "false") if v.kind == BOOL else repr(v.value) if v.kind == F64 else str(v.value)
        return f"<{tag} runtype={quoteattr(rt)} value={quoteattr(val)}/>"
    items = list(v.items()) if isinstance(v, dict) else [("", x) for x in v]
    inner = "".join(xml_variant(k, x, False) for k, x in items)
    s = f'<{tag} runtype="CLxListVariant">{inner}</{tag}>'
    return f'<?xml version="1.0" encoding="UTF-8"?><variant version="1.0">{s}</variant>' if top else s


def chunk(name: bytes, data: bytes, name_len=None) -> bytes:
    nl = name_len or len(name)
    return struct.pack("<IIQ", 0x0ABECEDA, nl, len(data)) + name.ljust(nl, b"\0") + data


def nd2_file(version: str, meta: list[tuple[bytes, bytes]], frames: list[bytes]) -> bytes:
    out = bytearray(chunk(b"ND2 FILE SIGNATURE CHUNK NAME01!", version.encode().ljust(64, b"\0")))
    entries = []
    for i, f in enumerate(frames):
        name = b"ImageDataSeq|%d!" % i
        entries.append((name, len(out), len(f)))
        # Frame chunks are padded so the pixels start 4096 bytes into the chunk.
        out += chunk(name, f, 4072)
    for name, data in meta:
        entries.append((name, len(out), len(data)))
        out += chunk(name, data)
    mp = b"".join(n + struct.pack("<QQ", o, s) for n, o, s in entries)
    mp += b"ND2 CHUNK MAP SIGNATURE 0000001!" + struct.pack("<QQ", 0, 0)
    at = len(out)
    out += chunk(b"ND2 FILEMAP SIGNATURE NAME 0001!", mp)
    out += b"ND2 CHUNK MAP SIGNATURE 0000001!" + struct.pack("<Q", at)
    return bytes(out)


def attributes(w, h, comp, bpc, sig, seq, compression=2, width_bytes=None):
    return {
        "uiWidth": w,
        "uiWidthBytes": width_bytes or w * comp * bpc // 8,
        "uiWidthAligned": w,
        "uiHeight": h,
        "uiComp": comp,
        "uiBpcInMemory": bpc,
        "uiBpcSignificant": sig,
        "uiSequenceCount": seq,
        "uiTileWidth": w,
        "uiTileHeight": h,
        "eCompression": compression,
        "dCompressionParam": 0.0,
        "ePixelType": 1,
        "uiVirtualComponents": comp,
    }


WAVES = {"DAPI": (358.0, 461.0), "FITC": (495.0, 519.0), "TRITC": (557.0, 576.0), "Cy5": (649.0, 670.0),
         "GFP": (488.0, 509.0), "mCherry": (587.0, 610.0)}


def spectrum(peak):
    if not peak:
        return {"uiCount": 0, "pPoint": {}}
    pts = [(peak - 20.0, 0.4), (peak, 1.0), (peak + 25.0, 0.3)]
    return {
        "uiCount": len(pts),
        "pPoint": {f"Point{i}": {"dWavelength": wl, "dTValue": t, "eType": 1} for i, (wl, t) in enumerate(pts)},
    }


def probe(name, ex, em):
    return {"m_sName": name, "m_ExcitationSpectrum": spectrum(ex), "m_EmissionSpectrum": spectrum(em)}


def picture(planes, cal=0.325, objective="Plan Apo λ 60x Oil", mag=60.0, na=1.4):
    return {
        "dTimeMSec": 0.0,
        "dTimeAbsolute": 2460000.5,
        "dXPos": 1200.5,
        "dYPos": -340.25,
        "dZPos": 3010.0,
        "dCalibration": cal,
        "bCalibrated": True,
        "dObjectiveMag": mag,
        "dObjectiveNA": na,
        "wsObjectiveName": objective,
        "dZoom": 1.0,
        "dRefractIndex1": 1.515,
        "sPicturePlanes": {
            "uiCount": len(planes),
            "uiCompCount": sum(p[2] for p in planes),
            "sPlaneNew": {
                f"a{i}": {
                    "uiCompCount": comps,
                    "uiSampleIndex": i,
                    "uiModalityMask": 16,
                    "sDescription": name,
                    "uiColor": T(U32, colour[0] | colour[1] << 8 | colour[2] << 16),
                    "pFluorescentProbe": probe(name, *WAVES.get(name, (0.0, 0.0))),
                }
                for i, (name, colour, comps) in enumerate(planes)
            },
        },
    }


def loop(etype, pars, nxt=None, valid=None):
    d = {"eType": etype, "uLoopPars": pars}
    if valid is not None:
        d["pItemValid"] = T(BYTES, bytes(valid))
    d["ppNextLevelEx"] = {"i0000000000": nxt} if nxt else {}
    return d


def time_pars(n, period):
    return {"uiCount": n, "dStart": 0.0, "dPeriod": period, "dDuration": period * n}


def text_info(desc, optics):
    return {
        "TextInfoItem_0": "",
        "TextInfoItem_5": desc,
        "TextInfoItem_6": "Camera: synthetic\r\nExposure: 10 ms",
        "TextInfoItem_9": "11/10/2026  10:00:00",
        "TextInfoItem_13": optics,
        "TextInfoItem_14": "NIS-Elements AR 5.42.00",
    }


def write_nd2_loops():
    """Version 3, five 12-bit channels, T(2) x XY(2 of 3) x Z(3), uncompressed."""
    w, h, nc = 16, 12, 5
    planes = [
        ("DAPI", (0, 0, 255), 1),
        ("FITC", (0, 255, 0), 1),
        ("TRITC", (255, 0, 0), 1),
        ("Cy5", (255, 0, 255), 1),
        ("BF", (255, 255, 255), 1),
    ]
    z = loop(4, {"uiCount": 3, "dZStep": 0.5, "dZLow": 0.0, "dZHigh": 1.0, "dZHome": 0.5, "iType": 1})
    pts = {
        "Points": {
            f"a{i}": {"dPosX": 100.0 * i, "dPosY": -50.0 * i, "dPosZ": 3000.0, "dPosName": n}
            for i, n in enumerate(["A1", "skip", "B2"])
        }
    }
    xy = loop(2, pts, z, valid=[1, 0, 1])
    exp = loop(1, time_pars(2, 1500.0), xy)
    frames = []
    nf = 2 * 2 * 3
    data = ramp((nf, h, w, nc), 9, 4095, np.uint16)
    for i in range(nf):
        frames.append(struct.pack("<d", 1500.0 * i) + data[i].tobytes())
    meta = [
        (b"ImageAttributesLV!", lite("SLxImageAttributes", attributes(w, h, nc, 16, 12, nf))),
        (b"ImageMetadataSeqLV|0!", lite("SLxPictureMetadata", picture(planes))),
        (b"ImageMetadataLV!", lite("SLxExperiment", exp)),
        (b"ImageTextInfoLV!", lite("SLxImageTextInfo", text_info("Synthetic 5-channel stack", "Plan Apo λ 60x Oil"))),
        (b"CustomData|AcqTimesCache!", struct.pack(f"<{nf}d", *[1500.0 * i for i in range(nf)])),
    ]
    (OUT / "loops_v3.nd2").write_bytes(nd2_file("Ver3.0", meta, frames))


def write_nd2_blobs():
    """Version 3, eight 16-bit channels, one frame: a soft spot per channel in its own colour
    around a ring (the app's composite golden of more than four channels)."""
    w, h, nc = 48, 36, 8
    planes = [
        ("DAPI", (0, 0, 255), 1),
        ("FITC", (0, 255, 0), 1),
        ("TRITC", (255, 0, 0), 1),
        ("Cy5", (255, 0, 255), 1),
        ("CFP", (0, 255, 255), 1),
        ("YFP", (255, 255, 0), 1),
        ("mCherry", (255, 128, 0), 1),
        ("BF", (255, 255, 255), 1),
    ]
    yy, xx = np.mgrid[0:h, 0:w].astype(np.float64)
    data = np.zeros((h, w, nc), np.uint16)
    for c in range(nc):
        a = 2 * np.pi * c / nc
        cx, cy = w / 2 + 13 * np.cos(a), h / 2 + 10 * np.sin(a)
        r2 = (xx - cx) ** 2 + (yy - cy) ** 2
        data[:, :, c] = (200 + 30000 * np.exp(-r2 / (2 * 3.5**2))).astype(np.uint16)
    frames = [struct.pack("<d", 0.0) + data.tobytes()]
    meta = [
        (b"ImageAttributesLV!", lite("SLxImageAttributes", attributes(w, h, nc, 16, 16, 1))),
        (b"ImageMetadataSeqLV|0!", lite("SLxPictureMetadata", picture(planes, cal=0.5))),
        (b"ImageTextInfoLV!", lite("SLxImageTextInfo", text_info("Synthetic 8-channel spots", "Plan Apo 20x"))),
    ]
    (OUT / "blobs_v3.nd2").write_bytes(nd2_file("Ver3.0", meta, frames))


def write_nd2_zlib():
    """Version 3, two 8-bit channels, padded rows, zlib frames, multi-phase time x Z."""
    w, h, nc = 21, 10, 2
    wb = 44  # 21 * 2 = 42, padded to 44
    planes = [("GFP", (0, 255, 0), 1), ("mCherry", (255, 0, 0), 1)]
    z = loop(4, {"uiCount": 3, "dZStep": 0.0, "dZLow": 10.0, "dZHigh": 12.0, "iType": 1})
    netime = loop(
        8,
        {
            "pPeriod": {"a0": time_pars(1, 100.0), "a1": time_pars(5, 200.0), "a2": time_pars(1, 300.0)},
            "pPeriodValid": T(BYTES, bytes([1, 0, 1])),
        },
        z,
    )
    nf = 2 * 3
    data = ramp((nf, h, w, nc), 3, 255, np.uint8)
    frames = []
    for i in range(nf):
        rows = b"".join(data[i, y].tobytes() + b"\xee\xee" for y in range(h))
        frames.append(struct.pack("<d", 100.0 * i) + zlib.compress(rows))
    meta = [
        (b"ImageAttributesLV!", lite("SLxImageAttributes", attributes(w, h, nc, 8, 8, nf, 0, wb))),
        (b"ImageMetadataSeqLV|0!", lite("SLxPictureMetadata", picture(planes, cal=0.1625))),
        (b"ImageMetadataLV!", lite("SLxExperiment", netime)),
    ]
    (OUT / "zlib_v3.nd2").write_bytes(nd2_file("Ver3.0", meta, frames))


def write_nd2_rgb_v2():
    """Version 2, XML metadata, one RGB plane (three components), a time loop."""
    w, h = 12, 9
    planes = [("Brightfield", (255, 255, 255), 3)]
    exp = loop(1, time_pars(3, 1000.0))
    nf = 3
    data = ramp((nf, h, w, 3), 11, 255, np.uint8)
    frames = [struct.pack("<d", 1000.0 * i) + data[i].tobytes() for i in range(nf)]
    pic = picture(planes, cal=1.29, objective="Plan Fluor 10x", mag=10.0, na=0.3)
    meta = [
        (b"ImageAttributes!", xml_variant("", attributes(w, h, 3, 8, 8, nf)).encode()),
        (b"ImageMetadataSeq|0!", xml_variant("", pic).encode()),
        (b"ImageMetadata!", xml_variant("", exp).encode()),
    ]
    (OUT / "rgb_v2.nd2").write_bytes(nd2_file("Ver2.1", meta, frames))


def box(btype: bytes, payload: bytes) -> bytes:
    return struct.pack(">I", 8 + len(payload)) + btype + payload


def legacy_xml(body: str) -> bytes:
    return ('<?xml version="1.0" encoding="UTF-8"?><variant version="1.0">' + body + "</variant>").encode()


def write_nd2_legacy():
    """Version 1 (JPEG 2000 boxes): two 12-bit channels, T(2) x Z(2), one codestream each."""
    from imagecodecs import jpeg2k_encode

    w, h, nc = 16, 12, 2
    nf = 4
    data = ramp((nf, nc, h, w), 21, 4095, np.uint16)
    boxes: list[tuple[bytes, bytes, bytes]] = []  # (box type, lim type, bytes)
    boxes.append((b"jP  ", b"jP  ", box(b"jP  ", b"\r\n\x87\n")))
    boxes.append((b"ftyp", b"ftyp", box(b"ftyp", b"jp2 \0\0\0\0jp2 ")))
    ihdr = box(b"ihdr", struct.pack(">IIHBBBB", h, w, 1, 11, 7, 0, 0))
    boxes.append((b"jp2h", b"jp2h", box(b"jp2h", ihdr)))
    planes = "".join(
        f'<a{i} runtype="CLxListVariant"><OpticalConfigName runtype="CLxStringW" value="{n}"/>'
        f'<Color runtype="lx_uint32" value="{c}"/><CompCount runtype="lx_uint32" value="1"/></a{i}>'
        for i, (n, c) in enumerate([("DAPI", 0xFF0000), ("Cy3", 0x0000FF)])
    )
    vimd = legacy_xml(
        '<Calibration runtype="double" value="0.645"/><Calibrated runtype="bool" value="true"/>'
        '<ObjectiveMag runtype="double" value="20"/><ObjectiveNA runtype="double" value="0.75"/>'
        '<ObjectiveName runtype="CLxStringW" value="Plan Apo 20x"/>'
        f'<PicturePlanes runtype="CLxListVariant"><Count runtype="lx_uint32" value="2"/>'
        f'<CompCount runtype="lx_uint32" value="2"/><Plane runtype="CLxListVariant">{planes}</Plane></PicturePlanes>'
    )
    aim = legacy_xml(
        '<Type runtype="lx_uint32" value="8"/><LoopPars runtype="CLxListVariant">'
        '<Period runtype="CLxListVariant"><a0 runtype="CLxListVariant"><Count runtype="lx_uint32" value="2"/>'
        '<Start runtype="double" value="0"/><Period runtype="double" value="500"/><Duration runtype="double" value="1000"/>'
        "</a0></Period></LoopPars>"
        '<NextLevelEx runtype="CLxListVariant"><Type runtype="lx_uint32" value="4"/><LoopPars runtype="CLxListVariant">'
        '<Count runtype="lx_uint32" value="2"/><ZStep runtype="double" value="1.5"/><ZHome runtype="double" value="0"/>'
        '<ZLow runtype="double" value="0"/><ZHigh runtype="double" value="1.5"/></LoopPars></NextLevelEx>'
    )
    desc = "Metadata:&#13;&#10;Dimensions: T(2) x λ(2) x Z(2)&#13;&#10;Camera Name: synthetic&#13;&#10;"
    tinf = legacy_xml(
        f'<TextInfoItem runtype="CLxListVariant"><Text runtype="CLxStringW" value="{desc}"/></TextInfoItem>'
        '<TextInfoItem runtype="CLxListVariant"><Text runtype="CLxStringW" value="Legacy synthetic"/></TextInfoItem>'
    )
    boxes.append((b"\0\0\0\0", b"AIM1", box(b"AIM1", aim)))
    boxes.append((b"\0\0\0\0", b"TINF", box(b"TINF", tinf)))
    for f in range(nf):
        boxes.append((b"\0\0\0\0", b"VIMD", box(b"VIMD", vimd)))
        boxes.append((b"\0\0\0\0", b"VCAL", box(b"VCAL", legacy_xml(f'<Time runtype="double" value="{500.0 * f}"/>'))))
        for c in range(nc):
            cs = jpeg2k_encode(data[f, c], level=0, codecformat="J2K", reversible=True, bitspersample=12)
            boxes.append((b"\0\0\0\0", b"LUNK", box(b"LUNK", bytes(cs))))
    out = bytearray()
    entries = []
    for bt, lt, b in boxes:
        entries.append((bt, lt, len(out)))
        out += b
    mp = struct.pack(">I", len(entries)) + b"".join(struct.pack("<4s4sI", bt, lt, off) + b"\0" * 4 for bt, lt, off in entries)
    start = len(out)
    out += mp
    total = len(out) + 40
    out += b"LABORATORY IMAGING ND BOX MAP 00" + struct.pack("<Q", total - start)
    (OUT / "legacy_v1.nd2").write_bytes(out)
    return data


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    write_lif()
    write_lof()
    write_nd2_loops()
    write_nd2_blobs()
    write_nd2_zlib()
    write_nd2_rgb_v2()
    write_nd2_legacy()
    for p in sorted(OUT.glob("*.*")):
        if p.suffix in {".lif", ".lof", ".nd2"}:
            print(f"{p.name:16} {p.stat().st_size:8} bytes")


if __name__ == "__main__":
    main()
