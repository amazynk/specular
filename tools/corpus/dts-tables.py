#!/usr/bin/env python3
"""Transcribes the DTS Coherent Acoustics core tables from ETSI TS 102 114 V1.6.1 Annex D.

Input: the text of the specification, as `pdftotext -layout ts_102114v010601p.pdf` writes it,
and of V1.2.1 (ts_102114v010201p.pdf) for the rows V1.6.1 lost
(the PDF is a free download from etsi.org; it is never committed). Output: the Rust table
file crates/formats/specular-dts/src/tables.rs.

Every table is checked while it is read: Huffman code books must be complete prefix codes
over exactly their levels; numeric tables must have their stated lengths.

Usage: python3 -I dts-tables.py <ts.txt> <out.rs> <ts-v1.2.1.txt>
"""
import re
import sys
from fractions import Fraction

lines = open(sys.argv[1], encoding="utf-8").read().replace("−", "-").split("\n")


def find(title, start=0):
    for i in range(start, len(lines)):
        if lines[i].startswith(title):
            return i
    raise SystemExit("not found: " + title)


def body(a, b):
    """Lines of a clause without page headers and footers."""
    for ln in lines[find(a, 14600):find(b, 14600)]:
        if "ETSI" in ln or not ln.strip():
            continue
        yield ln


def tokens(ln):
    """Cells separated by two or more spaces; single spaces inside a cell are thousands
    separators; decimal commas become points."""
    return [c.replace(" ", "").replace(",", ".") for c in re.split(r"\s{2,}", ln.strip())]


def is_int(s):
    return re.fullmatch(r"-?\d+", s) is not None


def is_num(s):
    return re.fullmatch(r"-?\d+(\.\d+)?(e[-+]?\d+)?", s) is not None


# ---- Huffman code books (D.5) ----
def parse_huff(src, start):
    """Huffman code books of D.5 in `src` (lines), by table name."""
    i0 = next(i for i in range(start, len(src)) if src[i].startswith("D.5     Huffman"))
    i1 = next(i for i in range(i0, len(src)) if src[i].startswith("D.6     Block"))
    huff = {}
    hdr = []  # (column, name) of the table titles above the current rows
    fresh = False
    for ln in src[i0:i1]:
        if "ETSI" in ln or not ln.strip():
            continue
        found = [(m.start(), m.group(1) + m.group(2))
                 for m in re.finditer(r"Table ([A-Z]+)\.?(\d+)", ln)]
        if found:
            # Titles may sit on two lines (one table a row above its neighbours).
            hdr = (hdr if fresh else []) + found
            fresh = True
            continue
        cells = [(m.start(), m.group(0)) for m in re.finditer(r"\S+(?: \S+)*", ln)]
        cells = [(x, c.replace(" ", "")) for x, c in cells]
        if not cells or not all(is_int(c) for _, c in cells) or len(cells) % 3:
            continue
        fresh = False
        for k in range(0, len(cells), 3):
            x = cells[k][0]
            if len(hdr) == 1:
                name = hdr[0][1]
            else:
                name = min(hdr, key=lambda h: abs(h[0] - x))[1]
            huff.setdefault(name, []).append(tuple(int(c) for _, c in cells[k:k + 3]))
    return huff


huff = parse_huff(lines, 14600)
# V1.6.1 drops the last four rows of D65 (levels -32, 32, -31, 31); V1.2.1 (2002-12, also
# a free ETSI download) prints them. Its rows must agree with V1.6.1 wherever both exist.
old = parse_huff(open(sys.argv[3], encoding="utf-8").read().replace("\u2212", "-")
                 .split("\n"), 3000)
filled = []
differ = []
fixed = []
for name, rows in huff.items():
    # A level printed twice in V1.6.1 (D129 prints 37 for -37): take the level V1.2.1 gives
    # the same code word.
    by_code = {(r[1], r[2]): r[0] for r in old.get(name, [])}
    lv = [r[0] for r in rows]
    for i, r in enumerate(rows):
        if lv.count(r[0]) > 1 and by_code.get((r[1], r[2]), r[0]) != r[0]:
            rows[i] = (by_code[(r[1], r[2])], r[1], r[2])
            fixed.append("%s %d->%d" % (name, r[0], rows[i][0]))
    have = {r[0]: r for r in rows}
    for r in old.get(name, []):
        if r[0] in have:
            if have[r[0]] != r:
                differ.append("%s %s/%s" % (name, have[r[0]], r))
        else:
            rows.append(r)
            filled.append("%s %d" % (name, r[0]))
print("filled from V1.2.1:", ", ".join(filled))
print("V1.6.1/V1.2.1 differ:", ", ".join(differ))
print("misprints fixed from V1.2.1:", ", ".join(fixed))

LEVELS = {3: range(-1, 2), 4: range(0, 4), 5: range(-2, 3), 7: range(-3, 4), 9: range(-4, 5),
          12: range(1, 13), 13: range(-6, 7), 17: range(-8, 9), 25: range(-12, 13),
          33: range(-16, 17), 65: range(-32, 33), 129: range(-64, 65)}
