//! The codestream header (ISO/IEC 21122-1 Annex A): the markers from SOC to the first slice
//! header.

use crate::{Error, Result};

pub(crate) const SOC: u16 = 0xFF10;
pub(crate) const EOC: u16 = 0xFF11;
const PIH: u16 = 0xFF12;
const CDT: u16 = 0xFF13;
const WGT: u16 = 0xFF14;
const COM: u16 = 0xFF15;
const NLT: u16 = 0xFF16;
const CWD: u16 = 0xFF17;
const CTS: u16 = 0xFF18;
const CRG: u16 = 0xFF19;
const TPC: u16 = 0xFF1A;
const WGR: u16 = 0xFF1B;
pub(crate) const SLH: u16 = 0xFF20;
pub(crate) const SLI: u16 = 0xFF21;
const CAP: u16 = 0xFF50;

/// Largest component count accepted.
pub const MAX_COMPONENTS: usize = 4;
/// Largest horizontal and vertical decomposition depths accepted.
pub const MAX_NLX: u8 = 8;
pub const MAX_NLY: u8 = 6;

/// Capability bits of the CAP marker.
pub mod cap {
    pub const STAR_TETRIX: u16 = 0x4000;
    pub const NLT_QUADRATIC: u16 = 0x2000;
    pub const NLT_EXTENDED: u16 = 0x1000;
    pub const VERTICAL_SUBSAMPLING: u16 = 0x0800;
    pub const DECOMPOSITION_SUPPRESSION: u16 = 0x0400;
    pub const LOSSLESS: u16 = 0x0200;
    pub const RAW_PER_PACKET: u16 = 0x0080;
    pub const FRAME_BUFFER: u16 = 0x0040;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Component {
    pub depth: u8,
    pub sx: u8,
    pub sy: u8,
    /// Its own horizontal and vertical decomposition depths, from a component table with
    /// three bytes per component (intoPIX TicoRAW, as in Nikon High Efficiency raw files);
    /// `None` for the picture header's depths. (0, 0) is a component without a wavelet
    /// transform.
    pub levels: Option<(u8, u8)>,
}

/// The non-linear transform of the NLT marker.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Nlt {
    Linear,
    Quadratic { sigma: bool, alpha: u16 },
    Extended { t1: u32, t2: u32, e: u8 },
}

/// The colour filter array layout named by the CRG marker.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cfa {
    Rggb,
    Bggr,
    Grbg,
    Gbrg,
}

impl Cfa {
    pub fn name(self) -> &'static str {
        match self {
            Cfa::Rggb => "RGGB",
            Cfa::Bggr => "BGGR",
            Cfa::Grbg => "GRBG",
            Cfa::Gbrg => "GBRG",
        }
    }

    /// The colour (0 red, 1 green, 2 blue) at mosaic column `x`, row `y`.
    pub fn at(self, x: usize, y: usize) -> usize {
        let p: [usize; 4] = match self {
            Cfa::Rggb => [0, 1, 1, 2],
            Cfa::Bggr => [2, 1, 1, 0],
            Cfa::Grbg => [1, 0, 2, 1],
            Cfa::Gbrg => [1, 2, 0, 1],
        };
        p[(x & 1) + 2 * (y & 1)]
    }

    /// Whether the greens sit on the anti-diagonal of each 2×2 cell (RGGB, BGGR).
    pub(crate) fn greens_anti_diagonal(self) -> bool {
        matches!(self, Cfa::Rggb | Cfa::Bggr)
    }
}

/// The Star-Tetrix parameters of the CTS marker.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tetrix {
    /// Only the two lines of each sensor row pair are used (in-line transform, `Cf` = 3).
    pub inline: bool,
    pub e1: u8,
    pub e2: u8,
}

/// Temporal prediction parameters of the TPC marker (TDC profiles).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tpc {
    /// Code groups per TDC selection group.
    pub si: u8,
    pub qbi: i8,
    pub qbr: i8,
    /// Per band: refresh position and the exponent of its period (0: no refresh).
    pub yh: Vec<u8>,
    pub sh: Vec<u8>,
}

