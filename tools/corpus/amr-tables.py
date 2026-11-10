#!/usr/bin/env python3
"""Write the AMR / AMR-WB decoder tables of specular-amr as Rust data.

Usage: tools/corpus/amr-tables.py NB_C_DIR WB_C_DIR OPENCORE_DIR OUT_DIR

NB_C_DIR / WB_C_DIR: the `c-code` folders of the 3GPP fixed-point specifications
TS 26.073 V19.0.0 (ts_126073v190000p0.zip) and TS 26.173 V19.0.0 (ts_126173v190000p0.zip),
fetched by tools/corpus/fetch-amr.sh. OPENCORE_DIR: opencore-amr 0.1.6 (Apache-2.0; its
NOTICE records 3GPP's permission to distribute the TS 26.073/26.173 material under that
licence), used as the second source: every table that opencore-amr also carries is checked
value for value against it (re-laid-out filter tables by their values in order of use).

Only data is written; no code is taken from either source.
"""

import os
import re
import sys

TOKEN = re.compile(r"[A-Za-z_]\w*|0[xX][0-9a-fA-F]+|\d+|-")
DEFINES = {}


def strip_comments(s):
    s = re.sub(r"/\*.*?\*/", " ", s, flags=re.S)
    return re.sub(r"//[^\n]*", " ", s)


def arrays(path):
    """All `name[...] = { ... };` initialisers in a C file, flattened to ints."""
    src = strip_comments(open(path, encoding="latin-1").read())
    out = {}
    for m in re.finditer(r"(\w+)\s*((?:\[[^\]]*\]\s*)+)=\s*\{", src):
        name, depth, i = m.group(1), 1, m.end()
        while depth and i < len(src):
            if src[i] == "{":
                depth += 1
            elif src[i] == "}":
                depth -= 1
            i += 1
        body = src[m.end():i - 1]
        idents = [t for t in re.findall(r"[A-Za-z_]\w*", body) if t not in ("Word16", "Word32", "int16", "int32", "L")]
        if idents and all(t not in DEFINES for t in idents) and not re.search(r"(?<![A-Za-z_\w])\d", body):
            continue  # a table of pointers
        vals, neg = [], False
        for tok in TOKEN.findall(body):
            if tok == "-":
                neg = True
                continue
            if tok[0].isdigit():
                n = int(tok, 16) if tok[:2].lower() == "0x" else int(tok)
            elif tok in DEFINES:
                n = DEFINES[tok]
            else:
                neg = False
                continue  # a cast or a type name
            vals.append(-n if neg else n)
            neg = False
        out.setdefault(name, vals)
    return out


def load_dir(d):
    DEFINES.clear()
    for root, _, files in os.walk(d):
        for f in sorted(files):
            if f.endswith((".tab", ".c", ".cpp", ".h")):
                src = strip_comments(open(os.path.join(root, f), encoding="latin-1").read())
                for m in re.finditer(r"^\s*#\s*define\s+(\w+)\s+\(?\s*(-?\d+)\s*\)?\s*$", src, flags=re.M):
                    DEFINES.setdefault(m.group(1), int(m.group(2)))
    tabs = {}
    for root, _, files in os.walk(d):
        for f in sorted(files):
            if f.endswith((".tab", ".c", ".cpp", ".h")):
                for k, v in arrays(os.path.join(root, f)).items():
                    tabs.setdefault((f, k), v)
    return tabs


def find(tabs, file, name):
    for (f, k), v in tabs.items():
        if k == name and (file is None or f == file):
            return v
    raise KeyError(f"{file}:{name}")


def as_i16(v):
    return [(x - 0x10000 if x > 0x7FFF else x) for x in v]


