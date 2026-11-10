#!/usr/bin/env python3
"""Regenerates the grid seam fixtures (grid420-*.avif, grid422.avif) in this directory.

Each fixture is a pair: `<name>.avif`, a 2x2 `grid` of lossless libaom tiles, and
`<name>.single.avif`, the same canvas encoded losslessly as one item. Both carry identical
Y'CbCr samples (BT.601, limited range): a mild luma ramp and steep Cb/Cr ramps that cross both
tile seams, so a decoder that upsamples chroma per tile (clamping at each tile's edge) shows
seams that the single item does not have. A correct decoder gives the two files the same
pixels. `chroma_sample_position` is patched in as in make_siting.py.

Usage: make_grid.py [out_dir]   (default: this directory). Needs ffmpeg with libaom-av1.
Made with ffmpeg 8.0.1 / libaom 3.13.1.
"""

import pathlib
import subprocess
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from make_siting import set_csp  # noqa: E402

# (name, canvas w, canvas h, tile w, tile h, 4:2:0?, chroma_sample_position). Tiles are
# 64x64, the smallest MIAF (and libavif) allows.
FIXTURES = [
    ("grid420-unknown", 128, 128, 64, 64, True, 0),
    ("grid420-colocated", 128, 128, 64, 64, True, 2),
    # Right and bottom tiles cropped by the canvas (odd canvas size).
    ("grid420-cropped", 126, 118, 64, 64, True, 0),
    ("grid422", 128, 128, 64, 64, False, 0),
]


def planes(w, h, is420):
    """Y, Cb, Cr sample lists (row-major) of a w x h picture."""
    cw, ch = (w + 1) // 2, ((h + 1) // 2 if is420 else h)
    y = [[126] * w for _ in range(h)]
    # Steep ramps of 7 code values per chroma sample (wrapping away from the seams), Cb along
    # x, Cr along y.
    u = [[60 + (7 * cx) % 140 for cx in range(cw)] for cy in range(ch)]
    v = [[200 - (7 * cy) % 140 for cx in range(cw)] for cy in range(ch)]
    return y, u, v


def crop(p, x0, y0, w, h):
    return [row[x0 : x0 + w] for row in p[y0 : y0 + h]]


def encode(y, u, v, is420, csp):
    h, w = len(y), len(y[0])
    raw = bytes(s for plane in (y, u, v) for row in plane for s in row)
    tmp = OUT / "grid.tmp.avif"  # the AVIF muxer needs a seekable output
    cmd = ["ffmpeg", "-v", "error", "-y", "-f", "rawvideo",
           "-pix_fmt", "yuv420p" if is420 else "yuv422p", "-s", f"{w}x{h}", "-i", "-",
           "-frames:v", "1", "-color_range", "tv", "-colorspace", "smpte170m",
           "-color_primaries", "smpte170m", "-color_trc", "smpte170m",
           "-c:v", "libaom-av1", "-aom-params", "lossless=1", "-still-picture", "1",
           "-f", "avif", str(tmp)]
    subprocess.run(cmd, input=raw, check=True)
    avif = tmp.read_bytes()
    tmp.unlink()
    return set_csp(avif, csp) if csp else avif


def find_box(data, typ):
    i = data.find(typ) - 4
    assert i >= 0, typ
    return data[i : i + int.from_bytes(data[i : i + 4], "big")]


def item_data(avif):
    """The payload of the single item in `mdat`."""
    mdat = find_box(avif, b"mdat")
    return mdat[8:]


def box(typ, payload):
    return (8 + len(payload)).to_bytes(4, "big") + typ + payload


def full(typ, version, flags, payload):
    return box(typ, bytes([version]) + flags.to_bytes(3, "big") + payload)


def u16(x):
    return x.to_bytes(2, "big")


def u32(x):
    return x.to_bytes(4, "big")


