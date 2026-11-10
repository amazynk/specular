#!/usr/bin/env python3
"""Transcribe the QCELP-13 tables of 3GPP2 C.S0020-A into crates/formats/specular-qcelp/src/tables.rs.

Usage: tools/corpus/qcelp-tables.py SPEC_TXT > crates/formats/specular-qcelp/src/tables.rs

SPEC_TXT is `pdftotext -layout` of ARIB STD-T64-C.S0020-A v1.0 (the 3GPP2 C.S0020-A v1.0
text republished by ARIB:
https://www.arib.or.jp/english/html/overview/doc/Specification/ARIB_STD-T64-C.S0020-Av1.0.pdf).
Only data is taken: the LSP VQ codebooks (Tables 2.4.3.2.6.3-1..5), the circular codebooks
(Tables 2.4.6.1-1/-2), the linear gain table (2.4.6.2.1-3), the rate 1/4 band-pass filter
(2.4.8.1.2-1) and the packet structures (Tables 2.4.7.1-1 .. 2.4.7.4-1). Printed index typos
are reported on stderr and the entries placed by their position in the table.
"""

import re
import sys

LINES = open(sys.argv[1], encoding="utf-8").read().split("\n")


def body_header(text, start=0):
    for i in range(start, len(LINES)):
        if text in LINES[i] and ".." not in LINES[i]:
            return i
    raise SystemExit(f"header not found: {text}")


def strip_lineno(s):
    # pdftotext keeps the spec's margin line numbers ("12   ...")
    return re.sub(r"^\s{0,3}\d{1,2}\s{2,}", " ", s)


NUM = r"-?\d+\.\d+"


def lsp_table(k, size):
    """LSPVQk as `size` (x, y) pairs."""
    out = [None] * size
    i = body_header(f"Table 2.4.3.2.6.3-{k}. LSP Vector")
    end = body_header("2.4.3.2.7 ", i) if k == 5 else body_header(f"Table 2.4.3.2.6.3-{k + 1}. LSP Vector", i)
    # pages: runs of (pair line, y line)
    rows = []
    j = i + 1
    while j < end:
        s = strip_lineno(LINES[j])
        toks = s.split()
        if len(toks) >= 2 and len(toks) % 2 == 0 and all(
            re.fullmatch(r"\d+", toks[t]) and re.fullmatch(NUM, toks[t + 1]) for t in range(0, len(toks), 2)
        ):
            ys = strip_lineno(LINES[j + 1]).split()
            if len(ys) != len(toks) // 2 or not all(re.fullmatch(NUM, y) for y in ys):
                raise SystemExit(f"LSPVQ{k}: y line missing at {j + 2}")
            rows.append((j, [(int(toks[t]), float(toks[t + 1])) for t in range(0, len(toks), 2)], [float(y) for y in ys]))
            j += 2
            continue
        j += 1
    # split rows into pages (parts) where the first column's index stops counting up
    pages, cur = [], []
    for r in rows:
        if cur and r[1][0][0] != cur[-1][1][0][0] + 1:
            pages.append(cur)
            cur = []
        cur.append(r)
    pages.append(cur)
    for p in pages:
        first = p[0][1][0][0]
        nrows = len(p)
        for ri, (ln, pairs, ys) in enumerate(p):
            for c, ((idx, x), y) in enumerate(zip(pairs, ys)):
                want = first + c * nrows + ri
                if idx != want:
                    print(f"LSPVQ{k}: line {ln + 1}: printed index {idx}, position {want}", file=sys.stderr)
                if want >= size or out[want] is not None:
                    raise SystemExit(f"LSPVQ{k}: bad position {want}")
                out[want] = (x, y)
    if any(v is None for v in out):
        raise SystemExit(f"LSPVQ{k}: missing entries")
    return out


def codebook(name):
    i = body_header(name)
    vals = {}
    j = i + 1
    while len(vals) < 128:
        s = LINES[j]
        if re.match(r"^\s*n\s", s):
            ns = [int(t) for t in s.split()[1:]]
            cs = LINES[j + 1].split()
            assert cs[0] == "c(n)", (j, cs)
            for n, c in zip(ns, cs[1:]):
                vals[n] = float(c)
            j += 2
            continue
        j += 1
    return [vals[n] for n in range(128)]


def gain_table():
    i = body_header("Table 2.4.6.2.1-3. Table for Conversion")
    vals = {}
    j = i + 1
    while len(vals) < 61:
        toks = LINES[j].split()
        if toks and all(re.fullmatch(r"\d+(\.\d+)?", t) for t in toks) and len(toks) % 2 == 0:
            for t in range(0, len(toks), 2):
                vals[int(toks[t])] = float(toks[t + 1])
        j += 1
    return [vals[g] for g in range(61)]


def bpf():
    i = body_header("Table 2.4.8.1.2-1. Impulse Response")
    vals = {}
    j = i + 1
    while len(vals) < 21:
        for n, h in re.findall(r"(\d+)\s+(-?\d\.\d+E-?\d+)", LINES[j]):
            vals[int(n)] = float(h)
        j += 1
    return [vals[n] for n in range(1, 22)]


