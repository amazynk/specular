//! Reflectance transformation imaging: images that store, per pixel, how the pixel's colour
//! changes with the direction of the light, so the viewer can move the light.
//!
//! Three families of files are read, all into the same [`Rti`]:
//!
//! - **PTM** (HP Labs polynomial texture maps, `.ptm`, versions 1.1 and 1.2): six biquadratic
//!   coefficients per pixel, either per colour channel (`PTM_FORMAT_RGB`) or for luminance
//!   with a separate colour (`PTM_FORMAT_LRGB`), uncompressed, through a lookup table
//!   (`PTM_FORMAT_PTM_LUT` / `PTM_C_LUT`) or as JPEG coefficient planes with inter-plane
//!   prediction (`PTM_FORMAT_JPEG_RGB` / `JPEG_LRGB`).
//! - **HSH** (Cultural Heritage Imaging's hemispherical harmonics, the `.rti` "universal RTI"
//!   container and the older binary `.hsh`): 4, 9 or 16 terms per colour channel.
//! - **Relight** (CNR-ISTI): a folder with `info.json` and `plane_N.jpg` images, three
//!   coefficient planes per JPEG, with a PTM, HSH, SH, RBF or bilinear basis in the RGB,
//!   LRGB, MRGB or MYCC colour layouts.
//!
//! Every format stores its coefficients as bytes with an affine mapping per plane, so an
//! [`Rti`] keeps them that way: [`Layer`]s of three byte planes (interleaved RGB order) with a
//! scale and offset per channel. Rows are top-down whatever the file's order.
//!
//! Relighting is linear in the coefficients for a given light, so [`Rti::prepare`] reduces
//! any basis and colour layout to one 3×3 matrix per layer plus a constant ([`Linear`]); the
//! GPU shader and the CPU reference ([`Rti::shade`]) evaluate exactly that. Normals come
//! from a least-squares (photometric stereo) fit of the luminance over fixed sample lights,
//! which is also linear in the coefficients ([`Prepared::normal`]).
//!
//! The light direction is `(x, y, z)` with x to the right of the displayed image, y up and z
//! towards the viewer.

mod basis;
mod hsh;
mod ptm;
mod relight;
mod shade;

#[cfg(test)]
mod tests;

use thiserror::Error;

pub use basis::{
    HSH_MAX_THETA, NORMAL_LIGHTS, hsh_weights, ptm_weights, ptm_weights_relight, sh_weights,
};
pub use relight::{is_relight_info, relight_files};
pub use shade::{DEFAULT_LIGHT, Linear, Look, Mode, Prepared};

/// Most coefficient layers (groups of three planes) an image may have.
pub const MAX_LAYERS: usize = 32;

/// Largest width or height accepted.
pub const MAX_SIDE: u32 = 1 << 16;

