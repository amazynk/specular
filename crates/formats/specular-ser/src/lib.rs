//! SER video (the "LUCAM-RECORDER" format of planetary and lunar capture software:
//! FireCapture, SharpCap, Genika, ASICap, Lucam Recorder), as described by Grischa Hahn's
//! SER format description, version 3.
//!
//! A 178-byte header (little-endian integers) is followed by `FrameCount` frames of
//! `ImageWidth × ImageHeight` pixels, each one sample (mono or a colour filter array) or
//! three (RGB, BGR), of `PixelDepthPerPlane` bits stored in one byte (1–8) or two (9–16),
//! rows top-down. An optional trailer holds one UTC timestamp per frame.
//!
//! The header's `LittleEndian` flag is meant to be 1 for little-endian 16-bit samples, but
//! most writers store 0 with little-endian data: 0 is read as little-endian and 1 as
//! big-endian, and for 16-bit samples the data has the last word ([`Ser::settle_order`]):
//! the byte order whose first frame is far smoother wins.
//!
//! This is not the FEI / TIA `.ser` of electron microscopes, which has another signature
//! and is not read here.

use thiserror::Error;

/// The file signature.
pub const MAGIC: &[u8; 14] = b"LUCAM-RECORDER";
/// Header size; frames start here.
pub const HEADER: usize = 178;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum Error {
    #[error("not a SER file")]
    NotSer,
    #[error("malformed SER: {0}")]
    Malformed(String),
    #[error("unsupported SER: {0}")]
    Unsupported(String),
}

pub type Result<T> = std::result::Result<T, Error>;

/// What a frame's samples are (the header's `ColorID`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Colour {
    Mono,
    /// A 2×2 Bayer colour filter array, by its four colours top row first ("RGGB").
    Bayer(&'static str),
    /// A cyan / yellow / magenta colour filter array ("CYYM"); shown as raw samples.
    Cmy(&'static str),
    /// Three samples per pixel, red first.
    Rgb,
    /// Three samples per pixel, blue first.
    Bgr,
}

impl Colour {
    fn from_id(id: i32) -> Option<Colour> {
        Some(match id {
            0 => Colour::Mono,
            8 => Colour::Bayer("RGGB"),
            9 => Colour::Bayer("GRBG"),
            10 => Colour::Bayer("GBRG"),
            11 => Colour::Bayer("BGGR"),
            16 => Colour::Cmy("CYYM"),
            17 => Colour::Cmy("YCMY"),
            18 => Colour::Cmy("YMCY"),
            19 => Colour::Cmy("MYYC"),
            100 => Colour::Rgb,
            101 => Colour::Bgr,
            _ => return None,
        })
    }

    /// Samples per pixel.
    pub fn planes(self) -> usize {
        match self {
            Colour::Rgb | Colour::Bgr => 3,
            _ => 1,
        }
    }

    /// A short name for the info panel: "Mono", "Bayer RGGB", "CMY CYYM", "RGB", "BGR".
    pub fn name(self) -> String {
        match self {
            Colour::Mono => "Mono".into(),
            Colour::Bayer(p) => format!("Bayer {p}"),
            Colour::Cmy(p) => format!("CMY {p}"),
            Colour::Rgb => "RGB".into(),
            Colour::Bgr => "BGR".into(),
        }
    }
}

/// Byte order of two-byte samples.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ByteOrder {
    Little,
    Big,
}

/// A parsed SER header with the frame layout and timestamps.
#[derive(Clone, Debug, PartialEq)]
pub struct Ser {
    pub colour: Colour,
    /// The header's raw `ColorID`.
    pub colour_id: i32,
    pub width: u32,
    pub height: u32,
    /// Significant bits per sample (1–16).
    pub depth: u32,
    /// Frames present in the data (the header's count, or fewer when the file is cut short).
    pub frames: u32,
    /// The header's `FrameCount`.
    pub declared_frames: u32,
    /// The header's `LittleEndian` flag as stored.
    pub endian_flag: i32,
    /// Byte order of two-byte samples (see the crate documentation).
    pub order: ByteOrder,
    pub observer: String,
    pub instrument: String,
    pub telescope: String,
    /// Capture start, local time and UTC, in ticks ([`ticks_text`]); 0 when not set.
    pub date_local: i64,
    pub date_utc: i64,
    /// One UTC timestamp per frame (ticks), when the file has the trailer.
    pub timestamps: Vec<i64>,
}

