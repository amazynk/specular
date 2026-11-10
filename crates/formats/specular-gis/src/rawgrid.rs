//! Raw raster bands as values: the sample types and band layouts (band sequential, band
//! interleaved by line or by pixel, with ESRI's row and band gaps) of ENVI, ESRI `.hdr`
//! labelled and ERDAS IMAGINE files.

/// A stored sample type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SampleType {
    U8,
    I8,
    U16,
    I16,
    U32,
    I32,
    U64,
    I64,
    F32,
    F64,
    /// Complex 32-bit float pairs: shown as magnitude.
    C32,
    /// Complex 64-bit float pairs: shown as magnitude.
    C64,
    /// Complex 16-bit signed integer pairs (TIFF complex int).
    CI16,
    /// Complex 32-bit signed integer pairs.
    CI32,
}

impl SampleType {
    pub fn bytes(self) -> usize {
        match self {
            SampleType::U8 | SampleType::I8 => 1,
            SampleType::U16 | SampleType::I16 => 2,
            SampleType::U32 | SampleType::I32 | SampleType::F32 | SampleType::CI16 => 4,
            SampleType::U64
            | SampleType::I64
            | SampleType::F64
            | SampleType::C32
            | SampleType::CI32 => 8,
            SampleType::C64 => 16,
        }
    }

    pub fn bits(self) -> u8 {
        (self.bytes() * 8).min(128) as u8
    }

    /// "16-bit signed", "32-bit float", "complex 64-bit float".
    pub fn name(self) -> String {
        match self {
            SampleType::U8 => "8-bit unsigned".into(),
            SampleType::I8 => "8-bit signed".into(),
            SampleType::U16 => "16-bit unsigned".into(),
            SampleType::I16 => "16-bit signed".into(),
            SampleType::U32 => "32-bit unsigned".into(),
            SampleType::I32 => "32-bit signed".into(),
            SampleType::U64 => "64-bit unsigned".into(),
            SampleType::I64 => "64-bit signed".into(),
            SampleType::F32 => "32-bit float".into(),
            SampleType::F64 => "64-bit float".into(),
            SampleType::C32 => "complex 32-bit float".into(),
            SampleType::C64 => "complex 64-bit float".into(),
            SampleType::CI16 => "complex 16-bit signed".into(),
            SampleType::CI32 => "complex 32-bit signed".into(),
        }
    }

    /// Whether samples are (real, imaginary) pairs.
    pub fn is_complex(self) -> bool {
        matches!(
            self,
            SampleType::C32 | SampleType::C64 | SampleType::CI16 | SampleType::CI32
        )
    }

    /// One complex sample as (real, imaginary); a real sample has a zero imaginary part.
    #[inline]
    pub fn read_complex(self, b: &[u8], big_endian: bool) -> (f64, f64) {
        let half = self.bytes() / 2;
        let part = match self {
            SampleType::C32 => SampleType::F32,
            SampleType::C64 => SampleType::F64,
            SampleType::CI16 => SampleType::I16,
            SampleType::CI32 => SampleType::I32,
            _ => return (self.read(b, big_endian), 0.0),
        };
        (part.read(b, big_endian), part.read(&b[half..], big_endian))
    }

    /// One sample as `f64` (complex: the magnitude).
    #[inline]
    pub fn read(self, b: &[u8], big_endian: bool) -> f64 {
        macro_rules! num {
            ($t:ty, $n:expr) => {{
                let a: [u8; $n] = b[..$n].try_into().unwrap_or([0; $n]);
                if big_endian {
                    <$t>::from_be_bytes(a)
                } else {
                    <$t>::from_le_bytes(a)
                }
            }};
        }
        match self {
            SampleType::U8 => f64::from(b[0]),
            SampleType::I8 => f64::from(b[0] as i8),
            SampleType::U16 => f64::from(num!(u16, 2)),
            SampleType::I16 => f64::from(num!(i16, 2)),
            SampleType::U32 => f64::from(num!(u32, 4)),
            SampleType::I32 => f64::from(num!(i32, 4)),
            SampleType::U64 => num!(u64, 8) as f64,
            SampleType::I64 => num!(i64, 8) as f64,
            SampleType::F32 => f64::from(num!(f32, 4)),
            SampleType::F64 => num!(f64, 8),
            SampleType::C32 | SampleType::C64 | SampleType::CI16 | SampleType::CI32 => {
                let (re, im) = self.read_complex(b, big_endian);
                re.hypot(im)
            }
        }
    }
}

/// How bands are interleaved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Interleave {
    /// Band sequential: each band's rows in turn.
    Bsq,
    /// Band interleaved by line: each row holds a line of every band.
    Bil,
    /// Band interleaved by pixel.
    Bip,
}

