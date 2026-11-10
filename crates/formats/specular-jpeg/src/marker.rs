//! Marker / header parsing. Never touches entropy-coded data in headers-only mode.

use std::ops::Range;
use std::sync::Arc;

use crate::error::{JpegError, Result};
use crate::huffman::HuffTable;
use crate::{Coding, ColorModel, ComponentInfo, JpegInfo};

/// Natural-order index of zig-zag position k. Extra 16 entries absorb run overflow on corrupt data.
pub(crate) const ZIGZAG: [u8; 80] = [
    0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27, 20,
    13, 6, 7, 14, 21, 28, 35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51, 58, 59,
    52, 45, 38, 31, 39, 46, 53, 60, 61, 54, 47, 55, 62, 63, // extra:
    63, 63, 63, 63, 63, 63, 63, 63, 63, 63, 63, 63, 63, 63, 63, 63,
];

#[derive(Clone, Debug)]
pub(crate) struct FrameComp {
    pub id: u8,
    pub h: u32,
    pub v: u32,
    pub tq: u8,
    /// Blocks per row / column actually covering the component (non-interleaved scan extent).
    pub bw: u32,
    pub bh: u32,
    /// Blocks per row / column padded to whole MCUs (interleaved scan extent).
    pub bw_pad: u32,
    pub bh_pad: u32,
}

#[derive(Clone, Debug)]
pub(crate) struct Frame {
    pub sof: u8,
    pub width: u32,
    pub height: u32,
    pub precision: u8,
    pub progressive: bool,
    pub arithmetic: bool,
    pub lossless: bool,
    /// A differential frame of a hierarchical image (T.81 Annex J): DC coded without
    /// prediction. Set only by the hierarchical decoder on the frames it splits out.
    pub differential: bool,
    pub comps: Vec<FrameComp>,
    pub max_h: u32,
    pub max_v: u32,
    pub mcus_x: u32,
    pub mcus_y: u32,
}

