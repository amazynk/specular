//! Zygo interferometer data: MetroPro `.dat` and Mx `.datx`.
//!
//! **MetroPro `.dat`** is big-endian: a header (834 bytes for format 1, 4096 for formats 2
//! and 3) describing the intensity array (`buckets × width × height` unsigned 16-bit) and
//! the connected phase array (signed 32-bit), which follow it in that order. Phase counts
//! become metres as `raw × S × λ × O / R` (interferometric scale factor, wavelength,
//! obliquity factor, phase resolution); counts from 2147483640 up are invalid (NaN).
//!
//! **Mx `.datx`** is HDF5: each dataset of `/Data/Surface` is a height map (float64 in the
//! dataset's `Unit`, `No Data` marking invalid points; `Fringes` and `Waves` scale by the
//! wavelength), `/Data/Intensity` datasets of the same size are extra planes, and the pixel
//! pitch is the `Data Context.Lateral Resolution:Value` attribute of the `/Attributes`
//! group.
//!
//! Both store rows top-down, as MetroPro and Mx show them.

use specular_em::dataset::{Dataset, Plane};
use specular_em::samples::Sample;

use crate::{
    Error, Result, alloc, area, f32be, height, i16be, i32be, meta, page, slice, text, u16be, u32be,
};

const MAGIC: [u32; 3] = [0x881b_036e, 0x881b_036f, 0x881b_0370];
/// Phase counts from here up mark invalid points.
const INVALID: i32 = 2_147_483_640;

pub fn probe(head: &[u8]) -> bool {
    u32be(head, 0).is_some_and(|m| MAGIC.contains(&m))
}

pub fn read(file: &[u8]) -> Result<Dataset> {
    if !probe(file) {
        return Err(Error::NotFormat("Zygo MetroPro"));
    }
    let h = slice(file, 0, 834, "header")?;
    let format = i16be(h, 4).unwrap_or(0);
    let size = i32be(h, 6).unwrap_or(0);
    if size < 834 {
        return Err(Error::Malformed(format!("header size {size}")));
    }
    let size = size as usize;
    let g16 = |o| usize::from(u16be(h, o).unwrap_or(0));
    let gi = |o| i32be(h, o).unwrap_or(0).max(0) as usize;
    let gf = |o| f64::from(f32be(h, o).unwrap_or(0.0));
    let (aw, ah, buckets, abytes) = (g16(52), g16(54), g16(56), gi(60));
    let (cw, ch, cbytes) = (g16(68), g16(70), gi(72));
    let scale = gf(164);
    let wavelength = gf(168);
    let obliquity = match gf(176) {
        o if o > 0.0 && o.is_finite() => o,
        _ => 1.0,
    };
    let resolution = match (format, i16be(h, 218).unwrap_or(0)) {
        (_, 0) => 4096.0,
        (_, 1) => 32768.0,
        (f, 2) if f >= 2 => 131072.0,
        (_, r) => return Err(Error::Unsupported(format!("phase resolution {r}"))),
    };
    let lateral = gf(184);

    let mut pages = Vec::new();
    if cw > 0 && ch > 0 && cbytes > 0 {
        let n = area(cw, ch)?;
        let at = size.checked_add(abytes).ok_or(Error::TooLarge)?;
        let data = slice(file, at, n * 4, "phase data")?;
        let k = scale * wavelength * obliquity / resolution;
        let mut v = alloc(n)?;
        v.extend(data.chunks_exact(4).map(|c| {
            let r = i32be(c, 0).unwrap_or(INVALID);
            if r >= INVALID {
                f32::NAN
            } else {
                (f64::from(r) * k) as f32
            }
        }));
        let mut pg = page(cw, ch, lateral, lateral);
        pg.planes.push(height(v));
        pg.sample = Some(Sample::I32);
        pg.levelling = true;
        pages.push(pg);
    }
    if aw > 0 && ah > 0 && abytes > 0 && buckets > 0 {
        let n = area(aw, ah)?;
        let buckets = buckets.min(abytes / (n * 2));
        let same = pages
            .first()
            .is_some_and(|p| p.width == aw && p.height == ah);
        if !same {
            let mut pg = page(aw, ah, lateral, lateral);
            pg.sample = Some(Sample::U16);
            pages.push(pg);
        }
        let pg = pages.last_mut().expect("page");
        for b in 0..buckets {
            let data = slice(file, size + b * n * 2, n * 2, "intensity data")?;
            let mut v = alloc(n)?;
            v.extend(
                data.chunks_exact(2)
                    .map(|c| f32::from(u16be(c, 0).unwrap_or(0))),
            );
            let label = if buckets > 1 {
                format!("Intensity {}", b + 1)
            } else {
                "Intensity".to_string()
            };
            pg.planes.push(Plane::values(label, "counts", v));
        }
    }
    if pages.is_empty() {
        return Err(Error::NoImage);
    }
    for pg in &mut pages {
        let date = text(h, 12, 30);
        pg.date = (!date.is_empty()).then_some(date);
        let mag = gf(180);
        pg.magnification = (mag > 0.0 && mag.is_finite()).then_some(mag);
        meta(pg, "Format", format.to_string());
        meta(pg, "Comment", text(h, 80, 82));
        meta(pg, "Objective", text(h, 246, 12));
        meta(pg, "Part", text(h, 258, 40));
        meta(pg, "Wavelength", format!("{wavelength} m"));
        meta(pg, "Interferometric scale factor", scale.to_string());
        meta(pg, "Obliquity factor", obliquity.to_string());
        meta(pg, "Numerical aperture", gf(172).to_string());
        meta(pg, "Phase resolution", resolution.to_string());
    }
    Ok(Dataset { pages })
}

