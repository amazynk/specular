//! JPEG-LS decoder (ITU-T T.87 | ISO/IEC 14495-1).
//!
//! Lossless and near-lossless coding, 2-16 bit samples, 1-4 components, non-interleaved,
//! line-interleaved (with subsampling) and sample-interleaved scans, custom coding parameters
//! (LSE type 1), mapping tables (LSE types 2 and 3, returned with the planes), oversize
//! dimensions (LSE type 4), heights from DNL, restart intervals, and the HP colour transforms
//! (APP8 `mrfx`, undone after decoding), and point transforms (Pt: samples shifted left by Pt
//! after decoding, clamped to 2^P - 1, matching the ISO reference libjpeg).
//!
//! # Not supported
//! A point transform together with a mapping table or an HP colour transform: the standard
//! does not say in which order they apply, and no encoder writes them.
//!
//! Output is one plane of `u16` samples per component at its own (possibly subsampled) size.
//! Truncated data gives a partial image with the missing lines 0; corrupt data is an error.

use std::sync::atomic::{AtomicBool, Ordering};

mod bits;
mod header;
mod scan;

use bits::Bits;
pub use header::Component;
use header::{EOI, SOI, SOS, State};
use scan::{Coder, Params};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("not a JPEG-LS stream")]
    NotJpegLs,
    #[error("truncated JPEG-LS data")]
    Truncated,
    #[error("malformed JPEG-LS data: {0}")]
    Malformed(&'static str),
    #[error("unsupported JPEG-LS feature: {0}")]
    Unsupported(String),
    #[error("JPEG-LS image too large ({0} bytes)")]
    TooLarge(u64),
    #[error("cancelled")]
    Cancelled,
    #[error("{0}")]
    NeedsMemory(#[from] specular_core::NeedsMemory),
}

pub type Result<T> = std::result::Result<T, Error>;

/// Colour transform signalled by the HP APP8 `mrfx` segment.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ColourTransform {
    #[default]
    None,
    Hp1,
    Hp2,
    Hp3,
}

/// A mapping table (palette) from an LSE segment. Entries are `entry_bytes` big-endian bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MappingTable {
    pub id: u8,
    pub entry_bytes: u8,
    pub entries: Vec<u32>,
}

/// Header information (up to the first scan).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Info {
    pub width: u32,
    pub height: u32,
    /// Sample precision P (2-16).
    pub precision: u8,
    pub components: Vec<Component>,
    /// NEAR of the first scan (0 = lossless).
    pub near: u16,
    /// Interleave mode of the first scan (0 none, 1 line, 2 sample).
    pub interleave: u8,
    /// Largest sample value (MAXVAL) of the first scan.
    pub maxval: u16,
    pub colour_transform: ColourTransform,
    pub restart_interval: u32,
    /// The first scan maps its samples through a mapping table.
    pub mapped: bool,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct DecodeOptions<'a> {
    pub cancel: Option<&'a AtomicBool>,
    /// Largest accepted width × height × components; 0 means 2^30.
    pub max_samples: u64,
}

/// The samples of one component.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plane {
    pub width: u32,
    pub height: u32,
    /// Row-major samples, 0..=MAXVAL (mapping table indices when `table` is set).
    pub data: Vec<u16>,
    pub table: Option<MappingTable>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub precision: u8,
    /// Largest NEAR of the scans (0 = lossless).
    pub near: u16,
    /// The HP colour transform found in the stream (already undone in `planes`).
    pub colour_transform: ColourTransform,
    /// One plane per frame component, in frame order.
    pub planes: Vec<Plane>,
    /// Data ended early; the missing samples are 0.
    pub truncated: bool,
}

/// Whether `head` starts like a JPEG-LS stream (SOI, then a SOF55 among the first segments).
pub fn probe(head: &[u8]) -> bool {
    if !head.starts_with(&[0xFF, SOI]) {
        return false;
    }
    let mut pos = 2;
    for _ in 0..16 {
        match header::code_at(head, pos) {
            Some((header::SOF55, _)) => return true,
            Some((c, after)) if (0xE0..=0xEF).contains(&c) || c == 0xFE || c == header::LSE => {
                let Some(l) = head.get(after..after + 2) else {
                    return false;
                };
                pos = after + u16::from_be_bytes([l[0], l[1]]) as usize;
            }
            _ => return false,
        }
    }
    false
}

