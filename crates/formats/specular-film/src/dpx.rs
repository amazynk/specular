//! SMPTE 268M Digital Picture Exchange, versions 1.0 and 2.0, in either byte order.
//!
//! Samples: 1, 8, 10, 12 and 16-bit integers (packed, filled method A or B; unsigned, or
//! signed and shown like floats from −1 to 1) and 32 or 64-bit floats; descriptors R, G, B,
//! A, luma, depth and composite video (not demodulated) as grey, RGB, RGBA, ABGR, CbYCrY
//! 4:2:2, CbYACrYA 4:2:2:4, CbYCr 4:4:4, CbYCrA and user-defined 2–8 channels (the first
//! three as RGB); run-length encoding, floats included.
//!
//! Pages: single-channel R, G and B elements (with an A element when there is one), luma and
//! colour-difference elements, or an RGB and an A element, of one depth and sign, are combined
//! into one colour page, shown first; every other element is a page of its own. See the crate
//! documentation for how transfer characteristics are shown.

use crate::log::{self, LogParams};
use crate::unpack::{Layout, Rows, Shape, for_rows};
use crate::{
    Error, Image, Info, Model, Pixels, Reader, Result, check_dims, check_payload, exif_orientation,
    num, orientation_name, tidy_date,
};

const ELEMENTS: usize = 780;
const ELEMENT_SIZE: usize = 72;

/// "SDPX" (big-endian) or "XPDS" (little-endian).
pub fn probe(head: &[u8]) -> bool {
    head.starts_with(b"SDPX") || head.starts_with(b"XPDS")
}

#[derive(Clone, Debug)]
struct Element {
    sign: u32,
    low_code: Option<u32>,
    low_q: Option<f32>,
    high_code: Option<u32>,
    high_q: Option<f32>,
    descriptor: u8,
    transfer: u8,
    colorimetric: u8,
    bits: u8,
    packing: u16,
    encoding: u16,
    offset: Option<u32>,
    eol: Option<u32>,
    eoi: Option<u32>,
    description: Option<String>,
}

struct Header<'a> {
    r: Reader<'a>,
    data_offset: u32,
    orientation: u32,
    width: u32,
    height: u32,
    elements: Vec<Element>,
    /// The film and television headers are present.
    industry: bool,
}

fn header(data: &[u8]) -> Result<Header<'_>> {
    if !probe(data) {
        return Err(Error::NotRecognised);
    }
    if data.len() < 1664 {
        return Err(Error::Truncated("DPX header".into()));
    }
    let r = Reader {
        d: data,
        be: data.starts_with(b"SDPX"),
    };
    let data_offset = r.u32(4).unwrap_or(0);
    let orientation = r.u16(768).unwrap_or(0) as u32;
    let count = r.u16(770).unwrap_or(0);
    let width = r.u32(772).unwrap_or(0);
    let height = r.u32(776).unwrap_or(0);
    let count = match count {
        1..=8 => count as usize,
        // Some writers leave it unset with one element described.
        0 | 0xffff => 1,
        _ => return Err(Error::Malformed(format!("{count} image elements"))),
    };
    let elements = (0..count)
        .map(|i| {
            let o = ELEMENTS + i * ELEMENT_SIZE;
            Element {
                sign: r.u32(o).unwrap_or(0),
                low_code: r.u32_set(o + 4),
                low_q: r.f32_set(o + 8),
                high_code: r.u32_set(o + 12),
                high_q: r.f32_set(o + 16),
                descriptor: r.u8(o + 20).unwrap_or(0),
                transfer: r.u8(o + 21).unwrap_or(0),
                colorimetric: r.u8(o + 22).unwrap_or(0),
                bits: r.u8(o + 23).unwrap_or(0),
                packing: r.u16(o + 24).unwrap_or(0),
                encoding: r.u16(o + 26).unwrap_or(0),
                offset: r.u32_set(o + 28),
                eol: r.u32_set(o + 32),
                eoi: r.u32_set(o + 36),
                description: r.str(o + 40, 32),
            }
        })
        .collect();
    let industry = data_offset as usize >= 2048 && data.len() >= 2048;
    Ok(Header {
        r,
        data_offset,
        orientation,
        width,
        height,
        elements,
        industry,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Desc {
    /// One channel shown as grey: R, G, B, A, luma, depth, composite video.
    Single,
    Rgb,
    Rgba,
    Abgr,
    /// CbYCrY.
    Ycc422,
    /// CbYACrYA.
    Ycca422,
    Ycc444,
    Ycca444,
    /// Colour difference alone: Cb Cr per pixel pair (only shown with a luma element).
    CbCr,
    /// User-defined, 2 to 8 components per pixel (descriptors 150–156).
    User(u8),
}

impl Desc {
    fn of(code: u8) -> Result<Desc> {
        Ok(match code {
            1..=4 | 6 | 8 | 9 => Desc::Single,
            7 => Desc::CbCr,
            50 => Desc::Rgb,
            51 => Desc::Rgba,
            52 => Desc::Abgr,
            100 => Desc::Ycc422,
            101 => Desc::Ycca422,
            102 => Desc::Ycc444,
            103 => Desc::Ycca444,
            150..=156 => Desc::User(code - 148),
            0 => {
                return Err(Error::Unsupported(
                    "DPX image elements with a user-defined layout (descriptor 0) are not supported: the file does not say how many channels each pixel has".into(),
                ));
            }
            5 => {
                return Err(Error::Unsupported(
                    "DPX image elements of the reserved descriptor 5 are not supported".into(),
                ));
            }
            c => {
                return Err(Error::Unsupported(format!(
                    "DPX image elements with descriptor {c} are not supported"
                )));
            }
        })
    }

    /// Samples per pixel (per pixel pair for 4:2:2 and colour difference).
    fn comps(self) -> usize {
        match self {
            Desc::Single => 1,
            Desc::Rgb | Desc::Ycc444 => 3,
            Desc::Rgba | Desc::Abgr | Desc::Ycca444 => 4,
            // A 4:2:2 pixel pair.
            Desc::Ycc422 => 4,
            Desc::Ycca422 => 6,
            Desc::CbCr => 2,
            Desc::User(n) => n as usize,
        }
    }

    /// Samples in one row of `w` pixels.
    fn samples(self, w: usize) -> usize {
        match self {
            Desc::Ycc422 | Desc::Ycca422 | Desc::CbCr => w.div_ceil(2) * self.comps(),
            _ => w * self.comps(),
        }
    }

    fn alpha(self) -> bool {
        matches!(
            self,
            Desc::Rgba | Desc::Abgr | Desc::Ycca422 | Desc::Ycca444
        )
    }

    fn ycc(self) -> bool {
        matches!(
            self,
            Desc::Ycc422 | Desc::Ycca422 | Desc::Ycc444 | Desc::Ycca444
        )
    }
}

fn descriptor_name(c: u8) -> String {
    match c {
        0 => "user-defined".into(),
        1 => "red".into(),
        2 => "green".into(),
        3 => "blue".into(),
        4 => "alpha".into(),
        6 => "luma".into(),
        7 => "colour difference".into(),
        8 => "depth".into(),
        9 => "composite video".into(),
        50 => "RGB".into(),
        51 => "RGBA".into(),
        52 => "ABGR".into(),
        100 => "CbYCrY 4:2:2".into(),
        101 => "CbYACrYA 4:2:2:4".into(),
        102 => "CbYCr 4:4:4".into(),
        103 => "CbYCrA 4:4:4:4".into(),
        150..=156 => format!("user-defined {} channels", c - 148),
        c => format!("descriptor {c}"),
    }
}

/// Names of the transfer characteristic and colorimetric codes.
fn transfer_name(c: u8) -> Option<&'static str> {
    Some(match c {
        0 => "user-defined",
        1 => "printing density",
        2 => "linear",
        3 => "logarithmic",
        4 => "unspecified video",
        5 => "SMPTE 274M",
        6 => "ITU-R BT.709",
        7 => "ITU-R BT.601 (625 lines)",
        8 => "ITU-R BT.601 (525 lines)",
        9 => "NTSC composite video",
        10 => "PAL composite video",
        11 => "Z depth, linear",
        12 => "Z depth, homogeneous",
        13 => "SMPTE ADX",
        14 => "ITU-R BT.2020 NCL",
        15 => "ITU-R BT.2020 CL",
        16 => "IEC 61966-2-4 xvYCC",
        17 => "ITU-R BT.2100 PQ",
        18 => "ITU-R BT.2100 HLG",
        _ => return None,
    })
}

fn packing_name(bits: u8, p: u16) -> Option<&'static str> {
    if matches!(bits, 8 | 16 | 32 | 64) {
        return None;
    }
    Some(match p {
        0 => "packed",
        1 => "filled (method A)",
        2 => "filled (method B)",
        _ => "unknown packing",
    })
}

