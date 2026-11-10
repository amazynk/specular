//! Galactic/Thermo SPC spectra (GRAMS): the new format (least or most significant byte
//! first) and the old one (`0x4D`), single and multi-file, evenly spaced, with one X array
//! (`TXVALS`) or with X values per subfile (`TXYXYS`, with or without a directory).
//!
//! Ordinates are IEEE floats when the exponent is `0x80`, else 32-bit (or, with `TSPREC`,
//! 16-bit) integers scaled by 2^(exponent − 32) (2^(exponent − 16)); the old format stores
//! its 32-bit integers with their 16-bit halves swapped. Each subfile is a page.

use specular_spectra::{Axis, Error, Facts, Plot, Spectra, Technique, Trace, warn};

/// Subfiles read at most (pages).
const MAX_SUBFILES: usize = 10_000;
/// Values in a file at most.
const MAX_VALUES: usize = 64 << 20;
/// Log fields kept at most.
const MAX_FIELDS: usize = 300;

const TSPREC: u8 = 0x01;
const TCGRAM: u8 = 0x02;
const TMULTI: u8 = 0x04;
const TRANDM: u8 = 0x08;
const TORDRD: u8 = 0x10;
const TALABS: u8 = 0x20;
const TXYXYS: u8 = 0x40;
const TXVALS: u8 = 0x80;

#[derive(Clone, Copy, PartialEq)]
enum Order {
    Lsb,
    Msb,
}

struct R<'a> {
    d: &'a [u8],
    o: Order,
}

impl R<'_> {
    fn bytes<const N: usize>(&self, at: usize) -> Option<[u8; N]> {
        self.d.get(at..at.checked_add(N)?)?.try_into().ok()
    }
    fn u16(&self, at: usize) -> Option<u16> {
        let b = self.bytes::<2>(at)?;
        Some(match self.o {
            Order::Lsb => u16::from_le_bytes(b),
            Order::Msb => u16::from_be_bytes(b),
        })
    }
    fn u32(&self, at: usize) -> Option<u32> {
        let b = self.bytes::<4>(at)?;
        Some(match self.o {
            Order::Lsb => u32::from_le_bytes(b),
            Order::Msb => u32::from_be_bytes(b),
        })
    }
    fn i32(&self, at: usize) -> Option<i32> {
        self.u32(at).map(|v| v as i32)
    }
    fn i16(&self, at: usize) -> Option<i16> {
        self.u16(at).map(|v| v as i16)
    }
    fn f32(&self, at: usize) -> Option<f32> {
        self.u32(at).map(f32::from_bits)
    }
    fn f64(&self, at: usize) -> Option<f64> {
        let b = self.bytes::<8>(at)?;
        Some(match self.o {
            Order::Lsb => f64::from_le_bytes(b),
            Order::Msb => f64::from_be_bytes(b),
        })
    }
    fn text(&self, at: usize, len: usize) -> String {
        let s = self.d.get(at..(at + len).min(self.d.len())).unwrap_or(&[]);
        let s = s.split(|&c| c == 0).next().unwrap_or(&[]);
        let t: String = s.iter().map(|&c| char::from(c)).collect();
        t.split_whitespace().collect::<Vec<_>>().join(" ")
    }
}

fn axis_ok(x: u8, y: u8) -> bool {
    (x <= 30 || x == 255) && (y <= 26 || (128..=131).contains(&y))
}

/// Whether `head` (at least the first 512 bytes) is an SPC file: the version byte and a
/// header whose counts, axis codes and limits make sense.
pub fn sniff(head: &[u8]) -> bool {
    let Some(&v) = head.get(1) else {
        return false;
    };
    match v {
        0x4B | 0x4C => {
            if head.len() < 512 {
                return false;
            }
            let r = R {
                d: head,
                o: if v == 0x4B { Order::Lsb } else { Order::Msb },
            };
            let flags = head[0];
            let (Some(npts), Some(first), Some(last), Some(nsub)) =
                (r.u32(4), r.f64(8), r.f64(16), r.u32(24))
            else {
                return false;
            };
            let npts_ok = flags & TXYXYS != 0 || (1..=50_000_000).contains(&npts);
            head[2] <= 0x3F
                && npts_ok
                && (1..=10_000_000).contains(&nsub)
                && (flags & TMULTI != 0 || nsub == 1)
                && first.is_finite()
                && last.is_finite()
                && axis_ok(head[28], head[29])
        }
        0x4D => {
            if head.len() < 256 {
                return false;
            }
            let r = R {
                d: head,
                o: Order::Lsb,
            };
            let (Some(exp), Some(npts), Some(first), Some(last)) =
                (r.i16(2), r.f32(4), r.f32(8), r.f32(12))
            else {
                return false;
            };
            (-128..=128).contains(&exp)
                && (1.0..=5e7).contains(&npts)
                && npts.fract() == 0.0
                && first.is_finite()
                && last.is_finite()
                && axis_ok(head[16], head[17])
        }
        _ => false,
    }
}

