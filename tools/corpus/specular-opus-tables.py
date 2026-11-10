#!/usr/bin/env python3
"""Extract the data tables specular-opus needs from RFC 6716.

Two outputs, both generated (do not edit them by hand):

* crates/formats/specular-opus/src/rfc_tables.rs: the tables printed in the
  RFC text itself (SILK PDFs, codebooks, LSF tables, pitch contours, LTP
  filters, the CELT allocation table, ...), parsed from the ASCII tables.
* crates/formats/specular-opus/src/celt_data.rs: the few CELT tables that the
  RFC only gives in its attached reference source (Appendix A.3, base64): the
  Laplace energy model, the band means, the 2.5 ms band layout, the band log
  widths, the PVQ pulse cache and band caps, and the conservative log2 table.
  Only the numbers of those arrays are copied, no code.

Usage: python3 -I tools/corpus/specular-opus-tables.py <rfc6716.txt>
       (rfc6716.txt from https://www.rfc-editor.org/rfc/rfc6716.txt)
"""
import base64
import io
import pathlib
import re
import sys
import tarfile

HERE = pathlib.Path(__file__).resolve().parent
CRATE = HERE.parent.parent / "crates/formats/specular-opus/src"


# ---------------------------------------------------------------- RFC text tables

class Rfc:
    def __init__(self, text):
        self.lines = text.splitlines()

    def block(self, n):
        """The raw lines of the table whose caption is 'Table n:'."""
        cap = None
        for i, l in enumerate(self.lines):
            if re.match(r"\s*Table %d:" % n, l):
                cap = i
                break
        if cap is None:
            raise SystemExit(f"Table {n} not found")
        out = []
        i = cap - 1
        while i >= 0:
            l = self.lines[i]
            s = l.strip()
            if s == "" or s.startswith("|") or s.startswith("+") or s.startswith("\f") \
                    or l.startswith("Valin, et al.") or l.startswith("RFC 6716"):
                if s.startswith("|"):
                    out.append(s)
                i -= 1
                continue
            break
        out.reverse()
        return out

    def rows(self, n):
        return [[c.strip() for c in r.strip("|").split("|")] for r in self.block(n)]

    def pdfs(self, n):
        """All '{a, b, ...}/ft' PDFs of a table, in reading order (cells may wrap rows)."""
        rows = self.rows(n)
        # The PDF is in the last column holding a '{'.
        col = None
        for r in rows:
            for j, c in enumerate(r):
                if "{" in c:
                    col = j
        text = " ".join(r[col] for r in rows if len(r) > col)
        out = []
        for m in re.finditer(r"\{([^}]*)\}\s*/\s*(\d+)", text):
            vals = [int(v) for v in m.group(1).replace(" ", "").split(",") if v != ""]
            if sum(vals) != int(m.group(2)):
                raise SystemExit(f"Table {n}: PDF {vals} does not sum to {m.group(2)}")
            out.append(vals)
        return out

    def numeric_rows(self, n, first_col_index=True):
        """Rows of integers; when first_col_index, the first cell (row label) is dropped."""
        out = []
        for r in self.rows(n):
            cells = r[1:] if first_col_index else r
            nums = []
            for c in cells:
                nums += [int(v) for v in re.findall(r"-?\d+", c)]
            if first_col_index and not re.fullmatch(r"\d+", r[0]):
                continue
            if nums:
                out.append(nums)
        return out

    def letter_rows(self, n):
        out = []
        for r in self.rows(n):
            if len(r) < 2 or not re.fullmatch(r"\d+|g", r[0]):
                continue
            letters = r[1].split()
            if letters and all(re.fullmatch(r"[a-pA-D]", x) for x in letters):
                out.append(letters)
        return out


def icdf(pdf):
    """The inverse CDF (ft - cumulative) of a PDF, dropping nothing (ec_dec_icdf form)."""
    ft = sum(pdf)
    out, acc = [], 0
    for f in pdf:
        acc += f
        out.append(ft - acc)
    return out


