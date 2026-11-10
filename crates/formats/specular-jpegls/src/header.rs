//! Marker segments (T.87 Annex C): SOF55, SOS, LSE, DRI, DNL and the APP8 colour transform.

use crate::{ColourTransform, Error, MappingTable, Result};

pub(crate) const SOI: u8 = 0xD8;
pub(crate) const EOI: u8 = 0xD9;
pub(crate) const SOS: u8 = 0xDA;
pub(crate) const DNL: u8 = 0xDC;
pub(crate) const DRI: u8 = 0xDD;
pub(crate) const SOF55: u8 = 0xF7;
pub(crate) const LSE: u8 = 0xF8;
pub(crate) const APP8: u8 = 0xE8;

/// One component of the frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Component {
    pub id: u8,
    /// Horizontal and vertical sampling factors (1-4).
    pub h: u8,
    pub v: u8,
    /// Size of this component's sample grid.
    pub width: u32,
    pub height: u32,
}

/// Coding parameters from an LSE type 1 segment; 0 means the default.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Preset {
    pub maxval: u16,
    pub t1: u16,
    pub t2: u16,
    pub t3: u16,
    pub reset: u16,
}

pub(crate) struct ScanHeader {
    /// Indices into the frame's components, and the mapping table id of each.
    pub comps: Vec<(usize, u8)>,
    pub near: u16,
    pub ilv: u8,
    /// Point transform Pt: decoded samples are shifted left by this.
    pub pt: u8,
}

/// Everything read from the marker segments so far.
#[derive(Default)]
pub(crate) struct State {
    pub frame: bool,
    pub precision: u8,
    pub width: u32,
    pub height: u32,
    pub comps: Vec<Component>,
    pub preset: Preset,
    pub restart: u32,
    pub transform: ColourTransform,
    pub tables: Vec<MappingTable>,
    /// Size from an LSE type 4 segment (height, width).
    pub oversize: Option<(u32, u32)>,
    /// Height from a DNL segment.
    pub dnl: Option<u32>,
}

fn be(b: &[u8]) -> u32 {
    b.iter().fold(0, |a, &x| (a << 8) | x as u32)
}

/// Finds the marker at `pos` (after any `FF` fill bytes): returns its code and the position after
/// it. `None` at the end of the data.
pub(crate) fn marker_at(data: &[u8], mut pos: usize) -> Result<Option<(u8, usize)>> {
    match data.get(pos) {
        None => return Ok(None),
        Some(0xFF) => {}
        Some(_) => return Err(Error::Malformed("expected a marker")),
    }
    while data.get(pos) == Some(&0xFF) {
        pos += 1;
    }
    match data.get(pos) {
        None => Ok(None),
        Some(&c) => Ok(Some((c, pos + 1))),
    }
}

/// The payload of the segment whose length field is at `pos`, and the position after it.
pub(crate) fn segment(data: &[u8], pos: usize) -> Result<(&[u8], usize)> {
    let len = data.get(pos..pos + 2).ok_or(Error::Truncated)?;
    let len = u16::from_be_bytes([len[0], len[1]]) as usize;
    if len < 2 {
        return Err(Error::Malformed("segment length below 2"));
    }
    let body = data.get(pos + 2..pos + len).ok_or(Error::Truncated)?;
    Ok((body, pos + len))
}

/// Position of the next marker in entropy-coded data at or after `pos` (an `FF` followed by a
/// byte >= 0x80), or the data length.
pub(crate) fn next_marker(data: &[u8], mut pos: usize) -> usize {
    while let Some(i) = data
        .get(pos..)
        .and_then(|d| d.iter().position(|&b| b == 0xFF))
    {
        let at = pos + i;
        match data.get(at + 1) {
            Some(&b) if b >= 0x80 => return at,
            Some(_) => pos = at + 2,
            None => return data.len(),
        }
    }
    data.len()
}

/// The code of the marker at `pos` (skipping fill bytes) and the position after it.
pub(crate) fn code_at(data: &[u8], pos: usize) -> Option<(u8, usize)> {
    marker_at(data, pos).ok().flatten()
}

