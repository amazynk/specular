//! DeltaVision (`.dv`, `.r3d`): the Priism/IVE header of Applied Precision's (now GE's)
//! widefield and OMX structured-illumination microscopes. The 1024-byte header is laid out
//! like an MRC header, with its own fields past word 22; sections are read by the MRC code.
//!
//! Fields used (from the published Priism header description, as the `mrc` package reads it):
//! `nx ny nsections` (0), the pixel type (12: 0 uint8, 1 int16, 2 float32, 6 uint16,
//! 7 int32), pixel sizes in µm (40), the extended header's length (92), the DeltaVision ID
//! −16224 (96, also giving the byte order), the extended header's integers and floats per
//! section (128, 130), the time points (180) and the image sequence (182: 0 ZTW, 1 WZT,
//! 2 ZWT, first letter fastest), the wavelengths (196: count; 198: five values in nm), the
//! origin z, x, y in µm (208) and ten 80-byte titles (220, 224).
//!
//! The extended header holds, per section, the integers then the floats: photosensor reading,
//! time stamp (s), stage x, y, z, min, max and mean intensity, exposure time (s), neutral
//! density, excitation and emission wavelengths (nm), intensity scaling, energy conversion.

use crate::mrc::{self, Mrc};
use crate::samples::{Order, Sample};
use crate::{Error, MAX_PLANE, Result};

/// The DeltaVision ID at byte 96.
const DVID: i16 = -16224;

/// Whether `head` (at least 1024 bytes) is a DeltaVision header: the ID in either byte order
/// and a sensible size and pixel type.
pub fn sniff(head: &[u8]) -> bool {
    head.len() >= mrc::HEADER && byte_order(head).is_some_and(|le| sane(head, le))
}

fn byte_order(h: &[u8]) -> Option<bool> {
    let x = [h[96], h[97]];
    if i16::from_le_bytes(x) == DVID {
        Some(true)
    } else if i16::from_be_bytes(x) == DVID {
        Some(false)
    } else {
        None
    }
}

fn i32_at(h: &[u8], o: usize, le: bool) -> i32 {
    let b = [h[o], h[o + 1], h[o + 2], h[o + 3]];
    if le {
        i32::from_le_bytes(b)
    } else {
        i32::from_be_bytes(b)
    }
}

fn i16_at(h: &[u8], o: usize, le: bool) -> i16 {
    let b = [h[o], h[o + 1]];
    if le {
        i16::from_le_bytes(b)
    } else {
        i16::from_be_bytes(b)
    }
}

fn f32_at(h: &[u8], o: usize, le: bool) -> f32 {
    f32::from_bits(i32_at(h, o, le) as u32)
}

fn sample(mode: i32) -> Result<Sample> {
    Ok(match mode {
        0 => Sample::U8,
        1 => Sample::I16,
        2 => Sample::F32,
        6 => Sample::U16,
        7 => Sample::I32,
        3 | 4 => {
            return Err(Error::Unsupported(
                "complex DeltaVision data (a Fourier transform) is not shown".into(),
            ));
        }
        m => return Err(Error::Unsupported(format!("DeltaVision pixel type {m}"))),
    })
}

fn sane(h: &[u8], le: bool) -> bool {
    let n = |o| i32_at(h, o, le);
    sample(n(12)).is_ok() && n(0) > 0 && n(4) > 0 && n(8) > 0 && n(92) >= 0
}

/// The order of the sections: which of Z, W (wavelength) and T varies fastest.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sequence {
    /// Z fastest, then T, then W.
    Ztw,
    /// W fastest, then Z, then T.
    Wzt,
    /// Z fastest, then W, then T.
    Zwt,
}

impl Sequence {
    /// The axes fastest first: 'z', 'w', 't'.
    pub fn letters(self) -> [char; 3] {
        match self {
            Sequence::Ztw => ['z', 't', 'w'],
            Sequence::Wzt => ['w', 'z', 't'],
            Sequence::Zwt => ['z', 'w', 't'],
        }
    }
}

/// One section's extended-header facts.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Section {
    /// Seconds from the start.
    pub time: f32,
    /// Stage position, µm.
    pub stage: [f32; 3],
    /// Exposure, seconds.
    pub exposure: f32,
    /// Excitation and emission wavelengths, nm.
    pub excitation: f32,
    pub emission: f32,
}

/// A parsed DeltaVision header.
#[derive(Clone, Debug, PartialEq)]
pub struct Dv {
    /// The sections, read as an MRC stack.
    pub mrc: Mrc,
    pub nz: usize,
    pub nt: usize,
    pub nw: usize,
    pub sequence: Sequence,
    /// Pixel size x, y and the z step, µm.
    pub pixel: [f32; 3],
    /// Wavelength of each channel, nm (0 when not given).
    pub waves: Vec<i16>,
    /// Origin z, x, y in µm.
    pub origin: [f32; 3],
    pub lens: i16,
    pub titles: Vec<String>,
    /// Each section's extended-header facts (empty without floats).
    pub sections: Vec<Section>,
}

impl Dv {
    /// The section holding position (z, w, t).
    pub fn section(&self, z: usize, w: usize, t: usize) -> usize {
        match self.sequence {
            Sequence::Ztw => z + self.nz * (t + self.nt * w),
            Sequence::Wzt => w + self.nw * (z + self.nz * t),
            Sequence::Zwt => z + self.nz * (w + self.nw * t),
        }
    }

    /// The size along `axis` ('z', 'w', 't').
    pub fn len(&self, axis: char) -> usize {
        match axis {
            'z' => self.nz,
            'w' => self.nw,
            _ => self.nt,
        }
    }
}