def rust_arr(name, ty, values, per_line=16, vis="pub(crate)"):
    rows = []
    for i in range(0, len(values), per_line):
        rows.append("    " + ", ".join(str(v) for v in values[i:i + per_line]) + ",")
    return f"{vis} static {name}: [{ty}; {len(values)}] = [\n" + "\n".join(rows) + "\n];\n"


def rust_2d(name, ty, rows, vis="pub(crate)"):
    w = len(rows[0])
    if any(len(r) != w for r in rows):
        raise SystemExit(f"{name}: ragged rows {[len(r) for r in rows]}")
    body = "\n".join("    [" + ", ".join(str(v) for v in r) + "]," for r in rows)
    return f"{vis} static {name}: [[{ty}; {w}]; {len(rows)}] = [\n{body}\n];\n"


def rust_icdfs(name, pdfs, vis="pub(crate)"):
    """A slice of icdf tables of varying length."""
    body = "\n".join("    &[" + ", ".join(str(v) for v in icdf(p)) + "]," for p in pdfs)
    return f"{vis} static {name}: [&[u8]; {len(pdfs)}] = [\n{body}\n];\n"


def rust_icdf(name, pdf, vis="pub(crate)"):
    return f"{vis} static {name}: [u8; {len(pdf)}] = [" + ", ".join(str(v) for v in icdf(pdf)) + "];\n"


