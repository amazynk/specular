#!/usr/bin/env python3
"""Decoded sensor data of the RAW variant corpus against independent decoders.

Compares the sensor dumps of `bench-raw` (`SPECULAR_RAW_CFA=<dump dir>`: 8 x u32 LE header
width, height, crop x, y, w, h, cpp, white; then u16 LE samples) with:

* tifffile + imagecodecs (zlib, libjpeg-turbo, libjxl) for DNG deflate, floating point, lossy
  JPEG and JPEG XL data: the stored samples decoded by the reference libraries. Floating point
  samples are compared after Specular's scaling (white level to 65535), in 16-bit steps.
* rawpy (LibRaw) `raw_image` for everything else (Canon sRAW/mRAW, Sony), which must match
  bit for bit.
* Fujifilm SuperCCD and four-colour (CYGM) sensors come out of Specular demosaiced (camera
  RGB, turned upright): compared with LibRaw's linear camera-RGB output (same size and
  alignment expected; correlation per channel, median relative difference).

    cargo run -p specular-raw --example bench-raw -- <files>   # with SPECULAR_RAW_CFA=<dumps>
    <venv>/bin/python tools/quality/raw_variants_oracle.py <dumps> [corpus dir]

X-Trans demosaic quality: a Bayer file's half-size LibRaw output (every pixel measured, no
interpolation) is the reference; it is mosaicked with the X-Trans pattern and demosaiced by
Specular (`examples/demosaic-quality.rs`) and by LibRaw (from a DNG of the same mosaic).
Prints PSNR against the reference:

    <venv>/bin/python tools/quality/raw_variants_oracle.py xtrans <demosaic-quality binary> \\
        <work dir> <raw files...>

The venv needs rawpy 0.27.1, imagecodecs and tifffile (test-only oracles). The corpus is
fetched by tools/corpus/fetch-raw-variants.sh. Prints a Markdown table.
"""

import sys
from pathlib import Path

import numpy as np

ROOT = Path(__file__).resolve().parents[2]


def load_dump(p):
    b = p.read_bytes()
    w, h, cx, cy, cw, ch, cpp, white = np.frombuffer(b[:32], "<u4")
    a = np.frombuffer(b[32:], "<u2")
    return a.reshape(h, w, cpp) if cpp > 1 else a.reshape(h, w), (cx, cy, cw, ch), int(white)


def dng_raw_page(path):
    """The raw image IFD of a DNG (the largest full-resolution CFA / LinearRaw IFD)."""
    import tifffile

    best = None
    with tifffile.TiffFile(path) as t:
        pages = list(t.pages)
        # tifffile exposes SubIFDs as `page.pages`.
        cands = []
        for p in pages:
            cands.append(p)
            cands.extend(getattr(p, "pages", None) or [])
        for p in cands:
            if p.photometric not in (32803, 34892):
                continue
            if (p.tags.valueof("NewSubfileType") or 0) & 1:
                continue
            if best is None or p.shape[0] * p.shape[1] > best[0]:
                best = (p.shape[0] * p.shape[1], p)
        if best is None:
            return None, None
        p = best[1]
        info = {
            "sampleformat": p.sampleformat,
            "bits": p.bitspersample,
            "compression": p.compression.name,
            "white": p.tags.valueof(50717),
            "lin": p.tags.valueof(50712),
        }
        return info, p.asarray()


def compare(mine, ref):
    d = mine.astype(np.int64) - ref.astype(np.int64)
    ad = np.abs(d)
    return int((d != 0).sum()), int(ad.max()), float(ad.mean())


def via_tiff(path, mine, white):
    p, ref = dng_raw_page(path)
    if p is None:
        return None
    ref = np.asarray(ref)
    if ref.ndim == 3 and mine.ndim == 2 and ref.shape[2] == 1:
        ref = ref[..., 0]
    ref = ref[: mine.shape[0], : mine.shape[1]]
    if p["sampleformat"] == 3:
        wl = p["white"]
        wl = float(np.ravel(wl)[0]) if wl is not None else 1.0
        scaled = np.clip(ref.astype(np.float64) * (65535.0 / wl), 0, 65535)
        steps = np.abs(mine.astype(np.float64) - scaled)
        # Specular rounds to the nearest 16-bit step: at most half a step off (+ f32 slack).
        bad = int((steps > 0.51).sum())
        return f"float {p['bits']}-bit vs tifffile/imagecodecs", bad, float(steps.max()), float(steps.mean())
    if mine.max() > (1 << p["bits"]) - 1:
        lin = p["lin"]
        if lin is None:
            # Mapped by OpcodeList2 (MapTable / MapPolynomial, as in Adobe's lossy DNGs):
            # compare with LibRaw, which applies them too.
            return None
        # Specular hands out linearized samples when the DNG opcodes need them.
        lin = np.asarray(lin, dtype=np.int64)
        ref = lin[np.minimum(ref.astype(np.int64), len(lin) - 1)]
    bad, mx, mean = compare(mine, ref)
    return f"{p['compression']} vs tifffile/imagecodecs", bad, mx, mean


