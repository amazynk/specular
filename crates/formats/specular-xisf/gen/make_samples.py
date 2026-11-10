#!/usr/bin/env python3
"""Writes XISF sample files for specular-xisf's tests.

    make_samples.py OUTDIR             a corpus of .xisf files plus OUTDIR/expected.tsv
    make_samples.py --fixtures OUTDIR  the tiny committed fixtures (tests/data)
    make_samples.py --big OUTDIR       also 4096x4096 Float32 files (uncompressed, zstd+sh)

Needs numpy, xisf (the PixInsight-compatible python package, used as an independent writer
for the codec cases), lz4 and zstandard. Cases the python package cannot write (normal
storage, 3-D geometry, big-endian, sub-blocks, inline / embedded blocks, UInt64) are written
by the small writer below, straight from the XISF 1.0 specification.

expected.tsv columns: file, image, channel, slice, width, height, sum (float64 over the
float32 values), min, max, and a few `x,y=value` samples separated by spaces.
"""

import base64
import os
import sys
import zlib

import lz4.block
import numpy as np
import zstandard
from xisf import XISF

ALIGN = {"XISF:BlockAlignmentSize": {"value": 16}}


# --- data ------------------------------------------------------------------------------------


def smooth(h, w, dtype, seed=0, scale=None):
    """A smooth gradient with a little noise: compressible, but not trivially so."""
    rng = np.random.default_rng(seed)
    y, x = np.mgrid[0:h, 0:w]
    base = 0.5 + 0.3 * np.sin(x / 7.0 + seed) * np.cos(y / 5.0) + 0.1 * (x + y) / (w + h)
    base = base + rng.normal(0, 0.01, (h, w))
    base = np.clip(base, 0, 1)
    dt = np.dtype(dtype)
    if dt.kind == "f":
        return base.astype(dt)
    top = scale if scale is not None else np.iinfo(dt).max
    return np.round(base * top).astype(dt)


def levels(h, w, dtype, seed=0):
    """`smooth` quantised to 64 levels: flat runs that even LZ4 without shuffling compresses."""
    q = np.round(smooth(h, w, np.float64, seed) * 63)
    dt = np.dtype(dtype)
    return (q / 63).astype(dt) if dt.kind == "f" else (q * 1000).astype(dt)


# --- a small spec writer ---------------------------------------------------------------------


def shuffle(b, item):
    a = np.frombuffer(b, dtype=np.uint8)
    n = len(a) // item
    whole = a[: n * item].reshape(n, item).T.tobytes()
    return whole + a[n * item :].tobytes()


def compress(raw, codec, item=None, sh=False):
    data = shuffle(raw, item) if sh else raw
    if codec == "zlib":
        return zlib.compress(data, 6)
    if codec == "lz4":
        return lz4.block.compress(data, store_size=False)
    if codec == "lz4hc":
        return lz4.block.compress(data, mode="high_compression", compression=9, store_size=False)
    if codec == "zstd":
        return zstandard.ZstdCompressor(level=3).compress(data)
    raise ValueError(codec)


def esc(s):
    return s.replace("&", "&amp;").replace('"', "&quot;").replace("<", "&lt;").replace(">", "&gt;")


