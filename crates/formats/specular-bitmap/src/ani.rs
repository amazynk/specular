//! Windows animated cursors (.ani): a RIFF `ACON` container of ICO/CUR frames.
//!
//! Chunks used: `anih` (header: frame and step counts, default rate in jiffies of 1/60 s,
//! flags), `LIST`/`fram` with one `icon` chunk per frame, optional `rate` (jiffies per step) and
//! `seq ` (frame index per step). Each frame is an ordinary ICO/CUR file; decode it with
//! [`crate::ico::Ico::parse`] on [`Ani::frame_data`].

use crate::{Error, Result, u32le};
use std::ops::Range;

pub fn probe(head: &[u8]) -> bool {
    head.len() >= 12 && &head[0..4] == b"RIFF" && &head[8..12] == b"ACON"
}

/// One step of the animation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Step {
    /// Index into [`Ani::frames`].
    pub frame: usize,
    /// Display time in jiffies (1/60 s).
    pub jiffies: u32,
}

#[derive(Clone, Debug)]
pub struct Ani {
    /// Byte ranges of the frames' ICO/CUR data in the file.
    pub frames: Vec<Range<usize>>,
    /// Playback order and timing.
    pub steps: Vec<Step>,
}

impl Ani {
    pub fn parse(data: &[u8]) -> Result<Ani> {
        if data.len() < 12 {
            return Err(Error::Truncated("RIFF header".into()));
        }
        if !probe(data) {
            return Err(Error::Malformed("not an ANI file".into()));
        }
        let riff_end = (u32le(data, 4).unwrap_or(0) as usize)
            .saturating_add(8)
            .min(data.len());
        let mut frames = Vec::new();
        let mut rate: Vec<u32> = Vec::new();
        let mut seq: Vec<u32> = Vec::new();
        let mut steps_n = 0usize;
        let mut default_rate = 10u32;
        let mut p = 12;
        // Walk chunks; LIST chunks are descended into.
        let mut ends = vec![riff_end];
        while let Some(&end) = ends.last() {
            if p + 8 > end {
                ends.pop();
                if let Some(&outer) = ends.last() {
                    p = end.min(outer);
                    continue;
                }
                break;
            }
            let id = &data[p..p + 4];
            let size = u32le(data, p + 4).unwrap_or(0) as usize;
            let body = p + 8;
            let body_end = body.saturating_add(size).min(end);
            let next = body.saturating_add(size).saturating_add(size & 1);
            match id {
                b"LIST" if size >= 4 => {
                    // Descend (into "fram" and any other list).
                    ends.push(body_end);
                    p = body + 4;
                    continue;
                }
                b"anih" if size >= 36 => {
                    let h = &data[body..body_end];
                    steps_n = u32le(h, 8).unwrap_or(0) as usize;
                    default_rate = u32le(h, 28).unwrap_or(10).max(1);
                    let flags = u32le(h, 32).unwrap_or(1);
                    if flags & 1 == 0 {
                        return Err(Error::Unsupported("ANI with raw frame data".into()));
                    }
                }
                b"icon" => frames.push(body..body_end),
                b"rate" => {
                    rate = data[body..body_end]
                        .chunks_exact(4)
                        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                        .collect()
                }
                b"seq " => {
                    seq = data[body..body_end]
                        .chunks_exact(4)
                        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                        .collect()
                }
                _ => {}
            }
            p = next;
        }
        if frames.is_empty() {
            return Err(Error::Malformed("ANI without frames".into()));
        }
        let n_steps = if !seq.is_empty() {
            seq.len()
        } else if steps_n > 0 {
            steps_n.min(frames.len().max(rate.len()))
        } else {
            frames.len()
        }
        .min(1 << 16);
        let steps = (0..n_steps)
            .filter_map(|i| {
                let frame = seq.get(i).map_or(i, |&f| f as usize);
                (frame < frames.len()).then(|| Step {
                    frame,
                    jiffies: rate.get(i).copied().unwrap_or(default_rate),
                })
            })
            .collect::<Vec<_>>();
        if steps.is_empty() {
            return Err(Error::Malformed("ANI sequence refers to no frame".into()));
        }
        Ok(Ani { frames, steps })
    }

    /// The ICO/CUR bytes of frame `i`.
    pub fn frame_data<'a>(&self, data: &'a [u8], i: usize) -> &'a [u8] {
        self.frames
            .get(i)
            .and_then(|r| data.get(r.clone()))
            .unwrap_or(&[])
    }
}
