//! Crate-local planar image type used by the composition layer.
//!
//! [`HeifFrame`] is a tightly packed planar YCbCr / monochrome picture
//! with an optional alpha plane: one byte per sample at 8 bits, one
//! little-endian 16-bit word per sample above 8 bits (sample values in
//! the low bits). It mirrors what the codec crates emit and what the
//! framework's `VideoFrame` carries, without depending on either, so
//! the standalone build can compose grids / overlays / alpha over
//! frames decoded by any HEVC / AV1 implementation.

use crate::error::{HeifError, Result};

/// Chroma sampling structure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Chroma {
    /// 4:0:0 — luma only.
    Mono,
    /// 4:2:0 — chroma halved horizontally and vertically.
    Yuv420,
    /// 4:2:2 — chroma halved horizontally.
    Yuv422,
    /// 4:4:4 — full-resolution chroma.
    Yuv444,
}

impl Chroma {
    /// From an HEVC / AV1-style `chroma_format_idc`.
    pub fn from_idc(idc: u8) -> Option<Self> {
        match idc {
            0 => Some(Chroma::Mono),
            1 => Some(Chroma::Yuv420),
            2 => Some(Chroma::Yuv422),
            3 => Some(Chroma::Yuv444),
            _ => None,
        }
    }

    /// The `chroma_format_idc` value.
    pub fn idc(&self) -> u8 {
        match self {
            Chroma::Mono => 0,
            Chroma::Yuv420 => 1,
            Chroma::Yuv422 => 2,
            Chroma::Yuv444 => 3,
        }
    }

    /// `(shift_x, shift_y)` of the chroma planes relative to luma.
    pub fn shift(&self) -> (u32, u32) {
        match self {
            Chroma::Mono => (0, 0),
            Chroma::Yuv420 => (1, 1),
            Chroma::Yuv422 => (1, 0),
            Chroma::Yuv444 => (0, 0),
        }
    }

    /// Number of colour planes (1 or 3).
    pub fn colour_planes(&self) -> usize {
        if *self == Chroma::Mono {
            1
        } else {
            3
        }
    }

    /// Chroma plane size for a `width × height` luma plane.
    pub fn chroma_dims(&self, width: u32, height: u32) -> (u32, u32) {
        let (sx, sy) = self.shift();
        (
            (width + (1 << sx) - 1) >> sx,
            (height + (1 << sy) - 1) >> sy,
        )
    }
}

/// Where a subsampled chroma sample sits among the two luma samples it covers, on one
/// axis.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Site {
    /// On the first (left or top) luma sample: "co-sited".
    #[default]
    Start,
    /// Midway between the two.
    Centre,
    /// On the second (right or bottom) luma sample.
    End,
}

impl Site {
    /// The offset from the first covered luma sample, in half luma samples (0, 1 or 2).
    pub fn half_offset(self) -> i64 {
        match self {
            Site::Start => 0,
            Site::Centre => 1,
            Site::End => 2,
        }
    }

    /// The site after the axis is reversed (over an even number of luma samples).
    pub fn mirrored(self) -> Site {
        match self {
            Site::Start => Site::End,
            Site::Centre => Site::Centre,
            Site::End => Site::Start,
        }
    }
}

/// Where subsampled chroma sits (H.273 chroma sample location). Only the subsampled axes
/// mean anything; transforms that reverse or swap axes carry it along.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ChromaSiting {
    /// Horizontally.
    pub x: Site,
    /// Vertically.
    pub y: Site,
}

impl Default for ChromaSiting {
    /// H.273 chroma sample location 0, the HEVC, AVC and VVC default: co-sited on the even
    /// luma columns, midway between the rows.
    fn default() -> Self {
        Self::from_h273(0)
    }
}

impl ChromaSiting {
    /// Midway on both axes (JPEG / JFIF).
    pub const CENTRE: ChromaSiting = ChromaSiting {
        x: Site::Centre,
        y: Site::Centre,
    };