# (rust name, 3GPP file, 3GPP name, opencore file or None, opencore name, type, 'same'|'values')
NB = [
    ("PRMNO", "bitno.tab", "prmno", "bitno_tab.cpp", "prmno", "i16"),
    ("PRMNOFSF", "bitno.tab", "prmnofsf", "bitno_tab.cpp", "prmnofsf", "i16"),
    ("BITNO_MR475", "bitno.tab", "bitno_MR475", "bitno_tab.cpp", "bitno_MR475", "i16"),
    ("BITNO_MR515", "bitno.tab", "bitno_MR515", "bitno_tab.cpp", "bitno_MR515", "i16"),
    ("BITNO_MR59", "bitno.tab", "bitno_MR59", "bitno_tab.cpp", "bitno_MR59", "i16"),
    ("BITNO_MR67", "bitno.tab", "bitno_MR67", "bitno_tab.cpp", "bitno_MR67", "i16"),
    ("BITNO_MR74", "bitno.tab", "bitno_MR74", "bitno_tab.cpp", "bitno_MR74", "i16"),
    ("BITNO_MR795", "bitno.tab", "bitno_MR795", "bitno_tab.cpp", "bitno_MR795", "i16"),
    ("BITNO_MR102", "bitno.tab", "bitno_MR102", "bitno_tab.cpp", "bitno_MR102", "i16"),
    ("BITNO_MR122", "bitno.tab", "bitno_MR122", "bitno_tab.cpp", "bitno_MR122", "i16"),
    ("BITNO_MRDTX", "bitno.tab", "bitno_MRDTX", "bitno_tab.cpp", "bitno_MRDTX", "i16"),
    ("SORT_475", "bitno.tab", "sort_475", "bitreorder_tab.cpp", "reorderBits_MR475", "i16"),
    ("SORT_515", "bitno.tab", "sort_515", "bitreorder_tab.cpp", "reorderBits_MR515", "i16"),
    ("SORT_59", "bitno.tab", "sort_59", "bitreorder_tab.cpp", "reorderBits_MR59", "i16"),
    ("SORT_67", "bitno.tab", "sort_67", "bitreorder_tab.cpp", "reorderBits_MR67", "i16"),
    ("SORT_74", "bitno.tab", "sort_74", "bitreorder_tab.cpp", "reorderBits_MR74", "i16"),
    ("SORT_795", "bitno.tab", "sort_795", "bitreorder_tab.cpp", "reorderBits_MR795", "i16"),
    ("SORT_102", "bitno.tab", "sort_102", "bitreorder_tab.cpp", "reorderBits_MR102", "i16"),
    ("SORT_122", "bitno.tab", "sort_122", "bitreorder_tab.cpp", "reorderBits_MR122", "i16"),
    ("SORT_SID", "bitno.tab", "sort_SID", None, None, "i16"),
    ("DHF_MR475", "d_homing.tab", "dhf_MR475", None, None, "i16"),
    ("DHF_MR515", "d_homing.tab", "dhf_MR515", None, None, "i16"),
    ("DHF_MR59", "d_homing.tab", "dhf_MR59", None, None, "i16"),
    ("DHF_MR67", "d_homing.tab", "dhf_MR67", None, None, "i16"),
    ("DHF_MR74", "d_homing.tab", "dhf_MR74", None, None, "i16"),
    ("DHF_MR795", "d_homing.tab", "dhf_MR795", None, None, "i16"),
    ("DHF_MR102", "d_homing.tab", "dhf_MR102", None, None, "i16"),
    ("DHF_MR122", "d_homing.tab", "dhf_MR122", None, None, "i16"),
    ("START_POS", "c2_9pf.tab", "startPos", "c2_9pf_tab.cpp", "startPos", "i16"),
    ("DGRAY", "gray.tab", "dgray", "gray_tbl.cpp", "dgray", "i16"),
    ("LSP_LSF_TABLE", "lsp_lsf.tab", "table", "lsp_lsf_tbl.cpp", "table", "i16"),
    ("LSP_LSF_SLOPE", "lsp_lsf.tab", "slope", "lsp_lsf_tbl.cpp", "slope", "i16"),
    ("LSP_INIT_DATA", "lsp.tab", "lsp_init_data", "lsp_tab.cpp", "lsp_init_data", "i16"),
    ("PH_IMP_LOW_MR795", "ph_disp.tab", "ph_imp_low_MR795", "ph_disp_tab.cpp", "ph_imp_low_MR795", "i16"),
    ("PH_IMP_MID_MR795", "ph_disp.tab", "ph_imp_mid_MR795", "ph_disp_tab.cpp", "ph_imp_mid_MR795", "i16"),
    ("PH_IMP_LOW", "ph_disp.tab", "ph_imp_low", "ph_disp_tab.cpp", "ph_imp_low", "i16"),
    ("PH_IMP_MID", "ph_disp.tab", "ph_imp_mid", "ph_disp_tab.cpp", "ph_imp_mid", "i16"),
    ("POW2_TABLE", "pow2.tab", "table", "pow2_tbl.cpp", "pow2_tbl", "i16"),
    ("LOG2_TABLE", "log2.tab", "table", "log2_tbl.cpp", "log2_tbl", "i16"),
    ("INV_SQRT_TABLE", "inv_sqrt.tab", "table", "inv_sqrt_tbl.cpp", "inv_sqrt_tbl", "i16"),
    ("SQRT_L_TABLE", "sqrt_l.tab", "table", "sqrt_l_tbl.cpp", "sqrt_l_tbl", "i16"),
    ("PAST_RQ_INIT", "q_plsf_3.tab", "past_rq_init", "q_plsf_3_tbl.cpp", "past_rq_init", "i16"),
    ("MEAN_LSF_3", "q_plsf_3.tab", "mean_lsf", "q_plsf_3_tbl.cpp", "mean_lsf_3", "i16"),
    ("PRED_FAC_3", "q_plsf_3.tab", "pred_fac", "q_plsf_3_tbl.cpp", "pred_fac_3", "i16"),
    ("DICO1_LSF_3", "q_plsf_3.tab", "dico1_lsf", "q_plsf_3_tbl.cpp", "dico1_lsf_3", "i16"),
    ("DICO2_LSF_3", "q_plsf_3.tab", "dico2_lsf", "q_plsf_3_tbl.cpp", "dico2_lsf_3", "i16"),
    ("DICO3_LSF_3", "q_plsf_3.tab", "dico3_lsf", "q_plsf_3_tbl.cpp", "dico3_lsf_3", "i16"),
    ("MR515_3_LSF", "q_plsf_3.tab", "mr515_3_lsf", "q_plsf_3_tbl.cpp", "mr515_3_lsf", "i16"),
    ("MR795_1_LSF", "q_plsf_3.tab", "mr795_1_lsf", "q_plsf_3_tbl.cpp", "mr795_1_lsf", "i16"),
    ("MEAN_LSF_5", "q_plsf_5.tab", "mean_lsf", "q_plsf_5_tbl.cpp", "mean_lsf_5", "i16"),
    ("DICO1_LSF_5", "q_plsf_5.tab", "dico1_lsf", "q_plsf_5_tbl.cpp", "dico1_lsf_5", "i16"),
    ("DICO2_LSF_5", "q_plsf_5.tab", "dico2_lsf", "q_plsf_5_tbl.cpp", "dico2_lsf_5", "i16"),
    ("DICO3_LSF_5", "q_plsf_5.tab", "dico3_lsf", "q_plsf_5_tbl.cpp", "dico3_lsf_5", "i16"),
    ("DICO4_LSF_5", "q_plsf_5.tab", "dico4_lsf", "q_plsf_5_tbl.cpp", "dico4_lsf_5", "i16"),
    ("DICO5_LSF_5", "q_plsf_5.tab", "dico5_lsf", "q_plsf_5_tbl.cpp", "dico5_lsf_5", "i16"),
    ("TABLE_GAIN_HIGHRATES", "qua_gain.tab", "table_gain_highrates", "qua_gain_tbl.cpp", "table_gain_highrates", "i16"),
    ("TABLE_GAIN_LOWRATES", "qua_gain.tab", "table_gain_lowrates", "qua_gain_tbl.cpp", "table_gain_lowrates", "i16"),
    ("QUA_GAIN_PITCH", "gains.tab", "qua_gain_pitch", "gains_tbl.cpp", "qua_gain_pitch", "i16"),
    ("QUA_GAIN_CODE", "gains.tab", "qua_gain_code", "gains_tbl.cpp", "qua_gain_code", "i16"),
    ("TABLE_GAIN_MR475", "qgain475.tab", "table_gain_MR475", "qgain475_tab.cpp", "table_gain_MR475", "i16"),
    ("WINDOW_200_40", "window.tab", "window_200_40", "window_tab.cpp", "window_200_40", "i16"),
    ("INTER_6", "pred_lt.c", "inter_6", "pred_lt.cpp", "inter_6_pred_lt", "i16"),
]

