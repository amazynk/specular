//! Scientific conventions built on TIFF, read from the tags as stored:
//!
//! - **ImageJ** hyperstacks: the `ImageJ=` ImageDescription (channels, slices, frames,
//!   spacing, unit, frame interval, display range, calibration function) and the binary
//!   IJMetadata tags 50838/50839 (slice labels, per-channel display ranges and LUTs, info,
//!   properties);
//! - **Zeiss LSM**: the CZ_LSMINFO tag 34412 (dimensions, voxel size, scan type, time
//!   interval) and the blocks it points to (channel colours and names, time stamps);
//! - **MetaMorph STK**: the UIC1–UIC3 tags 33628–33630 (planes, spatial calibration, z
//!   distances, wavelengths);
//! - **Olympus FluoView**: the MM_Header tag 34361 (image name, up to ten dimensions with
//!   their sizes, origins, steps and units) beside MM_Stamp 34362;
//! - **Molecular Dynamics GEL**: the MD tags 33445–33452 (square-root coding and scale);
//! - **SEM** vendors: Zeiss (CZ_SEM 34118), FEI / Thermo Fisher (34680 FEI_SFEG, 34682
//!   FEI_HELIOS), TESCAN (50431), and the text sidecars of Hitachi and JEOL microscopes;
//! - the baseline resolution tags.
//!
//! Everything here is parsing only; the caller decides what to show.

use crate::Directories;

pub const IMAGE_DESCRIPTION: u16 = 270;
pub const X_RESOLUTION: u16 = 282;
pub const Y_RESOLUTION: u16 = 283;
pub const RESOLUTION_UNIT: u16 = 296;
pub const SOFTWARE: u16 = 305;
pub const DATE_TIME: u16 = 306;
pub const PHOTOSHOP: u16 = 34377;
pub const IJ_META_COUNTS: u16 = 50838;
pub const IJ_META: u16 = 50839;
pub const CZ_LSMINFO: u16 = 34412;
pub const UIC1: u16 = 33628;
pub const UIC2: u16 = 33629;
pub const UIC3: u16 = 33630;
pub const MM_HEADER: u16 = 34361;
pub const MM_STAMP: u16 = 34362;
pub const MD_FILE_TAG: u16 = 33445;
pub const MD_SCALE_PIXEL: u16 = 33446;
pub const MD_LAB_NAME: u16 = 33448;
pub const MD_SAMPLE_INFO: u16 = 33449;
pub const MD_PREP_DATE: u16 = 33450;
pub const MD_PREP_TIME: u16 = 33451;
pub const MD_FILE_UNITS: u16 = 33452;
pub const CZ_SEM: u16 = 34118;
pub const FEI_SFEG: u16 = 34680;
pub const FEI_HELIOS: u16 = 34682;
pub const TESCAN: u16 = 50431;

/// A 256-entry RGB colour table (an ImageJ LUT).
pub type Lut = Box<[[u8; 3]; 256]>;

/// Upper bound on the planes a stack may declare (guards allocation from damaged headers).
pub const MAX_PLANES: u64 = 1 << 20;

// ----- Units ----------------------------------------------------------------------------

/// A length unit as ImageJ, MetaMorph and the SEM vendors write it, in the viewer's spelling
/// ("micron", "µm", "um" → "µm"); `None` for "pixel" (uncalibrated) and empty text.
pub fn normalise_unit(u: &str) -> Option<String> {
    let t = u.trim().trim_matches('"').trim();
    let low = t.to_ascii_lowercase();
    let s = match low.as_str() {
        "" | "pixel" | "pixels" | "px" => return None,
        "micron" | "microns" | "um" | "\\u00b5m" | "micrometer" | "micrometre" | "micrometers"
        | "micrometres" => "µm",
        "nanometer" | "nanometre" | "nanometers" | "nanometres" => "nm",
        "millimeter" | "millimetre" | "millimeters" | "millimetres" => "mm",
        "centimeter" | "centimetre" | "centimeters" | "centimetres" => "cm",
        "meter" | "metre" | "meters" | "metres" => "m",
        "angstrom" | "angstroms" | "a" | "\\u00c5" | "\\u212b" => "Å",
        "inches" => "inch",
        _ => {
            // "µm" written with a real micro or mu sign.
            if t == "\u{3bc}m" {
                return Some("µm".into());
            }
            return Some(t.to_owned());
        }
    };
    Some(s.to_owned())
}

/// Metres per `unit` for the lengths SEM vendors write ("nm", "µm", "mm"…).
pub fn metres_per(unit: &str) -> Option<f64> {
    Some(match unit.trim() {
        "pm" => 1e-12,
        "Å" | "A" => 1e-10,
        "nm" => 1e-9,
        "µm" | "um" | "\u{3bc}m" | "micron" => 1e-6,
        "mm" => 1e-3,
        "cm" => 1e-2,
        "m" => 1.0,
        "km" => 1e3,
        _ => return None,
    })
}

// ----- Resolution -------------------------------------------------------------------------

/// The baseline resolution: pixels per unit across and down, and the unit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Resolution {
    pub x: f64,
    pub y: f64,
    pub unit: ResolutionUnit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResolutionUnit {
    /// ResolutionUnit 1: no absolute unit (ImageJ stores calibrated sizes this way and names
    /// the unit in its description).
    None,
    Inch,
    Centimetre,
}

impl Resolution {
    /// The resolution tags of directory `d`; `None` without a positive XResolution.
    pub fn read(dirs: &Directories, data: &[u8], d: usize) -> Option<Resolution> {
        let x = *dirs.reals(data, d, X_RESOLUTION)?.first()?;
        let y = dirs
            .reals(data, d, Y_RESOLUTION)
            .and_then(|v| v.first().copied())
            .filter(|y| y.is_finite() && *y > 0.0)
            .unwrap_or(x);
        if !(x.is_finite() && x > 0.0) {
            return None;
        }
        let unit = match dirs
            .uints(data, d, RESOLUTION_UNIT)
            .and_then(|v| v.first().copied())
            .unwrap_or(2)
        {
            1 => ResolutionUnit::None,
            3 => ResolutionUnit::Centimetre,
            _ => ResolutionUnit::Inch,
        };
        Some(Resolution { x, y, unit })
    }

