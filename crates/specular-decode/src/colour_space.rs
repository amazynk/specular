//! A file's colour space as one short identity, for search and the gallery's value lists:
//! a canonical word for the well-known spaces ([`SPACES`]), else the description of its ICC
//! profile. Read from what the session has already parsed (its signalling, the profile's
//! header and a few tags, the Exif colour space tags); nothing is decoded.

use specular_color::math::{D65_XY, Mat3, rgb_to_pcs};

use crate::Session;

/// The canonical words, in the order lists show them: sRGB (and BT.709), Display P3, Adobe RGB
/// (1998), ProPhoto RGB (ROMM), BT.2020 primaries, BT.2100 PQ and HLG, greyscale, CMYK and
/// CIE L*a*b*.
pub const SPACES: [&str; 10] = [
    "srgb", "p3", "adobergb", "prophoto", "rec2020", "pq", "hlg", "grey", "cmyk", "lab",
];

/// Profile descriptions are kept to this many characters.
const DESCRIPTION_MAX: usize = 64;

/// The colour space of an open session (`exif`: its Exif TIFF structure, when it has one):
/// signalling that names a space and wins over a profile (PNG `cICP`, HDR `nclx`, JPEG XL enum
/// encodings), else the ICC profile, else the stored colour model (greyscale, CMYK,
/// L*a*b*), else the Exif ColorSpace / interoperability index. Empty when nothing names one.
pub fn colour_space(s: &dyn Session, exif: Option<&[u8]>) -> String {
    if let Some(w) = s.colour_space() {
        return w.to_string();
    }
    let icc = s
        .embedded_icc()
        .or_else(|| s.info().icc_profile.as_deref().map(Vec::as_slice));
    if let Some(name) = icc.and_then(icc_space) {
        return name;
    }
    let model = match s.colour_model() {
        Some("Greyscale") => Some("grey"),
        Some("CMYK" | "YCCK") => Some("cmyk"),
        Some("Lab") => Some("lab"),
        _ => None,
    };
    model
        .or_else(|| exif.and_then(crate::exif::exif_colour_space))
        .unwrap_or_default()
        .to_string()
}

/// The space CICP code points name: PQ and HLG by their transfer, else the primaries
/// (BT.709 counts as sRGB); linear light by a description. None for others and unspecified ones.
pub(crate) fn cicp_space(primaries: u16, transfer: u16) -> Option<&'static str> {
    match (primaries, transfer) {
        (_, 16) => Some("pq"),
        (_, 18) => Some("hlg"),
        // Linear light is not the named space; it is named for what it is.
        (1, 8) => Some("Linear sRGB"),
        (12, 8) => Some("Linear Display P3"),
        (9, 8) => Some("Linear Rec. 2020"),
        (_, 8) => None,
        (1, _) => Some("srgb"),
        (12, _) => Some("p3"),
        (9, _) => Some("rec2020"),
        _ => None,
    }
}

/// The identity of an ICC profile: grey, CMYK and L*a*b* by its data colour space; an RGB
/// profile by its `cicp` tag, its colorants or its description's naming; else its
/// description (shortened). None for a profile too short or without a description.
pub fn icc_space(icc: &[u8]) -> Option<String> {
    let space = icc.get(16..20)?;
    match space {
        b"GRAY" => return Some("grey".into()),
        b"CMYK" => return Some("cmyk".into()),
        b"Lab " => return Some("lab".into()),
        _ => {}
    }
    let tags = Tags::read(icc)?;
    let desc = tags.get(b"desc").and_then(description);
    let linear = desc
        .as_deref()
        .is_some_and(|d| d.to_ascii_lowercase().contains("linear"));
    if space == b"RGB " && !linear {
        if let Some(c) = tags
            .get(b"cicp")
            .filter(|c| c.len() >= 12 && &c[..4] == b"cicp")
            && let Some(w) = cicp_space(u16::from(c[8]), u16::from(c[9]))
        {
            return Some(w.into());
        }
        let col = |sig: &[u8; 4]| tags.get(sig).and_then(xyz);
        if let (Some(r), Some(g), Some(b)) = (col(b"rXYZ"), col(b"gXYZ"), col(b"bXYZ"))
            && let Some(w) = by_colorants([r, g, b])
        {
            return Some(w.into());
        }
        if let Some(w) = desc.as_deref().and_then(by_name) {
            return Some(w.into());
        }
    }
    desc.map(|d| shorten(&d))
}

/// The tag table of a profile.
struct Tags<'a> {
    icc: &'a [u8],
    count: usize,
}