impl Frame {
    pub fn total_blocks(&self) -> u64 {
        self.comps
            .iter()
            .map(|c| c.bw_pad as u64 * c.bh_pad as u64)
            .sum()
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ScanComp {
    /// Index into frame components.
    pub ci: usize,
    pub td: usize,
    pub ta: usize,
}

/// Arithmetic-coding conditioning parameters (DAC).
#[derive(Clone, Copy, Debug)]
pub(crate) struct ArithCond {
    pub dc_l: [u8; 4],
    pub dc_u: [u8; 4],
    pub ac_k: [u8; 4],
}

impl Default for ArithCond {
    fn default() -> Self {
        ArithCond {
            dc_l: [0; 4],
            dc_u: [1; 4],
            ac_k: [5; 4],
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Scan {
    pub comps: Vec<ScanComp>,
    pub ss: u8,
    pub se: u8,
    pub ah: u8,
    pub al: u8,
    /// First byte of entropy-coded data.
    pub data_start: usize,
    /// Position of the marker terminating the scan (data.len() if unknown/none).
    pub data_end: usize,
    pub restart_interval: u32,
    pub dc: [Option<Arc<HuffTable>>; 4],
    pub ac: [Option<Arc<HuffTable>>; 4],
    /// DAC conditioning in effect for the scan (arithmetic-coded frames).
    pub arith: ArithCond,
}

pub(crate) struct Parsed {
    pub info: JpegInfo,
    pub frame: Frame,
    /// Dequantisation table per frame component, natural order, latched at the component's first scan.
    pub quant: Vec<[u16; 64]>,
    pub scans: Vec<Scan>,
    pub saw_eoi: bool,
    /// A DHP segment precedes the first frame (hierarchical mode, T.81 Annex J): `frame` and
    /// `info` then describe the final image the DHP segment defines, and `scans` belong to the
    /// first frame only. Decoded by `crate::hier`.
    pub hierarchical: bool,
}

struct Reader<'a> {
    data: &'a [u8],
}

impl<'a> Reader<'a> {
    fn u16_at(&self, p: usize) -> Option<u16> {
        Some(u16::from_be_bytes([
            *self.data.get(p)?,
            *self.data.get(p + 1)?,
        ]))
    }
}

/// Find the next marker that terminates entropy-coded data starting at `p`:
/// an 0xFF followed by a byte that is not 0x00, not RSTn and not 0xFF. Returns data.len() if none.
/// The line count of the DNL segment after the first scan, searched from `pos` (just after the
/// SOF segment): skips the table segments up to the first SOS, then that scan's data.
fn dnl_height(data: &[u8], mut pos: usize) -> Option<u16> {
    loop {
        while pos < data.len() && data[pos] != 0xFF {
            pos += 1;
        }
        while pos + 1 < data.len() && data[pos + 1] == 0xFF {
            pos += 1;
        }
        let m = *data.get(pos + 1)?;
        if matches!(m, 0xD8 | 0x01 | 0x00 | 0xD0..=0xD7) {
            pos += 2;
            continue;
        }
        if matches!(m, 0xD9 | 0xC0..=0xC3 | 0xC5..=0xC7 | 0xC9..=0xCB | 0xCD..=0xCF) {
            return None;
        }
        let len = u16::from_be_bytes([*data.get(pos + 2)?, *data.get(pos + 3)?]) as usize;
        if len < 2 {
            return None;
        }
        pos += 2 + len;
        if m == 0xDA {
            let end = find_scan_end(data, pos.min(data.len()));
            if data.get(end + 1) != Some(&0xDC) {
                return None;
            }
            let y = u16::from_be_bytes([*data.get(end + 4)?, *data.get(end + 5)?]);
            return (y != 0).then_some(y);
        }
    }
}

pub(crate) fn find_scan_end(data: &[u8], mut p: usize) -> usize {
    let n = data.len();
    while p < n {
        // SWAR search for 0xFF, 8 bytes at a time.
        while p + 8 <= n {
            let w = u64::from_le_bytes(data[p..p + 8].try_into().unwrap());
            if has_ff(w) {
                break;
            }
            p += 8;
        }
        while p < n && data[p] != 0xFF {
            p += 1;
        }
        if p + 1 >= n {
            return n;
        }
        let m = data[p + 1];
        if m == 0x00 || (0xD0..=0xD7).contains(&m) || m == 0xFF {
            p += if m == 0xFF { 1 } else { 2 };
            continue;
        }
        return p;
    }
    n
}

#[inline(always)]
pub(crate) fn has_ff(w: u64) -> bool {
    let x = !w;
    (x.wrapping_sub(0x0101_0101_0101_0101) & !x & 0x8080_8080_8080_8080) != 0
}

/// Parses the headers (and with `all_scans`, locates every scan). Logs the markers, the frame
/// and the scan script at debug level (target `jpeg`), and errors with their byte offset.
pub(crate) fn parse(data: &[u8], all_scans: bool) -> Result<Parsed> {
    parse_logged(data, all_scans, false)
}

/// [`parse`] with every scan after a headers-only [`parse`] of the same data: does not log
/// the header markers and the frame again.
pub(crate) fn reparse(data: &[u8]) -> Result<Parsed> {
    parse_logged(data, true, true)
}

fn parse_logged(data: &[u8], all_scans: bool, again: bool) -> Result<Parsed> {
    let mut at = 0usize;
    let r = parse_at(data, all_scans, again, &mut at);
    match &r {
        Ok(p) => {
            let f = &p.frame;
            if !again {
                tracing::debug!(
                    target: "jpeg",
                    width = f.width,
                    height = f.height,
                    precision = f.precision,
                    sof = format_args!("{:#04x}", 0xff00 | f.sof as u16),
                    coding = ?p.info.coding,
                    color = ?p.info.color,
                    sampling = ?f.comps.iter().map(|c| (c.id, c.h, c.v)).collect::<Vec<_>>(),
                    restart_interval = p.scans[0].restart_interval,
                    header_len = p.info.header_len,
                    scans = p.scans.len(),
                    eoi = p.saw_eoi,
                    "frame"
                );
            }
        }
        Err(e) => tracing::debug!(target: "jpeg", offset = at, error = %e, "header parse failed"),
    }
    r
}

/// Logs one entry of the scan script (debug, target `jpeg`).
fn log_scan(i: usize, offset: usize, s: &Scan, f: &Frame) {
    tracing::debug!(
        target: "jpeg",
        scan = i,
        offset,
        data_start = s.data_start,
        comps = ?s.comps.iter().map(|c| f.comps[c.ci].id).collect::<Vec<_>>(),
        ss = s.ss,
        se = s.se,
        ah = s.ah,
        al = s.al,
        restart_interval = s.restart_interval,
        "scan"
    );
}

/// [`parse`]; `at` tracks the offset of the marker being parsed (for error reports).
fn parse_at(data: &[u8], all_scans: bool, again: bool, at: &mut usize) -> Result<Parsed> {
    if data.len() < 4 || data[0] != 0xFF || data[1] != 0xD8 {
        return Err(JpegError::NotJpeg);
    }
    let r = Reader { data };
    let mut pos = 2usize;

    let mut frame: Option<Frame> = None;
    let mut qt: [Option<[u16; 64]>; 4] = [None; 4];
    let mut dc: [Option<Arc<HuffTable>>; 4] = Default::default();
    let mut ac: [Option<Arc<HuffTable>>; 4] = Default::default();
    let mut arith = ArithCond::default();
    let mut restart_interval: u32 = 0;
    let mut first_restart: Option<u16> = None;
    let mut jfif = false;
    let mut adobe: Option<u8> = None;
    let mut icc_chunks: Vec<(u8, u8, Range<usize>)> = Vec::new();
    let mut exif: Option<Range<usize>> = None;
    let mut xmp: Option<Range<usize>> = None;
    let mut has_c2pa = false;
    let mut header_len = 0usize;
    let mut scans: Vec<Scan> = Vec::new();
    let mut quant: Vec<Option<[u16; 64]>> = Vec::new();
    let mut saw_eoi = false;
    let mut dhp: Option<Range<usize>> = None;

    loop {
        // Locate next marker, skipping garbage and fill bytes.
        while pos < data.len() && data[pos] != 0xFF {
            pos += 1;
        }
        while pos + 1 < data.len() && data[pos + 1] == 0xFF {
            pos += 1;
        }
        if pos + 1 >= data.len() {
            break;
        }
        let m = data[pos + 1];
        let mpos = pos;
        *at = mpos;
        pos += 2;
        if !(again && scans.is_empty()) {
            tracing::debug!(target: "jpeg", offset = mpos, marker = format_args!("FF{m:02X}"), "marker");
        }
        match m {
            0xD8 | 0x01 | 0xD0..=0xD7 | 0x00 => continue,
            0xD9 => {
                saw_eoi = true;
                break;
            }
            _ => {}
        }
        let len = r.u16_at(pos).ok_or(JpegError::Truncated("marker length"))? as usize;
        if len < 2 {
            return Err(JpegError::Malformed("marker length < 2"));
        }
        let seg_start = pos + 2;
        let seg_end = pos + len;
        if seg_end > data.len() {
            if !scans.is_empty() {
                break;
            }
            return Err(JpegError::Truncated("segment extends past end of file"));
        }
        let seg = &data[seg_start..seg_end];
        pos = seg_end;
        match m {
            0xC0..=0xCF if m != 0xC4 && m != 0xC8 && m != 0xCC => {
                if frame.is_some() {
                    if !scans.is_empty() {
                        // Multiple frames (hierarchical): stop at the first one.
                        break;
                    }
                    return Err(JpegError::Malformed("multiple SOF markers"));
                }
                if seg.len() >= 3 && seg[1] == 0 && seg[2] == 0 {
                    // Height 0: the number of lines comes from the DNL segment that follows
                    // the first scan (T.81 B.2.5). Read it ahead so every path sees a size.
                    let y = dnl_height(data, seg_end).ok_or(JpegError::Malformed(
                        "height 0 without a DNL segment after the first scan",
                    ))?;
                    let mut s = seg.to_vec();
                    s[1..3].copy_from_slice(&y.to_be_bytes());
                    frame = Some(parse_sof(m, &s)?);
                } else {
                    frame = Some(parse_sof(m, seg)?);
                }
            }
            0xC4 => parse_dht(seg, &mut dc, &mut ac)?,
            0xDB => parse_dqt(seg, &mut qt)?,
            0xCC => parse_dac(seg, &mut arith)?,
            0xDD => {
                if seg.len() < 2 {
                    return Err(JpegError::Malformed("DRI"));
                }
                restart_interval = u16::from_be_bytes([seg[0], seg[1]]) as u32;
            }
            0xE0 => {
                if seg.starts_with(b"JFIF\0") {
                    jfif = true;
                }
            }
            0xE1 => {
                if seg.starts_with(b"Exif\0") && seg.len() >= 6 && exif.is_none() {
                    exif = Some(seg_start + 6..seg_end);
                } else if seg.starts_with(b"http://ns.adobe.com/xap/1.0/\0") && xmp.is_none() {
                    xmp = Some(seg_start + 29..seg_end);
                }
            }
            0xE2 => {
                if seg.len() >= 14 && seg.starts_with(b"ICC_PROFILE\0") {
                    icc_chunks.push((seg[12], seg[13], seg_start + 14..seg_end));
                }
            }
            0xEB => {
                // JUMBF boxes (C2PA) use the "JP" common identifier.
                if seg.starts_with(b"JP") {
                    has_c2pa = true;
                }
            }
            0xEE => {
                if seg.len() >= 12 && seg.starts_with(b"Adobe") {
                    adobe = Some(seg[11]);
                }
            }
            0xDA => {
                let f = frame
                    .as_ref()
                    .ok_or(JpegError::Malformed("SOS before SOF"))?;
                if scans.is_empty() {
                    header_len = mpos;
                    first_restart = Some(restart_interval.min(u16::MAX as u32) as u16);
                    quant = vec![None; f.comps.len()];
                }
                let scan = parse_sos(seg, f, &dc, &ac, arith, restart_interval, seg_end)?;
                for sc in &scan.comps {
                    if quant[sc.ci].is_none() {
                        quant[sc.ci] = qt[f.comps[sc.ci].tq as usize];
                    }
                }
                if !(again && scans.is_empty()) {
                    log_scan(scans.len(), mpos, &scan, f);
                }
                scans.push(scan);
                if !all_scans {
                    break;
                }
                let end = find_scan_end(data, seg_end);
                scans.last_mut().unwrap().data_end = end;
                pos = end;
            }
            0xDE if frame.is_none() => dhp = Some(seg_start..seg_end),
            0xDC => {
                // DNL: only legal after the first scan; read ahead when the SOF is parsed.
            }
            _ => {}
        }
    }

    let mut frame = frame.ok_or(JpegError::Truncated("no SOF marker"))?;
    if scans.is_empty() {
        return Err(JpegError::Truncated("no SOS marker"));
    }
    let hierarchical = dhp.is_some();
    if let Some(r) = dhp {
        // The final image: the DHP geometry with the coding flags of the first frame.
        frame = parse_sof(frame.sof, &data[r])?;
        quant = vec![Some([1; 64]); frame.comps.len()];
    }
    // Components that never got a quant table latched use the current definition.
    let mut quant_out = Vec::with_capacity(frame.comps.len());
    for (i, c) in frame.comps.iter().enumerate() {
        match quant.get(i).copied().flatten().or(qt[c.tq as usize]) {
            Some(q) => quant_out.push(q),
            None if frame.lossless => quant_out.push([1; 64]),
            None => return Err(JpegError::Malformed("missing quantisation table")),
        }
    }
    let quant = quant_out;

    let icc_profile = assemble_icc(data, &mut icc_chunks);
    let exif_thumbnail = exif.as_ref().and_then(|r| exif_thumbnail(data, r.clone()));

    let color = match frame.comps.len() {
        1 => ColorModel::Gray,
        3 => {
            let ids: Vec<u8> = frame.comps.iter().map(|c| c.id).collect();
            if jfif {
                ColorModel::YCbCr
            } else if let Some(t) = adobe {
                if t == 0 {
                    ColorModel::Rgb
                } else {
                    ColorModel::YCbCr
                }
            } else if ids == [82, 71, 66] {
                ColorModel::Rgb
            } else {
                ColorModel::YCbCr
            }
        }
        4 => match adobe {
            Some(0) | None => ColorModel::Cmyk,
            Some(_) => ColorModel::Ycck,
        },
        _ => return Err(JpegError::Unsupported("component count must be 1, 3 or 4")),
    };
    let coding = if frame.lossless {
        Coding::Lossless
    } else if frame.arithmetic {
        Coding::Arithmetic
    } else if frame.progressive {
        Coding::Progressive
    } else if frame.sof == 0xC0 {
        Coding::Baseline
    } else {
        Coding::ExtendedHuffman
    };
    let info = JpegInfo {
        width: frame.width,
        height: frame.height,
        precision: frame.precision,
        coding,
        color,
        components: frame
            .comps
            .iter()
            .map(|c| ComponentInfo {
                id: c.id,
                h_samp: c.h as u8,
                v_samp: c.v as u8,
                quant_table: c.tq,
            })
            .collect(),
        max_h: frame.max_h as u8,
        max_v: frame.max_v as u8,
        restart_interval: first_restart.unwrap_or(0),
        icc_profile,
        exif,
        xmp,
        exif_thumbnail,
        adobe_transform: adobe,
        has_c2pa,
        header_len,
        sof_marker: frame.sof,
        jfif,
        first_scan_components: scans[0].comps.len() as u8,
    };
    Ok(Parsed {
        info,
        frame,
        quant,
        scans,
        saw_eoi,
        hierarchical,
    })
}

/// The frame geometry described by `info` (as [`parse_sof`] derives it from the SOF segment).
pub(crate) fn frame_from_info(info: &JpegInfo) -> Result<Frame> {
    let (w, h) = (info.width, info.height);
    if w > u16::MAX as u32 || h > u16::MAX as u32 || info.components.len() > 4 {
        return Err(JpegError::Malformed("SOF"));
    }
    let mut seg = vec![info.precision];
    seg.extend_from_slice(&(h as u16).to_be_bytes());
    seg.extend_from_slice(&(w as u16).to_be_bytes());
    seg.push(info.components.len() as u8);
    for c in &info.components {
        seg.extend_from_slice(&[c.id, (c.h_samp << 4) | (c.v_samp & 15), c.quant_table]);
    }
    parse_sof(info.sof_marker, &seg)
}

fn parse_sof(m: u8, seg: &[u8]) -> Result<Frame> {
    if seg.len() < 6 {
        return Err(JpegError::Malformed("SOF too short"));
    }
    let precision = seg[0];
    let height = u16::from_be_bytes([seg[1], seg[2]]) as u32;
    let width = u16::from_be_bytes([seg[3], seg[4]]) as u32;
    let n = seg[5] as usize;
    if seg.len() < 6 + 3 * n {
        return Err(JpegError::Malformed("SOF too short"));
    }
    if n == 0 || n > 4 {
        return Err(JpegError::Unsupported("component count"));
    }
    if width == 0 {
        return Err(JpegError::Malformed("zero width"));
    }
    if height == 0 {
        return Err(JpegError::Malformed("zero height"));
    }
    let lossless = matches!(m, 0xC3 | 0xC7 | 0xCB | 0xCF);
    if matches!(m, 0xC5..=0xC7 | 0xCD..=0xCF) {
        return Err(JpegError::Malformed(
            "differential frame without a DHP segment (hierarchical mode)",
        ));
    }
    if !lossless && precision != 8 && precision != 12 {
        return Err(JpegError::Malformed("sample precision"));
    }
    if lossless && !(2..=16).contains(&precision) {
        return Err(JpegError::Malformed("sample precision"));
    }
    let mut comps = Vec::with_capacity(n);
    for i in 0..n {
        let b = &seg[6 + 3 * i..9 + 3 * i];
        let (h, v) = ((b[1] >> 4) as u32, (b[1] & 15) as u32);
        if !(1..=4).contains(&h) || !(1..=4).contains(&v) {
            return Err(JpegError::Malformed("sampling factor"));
        }
        if b[2] > 3 {
            return Err(JpegError::Malformed("quant table index"));
        }
        comps.push(FrameComp {
            id: b[0],
            h,
            v,
            tq: b[2],
            bw: 0,
            bh: 0,
            bw_pad: 0,
            bh_pad: 0,
        });
    }
    if n == 1 {
        // A single-component frame always uses 1x1 MCUs (one block), whatever it declares.
        comps[0].h = 1;
        comps[0].v = 1;
    }
    let max_h = comps.iter().map(|c| c.h).max().unwrap();
    let max_v = comps.iter().map(|c| c.v).max().unwrap();
    let mcus_x = width.div_ceil(8 * max_h);
    let mcus_y = height.div_ceil(8 * max_v);
    for c in &mut comps {
        let cw = (width as u64 * c.h as u64).div_ceil(max_h as u64) as u32;
        let ch = (height as u64 * c.v as u64).div_ceil(max_v as u64) as u32;
        c.bw = cw.div_ceil(8);
        c.bh = ch.div_ceil(8);
        c.bw_pad = mcus_x * c.h;
        c.bh_pad = mcus_y * c.v;
    }
    Ok(Frame {
        sof: m,
        width,
        height,
        precision,
        progressive: matches!(m, 0xC2 | 0xCA),
        arithmetic: m >= 0xC8,
        lossless,
        differential: false,
        comps,
        max_h,
        max_v,
        mcus_x,
        mcus_y,
    })
}

fn parse_dht(
    mut seg: &[u8],
    dc: &mut [Option<Arc<HuffTable>>; 4],
    ac: &mut [Option<Arc<HuffTable>>; 4],
) -> Result<()> {
    while !seg.is_empty() {
        if seg.len() < 17 {
            return Err(JpegError::Malformed("DHT too short"));
        }
        let tc = seg[0] >> 4;
        let th = (seg[0] & 15) as usize;
        if tc > 1 || th > 3 {
            return Err(JpegError::Malformed("DHT class/index"));
        }
        let mut counts = [0u8; 16];
        counts.copy_from_slice(&seg[1..17]);
        let total: usize = counts.iter().map(|&c| c as usize).sum();
        if total > 256 || seg.len() < 17 + total {
            return Err(JpegError::Malformed("DHT counts"));
        }
        let values = &seg[17..17 + total];
        let t = HuffTable::new(&counts, values, tc == 1)?;
        if tc == 0 {
            dc[th] = Some(Arc::new(t));
        } else {
            ac[th] = Some(Arc::new(t));
        }
        seg = &seg[17 + total..];
    }
    Ok(())
}

fn parse_dqt(mut seg: &[u8], qt: &mut [Option<[u16; 64]>; 4]) -> Result<()> {
    while !seg.is_empty() {
        let pq = seg[0] >> 4;
        let tq = (seg[0] & 15) as usize;
        if tq > 3 || pq > 1 {
            return Err(JpegError::Malformed("DQT"));
        }
        let n = if pq == 0 { 64 } else { 128 };
        if seg.len() < 1 + n {
            return Err(JpegError::Malformed("DQT too short"));
        }
        let mut t = [0u16; 64];
        for k in 0..64 {
            let v = if pq == 0 {
                seg[1 + k] as u16
            } else {
                u16::from_be_bytes([seg[1 + 2 * k], seg[2 + 2 * k]])
            };
            t[ZIGZAG[k] as usize] = v;
        }
        qt[tq] = Some(t);
        seg = &seg[1 + n..];
    }
    Ok(())
}

fn parse_dac(seg: &[u8], a: &mut ArithCond) -> Result<()> {
    for pair in seg.chunks(2) {
        if pair.len() < 2 {
            return Err(JpegError::Malformed("DAC"));
        }
        let tc = pair[0] >> 4;
        let tb = (pair[0] & 15) as usize;
        if tb > 3 {
            return Err(JpegError::Malformed("DAC index"));
        }
        if tc == 0 {
            let (l, u) = (pair[1] & 15, pair[1] >> 4);
            if l > u {
                return Err(JpegError::Malformed("DAC L > U"));
            }
            a.dc_l[tb] = l;
            a.dc_u[tb] = u;
        } else {
            if pair[1] == 0 || pair[1] > 63 {
                return Err(JpegError::Malformed("DAC K"));
            }
            a.ac_k[tb] = pair[1];
        }
    }
    Ok(())
}

fn parse_sos(
    seg: &[u8],
    f: &Frame,
    dc: &[Option<Arc<HuffTable>>; 4],
    ac: &[Option<Arc<HuffTable>>; 4],
    arith: ArithCond,
    restart_interval: u32,
    data_start: usize,
) -> Result<Scan> {
    if seg.is_empty() {
        return Err(JpegError::Malformed("SOS"));
    }
    let n = seg[0] as usize;
    if n == 0 || n > 4 || seg.len() < 1 + 2 * n + 3 {
        return Err(JpegError::Malformed("SOS component count"));
    }
    let mut comps = Vec::with_capacity(n);
    for i in 0..n {
        let id = seg[1 + 2 * i];
        let t = seg[2 + 2 * i];
        let ci = f
            .comps
            .iter()
            .position(|c| c.id == id)
            .ok_or(JpegError::Malformed("SOS component id"))?;
        if comps.iter().any(|c: &ScanComp| c.ci == ci) {
            return Err(JpegError::Malformed("duplicate SOS component"));
        }
        let (td, ta) = ((t >> 4) as usize, (t & 15) as usize);
        if td > 3 || ta > 3 {
            return Err(JpegError::Malformed("SOS table index"));
        }
        comps.push(ScanComp { ci, td, ta });
    }
    if n > 1 {
        let blocks: u32 = comps
            .iter()
            .map(|c| f.comps[c.ci].h * f.comps[c.ci].v)
            .sum();
        if blocks > 10 {
            return Err(JpegError::Malformed("too many blocks per MCU"));
        }
    }
    let p = 1 + 2 * n;
    let (ss, se, ah, al) = (seg[p], seg[p + 1], seg[p + 2] >> 4, seg[p + 2] & 15);
    if !f.lossless {
        if f.progressive {
            if se > 63
                || ss > se
                || (ss == 0 && se != 0)
                || (ss > 0 && n != 1)
                || ah > 13
                || al > 13
            {
                return Err(JpegError::Malformed("progressive scan parameters"));
            }
        } else if ss != 0 || se != 63 || ah != 0 || al != 0 {
            // libjpeg tolerates bogus values here; so do we.
        }
    }
    if !f.arithmetic && !f.lossless {
        for c in &comps {
            let needs_dc = !f.progressive || (ss == 0 && ah == 0);
            let needs_ac = !f.progressive || ss > 0;
            if needs_dc && dc[c.td].is_none() {
                return Err(JpegError::Malformed("missing DC Huffman table"));
            }
            if needs_ac && ac[c.ta].is_none() {
                return Err(JpegError::Malformed("missing AC Huffman table"));
            }
        }
    }
    Ok(Scan {
        comps,
        ss,
        se: if f.progressive || f.lossless { se } else { 63 },
        ah,
        al,
        data_start,
        data_end: usize::MAX,
        restart_interval,
        dc: dc.clone(),
        ac: ac.clone(),
        arith,
    })
}

fn assemble_icc(data: &[u8], chunks: &mut [(u8, u8, Range<usize>)]) -> Option<Vec<u8>> {
    if chunks.is_empty() {
        return None;
    }
    chunks.sort_by_key(|c| c.0);
    let count = chunks[0].1 as usize;
    if count == 0 || chunks.len() != count {
        return None;
    }
    for (i, c) in chunks.iter().enumerate() {
        if c.0 as usize != i + 1 || c.1 as usize != count {
            return None;
        }
    }
    let mut out = Vec::with_capacity(chunks.iter().map(|c| c.2.len()).sum());
    for c in chunks.iter() {
        out.extend_from_slice(&data[c.2.clone()]);
    }
    Some(out)
}

/// Resolve the IFD1 JPEG thumbnail inside an EXIF TIFF payload to an input byte range.
fn exif_thumbnail(data: &[u8], tiff_range: Range<usize>) -> Option<Range<usize>> {
    let t = data.get(tiff_range.clone())?;
    if t.len() < 8 {
        return None;
    }
    let le = match &t[0..2] {
        b"II" => true,
        b"MM" => false,
        _ => return None,
    };
    let u16_at = |p: usize| -> Option<u32> {
        let b = t.get(p..p + 2)?;
        Some(if le {
            u16::from_le_bytes([b[0], b[1]])
        } else {
            u16::from_be_bytes([b[0], b[1]])
        } as u32)
    };
    let u32_at = |p: usize| -> Option<u32> {
        let b = t.get(p..p + 4)?;
        let a = [b[0], b[1], b[2], b[3]];
        Some(if le {
            u32::from_le_bytes(a)
        } else {
            u32::from_be_bytes(a)
        })
    };
    let ifd0 = u32_at(4)? as usize;
    let n0 = u16_at(ifd0)? as usize;
    let ifd1 = u32_at(ifd0 + 2 + 12 * n0)? as usize;
    if ifd1 == 0 || ifd1 == ifd0 {
        return None;
    }
    let n1 = u16_at(ifd1)? as usize;
    let (mut off, mut len) = (None, None);
    for i in 0..n1.min(256) {
        let e = ifd1 + 2 + 12 * i;
        let tag = u16_at(e)?;
        let typ = u16_at(e + 2)?;
        let val = if typ == 3 {
            u16_at(e + 8)?
        } else {
            u32_at(e + 8)?
        };
        match tag {
            0x0201 => off = Some(val as usize),
            0x0202 => len = Some(val as usize),
            _ => {}
        }
    }
    let (off, len) = (off?, len?);
    if len < 4 || off.checked_add(len)? > t.len() {
        return None;
    }
    if t[off] != 0xFF || t[off + 1] != 0xD8 {
        return None;
    }
    Some(tiff_range.start + off..tiff_range.start + off + len)
}
