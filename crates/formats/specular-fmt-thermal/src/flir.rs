//! FLIR FFF: the container, its records, and its transport in JPEG APP1 segments.

use crate::{
    Camera, Conditions, Error, Kind, Mapping, Palette, Pip, Pixels, Planck, Result, Thermal,
    Visible, malformed, tiff16,
};

const F: &str = "FLIR FFF";

pub(crate) const RAW_DATA: u16 = 0x01;
const EMBEDDED_IMAGE: u16 = 0x0e;
pub(crate) const CAMERA_INFO: u16 = 0x20;
const PALETTE_INFO: u16 = 0x22;
const PIP: u16 = 0x2a;

const KELVIN: f64 = 273.15;

/// Whether `d` starts as an FFF container.
pub(crate) fn is_fff(d: &[u8]) -> bool {
    (d.starts_with(b"FFF\0") || d.starts_with(b"AFF\0")) && d.len() >= 64
}

/// Calls `f` with the payload of each APP1 segment before the scan; stops when `f` says so.
fn app1_segments<'a>(d: &'a [u8], mut f: impl FnMut(&'a [u8]) -> bool) {
    if !d.starts_with(&[0xff, 0xd8]) {
        return;
    }
    let mut i = 2;
    while i + 4 <= d.len() {
        if d[i] != 0xff {
            return;
        }
        let m = d[i + 1];
        match m {
            0xff => {
                i += 1;
                continue;
            }
            0x01 | 0xd0..=0xd8 => {
                i += 2;
                continue;
            }
            0xd9 | 0xda => return,
            _ => {}
        }
        let len = usize::from(u16::from_be_bytes([d[i + 2], d[i + 3]]));
        if len < 2 {
            return;
        }
        let end = (i + 2 + len).min(d.len());
        if m == 0xe1 && f(&d[i + 4..end]) {
            return;
        }
        i += 2 + len;
    }
}

fn is_flir_payload(p: &[u8]) -> bool {
    p.starts_with(b"FLIR\0")
}

/// Whether the JPEG `d` carries FLIR APP1 segments (a walk over the segment headers before
/// the scan: a few reads for an ordinary JPEG).
pub fn is_flir_jpeg(d: &[u8]) -> bool {
    let mut found = false;
    app1_segments(d, |p| {
        found = is_flir_payload(p);
        found
    });
    found
}

/// The FFF reassembled from a JPEG's FLIR APP1 segments, in index order.
pub fn fff_from_jpeg(d: &[u8]) -> Result<Vec<u8>> {
    let mut parts: Vec<(u8, &[u8])> = Vec::new();
    app1_segments(d, |p| {
        if is_flir_payload(p) && p.len() >= 8 {
            parts.push((p[6], &p[8..]));
        }
        false
    });
    if parts.is_empty() {
        return Err(Error::NotThermal);
    }
    parts.sort_by_key(|&(i, _)| i);
    Ok(parts.iter().flat_map(|&(_, p)| p.iter().copied()).collect())
}

/// Reads fields of one record in its byte order.
#[derive(Clone, Copy)]
pub(crate) struct Rec<'a> {
    d: &'a [u8],
    le: bool,
}