fn i32_at(b: &[u8], o: usize) -> i32 {
    i32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}

fn i64_at(b: &[u8], o: usize) -> i64 {
    let mut a = [0u8; 8];
    a.copy_from_slice(&b[o..o + 8]);
    i64::from_le_bytes(a)
}

/// A header text field: Latin-1, NUL- or space-padded.
fn text(b: &[u8]) -> String {
    let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    b[..end]
        .iter()
        .map(|&c| char::from(c))
        .collect::<String>()
        .trim()
        .to_owned()
}

/// Whether `head` starts like a SER file.
pub fn sniff(head: &[u8]) -> bool {
    head.starts_with(MAGIC)
}

/// Reads the header of `data` (the whole file) and works out the frames and timestamps.
pub fn parse(data: &[u8]) -> Result<Ser> {
    if !sniff(data) {
        return Err(Error::NotSer);
    }
    if data.len() < HEADER {
        return Err(Error::Malformed("header cut short".into()));
    }
    let colour_id = i32_at(data, 18);
    let colour = Colour::from_id(colour_id)
        .ok_or_else(|| Error::Unsupported(format!("colour ID {colour_id}")))?;
    let endian_flag = i32_at(data, 22);
    let (w, h, depth, count) = (
        i32_at(data, 26),
        i32_at(data, 30),
        i32_at(data, 34),
        i32_at(data, 38),
    );
    if w <= 0 || h <= 0 {
        return Err(Error::Malformed(format!("frame size {w}×{h}")));
    }
    if !(1..=16).contains(&depth) {
        return Err(Error::Unsupported(format!("{depth} bits per sample")));
    }
    let (width, height, depth) = (w as u32, h as u32, depth as u32);
    let mut ser = Ser {
        colour,
        colour_id,
        width,
        height,
        depth,
        frames: 0,
        declared_frames: count.max(0) as u32,
        endian_flag,
        order: if endian_flag == 0 {
            ByteOrder::Little
        } else {
            ByteOrder::Big
        },
        observer: text(&data[42..82]),
        instrument: text(&data[82..122]),
        telescope: text(&data[122..162]),
        date_local: i64_at(data, 162),
        date_utc: i64_at(data, 170),
        timestamps: Vec::new(),
    };
    let fb = ser
        .frame_bytes()
        .ok_or_else(|| Error::Malformed("frame size overflows".into()))?;
    let avail = ((data.len() - HEADER) as u64 / fb as u64).min(u64::from(u32::MAX)) as u32;
    if avail == 0 {
        return Err(Error::Malformed("no complete frame".into()));
    }
    ser.frames = if ser.declared_frames == 0 {
        avail
    } else {
        ser.declared_frames.min(avail)
    };
    // The trailer: one i64 per declared frame right after the frames.
    let n = ser.declared_frames as usize;
    if n > 0 && ser.frames as usize == n {
        let start = HEADER + n * fb;
        if let Some(t) = data.get(start..start + n * 8) {
            ser.timestamps = t.chunks_exact(8).map(|c| i64_at(c, 0)).collect();
            if ser.timestamps.iter().all(|&t| t == 0) {
                ser.timestamps.clear();
            }
        }
    }
    ser.settle_order(data);
    Ok(ser)
}

impl Ser {
    /// Bytes per sample.
    pub fn sample_bytes(&self) -> usize {
        if self.depth <= 8 { 1 } else { 2 }
    }

    /// Bytes per frame, unless that overflows.
    pub fn frame_bytes(&self) -> Option<usize> {
        (self.width as usize)
            .checked_mul(self.height as usize)?
            .checked_mul(self.colour.planes())?
            .checked_mul(self.sample_bytes())
    }

