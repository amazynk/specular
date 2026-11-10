#!/usr/bin/env python3
"""Generate crates/formats/specular-vp9/src/tables.rs from the VP9 bitstream specification.

Every constant table (probabilities, trees, scans, quantiser lookups, filters, conversion tables)
is read from the text of the official specification PDF ("VP9 Bitstream & Decoding Process
Specification", v0.7 or v0.6, webmproject.org) with `pdftotext -layout`, checked against the
dimensions the specification declares for it, and written out as Rust statics.

Usage: tools/corpus/specular-vp9-tables.py [spec.pdf]
Default PDF: target/video2/specs/vp9-bitstream-specification-v0.7-20170222-draft.pdf
(fetch it with tools/corpus/specular-vp9-fetch.sh). Needs pdftotext (poppler-utils).
Run python with -I: the PDF is downloaded data.
"""
import os
import re
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
SPEC = "target/video2/specs/vp9-bitstream-specification-v0.7-20170222-draft.pdf"


def default_pdf():
    """The PDF under target/ of this checkout or of a checkout above it (for a worktree)."""
    d = ROOT
    while True:
        c = os.path.join(d, SPEC)
        if os.path.exists(c):
            return c
        if os.path.dirname(d) == d:
            raise SystemExit("specification PDF not found: run tools/corpus/specular-vp9-fetch.sh")
        d = os.path.dirname(d)
OUT = os.path.join(ROOT, "crates/formats/specular-vp9/src/tables.rs")

# Named constants used in table bodies and dimensions (sections 3 and 7 of the specification).
SYM = {
    "BLOCK_SIZES": 13, "BLOCK_INVALID": 14, "PARTITION_CONTEXTS": 16, "PARTITION_TYPES": 4,
    "MAX_PROB": 255, "SEG_LVL_MAX": 4, "MVREF_NEIGHBOURS": 8, "MB_MODE_COUNT": 14,
    "INTRA_MODES": 10, "BLOCK_SIZE_GROUPS": 4, "SKIP_CONTEXTS": 3, "IS_INTER_CONTEXTS": 4,
    "COMP_MODE_CONTEXTS": 5, "REF_CONTEXTS": 5, "MV_OFFSET_BITS": 10, "TX_SIZES": 4,
    "TX_SIZE_CONTEXTS": 2, "TX_MODES": 5, "INTER_MODE_CONTEXTS": 7, "INTER_MODES": 4,
    "INTERP_FILTER_CONTEXTS": 4, "SWITCHABLE_FILTERS": 3, "MV_CLASSES": 11, "CLASS0_SIZE": 2,
    "COEF_BANDS": 6, "PREV_COEF_CONTEXTS": 6, "UNCONSTRAINED_NODES": 3, "BLOCK_TYPES": 2,
    "REF_TYPES": 2,
    # Block sizes.
    "BLOCK_4X4": 0, "BLOCK_4X8": 1, "BLOCK_8X4": 2, "BLOCK_8X8": 3, "BLOCK_8X16": 4,
    "BLOCK_16X8": 5, "BLOCK_16X16": 6, "BLOCK_16X32": 7, "BLOCK_32X16": 8, "BLOCK_32X32": 9,
    "BLOCK_32X64": 10, "BLOCK_64X32": 11, "BLOCK_64X64": 12,
    # Transform sizes and types.
    "TX_4X4": 0, "TX_8X8": 1, "TX_16X16": 2, "TX_32X32": 3,
    "DCT_DCT": 0, "ADST_DCT": 1, "DCT_ADST": 2, "ADST_ADST": 3,
    # Partitions.
    "PARTITION_NONE": 0, "PARTITION_HORZ": 1, "PARTITION_VERT": 2, "PARTITION_SPLIT": 3,
    # Prediction modes.
    "DC_PRED": 0, "V_PRED": 1, "H_PRED": 2, "D45_PRED": 3, "D135_PRED": 4, "D117_PRED": 5,
    "D153_PRED": 6, "D207_PRED": 7, "D63_PRED": 8, "TM_PRED": 9, "NEARESTMV": 10,
    "NEARMV": 11, "ZEROMV": 12, "NEWMV": 13,
    # Interpolation filters.
    "EIGHTTAP": 0, "EIGHTTAP_SMOOTH": 1, "EIGHTTAP_SHARP": 2, "BILINEAR": 3,
    # Motion vectors.
    "MV_JOINT_ZERO": 0, "MV_JOINT_HNZVZ": 1, "MV_JOINT_HZVNZ": 2, "MV_JOINT_HNZVNZ": 3,
    **{f"MV_CLASS_{i}": i for i in range(11)},
    # Tokens.
    "ZERO_TOKEN": 0, "ONE_TOKEN": 1, "TWO_TOKEN": 2, "THREE_TOKEN": 3, "FOUR_TOKEN": 4,
    **{f"DCT_VAL_CATEGORY{i}": 4 + i for i in range(1, 7)},
    # Motion vector reference contexts.
    "BOTH_ZERO": 0, "ZERO_PLUS_PREDICTED": 1, "BOTH_PREDICTED": 2, "NEW_PLUS_NON_INTRA": 3,
    "BOTH_NEW": 4, "INTRA_PLUS_NON_INTRA": 5, "BOTH_INTRA": 6, "INVALID_CASE": 9,
}