    /// Pixel size (x, y) in metres, for an inch or centimetre unit.
    pub fn pixel_metres(&self) -> Option<(f64, f64)> {
        let m = match self.unit {
            ResolutionUnit::None => return None,
            ResolutionUnit::Inch => 0.0254,
            ResolutionUnit::Centimetre => 0.01,
        };
        Some((m / self.x, m / self.y))
    }
}

// ----- ImageJ -----------------------------------------------------------------------------

/// The `ImageJ=` description of a file ImageJ or Fiji wrote (or tifffile, imagej=True).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ImageJ {
    /// The ImageJ version that wrote it.
    pub version: String,
    /// Planes in the file (channels × slices × frames).
    pub images: u64,
    pub channels: u32,
    pub slices: u32,
    pub frames: u32,
    pub hyperstack: bool,
    /// "composite", "color", "grayscale".
    pub mode: Option<String>,
    /// Length unit of x (and y, z unless they have their own), normalised.
    pub unit: Option<String>,
    pub yunit: Option<String>,
    pub zunit: Option<String>,
    /// Slice spacing, in `zunit` (or `unit`).
    pub spacing: Option<f64>,
    /// Frame interval, in `tunit` (seconds by default).
    pub finterval: Option<f64>,
    pub tunit: Option<String>,
    pub fps: Option<f64>,
    /// Display range of single-channel images.
    pub min: Option<f64>,
    pub max: Option<f64>,
    /// Origin in pixels (x, y, z).
    pub origin: [Option<f64>; 3],
    /// The calibration function (0 straight line, 1–3 polynomials, 20 none, 21 uncalibrated
    /// OD) and its coefficients.
    pub cf: Option<i32>,
    pub coefficients: Vec<f64>,
    /// Unit of the calibrated values.
    pub vunit: Option<String>,
    /// Further `key=value` lines as stored.
    pub extra: Vec<(String, String)>,
}

impl ImageJ {
    /// Parses an ImageDescription; `None` unless it starts with `ImageJ=`.
    pub fn parse(desc: &str) -> Option<ImageJ> {
        let desc = desc.trim_start_matches('\u{feff}');
        if !desc.starts_with("ImageJ=") {
            return None;
        }
        let mut ij = ImageJ::default();
        let mut coeffs: Vec<(usize, f64)> = Vec::new();
        for line in desc.lines() {
            let Some((k, v)) = line.split_once('=') else {
                continue;
            };
            let (k, v) = (k.trim(), v.trim());
            let num = || v.parse::<f64>().ok().filter(|x| x.is_finite());
            let int = || v.parse::<u64>().ok();
            match k {
                "ImageJ" => ij.version = v.to_owned(),
                "images" => ij.images = int().unwrap_or(0).min(MAX_PLANES),
                "channels" => ij.channels = int().unwrap_or(1).clamp(1, MAX_PLANES) as u32,
                "slices" => ij.slices = int().unwrap_or(1).clamp(1, MAX_PLANES) as u32,
                "frames" => ij.frames = int().unwrap_or(1).clamp(1, MAX_PLANES) as u32,
                "hyperstack" => ij.hyperstack = v == "true",
                "mode" => ij.mode = Some(v.to_owned()),
                "unit" => ij.unit = normalise_unit(v),
                "yunit" => ij.yunit = normalise_unit(v),
                "zunit" => ij.zunit = normalise_unit(v),
                "tunit" => ij.tunit = Some(v.to_owned()).filter(|v| !v.is_empty()),
                "spacing" => ij.spacing = num(),
                "finterval" => ij.finterval = num(),
                "fps" => ij.fps = num(),
                "min" => ij.min = num(),
                "max" => ij.max = num(),
                "xorigin" => ij.origin[0] = num(),
                "yorigin" => ij.origin[1] = num(),
                "zorigin" => ij.origin[2] = num(),
                "cf" => ij.cf = v.parse().ok(),
                "vunit" => ij.vunit = Some(v.to_owned()).filter(|v| !v.is_empty()),
                _ => {
                    if let Some(i) = k
                        .strip_prefix('c')
                        .and_then(|n| n.parse::<usize>().ok())
                        .filter(|&i| i < 16)
                    {
                        if let Some(x) = num() {
                            coeffs.push((i, x));
                        }
                    } else {
                        ij.extra.push((k.to_owned(), v.to_owned()));
                    }
                }
            }
        }
        coeffs.sort_by_key(|c| c.0);
        for (i, (k, x)) in coeffs.into_iter().enumerate() {
            if k != i {
                break;
            }
            ij.coefficients.push(x);
        }
        ij.channels = ij.channels.max(1);
        ij.slices = ij.slices.max(1);
        ij.frames = ij.frames.max(1);
        let product = u64::from(ij.channels) * u64::from(ij.slices) * u64::from(ij.frames);
        if ij.images == 0 {
            ij.images = product;
        } else if product == 1 && ij.images > 1 {
            // A plain stack: its images are slices.
            ij.slices = ij.images.min(MAX_PLANES) as u32;
        }
        Some(ij)
    }

    /// The calibration function as a value transform: the straight line (16-bit signed data
    /// is stored offset, `c0 = −32768`) and the polynomials of degree 2 to 4; `None` for the
    /// rest (no function, optical density, exponentials ImageJ fits but rarely stores).
    pub fn calibration(&self) -> Option<Calibration> {
        let need = match self.cf? {
            0 => 2,
            1 => 3,
            2 => 4,
            3 => 5,
            _ => return None,
        };
        (self.coefficients.len() >= need).then(|| Calibration {
            coefficients: self.coefficients[..need].to_vec(),
        })
    }
}

/// A polynomial value calibration: `c0 + c1·x + c2·x² + …`.
#[derive(Clone, Debug, PartialEq)]
pub struct Calibration {
    pub coefficients: Vec<f64>,
}

