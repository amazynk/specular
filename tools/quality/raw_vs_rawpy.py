#!/usr/bin/env python3
"""Linear RAW develop against rawpy (LibRaw) per camera, as CIEDE2000.

Reads the dumps of `cargo test -p specular-raw --test rawpy_dump -- --ignored` (half-size
develop, as-shot white balance, linear Rec.709, 2x2 means) and develops each file with
rawpy the same way (half_size, camera WB, linear gamma, no auto-brightness, sRGB primaries,
no flip, 2x2 means). The two are aligned (crop offsets differ between decoders by a few
pixels), one exposure gain is fitted (LibRaw scales to its own white point), unclipped
pixels are compared in CIELAB with the 99th-percentile luminance as white.

    target/quality/venv/bin/python tools/quality/raw_vs_rawpy.py <dump dir> [--keep]

Prints a Markdown table and writes <dump dir>/report.md. Dumps are deleted unless --keep.
"""

import os
import sys
from pathlib import Path

import numpy as np
import rawpy
import colour

ROOT = Path(__file__).resolve().parents[2]
CORPUS = Path(os.environ.get("SPECULAR_RAW_CORPUS", ROOT / "tools/corpus/large/raw"))
M = np.array(
    [[0.4124564, 0.3575761, 0.1804375], [0.2126729, 0.7151522, 0.0721750], [0.0193339, 0.1191920, 0.9503041]]
)
D65 = np.array([0.95047, 1.0, 1.08883])


def reduce2(a):
    h, w = a.shape[0] // 2 * 2, a.shape[1] // 2 * 2
    a = a[:h, :w].astype(np.float64)
    return (a[0::2, 0::2] + a[1::2, 0::2] + a[0::2, 1::2] + a[1::2, 1::2]) / 4


def lab(rgb):
    xyz = rgb @ M.T
    return colour.XYZ_to_Lab(xyz, colour.XYZ_to_xy(D65))


def align(a, b, r=12):
    """Shift (dy, dx) of b relative to a that best matches luminance near the centre."""
    ya, yb = a @ M[1], b @ M[1]
    h, w = min(ya.shape[0], yb.shape[0]), min(ya.shape[1], yb.shape[1])
    s = min(h, w, 600) - 2 * r - 2
    cy, cx = h // 2, w // 2
    pa = np.log(ya[cy - s // 2 : cy + s // 2, cx - s // 2 : cx + s // 2] + 1e-4)
    best = (np.inf, 0, 0)
    for dy in range(-r, r + 1):
        for dx in range(-r, r + 1):
            y0, x0 = cy - s // 2 + dy, cx - s // 2 + dx
            if y0 < 0 or x0 < 0:
                continue
            pb = np.log(yb[y0 : y0 + s, x0 : x0 + s] + 1e-4)
            if pb.shape != pa.shape:
                continue
            d = pa - pb
            e = np.mean((d - d.mean()) ** 2)
            if e < best[0]:
                best = (e, dy, dx)
    return best[1], best[2]


def compare(ours, theirs):
    dy, dx = align(ours, theirs)
    # Overlap of ours[y, x] with theirs[y + dy, x + dx].
    h = min(ours.shape[0], theirs.shape[0] - dy) - max(0, -dy)
    w = min(ours.shape[1], theirs.shape[1] - dx) - max(0, -dx)
    oy, ox = max(0, -dy), max(0, -dx)
    a = ours[oy : oy + h, ox : ox + w]
    b = theirs[oy + dy : oy + dy + h, ox + dx : ox + dx + w]
    ya, yb = a @ M[1], b @ M[1]
    ok = (a.max(axis=2) < 0.9) & (b.max(axis=2) < 0.9) & (ya > 0.005) & (yb > 0.005)
    if ok.sum() < 1000:
        return None
    gain = np.median(yb[ok] / ya[ok])
    a = a * gain
    white = np.percentile(yb[ok], 99)
    la, lb = lab(a[ok] / white), lab(b[ok] / white)
    de = colour.delta_E(la, lb, method="CIE 2000")
    # White balance agreement: mean chromaticity ratio R/G and B/G.
    ra = a[ok].mean(axis=0)
    rb = b[ok].mean(axis=0)
    return {
        "shift": (dy, dx),
        "size": (ours.shape[1], ours.shape[0], theirs.shape[1], theirs.shape[0]),
        "gain": gain,
        "mean": float(de.mean()),
        "p95": float(np.percentile(de, 95)),
        "wb": (ra[0] / ra[1] / (rb[0] / rb[1]), ra[2] / ra[1] / (rb[2] / rb[1])),
        "n": int(ok.sum()),
    }


def main():
    dump = Path(sys.argv[1])
    keep = "--keep" in sys.argv
    rows = []
    lines = (dump / "manifest.tsv").read_text().splitlines()[1:]
    for line in lines:
        n, rel, w, h, matrix, wb, ms, *_ = line.split("\t")
        f = dump / f"{n}.rgb"
        if not f.exists():
            continue
        raw = np.fromfile(f, dtype="<u2", offset=8)
        ours = raw.reshape(int(h), int(w), 3).astype(np.float64) / 65535
        try:
            with rawpy.imread(str(CORPUS / rel)) as r:
                rgb = r.postprocess(
                    half_size=True,
                    use_camera_wb=True,
                    no_auto_bright=True,
                    gamma=(1, 1),
                    output_bps=16,
                    output_color=rawpy.ColorSpace.sRGB,
                    user_flip=0,
                )
            theirs = reduce2(rgb) / 65535
            # Linear (demosaiced) DNGs: we halve them at half size, LibRaw does not.
            if theirs.shape[0] >= 2 * ours.shape[0] - 2 and theirs.shape[1] >= 2 * ours.shape[1] - 2:
                theirs = reduce2(theirs)
            res = compare(ours, theirs)
            note = "" if res else "too few unclipped pixels"
        except Exception as e:  # LibRaw refusals (unsupported cameras) are reported.
            res, note = None, f"rawpy: {type(e).__name__}"
        rows.append((rel, matrix, wb, ms, res, note))
        if not keep:
            f.unlink()
        print(rel, res or note, flush=True)
    out = ["| file | matrix | WB | ours ms | shift | gain | R/G, B/G vs LibRaw | mean ΔE00 | p95 ΔE00 |", "|---|---|---|---|---|---|---|---|---|"]
    des = []
    for rel, matrix, wb, ms, res, note in rows:
        if res:
            des.append(res["mean"])
            out.append(
                f"| {rel} | {matrix} | {wb} | {ms} | {res['shift']} | {res['gain']:.3f} | "
                f"{res['wb'][0]:.3f}, {res['wb'][1]:.3f} | {res['mean']:.2f} | {res['p95']:.2f} |"
            )
        else:
            out.append(f"| {rel} | {matrix} | {wb} | {ms} | | | | {note} | |")
    if des:
        d = np.array(des)
        out.append("")
        out.append(
            f"{len(d)} files compared: median of per-file mean ΔE00 {np.median(d):.2f}, "
            f"{(d < 2).sum()} under 2, {(d < 5).sum()} under 5, worst {d.max():.2f}."
        )
    text = "\n".join(out) + "\n"
    (dump / "report.md").write_text(text)
    print(text)


if __name__ == "__main__":
    main()