# name -> (rust name, element type). The order is the order in tables.rs.
TABLES = [
    ("literal_to_type", "u8"),
    ("segmentation_feature_bits", "u8"),
    ("segmentation_feature_signed", "u8"),
    ("inv_map_table", "u8"),
    ("max_txsize_lookup", "u8"),
    ("ss_size_lookup", "u8"),
    ("extra_bits", "u16"),
    ("cat_probs", "u8"),
    ("mv_ref_blocks", "i8"),
    ("mode_2_counter", "u8"),
    ("counter_to_context", "u8"),
    ("idx_n_column_to_subblock", "u8"),
    ("subpel_filters", "i16"),
    ("dc_qlookup", "u16"),
    ("ac_qlookup", "u16"),
    ("cos64_lookup", "i32"),
    ("small_token_tree", "i8"),
    ("partition_tree", "i8"),
    ("cols_partition_tree", "i8"),
    ("rows_partition_tree", "i8"),
    ("intra_mode_tree", "i8"),
    ("segment_tree", "i8"),
    ("binary_tree", "i8"),
    ("tx_size_32_tree", "i8"),
    ("tx_size_16_tree", "i8"),
    ("tx_size_8_tree", "i8"),
    ("inter_mode_tree", "i8"),
    ("interp_filter_tree", "i8"),
    ("mv_joint_tree", "i8"),
    ("mv_class_tree", "i8"),
    ("mv_fr_tree", "i8"),
    ("token_tree", "i8"),
    ("default_scan_4x4", "u16"),
    ("col_scan_4x4", "u16"),
    ("row_scan_4x4", "u16"),
    ("default_scan_8x8", "u16"),
    ("col_scan_8x8", "u16"),
    ("row_scan_8x8", "u16"),
    ("default_scan_16x16", "u16"),
    ("col_scan_16x16", "u16"),
    ("row_scan_16x16", "u16"),
    ("default_scan_32x32", "u16"),
    ("b_width_log2_lookup", "u8"),
    ("b_height_log2_lookup", "u8"),
    ("num_4x4_blocks_wide_lookup", "u8"),
    ("num_4x4_blocks_high_lookup", "u8"),
    ("mi_width_log2_lookup", "u8"),
    ("num_8x8_blocks_wide_lookup", "u8"),
    ("mi_height_log2_lookup", "u8"),
    ("num_8x8_blocks_high_lookup", "u8"),
    ("size_group_lookup", "u8"),
    ("tx_mode_to_biggest_tx_size", "u8"),
    ("subsize_lookup", "u8"),
    ("coefband_4x4", "u8"),
    ("coefband_8x8plus", "u8"),
    ("energy_class", "u8"),
    ("mode2txfm_map", "u8"),
    ("pareto_table", "u8"),
    ("kf_partition_probs", "u8"),
    ("kf_y_mode_probs", "u8"),
    ("kf_uv_mode_probs", "u8"),
    ("default_partition_probs", "u8"),
    ("default_y_mode_probs", "u8"),
    ("default_uv_mode_probs", "u8"),
    ("default_skip_prob", "u8"),
    ("default_is_inter_prob", "u8"),
    ("default_comp_mode_prob", "u8"),
    ("default_comp_ref_prob", "u8"),
    ("default_single_ref_prob", "u8"),
    ("default_mv_sign_prob", "u8"),
    ("default_mv_bits_prob", "u8"),
    ("default_mv_class0_bit_prob", "u8"),
    ("default_tx_probs", "u8"),
    ("default_inter_mode_probs", "u8"),
    ("default_interp_filter_probs", "u8"),
    ("default_mv_joint_probs", "u8"),
    ("default_mv_class_probs", "u8"),
    ("default_mv_class0_fr_probs", "u8"),
    ("default_mv_class0_hp_prob", "u8"),
    ("default_mv_fr_probs", "u8"),
    ("default_mv_hp_prob", "u8"),
    ("default_coef_probs", "u8"),
]

# Tables whose rows the specification prints shorter than the declared row length (the
# missing trailing entries are unused): padded with zeros.
JAGGED = {"cat_probs"}

RANGES = {"u8": (0, 255), "i8": (-128, 127), "u16": (0, 65535), "i16": (-32768, 32767),
          "i32": (-(1 << 31), (1 << 31) - 1)}


def spec_text(pdf):
    txt = subprocess.run(["pdftotext", "-layout", pdf, "-"], check=True, capture_output=True,
                         text=True).stdout
    keep = []
    for line in txt.splitlines():
        # Page headers and footers carry digits (dates, page numbers): drop them.
        if "Copyright" in line or "Decoding Process Specification - v0" in line:
            continue
        keep.append(line.replace("\f", ""))
    return "\n".join(keep)


