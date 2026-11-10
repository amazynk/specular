//! The HEVC-core boundary: one coded picture (`hvcC` record + item /
//! tile bytes) → YCbCr planes, with cancellation.
//!
//! Everything container-side (layout, grids, transforms, alpha, colour,
//! the SPS-vs-`ispe` limits of [`crate::sps`]) lives above this
//! boundary, so the decoder core behind it can be swapped: implement
//! [`HevcCore`] and pass it in [`crate::DecodeOptions::core`]. Two cores
//! are built in: [`HeicRs`] (the in-tree fork of the heic-rs HEVC core)
//! and [`Oxideav`] (the in-tree `oxideav-h265` fork). The default,
//! [`DEFAULT_CORE`], is heic-rs with Oxideav as its [`Fallback`] for
//! pictures heic-rs reports as unsupported.

use std::sync::atomic::AtomicBool;

use heif::{Chroma, HeifFrame, HeifPixelFormat, Plane};

use crate::colour::VuiColour;

/// One coded picture to decode.
#[derive(Clone, Copy, Debug)]
pub struct CoreInput<'a> {
    /// The raw `HEVCDecoderConfigurationRecord` (`hvcC` body).
    pub hvcc: &'a [u8],
    /// The item's (or one `cexg` extent's) length-prefixed NAL units.
    pub data: &'a [u8],
    /// The item's `ispe`: the output must be cropped to it (top-left)
    /// and it must fit the decoded picture. `None`: the decoded size.
    pub crop: Option<(u32, u32)>,
    /// In-picture worker budget (0 / 1: serial).
    pub threads: usize,
    /// Cooperative cancellation.
    pub cancel: Option<&'a AtomicBool>,
}

/// A decoded coded picture.
#[derive(Clone, Debug)]
pub struct CoreOutput {
    /// The planes, cropped to [`CoreInput::crop`]: 8-bit samples as
    /// bytes, deeper ones as little-endian `u16` (see [`HeifFrame`]).
    pub frame: HeifFrame,
    /// The SPS VUI `video_signal_type`, when signalled.
    pub vui: Option<VuiColour>,
    /// CTUs the core could not decode and concealed from their
    /// neighbours (0 for an intact picture, and for cores that fail
    /// instead of concealing).
    pub concealed_ctus: usize,
}

/// Errors of a core.
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    /// The cancellation flag was raised.
    #[error("cancelled")]
    Cancelled,
    /// A valid stream using something the core does not implement.
    #[error("unsupported: {0}")]
    Unsupported(String),
    /// Framing / size inconsistency at the boundary.
    #[error("{0}")]
    Invalid(String),
    /// A bitstream / decode error of the core.
    #[error(transparent)]
    Bitstream(Box<dyn std::error::Error + Send + Sync>),
}

/// An HEVC decoder core.
pub trait HevcCore: Send + Sync + std::fmt::Debug {
    /// Short name for diagnostics.
    fn name(&self) -> &'static str;