impl Default for Tpc {
    fn default() -> Self {
        Tpc {
            si: 8,
            qbi: 0,
            qbr: 0,
            yh: Vec::new(),
            sh: Vec::new(),
        }
    }
}

/// Everything the header markers say.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Header {
    pub cap: u16,
    /// Codestream length in bytes (0: not given).
    pub lcod: u32,
    pub profile: u16,
    pub plev: u16,
    pub width: u32,
    pub height: u32,
    /// Precinct column width in multiples of 8 × 2^NLx samples (0: full width).
    pub cw: u16,
    /// Slice height in lines.
    pub slice_height: u32,
    pub ng: u8,
    pub ss: u8,
    pub bw: u8,
    pub fq: u8,
    pub br: u8,
    pub fslc: u8,
    pub ppoc: u8,
    pub cpih: u8,
    pub nlx: u8,
    pub nly: u8,
    pub lh: bool,
    pub rl: bool,
    pub qpih: u8,
    pub fs: u8,
    pub rm: u8,
    pub components: Vec<Component>,
    pub gains: Vec<u8>,
    pub priorities: Vec<u8>,
    pub nlt: Nlt,
    /// Components (the last ones) without a wavelet decomposition.
    pub sd: u8,
    pub tetrix: Option<Tetrix>,
    pub cfa: Option<Cfa>,
    /// Temporal differential coding (TDC profiles): the TPC marker's parameters (defaults
    /// when it is absent).
    pub tdc: Option<Tpc>,
    /// The refresh weights table (WGR marker).
    pub refresh_gains: Vec<u8>,
    pub refresh_priorities: Vec<u8>,
    /// The slices are SLI slices (temporal prediction).
    pub sli: bool,
    /// COM markers: their type and payload.
    pub comments: Vec<(u16, Vec<u8>)>,
    /// Byte offset of the first slice header.
    pub first_slice: usize,
    /// Bytes after the standard picture header fields (13 in intoPIX TicoRAW codestreams).
    pub pih_extra: Vec<u8>,
}

impl Header {
    pub fn depth(&self) -> u8 {
        self.components.first().map(|c| c.depth).unwrap_or(0)
    }

    /// An intoPIX TicoRAW codestream (Nikon High Efficiency raw): a picture header with
    /// 13 more bytes and a component table giving each component its own wavelet depths.
    pub fn ticoraw(&self) -> bool {
        self.pih_extra.len() == 13 && self.components.iter().all(|c| c.levels.is_some())
    }

    /// The full-resolution component grid: the picture size, or for TicoRAW (whose picture
    /// size is the colour filter mosaic and whose four components are its 2×2 cells) half
    /// of it each way.
    pub fn component_grid(&self) -> (usize, usize) {
        let (w, h) = (self.width as usize, self.height as usize);
        if self.ticoraw() {
            (w / 2, h / 2)
        } else {
            (w, h)
        }
    }

    /// Whether the precinct headers use the long field sizes.
    pub(crate) fn long_headers(&self) -> bool {
        // TicoRAW always uses the long packet headers, whatever the width.
        let short = (self.width as u64) * (self.components.len() as u64) < 32752 && !self.lh;
        !short || self.ticoraw()
    }
}

fn be16(d: &[u8], at: usize) -> Result<u16> {
    d.get(at..at + 2)
        .map(|s| u16::from_be_bytes([s[0], s[1]]))
        .ok_or_else(|| Error::Truncated("the codestream header ends early".into()))
}

fn be32(d: &[u8], at: usize) -> Result<u32> {
    d.get(at..at + 4)
        .map(|s| u32::from_be_bytes([s[0], s[1], s[2], s[3]]))
        .ok_or_else(|| Error::Truncated("the codestream header ends early".into()))
}

fn bad(s: impl Into<String>) -> Error {
    Error::Malformed(s.into())
}