#[derive(Debug, Error)]
pub enum Error {
    #[error("not a reflectance transformation image")]
    NotRti,
    #[error("malformed {0}")]
    Malformed(String),
    #[error("unsupported {0}")]
    Unsupported(String),
    #[error("truncated {0}")]
    Truncated(&'static str),
    #[error("RTI image too large ({0}×{1})")]
    TooLarge(u32, u32),
    #[error("JPEG coefficient plane: {0}")]
    Jpeg(String),
    #[error("{0}: {1}")]
    Io(String, std::io::Error),
    /// Under the size caps, but more than the memory budget has free now.
    #[error("{0}")]
    NeedsMemory(#[from] specular_core::NeedsMemory),
}

pub type Result<T> = std::result::Result<T, Error>;

/// The file a [`Rti`] came from, with the facts the info panel shows.
#[derive(Clone, Debug, PartialEq)]
pub enum Container {
    /// An HP polynomial texture map.
    Ptm {
        /// `PTM_1.1` or `PTM_1.2`.
        version: String,
        /// The format line, e.g. `PTM_FORMAT_JPEG_LRGB`.
        format: String,
        /// The six scales and biases of the header.
        scale: [f32; 6],
        bias: [i32; 6],
        /// The JPEG quality of compressed files.
        quality: Option<u32>,
    },
    /// A CHI hemispherical-harmonics file.
    Hsh {
        /// True for the old binary `.hsh` header, false for the `.rti` text header.
        legacy: bool,
        /// Per-term scale and bias (`.rti`) or minimum and maximum (`.hsh`).
        scale: Vec<f32>,
        bias: Vec<f32>,
    },
    /// A CNR-ISTI Relight folder.
    Relight {
        /// `ptm`, `hsh`, `sh`, `rbf` or `bilinear`.
        kind: String,
        /// `rgb`, `lrgb`, `mrgb` or `mycc`.
        colorspace: String,
        /// Coefficient planes (not counting the constant term of the learned bases).
        planes: usize,
        /// JPEG quality of the planes, when recorded.
        quality: Option<u32>,
        /// Number of light directions sampled by the learned bases.
        lights: usize,
    },
}

impl Container {
    /// Short format name for the info panel.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Ptm { .. } => "PTM",
            Self::Hsh { legacy: false, .. } => "RTI (HSH)",
            Self::Hsh { legacy: true, .. } => "HSH",
            Self::Relight { .. } => "Relight",
        }
    }
}

/// How the light direction weights the coefficients.
#[derive(Clone, Debug, PartialEq)]
pub enum Basis {
    /// Biquadratic polynomial in the light's x and y. `relight_order` is Relight's term order
    /// `1, x, y, x², xy, y²`; otherwise the PTM order `x², y², xy, x, y, 1`.
    Ptm { relight_order: bool },
    /// Hemispherical harmonics with 4, 9 or 16 terms.
    Hsh { terms: usize },
    /// Real spherical harmonics with up to 9 terms.
    Sh { terms: usize },
    /// Radial basis functions over sampled lights (Relight). `basis` holds, for each
    /// coefficient plane plus the mean (`planes + 1`), each sampled light and each output
    /// channel, the weight of that plane, already unpacked from bytes.
    Rbf {
        lights: Vec<[f32; 3]>,
        sigma: f32,
        basis: Vec<f32>,
    },
    /// Bilinear interpolation over an octahedral grid of `resolution²` light directions
    /// (Relight). `basis` is laid out like [`Basis::Rbf`]'s with grid cells for lights.
    Bilinear { resolution: usize, basis: Vec<f32> },
}

impl Basis {
    /// Short name for the info panel.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Ptm { .. } => "PTM",
            Self::Hsh { .. } => "HSH",
            Self::Sh { .. } => "SH",
            Self::Rbf { .. } => "RBF",
            Self::Bilinear { .. } => "Bilinear",
        }
    }

    /// Number of basis terms (sampled lights for the learned bases).
    pub fn terms(&self) -> usize {
        match self {
            Self::Ptm { .. } => 6,
            Self::Hsh { terms } | Self::Sh { terms } => *terms,
            Self::Rbf { lights, .. } => lights.len(),
            Self::Bilinear { resolution, .. } => resolution * resolution,
        }
    }

    /// The polynomial order of the analytic bases (2 for PTM, √terms for HSH and SH).
    pub fn order(&self) -> Option<usize> {
        match self {
            Self::Ptm { .. } => Some(2),
            Self::Hsh { terms } | Self::Sh { terms } => {
                Some((*terms as f64).sqrt().round() as usize)
            }
            _ => None,
        }
    }
}

/// How the coefficient layers combine into a colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Colour {
    /// Layer `t` holds term `t` for red, green and blue.
    Rgb,
    /// Layer 0 is the colour, layers 1.. hold the luminance terms, three per layer; the
    /// result is the colour times the luminance.
    Lrgb,
    /// Each plane contributes to all three channels with its own weight per channel
    /// (Relight's learned bases).
    Mrgb,
    /// Like `Mrgb` in a luma/chroma space: the first `chroma` layers hold one luma and two
    /// chroma planes, the rest luma only.
    Mycc { chroma: usize },
}

