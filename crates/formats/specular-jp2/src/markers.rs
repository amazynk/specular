//! Codestream marker segments (Annex A): main header, tile-part index and tile-part headers.

use std::ops::Range;

use crate::{Jp2Error, Result};

pub(crate) const SOC: u16 = 0xFF4F;
pub(crate) const CAP: u16 = 0xFF50;
pub(crate) const SIZ: u16 = 0xFF51;
pub(crate) const COD: u16 = 0xFF52;
pub(crate) const COC: u16 = 0xFF53;
pub(crate) const TLM: u16 = 0xFF55;
pub(crate) const PLM: u16 = 0xFF57;
pub(crate) const PLT: u16 = 0xFF58;
pub(crate) const QCD: u16 = 0xFF5C;
pub(crate) const QCC: u16 = 0xFF5D;
pub(crate) const RGN: u16 = 0xFF5E;
pub(crate) const POC: u16 = 0xFF5F;
pub(crate) const PPM: u16 = 0xFF60;
pub(crate) const PPT: u16 = 0xFF61;
pub(crate) const CRG: u16 = 0xFF63;
pub(crate) const COM: u16 = 0xFF64;
/// Non-linearity point transform (T.801 | ISO/IEC 15444-2, used by HTJ2K in OpenEXR).
pub(crate) const NLT: u16 = 0xFF76;
pub(crate) const SOT: u16 = 0xFF90;
pub(crate) const SOD: u16 = 0xFF93;
pub(crate) const EOC: u16 = 0xFFD9;

/// Caps that keep allocations bounded for hostile headers.
pub(crate) const MAX_COMPONENTS: usize = 16384;
const MAX_TILE_PARTS: usize = 1 << 20;
const MAX_POC: usize = 1 << 12;

/// Progression order (Table A.16).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Progression {
    /// Layer-resolution-component-position.
    Lrcp,
    /// Resolution-layer-component-position.
    Rlcp,
    /// Resolution-position-component-layer.
    Rpcl,
    /// Position-component-resolution-layer.
    Pcrl,
    /// Component-position-resolution-layer.
    Cprl,
}

