//! Igor Pro binary waves (`.ibw`, versions 1, 2, 3 and 5): [`parse`] reads the headers, units,
//! dimension labels and the wave note; [`Ibw::read_plane`] reads one plane.
//!
//! The first `i16` is the version, and its byte order is the file's (Macintosh files are
//! big-endian). Versions 1–3: a bin header (8, 16 or 20 bytes), a 110-byte wave header
//! (`type` at 0, `bname` at 6, `dataUnits` at 34, `xUnits` at 38, `npnts` at 42, the x scaling
//! `hsA` / `hsB` at 48 / 56), the data, 16 bytes of padding, then the note (and in version 3
//! the dependency formula). Version 5: a 64-byte bin header with the sizes of what follows the
//! data, a 320-byte wave header (`npnts` at 12, `type` at 16, `bname` at 28, `nDim[4]` at 68,
//! `sfA[4]` at 84, `sfB[4]` at 116, `dataUnits` at 148, `dimUnits[4][4]` at 152), the data, then
//! the formula, note, extended data units, extended dimension units and dimension labels
//! (32-byte entries; the first is the dimension's own name).
//!
//! The data is stored with the first dimension (rows in Igor's terms, x on screen) fastest;
//! dimensions past the second are planes. Value `i` of dimension `d` is at
//! `sfB[d] + i * sfA[d]`. Asylum Research and other AFM software write their settings into
//! the note as `Key: value` lines.

use specular_em::samples::{self, Order, Sample};

use crate::{Error, Result};

/// One dimension: size, scaling, unit and labels.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Dim {
    pub len: usize,
    /// Value of index 0.
    pub offset: f64,
    /// Step per index.
    pub delta: f64,
    pub unit: String,
    /// The dimension's own label.
    pub label: Option<String>,
    /// Per-index labels (empty when none).
    pub labels: Vec<String>,
}

/// A parsed wave.
#[derive(Clone, Debug, PartialEq)]
pub struct Ibw {
    pub version: u16,
    pub order: Order,
    pub name: String,
    pub sample: Sample,
    /// The used dimensions (at least one).
    pub dims: Vec<Dim>,
    pub data_unit: String,
    pub note: String,
    data_offset: usize,
}

const MAX_LABEL: usize = 32;

/// Whether `head` starts like a binary wave of a known version.
pub fn sniff(head: &[u8]) -> bool {
    head.len() >= 8 && order(head).is_some()
}

fn order(head: &[u8]) -> Option<(u16, Order)> {
    let le = u16::from_le_bytes([head[0], head[1]]);
    let be = u16::from_be_bytes([head[0], head[1]]);
    let ok = |v: u16| matches!(v, 1 | 2 | 3 | 5);
    if ok(le) {
        Some((le, Order::Little))
    } else if ok(be) {
        Some((be, Order::Big))
    } else {
        None
    }
}

struct H<'a> {
    b: &'a [u8],
    le: bool,
}

impl H<'_> {
    fn bytes<const N: usize>(&self, o: usize) -> Result<[u8; N]> {
        let mut a = [0u8; N];
        a.copy_from_slice(
            self.b
                .get(o..o + N)
                .ok_or(Error::Truncated("wave header"))?,
        );
        Ok(a)
    }

    fn i16(&self, o: usize) -> Result<i16> {
        let a = self.bytes::<2>(o)?;
        Ok(if self.le {
            i16::from_le_bytes(a)
        } else {
            i16::from_be_bytes(a)
        })
    }

    fn i32(&self, o: usize) -> Result<i32> {
        let a = self.bytes::<4>(o)?;
        Ok(if self.le {
            i32::from_le_bytes(a)
        } else {
            i32::from_be_bytes(a)
        })
    }

    fn f64(&self, o: usize) -> Result<f64> {
        Ok(samples::f64_at(&self.bytes::<8>(o)?, self.le))
    }

    fn size(&self, o: usize) -> Result<usize> {
        usize::try_from(self.i32(o)?).map_err(|_| Error::Malformed("negative size".into()))
    }

    fn cstr(&self, o: usize, n: usize) -> Result<String> {
        let b = self
            .b
            .get(o..o + n)
            .ok_or(Error::Truncated("wave header"))?;
        Ok(text(b))
    }
}

