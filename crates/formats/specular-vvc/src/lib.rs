//! The VVC (ITU-T H.266) decoder of Specular, written from the Recommendation (V4, 01/2026).
//!
//! [`decode_still`] decodes one intra picture: the Main 10 and Main 10 4:4:4 profiles and
//! their Still Picture variants, 8 to 10 bits, 4:0:0, 4:2:0, 4:2:2 and 4:4:4, with every
//! intra tool (MIP, ISP, MRL, LFNST, MTS, CCLM, intra block copy, palette, transform skip,
//! BDPCM, joint CbCr, dependent quantisation, sign hiding, scaling lists, adaptive colour
//! transform, LMCS), the loop filters (deblocking, SAO, ALF, CC-ALF), and any layout of
//! subpictures, tiles, slices and wavefront rows. What it does not decode is refused with
//! [`Error::Unsupported`], naming the feature in plain words: inter prediction (P and B
//! slices), and the range-extension tools of the 12- and 16-bit profiles.
//!
//! # Inputs
//!
//! `parameter_sets` are SPS, PPS and APS NAL units (from a `vvcC` record, see [`vvcc`]),
//! each without a start code or length prefix. `nal_units` are the item's own NAL units:
//! APS, picture header and coded slices; others are ignored, and so are NAL units of
//! layers above 0. [`nal::split_length_prefixed`] and [`nal::split_annexb`] split framed
//! data.
//!
//! # Damage and cancellation
//!
//! A slice that fails part way is not fatal: its remaining CTUs are left grey and
//! [`Decoded::damaged`] counts them. [`Options::cancel`] is polled once per CTU row.

#![deny(unsafe_code)]
// Loops index several spec arrays at once, as the Recommendation writes them.
#![allow(clippy::needless_range_loop)]

mod bits;
mod cabac;
mod ctu;
mod decoder;
mod error;
mod filter;
mod frame;
mod intra;
mod lmcs;
pub mod nal;
mod picture;
// Parameter sets keep every field they parse, inter-prediction ones included.
#[allow(dead_code)]
mod ps;
#[rustfmt::skip]
mod tables;
mod transform;
pub mod vvcc;

use core::sync::atomic::AtomicBool;

pub use error::{Error, Result};
pub use frame::{ChromaFormat, Frame};
pub use ps::sps::Signal;

/// How to decode.
#[derive(Clone, Copy, Debug, Default)]
pub struct Options<'a> {
    /// Worker threads; 0 and 1 both mean the calling thread only. More help pictures coded
    /// with several tiles, slices or wavefront rows, and the loop filters.
    pub threads: usize,
    /// Polled at least once per CTU row; `true` stops the decode with [`Error::Cancelled`].
    pub cancel: Option<&'a AtomicBool>,
}

/// A decoded picture and how complete it is.
#[derive(Debug)]
pub struct Decoded {
    /// The picture, cropped to the conformance window.
    pub frame: Frame,
    /// CTUs that could not be decoded (left grey); zero for an intact stream.
    pub damaged: usize,
    /// CTUs in the picture.
    pub total: usize,
    /// The VUI colour description, when the SPS has one.
    pub signal: Option<Signal>,
    /// Picture order count of the picture.
    pub poc: i32,
    /// Index of the coded video sequence the picture belongs to (output order is by
    /// sequence, then picture order count).
    pub cvs: u32,
}

/// Size and format of the pictures a set of parameter sets describes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Info {
    /// Cropped to the conformance window.
    pub width: u32,
    pub height: u32,
    pub bit_depth: u8,
    pub chroma: ChromaFormat,
    /// general_profile_idc of the SPS (0 when not signalled).
    pub profile_idc: u8,
}

/// Reads [`Info`] from the first SPS and PPS among `parameter_sets`.
pub fn info(parameter_sets: &[&[u8]]) -> Result<Info> {
    decoder::info(parameter_sets)
}

/// Decodes a single intra picture.
pub fn decode_still(parameter_sets: &[&[u8]], nal_units: &[&[u8]]) -> Result<Frame> {
    decode_still_with(parameter_sets, nal_units, &Options::default()).map(|d| d.frame)
}

/// Decodes a single intra picture with the given [`Options`]: the first coded picture among
/// `nal_units`.
pub fn decode_still_with(
    parameter_sets: &[&[u8]],
    nal_units: &[&[u8]],
    options: &Options<'_>,
) -> Result<Decoded> {
    let mut d = decoder::Decoder::new(options.threads);
    for ps in parameter_sets {
        d.push_parameter_set(ps)?;
    }
    let mut out = None;
    d.decode_units(nal_units, options.cancel, &mut |p| {
        if out.is_none() {
            out = Some(p);
        }
        false
    })?;
    out.ok_or(Error::NoSlices)
}

/// Decodes every intra picture of a sequence of NAL units (an Annex B stream split by
/// [`nal::split_annexb`], say), in decoding order, handing each to `sink`; pictures with
/// inter slices are skipped and counted in the result. `sink` returns `true` to stop.
pub fn decode_intra_pictures(
    nal_units: &[&[u8]],
    options: &Options<'_>,
    sink: &mut dyn FnMut(Decoded) -> bool,
) -> Result<usize> {
    let mut d = decoder::Decoder::new(options.threads);
    d.decode_units(nal_units, options.cancel, sink)
}