impl Calibration {
    pub fn apply(&self, x: f64) -> f64 {
        self.coefficients
            .iter()
            .rev()
            .fold(0.0, |acc, &c| acc * x + c)
    }

    /// Whether it changes nothing (`0 + 1·x`).
    pub fn is_identity(&self) -> bool {
        self.coefficients.len() == 2 && self.coefficients[0] == 0.0 && self.coefficients[1] == 1.0
    }
}

/// The binary IJMetadata (tags 50838 byte counts, 50839 data).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct IjMetadata {
    /// "Show Info" text.
    pub info: Option<String>,
    /// One label per plane, in file order.
    pub labels: Vec<String>,
    /// Display range (min, max) per channel.
    pub ranges: Vec<(f64, f64)>,
    /// LUT per channel.
    pub luts: Vec<Lut>,
    /// Image properties (key, value).
    pub properties: Vec<(String, String)>,
    /// The ROI, overlay and plot blocks are present (not read).
    pub has_roi: bool,
}

impl IjMetadata {
    /// Reads the IJMetadata from directory `d`.
    pub fn read(dirs: &Directories, data: &[u8], d: usize) -> Option<IjMetadata> {
        let counts = dirs.uints(data, d, IJ_META_COUNTS)?;
        let meta = dirs.bytes(data, d, IJ_META)?;
        IjMetadata::parse(meta, &counts)
    }

    /// Parses the data of tag 50839 split by the byte counts of tag 50838.
    pub fn parse(data: &[u8], counts: &[u64]) -> Option<IjMetadata> {
        let le = match data.get(..4)? {
            b"IJIJ" => false,
            b"JIJI" => true,
            _ => return None,
        };
        let header = *counts.first()? as usize;
        if header < 4 || header > data.len() || !(header - 4).is_multiple_of(8) {
            return None;
        }
        let u32_at = |b: &[u8]| {
            let a = [b[0], b[1], b[2], b[3]];
            if le {
                u32::from_le_bytes(a)
            } else {
                u32::from_be_bytes(a)
            }
        };
        let mut blocks: Vec<&[u8]> = Vec::new();
        let mut pos = header;
        for &c in &counts[1..] {
            let end = pos.checked_add(c as usize)?;
            blocks.push(data.get(pos..end)?);
            pos = end;
        }
        let utf16 = |b: &[u8]| {
            let units: Vec<u16> = b
                .chunks_exact(2)
                .map(|c| {
                    if le {
                        u16::from_le_bytes([c[0], c[1]])
                    } else {
                        u16::from_be_bytes([c[0], c[1]])
                    }
                })
                .collect();
            String::from_utf16_lossy(&units)
        };
        let mut m = IjMetadata::default();
        let mut next = blocks.into_iter();
        for pair in data[4..header].chunks_exact(8) {
            let (typ, n) = (u32_at(&pair[..4]), u32_at(&pair[4..]) as usize);
            for _ in 0..n.min(1 << 20) {
                let Some(b) = next.next() else {
                    return Some(m);
                };
                match &typ.to_be_bytes() {
                    b"info" => m.info = Some(utf16(b)),
                    b"labl" => m.labels.push(utf16(b)),
                    b"rang" => {
                        let v: Vec<f64> = b
                            .chunks_exact(8)
                            .map(|c| {
                                let a = [c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7]];
                                if le {
                                    f64::from_le_bytes(a)
                                } else {
                                    f64::from_be_bytes(a)
                                }
                            })
                            .collect();
                        m.ranges.extend(v.chunks_exact(2).map(|p| (p[0], p[1])));
                    }
                    b"luts" => {
                        if b.len() >= 768 {
                            let mut lut: Lut = Box::new([[0; 3]; 256]);
                            for (i, e) in lut.iter_mut().enumerate() {
                                *e = [b[i], b[256 + i], b[512 + i]];
                            }
                            m.luts.push(lut);
                        }
                    }
                    b"prop" => {
                        let s = utf16(b);
                        m.properties.push((s, String::new()));
                    }
                    b"roi " | b"over" => m.has_roi = true,
                    _ => {}
                }
            }
        }
        // Properties alternate key and value.
        let props = std::mem::take(&mut m.properties);
        m.properties = props
            .chunks(2)
            .map(|p| {
                (
                    p[0].0.clone(),
                    p.get(1).map(|v| v.0.clone()).unwrap_or_default(),
                )
            })
            .collect();
        Some(m)
    }
}

/// Whether a LUT is a ramp from black to one colour (ImageJ's "Green", "Magenta"…): the
/// channel then shows in that colour, added to the others. Returns the colour.
pub fn ramp_colour(lut: &[[u8; 3]; 256]) -> Option<[u8; 3]> {
    let top = lut[255];
    if lut[0] != [0, 0, 0] || top == [0, 0, 0] {
        return None;
    }
    let ok = lut.iter().enumerate().all(|(i, e)| {
        (0..3).all(|k| {
            let want = f64::from(top[k]) * i as f64 / 255.0;
            (f64::from(e[k]) - want).abs() <= 1.5
        })
    });
    ok.then_some(top)
}

