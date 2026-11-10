//! Codestream decoding: header checks, the slice index, slices decoded in parallel, then the
//! inverse wavelet, colour and output transforms.

use crate::bits::Bits;
use crate::entropy::{Plane, SliceDecoder};
use crate::header::{self, Header, Nlt, SLH, SLI};
use crate::layout::Layout;
use crate::transform;
use crate::{Error, MAX_SAMPLES, Result};
use rayon::prelude::*;

/// Decoded component samples (after every inverse transform).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Samples {
    pub depth: u8,
    /// One plane per component; a Star-Tetrix image gives one plane, the mosaic.
    pub planes: Vec<SamplePlane>,
    /// The mosaic's colour filter layout (Star-Tetrix images).
    pub cfa: Option<header::Cfa>,
    /// The codestream ended early or was damaged; later precincts are flat.
    pub truncated: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SamplePlane {
    pub width: usize,
    pub height: usize,
    pub data: Vec<u16>,
}

fn unsupported(s: impl Into<String>) -> Error {
    Error::Unsupported(s.into())
}

/// Checks that the header describes something this decoder implements.
pub(crate) fn check(h: &Header) -> Result<()> {
    if let Some(t) = &h.tdc {
        if !h.sli {
            return Err(Error::Malformed(
                "a TDC profile codestream without SLI slices".into(),
            ));
        }
        if h.cap & header::cap::FRAME_BUFFER == 0 {
            return Err(Error::Malformed(
                "SLI slices without the frame buffer capability".into(),
            ));
        }
        if t.si == 0 || !t.si.is_power_of_two() {
            return Err(Error::Malformed(format!(
                "TDC selection groups of {} code groups",
                t.si
            )));
        }
    } else if h.sli {
        return Err(Error::Malformed("SLI slices outside a TDC profile".into()));
    }
    if h.width == 0 || h.height == 0 {
        return Err(Error::Malformed("empty image".into()));
    }
    let depth = h.depth();
    if h.components.iter().any(|c| c.depth != depth) {
        return Err(unsupported(
            "JPEG XS images whose components differ in bit depth are not supported",
        ));
    }
    if depth > 16 {
        return Err(unsupported(format!(
            "{depth}-bit JPEG XS images are outside every JPEG XS profile (they stop at 16 bits)"
        )));
    }
    if h.bw < depth || h.bw > 24 || h.fq > h.bw {
        return Err(Error::Malformed(format!(
            "{}-bit coefficients with {} fractional bits for {depth}-bit samples",
            h.bw, h.fq
        )));
    }
    if h.br == 0 || h.br > 8 {
        return Err(Error::Malformed(format!("raw counts of {} bits", h.br)));
    }
    if h.qpih > 1 {
        return Err(unsupported(format!("quantiser type {}", h.qpih)));
    }
    if h.fs > 1 || h.rm > 1 {
        return Err(Error::Malformed("unknown sign or run mode".into()));
    }
    if let Nlt::Extended { e, .. } = h.nlt
        && (e >= h.bw || h.bw - e < 1)
    {
        return Err(Error::Malformed(
            "extended non-linear transform out of range".into(),
        ));
    }
    if matches!(h.nlt, Nlt::Quadratic { .. } | Nlt::Extended { .. }) && 2 * h.bw < depth {
        return Err(Error::Malformed("non-linear transform out of range".into()));
    }
    let nc = h.components.len();
    match h.cpih {
        0 => {}
        1 => {
            if nc < 3 || h.components[..3].iter().any(|c| c.sx != 1 || c.sy != 1) {
                return Err(Error::Malformed(
                    "the colour transform needs three full-resolution components".into(),
                ));
            }
        }
        3 => {
            if nc != 4 || h.components.iter().any(|c| c.sx != 1 || c.sy != 1) {
                return Err(Error::Malformed(
                    "the Star-Tetrix transform needs four full-resolution components".into(),
                ));
            }
            if h.tetrix.is_none() || h.cfa.is_none() {
                return Err(Error::Malformed(
                    "a Star-Tetrix image without its CTS or CRG marker".into(),
                ));
            }
        }
        t => {
            return Err(unsupported(format!(
                "JPEG XS colour transform {t} is not supported"
            )));
        }
    }
    let (gw, gh) = h.component_grid();
    let samples: u64 = h
        .components
        .iter()
        .map(|c| (gw / c.sx as usize) as u64 * (gh / c.sy as usize) as u64)
        .sum();
    if samples > MAX_SAMPLES {
        return Err(Error::TooLarge);
    }
    Ok(())
}