impl Interleave {
    pub fn parse(s: &str) -> Option<Interleave> {
        match s.trim().to_ascii_lowercase().as_str() {
            "bsq" => Some(Interleave::Bsq),
            "bil" => Some(Interleave::Bil),
            "bip" => Some(Interleave::Bip),
            _ => None,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Interleave::Bsq => "BSQ",
            Interleave::Bil => "BIL",
            Interleave::Bip => "BIP",
        }
    }
}

/// Where each sample of a raw raster is: byte offset of band `b`, row `y`, column `x` =
/// `offset + b·band_stride + y·row_stride + x·pixel_stride`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RawLayout {
    pub width: u32,
    pub height: u32,
    pub bands: u32,
    pub sample: SampleType,
    pub big_endian: bool,
    pub interleave: Interleave,
    pub offset: u64,
    pub band_stride: u64,
    pub row_stride: u64,
    pub pixel_stride: u64,
}

impl RawLayout {
    /// The packed layout of an interleave (no gaps).
    pub fn packed(
        width: u32,
        height: u32,
        bands: u32,
        sample: SampleType,
        big_endian: bool,
        interleave: Interleave,
        offset: u64,
    ) -> RawLayout {
        let s = sample.bytes() as u64;
        let (w, h, n) = (u64::from(width), u64::from(height), u64::from(bands));
        let (band_stride, row_stride, pixel_stride) = match interleave {
            // Saturating: a stride past u64 lies beyond any file, so reads come back truncated.
            Interleave::Bsq => (w.saturating_mul(h).saturating_mul(s), w * s, s),
            Interleave::Bil => (w * s, w.saturating_mul(n).saturating_mul(s), s),
            Interleave::Bip => (s, w.saturating_mul(n).saturating_mul(s), n * s),
        };
        RawLayout {
            width,
            height,
            bands,
            sample,
            big_endian,
            interleave,
            offset,
            band_stride,
            row_stride,
            pixel_stride,
        }
    }

    /// One past the last byte read (`u64::MAX` when that is past u64: header values are
    /// the file's own).
    pub fn end(&self) -> u64 {
        if self.width == 0 || self.height == 0 || self.bands == 0 {
            return self.offset;
        }
        self.offset
            .saturating_add(u64::from(self.bands - 1).saturating_mul(self.band_stride))
            .saturating_add(u64::from(self.height - 1).saturating_mul(self.row_stride))
            .saturating_add(u64::from(self.width - 1).saturating_mul(self.pixel_stride))
            .saturating_add(self.sample.bytes() as u64)
    }

    /// Band `band` as values, row 0 first; `nodata` (and non-finite values) become NaN.
    /// Rows past the end of `data` are NaN too, and the result says whether any were.
    pub fn band(&self, data: &[u8], band: u32, nodata: Option<f64>) -> (Vec<f32>, bool) {
        let mut out = vec![f32::NAN; self.width as usize * self.height as usize];
        let (s, be) = (self.sample, self.big_endian);
        let truncated = self.rows(data, band, |y, r, ps| {
            let w = self.width as usize;
            for (x, v) in out[y * w..(y + 1) * w].iter_mut().enumerate() {
                let o = x * ps;
                let f = s.read(&r[o..o + s.bytes()], be);
                *v = match nodata {
                    Some(nd) if f == nd || (nd.is_nan() && f.is_nan()) => f32::NAN,
                    _ => f as f32,
                };
            }
        });
        (out, truncated)
    }

    /// Band `band` as (real, imaginary) pairs, row 0 first (a real band has zero imaginary
    /// parts). Rows past the end of `data` are NaN, and the result says whether any were.
    pub fn band_complex(&self, data: &[u8], band: u32) -> (Vec<[f32; 2]>, bool) {
        let mut out = vec![[f32::NAN; 2]; self.width as usize * self.height as usize];
        let (s, be) = (self.sample, self.big_endian);
        let truncated = self.rows(data, band, |y, r, ps| {
            let w = self.width as usize;
            for (x, v) in out[y * w..(y + 1) * w].iter_mut().enumerate() {
                let o = x * ps;
                let (re, im) = s.read_complex(&r[o..o + s.bytes()], be);
                *v = [re as f32, im as f32];
            }
        });
        (out, truncated)
    }

