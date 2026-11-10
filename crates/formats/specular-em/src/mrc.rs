//! MRC / MRCS (MRC2014, with the older IMOD and CCP4 layouts): [`parse`] reads the 1024-byte
//! header and the extended header's FEI records; [`Mrc::read_plane`] reads one stored
//! section, [`Mrc::read_xyz`] one z section whatever the stored axis order.
//!
//! Header words (4 bytes each, in the file's byte order): `nx ny nz` (0), `mode` (12), the
//! start indices `nxstart nystart nzstart` (16), the sampling `mx my mz` (28), the cell
//! `cella` in Å (40) and its angles (52), the axis order `mapc mapr maps` (64; crystallographic
//! maps often keep y or z fastest), `dmin dmax dmean` (76), `ispg` (88),
//! the extended header's length `nsymbt` (92), its type `exttyp` (104), `nversion` (108),
//! IMOD's stamp and flags (152, 156), `origin` (196), `"MAP "` (208), the machine stamp (212),
//! `rms` (216), the label count (220) and ten 80-byte labels (224). Data starts at
//! `1024 + nsymbt` (past the symmetry records of a CCP4 map).
//!
//! Sections are stored x fastest with the first row at the bottom of the image (y up);
//! planes come out in file row order and the caller flips them for display.

use crate::samples::{self, Order, Sample, f64_at};
use crate::{Error, MAX_PLANE, Result};

/// Header length.
pub const HEADER: usize = 1024;
/// IMOD's stamp (`1146047817`) at byte 152.
const IMOD_STAMP: u32 = 1_146_047_817;

/// How a mode stores its values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Storage {
    Plain(Sample),
    /// Mode 101: two 4-bit unsigned values per byte, low nibble first, rows padded to a byte.
    Nibbles,
}

/// FEI1 / FEI2 extended-header metadata (EPU, Velox), from the first section's record.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Fei {
    pub microscope: Option<String>,
    pub application: Option<String>,
    /// Acquisition time, seconds since the Unix epoch.
    pub timestamp: Option<f64>,
    /// Accelerating voltage in volts.
    pub voltage: Option<f64>,
    /// Pixel size in metres (x, y).
    pub pixel_size: Option<(f64, f64)>,
    /// "TEM" or "STEM".
    pub mode: Option<&'static str>,
    pub magnification: Option<f64>,
    /// Camera length in metres.
    pub camera_length: Option<f64>,
    /// Exposure (integration time) in seconds.
    pub exposure: Option<f64>,
    pub camera: Option<String>,
    pub binning: Option<(i32, i32)>,
    /// Defocus in metres.
    pub defocus: Option<f64>,
}

/// A parsed header.
#[derive(Clone, Debug, PartialEq)]
pub struct Mrc {
    pub nx: usize,
    pub ny: usize,
    pub nz: usize,
    pub mode: i32,
    pub storage: Storage,
    pub order: Order,
    /// Sampling along each axis.
    pub sampling: [i32; 3],
    /// Cell dimensions in Å.
    pub cell: [f32; 3],
    /// Voxel size in Å (x, y, z) when the cell and sampling give a sensible one.
    pub voxel: Option<[f64; 3]>,
    pub origin: [f32; 3],
    pub dmin: f32,
    pub dmax: f32,
    pub dmean: f32,
    /// Space group: 0 for image stacks, 1 for a volume, 401+ for volume stacks.
    pub ispg: i32,
    pub nversion: i32,
    pub exttyp: [u8; 4],
    /// Extended header length.
    pub nsymbt: usize,
    pub labels: Vec<String>,
    /// Whether the header carries IMOD's stamp.
    pub imod: bool,
    pub fei: Option<Fei>,
    /// The axis (0 x, 1 y, 2 z) along the columns, rows and sections (`MAPC MAPR MAPS`,
    /// bytes 64–75; crystallographic maps often store y or z fastest).
    pub axes: [usize; 3],
    /// The first column, row and section's grid index (`NXSTART NYSTART NZSTART`).
    pub start: [i32; 3],
    /// Cell angles α, β, γ in degrees.
    pub angles: [f32; 3],
    /// RMS deviation from the mean (word 55).
    pub rms: f32,
    data_offset: usize,
}