/// Whether the bytes are an OLE compound document, as Shimadzu's `.spc` files are.
fn is_ole(d: &[u8]) -> bool {
    d.starts_with(&[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1])
}

/// Reads an SPC file.
pub fn parse(d: &[u8]) -> Result<Spectra, Error> {
    if is_ole(d) {
        return Err(Error::Unsupported(
            "This .spc file is a Shimadzu spectrum (an OLE compound document), a different \
             format from Galactic SPC; it is not supported."
                .into(),
        ));
    }
    match d.get(1) {
        Some(0x4B) => new_format(d, Order::Lsb),
        Some(0x4C) => new_format(d, Order::Msb),
        Some(0x4D) => old_format(d),
        Some(0xCF) => Err(Error::Unsupported(
            "This SPC file is in Shimadzu's own variant, which is not supported.".into(),
        )),
        _ => Err(Error::NotThisFormat),
    }
}

/// What the header's axis codes and texts say.
struct Axes {
    x: Axis,
    y: Axis,
    z: Axis,
}

fn x_axis(code: u8) -> Axis {
    let (t, u, rev) = match code {
        1 => ("Wavenumber", "cm⁻¹", true),
        2 => ("Wavelength", "µm", false),
        3 => ("Wavelength", "nm", false),
        4 => ("Time", "s", false),
        5 => ("Time", "min", false),
        6 => ("Frequency", "Hz", false),
        7 => ("Frequency", "kHz", false),
        8 => ("Frequency", "MHz", false),
        9 => ("m/z", "", false),
        10 => ("Chemical shift", "ppm", true),
        11 => ("Time", "days", false),
        12 => ("Time", "years", false),
        13 => ("Raman shift", "cm⁻¹", false),
        14 => ("Energy", "eV", false),
        16 => ("Diode", "", false),
        17 => ("Channel", "", false),
        18 => ("Angle", "°", false),
        19 => ("Temperature", "°F", false),
        20 => ("Temperature", "°C", false),
        21 => ("Temperature", "K", false),
        22 => ("Data point", "", false),
        23 => ("Time", "ms", false),
        24 => ("Time", "µs", false),
        25 => ("Time", "ns", false),
        26 => ("Frequency", "GHz", false),
        27 => ("Length", "cm", false),
        28 => ("Length", "m", false),
        29 => ("Length", "mm", false),
        30 => ("Time", "h", false),
        255 => ("Optical path difference", "", false),
        _ => ("X", "", false),
    };
    Axis::new(t, u, rev)
}

fn y_axis(code: u8) -> Axis {
    let (t, u) = match code {
        1 => ("Interferogram", ""),
        2 => ("Absorbance", ""),
        3 => ("Kubelka-Munk", ""),
        4 => ("Counts", ""),
        5 => ("Voltage", "V"),
        6 => ("Angle", "°"),
        7 => ("Current", "mA"),
        8 => ("Length", "mm"),
        9 => ("Voltage", "mV"),
        10 => ("log(1/R)", ""),
        11 => ("Percent", "%"),
        12 => ("Intensity", ""),
        13 => ("Relative intensity", ""),
        14 => ("Energy", ""),
        16 => ("Level", "dB"),
        19 => ("Temperature", "°F"),
        20 => ("Temperature", "°C"),
        21 => ("Temperature", "K"),
        22 => ("Refractive index", ""),
        23 => ("Extinction coefficient", ""),
        24 => ("Real", ""),
        25 => ("Imaginary", ""),
        26 => ("Complex", ""),
        128 => ("Transmittance", ""),
        129 => ("Reflectance", ""),
        130 => ("Single beam", ""),
        131 => ("Emission", ""),
        _ => ("Intensity", ""),
    };
    Axis::new(t, u, false)
}

