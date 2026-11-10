"""CABAC context initialisation tables of ITU-T H.266 (V4, 01/2026) as Rust.

Usage: python3 -I tools/corpus/vvc-ctx-tables.py h266.txt > crates/formats/specular-vvc/src/cabac/init.rs
where h266.txt is `pdftotext -layout` of T-REC-H.266-202601-I!!PDF-E (free from itu.int).
"""
import re
import sys

L = open(sys.argv[1], encoding='utf-8').read().replace('−', '-').split('\n')
first = next(i for i, l in enumerate(L) if re.match(r'\s*9\.3\.2\.2\s+Initialization process for context', l))
last = next(i for i in range(first, len(L)) if re.match(r'\s*9\.3\.2\.3\s+Storage process', L[i]))
starts = []
for i in range(first, last):
    m = re.match(r'\s*Table (\d+) – Specification of initValue and shiftIdx for', L[i])
    if m:
        starts.append((i, int(m[1])))
res = {}
for k, (i, t) in enumerate(starts):
    e = starts[k + 1][0] if k + 1 < len(starts) else last
    iv = []
    sh = []
    for l in L[i:e]:
        s = l.strip()
        if s.startswith('initValue'):
            iv += [int(x) for x in s.split()[1:]]
        elif s.startswith('shiftIdx'):
            sh += [int(x) for x in s.split()[1:]]
    a, b = res.get(t, ([], []))
    res[t] = (a + iv, b + sh)
names = {52: 'ALF_CTB_FLAG', 53: 'ALF_USE_APS_FLAG', 54: 'ALF_CTB_CC_CB_IDC', 55: 'ALF_CTB_CC_CR_IDC',
         56: 'ALF_CTB_FILTER_ALT_IDX', 57: 'SAO_MERGE_FLAG', 58: 'SAO_TYPE_IDX', 59: 'SPLIT_CU_FLAG',
         60: 'SPLIT_QT_FLAG', 61: 'MTT_SPLIT_CU_VERTICAL_FLAG', 62: 'MTT_SPLIT_CU_BINARY_FLAG',
         63: 'NON_INTER_FLAG', 64: 'CU_SKIP_FLAG', 65: 'PRED_MODE_IBC_FLAG', 66: 'PRED_MODE_FLAG',
         67: 'PRED_MODE_PLT_FLAG', 68: 'CU_ACT_ENABLED_FLAG', 69: 'INTRA_BDPCM_LUMA_FLAG',
         70: 'INTRA_BDPCM_LUMA_DIR_FLAG', 71: 'INTRA_MIP_FLAG', 72: 'INTRA_LUMA_REF_IDX',
         73: 'INTRA_SUBPARTITIONS_MODE_FLAG', 74: 'INTRA_SUBPARTITIONS_SPLIT_FLAG',
         75: 'INTRA_LUMA_MPM_FLAG', 76: 'INTRA_LUMA_NOT_PLANAR_FLAG', 77: 'INTRA_BDPCM_CHROMA_FLAG',
         78: 'INTRA_BDPCM_CHROMA_DIR_FLAG', 79: 'CCLM_MODE_FLAG', 80: 'CCLM_MODE_IDX',
         81: 'INTRA_CHROMA_PRED_MODE', 82: 'GENERAL_MERGE_FLAG', 83: 'INTER_PRED_IDC',
         84: 'INTER_AFFINE_FLAG', 85: 'CU_AFFINE_TYPE_FLAG', 86: 'SYM_MVD_FLAG', 87: 'REF_IDX',
         88: 'MVP_FLAG', 89: 'AMVR_FLAG', 90: 'AMVR_PRECISION_IDX', 91: 'BCW_IDX', 92: 'CU_CODED_FLAG',
         93: 'CU_SBT_FLAG', 94: 'CU_SBT_QUAD_FLAG', 95: 'CU_SBT_HORIZONTAL_FLAG', 96: 'CU_SBT_POS_FLAG',
         97: 'LFNST_IDX', 98: 'MTS_IDX', 99: 'COPY_ABOVE_PALETTE_INDICES_FLAG',
         100: 'PALETTE_TRANSPOSE_FLAG', 101: 'RUN_COPY_FLAG', 102: 'REGULAR_MERGE_FLAG',
         103: 'MMVD_MERGE_FLAG', 104: 'MMVD_CAND_FLAG', 105: 'MMVD_DISTANCE_IDX', 106: 'CIIP_FLAG',
         107: 'MERGE_SUBBLOCK_FLAG', 108: 'MERGE_SUBBLOCK_IDX', 109: 'MERGE_IDX',
         110: 'ABS_MVD_GREATER0_FLAG', 111: 'ABS_MVD_GREATER1_FLAG', 112: 'TU_Y_CODED_FLAG',
         113: 'TU_CB_CODED_FLAG', 114: 'TU_CR_CODED_FLAG', 115: 'CU_QP_DELTA_ABS',
         116: 'CU_CHROMA_QP_OFFSET_FLAG', 117: 'CU_CHROMA_QP_OFFSET_IDX', 118: 'TRANSFORM_SKIP_FLAG',
         119: 'TU_JOINT_CBCR_RESIDUAL_FLAG', 120: 'LAST_SIG_COEFF_X_PREFIX',
         121: 'LAST_SIG_COEFF_Y_PREFIX', 122: 'SB_CODED_FLAG', 123: 'SIG_COEFF_FLAG',
         124: 'PAR_LEVEL_FLAG', 125: 'ABS_LEVEL_GTX_FLAG', 126: 'COEFF_SIGN_FLAG'}
# Elements with contexts for initType 1 and 2 only (Table 51): contexts per initType.
two = {63: 2, 66: 2, 83: 6, 84: 3, 85: 1, 86: 1, 87: 2, 89: 2, 91: 1, 93: 2, 94: 1, 95: 3, 96: 1,
       102: 2, 103: 1, 104: 1, 105: 1, 106: 1, 107: 3, 108: 1}
out = []
off = 0
rows = [[], [], []]
for t in range(52, 127):
    iv, sh = res[t]
    if t in two:
        n = two[t]
        assert len(iv) == 2 * n, (t, len(iv))
        per = [None, (iv[0:n], sh[0:n]), (iv[n:2 * n], sh[n:2 * n])]
        per[0] = per[1]
    else:
        assert len(iv) % 3 == 0, (t, len(iv))
        n = len(iv) // 3
        per = [(iv[k * n:(k + 1) * n], sh[k * n:(k + 1) * n]) for k in range(3)]
    out.append(f"pub const {names[t]}: usize = {off}; // {n} (Table {t})")
    for k in range(3):
        rows[k] += [f"({a}, {b})" for a, b in zip(*per[k])]
    off += n
print("//! CABAC context initialisation (Tables 51 to 126 of ITU-T H.266 V4), extracted from")
print("//! the Recommendation's text by tools/corpus/vvc-ctx-tables.py. The initType 0 entries of")
print("//! elements that only exist in P and B slices repeat the initType 1 values (never used).\n")
print("\n".join(out))
print(f"/// Number of context variables.\npub const NUM_CTX: usize = {off};")
print("/// (initValue, shiftIdx) per initType.")
print("#[rustfmt::skip]")
print("pub static INIT: [[(u8, u8); NUM_CTX]; 3] = [")
for k in range(3):
    print("    [" + ", ".join(rows[k]) + "],")
print("];")