def rfc_tables(rfc):
    out = ["// Generated by tools/corpus/specular-opus-tables.py from the text of RFC 6716.\n"
           "// Do not edit. PDFs are stored as inverse CDFs (ft minus the running sum).\n"]

    def one(n, k=1):
        p = rfc.pdfs(n)
        if len(p) != k:
            raise SystemExit(f"Table {n}: {len(p)} PDFs, expected {k}")
        return p

    # SILK header and stereo.
    lbrr = one(4, 2)
    # Both start with a zero-probability symbol 0 (the flag set is never empty): drop it.
    out.append("// Table 4 (symbol 0 has probability 0 and is dropped: add 1 to the result).\n")
    out.append(rust_icdf("LBRR_FLAGS_40MS", lbrr[0][1:]))
    out.append(rust_icdf("LBRR_FLAGS_60MS", lbrr[1][1:]))
    st = one(6, 3)
    out.append("// Table 6.\n")
    out.append(rust_icdf("STEREO_STAGE1", st[0]))
    out.append(rust_icdf("STEREO_STAGE2", st[1]))
    out.append(rust_icdf("STEREO_STAGE3", st[2]))
    w = [r[0] for r in rfc.numeric_rows(7)]
    assert len(w) == 16
    out.append("// Table 7.\n")
    out.append(rust_arr("STEREO_WEIGHTS_Q13", "i32", w))
    out.append("// Table 8.\n")
    out.append(rust_icdf("MID_ONLY", one(8)[0]))
    ft = one(9, 2)
    out.append("// Table 9: inactive {0,1}, active {2..5} (leading zero-probability symbols dropped).\n")
    out.append(rust_icdf("FRAME_TYPE_INACTIVE", ft[0][:2]))
    out.append(rust_icdf("FRAME_TYPE_ACTIVE", ft[1][2:]))
    out.append("// Table 11: by signal type (inactive, unvoiced, voiced).\n")
    out.append(rust_2d("GAIN_MSB", "u8", [icdf(p) for p in one(11, 3)]))
    out.append("// Table 12.\n")
    out.append(rust_icdf("GAIN_LSB", one(12)[0]))
    out.append("// Table 13.\n")
    out.append(rust_icdf("GAIN_DELTA", one(13)[0]))

    # LSF.
    out.append("// Table 14: NB/MB unvoiced, NB/MB voiced, WB unvoiced, WB voiced.\n")
    out.append(rust_2d("LSF_STAGE1", "u8", [icdf(p) for p in one(14, 4)]))
    out.append("// Tables 15 (a..h) and 16 (i..p).\n")
    out.append(rust_2d("LSF_STAGE2_NBMB", "u8", [icdf(p) for p in one(15, 8)]))
    out.append(rust_2d("LSF_STAGE2_WB", "u8", [icdf(p) for p in one(16, 8)]))
    sel = rfc.letter_rows(17)
    assert len(sel) == 32 and all(len(r) == 10 for r in sel), sel
    out.append("// Table 17: stage-2 PDF per coefficient (0 = a).\n")
    out.append(rust_2d("LSF_SEL_NBMB", "u8", [[ord(x) - ord("a") for x in r] for r in sel]))
    sel = rfc.letter_rows(18)
    assert len(sel) == 32 and all(len(r) == 16 for r in sel), sel
    out.append("// Table 18: stage-2 PDF per coefficient (0 = i).\n")
    out.append(rust_2d("LSF_SEL_WB", "u8", [[ord(x) - ord("i") for x in r] for r in sel]))
    out.append("// Table 19.\n")
    out.append(rust_icdf("LSF_EXT", one(19)[0]))
    # Table 20: columns A, B (9 rows) and C, D (15 rows).
    a, b, c, d = [], [], [], []
    for r in rfc.rows(20):
        if not re.fullmatch(r"\d+", r[0]):
            continue
        vals = r[1:]
        if vals[0]:
            a.append(int(vals[0])); b.append(int(vals[1]))
        c.append(int(vals[2])); d.append(int(vals[3]))
    assert len(a) == 9 and len(c) == 15
    out.append("// Table 20: prediction weights A, B (NB/MB) and C, D (WB).\n")
    out.append(rust_2d("LSF_PRED_NBMB", "u8", [a, b]))
    out.append(rust_2d("LSF_PRED_WB", "u8", [c, d]))
    sel = rfc.letter_rows(21)
    assert len(sel) == 32 and all(len(r) == 9 for r in sel)
    out.append("// Table 21: 0 = A, 1 = B.\n")
    out.append(rust_2d("LSF_PRED_SEL_NBMB", "u8", [[ord(x) - ord("A") for x in r] for r in sel]))
    sel = rfc.letter_rows(22)
    assert len(sel) == 32 and all(len(r) == 15 for r in sel)
    out.append("// Table 22: 0 = C, 1 = D.\n")
    out.append(rust_2d("LSF_PRED_SEL_WB", "u8", [[ord(x) - ord("C") for x in r] for r in sel]))
    cb = rfc.numeric_rows(23)
    assert len(cb) == 32 and all(len(r) == 10 for r in cb)
    out.append("// Table 23.\n")
    out.append(rust_2d("LSF_CB1_NBMB", "u8", cb))
    cb = rfc.numeric_rows(24)
    assert len(cb) == 32 and all(len(r) == 16 for r in cb)
    out.append("// Table 24.\n")
    out.append(rust_2d("LSF_CB1_WB", "u8", cb))
    nbmb, wb = [], []
    for r in rfc.rows(25):
        if re.fullmatch(r"\d+", r[0]):
            if r[1]:
                nbmb.append(int(r[1]))
            wb.append(int(r[2]))
    assert len(nbmb) == 11 and len(wb) == 17
    out.append("// Table 25.\n")
    out.append(rust_arr("LSF_MIN_SPACING_NBMB", "i32", nbmb))
    out.append(rust_arr("LSF_MIN_SPACING_WB", "i32", wb))
    out.append("// Table 26.\n")
    out.append(rust_icdf("LSF_INTERP", one(26)[0]))
    nbmb, wb = [], []
    for r in rfc.rows(27):
        if re.fullmatch(r"\d+", r[0]):
            if r[1]:
                nbmb.append(int(r[1]))
            wb.append(int(r[2]))
    assert len(nbmb) == 10 and len(wb) == 16
    out.append("// Table 27.\n")
    out.append(rust_arr("LSF_ORDER_NBMB", "u8", nbmb))
    out.append(rust_arr("LSF_ORDER_WB", "u8", wb))
    cos = []
    for r in rfc.numeric_rows(28):
        cos += r
    assert len(cos) == 129, len(cos)
    out.append("// Table 28.\n")
    out.append(rust_arr("LSF_COS_Q12", "i32", cos))

    # Pitch and LTP.
    out.append("// Table 29.\n")
    out.append(rust_icdf("PITCH_HIGH", one(29)[0]))
    low = one(30, 3)
    out.append("// Table 30: NB, MB, WB.\n")
    out.append(rust_icdf("PITCH_LOW_NB", low[0]))
    out.append(rust_icdf("PITCH_LOW_MB", low[1]))
    out.append(rust_icdf("PITCH_LOW_WB", low[2]))
    out.append("// Table 31.\n")
    out.append(rust_icdf("PITCH_DELTA", one(31)[0]))
    con = one(32, 4)
    out.append("// Table 32: NB 10 ms, NB 20 ms, MB/WB 10 ms, MB/WB 20 ms.\n")
    out.append(rust_icdf("PITCH_CONTOUR_NB10", con[0]))
    out.append(rust_icdf("PITCH_CONTOUR_NB20", con[1]))
    out.append(rust_icdf("PITCH_CONTOUR_WB10", con[2]))
    out.append(rust_icdf("PITCH_CONTOUR_WB20", con[3]))
    for t, name, n, k in ((33, "PITCH_CB_NB10", 3, 2), (34, "PITCH_CB_NB20", 11, 4),
                          (35, "PITCH_CB_WB10", 12, 2), (36, "PITCH_CB_WB20", 34, 4)):
        rows = rfc.numeric_rows(t)
        assert len(rows) == n and all(len(r) == k for r in rows), (t, rows)
        out.append(f"// Table {t}.\n")
        out.append(rust_2d(name, "i8", rows))
    out.append("// Table 37.\n")
    out.append(rust_icdf("LTP_PERIODICITY", one(37)[0]))
    out.append("// Table 38.\n")
    out.append(rust_icdfs("LTP_FILTER", one(38, 3)))
    for t, name, n in ((39, "LTP_CB0", 8), (40, "LTP_CB1", 16), (41, "LTP_CB2", 32)):
        rows = rfc.numeric_rows(t)
        assert len(rows) == n and all(len(r) == 5 for r in rows), (t, len(rows))
        out.append(f"// Table {t}.\n")
        out.append(rust_2d(name, "i8", rows))
    out.append("// Table 42.\n")
    out.append(rust_icdf("LTP_SCALE", one(42)[0]))
    out.append("// Table 43.\n")
    out.append(rust_icdf("LCG_SEED", one(43)[0]))

    # Excitation.
    out.append("// Table 45: inactive/unvoiced, voiced.\n")
    out.append(rust_2d("RATE_LEVEL", "u8", [icdf(p) for p in one(45, 2)]))
    pc = one(46, 11)
    out.append("// Table 46: rate levels 0..10 (level 10's last symbol has probability 0).\n")
    out.append(rust_2d("PULSE_COUNT", "u8", [icdf(p) for p in pc]))
    for t, name in ((47, "SHELL16"), (48, "SHELL8"), (49, "SHELL4"), (50, "SHELL2")):
        p = one(t, 16)
        out.append(f"// Table {t}: pulse counts 1..16.\n")
        out.append(rust_icdfs(name, p))
    out.append("// Table 51.\n")
    out.append(rust_icdf("EXC_LSB", one(51)[0]))
    sg = one(52, 42)
    out.append("// Table 52: [signal type * 2 + offset type][min(pulses, 6)], first icdf entry.\n")
    out.append(rust_2d("SIGN", "u8", [[icdf(sg[i * 7 + k])[0] for k in range(7)] for i in range(6)]))

    # CELT.
    alloc = rfc.numeric_rows(57, first_col_index=False)
    alloc = [r for r in alloc if len(r) == 11]
    assert alloc[0] == list(range(11)), alloc[0]
    alloc = alloc[1:]
    assert len(alloc) == 21
    out.append("// Table 57: [band][q], 1/32 bit per MDCT bin.\n")
    out.append(rust_2d("CELT_ALLOC", "u8", alloc))
    out.append("// Table 58.\n")
    out.append(rust_icdf("CELT_TRIM", one(58)[0]))
    return "\n".join(out)


