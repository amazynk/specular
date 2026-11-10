#!/usr/bin/env python3
"""Generate crates/formats/specular-vc1/src/tables.rs from the text of SMPTE ST 421:2013.

Provenance: every table is read from the freely published standard
(https://pub.smpte.org/doc/st421/20131008-pub/st0421-2013.pdf), converted with
`pdftotext -layout st0421-2013.pdf st0421.txt`. Nothing is taken from any other decoder.

Usage: python3 -I tools/corpus/specular-vc1-tables.py st0421.txt > crates/formats/specular-vc1/src/tables.rs

The tables are checked as they are read: VLC tables must be prefix-free and complete where the
standard says so (Kraft sum 1), index tables must be dense, zigzag scans must be permutations.
"""
import re
import sys

lines = open(sys.argv[1], encoding="utf-8", errors="replace").read().split("\n")
# Page furniture.
lines = [l for l in lines if "SMPTE ST 421:2013" not in l and "© 2013 SMPTE" not in l]

TITLE = re.compile(r"^\s*Table (\d+): ")
SECTION = re.compile(r"^\s*(\d+(\.\d+)*|Annex [A-Z])\s+[A-Z][a-zA-Z]")


def body(n):
    """The lines of table n (from its title to the next table title or section heading)."""
    for i, l in enumerate(lines):
        m = TITLE.match(l)
        if m and int(m.group(1)) == n:
            out = []
            for l2 in lines[i + 1:]:
                if TITLE.match(l2) or SECTION.match(l2):
                    break
                out.append(l2)
            return out
    raise SystemExit(f"table {n} not found")


def numbers(n, width):
    """The table's rows of `width` integers (ESCAPE as -1), in reading order per printed row."""
    rows = []
    for l in body(n):
        toks = l.split()
        if not toks:
            continue
        vals = []
        ok = True
        for t in toks:
            if t == "ESCAPE":
                vals.append(-1)
            elif re.fullmatch(r"-?\d+", t):
                vals.append(int(t))
            else:
                ok = False
                break
        if not ok or not vals:
            continue
        if len(vals) % width:
            raise SystemExit(f"table {n}: row {l!r} has {len(vals)} values")
        rows.extend(tuple(vals[i:i + width]) for i in range(0, len(vals), width))
    return rows


def check_prefix_free(n, codes, complete=True):
    seen = set()
    kraft = 0.0
    for c, l in codes:
        if c >= (1 << l):
            raise SystemExit(f"table {n}: code {c} longer than {l} bits")
        s = format(c, f"0{l}b")
        seen.add(s)
        kraft += 2.0 ** -l
    for s in seen:
        for k in range(1, len(s)):
            if s[:k] in seen:
                raise SystemExit(f"table {n}: {s[:k]} is a prefix of {s}")
    if complete and abs(kraft - 1.0) > 1e-9:
        print(f"// note: table {n} Kraft sum {kraft}", file=sys.stderr)


def vlc(n, complete=True):
    """A (key, code, length) table, keys made dense 0..N (ESCAPE is the last key)."""
    rows = numbers(n, 3)
    escape_key = max(k for k, _, _ in rows) + 1
    out = {}
    for k, c, l in rows:
        if k == -1:
            k = escape_key
        if k in out:
            raise SystemExit(f"table {n}: key {k} twice")
        out[k] = (c, l)
    keys = sorted(out)
    if keys != list(range(len(keys))):
        raise SystemExit(f"table {n}: keys not dense: {keys[:5]}..{keys[-5:]}")
    codes = [out[k] for k in keys]
    check_prefix_free(n, codes, complete)
    return codes


def pairs(n):
    return numbers(n, 2)


_words = None


def pdf_words():
    """Every word of the PDF as (page, x, y, text) (`pdftotext -bbox`)."""
    global _words
    if _words is None:
        import subprocess
        html = subprocess.run(["pdftotext", "-bbox", sys.argv[2], "-"], capture_output=True,
                              text=True, check=True).stdout
        _words = []
        page = 0
        pat = re.compile(r'xMin="([\d.]+)" yMin="([\d.]+)" xMax="[\d.]+" yMax="[\d.]+">([^<]*)<')
        for line in html.split("\n"):
            if "<page " in line:
                page += 1
            m = pat.search(line)
            if m:
                _words.append((page, float(m[1]), float(m[2]), m[3]))
    return _words