class Writer:
    """Builds a monolithic XISF file; attachment offsets are placeholders until `bytes()`."""

    def __init__(self):
        self.elems = []
        self.blocks = []

    def attach(self, data):
        self.blocks.append(data)
        return f"attachment:@{len(self.blocks) - 1:04d}@:{len(data)}"

    def image(self, geometry, sample, raw, storage=None, color=None, big=False, codec=None,
              sh=False, item=None, subblocks=None, where="attachment", children="", extra=""):
        attrs = [f'geometry="{geometry}"', f'sampleFormat="{sample}"']
        if storage:
            attrs.append(f'pixelStorage="{storage}"')
        if color:
            attrs.append(f'colorSpace="{color}"')
        if big:
            attrs.append('byteOrder="big"')
        if sample.startswith("Float"):
            attrs.append('bounds="0:1"')
        block = raw
        if subblocks:
            parts, sizes, at = [], [], 0
            data = shuffle(raw, item) if sh else raw
            for n in subblocks:
                piece = data[at : at + n]
                at += n
                c = compress(piece, codec)
                parts.append(c)
                sizes.append(f"{len(c)},{len(piece)}")
            block = b"".join(parts)
            # Shuffled as a whole block, then cut into sub-blocks.
            name = codec + ("+sh" if sh else "")
            comp = f"{name}:{len(raw)}" + (f":{item}" if sh else "")
            attrs.append(f'compression="{comp}" subblocks="{":".join(sizes)}"')
        elif codec:
            block = compress(raw, codec, item, sh)
            name = codec + ("+sh" if sh else "")
            comp = f"{name}:{len(raw)}" + (f":{item}" if sh else "")
            attrs.append(f'compression="{comp}"')
        body = children
        if where == "attachment":
            attrs.append(f'location="{self.attach(block)}"')
        elif where == "inline-base64":
            attrs.append('location="inline:base64"')
            body = base64.b64encode(block).decode() + children
        elif where == "inline-hex":
            attrs.append('location="inline:hex"')
            body = block.hex() + children
        elif where == "embedded":
            attrs.append('location="embedded"')
            body = f'<Data encoding="base64">{base64.b64encode(block).decode()}</Data>' + children
        if extra:
            attrs.append(extra)
        self.elems.append(f"<Image {' '.join(attrs)}>{body}</Image>")

    def raw(self, xml):
        self.elems.append(xml)

    def bytes(self):
        xml = (
            '<?xml version="1.0" encoding="UTF-8"?>\n'
            '<xisf version="1.0" xmlns="http://www.pixinsight.com/xisf">'
            + "".join(self.elems)
            + "</xisf>"
        )
        # Offsets change the header's length, so lay out until they are stable.
        offsets = [0] * len(self.blocks)
        while True:
            filled = xml
            for i, o in enumerate(offsets):
                filled = filled.replace(f"@{i:04d}@", str(o))
            hdr = filled.encode()
            pos, new = 16 + len(hdr), []
            for b in self.blocks:
                pos = (pos + 15) // 16 * 16
                new.append(pos)
                pos += len(b)
            if new == offsets:
                break
            offsets = new
        out = bytearray(b"XISF0100" + len(hdr).to_bytes(4, "little") + bytes(4) + hdr)
        for o, b in zip(offsets, self.blocks):
            out += bytes(o - len(out))
            out += b
        return bytes(out)


# --- expectations ----------------------------------------------------------------------------


def planes(arr):
    """arr: (channels, depth, h, w) → [(c, z, plane)]."""
    out = []
    for c in range(arr.shape[0]):
        for z in range(arr.shape[1]):
            out.append((c, z, arr[c, z]))
    return out


