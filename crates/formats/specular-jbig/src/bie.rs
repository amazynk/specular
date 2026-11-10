//! The bi-level image header (BIH, T.82 6.2.2) and the layout of the bi-level image data
//! (BID, 6.2.4 to 6.2.7): stripe data entities (SDEs) and floating marker segments.

use crate::{Error, Result};

pub(crate) const ESC: u8 = 0xff;
pub(crate) const STUFF: u8 = 0x00;
pub(crate) const RESERVE: u8 = 0x01;
pub(crate) const SDNORM: u8 = 0x02;
pub(crate) const SDRST: u8 = 0x03;
pub(crate) const ABORT: u8 = 0x04;
pub(crate) const NEWLEN: u8 = 0x05;
pub(crate) const ATMOVE: u8 = 0x06;
pub(crate) const COMMENT: u8 = 0x07;

pub(crate) const HITOLO: u8 = 0x08;
pub(crate) const SEQ: u8 = 0x04;
pub(crate) const ILEAVE: u8 = 0x02;
pub(crate) const SMID: u8 = 0x01;

pub(crate) const LRLTWO: u8 = 0x40;
pub(crate) const VLENGTH: u8 = 0x20;
pub(crate) const TPDON: u8 = 0x10;
pub(crate) const TPBON: u8 = 0x08;
pub(crate) const DPON: u8 = 0x04;
pub(crate) const DPPRIV: u8 = 0x02;
pub(crate) const DPLAST: u8 = 0x01;

/// Size of a private deterministic-prediction table (6.6.3).
pub(crate) const DPTABLE_LEN: usize = 1728;

/// Most resolution layers accepted (each halves the size; more than 31 cannot matter).
pub(crate) const MAX_LAYERS: u8 = 31;

#[derive(Clone, Debug)]
pub(crate) struct Bih {
    pub dl: u8,
    pub d: u8,
    pub planes: u8,
    pub xd: u32,
    pub yd: u32,
    pub l0: u32,
    pub mx: u8,
    pub my: u8,
    pub order: u8,
    pub options: u8,
    /// The private DP table, when the header carries one.
    pub dptable: Option<[u8; DPTABLE_LEN]>,
    /// Where the BID starts.
    pub data_offset: usize,
}

fn be32(d: &[u8], o: usize) -> u32 {
    u32::from_be_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]])
}

/// Whether `head` starts with a plausible BIH (the format has no signature).
pub(crate) fn probe(head: &[u8]) -> bool {
    if head.len() < 20 {
        return false;
    }
    let (dl, d, p, fill) = (head[0], head[1], head[2], head[3]);
    let (xd, yd, l0) = (be32(head, 4), be32(head, 8), be32(head, 12));
    let (mx, order, options) = (head[16], head[18], head[19]);
    fill == 0
        && dl <= d
        && d <= MAX_LAYERS
        && (1..=32).contains(&p)
        && (1..=1 << 24).contains(&xd)
        && yd >= 1
        && (1..=1 << 24).contains(&l0)
        && mx <= 127
        && order & 0xf0 == 0
        && !matches!(order & 7, 1 | 7)
        && options & 0x80 == 0
        // Without DP the DP flags have no meaning; encoders leave them clear.
        && (options & DPON != 0 || options & (DPPRIV | DPLAST) == 0)
        && (yd != u32::MAX || options & VLENGTH != 0 || yd < 1 << 24)
}