/// Reads the header up to the first scan.
pub fn read_info(data: &[u8]) -> Result<Info> {
    let (st, scan, _) = parse_to_scan(data, State::default(), 2, true)?;
    let Some(scan) = scan else {
        return Err(Error::Malformed("no scan"));
    };
    let params = Params::new(st.precision, scan.near, st.preset)?;
    Ok(Info {
        width: st.width,
        height: st.height,
        precision: st.precision,
        components: st.comps.clone(),
        near: scan.near,
        interleave: scan.ilv,
        maxval: params.maxval as u16,
        colour_transform: st.transform,
        restart_interval: st.restart,
        mapped: scan.comps.iter().any(|&(_, t)| t != 0),
    })
}

/// Walks marker segments from `pos` up to the next SOS (returned with the position of its
/// scan data) or EOI / the end of data (no scan).
fn parse_to_scan(
    data: &[u8],
    mut st: State,
    mut pos: usize,
    first: bool,
) -> Result<(State, Option<header::ScanHeader>, usize)> {
    if first && !data.starts_with(&[0xFF, SOI]) {
        return Err(Error::NotJpegLs);
    }
    loop {
        let Some((code, after)) = header::marker_at(data, pos)? else {
            return Ok((st, None, data.len()));
        };
        match code {
            SOS => {
                let (body, next) = header::segment(data, after)?;
                if !st.frame {
                    return Err(if first {
                        Error::NotJpegLs
                    } else {
                        Error::Malformed("scan before frame header")
                    });
                }
                if st.comps[0].width == 0 {
                    st.resolve_size(data, next)?;
                }
                let scan = st.sos(body)?;
                return Ok((st, Some(scan), next));
            }
            EOI => return Ok((st, None, after)),
            SOI => return Err(Error::Malformed("SOI inside the image")),
            0xD0..=0xD7 => pos = after,
            0x01 => pos = after,
            _ => {
                if !st.frame && first && code != header::SOF55 && !is_skippable(code) {
                    return Err(Error::NotJpegLs);
                }
                pos = st.segment(code, data, after)?;
            }
        }
    }
}

fn is_skippable(code: u8) -> bool {
    (0xE0..=0xEF).contains(&code) || code == 0xFE || code == header::LSE || code == header::DRI
}

/// Output plane filled line by line, growing as lines arrive so corrupt headers that claim a
/// huge size fail before allocating it.
struct Out {
    w: usize,
    h: usize,
    data: Vec<u16>,
    rows: usize,
}

impl Out {
    fn push(&mut self, line: impl ExactSizeIterator<Item = i32>) -> Result<()> {
        if self.rows >= self.h {
            return Ok(());
        }
        let need = (self.rows + 1) * self.w;
        if self.data.capacity() < need {
            let total = self.w * self.h;
            let grow = self
                .data
                .capacity()
                .max(64 * self.w)
                .min(total - self.data.len());
            self.data
                .try_reserve_exact(grow)
                .map_err(|_| Error::TooLarge(2 * total as u64))?;
        }
        self.data.extend(line.map(|v| v as u16));
        self.rows += 1;
        Ok(())
    }

    fn pad(&mut self) -> Result<()> {
        let total = self.w * self.h;
        self.data
            .try_reserve_exact(total - self.data.len())
            .map_err(|_| Error::TooLarge(2 * total as u64))?;
        self.data.resize(total, 0);
        self.rows = self.h;
        Ok(())
    }
}

