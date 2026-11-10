//! Context variable layout and initialisation values (clause 9.3.2.2).
//!
//! The contexts of the I-slice syntax come first, in the order they always
//! had; the inter-only ones follow, so an intra picture touches the same
//! leading block of the array it always did.

/// Offsets of each syntax element's context block in the flat context array.
pub mod off {
    /// `sao_merge_left_flag` and `sao_merge_up_flag`.
    pub const SAO_MERGE: usize = 0;
    /// `sao_type_idx_luma` and `sao_type_idx_chroma`.
    pub const SAO_TYPE: usize = 1;
    /// `split_cu_flag`, three contexts.
    pub const SPLIT_CU: usize = 2;
    /// `cu_transquant_bypass_flag`.
    pub const TQ_BYPASS: usize = 5;
    /// `part_mode`, first bin.
    pub const PART_MODE: usize = 6;
    /// `prev_intra_luma_pred_flag`.
    pub const PREV_INTRA: usize = 7;
    /// `intra_chroma_pred_mode`.
    pub const INTRA_CHROMA: usize = 8;
    /// `split_transform_flag`, three contexts.
    pub const SPLIT_TRANSFORM: usize = 9;
    /// `cbf_luma`, two contexts.
    pub const CBF_LUMA: usize = 12;
    /// `cbf_cb` and `cbf_cr`, five contexts.
    pub const CBF_CHROMA: usize = 14;
    /// `cu_qp_delta_abs`, two contexts.
    pub const CU_QP_DELTA: usize = 19;
    /// `transform_skip_flag` for luma.
    pub const TS_LUMA: usize = 21;
    /// `transform_skip_flag` for chroma.
    pub const TS_CHROMA: usize = 22;
    /// `last_sig_coeff_x_prefix`, eighteen contexts.
    pub const LAST_X: usize = 23;
    /// `last_sig_coeff_y_prefix`, eighteen contexts.
    pub const LAST_Y: usize = 41;
    /// `coded_sub_block_flag`, four contexts.
    pub const CSBF: usize = 59;
    /// `sig_coeff_flag`, forty-four contexts (two are range-extension only).
    pub const SIG: usize = 63;
    /// `coeff_abs_level_greater1_flag`, twenty-four contexts.
    pub const GT1: usize = 107;
    /// `coeff_abs_level_greater2_flag`, six contexts.
    pub const GT2: usize = 131;
    /// `cu_chroma_qp_offset_flag`.
    pub const CHROMA_QP_FLAG: usize = 137;
    /// `cu_chroma_qp_offset_idx`.
    pub const CHROMA_QP_IDX: usize = 138;
    /// `log2_res_scale_abs_plus1`, four contexts per chroma component.
    pub const RES_SCALE_ABS: usize = 139;
    /// `res_scale_sign_flag`, one context per chroma component.
    pub const RES_SCALE_SIGN: usize = 147;
    /// `cu_skip_flag`, three contexts.
    pub const SKIP: usize = 149;
    /// `pred_mode_flag`.
    pub const PRED_MODE: usize = 152;
    /// `part_mode` bins with contexts 1 to 3; `PART_MODE_EXT + k - 1` is
    /// context `k`.
    pub const PART_MODE_EXT: usize = 153;
    /// `merge_flag`.
    pub const MERGE_FLAG: usize = 156;
    /// `merge_idx`, first bin.
    pub const MERGE_IDX: usize = 157;
    /// `inter_pred_idc`, five contexts.
    pub const INTER_PRED_IDC: usize = 158;
    /// `ref_idx_l0` / `ref_idx_l1`, two contexts.
    pub const REF_IDX: usize = 163;
    /// `mvp_l0_flag` / `mvp_l1_flag`.
    pub const MVP_FLAG: usize = 165;
    /// `rqt_root_cbf`.
    pub const RQT_ROOT_CBF: usize = 166;
    /// `abs_mvd_greater0_flag`.
    pub const MVD_GT0: usize = 167;
    /// `abs_mvd_greater1_flag`.
    pub const MVD_GT1: usize = 168;
    /// `explicit_rdpcm_flag`, luma then chroma.
    pub const RDPCM_FLAG: usize = 169;
    /// `explicit_rdpcm_dir_flag`, luma then chroma.
    pub const RDPCM_DIR: usize = 171;
}

