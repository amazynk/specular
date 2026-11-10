//! Still-picture entry point (Specular addition): one HEIC coded item —
//! its `hvcC` record plus the length-prefixed item payload — to one
//! reconstructed, conformance-cropped picture.

use std::sync::atomic::AtomicBool;

use crate::hvcc::{parse_hvcc_with_len, split_length_prefixed, HvccError};
use crate::picture::Picture;
use crate::sequence::{SequenceDecoder, SequenceError};
use crate::vui::VideoSignalType;

/// Options for [`decode_still`].
#[derive(Debug, Clone, Copy, Default)]
pub struct StillOptions<'a> {
    /// Worker budget for the in-picture parallelism (WPP rows, loop
    /// filters); 0 and 1 both mean serial.
    pub threads: usize,
    /// Cooperative cancellation, polled once per NAL unit.
    pub cancel: Option<&'a AtomicBool>,
}

/// A decoded still picture.
#[derive(Debug)]
pub struct StillPicture {
    /// The output picture (conformance window applied).
    pub picture: Picture,
    /// The SPS VUI `video_signal_type` block, when signalled.
    pub signal: Option<VideoSignalType>,
}

/// Errors of [`decode_still`].
#[derive(Debug)]
pub enum StillError {
    /// The `hvcC` record or the length-prefixed payload is malformed.
    Hvcc(HvccError),
    /// A parse / decode error of the bitstream.
    Decode(SequenceError),
    /// The payload held no decodable picture.
    NoPicture,
    /// The cancellation flag was raised.
    Cancelled,
}

impl core::fmt::Display for StillError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Hvcc(e) => write!(f, "hvcC / sample framing: {e}"),
            Self::Decode(e) => write!(f, "{e}"),
            Self::NoPicture => f.write_str("no picture decoded"),
            Self::Cancelled => f.write_str("cancelled"),
        }
    }
}

impl std::error::Error for StillError {}

impl From<HvccError> for StillError {
    fn from(e: HvccError) -> Self {
        Self::Hvcc(e)
    }
}

impl From<SequenceError> for StillError {
    fn from(e: SequenceError) -> Self {
        Self::Decode(e)
    }
}

/// Decode one coded HEIC item: `hvcc` is the raw
/// `HEVCDecoderConfigurationRecord` (its parameter-set arrays are fed
/// first), `payload` the item's length-prefixed NAL units.
///
/// # Errors
/// [`StillError`] on malformed framing, a decode error, or no picture.
pub fn decode_still(
    hvcc: &[u8],
    payload: &[u8],
    opts: &StillOptions<'_>,
) -> Result<StillPicture, StillError> {
    let (rec, _) = parse_hvcc_with_len(hvcc)?;
    let mut seq = SequenceDecoder::new();
    seq.set_threads(opts.threads.max(1));
    // Polled per NAL unit (each slice segment); CTU-loop polling is
    // left to the decoder-core decision.
    let cancelled = || {
        opts.cancel
            .is_some_and(|c| c.load(std::sync::atomic::Ordering::Relaxed))
    };
    // Single-layer decoding: enhancement-layer NAL units (an `lhv1`
    // item sharing the base item's data, MPEG multilayer00x) are
    // dropped, as a base-layer-only decoder does (§F.8, Annex F).
    let units = rec
        .nal_units
        .into_iter()
        .chain(split_length_prefixed(payload, rec.length_size)?)
        .filter(|u| u.header.nuh_layer_id == 0);
    // The output picture; else any base-layer picture (a multilayer
    // VPS can mark the base layer as not output, but an `hvc1` item is
    // its base layer — libavcodec shows it too). Kept as they come: an
    // item holding thousands of pictures (a kilobyte each) must not keep
    // every one, and once an output picture is found the rest is unused.
    let mut kept: Option<crate::sequence::DecodedFrame> = None;
    let mut keep = |seq: &mut SequenceDecoder| {
        for f in seq.take_decoded() {
            if f.layer_id == 0 && kept.as_ref().is_none_or(|k| !k.output && f.output) {
                kept = Some(f);
            }
        }
        kept.as_ref().is_some_and(|k| k.output)
    };
    let mut done = false;
    for unit in units {
        if cancelled() {
            return Err(StillError::Cancelled);
        }
        seq.push_nal_unit(unit)?;
        if keep(&mut seq) {
            done = true;
            break;
        }
    }
    if !done {
        seq.flush()?;
        keep(&mut seq);
    }
    seq.release_references();
    let Some(frame) = kept else {
        return Err(StillError::NoPicture);
    };
    let picture = if frame.crop.is_whole(&frame.picture) {
        frame.picture
    } else {
        frame.output_picture()
    };
    Ok(StillPicture {
        picture,
        signal: frame.signal,
    })
}