impl Progression {
    fn from_u8(v: u8) -> Result<Progression> {
        Ok(match v {
            0 => Progression::Lrcp,
            1 => Progression::Rlcp,
            2 => Progression::Rpcl,
            3 => Progression::Pcrl,
            4 => Progression::Cprl,
            _ => return Err(Jp2Error::Malformed("bad progression order")),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CompSiz {
    pub prec: u8,
    pub signed: bool,
    pub dx: u8,
    pub dy: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Siz {
    pub rsiz: u16,
    pub x1: u32,
    pub y1: u32,
    pub x0: u32,
    pub y0: u32,
    pub tw: u32,
    pub th: u32,
    pub tx0: u32,
    pub ty0: u32,
    pub comps: Vec<CompSiz>,
    pub tiles_x: u32,
    pub tiles_y: u32,
}

impl Siz {
    pub fn num_tiles(&self) -> usize {
        self.tiles_x as usize * self.tiles_y as usize
    }
    /// Tile rectangle on the reference grid.
    pub fn tile_rect(&self, t: usize) -> crate::geom::Rect {
        let p = (t % self.tiles_x as usize) as u64;
        let q = (t / self.tiles_x as usize) as u64;
        let x0 = (self.tx0 as u64 + p * self.tw as u64).max(self.x0 as u64);
        let y0 = (self.ty0 as u64 + q * self.th as u64).max(self.y0 as u64);
        let x1 = (self.tx0 as u64 + (p + 1) * self.tw as u64).min(self.x1 as u64);
        let y1 = (self.ty0 as u64 + (q + 1) * self.th as u64).min(self.y1 as u64);
        crate::geom::Rect::new(x0 as u32, y0 as u32, x1 as u32, y1 as u32)
    }
}

/// Code-block style bits (Table A.19).
pub(crate) const CBS_BYPASS: u8 = 0x01;
pub(crate) const CBS_RESET: u8 = 0x02;
pub(crate) const CBS_TERMALL: u8 = 0x04;
pub(crate) const CBS_CAUSAL: u8 = 0x08;
#[allow(dead_code)]
pub(crate) const CBS_ERTERM: u8 = 0x10;
pub(crate) const CBS_SEGSYM: u8 = 0x20;
pub(crate) const CBS_HT: u8 = 0x40;
/// With [`CBS_HT`]: code-blocks may be HT or Part 1 (signalled per block).
pub(crate) const CBS_MIXED: u8 = 0x80;

/// Per-component coding style (SPcod / SPcoc).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CompCoding {
    pub levels: u8,
    /// Code-block width / height exponents.
    pub cbw: u8,
    pub cbh: u8,
    pub style: u8,
    pub reversible: bool,
    /// Precinct size exponents (PPx, PPy) per resolution level, `levels + 1` entries.
    pub precincts: Vec<(u8, u8)>,
}

/// SGcod part of COD.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CodGlobal {
    pub sop: bool,
    pub eph: bool,
    pub prog: Progression,
    pub layers: u16,
    pub mct: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Quant {
    /// 0: none (reversible), 1: scalar derived, 2: scalar expounded.
    pub style: u8,
    pub guard: u8,
    /// (exponent, mantissa) per band in band order (LL, then HL/LH/HH per level).
    pub steps: Vec<(u8, u16)>,
}

impl Quant {
    /// (exponent, mantissa) of band `b` (0 = LL, 1.. = HL/LH/HH of decomposition levels from the
    /// lowest) at decomposition level `nb` of `levels`.
    pub fn step(&self, band_index: usize, nb: u8, levels: u8) -> (u8, u16) {
        if self.style == 1 {
            let (e0, m0) = self.steps.first().copied().unwrap_or((0, 0));
            let e = e0 as i32 - levels as i32 + nb as i32;
            (e.clamp(0, 31) as u8, m0)
        } else {
            // Lenient: missing entries reuse the last one.
            self.steps
                .get(band_index)
                .or(self.steps.last())
                .copied()
                .unwrap_or((0, 0))
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Poc {
    pub rs: u8,
    pub cs: u16,
    pub lye: u16,
    pub re: u8,
    pub ce: u16,
    pub prog: Progression,
}

/// Marker-segment state of one header (main or tile).
#[derive(Clone, Debug, Default)]
pub(crate) struct Params {
    pub cod: Option<CodGlobal>,
    pub cod_comp: Option<CompCoding>,
    pub coc: Vec<Option<CompCoding>>,
    pub qcd: Option<Quant>,
    pub qcc: Vec<Option<Quant>>,
    pub rgn: Vec<Option<u8>>,
    pub poc: Vec<Poc>,
}

impl Params {
    fn new(nc: usize) -> Params {
        Params {
            coc: vec![None; nc],
            qcc: vec![None; nc],
            rgn: vec![None; nc],
            ..Default::default()
        }
    }
}

/// Fully resolved coding parameters of a tile.
#[derive(Clone, Debug)]
pub(crate) struct TileParams {
    pub cod: CodGlobal,
    pub comps: Vec<CompCoding>,
    pub quant: Vec<Quant>,
    pub roishift: Vec<u8>,
    pub poc: Vec<Poc>,
}

#[derive(Clone, Debug)]
pub(crate) struct TilePart {
    pub tile: u16,
    pub tpsot: u8,
    pub header: Range<usize>,
    pub data: Range<usize>,
    /// Index of this tile-part in codestream order (for PPM association).
    pub seq: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct Codestream<'a> {
    pub data: &'a [u8],
    pub siz: Siz,
    pub main: Params,
    /// PPM packet headers per tile-part (codestream order).
    pub ppm: Option<Vec<Vec<u8>>>,
    pub parts: Vec<TilePart>,
    /// Per tile: indices into `parts`, sorted by TPsot.
    pub tiles: Vec<Vec<usize>>,
    /// The codestream ended before its EOC / announced tile-parts.
    pub truncated: bool,
    pub comment: Option<String>,
    /// The CAP marker announces Part 15 (HTJ2K) capabilities.
    pub cap_ht: bool,
    /// Per component: the main header's non-linearity type (`Tnlt`; 0 for none).
    pub nlt: Vec<u8>,
    /// Per component: the lookup table of a type 4 non-linearity.
    pub nlt_lut: Vec<Option<NltLut>>,
}

/// A lookup-table non-linearity (`Tnlt` 4: OpenJPH's binary complement after a lookup table,
/// as OpenEXR's lossy LJ2K writes it): `Dmin` and `Dmax` (fractions of 2^32 - 1) bound the
/// table's domain, `points` its values (fractions of 2^`bit_depth`).
#[derive(Clone, Debug, PartialEq)]
pub struct NltLut {
    pub bit_depth: u8,
    pub signed: bool,
    pub d_min: u32,
    pub d_max: u32,
    pub points: Vec<u32>,
}

/// Parses the type parameters of an NLT segment of type 4 (`b` from `Cnlt` on): `Npoints`
/// (the table holds one more), `Dmin`, `Dmax`, the point precision, then the points (1, 2 or
/// 4 bytes each).
fn parse_nlt_lut(b: &[u8]) -> Option<NltLut> {
    let bd = *b.get(2)?;
    let n = u16::from_be_bytes([*b.get(4)?, *b.get(5)?]) as usize + 1;
    let rd32 = |p: usize| Some(u32::from_be_bytes(b.get(p..p + 4)?.try_into().ok()?));
    let (d_min, d_max) = (rd32(6)?, rd32(10)?);
    let pt = *b.get(14)?;
    if !(2..=8192).contains(&n) || d_min >= d_max || pt == 0 || pt > 32 || (bd & 0x7F) >= 32 {
        return None;
    }
    let bytes = if pt <= 8 {
        1
    } else if pt <= 16 {
        2
    } else {
        4
    };
    let raw = b.get(15..15 + n * bytes)?;
    let points = raw
        .chunks_exact(bytes)
        .map(|c| c.iter().fold(0u32, |a, &v| a << 8 | v as u32))
        .collect();
    Some(NltLut {
        bit_depth: (bd & 0x7F) + 1,
        signed: bd & 0x80 != 0,
        d_min,
        d_max,
        points,
    })
}

struct Reader<'a> {
    d: &'a [u8],
    p: usize,
}

impl<'a> Reader<'a> {
    fn u8(&mut self) -> Result<u8> {
        let v = *self
            .d
            .get(self.p)
            .ok_or(Jp2Error::Malformed("marker segment too short"))?;
        self.p += 1;
        Ok(v)
    }
    fn u16(&mut self) -> Result<u16> {
        Ok(((self.u8()? as u16) << 8) | self.u8()? as u16)
    }
    fn u32(&mut self) -> Result<u32> {
        Ok(((self.u16()? as u32) << 16) | self.u16()? as u32)
    }
    fn rest(&self) -> &'a [u8] {
        self.d.get(self.p..).unwrap_or(&[])
    }
    fn remaining(&self) -> usize {
        self.d.len().saturating_sub(self.p)
    }
}

/// Read one marker segment at `pos`: returns (marker, body range, next position).
fn segment(data: &[u8], pos: usize) -> Result<(u16, Range<usize>, usize)> {
    if pos + 2 > data.len() {
        return Err(Jp2Error::Truncated("codestream header"));
    }
    let m = u16::from_be_bytes([data[pos], data[pos + 1]]);
    if m >> 8 != 0xFF {
        return Err(Jp2Error::Malformed("expected a marker"));
    }
    if m == SOD || m == EOC || m == SOC || (0xFF30..=0xFF3F).contains(&m) {
        return Ok((m, pos + 2..pos + 2, pos + 2));
    }
    if pos + 4 > data.len() {
        return Err(Jp2Error::Truncated("codestream header"));
    }
    let len = u16::from_be_bytes([data[pos + 2], data[pos + 3]]) as usize;
    if len < 2 {
        return Err(Jp2Error::Malformed("bad marker segment length"));
    }
    let end = pos + 2 + len;
    if end > data.len() {
        return Err(Jp2Error::Truncated("marker segment"));
    }
    Ok((m, pos + 4..end, end))
}

fn parse_siz(b: &[u8]) -> Result<Siz> {
    let mut r = Reader { d: b, p: 0 };
    let rsiz = r.u16()?;
    let x1 = r.u32()?;
    let y1 = r.u32()?;
    let x0 = r.u32()?;
    let y0 = r.u32()?;
    let tw = r.u32()?;
    let th = r.u32()?;
    let tx0 = r.u32()?;
    let ty0 = r.u32()?;
    let nc = r.u16()? as usize;
    if nc == 0 || nc > MAX_COMPONENTS {
        return Err(Jp2Error::Malformed("bad component count"));
    }
    if x0 >= x1 || y0 >= y1 {
        return Err(Jp2Error::Malformed("empty image area"));
    }
    if tw == 0 || th == 0 || tx0 > x0 || ty0 > y0 {
        return Err(Jp2Error::Malformed("bad tile grid"));
    }
    if tx0 as u64 + tw as u64 <= x0 as u64 || ty0 as u64 + th as u64 <= y0 as u64 {
        return Err(Jp2Error::Malformed("first tile does not cover the image"));
    }
    let mut comps = Vec::with_capacity(nc);
    for _ in 0..nc {
        let s = r.u8()?;
        let dx = r.u8()?;
        let dy = r.u8()?;
        let prec = (s & 0x7F) + 1;
        if prec > 38 || dx == 0 || dy == 0 {
            return Err(Jp2Error::Malformed("bad component parameters"));
        }
        comps.push(CompSiz {
            prec,
            signed: s & 0x80 != 0,
            dx,
            dy,
        });
    }
    let tiles_x = (x1 - tx0).div_ceil(tw);
    let tiles_y = (y1 - ty0).div_ceil(th);
    if tiles_x as u64 * tiles_y as u64 > 65535 {
        return Err(Jp2Error::Malformed("more than 65535 tiles"));
    }
    Ok(Siz {
        rsiz,
        x1,
        y1,
        x0,
        y0,
        tw,
        th,
        tx0,
        ty0,
        comps,
        tiles_x,
        tiles_y,
    })
}

fn parse_spcod(r: &mut Reader, user_precincts: bool) -> Result<CompCoding> {
    let levels = r.u8()?;
    let cbw = r.u8()?.saturating_add(2);
    let cbh = r.u8()?.saturating_add(2);
    let style = r.u8()?;
    let transform = r.u8()?;
    if levels > 32 {
        return Err(Jp2Error::Malformed("more than 32 decomposition levels"));
    }
    if cbw > 10 || cbh > 10 || cbw + cbh > 12 {
        return Err(Jp2Error::Malformed("bad code-block size"));
    }
    // Bits 6..7: 0x40 HT code-blocks, 0xC0 mixed (decided per code-block); 0x80 alone is
    // reserved and read as Part 1.
    let style = if style & CBS_HT == 0 {
        style & !CBS_MIXED
    } else {
        style
    };
    if transform > 1 {
        return Err(Jp2Error::Unsupported("Part 2 wavelet kernel"));
    }
    let mut precincts = Vec::with_capacity(levels as usize + 1);
    for i in 0..=levels {
        if user_precincts {
            let b = r.u8()?;
            let (px, py) = (b & 0xF, b >> 4);
            if i > 0 && (px == 0 || py == 0) {
                return Err(Jp2Error::Malformed("zero precinct size above resolution 0"));
            }
            precincts.push((px, py));
        } else {
            precincts.push((15, 15));
        }
    }
    Ok(CompCoding {
        levels,
        cbw,
        cbh,
        style,
        reversible: transform == 1,
        precincts,
    })
}

fn comp_index(r: &mut Reader, nc: usize) -> Result<usize> {
    let c = if nc < 257 {
        r.u8()? as usize
    } else {
        r.u16()? as usize
    };
    if c >= nc {
        return Err(Jp2Error::Malformed("component index out of range"));
    }
    Ok(c)
}

fn parse_quant(r: &mut Reader) -> Result<Quant> {
    let s = r.u8()?;
    let style = s & 0x1F;
    let guard = s >> 5;
    let mut steps = Vec::new();
    match style {
        0 => {
            while r.remaining() > 0 {
                steps.push((r.u8()? >> 3, 0));
            }
        }
        1 | 2 => {
            while r.remaining() >= 2 {
                let v = r.u16()?;
                steps.push(((v >> 11) as u8, v & 0x7FF));
            }
        }
        _ => return Err(Jp2Error::Malformed("bad quantization style")),
    }
    if steps.is_empty() {
        return Err(Jp2Error::Malformed("empty quantization marker"));
    }
    Ok(Quant {
        style,
        guard,
        steps,
    })
}

/// Apply one header marker segment to `p`. Returns PPM/PPT data (Z index, bytes) when present.
fn apply_marker<'a>(
    m: u16,
    body: &'a [u8],
    p: &mut Params,
    nc: usize,
) -> Result<Option<(u8, &'a [u8])>> {
    let mut r = Reader { d: body, p: 0 };
    match m {
        COD => {
            let scod = r.u8()?;
            let prog = Progression::from_u8(r.u8()?)?;
            let layers = r.u16()?;
            let mct = r.u8()?;
            if layers == 0 {
                return Err(Jp2Error::Malformed("zero quality layers"));
            }
            p.cod = Some(CodGlobal {
                sop: scod & 2 != 0,
                eph: scod & 4 != 0,
                prog,
                layers,
                mct,
            });
            p.cod_comp = Some(parse_spcod(&mut r, scod & 1 != 0)?);
        }
        COC => {
            let c = comp_index(&mut r, nc)?;
            let s = r.u8()?;
            p.coc[c] = Some(parse_spcod(&mut r, s & 1 != 0)?);
        }
        QCD => p.qcd = Some(parse_quant(&mut r)?),
        QCC => {
            let c = comp_index(&mut r, nc)?;
            p.qcc[c] = Some(parse_quant(&mut r)?);
        }
        RGN => {
            let c = comp_index(&mut r, nc)?;
            let style = r.u8()?;
            let shift = r.u8()?;
            if style == 0 {
                p.rgn[c] = Some(shift);
            }
        }
        POC => {
            let wide = nc >= 257;
            let entry = if wide { 9 } else { 7 };
            while r.remaining() >= entry {
                let rs = r.u8()?;
                let cs = if wide { r.u16()? } else { r.u8()? as u16 };
                let lye = r.u16()?;
                let re = r.u8()?;
                let ce = if wide { r.u16()? } else { r.u8()? as u16 };
                let ce = if ce == 0 {
                    if wide { 16384 } else { 256 }
                } else {
                    ce
                };
                let prog = Progression::from_u8(r.u8()?)?;
                if p.poc.len() < MAX_POC {
                    p.poc.push(Poc {
                        rs,
                        cs,
                        lye,
                        re,
                        ce,
                        prog,
                    });
                }
            }
        }
        PPM | PPT => {
            let z = r.u8()?;
            return Ok(Some((z, r.rest())));
        }
        CAP => {}
        _ => {}
    }
    Ok(None)
}

pub(crate) fn parse(data: &[u8]) -> Result<Codestream<'_>> {
    if data.len() < 4 || u16::from_be_bytes([data[0], data[1]]) != SOC {
        return Err(Jp2Error::NotJp2);
    }
    let (m, body, mut pos) = segment(data, 2)?;
    if m != SIZ {
        return Err(Jp2Error::Malformed("SIZ is not the first marker"));
    }
    let siz = parse_siz(&data[body])?;
    let nc = siz.comps.len();
    let mut main = Params::new(nc);
    let mut ppm_chunks: Vec<(u8, &[u8])> = Vec::new();
    let mut comment = None;
    let mut cap_ht = false;
    let mut nlt = vec![0u8; nc];
    let mut nlt_lut: Vec<Option<NltLut>> = vec![None; nc];
    loop {
        let (m, body, next) = segment(data, pos)?;
        if m == SOT {
            break;
        }
        if m == EOC || m == SOD {
            return Err(Jp2Error::Malformed("codestream without tiles"));
        }
        let b = &data[body];
        if m == CAP && b.len() >= 4 {
            // Pcap bit 15 (counting from the MSB as bit 1): Part 15.
            cap_ht = u32::from_be_bytes([b[0], b[1], b[2], b[3]]) & 1 << (32 - 15) != 0;
        }
        if m == NLT && b.len() >= 4 {
            // Cnlt (0xFFFF: every component), BDnlt, Tnlt, then type parameters.
            let c = u16::from_be_bytes([b[0], b[1]]) as usize;
            // A type 4 segment that does not parse counts as an unknown type.
            let (t, lut) = match b[3] {
                4 => match parse_nlt_lut(b) {
                    Some(l) => (4, Some(l)),
                    None => (0xFF, None),
                },
                t => (t, None),
            };
            if c == 0xFFFF {
                nlt.fill(t);
                nlt_lut.fill(lut);
            } else if c < nc {
                nlt[c] = t;
                nlt_lut[c] = lut;
            }
        }
        if m == COM && comment.is_none() && b.len() >= 2 && b[1] == 1 {
            comment = Some(String::from_utf8_lossy(&b[2..]).into_owned());
        }
        if let Some((z, d)) = apply_marker(m, b, &mut main, nc)?
            && m == PPM
        {
            ppm_chunks.push((z, d));
        }
        pos = next;
    }
    if main.cod.is_none() || main.cod_comp.is_none() {
        return Err(Jp2Error::Malformed("missing COD marker"));
    }
    if main.qcd.is_none() && main.qcc.iter().any(|q| q.is_none()) {
        return Err(Jp2Error::Malformed("missing QCD marker"));
    }

    // Tile-part index.
    let mut parts = Vec::new();
    let mut truncated = false;
    let ntiles = siz.num_tiles();
    while pos + 2 <= data.len() && parts.len() < MAX_TILE_PARTS {
        let m = u16::from_be_bytes([data[pos], data[pos + 1]]);
        if m == EOC {
            break;
        }
        if m != SOT {
            // Garbage after the last tile-part: tolerate.
            break;
        }
        let sot = pos;
        let (_, body, mut hp) = match segment(data, pos) {
            Ok(s) => s,
            Err(_) => {
                truncated = true;
                break;
            }
        };
        let mut r = Reader {
            d: &data[body],
            p: 0,
        };
        let (Ok(isot), Ok(psot), Ok(tpsot), Ok(_tnsot)) = (r.u16(), r.u32(), r.u8(), r.u8()) else {
            return Err(Jp2Error::Malformed("short SOT"));
        };
        let end = if psot == 0 {
            // Last tile-part: runs to EOC.
            let mut e = data.len();
            if e >= 2 && data[e - 2] == 0xFF && data[e - 1] == 0xD9 {
                e -= 2;
            }
            e
        } else {
            let e = sot as u64 + psot as u64;
            if e > data.len() as u64 {
                truncated = true;
                data.len()
            } else {
                e as usize
            }
        };
        let hstart = hp;
        let mut ok = false;
        while hp + 2 <= end {
            let Ok((m, _, next)) = segment(&data[..end], hp) else {
                break;
            };
            if m == SOD {
                ok = true;
                break;
            }
            hp = next;
        }
        if !ok {
            truncated = true;
            break;
        }
        let header = hstart..hp;
        let dstart = hp + 2;
        if (isot as usize) < ntiles && dstart <= end {
            parts.push(TilePart {
                tile: isot,
                tpsot,
                header,
                data: dstart..end,
                seq: parts.len(),
            });
        }
        if end <= sot {
            break;
        }
        pos = end;
        if psot == 0 {
            break;
        }
    }
    if parts.is_empty() {
        return Err(Jp2Error::Truncated("no tile-parts"));
    }
    let mut tiles: Vec<Vec<usize>> = vec![Vec::new(); ntiles];
    for (i, p) in parts.iter().enumerate() {
        tiles[p.tile as usize].push(i);
    }
    for t in &mut tiles {
        t.sort_by_key(|&i| (parts[i].tpsot, i));
    }

    let ppm = if ppm_chunks.is_empty() {
        None
    } else {
        ppm_chunks.sort_by_key(|c| c.0);
        let all: Vec<u8> = ppm_chunks
            .iter()
            .flat_map(|c| c.1.iter().copied())
            .collect();
        let mut v = Vec::new();
        let mut p = 0usize;
        while p + 4 <= all.len() && v.len() < parts.len() {
            let n = u32::from_be_bytes([all[p], all[p + 1], all[p + 2], all[p + 3]]) as usize;
            p += 4;
            let e = (p + n).min(all.len());
            v.push(all[p..e].to_vec());
            p = e;
        }
        Some(v)
    };

    Ok(Codestream {
        data,
        siz,
        main,
        ppm,
        parts,
        tiles,
        truncated,
        comment,
        cap_ht,
        nlt,
        nlt_lut,
    })
}

impl Codestream<'_> {
    /// Main-header coding of component `c` (COC over COD).
    pub fn main_comp(&self, c: usize) -> &CompCoding {
        self.main.coc[c]
            .as_ref()
            .or(self.main.cod_comp.as_ref())
            .expect("COD checked in parse")
    }

    /// Resolve the tile's coding parameters (tile-part headers over main header) and collect
    /// its PPT packet headers.
    pub fn tile_params(&self, t: usize) -> Result<(TileParams, Option<Vec<u8>>)> {
        let nc = self.siz.comps.len();
        let mut tp = Params::new(nc);
        let mut ppt: Vec<(usize, u8, &[u8])> = Vec::new();
        for (k, &pi) in self.tiles[t].iter().enumerate() {
            let part = &self.parts[pi];
            let mut pos = part.header.start;
            while pos < part.header.end {
                let (m, body, next) = segment(&self.data[..part.header.end], pos)?;
                if let Some((z, d)) = apply_marker(m, &self.data[body], &mut tp, nc)?
                    && m == PPT
                {
                    ppt.push((k, z, d));
                }
                pos = next;
            }
        }
        let cod = tp.cod.or(self.main.cod).expect("COD checked in parse");
        let mut comps = Vec::with_capacity(nc);
        let mut quant = Vec::with_capacity(nc);
        let mut roishift = Vec::with_capacity(nc);
        for c in 0..nc {
            let cc = tp.coc[c]
                .as_ref()
                .or(tp.cod_comp.as_ref())
                .or(self.main.coc[c].as_ref())
                .or(self.main.cod_comp.as_ref())
                .expect("COD checked in parse");
            comps.push(cc.clone());
            let q = tp.qcc[c]
                .as_ref()
                .or(tp.qcd.as_ref())
                .or(self.main.qcc[c].as_ref())
                .or(self.main.qcd.as_ref())
                .ok_or(Jp2Error::Malformed("missing quantization"))?;
            quant.push(q.clone());
            roishift.push(tp.rgn[c].or(self.main.rgn[c]).unwrap_or(0));
        }
        let poc = if !tp.poc.is_empty() {
            tp.poc
        } else {
            self.main.poc.clone()
        };
        let ppt_data = if ppt.is_empty() {
            None
        } else {
            ppt.sort_by_key(|p| (p.0, p.1));
            Some(ppt.iter().flat_map(|p| p.2.iter().copied()).collect())
        };
        Ok((
            TileParams {
                cod,
                comps,
                quant,
                roishift,
                poc,
            },
            ppt_data,
        ))
    }
}

// Silence unused-constant lints for markers we only skip.
#[allow(dead_code)]
const SKIPPED: [u16; 5] = [TLM, PLM, PLT, CRG, COM];

#[cfg(test)]
mod nlt_tests {
    use super::*;

    fn segment(points: &[u16], d: (u32, u32), pt: u8) -> Vec<u8> {
        let mut b = vec![0xFF, 0xFF, 0x8F, 4];
        b.extend(((points.len() - 1) as u16).to_be_bytes());
        b.extend(d.0.to_be_bytes());
        b.extend(d.1.to_be_bytes());
        b.push(pt);
        for p in points {
            b.extend(p.to_be_bytes());
        }
        b
    }

    #[test]
    fn type4_lookup_tables_parse_and_bad_ones_are_refused() {
        let l = parse_nlt_lut(&segment(&[0, 30000, 65535], (0, u32::MAX), 16)).unwrap();
        assert_eq!((l.bit_depth, l.signed), (16, true));
        assert_eq!(l.points, [0, 30000, 65535]);
        // Dmin must be below Dmax, the precision 1..=32, the points all present.
        assert!(parse_nlt_lut(&segment(&[0, 1], (5, 5), 16)).is_none());
        assert!(parse_nlt_lut(&segment(&[0, 1], (0, 9), 0)).is_none());
        let mut cut = segment(&[0, 1, 2], (0, 9), 16);
        cut.pop();
        assert!(parse_nlt_lut(&cut).is_none());
    }
}