    /// From an H.273 `ChromaSampleLocType` (0..=5); other values read as 0.
    pub fn from_h273(loc: u8) -> Self {
        let (x, y) = match loc {
            1 => (Site::Centre, Site::Centre),
            2 => (Site::Start, Site::Start),
            3 => (Site::Centre, Site::Start),
            4 => (Site::Start, Site::End),
            5 => (Site::Centre, Site::End),
            _ => (Site::Start, Site::Centre),
        };
        Self { x, y }
    }
}

/// Sample layout of a [`HeifFrame`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct HeifPixelFormat {
    /// Chroma structure.
    pub chroma: Chroma,
    /// Bits per sample (8..=16).
    pub bit_depth: u8,
    /// Whether a full-resolution alpha plane follows the colour planes.
    pub has_alpha: bool,
    /// Where subsampled chroma sits.
    pub siting: ChromaSiting,
}

impl HeifPixelFormat {
    /// Build a format; `bit_depth` outside `8..=16` is rejected.
    pub fn new(chroma: Chroma, bit_depth: u8, has_alpha: bool) -> Result<Self> {
        if !(8..=16).contains(&bit_depth) {
            return Err(HeifError::unsupported(format!("bit depth {bit_depth}")));
        }
        Ok(Self {
            chroma,
            bit_depth,
            has_alpha,
            siting: ChromaSiting::default(),
        })
    }

    /// The same layout with chroma sited `siting`.
    pub fn with_siting(self, siting: ChromaSiting) -> Self {
        Self { siting, ..self }
    }

    /// Bytes per sample (1 or 2).
    pub fn bytes_per_sample(&self) -> usize {
        if self.bit_depth > 8 {
            2
        } else {
            1
        }
    }

    /// Maximum sample value.
    pub fn max_value(&self) -> u16 {
        ((1u32 << self.bit_depth) - 1) as u16
    }

    /// Number of planes (colour planes + alpha).
    pub fn plane_count(&self) -> usize {
        self.chroma.colour_planes() + self.has_alpha as usize
    }

    /// Index of the alpha plane, when present.
    pub fn alpha_plane(&self) -> Option<usize> {
        self.has_alpha.then_some(self.chroma.colour_planes())
    }

    /// Size of plane `plane` for a `width × height` picture.
    pub fn plane_dims(&self, plane: usize, width: u32, height: u32) -> (u32, u32) {
        if plane == 0 || Some(plane) == self.alpha_plane() {
            (width, height)
        } else {
            self.chroma.chroma_dims(width, height)
        }
    }

    /// The same layout with an alpha plane.
    pub fn with_alpha(&self) -> Self {
        Self {
            has_alpha: true,
            ..*self
        }
    }

    /// The same layout without an alpha plane.
    pub fn without_alpha(&self) -> Self {
        Self {
            has_alpha: false,
            ..*self
        }
    }
}

/// The two chroma samples (clamped to the `chroma` samples there are) and the weight of the
/// second, in quarters (0..=3), for each of `luma` samples along an axis subsampled by
/// `shift` (0 or 1) with chroma at `site`: bilinear interpolation at the sited positions,
/// the chroma position of luma sample `x` being `(x − offset) / 2`.
pub fn chroma_taps(luma: u32, chroma: u32, shift: u32, site: Site) -> Vec<(u32, u32, u32)> {
    let last = chroma.saturating_sub(1);
    (0..luma)
        .map(|x| {
            if shift == 0 {
                let c = x.min(last);
                return (c, c, 0);
            }
            let q = 2 * i64::from(x) - site.half_offset();
            if q <= 0 {
                return (0, 0, 0);
            }
            let c0 = (q / 4) as u32;
            let w = (q % 4) as u32;
            if c0 >= last {
                (last, last, 0)
            } else {
                (c0, c0 + 1, w)
            }
        })
        .collect()
}

/// One plane: `stride` bytes per row, rows packed top to bottom. The
/// contract's `Plane` (identical fields in every image crate).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plane {
    /// Bytes per row.
    pub stride: usize,
    /// `stride × rows` bytes.
    pub data: Vec<u8>,
}