/// An ImageJ colour table file (`.lut`): 768 bytes of 256 reds, greens then blues; the
/// same after NIH Image's 32-byte `ICOL` header; or text, one row per entry of red, green
/// and blue (after an index column, if any), a header line allowed. A text table of fewer
/// than 256 rows is stretched over 256 entries, as ImageJ does.
pub fn parse_lut(bytes: &[u8]) -> Option<Lut> {
    let planes = |b: &[u8]| -> Lut {
        let mut t: Lut = Box::new([[0; 3]; 256]);
        for (i, e) in t.iter_mut().enumerate() {
            *e = [b[i], b[256 + i], b[512 + i]];
        }
        t
    };
    if bytes.len() >= 800 && bytes.starts_with(b"ICOL") {
        return Some(planes(&bytes[32..800]));
    }
    let text = std::str::from_utf8(bytes).ok().filter(|t| t.is_ascii());
    if bytes.len() == 768 && text.is_none_or(|t| t.contains('\0')) {
        return Some(planes(bytes));
    }
    let mut rows: Vec<[f64; 3]> = Vec::new();
    for line in text?.lines() {
        let cells: Vec<&str> = line
            .split(|c: char| c == ',' || c.is_whitespace())
            .filter(|c| !c.is_empty())
            .collect();
        if cells.is_empty() {
            continue;
        }
        let nums: Option<Vec<f64>> = cells.iter().map(|c| c.parse().ok()).collect();
        let Some(nums) = nums else {
            // A header line, only before the table.
            if rows.is_empty() {
                continue;
            }
            return None;
        };
        let rgb = match nums.as_slice() {
            [r, g, b] | [_, r, g, b] => [*r, *g, *b],
            _ => return None,
        };
        if rgb.iter().any(|v| !(0.0..=255.0).contains(v)) || rows.len() == 256 {
            return None;
        }
        rows.push(rgb);
    }
    if rows.len() < 2 {
        return None;
    }
    let n = rows.len();
    let mut t: Lut = Box::new([[0; 3]; 256]);
    for (i, e) in t.iter_mut().enumerate() {
        let x = i as f64 * n as f64 / 256.0;
        let i0 = (x as usize).min(n - 1);
        let i1 = (i0 + 1).min(n - 1);
        let f = x - i0 as f64;
        for (k, o) in e.iter_mut().enumerate() {
            *o = (rows[i0][k] * (1.0 - f) + rows[i1][k] * f).round() as u8;
        }
    }
    Some(t)
}

// ----- Zeiss LSM --------------------------------------------------------------------------

/// The CZ_LSMINFO structure of a Zeiss LSM 510/710 file.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LsmInfo {
    /// x, y, z, channels, time points.
    pub dims: [u32; 5],
    /// 1 8-bit, 2 12-bit, 5 32-bit float; 0 different per channel.
    pub data_type: u32,
    /// Voxel size x, y, z in metres (0 when not given).
    pub voxel: [f64; 3],
    /// 0 xyz, 1 xz line, 2 line, 3 time series xy, 4 time series xz, 5 time series mean of
    /// ROIs, 6 time series xyz, 7 spline, 8 spline xz, 9 time series spline, 10 time series
    /// of a point.
    pub scan_type: u16,
    /// 0 original data, 1 calculated, 2 animation.
    pub type_of_data: u32,
    /// Seconds between time points (0 when not given).
    pub time_interval: f64,
    /// Display colour and name of each channel.
    pub channel_colours: Vec<[u8; 3]>,
    pub channel_names: Vec<String>,
    /// Acquisition time of each time point, seconds.
    pub timestamps: Vec<f64>,
}

impl LsmInfo {
    /// Reads the structure of directory `d` (the first IFD).
    pub fn read(dirs: &Directories, data: &[u8], d: usize) -> Option<LsmInfo> {
        let e = dirs.entry(d, CZ_LSMINFO)?;
        let start = usize::try_from(e.offset).ok()?;
        let tag = data.get(start..)?;
        LsmInfo::parse(data, &tag[..tag.len().min(512)])
    }

    /// Parses the structure `tag`; its offsets point into `file`. The structure is always
    /// little-endian.
    pub fn parse(file: &[u8], tag: &[u8]) -> Option<LsmInfo> {
        let u32_at = |b: &[u8], o: usize| -> Option<u32> {
            b.get(o..o + 4)
                .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        };
        let f64_at = |b: &[u8], o: usize| -> Option<f64> {
            b.get(o..o + 8)
                .map(|c| f64::from_le_bytes([c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7]]))
        };
        let magic = u32_at(tag, 0)?;
        if magic != 0x0300_494C && magic != 0x0400_494C {
            return None;
        }
        let size = (u32_at(tag, 4)? as usize).clamp(8, tag.len());
        let tag = &tag[..size];
        let dim = |o| u32_at(tag, o).unwrap_or(1).max(1);
        let mut info = LsmInfo {
            dims: [dim(8), dim(12), dim(16), dim(20), dim(24)],
            data_type: u32_at(tag, 28).unwrap_or(0),
            ..Default::default()
        };
        for (k, o) in [40, 48, 56].into_iter().enumerate() {
            info.voxel[k] = f64_at(tag, o)
                .filter(|v| v.is_finite() && *v > 0.0)
                .unwrap_or(0.0);
        }
        info.scan_type = tag
            .get(88..90)
            .map_or(0, |b| u16::from_le_bytes([b[0], b[1]]));
        info.type_of_data = u32_at(tag, 92).unwrap_or(0);
        info.time_interval = f64_at(tag, 112)
            .filter(|v| v.is_finite() && *v > 0.0)
            .unwrap_or(0.0);
        // Channel colours and names.
        if let Some(off) = u32_at(tag, 108).filter(|&o| o > 0).map(|o| o as usize)
            && let Some(b) = file.get(off..)
        {
            let n_colours = u32_at(b, 4).unwrap_or(0) as usize;
            let n_names = u32_at(b, 8).unwrap_or(0) as usize;
            let colours_at = u32_at(b, 12).unwrap_or(0) as usize;
            let names_at = u32_at(b, 16).unwrap_or(0) as usize;
            if n_colours <= 1024 && colours_at > 0 {
                for i in 0..n_colours {
                    if let Some(c) = b.get(colours_at + 4 * i..colours_at + 4 * i + 3) {
                        info.channel_colours.push([c[0], c[1], c[2]]);
                    }
                }
            }
            if n_names <= 1024 && names_at > 0 {
                // Each name: a 32-bit length, then the text, NUL-terminated.
                let mut p = names_at;
                for _ in 0..n_names {
                    let Some(len) = u32_at(b, p).map(|l| l as usize).filter(|&l| l < 4096) else {
                        break;
                    };
                    let Some(s) = b.get(p + 4..p + 4 + len) else {
                        break;
                    };
                    let end = s.iter().position(|&c| c == 0).unwrap_or(s.len());
                    info.channel_names
                        .push(String::from_utf8_lossy(&s[..end]).into_owned());
                    p += 4 + len;
                }
            }
        }
        // Time stamps.
        if let Some(off) = u32_at(tag, 132).filter(|&o| o > 0).map(|o| o as usize)
            && let Some(b) = file.get(off..)
        {
            let n = u32_at(b, 4).unwrap_or(0) as usize;
            if n <= 1 << 20 {
                info.timestamps = (0..n).map_while(|i| f64_at(b, 8 + 8 * i)).collect();
            }
        }
        Some(info)
    }
}

