//! Andor SIF (`Andor Technology Multi-Channel File`): a text header of space- and
//! line-separated values and length-prefixed strings, then 32-bit float frames.
//!
//! The parts read here:
//! - the third line: the file version, the acquisition time (Unix seconds), the detector
//!   temperature, then (after binary fields) exposure, cycle and accumulated-cycle times,
//!   the accumulation count and the gain;
//! - the detector type (fourth line) and size;
//! - the wavelength calibration: the polynomial over pixel numbers from 1 on the line after
//!   the calibration-version line (for some files one polynomial per frame in the user
//!   text);
//! - the image structure, found by its three axis names (frame axis, data type, image
//!   axis, each after its length): the frame and sub-image counts and sizes, each sub-image's
//!   area and binning, a time stamp per frame, then the frames.

use std::sync::Arc;

use crate::spectral::{Num, Page, Spectral, polynomial};
use crate::{Error, Result};

pub const MAGIC: &[u8] = b"Andor Technology Multi-Channel File\n";
/// Most frames read.
const MAX_FRAMES: usize = 1 << 20;
/// How far into the file the header is looked for.
const HEADER_MAX: usize = 1 << 22;

pub fn sniff(head: &[u8]) -> bool {
    head.starts_with(MAGIC)
}

/// The line starting at `at` (without its newline) and where the next one starts.
fn line(b: &[u8], at: usize) -> Option<(&[u8], usize)> {
    let rest = b.get(at..)?;
    let n = rest.iter().position(|&c| c == b'\n')?;
    Some((&rest[..n], at + n + 1))
}

/// Printable tokens of a line (binary bytes count as separators).
fn tokens(l: &[u8]) -> Vec<String> {
    l.split(|c| !(0x21..0x7f).contains(c))
        .filter(|t| !t.is_empty())
        .map(|t| String::from_utf8_lossy(t).into_owned())
        .collect()
}

fn floats(l: &[u8]) -> Option<Vec<f64>> {
    let t = tokens(l);
    if t.is_empty() {
        return None;
    }
    t.iter().map(|v| v.parse().ok()).collect()
}

/// A length line followed by that many bytes of printable text, at `at`: the text and
/// where it ends.
fn counted(b: &[u8], at: usize) -> Option<(&str, usize)> {
    let (l, next) = line(b, at)?;
    if l.is_empty() || l.len() > 4 || !l.iter().all(u8::is_ascii_digit) {
        return None;
    }
    let n: usize = std::str::from_utf8(l).ok()?.parse().ok()?;
    let s = b.get(next..next + n)?;
    if n == 0 || !s.iter().all(|c| (0x20..0x7f).contains(c)) {
        return None;
    }
    Some((std::str::from_utf8(s).ok()?, next + n))
}

/// The image structure: (frame axis, data type, image axis) and where the structure line
/// starts.
fn find_axes(b: &[u8]) -> Option<([String; 3], usize, usize)> {
    let lim = b.len().min(HEADER_MAX);
    let mut i = 0;
    while i < lim {
        let nl = b[i..lim].iter().position(|&c| c == b'\n')?;
        let at = i + nl + 1;
        i = at;
        let Some((a, p)) = counted(b, at) else {
            continue;
        };
        let Some((d, q)) = counted(b, p) else {
            continue;
        };
        let Some((c, r)) = counted(b, q) else {
            continue;
        };
        // The structure line: a 655xx version and eight counts.
        let Some((l, _)) = line(b, r) else {
            continue;
        };
        let t = tokens(l);
        if t.len() >= 9
            && t[0].starts_with("655")
            && t[1..9].iter().all(|v| v.parse::<i64>().is_ok())
        {
            return Some(([a.to_owned(), d.to_owned(), c.to_owned()], at, r));
        }
    }
    None
}

