//! Borrowed pixel data: rows of interleaved samples, placed somewhere in the image.

/// Interleaved samples of one type.
#[derive(Clone, Copy, Debug)]
pub enum Samples<'a> {
    U8(&'a [u8]),
    U16(&'a [u16]),
    F32(&'a [f32]),
    F64(&'a [f64]),
}

impl Samples<'_> {
    pub fn len(&self) -> usize {
        match self {
            Samples::U8(d) => d.len(),
            Samples::U16(d) => d.len(),
            Samples::F32(d) => d.len(),
            Samples::F64(d) => d.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn get(&self, i: usize) -> f64 {
        match self {
            Samples::U8(d) => f64::from(d[i]),
            Samples::U16(d) => f64::from(d[i]),
            Samples::F32(d) => f64::from(d[i]),
            Samples::F64(d) => d[i],
        }
    }
}

/// A `width`×`height` block of `channels` interleaved samples per pixel, `stride` samples
/// from one row to the next, whose top-left pixel is image pixel `origin`. A view of a region
/// lets statistics run on just the decoded bounding box of a shape.
#[derive(Clone, Copy, Debug)]
pub struct View<'a> {
    pub width: u32,
    pub height: u32,
    pub channels: u32,
    pub stride: usize,
    pub origin: (u32, u32),
    pub data: Samples<'a>,
}

/// The data is shorter than the dimensions say.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShortData;

impl std::fmt::Display for ShortData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("pixel data shorter than its dimensions")
    }
}

impl std::error::Error for ShortData {}

impl<'a> View<'a> {
    /// Tightly packed rows at the image origin.
    pub fn new(
        width: u32,
        height: u32,
        channels: u32,
        data: Samples<'a>,
    ) -> Result<View<'a>, ShortData> {
        View::with_stride(
            width,
            height,
            channels,
            width as usize * channels as usize,
            data,
        )
    }

    /// Rows `stride` samples apart.
    pub fn with_stride(
        width: u32,
        height: u32,
        channels: u32,
        stride: usize,
        data: Samples<'a>,
    ) -> Result<View<'a>, ShortData> {
        let row = width as usize * channels as usize;
        let need = if height == 0 || width == 0 {
            0
        } else {
            (height as usize - 1) * stride + row
        };
        if channels == 0 || stride < row || data.len() < need {
            return Err(ShortData);
        }
        Ok(View {
            width,
            height,
            channels,
            stride,
            origin: (0, 0),
            data,
        })
    }

    /// The same view placed with its top-left pixel at image pixel `origin`.
    pub fn at(self, origin: (u32, u32)) -> View<'a> {
        View { origin, ..self }
    }

    /// The image-space pixel rectangle `(x0, y0, x1, y1)`, exclusive ends.
    pub fn extent(&self) -> (u32, u32, u32, u32) {
        (
            self.origin.0,
            self.origin.1,
            self.origin.0 + self.width,
            self.origin.1 + self.height,
        )
    }

    /// Sample `c` of image pixel `(x, y)`, which must be inside the view.
    pub fn get(&self, x: u32, y: u32, c: u32) -> f64 {
        let (vx, vy) = ((x - self.origin.0) as usize, (y - self.origin.1) as usize);
        self.data
            .get(vy * self.stride + vx * self.channels as usize + c as usize)
    }

    /// Bilinear interpolation of channel `c` at image point `(x, y)` between pixel centres;
    /// within half a pixel of the view's edge the edge pixels extend. `None` outside the view.
    pub fn bilinear(&self, x: f64, y: f64, c: u32) -> Option<f64> {
        let (x0, y0, x1, y1) = self.extent();
        if !(x >= f64::from(x0) && x <= f64::from(x1) && y >= f64::from(y0) && y <= f64::from(y1))
            || self.width == 0
            || self.height == 0
        {
            return None;
        }
        let u = (x - 0.5 - f64::from(x0)).clamp(0.0, f64::from(self.width - 1));
        let v = (y - 0.5 - f64::from(y0)).clamp(0.0, f64::from(self.height - 1));
        let (i, j) = (u.floor(), v.floor());
        let (fx, fy) = (u - i, v - j);
        let (i, j) = (i as u32 + x0, j as u32 + y0);
        let i1 = (i + 1).min(x1 - 1);
        let j1 = (j + 1).min(y1 - 1);
        let top = self.get(i, j, c) * (1.0 - fx) + self.get(i1, j, c) * fx;
        let bottom = self.get(i, j1, c) * (1.0 - fx) + self.get(i1, j1, c) * fx;
        Some(top * (1.0 - fy) + bottom * fy)
    }
}