impl State {
    /// Handles one marker segment other than SOS / EOI / SOI. `pos` is after the marker code.
    pub(crate) fn segment(&mut self, code: u8, data: &[u8], pos: usize) -> Result<usize> {
        let (body, next) = segment(data, pos)?;
        match code {
            SOF55 => self.sof(body)?,
            0xC0..=0xCF if code != 0xC4 && code != 0xC8 && code != 0xCC => {
                return Err(Error::NotJpegLs);
            }
            LSE => self.lse(body)?,
            DRI => {
                if !(2..=4).contains(&body.len()) {
                    return Err(Error::Malformed("DRI length"));
                }
                self.restart = be(body);
            }
            DNL => {
                if body.len() != 2 {
                    return Err(Error::Malformed("DNL length"));
                }
                self.dnl = Some(be(body));
            }
            APP8 if body.starts_with(b"mrfx") => {
                let t = *body.get(4).ok_or(Error::Malformed("short mrfx segment"))?;
                self.transform = match t {
                    0 => ColourTransform::None,
                    1 => ColourTransform::Hp1,
                    2 => ColourTransform::Hp2,
                    3 => ColourTransform::Hp3,
                    n => {
                        return Err(Error::Unsupported(format!("HP colour transform {n}")));
                    }
                };
            }
            // APPn, COM, DQT/DHT-like tables and anything else unknown: skipped.
            _ => {}
        }
        Ok(next)
    }

    fn sof(&mut self, b: &[u8]) -> Result<()> {
        if self.frame {
            return Err(Error::Malformed("second frame header"));
        }
        if b.len() < 6 {
            return Err(Error::Malformed("short SOF55"));
        }
        let p = b[0];
        if !(2..=16).contains(&p) {
            return Err(Error::Malformed("sample precision outside 2-16"));
        }
        let nf = b[5] as usize;
        if nf == 0 {
            return Err(Error::Malformed("no components"));
        }
        if b.len() < 6 + 3 * nf {
            return Err(Error::Malformed("short SOF55"));
        }
        if nf > 4 {
            return Err(Error::Unsupported(format!("{nf} components")));
        }
        self.precision = p;
        self.height = be(&b[1..3]);
        self.width = be(&b[3..5]);
        for c in b[6..6 + 3 * nf].chunks_exact(3) {
            let (h, v) = (c[1] >> 4, c[1] & 15);
            if !(1..=4).contains(&h) || !(1..=4).contains(&v) {
                return Err(Error::Malformed("sampling factor outside 1-4"));
            }
            if self.comps.iter().any(|o| o.id == c[0]) {
                return Err(Error::Malformed("duplicate component id"));
            }
            self.comps.push(Component {
                id: c[0],
                h,
                v,
                width: 0,
                height: 0,
            });
        }
        self.frame = true;
        Ok(())
    }

    fn lse(&mut self, b: &[u8]) -> Result<()> {
        let id = *b.first().ok_or(Error::Malformed("empty LSE"))?;
        match id {
            1 => {
                if b.len() < 11 {
                    return Err(Error::Malformed("short LSE preset"));
                }
                let w = |i: usize| u16::from_be_bytes([b[i], b[i + 1]]);
                self.preset = Preset {
                    maxval: w(1),
                    t1: w(3),
                    t2: w(5),
                    t3: w(7),
                    reset: w(9),
                };
            }
            2 | 3 => {
                if b.len() < 3 {
                    return Err(Error::Malformed("short LSE mapping table"));
                }
                let (tid, wt) = (b[1], b[2]);
                if tid == 0 {
                    return Err(Error::Malformed("mapping table id 0"));
                }
                if wt == 0 {
                    return Err(Error::Malformed("mapping table entry width 0"));
                }
                if wt > 4 {
                    return Err(Error::Unsupported(format!(
                        "mapping table entries of {wt} bytes"
                    )));
                }
                let entries = b[3..].chunks_exact(wt as usize).map(be);
                if id == 2 {
                    if self.tables.iter().any(|t| t.id == tid) {
                        // A redefinition replaces the old table.
                        self.tables.retain(|t| t.id != tid);
                    }
                    self.tables.push(MappingTable {
                        id: tid,
                        entry_bytes: wt,
                        entries: entries.collect(),
                    });
                } else {
                    let t = self
                        .tables
                        .iter_mut()
                        .find(|t| t.id == tid)
                        .ok_or(Error::Malformed("mapping table continuation without table"))?;
                    if t.entry_bytes != wt {
                        return Err(Error::Malformed("mapping table continuation width"));
                    }
                    if t.entries.len() + b.len() / wt as usize > 1 << 16 {
                        return Err(Error::Malformed("mapping table longer than 65536"));
                    }
                    t.entries.extend(entries);
                }
            }
            4 => {
                let wxy = *b.get(1).ok_or(Error::Malformed("short LSE size"))? as usize;
                if !(2..=4).contains(&wxy) || b.len() < 2 + 2 * wxy {
                    return Err(Error::Malformed("LSE size segment"));
                }
                let ye = be(&b[2..2 + wxy]);
                let xe = be(&b[2 + wxy..2 + 2 * wxy]);
                self.oversize = Some((ye, xe));
            }
            n => return Err(Error::Unsupported(format!("LSE segment type {n}"))),
        }
        Ok(())
    }

