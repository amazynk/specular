//! Old-style JPEG (TIFF 6.0 section 22, compression 6): each strip / tile becomes a JPEG
//! stream for `specular-jpeg`.
//!
//! The common variants:
//! - every strip is a whole JPEG stream (starts with SOI): used as it is;
//! - `JPEGInterchangeFormat` points at a stream whose tables (and frame / scan headers) apply
//!   to the strips, which hold only entropy-coded data;
//! - no interchange stream: the tables come from `JPEGQTables`, `JPEGDCTables` and
//!   `JPEGACTables` (one per component), with `JPEGRestartInterval`.
//!
//! The frame header is always rebuilt for the unit's size (an interchange stream's frame
//! describes the whole image), keeping its sampling factors and table selectors.

use crate::ifd::{self, Reader, find};
use crate::{Ifd, Photometric, Result};
use std::ops::Range;

/// The old-style JPEG tags of an IFD.
#[derive(Clone, Debug, Default)]
pub(crate) struct Tags {
    /// JPEGInterchangeFormat (513) to the end of the file (its length tag is often wrong).
    pub jif: Option<Range<usize>>,
    /// JPEGProc (512): 1 baseline, 14 lossless.
    pub process: u16,
    pub restart: u16,
    /// Offsets of the per-component tables (64-byte quantisation tables; Huffman tables as
    /// 16 counts and their values).
    pub q: Vec<u64>,
    pub dc: Vec<u64>,
    pub ac: Vec<u64>,
    /// ReferenceBlackWhite (532) when it is not the YCbCr default (0 255 128 255 128 255):
    /// libtiff scales old-style JPEG's YCbCr samples by it before colour conversion.
    pub rbw: Option<[f64; 6]>,
}

impl Tags {
    pub fn parse(r: &Reader, entries: &[ifd::Entry]) -> Result<Tags> {
        let get = |tag: u16| find(entries, tag).and_then(|e| r.uint(e).ok());
        let list = |tag: u16| {
            find(entries, tag)
                .and_then(|e| r.uints(e).ok())
                .unwrap_or_default()
        };
        let jif = get(513).and_then(|o| {
            let o = usize::try_from(o).ok()?;
            (o.checked_add(2)? <= r.data.len()).then_some(o..r.data.len())
        });
        let rbw = find(entries, 532)
            .and_then(|e| r.reals(e).ok())
            .and_then(|v| <[f64; 6]>::try_from(v.get(..6)?).ok())
            .filter(|v| *v != [0.0, 255.0, 128.0, 255.0, 128.0, 255.0])
            .filter(|v| v[1] > v[0] && v[3] > v[2] && v[5] > v[4]);
        Ok(Tags {
            rbw,
            jif,
            process: get(512).unwrap_or(1) as u16,
            restart: get(515).unwrap_or(0) as u16,
            q: list(519),
            dc: list(520),
            ac: list(521),
        })
    }