pub(crate) fn parse(data: &[u8]) -> Result<Bih> {
    if data.len() < 20 {
        return Err(Error::NotRecognised);
    }
    if !probe(data) {
        return Err(Error::NotRecognised);
    }
    let options = data[19];
    let mut b = Bih {
        dl: data[0],
        d: data[1],
        planes: data[2],
        xd: be32(data, 4),
        yd: be32(data, 8),
        l0: be32(data, 12),
        mx: data[16],
        my: data[17],
        order: data[18],
        options,
        dptable: None,
        data_offset: 20,
    };
    if options & DPON != 0 && options & DPPRIV != 0 {
        if options & DPLAST != 0 {
            return Err(Error::Unsupported(
                "this JBIG file reuses a prediction table from an earlier file, so it cannot be \
                 shown on its own"
                    .into(),
            ));
        }
        let t = data
            .get(20..20 + DPTABLE_LEN)
            .ok_or_else(|| Error::Truncated("the private prediction table is cut short".into()))?;
        let mut tab = [0u8; DPTABLE_LEN];
        tab.copy_from_slice(t);
        b.dptable = Some(tab);
        b.data_offset += DPTABLE_LEN;
    }
    Ok(b)
}

/// An adaptive-template move (6.2.6.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AtMove {
    /// Line within the stripe at whose start the move takes effect.
    pub yat: u32,
    pub tx: i8,
    pub ty: u8,
}

/// One stripe data entity.
#[derive(Clone, Debug, Default)]
pub(crate) struct Sde {
    /// The stripe coded data (stuffing removed); empty when only scanned.
    pub scd: Vec<u8>,
    /// Ended with SDRST: the state is reset before the next stripe of this plane and layer.
    pub reset: bool,
    /// The SDE ended without its terminating marker (the file is cut short).
    pub cut: bool,
    /// ATMOVE segments that precede it.
    pub moves: Vec<AtMove>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Bid {
    pub sdes: Vec<Sde>,
    pub newlen: Option<u32>,
    pub comments: Vec<String>,
    /// An ABORT marker ended the data early.
    pub aborted: bool,
    pub at_moves: usize,
}

/// Split the BID into SDEs and marker segments. `keep`: copy out each SDE's coded data.
pub(crate) fn scan(data: &[u8], start: usize, keep: bool) -> Result<Bid> {
    let mut bid = Bid::default();
    let mut moves = Vec::new();
    let mut i = start;
    let n = data.len();
    while i < n {
        // Floating marker segments between SDEs.
        if data[i] == ESC && i + 1 < n {
            match data[i + 1] {
                NEWLEN => {
                    let v = data
                        .get(i + 2..i + 6)
                        .ok_or_else(|| Error::Truncated("NEWLEN marker cut short".into()))?;
                    bid.newlen = Some(u32::from_be_bytes([v[0], v[1], v[2], v[3]]));
                    i += 6;
                    continue;
                }
                ATMOVE => {
                    let v = data
                        .get(i + 2..i + 8)
                        .ok_or_else(|| Error::Truncated("ATMOVE marker cut short".into()))?;
                    moves.push(AtMove {
                        yat: u32::from_be_bytes([v[0], v[1], v[2], v[3]]),
                        tx: v[4] as i8,
                        ty: v[5],
                    });
                    bid.at_moves += 1;
                    i += 8;
                    continue;
                }
                COMMENT => {
                    let v = data
                        .get(i + 2..i + 6)
                        .ok_or_else(|| Error::Truncated("comment cut short".into()))?;
                    let len = u32::from_be_bytes([v[0], v[1], v[2], v[3]]) as usize;
                    let body = data.get(i + 6..).unwrap_or(&[]);
                    let body = &body[..len.min(body.len())];
                    let text = String::from_utf8_lossy(body);
                    let text = text.trim_matches(|c: char| c == '\0' || c.is_whitespace());
                    if !text.is_empty() && bid.comments.len() < 64 {
                        bid.comments.push(text.chars().take(4096).collect());
                    }
                    i = i.saturating_add(6).saturating_add(len);
                    continue;
                }
                RESERVE => {
                    return Err(Error::Malformed(
                        "a reserved JBIG marker appears in the data".into(),
                    ));
                }
                ABORT => {
                    bid.aborted = true;
                    break;
                }
                _ => {}
            }
        }
        // An SDE: protected coded data up to ESC SDNORM / ESC SDRST.
        let mut sde = Sde {
            moves: std::mem::take(&mut moves),
            ..Sde::default()
        };
        let mut seg = i;
        loop {
            let Some(off) = data[i..].iter().position(|&b| b == ESC) else {
                if keep {
                    sde.scd.extend_from_slice(&data[seg..]);
                }
                sde.cut = true;
                i = n;
                break;
            };
            let at = i + off;
            if keep {
                sde.scd.extend_from_slice(&data[seg..at]);
            }
            let Some(&m) = data.get(at + 1) else {
                sde.cut = true;
                i = n;
                break;
            };
            match m {
                STUFF => {
                    if keep {
                        sde.scd.push(ESC);
                    }
                    i = at + 2;
                    seg = i;
                }
                SDNORM | SDRST => {
                    sde.reset = m == SDRST;
                    i = at + 2;
                    break;
                }
                NEWLEN => {
                    // Some encoders put NEWLEN right before the SDE's end (6.2.6.2, note 2).
                    let v = data
                        .get(at + 2..at + 6)
                        .ok_or_else(|| Error::Truncated("NEWLEN marker cut short".into()))?;
                    bid.newlen = Some(u32::from_be_bytes([v[0], v[1], v[2], v[3]]));
                    i = at + 6;
                    seg = i;
                }
                ABORT => {
                    bid.aborted = true;
                    sde.cut = true;
                    i = n;
                    break;
                }
                COMMENT | ATMOVE => {
                    // Marker segments may not occur inside an SDE; treat the SDE as ended
                    // (the segment is read on the next pass).
                    sde.cut = true;
                    i = at;
                    break;
                }
                _ => {
                    return Err(Error::Malformed(format!(
                        "unexpected marker 0x{m:02x} in the coded data"
                    )));
                }
            }
        }
        let stop = bid.aborted;
        bid.sdes.push(sde);
        if stop {
            break;
        }
    }
    Ok(bid)
}

/// Ceiling halving `n` times (6.2.3).
pub(crate) fn reduce(v: u64, n: u32) -> u64 {
    if n >= 64 {
        return u64::from(v > 0);
    }
    v.div_ceil(1u64 << n)
}

impl Bih {
    pub(crate) fn layers(&self) -> u32 {
        u32::from(self.d - self.dl) + 1
    }