    /// Decode one coded picture.
    fn decode(&self, input: &CoreInput<'_>) -> Result<CoreOutput, CoreError>;

    /// Decode independent coded pictures (the tiles of a grid), handing
    /// each result to `sink(index, result)` as it completes, in any
    /// order and possibly from several threads. `sink` returns `false`
    /// to stop: the pictures not yet started are then skipped. The
    /// default decodes them one by one, in parallel on the current rayon
    /// pool when `parallel`; a core that keeps several pictures in
    /// flight (a hardware decoder) overrides it.
    fn decode_batch(&self, inputs: &[CoreInput<'_>], parallel: bool, sink: &BatchSink<'_>) {
        decode_each(self, inputs, parallel, sink);
    }
}

/// Receives one result of [`HevcCore::decode_batch`]; `false`: stop.
pub type BatchSink<'s> = dyn Fn(usize, Result<CoreOutput, CoreError>) -> bool + Sync + 's;

/// [`HevcCore::decode_batch`]'s default: [`HevcCore::decode`] per
/// picture, on the rayon pool when `parallel`.
pub fn decode_each<C: HevcCore + ?Sized>(
    core: &C,
    inputs: &[CoreInput<'_>],
    parallel: bool,
    sink: &BatchSink<'_>,
) {
    use rayon::prelude::*;
    let one = |(i, input): (usize, &CoreInput<'_>)| {
        if sink(i, core.decode(input)) {
            Ok(())
        } else {
            Err(())
        }
    };
    let _ = if parallel && inputs.len() > 1 {
        inputs.par_iter().enumerate().try_for_each(one)
    } else {
        inputs.iter().enumerate().try_for_each(one)
    };
}

/// The in-tree `oxideav-h265` fork: every chroma format, 8–16 bit,
/// multi-slice, lossless, range extensions.
#[derive(Clone, Copy, Debug, Default)]
pub struct Oxideav;

impl HevcCore for Oxideav {
    fn name(&self) -> &'static str {
        "oxideav-h265"
    }

    fn decode(&self, input: &CoreInput<'_>) -> Result<CoreOutput, CoreError> {
        let still = h265::decode_still(
            input.hvcc,
            input.data,
            &h265::StillOptions {
                threads: input.threads,
                cancel: input.cancel,
            },
        )
        .map_err(|e| match e {
            h265::StillError::Cancelled => CoreError::Cancelled,
            e => CoreError::Bitstream(Box::new(e)),
        })?;
        let pic = &still.picture;
        let chroma = Chroma::from_idc(pic.chroma_array_type())
            .ok_or_else(|| CoreError::Invalid("decoded chroma format".into()))?;
        use h265::picture::Plane as P;
        let frame = pack_frame(
            chroma,
            (pic.bit_depth_luma(), pic.bit_depth_chroma()),
            (pic.width_luma() as u32, pic.height_luma() as u32),
            input.crop,
            |plane, y| pic.row([P::Luma, P::Cb, P::Cr][plane], y),
        )?;
        Ok(CoreOutput {
            frame,
            vui: still.signal.as_ref().map(VuiColour::from_signal),
            concealed_ctus: 0,
        })
    }
}

/// The in-tree fork of the heic-rs HEVC core ([`hevc`]): intra 4:0:0 to
/// 4:4:4 at 8–16 bits, tiles, wavefronts (decoded row-parallel within
/// the thread budget), dependent slices. Damaged CTUs are concealed and
/// counted in [`CoreOutput::concealed_ctus`] rather than failing the
/// picture.
#[derive(Clone, Copy, Debug, Default)]
pub struct HeicRs;

impl HevcCore for HeicRs {
    fn name(&self) -> &'static str {
        "heic-rs"
    }

    fn decode(&self, input: &CoreInput<'_>) -> Result<CoreOutput, CoreError> {
        let d = hevc::hvcc::decode(
            input.hvcc,
            input.data,
            &hevc::Options {
                threads: input.threads,
                cancel: input.cancel,
            },
        )
        .map_err(|e| match e {
            hevc::Error::Cancelled => CoreError::Cancelled,
            hevc::Error::Unsupported(m) => CoreError::Unsupported(m.into()),
            e => CoreError::Bitstream(Box::new(e)),
        })?;
        hevc_output(&d, input.crop)
    }
}

/// A picture of the heic-rs core (a still or a sequence's) as a [`CoreOutput`] cropped to
/// `crop`.
pub(crate) fn hevc_output(
    d: &hevc::Decoded,
    crop: Option<(u32, u32)>,
) -> Result<CoreOutput, CoreError> {
    let f = &d.frame;
    let chroma = match f.chroma {
        hevc::ChromaFormat::Monochrome => Chroma::Mono,
        hevc::ChromaFormat::Yuv420 => Chroma::Yuv420,
        hevc::ChromaFormat::Yuv422 => Chroma::Yuv422,
        hevc::ChromaFormat::Yuv444 => Chroma::Yuv444,
    };
    let (ys, cs) = (f.y_stride as usize, f.c_stride as usize);
    let frame = pack_frame(
        chroma,
        (f.bit_depth, f.bit_depth_chroma),
        (f.width, f.height),
        crop,
        |plane, y| {
            let (p, s) = match plane {
                0 => (&f.y, ys),
                1 => (&f.cb, cs),
                _ => (&f.cr, cs),
            };
            p.get(y * s..).unwrap_or(&[])
        },
    )?;
    Ok(CoreOutput {
        frame,
        vui: d.signal.as_ref().map(VuiColour::from_hevc),
        concealed_ctus: d.damaged,
    })
}

/// A core that hands a picture its `primary` reports as
/// [`CoreError::Unsupported`] to `fallback`; every other result
/// (success, cancellation, a bitstream error) is the primary's.
#[derive(Clone, Copy, Debug, Default)]
pub struct Fallback<P, F> {
    pub primary: P,
    pub fallback: F,
}

impl<P: HevcCore, F: HevcCore> HevcCore for Fallback<P, F> {
    fn name(&self) -> &'static str {
        self.primary.name()
    }

    fn decode(&self, input: &CoreInput<'_>) -> Result<CoreOutput, CoreError> {
        match self.primary.decode(input) {
            Err(CoreError::Unsupported(_)) => self.fallback.decode(input),
            r => r,
        }
    }

