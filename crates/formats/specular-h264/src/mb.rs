//! Per-macroblock state kept for the whole picture: what neighbouring macroblocks need for
//! prediction and entropy-coding contexts, and what the deblocking filter needs.

pub const I4: u8 = 1;
pub const I8: u8 = 2;
pub const I16: u8 = 4;
pub const PCM: u8 = 8;
pub const INTER: u8 = 16;
pub const SKIP: u8 = 32;
/// B_Skip or B_Direct_16x16.
pub const DIRECT: u8 = 64;
pub const INTRA: u8 = I4 | I8 | I16 | PCM;

#[derive(Clone, Copy)]
pub struct MbInfo {
    /// Slice number within the picture, 1-based; 0 = not (yet) decoded.
    pub slice: u16,
    pub kind: u8,
    /// A field macroblock (in a field picture, or of a field pair in an MBAFF frame).
    pub field: bool,
    /// coded_block_pattern: luma bits 0..3, chroma in bits 4..5.
    pub cbp: u8,
    /// QP_Y as the deblocking filter sees it (0 for I_PCM; negative at high bit depths).
    pub qp: i8,
    /// QP_C (without the bit-depth offset) per chroma component for the deblocking filter.
    pub qpc: [i8; 2],
    pub chroma_pred: u8,
    pub t8: bool,
    /// CAVLC total_coeff per 4x4 block: luma raster 0..16, Cb 16..24, Cr 24..32 (chroma
    /// blocks in raster order of 2 columns: 4 for 4:2:0, 8 for 4:2:2); in 4:4:4 Cb 16..32
    /// and Cr 32..48 in the luma raster.
    pub nzc: [u8; 48],
    /// CABAC coded_block_flag: bits 0..48 the 4x4 blocks (as `nzc`), 48 luma DC, 49 Cb DC,
    /// 50 Cr DC, and in 4:4:4 the 8x8 blocks of plane c at 52 + 4 * c.
    pub cbf: u64,
    /// Luma 4x4 blocks (raster) with non-zero coefficients, for the deblocking filter.
    pub nzmask: u16,
    /// Intra4x4/8x8 prediction mode per 4x4 block (raster); 2 (DC) for other macroblocks.
    pub ipred: [u8; 16],
    /// Per list and 8x8 partition; -1 when the list is not used.
    pub ref_idx: [[i8; 4]; 2],
    /// Picture referenced per list and 8x8 partition (`RefPic::id`), 0 if none.
    pub ref_id: [[u32; 4]; 2],
    /// Per list and 4x4 block (raster).
    pub mv: [[[i16; 2]; 16]; 2],
    /// |mvd| per list and 4x4 block, clipped, for CABAC contexts.
    pub mvd: [[[u8; 2]; 16]; 2],
    /// 8x8 partitions predicted in direct mode (bit per partition).
    pub direct8: u8,
}

impl Default for MbInfo {
    fn default() -> MbInfo {
        MbInfo {
            slice: 0,
            kind: 0,
            field: false,
            cbp: 0,
            qp: 0,
            qpc: [0; 2],
            chroma_pred: 0,
            t8: false,
            nzc: [0; 48],
            cbf: 0,
            nzmask: 0,
            ipred: [2; 16],
            ref_idx: [[-1; 4]; 2],
            ref_id: [[0; 4]; 2],
            mv: [[[0; 2]; 16]; 2],
            mvd: [[[0; 2]; 16]; 2],
            direct8: 0,
        }
    }
}

impl MbInfo {
    #[inline(always)]
    pub fn intra(&self) -> bool {
        self.kind & INTRA != 0
    }
}