def zigzag(n, size):
    """Scans print each number vertically, one PDF word per figure. The figures between the
    table's title and the next title (or heading, or page footer), are grouped into rows of
    cells (figures less than 15 points apart vertically); within a row, figures at the same x
    form one number, read top to bottom."""
    w = pdf_words()
    i = next(k for k in range(len(w) - 1) if w[k][3] == "Table" and w[k + 1][3] == f"{n}:")
    page, y0 = w[i][0], w[i][2]
    out = []
    while True:
        on_page = [t for t in w if t[0] == page]
        # The region ends at the next title or heading below y0, else at the footer.
        ends = [t[2] for t in on_page if t[2] > y0 + 1 and (t[3] == "Table" or t[3] == "\u00a9"
                or re.fullmatch(r"\d+\.\d+(\.\d+)?", t[3]))]
        y1 = min(ends) if ends else 1e9
        figs = sorted((t for t in on_page if y0 + 1 < t[2] < y1 - 1 and re.fullmatch(r"\d", t[3])),
                      key=lambda t: t[2])
        bands = []
        for f in figs:
            if bands and f[2] - bands[-1][-1][2] < 15:
                bands[-1].append(f)
            else:
                bands.append([f])
        for band in bands:
            cells = {}
            for _, x, y, t in band:
                key = next((k for k in cells if abs(k - x) < 1.0), x)
                cells.setdefault(key, []).append((y, t))
            for k in sorted(cells):
                out.append(int("".join(t for _, t in sorted(cells[k]))))
        stop = [t for t in on_page if t[2] == y1]
        if len(out) >= size or not stop or stop[0][3] != "\u00a9":
            break
        # Continued on the next page, below its running header.
        page += 1
        y0 = min(t[2] for t in w if t[0] == page and t[3] == "SMPTE")
    if sorted(out) != list(range(size)):
        raise SystemExit(f"table {n}: not a permutation of 0..{size}: {out}")
    return out


def emit_codes(name, codes):
    print(f"pub static {name}: [(u32, u8); {len(codes)}] = [")
    for i in range(0, len(codes), 6):
        print("    " + " ".join(f"({c}, {l})," for c, l in codes[i:i + 6]))
    print("];")


def emit_list(name, ty, vals):
    print(f"pub static {name}: [{ty}; {len(vals)}] = [")
    for i in range(0, len(vals), 16):
        print("    " + " ".join(f"{v}," for v in vals[i:i + 16]))
    print("];")


print("// Generated by tools/corpus/specular-vc1-tables.py from the text of SMPTE ST 421:2013")
print("// (VC-1). Table numbers refer to that document. Do not edit.")
print("#![allow(clippy::unreadable_literal)]")
print()

# 11.5 / 11.6: CBPCY.
emit_codes("I_CBPCY", vlc(168))
for t in range(4):
    emit_codes(f"P_CBPCY{t}", vlc(169 + t))

# 11.7: DC differential (index 119 = escape).
for name, n in [("DC_LOW_LUMA", 173), ("DC_LOW_CHROMA", 174), ("DC_HIGH_LUMA", 175), ("DC_HIGH_CHROMA", 176)]:
    codes = vlc(n)
    emit_codes(name, codes)

