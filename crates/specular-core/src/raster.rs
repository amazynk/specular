use crate::hdr::{HdrEncoding, Primaries};
use crate::yuv::YuvEncoding;

/// How the planes of a [`Raster`] are to be interpreted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PixelLayout {
    /// One 8-bit plane.
    Gray,
    /// Y, Cb, Cr planes, BT.601 full range (JFIF), chroma sited centred.
    YCbCr,
    /// R, G, B planes.
    Rgb,
    /// One interleaved RGBA8 plane (straight alpha).
    Rgba,
    /// One interleaved plane of native-endian `u16` RGBA (straight alpha, 8 bytes per pixel)
    /// with the given transfer and primaries: 16-bit PNG, PQ/HLG content.
    Rgba16(HdrEncoding),
    /// One interleaved plane of native-endian IEEE half-float RGBA (straight alpha, 8 bytes
    /// per pixel), linear light with 1.0 = SDR reference white, in the given primaries.
    Rgba16F(Primaries),
    /// One plane of native-endian `u16` grey (2 bytes per pixel), sRGB-coded or described by
    /// the image's profile like [`PixelLayout::Gray`]: 16-bit grey PNG and JPEG 2000. Kept
    /// compact in memory and in upload; expanded to RGB on the GPU.
    Gray16,
    /// One interleaved plane of native-endian `u16` grey and alpha (straight alpha, 4 bytes
    /// per pixel), coded like [`PixelLayout::Gray16`].
    GrayAlpha16,
    /// Video: Y, Cb, Cr planes as a decoder outputs them, with the matrix, range, bit depth
    /// (8-bit samples, or native-endian `u16` above 8 bits), chroma siting and the transfer
    /// and primaries of the result. Converted on the GPU; [`crate::yuv::to_rgb`] is the CPU
    /// reference.
    Yuv(YuvEncoding),
}

impl PixelLayout {
    /// Whether the samples are wider than 8 bits (one interleaved plane of 16-bit samples:
    /// [`PixelLayout::pixel_bytes`] per pixel).
    pub fn is_wide(self) -> bool {
        matches!(
            self,
            PixelLayout::Rgba16(_)
                | PixelLayout::Rgba16F(_)
                | PixelLayout::Gray16
                | PixelLayout::GrayAlpha16
        )
    }

    /// Whether the samples are wider than 8 bits in any layout, planar video included (where
    /// [`PixelLayout::is_wide`], which promises one interleaved plane, is false).
    pub fn has_wide_samples(self) -> bool {
        self.is_wide() || matches!(self, PixelLayout::Yuv(e) if e.bits > 8)
    }

    /// Bytes per pixel of the (first) plane: 1 for the 8-bit planar layouts, 4 for RGBA8 and
    /// 16-bit grey + alpha, 2 for 16-bit grey, 8 for the 16-bit RGBA layouts.
    pub fn pixel_bytes(self) -> usize {
        match self {
            PixelLayout::Gray | PixelLayout::YCbCr | PixelLayout::Rgb => 1,
            PixelLayout::Gray16 => 2,
            PixelLayout::Rgba | PixelLayout::GrayAlpha16 => 4,
            PixelLayout::Rgba16(_) | PixelLayout::Rgba16F(_) => 8,
            PixelLayout::Yuv(e) => e.sample_bytes(),
        }
    }

    /// Whether the layout is grey (one sample of colour per pixel).
    pub fn is_gray(self) -> bool {
        matches!(
            self,
            PixelLayout::Gray | PixelLayout::Gray16 | PixelLayout::GrayAlpha16
        )
    }

    /// Whether the layout carries an alpha channel.
    pub fn has_alpha(self) -> bool {
        !matches!(
            self,
            PixelLayout::Gray
                | PixelLayout::YCbCr
                | PixelLayout::Rgb
                | PixelLayout::Gray16
                | PixelLayout::Yuv(_)
        )
    }

    /// Whether the samples can hold values above SDR reference white.
    pub fn is_hdr(self) -> bool {
        match self {
            PixelLayout::Rgba16(e) => e.is_hdr(),
            PixelLayout::Rgba16F(_) => true,
            PixelLayout::Yuv(e) => e.colour.is_hdr(),
            _ => false,
        }
    }
}

/// One 8-bit plane. `h_samp`/`v_samp` are the subsampling divisors relative to the raster
/// size: a plane with `h_div == 2` has half the raster's width (rounded up).
#[derive(Clone, Debug)]
pub struct RasterPlane {
    pub width: u32,
    pub height: u32,
    pub stride: usize,
    pub data: Vec<u8>,
    pub h_div: u8,
    pub v_div: u8,
}

impl RasterPlane {
    pub fn bytes_per_pixel(&self) -> usize {
        if self.width == 0 {
            0
        } else {
            (self.stride / self.width as usize).max(1)
        }
    }
}

