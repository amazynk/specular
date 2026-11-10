//! Adobe colour tables (`.act`), shown as a swatch sheet.
//!
//! 256 RGB triples (768 bytes), optionally followed by the number of colours in use and the
//! transparent colour's index (big-endian words; 0xFFFF for none). There is no signature:
//! a file is one only by its exact size, 768 or 772 bytes.

use crate::swatch::{Palette, Swatch};
use crate::{Error, Image, Info, Result};

/// Exactly 768 bytes, or 772 with a sensible count and transparent index.
pub fn probe(data: &[u8]) -> bool {
    match data.len() {
        768 => true,
        772 => {
            let n = u16::from_be_bytes([data[768], data[769]]);
            let t = u16::from_be_bytes([data[770], data[771]]);
            n <= 256 && (t < 256 || t == 0xFFFF)
        }
        _ => false,
    }
}

pub fn parse(data: &[u8]) -> Result<Palette> {
    if !probe(data) {
        return Err(Error::Unsupported(format!(
            "an Adobe colour table is 768 or 772 bytes; this file has {}",
            data.len()
        )));
    }
    let (count, transparent) = if data.len() == 772 {
        let n = u16::from_be_bytes([data[768], data[769]]);
        let t = u16::from_be_bytes([data[770], data[771]]);
        (if n == 0 { 256 } else { n as usize }, t)
    } else {
        (256, 0xFFFF)
    };
    let swatches: Vec<Swatch> = data[..768]
        .chunks_exact(3)
        .take(count)
        .enumerate()
        .map(|(i, c)| {
            let mut s = Swatch::rgb("", c[0], c[1], c[2]);
            if i == transparent as usize {
                s.rgba[3] = 0;
            }
            s
        })
        .collect();
    let mut fields = Vec::new();
    if data.len() == 772 {
        fields.push(("Colours in use".into(), count.to_string()));
        if transparent != 0xFFFF {
            fields.push(("Transparent index".into(), transparent.to_string()));
        }
    }
    Ok(Palette {
        name: String::new(),
        swatches,
        columns: 16,
        fields,
        coding: "Adobe colour table".into(),
    })
}

pub fn info(data: &[u8], _page: u32) -> Result<Info> {
    parse(data)?.info()
}

pub fn decode(data: &[u8], _page: u32) -> Result<Image> {
    parse(data)?.render()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_count_and_transparency() {
        let mut d: Vec<u8> = (0..768).map(|i| (i % 256) as u8).collect();
        assert!(probe(&d));
        assert_eq!(parse(&d).unwrap().swatches.len(), 256);
        d.extend_from_slice(&[0, 4, 0, 1]);
        let p = parse(&d).unwrap();
        assert_eq!(p.swatches.len(), 4);
        assert_eq!(p.swatches[1].rgba[3], 0);
        assert_eq!(p.swatches[2].rgba, [6, 7, 8, 255]);
        assert!(matches!(parse(&d[..700]), Err(Error::Unsupported(_))));
    }
}