/// Total number of context variables tracked.
pub const NUM_CTX: usize = 173;

/// `initValue` for every context, for `initType == 0` (I slices). The
/// inter-only contexts have no type-0 values; they are never read in an I
/// slice and are filled with the neutral 154.
pub static INIT_VALUES: [u8; NUM_CTX] = [
    153, // sao_merge
    200, // sao_type_idx
    139, 141, 157, // split_cu_flag
    154, // cu_transquant_bypass_flag
    184, // part_mode
    184, // prev_intra_luma_pred_flag
    63,  // intra_chroma_pred_mode
    153, 138, 138, // split_transform_flag
    111, 141, // cbf_luma
    94, 138, 182, 154, 154, // cbf_cb / cbf_cr
    154, 154, // cu_qp_delta_abs
    139, // transform_skip_flag, luma
    139, // transform_skip_flag, chroma
    // last_sig_coeff_x_prefix
    110, 110, 124, 125, 140, 153, 125, 127, 140, 109, 111, 143, 127, 111, 79, 108, 123, 63,
    // last_sig_coeff_y_prefix
    110, 110, 124, 125, 140, 153, 125, 127, 140, 109, 111, 143, 127, 111, 79, 108, 123, 63,
    // coded_sub_block_flag
    91, 171, 134, 141,
    // sig_coeff_flag (42 core contexts then 2 for transform-skip contexts)
    111, 111, 125, 110, 110, 94, 124, 108, 124, 107, 125, 141, 179, 153, 125, 107, 125, 141, 179,
    153, 125, 107, 125, 141, 179, 153, 125, 140, 139, 182, 182, 152, 136, 152, 136, 153, 136, 139,
    111, 136, 139, 111, 141, 111, // coeff_abs_level_greater1_flag
    140, 92, 137, 138, 140, 152, 138, 139, 153, 74, 149, 92, 139, 107, 122, 152, 140, 179, 166,
    182, 140, 227, 122, 197, // coeff_abs_level_greater2_flag
    138, 153, 136, 167, 152, 152, // cu_chroma_qp_offset_flag
    154, // cu_chroma_qp_offset_idx
    154, // log2_res_scale_abs_plus1
    154, 154, 154, 154, 154, 154, 154, 154, // res_scale_sign_flag
    154, 154, // inter-only contexts, unused in I slices
    154, 154, 154, 154, 154, 154, 154, 154, 154, 154, 154, 154, 154, 154, 154, 154, 154, 154, 154,
    154, 154, 154, 154, 154, // explicit_rdpcm_flag and _dir_flag: inter only
];