// ----- MetaMorph STK ----------------------------------------------------------------------

/// The MetaMorph stack tags.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Stk {
    /// Planes in the stack (the UIC2 count).
    pub planes: u32,
    /// Spatial calibration on, with the size of a pixel in `units`.
    pub spatial: bool,
    pub x_calibration: Option<f64>,
    pub y_calibration: Option<f64>,
    pub units: Option<String>,
    pub name: Option<String>,
    /// Z distance of each plane.
    pub z_distance: Vec<f64>,
    /// Wavelength of each plane.
    pub wavelengths: Vec<f64>,
    /// Creation date (Julian day) and time (ms) of each plane.
    pub created: Vec<(u32, u32)>,
}

impl Stk {
    /// Reads the UIC tags of directory `d`; `None` without UIC2. MetaMorph writes them
    /// little-endian only.
    pub fn read(dirs: &Directories, data: &[u8], d: usize) -> Option<Stk> {
        if !dirs.little_endian {
            return None;
        }
        let u32_at = |o: u64| -> Option<u32> {
            let o = usize::try_from(o).ok()?;
            data.get(o..o.checked_add(4)?)
                .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        };
        let uic2 = dirs.entry(d, UIC2)?;
        if uic2.count == 0 || uic2.count > MAX_PLANES {
            return None;
        }
        let n = uic2.count as u32;
        let mut stk = Stk {
            planes: n,
            ..Default::default()
        };
        for i in 0..u64::from(n) {
            // Offsets are file values (BigTIFF: any 64-bit value): saturate, then `u32_at`
            // finds nothing there.
            let p = uic2.offset.saturating_add(i * 24);
            let (Some(num), Some(den)) = (u32_at(p), u32_at(p.saturating_add(4))) else {
                break;
            };
            stk.z_distance.push(if den == 0 {
                0.0
            } else {
                f64::from(num) / f64::from(den)
            });
            stk.created.push((
                u32_at(p.saturating_add(8)).unwrap_or(0),
                u32_at(p.saturating_add(12)).unwrap_or(0),
            ));
        }
        if let Some(uic3) = dirs.entry(d, UIC3) {
            for i in 0..u64::from(n).min(uic3.count.max(u64::from(n))) {
                let p = uic3.offset.saturating_add(i * 8);
                let (Some(num), Some(den)) = (u32_at(p), u32_at(p.saturating_add(4))) else {
                    break;
                };
                stk.wavelengths.push(if den == 0 {
                    0.0
                } else {
                    f64::from(num) / f64::from(den)
                });
            }
        }
        if let Some(uic1) = dirs.entry(d, UIC1).filter(|e| e.typ == 4) {
            let fraction = |off: u32| -> Option<f64> {
                let (a, b) = (u32_at(u64::from(off))?, u32_at(u64::from(off) + 4)?);
                (b != 0).then(|| f64::from(a) / f64::from(b))
            };
            let text = |off: u32| -> Option<String> {
                let off = off as usize;
                let len = u32_at(off as u64)? as usize;
                if len >= 1024 {
                    return None;
                }
                let s = data.get(off + 4..off + 4 + len)?;
                let end = s.iter().position(|&c| c == 0).unwrap_or(s.len());
                Some(String::from_utf8_lossy(&s[..end]).trim().to_owned()).filter(|s| !s.is_empty())
            };
            for i in 0..uic1.count.min(4096) {
                let p = uic1.offset.saturating_add(i * 8);
                let (Some(id), Some(v)) = (u32_at(p), u32_at(p.saturating_add(4))) else {
                    break;
                };
                match id {
                    3 => stk.spatial = v != 0,
                    4 if v >= 8 => stk.x_calibration = fraction(v),
                    5 if v >= 8 => stk.y_calibration = fraction(v),
                    6 if v >= 8 => stk.units = text(v),
                    7 if v >= 8 => stk.name = text(v),
                    _ => {}
                }
            }
        }
        Some(stk)
    }
}

// ----- Olympus FluoView -------------------------------------------------------------------

/// One FluoView dimension: "X", "Y", "Z", "T", "Ch", "Wavelength", "Time", …
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FluoviewDim {
    pub name: String,
    pub size: u32,
    pub origin: f64,
    /// The step between positions, in `unit`.
    pub step: f64,
    pub unit: String,
}

impl FluoviewDim {
    /// The axis letter tifffile gives the dimension (X, Y, Z, T, C; Q for others).
    pub fn axis(&self) -> char {
        match self.name.to_ascii_uppercase().as_str() {
            "X" => 'X',
            "Y" => 'Y',
            "Z" => 'Z',
            "T" | "TIME" => 'T',
            "CH" | "WAVELENGTH" => 'C',
            "XY" => 'R',
            "EVENT" => 'V',
            "EXPOSURE" => 'L',
            _ => 'Q',
        }
    }
}

/// The FluoView MM_Header of a file.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Fluoview {
    pub name: Option<String>,
    /// The header's dimensions, fastest first (X, Y, then the stack's axes), with the
    /// unused ones (no name, or size 0) left out.
    pub dims: Vec<FluoviewDim>,
}

/// Bytes of MM_Header up to the end of its dimensions.
const MM_HEADER_DIMS: usize = 284 + 10 * 100;