for name, rows in huff.items():
    n = int(re.sub(r"^[A-Z]+", "", name))
    lv = sorted(r[0] for r in rows)
    assert lv == list(LEVELS[n]), (name, len(rows), [v for v in set(lv) if lv.count(v) > 1],
                                   sorted(set(LEVELS[n]) - set(lv)))
    assert sum(Fraction(1, 2 ** r[1]) for r in rows) == 1, name
    for a in rows:
        assert a[2] < 2 ** a[1], (name, a)
        for b in rows:
            if a is not b:
                l = min(a[1], b[1])
                assert a[2] >> (a[1] - l) != b[2] >> (b[1] - l), (name, a, b)

# ---- Numeric tables ----


def column_table(a, b, ncols, want, plain=False):
    """Rows of `index value...`, possibly several row groups side by side."""
    out = {}
    for ln in body(a, b):
        # An unused index ("invalid") reads as 0.
        cells = [c.replace(",", ".") for c in ln.split()] if plain else tokens(ln)
        cells = ["0" if c.lower() == "invalid" else c for c in cells]
        if not cells or not all(is_num(c) for c in cells):
            continue
        if len(cells) % (ncols + 1):
            continue
        for k in range(0, len(cells), ncols + 1):
            idx = int(cells[k])
            assert idx not in out, (a, idx)
            out[idx] = cells[k + 1:k + 1 + ncols]
    assert sorted(out) == list(range(want)), (a, len(out))
    return [out[i] for i in range(want)]


scale6 = [int(r[0]) for r in column_table("D.1.1      6-bit", "D.1.2      7-bit", 2, 64)]
scale7 = [int(r[0]) for r in column_table("D.1.2      7-bit", "D.2     Quantization", 2, 128)]
step_lossy = [int(r[0]) for r in column_table("D.2.1   Lossy", "D.2.2   Lossless", 2, 32)]
step_lossless = [int(r[0]) for r in column_table("D.2.2   Lossless", "D.3       Scale", 2, 32)]
joint = [r[0] for r in column_table("D.3       Scale", "D.4       Dynamic", 1, 129, True)]
qmf = column_table("D.8      32-Band", "D.9      1 024", 4, 512)
lfe = column_table("D.8      32-Band", "D.9      1 024", 4, 512)

out = ["""//! DTS Coherent Acoustics core tables, transcribed from ETSI TS 102 114 V1.6.1 (2019-08),
//! Annex D (a free download from etsi.org), by tools/corpus/dts-tables.py from the text of
//! the specification. Each Huffman code book was checked to be a complete prefix code over
//! its levels when transcribed. Not in the specification (\"due to its extensive size\") and
//! so not here: the ADPCM prediction (D.10.1) and high-frequency VQ (D.10.2) code books.

// The printed digits are kept as printed.
#![allow(clippy::excessive_precision)]
"""]


def arr(name, ty, vals, doc, per=8):
    if ty == "f32":
        # The printed digits, as Rust float literals.
        vals = [v if ("." in v or "e" in v) else v + ".0" for v in vals]
        vals = [re.sub(r"\.e", ".0e", v) for v in vals]
    out.append("/// " + doc)
    out.append("#[rustfmt::skip]")
    out.append("pub(crate) const %s: [%s; %d] = [" % (name, ty, len(vals)))
    for i in range(0, len(vals), per):
        out.append("    " + " ".join("%s," % v for v in vals[i:i + per]))
    out.append("];")
    out.append("")


arr("SCALE_6BIT", "u32", scale6, "D.1.1: scale factors of the 6-bit square-root quantizer.")
arr("SCALE_7BIT", "u32", scale7, "D.1.2: scale factors of the 7-bit square-root quantizer.")
arr("STEP_LOSSY", "u32", step_lossy, "D.2.1: quantization step sizes × 2^22, by ABITS.")
arr("STEP_LOSSLESS", "u32", step_lossless, "D.2.2: lossless step sizes × 2^22, by ABITS.")
arr("JOINT_SCALE", "f32", joint, "D.3: joint intensity scale factors, by index + 64.", 6)
arr("QMF_PERFECT", "f32", [r[0] for r in qmf], "D.8: 512-tap perfect-reconstruction QMF.", 4)
arr("QMF_LOSSY", "f32", [r[1] for r in qmf], "D.8: 512-tap non-perfect-reconstruction QMF.", 4)
arr("LFE_64X", "f32", [r[2] for r in lfe], "D.8: 512-tap 64× LFE interpolation FIR.", 4)
arr("LFE_128X", "f32", [r[3] for r in lfe], "D.8: 512-tap 128× LFE interpolation FIR.", 4)

out.append("/// D.5: Huffman code books as (level, code length, code), by name.")
order = sorted(huff, key=lambda s: (int(re.sub(r"^[A-Z]+", "", s)), s))
for name in order:
    rows = sorted(huff[name], key=lambda r: r[0])
    out.append("#[rustfmt::skip]")
    out.append("pub(crate) const H_%s: [(i8, u8, u16); %d] = [" % (name, len(rows)))
    for i in range(0, len(rows), 6):
        out.append("    " + " ".join("(%d, %d, %d)," % r for r in rows[i:i + 6]))
    out.append("];")
open(sys.argv[2], "w").write("\n".join(out) + "\n")
print("tables:", " ".join(order))