    /// Frames are shorter than the header says (the file was cut).
    pub fn truncated(&self) -> bool {
        self.frames < self.declared_frames
    }

    /// The bytes of frame `i`.
    pub fn frame<'a>(&self, data: &'a [u8], i: u32) -> Option<&'a [u8]> {
        if i >= self.frames {
            return None;
        }
        let fb = self.frame_bytes()?;
        data.get(HEADER + i as usize * fb..)?.get(..fb)
    }

    /// The samples of frame `i` (interleaved red, green, blue for colour frames, BGR
    /// reordered), as stored values (0..2^depth for well-formed data).
    pub fn samples(&self, data: &[u8], i: u32, out: &mut Vec<u16>) -> Option<()> {
        let f = self.frame(data, i)?;
        out.clear();
        if self.sample_bytes() == 1 {
            out.extend(f.iter().map(|&b| u16::from(b)));
        } else {
            match self.order {
                ByteOrder::Little => {
                    out.extend(f.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])))
                }
                ByteOrder::Big => {
                    out.extend(f.chunks_exact(2).map(|c| u16::from_be_bytes([c[0], c[1]])))
                }
            }
        }
        if self.colour == Colour::Bgr {
            for px in out.chunks_exact_mut(3) {
                px.swap(0, 2);
            }
        }
        Some(())
    }

    /// Settles the byte order of two-byte samples from the first frame: neighbouring
    /// samples of one colour differ far less in the right order than with their bytes
    /// swapped (where the low byte's noise lands in the high byte). Kept as the flag says
    /// unless the other order is at least four times smoother.
    pub fn settle_order(&mut self, data: &[u8]) {
        if self.sample_bytes() != 2 {
            return;
        }
        let Some(f) = self.frame(data, 0) else { return };
        // Same-colour neighbour along a row, in samples.
        let step = match self.colour {
            Colour::Mono => 1,
            Colour::Bayer(_) | Colour::Cmy(_) => 2,
            Colour::Rgb | Colour::Bgr => 3,
        };
        let row = self.width as usize * self.colour.planes();
        let rows = self.height as usize;
        // Up to ~256 Ki pairs from rows spread over the frame.
        let row_step = (rows * row / (1 << 18)).max(1);
        let (mut le, mut be, mut n) = (0f64, 0f64, 0u64);
        let s = |i: usize| [f[2 * i], f[2 * i + 1]];
        for y in (0..rows).step_by(row_step) {
            for x in 0..row.saturating_sub(step) {
                let (a, b) = (s(y * row + x), s(y * row + x + step));
                le += (f64::from(u16::from_le_bytes(a)) - f64::from(u16::from_le_bytes(b))).abs();
                be += (f64::from(u16::from_be_bytes(a)) - f64::from(u16::from_be_bytes(b))).abs();
                n += 1;
            }
        }
        if n == 0 {
            return;
        }
        let (mine, other, flip) = match self.order {
            ByteOrder::Little => (le, be, ByteOrder::Big),
            ByteOrder::Big => (be, le, ByteOrder::Little),
        };
        if other * 4.0 < mine {
            self.order = flip;
        }
    }

    /// Mean frame rate from the timestamps (frames per second), when they increase.
    pub fn frame_rate(&self) -> Option<f64> {
        let (first, last) = (*self.timestamps.first()?, *self.timestamps.last()?);
        let n = self.timestamps.len();
        (n >= 2 && last > first).then(|| (n - 1) as f64 / ((last - first) as f64 * 1e-7))
    }
}