def grid_file(w, h, cols, rows, tiles, av1c, colr):
    """A HEIF file whose primary item (1) is a cols x rows grid of the AV1 `tiles` (items 2..)."""
    grid = bytes([0, 0, rows - 1, cols - 1]) + u16(w) + u16(h)
    datas = [grid] + tiles
    ids = list(range(1, len(datas) + 1))

    def meta(offsets):
        hdlr = full(b"hdlr", 0, 0, u32(0) + b"pict" + bytes(12) + b"\0")
        pitm = full(b"pitm", 0, 0, u16(1))
        iloc = full(b"iloc", 0, 0, bytes([0x44, 0x00]) + u16(len(datas)) + b"".join(
            u16(i) + u16(0) + u16(1) + u32(o) + u32(len(d))
            for i, o, d in zip(ids, offsets, datas)))
        infe = [full(b"infe", 2, 0, u16(1) + u16(0) + b"grid" + b"\0")] + [
            full(b"infe", 2, 1, u16(i) + u16(0) + b"av01" + b"\0") for i in ids[1:]]
        iinf = full(b"iinf", 0, 0, u16(len(infe)) + b"".join(infe))
        dimg = box(b"dimg", u16(1) + u16(len(tiles)) + b"".join(u16(i) for i in ids[1:]))
        iref = full(b"iref", 0, 0, dimg)
        tw, th = TILE
        ipco = box(b"ipco", full(b"ispe", 0, 0, u32(w) + u32(h))
                   + full(b"ispe", 0, 0, u32(tw) + u32(th)) + av1c + colr
                   + full(b"pixi", 0, 0, bytes([3, 8, 8, 8])))
        # Properties 1: canvas ispe, 2: tile ispe, 3: av1C (essential), 4: colr, 5: pixi.
        assoc = [u16(1) + bytes([3, 1, 4, 5])] + [
            u16(i) + bytes([3, 2, 0x83, 5]) for i in ids[1:]]
        ipma = full(b"ipma", 0, 0, u32(len(assoc)) + b"".join(assoc))
        return box(b"meta", bytes(4) + hdlr + pitm + iloc + iinf + iref
                   + box(b"iprp", ipco + ipma))

    ftyp = box(b"ftyp", b"avif" + u32(0) + b"avifmif1miaf")
    head = len(ftyp) + len(meta([0] * len(datas))) + 8
    offsets, pos = [], head
    for d in datas:
        offsets.append(pos)
        pos += len(d)
    return ftyp + meta(offsets) + box(b"mdat", b"".join(datas))


def main():
    global OUT, TILE
    OUT = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else pathlib.Path(__file__).parent)
    for name, w, h, tw, th, is420, csp in FIXTURES:
        TILE = (tw, th)
        cols, rows = (w + tw - 1) // tw, (h + th - 1) // th
        y, u, v = planes(cols * tw, rows * th, is420)
        sy = 2 if is420 else 1
        tiles, av1c, colr = [], None, None
        for r in range(rows):
            for c in range(cols):
                t = encode(crop(y, c * tw, r * th, tw, th),
                           crop(u, c * tw // 2, r * th // sy, tw // 2, th // sy),
                           crop(v, c * tw // 2, r * th // sy, tw // 2, th // sy), is420, csp)
                av1c, colr = av1c or find_box(t, b"av1C"), colr or find_box(t, b"colr")
                tiles.append(item_data(t))
        grid = grid_file(w, h, cols, rows, tiles, av1c, colr)
        cw, ch = (w + 1) // 2, ((h + 1) // 2 if is420 else h)
        single = encode(crop(y, 0, 0, w, h), crop(u, 0, 0, cw, ch), crop(v, 0, 0, cw, ch),
                        is420, csp)
        (OUT / f"{name}.avif").write_bytes(grid)
        (OUT / f"{name}.single.avif").write_bytes(single)
        print(f"{name}.avif: {len(grid)} bytes, {name}.single.avif: {len(single)} bytes")


if __name__ == "__main__":
    main()