impl Plane {
    /// Every field as a positional argument, in declaration order.
    pub fn new(stride: usize, data: Vec<u8>) -> Self {
        Self { stride, data }
    }
}

/// A planar picture.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HeifFrame {
    /// Luma width in pixels.
    pub width: u32,
    /// Luma height in pixels.
    pub height: u32,
    /// Sample layout.
    pub format: HeifPixelFormat,
    /// The planes, `format.plane_count()` of them.
    pub planes: Vec<Plane>,
}

/// Upper bound on the luma pixel count of a frame this crate allocates.
pub const MAX_FRAME_PIXELS: u64 = 1 << 30;

impl HeifFrame {
    /// Allocate a frame filled with `value` in every plane.
    pub fn filled(width: u32, height: u32, format: HeifPixelFormat, value: u16) -> Result<Self> {
        Self::check_dims(width, height)?;
        let bps = format.bytes_per_sample();
        let mut planes = Vec::with_capacity(format.plane_count());
        for p in 0..format.plane_count() {
            let (w, h) = format.plane_dims(p, width, height);
            let stride = w as usize * bps;
            let mut data = vec![0u8; stride * h as usize];
            if value != 0 {
                if bps == 1 {
                    data.fill(value as u8);
                } else {
                    for px in data.chunks_exact_mut(2) {
                        px.copy_from_slice(&value.to_le_bytes());
                    }
                }
            }
            planes.push(Plane { stride, data });
        }
        Ok(Self {
            width,
            height,
            format,
            planes,
        })
    }

    /// Allocate a black (all-zero) frame.
    pub fn zeroed(width: u32, height: u32, format: HeifPixelFormat) -> Result<Self> {
        Self::filled(width, height, format, 0)
    }

    fn check_dims(width: u32, height: u32) -> Result<()> {
        if width == 0 || height == 0 {
            return Err(HeifError::invalid("zero-sized frame"));
        }
        if width as u64 * height as u64 > MAX_FRAME_PIXELS {
            return Err(HeifError::exhausted(format!(
                "frame {width}x{height} exceeds {MAX_FRAME_PIXELS} pixels"
            )));
        }
        Ok(())
    }

    /// Verify plane count / sizes match the declared layout.
    pub fn validate(&self) -> Result<()> {
        Self::check_dims(self.width, self.height)?;
        if self.planes.len() != self.format.plane_count() {
            return Err(HeifError::invalid(format!(
                "frame has {} planes, layout needs {}",
                self.planes.len(),
                self.format.plane_count()
            )));
        }
        let bps = self.format.bytes_per_sample();
        for (i, p) in self.planes.iter().enumerate() {
            let (w, h) = self.format.plane_dims(i, self.width, self.height);
            if p.stride < w as usize * bps {
                return Err(HeifError::invalid(format!(
                    "plane {i}: stride {} < {} bytes per row",
                    p.stride,
                    w as usize * bps
                )));
            }
            if p.stride == 0 || p.data.len() < p.stride * (h as usize - 1) + w as usize * bps {
                return Err(HeifError::invalid(format!(
                    "plane {i}: {} bytes, need {} rows of stride {}",
                    p.data.len(),
                    h,
                    p.stride
                )));
            }
        }
        Ok(())
    }

    /// Size of plane `plane`.
    pub fn plane_dims(&self, plane: usize) -> (u32, u32) {
        self.format.plane_dims(plane, self.width, self.height)
    }

    /// Read one sample (no bounds check beyond the slice's own).
    #[inline]
    pub fn sample(&self, plane: usize, x: u32, y: u32) -> u16 {
        let p = &self.planes[plane];
        let bps = self.format.bytes_per_sample();
        let i = y as usize * p.stride + x as usize * bps;
        if bps == 1 {
            p.data[i] as u16
        } else {
            u16::from_le_bytes([p.data[i], p.data[i + 1]])
        }
    }