def via_rawpy(path, mine):
    import rawpy

    r = rawpy.imread(str(path))
    ref = r.raw_image
    if mine.ndim == 3 and ref.ndim == 2:
        return via_rawpy_rgb(r, mine)
    if ref.ndim == 3:
        ref = ref[..., : mine.shape[2] if mine.ndim == 3 else 1]
        if mine.ndim == 2:
            ref = ref[..., 0]
    note = ""
    extra = (ref.shape[0] - mine.shape[0], ref.shape[1] - mine.shape[1])
    if extra != (0, 0) and 0 <= extra[0] <= 8 and 0 <= extra[1] <= 8:
        # LibRaw keeps a few coded rows / columns outside the image (ARW 1.0: the last row).
        ref = ref[: mine.shape[0], : mine.shape[1]]
        note = f" (its {extra[0]} extra rows, {extra[1]} extra columns left out)"
    if ref.shape[:2] != mine.shape[:2]:
        return f"rawpy shape {ref.shape} vs ours {mine.shape}", -1, 0, 0.0
    bad, mx, mean = compare(mine, ref)
    return "LibRaw raw_image" + note, bad, mx, mean


def linear_rgb(r, **kw):
    import rawpy

    return r.postprocess(
        gamma=(1, 1),
        no_auto_bright=True,
        output_color=rawpy.ColorSpace.raw,
        user_wb=[1, 1, 1, 1],
        output_bps=16,
        **kw,
    )


def via_rawpy_rgb(r, mine):
    """Demosaiced camera RGB (SuperCCD, CYGM) against LibRaw's linear camera RGB."""
    ref = linear_rgb(r).astype(np.float64)
    if ref.shape != mine.shape:
        return f"LibRaw RGB shape {ref.shape} vs ours {mine.shape}", -1, 0, 0.0
    m = mine[8:-8, 8:-8].astype(np.float64)
    ref = ref[8:-8, 8:-8]
    notes = []
    worst = 0.0
    for c in range(3):
        a, b = m[..., c].ravel(), ref[..., c].ravel()
        k = (a * b).sum() / max((a * a).sum(), 1.0)
        corr = np.corrcoef(a, b)[0, 1]
        rel = np.median(np.abs(a * k - b) / np.maximum(b, 64))
        worst = max(worst, rel)
        notes.append(f"{'RGB'[c]} corr {corr:.4f}")
    return "LibRaw linear RGB (demosaiced: " + ", ".join(notes) + ")", -1, 0, float(worst)


XTRANS = ["GGRGGB", "GGBGGR", "BRGRBG", "GGBGGR", "GGRGGB", "RBGBRG"]


def write_dng(path, mosaic, pattern):
    """A minimal little-endian CFA DNG: 16-bit samples, black 0, white 65535, identity colour."""
    import struct

    h, w = mosaic.shape
    data = mosaic.astype("<u2").tobytes()
    cfa = bytes("RGB".index(ch) for row in pattern for ch in row)
    n = len(pattern)
    ents = [
        (254, 4, [0]), (256, 4, [w]), (257, 4, [h]), (258, 3, [16]), (259, 3, [1]),
        (262, 3, [32803]), (271, 2, b"Specular\0"), (272, 2, b"test\0"), (273, 4, [0]),
        (277, 3, [1]), (278, 4, [h]), (279, 4, [len(data)]), (284, 3, [1]),
        (33421, 3, [n, n]), (33422, 1, cfa), (50706, 1, bytes([1, 4, 0, 0])),
        (50708, 2, b"Specular test\0"), (50710, 1, bytes([0, 1, 2])), (50711, 3, [1]),
        (50714, 4, [0]), (50717, 4, [65535]),
        (50721, 10, [1, 1, 0, 1, 0, 1, 0, 1, 1, 1, 0, 1, 0, 1, 0, 1, 1, 1]),
        (50728, 5, [1, 1, 1, 1, 1, 1]), (50778, 3, [21]),
    ]
    fmt = {3: "<H", 4: "<I"}
    ifd_off = 8
    extra_off = ifd_off + 2 + 12 * len(ents) + 4
    extra = b""
    body = b""
    strip_entry = None
    for tag, typ, val in ents:
        if typ in (1, 2):
            raw = bytes(val)
            count = len(raw)
        elif typ in (5, 10):
            count = len(val) // 2
            raw = struct.pack("<" + ("i" if typ == 10 else "I") * len(val), *val)
        else:
            count = len(val)
            raw = b"".join(struct.pack(fmt[typ], v) for v in val)
        if len(raw) <= 4:
            field = raw.ljust(4, b"\0")
        else:
            field = struct.pack("<I", extra_off + len(extra))
            extra += raw + (b"\0" if len(raw) % 2 else b"")
        if tag == 273:
            strip_entry = len(body) + 8
        body += struct.pack("<HHI", tag, typ, count) + field
    data_off = extra_off + len(extra)
    body = body[:strip_entry] + struct.pack("<I", data_off) + body[strip_entry + 4:]
    out = b"II*\0" + struct.pack("<I", ifd_off) + struct.pack("<H", len(ents)) + body
    out += b"\0\0\0\0" + extra + data
    Path(path).write_bytes(out)