def tsv_rows(name, index, arr):
    rows = []
    for c, z, p in planes(arr):
        f = p.astype(np.float32)
        h, w = f.shape
        pts = [(0, 0), (w - 1, 0), (0, h - 1), (w - 1, h - 1), (w // 2, h // 2), (w // 3, h // 5)]
        s = " ".join(f"{x},{y}={repr(float(f[y, x]))}" for x, y in pts)
        rows.append(
            f"{name}\t{index}\t{c}\t{z}\t{w}\t{h}\t{repr(float(f.astype(np.float64).sum()))}"
            f"\t{repr(float(f.min()))}\t{repr(float(f.max()))}\t{s}"
        )
    return rows


# --- cases -----------------------------------------------------------------------------------

CODECS = ["zlib", "lz4", "lz4hc", "zstd"]


def write_python(path, hwc, codec=None, sh=False, meta=None):
    """Writes with the python xisf package; returns the codec it actually used."""
    _, used = XISF.write(path, hwc, creator_app="specular make_samples", image_metadata=meta or {},
                         xisf_metadata=dict(ALIGN), codec=codec, shuffle=sh)
    if codec and used is None:
        raise SystemExit(f"{path}: the python writer refused to compress (data too noisy)")
    return used


def fixtures(out):
    """Tiny files for tests/data: every codec, with and without shuffling, via python xisf."""
    os.makedirs(out, exist_ok=True)
    # Few levels with flat runs, so that even LZ4 without shuffling finds matches.
    a = (smooth(16, 24, np.uint16, seed=1, scale=15) * 64).astype(np.uint16)
    for codec in CODECS:
        for sh in (False, True):
            name = f"{codec}{'_sh' if sh else ''}.xisf"
            write_python(os.path.join(out, name), a[:, :, None], codec, sh)
    with open(os.path.join(out, "expected_u16.bin"), "wb") as f:
        f.write(a.astype("<u2").tobytes())


def corpus(out, big):
    os.makedirs(out, exist_ok=True)
    rows = []

    def py(name, hwc, codec=None, sh=False, meta=None):
        write_python(os.path.join(out, name), hwc, codec, sh, meta)
        rows.extend(tsv_rows(name, 0, np.transpose(hwc, (2, 0, 1))[:, None]))

    def own(name, w):
        with open(os.path.join(out, name), "wb") as f:
            f.write(w.bytes())

    # python xisf: sample formats, codecs, RGB.
    py("py_u8_gray.xisf", smooth(64, 80, np.uint8, 2)[:, :, None])
    py("py_u16_gray.xisf", smooth(64, 80, np.uint16, 3)[:, :, None])
    py("py_u32_gray.xisf", smooth(48, 50, np.uint32, 4)[:, :, None])
    py("py_f32_gray.xisf", smooth(64, 80, np.float32, 5)[:, :, None])
    py("py_f64_gray.xisf", smooth(40, 30, np.float64, 6)[:, :, None])
    rgb = np.stack([smooth(60, 90, np.uint16, s) for s in (7, 8, 9)], axis=2)
    py("py_u16_rgb.xisf", rgb)
    rgbf = np.stack([smooth(60, 90, np.float32, s) for s in (10, 11, 12)], axis=2)
    py("py_f32_rgb_zstd_sh.xisf", rgbf, "zstd", True)
    for codec in CODECS:
        for sh in (False, True):
            suffix = f"{codec}{'_sh' if sh else ''}"
            py(f"py_u16_{suffix}.xisf", levels(100, 120, np.uint16, 13)[:, :, None], codec, sh)
            py(f"py_f32_{suffix}.xisf", levels(100, 120, np.float32, 14)[:, :, None], codec, sh)

    # A TAN WCS and some keywords / properties.
    wcs = {
        "SIMPLE": [{"value": "T", "comment": ""}],
        "OBJECT": [{"value": "'M31     '", "comment": "target"}],
        "CTYPE1": [{"value": "'RA---TAN'", "comment": ""}],
        "CTYPE2": [{"value": "'DEC--TAN'", "comment": ""}],
        "CRVAL1": [{"value": "10.684708", "comment": "RA of reference pixel [deg]"}],
        "CRVAL2": [{"value": "41.26875", "comment": "Dec of reference pixel [deg]"}],
        "CRPIX1": [{"value": "40.5", "comment": ""}],
        "CRPIX2": [{"value": "32.5", "comment": ""}],
        "CD1_1": [{"value": "-0.000277778", "comment": ""}],
        "CD1_2": [{"value": "0.0", "comment": ""}],
        "CD2_1": [{"value": "0.0", "comment": ""}],
        "CD2_2": [{"value": "0.000277778", "comment": ""}],
        "EXPTIME": [{"value": "300.0", "comment": "[s]"}],
    }
    props = {
        "Observation:Object:Name": {"id": "Observation:Object:Name", "type": "String", "value": "M31"},
        "Instrument:ExposureTime": {"id": "Instrument:ExposureTime", "type": "Float32", "value": 300.0},
    }
    py("py_wcs_tan.xisf", smooth(64, 80, np.uint16, 15)[:, :, None],
       meta={"FITSKeywords": wcs, "XISFProperties": props})

    # Own writer: what the python package cannot write.
    def raw_of(arr_czhw, storage, big=False):
        """(c, z, h, w) array → block bytes in the given storage and byte order."""
        a = arr_czhw
        if storage == "Normal":
            a = np.transpose(a, (1, 2, 3, 0))  # z, h, w, c
        dt = a.dtype.newbyteorder(">" if big else "<")
        return np.ascontiguousarray(a).astype(dt).tobytes()

    def own_case(name, arr, sample, storage=None, color=None, big=False, **kw):
        c, z, h, w = arr.shape
        geom = f"{w}:{h}" + (f":{z}" if z > 1 else "") + f":{c}"
        wr = Writer()
        wr.image(geom, sample, raw_of(arr, storage or "Planar", big), storage, color, big, **kw)
        own(name, wr)
        rows.extend(tsv_rows(name, 0, arr))

    rgb8 = np.stack([smooth(30, 40, np.uint8, s) for s in (16, 17, 18)])[:, None]
    own_case("own_u8_rgb_normal.xisf", rgb8, "UInt8", "Normal", "RGB")
    rgb16 = np.stack([smooth(30, 40, np.uint16, s) for s in (19, 20, 21)])[:, None]
    own_case("own_u16_rgb_normal_zstd_sh.xisf", rgb16, "UInt16", "Normal", "RGB", codec="zstd",
             sh=True, item=2)
    own_case("own_u16_be.xisf", smooth(33, 47, np.uint16, 22)[None, None], "UInt16", big=True)
    own_case("own_f32_be_lz4_sh.xisf", smooth(33, 47, np.float32, 23)[None, None], "Float32",
             big=True, codec="lz4", sh=True, item=4)
    own_case("own_f64_be.xisf", smooth(20, 21, np.float64, 24)[None, None], "Float64", big=True)
    u64 = (smooth(25, 26, np.uint32, 25).astype(np.uint64) * 1000)[None, None]
    own_case("own_u64.xisf", u64, "UInt64")
    cube = np.stack([smooth(16, 18, np.uint16, s) for s in range(26, 30)])[None]
    own_case("own_u16_cube_planar.xisf", cube, "UInt16")
    cube2 = np.stack([np.stack([smooth(10, 12, np.float32, 30 + 4 * c + s) for s in range(3)])
                      for c in range(2)])
    own_case("own_f32_cube_normal_2ch.xisf", cube2, "Float32", "Normal")
    sub = smooth(64, 64, np.uint16, 31)[None, None]
    own_case("own_u16_zlib_subblocks.xisf", sub, "UInt16", codec="zlib",
             subblocks=[3000, 3000, 2192])
    own_case("own_u16_lz4_sh_subblocks.xisf", sub, "UInt16", codec="lz4", sh=True, item=2,
             subblocks=[4096, 4096])
    small = smooth(6, 7, np.uint16, 32)[None, None]
    own_case("own_inline_base64.xisf", small, "UInt16", where="inline-base64")
    own_case("own_inline_hex.xisf", small, "UInt16", where="inline-hex")
    own_case("own_embedded_zlib.xisf", small, "UInt16", where="embedded", codec="zlib")

    # Two images in one file, one behind a <Reference>.
    wr = Writer()
    a = smooth(20, 22, np.uint8, 33)[None, None]
    b = smooth(11, 13, np.float32, 34)[None, None]
    wr.image("22:20:1", "UInt8", raw_of(a, "Planar"), extra='id="first"')
    wr.raw('<Metadata><Property id="XISF:CreatorApplication" type="String">make_samples'
           '</Property></Metadata>')
    wr.image("13:11:1", "Float32", raw_of(b, "Planar"), extra='id="second"')
    own("own_two_images.xisf", wr)
    rows.extend(tsv_rows("own_two_images.xisf", 0, a))
    rows.extend(tsv_rows("own_two_images.xisf", 1, b))

    if big:
        f = smooth(4096, 4096, np.float32, 40)
        py("big_f32.xisf", f[:, :, None])
        py("big_f32_zstd_sh.xisf", f[:, :, None], "zstd", True)

    with open(os.path.join(out, "expected.tsv"), "w") as fh:
        fh.write("file\timage\tchannel\tslice\twidth\theight\tsum\tmin\tmax\tsamples\n")
        fh.write("\n".join(rows) + "\n")


def main():
    args = sys.argv[1:]
    if len(args) == 2 and args[0] == "--fixtures":
        fixtures(args[1])
    elif len(args) == 2 and args[0] == "--big":
        corpus(args[1], True)
    elif len(args) == 1:
        corpus(args[0], False)
    else:
        raise SystemExit(__doc__)


if __name__ == "__main__":
    main()