fn axes(xt: u8, yt: u8, zt: u8, flags: u8, catxt: &[u8]) -> Axes {
    let mut a = Axes {
        x: x_axis(xt),
        y: y_axis(yt),
        z: x_axis(zt),
    };
    if flags & TALABS != 0 {
        let parts: Vec<String> = catxt
            .split(|&c| c == 0)
            .map(|s| s.iter().map(|&c| char::from(c)).collect::<String>())
            .map(|s| s.trim().to_string())
            .collect();
        for (ax, lbl) in [&mut a.x, &mut a.y, &mut a.z].into_iter().zip(parts) {
            if lbl.is_empty() {
                continue;
            }
            // "Wavenumber (cm-1)": a title and its unit; "(arb)": a unit only.
            match lbl.strip_suffix(')').and_then(|l| l.rsplit_once('(')) {
                Some((t, u)) => {
                    let t = t.trim();
                    if !t.is_empty() {
                        ax.title = t.to_string();
                    }
                    ax.unit = u.trim().to_string();
                }
                None => {
                    ax.title = lbl;
                    ax.unit = String::new();
                }
            }
        }
    }
    a
}

fn technique(exper: u8, xt: u8) -> Technique {
    match exper {
        1..=3 => Technique::Chromatogram,
        4 if xt == 13 => Technique::Raman,
        4 | 5 => Technique::Infrared,
        6 => Technique::UvVis,
        8 => Technique::Mass,
        9 => Technique::Nmr,
        10 => Technique::Raman,
        11 => Technique::Fluorescence,
        _ => match xt {
            1 => Technique::Infrared,
            13 => Technique::Raman,
            9 => Technique::Mass,
            10 => Technique::Nmr,
            _ => Technique::Other,
        },
    }
}

const EXPERIMENTS: [&str; 14] = [
    "General",
    "Gas chromatogram",
    "General chromatogram",
    "HPLC chromatogram",
    "FT-IR, FT-NIR or FT-Raman",
    "NIR",
    "UV-Vis",
    "X-ray diffraction",
    "Mass spectrum",
    "NMR spectrum or FID",
    "Raman",
    "Fluorescence",
    "Atomic",
    "Chromatography diode array",
];

/// Ordinates of a subfile: `n` values at `at` with exponent `exp`.
fn ordinates(
    r: &R<'_>,
    at: usize,
    n: usize,
    exp: i32,
    short: bool,
    swapped: bool,
) -> Option<Vec<f64>> {
    let width = if short && exp != -128 && exp != 128 {
        2
    } else {
        4
    };
    if at.checked_add(n.checked_mul(width)?)? > r.d.len() {
        return None;
    }
    let mut y = Vec::with_capacity(n);
    if exp == -128 || exp == 128 {
        for i in 0..n {
            y.push(f64::from(r.f32(at + 4 * i)?));
        }
    } else if short {
        let s = 2f64.powi(exp - 16);
        for i in 0..n {
            y.push(f64::from(r.i16(at + 2 * i)?) * s);
        }
    } else {
        let s = 2f64.powi(exp - 32);
        for i in 0..n {
            let v = if swapped {
                let b = r.bytes::<4>(at + 4 * i)?;
                i32::from_be_bytes([b[1], b[0], b[3], b[2]])
            } else {
                r.i32(at + 4 * i)?
            };
            y.push(f64::from(v) * s);
        }
    }
    Some(y)
}

fn date(packed: u32) -> Option<String> {
    let year = packed >> 20;
    if year == 0 {
        return None;
    }
    let month = (packed >> 16) & 0xF;
    let day = (packed >> 11) & 0x1F;
    let hour = (packed >> 6) & 0x1F;
    let minute = packed & 0x3F;
    Some(format!(
        "{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}"
    ))
}

fn linspace(first: f64, last: f64, n: usize) -> Vec<f64> {
    if n <= 1 {
        return vec![first; n];
    }
    let step = (last - first) / (n - 1) as f64;
    (0..n).map(|i| first + step * i as f64).collect()
}

fn short(v: f64) -> String {
    let s = format!("{v:.4}");
    let s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    if s == "-0" { "0".into() } else { s }
}