def strip_comments(s):
    s = re.sub(r"/\*.*?\*/", " ", s, flags=re.S)
    return re.sub(r"//[^\n]*", " ", s)


def ev(expr):
    """Evaluates one table element: integers, named constants, unary/binary minus, parentheses."""
    e = expr.strip()
    if not re.fullmatch(r"[A-Za-z0-9_\s\-\+\*\(\)]+", e):
        raise ValueError(f"bad element {expr!r}")
    names = re.findall(r"[A-Za-z_][A-Za-z0-9_]*", e)
    for n in names:
        if n not in SYM:
            raise ValueError(f"unknown name {n} in {expr!r}")
    py = re.sub(r"[A-Za-z_][A-Za-z0-9_]*", lambda m: str(SYM[m.group(0)]), e)
    return int(eval(py, {"__builtins__": {}}))  # digits, names replaced, + - * ( ) only


def parse_braces(s, i):
    """Parses `{ ... }` starting at s[i] == '{'; returns (nested list, index after '}')."""
    assert s[i] == "{"
    i += 1
    items, cur = [], ""
    while True:
        c = s[i]
        if c == "{":
            sub, i = parse_braces(s, i)
            items.append(sub)
            cur = ""
            continue
        if c == "}":
            if cur.strip():
                items.append(ev(cur))
            return items, i + 1
        if c == ",":
            if cur.strip():
                items.append(ev(cur))
            cur = ""
        else:
            cur += c
        i += 1


def find_table(text, name):
    m = re.search(r"\b" + re.escape(name) + r"\s*((?:\[[^\]]*\]\s*)+)=\s*\{", text)
    if not m:
        raise SystemExit(f"table {name} not found in the specification text")
    dims = [ev(d) for d in re.findall(r"\[([^\]]*)\]", m.group(1))]
    body, _ = parse_braces(text, m.end() - 1)
    return dims, body


def shape(v, dims, name, jagged):
    """Checks v against dims (padding short rows of jagged tables) and returns it."""
    if not dims:
        if isinstance(v, list):
            raise SystemExit(f"{name}: nesting deeper than declared")
        return v
    if not isinstance(v, list):
        raise SystemExit(f"{name}: expected a list of {dims[0]}")
    if len(dims) == 1 and jagged and len(v) < dims[0]:
        v = v + [0] * (dims[0] - len(v))
    if len(v) != dims[0]:
        raise SystemExit(f"{name}: {len(v)} entries where {dims[0]} are declared")
    return [shape(x, dims[1:], name, jagged) for x in v]


def flat(v):
    return [y for x in v for y in flat(x)] if isinstance(v, list) else [v]


def rust_type(t, dims):
    s = t
    for d in reversed(dims):
        s = f"[{s}; {d}]"
    return s


def rust_value(v, depth=0):
    if not isinstance(v, list):
        return str(v)
    if not isinstance(v[0], list):
        parts, line, out = [str(x) for x in v], "", []
        for p in parts:
            if len(line) + len(p) + 2 > 92:
                out.append(line.rstrip())
                line = ""
            line += p + ", "
        out.append(line.rstrip())
        if len(out) == 1:
            return "[" + out[0].rstrip(",") + "]"
        ind = "    " * (depth + 1)
        return "[\n" + "".join(ind + o + "\n" for o in out) + "    " * depth + "]"
    ind = "    " * (depth + 1)
    return "[\n" + "".join(ind + rust_value(x, depth + 1) + ",\n" for x in v) + "    " * depth + "]"


def main():
    pdf = sys.argv[1] if len(sys.argv) > 1 else default_pdf()
    text = strip_comments(spec_text(pdf))
    # The specification declares inv_map_table[ MAX_PROB ] but prints MAX_PROB - 1 entries
    # plus a repeated last value; the declared size is kept and checked.
    out = [
        "//! Constant tables of the VP9 Bitstream & Decoding Process Specification, generated by",
        "//! `tools/corpus/specular-vp9-tables.py` from the specification text. Do not edit.",
        "#![allow(dead_code)]",
        "",
    ]
    for name, t in TABLES:
        dims, body = find_table(text, name)
        v = shape(body, dims, name, name in JAGGED)
        lo, hi = RANGES[t]
        for x in flat(v):
            if not lo <= x <= hi:
                raise SystemExit(f"{name}: value {x} does not fit {t}")
        out.append(f"pub static {name.upper()}: {rust_type(t, dims)} = {rust_value(v)};")
        out.append("")
        print(f"{name}: {dims} ({len(flat(v))} values)")
    with open(OUT, "w") as f:
        f.write("\n".join(out))
    try:
        subprocess.run(["rustfmt", "--edition", "2024", OUT], check=True)
    except (OSError, subprocess.CalledProcessError) as e:
        print(f"rustfmt failed: {e}")
    print(f"wrote {OUT}")


if __name__ == "__main__":
    main()