/// Ticks (100 ns since 0001-01-01 00:00, as .NET's `DateTime`) as
/// "YYYY-MM-DD HH:MM:SS.fff"; `None` for 0 or an out-of-range value.
pub fn ticks_text(ticks: i64) -> Option<String> {
    // Days from 0001-01-01 to 1970-01-01.
    const UNIX_DAYS: i64 = 719_162;
    if ticks <= 0 {
        return None;
    }
    let ms_total = ticks / 10_000;
    let days = ms_total.div_euclid(86_400_000);
    let ms = ms_total.rem_euclid(86_400_000);
    let (y, m, d) = civil(days - UNIX_DAYS);
    if !(1..=9999).contains(&y) {
        return None;
    }
    Some(format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02}.{:03}",
        ms / 3_600_000,
        ms / 60_000 % 60,
        ms / 1000 % 60,
        ms % 1000
    ))
}

/// Year, month and day of `z` days since 1970-01-01 (proleptic Gregorian).
fn civil(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

/// Builds a SER file (for tests and fixtures): the header, `frames` (each the stored
/// bytes of one frame) and, when given, the timestamp trailer.
pub fn write(header: &Header, frames: &[Vec<u8>], timestamps: Option<&[i64]>) -> Vec<u8> {
    let mut out = Vec::with_capacity(HEADER + frames.iter().map(Vec::len).sum::<usize>());
    out.extend_from_slice(MAGIC);
    for v in [
        header.lu_id,
        header.colour_id,
        header.endian_flag,
        header.width as i32,
        header.height as i32,
        header.depth as i32,
        header.frame_count.unwrap_or(frames.len() as u32) as i32,
    ] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    for t in [&header.observer, &header.instrument, &header.telescope] {
        let mut f = [0u8; 40];
        for (o, c) in f.iter_mut().zip(t.bytes()) {
            *o = c;
        }
        out.extend_from_slice(&f);
    }
    out.extend_from_slice(&header.date_local.to_le_bytes());
    out.extend_from_slice(&header.date_utc.to_le_bytes());
    debug_assert_eq!(out.len(), HEADER);
    for f in frames {
        out.extend_from_slice(f);
    }
    if let Some(ts) = timestamps {
        for t in ts {
            out.extend_from_slice(&t.to_le_bytes());
        }
    }
    out
}

/// The header fields [`write`] stores.
#[derive(Clone, Debug, Default)]
pub struct Header {
    pub lu_id: i32,
    pub colour_id: i32,
    pub endian_flag: i32,
    pub width: u32,
    pub height: u32,
    pub depth: u32,
    /// `FrameCount` when it should differ from the frames given (a cut file).
    pub frame_count: Option<u32>,
    pub observer: String,
    pub instrument: String,
    pub telescope: String,
    pub date_local: i64,
    pub date_utc: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(colour_id: i32, w: u32, h: u32, depth: u32) -> Header {
        Header {
            colour_id,
            width: w,
            height: h,
            depth,
            observer: "Ann".into(),
            telescope: "C11".into(),
            ..Default::default()
        }
    }

    #[test]
    fn mono_8bit_with_timestamps() {
        let frames: Vec<Vec<u8>> = (0..3u8)
            .map(|k| (0..6).map(|i| i * 10 + k).collect())
            .collect();
        // 2024-05-01 21:03:04.5 UTC in ticks, frames 20 ms apart.
        let t0 = 638_501_941_845_000_000i64;
        let ts: Vec<i64> = (0..3).map(|k| t0 + k * 200_000).collect();
        let bytes = write(&header(0, 3, 2, 8), &frames, Some(&ts));
        let s = parse(&bytes).unwrap();
        assert_eq!((s.width, s.height, s.depth, s.frames), (3, 2, 8, 3));
        assert_eq!(s.colour, Colour::Mono);
        assert_eq!(s.observer, "Ann");
        assert_eq!(s.telescope, "C11");
        assert_eq!(s.timestamps, ts);
        assert!((s.frame_rate().unwrap() - 50.0).abs() < 1e-9);
        let mut v = Vec::new();
        s.samples(&bytes, 2, &mut v).unwrap();
        assert_eq!(v, [2, 12, 22, 32, 42, 52]);
        assert!(s.samples(&bytes, 3, &mut v).is_none());
        assert_eq!(ticks_text(t0).as_deref(), Some("2024-05-01 21:03:04.500"));
        assert_eq!(ticks_text(0), None);
    }

    /// A smooth 16-bit ramp: the byte order is read from the data whatever the flag says.
    #[test]
    fn byte_order_quirk() {
        let (w, h) = (64u32, 8u32);
        let vals: Vec<u16> = (0..w * h)
            .map(|i| 1000 + (i % w) as u16 * 7 + (i / w) as u16)
            .collect();
        let le: Vec<u8> = vals.iter().flat_map(|v| v.to_le_bytes()).collect();
        let be: Vec<u8> = vals.iter().flat_map(|v| v.to_be_bytes()).collect();
        // (flag, stored bytes): the common inversion (0 with little-endian data), the
        // specification's meaning (1 with little-endian data), and big-endian data under
        // either flag.
        for (flag, data) in [(0, &le), (1, &le), (0, &be), (1, &be)] {
            let mut hd = header(0, w, h, 16);
            hd.endian_flag = flag;
            let bytes = write(&hd, std::slice::from_ref(data), None);
            let s = parse(&bytes).unwrap();
            let mut v = Vec::new();
            s.samples(&bytes, 0, &mut v).unwrap();
            assert_eq!(v, vals, "flag {flag}");
        }
        // A flat frame keeps the flag's (inverted) reading.
        let mut hd = header(0, 4, 1, 16);
        hd.endian_flag = 1;
        let bytes = write(&hd, &[vec![1, 2, 1, 2, 1, 2, 1, 2]], None);
        assert_eq!(parse(&bytes).unwrap().order, ByteOrder::Big);
    }

    #[test]
    fn colour_ids_and_bgr() {
        let bytes = write(&header(101, 2, 1, 8), &[vec![1, 2, 3, 4, 5, 6]], None);
        let s = parse(&bytes).unwrap();
        assert_eq!(s.colour, Colour::Bgr);
        let mut v = Vec::new();
        s.samples(&bytes, 0, &mut v).unwrap();
        assert_eq!(v, [3, 2, 1, 6, 5, 4]);
        for (id, c) in [
            (8, Colour::Bayer("RGGB")),
            (9, Colour::Bayer("GRBG")),
            (10, Colour::Bayer("GBRG")),
            (11, Colour::Bayer("BGGR")),
            (16, Colour::Cmy("CYYM")),
            (100, Colour::Rgb),
        ] {
            let b = write(
                &header(id, 2, 2, 8),
                &[vec![0; 2 * 2 * Colour::from_id(id).unwrap().planes()]],
                None,
            );
            assert_eq!(parse(&b).unwrap().colour, c);
        }
        let b = write(&header(7, 2, 2, 8), &[vec![0; 4]], None);
        assert_eq!(parse(&b), Err(Error::Unsupported("colour ID 7".into())));
    }

    #[test]
    fn cut_files_and_bad_headers() {
        // Five frames declared, two and a half present: two frames, no trailer.
        let mut hd = header(0, 2, 2, 8);
        hd.frame_count = Some(5);
        let mut bytes = write(&hd, &[vec![1; 4], vec![2; 4]], None);
        bytes.extend([3, 3]);
        let s = parse(&bytes).unwrap();
        assert_eq!((s.frames, s.declared_frames), (2, 5));
        assert!(s.truncated());
        assert!(s.timestamps.is_empty());
        assert_eq!(
            parse(b"LUCAM-RECORDER"),
            Err(Error::Malformed("header cut short".into()))
        );
        assert_eq!(parse(b"not a ser file at all"), Err(Error::NotSer));
        let b = write(&header(0, 2, 2, 8), &[], None);
        assert!(matches!(parse(&b), Err(Error::Malformed(_))));
        let b = write(&header(0, 2, 2, 17), &[vec![0; 8]], None);
        assert!(matches!(parse(&b), Err(Error::Unsupported(_))));
        // FEI TIA .ser starts with a byte-order mark and series ID, not the signature.
        assert!(!sniff(&[0x49, 0x49, 0x97, 0x01, 0x10, 0x02]));
    }
}