/// Decodes a JPEG-LS stream.
pub fn decode(data: &[u8], opts: &DecodeOptions) -> Result<Image> {
    let (mut st, mut scan, mut pos) = parse_to_scan(data, State::default(), 2, true)?;
    if scan.is_none() {
        return Err(if st.frame {
            Error::Truncated
        } else {
            Error::NotJpegLs
        });
    }
    let max = if opts.max_samples == 0 {
        1 << 30
    } else {
        opts.max_samples
    };
    let samples: u64 = st
        .comps
        .iter()
        .map(|c| c.width as u64 * c.height as u64)
        .sum();
    if samples > max || st.width > 1 << 20 {
        return Err(Error::TooLarge(samples * 2));
    }
    // 16-bit planes, held against the memory budget while they are decoded.
    let _res = specular_core::budget::reserve_n(samples, 2)?;
    let mut out: Vec<Out> = st
        .comps
        .iter()
        .map(|c| Out {
            w: c.width as usize,
            h: c.height as usize,
            data: Vec::new(),
            rows: 0,
        })
        .collect();
    let mut tables: Vec<Option<MappingTable>> = vec![None; st.comps.len()];
    let mut pts = vec![0u8; st.comps.len()];
    let mut near = 0;
    let mut truncated = false;
    while let Some(sc) = scan {
        let params = Params::new(st.precision, sc.near, st.preset)?;
        near = near.max(sc.near);
        for &(i, t) in &sc.comps {
            pts[i] = sc.pt;
            if t != 0 {
                tables[i] = st.tables.iter().find(|m| m.id == t).cloned();
            }
        }
        let end = decode_scan(&st, &sc, &params, data, pos, &mut out, opts.cancel)?;
        match end {
            None => {
                truncated = true;
                break;
            }
            Some(p) => {
                let (s, next, p) = parse_to_scan(data, st, p, false)?;
                st = s;
                scan = next;
                pos = p;
            }
        }
    }
    if out.iter().any(|o| o.rows < o.h) {
        truncated = true;
        let decoded: usize = out.iter().map(|o| o.data.len()).sum();
        if decoded == 0 {
            return Err(Error::Truncated);
        }
        // Do not let a few bytes claim a huge zero-filled image.
        let total: u64 = out.iter().map(|o| (o.w * o.h) as u64 * 2).sum();
        if total > 64 * data.len() as u64 + (64 << 20) {
            return Err(Error::Truncated);
        }
        for o in &mut out {
            o.pad()?;
        }
    }
    let mut planes: Vec<Plane> = out
        .into_iter()
        .zip(&st.comps)
        .zip(tables)
        .map(|((o, c), table)| Plane {
            width: c.width,
            height: c.height,
            data: o.data,
            table,
        })
        .collect();
    if pts.iter().any(|&p| p != 0) {
        if st.transform != ColourTransform::None {
            return Err(Error::Unsupported(
                "point transform with an HP colour transform".into(),
            ));
        }
        let max = (1u32 << st.precision) - 1;
        for (plane, &pt) in planes.iter_mut().zip(&pts) {
            if pt != 0 {
                for v in &mut plane.data {
                    *v = ((*v as u32) << pt).min(max) as u16;
                }
            }
        }
    }
    if st.transform != ColourTransform::None {
        undo_hp(&mut planes, st.transform, st.precision)?;
    }
    Ok(Image {
        width: st.width,
        height: st.height,
        precision: st.precision,
        near,
        colour_transform: st.transform,
        planes,
        truncated,
    })
}

/// Inverse HP colour transforms (lossless, modulo 2^P), applied to components 1-3.
fn undo_hp(planes: &mut [Plane], t: ColourTransform, precision: u8) -> Result<()> {
    let [p1, p2, p3, ..] = planes else {
        return Err(Error::Malformed(
            "colour transform without three components",
        ));
    };
    if p1.data.len() != p2.data.len() || p1.data.len() != p3.data.len() {
        return Err(Error::Malformed(
            "colour transform on subsampled components",
        ));
    }
    let mask = (1i32 << precision) - 1;
    let half = 1i32 << (precision - 1);
    for ((a, b), c) in p1
        .data
        .iter_mut()
        .zip(p2.data.iter_mut())
        .zip(p3.data.iter_mut())
    {
        let (v1, v2, v3) = (*a as i32, *b as i32, *c as i32);
        let (r, g, bl) = match t {
            ColourTransform::Hp1 => (v1 + v2 - half, v2, v3 + v2 - half),
            ColourTransform::Hp2 => {
                let r = (v1 + v2 - half) & mask;
                (r, v2, v3 + ((r + v2) >> 1) - half)
            }
            ColourTransform::Hp3 => {
                let g = v1 - ((v3 + v2) >> 2) + (half >> 1);
                let g = g & mask;
                (v3 + g - half, g, v2 + g - half)
            }
            ColourTransform::None => (v1, v2, v3),
        };
        *a = (r & mask) as u16;
        *b = (g & mask) as u16;
        *c = (bl & mask) as u16;
    }
    Ok(())
}

/// Splits scan data into restart intervals: yields each interval's bytes and, at the end, the
/// position of the marker that ends the scan.
struct Intervals<'a> {
    data: &'a [u8],
    pos: usize,
    /// Position of the scan's terminating marker, once found.
    end: Option<usize>,
}

impl<'a> Intervals<'a> {
    fn next(&mut self) -> Option<&'a [u8]> {
        if self.end.is_some() {
            return None;
        }
        let start = self.pos;
        let m = header::next_marker(self.data, start);
        let slice = &self.data[start..m];
        match header::code_at(self.data, m) {
            Some((c, after)) if (0xD0..=0xD7).contains(&c) => self.pos = after,
            _ => self.end = Some(m),
        }
        Some(slice)
    }

    /// Skips any remaining intervals; the position of the marker ending the scan.
    fn finish(&mut self) -> usize {
        while self.next().is_some() {}
        self.end.unwrap_or(self.data.len())
    }
}