/// Parses the header and extended header of a whole file.
pub fn parse(file: &[u8]) -> Result<Dv> {
    if file.len() < mrc::HEADER {
        return Err(Error::NotMrc);
    }
    let h = &file[..mrc::HEADER];
    let le = byte_order(h).ok_or(Error::NotMrc)?;
    let w = |o| i32_at(h, o, le);
    let s = |o| i16_at(h, o, le);
    let f = |o| f32_at(h, o, le);
    let (nx, ny, nsec, mode) = (w(0), w(4), w(8), w(12));
    if nx <= 0 || ny <= 0 || nsec <= 0 {
        return Err(Error::Malformed(format!("size {nx} × {ny} × {nsec}")));
    }
    let sample = sample(mode)?;
    let (nx, ny, nsec) = (nx as usize, ny as usize, nsec as usize);
    if (nx * ny) as u64 > MAX_PLANE {
        return Err(Error::TooLarge);
    }
    let next = usize::try_from(w(92)).map_err(|_| Error::Malformed("extended header".into()))?;
    let data_offset = mrc::HEADER.saturating_add(next);
    if data_offset > file.len() {
        return Err(Error::Truncated("extended header"));
    }
    let nt = usize::try_from(s(180)).unwrap_or(0).max(1);
    let nw = usize::try_from(s(196)).unwrap_or(0).max(1);
    if nw > 5 || nsec % (nt * nw) != 0 {
        return Err(Error::Malformed(format!(
            "{nsec} sections for {nt} time points and {nw} wavelengths"
        )));
    }
    let nz = nsec / (nt * nw);
    let sequence = match s(182) {
        1 => Sequence::Wzt,
        2 => Sequence::Zwt,
        _ => Sequence::Ztw,
    };
    let order = if le { Order::Little } else { Order::Big };
    let nints = usize::try_from(s(128)).unwrap_or(0);
    let nfloats = usize::try_from(s(130)).unwrap_or(0);
    let mut sections = Vec::new();
    let per = 4 * (nints + nfloats);
    if nfloats > 0 && per > 0 {
        let ext = &file[mrc::HEADER..data_offset];
        let n = (ext.len() / per).min(nsec);
        sections.reserve(n);
        for k in 0..n {
            let at = k * per + 4 * nints;
            let fl = |i: usize| {
                (i < nfloats)
                    .then(|| f32_at(ext, at + 4 * i, le))
                    .filter(|v| v.is_finite())
                    .unwrap_or(0.0)
            };
            sections.push(Section {
                time: fl(1),
                stage: [fl(2), fl(3), fl(4)],
                exposure: fl(8),
                excitation: fl(10),
                emission: fl(11),
            });
        }
    }
    let ntitles = usize::try_from(w(220)).unwrap_or(0).min(10);
    let titles = (0..ntitles)
        .filter_map(|i| {
            let b = &h[224 + 80 * i..224 + 80 * (i + 1)];
            let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
            let t = String::from_utf8_lossy(&b[..end]).trim().to_owned();
            (!t.is_empty()).then_some(t)
        })
        .collect();
    let finite = |v: f32| if v.is_finite() { v } else { 0.0 };
    let mrc = Mrc::stack(nx, ny, nsec, mode, sample, order, data_offset);
    Ok(Dv {
        mrc,
        nz,
        nt,
        nw,
        sequence,
        pixel: [f(40), f(44), f(48)].map(finite),
        waves: (0..nw).map(|i| s(198 + 2 * i)).collect(),
        origin: [f(208), f(212), f(216)].map(finite),
        lens: s(162),
        titles,
        sections,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(le: bool, nt: i16, nw: i16, seq: i16) -> Vec<u8> {
        let mut h = vec![0u8; mrc::HEADER];
        let put32 = |h: &mut Vec<u8>, o: usize, v: i32| {
            let b = if le { v.to_le_bytes() } else { v.to_be_bytes() };
            h[o..o + 4].copy_from_slice(&b);
        };
        let put16 = |h: &mut Vec<u8>, o: usize, v: i16| {
            let b = if le { v.to_le_bytes() } else { v.to_be_bytes() };
            h[o..o + 2].copy_from_slice(&b);
        };
        put32(&mut h, 0, 3);
        put32(&mut h, 4, 2);
        put32(&mut h, 8, 12);
        put32(&mut h, 12, 6);
        put16(&mut h, 96, DVID);
        put16(&mut h, 180, nt);
        put16(&mut h, 182, seq);
        put16(&mut h, 196, nw);
        h
    }

    #[test]
    fn sections_follow_the_sequence() {
        let mut f = header(false, 2, 3, 1);
        f.resize(mrc::HEADER + 12 * 12, 0);
        assert!(sniff(&f));
        let d = parse(&f).unwrap();
        assert_eq!((d.nz, d.nw, d.nt), (2, 3, 2));
        assert_eq!(d.sequence, Sequence::Wzt);
        assert_eq!(d.section(1, 2, 1), 2 + 3 * (1 + 2));
        assert_eq!(d.mrc.order, Order::Big);
        let d = parse(&{
            let mut f = header(true, 2, 3, 0);
            f.resize(mrc::HEADER + 12 * 12, 0);
            f
        })
        .unwrap();
        assert_eq!(d.section(1, 2, 1), 1 + 2 * (1 + 2 * 2));
    }

    #[test]
    fn mismatched_counts_and_complex_data_are_refused() {
        let f = header(true, 5, 1, 0);
        assert!(parse(&f).is_err());
        let mut f = header(true, 1, 1, 0);
        f[12..16].copy_from_slice(&4i32.to_le_bytes());
        assert!(!sniff(&f));
        let e = parse(&f).unwrap_err().to_string();
        assert!(e.contains("complex"), "{e}");
    }
}