/// How the element's code values are shown.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Look {
    Log(LogParams),
    Linear,
    Display,
}

struct Plan {
    desc: Desc,
    layout: Layout,
    look: Look,
    /// Reference low and high code values in use.
    lo: f32,
    hi: f32,
    max: f32,
    bt709: bool,
    /// Two's-complement integer samples (shown like floats, −1 to 1).
    signed: bool,
}

fn plan(h: &Header, e: &Element) -> Result<Plan> {
    plan_as(h, e, Desc::of(e.descriptor)?)
}

/// The plan of element `e` shown with layout `desc` (its own, or that of a page combining
/// it with other elements).
fn plan_as(h: &Header, e: &Element, desc: Desc) -> Result<Plan> {
    let bits = e.bits;
    let float = matches!(bits, 32 | 64);
    if !float && !matches!(bits, 1 | 8 | 10 | 12 | 16) {
        return Err(Error::Unsupported(format!(
            "DPX samples of {bits} bits are not supported"
        )));
    }
    let signed = e.sign == 1 && !float && bits > 1;
    if signed && desc.ycc() {
        return Err(Error::Unsupported(
            "DPX YCbCr with signed samples is not supported: the standard does not say where black and the colour-difference zero lie".into(),
        ));
    }
    let shape = match (bits, e.packing) {
        (32 | 64, _) => Shape::Float,
        (8, _) => Shape::Container {
            unit: 1,
            left: true,
        },
        (16, _) => Shape::Container {
            unit: 2,
            left: true,
        },
        (_, 0) => Shape::Stream { lsb_first: true },
        (10, 1) => Shape::Container {
            unit: 4,
            left: true,
        },
        (10, 2) => Shape::Container {
            unit: 4,
            left: false,
        },
        (12, 1) => Shape::Container {
            unit: 2,
            left: true,
        },
        (12, 2) => Shape::Container {
            unit: 2,
            left: false,
        },
        (1, _) => Shape::Stream { lsb_first: true },
        (_, p) => {
            return Err(Error::Unsupported(format!(
                "DPX packing {p} is not one the standard defines"
            )));
        }
    };
    // SMPTE 268M puts the first datum of a 32-bit word in its low bits. Writers keep that for
    // one-component images (FFmpeg and OpenImageIO read them so) but put R, or the first
    // component, in the high bits otherwise, as every reader expects.
    let low_first = desc == Desc::Single && bits == 10;
    let layout = Layout {
        bits,
        shape,
        be: h.r.be,
        low_first,
    };
    layout.validate()?;
    let max = if float {
        1.0
    } else {
        ((1u32 << bits) - 1) as f32
    };
    let refs = match (e.low_code, e.high_code) {
        (Some(l), Some(hc)) if hc > l && (hc as f32) <= max && !float && !signed => {
            Some((l as f32, hc as f32))
        }
        _ => None,
    };
    let look = if desc.ycc() {
        Look::Display
    } else {
        match e.transfer {
            1 | 3 => {
                let mut p = LogParams::default();
                let ten = |v: f32| v * 1023.0 / max;
                match refs {
                    // Reference codes inside the range are the black and white points (how
                    // Apple, Nuke and FFmpeg write log DPX: 95 and 685).
                    Some((l, hc)) if l > 0.0 || hc < max => {
                        if ten(l) < 400.0 && ten(hc) > 400.0 {
                            p.black = ten(l);
                            p.white = ten(hc);
                        }
                    }
                    // The full code range against the reference densities: density per code.
                    Some((l, hc)) => {
                        if let (Some(lq), Some(hq)) = (e.low_q, e.high_q) {
                            let d = (hq - lq) / ten(hc - l);
                            if (0.0005..0.01).contains(&d) {
                                p.density = d;
                            }
                        }
                    }
                    None => {}
                }
                if h.industry {
                    let black = h.r.f32_set(1952);
                    let white = h.r.f32_set(1964);
                    if let (Some(b), Some(w)) = (black, white) {
                        let (b, w) = (b * 1023.0 / max.max(1.0), w * 1023.0 / max.max(1.0));
                        if b > 0.0 && w > b && w <= 1023.0 {
                            p.black = b;
                            p.white = w;
                        }
                    }
                }
                Look::Log(p)
            }
            2 | 11 | 12 => Look::Linear,
            4..=10 => Look::Display,
            // Float samples with no (or a user-defined) transfer are scene-linear by
            // convention; signed integers are read like them.
            _ if float || signed => Look::Linear,
            _ => Look::Display,
        }
    };
    let ycc_default = || {
        let s = max / 255.0;
        (16.0 * s, 235.0 * s)
    };
    // Reference codes left at zero (rather than the "undefined" all-ones) come from writers
    // that zero the header and store full-range YCbCr (ImageMagick).
    let zeroed = e.low_code == Some(0) && e.high_code == Some(0);
    let (lo, hi) = match look {
        Look::Display if desc.ycc() && zeroed => (0.0, max),
        Look::Display if desc.ycc() => refs.unwrap_or_else(ycc_default),
        _ => refs.unwrap_or((0.0, max)),
    };
    Ok(Plan {
        desc,
        layout,
        look,
        lo,
        hi,
        max,
        // BT.601 only when the colorimetric code says so (601, composite video); HD and
        // unspecified YCbCr take BT.709.
        bt709: !matches!(e.colorimetric, 7..=10),
        signed,
    })
}

