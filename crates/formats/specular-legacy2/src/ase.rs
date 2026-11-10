//! Adobe Swatch Exchange (`.ase`), shown as a swatch sheet.
//!
//! Big-endian: `ASEF`, version 1.0, a block count, then blocks of a type word (0x0001 a
//! colour, 0xC001 a group start, 0xC002 a group end) and a length. Colour and group blocks
//! start with a name (a count of UTF-16 units including the NUL, then the units); a colour
//! then has its model (`RGB `, `CMYK`, `LAB `, `Gray`), its values as 32-bit floats and a
//! type word (0 global, 1 spot, 2 process).
//!
//! RGB, CMYK and grey values are 0–1 (CMYK: 1 = full ink, grey: 1 = white); Lab is L 0–1
//! (for 0–100), a and b in their own units.

use crate::swatch::{MAX_COLOURS, Palette, Swatch, cmyk_to_rgb, lab_to_rgb, short, unit8, utf16be};
use crate::{Error, Image, Info, Result};

pub const MAGIC: &[u8; 4] = b"ASEF";

pub fn probe(head: &[u8]) -> bool {
    head.len() >= 8 && head.starts_with(MAGIC) && head[4..6] == [0, 1]
}

fn u16be(d: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_be_bytes(d.get(o..o + 2)?.try_into().ok()?))
}

fn u32be(d: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_be_bytes(d.get(o..o + 4)?.try_into().ok()?))
}

fn f32be(d: &[u8], o: usize) -> Option<f64> {
    u32be(d, o).map(|b| f64::from(f32::from_bits(b)))
}

/// A block's name and where its body continues.
fn name(b: &[u8]) -> Option<(String, usize)> {
    let n = u16be(b, 0)? as usize;
    let s = b.get(2..2 + n * 2)?;
    Some((short(&utf16be(s)), 2 + n * 2))
}

fn colour(b: &[u8], at: usize) -> Option<([u8; 3], String, u16)> {
    let model = b.get(at..at + 4)?;
    let v = |k: usize| f32be(b, at + 4 + k * 4);
    let (rgb, stored, n) = match model {
        b"RGB " => {
            let (r, g, bl) = (v(0)?, v(1)?, v(2)?);
            ([unit8(r), unit8(g), unit8(bl)], String::new(), 3)
        }
        b"CMYK" => {
            let (c, m, y, k) = (v(0)?, v(1)?, v(2)?, v(3)?);
            (
                cmyk_to_rgb(c, m, y, k),
                format!(
                    "CMYK {:.0} {:.0} {:.0} {:.0} %",
                    c * 100.0,
                    m * 100.0,
                    y * 100.0,
                    k * 100.0
                ),
                4,
            )
        }
        b"LAB " => {
            let (l, a, bb) = (v(0)? * 100.0, v(1)?, v(2)?);
            (
                lab_to_rgb(l, a, bb),
                format!("Lab {l:.1} {a:.1} {bb:.1}"),
                3,
            )
        }
        b"Gray" => {
            let g = unit8(v(0)?);
            ([g, g, g], format!("Gray {:.0} %", v(0)? * 100.0), 1)
        }
        m => (
            [128; 3],
            format!(
                "colour model {}, shown grey",
                String::from_utf8_lossy(m).trim()
            ),
            0,
        ),
    };
    let kind = u16be(b, at + 4 + n * 4).unwrap_or(2);
    Some((rgb, stored, kind))
}