/// `initValue` for `initType == 1`.
pub static INIT_VALUES_1: [u8; NUM_CTX] = [
    153, // sao_merge
    185, // sao_type_idx
    107, 139, 126, // split_cu_flag
    154, // cu_transquant_bypass_flag
    154, // part_mode, context 0
    154, // prev_intra_luma_pred_flag
    152, // intra_chroma_pred_mode
    124, 138, 94, // split_transform_flag
    153, 111, // cbf_luma
    149, 107, 167, 154, 154, // cbf_cb / cbf_cr
    154, 154, // cu_qp_delta_abs
    139, // transform_skip_flag, luma
    139, // transform_skip_flag, chroma
    // last_sig_coeff_x_prefix
    125, 110, 94, 110, 95, 79, 125, 111, 110, 78, 110, 111, 111, 95, 94, 108, 123, 108,
    // last_sig_coeff_y_prefix
    125, 110, 94, 110, 95, 79, 125, 111, 110, 78, 110, 111, 111, 95, 94, 108, 123, 108,
    // coded_sub_block_flag
    121, 140, 61, 154, // sig_coeff_flag
    155, 154, 139, 153, 139, 123, 123, 63, 153, 166, 183, 140, 136, 153, 154, 166, 183, 140, 136,
    153, 154, 166, 183, 140, 136, 153, 154, 170, 153, 123, 123, 107, 121, 107, 121, 167, 151, 183,
    140, 151, 183, 140, 140, 140, // coeff_abs_level_greater1_flag
    154, 196, 196, 167, 154, 152, 167, 182, 182, 134, 149, 136, 153, 121, 136, 137, 169, 194, 166,
    167, 154, 167, 137, 182, // coeff_abs_level_greater2_flag
    107, 167, 91, 122, 107, 167, // cu_chroma_qp_offset_flag
    154, // cu_chroma_qp_offset_idx
    154, // log2_res_scale_abs_plus1
    154, 154, 154, 154, 154, 154, 154, 154, // res_scale_sign_flag
    154, 154, // cu_skip_flag
    197, 185, 201, // pred_mode_flag
    149, // part_mode, contexts 1 to 3
    139, 154, 154, // merge_flag
    110, // merge_idx
    122, // inter_pred_idc
    95, 79, 63, 31, 31, // ref_idx
    153, 153, // mvp_flag
    168, // rqt_root_cbf
    79,  // abs_mvd_greater0_flag
    140, // abs_mvd_greater1_flag
    198, 139, 139, // explicit_rdpcm_flag
    139, 139, // explicit_rdpcm_dir_flag
];

/// `initValue` for `initType == 2`.
pub static INIT_VALUES_2: [u8; NUM_CTX] = [
    153, // sao_merge
    160, // sao_type_idx
    107, 139, 126, // split_cu_flag
    154, // cu_transquant_bypass_flag
    154, // part_mode, context 0
    183, // prev_intra_luma_pred_flag
    152, // intra_chroma_pred_mode
    224, 167, 122, // split_transform_flag
    153, 111, // cbf_luma
    149, 92, 167, 154, 154, // cbf_cb / cbf_cr
    154, 154, // cu_qp_delta_abs
    139, // transform_skip_flag, luma
    139, // transform_skip_flag, chroma
    // last_sig_coeff_x_prefix
    125, 110, 124, 110, 95, 94, 125, 111, 111, 79, 125, 126, 111, 111, 79, 108, 123, 93,
    // last_sig_coeff_y_prefix
    125, 110, 124, 110, 95, 94, 125, 111, 111, 79, 125, 126, 111, 111, 79, 108, 123, 93,
    // coded_sub_block_flag
    121, 140, 61, 154, // sig_coeff_flag
    170, 154, 139, 153, 139, 123, 123, 63, 124, 166, 183, 140, 136, 153, 154, 166, 183, 140, 136,
    153, 154, 166, 183, 140, 136, 153, 154, 170, 153, 138, 138, 122, 121, 122, 121, 167, 151, 183,
    140, 151, 183, 140, 140, 140, // coeff_abs_level_greater1_flag
    154, 196, 167, 167, 154, 152, 167, 182, 182, 134, 149, 136, 153, 121, 136, 122, 169, 208, 166,
    167, 154, 152, 167, 182, // coeff_abs_level_greater2_flag
    107, 167, 91, 107, 107, 167, // cu_chroma_qp_offset_flag
    154, // cu_chroma_qp_offset_idx
    154, // log2_res_scale_abs_plus1
    154, 154, 154, 154, 154, 154, 154, 154, // res_scale_sign_flag
    154, 154, // cu_skip_flag
    197, 185, 201, // pred_mode_flag
    134, // part_mode, contexts 1 to 3
    139, 154, 154, // merge_flag
    154, // merge_idx
    137, // inter_pred_idc
    95, 79, 63, 31, 31, // ref_idx
    153, 153, // mvp_flag
    168, // rqt_root_cbf
    79,  // abs_mvd_greater0_flag
    169, // abs_mvd_greater1_flag
    198, 139, 139, // explicit_rdpcm_flag
    139, 139, // explicit_rdpcm_dir_flag
];