pub fn parse(data: &[u8]) -> Result<Spectral> {
    if !sniff(data) {
        return Err(Error::Malformed("not a SIF file".into()));
    }
    let bad = |m: &str| Error::Malformed(format!("SIF: {m}"));
    let mut s = Spectral::default();
    // Lines 2 and 3.
    let (_, p) = line(data, MAGIC.len()).ok_or_else(|| bad("truncated"))?;
    let (l3, p) = line(data, p).ok_or_else(|| bad("truncated"))?;
    let t3 = tokens(l3);
    let num = |k: usize| {
        t3.get(k)
            .and_then(|v| v.parse::<f64>().ok())
            .filter(|v| v.is_finite())
    };
    let version = t3.first().cloned().unwrap_or_default();
    s.version = format!("SIF {version}");
    if let Some(t) = num(4).filter(|t| *t > 0.0) {
        s.date = unix(t as i64);
    }
    s.temperature = num(5);
    s.exposure = num(7);
    if let Some(c) = num(8) {
        s.facts.push(("Cycle time (s)".into(), format!("{c}")));
    }
    if let Some(c) = num(10) {
        s.facts.push(("Accumulations".into(), format!("{c}")));
    }
    if let Some(g) = num(15) {
        s.gain = Some(format!("{g}"));
    }
    let (l4, p) = line(data, p).ok_or_else(|| bad("truncated"))?;
    let det = String::from_utf8_lossy(l4).trim().to_owned();
    if !det.is_empty() {
        s.detector = Some(det);
    }
    let (l5, _) = line(data, p).ok_or_else(|| bad("truncated"))?;
    let dims = tokens(l5);
    if dims.len() >= 2 {
        s.facts
            .push(("Detector size".into(), format!("{}×{}", dims[0], dims[1])));
    }
    let (axes, axes_at, st) = find_axes(data).ok_or_else(|| bad("no image structure"))?;
    // The spectrograph: a "N name" line among the header lines (the readers' layouts move).
    let head = &data[..axes_at];
    for l in head.split(|&c| c == b'\n') {
        let t = tokens(l);
        if t.len() == 2
            && t[0].parse::<u32>().is_ok()
            && ["SR", "Shamrock", "Kymera", "Mechelle", "Holospec"]
                .iter()
                .any(|p| t[1].starts_with(p))
        {
            s.spectrograph = Some(t[1].clone());
        }
    }
    // Calibration: the line after the last calibration-version line ("655xx" with binary
    // fields) before the axes.
    let mut calib: Option<Vec<f64>> = None;
    let lines: Vec<&[u8]> = head.split(|&c| c == b'\n').collect();
    if let Some(k) = lines
        .iter()
        .rposition(|l| l.starts_with(b"655") && l.iter().any(|c| *c < 0x09 && *c != 0))
        .or_else(|| {
            lines
                .iter()
                .rposition(|l| l.starts_with(b"655") && l.contains(&0))
        })
    {
        calib = lines
            .get(k + 1)
            .and_then(|l| floats(l))
            .filter(|v| v.len() >= 2);
    }
    // Per-frame calibrations in the user text.
    let mut per_frame: Vec<Vec<f64>> = Vec::new();
    for l in head.split(|&c| c == b'\n') {
        if let Some(rest) = l.strip_prefix(b"Calibration data for frame ") {
            let text = String::from_utf8_lossy(rest);
            if let Some((_, v)) = text.split_once(':') {
                per_frame.push(v.split(',').filter_map(|x| x.trim().parse().ok()).collect());
            }
        }
    }
    // Structure.
    let (sl, mut q) = line(data, st).ok_or_else(|| bad("truncated structure"))?;
    let t = tokens(sl);
    let int = |k: usize| t.get(k).and_then(|v| v.parse::<i64>().ok()).unwrap_or(0);
    // The total length counts every frame; the image length one frame.
    let (frames, subs, total) = (int(5), int(6), int(8));
    if !(1..=MAX_FRAMES as i64).contains(&frames) || !(1..=4096).contains(&subs) || total <= 0 {
        return Err(bad("image counts"));
    }
    let (frames, subs, total) = (frames as usize, subs as usize, total as usize);
    let mut areas = Vec::new();
    for _ in 0..subs {
        let (l, n) = line(data, q).ok_or_else(|| bad("truncated sub-image"))?;
        q = n;
        let t = tokens(l);
        let v: Vec<i64> = t.iter().skip(1).filter_map(|x| x.parse().ok()).collect();
        if v.len() < 6 {
            return Err(bad("sub-image area"));
        }
        let (x0, y1, x1, y0, ybin, xbin) = (v[0], v[1], v[2], v[3], v[4].max(1), v[5].max(1));
        let w = (1 + x1 - x0) / xbin;
        let h = (1 + y1 - y0) / ybin;
        if w <= 0 || h <= 0 {
            return Err(bad("sub-image size"));
        }
        areas.push((
            w as usize,
            h as usize,
            [x0.max(0) as u32, y0.max(0) as u32, xbin as u32, ybin as u32],
        ));
    }
    // Time stamps (after optional blank space), then a 0 or a 1 (with one more number per
    // frame in the newest version) before the frames.
    while data.get(q).is_some_and(|c| *c == b' ' || *c == b'\n') {
        q += 1;
    }
    for _ in 0..frames {
        let (_, n) = line(data, q).ok_or_else(|| bad("truncated time stamps"))?;
        q = n;
    }
    let frame_bytes = total * 4;
    let need = frames * frame_bytes;
    let fits = |at: usize| at + need <= data.len();
    if let Some((l, n)) = line(data, q) {
        let t = tokens(l);
        let flag = (t.len() == 1).then(|| t[0].parse::<i64>().ok()).flatten();
        let printable = l.iter().all(|c| (0x20..0x7f).contains(c));
        match flag {
            Some(0) if printable && fits(n) => q = n,
            Some(1) if printable && version == "65567" => {
                let mut r = n;
                for _ in 0..frames {
                    match line(data, r) {
                        Some((_, x)) => r = x,
                        None => break,
                    }
                }
                if fits(r) {
                    q = r;
                }
            }
            _ => {}
        }
    }
    if !fits(q) {
        return Err(Error::Truncated("SIF frames"));
    }
    s.frames = frames;
    let width = areas[0].0;
    let height: usize = areas.iter().map(|a| a.1).sum();
    if width * height != total {
        return Err(bad("sub-image sizes do not add up to the frame"));
    }
    let wl_axis =
        axes[0].eq_ignore_ascii_case("Wavelength") || axes[2].eq_ignore_ascii_case("Wavelength");
    let common = calib.as_deref().and_then(|c| polynomial(c, width, 1.0));
    for f in 0..frames {
        let wavelengths: Option<Arc<[f64]>> = per_frame
            .get(f)
            .and_then(|c| polynomial(c, width, 1.0))
            .or_else(|| common.clone());
        s.pages.push(Page {
            frame: f,
            region: 0,
            width,
            height,
            offset: q + f * frame_bytes,
            sample: Num::F32,
            wavelengths,
            sensor: (areas.len() == 1).then_some(areas[0].2),
        });
    }
    s.facts.push((
        "Axes".into(),
        format!("{}, {}, {}", axes[0], axes[1], axes[2]),
    ));
    if wl_axis && common.is_none() && per_frame.is_empty() {
        s.facts.push(("Calibration".into(), "none".into()));
    }
    Ok(s)
}