pub fn parse(data: &[u8]) -> Result<Palette> {
    if !probe(data) {
        return Err(Error::NotRecognised);
    }
    let blocks = u32be(data, 8).ok_or_else(|| Error::Truncated("header".into()))?;
    let mut at = 12;
    let mut group = String::new();
    let mut groups = 0usize;
    let mut spot = 0usize;
    let mut swatches = Vec::new();
    for _ in 0..blocks {
        let (Some(kind), Some(len)) = (u16be(data, at), u32be(data, at + 2)) else {
            break;
        };
        let body = data
            .get(at + 6..(at + 6).saturating_add(len as usize))
            .ok_or_else(|| Error::Truncated("swatch block".into()))?;
        at = at + 6 + len as usize;
        match kind {
            0xC001 => {
                group = name(body).map(|n| n.0).unwrap_or_default();
                groups += 1;
            }
            0xC002 => group.clear(),
            0x0001 => {
                let (nm, p) = name(body).ok_or_else(|| Error::Malformed("colour name".into()))?;
                let (rgb, mut stored, t) =
                    colour(body, p).ok_or_else(|| Error::Truncated("colour values".into()))?;
                if t == 1 {
                    spot += 1;
                    stored = if stored.is_empty() {
                        "spot".into()
                    } else {
                        format!("{stored}, spot")
                    };
                }
                if swatches.len() >= MAX_COLOURS {
                    return Err(Error::Malformed("more than 65536 colours".into()));
                }
                swatches.push(Swatch {
                    name: nm,
                    rgba: [rgb[0], rgb[1], rgb[2], 255],
                    stored,
                    group: group.clone(),
                });
            }
            // Unknown blocks are skipped by their length.
            _ => {}
        }
    }
    let mut fields = Vec::new();
    if groups > 0 {
        fields.push(("Groups".into(), groups.to_string()));
    }
    if spot > 0 {
        fields.push(("Spot colours".into(), spot.to_string()));
    }
    Ok(Palette {
        name: String::new(),
        swatches,
        columns: 0,
        fields,
        coding: "Adobe Swatch Exchange".into(),
    })
}

pub fn info(data: &[u8], _page: u32) -> Result<Info> {
    parse(data)?.info()
}

pub fn decode(data: &[u8], _page: u32) -> Result<Image> {
    parse(data)?.render()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    fn name_bytes(s: &str) -> Vec<u8> {
        let u: Vec<u16> = s.encode_utf16().chain([0]).collect();
        let mut d = (u.len() as u16).to_be_bytes().to_vec();
        for c in u {
            d.extend_from_slice(&c.to_be_bytes());
        }
        d
    }

    /// A file with one group of the colours given: (name, model, values, type).
    pub(crate) fn file(colours: &[(&str, &[u8; 4], &[f32], u16)]) -> Vec<u8> {
        let mut blocks: Vec<(u16, Vec<u8>)> = vec![(0xC001, name_bytes("Group"))];
        for (n, model, vals, t) in colours {
            let mut b = name_bytes(n);
            b.extend_from_slice(*model);
            for v in *vals {
                b.extend_from_slice(&v.to_bits().to_be_bytes());
            }
            b.extend_from_slice(&t.to_be_bytes());
            blocks.push((1, b));
        }
        blocks.push((0xC002, Vec::new()));
        let mut d = b"ASEF\x00\x01\x00\x00".to_vec();
        d.extend_from_slice(&(blocks.len() as u32).to_be_bytes());
        for (k, b) in blocks {
            d.extend_from_slice(&k.to_be_bytes());
            d.extend_from_slice(&(b.len() as u32).to_be_bytes());
            d.extend_from_slice(&b);
        }
        d
    }

    #[test]
    fn reads_groups_models_and_spots() {
        let f = file(&[
            ("Red", b"RGB ", &[1.0, 0.0, 0.0], 2),
            ("Cyan", b"CMYK", &[1.0, 0.0, 0.0, 0.0], 1),
            ("Grey", b"Gray", &[0.5], 0),
            ("White", b"LAB ", &[1.0, 0.0, 0.0], 0),
        ]);
        let p = parse(&f).unwrap();
        assert_eq!(p.swatches.len(), 4);
        assert_eq!(p.swatches[0].rgba, [255, 0, 0, 255]);
        assert_eq!(p.swatches[0].group, "Group");
        assert_eq!(p.swatches[1].rgba, [0, 255, 255, 255]);
        assert!(p.swatches[1].stored.ends_with("spot"));
        assert_eq!(p.swatches[2].rgba, [128, 128, 128, 255]);
        assert_eq!(p.swatches[3].rgba, [255, 255, 255, 255]);
        assert!(p.fields.contains(&("Spot colours".into(), "1".into())));
        assert!(matches!(parse(&f[..40]), Err(Error::Truncated(_))));
    }
}