    /// Calls `row(y, bytes, pixel_stride)` for each row of `band` within `data`, the bytes
    /// starting at the row's first sample; true when rows past the end were left out.
    fn rows(&self, data: &[u8], band: u32, mut row: impl FnMut(usize, &[u8], usize)) -> bool {
        let (w, h) = (self.width as usize, self.height as usize);
        let s = self.sample.bytes();
        if band >= self.bands {
            return true;
        }
        if w == 0 {
            return false;
        }
        // Checked: offsets and strides come from the header, and one past u64 is past the
        // end of the data as well.
        let base = u64::from(band)
            .checked_mul(self.band_stride)
            .and_then(|b| b.checked_add(self.offset));
        let span = (w as u64)
            .saturating_sub(1)
            .checked_mul(self.pixel_stride)
            .and_then(|v| v.checked_add(s as u64));
        for y in 0..h {
            let start = (y as u64)
                .checked_mul(self.row_stride)
                .zip(base)
                .and_then(|(r, b)| r.checked_add(b));
            let Some((start, last)) = start
                .zip(span)
                .and_then(|(a, n)| Some((a, a.checked_add(n)?)))
                .filter(|&(_, last)| last <= data.len() as u64)
            else {
                return true;
            };
            row(
                y,
                &data[start as usize..last as usize],
                self.pixel_stride as usize,
            );
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layouts() {
        // 3 × 2, 2 bands of u16 big-endian: value = 100·band + 10·row + col.
        let v = |b: u32, y: u32, x: u32| (100 * b + 10 * y + x) as u16;
        for il in [Interleave::Bsq, Interleave::Bil, Interleave::Bip] {
            let l = RawLayout::packed(3, 2, 2, SampleType::U16, true, il, 4);
            let mut d = vec![0u8; l.end() as usize];
            for b in 0..2 {
                for y in 0..2 {
                    for x in 0..3 {
                        let o = (l.offset
                            + u64::from(b) * l.band_stride
                            + u64::from(y) * l.row_stride
                            + u64::from(x) * l.pixel_stride)
                            as usize;
                        d[o..o + 2].copy_from_slice(&v(b, y, x).to_be_bytes());
                    }
                }
            }
            assert_eq!(d.len(), 4 + 24);
            let (b1, t) = l.band(&d, 1, Some(111.0));
            assert!(!t);
            assert_eq!(b1[0], 100.0);
            assert!(b1[4].is_nan());
            assert_eq!(b1[5], 112.0);
            let (_, t) = l.band(&d[..20], 1, None);
            assert!(t);
        }
        assert_eq!(
            SampleType::C32.read(&[0, 0, 0x40, 0x40, 0, 0, 0x80, 0x40], false),
            5.0
        );
        assert_eq!(SampleType::I16.read(&[0xff, 0xfe], true), -2.0);
        assert_eq!(
            SampleType::CI16.read_complex(&[0, 3, 0xff, 0xfc], true),
            (3.0, -4.0)
        );
        assert_eq!(SampleType::CI16.read(&[3, 0, 4, 0], false), 5.0);
    }

    #[test]
    fn complex_bands() {
        // 2 × 1, 2 bands of complex i16 little-endian, BIP: (band + x, -x).
        let l = RawLayout::packed(2, 1, 2, SampleType::CI16, false, Interleave::Bip, 0);
        let mut d = Vec::new();
        for x in 0..2i16 {
            for b in 0..2i16 {
                d.extend((b + x).to_le_bytes());
                d.extend((-x).to_le_bytes());
            }
        }
        let (v, t) = l.band_complex(&d, 1);
        assert!(!t);
        assert_eq!(v, [[1.0, 0.0], [2.0, -1.0]]);
        let (v, t) = l.band_complex(&d[..6], 1);
        assert!(t && v[0][0].is_nan());
    }

    #[test]
    fn hostile_offsets_and_strides() {
        // A header offset near u64::MAX, and strides whose products pass u64.
        let mut l = RawLayout::packed(
            4,
            4,
            2,
            SampleType::U16,
            false,
            Interleave::Bsq,
            u64::MAX - 3,
        );
        assert_eq!(l.end(), u64::MAX);
        let d = [0u8; 64];
        let (v, t) = l.band(&d, 1, None);
        assert!(t && v.iter().all(|v| v.is_nan()));
        l.offset = 0;
        l.row_stride = u64::MAX / 2;
        l.band_stride = u64::MAX;
        assert_eq!(l.end(), u64::MAX);
        assert!(l.band(&d, 0, None).1);
        assert!(l.band(&d, 1, None).1);
        let big = RawLayout::packed(
            u32::MAX,
            u32::MAX,
            u32::MAX,
            SampleType::C64,
            true,
            Interleave::Bip,
            0,
        );
        assert_eq!(big.row_stride, u64::MAX);
        assert_eq!(big.end(), u64::MAX);
    }
}
