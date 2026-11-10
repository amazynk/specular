//! The ISO/IEC 14496-3 data tables the AAC-LC core needs, and the codebook decoders built
//! from them. The Huffman codebooks and scalefactor band offsets are copied data
//! (`data/lc.rs`, provenance in `data/PROVENANCE.md`); the rest is computed or small.

use std::sync::OnceLock;

use crate::data::lc;
use crate::vlc::Vlc;

pub(crate) use lc::{SWB_LONG, SWB_SHORT};

/// TNS maximum bands for long and short windows by sampling frequency index (AAC-LC,
/// Table 4.156).
pub(crate) const TNS_MAX_BANDS_LONG: [u8; 13] =
    [31, 31, 34, 40, 42, 51, 46, 46, 42, 42, 42, 39, 39];
pub(crate) const TNS_MAX_BANDS_SHORT: [u8; 13] = [9, 9, 10, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14];

/// The last scalefactor band (exclusive) AAC Main prediction runs on, by sampling frequency
/// index (`PRED_SFB_MAX`, §4.6.7).
pub(crate) const PRED_SFB_MAX: [u8; 13] = [33, 33, 38, 40, 40, 40, 41, 41, 37, 37, 37, 34, 34];

/// Long-term prediction gains by `ltp_coef` (Table 4.147).
pub(crate) const LTP_COEF: [f32; 8] = [
    0.570_829, 0.696_616, 0.813_004, 0.911_304, 0.984_900, 1.067_894, 1.194_601, 1.369_533,
];

/// Bands long-term prediction may be used in (`MAX_LTP_LONG_SFB`).
pub(crate) const LTP_LONG_SFB: usize = 40;

/// The bands of the 1024-sample tables that start below `limit`, the last one ending there:
/// the scalefactor bands of 960-sample frames (§4.5.4: the 960 and 120 tables are the
/// 1024 and 128 ones cut at the shorter length).
fn cut(t: &[u16], limit: u16) -> Vec<u16> {
    let mut v: Vec<u16> = t.iter().copied().take_while(|&o| o < limit).collect();
    v.push(limit);
    v
}

/// Scalefactor band offsets of long windows for a frame length (1024 or 960).
pub(crate) fn swb_long(rate_index: usize, frame_len: usize) -> &'static [u16] {
    static CUT: OnceLock<Vec<Vec<u16>>> = OnceLock::new();
    if frame_len == 1024 {
        return SWB_LONG[rate_index];
    }
    let t = CUT.get_or_init(|| SWB_LONG.iter().map(|t| cut(t, 960)).collect());
    &t[rate_index]
}

/// Scalefactor band offsets of short windows for a frame length (1024 or 960).
pub(crate) fn swb_short(rate_index: usize, frame_len: usize) -> &'static [u16] {
    static CUT: OnceLock<Vec<Vec<u16>>> = OnceLock::new();
    if frame_len == 1024 {
        return SWB_SHORT[rate_index];
    }
    let t = CUT.get_or_init(|| SWB_SHORT.iter().map(|t| cut(t, 120)).collect());
    &t[rate_index]
}

/// Whether every table is present and well formed.
pub(crate) fn ready() -> bool {
    books().is_some()
        && SWB_LONG.iter().all(|t| t.last() == Some(&1024))
        && SWB_SHORT.iter().all(|t| t.last() == Some(&128))
}

/// The decoders for the scalefactor codebook and spectral codebooks 1–11.
pub(crate) struct Books {
    pub scalefactor: Vlc,
    pub spectral: Vec<Vlc>,
}

/// Builds (once) the codebook decoders; `None` if a table is malformed.
pub(crate) fn books() -> Option<&'static Books> {
    static BOOKS: OnceLock<Option<Books>> = OnceLock::new();
    BOOKS
        .get_or_init(|| {
            let list = |t: &[(u32, u8)]| -> Vec<(u32, u8, u32)> {
                t.iter()
                    .enumerate()
                    .map(|(i, &(c, l))| (c, l, i as u32))
                    .collect()
            };
            let scalefactor = Vlc::new(&list(&lc::SCALEFACTOR))?;
            let spectral = lc::SPECTRAL
                .iter()
                .map(|t| {
                    if t.is_empty() {
                        None
                    } else {
                        Vlc::new(&list(t))
                    }
                })
                .collect::<Option<Vec<_>>>()?;
            Some(Books {
                scalefactor,
                spectral,
            })
        })
        .as_ref()
}