impl Fluoview {
    /// Reads MM_Header of directory `d`; `None` without MM_Stamp and MM_Header (as tifffile
    /// recognises FluoView files).
    pub fn read(dirs: &Directories, data: &[u8], d: usize) -> Option<Fluoview> {
        dirs.entry(d, MM_STAMP)?;
        let mut b = dirs.bytes(data, d, MM_HEADER)?;
        if b.len() < MM_HEADER_DIMS {
            // A tag holding the header's offset rather than the header.
            let off = *dirs.uints(data, d, MM_HEADER)?.first()?;
            b = data.get(usize::try_from(off).ok()?..)?;
        }
        Fluoview::parse(b.get(..MM_HEADER_DIMS)?, dirs.little_endian)
    }

    /// Parses the header's first [`MM_HEADER_DIMS`] bytes.
    pub fn parse(b: &[u8], little: bool) -> Option<Fluoview> {
        if b.len() < MM_HEADER_DIMS {
            return None;
        }
        let text = |s: &[u8]| -> String {
            let end = s.iter().position(|&c| c == 0).unwrap_or(s.len());
            String::from_utf8_lossy(&s[..end]).trim().to_owned()
        };
        let i32_at = |o: usize| {
            let x = [b[o], b[o + 1], b[o + 2], b[o + 3]];
            if little {
                i32::from_le_bytes(x)
            } else {
                i32::from_be_bytes(x)
            }
        };
        let f64_at = |o: usize| {
            let mut x = [0u8; 8];
            x.copy_from_slice(&b[o..o + 8]);
            if little {
                f64::from_le_bytes(x)
            } else {
                f64::from_be_bytes(x)
            }
        };
        let mut dims = Vec::new();
        for k in 0..10 {
            let at = 284 + 100 * k;
            let name = text(&b[at..at + 16]);
            let size = i32_at(at + 16);
            if name.is_empty() || size <= 0 {
                continue;
            }
            let finite = |v: f64| if v.is_finite() { v } else { 0.0 };
            dims.push(FluoviewDim {
                name,
                size: size.min(MAX_PLANES as i32) as u32,
                origin: finite(f64_at(at + 20)),
                step: finite(f64_at(at + 28)),
                unit: text(&b[at + 36..at + 100]),
            });
        }
        let name = Some(text(&b[3..260])).filter(|s| !s.is_empty());
        Some(Fluoview { name, dims })
    }
}

// ----- Molecular Dynamics GEL -------------------------------------------------------------

/// The Molecular Dynamics tags of a GEL file (or a TIFF from a Storm/Typhoon scanner).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MdGel {
    /// 2: square-root coded (value = stored² × scale); 128: linear (value = stored × scale).
    pub file_tag: u32,
    pub scale: f64,
    pub units: Option<String>,
    pub lab_name: Option<String>,
    pub sample_info: Option<String>,
    pub prep_date: Option<String>,
    pub prep_time: Option<String>,
}

impl MdGel {
    /// Reads the tags of directory `d`; `None` without MD_FileTag.
    pub fn read(dirs: &Directories, data: &[u8], d: usize) -> Option<MdGel> {
        let file_tag = *dirs.uints(data, d, MD_FILE_TAG)?.first()? as u32;
        let text = |tag| {
            dirs.ascii(data, d, tag)
                .map(|s| s.trim().to_owned())
                .filter(|s| !s.is_empty())
        };
        let scale = dirs
            .reals(data, d, MD_SCALE_PIXEL)
            .and_then(|v| v.first().copied())
            .filter(|s| s.is_finite() && *s > 0.0)
            .unwrap_or(1.0);
        Some(MdGel {
            file_tag,
            scale,
            units: text(MD_FILE_UNITS),
            lab_name: text(MD_LAB_NAME),
            sample_info: text(MD_SAMPLE_INFO),
            prep_date: text(MD_PREP_DATE),
            prep_time: text(MD_PREP_TIME),
        })
    }

    /// The value of a stored sample.
    pub fn value(&self, stored: f64) -> f64 {
        if self.file_tag == 2 {
            stored * stored * self.scale
        } else {
            stored * self.scale
        }
    }
}

// ----- SEM vendors ------------------------------------------------------------------------

/// What an electron microscope recorded about an image.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Instrument {
    /// "Zeiss", "Thermo Fisher (FEI)", "TESCAN", "Hitachi", "JEOL".
    pub vendor: String,
    /// Model, e.g. "Helios G4 UX".
    pub model: Option<String>,
    /// Size of a pixel across and down, metres.
    pub pixel: Option<(f64, f64)>,
    /// Accelerating voltage, volts.
    pub voltage: Option<f64>,
    pub magnification: Option<f64>,
    pub detector: Option<String>,
    /// Signal or imaging mode ("SE", "BSE").
    pub mode: Option<String>,
    /// As stored.
    pub date: Option<String>,
    /// Working distance, metres.
    pub working_distance: Option<f64>,
    /// Every field (section or key, name, value), in file order.
    pub fields: Vec<(String, String)>,
}

/// A number at the start of `s` and the rest ("5.00 kV" → (5.0, "kV")).
fn number_unit(s: &str) -> Option<(f64, &str)> {
    let s = s.trim();
    let end = s
        .char_indices()
        .find(|&(i, c)| {
            !(c.is_ascii_digit()
                || c == '.'
                || ((c == '-' || c == '+') && (i == 0 || s[..i].ends_with(['e', 'E'])))
                || ((c == 'e' || c == 'E') && i > 0))
        })
        .map_or(s.len(), |(i, _)| i);
    let v: f64 = s[..end].parse().ok()?;
    v.is_finite().then_some((v, s[end..].trim()))
}

/// A length with its unit in metres ("2.5 nm", "8.5 mm").
fn length(s: &str) -> Option<f64> {
    let (v, u) = number_unit(s)?;
    Some(v * metres_per(u.split_whitespace().next().unwrap_or(""))?)
}

/// A voltage with its unit in volts ("5.00 kV", "15000 Volt").
fn volts(s: &str) -> Option<f64> {
    let (v, u) = number_unit(s)?;
    let u = u.to_ascii_lowercase();
    Some(if u.starts_with("kv") {
        v * 1e3
    } else if u.starts_with("mv") {
        v * 1e6
    } else {
        v
    })
}