fn new_format(d: &[u8], o: Order) -> Result<Spectra, Error> {
    let r = R { d, o };
    if d.len() < 512 {
        return Err(Error::Truncated("the SPC header is cut short".into()));
    }
    let flags = d[0];
    let exper = d[2];
    let fexp = i32::from(d[3] as i8);
    let fnpts = r.u32(4).unwrap_or(0) as usize;
    let first = r.f64(8).unwrap_or(0.0);
    let last = r.f64(16).unwrap_or(0.0);
    let nsub = r.u32(24).unwrap_or(0) as usize;
    let (xt, yt, zt) = (d[28], d[29], d[30]);
    let a = axes(xt, yt, zt, flags, &d[218..248]);
    let logoff = r.u32(248).unwrap_or(0) as usize;
    let zinc = r.f32(312).unwrap_or(0.0);
    let mut warnings = Vec::new();
    let mut facts = Facts {
        format: "SPC".into(),
        version: match o {
            Order::Lsb => "new format".into(),
            Order::Msb => "new format (most significant byte first)".into(),
        },
        technique: technique(exper, xt),
        title: Some(r.text(88, 130)).filter(|s| !s.is_empty()),
        instrument: Some(r.text(45, 9)).filter(|s| !s.is_empty()),
        date: date(r.u32(32).unwrap_or(0)),
        ..Facts::default()
    };
    if let Some(e) = EXPERIMENTS.get(usize::from(exper)) {
        facts.fields.push(("Experiment".into(), (*e).into()));
    }
    for (label, at, len) in [("Resolution", 36, 9), ("Method", 264, 48)] {
        let t = r.text(at, len);
        if !t.is_empty() {
            facts.fields.push((label.into(), t));
        }
    }
    if flags & TCGRAM != 0 {
        facts.fields.push(("Display".into(), "Chromatogram".into()));
    }
    if logoff >= 512 {
        log(&r, logoff, &mut facts.fields, &mut warnings);
    }
    let multi = flags & TMULTI != 0;
    let xyxy = flags & TXYXYS != 0;
    let short_y = flags & TSPREC != 0;
    if nsub == 0 {
        return Err(Error::Malformed("the SPC file has no subfiles".into()));
    }
    let mut pos = 512;
    let mut global_x = None;
    // The first subfile must fit before anything is sized by the header's counts.
    if !xyxy {
        let x_bytes = if flags & TXVALS != 0 { 4 * fnpts } else { 0 };
        let width = if short_y && fexp != -128 { 2 } else { 4 };
        if 512 + x_bytes + 32 + fnpts.saturating_mul(width) > d.len() {
            return Err(Error::Truncated("the SPC data is cut short".into()));
        }
    }
    if !xyxy {
        if flags & TXVALS != 0 {
            let mut x = Vec::with_capacity(fnpts.min(d.len() / 4));
            for i in 0..fnpts {
                x.push(f64::from(r.f32(512 + 4 * i).ok_or_else(|| {
                    Error::Truncated("the X values are cut short".into())
                })?));
            }
            pos += 4 * fnpts;
            global_x = Some(x);
        } else {
            global_x = Some(linspace(first, last, fnpts));
        }
    }
    // Subfile starts: from the directory of an XYXY file, else one after another.
    let directory: Option<Vec<usize>> = (xyxy && fnpts > 0).then(|| {
        (0..nsub.min(MAX_SUBFILES))
            .filter_map(|i| r.u32(fnpts + 12 * i).map(|p| p as usize))
            .collect()
    });
    let mut pages = Vec::new();
    let mut values = 0usize;
    let count = nsub.min(MAX_SUBFILES);
    if nsub > MAX_SUBFILES {
        warn(
            &mut warnings,
            format!("Only the first {MAX_SUBFILES} of {nsub} subfiles are shown."),
        );
    }
    for i in 0..count {
        let at = match &directory {
            Some(dir) => match dir.get(i) {
                Some(&p) => p,
                None => break,
            },
            None => pos,
        };
        if at + 32 > d.len() {
            warn(
                &mut warnings,
                format!("The file ends after {i} of {nsub} subfiles."),
            );
            break;
        }
        let subexp = i32::from(d[at + 1] as i8);
        let subtime = r.f32(at + 4).unwrap_or(0.0);
        let subnext = r.f32(at + 8).unwrap_or(0.0);
        let subnpts = r.u32(at + 16).unwrap_or(0) as usize;
        let n = if xyxy { subnpts } else { fnpts };
        values += n;
        if values > MAX_VALUES {
            return Err(Error::TooLarge);
        }
        let mut data = at + 32;
        let x = if xyxy {
            let mut x = Vec::with_capacity(n.min(d.len() / 4));
            for k in 0..n {
                match r.f32(data + 4 * k) {
                    Some(v) => x.push(f64::from(v)),
                    None => break,
                }
            }
            data += 4 * n;
            x
        } else {
            global_x.clone().unwrap_or_default()
        };
        let exp = if multi { subexp } else { fexp };
        let Some(y) = ordinates(&r, data, n, exp, short_y, false) else {
            warn(
                &mut warnings,
                format!("The file ends inside subfile {}.", i + 1),
            );
            break;
        };
        if x.len() != y.len() {
            warn(
                &mut warnings,
                format!("Subfile {} has fewer X values than points.", i + 1),
            );
            break;
        }
        pos = data + n * if short_y && exp != -128 { 2 } else { 4 };
        let title = if count == 1 {
            "Spectrum".to_string()
        } else {
            let z = if flags & (TRANDM | TORDRD) != 0 || subtime != 0.0 || subnext != 0.0 {
                f64::from(subtime)
            } else {
                f64::from(zinc) * i as f64
            };
            if a.z.title == "X" {
                format!("Subfile {}", i + 1)
            } else if a.z.unit.is_empty() {
                format!("{} {}", a.z.title, short(z))
            } else {
                format!("{} {} {}", a.z.title, short(z), a.z.unit)
            }
        };
        pages.push(Plot {
            title,
            x: a.x.clone(),
            y: a.y.clone(),
            traces: vec![Trace {
                name: String::new(),
                x,
                y,
                sticks: false,
            }],
            ..Plot::default()
        });
    }
    finish(facts, pages, warnings)
}