# ---------------------------------------------------------------- attached source

def attachment(rfc_text):
    lines = [l[6:] for l in rfc_text.splitlines() if l.startswith("   ###")]
    data = base64.b64decode("".join(lines))
    files = {}
    with tarfile.open(fileobj=io.BytesIO(data), mode="r:gz") as tar:
        for m in tar.getmembers():
            if m.isfile():
                files[m.name] = tar.extractfile(m).read().decode("latin-1")
    return files


def array(src, name):
    """The numbers of the C array `name` (after its '= {' up to the matching '};')."""
    m = re.search(r"\b" + re.escape(name) + r"\s*(\[[^\]]*\]\s*)+=\s*\{", src)
    if not m:
        raise SystemExit(f"array {name} not found")
    body = src[m.end():src.index("};", m.end())]
    body = re.sub(r"/\*.*?\*/", " ", body, flags=re.S)
    return [int(v, 0) for v in re.findall(r"-?(?:0x[0-9a-fA-F]+|\d+)", body)]


def celt_data(files):
    def find(suffix):
        for k, v in files.items():
            if k.endswith(suffix):
                return v
        raise SystemExit(f"{suffix} not in the attachment")

    quant = find("celt/quant_bands.c")
    modes = find("celt/static_modes_float.h")
    modes_c = find("celt/modes.c")
    rate = find("celt/rate.c")

    out = ["// Generated by tools/corpus/specular-opus-tables.py from the reference source\n"
           "// attached to RFC 6716 (Appendix A.3): table data only. Do not edit.\n"]
    out.append("// Laplace parameters of the coarse energy (e_prob_model): [LM][intra][band*2 + {prob, decay}].\n")
    out.append(rust_arr("E_PROB_MODEL", "u8", array(quant, "e_prob_model"), 21 * 2))
    out.append("// Mean band energies, log2 in Q4 (eMeans, the fixed-point listing).\n")
    out.append(rust_arr("E_MEANS_Q6", "i8", array(quant, "eMeans"), 25))
    out.append("// Band edges of the 2.5 ms (LM 0) layout, in MDCT bins (eband5ms).\n")
    out.append(rust_arr("EBAND_5MS", "i16", array(modes_c, "eband5ms"), 22))
    out.append("// log2 of the band widths in 1/8 bits (logN400).\n")
    out.append(rust_arr("LOG_N400", "i16", array(modes, "logN400"), 21))
    out.append("// PVQ pulse cache: index per (LM, band) and bit costs (cache_index50, cache_bits50).\n")
    out.append(rust_arr("CACHE_INDEX50", "i16", array(modes, "cache_index50"), 21))
    out.append(rust_arr("CACHE_BITS50", "u8", array(modes, "cache_bits50")))
    out.append("// Maximum band allocations per (LM, stereo, band) (cache_caps50).\n")
    out.append(rust_arr("CACHE_CAPS50", "u8", array(modes, "cache_caps50"), 21))
    out.append("// Conservative log2 in 1/8 bits of 0..23 (LOG2_FRAC_TABLE).\n")
    out.append(rust_arr("LOG2_FRAC_TABLE", "u8", array(rate, "LOG2_FRAC_TABLE"), 24))
    return "\n".join(out)


def main():
    if len(sys.argv) != 2:
        raise SystemExit(__doc__)
    text = pathlib.Path(sys.argv[1]).read_text(encoding="latin-1")
    (CRATE / "rfc_tables.rs").write_text(rfc_tables(Rfc(text)))
    (CRATE / "celt_data.rs").write_text(celt_data(attachment(text)))
    print(f"wrote {CRATE / 'rfc_tables.rs'} and {CRATE / 'celt_data.rs'}")


if __name__ == "__main__":
    main()