FIELDS = {"LSPV": 0, "LSP": 1, "PLAG": 2, "PFRAC": 3, "PGAIN": 4, "CINDEX": 5, "CBGAIN": 6, "CBSIGN": 7, "CBSEED": 8}


def packing(table, nbits, until):
    i = body_header(table)
    end = body_header(until, i + 1)
    bits = {}
    for j in range(i + 1, end):
        s = LINES[j]
        if "Table" in s or "shall" in s:
            continue
        for b, code in re.findall(r"(?<![\w\[])(\d+)\s+([A-Z]+\d*\[\d\]|RESERVED)", s):
            b = int(b)
            if b in bits:
                raise SystemExit(f"{table}: bit {b} twice")
            if code == "RESERVED":
                bits[b] = (9, 0, 0)
            else:
                m = re.fullmatch(r"([A-Z]+?)(\d*)\[(\d)\]", code)
                name, idx, pos = m.group(1), m.group(2), int(m.group(3))
                if name not in FIELDS:
                    raise SystemExit(f"{table}: unknown code {code}")
                bits[b] = (FIELDS[name], int(idx) if idx else 0, pos)
    if sorted(bits) != list(range(nbits)):
        raise SystemExit(f"{table}: bits {sorted(set(range(nbits)) - set(bits))} missing")
    return [bits[b] for b in range(nbits - 1, -1, -1)]


def fmt_f(v):
    s = repr(float(v))
    return s


def emit():
    p = print
    p("//! QCELP-13 tables, transcribed from 3GPP2 C.S0020-A v1.0 (ARIB STD-T64-C.S0020-A v1.0)")
    p("//! by `tools/corpus/qcelp-tables.py`. Generated: do not edit by hand.")
    p("")
    p("// spec values that happen to lie near 1/π")
    p("#![allow(clippy::approx_constant)]")
    p("")
    sizes = [64, 128, 128, 64, 64]
    for k, n in enumerate(sizes, 1):
        t = lsp_table(k, n)
        p(f"/// Table 2.4.3.2.6.3-{k}: LSPVQ{k}, differential (x, y) pairs.")
        p(f"pub(crate) static LSPVQ{k}: [[f32; 2]; {n}] = [")
        for x, y in t:
            p(f"    [{fmt_f(x)}, {fmt_f(y)}],")
        p("];")
        p("")
    for name, title, tab in (("CODEBOOK_HALF", "Table 2.4.6.1-1: circular codebook for Rate 1/2.", "Table 2.4.6.1-1. Circular"),
                             ("CODEBOOK_FULL", "Table 2.4.6.1-2: circular codebook for Rate 1.", "Table 2.4.6.1-2. Circular")):
        cb = codebook(tab)
        p(f"/// {title}")
        p(f"pub(crate) static {name}: [f32; 128] = [")
        for r in range(0, 128, 8):
            p("    " + " ".join(fmt_f(v) + "," for v in cb[r:r + 8]))
        p("];")
        p("")
    g = gain_table()
    p("/// Table 2.4.6.2.1-3: the linear gain for G1 = 0..60 dB (three fractional bits).")
    p("pub(crate) static GAIN_LINEAR: [f32; 61] = [")
    for r in range(0, 61, 8):
        p("    " + " ".join(fmt_f(v) + "," for v in g[r:r + 8]))
    p("];")
    p("")
    h = bpf()
    p("/// Table 2.4.8.1.2-1: the band-pass filter of the Rate 1/4 excitation, h(1)..h(21).")
    p("pub(crate) static BPF: [f32; 21] = [")
    for r in range(0, 21, 4):
        p("    " + " ".join(fmt_f(v) + "," for v in h[r:r + 4]))
    p("];")
    p("")
    p("/// Packet structures (Tables 2.4.7.1-1 .. 2.4.7.4-1), first transmitted bit first:")
    p("/// (field, field number, bit of the field). Fields: 0 LSPV, 1 LSP, 2 PLAG, 3 PFRAC,")
    p("/// 4 PGAIN, 5 CBINDEX, 6 CBGAIN, 7 CBSIGN, 8 CBSEED, 9 reserved.")
    for name, tab, n, until in (("PACK_FULL", "Table 2.4.7.1-1. Rate 1 Packet Structure (Part 1", 266, "2.4.7.2 "),
                                ("PACK_HALF", "Table 2.4.7.2-1. Rate 1/2 Packet", 124, "2.4.7.3 "),
                                ("PACK_QUARTER", "Table 2.4.7.3-1. Rate 1/4 Packet", 54, "2.4.7.4 "),
                                ("PACK_EIGHTH", "Table 2.4.7.4-1. Rate 1/8 Packet", 20, "2.4.8 ")):
        bits = packing(tab, n, until)
        p(f"pub(crate) static {name}: [(u8, u8, u8); {n}] = [")
        for r in range(0, n, 6):
            p("    " + " ".join(f"({a}, {b}, {c})," for a, b, c in bits[r:r + 6]))
        p("];")
        p("")


emit()