fn finish(mut facts: Facts, pages: Vec<Plot>, warnings: Vec<String>) -> Result<Spectra, Error> {
    if pages.is_empty() {
        return Err(Error::Truncated("no subfile could be read".into()));
    }
    facts.spectra = pages.len() as u64;
    facts.points = pages
        .iter()
        .flat_map(|p| p.traces.iter())
        .map(|t| t.y.len() as u64)
        .max()
        .unwrap_or(0);
    facts.warnings = warnings;
    Ok(Spectra { facts, pages })
}

/// The log block's text: `KEY=VALUE` lines as fields.
fn log(r: &R<'_>, at: usize, fields: &mut Vec<(String, String)>, warnings: &mut Vec<String>) {
    let (Some(size), Some(txto)) = (r.u32(at), r.u32(at + 8)) else {
        warn(warnings, "The log block is cut short.");
        return;
    };
    let start = at.saturating_add(txto as usize);
    let end = at.saturating_add(size as usize).min(r.d.len());
    let Some(text) = r.d.get(start..end.max(start)) else {
        return;
    };
    let text = text.split(|&c| c == 0).next().unwrap_or(&[]);
    let text: String = text.iter().map(|&c| char::from(c)).collect();
    for line in text.lines() {
        if fields.len() >= MAX_FIELDS {
            break;
        }
        if let Some((k, v)) = line.split_once('=') {
            let (k, v) = (k.trim(), v.trim());
            if !k.is_empty() && !v.is_empty() {
                fields.push((k.to_string(), v.to_string()));
            }
        }
    }
}