/// Parses the markers of a codestream up to its first slice header.
pub fn parse(d: &[u8]) -> Result<Header> {
    if be16(d, 0).ok() != Some(SOC) {
        return Err(Error::NotRecognised);
    }
    let mut h = Header {
        cap: 0,
        lcod: 0,
        profile: 0,
        plev: 0,
        width: 0,
        height: 0,
        cw: 0,
        slice_height: 0,
        ng: 4,
        ss: 8,
        bw: 20,
        fq: 8,
        br: 4,
        fslc: 0,
        ppoc: 0,
        cpih: 0,
        nlx: 0,
        nly: 0,
        lh: false,
        rl: false,
        qpih: 0,
        fs: 0,
        rm: 0,
        components: Vec::new(),
        gains: Vec::new(),
        priorities: Vec::new(),
        nlt: Nlt::Linear,
        sd: 0,
        tetrix: None,
        cfa: None,
        tdc: None,
        refresh_gains: Vec::new(),
        refresh_priorities: Vec::new(),
        sli: false,
        comments: Vec::new(),
        first_slice: 0,
        pih_extra: Vec::new(),
    };
    let mut seen_pih = false;
    let mut seen_cap = false;
    let mut crg: Option<Vec<(u16, u16)>> = None;
    let mut tpc: Option<Tpc> = None;
    let mut at = 2;
    loop {
        let m = be16(d, at)?;
        if m == SLH || m == SLI {
            h.sli = m == SLI;
            h.first_slice = at;
            break;
        }
        if m == EOC {
            return Err(bad("the codestream ends before any slice"));
        }
        if m & 0xFF00 != 0xFF00 || m == 0xFFFF {
            return Err(bad(format!(
                "unexpected bytes {m:#06x} among the header markers"
            )));
        }
        let len = be16(d, at + 2)? as usize;
        if len < 2 {
            return Err(bad(format!("marker {m:#06x} has length {len}")));
        }
        let body = d
            .get(at + 4..at + 2 + len)
            .ok_or_else(|| Error::Truncated("the codestream header ends early".into()))?;
        match m {
            CAP => {
                seen_cap = true;
                h.cap = match body.len() {
                    0 => 0,
                    1 => u16::from(body[0]) << 8,
                    _ => u16::from_be_bytes([body[0], body[1]]),
                };
            }
            PIH => {
                // TicoRAW (Nikon High Efficiency) appends 13 bytes of its own.
                if body.len() != 24 && body.len() != 37 {
                    return Err(bad(format!("picture header of {} bytes", body.len())));
                }
                h.pih_extra = body[24..].to_vec();
                seen_pih = true;
                h.lcod = be32(body, 0)?;
                h.profile = be16(body, 4)?;
                h.plev = be16(body, 6)?;
                h.width = u32::from(be16(body, 8)?);
                h.height = u32::from(be16(body, 10)?);
                h.cw = be16(body, 12)?;
                let hsl = u32::from(be16(body, 14)?);
                let nc = body[16] as usize;
                h.ng = body[17];
                h.ss = body[18];
                h.bw = body[19];
                h.fq = body[20] >> 4;
                h.br = body[20] & 15;
                h.fslc = body[21] >> 7;
                h.ppoc = (body[21] >> 4) & 7;
                h.cpih = body[21] & 15;
                h.nlx = body[22] >> 4;
                h.nly = body[22] & 15;
                h.lh = body[23] & 0x80 != 0;
                h.rl = body[23] & 0x40 != 0;
                h.qpih = (body[23] >> 4) & 3;
                h.fs = (body[23] >> 2) & 3;
                h.rm = body[23] & 3;
                h.slice_height = hsl << h.nly.min(15);
                if nc == 0 {
                    return Err(bad("no components"));
                }
                if nc > MAX_COMPONENTS {
                    return Err(Error::Unsupported(format!(
                        "JPEG XS images with {nc} components are not supported (at most {MAX_COMPONENTS})"
                    )));
                }
                h.components = vec![
                    Component {
                        depth: 0,
                        sx: 1,
                        sy: 1,
                        levels: None,
                    };
                    nc
                ];
            }
            CDT => {
                let nc = h.components.len();
                // Two bytes per component; TicoRAW adds a third, the component's own
                // decomposition depths (NLx in the high nibble, NLy in the low one).
                let per = match body.len() {
                    n if seen_pih && n == 2 * nc => 2,
                    n if seen_pih && n == 3 * nc => 3,
                    _ => return Err(bad("component table does not match the picture header")),
                };
                for (c, b) in h.components.iter_mut().zip(body.chunks_exact(per)) {
                    c.depth = b[0];
                    c.sx = b[1] >> 4;
                    c.sy = b[1] & 15;
                    if per == 3 {
                        c.levels = Some((b[2] >> 4, b[2] & 15));
                    }
                    if !matches!(c.sx, 1 | 2) || !matches!(c.sy, 1 | 2) {
                        return Err(Error::Unsupported(format!(
                            "a component sampled 1:{} horizontally and 1:{} vertically",
                            c.sx, c.sy
                        )));
                    }
                }
            }
            WGT => {
                if body.is_empty() || body.len() % 2 != 0 {
                    return Err(bad("weights table of odd length"));
                }
                h.gains = body.iter().step_by(2).copied().collect();
                h.priorities = body.iter().skip(1).step_by(2).copied().collect();
            }
            WGR => {
                if body.is_empty() || body.len() % 2 != 0 {
                    return Err(bad("refresh weights table of odd length"));
                }
                h.refresh_gains = body.iter().step_by(2).copied().collect();
                h.refresh_priorities = body.iter().skip(1).step_by(2).copied().collect();
            }
            TPC => {
                if body.len() < 2 || body.len() % 2 != 0 {
                    return Err(bad("TPC marker of the wrong length"));
                }
                let sa = |v: u8| -> i8 { (v & 7) as i8 - if v & 8 != 0 { 8 } else { 0 } };
                let mut t = Tpc {
                    si: body[0],
                    qbi: sa(body[1] >> 4),
                    qbr: sa(body[1] & 15),
                    yh: Vec::new(),
                    sh: Vec::new(),
                };
                for p in body[2..].chunks_exact(2) {
                    if p[1] == 0 || p[1] > 7 {
                        return Err(bad("TPC refresh period out of range"));
                    }
                    t.yh.push(p[0]);
                    t.sh.push(p[1]);
                }
                tpc = Some(t);
            }
            COM => {
                if body.len() < 2 {
                    return Err(bad("comment marker too short"));
                }
                h.comments
                    .push((u16::from_be_bytes([body[0], body[1]]), body[2..].to_vec()));
            }
            NLT => match body.first() {
                Some(1) if body.len() == 3 => {
                    let v = u16::from_be_bytes([body[1], body[2]]);
                    h.nlt = Nlt::Quadratic {
                        sigma: v & 0x8000 != 0,
                        alpha: v & 0x7FFF,
                    };
                }
                Some(2) if body.len() == 10 => {
                    h.nlt = Nlt::Extended {
                        t1: be32(body, 1)?,
                        t2: be32(body, 5)?,
                        e: body[9],
                    };
                }
                Some(t) => {
                    return Err(Error::Unsupported(format!(
                        "non-linear transform type {t} is not supported"
                    )));
                }
                None => return Err(bad("empty NLT marker")),
            },
            CWD => {
                if body.len() != 1 {
                    return Err(bad("CWD marker of the wrong length"));
                }
                h.sd = body[0];
            }
            CTS => {
                if body.len() != 2 {
                    return Err(bad("CTS marker of the wrong length"));
                }
                let cf = body[0] & 15;
                if cf != 0 && cf != 3 {
                    return Err(Error::Unsupported(format!(
                        "Star-Tetrix transform type {cf} is not supported"
                    )));
                }
                h.tetrix = Some(Tetrix {
                    inline: cf == 3,
                    e1: body[1] >> 4,
                    e2: body[1] & 15,
                });
            }
            CRG => {
                if body.len() % 4 != 0 {
                    return Err(bad("CRG marker of the wrong length"));
                }
                crg = Some(
                    body.chunks_exact(4)
                        .map(|c| {
                            (
                                u16::from_be_bytes([c[0], c[1]]),
                                u16::from_be_bytes([c[2], c[3]]),
                            )
                        })
                        .collect(),
                );
            }
            // Markers this decoder does not know are skipped (their length is given).
            _ => {}
        }
        at += 2 + len;
    }
    if !seen_cap {
        return Err(bad("no CAP marker"));
    }
    if !seen_pih {
        return Err(bad("no picture header"));
    }
    if h.components.iter().any(|c| c.depth == 0) {
        return Err(bad("no component table"));
    }
    if let Some(v) = crg {
        h.cfa = Some(cfa_of(&v)?);
    }
    if matches!(h.profile, 0x4A45 | 0x6A45) || h.sli {
        h.tdc = Some(tpc.unwrap_or_default());
    }
    Ok(h)
}