/// A magnification ("25.00 K X", "1000 x", "1.2 M X").
fn magnification(s: &str) -> Option<f64> {
    let (v, u) = number_unit(s)?;
    let k = match u.trim_start().chars().next() {
        Some('K' | 'k') => 1e3,
        Some('M') => 1e6,
        _ => 1.0,
    };
    Some(v * k).filter(|m| *m > 0.0)
}

/// Sections and `key = value` lines of INI text, as (section, key, value).
fn ini(text: &str) -> Vec<(String, String, String)> {
    let mut section = String::new();
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim().trim_matches('\0');
        if line.starts_with('[') && line.ends_with(']') {
            section = line[1..line.len() - 1].to_owned();
        } else if let Some((k, v)) = line.split_once('=') {
            out.push((section.clone(), k.trim().to_owned(), v.trim().to_owned()));
        }
    }
    out
}

/// Text of a tag stored as ASCII, BYTE or UNDEFINED.
fn tag_text(dirs: &Directories, data: &[u8], d: usize, tag: u16) -> Option<String> {
    let b = dirs.bytes(data, d, tag)?;
    Some(String::from_utf8_lossy(b).replace('\0', "\n"))
}

impl Instrument {
    /// The vendor tags of directory `d`, the first that is present.
    pub fn read(dirs: &Directories, data: &[u8], d: usize) -> Option<Instrument> {
        if let Some(t) = tag_text(dirs, data, d, CZ_SEM) {
            return Some(Instrument::zeiss(&t));
        }
        for tag in [FEI_HELIOS, FEI_SFEG] {
            if let Some(t) = tag_text(dirs, data, d, tag) {
                return Some(Instrument::fei(&t));
            }
        }
        if let Some(t) = tag_text(dirs, data, d, TESCAN) {
            return Some(Instrument::tescan(&t));
        }
        None
    }

    /// FEI / Thermo Fisher INI text (tags 34680 and 34682).
    pub fn fei(text: &str) -> Instrument {
        let fields = ini(text);
        let get = |sec: &str, key: &str| {
            fields
                .iter()
                .find(|(s, k, _)| s.eq_ignore_ascii_case(sec) && k.eq_ignore_ascii_case(key))
                .map(|f| f.2.as_str())
                .filter(|v| !v.is_empty())
        };
        let num = |sec: &str, key: &str| {
            get(sec, key)
                .and_then(|v| v.parse::<f64>().ok())
                .filter(|v| v.is_finite() && *v > 0.0)
        };
        let beam = get("Beam", "Beam").unwrap_or("EBeam");
        let pixel = num("Scan", "PixelWidth").map(|w| (w, num("Scan", "PixelHeight").unwrap_or(w)));
        let voltage = num("Beam", "HV").or_else(|| num(beam, "HV"));
        let date = match (get("User", "Date"), get("User", "Time")) {
            (Some(d), Some(t)) => Some(format!("{d} {t}")),
            (d, t) => d.or(t).map(str::to_owned),
        };
        Instrument {
            vendor: "Thermo Fisher (FEI)".into(),
            model: get("System", "SystemType").map(str::to_owned),
            pixel,
            voltage,
            magnification: fields
                .iter()
                .find(|(_, k, _)| k.eq_ignore_ascii_case("Magnification"))
                .and_then(|f| f.2.parse::<f64>().ok())
                .filter(|m| m.is_finite() && *m > 0.0),
            detector: get("Detectors", "Name").map(str::to_owned),
            mode: get("Detectors", "Mode").map(str::to_owned),
            date,
            working_distance: num("Stage", "WorkingDistance").or_else(|| num(beam, "WD")),
            fields: fields
                .into_iter()
                .map(|(s, k, v)| (format!("{s}.{k}"), v))
                .collect(),
        }
    }

    /// Zeiss SmartSEM text (tag 34118): an upper-case key line, then `Name = value unit`
    /// (or `Name :value`).
    pub fn zeiss(text: &str) -> Instrument {
        let mut fields: Vec<(String, String, String)> = Vec::new();
        let mut key: Option<String> = None;
        for line in text.lines() {
            let line = line.trim().trim_matches('\0');
            if line.is_empty() {
                continue;
            }
            let upper =
                line.chars().any(|c| c.is_alphabetic()) && !line.chars().any(|c| c.is_lowercase());
            if upper && !line.contains('=') && !line.contains(':') {
                key = Some(line.to_owned());
            } else if let Some(k) = key.take() {
                let (name, value) = line
                    .split_once('=')
                    .or_else(|| line.split_once(':'))
                    .unwrap_or(("", line));
                fields.push((k, name.trim().to_owned(), value.trim().to_owned()));
            }
        }
        let get = |k: &str| {
            fields
                .iter()
                .find(|f| f.0 == k)
                .map(|f| f.2.as_str())
                .filter(|v| !v.is_empty())
        };
        let detector = fields
            .iter()
            .find(|f| f.0 == "DP_DETECTOR_CHANNEL" || f.1.eq_ignore_ascii_case("Signal A"))
            .map(|f| f.2.clone());
        let date = match (get("AP_DATE"), get("AP_TIME")) {
            (Some(d), Some(t)) => Some(format!("{d} {t}")),
            (d, t) => d.or(t).map(str::to_owned),
        };
        // The image's own pixel (its store resolution) before the scan's, which differ when
        // the image is stored smaller than it was scanned.
        let pixel = get("AP_IMAGE_PIXEL_SIZE")
            .and_then(length)
            .filter(|p| *p > 0.0)
            .or_else(|| get("AP_PIXEL_SIZE").and_then(length))
            .map(|p| (p, p));
        Instrument {
            vendor: "Zeiss".into(),
            model: get("DP_COLUMN_TYPE")
                .or(get("SV_SEM_TYPE"))
                .map(str::to_owned),
            pixel,
            voltage: get("AP_ACTUALKV")
                .or(get("AP_MANUALKV"))
                .and_then(volts)
                .filter(|v| *v > 0.0),
            magnification: get("AP_MAG").and_then(magnification),
            detector,
            mode: None,
            date,
            working_distance: get("AP_WD").and_then(length),
            fields: fields
                .into_iter()
                .map(|(k, n, v)| (if n.is_empty() { k } else { n }, v))
                .collect(),
        }
    }

