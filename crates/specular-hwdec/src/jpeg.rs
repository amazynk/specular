//! Just enough JPEG parsing to drive a fixed-function decoder: the tables, the frame and the
//! single interleaved scan of a baseline (or 8-bit extended Huffman) file. Anything else
//! (progressive, arithmetic, 12-bit, several scans, Adobe RGB/CMYK, missing tables, unusual
//! sampling) is [`Unsupported`](crate::Error::Unsupported) and goes to the software decoder.
//!
//! The parsing happens here, in safe Rust, so the driver never sees the container: it gets
//! validated parameter structs and the entropy-coded bytes of the scan.

use crate::Error;

/// Chroma subsampling of a hardware-decodable JPEG.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sampling {
    Gray,
    S444,
    S422,
    S420,
    /// Luma 1×2 (vertical 4:2:2, "4:4:0").
    S440,
}

#[derive(Clone, Debug)]
pub struct FrameComponent {
    pub id: u8,
    pub h: u8,
    pub v: u8,
    pub tq: u8,
}

#[derive(Clone, Debug, Default)]
pub struct Huffman {
    pub bits: [u8; 16],
    pub values: Vec<u8>,
}

/// A parsed baseline JPEG, borrowing the scan data.
#[derive(Clone, Debug)]
pub struct Baseline<'a> {
    pub width: u16,
    pub height: u16,
    pub components: Vec<FrameComponent>,
    /// Quantisation tables 0–3 in zig-zag order.
    pub quant: [Option<[u8; 64]>; 4],
    /// DC and AC tables 0–1.
    pub dc: [Option<Huffman>; 2],
    pub ac: [Option<Huffman>; 2],
    pub restart_interval: u16,
    /// (component selector, DC table, AC table) of the scan.
    pub scan: Vec<(u8, u8, u8)>,
    /// Entropy-coded data of the scan, restart markers included, up to the next marker (the
    /// EOI included when it follows).
    pub data: &'a [u8],
    pub sampling: Sampling,
}

impl Baseline<'_> {
    /// MCU size in pixels.
    pub fn mcu(&self) -> (u32, u32) {
        if self.components.len() == 1 {
            return (8, 8);
        }
        let h = self.components.iter().map(|c| c.h).max().unwrap_or(1) as u32;
        let v = self.components.iter().map(|c| c.v).max().unwrap_or(1) as u32;
        (8 * h, 8 * v)
    }

    pub fn num_mcus(&self) -> u32 {
        let (mw, mh) = self.mcu();
        (self.width as u32).div_ceil(mw) * (self.height as u32).div_ceil(mh)
    }
}

fn unsupported(s: &str) -> Error {
    Error::Unsupported(s.to_owned())
}

fn be16(d: &[u8], i: usize) -> Result<u16, Error> {
    match d.get(i..i + 2) {
        Some(b) => Ok(u16::from_be_bytes([b[0], b[1]])),
        None => Err(Error::Malformed("truncated marker".into())),
    }
}