    /// Size of layer `d` for image height `yd`.
    pub(crate) fn layer_size(&self, d: u8, yd: u32) -> (u64, u64) {
        let k = u32::from(self.d - d);
        (reduce(u64::from(self.xd), k), reduce(u64::from(yd), k))
    }

    /// Number of stripes (equation 4).
    pub(crate) fn stripes(&self, yd: u32) -> u64 {
        let y0 = reduce(u64::from(yd), u32::from(self.d));
        y0.div_ceil(u64::from(self.l0)).max(1)
    }

    /// Position of SDE (stripe `s`, layer `d`, plane `p`) in the BID (Table 11).
    pub(crate) fn sde_index(&self, s: u64, d: u8, p: u8, stripes: u64) -> Result<u64> {
        let nd = u64::from(self.layers());
        let np = u64::from(self.planes);
        let di = if self.order & HITOLO != 0 {
            u64::from(self.d - d)
        } else {
            u64::from(d - self.dl)
        };
        let pi = u64::from(self.planes - 1 - p);
        // (index, count) per dimension, outer to inner.
        let (o, m, i) = match self.order & 7 {
            0 => ((pi, np), (di, nd), (s, stripes)),
            o if o == ILEAVE => ((di, nd), (pi, np), (s, stripes)),
            o if o == ILEAVE | SMID => ((di, nd), (s, stripes), (pi, np)),
            o if o == SEQ => ((s, stripes), (pi, np), (di, nd)),
            o if o == SEQ | SMID => ((pi, np), (s, stripes), (di, nd)),
            o if o == SEQ | ILEAVE => ((s, stripes), (di, nd), (pi, np)),
            _ => return Err(Error::Malformed("invalid JBIG stripe order".into())),
        };
        Ok((o.0 * m.1 + m.0) * i.1 + i.0)
    }
}