impl<'a> Rec<'a> {
    /// A record whose first u16 is 2 in its byte order (little-endian when neither says 2).
    pub(crate) fn checked(d: &'a [u8]) -> Self {
        let le = !(d.len() >= 2 && u16::from_be_bytes([d[0], d[1]]) == 2);
        Rec { d, le }
    }
    fn bytes<const N: usize>(&self, at: usize) -> Option<[u8; N]> {
        self.d.get(at..at + N)?.try_into().ok()
    }
    pub(crate) fn u16(&self, at: usize) -> Option<u16> {
        let b = self.bytes::<2>(at)?;
        Some(if self.le {
            u16::from_le_bytes(b)
        } else {
            u16::from_be_bytes(b)
        })
    }
    fn i16(&self, at: usize) -> Option<i16> {
        self.u16(at).map(|v| v as i16)
    }
    pub(crate) fn u32(&self, at: usize) -> Option<u32> {
        let b = self.bytes::<4>(at)?;
        Some(if self.le {
            u32::from_le_bytes(b)
        } else {
            u32::from_be_bytes(b)
        })
    }
    fn i32(&self, at: usize) -> Option<i32> {
        self.u32(at).map(|v| v as i32)
    }
    fn f32(&self, at: usize) -> Option<f64> {
        let v = f64::from(f32::from_bits(self.u32(at)?));
        v.is_finite().then_some(v)
    }
    fn str(&self, at: usize, n: usize) -> String {
        let Some(s) = self.d.get(at..(at + n).min(self.d.len())) else {
            return String::new();
        };
        let s = &s[..s.iter().position(|&b| b == 0).unwrap_or(s.len())];
        s.iter()
            .map(|&b| char::from(b))
            .collect::<String>()
            .trim()
            .to_owned()
    }
}

/// The FFF header's fields in its byte order (the version, 100 and up, tells it).
pub(crate) fn header(fff: &[u8]) -> Rec<'_> {
    let be = u32::from_be_bytes([fff[0x14], fff[0x15], fff[0x16], fff[0x17]]);
    Rec {
        d: fff,
        le: !(100..200).contains(&be),
    }
}

pub(crate) fn read_fff(fff: &[u8], kind: Kind) -> Result<Thermal> {
    if !is_fff(fff) {
        return Err(malformed(F, "no FFF header"));
    }
    let head = header(fff);
    let dir = head.u32(0x18).unwrap_or(0) as usize;
    let count = head.u32(0x1c).unwrap_or(0) as usize;
    if count > fff.len() / 32 || dir.saturating_add(count * 32) > fff.len() {
        return Err(malformed(F, "record directory past the end"));
    }
    let mut raw = None;
    let mut camera_info = None;
    let mut palette_info = None;
    let mut embedded = None;
    let mut pip = None;
    for i in 0..count {
        let e = dir + 32 * i;
        let ty = head.u16(e).unwrap_or(0);
        let off = head.u32(e + 0x0c).unwrap_or(0) as usize;
        let len = head.u32(e + 0x10).unwrap_or(0) as usize;
        if ty == 0 || off >= fff.len() {
            continue;
        }
        let rec = &fff[off..off.saturating_add(len).min(fff.len())];
        let slot = match ty {
            RAW_DATA => &mut raw,
            CAMERA_INFO => &mut camera_info,
            PALETTE_INFO => &mut palette_info,
            EMBEDDED_IMAGE => &mut embedded,
            PIP => &mut pip,
            _ => continue,
        };
        slot.get_or_insert(rec);
    }
    let raw = raw.ok_or(Error::NoRawData)?;
    let (width, height, values) = raw_data(raw)?;
    let (planck, conditions, camera) = match camera_info {
        Some(c) => camera_info_record(c),
        None => return Err(malformed(F, "no CameraInfo record")),
    };
    Ok(Thermal {
        kind,
        width,
        height,
        pixels: Pixels::Counts {
            values,
            mapping: Mapping::Planck(planck),
        },
        conditions,
        camera,
        palette: palette_info.and_then(palette),
        visible: embedded.and_then(visible),
        pip: pip.and_then(pip_record),
    })
}