/// An HDF5 file (the `.datx` check needs the whole file: [`probe_datx_file`]).
pub fn probe_datx(head: &[u8]) -> bool {
    specular_hdf5::sniff(head)
}

/// An HDF5 file with a `/Data/Surface` dataset.
pub fn probe_datx_file(file: &[u8]) -> bool {
    probe_datx(file)
        && specular_hdf5::File::open(file)
            .is_ok_and(|f| f.children("/Data/Surface").any(|o| o.dataset().is_some()))
}

fn h5err(e: specular_hdf5::Error) -> Error {
    Error::Malformed(format!("HDF5: {e}"))
}

/// Metres per unit of a `.datx` height unit, given the dataset's wavelength, scale factor
/// and obliquity factor for fringes.
fn datx_unit(unit: &str, wavelength: f64, scale: f64, obliquity: f64) -> Option<f64> {
    Some(match unit {
        "Meters" => 1.0,
        "MilliMeters" => 1e-3,
        "MicroMeters" => 1e-6,
        "NanoMeters" => 1e-9,
        "Angstroms" => 1e-10,
        "Fringes" => wavelength * scale * obliquity,
        "Waves" => wavelength,
        _ => return None,
    })
}

pub fn read_datx(file: &[u8]) -> Result<Dataset> {
    if !probe_datx(file) {
        return Err(Error::NotFormat("Zygo .datx"));
    }
    let f = specular_hdf5::File::open(file).map_err(h5err)?;
    // The pixel pitch, from the measurement's attribute group.
    let mut lateral = 0.0;
    let mut instrument = None;
    for g in f.children("/Attributes") {
        if let Some(v) = g
            .attr("Data Context.Lateral Resolution:Value")
            .and_then(|v| v.as_f64())
        {
            lateral = v;
        }
        if let Some(s) = g
            .attr("Data Context.Data Attributes.Instrument")
            .and_then(|v| v.as_str())
        {
            instrument = Some(s.to_string());
        }
    }
    let mut pages = Vec::new();
    let surfaces: Vec<&specular_hdf5::Object> = f
        .children("/Data/Surface")
        .filter(|o| o.dataset().is_some_and(|d| d.shape().len() == 2))
        .collect();
    // The surfaces' samples together: each may be a fill value with no data behind it.
    let mut total = 0usize;
    for o in surfaces {
        let shape = o.dataset().map(|d| d.shape().to_vec()).unwrap_or_default();
        let (h, w) = (shape[0] as usize, shape[1] as usize);
        let n = area(w, h)?;
        total = total.saturating_add(n);
        if total > crate::MAX_VALUES {
            return Err(Error::TooLarge);
        }
        let index = f.index_of(&o.path).ok_or(Error::NoImage)?;
        let raw = f.read_all::<f64>(file, index, n as u64).map_err(h5err)?;
        let num = |k: &str| o.attr(k).and_then(|v| v.as_f64());
        let unit = o.attr("Unit").and_then(|v| v.as_str()).unwrap_or("");
        let wavelength = num("Wavelength").unwrap_or(0.0);
        let k = datx_unit(
            unit,
            wavelength,
            num("Interferometric Scale Factor").unwrap_or(1.0),
            num("Obliquity Factor").unwrap_or(1.0),
        )
        .ok_or_else(|| Error::Unsupported(format!("height unit {unit:?}")))?;
        let nodata = num("No Data");
        let mut v = alloc(n)?;
        v.extend(raw.iter().map(|&r| {
            if Some(r) == nodata || !r.is_finite() {
                f32::NAN
            } else {
                (r * k) as f32
            }
        }));
        let mut pg = page(w, h, lateral, lateral);
        pg.planes.push(height(v));
        pg.sample = Some(Sample::F64);
        pg.levelling = true;
        pg.instrument = instrument.clone();
        meta(&mut pg, "Unit", unit);
        meta(&mut pg, "Wavelength", format!("{wavelength} m"));
        pages.push(pg);
    }
    // Intensity maps join the first page of their size.
    for o in f.children("/Data/Intensity") {
        let Some(d) = o.dataset() else { continue };
        let shape = d.shape();
        if shape.len() != 2 {
            continue;
        }
        let (h, w) = (shape[0] as usize, shape[1] as usize);
        let Some(pg) = pages.iter_mut().find(|p| p.width == w && p.height == h) else {
            continue;
        };
        let n = area(w, h)?;
        let index = f.index_of(&o.path).ok_or(Error::NoImage)?;
        let raw = f.read_all::<f64>(file, index, n as u64).map_err(h5err)?;
        let nodata = o.attr("No Data").and_then(|v| v.as_f64());
        let mut v = alloc(n)?;
        v.extend(raw.iter().map(|&r| {
            if Some(r) == nodata {
                f32::NAN
            } else {
                r as f32
            }
        }));
        pg.planes.push(Plane::values("Intensity", "counts", v));
    }
    if pages.is_empty() {
        return Err(Error::NoImage);
    }
    Ok(Dataset { pages })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metropro_phase_and_intensity() {
        let mut f = vec![0u8; 834];
        f[..4].copy_from_slice(&MAGIC[0].to_be_bytes());
        f[4..6].copy_from_slice(&1i16.to_be_bytes());
        f[6..10].copy_from_slice(&834i32.to_be_bytes());
        f[52..54].copy_from_slice(&2u16.to_be_bytes());
        f[54..56].copy_from_slice(&1u16.to_be_bytes());
        f[56..58].copy_from_slice(&1u16.to_be_bytes());
        f[60..64].copy_from_slice(&4i32.to_be_bytes());
        f[68..70].copy_from_slice(&2u16.to_be_bytes());
        f[70..72].copy_from_slice(&1u16.to_be_bytes());
        f[72..76].copy_from_slice(&8i32.to_be_bytes());
        f[164..168].copy_from_slice(&0.5f32.to_be_bytes());
        f[168..172].copy_from_slice(&6e-7f32.to_be_bytes());
        f[184..188].copy_from_slice(&1e-6f32.to_be_bytes());
        f.extend_from_slice(&7u16.to_be_bytes());
        f.extend_from_slice(&9u16.to_be_bytes());
        f.extend_from_slice(&4096i32.to_be_bytes());
        f.extend_from_slice(&INVALID.to_be_bytes());
        let d = read(&f).unwrap();
        let p = &d.pages[0];
        assert_eq!(p.planes.len(), 2);
        let (z, _) = p.read_plane(&f, 0).unwrap();
        assert!((z[0] - 3e-7).abs() < 1e-12);
        assert!(z[1].is_nan());
        let (i, _) = p.read_plane(&f, 1).unwrap();
        assert_eq!(i, [7.0, 9.0]);
        assert!(read(&f[..840]).is_err());
    }

    #[test]
    fn datx_units() {
        assert_eq!(datx_unit("NanoMeters", 0.0, 1.0, 1.0), Some(1e-9));
        assert_eq!(datx_unit("Fringes", 6e-7, 0.5, 1.0), Some(3e-7));
        assert_eq!(datx_unit("Furlongs", 0.0, 1.0, 1.0), None);
    }
}
