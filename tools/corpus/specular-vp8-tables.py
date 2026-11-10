#!/usr/bin/env python3
"""Extracts the VP8 constant tables from the text of RFC 6386 into Rust.

Usage: specular-vp8-tables.py <rfc6386.txt> crates/formats/specular-vp8/src/tables.rs
(the RFC text: https://www.rfc-editor.org/rfc/rfc6386.txt)

Every table comes from the RFC's own text: probabilities, trees, quantiser lookups, bands,
filters, and the zigzag scan (which the RFC gives only in its reference source, section 20.16).
Tree leaves are written as -(value) with values numbered in the order of the RFC's enums.
"""
import re
import subprocess
import sys

src, out = sys.argv[1], sys.argv[2]
raw = open(src, encoding="utf-8").read()

# Drop page furniture (footer, form feed, running header) so blocks read continuously.
lines = []
for line in raw.split("\n"):
    line = line.replace("\f", "")
    if re.match(r"^Bankoski, et al\.", line) or re.match(r"^RFC 6386 ", line):
        continue
    lines.append(line)
text = "\n".join(lines)


def strip_comments(s):
    s = re.sub(r"/\*.*?\*/", " ", s, flags=re.S)
    return re.sub(r"//[^\n]*", " ", s)


def block(decl, start=0):
    """The brace-delimited initialiser following the first `decl` (a regex) after `start`."""
    m = re.compile(decl).search(text, start)
    if not m:
        sys.exit(f"not found: {decl}")
    i = text.index("{", m.start())
    depth, j = 0, i
    while True:
        c = text[j]
        if c == "{":
            depth += 1
        elif c == "}":
            depth -= 1
            if depth == 0:
                break
        j += 1
    return strip_comments(text[i : j + 1]), m.start()


def ints(decl, count, start=0):
    body, _ = block(decl, start)
    v = [int(x) for x in re.findall(r"-?\d+", body)]
    if len(v) != count:
        sys.exit(f"{decl}: {len(v)} values, wanted {count}")
    return v


def enum(first, start=0):
    """Member names of the `typedef enum` whose first member is `first`, in order."""
    m = re.compile(r"typedef enum\s*\{\s*" + first + r"\b").search(text, start)
    if not m:
        sys.exit(f"enum not found: {first}")
    body = strip_comments(text[m.end() - len(first) : m.end() + 4000])
    body = body[: body.index("}")]
    names = []
    for part in body.split(","):
        name = part.split("=")[0].strip()
        if name and not name.startswith("num_") and not name.startswith("mv_num") \
                and name not in ("MVPcount", "mvnum_partition"):
            names.append(name)
    return names


def tree(decl, names, size):
    body, _ = block(decl)
    lookup = {n.lower(): i for i, n in enumerate(names)}
    out = []
    for tok in re.findall(r"-?\s*[A-Za-z_][A-Za-z0-9_]*|-?\d+", body):
        tok = tok.replace(" ", "")
        if re.fullmatch(r"-?\d+", tok):
            out.append(int(tok))
        else:
            name = tok.lstrip("-").lower()
            if name not in lookup:
                sys.exit(f"{decl}: unknown leaf {tok}")
            out.append(-lookup[name])
    if len(out) != size:
        sys.exit(f"{decl}: {len(out)} entries, wanted {size}")
    return out


ymodes = enum("DC_PRED")[:5]
bmodes = enum("B_DC_PRED")
mv_refs = enum("mv_nearest")
sub_mv_refs = enum("LEFT4x4")
partitions = enum("mv_top_bottom")
tokens = enum("DCT_0")
assert ymodes == ["DC_PRED", "V_PRED", "H_PRED", "TM_PRED", "B_PRED"], ymodes
assert len(bmodes) == 10 and len(tokens) == 12 and len(mv_refs) == 5, (bmodes, tokens, mv_refs)
assert len(sub_mv_refs) == 4 and len(partitions) == 4, (sub_mv_refs, partitions)

# Section 20.16 (tokens.c) is the only place that spells out the scan order.
zz_at = text.index("static const unsigned int zigzag[16]")

