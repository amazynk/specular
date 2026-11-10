//! Digital Surf `.sur` / `.pro` (MountainsMap): a run of objects, each a 512-byte
//! little-endian header, a comment, a private zone and its points (16- or 32-bit integers,
//! `DSCOMPRESSED` files zlib-deflated in streams).
//!
//! Consecutive objects of one size become the planes of one page: a surface with its
//! intensity, the R, G and B layers of a colour image (shown as colour, 0..=255 scaled to
//! 0..=1), the Z, R, G, B and quality layers of a colour surface. Every layer's value is
//! `(raw − zmin) × dz + zoffset` in its Z unit (lengths converted to metres); points below
//! `zmin` in an object with non-measured points are NaN. Rows are in file order, the first
//! at the top.
//! Spectra and hyperspectral maps are refused.

use specular_em::dataset::{Dataset, Page, Plane};
use specular_em::samples::Sample;

use crate::{Error, Result, alloc, area, f32le, i16le, i32le, meta, page, slice, text};

const HEADER: usize = 512;

pub fn probe(head: &[u8]) -> bool {
    head.starts_with(b"DIGITAL SURF") || head.starts_with(b"DSCOMPRESSED")
}

/// Metres per unit of a length unit name; `None` for anything else ("a.u.", counts).
fn metres(unit: &str) -> Option<f64> {
    Some(match unit {
        "m" => 1.0,
        "cm" => 1e-2,
        "mm" => 1e-3,
        "\u{b5}m" | "\u{3bc}m" | "um" => 1e-6,
        "nm" => 1e-9,
        "pm" => 1e-12,
        "Å" | "A" => 1e-10,
        _ => return None,
    })
}

struct Object {
    kind: i16,
    name: String,
    width: usize,
    height: usize,
    label: String,
    unit_x: String,
    unit_y: String,
    unit_z: String,
    dx: f64,
    dy: f64,
    dz: f64,
    x0: f64,
    y0: f64,
    z0: f64,
    zmin: i32,
    special: bool,
    bits: i16,
    date: String,
    comment: String,
    values: Vec<f32>,
}

fn object(file: &[u8], at: usize) -> Result<(Object, usize)> {
    let h = slice(file, at, HEADER, "object header")?;
    let compressed = h.starts_with(b"DSCOMPRESSED");
    if !probe(h) {
        return Err(Error::NotFormat("Digital Surf"));
    }
    let g16 = |o| i16le(h, o).unwrap_or(0);
    let g32 = |o| i32le(h, o).unwrap_or(0);
    let gf = |o| f64::from(f32le(h, o).unwrap_or(0.0));
    let kind = g16(18);
    if matches!(kind, 20 | 21) {
        return Err(Error::Unsupported("Digital Surf spectra".into()));
    }
    if !matches!(kind, 1 | 2 | 3 | 10 | 11 | 12 | 13 | 16) {
        return Err(Error::Unsupported(format!(
            "Digital Surf object type {kind}"
        )));
    }
    let bits = g16(98);
    if !matches!(bits, 16 | 32) {
        return Err(Error::Malformed(format!("{bits}-bit points")));
    }
    let (w, hgt, n) = (g32(108), g32(112), g32(116));
    if w <= 0 || hgt <= 0 {
        return Err(Error::NoImage);
    }
    let (w, hgt) = (w as usize, hgt as usize);
    let count = area(w, hgt)?;
    if n as i64 != count as i64 {
        return Err(Error::Malformed(format!("{n} points for {w} × {hgt}")));
    }
    let comment_size = g16(334).max(0) as usize;
    let private_size = g16(336).max(0) as usize;
    let comment = text(
        slice(file, at + HEADER, comment_size, "comment")?,
        0,
        comment_size,
    );
    let mut p = at + HEADER + comment_size + private_size;
    let bytes = count * usize::from(bits.unsigned_abs()) / 8;
    let raw;
    let data: &[u8] = if compressed {
        let streams = crate::u32le(file, p).ok_or(Error::Truncated("stream count"))? as usize;
        p += 4;
        let mut out = Vec::new();
        out.try_reserve_exact(bytes).map_err(|_| Error::TooLarge)?;
        for _ in 0..streams {
            let head = slice(file, p, 8, "stream header")?;
            let packed = crate::u32le(head, 4).unwrap_or(0) as usize;
            let z = slice(file, p + 8, packed, "compressed stream")?;
            let room = bytes - out.len();
            let mut part = specular_png::inflate_zlib(z, room)
                .map_err(|e| Error::Malformed(format!("compressed points: {e:?}")))?;
            out.append(&mut part);
            p += 8 + packed;
        }
        if out.len() < bytes {
            return Err(Error::Truncated("compressed points"));
        }
        raw = out;
        &raw
    } else {
        let d = slice(file, p, bytes, "points")?;
        p += bytes;
        d
    };
    let unit = |o| text(h, o, 16);
    let (unit_x, unit_y, unit_z) = (unit(180), unit(196), unit(212));
    let zmin = g32(100);
    let special = g16(86) == 1;
    let dz = gf(128);
    let z0 = gf(474);
    let scale = metres(&unit_z);
    let mut values = alloc(count)?;
    let size = usize::from(bits.unsigned_abs()) / 8;
    values.extend(data[..bytes].chunks_exact(size).map(|c| {
        let r = if size == 2 {
            i32::from(i16le(c, 0).unwrap_or(0))
        } else {
            i32le(c, 0).unwrap_or(0)
        };
        if special && r < zmin {
            f32::NAN
        } else {
            (((f64::from(r) - f64::from(zmin)) * dz + z0) * scale.unwrap_or(1.0)) as f32
        }
    }));
    let d = |o| g16(o);
    let date = if d(316) > 0 {
        format!(
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
            d(316),
            d(314),
            d(312),
            d(310),
            d(308),
            d(306)
        )
    } else {
        String::new()
    };
    let obj = Object {
        kind,
        name: text(h, 20, 30),
        width: w,
        height: hgt,
        label: text(h, 164, 16),
        unit_x,
        unit_y,
        unit_z,
        dx: gf(120),
        dy: gf(124),
        dz,
        x0: gf(466),
        y0: gf(470),
        z0,
        zmin,
        special,
        bits,
        date,
        comment,
        values,
    };
    Ok((obj, p))
}