fn word(h: &[u8], o: usize, le: bool) -> i32 {
    let b = [h[o], h[o + 1], h[o + 2], h[o + 3]];
    if le {
        i32::from_le_bytes(b)
    } else {
        i32::from_be_bytes(b)
    }
}

fn float(h: &[u8], o: usize, le: bool) -> f32 {
    f32::from_bits(word(h, o, le) as u32)
}

fn storage(mode: i32) -> Option<Storage> {
    Some(Storage::Plain(match mode {
        0 => Sample::I8,
        1 => Sample::I16,
        2 => Sample::F32,
        3 => Sample::ComplexI16,
        4 => Sample::ComplexF32,
        6 => Sample::U16,
        12 => Sample::F16,
        101 => return Some(Storage::Nibbles),
        _ => return None,
    }))
}

/// The byte order: from the machine stamp, else whichever order gives a sensible header.
fn byte_order(h: &[u8]) -> Option<bool> {
    match h[212] {
        0x44 | 0x41 => return Some(true),
        0x11 => return Some(false),
        _ => {}
    }
    let sane = |le: bool| {
        let n = |o| word(h, o, le);
        storage(n(12)).is_some()
            && (1..=1 << 20).contains(&n(0))
            && (1..=1 << 20).contains(&n(4))
            && (1..=1 << 24).contains(&n(8))
    };
    if sane(true) {
        Some(true)
    } else if sane(false) {
        Some(false)
    } else {
        None
    }
}

/// Whether `head` (at least 1024 bytes) looks like an MRC header: the `MAP ` tag and a
/// sensible size and mode.
pub fn sniff(head: &[u8]) -> bool {
    head.len() >= HEADER
        && (&head[208..212] == b"MAP " || &head[208..212] == b"MAP\0")
        && byte_order(head).is_some_and(|le| {
            storage(word(head, 12, le)).is_some() && word(head, 0, le) > 0 && word(head, 4, le) > 0
        })
}

/// Parses the header (and the FEI extended header) of a whole file.
pub fn parse(file: &[u8]) -> Result<Mrc> {
    if file.len() < HEADER {
        return Err(Error::NotMrc);
    }
    let h = &file[..HEADER];
    let le = byte_order(h).ok_or(Error::NotMrc)?;
    let order = if le { Order::Little } else { Order::Big };
    let w = |o| word(h, o, le);
    let f = |o| float(h, o, le);
    let (nx, ny, nz, mode) = (w(0), w(4), w(8), w(12));
    if nx <= 0 || ny <= 0 || nz <= 0 {
        return Err(Error::Malformed(format!("size {nx} × {ny} × {nz}")));
    }
    let mut storage =
        storage(mode).ok_or_else(|| Error::Unsupported(format!("MRC mode {mode}")))?;
    let (nx, ny, nz) = (nx as usize, ny as usize, nz as usize);
    if (nx * ny) as u64 > MAX_PLANE {
        return Err(Error::TooLarge);
    }
    let nversion = w(108);
    let imod = w(152) as u32 == IMOD_STAMP;
    // Mode 0 is signed in MRC2014; IMOD wrote unsigned bytes unless its flag says otherwise.
    if mode == 0 && nversion < 20140 && imod && w(156) & 1 == 0 {
        storage = Storage::Plain(Sample::U8);
    }
    let nsymbt = usize::try_from(w(92)).unwrap_or(0);
    let data_offset = HEADER.saturating_add(nsymbt);
    if data_offset > file.len() {
        return Err(Error::Truncated("extended header"));
    }
    // MAPC/MAPR/MAPS: a permutation of 1, 2, 3; anything else (zeros in some EM writers)
    // is the usual x, y, z.
    let map = [w(64), w(68), w(72)];
    let axes = if (1..=3).all(|a| map.contains(&a)) {
        map.map(|a| (a - 1) as usize)
    } else {
        [0, 1, 2]
    };
    if axes != [0, 1, 2] && storage == Storage::Nibbles {
        return Err(Error::Unsupported(
            "4-bit maps in another axis order".into(),
        ));
    }
    let sampling = [w(28), w(32), w(36)];
    let cell = [f(40), f(44), f(48)];
    let size = |c: f32, m: i32| {
        let v = (f64::from(c) / f64::from(m)).abs();
        (m > 0 && v.is_finite() && v > 0.0 && v < 1e7).then_some(v)
    };
    let voxel = match (
        size(cell[0], sampling[0]),
        size(cell[1], sampling[1]),
        size(cell[2], sampling[2]),
    ) {
        (Some(x), Some(y), z) => Some([x, y, z.unwrap_or(x)]),
        _ => None,
    };
    let nlabl = usize::try_from(w(220)).unwrap_or(0).min(10);
    let labels = (0..nlabl)
        .filter_map(|i| {
            let b = &h[224 + 80 * i..224 + 80 * (i + 1)];
            let s: String = b
                .iter()
                .take_while(|&&c| c != 0)
                .map(|&c| char::from(c))
                .collect();
            let s = s.trim().to_string();
            (!s.is_empty()).then_some(s)
        })
        .collect();
    let mut exttyp = [0u8; 4];
    exttyp.copy_from_slice(&h[104..108]);
    let fei = if &exttyp == b"FEI1" || &exttyp == b"FEI2" {
        fei(&file[HEADER..data_offset])
    } else {
        None
    };
    Ok(Mrc {
        nx,
        ny,
        nz,
        mode,
        storage,
        order,
        sampling,
        cell,
        voxel,
        origin: [f(196), f(200), f(204)],
        dmin: f(76),
        dmax: f(80),
        dmean: f(84),
        ispg: w(88),
        nversion,
        exttyp,
        nsymbt,
        labels,
        imod,
        fei,
        axes,
        start: [w(16), w(20), w(24)],
        angles: [f(52), f(56), f(60)],
        rms: f(216),
        data_offset,
    })
}