fn old_format(d: &[u8]) -> Result<Spectra, Error> {
    let r = R { d, o: Order::Lsb };
    if d.len() < 256 {
        return Err(Error::Truncated("the SPC header is cut short".into()));
    }
    let flags = d[0];
    let oexp = i32::from(r.i16(2).unwrap_or(0));
    let onpts = r.f32(4).unwrap_or(0.0);
    let first = f64::from(r.f32(8).unwrap_or(0.0));
    let last = f64::from(r.f32(12).unwrap_or(0.0));
    let (xt, yt) = (d[16], d[17]);
    if !(1.0..=5e7).contains(&onpts) {
        return Err(Error::Malformed(
            "the SPC header gives no point count".into(),
        ));
    }
    let npts = onpts as usize;
    if 256 + 4 * npts > d.len() {
        return Err(Error::Truncated("the spectrum is cut short".into()));
    }
    let a = axes(xt, yt, 0, flags, &d[194..224]);
    let year = r.u16(18).unwrap_or(0) & 0x0FFF;
    let mut facts = Facts {
        format: "SPC".into(),
        version: "old format".into(),
        technique: technique(0, xt),
        title: Some(r.text(64, 130)).filter(|s| !s.is_empty()),
        date: (year > 0).then(|| {
            format!(
                "{year:04}-{:02}-{:02} {:02}:{:02}",
                d[20], d[21], d[22], d[23]
            )
        }),
        ..Facts::default()
    };
    let res = r.text(24, 8);
    if !res.is_empty() {
        facts.fields.push(("Resolution".into(), res));
    }
    let mut warnings = Vec::new();
    let x = linspace(first, last, npts);
    let mut pages = Vec::new();
    let mut pos = 224;
    let mut values = 0usize;
    while pos + 32 <= d.len() && pages.len() < MAX_SUBFILES {
        let subexp = i32::from(d[pos + 1]);
        let subnpts = r.u32(pos + 16).unwrap_or(0) as usize;
        let n = if subnpts > 0 { subnpts } else { npts };
        let data = pos + 32;
        if data + 4 * n > d.len() {
            if pages.is_empty() {
                return Err(Error::Truncated("the spectrum is cut short".into()));
            }
            break;
        }
        values += n;
        if values > MAX_VALUES {
            return Err(Error::TooLarge);
        }
        let (exp, swapped) = if subexp == 128 {
            (-128, false)
        } else if subexp > 0 && subexp < 128 {
            (subexp, true)
        } else {
            (oexp, true)
        };
        let y = ordinates(&r, data, n, exp, false, swapped)
            .ok_or_else(|| Error::Truncated("the spectrum is cut short".into()))?;
        let xs = if n == npts {
            x.clone()
        } else {
            linspace(first, last, n)
        };
        pages.push(Plot {
            title: String::new(),
            x: a.x.clone(),
            y: a.y.clone(),
            traces: vec![Trace {
                name: String::new(),
                x: xs,
                y,
                sticks: false,
            }],
            ..Plot::default()
        });
        pos = data + 4 * n;
    }
    if pos + 32 <= d.len() && pages.len() >= MAX_SUBFILES {
        warn(
            &mut warnings,
            format!("Only the first {MAX_SUBFILES} subfiles are shown."),
        );
    }
    let count = pages.len();
    for (i, p) in pages.iter_mut().enumerate() {
        p.title = if count == 1 {
            "Spectrum".into()
        } else {
            format!("Subfile {}", i + 1)
        };
    }
    finish(facts, pages, warnings)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(flags: u8, exp: i8, npts: u32, nsub: u32) -> Vec<u8> {
        let mut h = vec![0u8; 512];
        h[0] = flags;
        h[1] = 0x4B;
        h[3] = exp as u8;
        h[4..8].copy_from_slice(&npts.to_le_bytes());
        h[8..16].copy_from_slice(&400.0f64.to_le_bytes());
        h[16..24].copy_from_slice(&4000.0f64.to_le_bytes());
        h[24..28].copy_from_slice(&nsub.to_le_bytes());
        h[28] = 1;
        h[29] = 2;
        h
    }

    #[test]
    fn reads_floats_and_integers() {
        let mut f = header(0, -128, 3, 1);
        f.extend([0u8; 32]);
        for v in [1.0f32, 2.0, 3.0] {
            f.extend(v.to_le_bytes());
        }
        assert!(sniff(&f));
        let s = parse(&f).expect("parses");
        assert_eq!(s.pages[0].traces[0].y, vec![1.0, 2.0, 3.0]);
        assert_eq!(s.pages[0].traces[0].x, vec![400.0, 2200.0, 4000.0]);
        assert_eq!(s.pages[0].x.title, "Wavenumber");
        assert_eq!(s.facts.technique, Technique::Infrared);
        // 32-bit integers with exponent 16: value / 65536.
        let mut f = header(0, 16, 2, 1);
        f.extend([0u8; 32]);
        for v in [65536i32, -131072] {
            f.extend(v.to_le_bytes());
        }
        assert_eq!(
            parse(&f).expect("parses").pages[0].traces[0].y,
            vec![1.0, -2.0]
        );
    }

    #[test]
    fn refuses_other_files() {
        assert!(!sniff(b"\x00\x4B"));
        assert!(!sniff(&[0u8; 512]));
        let ole = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];
        assert!(matches!(parse(&ole), Err(Error::Unsupported(_))));
        assert!(matches!(parse(b"xx"), Err(Error::NotThisFormat)));
    }

    #[test]
    fn dates() {
        let p = (2024 << 20) | (3 << 16) | (9 << 11) | (14 << 6) | 5;
        assert_eq!(date(p).as_deref(), Some("2024-03-09 14:05"));
    }
}