/// Byte offset of each slice's first precinct, as far as the data goes; and whether the
/// index stopped early.
fn slice_index(d: &[u8], h: &Header, l: &Layout, rows_per_slice: usize) -> (Vec<usize>, bool) {
    let nslices = l.npy.div_ceil(rows_per_slice);
    let tdc = h.tdc.is_some();
    let hdr_bits = 40
        + if tdc {
            16 + 4 * l.bands.len()
        } else {
            2 * l.bands.len()
        };
    let hdr_bytes = hdr_bits.div_ceil(8);
    let marker = if tdc { SLI } else { SLH };
    let mut out = Vec::with_capacity(nslices);
    let mut at = h.first_slice;
    for k in 0..nslices {
        let ok = d.get(at..at + 6).is_some_and(|s| {
            u16::from_be_bytes([s[0], s[1]]) == marker
                && u16::from_be_bytes([s[2], s[3]]) == 4
                && u16::from_be_bytes([s[4], s[5]]) as usize == k & 0xFFFF
        });
        if !ok {
            return (out, true);
        }
        at += 6;
        out.push(at);
        let rows = rows_per_slice.min(l.npy - k * rows_per_slice);
        for _ in 0..rows * l.npx {
            let Some(s) = d.get(at..at + 3) else {
                return (out, true);
            };
            let lprc = (s[0] as usize) << 16 | (s[1] as usize) << 8 | s[2] as usize;
            at += hdr_bytes + lprc;
        }
    }
    (out, at > d.len())
}

/// Decodes a bare codestream to component samples.
pub fn decode_samples(d: &[u8]) -> Result<Samples> {
    Sequence::new().decode(d)
}

/// Decodes the frames of a JPEG XS video in order, keeping the frame buffer that temporal
/// differential coding (TDC profiles) predicts from. Intra-only codestreams need no state.
#[derive(Default)]
pub struct Sequence {
    /// Frame buffer coefficient planes and the picture size they belong to.
    fb: Option<(u32, u32, Vec<Vec<i32>>)>,
}

impl Sequence {
    pub fn new() -> Self {
        Sequence::default()
    }

    /// Decodes the next frame.
    pub fn decode(&mut self, d: &[u8]) -> Result<Samples> {
        let h = header::parse(d)?;
        check(&h)?;
        if h.ticoraw() {
            return Err(unsupported(
                "intoPIX TicoRAW codestreams are camera raw data (Nikon High Efficiency): \
                 open the raw file instead",
            ));
        }
        let l = Layout::new(&h)?;
        decode_with(d, &h, &l, &mut self.fb)
    }
}

type Fb = Option<(u32, u32, Vec<Vec<i32>>)>;

/// Splits component planes into the rows each slice covers.
fn split<'a>(
    planes: &'a mut [Vec<i32>],
    l: &Layout,
    rows_per_slice: usize,
    n: usize,
) -> Vec<Vec<Plane<'a>>> {
    let mut per_slice: Vec<Vec<Plane>> = (0..n).map(|_| Vec::new()).collect();
    for (ci, p) in planes.iter_mut().enumerate() {
        let c = &l.comps[ci];
        let rows = rows_per_slice * (l.ph >> (c.sy - 1));
        let w = c.w;
        if w == 0 || c.h == 0 {
            for s in per_slice.iter_mut() {
                s.push(Plane {
                    data: &mut [],
                    row0: 0,
                    width: 0,
                });
            }
            continue;
        }
        let mut chunks = p.chunks_mut(rows * w);
        for (k, s) in per_slice.iter_mut().enumerate() {
            s.push(Plane {
                data: chunks.next().unwrap_or(&mut []),
                row0: k * rows,
                width: w,
            });
        }
    }
    per_slice
}

/// Component planes after the inverse wavelet transform, before any colour or output
/// transform: signed values with `Header::fq` fractional bits. For codecs that build on
/// JPEG XS with transforms of their own (intoPIX TicoRAW in Nikon raw files).
#[derive(Clone, Debug)]
pub struct Coefficients {
    pub header: Header,
    /// Width, height and values of each component.
    pub planes: Vec<(usize, usize, Vec<i32>)>,
    /// The codestream ended early or was damaged; later precincts are flat.
    pub truncated: bool,
    /// Memory reserved for the planes and the caller's output (10 bytes a sample); hold it
    /// until the decode is done.
    pub memory: std::sync::Arc<specular_core::budget::Reservation>,
}

/// Decodes a bare codestream (intra-only) up to the inverse wavelet transform.
pub fn decode_coefficients(d: &[u8]) -> Result<Coefficients> {
    let h = header::parse(d)?;
    check(&h)?;
    if h.tdc.is_some() {
        return Err(unsupported(
            "temporally predicted JPEG XS frames need the frames before them",
        ));
    }
    let l = Layout::new(&h)?;
    let (planes, truncated, memory) = wavelet_planes(d, &h, &l, &mut None)?;
    Ok(Coefficients {
        planes: planes_with_size(planes, &l),
        header: h,
        truncated,
        memory: std::sync::Arc::new(memory),
    })
}