fn start(o: &Object) -> Page {
    let mx = metres(&o.unit_x).unwrap_or(1.0);
    let my = metres(&o.unit_y).unwrap_or(1.0);
    let mut pg = page(o.width, o.height, o.dx * mx, o.dy * my);
    if let Some(x) = pg.x.as_mut() {
        x.offset = o.x0 * mx;
        if metres(&o.unit_x).is_none() {
            x.unit = o.unit_x.clone();
        }
    }
    if let Some(y) = pg.y.as_mut() {
        y.offset = o.y0 * my;
        if metres(&o.unit_y).is_none() {
            y.unit = o.unit_y.clone();
        }
    }
    let name = o.name.trim_matches(|c: char| c == '*' || c.is_whitespace());
    pg.title = (!name.is_empty()).then(|| name.to_string());
    pg.date = (!o.date.is_empty()).then(|| o.date.clone());
    pg.sample = Some(if o.bits == 16 {
        Sample::I16
    } else {
        Sample::I32
    });
    pg.levelling = matches!(o.kind, 1 | 2 | 11 | 13 | 16) && metres(&o.unit_z).is_some();
    pg.rgb = o.kind == 12;
    meta(&mut pg, "Object type", o.kind.to_string());
    meta(&mut pg, "Z step", format!("{} {}", o.dz, o.unit_z));
    meta(&mut pg, "Z offset", format!("{} {}", o.z0, o.unit_z));
    meta(&mut pg, "Z minimum", o.zmin.to_string());
    meta(&mut pg, "Non-measured points", o.special.to_string());
    meta(&mut pg, "Comment", o.comment.clone());
    pg
}

