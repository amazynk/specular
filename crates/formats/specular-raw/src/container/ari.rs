//! ARRI ARI (ARRIRAW camera frames): `ARRI` and the byte-order mark `12 34 56 78`, then
//! little-endian header fields: frame size (20, 24), data offset and size (68, 72), colour
//! temperature of the white balance (92), white balance gains R, G, B as floats (100..112),
//! black level (120), recording date and time in BCD (380, 384) and the camera model (668).
//! The 12-bit Bayer samples (BGGR) are packed two in three bytes, low bits first. No previews.
//! Colour: ARRI's published ALEXA (ALEV III sensor) matrices for that temperature.

use super::{Black, Sensor, Storage};
use crate::cfa::Cfa;
use crate::error::{RawError, Result};
use crate::vendor::VendorCodec;
use crate::{RawFormat, RawInfo, SensorCompression};

pub(super) fn is_ari(d: &[u8]) -> bool {
    d.starts_with(b"ARRI\x12\x34\x56\x78")
}

/// Data offset when the header gives none.
const DATA: usize = 4096;

fn u32le(d: &[u8], p: usize) -> u32 {
    d.get(p..p + 4)
        .map_or(0, |s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

fn f32le(d: &[u8], p: usize) -> Option<f32> {
    Some(f32::from_bits(u32le(d, p))).filter(|v| v.is_finite() && *v > 0.05 && *v < 20.0)
}

/// `YYYYMMDD` and `HHMMSSff` in BCD.
fn date_time(date: u32, time: u32) -> Option<String> {
    let bcd = |v: u32| -> Option<u32> {
        let mut out = 0;
        for i in (0..8).rev() {
            let n = (v >> (4 * i)) & 15;
            if n > 9 {
                return None;
            }
            out = out * 10 + n;
        }
        Some(out)
    };
    let (d, t) = (bcd(date)?, bcd(time)?);
    let (y, mo, da) = (d / 10000, d / 100 % 100, d % 100);
    let (h, mi, s) = (t / 1_000_000, t / 10_000 % 100, t / 100 % 100);
    ((1980..=2200).contains(&y)
        && (1..=12).contains(&mo)
        && (1..=31).contains(&da)
        && h < 24
        && mi < 60
        && s < 60)
        .then(|| format!("{y:04}:{mo:02}:{da:02} {h:02}:{mi:02}:{s:02}"))
}

pub(super) fn parse(d: &[u8]) -> Result<(RawInfo, Option<Sensor>)> {
    if !is_ari(d) || d.len() < 28 {
        return Err(RawError::NotRaw);
    }
    let (w, h) = (u32le(d, 20), u32le(d, 24));
    let model = d
        .get(668..732)
        .map(|b| {
            let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
            String::from_utf8_lossy(&b[..end]).trim().to_string()
        })
        .unwrap_or_default();
    let info = RawInfo {
        format: Some(RawFormat::Ari),
        make: "ARRI".into(),
        model,
        orientation: 1,
        date_time: date_time(u32le(d, 380), u32le(d, 384)),
        ..Default::default()
    };
    let mut s = Sensor::new(w, h, 12, SensorCompression::Uncompressed);
    s.cfa = Cfa::bayer("BGGR");
    let black = match u32le(d, 120) {
        b @ 1..=2047 => b as f32,
        _ => 256.0,
    };
    s.black = Black::Levels([black; 4]);
    s.white = Some(4095.0);
    if let (Some(r), Some(g), Some(b)) = (f32le(d, 100), f32le(d, 104), f32le(d, 108)) {
        s.wb = Some([r / g, 1.0, b / g]);
    }
    // Every ALEXA but the ALEXA 35 (ALEV 4, another sensor) has the ALEV III sensor.
    if !info.model.contains("35") {
        let cct = match u32le(d, 92) {
            t @ 1500..=20000 => t as f64,
            _ => 5600.0,
        };
        if let Some(m) = crate::color::alexa_rgb_cam(cct) {
            s.color = super::ColorSource::Matrix(m, crate::MatrixSource::Published);
        }
    }
    let offset = match u32le(d, 68) as usize {
        o if (28..1 << 20).contains(&o) => o,
        _ => DATA,
    };
    let s = if w == 0 || h == 0 || w > 32768 || h > 32768 || w % 2 != 0 {
        s.unsupported("ARI frame size")
    } else {
        let len = w as usize * h as usize * 3 / 2;
        s.storage = Storage::Vendor {
            range: offset..offset + len,
            codec: VendorCodec::Ari,
        };
        s
    };
    let mut info = info;
    info.sensor = Some(s.info());
    Ok((info, Some(s)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(w: u32, h: u32, samples: &[u16]) -> Vec<u8> {
        let mut f = b"ARRI\x12\x34\x56\x78".to_vec();
        f.resize(DATA, 0);
        f[20..24].copy_from_slice(&w.to_le_bytes());
        f[24..28].copy_from_slice(&h.to_le_bytes());
        f[68..72].copy_from_slice(&(DATA as u32).to_le_bytes());
        for (i, v) in [1.5f32, 1.0, 2.0].iter().enumerate() {
            f[100 + 4 * i..104 + 4 * i].copy_from_slice(&v.to_le_bytes());
        }
        f[120..124].copy_from_slice(&256u32.to_le_bytes());
        f[380..384].copy_from_slice(&0x2019_0722u32.to_le_bytes());
        f[384..388].copy_from_slice(&0x1611_3700u32.to_le_bytes());
        f[668..674].copy_from_slice(b"A-MINI");
        f.extend(crate::ari::tests::pack(samples));
        f
    }

    #[test]
    fn frame() {
        let (w, h) = (8u32, 4u32);
        let samples: Vec<u16> = (0..w * h).map(|i| 256 + (i * 101 % 3000) as u16).collect();
        let f = file(w, h, &samples);
        let raw = crate::parse(&f).unwrap();
        let info = raw.info();
        assert_eq!(info.format, Some(RawFormat::Ari));
        assert_eq!(
            (info.make.as_str(), info.model.as_str()),
            ("ARRI", "A-MINI")
        );
        assert_eq!(info.date_time.as_deref(), Some("2019:07:22 16:11:37"));
        let img = raw.decode_raw(&f, &Default::default()).unwrap();
        assert_eq!(img.data, samples);
        assert_eq!(img.black, [256.0; 4]);
        assert_eq!(img.wb, Some([1.5, 1.0, 2.0]));
        assert!(!img.truncated);
        assert!(crate::develop(&img, &Default::default()).is_ok());
    }

    #[test]
    fn video_raw_refusals() {
        let red = b"\0\0\x02\x00RED2xxxx";
        assert!(crate::video_raw_refusal(red, "mov").is_some_and(|w| w.contains("R3D")));
        assert!(crate::video_raw_refusal(&[], "R3D").is_some());
        assert!(
            crate::video_raw_refusal(b"\0\0\0\x14ftypqt  ", "braw")
                .is_some_and(|w| w.contains("Blackmagic"))
        );
        let f = file(8, 2, &[300; 16]);
        assert_eq!(crate::video_raw_refusal(&f, "ari"), None);
        // Rawzor by its signature, whatever the name; other `.rwz` files are Outlook rules.
        let rz = b"rawzor\x01\x00\x01\x00";
        assert!(crate::video_raw_refusal(rz, "rwz").is_some_and(|w| w.contains("compression")));
        assert!(crate::video_raw_refusal(rz, "bin").is_some_and(|w| w.contains("Rawzor")));
        assert!(crate::video_raw_refusal(&f, "RWZ").is_some_and(|w| w.contains("Outlook")));
    }

    #[test]
    fn truncated_and_hostile() {
        let samples = vec![300u16; 64];
        let f = file(8, 8, &samples);
        for n in [8, 27, 28, 700, DATA, f.len() - 1] {
            let b = &f[..n];
            let _ = crate::parse(b).map(|r| r.decode_raw(b, &Default::default()));
        }
        let mut big = f.clone();
        big[20..28].copy_from_slice(&[0xf0, 0xff, 0, 0, 0xf0, 0xff, 0, 0]);
        assert!(
            crate::parse(&big)
                .unwrap()
                .decode_raw(&big, &Default::default())
                .is_err()
        );
        assert_eq!(date_time(0x2019_1340, 0), None);
    }
}