/// RawData: a byte-order check, width and height, then the image 32 bytes in.
fn raw_data(rec: &[u8]) -> Result<(usize, usize, Vec<u16>)> {
    let r = Rec::checked(rec);
    let (Some(w), Some(h)) = (r.u16(2), r.u16(4)) else {
        return Err(malformed(F, "short RawData record"));
    };
    let (w, h) = (usize::from(w), usize::from(h));
    let body = rec.get(32..).unwrap_or_default();
    if body.starts_with(b"\x89PNG") {
        let opts = specular_png::DecodeOptions {
            output: specular_png::Output::Auto16,
            ..Default::default()
        };
        let img = specular_png::decode(body, &opts)
            .map_err(|e| malformed(F, format!("RawData PNG: {e}")))?;
        let (pw, ph) = (img.width as usize, img.height as usize);
        let plane = &img.planes[0];
        let px: Vec<u16> = match img.layout {
            specular_png::Layout::Gray16 => (0..ph)
                .flat_map(|y| {
                    plane.data[y * plane.stride..y * plane.stride + 2 * pw]
                        .chunks_exact(2)
                        // FLIR stores little-endian counts in the big-endian PNG samples.
                        .map(|b| u16::from_ne_bytes([b[0], b[1]]).swap_bytes())
                })
                .collect(),
            l => {
                return Err(Error::Unsupported {
                    format: F,
                    what: format!("RawData PNG layout {l:?}"),
                });
            }
        };
        return Ok((pw, ph, px));
    }
    if tiff16::is_tiff(body) {
        return tiff16::read(body);
    }
    if specular_jpegls::probe(body) {
        // CSQ frames: lossless JPEG-LS, one 16-bit component.
        let img = specular_jpegls::decode(body, &specular_jpegls::DecodeOptions::default())
            .map_err(|e| malformed(F, format!("RawData JPEG-LS: {e}")))?;
        let [plane] = <[_; 1]>::try_from(img.planes).map_err(|_| Error::Unsupported {
            format: F,
            what: "RawData JPEG-LS with more than one component".into(),
        })?;
        if plane.table.is_some() {
            return Err(Error::Unsupported {
                format: F,
                what: "RawData JPEG-LS with a mapping table".into(),
            });
        }
        return Ok((plane.width as usize, plane.height as usize, plane.data));
    }
    let n = w * h;
    if n == 0 || body.len() < 2 * n {
        return Err(malformed(F, "RawData shorter than its image"));
    }
    let px = body[..2 * n]
        .chunks_exact(2)
        .map(|b| {
            if r.le {
                u16::from_le_bytes([b[0], b[1]])
            } else {
                u16::from_be_bytes([b[0], b[1]])
            }
        })
        .collect();
    Ok((w, h, px))
}

pub(crate) fn camera_info_record(rec: &[u8]) -> (Planck, Conditions, Camera) {
    let r = Rec::checked(rec);
    let c = |k: Option<f64>| k.map(|k| k - KELVIN);
    let planck = Planck {
        r1: r.f32(0x58).unwrap_or(0.0),
        b: r.f32(0x5c).unwrap_or(0.0),
        f: r.f32(0x60).unwrap_or(1.0),
        alpha1: r.f32(0x70).unwrap_or(0.006569),
        alpha2: r.f32(0x74).unwrap_or(0.01262),
        beta1: r.f32(0x78).unwrap_or(-0.002276),
        beta2: r.f32(0x7c).unwrap_or(-0.00667),
        x: r.f32(0x80).unwrap_or(1.9),
        o: f64::from(r.i32(0x308).unwrap_or(0)),
        r2: r.f32(0x30c).unwrap_or(1.0),
    };
    let conditions = Conditions {
        emissivity: r.f32(0x20),
        distance: r.f32(0x24),
        reflected: c(r.f32(0x28)),
        atmospheric: c(r.f32(0x2c)),
        window_temperature: c(r.f32(0x30)),
        window_transmission: r.f32(0x34),
        humidity: r.f32(0x3c).map(|h| if h > 2.0 { h / 100.0 } else { h }),
    };
    let range = match (r.f32(0x94), r.f32(0x90)) {
        (Some(lo), Some(hi)) if hi > lo => Some((lo - KELVIN, hi - KELVIN)),
        _ => None,
    };
    let date = r.u32(0x384).filter(|&s| s != 0).map(|s| {
        let ms = r.u32(0x388).unwrap_or(0);
        let tz = r.i16(0x38c).unwrap_or(0);
        date_string(i64::from(s), ms, tz)
    });
    let camera = Camera {
        model: r.str(0xd4, 32),
        part_number: r.str(0xf4, 16),
        serial: r.str(0x104, 16),
        software: r.str(0x114, 16),
        lens: r.str(0x170, 32),
        lens_part_number: r.str(0x190, 16),
        lens_serial: r.str(0x1a0, 16),
        filter: r.str(0x1ec, 16),
        field_of_view: r.f32(0x1b4).filter(|&v| v > 0.0),
        range,
        wavelength: None,
        date,
    };
    (planck, conditions, camera)
}