impl Colour {
    pub fn name(self) -> &'static str {
        match self {
            Self::Rgb => "RGB",
            Self::Lrgb => "LRGB",
            Self::Mrgb => "MRGB",
            Self::Mycc { .. } => "MYCC",
        }
    }
}

/// Three coefficient planes, interleaved, with their byte-to-value mapping:
/// `value = byte · scale[c] + offset[c]`.
#[derive(Clone, Debug, PartialEq)]
pub struct Layer {
    /// `width · height · 3` bytes, rows top-down.
    pub data: Vec<u8>,
    pub scale: [f32; 3],
    pub offset: [f32; 3],
}

/// A reflectance transformation image.
#[derive(Clone, Debug, PartialEq)]
pub struct Rti {
    pub width: u32,
    pub height: u32,
    pub container: Container,
    pub basis: Basis,
    pub colour: Colour,
    /// Number of coefficient planes in use: `3 · terms` for RGB, `3 + terms` for LRGB, the
    /// file's plane count for MRGB and MYCC. Planes past it in the last layer are zero.
    pub planes: usize,
    pub layers: Vec<Layer>,
}

impl Rti {
    /// Bytes of the coefficient layers (what a holder of the image keeps reserved).
    pub fn layer_bytes(&self) -> u64 {
        self.layers.iter().map(|l| l.data.len() as u64).sum()
    }

    /// Reads a PTM or HSH file from its bytes. Relight folders need [`Rti::relight`].
    pub fn read(data: &[u8]) -> Result<Rti> {
        if ptm::probe(data) {
            ptm::read(data)
        } else if hsh::probe(data) {
            hsh::read(data)
        } else {
            Err(Error::NotRti)
        }
    }

    /// Reads a Relight `info.json`, loading the plane images through `load` (given the file
    /// name, e.g. `plane_0.jpg`, relative to the folder).
    pub fn relight(info: &[u8], load: &mut dyn FnMut(&str) -> Result<Vec<u8>>) -> Result<Rti> {
        relight::read(info, load)
    }

    /// True when `head` (the start of a file) looks like a PTM or HSH file.
    pub fn probe(head: &[u8]) -> bool {
        ptm::probe(head) || hsh::probe(head)
    }

    /// Bytes held.
    pub fn memory_bytes(&self) -> usize {
        self.layers.iter().map(|l| l.data.len()).sum::<usize>()
            + match &self.basis {
                Basis::Rbf { basis, lights, .. } => basis.len() * 4 + lights.len() * 12,
                Basis::Bilinear { basis, .. } => basis.len() * 4,
                _ => 0,
            }
    }

    /// The coefficient values of layer `layer` at `(x, y)`.
    pub fn coef(&self, layer: usize, x: u32, y: u32) -> [f32; 3] {
        let l = &self.layers[layer];
        let i = (y as usize * self.width as usize + x as usize) * 3;
        std::array::from_fn(|c| l.data[i + c] as f32 * l.scale[c] + l.offset[c])
    }

    /// Where coefficient plane `p`, in the file format's own order, is held: its layer and
    /// channel. PTM LRGB: `a0..a5`, then red, green and blue; PTM RGB: `a0..a5` of red, of
    /// green, of blue; HSH: term `k` of channel `c` is `3k + c`; Relight: channel `p % 3` of
    /// `plane_{p / 3}.jpg`.
    pub fn plane_location(&self, p: usize) -> Option<(usize, usize)> {
        if p >= self.planes {
            return None;
        }
        Some(match (&self.container, self.colour) {
            (Container::Ptm { .. }, Colour::Lrgb) if p < 6 => (1 + p / 3, p % 3),
            (Container::Ptm { .. }, Colour::Lrgb) => (0, p - 6),
            (Container::Ptm { .. }, _) => (p % 6, p / 6),
            _ => (p / 3, p % 3),
        })
    }