    /// TESCAN text (tag 50431): `key=value` lines under `[MAIN]`, lengths in metres.
    pub fn tescan(text: &str) -> Instrument {
        // The tag may carry binary data before the text.
        let start = text
            .find("[MAIN]")
            .or_else(|| text.find("AccFrames"))
            .unwrap_or(0);
        let fields = ini(&text[start..]);
        let get = |key: &str| {
            fields
                .iter()
                .find(|(_, k, _)| k.eq_ignore_ascii_case(key))
                .map(|f| f.2.as_str())
                .filter(|v| !v.is_empty())
        };
        let num = |key: &str| {
            get(key)
                .and_then(|v| v.parse::<f64>().ok())
                .filter(|v| v.is_finite() && *v > 0.0)
        };
        let date = match (get("Date"), get("Time")) {
            (Some(d), Some(t)) => Some(format!("{d} {t}")),
            (d, t) => d.or(t).map(str::to_owned),
        };
        Instrument {
            vendor: "TESCAN".into(),
            model: get("DeviceModel").or(get("Device")).map(str::to_owned),
            pixel: num("PixelSizeX").map(|x| (x, num("PixelSizeY").unwrap_or(x))),
            voltage: num("HV"),
            magnification: num("Magnification"),
            detector: get("Detector").or(get("Detector0")).map(str::to_owned),
            mode: get("ScanMode").map(str::to_owned),
            date,
            working_distance: num("WD"),
            fields: fields.into_iter().map(|(_, k, v)| (k, v)).collect(),
        }
    }

    /// A Hitachi SEM's `.txt` sidecar (`[SemImageFile]`, `PixelSize` in nm).
    pub fn hitachi(text: &str) -> Option<Instrument> {
        if !text.contains("[SemImageFile]") && !text.contains("InstructName") {
            return None;
        }
        let fields = ini(text);
        let get = |key: &str| {
            fields
                .iter()
                .find(|(_, k, _)| k.eq_ignore_ascii_case(key))
                .map(|f| f.2.as_str())
                .filter(|v| !v.is_empty())
        };
        let pixel = get("PixelSize")
            .and_then(|v| v.parse::<f64>().ok())
            .filter(|v| v.is_finite() && *v > 0.0)
            .map(|p| (p * 1e-9, p * 1e-9));
        let wd = get("WorkingDistance").and_then(|v| {
            let (x, u) = number_unit(v)?;
            Some(x * metres_per(if u.is_empty() { "um" } else { u })?)
        });
        let date = match (get("Date"), get("Time")) {
            (Some(d), Some(t)) => Some(format!("{d} {t}")),
            (d, t) => d.or(t).map(str::to_owned),
        };
        Some(Instrument {
            vendor: "Hitachi".into(),
            model: get("InstructName").map(str::to_owned),
            pixel,
            voltage: get("AcceleratingVoltage")
                .and_then(volts)
                .filter(|v| *v > 0.0),
            magnification: get("Magnification").and_then(magnification),
            detector: get("SignalName").map(str::to_owned),
            mode: None,
            date,
            working_distance: wd,
            fields: fields.into_iter().map(|(_, k, v)| (k, v)).collect(),
        })
    }

    /// A JEOL SEM's `.txt` sidecar (`$CM_FORMAT JEOL/SEM`): the pixel size from the micron
    /// bar (`$$SM_MICRON_BAR` pixels for `$$SM_MICRON_MARKER`), over the full scan width
    /// (`$CM_FULL_SIZE`) for an image of `width` pixels.
    pub fn jeol(text: &str, width: u32) -> Option<Instrument> {
        if !text.contains("JEOL") || !text.contains("$CM_") {
            return None;
        }
        let fields: Vec<(String, String)> = text
            .lines()
            .filter_map(|l| {
                let l = l.trim();
                let l = l.strip_prefix('$')?.trim_start_matches('$');
                let (k, v) = l.split_once(char::is_whitespace).unwrap_or((l, ""));
                Some((k.to_owned(), v.trim().to_owned()))
            })
            .collect();
        let get = |key: &str| {
            fields
                .iter()
                .find(|(k, _)| k == key)
                .map(|f| f.1.as_str())
                .filter(|v| !v.is_empty())
        };
        let bar = get("SM_MICRON_BAR")
            .and_then(|v| v.parse::<f64>().ok())
            .filter(|v| *v > 0.0);
        let marker = get("SM_MICRON_MARKER").and_then(length);
        let full_w = get("CM_FULL_SIZE")
            .and_then(|v| v.split_whitespace().next()?.parse::<f64>().ok())
            .filter(|v| *v > 0.0);
        let pixel = match (bar, marker) {
            (Some(b), Some(m)) => {
                let mut p = m / b;
                if let Some(fw) = full_w
                    && width > 0
                {
                    p *= fw / f64::from(width);
                }
                Some((p, p))
            }
            _ => None,
        };
        let date = match (get("CM_DATE"), get("CM_TIME")) {
            (Some(d), Some(t)) => Some(format!("{d} {t}")),
            (d, t) => d.or(t).map(str::to_owned),
        };
        Some(Instrument {
            vendor: "JEOL".into(),
            model: get("CM_INSTRUMENT").map(str::to_owned),
            pixel,
            // kilovolts
            voltage: get("CM_ACCEL_VOLT")
                .and_then(|v| v.parse::<f64>().ok())
                .filter(|v| v.is_finite() && *v > 0.0)
                .map(|kv| kv * 1e3),
            magnification: get("CM_MAG").and_then(magnification),
            detector: get("CM_SIGNAL").map(str::to_owned),
            mode: None,
            date,
            working_distance: get("SM_WD")
                .and_then(|v| v.parse::<f64>().ok())
                .map(|mm| mm * 1e-3),
            fields,
        })
    }
}

#[cfg(test)]
mod tests;