tables = [
    ("Y modes in key frames (section 11.2).", "KF_YMODE_TREE", "i8", tree(r"const tree_index kf_ymode_tree", ymodes, 8)),
    ("Y modes in interframes (section 16.1).", "YMODE_TREE", "i8", tree(r"const tree_index ymode_tree", ymodes, 8)),
    ("Chroma modes (section 11.4).", "UV_MODE_TREE", "i8", tree(r"const tree_index uv_mode_tree", ymodes, 6)),
    ("Subblock modes (section 11.2).", "BMODE_TREE", "i8", tree(r"const tree_index bmode_tree", bmodes, 18)),
    ("Segment ids (section 10).", "SEGMENT_TREE", "i8", tree(r"const tree_index mb_segment_tree", ["0", "1", "2", "3"], 6)),
    ("Macroblock motion vector modes, relative to `mv_nearest` (section 16.2).", "MV_REF_TREE", "i8", tree(r"const tree_index mv_ref_tree", mv_refs, 8)),
    ("Split partitionings (section 16.4).", "MV_PARTITION_TREE", "i8", tree(r"const tree_index mvpartition_tree", partitions, 6)),
    ("Subblock motion vector modes, relative to `LEFT4x4` (section 16.4).", "SUB_MV_REF_TREE", "i8", tree(r"const tree_index sub_mv_ref_tree", sub_mv_refs, 6)),
    ("Short motion vector magnitudes (section 17.1).", "SMALL_MV_TREE", "i8", tree(r"const tree_index small_mvtree", [str(i) for i in range(8)], 14)),
    ("DCT tokens (section 13.2).", "COEFF_TREE", "i8", tree(r"const tree_index coeff_tree", tokens, 22)),
    ("", "KF_YMODE_PROB", "u8", ints(r"const Prob kf_ymode_prob \[", 4)),
    ("", "KF_UV_MODE_PROB", "u8", ints(r"const Prob kf_uv_mode_prob \[", 3)),
    ("Defaults, restored on key frames (section 16.1).", "YMODE_PROB", "u8", ints(r"Prob ymode_prob \[", 4)),
    ("", "UV_MODE_PROB", "u8", ints(r"Prob uv_mode_prob \[", 3)),
    ("Subblock modes in interframes (section 16.1).", "BMODE_PROB", "u8", ints(r"const Prob bmode_prob \[num_intra_bmodes - 1\] = \{", 9)),
    ("", "MV_PARTITION_PROB", "u8", ints(r"const Prob mvpartition_probs \[", 3)),
    ("", "SUB_MV_REF_PROB", "u8", ints(r"const Prob sub_mv_ref_prob \[5\]", 15)),
    ("", "MODE_CONTEXTS", "u8", ints(r"const int vp8_mode_contexts\[6\]\[4\]", 24)),
    ("Key frame subblock modes by [above][left] mode (section 11.5).", "KF_BMODE_PROB", "u8", ints(r"const Prob kf_bmode_prob \[num_intra_bmodes\] \[num_intra_bmodes\]\s*\[num_intra_bmodes-1\] =", 900)),
    ("", "COEFF_BANDS", "u8", ints(r"const int coeff_bands \[16\] = \{", 16)),
    ("", "ZIGZAG", "u8", ints(r"static const unsigned int zigzag\[16\]", 16, zz_at)),
    ("", "PCAT1", "u8", ints(r"const Prob Pcat1\[\]", 2)),
    ("", "PCAT2", "u8", ints(r"const Prob Pcat2\[\]", 3)),
    ("", "PCAT3", "u8", ints(r"const Prob Pcat3\[\]", 4)),
    ("", "PCAT4", "u8", ints(r"const Prob Pcat4\[\]", 5)),
    ("", "PCAT5", "u8", ints(r"const Prob Pcat5\[\]", 6)),
    ("", "PCAT6", "u8", ints(r"const Prob Pcat6\[\] =", 12)),
    ("[plane][band][context][node] (section 13.4).", "COEFF_UPDATE_PROBS", "u8", ints(r"const Prob coeff_update_probs \[4\] \[8\] \[3\] \[num_dct_tokens-1\] =", 1056)),
    ("[plane][band][context][node] (section 13.5).", "DEFAULT_COEFF_PROBS", "u8", ints(r"const Prob default_coeff_probs \[4\] \[8\] \[3\] \[num_dct_tokens - 1\] =", 1056)),
    ("Section 14.1.", "DC_QLOOKUP", "u16", ints(r"static const int dc_qlookup\[QINDEX_RANGE\]", 128)),
    ("", "AC_QLOOKUP", "u16", ints(r"static const int ac_qlookup\[QINDEX_RANGE\]", 128)),
    ("Row then column (section 17.2).", "MV_UPDATE_PROBS", "u8", ints(r"const MV_CONTEXT vp8_mv_update_probs\[2\]", 38)),
    ("", "DEFAULT_MV_CONTEXT", "u8", ints(r"const MV_CONTEXT default_mv_context\[2\]", 38)),
    ("Six-tap filters by eighth-pixel offset (section 18.3).", "SIXTAP_FILTERS", "i16", ints(r"const int filters \[8\] \[6\] = \{", 48)),
    ("", "BILINEAR_FILTERS", "i16", ints(r"const int BilinearFilters\[8\]\[6\]", 48)),
]

o = ["//! VP8 constant tables, generated from the text of RFC 6386 by",
     "//! `tools/corpus/specular-vp8-tables.py`; do not edit.",
     "// The RFC's layout is kept; arrays are flattened row-major.",
     "#![allow(dead_code)]",
     ""]
for doc, name, ty, vals in tables:
    if doc:
        o.append(f"/// {doc}")
    o.append(f"pub const {name}: [{ty}; {len(vals)}] = [")
    per = 16 if len(vals) > 16 else len(vals)
    for k in range(0, len(vals), per):
        o.append("    " + ", ".join(str(v) for v in vals[k : k + per]) + ",")
    o.append("];")
    o.append("")
open(out, "w", encoding="utf-8").write("\n".join(o))
# The committed file is rustfmt's layout of this output.
subprocess.run(["rustfmt", "--edition", "2024", out], check=True)
print(f"wrote {len(tables)} tables to {out}")