/// Entropy decoding of every slice, then the inverse wavelet transform of each component.
/// The reservation returned covers the planes and the output samples: hold it to the end.
fn wavelet_planes(
    d: &[u8],
    h: &Header,
    l: &Layout,
    fb: &mut Fb,
) -> Result<(Vec<Vec<i32>>, bool, specular_core::budget::Reservation)> {
    let rows_per_slice = h.slice_height as usize >> h.nly;
    if rows_per_slice == 0 {
        return Err(Error::Malformed("slices of zero height".into()));
    }
    let (slices, mut truncated) = slice_index(d, h, l, rows_per_slice);
    if slices.is_empty() {
        return Err(Error::Truncated(
            "the codestream ends before its first slice".into(),
        ));
    }
    // i32 coefficient planes, the entropy decoder's coefficients and the 16-bit samples.
    let samples: u64 = l.comps.iter().map(|c| (c.w * c.h) as u64).sum();
    let res = specular_core::budget::reserve_n(samples, 4 + 4 + 2)?;
    // Coefficient planes (and the frame buffer), cut into the rows each slice covers.
    let mut planes: Vec<Vec<i32>> = l.comps.iter().map(|c| vec![0i32; c.w * c.h]).collect();
    let tdc = h.tdc.is_some();
    if tdc {
        let fresh = !matches!(fb, Some((w, ht, p)) if *w == h.width && *ht == h.height
            && p.len() == planes.len() && p.iter().zip(&planes).all(|(a, b)| a.len() == b.len()));
        if fresh {
            *fb = Some((h.width, h.height, planes.clone()));
        }
    } else {
        *fb = None;
    }
    let per_slice = split(&mut planes, l, rows_per_slice, slices.len());
    let fb_slices: Vec<Option<Vec<Plane>>> = match fb.as_mut() {
        Some((_, _, f)) if tdc => split(f, l, rows_per_slice, slices.len())
            .into_iter()
            .map(Some)
            .collect(),
        _ => (0..slices.len()).map(|_| None).collect(),
    };
    let results: Vec<Result<()>> = per_slice
        .into_par_iter()
        .zip(fb_slices.into_par_iter())
        .zip(slices.par_iter())
        .enumerate()
        .map(|(k, ((mut pl, mut fbs), &at))| {
            let mut dec = SliceDecoder::new(h, l);
            let mut r = Bits::new(d, at);
            let row0 = k * rows_per_slice;
            for py in row0..(row0 + rows_per_slice).min(l.npy) {
                for px in 0..l.npx {
                    dec.precinct(&mut r, px, py, py == row0, &mut pl, fbs.as_deref_mut())?;
                }
            }
            Ok(())
        })
        .collect();
    for r in results {
        match r {
            Ok(()) => {}
            Err(Error::Truncated(_) | Error::Malformed(_)) => truncated = true,
            Err(e) => return Err(e),
        }
    }
    for (p, c) in planes.iter_mut().zip(&l.comps) {
        if c.nlx > 0 {
            crate::dwt::inverse(p, c.w, c.h, c.nlx, c.nly);
        }
    }
    Ok((planes, truncated, res))
}

fn decode_with(d: &[u8], h: &Header, l: &Layout, fb: &mut Fb) -> Result<Samples> {
    let (mut planes, truncated, _mem) = wavelet_planes(d, h, l, fb)?;
    let depth = h.depth();
    let (planes, cfa) = match h.cpih {
        1 => {
            transform::inverse_rct(&mut planes);
            (planes_with_size(planes, l), None)
        }
        3 => {
            let cfa = h.cfa.unwrap_or(header::Cfa::Rggb);
            let t = h.tetrix.unwrap_or(header::Tetrix {
                inline: false,
                e1: 0,
                e2: 0,
            });
            let (w, ht) = (l.comps[0].w, l.comps[0].h);
            let m = transform::inverse_tetrix(&planes, w, ht, cfa, t);
            (vec![(2 * w, 2 * ht, m)], Some(cfa))
        }
        _ => (planes_with_size(planes, l), None),
    };
    let planes = planes
        .into_iter()
        .map(|(w, ht, v)| {
            let mut data = vec![0u16; v.len()];
            transform::output(&v, &mut data, h.bw, depth, h.nlt);
            SamplePlane {
                width: w,
                height: ht,
                data,
            }
        })
        .collect();
    Ok(Samples {
        depth,
        planes,
        cfa,
        truncated,
    })
}

fn planes_with_size(p: Vec<Vec<i32>>, l: &Layout) -> Vec<(usize, usize, Vec<i32>)> {
    p.into_iter()
        .zip(&l.comps)
        .map(|(v, c)| (c.w, c.h, v))
        .collect()
}