impl<'a> Tags<'a> {
    fn read(icc: &'a [u8]) -> Option<Tags<'a>> {
        let count = be32(icc, 128)? as usize;
        Some(Tags {
            icc,
            count: count.min(256),
        })
    }

    /// A tag's data (from its type signature on).
    fn get(&self, sig: &[u8; 4]) -> Option<&'a [u8]> {
        (0..self.count).find_map(|i| {
            let e = 132 + i * 12;
            if self.icc.get(e..e + 4)? != sig {
                return None;
            }
            let off = be32(self.icc, e + 4)? as usize;
            let len = be32(self.icc, e + 8)? as usize;
            self.icc.get(off..off.checked_add(len)?)
        })
    }
}

fn be32(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

/// An `XYZ ` tag's first value.
fn xyz(t: &[u8]) -> Option<[f64; 3]> {
    if t.get(..4)? != b"XYZ " {
        return None;
    }
    let s15 = |at: usize| be32(t, at).map(|v| f64::from(v as i32) / 65536.0);
    Some([s15(8)?, s15(12)?, s15(16)?])
}

/// A description tag's text: `desc` (ICC v2 ASCII), `mluc` (v4: the English record, else the
/// first) or `text`.
fn description(t: &[u8]) -> Option<String> {
    let text = match t.get(..4)? {
        b"desc" => {
            let n = be32(t, 8)? as usize;
            let b = t.get(12..12usize.checked_add(n)?)?;
            b.iter()
                .take_while(|&&c| c != 0)
                .map(|&c| c as char)
                .collect()
        }
        b"text" => t[8.min(t.len())..]
            .iter()
            .take_while(|&&c| c != 0)
            .map(|&c| c as char)
            .collect(),
        b"mluc" => {
            let records = (be32(t, 8)? as usize).min(64);
            let size = be32(t, 12)? as usize;
            let record = |i: usize| {
                let r = 16 + i * size;
                Some((t.get(r..r + 2)?, be32(t, r + 4)?, be32(t, r + 8)?))
            };
            let pick = (0..records)
                .find(|&i| record(i).is_some_and(|r| r.0 == b"en"))
                .unwrap_or(0);
            let (_, len, off) = record(pick)?;
            let b = t.get(off as usize..(off as usize).checked_add(len as usize)?)?;
            let units: Vec<u16> = b
                .chunks_exact(2)
                .map(|c| u16::from_be_bytes([c[0], c[1]]))
                .take_while(|&u| u != 0)
                .collect();
            String::from_utf16_lossy(&units)
        }
        _ => return None,
    };
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    (!text.is_empty()).then_some(text)
}

/// At most [`DESCRIPTION_MAX`] characters and 192 bytes (caches keep up to 255).
fn shorten(d: &str) -> String {
    let mut bytes = 0;
    d.chars()
        .take(DESCRIPTION_MAX)
        .take_while(|c| {
            bytes += c.len_utf8();
            bytes <= 192
        })
        .collect::<String>()
        .trim()
        .to_string()
}

/// The known RGB spaces' colorants (XYZ, D50-adapted): within 0.015 in every component.
fn by_colorants(c: [[f64; 3]; 3]) -> Option<&'static str> {
    const D50_XY: [f64; 2] = [0.3457, 0.3585];
    let known: [(&str, Mat3); 5] = [
        (
            "srgb",
            rgb_to_pcs([0.64, 0.33], [0.30, 0.60], [0.15, 0.06], D65_XY),
        ),
        (
            "p3",
            rgb_to_pcs([0.680, 0.320], [0.265, 0.690], [0.150, 0.060], D65_XY),
        ),
        (
            "adobergb",
            rgb_to_pcs([0.64, 0.33], [0.21, 0.71], [0.15, 0.06], D65_XY),
        ),
        (
            "prophoto",
            rgb_to_pcs([0.7347, 0.2653], [0.1596, 0.8404], [0.0366, 0.0001], D50_XY),
        ),
        (
            "rec2020",
            rgb_to_pcs([0.708, 0.292], [0.170, 0.797], [0.131, 0.046], D65_XY),
        ),
    ];
    known.into_iter().find_map(|(w, m)| {
        let close = (0..3).all(|col| (0..3).all(|row| (m.0[row][col] - c[col][row]).abs() < 0.015));
        close.then_some(w)
    })
}