/// Text up to the first NUL, as UTF-8 or else Mac Roman / Latin-1 (read as Latin-1).
fn text(b: &[u8]) -> String {
    let b = b.split(|&c| c == 0).next().unwrap_or(&[]);
    match std::str::from_utf8(b) {
        Ok(s) => s.trim().to_string(),
        Err(_) => b
            .iter()
            .map(|&c| char::from(c))
            .collect::<String>()
            .trim()
            .to_string(),
    }
}

fn sample(t: i16) -> Result<Sample> {
    let complex = t & 1 != 0;
    let unsigned = t & 0x40 != 0;
    let s = match (t & !0x41, unsigned) {
        (0, _) => return Err(Error::Unsupported("text wave".into())),
        (2, false) => Sample::F32,
        (4, false) => Sample::F64,
        (8, false) => Sample::I8,
        (8, true) => Sample::U8,
        (0x10, false) => Sample::I16,
        (0x10, true) => Sample::U16,
        (0x20, false) => Sample::I32,
        (0x20, true) => Sample::U32,
        (0x80, false) => Sample::I64,
        (0x80, true) => Sample::U64,
        _ => return Err(Error::Unsupported(format!("wave type {t:#x}"))),
    };
    if !complex {
        return Ok(s);
    }
    match s {
        Sample::F32 => Ok(Sample::ComplexF32),
        Sample::F64 => Ok(Sample::ComplexF64),
        Sample::I16 => Ok(Sample::ComplexI16),
        _ => Err(Error::Unsupported(format!("complex wave type {t:#x}"))),
    }
}

/// Parses a whole file.
pub fn parse(file: &[u8]) -> Result<Ibw> {
    if file.len() < 8 {
        return Err(Error::NotIbw);
    }
    let (version, order) = order(file).ok_or(Error::NotIbw)?;
    let h = H {
        b: file,
        le: order == Order::Little,
    };
    if version == 5 {
        parse5(file, &h, order)
    } else {
        parse123(file, &h, version, order)
    }
}

fn parse123(file: &[u8], h: &H, version: u16, order: Order) -> Result<Ibw> {
    let bin = match version {
        1 => 8,
        2 => 16,
        _ => 20,
    };
    let wfm = h.size(2)?;
    let note_size = if version >= 2 { h.size(6)? } else { 0 };
    let w = bin;
    let sample = sample(h.i16(w)?)?;
    let name = h.cstr(w + 6, 20)?;
    let data_unit = h.cstr(w + 34, 4)?;
    let x_unit = h.cstr(w + 38, 4)?;
    let npnts = h.size(w + 42)?;
    if npnts == 0 {
        return Err(Error::NoImage);
    }
    // The wave section is the 110-byte header plus the data; a dependent wave (one computed
    // from a formula) may be saved without its values.
    if npnts.saturating_mul(sample.bytes()) > wfm.saturating_sub(110) {
        return Err(Error::Unsupported(format!(
            "{npnts} points but {} data bytes (a dependent wave saved without values)",
            wfm.saturating_sub(110)
        )));
    }
    let delta = h.f64(w + 48)?;
    let offset = h.f64(w + 56)?;
    let data_offset = w + 110;
    let note = if note_size > 0 {
        let start = bin.saturating_add(wfm);
        file.get(start..start.saturating_add(note_size))
            .map(note_text)
            .unwrap_or_default()
    } else {
        String::new()
    };
    Ok(Ibw {
        version,
        order,
        name,
        sample,
        dims: vec![Dim {
            len: npnts,
            offset,
            delta,
            unit: x_unit,
            label: None,
            labels: Vec::new(),
        }],
        data_unit,
        note,
        data_offset,
    })
}

fn note_text(b: &[u8]) -> String {
    match std::str::from_utf8(b) {
        Ok(s) => s.to_string(),
        Err(_) => b.iter().map(|&c| char::from(c)).collect(),
    }
}

