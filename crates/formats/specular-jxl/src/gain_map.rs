//! JPEG XL gain maps: the `jhgm` box of the container (ISO/IEC 18181-2 amendment).
//!
//! The box holds a *gain map bundle*:
//!
//! ```text
//! jhgm_version            u8      0
//! metadata_size           u16 BE
//! metadata                        ISO 21496-1 gain-map metadata (not interpreted here)
//! colour_encoding_size    u8      0: none
//! colour_encoding                 JXL ColorEncoding bundle of the alternate rendition
//! alt_icc_size            u32 BE
//! alt_icc                         the alternate's ICC profile, JXL-compressed
//! gain_map                        a bare JPEG XL codestream, to the end of the box
//! ```
//!
//! Everything is bounds-checked; nothing here allocates more than the box holds.

use crate::bitstream::Bitstream;
use crate::common::Bundle;
use crate::image::color::{ColourEncoding, ColourSpace, Primaries as JxlPrimaries, WhitePoint};
use crate::{Error, Result};
use std::ops::Range;

/// Colour primaries (D65 white) the rest of Specular can convert.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Primaries {
    Bt709,
    DisplayP3,
    Bt2020,
}

/// The primaries of a colour encoding: enum RGB (or grey) with sRGB, P3 or BT.2100
/// primaries and a D65 white point, or the same written as custom chromaticities (as libjxl
/// stores an ICC profile it can describe: the colorants adapted to D50 with Bradford, white
/// D50). `None` for ICC-described or XYB-declared encodings and other primaries.
pub(crate) fn enum_primaries(ce: &ColourEncoding) -> Option<Primaries> {
    let ColourEncoding::Enum(e) = ce else {
        return None;
    };
    match e.colour_space {
        ColourSpace::Grey => {
            return (e.white_point == WhitePoint::D65).then_some(Primaries::Bt709);
        }
        ColourSpace::Rgb => {}
        _ => return None,
    }
    if e.white_point == WhitePoint::D65 {
        match e.primaries {
            JxlPrimaries::Srgb => return Some(Primaries::Bt709),
            JxlPrimaries::P3 => return Some(Primaries::DisplayP3),
            JxlPrimaries::Bt2100 => return Some(Primaries::Bt2020),
            JxlPrimaries::Custom { .. } => {}
        }
    }
    let xy = |c: &crate::image::color::Customxy| (c.x as f64 / 1e6, c.y as f64 / 1e6);
    let white = match &e.white_point {
        WhitePoint::D65 => D65,
        WhitePoint::Custom(c) => xy(c),
        _ => return None,
    };
    let rgb = match &e.primaries {
        JxlPrimaries::Custom { red, green, blue } => [xy(red), xy(green), xy(blue)],
        JxlPrimaries::Srgb => KNOWN[0].1,
        JxlPrimaries::P3 => KNOWN[1].1,
        JxlPrimaries::Bt2100 => KNOWN[2].1,
    };
    let near =
        |a: (f64, f64), b: (f64, f64)| (a.0 - b.0).abs() < 0.002 && (a.1 - b.1).abs() < 0.002;
    let target = if near(white, D65) {
        None
    } else if near(white, D50) {
        Some(D50)
    } else {
        return None;
    };
    KNOWN.iter().find_map(|(p, known)| {
        let k = known.map(|c| target.map_or(c, |w| bradford(c, D65, w)));
        k.iter().zip(&rgb).all(|(a, b)| near(*a, *b)).then_some(*p)
    })
}

const D65: (f64, f64) = (0.3127, 0.3290);
const D50: (f64, f64) = (0.3457, 0.3585);

/// The primaries Specular converts, as xy with a D65 white.
const KNOWN: [(Primaries, [(f64, f64); 3]); 3] = [
    (Primaries::Bt709, [(0.64, 0.33), (0.30, 0.60), (0.15, 0.06)]),
    (
        Primaries::DisplayP3,
        [(0.68, 0.32), (0.265, 0.69), (0.15, 0.06)],
    ),
    (
        Primaries::Bt2020,
        [(0.708, 0.292), (0.170, 0.797), (0.131, 0.046)],
    ),
];