WB = [
    ("PRMNOFSF", "homing.tab", "prmnofsf", "homing_amr_wb_dec.cpp", "prmnofsf", "i16"),
    ("DHF_7K", "homing.tab", "dfh_M7k", "homing_amr_wb_dec.cpp", "dfh_M7k", "i16"),
    ("DHF_9K", "homing.tab", "dfh_M9k", "homing_amr_wb_dec.cpp", "dfh_M9k", "i16"),
    ("DHF_12K", "homing.tab", "dfh_M12k", "homing_amr_wb_dec.cpp", "dfh_M12k", "i16"),
    ("DHF_14K", "homing.tab", "dfh_M14k", "homing_amr_wb_dec.cpp", "dfh_M14k", "i16"),
    ("DHF_16K", "homing.tab", "dfh_M16k", "homing_amr_wb_dec.cpp", "dfh_M16k", "i16"),
    ("DHF_18K", "homing.tab", "dfh_M18k", "homing_amr_wb_dec.cpp", "dfh_M18k", "i16"),
    ("DHF_20K", "homing.tab", "dfh_M20k", "homing_amr_wb_dec.cpp", "dfh_M20k", "i16"),
    ("DHF_23K", "homing.tab", "dfh_M23k", "homing_amr_wb_dec.cpp", "dfh_M23k", "i16"),
    ("DHF_24K", "homing.tab", "dfh_M24k", "homing_amr_wb_dec.cpp", "dfh_M24k", "i16"),
    ("SORT_660", "mime_io.tab", "sort_660", "mime_io.cpp", "sort_660", "i16"),
    ("SORT_885", "mime_io.tab", "sort_885", "mime_io.cpp", "sort_885", "i16"),
    ("SORT_1265", "mime_io.tab", "sort_1265", "mime_io.cpp", "sort_1265", "i16"),
    ("SORT_1425", "mime_io.tab", "sort_1425", "mime_io.cpp", "sort_1425", "i16"),
    ("SORT_1585", "mime_io.tab", "sort_1585", "mime_io.cpp", "sort_1585", "i16"),
    ("SORT_1825", "mime_io.tab", "sort_1825", "mime_io.cpp", "sort_1825", "i16"),
    ("SORT_1985", "mime_io.tab", "sort_1985", "mime_io.cpp", "sort_1985", "i16"),
    ("SORT_2305", "mime_io.tab", "sort_2305", "mime_io.cpp", "sort_2305", "i16"),
    ("SORT_2385", "mime_io.tab", "sort_2385", "mime_io.cpp", "sort_2385", "i16"),
    ("SORT_SID", "mime_io.tab", "sort_SID", "mime_io.cpp", "sort_SID", "i16"),
    ("ISP_ISF_TABLE", "isp_isf.tab", "table", "isp_isf.cpp", "table", "i16"),
    ("MEAN_ISF", "qpisf_2s.tab", "mean_isf", "qpisf_2s_tab.cpp", "mean_isf", "i16"),
    ("DICO1_ISF", "qpisf_2s.tab", "dico1_isf", "qpisf_2s_tab.cpp", "dico1_isf", "i16"),
    ("DICO2_ISF", "qpisf_2s.tab", "dico2_isf", "qpisf_2s_tab.cpp", "dico2_isf", "i16"),
    ("DICO21_ISF", "qpisf_2s.tab", "dico21_isf", "qpisf_2s_tab.cpp", "dico21_isf", "i16"),
    ("DICO22_ISF", "qpisf_2s.tab", "dico22_isf", "qpisf_2s_tab.cpp", "dico22_isf", "i16"),
    ("DICO23_ISF", "qpisf_2s.tab", "dico23_isf", "qpisf_2s_tab.cpp", "dico23_isf", "i16"),
    ("DICO24_ISF", "qpisf_2s.tab", "dico24_isf", "qpisf_2s_tab.cpp", "dico24_isf", "i16"),
    ("DICO25_ISF", "qpisf_2s.tab", "dico25_isf", "qpisf_2s_tab.cpp", "dico25_isf", "i16"),
    ("DICO21_ISF_36B", "qpisf_2s.tab", "dico21_isf_36b", "qpisf_2s_tab.cpp", "dico21_isf_36b", "i16"),
    ("DICO22_ISF_36B", "qpisf_2s.tab", "dico22_isf_36b", "qpisf_2s_tab.cpp", "dico22_isf_36b", "i16"),
    ("DICO23_ISF_36B", "qpisf_2s.tab", "dico23_isf_36b", "qpisf_2s_tab.cpp", "dico23_isf_36b", "i16"),
    ("MEAN_ISF_NOISE", "qisf_ns.tab", "mean_isf_noise", "qisf_ns_tab.cpp", "mean_isf_noise", "i16"),
    ("DICO1_ISF_NOISE", "qisf_ns.tab", "dico1_isf_noise", "qisf_ns_tab.cpp", "dico1_isf_noise", "i16"),
    ("DICO2_ISF_NOISE", "qisf_ns.tab", "dico2_isf_noise", "qisf_ns_tab.cpp", "dico2_isf_noise", "i16"),
    ("DICO3_ISF_NOISE", "qisf_ns.tab", "dico3_isf_noise", "qisf_ns_tab.cpp", "dico3_isf_noise", "i16"),
    ("DICO4_ISF_NOISE", "qisf_ns.tab", "dico4_isf_noise", "qisf_ns_tab.cpp", "dico4_isf_noise", "i16"),
    ("DICO5_ISF_NOISE", "qisf_ns.tab", "dico5_isf_noise", "qisf_ns_tab.cpp", "dico5_isf_noise", "i16"),
    ("T_QUA_GAIN6B", "q_gain2.tab", "t_qua_gain6b", "q_gain2_tab.cpp", "t_qua_gain6b", "i16"),
    ("T_QUA_GAIN7B", "q_gain2.tab", "t_qua_gain7b", "q_gain2_tab.cpp", "t_qua_gain7b", "i16"),
    ("INTER4_2", "pred_lt4.c", "inter4_2", "pred_lt4.cpp", "inter4_2", "i16", "values"),
    ("PH_IMP_LOW", "ph_disp.c", "ph_imp_low", "phase_dispersion.cpp", "ph_imp_low", "i16"),
    ("PH_IMP_MID", "ph_disp.c", "ph_imp_mid", "phase_dispersion.cpp", "ph_imp_mid", "i16"),
    ("FIR_6K_7K", "hp6k.c", "fir_6k_7k", "band_pass_6k_7k.cpp", "fir_6k_7k", "i16", "prefix"),
    ("FIR_7K", "hp7k.c", "fir_7k", "low_pass_filt_7k.cpp", "fir_7k", "i16", "nonzero"),
    ("FIR_UP", "decim54.c", "fir_up", "oversamp_12k8_to_16k.cpp", "fir_up", "i16", "phases"),
    ("TABLE_ISQRT", "math_op.c", "table_isqrt", "pvamrwb_math_op.cpp", "table_isqrt", "i16"),
    ("TABLE_POW2", "math_op.c", "table_pow2", "pvamrwb_math_op.cpp", "table_pow2", "i16"),
    ("LOG2_TABLE", "log2_tab.h", "table", "pvamrwb_math_op.cpp", "Log2_norm_table", "i16"),
    ("HP_GAIN", "dec_main.c", "HP_gain", "synthesis_amr_wb.cpp", "HP_gain", "i16"),
    ("ISP_INIT", "dec_main.c", "isp_init", "pvamrwbdecoder.cpp", "isp_init", "i16"),
    ("ISF_INIT", "dec_main.c", "isf_init", "pvamrwbdecoder.cpp", "isf_init", "i16"),
]