/// What a page shows: one image element, or single-channel elements (R, G, B and A; luma,
/// colour difference and A; RGB and A) combined into one colour image.
#[derive(Clone, Debug, PartialEq)]
struct PageDef {
    /// The elements shown, the first giving the facts.
    parts: Vec<usize>,
    /// The combined layout (`None` for a page of one element).
    merged: Option<Desc>,
}

/// The pages of a file: a combined colour page first (when its elements agree in depth and
/// sign), then every element not taking part, in file order.
fn pages(h: &Header) -> Vec<PageDef> {
    let els = &h.elements;
    let find = |code: u8| els.iter().position(|e| e.descriptor == code);
    let same = |a: usize, b: usize| {
        let (x, y) = (&els[a], &els[b]);
        x.bits == y.bits && (x.sign == 1) == (y.sign == 1) && x.bits != 1
    };
    let with_alpha = |first: usize| find(4).filter(|&a| same(first, a));
    let merged = match (find(1), find(2), find(3), find(6), find(7), find(50)) {
        (Some(r), Some(g), Some(b), ..) if same(r, g) && same(r, b) => {
            let mut parts = vec![r, g, b];
            let a = with_alpha(r);
            parts.extend(a);
            Some(PageDef {
                parts,
                merged: Some(if a.is_some() { Desc::Rgba } else { Desc::Rgb }),
            })
        }
        (_, _, _, Some(y), Some(c), _) if same(y, c) => {
            let mut parts = vec![y, c];
            let a = with_alpha(y);
            parts.extend(a);
            Some(PageDef {
                parts,
                merged: Some(if a.is_some() {
                    Desc::Ycca422
                } else {
                    Desc::Ycc422
                }),
            })
        }
        (.., Some(rgb)) => with_alpha(rgb).map(|a| PageDef {
            parts: vec![rgb, a],
            merged: Some(Desc::Rgba),
        }),
        _ => None,
    };
    let mut out = Vec::with_capacity(els.len());
    let used: Vec<usize> = merged.as_ref().map(|m| m.parts.clone()).unwrap_or_default();
    out.extend(merged);
    out.extend(
        (0..els.len())
            .filter(|i| !used.contains(i))
            .map(|i| PageDef {
                parts: vec![i],
                merged: None,
            }),
    );
    out
}

/// Page `page`, its first element and the plan it is shown with.
fn page_plan<'a>(h: &'a Header, page: u32) -> Result<(PageDef, &'a Element, Plan)> {
    let pd = pages(h)
        .into_iter()
        .nth(page as usize)
        .ok_or_else(|| Error::Malformed(format!("no page {}", page + 1)))?;
    let e = &h.elements[pd.parts[0]];
    let p = match pd.merged {
        Some(d) => {
            // Every part must be readable on its own.
            for &i in &pd.parts {
                plan(h, &h.elements[i])?;
            }
            plan_as(h, e, d)?
        }
        None => plan(h, e)?,
    };
    if p.desc == Desc::CbCr {
        return Err(Error::Unsupported(
            "DPX image elements holding only colour difference (CbCr) cannot be shown without a luma element".into(),
        ));
    }
    Ok((pd, e, p))
}

/// The layout a page is drawn with: its own, or for user-defined elements the first three
/// components as RGB (the first as grey when there are two).
fn shown(pd: &PageDef, p: &Plan) -> Option<Desc> {
    match (pd.merged, p.desc) {
        (Some(d), _) => Some(d),
        (None, Desc::User(n)) if n >= 3 => Some(Desc::Rgb),
        (None, Desc::User(_)) => Some(Desc::Single),
        _ => None,
    }
}

/// Header facts of page `page`.
pub fn info(data: &[u8], page: u32) -> Result<Info> {
    let h = header(data)?;
    check_dims(h.width, h.height)?;
    let (pd, e, p) = page_plan(&h, page)?;
    let desc = shown(&pd, &p).unwrap_or(p.desc);
    let model = match desc {
        Desc::Single if e.bits == 1 => Model::Bilevel,
        Desc::Single => Model::Grey,
        Desc::Rgb | Desc::Rgba | Desc::Abgr => Model::Rgb,
        _ => Model::YCbCr,
    };
    let mut coding = format!("{}-bit", e.bits);
    if matches!(e.bits, 32 | 64) {
        coding.push_str(" float");
    } else if p.signed {
        coding.push_str(" signed");
    }
    coding.push(' ');
    let names: Vec<String> = pd
        .parts
        .iter()
        .map(|&i| descriptor_name(h.elements[i].descriptor))
        .collect();
    coding.push_str(&names.join(" + "));
    if pd.merged.is_some() {
        coding.push_str(" elements");
    }
    match (p.desc, e.descriptor) {
        (Desc::User(n), _) if pd.merged.is_none() => coding.push_str(if n >= 3 {
            " (first three shown as RGB)"
        } else {
            " (first shown as grey)"
        }),
        (_, 9) => coding.push_str(" (shown as grey, not demodulated)"),
        _ => {}
    }
    if let Some(pk) = packing_name(e.bits, e.packing) {
        coding.push_str(", ");
        coding.push_str(pk);
    }
    coding.push_str(if h.r.be {
        ", big-endian"
    } else {
        ", little-endian"
    });
    if let Some(t) = transfer_name(e.transfer).filter(|_| e.transfer != 0) {
        coding.push_str(", ");
        coding.push_str(t);
    }
    if pd.parts.iter().any(|&i| h.elements[i].encoding == 1) {
        coding.push_str(", RLE");
    }
    let (summary, fields) = facts(&h, pd.parts[0] as u32);
    Ok(Info {
        width: h.width,
        height: h.height,
        pages: pages(&h).len() as u32,
        bit_depth: e.bits,
        model,
        has_alpha: desc.alpha(),
        orientation: exif_orientation(h.orientation),
        coding,
        summary,
        fields,
    })
}

/// SMPTE time code from its BCD field: "HH:MM:SS:FF" (";" before the frames when the
/// drop-frame flag is set).
fn timecode(v: u32) -> Option<String> {
    let digit = |b: u32, mask: u32| -> Option<u32> {
        let b = b & mask;
        let (t, u) = (b >> 4, b & 0xf);
        (u < 10 && t < 10).then_some(t * 10 + u)
    };
    let hh = digit(v >> 24, 0x3f)?;
    let mm = digit(v >> 16, 0x7f)?;
    let ss = digit(v >> 8, 0x7f)?;
    let ff = digit(v, 0x3f)?;
    let drop = v & 0x40 != 0;
    Some(format!(
        "{hh:02}:{mm:02}:{ss:02}{}{ff:02}",
        if drop { ';' } else { ':' }
    ))
}

