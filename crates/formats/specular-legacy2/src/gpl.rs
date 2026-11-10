//! GIMP palettes (`.gpl`), shown as a swatch sheet.
//!
//! Text: the line `GIMP Palette`, then optional `Name:` and `Columns:` lines, `#` comments,
//! and one colour per line: red, green and blue (0–255) and an optional name. Inkscape,
//! Krita and Aseprite write the same format.

use crate::swatch::{MAX_COLOURS, Palette, Swatch, short};
use crate::{Error, Image, Info, Result};

const MAGIC: &[u8] = b"GIMP Palette";

pub fn probe(head: &[u8]) -> bool {
    let head = head.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(head);
    head.starts_with(MAGIC)
}

pub fn parse(data: &[u8]) -> Result<Palette> {
    if !probe(data) {
        return Err(Error::NotRecognised);
    }
    let text = String::from_utf8_lossy(data);
    let mut p = Palette {
        coding: "GIMP palette".into(),
        ..Palette::default()
    };
    for line in text.lines().skip(1) {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(v) = line.strip_prefix("Name:") {
            p.name = short(v);
            continue;
        }
        if let Some(v) = line.strip_prefix("Columns:") {
            p.columns = v.trim().parse::<u32>().unwrap_or(0).min(256);
            continue;
        }
        let mut it = line.split_whitespace();
        let mut rgb = [0u8; 3];
        let mut ok = true;
        for c in &mut rgb {
            match it.next().and_then(|v| v.parse::<i64>().ok()) {
                Some(v) => *c = v.clamp(0, 255) as u8,
                None => ok = false,
            }
        }
        if !ok {
            // A line that is not a colour (a newer key GIMP may add): skipped.
            continue;
        }
        let name: Vec<&str> = it.collect();
        if p.swatches.len() >= MAX_COLOURS {
            return Err(Error::Malformed("more than 65536 colours".into()));
        }
        p.swatches
            .push(Swatch::rgb(short(&name.join(" ")), rgb[0], rgb[1], rgb[2]));
    }
    if p.columns > 0 {
        p.fields.push(("Columns".into(), p.columns.to_string()));
    }
    Ok(p)
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
    fn reads_names_columns_and_colours() {
        let f = b"GIMP Palette\nName: Test\nColumns: 2\n#\n255 0 0 Red\n  0 255   0\tGreen one\nbad line\n0 0 255\n";
        let p = parse(f).unwrap();
        assert_eq!(p.name, "Test");
        assert_eq!(p.columns, 2);
        assert_eq!(p.swatches.len(), 3);
        assert_eq!(p.swatches[1].name, "Green one");
        assert_eq!(p.swatches[2].rgba, [0, 0, 255, 255]);
        let i = info(f, 0).unwrap();
        assert_eq!((i.width, i.height), (68, 68));
        assert!(matches!(
            parse(b"GIMP Palette\n"),
            Ok(p) if p.swatches.is_empty()
        ));
        assert!(info(b"GIMP Palette\n", 0).is_err());
    }
}