    /// The JPEG stream of one unit `w`×`h` with entropy-coded (or complete) data `src`.
    pub fn stream(&self, data: &[u8], ifd: &Ifd, src: &[u8], w: u32, h: u32) -> Vec<u8> {
        if src.starts_with(&[0xFF, 0xD8]) {
            return src.to_vec();
        }
        let mut src = src;
        let ncomp = (ifd.samples as usize).clamp(1, 4);
        let mut out = Vec::with_capacity(src.len() + 1024);
        out.extend_from_slice(&[0xFF, 0xD8]);
        // Tables, frame and scan headers from the interchange stream, if any.
        let mut sof: Option<Vec<u8>> = None;
        let mut sos: Option<Vec<u8>> = None;
        let mut have_dqt = false;
        let mut have_dht = false;
        let mut have_dri = false;
        if let Some(j) = &self.jif {
            let s = &data[j.clone()];
            let mut p = 2usize;
            if s.starts_with(&[0xFF, 0xD8]) {
                while p + 4 <= s.len() {
                    if s[p] != 0xFF {
                        break;
                    }
                    let m = s[p + 1];
                    if m == 0xFF {
                        p += 1;
                        continue;
                    }
                    if m == 0xD8 || (0xD0..=0xD7).contains(&m) {
                        p += 2;
                        continue;
                    }
                    if m == 0xD9 {
                        break;
                    }
                    let len = u16::from_be_bytes([s[p + 2], s[p + 3]]) as usize;
                    let Some(seg) = s.get(p..p + 2 + len) else {
                        break;
                    };
                    match m {
                        0xDB => {
                            out.extend_from_slice(seg);
                            have_dqt = true;
                        }
                        0xC4 => {
                            out.extend_from_slice(seg);
                            have_dht = true;
                        }
                        0xDD => {
                            out.extend_from_slice(seg);
                            have_dri = true;
                        }
                        0xC0..=0xCF if !matches!(m, 0xC4 | 0xC8 | 0xCC) => {
                            sof = Some(seg.to_vec());
                        }
                        0xDA => {
                            sos = Some(seg.to_vec());
                            // Some writers point the first strip a few bytes before the
                            // scan data (into the scan header): skip to the entropy data.
                            let scan = j.start + p + 2 + len;
                            let at = (src.as_ptr() as usize).wrapping_sub(data.as_ptr() as usize);
                            // Their byte counts are not to be trusted either: let the
                            // decoder read on (it stops after this unit's MCUs).
                            if at < scan && scan - at < src.len().min(64) {
                                src = &data[scan..];
                            }
                            break;
                        }
                        _ => {}
                    }
                    p += 2 + len;
                }
            }
        }
        let at = |o: u64, len: usize| -> Option<Range<usize>> {
            let o = usize::try_from(o).ok()?;
            Some(o..o.checked_add(len)?)
        };
        if !have_dqt {
            for (i, &o) in self.q.iter().enumerate().take(4) {
                // Table offsets are file values (LONG8 too): ranges are checked.
                if let Some(t) = at(o, 64).and_then(|r| data.get(r)) {
                    out.extend_from_slice(&[0xFF, 0xDB, 0, 67, i as u8]);
                    out.extend_from_slice(t);
                }
            }
        }
        if !have_dht {
            for (class, tables) in [(0u8, &self.dc), (1, &self.ac)] {
                for (i, &o) in tables.iter().enumerate().take(4) {
                    let Some(counts) = at(o, 16).and_then(|r| data.get(r)) else {
                        continue;
                    };
                    let n: usize = counts.iter().map(|&c| c as usize).sum();
                    let Some(t) = at(o, 16 + n).and_then(|r| data.get(r)) else {
                        continue;
                    };
                    let len = (3 + t.len()) as u16;
                    out.extend_from_slice(&[0xFF, 0xC4]);
                    out.extend_from_slice(&len.to_be_bytes());
                    out.push((class << 4) | i as u8);
                    out.extend_from_slice(t);
                }
            }
        }
        if !have_dri && self.restart > 0 {
            out.extend_from_slice(&[0xFF, 0xDD, 0, 4]);
            out.extend_from_slice(&self.restart.to_be_bytes());
        }
        // Frame header for this unit: sampling and quantisation selectors from the
        // interchange stream's frame, else from YCbCrSubSampling.
        let mut comps: Vec<[u8; 3]> = (0..ncomp)
            .map(|i| {
                let samp = if i == 0 && ifd.photometric == Photometric::YCbCr && ncomp >= 3 {
                    let (hs, vs) = ifd.subsampling;
                    ((hs.clamp(1, 4) as u8) << 4) | vs.clamp(1, 4) as u8
                } else {
                    0x11
                };
                let tq = i.min(self.q.len().saturating_sub(1)) as u8;
                [i as u8 + 1, samp, tq]
            })
            .collect();
        let mut marker = 0xC0u8;
        if let Some(f) = &sof
            && f.len() >= 10
        {
            let n = f[9] as usize;
            if n == ncomp && f.len() >= 10 + 3 * n {
                comps = (0..n)
                    .map(|i| [f[10 + 3 * i], f[11 + 3 * i], f[12 + 3 * i]])
                    .collect();
                marker = f[1];
            }
        }
        let len = (8 + 3 * comps.len()) as u16;
        out.extend_from_slice(&[0xFF, marker]);
        out.extend_from_slice(&len.to_be_bytes());
        out.push(8);
        out.extend_from_slice(&(h.min(65535) as u16).to_be_bytes());
        out.extend_from_slice(&(w.min(65535) as u16).to_be_bytes());
        out.push(comps.len() as u8);
        for c in &comps {
            out.extend_from_slice(c);
        }
        // A baseline scan header: the interchange stream's component selectors, but always
        // the full spectral range (some writers leave Se at 0).
        let sel: Option<Vec<[u8; 2]>> = sos.as_ref().and_then(|s| {
            let n = *s.get(4)? as usize;
            (n == comps.len())
                .then(|| {
                    (0..n)
                        .map(|i| Some([*s.get(5 + 2 * i)?, *s.get(6 + 2 * i)?]))
                        .collect()
                })
                .flatten()
        });
        match sel {
            Some(sel) => {
                let len = (6 + 2 * sel.len()) as u16;
                out.extend_from_slice(&[0xFF, 0xDA]);
                out.extend_from_slice(&len.to_be_bytes());
                out.push(sel.len() as u8);
                for c in &sel {
                    out.extend_from_slice(c);
                }
                out.extend_from_slice(&[0, 63, 0]);
            }
            None => {
                let n = comps.len();
                let len = (6 + 2 * n) as u16;
                out.extend_from_slice(&[0xFF, 0xDA]);
                out.extend_from_slice(&len.to_be_bytes());
                out.push(n as u8);
                for (i, c) in comps.iter().enumerate() {
                    let td = i.min(self.dc.len().saturating_sub(1)) as u8;
                    let ta = i.min(self.ac.len().saturating_sub(1)) as u8;
                    out.extend_from_slice(&[c[0], (td << 4) | ta]);
                }
                out.extend_from_slice(&[0, 63, 0]);
            }
        }
        out.extend_from_slice(src);
        out.extend_from_slice(&[0xFF, 0xD9]);
        out
    }
}