/// Chromaticity `c` adapted from white `from` to white `to` (Bradford).
fn bradford(c: (f64, f64), from: (f64, f64), to: (f64, f64)) -> (f64, f64) {
    const M: [[f64; 3]; 3] = [
        [0.8951, 0.2664, -0.1614],
        [-0.7502, 1.7135, 0.0367],
        [0.0389, -0.0685, 1.0296],
    ];
    const MI: [[f64; 3]; 3] = [
        [0.986_992_9, -0.147_054_3, 0.159_962_7],
        [0.432_305_3, 0.518_360_3, 0.049_291_2],
        [-0.008_528_7, 0.040_042_8, 0.968_486_7],
    ];
    let xyz = |p: (f64, f64)| [p.0 / p.1, 1.0, (1.0 - p.0 - p.1) / p.1];
    let mul = |m: &[[f64; 3]; 3], v: [f64; 3]| {
        [0, 1, 2].map(|i| m[i][0] * v[0] + m[i][1] * v[1] + m[i][2] * v[2])
    };
    let (ws, wd) = (mul(&M, xyz(from)), mul(&M, xyz(to)));
    let lms = mul(&M, xyz(c));
    let a = [0, 1, 2].map(|i| lms[i] * wd[i] / ws[i]);
    let v = mul(&MI, a);
    let sum = v[0] + v[1] + v[2];
    (v[0] / sum, v[1] / sum)
}

/// A parsed gain map bundle; every slice borrows the box.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GainMapBundle<'a> {
    /// ISO 21496-1 metadata, as stored.
    pub metadata: &'a [u8],
    /// The alternate rendition's ColorEncoding bundle; empty when absent.
    pub colour_encoding: &'a [u8],
    /// The alternate rendition's compressed ICC profile; empty when absent.
    pub alt_icc: &'a [u8],
    /// The gain map: a bare JPEG XL codestream (starts `FF 0A`).
    pub codestream: &'a [u8],
}

impl GainMapBundle<'_> {
    /// The primaries of the alternate rendition's colour encoding, when it has one the
    /// renderer knows (see [`Primaries`]).
    pub fn alt_primaries(&self) -> Option<Primaries> {
        if self.colour_encoding.is_empty() {
            return None;
        }
        let bytes = self.colour_encoding;
        let r = std::panic::catch_unwind(|| {
            let mut bs = Bitstream::new(bytes);
            <ColourEncoding as Bundle<()>>::parse(&mut bs, ()).ok()
        });
        enum_primaries(&r.ok()??)
    }
}

/// Parses the body of a `jhgm` box.
pub fn parse_gain_map_bundle(body: &[u8]) -> Result<GainMapBundle<'_>> {
    let bad = |s: &str| Error::Malformed(format!("jhgm: {s}"));
    let (&version, rest) = body.split_first().ok_or_else(|| bad("empty box"))?;
    if version != 0 {
        return Err(Error::Unsupported(format!("jhgm version {version}")));
    }
    let (n, rest) = split_be::<2>(rest).ok_or_else(|| bad("no metadata size"))?;
    let (metadata, rest) = split(rest, n).ok_or_else(|| bad("metadata past the box"))?;
    let (n, rest) = split_be::<1>(rest).ok_or_else(|| bad("no colour encoding size"))?;
    let (colour_encoding, rest) =
        split(rest, n).ok_or_else(|| bad("colour encoding past the box"))?;
    let (n, rest) = split_be::<4>(rest).ok_or_else(|| bad("no ICC size"))?;
    let (alt_icc, codestream) = split(rest, n).ok_or_else(|| bad("ICC past the box"))?;
    if !codestream.starts_with(&[0xff, 0x0a]) {
        return Err(bad("the gain map is not a JPEG XL codestream"));
    }
    Ok(GainMapBundle {
        metadata,
        colour_encoding,
        alt_icc,
        codestream,
    })
}

/// A big-endian size field of `N` bytes, then the rest.
fn split_be<const N: usize>(b: &[u8]) -> Option<(usize, &[u8])> {
    let (head, rest) = b.split_at_checked(N)?;
    let n = head.iter().fold(0usize, |a, &v| (a << 8) | v as usize);
    Some((n, rest))
}

fn split(b: &[u8], n: usize) -> Option<(&[u8], &[u8])> {
    b.split_at_checked(n)
}