    /// Write one sample.
    #[inline]
    pub fn set_sample(&mut self, plane: usize, x: u32, y: u32, v: u16) {
        let bps = self.format.bytes_per_sample();
        let p = &mut self.planes[plane];
        let i = y as usize * p.stride + x as usize * bps;
        if bps == 1 {
            p.data[i] = v as u8;
        } else {
            p.data[i..i + 2].copy_from_slice(&v.to_le_bytes());
        }
    }

    /// One row of plane `plane` as bytes (`width × bps` long).
    pub fn row(&self, plane: usize, y: u32) -> &[u8] {
        let p = &self.planes[plane];
        let (w, _) = self.plane_dims(plane);
        let start = y as usize * p.stride;
        &p.data[start..start + w as usize * self.format.bytes_per_sample()]
    }

    /// A tightly packed copy (stride == row bytes).
    pub fn tight(&self) -> Self {
        let bps = self.format.bytes_per_sample();
        let planes = (0..self.planes.len())
            .map(|i| {
                let (w, h) = self.plane_dims(i);
                let stride = w as usize * bps;
                let mut data = Vec::with_capacity(stride * h as usize);
                for y in 0..h {
                    data.extend_from_slice(self.row(i, y));
                }
                Plane { stride, data }
            })
            .collect();
        Self {
            width: self.width,
            height: self.height,
            format: self.format,
            planes,
        }
    }

    /// The colour planes only (drops alpha).
    pub fn without_alpha(&self) -> Self {
        let n = self.format.chroma.colour_planes();
        Self {
            width: self.width,
            height: self.height,
            format: self.format.without_alpha(),
            planes: self.planes[..n].to_vec(),
        }
    }

    /// The alpha plane as a monochrome frame, when present.
    pub fn alpha_as_frame(&self) -> Option<Self> {
        let i = self.format.alpha_plane()?;
        Some(Self {
            width: self.width,
            height: self.height,
            format: HeifPixelFormat {
                chroma: Chroma::Mono,
                bit_depth: self.format.bit_depth,
                has_alpha: false,
                siting: self.format.siting,
            },
            planes: vec![self.planes[i].clone()],
        })
    }

    /// Promote to 4:4:4 (chroma interpolated bilinearly at its [`ChromaSiting`], rounded
    /// to the nearest code; neutral chroma added for monochrome), keeping depth and alpha.
    pub fn promote_to_444(&self) -> Result<Self> {
        let fmt = HeifPixelFormat {
            chroma: Chroma::Yuv444,
            ..self.format
        };
        let mid = 1u16 << (self.format.bit_depth - 1);
        let mut out = HeifFrame::filled(self.width, self.height, fmt, mid)?;
        // Luma.
        for y in 0..self.height {
            let row = self.row(0, y).to_vec();
            let stride = out.planes[0].stride;
            out.planes[0].data[y as usize * stride..y as usize * stride + row.len()]
                .copy_from_slice(&row);
        }
        if self.format.chroma != Chroma::Mono {
            let (sx, sy) = self.format.chroma.shift();
            let (cw, ch) = self.plane_dims(1);
            let s = self.format.siting;
            let xs = chroma_taps(self.width, cw, sx, s.x);
            let ys = chroma_taps(self.height, ch, sy, s.y);
            for p in 1..3 {
                for (y, &(y0, y1, wy)) in ys.iter().enumerate() {
                    for (x, &(x0, x1, wx)) in xs.iter().enumerate() {
                        let at = |cx, cy| u32::from(self.sample(p, cx, cy));
                        let top = at(x0, y0) * (4 - wx) + at(x1, y0) * wx;
                        let bottom = at(x0, y1) * (4 - wx) + at(x1, y1) * wx;
                        let v = (top * (4 - wy) + bottom * wy + 8) >> 4;
                        out.set_sample(p, x as u32, y as u32, v as u16);
                    }
                }
            }
        }
        if let Some(a) = self.format.alpha_plane() {
            let dst = out.format.alpha_plane().expect("alpha kept");
            out.planes[dst] = self.planes[a].clone();
        }
        Ok(out)
    }