/// A word for a description that names a known space (for LUT profiles without colorants).
fn by_name(d: &str) -> Option<&'static str> {
    let k: String = d
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect::<String>()
        .to_ascii_lowercase();
    Some(if k.contains("srgb") || k.contains("iec61966") {
        "srgb"
    } else if k.contains("adobergb") || k.contains("oprgb") {
        "adobergb"
    } else if k.contains("prophoto") || k.contains("romm") {
        "prophoto"
    } else if k.contains("2100pq") {
        "pq"
    } else if k.contains("2100hlg") {
        "hlg"
    } else if k.contains("2020") {
        "rec2020"
    } else if k.contains("displayp3") || k.contains("p3d65") {
        "p3"
    } else {
        return None;
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use specular_color::math::bt709_to_pcs;
    use specular_color::profiles::matrix_shaper_icc;

    #[test]
    fn colorants_name_the_known_spaces() {
        let icc = |m: &Mat3, d: &str| matrix_shaper_icc(m, 2.2, d);
        let d65 = D65_XY;
        assert_eq!(
            icc_space(&icc(&bt709_to_pcs(), "x")).as_deref(),
            Some("srgb")
        );
        let p3 = rgb_to_pcs([0.680, 0.320], [0.265, 0.690], [0.150, 0.060], d65);
        assert_eq!(icc_space(&icc(&p3, "Display P3")).as_deref(), Some("p3"));
        let adobe = rgb_to_pcs([0.64, 0.33], [0.21, 0.71], [0.15, 0.06], d65);
        assert_eq!(
            icc_space(&icc(&adobe, "whatever")).as_deref(),
            Some("adobergb")
        );
        let bt2020 = rgb_to_pcs([0.708, 0.292], [0.170, 0.797], [0.131, 0.046], d65);
        assert_eq!(icc_space(&icc(&bt2020, "")).as_deref(), Some("rec2020"));
        // Published ProPhoto colorants (as in Kodak's and Adobe's profiles).
        let pro = Mat3([
            [0.7977, 0.1352, 0.0313],
            [0.2880, 0.7119, 0.0001],
            [0.0000, 0.0000, 0.8249],
        ]);
        assert_eq!(
            icc_space(&icc(&pro, "ProPhoto")).as_deref(),
            Some("prophoto")
        );
        // Published sRGB colorants (HP's sRGB IEC61966-2.1).
        let hp = Mat3([
            [0.4361, 0.3851, 0.1431],
            [0.2225, 0.7169, 0.0606],
            [0.0139, 0.0971, 0.7141],
        ]);
        assert_eq!(
            icc_space(&icc(&hp, "sRGB IEC61966-2.1")).as_deref(),
            Some("srgb")
        );
    }

    #[test]
    fn other_profiles_by_description() {
        // ColorMatch RGB's primaries: not a known space, so its description.
        let m = rgb_to_pcs(
            [0.630, 0.340],
            [0.295, 0.605],
            [0.150, 0.075],
            [0.3457, 0.3585],
        );
        let icc = matrix_shaper_icc(&m, 1.8, "  ColorMatch   RGB ");
        assert_eq!(icc_space(&icc).as_deref(), Some("ColorMatch RGB"));
        // Linear sRGB is not sRGB.
        let icc = matrix_shaper_icc(&bt709_to_pcs(), 1.0, "sRGB linear");
        assert_eq!(icc_space(&icc).as_deref(), Some("sRGB linear"));
        let long = "x".repeat(100);
        let icc = matrix_shaper_icc(&m, 1.8, &long);
        assert_eq!(icc_space(&icc).map(|d| d.len()), Some(DESCRIPTION_MAX));
        assert_eq!(by_name("Rec. 2100 PQ"), Some("pq"));
        assert_eq!(by_name("ITU-R BT.2020"), Some("rec2020"));
        assert_eq!(by_name("Generic RGB"), None);
    }

    #[test]
    fn header_spaces_and_cicp() {
        let mut icc = matrix_shaper_icc(&bt709_to_pcs(), 2.2, "Dot Gain 20%");
        icc[16..20].copy_from_slice(b"GRAY");
        assert_eq!(icc_space(&icc).as_deref(), Some("grey"));
        icc[16..20].copy_from_slice(b"CMYK");
        assert_eq!(icc_space(&icc).as_deref(), Some("cmyk"));
        assert_eq!(icc_space(&icc[..10]), None);
        assert_eq!(cicp_space(1, 13), Some("srgb"));
        assert_eq!(cicp_space(9, 16), Some("pq"));
        assert_eq!(cicp_space(9, 18), Some("hlg"));
        assert_eq!(cicp_space(12, 13), Some("p3"));
        assert_eq!(cicp_space(1, 8), Some("Linear sRGB"));
        assert_eq!(cicp_space(2, 8), None);
        assert_eq!(cicp_space(2, 2), None);
    }
}