    /// A copy box-filtered by an integer factor (rounded averages of the bytes; relighting
    /// is linear in the coefficients, so this is the relit image downscaled, except for the
    /// LRGB colour × luminance product). Sizes round up.
    pub fn downscaled(&self, factor: u32) -> Rti {
        if factor <= 1 {
            return self.clone();
        }
        let (w, h) = (self.width as usize, self.height as usize);
        let f = factor as usize;
        let (nw, nh) = (w.div_ceil(f), h.div_ceil(f));
        let layers = self
            .layers
            .iter()
            .map(|l| {
                let mut data = vec![0u8; nw * nh * 3];
                for oy in 0..nh {
                    let y0 = oy * f;
                    let y1 = (y0 + f).min(h);
                    for ox in 0..nw {
                        let x0 = ox * f;
                        let x1 = (x0 + f).min(w);
                        let mut sum = [0u32; 3];
                        for y in y0..y1 {
                            let row = &l.data[(y * w + x0) * 3..(y * w + x1) * 3];
                            for px in row.chunks_exact(3) {
                                for c in 0..3 {
                                    sum[c] += px[c] as u32;
                                }
                            }
                        }
                        let n = ((y1 - y0) * (x1 - x0)) as u32;
                        for c in 0..3 {
                            data[(oy * nw + ox) * 3 + c] = ((sum[c] + n / 2) / n) as u8;
                        }
                    }
                }
                Layer {
                    data,
                    scale: l.scale,
                    offset: l.offset,
                }
            })
            .collect();
        Rti {
            width: nw as u32,
            height: nh as u32,
            container: self.container.clone(),
            basis: self.basis.clone(),
            colour: self.colour,
            planes: self.planes,
            layers,
        }
    }
}

/// Checks a size before allocating for it.
/// Checks a JPEG plane's header before decoding it: its size must be the image's (decoding
/// first let a 1 KB stream claim 65535×65535), and a large plane needs at least a byte per
/// 1024 pixels (a flat JPEG plane takes about one per 85).
pub(crate) fn jpeg_fits(stream: &[u8], width: u32, height: u32) -> Result<()> {
    let info = specular_jpeg::read_info(stream).map_err(|e| Error::Jpeg(e.to_string()))?;
    if info.width != width || info.height != height {
        return Err(Error::Malformed(format!(
            "a JPEG plane is {}×{}, the image {width}×{height}",
            info.width, info.height
        )));
    }
    let px = u64::from(width) * u64::from(height);
    if px > 1 << 24 && (stream.len() as u64).saturating_mul(1024) < px {
        return Err(Error::Truncated("JPEG plane"));
    }
    Ok(())
}

/// Reserves `layers` coefficient layers of `width` × `height` from the memory budget before
/// they are allocated (up to 32 layers of a gigapixel each from a header alone). The holder of
/// the image reserves them again for as long as it keeps them ([`Rti::layer_bytes`]).
pub(crate) fn reserve_layers(
    layers: usize,
    width: u32,
    height: u32,
) -> Result<specular_core::Reservation> {
    Ok(specular_core::budget::reserve_n(
        layers as u64,
        u64::from(width) * u64::from(height) * 3,
    )?)
}

pub(crate) fn check_size(width: u32, height: u32) -> Result<()> {
    if width == 0 || height == 0 {
        return Err(Error::Malformed("RTI: zero size".into()));
    }
    if width > MAX_SIDE || height > MAX_SIDE || width as u64 * height as u64 > 1 << 30 {
        return Err(Error::TooLarge(width, height));
    }
    Ok(())
}

/// An empty layer of the given size with an identity mapping.
pub(crate) fn layer(width: u32, height: u32, scale: [f32; 3], offset: [f32; 3]) -> Layer {
    Layer {
        data: vec![0; width as usize * height as usize * 3],
        scale,
        offset,
    }
}