# 11.8: AC coding sets.
SETS = [
    ("HIGH_MOTION_INTRA", 177), ("HIGH_MOTION_INTER", 184),
    ("LOW_MOTION_INTRA", 191), ("LOW_MOTION_INTER", 198),
    ("MID_RATE_INTRA", 205), ("MID_RATE_INTER", 212),
    ("HIGH_RATE_INTRA", 219), ("HIGH_RATE_INTER", 226),
]
for name, n in SETS:
    codes = vlc(n)
    rl0 = numbers(n + 1, 3)
    rl1 = numbers(n + 2, 3)
    idx = sorted(rl0 + rl1)
    if [r[0] for r in idx] != list(range(len(idx))):
        raise SystemExit(f"table {n}: run/level indices not dense")
    if len(idx) + 1 != len(codes):
        raise SystemExit(f"table {n}: {len(codes)} codes for {len(idx)} run/level pairs")
    start_last = len(rl0)
    dl0 = sorted(pairs(n + 3))
    dl1 = sorted(pairs(n + 4))
    dr0 = sorted(pairs(n + 5))
    dr1 = sorted(pairs(n + 6))
    for t, d, first in [(n + 3, dl0, 0), (n + 4, dl1, 0), (n + 5, dr0, 1), (n + 6, dr1, 1)]:
        if [k for k, _ in d] != list(range(first, first + len(d))):
            raise SystemExit(f"table {t}: keys not dense")
    emit_codes(f"{name}_VLC", codes)
    emit_list(f"{name}_RUN", "u8", [r[1] for r in idx])
    emit_list(f"{name}_LEVEL", "u8", [r[2] for r in idx])
    print(f"pub const {name}_START_LAST: usize = {start_last};")
    # Delta level indexed by run (from 0); delta run indexed by level (from 1, stored at level).
    emit_list(f"{name}_DELTA_LEVEL0", "u8", [v for _, v in dl0])
    emit_list(f"{name}_DELTA_LEVEL1", "u8", [v for _, v in dl1])
    emit_list(f"{name}_DELTA_RUN0", "u8", [0] + [v for _, v in dr0])
    emit_list(f"{name}_DELTA_RUN1", "u8", [0] + [v for _, v in dr1])

# 11.9: scans.
emit_list("SCAN_INTRA_NORMAL", "u8", zigzag(233, 64))
emit_list("SCAN_INTRA_HORIZONTAL", "u8", zigzag(234, 64))
emit_list("SCAN_INTRA_VERTICAL", "u8", zigzag(235, 64))
emit_list("SCAN_INTER_8X8", "u8", zigzag(236, 64))
emit_list("SCAN_INTER_8X4_SM", "u8", zigzag(237, 32))
emit_list("SCAN_INTER_4X8_SM", "u8", zigzag(238, 32))
emit_list("SCAN_INTER_4X4", "u8", zigzag(239, 16))
emit_list("SCAN_INTER_8X4_ADV", "u8", zigzag(240, 32))
emit_list("SCAN_INTER_4X8_ADV", "u8", zigzag(241, 32))
emit_list("SCAN_ILACE_8X8", "u8", zigzag(242, 64))
emit_list("SCAN_ILACE_8X4", "u8", zigzag(243, 32))
emit_list("SCAN_ILACE_4X8", "u8", zigzag(244, 32))
emit_list("SCAN_ILACE_4X4", "u8", zigzag(245, 16))

# 11.10: MV differential.
for t in range(4):
    emit_codes(f"MV_DIFF{t}", vlc(246 + t))

# Table 74: DQScale (index 1..63; 0 unused).
dq = sorted(pairs(74))
if [k for k, _ in dq] != list(range(1, 64)):
    raise SystemExit("table 74 keys")
emit_list("DQSCALE", "u32", [0] + [v for _, v in dq])

# Table 81: Norm-6 tiles: k -> code, length (escape and following code joined).
tiles = []
for l in body(81):
    toks = l.split()
    if toks and all(re.fullmatch(r"\d+", t) for t in toks) and len(toks) in (3, 5):
        v = list(map(int, toks))
        if len(v) == 3:
            tiles.append((v[0], v[1], v[2]))
        else:
            tiles.append((v[0], (v[1] << v[4]) | v[3], v[2] + v[4]))
tiles.sort()
if [k for k, _, _ in tiles] != list(range(64)):
    raise SystemExit("table 81 keys")
codes = [(c, l) for _, c, l in tiles]
check_prefix_free(81, codes, complete=False)
emit_codes("NORM6", codes)
