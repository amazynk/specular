//! Intra-only H.265 / HEVC decoder (ITU-T H.265 | ISO/IEC 23008-2).
//!
//! In-tree fork of `oxideav-h265` 0.0.14 (git 66097811d122959efcc9e07e20810d46ee18e3e2,
//! MIT, see `LICENSE`). Reduced to what HEIC still images need: the encoder,
//! the oxideav-core registry decoder and (see below) the inter-prediction /
//! multi-layer paths are removed. [`SequenceDecoder`] decodes length-prefixed
//! or Annex B NAL units to [`Picture`]s.
#![deny(unsafe_code)]
#![warn(missing_debug_implementations)]

// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub mod availability;
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub mod binarization;
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub mod bitreader;
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub mod cabac;
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub mod ctx_init;
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub mod deblock;
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub mod decode;
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub mod dpb;
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub mod hrd;
pub mod hvcc;
/// Annex H inter-layer reference picture derivation (resampling).
pub mod ilref;
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub mod inter_pred;
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub mod inter_recon;
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub mod intra_mode_field;
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub mod intra_pred;
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub mod motion;
pub mod nal;
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub mod palette;
pub mod picture;
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub mod poc;
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub mod pps;
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub mod pu_mv;
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub mod recon;
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub mod residual;
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub mod sao;
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub mod scaling_list;
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub mod scan;
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub mod sei;
pub mod sequence;
pub mod still;
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub mod slice;
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub mod slice_data;
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub mod sps;
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub mod transform;
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub mod transform_tree;
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub mod transform_unit;
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub mod vps;
/// Annex F `vps_extension( )` — the multi-layer layer model.
pub mod vps_ext;
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub mod vui;

// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub use availability::{AvailabilityError, PictureTiling, TilingParams};
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub use bitreader::{BitReader, BitReaderError};
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub use cabac::{init_type, CabacEngine, CabacError, ContextModel};
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub use ctx_init::SliceContexts;
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub use deblock::{
    deblock_picture, deblock_picture_full, derive_boundary_strength, derive_edge_flags,
    filter_cu_edges, filter_cu_edges_full, BoundaryStrength, DeblockCu, DeblockCuDesc,
    DeblockCuParams, EdgeFlags, EdgeType, NoFilterMap, TransformSplit,
};
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub use decode::{PictureHeaderInfo, PictureRefState, PictureSequenceState, SliceRefParams};
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub use dpb::{
    build_rps_poc_lists, no_backward_pred_flag, select_col_pic, Dpb, DpbEntry, LongTermEntry,
    Marking, RefPicListParams, RefPicLists, ResolvedRps, RpsPocLists,
};
pub use hrd::HrdError;
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub use hrd::{
    CpbEntry, HrdCommonInfo, HrdParameters, SubLayerHrd, SubLayerHrdParameters, VpsHrdEntry,
    HEVC_MAX_CPB_CNT, HEVC_MAX_ELEMENTAL_DURATION_IN_TC_MINUS1,
};
pub use hvcc::{
    extradata_is_hvcc, nal_unit_from_coded, parse_hvcc, split_length_prefixed, HvccError,
    HvccRecord,
};
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub use inter_pred::{
    default_weighted_pred, explicit_weighted_pred, interp_chroma_block, interp_luma_block,
    predict_inter_pu, predict_inter_pu_weighted, InterPredError, InterPredGeometry,
    InterPrediction, ListPrediction, MotionVector, PuWeights, RefPlane, WpListWeights,
};
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub use inter_recon::SliceWpTables;
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub use intra_mode_field::{IntraModeField, Neighbour, MIN_BLOCK_LOG2, MIN_BLOCK_SIZE};
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub use motion::{
    append_combined_bi_candidates, append_zero_merge_candidates, build_merge_candidate,
    derive_chroma_mv, derive_mvp_candidate, derive_spatial_merge_candidates, derive_temporal_mv,
    reconstruct_mv, MergeCandidate, MergeListParams, MotionCell, MotionField, Mv, MvpContext,
    NeighbourPu, PartitionContext, RefPicId, SpatialMergeCandidates, SpatialMergeNeighbours,
    TemporalMvContext,
};
pub use nal::{collect_nal_units, NalError, NalHeader, NalIter, NalUnit};
pub use picture::{clip1, sub_wh_c, Picture, Plane};
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub use poc::{diff_pic_order_cnt, NalKind, PicOrderCnt, PocState};
pub use pps::PpsError;
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub use pps::{
    ChromaQpOffsetListEntry, DeblockingFilterControl, PicParameterSet, PpsRangeExtension, TileInfo,
};
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub use pu_mv::{
    pu_partitions, resolve_cu_motion, resolve_pu_motion, InterCuDesc, PartMode as PuPartMode,
    PuGeometry, PuMotion, PuMvContext, PuRect,
};
pub use recon::ReconError;
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub use recon::{
    build_slice_addr_map, reconstruct_inter_pu, reconstruct_inter_pu_weighted,
    reconstruct_intra_ctu, reconstruct_intra_ctu_ctx, reconstruct_intra_picture,
    IntraPictureParams, PlacedCtu, ReconCtx, ReconParams, ResolvedList, SliceSegmentBoundary,
};
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub use sao::{
    apply_sao_ctb, apply_sao_picture, apply_sao_picture_full, apply_sao_picture_in_place,
    ResolvedSao, ResolvedSaoComponent,
};
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub use scaling_list::{
    ScalingFactorMatrix, ScalingFactors, ScalingListData, ScalingListError, ScalingListMatrix,
    MAX_COEF_NUM, NUM_MATRIX_IDS, NUM_SIZE_IDS,
};
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub use scan::{
    horizontal, scan_order, traverse, up_right_diagonal, vertical, ScanIdx, ScanOrderError, ScanPos,
};
pub use sequence::{decode_annexb_sequence, DecodedFrame, SequenceDecoder, SequenceError};
pub use slice::SliceError;
pub use still::{decode_still, StillError, StillOptions, StillPicture};
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub use slice::{
    EntryPointOffsets, NumPicTotalCurrInputs, PredWeightEntry, PredWeightTable,
    PredWeightTableInputs, RefPicListsModification, SliceDeblocking, SliceLongTermRefPic,
    SliceLongTermRefPicSource, SliceSegmentHeader, SliceType, BLA_W_LP, IDR_N_LP, IDR_W_RADL,
    RSV_IRAP_VCL23,
};
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub use slice_data::{
    decode_coding_quadtree, decode_coding_tree_unit, decode_coding_tree_unit_in_picture,
    decode_sao, CodingQuadtree, CodingTreeUnit, CodingUnit, IntraLumaMode, PcmSamples,
    PictureParseState, PredictionUnit, SaoComponent, SaoCtbParams, SliceDataParams,
};
pub use sps::{ShortTermRefPicSetMaterializeError, SpsError};
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub use sps::{
    ConformanceWindow, LongTermRefPicEntry, MaterializedShortTermRefPicSet, OpaqueTail, PcmInfo,
    SeqParameterSet, ShortTermRefPicSet, SpsExtensionFlags, SpsRangeExtension,
    HEVC_MAX_NUM_LONG_TERM_RPS, HEVC_MAX_NUM_SHORT_TERM_RPS, HEVC_MAX_RPS_PICS,
};
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub use transform_tree::{decode_transform_tree, TransformTree, TransformTreeParams};
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub use transform_unit::{
    decode_transform_unit, CuPredMode, QuantGroupState, TransformUnit, TransformUnitParams,
};
pub use vps::VpsError;
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub use vps::{
    HevcVps, LayerIdInclusionRow, ProfileTierLevel, SubLayerOrderingInfo, VpsTimingInfo,
    HEVC_MAX_SUB_LAYERS, HEVC_VPS_MAX_NUM_LAYERS, HEVC_VPS_MAX_NUM_LAYER_SETS,
};
pub use vui::VuiError;
// internal — exposed for tests/fuzz; not part of the stable API
#[doc(hidden)]
pub use vui::{
    BitstreamRestriction, ColourDescription, DefaultDisplayWindow, VideoSignalType, VuiParameters,
    VuiTimingInfo, EXTENDED_SAR,
};