/// The first FEI record (little-endian, packed).
fn fei(ext: &[u8]) -> Option<Fei> {
    if ext.len() < 4 {
        return None;
    }
    let size = usize::try_from(i32::from_le_bytes([ext[0], ext[1], ext[2], ext[3]])).ok()?;
    let r = ext.get(..size.min(ext.len()))?;
    let f64v = |o: usize| {
        r.get(o..o + 8)
            .map(|b| f64_at(b, true))
            .filter(|v| v.is_finite() && *v != 0.0)
    };
    let i32v = |o: usize| {
        r.get(o..o + 4)
            .map(|b| i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    };
    let s16 = |o: usize| {
        let b = r.get(o..o + 16)?;
        let s: String = b
            .iter()
            .take_while(|&&c| c != 0)
            .map(|&c| char::from(c))
            .collect();
        let s = s.trim().to_string();
        (!s.is_empty() && s.chars().all(|c| !c.is_control())).then_some(s)
    };
    let pos = |v: Option<f64>| v.filter(|&v| v > 0.0);
    let px = match (pos(f64v(156)), pos(f64v(164))) {
        (Some(x), Some(y)) if x < 1.0 && y < 1.0 => Some((x, y)),
        _ => None,
    };
    Some(Fei {
        microscope: s16(20),
        application: s16(52),
        timestamp: f64v(12)
            .filter(|&d| d > 25569.0 && d < 80000.0)
            .map(|d| (d - 25569.0) * 86400.0),
        voltage: pos(f64v(84)),
        pixel_size: px,
        mode: match i32v(244) {
            Some(1) => Some("TEM"),
            Some(2) => Some("STEM"),
            _ => None,
        },
        magnification: pos(f64v(289)),
        camera_length: pos(f64v(301)),
        exposure: pos(f64v(419)),
        camera: s16(435),
        binning: match (i32v(427), i32v(431)) {
            (Some(x), Some(y)) if x > 0 && y > 0 && x < 1024 && y < 1024 => Some((x, y)),
            _ => None,
        },
        defocus: f64v(220).filter(|d| d.abs() < 1.0),
    })
}

impl Mrc {
    /// A plain image stack of `nz` sections of `nx × ny` samples from `data_offset` (another
    /// format laid out like MRC, such as DeltaVision).
    pub(crate) fn stack(
        nx: usize,
        ny: usize,
        nz: usize,
        mode: i32,
        sample: Sample,
        order: Order,
        data_offset: usize,
    ) -> Mrc {
        Mrc {
            nx,
            ny,
            nz,
            mode,
            storage: Storage::Plain(sample),
            order,
            sampling: [1, 1, 1],
            cell: [0.0; 3],
            voxel: None,
            origin: [0.0; 3],
            dmin: 0.0,
            dmax: 0.0,
            dmean: 0.0,
            ispg: 0,
            nversion: 0,
            exttyp: [0; 4],
            nsymbt: data_offset - HEADER,
            labels: Vec::new(),
            imod: false,
            fei: None,
            axes: [0, 1, 2],
            start: [0; 3],
            angles: [90.0; 3],
            rms: 0.0,
            data_offset,
        }
    }

    /// Bytes per section.
    pub fn section_bytes(&self) -> usize {
        match self.storage {
            Storage::Plain(s) => self.nx * self.ny * s.bytes(),
            Storage::Nibbles => self.nx.div_ceil(2) * self.ny,
        }
    }

    /// The stored sample type (`U8` for 4-bit data).
    pub fn sample(&self) -> Sample {
        match self.storage {
            Storage::Plain(s) => s,
            Storage::Nibbles => Sample::U8,
        }
    }

    /// Bits per value.
    pub fn bits(&self) -> u32 {
        match self.storage {
            Storage::Plain(s) => s.bits(),
            Storage::Nibbles => 4,
        }
    }

    /// Whether the sections are a volume (rather than a stack of images).
    /// Whether the columns, rows and sections are not x, y and z.
    pub fn reordered(&self) -> bool {
        self.axes != [0, 1, 2]
    }

    /// Points along x, y and z.
    pub fn size_xyz(&self) -> [usize; 3] {
        let mut s = [0; 3];
        for (k, n) in [self.nx, self.ny, self.nz].into_iter().enumerate() {
            s[self.axes[k]] = n;
        }
        s
    }

    /// The first grid point along x, y and z, in Å: the MRC2014 `origin` when set, else
    /// the CCP4 start indices times the voxel size.
    pub fn origin_xyz(&self) -> [f64; 3] {
        if self.origin.iter().any(|&o| o != 0.0 && o.is_finite()) {
            return self.origin.map(f64::from);
        }
        let mut o = [0.0; 3];
        if let Some(v) = self.voxel {
            for k in 0..3 {
                let a = self.axes[k];
                o[a] = f64::from(self.start[k]) * v[a];
            }
        }
        o
    }

    /// Section `z` of the map in x, y, z order (x across, rows along y, file row order: y
    /// up), whatever the stored axis order. Returns whether data were missing.
    pub fn read_xyz(&self, file: &[u8], z: usize, out: &mut [f32]) -> Result<bool> {
        if !self.reordered() {
            return self.read_plane(file, z, out);
        }
        let [sx, sy, sz] = self.size_xyz();
        let Storage::Plain(s) = self.storage else {
            return Err(Error::Unsupported(
                "4-bit maps in another axis order".into(),
            ));
        };
        if z >= sz || out.len() != sx * sy {
            return Err(Error::Malformed(format!("section {z} of {sz}")));
        }
        // Bytes between neighbours along each stored axis, then along x, y, z.
        let b = s.bytes();
        let stored = [b, b * self.nx, b * self.nx * self.ny];
        let mut step = [0usize; 3];
        for k in 0..3 {
            step[self.axes[k]] = stored[k];
        }
        let mut missing = false;
        for (y, row) in out.chunks_exact_mut(sx).enumerate() {
            let at = self
                .data_offset
                .saturating_add(y.saturating_mul(step[1]))
                .saturating_add(z.saturating_mul(step[2]));
            let src = file.get(at.min(file.len())..).unwrap_or(&[]);
            missing |= samples::convert(src, s, self.order, step[0], row);
        }
        Ok(missing)
    }

    pub fn is_volume(&self) -> bool {
        self.size_xyz()[2] > 1 && self.ispg >= 1 && self.sampling[2] > 1
    }

    /// Where the data starts.
    pub fn data_offset(&self) -> usize {
        self.data_offset
    }

    /// Reads section `z` (`nx × ny` values in file order, the bottom row first) into `out`.
    /// Returns whether samples were missing (a truncated file; they read as NaN).
    pub fn read_plane(&self, file: &[u8], z: usize, out: &mut [f32]) -> Result<bool> {
        if z >= self.nz || out.len() != self.nx * self.ny {
            return Err(Error::Malformed(format!("section {z} of {}", self.nz)));
        }
        let start = self
            .data_offset
            .saturating_add(z.saturating_mul(self.section_bytes()))
            .min(file.len());
        let data = &file[start..];
        match self.storage {
            Storage::Plain(s) => Ok(samples::convert(data, s, self.order, s.bytes(), out)),
            Storage::Nibbles => {
                let row = self.nx.div_ceil(2);
                let mut missing = false;
                for (y, line) in out.chunks_exact_mut(self.nx).enumerate() {
                    let src = data.get(y * row..).unwrap_or(&[]);
                    for (x, o) in line.iter_mut().enumerate() {
                        *o = match src.get(x / 2) {
                            Some(&b) => f32::from(if x % 2 == 0 { b & 15 } else { b >> 4 }),
                            None => {
                                missing = true;
                                f32::NAN
                            }
                        };
                    }
                }
                Ok(missing)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn header(nx: i32, ny: i32, nz: i32, mode: i32, le: bool) -> Vec<u8> {
        let mut h = vec![0u8; HEADER];
        let mut put = |o: usize, v: i32| {
            let b = if le { v.to_le_bytes() } else { v.to_be_bytes() };
            h[o..o + 4].copy_from_slice(&b);
        };
        put(0, nx);
        put(4, ny);
        put(8, nz);
        put(12, mode);
        put(28, nx);
        put(32, ny);
        put(36, nz);
        put(88, 1);
        put(108, 20140);
        let cell = |v: f32| v.to_bits() as i32;
        put(40, cell(nx as f32 * 1.5));
        put(44, cell(ny as f32 * 1.5));
        put(48, cell(nz as f32 * 2.0));
        h[208..212].copy_from_slice(b"MAP ");
        if le {
            h[212..214].copy_from_slice(&[0x44, 0x44]);
        } else {
            h[212..214].copy_from_slice(&[0x11, 0x11]);
        }
        h
    }

    #[test]
    fn modes() {
        for le in [true, false] {
            let mut f = header(3, 2, 2, 1, le);
            for v in -6i16..6 {
                f.extend(if le { v.to_le_bytes() } else { v.to_be_bytes() });
            }
            assert!(sniff(&f));
            let m = parse(&f).unwrap();
            assert_eq!(m.voxel, Some([1.5, 1.5, 2.0]));
            assert!(m.is_volume());
            let mut out = vec![0f32; 6];
            assert!(!m.read_plane(&f, 1, &mut out).unwrap());
            assert_eq!(out, [0.0, 1.0, 2.0, 3.0, 4.0, 5.0]);
            // Truncated: missing samples are NaN.
            assert!(m.read_plane(&f[..f.len() - 2], 1, &mut out).unwrap());
            assert!(out[5].is_nan());
        }
        // 4-bit, odd width: rows padded to a byte.
        let mut f = header(3, 2, 1, 101, true);
        f.extend([0x21, 0x03, 0x54, 0x06]);
        let m = parse(&f).unwrap();
        let mut out = vec![0f32; 6];
        m.read_plane(&f, 0, &mut out).unwrap();
        assert_eq!(out, [1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        // IMOD unsigned bytes.
        let mut f = header(2, 1, 1, 0, true);
        f[108..112].copy_from_slice(&0i32.to_le_bytes());
        f[152..156].copy_from_slice(&IMOD_STAMP.to_le_bytes());
        f.extend([200, 1]);
        let m = parse(&f).unwrap();
        let mut out = vec![0f32; 2];
        m.read_plane(&f, 0, &mut out).unwrap();
        assert_eq!(out, [200.0, 1.0]);
        // Signed when IMOD's flag says so.
        f[156] = 1;
        let m = parse(&f).unwrap();
        m.read_plane(&f, 0, &mut out).unwrap();
        assert_eq!(out, [-56.0, 1.0]);
    }

    #[test]
    fn rejects() {
        assert_eq!(parse(&[0u8; 100]).unwrap_err(), Error::NotMrc);
        let f = header(3, 2, 1, 7, true);
        assert!(matches!(parse(&f), Err(Error::Unsupported(_))));
        let mut f = header(3, 2, 1, 2, true);
        f[92..96].copy_from_slice(&5000i32.to_le_bytes());
        assert_eq!(parse(&f).unwrap_err(), Error::Truncated("extended header"));
        // Any byte order guess on a header without a stamp never panics.
        let mut f = header(3, 2, 1, 2, true);
        f[212] = 0;
        assert_eq!(parse(&f).unwrap().order, Order::Little);
    }
}