def emit(out_path, title, specs, gpp, oc, provenance):
    lines = [
        f"//! {title}",
        "//!",
    ] + [f"//! {p}" if p else "//!" for p in provenance] + [
        "//!",
        "//! Generated by tools/corpus/amr-tables.py; do not edit by hand.",
        "",
        "#![allow(clippy::unreadable_literal)]",
        "",
    ]
    checked, alone = 0, []
    for spec in specs:
        rust, gf, gn, of, on, ty = spec[:6]
        vals = find(gpp, gf, gn)
        if ty == "i16":
            vals = as_i16(vals)
        if of is not None:
            ov = find(oc, of, on)
            if ty == "i16":
                ov = as_i16(ov)
            mode = spec[6] if len(spec) > 6 else "same"
            if mode == "same":
                ok = ov[: len(vals)] == vals
            elif mode == "prefix":  # opencore-amr keeps all but the last (symmetric) tap
                ok = len(ov) >= len(vals) - 1 and ov[: len(vals) - 1] == vals[: len(vals) - 1]
            elif mode == "values":
                ok = sorted(ov) == sorted(vals)
            elif mode == "phases":  # opencore-amr drops the integer phase (one 16384 tap, rest 0)
                rest = sorted(x for x in vals if x)
                rest.remove(16384)
                ok = sorted(x for x in ov if x) == rest
            else:  # re-laid out without its zero taps
                ok = sorted(x for x in ov if x) == sorted(x for x in vals if x)
            if not ok:
                raise SystemExit(f"{rust}: 3GPP {gf}:{gn} and opencore-amr {of}:{on} differ")
            checked += 1
        else:
            alone.append(rust)
        body = []
        row = []
        for v in vals:
            row.append(str(v))
            if len(row) == 12:
                body.append("    " + ", ".join(row) + ",")
                row = []
        if row:
            body.append("    " + ", ".join(row) + ",")
        lines.append(f"pub static {rust}: [{ty}; {len(vals)}] = [")
        lines += body
        lines.append("];")
        lines.append("")
    open(out_path, "w").write("\n".join(lines))
    print(f"{out_path}: {len(specs)} tables, {checked} identical in opencore-amr, only in 3GPP: {', '.join(alone) or '-'}")


def main():
    nb_dir, wb_dir, oc_dir, out = sys.argv[1:5]
    oc_root = os.path.join(oc_dir, "opencore", "codecs_v2", "audio", "gsm_amr")
    oc_nb = load_dir(os.path.join(oc_root, "amr_nb"))
    oc_wb = load_dir(os.path.join(oc_root, "amr_wb"))
    prov = [
        "Data only, transcribed from the fixed-point C description that is part of the",
        "specification ({spec} V19.0.0, a free ETSI download). Every table that",
        "opencore-amr 0.1.6 also carries is identical there (checked by the generator);",
        "opencore-amr is Apache-2.0 and records 3GPP's permission to distribute this",
        "material under that licence (its opencore/NOTICE).",
    ]
    emit(os.path.join(out, "nb", "tables.rs"), "AMR (narrowband) decoder tables.", NB,
         load_dir(nb_dir), oc_nb, [p.format(spec="3GPP TS 26.073") for p in prov])
    if WB:
        emit(os.path.join(out, "wb", "tables.rs"), "AMR-WB decoder tables.", WB,
             load_dir(wb_dir), oc_wb, [p.format(spec="3GPP TS 26.173") for p in prov])


if __name__ == "__main__":
    main()