fn facts(h: &Header, page: u32) -> crate::Facts {
    let r = h.r;
    let mut s: Vec<(&'static str, String)> = Vec::new();
    let mut f: Vec<(String, String)> = Vec::new();
    let mut field = |name: &str, v: Option<String>| {
        if let Some(v) = v.filter(|v| !v.is_empty()) {
            f.push((name.to_owned(), v));
        }
    };
    let u = |o: usize| r.u32_set(o).filter(|&v| v != 0).map(|v| v.to_string());
    let fl = |o: usize| r.f32_set(o).filter(|&v| v != 0.0).map(num);
    field("Version", r.str(8, 8));
    field("Image data offset", r.u32_set(4).map(|v| v.to_string()));
    field("File size", u(16));
    field(
        "Ditto key",
        r.u32_set(20).map(|v| match v {
            0 => "same as the previous frame".to_owned(),
            1 => "new frame".to_owned(),
            v => v.to_string(),
        }),
    );
    field("Generic header size", u(24));
    field("Industry header size", u(28));
    field("User data size", u(32));
    field("File name", r.str(36, 100));
    let created = r.str(136, 24).map(|d| tidy_date(&d));
    field("Created", created.clone());
    field("Creator", r.str(160, 100));
    field("Project", r.str(260, 200));
    field("Copyright", r.str(460, 200));
    field(
        "Encryption key",
        r.u32_set(660)
            .filter(|&v| v != 0)
            .map(|v| format!("0x{v:08x}")),
    );
    field(
        "Orientation",
        Some(orientation_name(h.orientation).to_owned()),
    );
    field("Image elements", Some(h.elements.len().to_string()));
    field("Pixels per line", Some(h.width.to_string()));
    field("Lines per element", Some(h.height.to_string()));
    for (i, e) in h.elements.iter().enumerate() {
        let n = i + 1;
        let mut ef = |k: &str, v: Option<String>| field(&format!("Element {n} {k}"), v);
        ef("descriptor", Some(descriptor_name(e.descriptor)));
        ef(
            "transfer",
            Some(transfer_name(e.transfer).map_or_else(|| e.transfer.to_string(), str::to_owned)),
        );
        ef(
            "colorimetric",
            Some(
                transfer_name(e.colorimetric)
                    .map_or_else(|| e.colorimetric.to_string(), str::to_owned),
            ),
        );
        ef("bit size", Some(e.bits.to_string()));
        ef(
            "packing",
            packing_name(e.bits, e.packing).map(str::to_owned),
        );
        ef(
            "encoding",
            Some(if e.encoding == 1 { "RLE" } else { "none" }.to_owned()),
        );
        ef(
            "data sign",
            Some(if e.sign == 1 { "signed" } else { "unsigned" }.to_owned()),
        );
        ef("data offset", e.offset.map(|v| v.to_string()));
        if let (Some(l), Some(hc)) = (e.low_code, e.high_code)
            && hc > 0
        {
            ef("reference codes", Some(format!("{l} – {hc}")));
        }
        if let (Some(l), Some(hq)) = (e.low_q, e.high_q)
            && hq != 0.0
        {
            ef(
                "reference quantities",
                Some(format!("{} – {}", num(l), num(hq))),
            );
        }
        ef(
            "end-of-line padding",
            e.eol.filter(|&v| v != 0).map(|v| v.to_string()),
        );
        ef(
            "end-of-image padding",
            e.eoi.filter(|&v| v != 0).map(|v| v.to_string()),
        );
        ef("description", e.description.clone());
    }
    // Image source header.
    field("X offset", u(1408));
    field("Y offset", u(1412));
    field("X centre", fl(1416));
    field("Y centre", fl(1420));
    field("X original size", u(1424));
    field("Y original size", u(1428));
    field("Source file name", r.str(1432, 100));
    field("Source created", r.str(1532, 24).map(|d| tidy_date(&d)));
    let device = r.str(1556, 32);
    let serial = r.str(1588, 32);
    field("Input device", device.clone());
    field("Input device serial", serial.clone());
    let border: Vec<u16> = (0..4).filter_map(|i| r.u16(1620 + 2 * i)).collect();
    if border.len() == 4 && border.iter().any(|&b| b != 0 && b != 0xffff) {
        field(
            "Border",
            Some(format!(
                "left {}, right {}, top {}, bottom {}",
                border[0], border[1], border[2], border[3]
            )),
        );
    }
    if let (Some(a), Some(b)) = (r.u32_set(1628), r.u32_set(1632))
        && a != 0
        && b != 0
    {
        field("Pixel aspect ratio", Some(format!("{a}:{b}")));
    }
    field("X scanned size", fl(1636));
    field("Y scanned size", fl(1640));
    if let Some(d) = match (&device, &serial) {
        (Some(d), Some(sn)) => Some(format!("{d} ({sn})")),
        (Some(d), None) => Some(d.clone()),
        (None, Some(sn)) => Some(sn.clone()),
        _ => None,
    } {
        s.push(("device", d));
    }
    if let Some(c) = created {
        s.push(("created", c));
    }
    for (key, o, len) in [
        ("creator", 160, 100),
        ("project", 260, 200),
        ("copyright", 460, 200),
    ] {
        if let Some(v) = r.str(o, len) {
            s.push((key, v));
        }
    }
    if h.industry {
        let mfg = r.str(1664, 2);
        let ftype = r.str(1666, 2);
        let perfs = r.str(1668, 2);
        let prefix = r.str(1670, 6);
        let count = r.str(1676, 4);
        field("Film manufacturer ID", mfg.clone());
        field("Film type", ftype.clone());
        field("Perfs offset", perfs.clone());
        field("Prefix", prefix.clone());
        field("Count", count.clone());
        if let (Some(p), Some(c)) = (&prefix, &count) {
            let mut k = String::new();
            for part in [&mfg, &ftype].into_iter().flatten() {
                k.push_str(part);
                k.push(' ');
            }
            k.push_str(p);
            k.push(' ');
            k.push_str(c);
            if let Some(pf) = &perfs {
                k.push('+');
                k.push_str(pf);
            }
            s.push(("edge-code", k));
        }
        let format = r.str(1680, 32);
        field("Film format", format.clone());
        let pos = u(1712);
        let seq = u(1716);
        let held = u(1720);
        field("Frame position", pos.clone());
        field("Sequence length", seq.clone());
        field("Held count", held.clone());
        let film_rate = fl(1724);
        field("Frame rate", film_rate.clone());
        field("Shutter angle", fl(1728));
        let frame_id = r.str(1732, 32);
        field("Frame ID", frame_id.clone());
        let slate = r.str(1764, 100);
        field("Slate", slate.clone());
        let tc = r.u32_set(1920).filter(|&v| v != 0).and_then(timecode);
        field("Time code", tc.clone());
        field(
            "User bits",
            r.u32_set(1924)
                .filter(|&v| v != 0)
                .map(|v| format!("{v:08x}")),
        );
        field(
            "Interlace",
            r.u8(1928).filter(|&v| v != 0xff).map(|v| {
                if v == 0 {
                    "progressive".to_owned()
                } else {
                    "interlaced (2:1)".to_owned()
                }
            }),
        );
        field(
            "Field number",
            r.u8(1929)
                .filter(|&v| v != 0xff && v != 0)
                .map(|v| v.to_string()),
        );
        field(
            "Video signal",
            r.u8(1930).filter(|&v| v != 0xff && v != 0).map(|v| {
                match v {
                    1 => "NTSC",
                    2 => "PAL",
                    3 => "PAL-M",
                    4 => "SECAM",
                    50 => "YCbCr BT.601 525-line 2:1 4:3",
                    51 => "YCbCr BT.601 625-line 2:1 4:3",
                    100 => "YCbCr BT.601 525-line 2:1 16:9",
                    101 => "YCbCr BT.601 625-line 2:1 16:9",
                    150 => "YCbCr 1050-line 2:1 16:9",
                    151 => "YCbCr 1125-line 2:1 16:9 (SMPTE 274M)",
                    152 => "YCbCr 1250-line 2:1 16:9",
                    153 => "YCbCr 1125-line 2:1 16:9 (SMPTE 240M)",
                    200 => "YCbCr 525-line 1:1 16:9",
                    201 => "YCbCr 625-line 1:1 16:9",
                    202 => "YCbCr 750-line 1:1 16:9 (SMPTE 296M)",
                    203 => "YCbCr 1125-line 1:1 16:9 (SMPTE 274M)",
                    _ => "other",
                }
                .to_owned()
            }),
        );
        field("Horizontal sample rate", fl(1932));
        field("Vertical sample rate", fl(1936));
        let tv_rate = fl(1940);
        field("Temporal sampling rate", tv_rate.clone());
        field("Time offset", fl(1944));
        field("Gamma", fl(1948));
        field("Black level", fl(1952));
        field("Black gain", fl(1956));
        field("Breakpoint", fl(1960));
        field("White level", fl(1964));
        field("Integration time", fl(1968));
        if let Some(t) = tc {
            s.push(("timecode", t));
        }
        if let Some(rate) = film_rate.or(tv_rate) {
            s.push(("frame-rate", format!("{rate} fps")));
        }
        if let Some(p) = &pos {
            let mut v = p.clone();
            if let Some(q) = &seq {
                v = format!("{v} / {q}");
            }
            s.push(("frame", v));
        } else if let Some(id) = &frame_id {
            s.push(("frame", id.clone()));
        }
        // Format and slate as two values of one fact: the app joins them in its direction.
        for v in [format, slate].into_iter().flatten() {
            s.push(("film", v));
        }
    }
    if let Some(e) = h.elements.get(page as usize) {
        if let Some(t) = transfer_name(e.transfer).filter(|_| e.transfer != 0 && e.transfer != 0xff)
        {
            s.push(("transfer", t.to_owned()));
        }
        if let Some(c) =
            transfer_name(e.colorimetric).filter(|_| e.colorimetric != 0 && e.colorimetric != 0xff)
        {
            s.push(("colorimetric", c.to_owned()));
        }
        if let Some(d) = &e.description {
            s.push(("comment", d.clone()));
        }
    }
    (s, f)
}

/// Where an element's samples are, after run-length expansion when encoded.
struct Prepared {
    expanded: Option<Vec<u8>>,
    /// The budget held for `expanded`, given back with it.
    _held: Option<specular_core::Reservation>,
    end: usize,
    rows: Rows,
    layout: Layout,
    n: usize,
    truncated: bool,
}

impl Prepared {
    fn src<'a>(&'a self, data: &'a [u8]) -> &'a [u8] {
        self.expanded.as_deref().unwrap_or(&data[..self.end])
    }
}

fn prepare(h: &Header, e: &Element, p: &Plan, data: &[u8], index: usize) -> Result<Prepared> {
    let (w, ht) = (h.width as usize, h.height as usize);
    let page = index;
    let offset = match e.offset.filter(|&o| o != 0) {
        Some(o) => o as usize,
        None if page == 0 => h.data_offset as usize,
        None => {
            return Err(Error::Malformed(format!(
                "image element {} has no data offset",
                page + 1
            )));
        }
    };
    if offset < 768 || offset > data.len() {
        return Err(Error::Truncated(format!(
            "image element {} starts past the end of the file",
            page + 1
        )));
    }
    // The element ends where the next one starts, or at the end of the file.
    let end = h
        .elements
        .iter()
        .filter_map(|o| o.offset)
        .map(|o| o as usize)
        .filter(|&o| o > offset && o <= data.len())
        .min()
        .unwrap_or(data.len());
    let n = p.desc.samples(w);
    let need = p.layout.bytes_for(n as u64 * ht as u64);
    let ratio = if e.encoding == 1 { 256 } else { 1 };
    check_payload(end - offset, need, ratio)?;
    let eol = e.eol.filter(|&v| v < 1 << 20).unwrap_or(0) as usize;
    if e.encoding == 1 {
        let float = p.layout.shape == Shape::Float;
        let (bytes, short, held) = rle_expand(p, &data[..end], offset, n, ht)?;
        return Ok(Prepared {
            expanded: Some(bytes),
            _held: Some(held),
            end,
            rows: Rows {
                base: 0,
                stride: 0,
                n,
            },
            layout: expanded_layout(float),
            n,
            truncated: short,
        });
    }
    Ok(Prepared {
        expanded: None,
        _held: None,
        end,
        rows: Rows::choose(&p.layout, offset, end - offset, n, ht, eol),
        layout: p.layout,
        n,
        truncated: false,
    })
}

/// The layout of expanded and combined samples: little-endian 16-bit codes or 32-bit floats.
fn expanded_layout(float: bool) -> Layout {
    Layout {
        bits: if float { 32 } else { 16 },
        shape: if float {
            Shape::Float
        } else {
            Shape::Container {
                unit: 2,
                left: true,
            }
        },
        be: false,
        low_first: false,
    }
}

/// Where sample `j` of a row of an element (layout `src`, descriptor `code`) goes in a row
/// of the combined layout `out`; `None` when it is not shown.
fn slot(out: Desc, src: Desc, code: u8, j: usize) -> Option<usize> {
    let (pair, odd) = (j / 2, j & 1);
    Some(match (out, src) {
        (Desc::Rgb | Desc::Rgba, Desc::Single) => {
            let c = match code {
                1 => 0,
                2 => 1,
                3 => 2,
                4 => 3,
                _ => return None,
            };
            j * out.comps() + c
        }
        (Desc::Rgba, Desc::Rgb) => j / 3 * 4 + j % 3,
        (Desc::Ycc422, Desc::Single) => pair * 4 + 1 + 2 * odd,
        (Desc::Ycca422, Desc::Single) if code == 4 => pair * 6 + 2 + 3 * odd,
        (Desc::Ycca422, Desc::Single) => pair * 6 + 1 + 3 * odd,
        (Desc::Ycc422, Desc::CbCr) => pair * 4 + 2 * odd,
        (Desc::Ycca422, Desc::CbCr) => pair * 6 + 3 * odd,
        (Desc::Rgb | Desc::Single, Desc::User(n)) => {
            let n = n as usize;
            let c = j % n;
            if c >= out.comps() {
                return None;
            }
            j / n * out.comps() + c
        }
        _ => return None,
    })
}

/// The samples of `parts` (elements of one depth) interleaved as `desc`: separate colour
/// elements combined, or a user-defined element's first components picked. 16-bit codes, or
/// 32-bit floats for float elements; held against the memory budget.
fn gather(h: &Header, parts: &[usize], desc: Desc, data: &[u8]) -> Result<Prepared> {
    let (w, ht) = (h.width as usize, h.height as usize);
    let n_out = desc.samples(w);
    let first = plan(h, &h.elements[parts[0]])?;
    let float = first.layout.shape == Shape::Float;
    let size = if float { 4 } else { 2 };
    let total = n_out as u64 * ht as u64;
    let held = specular_core::current_budget().reserve_n(total, size as u64)?;
    let mut buf = vec![0u8; total as usize * size];
    let mut truncated = false;
    for &i in parts {
        let e = &h.elements[i];
        let p = plan(h, e)?;
        let prep = prepare(h, e, &p, data, i)?;
        truncated |= prep.truncated;
        let slots: Vec<Option<usize>> = (0..prep.n)
            .map(|j| slot(desc, p.desc, e.descriptor, j).filter(|&o| o < n_out))
            .collect();
        let src = prep.src(data);
        let (rows, layout) = (prep.rows, prep.layout);
        let short = if float {
            for_rows(&mut buf, n_out * 4, prep.n, |y, row, s: &mut [f32]| {
                let (base, f0) = rows.at(y);
                let got = layout.unpack_float(src, base, f0, s);
                s[got..].fill(0.5);
                for (&v, o) in s.iter().zip(&slots) {
                    if let Some(o) = *o {
                        row[o * 4..o * 4 + 4].copy_from_slice(&v.to_le_bytes());
                    }
                }
                got == s.len()
            })
        } else {
            let grey = (p.max / 2.0) as u16;
            for_rows(&mut buf, n_out * 2, prep.n, |y, row, s: &mut [u16]| {
                let (base, f0) = rows.at(y);
                let got = layout.unpack(src, base, f0, s);
                s[got..].fill(grey);
                for (&v, o) in s.iter().zip(&slots) {
                    if let Some(o) = *o {
                        row[o * 2..o * 2 + 2].copy_from_slice(&v.to_le_bytes());
                    }
                }
                got == s.len()
            })
        };
        truncated |= short;
    }
    Ok(Prepared {
        expanded: Some(buf),
        _held: Some(held),
        end: data.len(),
        rows: Rows {
            base: 0,
            stride: 0,
            n: n_out,
        },
        layout: expanded_layout(float),
        n: n_out,
        truncated,
    })
}

/// The stored integer code values of image element `page`, `n` samples per row in file
/// order (for tests against other readers): (width, height, samples per row, samples).
#[doc(hidden)]
pub fn decode_codes(data: &[u8], page: u32) -> Result<(u32, u32, usize, Vec<u16>)> {
    decode_codes_present(data, page).map(|(w, h, n, c, _)| (w, h, n, c))
}

/// [`decode_codes`] plus how many samples, from the start, the file holds (fewer than all
/// when it is truncated; the rest are 0).
#[doc(hidden)]
pub fn decode_codes_present(data: &[u8], page: u32) -> Result<(u32, u32, usize, Vec<u16>, usize)> {
    let h = header(data)?;
    check_dims(h.width, h.height)?;
    let (pd, e, p) = page_plan(&h, page)?;
    if p.layout.shape == Shape::Float {
        return Err(Error::Unsupported(
            "float samples have no code values".into(),
        ));
    }
    let prep = match pd.merged {
        Some(d) => gather(&h, &pd.parts, d, data)?,
        None => prepare(&h, e, &p, data, pd.parts[0])?,
    };
    let src = prep.src(data);
    let mut out = vec![0u16; prep.n * h.height as usize];
    let mut present = 0;
    let mut whole = true;
    for (y, row) in out.chunks_exact_mut(prep.n.max(1)).enumerate() {
        let (base, first) = prep.rows.at(y);
        let got = prep.layout.unpack(src, base, first, row);
        if whole {
            present += got;
            whole = got == row.len();
        }
    }
    Ok((h.width, h.height, prep.n, out, present))
}

/// Decode image element `page`.
pub fn decode(data: &[u8], page: u32) -> Result<Image> {
    let h = header(data)?;
    check_dims(h.width, h.height)?;
    let (pd, e, p) = page_plan(&h, page)?;
    let (w, ht) = (h.width as usize, h.height as usize);
    let view = shown(&pd, &p);
    let prep = match view {
        Some(d) => gather(&h, &pd.parts, d, data)?,
        None => prepare(&h, e, &p, data, pd.parts[0])?,
    };
    let p = match view {
        Some(desc) => Plan { desc, ..p },
        None => p,
    };
    let (rows, layout, n, mut truncated) = (prep.rows, prep.layout, prep.n, prep.truncated);
    let src = prep.src(data);
    let grey = (p.max / 2.0) as u16;
    // Fills `s` with the samples of row `y`; whether all were present.
    let fetch = |y: usize, s: &mut [u16]| -> bool {
        let (base, first) = rows.at(y);
        let got = layout.unpack(src, base, first, s);
        if got < s.len() {
            s[got..].fill(grey);
            false
        } else {
            true
        }
    };
    let pixels = if p.layout.shape == Shape::Float {
        let mut out = vec![0f32; w * ht * 4];
        let look = p.look;
        let short = for_rows(&mut out, w * 4, n, |y, row, s: &mut [f32]| {
            let (base, first) = rows.at(y);
            let got = layout.unpack_float(src, base, first, s);
            s[got..].fill(0.5);
            float_row(p.desc, look, p.bt709, s, row);
            got == s.len()
        });
        truncated |= short;
        Pixels::RgbaF32(out)
    } else if p.signed {
        // Two's complement, −1 to 1 (the most negative code clipped to −1), then as floats.
        let half = 1i32 << (p.layout.bits - 1);
        let inv = 1.0 / (half - 1) as f32;
        let mut out = vec![0f32; w * ht * 4];
        let look = p.look;
        let short = for_rows(&mut out, w * 4, n, |y, row, s: &mut [u16]| {
            let ok = fetch(y, s);
            let f: Vec<f32> = s
                .iter()
                .map(|&c| {
                    let v = c as i32;
                    let v = if v >= half { v - 2 * half } else { v };
                    (v as f32 * inv).max(-1.0)
                })
                .collect();
            float_row(p.desc, look, p.bt709, &f, row);
            ok
        });
        truncated |= short;
        Pixels::RgbaF32(out)
    } else if p.desc.ycc() {
        let mut out = vec![0u16; w * ht * 4];
        let short = for_rows(&mut out, w * 4, n, |y, row, s: &mut [u16]| {
            let ok = fetch(y, s);
            ycc_row(&p, w, s, row);
            ok
        });
        truncated |= short;
        Pixels::Rgba16(out)
    } else {
        match p.look {
            Look::Display => {
                let table = log::display_table(p.layout.bits.min(16), p.lo, p.hi);
                let alpha = log::display_table(p.layout.bits.min(16), 0.0, p.max);
                let eight = e.bits <= 8;
                let ch = if p.desc == Desc::Single { 1 } else { 4 };
                let mut out = vec![0u16; w * ht * ch];
                let short = for_rows(&mut out, w * ch, n, |y, row, s: &mut [u16]| {
                    let ok = fetch(y, s);
                    display_row(p.desc, &table, &alpha, s, row);
                    ok
                });
                truncated |= short;
                match (eight, ch) {
                    (true, 1) => Pixels::Gray8(to8(out)),
                    (true, _) => Pixels::Rgba8(to8(out)),
                    (false, 1) => Pixels::Gray16(out),
                    (false, _) => Pixels::Rgba16(out),
                }
            }
            Look::Log(lp) => {
                let table = log::log_table(p.layout.bits, &lp);
                f32_image(&p, w, ht, n, &table, &fetch, &mut truncated)
            }
            Look::Linear => {
                let table = log::linear_table(p.layout.bits, p.lo, p.hi);
                f32_image(&p, w, ht, n, &table, &fetch, &mut truncated)
            }
        }
    };
    Ok(Image {
        width: h.width,
        height: h.height,
        pixels,
        truncated,
    })
}

fn to8(v: Vec<u16>) -> Vec<u8> {
    v.into_iter()
        .map(|x| ((x as u32 * 255 + 32767) / 65535) as u8)
        .collect()
}

fn f32_image(
    p: &Plan,
    w: usize,
    ht: usize,
    n: usize,
    table: &[f32],
    fetch: &(dyn Fn(usize, &mut [u16]) -> bool + Sync),
    truncated: &mut bool,
) -> Pixels {
    let inv = 1.0 / p.max;
    let mut out = vec![0f32; w * ht * 4];
    let short = for_rows(&mut out, w * 4, n, |y, row, s: &mut [u16]| {
        let ok = fetch(y, s);
        let t = |c: u16| table.get(c as usize).copied().unwrap_or(0.0);
        let a = |c: u16| c as f32 * inv;
        for (x, o) in row.chunks_exact_mut(4).enumerate() {
            let px = match p.desc {
                Desc::Single => {
                    let v = t(s[x]);
                    [v, v, v, 1.0]
                }
                Desc::Rgb => [t(s[3 * x]), t(s[3 * x + 1]), t(s[3 * x + 2]), 1.0],
                Desc::Rgba => [
                    t(s[4 * x]),
                    t(s[4 * x + 1]),
                    t(s[4 * x + 2]),
                    a(s[4 * x + 3]),
                ],
                Desc::Abgr => [
                    t(s[4 * x + 3]),
                    t(s[4 * x + 2]),
                    t(s[4 * x + 1]),
                    a(s[4 * x]),
                ],
                _ => [0.0; 4],
            };
            o.copy_from_slice(&px);
        }
        ok
    });
    *truncated |= short;
    Pixels::RgbaF32(out)
}

fn display_row(desc: Desc, table: &[u16], alpha: &[u16], s: &[u16], row: &mut [u16]) {
    let t = |c: u16| table.get(c as usize).copied().unwrap_or(0);
    let a = |c: u16| alpha.get(c as usize).copied().unwrap_or(0);
    match desc {
        Desc::Single => {
            for (o, &c) in row.iter_mut().zip(s) {
                *o = t(c);
            }
        }
        Desc::Rgb => {
            for (o, c) in row.chunks_exact_mut(4).zip(s.chunks_exact(3)) {
                o.copy_from_slice(&[t(c[0]), t(c[1]), t(c[2]), 65535]);
            }
        }
        Desc::Rgba => {
            for (o, c) in row.chunks_exact_mut(4).zip(s.chunks_exact(4)) {
                o.copy_from_slice(&[t(c[0]), t(c[1]), t(c[2]), a(c[3])]);
            }
        }
        Desc::Abgr => {
            for (o, c) in row.chunks_exact_mut(4).zip(s.chunks_exact(4)) {
                o.copy_from_slice(&[t(c[3]), t(c[2]), t(c[1]), a(c[0])]);
            }
        }
        _ => {}
    }
}

/// Float samples: linear (or log through the printing-density curve, or display-coded
/// through the sRGB curve) to linear RGBA.
fn float_row(desc: Desc, look: Look, _bt709: bool, s: &[f32], row: &mut [f32]) {
    let t = |v: f32| {
        let v = if v.is_finite() { v } else { 0.0 };
        match look {
            Look::Log(p) => p.linear(v * 1023.0),
            Look::Linear => v,
            Look::Display => {
                if v <= 0.04045 {
                    v / 12.92
                } else {
                    ((v + 0.055) / 1.055).powf(2.4)
                }
            }
        }
    };
    for (x, o) in row.chunks_exact_mut(4).enumerate() {
        let px = match desc {
            Desc::Single => {
                let v = t(s[x]);
                [v, v, v, 1.0]
            }
            Desc::Rgb => [t(s[3 * x]), t(s[3 * x + 1]), t(s[3 * x + 2]), 1.0],
            Desc::Rgba => [t(s[4 * x]), t(s[4 * x + 1]), t(s[4 * x + 2]), s[4 * x + 3]],
            Desc::Abgr => [t(s[4 * x + 3]), t(s[4 * x + 2]), t(s[4 * x + 1]), s[4 * x]],
            _ => [0.0, 0.0, 0.0, 1.0],
        };
        o.copy_from_slice(&px);
    }
}

/// One row of YCbCr samples to display RGBA16: Y stretched from the reference codes, Cb and
/// Cr centred on half range with the BT.601 / BT.709 span; 4:2:2 chroma is co-sited with
/// the even pixels and interpolated for the odd ones.
fn ycc_row(p: &Plan, w: usize, s: &[u16], row: &mut [u16]) {
    let (kr, kb) = if p.bt709 {
        (0.2126f32, 0.0722f32)
    } else {
        (0.299, 0.114)
    };
    let kg = 1.0 - kr - kb;
    let yspan = (p.hi - p.lo).max(1e-6);
    // Video range: chroma spans 224/219 of luma; full range: the whole code range.
    let full = p.lo == 0.0 && p.hi == p.max;
    let cspan = if full { p.max } else { yspan * 224.0 / 219.0 };
    let mid = (p.max + 1.0) / 2.0;
    let amax = p.max;
    // (cb, y, cr, alpha) of pixel x.
    let px = |x: usize| -> (f32, f32, f32, f32) {
        match p.desc {
            Desc::Ycc444 => (
                s[3 * x] as f32,
                s[3 * x + 1] as f32,
                s[3 * x + 2] as f32,
                amax,
            ),
            Desc::Ycca444 => (
                s[4 * x] as f32,
                s[4 * x + 1] as f32,
                s[4 * x + 2] as f32,
                s[4 * x + 3] as f32,
            ),
            Desc::Ycc422 | Desc::Ycca422 => {
                let per = if p.desc == Desc::Ycc422 { 4 } else { 6 };
                let pair = x / 2;
                let b = pair * per;
                let (cb, cr, y, a) = if p.desc == Desc::Ycc422 {
                    // Cb Y0 Cr Y1
                    (s[b], s[b + 2], s[b + 1 + 2 * (x & 1)], amax as u16)
                } else {
                    // Cb Y0 A0 Cr Y1 A1
                    let o = 3 * (x & 1);
                    (s[b], s[b + 3], s[b + 1 + o], s[b + 2 + o])
                };
                let (mut cb, mut cr) = (cb as f32, cr as f32);
                if x & 1 == 1 && b + per < s.len() && x + 1 < w {
                    let off = if p.desc == Desc::Ycc422 { 2 } else { 3 };
                    cb = (cb + s[b + per] as f32) / 2.0;
                    cr = (cr + s[b + per + off] as f32) / 2.0;
                }
                (cb, y as f32, cr, a as f32)
            }
            _ => (mid, 0.0, mid, amax),
        }
    };
    for (x, o) in row.chunks_exact_mut(4).enumerate() {
        let (cb, y, cr, a) = px(x);
        let y = (y - p.lo) / yspan;
        let cb = (cb - mid) / cspan;
        let cr = (cr - mid) / cspan;
        let r = y + 2.0 * (1.0 - kr) * cr;
        let b = y + 2.0 * (1.0 - kb) * cb;
        let g = (y - kr * r - kb * b) / kg;
        let q = |v: f32| (v.clamp(0.0, 1.0) * 65535.0 + 0.5) as u16;
        o.copy_from_slice(&[q(r), q(g), q(b), q(a / amax)]);
    }
}

/// Run-length encoded elements (SMPTE 268M): the sample stream is a sequence of runs, each
/// starting with a flag sample whose low bit says whether one pixel repeats (1) or literal
/// pixels follow (0), and whose other bits count the pixels. Expanded to 16-bit samples (32-bit
/// floats for float elements) in one continuous stream, with whether the runs fell short.
///
/// A float element's flag is a float holding the whole number (or, as some writers store
/// it, an integer in the word's bits).
///
/// The unpacked stream and the expansion are reserved from the memory budget; the expansion
/// is sized by a first pass over the runs, so it is never more than the runs describe (nor
/// more than the image holds). The reservation comes back with the bytes.
fn rle_expand(
    p: &Plan,
    d: &[u8],
    offset: usize,
    n: usize,
    ht: usize,
) -> Result<(Vec<u8>, bool, specular_core::Reservation)> {
    let comps = p.desc.comps();
    let total = n * ht;
    let budget = specular_core::current_budget();
    // Unpack the whole element as one continuous stream.
    let avail = d.len().saturating_sub(offset) as u64;
    let bits = p.layout.bits as u64;
    let count = match p.layout.shape {
        Shape::Stream { .. } => avail * 8 / bits,
        Shape::Container { unit, .. } => avail / unit as u64 * (unit as u64 * 8 / bits),
        Shape::Float => avail * 8 / bits,
    }
    .min(total as u64 * 2 + 64) as usize;
    let (out, len, held) = if p.layout.shape == Shape::Float {
        let src_held = budget.reserve_n(count as u64, 4)?;
        let mut src = vec![0f32; count];
        let got = p.layout.unpack_float(d, offset, 0, &mut src);
        src.truncate(got);
        let wide = bits == 64;
        let flag = |v: f32| -> usize {
            if v.is_finite() && v >= 0.0 && v.fract() == 0.0 && v < 16_777_216.0 {
                v as usize
            } else if !wide && v.to_bits() < 1 << 24 {
                v.to_bits() as usize
            } else {
                0
            }
        };
        let put = |v: f32, o: &mut Vec<u8>| o.extend_from_slice(&v.to_le_bytes());
        let len = walk_runs(&src, comps, total, flag, put, None);
        let held = budget.reserve_n(len as u64, 4)?;
        let mut out: Vec<u8> = Vec::with_capacity(len * 4);
        walk_runs(&src, comps, total, flag, put, Some(&mut out));
        drop((src, src_held));
        (out, len, held)
    } else {
        let src_held = budget.reserve_n(count as u64, 2)?;
        let mut src = vec![0u16; count];
        let got = p.layout.unpack(d, offset, 0, &mut src);
        src.truncate(got);
        let flag = |v: u16| v as usize;
        let put = |v: u16, o: &mut Vec<u8>| o.extend_from_slice(&v.to_le_bytes());
        // What the runs describe, up to the image.
        let len = walk_runs(&src, comps, total, flag, put, None);
        let held = budget.reserve_n(len as u64, 2)?;
        let mut out: Vec<u8> = Vec::with_capacity(len * 2);
        walk_runs(&src, comps, total, flag, put, Some(&mut out));
        drop((src, src_held));
        (out, len, held)
    };
    let short = len < total;
    if out.is_empty() && total > 0 {
        return Err(Error::Malformed("no run-length data".into()));
    }
    Ok((out, short, held))
}

/// Walks the runs of `src` up to `total` samples, appending them to `out` with `put` when
/// given. The number of samples the runs give.
fn walk_runs<T: Copy>(
    src: &[T],
    comps: usize,
    total: usize,
    flag: impl Fn(T) -> usize,
    put: impl Fn(T, &mut Vec<u8>),
    mut out: Option<&mut Vec<u8>>,
) -> usize {
    let mut emit = |v: &[T]| {
        if let Some(o) = out.as_deref_mut() {
            for &x in v {
                put(x, o);
            }
        }
    };
    let mut len = 0usize;
    let mut i = 0;
    while len < total && i < src.len() {
        let f = flag(src[i]);
        i += 1;
        let run = (f >> 1).max(1);
        if f & 1 == 1 {
            let Some(v) = src.get(i..i + comps) else {
                break;
            };
            i += comps;
            let take = (run * comps).min(total - len);
            let mut left = take;
            while left > 0 {
                let k = comps.min(left);
                emit(&v[..k]);
                left -= k;
            }
            len += take;
        } else {
            let lit = (run * comps).min(src.len() - i);
            let take = lit.min(total - len);
            emit(&src[i..i + take]);
            len += take;
            i += lit;
        }
    }
    len
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timecodes() {
        assert_eq!(timecode(0x01020304).as_deref(), Some("01:02:03:04"));
        assert_eq!(timecode(0x23595923).as_deref(), Some("23:59:59:23"));
        assert_eq!(timecode(0x10203040 | 0x40).as_deref(), Some("10:20:30;00"));
        assert_eq!(timecode(0x0a000000), None);
    }
}