    /// Attach `alpha` (a monochrome frame of the same size and depth)
    /// as the alpha plane.
    pub fn with_alpha_plane(&self, alpha: &HeifFrame) -> Result<Self> {
        if alpha.width != self.width || alpha.height != self.height {
            return Err(HeifError::invalid(format!(
                "alpha plane {}x{} does not match the {}x{} master",
                alpha.width, alpha.height, self.width, self.height
            )));
        }
        if alpha.format.bit_depth != self.format.bit_depth {
            return Err(HeifError::unsupported(format!(
                "alpha depth {} differs from master depth {}",
                alpha.format.bit_depth, self.format.bit_depth
            )));
        }
        let mut out = self.without_alpha();
        out.format = out.format.with_alpha();
        out.planes.push(alpha.planes[0].clone());
        out.validate()?;
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fmt(chroma: Chroma, depth: u8, alpha: bool) -> HeifPixelFormat {
        HeifPixelFormat::new(chroma, depth, alpha).unwrap()
    }

    #[test]
    fn plane_geometry() {
        let f = fmt(Chroma::Yuv420, 8, true);
        assert_eq!(f.plane_count(), 4);
        assert_eq!(f.alpha_plane(), Some(3));
        assert_eq!(f.plane_dims(1, 5, 3), (3, 2));
        assert_eq!(f.plane_dims(3, 5, 3), (5, 3));
        let g = fmt(Chroma::Mono, 10, false);
        assert_eq!(g.plane_count(), 1);
        assert_eq!(g.bytes_per_sample(), 2);
        assert_eq!(g.max_value(), 1023);
        assert_eq!(
            fmt(Chroma::Yuv422, 8, false).chroma.chroma_dims(5, 3),
            (3, 3)
        );
        assert!(HeifPixelFormat::new(Chroma::Mono, 7, false).is_err());
        assert_eq!(Chroma::from_idc(2), Some(Chroma::Yuv422));
        assert_eq!(Chroma::from_idc(4), None);
    }

    #[test]
    fn filled_sample_roundtrip_16bit() {
        let mut f = HeifFrame::filled(4, 2, fmt(Chroma::Yuv444, 10, false), 512).unwrap();
        f.validate().unwrap();
        assert_eq!(f.sample(2, 3, 1), 512);
        f.set_sample(2, 3, 1, 1023);
        assert_eq!(f.sample(2, 3, 1), 1023);
        assert_eq!(f.row(0, 1).len(), 8);
        let t = f.tight();
        assert_eq!(t, f);
        assert!(HeifFrame::zeroed(0, 1, f.format).is_err());
    }

    #[test]
    fn alpha_attach_and_detach() {
        let base = HeifFrame::filled(2, 2, fmt(Chroma::Yuv420, 8, false), 7).unwrap();
        let alpha = HeifFrame::filled(2, 2, fmt(Chroma::Mono, 8, false), 200).unwrap();
        let with = base.with_alpha_plane(&alpha).unwrap();
        assert!(with.format.has_alpha);
        assert_eq!(with.sample(3, 1, 1), 200);
        assert_eq!(with.without_alpha(), base);
        assert_eq!(with.alpha_as_frame().unwrap(), alpha);
        let bad = HeifFrame::filled(3, 2, fmt(Chroma::Mono, 8, false), 0).unwrap();
        assert!(base.with_alpha_plane(&bad).is_err());
        let deep = HeifFrame::filled(2, 2, fmt(Chroma::Mono, 10, false), 0).unwrap();
        assert!(base.with_alpha_plane(&deep).is_err());
    }

    #[test]
    fn validate_catches_short_planes() {
        let mut f = HeifFrame::zeroed(4, 4, fmt(Chroma::Yuv420, 8, false)).unwrap();
        f.planes[1].data.truncate(1);
        assert!(f.validate().is_err());
        f.planes.pop();
        assert!(f.validate().is_err());
    }
}
