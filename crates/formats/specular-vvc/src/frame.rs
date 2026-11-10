//! What a decoded picture looks like once it leaves the decoder.

/// Chroma sampling of a picture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ChromaFormat {
    Monochrome,
    #[default]
    Yuv420,
    Yuv422,
    Yuv444,
}

impl ChromaFormat {
    /// From sps_chroma_format_idc.
    pub const fn from_idc(idc: u8) -> Option<ChromaFormat> {
        Some(match idc {
            0 => ChromaFormat::Monochrome,
            1 => ChromaFormat::Yuv420,
            2 => ChromaFormat::Yuv422,
            3 => ChromaFormat::Yuv444,
            _ => return None,
        })
    }

    pub const fn x_shift(self) -> u32 {
        match self {
            ChromaFormat::Yuv420 | ChromaFormat::Yuv422 => 1,
            _ => 0,
        }
    }

    pub const fn y_shift(self) -> u32 {
        match self {
            ChromaFormat::Yuv420 => 1,
            _ => 0,
        }
    }
}

/// A decoded picture cropped to its conformance window: planar YCbCr (or G/B/R when the
/// matrix is the identity), one `u16` per sample whatever the bit depth.
#[derive(Debug, Clone, Default)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub bit_depth: u8,
    pub chroma: ChromaFormat,
    /// Luma, `y_stride` samples per row.
    pub y: Vec<u16>,
    /// Cb and Cr, `c_stride` samples per row; empty when monochrome.
    pub cb: Vec<u16>,
    pub cr: Vec<u16>,
    pub y_stride: u32,
    pub c_stride: u32,
}

impl Frame {
    /// Width and height of each chroma plane.
    pub fn chroma_size(&self) -> (u32, u32) {
        if self.chroma == ChromaFormat::Monochrome {
            return (0, 0);
        }
        (
            self.width.div_ceil(1 << self.chroma.x_shift()),
            self.height.div_ceil(1 << self.chroma.y_shift()),
        )
    }
}