    fn decode_batch(&self, inputs: &[CoreInput<'_>], parallel: bool, sink: &BatchSink<'_>) {
        // The primary's own batching (e.g. pictures in flight on a hardware
        // decoder); the pictures it refuses go to the fallback.
        self.primary
            .decode_batch(inputs, parallel, &|i, r| match r {
                Err(CoreError::Unsupported(_)) => sink(i, self.fallback.decode(&inputs[i])),
                r => sink(i, r),
            });
    }
}

/// The default core: [`HeicRs`] (faster, and it refuses what it does not
/// implement rather than decoding it wrongly), with [`Oxideav`] for the
/// pictures it refuses (range-extension tools, inter-coded layers).
pub const DEFAULT_CORE: Fallback<HeicRs, Oxideav> = Fallback {
    primary: HeicRs,
    fallback: Oxideav,
};

/// Pack a decoded picture of `decoded` luma size into a [`HeifFrame`]
/// cropped to `crop`, reading rows through `row(plane, y)` (plane 0 =
/// Y, 1 = Cb, 2 = Cr; at least the plane's cropped width long). A
/// helper for cores.
pub fn pack_frame<'p, T: Copy + Into<u16> + 'p>(
    chroma: Chroma,
    (depth, depth_chroma): (u8, u8),
    (dw, dh): (u32, u32),
    crop: Option<(u32, u32)>,
    row: impl Fn(usize, usize) -> &'p [T],
) -> Result<HeifFrame, CoreError> {
    // A frame has one sample depth: when luma and chroma differ (an SPS may
    // code e.g. 8-bit luma with 10-bit chroma) every plane is widened to the
    // larger, shifting the shallower planes' samples up so that each plane
    // keeps its full range (white stays white, the chroma midpoint stays
    // the midpoint).
    let depth_chroma = if chroma == Chroma::Mono {
        depth
    } else {
        depth_chroma
    };
    let out_depth = depth.max(depth_chroma);
    let shift = |plane: usize| out_depth - if plane == 0 { depth } else { depth_chroma };
    let format = HeifPixelFormat::new(chroma, out_depth, false)
        .map_err(|e| CoreError::Invalid(e.to_string()))?;
    let (w, h) = match crop {
        Some((iw, ih)) if iw <= dw && ih <= dh && iw > 0 && ih > 0 => (iw, ih),
        Some((iw, ih)) => {
            return Err(CoreError::Invalid(format!(
                "ispe {iw}x{ih} does not fit the decoded {dw}x{dh} picture"
            )));
        }
        None => (dw, dh),
    };
    let wide = format.bytes_per_sample() == 2;
    let mut planes = Vec::with_capacity(chroma.colour_planes());
    for i in 0..chroma.colour_planes() {
        let (pw, ph) = format.plane_dims(i, w, h);
        let pw = pw as usize;
        let sh = shift(i);
        let mut data = Vec::with_capacity(pw * ph as usize * (1 + wide as usize));
        for y in 0..ph as usize {
            let r = row(i, y)
                .get(..pw)
                .ok_or_else(|| CoreError::Invalid("decoded row too short".into()))?;
            if wide {
                data.extend(r.iter().flat_map(|&v| (v.into() << sh).to_le_bytes()));
            } else {
                data.extend(r.iter().map(|&v| v.into() as u8));
            }
        }
        planes.push(Plane {
            stride: pw * (1 + wide as usize),
            data,
        });
    }
    let f = HeifFrame {
        width: w,
        height: h,
        format,
        planes,
    };
    f.validate()
        .map_err(|e| CoreError::Invalid(e.to_string()))?;
    Ok(f)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixed_depths_widen_to_the_larger() {
        // 4x2 4:2:0: 8-bit luma, 10-bit chroma.
        let y: [u16; 8] = [0, 1, 128, 255, 10, 20, 30, 40];
        let cb: [u16; 2] = [512, 1023];
        let cr: [u16; 2] = [0, 300];
        let f = pack_frame(Chroma::Yuv420, (8, 10), (4, 2), None, |p, row| match p {
            0 => &y[row * 4..row * 4 + 4],
            1 => &cb[..],
            _ => &cr[..],
        })
        .unwrap();
        assert_eq!(f.format.bit_depth, 10);
        let s = |p: usize| -> Vec<u16> {
            f.planes[p]
                .data
                .chunks_exact(2)
                .map(|b| u16::from_le_bytes([b[0], b[1]]))
                .collect()
        };
        assert_eq!(s(0), y.iter().map(|v| v << 2).collect::<Vec<_>>());
        assert_eq!(s(1), cb);
        assert_eq!(s(2), cr);

        // The other way round: 10-bit luma, 8-bit chroma.
        let y10: [u16; 8] = [0, 1023, 512, 4, 5, 6, 7, 8];
        let c8: [u16; 2] = [128, 255];
        let f = pack_frame(
            Chroma::Yuv420,
            (10, 8),
            (4, 2),
            Some((3, 2)),
            |p, row| match p {
                0 => &y10[row * 4..row * 4 + 4],
                _ => &c8[..],
            },
        )
        .unwrap();
        assert_eq!(f.format.bit_depth, 10);
        assert_eq!((f.width, f.height), (3, 2));
        let cb = &f.planes[1].data;
        assert_eq!(u16::from_le_bytes([cb[0], cb[1]]), 512);
        assert_eq!(u16::from_le_bytes([cb[2], cb[3]]), 1020);

        // Monochrome ignores the chroma depth; equal depths are unchanged.
        let f = pack_frame(Chroma::Mono, (8, 10), (4, 2), None, |_, row| {
            &y[row * 4..row * 4 + 4]
        })
        .unwrap();
        assert_eq!(f.format.bit_depth, 8);
        assert_eq!(
            f.planes[0].data,
            y.iter().map(|&v| v as u8).collect::<Vec<_>>()
        );
    }
}