/// "2024-06-21 10:30:38 UTC" from Unix seconds.
fn unix(s: i64) -> Option<String> {
    if !(0..=253_402_300_799).contains(&s) {
        return None;
    }
    let days = s.div_euclid(86_400);
    let rem = s.rem_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    Some(format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02} UTC",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sif(frames: usize, calib: &str) -> Vec<u8> {
        let mut b = MAGIC.to_vec();
        b.extend_from_slice(b"65538 1\n65567 0 0 1 1504875832 -50 \x00 \x00 \x00 \x02 \x00 0 0.1 0.2 0.2 3 \x00 1 1e-06 0 1 300 0 0 0\n");
        b.extend_from_slice(b"DU401_BV\n1024 255 \n");
        b.extend_from_slice(b"65540 \x02 \x00 \x01 \x00 \x01 \x00\n");
        b.extend_from_slice(calib.as_bytes());
        b.extend_from_slice(
            b"\n0 1 0 0\n0 1 0 0\n422\n13\n13\n1\n10\nWavelength6\nCounts12\nPixel number",
        );
        b.extend_from_slice(
            format!(
                "65541 1 4 2 1 {frames} 1 {} 8\n65538 1 2 4 1 1 1 0\n",
                frames * 8
            )
            .as_bytes(),
        );
        for _ in 0..frames {
            b.extend_from_slice(b"         0\n");
        }
        b.extend_from_slice(b"0\n");
        for i in 0..frames * 8 {
            b.extend_from_slice(&(i as f32).to_le_bytes());
        }
        b
    }

    #[test]
    fn frames_calibration_and_facts() {
        let b = sif(2, "500 2 0 0");
        let s = parse(&b).unwrap();
        assert_eq!(s.version, "SIF 65567");
        assert_eq!(s.pages.len(), 2);
        assert_eq!((s.pages[0].width, s.pages[0].height), (4, 2));
        assert_eq!(s.temperature, Some(-50.0));
        assert_eq!(s.exposure, Some(0.1));
        assert_eq!(s.detector.as_deref(), Some("DU401_BV"));
        assert_eq!(
            s.pages[0].wavelengths.as_deref(),
            Some(&[502.0, 504.0, 506.0, 508.0][..])
        );
        let (v, t) = s.pages[1].read(&b).unwrap();
        assert!(!t);
        assert_eq!(v[0], 8.0);
        // The identity polynomial is no calibration.
        let u = parse(&sif(1, "0 1 0 0")).unwrap();
        assert!(u.pages[0].wavelengths.is_none());
    }

    #[test]
    fn hostile_files_are_refused() {
        assert!(parse(MAGIC).is_err());
        let mut b = sif(2, "500 2 0 0");
        b.truncate(b.len() - 5);
        assert!(parse(&b).is_err());
    }
}