/// Decoded pixels of (part of) an image at some scale.
#[derive(Clone, Debug)]
pub struct Raster {
    /// Size of this raster in pixels.
    pub width: u32,
    pub height: u32,
    /// Top-left of this raster inside the scaled image (non-zero for region decodes).
    pub origin: (u32, u32),
    /// Downscale factor relative to the full image (1, 2, 4, 8...).
    pub scale: u32,
    pub layout: PixelLayout,
    pub planes: Vec<RasterPlane>,
    /// The data ended early; the missing part is grey.
    pub truncated: bool,
}

impl Raster {
    pub fn memory_bytes(&self) -> usize {
        self.planes.iter().map(|p| p.data.len()).sum()
    }
}

/// EXIF orientation (1..=8), applied in the view transform; pixels are never rotated.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Hash)]
pub struct Orientation(pub u8);

impl Orientation {
    pub const NORMAL: Orientation = Orientation(1);

    pub fn from_exif(v: u8) -> Orientation {
        if (1..=8).contains(&v) {
            Orientation(v)
        } else {
            Orientation(1)
        }
    }

    /// Whether width and height are exchanged on screen.
    pub fn swaps_axes(self) -> bool {
        self.0 >= 5
    }

    /// Displayed size for a stored size.
    pub fn display_size(self, w: u32, h: u32) -> (u32, u32) {
        if self.swaps_axes() { (h, w) } else { (w, h) }
    }

    /// Decompose into (quarter turns clockwise, horizontal flip applied first).
    pub fn to_rotation_flip(self) -> (u8, bool) {
        match self.0 {
            2 => (0, true),
            3 => (2, false),
            4 => (2, true),
            // 5 is the transpose (row 0 on the left, column 0 at the top): flip, then three
            // quarter turns; 7 the transverse: flip, then one.
            5 => (3, true),
            6 => (1, false),
            7 => (1, true),
            8 => (3, false),
            _ => (0, false),
        }
    }

    pub fn from_rotation_flip(quarter_turns: u8, flip: bool) -> Orientation {
        let q = quarter_turns % 4;
        Orientation(match (q, flip) {
            (0, false) => 1,
            (0, true) => 2,
            (2, false) => 3,
            (2, true) => 4,
            (3, true) => 5,
            (1, false) => 6,
            (1, true) => 7,
            _ => 8,
        })
    }

    /// Compose with an additional user rotation (quarter turns clockwise).
    pub fn rotated(self, quarter_turns: i32) -> Orientation {
        let (q, f) = self.to_rotation_flip();
        let q = (q as i32 + quarter_turns).rem_euclid(4) as u8;
        Orientation::from_rotation_flip(q, f)
    }

    /// Compose with a horizontal flip of the displayed image.
    pub fn flipped_h(self) -> Orientation {
        let (q, f) = self.to_rotation_flip();
        // flip(display) ∘ rot(q) ∘ flip?(f)  ==  rot(-q) ∘ flip ∘ flip?(f)
        Orientation::from_rotation_flip((4 - q) % 4, !f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Where stored pixel (x, y) of a w×h image lands on screen, per the TIFF/EXIF definitions
    /// of the eight orientations, against the view's flip-then-quarter-turns decomposition.
    #[test]
    fn orientation_matches_exif() {
        let (w, h) = (3u32, 2u32);
        let exif = |o: u8, x: u32, y: u32| -> (u32, u32) {
            match o {
                1 => (x, y),
                2 => (w - 1 - x, y),
                3 => (w - 1 - x, h - 1 - y),
                4 => (x, h - 1 - y),
                5 => (y, x),
                6 => (h - 1 - y, x),
                7 => (h - 1 - y, w - 1 - x),
                _ => (y, w - 1 - x),
            }
        };
        for o in 1..=8u8 {
            let (q, flip) = Orientation(o).to_rotation_flip();
            for y in 0..h {
                for x in 0..w {
                    let (mut px, mut py, mut cw, mut ch) = (x, y, w, h);
                    if flip {
                        px = cw - 1 - px;
                    }
                    for _ in 0..q {
                        (px, py) = (ch - 1 - py, px);
                        std::mem::swap(&mut cw, &mut ch);
                    }
                    assert_eq!((px, py), exif(o, x, y), "orientation {o} at ({x}, {y})");
                }
            }
        }
    }

    #[test]
    fn orientation_round_trip() {
        for v in 1..=8u8 {
            let o = Orientation(v);
            let (q, f) = o.to_rotation_flip();
            assert_eq!(Orientation::from_rotation_flip(q, f), o);
        }
        assert_eq!(Orientation(1).rotated(1), Orientation(6));
        assert_eq!(Orientation(6).rotated(-1), Orientation(1));
        assert_eq!(Orientation(1).flipped_h(), Orientation(2));
    }
}