/// The colour filter pattern of a CRG table (four components, each at a half-sample offset).
fn cfa_of(v: &[(u16, u16)]) -> Result<Cfa> {
    const H: u16 = 32768;
    let t: [(Cfa, [(u16, u16); 4]); 4] = [
        (Cfa::Rggb, [(0, 0), (H, 0), (0, H), (H, H)]),
        (Cfa::Bggr, [(H, H), (H, 0), (0, H), (0, 0)]),
        (Cfa::Grbg, [(H, 0), (0, 0), (H, H), (0, H)]),
        (Cfa::Gbrg, [(0, H), (0, 0), (H, H), (H, 0)]),
    ];
    t.iter()
        .find(|(_, p)| v == p)
        .map(|(c, _)| *c)
        .ok_or_else(|| {
            Error::Unsupported("this colour filter layout (CRG marker) is not supported".into())
        })
}

/// Profile name (ISO/IEC 21122-2), or `None` for an unknown code.
pub fn profile_name(p: u16) -> Option<&'static str> {
    Some(match p {
        0x0000 => "Unrestricted",
        0x1500 => "Light 422.10",
        0x1A00 => "Light 444.12",
        0x2500 => "Light-Subline 422.10",
        0x3240 => "Main 420.12",
        0x3540 => "Main 422.10",
        0x3A40 => "Main 444.12",
        0x3E40 => "Main 4444.12",
        0x4240 => "High 420.12",
        0x4A40 => "High 444.12",
        0x4E40 => "High 4444.12",
        0x6EC0 => "MLS.12",
        0x6ED0 => "MLS.16",
        0x9300 => "Light Bayer",
        0xB340 => "Main Bayer",
        0xC340 => "High Bayer",
        0x4A44 => "CHigh 444.12",
        0x4A45 => "TDC 444.12",
        0x6A45 => "TDC MLS 444.12",
        _ => return None,
    })
}

/// Level and sublevel names of a `Plev` value.
pub fn level_name(plev: u16) -> (String, String) {
    let level = match plev & 0x7C00 {
        0x0000 => "Unrestricted".to_owned(),
        0x0400 => "1k-1".to_owned(),
        0x1000 => "2k-1".to_owned(),
        0x2000 => "4k-1".to_owned(),
        0x2400 => "4k-2".to_owned(),
        0x2800 => "4k-3".to_owned(),
        0x3000 => "8k-1".to_owned(),
        0x3400 => "8k-2".to_owned(),
        0x3800 => "8k-3".to_owned(),
        0x4000 => "10k-1".to_owned(),
        v => format!("{v:#06x}"),
    };
    let sub = match plev & 0x009F {
        0x0000 => "Unrestricted".to_owned(),
        0x0080 => "Full".to_owned(),
        0x0010 => "12 bpp".to_owned(),
        0x000C => "9 bpp".to_owned(),
        0x0008 => "6 bpp".to_owned(),
        0x0006 => "4 bpp".to_owned(),
        0x0004 => "3 bpp".to_owned(),
        0x0003 => "2 bpp".to_owned(),
        v => format!("{v:#06x}"),
    };
    (level, sub)
}