/// What a container says about a gain map.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Found {
    None,
    /// The byte range of a `jhgm` box's body.
    Box(Range<usize>),
    /// The byte range of the Brotli stream of a `brob` box holding a `jhgm` box.
    Brotli(Range<usize>),
    /// Cut short: there, but not readable.
    Unreadable(&'static str),
}

/// Walks the top-level ISOBMFF boxes of a container for the first `jhgm` box (only box
/// headers are read). A bare codestream has none.
pub(crate) fn find_gain_map(data: &[u8]) -> Found {
    const SIG: [u8; 12] = [
        0, 0, 0, 0x0c, b'J', b'X', b'L', b' ', 0x0d, 0x0a, 0x87, 0x0a,
    ];
    if !data.starts_with(&SIG) {
        return Found::None;
    }
    let mut i = 0usize;
    while let Some(head) = data.get(i..i + 8) {
        let size = u32::from_be_bytes([head[0], head[1], head[2], head[3]]) as u64;
        let kind = &head[4..8];
        let (size, header) = match size {
            0 => ((data.len() - i) as u64, 8),
            1 => match data.get(i + 8..i + 16) {
                Some(b) => (u64::from_be_bytes(b.try_into().unwrap_or([0; 8])), 16),
                None => break,
            },
            s => (s, 8),
        };
        let end = usize::try_from(size)
            .ok()
            .and_then(|s| i.checked_add(s))
            .filter(|&e| size >= header && e <= data.len());
        let Some(end) = end else {
            // A box running past the data: the file is cut short (or the size is garbage).
            return if kind == b"jhgm" {
                Found::Unreadable("truncated jhgm box")
            } else {
                Found::None
            };
        };
        let body = i + header as usize..end;
        match kind {
            b"jhgm" => return Found::Box(body),
            b"brob" if data.get(body.start..body.start + 4) == Some(b"jhgm") => {
                return Found::Brotli(body.start + 4..body.end);
            }
            _ => {}
        }
        i = end;
    }
    Found::None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The custom encodings libjxl writes for sRGB and Display P3 ICC profiles (colorants
    /// adapted to D50, white D50; values from libjxl 0.12 output) name their primaries.
    #[test]
    fn d50_adapted_custom_encodings_name_their_primaries() {
        use crate::image::color::{Customxy, EnumColourEncoding};
        let c = |x: i32, y: i32| Customxy { x, y };
        let enc = |r: Customxy, g: Customxy, b: Customxy| {
            ColourEncoding::Enum(EnumColourEncoding {
                colour_space: ColourSpace::Rgb,
                white_point: WhitePoint::Custom(c(345_703, 358_538)),
                primaries: JxlPrimaries::Custom {
                    red: r,
                    green: g,
                    blue: b,
                },
                ..Default::default()
            })
        };
        let srgb = enc(c(648_438, 330_868), c(321_176, 597_877), c(155_902, 66_052));
        let p3 = enc(c(682_041, 319_353), c(284_548, 674_632), c(155_896, 66_062));
        assert_eq!(enum_primaries(&srgb), Some(Primaries::Bt709));
        assert_eq!(enum_primaries(&p3), Some(Primaries::DisplayP3));
        // BT.2020's colorants adapted the same way.
        let a = KNOWN[2].1.map(|p| bradford(p, D65, D50));
        let q = |p: (f64, f64)| c((p.0 * 1e6).round() as i32, (p.1 * 1e6).round() as i32);
        let wide = enc(q(a[0]), q(a[1]), q(a[2]));
        assert_eq!(enum_primaries(&wide), Some(Primaries::Bt2020));
        // Other primaries: none.
        let other = enc(c(700_000, 300_000), c(200_000, 700_000), c(150_000, 60_000));
        assert_eq!(enum_primaries(&other), None);
    }

    fn bundle(meta: &[u8], ce: &[u8], icc: &[u8], cs: &[u8]) -> Vec<u8> {
        let mut b = vec![0];
        b.extend_from_slice(&(meta.len() as u16).to_be_bytes());
        b.extend_from_slice(meta);
        b.push(ce.len() as u8);
        b.extend_from_slice(ce);
        b.extend_from_slice(&(icc.len() as u32).to_be_bytes());
        b.extend_from_slice(icc);
        b.extend_from_slice(cs);
        b
    }

    #[test]
    fn bundle_fields() {
        let b = bundle(b"meta", &[0x01], b"icc", &[0xff, 0x0a, 1, 2]);
        let g = parse_gain_map_bundle(&b).unwrap();
        assert_eq!(g.metadata, b"meta");
        assert_eq!(g.colour_encoding, &[0x01]);
        assert_eq!(g.alt_icc, b"icc");
        assert_eq!(g.codestream, &[0xff, 0x0a, 1, 2]);
        // all_default: sRGB.
        assert_eq!(g.alt_primaries(), Some(Primaries::Bt709));
        // Every cut before the codestream signature is complete fails.
        for n in 0..b.len() - 2 {
            assert!(parse_gain_map_bundle(&b[..n]).is_err(), "{n}");
        }
        let mut v1 = b.clone();
        v1[0] = 1;
        assert!(matches!(
            parse_gain_map_bundle(&v1),
            Err(Error::Unsupported(_))
        ));
    }

    /// ColorEncoding bundles written field by field (LSB first).
    #[test]
    fn alternate_primaries() {
        // all_default 0, want_icc 0, RGB (2 bits 0), D65 (2 bits 1), primaries P3 (11:
        // selector 2, 4 bits 9), have_gamma 0, transfer sRGB (13: selector 2, 4 bits 11),
        // intent relative (2 bits 1).
        let mut bits: Vec<(u32, u32)> = vec![(0, 1), (0, 1), (0, 2), (1, 2), (2, 2), (9, 4)];
        bits.extend([(0, 1), (2, 2), (11, 4), (1, 2)]);
        let mut v = 0u64;
        let mut n = 0;
        for (val, w) in bits {
            v |= u64::from(val) << n;
            n += w;
        }
        let bytes = v.to_le_bytes()[..(n as usize).div_ceil(8)].to_vec();
        let cs = [0xff, 0x0a];
        let b = bundle(b"", &bytes, b"", &cs);
        assert_eq!(
            parse_gain_map_bundle(&b).unwrap().alt_primaries(),
            Some(Primaries::DisplayP3)
        );
        // want_icc: no primaries.
        let b = bundle(b"", &[0b10], b"", &cs);
        assert_eq!(parse_gain_map_bundle(&b).unwrap().alt_primaries(), None);
        // Cut short: no primaries, no panic.
        let b = bundle(b"", &bytes[..1], b"", &cs);
        assert_eq!(parse_gain_map_bundle(&b).unwrap().alt_primaries(), None);
    }

    #[test]
    fn box_walk() {
        let sig = [
            0, 0, 0, 0x0c, b'J', b'X', b'L', b' ', 0x0d, 0x0a, 0x87, 0x0a,
        ];
        let mut f = sig.to_vec();
        f.extend_from_slice(&[0, 0, 0, 12, b'f', b't', b'y', b'p', 1, 2, 3, 4]);
        let at = f.len();
        f.extend_from_slice(&[0, 0, 0, 10, b'j', b'h', b'g', b'm', 7, 8]);
        assert_eq!(find_gain_map(&f), Found::Box(at + 8..at + 10));
        // A size-0 (to the end) box.
        let mut g = f[..at].to_vec();
        g.extend_from_slice(&[0, 0, 0, 0, b'j', b'h', b'g', b'm', 7, 8, 9]);
        assert_eq!(find_gain_map(&g), Found::Box(at + 8..at + 11));
        // A 64-bit size.
        let mut g = f[..at].to_vec();
        g.extend_from_slice(&[
            0, 0, 0, 1, b'j', b'h', b'g', b'm', 0, 0, 0, 0, 0, 0, 0, 17, 5,
        ]);
        assert_eq!(find_gain_map(&g), Found::Box(at + 16..at + 17));
        assert!(matches!(
            find_gain_map(&f[..f.len() - 1]),
            Found::Unreadable(_)
        ));
        let mut b = f[..at].to_vec();
        b.extend_from_slice(&[
            0, 0, 0, 14, b'b', b'r', b'o', b'b', b'j', b'h', b'g', b'm', 0, 0,
        ]);
        assert_eq!(find_gain_map(&b), Found::Brotli(at + 12..at + 14));
        assert_eq!(find_gain_map(&[0xff, 0x0a, 0, 0]), Found::None);
        assert_eq!(find_gain_map(&sig), Found::None);
    }
}