/// "YYYY-MM-DD HH:MM:SS.mmm" from Unix seconds, shifted by the recorded zone (minutes
/// west of UTC, as the camera writes it), with the offset appended.
fn date_string(secs: i64, ms: u32, tz_minutes_west: i16) -> String {
    let local = secs - i64::from(tz_minutes_west) * 60;
    let days = local.div_euclid(86_400);
    let t = local.rem_euclid(86_400);
    let (y, m, d) = civil(days);
    let off = -i32::from(tz_minutes_west);
    let sign = if off < 0 { '-' } else { '+' };
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02}.{:03} {sign}{:02}:{:02}",
        t / 3600,
        t / 60 % 60,
        t % 60,
        ms % 1000,
        off.abs() / 60,
        off.abs() % 60
    )
}

/// Days since 1970-01-01 to a proleptic Gregorian date (Hinnant's algorithm).
pub(crate) fn civil(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

/// sRGB from a FLIR palette entry (Y, Cr, Cb).
fn ycrcb(p: &[u8]) -> [u8; 3] {
    let (y, cr, cb) = (
        f64::from(p[0]),
        f64::from(p[1]) - 128.0,
        f64::from(p[2]) - 128.0,
    );
    let q = |v: f64| v.round().clamp(0.0, 255.0) as u8;
    [
        q(y + 1.402 * cr),
        q(y - 0.714_136 * cr - 0.344_136 * cb),
        q(y + 1.772 * cb),
    ]
}

fn palette(rec: &[u8]) -> Option<Palette> {
    let r = Rec { d: rec, le: true };
    let n = usize::from(*rec.first()?);
    let colours: Vec<[u8; 3]> = rec
        .get(0x70..0x70 + 3 * n)?
        .chunks_exact(3)
        .map(ycrcb)
        .collect();
    if colours.is_empty() {
        return None;
    }
    let colour = |at: usize| rec.get(at..at + 3).map(ycrcb);
    Some(Palette {
        name: r.str(0x50, 32),
        colours,
        above: colour(0x06),
        below: colour(0x09),
        isotherm: colour(0x12),
    })
}

fn visible(rec: &[u8]) -> Option<Visible> {
    let r = Rec::checked(rec);
    let body = rec.get(32..)?;
    if !(body.starts_with(&[0xff, 0xd8]) || body.starts_with(b"\x89PNG")) {
        return None;
    }
    Some(Visible {
        bytes: body.to_vec(),
        width: u32::from(r.u16(2)?),
        height: u32::from(r.u16(4)?),
    })
}

fn pip_record(rec: &[u8]) -> Option<Pip> {
    // The record has no byte-order check; take the order that gives a plausible scale.
    let le = Rec { d: rec, le: true };
    let be = Rec { d: rec, le: false };
    let ok = |r: &Rec| r.f32(0).is_some_and(|v| (0.01..100.0).contains(&v));
    let r = if ok(&le) {
        le
    } else if ok(&be) {
        be
    } else {
        return None;
    };
    Some(Pip {
        real_to_ir: r.f32(0)?,
        offset_x: i32::from(r.i16(4)?),
        offset_y: i32::from(r.i16(6)?),
        x1: u32::from(r.u16(8)?),
        x2: u32::from(r.u16(0xa)?),
        y1: u32::from(r.u16(0xc)?),
        y2: u32::from(r.u16(0xe)?),
    })
}