/// Reads every object; objects of one size following each other share a page.
pub fn read(file: &[u8]) -> Result<Dataset> {
    if !probe(file) {
        return Err(Error::NotFormat("Digital Surf"));
    }
    let mut pages: Vec<Page> = Vec::new();
    let mut at = 0;
    while at + HEADER <= file.len() && probe(&file[at..]) {
        let (mut o, next) = match object(file, at) {
            Ok(v) => v,
            // A damaged trailing object leaves the ones before it readable.
            Err(e) if pages.is_empty() => return Err(e),
            Err(_) => break,
        };
        at = next;
        let same = pages
            .last()
            .is_some_and(|p| p.width == o.width && p.height == o.height);
        if !same {
            pages.push(start(&o));
        }
        let pg = pages.last_mut().expect("page");
        if pg.rgb {
            // The R, G and B layers of a colour image, shown as colour in 0..=1.
            for v in o.values.iter_mut() {
                *v /= 255.0;
            }
        }
        let (label, unit) = match metres(&o.unit_z) {
            Some(_) => (
                if o.label.is_empty() || o.label == "Z" {
                    "Height".to_string()
                } else {
                    o.label.clone()
                },
                "m".to_string(),
            ),
            None => (o.label.clone(), o.unit_z.clone()),
        };
        pg.planes
            .push(Plane::values(label, unit, std::mem::take(&mut o.values)));
    }
    for p in &mut pages {
        if p.rgb && p.planes.len() < 3 {
            p.rgb = false;
        }
    }
    if pages.is_empty() {
        return Err(Error::NoImage);
    }
    Ok(Dataset { pages })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(kind: i16, w: i32, h: i32, bits: i16, zmin: i32, special: bool) -> Vec<u8> {
        let mut b = vec![0u8; HEADER];
        b[..12].copy_from_slice(b"DIGITAL SURF");
        b[18..20].copy_from_slice(&kind.to_le_bytes());
        b[20..27].copy_from_slice(b"* DVD *");
        b[86..88].copy_from_slice(&i16::from(special).to_le_bytes());
        b[98..100].copy_from_slice(&bits.to_le_bytes());
        b[100..104].copy_from_slice(&zmin.to_le_bytes());
        b[108..112].copy_from_slice(&w.to_le_bytes());
        b[112..116].copy_from_slice(&h.to_le_bytes());
        b[116..120].copy_from_slice(&(w * h).to_le_bytes());
        b[120..124].copy_from_slice(&2f32.to_le_bytes());
        b[124..128].copy_from_slice(&3f32.to_le_bytes());
        b[128..132].copy_from_slice(&0.5f32.to_le_bytes());
        b[164..165].copy_from_slice(b"Z");
        b[180..182].copy_from_slice(b"um");
        b[196..198].copy_from_slice(b"um");
        b[212..214].copy_from_slice(b"um");
        b[474..478].copy_from_slice(&1f32.to_le_bytes());
        b
    }

    #[test]
    fn surface_and_intensity() {
        let mut f = header(2, 3, 2, 32, -4, true);
        for v in [-4i32, -6, 0, 2, 4, 6] {
            f.extend_from_slice(&v.to_le_bytes());
        }
        let mut i = header(10, 3, 2, 16, 0, false);
        i[212..214].fill(0);
        f.extend_from_slice(&i);
        for v in [1i16, 2, 3, 4, 5, 6] {
            f.extend_from_slice(&v.to_le_bytes());
        }
        let d = read(&f).unwrap();
        assert_eq!(d.pages.len(), 1);
        let p = &d.pages[0];
        assert_eq!(p.title.as_deref(), Some("DVD"));
        assert!(p.levelling);
        assert_eq!(p.planes.len(), 2);
        let (z, _) = p.read_plane(&f, 0).unwrap();
        assert!((z[0] - 1e-6).abs() < 1e-12);
        assert!(z[1].is_nan());
        assert!((z[5] - 6e-6).abs() < 1e-12);
        let (i, _) = p.read_plane(&f, 1).unwrap();
        assert_eq!(i, [1.5, 2.0, 2.5, 3.0, 3.5, 4.0]);
        assert!((p.x.as_ref().unwrap().step - 2e-6).abs() < 1e-15);
        assert!((p.y.as_ref().unwrap().step - 3e-6).abs() < 1e-15);
    }

    #[test]
    fn refuses_spectra_and_truncation() {
        let mut f = header(20, 4, 1, 32, 0, false);
        f.extend_from_slice(&[0; 16]);
        assert!(matches!(read(&f), Err(Error::Unsupported(_))));
        let f = header(2, 4, 4, 32, 0, false);
        assert!(matches!(read(&f), Err(Error::Truncated(_))));
        assert!(read(b"DIGITAL SURF").is_err());
    }
}