/// Crate-local error type for the structural utilities (the NAL
/// walker and parameter-set parsers surface their own [`NalError`] /
/// [`VpsError`] types directly; the decode drivers use
/// [`sequence::SequenceError`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// The requested functionality is not implemented (the encoder,
    /// and the remaining decoder gaps listed in `README.md`).
    NotImplemented,
    /// A NAL-walker error surfaced through the top-level entry
    /// points.
    Nal(NalError),
    /// A VPS-parser error surfaced through the top-level entry
    /// points.
    Vps(VpsError),
    /// An SPS-parser error surfaced through the top-level entry
    /// points.
    Sps(SpsError),
    /// A PPS-parser error surfaced through the top-level entry
    /// points.
    Pps(PpsError),
    /// A slice-segment-header-parser error surfaced through the
    /// top-level entry points.
    Slice(SliceError),
    /// An `hrd_parameters()` parser error surfaced through the
    /// top-level entry points.
    Hrd(HrdError),
    /// A `vui_parameters()` parser error surfaced through the
    /// top-level entry points.
    Vui(VuiError),
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NotImplemented => f.write_str("oxideav-h265: decoder/encoder not wired up yet"),
            Self::Nal(e) => write!(f, "oxideav-h265 NAL error: {e}"),
            Self::Vps(e) => write!(f, "oxideav-h265 VPS error: {e}"),
            Self::Sps(e) => write!(f, "oxideav-h265 SPS error: {e}"),
            Self::Pps(e) => write!(f, "oxideav-h265 PPS error: {e}"),
            Self::Slice(e) => write!(f, "oxideav-h265 slice header error: {e}"),
            Self::Hrd(e) => write!(f, "oxideav-h265 hrd error: {e}"),
            Self::Vui(e) => write!(f, "oxideav-h265 vui error: {e}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<NalError> for Error {
    fn from(e: NalError) -> Self {
        Self::Nal(e)
    }
}

impl From<VpsError> for Error {
    fn from(e: VpsError) -> Self {
        Self::Vps(e)
    }
}

impl From<SpsError> for Error {
    fn from(e: SpsError) -> Self {
        Self::Sps(e)
    }
}

impl From<PpsError> for Error {
    fn from(e: PpsError) -> Self {
        Self::Pps(e)
    }
}

impl From<SliceError> for Error {
    fn from(e: SliceError) -> Self {
        Self::Slice(e)
    }
}

impl From<HrdError> for Error {
    fn from(e: HrdError) -> Self {
        Self::Hrd(e)
    }
}

impl From<VuiError> for Error {
    fn from(e: VuiError) -> Self {
        Self::Vui(e)
    }
}