/// Line buffers of one component in a scan.
struct Lines {
    prev: Vec<i32>,
    cur: Vec<i32>,
    ri: usize,
}

fn lines(n: usize) -> Result<Lines> {
    let mut prev = Vec::new();
    let mut cur = Vec::new();
    prev.try_reserve_exact(n)
        .and_then(|_| cur.try_reserve_exact(n))
        .map_err(|_| Error::TooLarge(8 * n as u64))?;
    prev.resize(n, 0);
    cur.resize(n, 0);
    Ok(Lines { prev, cur, ri: 0 })
}

/// Decodes one scan into `out`. Returns the position of the marker after it, or `None` if the
/// data ended first (the lines decoded so far are kept).
fn decode_scan(
    st: &State,
    sc: &header::ScanHeader,
    params: &Params,
    data: &[u8],
    pos: usize,
    out: &mut [Out],
    cancel: Option<&AtomicBool>,
) -> Result<Option<usize>> {
    let mut coder = Coder::new(params);
    let mut iv = Intervals {
        data,
        pos,
        end: None,
    };
    let Some(first) = iv.next() else {
        return Ok(None);
    };
    let mut br = Bits::new(first);
    let restart = st.restart as usize;
    let lossless = params.near == 0;
    let cancelled = || cancel.is_some_and(|c| c.load(Ordering::Relaxed));
    let nc = sc.comps.len();
    let mut bufs = Vec::with_capacity(nc);
    if sc.ilv == 2 {
        bufs.push(lines((st.comps[sc.comps[0].0].width as usize + 2) * nc)?);
    } else {
        for &(i, _) in &sc.comps {
            bufs.push(lines(st.comps[i].width as usize + 2)?);
        }
    }
    // MCU count: lines for ILV 0 / 2, groups of V_i lines for ILV 1.
    let vs: Vec<usize> = sc
        .comps
        .iter()
        .map(|&(i, _)| st.comps[i].v as usize)
        .collect();
    let mcus = if sc.ilv == 1 {
        sc.comps
            .iter()
            .zip(&vs)
            .map(|(&(i, _), &v)| (st.comps[i].height as usize).div_ceil(v))
            .max()
            .unwrap_or(0)
    } else {
        st.comps[sc.comps[0].0].height as usize
    };
    for m in 0..mcus {
        if m % 64 == 0 && cancelled() {
            return Err(Error::Cancelled);
        }
        if restart > 0 && m > 0 && m % restart == 0 {
            if br.overrun() {
                return Ok(None);
            }
            let Some(next) = iv.next() else {
                return Ok(None);
            };
            br = Bits::new(next);
            coder.reset();
            for b in &mut bufs {
                b.prev.fill(0);
                b.cur.fill(0);
                b.ri = 0;
            }
        }
        let res = if sc.ilv == 2 {
            let b = &mut bufs[0];
            let n = b.cur.len();
            for c in 0..nc {
                b.prev[n - nc + c] = b.prev[n - 2 * nc + c];
                b.cur[c] = b.prev[nc + c];
            }
            let r = coder.line_interleaved(&mut br, &b.prev, &mut b.cur, nc, &mut b.ri);
            if r.is_ok() && !br.overrun() {
                for (c, &(i, _)) in sc.comps.iter().enumerate() {
                    out[i].push(b.cur[nc + c..n - nc].iter().step_by(nc).copied())?;
                }
            }
            std::mem::swap(&mut b.prev, &mut b.cur);
            r
        } else {
            let mut r = Ok(());
            'comps: for ((&(i, _), b), &v) in sc.comps.iter().zip(&mut bufs).zip(&vs) {
                let reps = if sc.ilv == 1 { v } else { 1 };
                for _ in 0..reps {
                    let n = b.cur.len();
                    b.prev[n - 1] = b.prev[n - 2];
                    b.cur[0] = b.prev[1];
                    r = if lossless {
                        coder.line::<true>(&mut br, &b.prev, &mut b.cur, &mut b.ri)
                    } else {
                        coder.line::<false>(&mut br, &b.prev, &mut b.cur, &mut b.ri)
                    };
                    if r.is_err() || br.overrun() {
                        break 'comps;
                    }
                    out[i].push(b.cur[1..n - 1].iter().copied())?;
                    std::mem::swap(&mut b.prev, &mut b.cur);
                }
            }
            r
        };
        match res {
            Err(Error::Truncated) => return Ok(None),
            Err(e) => return Err(e),
            Ok(()) if br.overrun() => return Ok(None),
            Ok(()) => {}
        }
    }
    Ok(Some(iv.finish()))
}

#[cfg(test)]
mod tests;