fn parse5(file: &[u8], h: &H, order: Order) -> Result<Ibw> {
    let wfm = h.size(4)?;
    let formula = h.size(8)?;
    let note_size = h.size(12)?;
    let data_eunits = h.size(16)?;
    let dim_eunits: Vec<usize> = (0..4).map(|i| h.size(20 + 4 * i)).collect::<Result<_>>()?;
    let dim_labels: Vec<usize> = (0..4).map(|i| h.size(36 + 4 * i)).collect::<Result<_>>()?;
    let w = 64;
    let npnts = h.size(w + 12)?;
    let sample = sample(h.i16(w + 16)?)?;
    let name = h.cstr(w + 28, 32)?;
    let n: Vec<usize> = (0..4)
        .map(|i| h.size(w + 68 + 4 * i))
        .collect::<Result<_>>()?;
    if npnts == 0 || n[0] == 0 {
        return Err(Error::NoImage);
    }
    let used = n.iter().take_while(|&&d| d > 0).count();
    let total = n[..used]
        .iter()
        .try_fold(1usize, |a, &d| a.checked_mul(d))
        .ok_or(Error::TooLarge)?;
    if total != npnts {
        return Err(Error::Malformed(format!(
            "{npnts} points in {:?}",
            &n[..used]
        )));
    }
    if n[0] as u64 * n.get(1).copied().filter(|&d| d > 0).unwrap_or(1) as u64
        > specular_em::MAX_PLANE
    {
        return Err(Error::TooLarge);
    }
    // What follows the data.
    let mut pos = 64usize.saturating_add(wfm).saturating_add(formula);
    let mut next = |len: usize| {
        let s = file.get(pos..pos.saturating_add(len)).unwrap_or(&[]);
        pos = pos.saturating_add(len);
        s
    };
    let note = note_text(next(note_size));
    let data_eunit = text(next(data_eunits));
    let eunits: Vec<String> = dim_eunits.iter().map(|&l| text(next(l))).collect();
    let labels: Vec<Vec<String>> = dim_labels
        .iter()
        .map(|&l| next(l).chunks(MAX_LABEL).map(text).collect())
        .collect();
    let mut data_unit = h.cstr(w + 148, 4)?;
    if !data_eunit.is_empty() {
        data_unit = data_eunit;
    }
    let dims = (0..used)
        .map(|i| {
            let mut unit = h.cstr(w + 152 + 4 * i, 4)?;
            if !eunits[i].is_empty() {
                unit.clone_from(&eunits[i]);
            }
            let mut l = labels[i].clone();
            let label = if l.is_empty() {
                None
            } else {
                Some(l.remove(0)).filter(|s| !s.is_empty())
            };
            l.truncate(n[i]);
            if l.iter().all(String::is_empty) {
                l.clear();
            }
            Ok(Dim {
                len: n[i],
                delta: h.f64(w + 84 + 8 * i)?,
                offset: h.f64(w + 116 + 8 * i)?,
                unit,
                label,
                labels: l,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(Ibw {
        version: 5,
        order,
        name,
        sample,
        dims,
        data_unit,
        note,
        data_offset: 64 + 320,
    })
}

impl Ibw {
    pub fn width(&self) -> usize {
        self.dims[0].len
    }

    pub fn height(&self) -> usize {
        self.dims.get(1).map_or(1, |d| d.len)
    }

    /// Planes: the product of the dimensions past the second.
    pub fn planes(&self) -> usize {
        self.dims.iter().skip(2).map(|d| d.len).product()
    }

    /// `Key: value` lines of the note (Asylum Research and others).
    pub fn note_pairs(&self) -> Vec<(String, String)> {
        self.note
            .split(['\r', '\n'])
            .filter_map(|l| {
                let (k, v) = l.split_once(':')?;
                let (k, v) = (k.trim(), v.trim());
                (!k.is_empty() && k.len() <= 64 && !k.contains('='))
                    .then(|| (k.to_string(), v.to_string()))
            })
            .collect()
    }

    /// Reads plane `p` (`width × height` values, file order) into `out`. Returns whether
    /// samples were missing (they read as NaN).
    pub fn read_plane(&self, file: &[u8], p: usize, out: &mut [f32]) -> Result<bool> {
        let n = self.width() * self.height();
        if p >= self.planes() || out.len() != n {
            return Err(Error::Malformed(format!("plane {p} of {}", self.planes())));
        }
        let size = self.sample.bytes();
        let start = self
            .data_offset
            .saturating_add(p * n * size)
            .min(file.len());
        Ok(samples::convert(
            &file[start..],
            self.sample,
            self.order,
            size,
            out,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A version-5 wave: 3 × 2 × 2 `f32`, little-endian, with units, labels and a note.
    fn v5() -> Vec<u8> {
        let mut f = vec![0u8; 64 + 320];
        let put32 =
            |f: &mut Vec<u8>, o: usize, v: i32| f[o..o + 4].copy_from_slice(&v.to_le_bytes());
        f[0..2].copy_from_slice(&5i16.to_le_bytes());
        put32(&mut f, 4, 320 + 12 * 4);
        let note = b"ScanSize: 1e-06\rImagingMode: AC Mode\r";
        put32(&mut f, 12, note.len() as i32);
        put32(&mut f, 16, 0);
        put32(&mut f, 36 + 8, 3 * 32);
        let w = 64;
        put32(&mut f, w + 12, 12);
        f[w + 16..w + 18].copy_from_slice(&2i16.to_le_bytes());
        f[w + 28..w + 32].copy_from_slice(b"wave");
        put32(&mut f, w + 68, 3);
        put32(&mut f, w + 72, 2);
        put32(&mut f, w + 76, 2);
        f[w + 84..w + 92].copy_from_slice(&0.5f64.to_le_bytes());
        f[w + 116..w + 124].copy_from_slice(&1.0f64.to_le_bytes());
        f[w + 148..w + 149].copy_from_slice(b"m");
        f[w + 152..w + 153].copy_from_slice(b"m");
        for v in 0..12 {
            f.extend((v as f32).to_le_bytes());
        }
        f.extend(note);
        let mut lab = vec![0u8; 96];
        lab[32..38].copy_from_slice(b"Height");
        lab[64..67].copy_from_slice(b"Amp");
        lab[67] = 0;
        lab[70] = b'x';
        f.extend(lab);
        f
    }

    #[test]
    fn version5() {
        let f = v5();
        let w = parse(&f).unwrap();
        assert_eq!((w.width(), w.height(), w.planes()), (3, 2, 2));
        assert_eq!(w.dims[0].delta, 0.5);
        assert_eq!(w.dims[0].offset, 1.0);
        assert_eq!(w.dims[0].unit, "m");
        assert_eq!(w.data_unit, "m");
        assert_eq!(w.dims[2].labels, ["Height", "Amp"]);
        assert_eq!(
            w.note_pairs()[1],
            ("ImagingMode".to_string(), "AC Mode".to_string())
        );
        let mut out = vec![0f32; 6];
        assert!(!w.read_plane(&f, 1, &mut out).unwrap());
        assert_eq!(out, [6.0, 7.0, 8.0, 9.0, 10.0, 11.0]);
        for cut in 0..f.len() {
            if let Ok(w) = parse(&f[..cut]) {
                let _ = w.read_plane(&f[..cut], 0, &mut out);
            }
        }
    }

    #[test]
    fn version2_big_endian() {
        let mut f = vec![0u8; 16 + 110];
        f[0..2].copy_from_slice(&2i16.to_be_bytes());
        f[2..6].copy_from_slice(&(110i32 + 16 + 4).to_be_bytes());
        f[16..18].copy_from_slice(&0x10i16.to_be_bytes());
        f[16 + 42..16 + 46].copy_from_slice(&2i32.to_be_bytes());
        f[16 + 48..16 + 56].copy_from_slice(&1.0f64.to_be_bytes());
        f.extend((-3i16).to_be_bytes());
        f.extend(4i16.to_be_bytes());
        let w = parse(&f).unwrap();
        assert_eq!(w.order, Order::Big);
        let mut out = vec![0f32; 2];
        w.read_plane(&f, 0, &mut out).unwrap();
        assert_eq!(out, [-3.0, 4.0]);
        // A text wave is refused.
        f[16..18].copy_from_slice(&0i16.to_be_bytes());
        assert!(matches!(parse(&f), Err(Error::Unsupported(_))));
    }
}