def psnr(a, b):
    """PSNR of gamma 2.2 encoded values (interior, all channels) and of linear values."""
    a = np.clip(a.astype(np.float64) / 65535.0, 0, 1)[16:-16, 16:-16]
    b = np.clip(b.astype(np.float64) / 65535.0, 0, 1)[16:-16, 16:-16]
    out = []
    for f in (lambda x: x ** (1 / 2.2), lambda x: x):
        mse = ((f(a) - f(b)) ** 2).mean()
        out.append(10 * np.log10(1.0 / max(mse, 1e-12)))
    return out


def xtrans(binary, work, files):
    import subprocess

    import rawpy

    work = Path(work)
    work.mkdir(parents=True, exist_ok=True)
    print("| reference | size | Specular PSNR (gamma / linear) | LibRaw PSNR (gamma / linear) |")
    print("|---|---|---|---|")
    for f in files:
        with rawpy.imread(f) as r:
            ref = linear_rgb(r, half_size=True)
        h, w = ref.shape[0] // 6 * 6, ref.shape[1] // 6 * 6
        ref = np.ascontiguousarray(ref[:h, :w])
        refp = work / "reference.bin"
        refp.write_bytes(np.array([w, h], "<u4").tobytes() + ref.astype("<u2").tobytes())
        outp = work / "specular.bin"
        subprocess.run([binary, str(refp), str(outp)], check=True)
        mine = np.frombuffer(outp.read_bytes()[8:], "<u2").reshape(h, w, 3)
        yy, xx = np.mgrid[0:h, 0:w]
        col = np.array([["RGB".index(c) for c in row] for row in XTRANS])[yy % 6, xx % 6]
        mosaic = np.take_along_axis(ref, col[..., None], 2)[..., 0]
        dng = work / "mosaic.dng"
        write_dng(dng, mosaic, XTRANS)
        with rawpy.imread(str(dng)) as r:
            lib = linear_rgb(r)
        ours, theirs = psnr(mine, ref), psnr(lib[:h, :w], ref)
        print(f"| {Path(f).name} | {w}x{h} | {ours[0]:.2f} / {ours[1]:.2f} | {theirs[0]:.2f} / {theirs[1]:.2f} |")


def main():
    if sys.argv[1] == "xtrans":
        xtrans(sys.argv[2], sys.argv[3], sys.argv[4:])
        return
    dumps = Path(sys.argv[1])
    corpus = Path(sys.argv[2]) if len(sys.argv) > 2 else ROOT / "tools/corpus/raw-variants"
    files = {p.name: p for p in corpus.rglob("*") if p.is_file()}
    print("| file | oracle | samples | mismatches | max diff | mean diff |")
    print("|---|---|---|---|---|---|")
    for d in sorted(dumps.glob("*.cfa")):
        name = d.name[: -len(".cfa")]
        path = files.get(name)
        if path is None:
            continue
        mine, _crop, white = load_dump(d)
        res = None
        try:
            if path.suffix.lower() in (".dng", ".gpr"):
                res = via_tiff(path, mine, white)
        except Exception as e:  # noqa: BLE001 - report the oracle's failure and fall back
            print(f"<!-- tifffile failed on {name}: {e} -->")
        if res is None:
            try:
                res = via_rawpy(path, mine)
            except Exception as e:  # noqa: BLE001
                res = (f"no oracle ({e})", -1, 0, 0.0)
        oracle, bad, mx, mean = res
        print(f"| {name} | {oracle} | {mine.size} | {bad} | {mx:g} | {mean:.4f} |")


if __name__ == "__main__":
    main()