    /// Final frame size and per-component sizes; called before the first scan.
    pub(crate) fn resolve_size(&mut self, data: &[u8], scan_data: usize) -> Result<()> {
        if let Some((ye, xe)) = self.oversize {
            if self.height == 0 {
                self.height = ye;
            }
            if self.width == 0 {
                self.width = xe;
            }
        }
        if self.height == 0 && self.dnl.is_none() {
            // Height from the DNL segment after the first scan.
            let mut pos = scan_data;
            loop {
                pos = next_marker(data, pos);
                match code_at(data, pos) {
                    Some((c, after)) if (0xD0..=0xD7).contains(&c) => pos = after,
                    Some((DNL, after)) => {
                        let (body, _) = segment(data, after)?;
                        if body.len() == 2 {
                            self.dnl = Some(be(body));
                        }
                        break;
                    }
                    _ => break,
                }
            }
        }
        if self.height == 0 {
            self.height = self.dnl.unwrap_or(0);
        }
        if self.width == 0 || self.height == 0 {
            return Err(Error::Malformed("image has no size"));
        }
        let hmax = self.comps.iter().map(|c| c.h).max().unwrap_or(1) as u64;
        let vmax = self.comps.iter().map(|c| c.v).max().unwrap_or(1) as u64;
        let (w, h) = (self.width as u64, self.height as u64);
        for c in &mut self.comps {
            c.width = (w * c.h as u64).div_ceil(hmax) as u32;
            c.height = (h * c.v as u64).div_ceil(vmax) as u32;
        }
        Ok(())
    }

    pub(crate) fn sos(&self, b: &[u8]) -> Result<ScanHeader> {
        if !self.frame {
            return Err(Error::Malformed("scan before frame header"));
        }
        let ns = *b.first().ok_or(Error::Malformed("short SOS"))? as usize;
        if ns == 0 || ns > 4 || b.len() < 1 + 2 * ns + 3 {
            return Err(Error::Malformed("SOS component count"));
        }
        let mut comps = Vec::with_capacity(ns);
        for c in b[1..1 + 2 * ns].chunks_exact(2) {
            let i = self
                .comps
                .iter()
                .position(|o| o.id == c[0])
                .ok_or(Error::Malformed("scan names an unknown component"))?;
            if comps.iter().any(|&(j, _)| j == i) {
                return Err(Error::Malformed("component twice in a scan"));
            }
            if c[1] != 0 && !self.tables.iter().any(|t| t.id == c[1]) {
                return Err(Error::Malformed("scan uses an undefined mapping table"));
            }
            comps.push((i, c[1]));
        }
        let rest = &b[1 + 2 * ns..];
        let near = rest[0] as u16;
        let mut ilv = rest[1];
        // Pt: the scan is coded as usual (MAXVAL and the thresholds still follow P) and its
        // samples are shifted left by Pt afterwards (as the ISO reference libjpeg does).
        let pt = rest[2] & 15;
        if pt >= self.precision {
            return Err(Error::Malformed("point transform not below the precision"));
        }
        if pt != 0 && comps.iter().any(|&(_, t)| t != 0) {
            return Err(Error::Unsupported(
                "point transform with a mapping table".into(),
            ));
        }
        if ilv > 2 {
            return Err(Error::Malformed("interleave mode above 2"));
        }
        if ns == 1 {
            ilv = 0;
        } else if ilv == 0 {
            return Err(Error::Malformed(
                "several components in a non-interleaved scan",
            ));
        }
        if ilv == 2 {
            let c0 = &self.comps[comps[0].0];
            if comps
                .iter()
                .any(|&(i, _)| (self.comps[i].width, self.comps[i].height) != (c0.width, c0.height))
            {
                return Err(Error::Malformed(
                    "sample-interleaved components of different sizes",
                ));
            }
        }
        Ok(ScanHeader {
            comps,
            near,
            ilv,
            pt,
        })
    }
}