/// Parses `data` as a single-scan baseline JPEG.
pub fn parse(data: &[u8]) -> Result<Baseline<'_>, Error> {
    if !data.starts_with(&[0xff, 0xd8]) {
        return Err(Error::Malformed("not a JPEG".into()));
    }
    let mut quant = [None; 4];
    let mut dc: [Option<Huffman>; 2] = [None, None];
    let mut ac: [Option<Huffman>; 2] = [None, None];
    let mut frame: Option<(u16, u16, Vec<FrameComponent>)> = None;
    let mut restart_interval = 0;
    let mut adobe_transform: Option<u8> = None;
    let mut i = 2;
    loop {
        // Skip fill bytes before a marker.
        while data.get(i) == Some(&0xff) && data.get(i + 1) == Some(&0xff) {
            i += 1;
        }
        if data.get(i) != Some(&0xff) {
            return Err(Error::Malformed("expected a marker".into()));
        }
        let m = *data
            .get(i + 1)
            .ok_or_else(|| Error::Malformed("truncated".into()))?;
        i += 2;
        if m == 0xd8 || (0xd0..=0xd7).contains(&m) || m == 0x01 {
            continue;
        }
        if m == 0xd9 {
            return Err(Error::Malformed("no scan".into()));
        }
        let len = be16(data, i)? as usize;
        if len < 2 || i + len > data.len() {
            return Err(Error::Malformed("segment runs past the end".into()));
        }
        let seg = &data[i + 2..i + len];
        match m {
            0xc0 | 0xc1 => {
                if frame.is_some() {
                    return Err(Error::Malformed("two frames".into()));
                }
                if seg.len() < 6 || seg[0] != 8 {
                    return Err(unsupported("not 8-bit"));
                }
                let h = be16(seg, 1)?;
                let w = be16(seg, 3)?;
                let n = seg[5] as usize;
                if w == 0 || h == 0 {
                    return Err(unsupported("height from DNL"));
                }
                if !(n == 1 || n == 3) || seg.len() < 6 + 3 * n {
                    return Err(unsupported("component count"));
                }
                let comps = (0..n)
                    .map(|k| {
                        let c = &seg[6 + 3 * k..9 + 3 * k];
                        FrameComponent {
                            id: c[0],
                            h: c[1] >> 4,
                            v: c[1] & 15,
                            tq: c[2],
                        }
                    })
                    .collect::<Vec<_>>();
                if comps
                    .iter()
                    .any(|c| c.tq > 3 || !(1..=4).contains(&c.h) || !(1..=4).contains(&c.v))
                {
                    return Err(Error::Malformed("bad component".into()));
                }
                frame = Some((w, h, comps));
            }
            0xc2 | 0xc3 | 0xc5..=0xc7 | 0xc9..=0xcb | 0xcd..=0xcf => {
                return Err(unsupported("not a baseline JPEG"));
            }
            0xc4 => {
                let mut s = seg;
                while !s.is_empty() {
                    if s.len() < 17 {
                        return Err(Error::Malformed("short DHT".into()));
                    }
                    let (class, id) = (s[0] >> 4, (s[0] & 15) as usize);
                    let mut bits = [0u8; 16];
                    bits.copy_from_slice(&s[1..17]);
                    let n: usize = bits.iter().map(|&b| b as usize).sum();
                    if s.len() < 17 + n {
                        return Err(Error::Malformed("short DHT".into()));
                    }
                    if id > 1 || class > 1 {
                        return Err(unsupported("Huffman table id above 1"));
                    }
                    let max = if class == 0 { 12 } else { 162 };
                    if n > max {
                        return Err(Error::Malformed("Huffman table too large".into()));
                    }
                    let t = Huffman {
                        bits,
                        values: s[17..17 + n].to_vec(),
                    };
                    if class == 0 {
                        dc[id] = Some(t);
                    } else {
                        ac[id] = Some(t);
                    }
                    s = &s[17 + n..];
                }
            }
            0xdb => {
                let mut s = seg;
                while !s.is_empty() {
                    let (pq, tq) = (s[0] >> 4, (s[0] & 15) as usize);
                    if pq != 0 {
                        return Err(unsupported("16-bit quantisation table"));
                    }
                    if tq > 3 || s.len() < 65 {
                        return Err(Error::Malformed("bad DQT".into()));
                    }
                    let mut q = [0u8; 64];
                    q.copy_from_slice(&s[1..65]);
                    quant[tq] = Some(q);
                    s = &s[65..];
                }
            }
            0xdd => {
                restart_interval = be16(seg, 0)?;
            }
            0xee => {
                if seg.len() >= 12 && seg.starts_with(b"Adobe") {
                    adobe_transform = Some(seg[11]);
                }
            }
            0xda => {
                let (width, height, components) =
                    frame.ok_or_else(|| Error::Malformed("scan before frame".into()))?;
                let ns = *seg.first().unwrap_or(&0) as usize;
                if ns != components.len() || seg.len() < 1 + 2 * ns + 3 {
                    return Err(unsupported("several scans"));
                }
                let scan: Vec<(u8, u8, u8)> = (0..ns)
                    .map(|k| (seg[1 + 2 * k], seg[2 + 2 * k] >> 4, seg[2 + 2 * k] & 15))
                    .collect();
                let (ss, se, a) = (seg[1 + 2 * ns], seg[2 + 2 * ns], seg[3 + 2 * ns]);
                if ss != 0 || se != 63 || a != 0 {
                    return Err(unsupported("spectral selection"));
                }
                for (k, &(cs, td, ta)) in scan.iter().enumerate() {
                    if components[k].id != cs {
                        return Err(unsupported("scan order differs from frame"));
                    }
                    if td > 1 || ta > 1 {
                        return Err(unsupported("Huffman table id above 1"));
                    }
                    if dc[td as usize].is_none() || ac[ta as usize].is_none() {
                        return Err(unsupported("missing Huffman table"));
                    }
                    if quant[components[k].tq as usize].is_none() {
                        return Err(Error::Malformed("missing quantisation table".into()));
                    }
                }
                if components.len() == 3 && adobe_transform == Some(0) {
                    return Err(unsupported("RGB JPEG"));
                }
                let sampling = sampling_of(&components)?;
                let start = i + len;
                let mut end = scan_end(data, start);
                // Keep the EOI: AMD's VCN JPEG engine (Mesa) leaves the surface untouched
                // without it.
                if data.get(end..end + 2) == Some(&[0xff, 0xd9]) {
                    end += 2;
                }
                return Ok(Baseline {
                    width,
                    height,
                    components,
                    quant,
                    dc,
                    ac,
                    restart_interval,
                    scan,
                    data: &data[start..end],
                    sampling,
                });
            }
            _ => {}
        }
        i += len;
    }
}

fn sampling_of(c: &[FrameComponent]) -> Result<Sampling, Error> {
    if c.len() == 1 {
        return Ok(Sampling::Gray);
    }
    if c[1..].iter().any(|c| c.h != 1 || c.v != 1) {
        return Err(unsupported("chroma sampling"));
    }
    match (c[0].h, c[0].v) {
        (1, 1) => Ok(Sampling::S444),
        (2, 1) => Ok(Sampling::S422),
        (2, 2) => Ok(Sampling::S420),
        (1, 2) => Ok(Sampling::S440),
        _ => Err(unsupported("chroma sampling")),
    }
}

/// End of the entropy-coded segment starting at `i`: the first marker other than a stuffed
/// zero or a restart marker (or the end of the data for truncated files).
fn scan_end(d: &[u8], mut i: usize) -> usize {
    while i + 1 < d.len() {
        match memchr_ff(&d[i..]) {
            None => return d.len(),
            Some(k) => {
                i += k;
                match d.get(i + 1) {
                    Some(0x00) | Some(0xd0..=0xd7) | Some(0xff) => i += 1,
                    Some(_) => return i,
                    None => return d.len(),
                }
            }
        }
    }
    d.len()
}

fn memchr_ff(s: &[u8]) -> Option<usize> {
    s.iter().position(|&b| b == 0xff)
}
