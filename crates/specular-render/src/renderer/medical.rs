//! Medical images: modality values on the GPU and the VOI window applied in the image shader
//! (`shaders/medical.wgsl`), so window/level changes redraw without a decode or an upload.
//!
//! The values live in an R32Float texture with box-filtered mips. f32 holds every integer up
//! to 2^24 exactly, so the modality values of 8- to 16-bit stored samples under an integer
//! rescale (CT: slope 1, intercept −1024) are exact; a fractional slope, or 32-bit samples,
//! keep about 7 significant digits (relative error ≤ 6e-8). The window itself is evaluated in
//! f32: its edges are exact for integer data and half-integer edges, and the ramp between
//! them is within 1e-6 of the f64 reference (`specular_dicom::Voi::apply`), far below the
//! 1/65535 of the 16-bit CPU raster. Images wider or taller than the GPU's texture limit
//! keep a box-downscaled copy (window/level works, at the reduced resolution).

use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use specular_core::nd::{self, Stretch};
use specular_gfx::BufferInit;
use specular_gfx::wgpu::util::DeviceExt;
use specular_gfx::{FULLSCREEN_WGSL, Gpu, HDR, fullscreen_pipeline, wgpu};

use super::{
    FILE_LUT_MAP, GpuImage, ImageRenderer, RenderOptions, inspect_rows, storage_entry, tex_entry,
    texture_2d, texture_bytes, uniform_entry,
};
use crate::view::View;

/// The VOI LUT function (DICOM PS3.3 C.11.2.1.3, VOI LUT Function).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum VoiShape {
    /// LINEAR: the window's edges at c − 0.5 ± (w − 1) / 2 (C.11.2.1.2.1).
    #[default]
    Linear,
    /// LINEAR_EXACT: the edges at c ± w / 2 (C.11.2.1.3.2).
    LinearExact,
    /// SIGMOID: 1 / (1 + exp(−4 (x − c) / w)) (C.11.2.1.3.1).
    Sigmoid,
}

/// A window over the modality values.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Window {
    pub center: f64,
    pub width: f64,
    pub shape: VoiShape,
    /// Grey inverted after the window (MONOCHROME1, or the user's invert).
    pub invert: bool,
}

impl Window {
    /// The window's output for modality value `x` in 0..=1 (before inversion), in f64: the
    /// reference the shader is tested against.
    pub fn apply(&self, x: f64) -> f64 {
        let (c, w) = (self.center, self.width);
        match self.shape {
            VoiShape::Sigmoid => 1.0 / (1.0 + (-4.0 * (x - c) / w.max(f64::MIN_POSITIVE)).exp()),
            VoiShape::Linear | VoiShape::LinearExact => {
                let (c, w) = if self.shape == VoiShape::Linear {
                    (c - 0.5, (w - 1.0).max(0.0))
                } else {
                    (c, w.max(0.0))
                };
                if x <= c - w / 2.0 {
                    0.0
                } else if x > c + w / 2.0 || w == 0.0 {
                    1.0
                } else {
                    ((x - c) / w + 0.5).clamp(0.0, 1.0)
                }
            }
        }
    }

    /// The displayed grey for `x` (inversion included), 0..=1.
    pub fn grey(&self, x: f64) -> f64 {
        let g = self.apply(x);
        if self.invert { 1.0 - g } else { g }
    }

    /// Uniform rows: `voi` and `mode` of `shaders/medical.wgsl` (also read by the volume
    /// renderer, `specular-render-volume`).
    pub fn rows(&self) -> ([f32; 4], [f32; 4]) {
        let (c, w) = (self.center, self.width);
        let voi = match self.shape {
            VoiShape::Sigmoid => [c as f32, (1.0 / w.max(f64::MIN_POSITIVE)) as f32, 0.0, 0.0],
            VoiShape::Linear | VoiShape::LinearExact => {
                let (c, w) = if self.shape == VoiShape::Linear {
                    (c - 0.5, (w - 1.0).max(0.0))
                } else {
                    (c, w.max(0.0))
                };
                let inv = if w > 0.0 { 1.0 / w } else { 0.0 };
                [
                    c as f32,
                    inv as f32,
                    (c - w / 2.0) as f32,
                    (c + w / 2.0) as f32,
                ]
            }
        };
        let mode = [
            (self.shape == VoiShape::Sigmoid) as u32 as f32,
            self.invert as u32 as f32,
            0.0,
            0.0,
        ];
        (voi, mode)
    }
}

/// An image's modality values on the GPU.
#[derive(Debug)]
pub struct Values {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    /// Texture size (the stored size unless that exceeds the GPU's limit).
    pub width: u32,
    pub height: u32,
    mips: u32,
    /// The window drawn; `None` draws the image's ordinary (CPU-rendered) pixels.
    window: Option<Window>,
    /// The frame's display shutters and overlay planes (R8Unorm, bit 0 shutter, bits 1..=4
    /// the overlay's ink), at the values' size.
    mask: Option<Mask>,
    /// Overlay planes drawn (the user's toggle; shutters always are).
    overlays: bool,
    /// Value channels (1: R32Float; 2 to [`MAX_CHANNELS`]: Rgba32Float, four channels per
    /// array layer, science images).
    channels: u32,
    /// The science look drawn (takes precedence over `window`), with its equalisation
    /// tables when the stretch needs them.
    science: Option<(ScienceLook, Option<Lut>)>,
    /// The modality values of the smallest and largest stored sample the bit depth allows
    /// (HiLo marks values at or beyond them).
    limits: Option<[f32; 2]>,
}

/// Most channels a science image composites at once (four per layer of the values' array
/// texture).
pub const MAX_CHANNELS: usize = 16;

/// Array layers of four channels the values of `channels` channels take.
fn layers_of(channels: u32) -> u32 {
    channels.div_ceil(4).max(1)
}

/// How a channel of a science image shows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChannelLook {
    /// Values at or below `lo` show black, at or above `hi` full colour.
    pub lo: f32,
    pub hi: f32,
    /// Its colour, display-coded 0..=1 (white for a single grey channel).
    pub colour: [f32; 3],
    pub on: bool,
    /// The midtones balance of [`Stretch::Auto`] (0.5 leaves values as they are).
    pub mid: f32,
}

/// How a science image (FITS, XISF) shows: per-channel limits and colours, and the stretch,
/// all drawn on the GPU from the real values ([`ImageRenderer::set_science`]).
#[derive(Clone, Debug, PartialEq)]
pub struct ScienceLook {
    pub stretch: Stretch,
    pub channels: Vec<ChannelLook>,
    /// For [`Stretch::HistEq`]: each channel's table ([`specular_core::nd::histeq_lut`]),
    /// all of one length.
    pub luts: Vec<Vec<f32>>,
    /// The file's colour table (an ImageJ LUT, display-coded sRGB) a single channel shows
    /// through after its stretch, unless false colour is on.
    pub colour_table: Option<Arc<[[u8; 3]; 256]>>,
    /// Relief shading of a single-band elevation model.
    pub hillshade: Option<Hillshade>,
    /// A single channel's values past a level painted in one colour (a thermal isotherm).
    pub isotherm: Option<Isotherm>,
}

/// Values at or above (`above`) or at or below a `level`, in the values' own unit, painted
/// in `colour` (display-coded 0..=1) over the stretched colours of a single channel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Isotherm {
    pub above: bool,
    pub level: f32,
    pub colour: [f32; 3],
}

impl Isotherm {
    /// Whether value `x` is marked.
    pub fn marks(&self, x: f32) -> bool {
        if self.above {
            x >= self.level
        } else {
            x <= self.level
        }
    }
}

/// Relief shading of an elevation model, as cartographic hillshades draw it: the slope of
/// each value from its 3×3 neighbours (Horn's weights), lit by a sun `azimuth` degrees
/// clockwise from north and `altitude` degrees above the horizon. Without false colour the
/// shade shows alone (grey); with it, it darkens the map's colours.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hillshade {
    pub azimuth: f32,
    pub altitude: f32,
    /// The ground distance from one stored pixel to the next, in the values' unit (metres
    /// for an elevation in metres): east along a row (x), and north down a column (y,
    /// negative when rows run south, as in a north-up map).
    pub step: [f32; 2],
    /// Vertical exaggeration.
    pub z: f32,
    /// What the relief shows: the shade, or the slope or aspect in colour.
    pub kind: Relief,
}

/// What a relief view shows from the 3×3 gradient.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Relief {
    /// Lit by the sun (grey, or darkening false colour).
    #[default]
    Shade,
    /// The slope, 0° to [`SLOPE_MAX`] and steeper, through [`SLOPE_MAP`] reversed: gentle
    /// ground light, steep ground dark.
    Slope,
    /// The direction the slope faces, clockwise from north, through the cyclic
    /// [`ASPECT_MAP`]; flat ground in [`FLAT_GREY`].
    Aspect,
}

/// Slopes at or above this (degrees) take the slope map's last colour.
pub const SLOPE_MAX: f32 = 60.0;
/// The slope's colour map, read from its light end: pale on gentle ground, red, then dark
/// on steep.
pub const SLOPE_MAP: specular_inspect::ColourMap = specular_inspect::ColourMap::Lajolla;
/// The aspect's colour map (cyclic, so north meets north).
pub const ASPECT_MAP: specular_inspect::ColourMap = specular_inspect::ColourMap::Phase;
/// Flat ground in an aspect view (display-coded grey).
pub const FLAT_GREY: f32 = 0.6;

impl Default for Hillshade {
    /// The cartographic convention: the sun in the north-west, 45° up.
    fn default() -> Self {
        Hillshade {
            azimuth: 315.0,
            altitude: 45.0,
            step: [1.0, -1.0],
            z: 1.0,
            kind: Relief::Shade,
        }
    }
}

impl Hillshade {
    /// The direction towards the sun: east, north, up.
    pub fn sun(&self) -> [f32; 3] {
        let (az, alt) = (self.azimuth.to_radians(), self.altitude.to_radians());
        [az.sin() * alt.cos(), az.cos() * alt.cos(), alt.sin()]
    }

    /// The slope factors of the shader: 1 / (8 × step) × exaggeration, per axis.
    pub fn factors(&self) -> [f32; 2] {
        self.step.map(|d| {
            if d != 0.0 && d.is_finite() {
                self.z / (8.0 * d)
            } else {
                0.0
            }
        })
    }

    /// The shade 0..=1 of stored pixel (`x`, `y`) of `values` (`w`×`h`, row-major): the CPU
    /// reference of `shaders/medical.wgsl`'s hillshade. Neighbours past the edges are the
    /// edge's own; neighbours without data take the centre's value; `None` where the pixel
    /// itself has none.
    pub fn shade(&self, values: &[f32], w: usize, h: usize, x: usize, y: usize) -> Option<f32> {
        let (gx, gy) = self.gradient(values, w, h, x, y)?;
        let n = [-gx, -gy, 1.0];
        let len = (n[0] * n[0] + n[1] * n[1] + 1.0).sqrt();
        let l = self.sun();
        Some(((n[0] * l[0] + n[1] * l[1] + n[2] * l[2]) / len).max(0.0))
    }

    /// The colour (display-coded) of stored pixel (`x`, `y`) in a slope or aspect view, the
    /// CPU reference of the shader's: the slope in degrees over [`SLOPE_MAX`] through
    /// [`SLOPE_MAP`] (reversed), or the aspect over 360° through [`ASPECT_MAP`] ([`FLAT_GREY`] where
    /// flat). The shade's grey for [`Relief::Shade`]. `None` without data.
    pub fn colour(
        &self,
        values: &[f32],
        w: usize,
        h: usize,
        x: usize,
        y: usize,
    ) -> Option<[f32; 3]> {
        match self.kind {
            Relief::Shade => self.shade(values, w, h, x, y).map(|s| [s; 3]),
            Relief::Slope => {
                let (gx, gy) = self.gradient(values, w, h, x, y)?;
                let deg = gx.hypot(gy).atan().to_degrees();
                Some(SLOPE_MAP.sample(1.0 - deg / SLOPE_MAX))
            }
            Relief::Aspect => {
                let (gx, gy) = self.gradient(values, w, h, x, y)?;
                if gx == 0.0 && gy == 0.0 {
                    return Some([FLAT_GREY; 3]);
                }
                let a = (-gx).atan2(-gy).to_degrees().rem_euclid(360.0);
                Some(ASPECT_MAP.sample(a / 360.0))
            }
        }
    }

    /// The gradient (height per ground unit east, north) at stored pixel (`x`, `y`) from
    /// Horn's 3×3 weights, as the shader computes it.
    fn gradient(
        &self,
        values: &[f32],
        w: usize,
        h: usize,
        x: usize,
        y: usize,
    ) -> Option<(f32, f32)> {
        let c = values[y * w + x];
        if !c.is_finite() {
            return None;
        }
        let at = |dx: isize, dy: isize| {
            let xi = (x as isize + dx).clamp(0, w as isize - 1) as usize;
            let yi = (y as isize + dy).clamp(0, h as isize - 1) as usize;
            let v = values[yi * w + xi];
            if v.is_finite() { v } else { c }
        };
        let [fx, fy] = self.factors();
        let gx = ((at(1, -1) + 2.0 * at(1, 0) + at(1, 1))
            - (at(-1, -1) + 2.0 * at(-1, 0) + at(-1, 1)))
            * fx;
        let gy = ((at(-1, 1) + 2.0 * at(0, 1) + at(1, 1))
            - (at(-1, -1) + 2.0 * at(0, -1) + at(1, -1)))
            * fy;
        Some((gx, gy))
    }
}

impl ScienceLook {
    /// The displayed colour (display-coded, before the scene's linearisation) of values `v`
    /// (one per channel): the CPU reference of `shaders/medical.wgsl`'s science path.
    pub fn colour(&self, v: &[f32]) -> [f32; 3] {
        if let (Some(i), [c], Some(&x)) = (&self.isotherm, self.channels.as_slice(), v.first())
            && c.on
            && i.marks(x)
        {
            return i.colour;
        }
        if let (Some(t), [c], Some(&x)) = (&self.colour_table, self.channels.as_slice(), v.first())
            && c.on
        {
            let inv = if c.hi > c.lo {
                1.0 / (c.hi - c.lo)
            } else {
                0.0
            };
            let lut = self.luts.first().map_or(&[][..], |l| l.as_slice());
            return table_at(t, nd::apply_mid(self.stretch, (x - c.lo) * inv, lut, c.mid));
        }
        let mut rgb = [0.0f32; 3];
        for (k, (c, &x)) in self.channels.iter().zip(v).enumerate() {
            if !c.on {
                continue;
            }
            let inv = if c.hi > c.lo {
                1.0 / (c.hi - c.lo)
            } else {
                0.0
            };
            let lut = self.luts.get(k).map_or(&[][..], |l| l.as_slice());
            let y = nd::apply_mid(self.stretch, (x - c.lo) * inv, lut, c.mid);
            for (o, w) in rgb.iter_mut().zip(c.colour) {
                *o += w * y;
            }
        }
        rgb.map(|x| x.min(1.0))
    }
}

/// A colour table at `t` (0..1), display-coded: neighbouring entries interpolated, as the
/// shader's `cmap`.
fn table_at(t: &[[u8; 3]; 256], x: f32) -> [f32; 3] {
    let x = x.clamp(0.0, 1.0) * 255.0;
    let i = (x.floor() as usize).min(254);
    let f = x - i as f32;
    let (a, b) = (t[i], t[i + 1]);
    [0, 1, 2].map(|k| {
        let (a, b) = (f32::from(a[k]) / 255.0, f32::from(b[k]) / 255.0);
        a + (b - a) * f
    })
}

#[derive(Debug)]
struct Lut {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
}

/// Overlay inks a mask can index (bits 1..=4, 0 for none).
pub const MAX_INKS: usize = 15;

/// A frame's display shutters and overlays for [`ImageRenderer::set_graphics`].
#[derive(Clone, Copy, Debug)]
pub struct ValueMask<'a> {
    /// At the stored size, row-major: bit 0 shuttered, bits 1..=4 the ink of the overlay
    /// shown there (1-based into `inks`; 0 none).
    pub mask: &'a [u8],
    /// The shutter's colour, display-coded (as the window's output) 0..=1.
    pub shutter: [f32; 3],
    /// Overlay inks, display-coded 0..=1; past [`MAX_INKS`] ignored.
    pub inks: &'a [[f32; 3]],
}

#[derive(Debug)]
struct Mask {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    /// The shutter's colour (rgb, 1) and the inks (rgb, 1), display-coded.
    shutter: [f32; 4],
    inks: [[f32; 4]; MAX_INKS],
}

impl Values {
    pub fn bytes(&self) -> u64 {
        texture_bytes(&self.texture)
            + self.mask.as_ref().map_or(0, |m| texture_bytes(&m.texture))
            + self
                .science
                .as_ref()
                .and_then(|s| s.1.as_ref())
                .map_or(0, |l| texture_bytes(&l.texture))
    }

    pub(super) fn into_textures(self) -> impl Iterator<Item = wgpu::Texture> {
        std::iter::once(self.texture)
            .chain(self.mask.map(|m| m.texture))
            .chain(self.science.and_then(|s| s.1).map(|l| l.texture))
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct MedUniform {
    m0: [f32; 4],
    m1: [f32; 4],
    img: [f32; 4],
    tex: [f32; 4],
    canvas: [f32; 4],
    grid: [f32; 4],
    misc: [f32; 4],
    clip: [f32; 4],
    mask: [f32; 4],
    voi: [f32; 4],
    mode: [f32; 4],
    shutter: [f32; 4],
    inks: [[f32; 4]; MAX_INKS],
    sci: [f32; 4],
    /// Per channel (four per vector): the lower limit and 1 / (upper − lower).
    lo: [[f32; 4]; MAX_CHANNELS / 4],
    inv: [[f32; 4]; MAX_CHANNELS / 4],
    cols: [[f32; 4]; MAX_CHANNELS],
    /// False colour on, map index, HiLo on, unused.
    fc: [f32; 4],
    /// HiLo thresholds: high, low (modality values).
    hl: [f32; 4],
    /// HiLo colours, display-coded.
    hilo_hi: [f32; 4],
    hilo_lo: [f32; 4],
    /// Per channel (four per vector), the midtones balance of the automatic stretch.
    mid: [[f32; 4]; MAX_CHANNELS / 4],
    /// Relief: 0 off, 1 hillshade, 2 slope, 3 aspect; then the direction towards the sun
    /// (east, north, up).
    hs: [f32; 4],
    /// Relief slope factors per stored pixel (x, y), the slope and aspect maps.
    hsr: [f32; 4],
    /// Isotherm on, above (1) or below (0), the level, unused.
    iso: [f32; 4],
    /// The isotherm's colour, display-coded.
    iso_col: [f32; 4],
    /// GSDF display: JND indices of black and white, black in cd/m², 1 / (white − black);
    /// all 0 for sRGB greys.
    gsdf: [f32; 4],
}

/// The medical pipelines, built on first use.
pub(super) struct Pipes {
    draw: wgpu::RenderPipeline,
    draw_bgl: wgpu::BindGroupLayout,
    mip: wgpu::RenderPipeline,
    /// Mips of multi-channel (Rgba32Float) values.
    mip4: wgpu::RenderPipeline,
    mip_bgl: wgpu::BindGroupLayout,
    /// Bilinear demosaic of a raw colour filter array frame into Rgba32Float values.
    debayer: wgpu::RenderPipeline,
    debayer_bgl: wgpu::BindGroupLayout,
    /// Bound when an image has no mask.
    no_mask: wgpu::TextureView,
    /// Bound when an image has no equalisation table.
    no_lut: wgpu::TextureView,
}

impl Pipes {
    fn new(gpu: &Gpu) -> Pipes {
        let d = &gpu.device;
        let module = specular_gfx::shader(
            d,
            "medical",
            &format!(
                "{FULLSCREEN_WGSL}\n{}",
                include_str!("../shaders/medical.wgsl")
            ),
        );
        use wgpu::TextureViewDimension as Dim;
        let draw_bgl = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("medical"),
            entries: &[
                uniform_entry(0),
                tex_entry(1, Dim::D2Array, false),
                tex_entry(2, Dim::D2, false),
                tex_entry(3, Dim::D2, false),
                storage_entry(4),
            ],
        });
        let mip_bgl = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("values mip"),
            entries: &[tex_entry(1, Dim::D2Array, false)],
        });
        let draw = fullscreen_pipeline(
            gpu,
            "medical",
            &module,
            "fs_medical",
            &[Some(&draw_bgl)],
            HDR,
            None,
        );
        let mip = fullscreen_pipeline(
            gpu,
            "values mip",
            &module,
            "fs_values_mip",
            &[Some(&mip_bgl)],
            VALUES,
            None,
        );
        let mip4 = fullscreen_pipeline(
            gpu,
            "values mip 4",
            &module,
            "fs_values_mip",
            &[Some(&mip_bgl)],
            VALUES4,
            None,
        );
        let debayer_module = specular_gfx::shader(
            d,
            "debayer",
            &format!(
                "{FULLSCREEN_WGSL}\n{}",
                include_str!("../shaders/debayer.wgsl")
            ),
        );
        let debayer_bgl = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("debayer"),
            entries: &[uniform_entry(0), tex_entry(1, Dim::D2, false)],
        });
        let debayer = fullscreen_pipeline(
            gpu,
            "debayer",
            &debayer_module,
            "fs_debayer",
            &[Some(&debayer_bgl)],
            VALUES4,
            None,
        );
        let one = |label, format, data: &[u8]| {
            d.create_texture_with_data(
                &gpu.queue,
                &wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d {
                        width: 1,
                        height: 1,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                },
                wgpu::util::TextureDataOrder::LayerMajor,
                data,
            )
            .create_view(&Default::default())
        };
        let no_mask = one("no mask", MASK, &[0]);
        let no_lut = one("no lut", VALUES, &[0; 4]);
        Pipes {
            draw,
            draw_bgl,
            mip,
            mip4,
            mip_bgl,
            debayer,
            debayer_bgl,
            no_mask,
            no_lut,
        }
    }
}

const VALUES: wgpu::TextureFormat = wgpu::TextureFormat::R32Float;
const VALUES4: wgpu::TextureFormat = wgpu::TextureFormat::Rgba32Float;
const MASK: wgpu::TextureFormat = wgpu::TextureFormat::R8Unorm;

/// Downscales a mask (`w`×`h`) by `step`: a shutter bit anywhere in a box keeps it, and so
/// does an overlay (thin overlay lines stay visible), the box taking the highest ink in it
/// (inks are numbered in layer order, so mostly the one on top).
fn downscale_mask(mask: &[u8], w: u32, h: u32, step: u32) -> Vec<u8> {
    const INK: u8 = 0b1_1110;
    let (tw, th) = (w.div_ceil(step), h.div_ceil(step));
    let mut out = vec![0u8; tw as usize * th as usize];
    for y in 0..h {
        let row = &mask[y as usize * w as usize..][..w as usize];
        let o = &mut out[(y / step) as usize * tw as usize..][..tw as usize];
        for (x, &b) in row.iter().enumerate() {
            let m = &mut o[x / step as usize];
            *m = (*m & 1 | b & 1) | (*m & INK).max(b & INK);
        }
    }
    out
}

/// Box-downscales `data` (`w`×`h`) by an integer `step` (partial boxes at the edges average
/// what they cover; non-finite values are left out, and a box of none is NaN).
fn downscale(data: &[f32], w: u32, h: u32, step: u32) -> (Vec<f32>, u32, u32) {
    let (tw, th) = (w.div_ceil(step), h.div_ceil(step));
    let mut out = vec![0.0f32; tw as usize * th as usize];
    for ty in 0..th {
        for tx in 0..tw {
            let mut sum = 0.0f64;
            let mut n = 0u32;
            for y in ty * step..((ty + 1) * step).min(h) {
                let row = &data[y as usize * w as usize..][..w as usize];
                for &v in &row[(tx * step) as usize..((tx + 1) * step).min(w) as usize] {
                    if v.is_finite() {
                        sum += f64::from(v);
                        n += 1;
                    }
                }
            }
            out[(ty * tw + tx) as usize] = if n == 0 {
                f32::NAN
            } else {
                (sum / f64::from(n)) as f32
            };
        }
    }
    (out, tw, th)
}

impl GpuImage {
    /// The modality values are on the GPU.
    pub fn has_values(&self) -> bool {
        self.values.is_some()
    }

    /// The window drawn over the values, if any.
    pub fn window(&self) -> Option<Window> {
        self.values.as_ref().and_then(|v| v.window)
    }

    /// Draws the values through `window` (`None`: the ordinary pixels again). Bumps the
    /// revision when it changes, so the scene, navigator and loupe redraw. Without values
    /// this does nothing and returns false.
    pub fn set_window(&mut self, window: Option<Window>) -> bool {
        let Some(v) = self.values.as_mut() else {
            return false;
        };
        if v.window != window {
            v.window = window;
            self.revision += 1;
        }
        true
    }

    /// Shows or hides the overlay planes (shutters stay). Bumps the revision on a change.
    pub fn set_overlays(&mut self, on: bool) {
        if let Some(v) = self.values.as_mut()
            && v.overlays != on
        {
            v.overlays = on;
            self.revision += 1;
        }
    }

    /// Sets the modality values of the bit depth's smallest and largest stored sample
    /// (`[low, high]`, e.g. from BitsStored through the modality LUT) for HiLo. Bumps the
    /// revision on a change.
    pub fn set_value_limits(&mut self, limits: Option<[f32; 2]>) {
        if let Some(v) = self.values.as_mut()
            && v.limits != limits
        {
            v.limits = limits;
            self.revision += 1;
        }
    }

    /// The limits [`GpuImage::set_value_limits`] set.
    pub fn value_limits(&self) -> Option<[f32; 2]> {
        self.values.as_ref().and_then(|v| v.limits)
    }

    /// The values carry a shutter / overlay mask.
    pub fn has_graphics(&self) -> bool {
        self.values.as_ref().is_some_and(|v| v.mask.is_some())
    }

    /// Whether drawing goes through the medical shader.
    pub(super) fn draws_values(&self) -> bool {
        self.values
            .as_ref()
            .is_some_and(|v| v.window.is_some() || v.science.is_some())
    }

    /// Number of value channels on the GPU (0 without values).
    pub fn value_channels(&self) -> u32 {
        self.values.as_ref().map_or(0, |v| v.channels)
    }

    /// The science look drawn, if any.
    pub fn science(&self) -> Option<&ScienceLook> {
        self.values
            .as_ref()
            .and_then(|v| v.science.as_ref())
            .map(|s| &s.0)
    }
}

impl ImageRenderer {
    fn medical_pipes(&mut self, gpu: &Gpu) -> &Pipes {
        self.medical.get_or_insert_with(|| Pipes::new(gpu))
    }

    /// Uploads an image's modality values (`width`×`height` = its stored size, row-major)
    /// and builds their mips. The window is kept if the image had values already.
    pub fn set_values(&mut self, gpu: &Gpu, img: &mut GpuImage, data: &[f32]) {
        let (w, h) = (img.width, img.height);
        assert_eq!(data.len(), w as usize * h as usize, "values size");
        let step = w.max(h).div_ceil(self.max_tex).max(1);
        let small;
        let (data, tw, th) = if step > 1 {
            small = downscale(data, w, h, step);
            (small.0.as_slice(), small.1, small.2)
        } else {
            (data, w, h)
        };
        self.upload_values(gpu, img, data, tw, th, 1);
    }

    /// Uploads the planes of a science image (1 to [`MAX_CHANNELS`], each `width`×`height`
    /// = the image's stored size, row-major) as its values, for [`ImageRenderer::set_science`].
    /// Undefined values (NaN, ±∞) are stored as the plane's lowest finite value, so they show
    /// at the bottom of the stretch (black) and do not spread through the mips. The look is
    /// kept if the image had values already.
    pub fn set_value_channels(&mut self, gpu: &Gpu, img: &mut GpuImage, planes: &[&[f32]]) {
        let (w, h) = (img.width, img.height);
        let planes = &planes[..planes.len().min(MAX_CHANNELS)];
        assert!(!planes.is_empty(), "no planes");
        let step = w.max(h).div_ceil(self.max_tex).max(1);
        let (tw, th) = (w.div_ceil(step), h.div_ceil(step));
        // Non-finite values (no data) go up as they are: the shader leaves them out of its
        // filters and shows the canvas where a shown channel has none.
        let prepared: Vec<Vec<f32>> = planes
            .iter()
            .map(|p| {
                assert_eq!(p.len(), w as usize * h as usize, "values size");
                if step > 1 {
                    downscale(p, w, h, step).0
                } else {
                    p.to_vec()
                }
            })
            .collect();
        if prepared.len() == 1 {
            self.upload_values(gpu, img, &prepared[0], tw, th, 1);
        } else {
            // Four channels per array layer, layer after layer.
            let layer = tw as usize * th as usize * 4;
            let n = prepared.len() as u32;
            let mut rgba = vec![0.0f32; layer * layers_of(n) as usize];
            for (c, p) in prepared.iter().enumerate() {
                let l = &mut rgba[c / 4 * layer..][..layer];
                for (o, &v) in l.chunks_exact_mut(4).zip(p) {
                    o[c % 4] = v;
                }
            }
            self.upload_values(gpu, img, &rgba, tw, th, n);
        }
    }

    /// Uploads a one-shot colour camera's raw frame (`width`×`height` = the image's stored
    /// size, row-major) and demosaics it on the GPU (bilinear, [`nd::Cfa::debayer_at`]) into
    /// red, green and blue values, as [`ImageRenderer::set_value_channels`] would hold three
    /// planes. Undefined values are stored as the frame's lowest finite value first. A frame
    /// past the GPU's texture limit is demosaiced here and box-downscaled. The look is kept.
    pub fn set_value_cfa(&mut self, gpu: &Gpu, img: &mut GpuImage, raw: &[f32], cfa: nd::Cfa) {
        let (w, h) = (img.width, img.height);
        assert_eq!(raw.len(), w as usize * h as usize, "values size");
        let floor = raw
            .iter()
            .copied()
            .filter(|v| v.is_finite())
            .fold(f32::INFINITY, f32::min);
        let floor = if floor.is_finite() { floor } else { 0.0 };
        let clean: Vec<f32> = raw
            .iter()
            .map(|&v| if v.is_finite() { v } else { floor })
            .collect();
        if w.max(h) > self.max_tex {
            let (wu, hu) = (w as usize, h as usize);
            let mut planes = [
                vec![0.0f32; wu * hu],
                vec![0.0f32; wu * hu],
                vec![0.0f32; wu * hu],
            ];
            let [r, g, b] = &mut planes;
            for y in 0..hu {
                let o = y * wu;
                cfa.debayer_row(&clean, wu, hu, y, |x, [cr, cg, cb]| {
                    r[o + x] = cr;
                    g[o + x] = cg;
                    b[o + x] = cb;
                });
            }
            let refs: Vec<&[f32]> = planes.iter().map(Vec::as_slice).collect();
            self.set_value_channels(gpu, img, &refs);
            return;
        }
        let source = self.texture(
            &gpu.device,
            "raw cfa",
            w,
            h,
            1,
            VALUES,
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        );
        gpu.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &source,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            bytemuck::cast_slice(&clean),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(w * 4),
                rows_per_image: Some(h),
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        drop(clean);
        let texture = self.values_texture(gpu, w, h, VALUES4, 1);
        let mips = texture.mip_level_count();
        let pat = cfa.0.map(u32::from);
        let ub = gpu.device.buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("debayer"),
            contents: bytemuck::cast_slice(&pat),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let src_view = source.create_view(&Default::default());
        let dst_view = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2),
            base_mip_level: 0,
            mip_level_count: Some(1),
            ..Default::default()
        });
        let pipes = self.medical_pipes(gpu);
        let bind = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("debayer"),
            layout: &pipes.debayer_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: ub.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&src_view),
                },
            ],
        });
        let mut enc = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("debayer"),
            });
        {
            let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("debayer"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &dst_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            rp.set_pipeline(&pipes.debayer);
            rp.set_bind_group(0, &bind, &[]);
            rp.draw(0..3, 0..1);
        }
        drop((src_view, dst_view));
        self.finish_values(gpu, img, texture, (w, h, mips), 3, enc);
        self.recycle(source);
    }

    /// Uploads `channels` interleaved values per texel (1, or 4 slots per array layer for 2 to
    /// [`MAX_CHANNELS`] channels, layer after layer) at the texture size `tw`×`th`, and
    /// builds the mips.
    fn upload_values(
        &mut self,
        gpu: &Gpu,
        img: &mut GpuImage,
        data: &[f32],
        tw: u32,
        th: u32,
        channels: u32,
    ) {
        let (format, texel) = if channels > 1 {
            (VALUES4, 16)
        } else {
            (VALUES, 4)
        };
        let mips = 32 - tw.max(th).leading_zeros();
        let layers = if channels > 1 { layers_of(channels) } else { 1 };
        let texture = self.values_texture(gpu, tw, th, format, layers);
        gpu.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            bytemuck::cast_slice(data),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(tw * texel),
                rows_per_image: Some(th),
            },
            wgpu::Extent3d {
                width: tw,
                height: th,
                depth_or_array_layers: layers,
            },
        );
        let enc = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("values mips"),
            });
        self.finish_values(gpu, img, texture, (tw, th, mips), channels, enc);
    }

    /// A values texture with its mip chain (`layers` array layers; only single-layer ones
    /// come from and go back to the spares).
    fn values_texture(
        &mut self,
        gpu: &Gpu,
        tw: u32,
        th: u32,
        format: wgpu::TextureFormat,
        layers: u32,
    ) -> wgpu::Texture {
        let mips = 32 - tw.max(th).leading_zeros();
        let usage = wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::COPY_DST;
        if layers > 1 {
            return texture_2d(&gpu.device, "values", tw, th, layers, mips, format, usage);
        }
        self.texture(&gpu.device, "values", tw, th, mips, format, usage)
    }

    /// Builds the mips of `texture` (level 0 filled, or filled by the passes already in
    /// `enc`), submits, and makes it the image's values.
    fn finish_values(
        &mut self,
        gpu: &Gpu,
        img: &mut GpuImage,
        texture: wgpu::Texture,
        (tw, th, mips): (u32, u32, u32),
        channels: u32,
        mut enc: wgpu::CommandEncoder,
    ) {
        let pipes = self.medical_pipes(gpu);
        // One layer of one level: read through the mip pipeline's array binding, or drawn to.
        let mip_view = |m: u32, layer: u32, dim| {
            texture.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(dim),
                base_mip_level: m,
                mip_level_count: Some(1),
                base_array_layer: layer,
                array_layer_count: Some(1),
                ..Default::default()
            })
        };
        use wgpu::TextureViewDimension as Dim;
        for (layer, m) in
            (0..texture.depth_or_array_layers()).flat_map(|l| (1..mips).map(move |m| (l, m)))
        {
            let prev = mip_view(m - 1, layer, Dim::D2Array);
            let v = mip_view(m, layer, Dim::D2);
            let bind = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("values mip"),
                layout: &pipes.mip_bgl,
                entries: &[wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&prev),
                }],
            });
            let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("values mip"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &v,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            rp.set_pipeline(if channels > 1 {
                &pipes.mip4
            } else {
                &pipes.mip
            });
            rp.set_bind_group(0, &bind, &[]);
            rp.draw(0..3, 0..1);
        }
        gpu.queue.submit([enc.finish()]);
        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(Dim::D2Array),
            ..Default::default()
        });
        let window = img.values.as_ref().and_then(|v| v.window);
        let overlays = img.values.as_ref().is_none_or(|v| v.overlays);
        // The science look (and its tables) carries over to the next planes.
        let science = img.values.as_mut().and_then(|v| v.science.take());
        let limits = img.values.as_ref().and_then(|v| v.limits);
        // The mask belongs to the frame: new values drop it (see `set_graphics`).
        let old = img.values.replace(Values {
            texture,
            view,
            width: tw,
            height: th,
            mips,
            window,
            mask: None,
            overlays,
            channels,
            science,
            limits,
        });
        if let Some(o) = old {
            for t in o.into_textures() {
                self.recycle(t);
            }
        }
        img.revision += 1;
    }

    /// Draws the values as a science image through `look` (`None`: the ordinary pixels
    /// again), uploading its equalisation tables when the stretch needs them. Bumps the
    /// revision. Without values this does nothing and returns false.
    pub fn set_science(
        &mut self,
        gpu: &Gpu,
        img: &mut GpuImage,
        look: Option<ScienceLook>,
    ) -> bool {
        let Some(old) = img.values.as_mut().map(|v| v.science.take()) else {
            return false;
        };
        if let Some((_, Some(l))) = old {
            self.recycle(l.texture);
        }
        let new = look.map(|look| {
            let rows = look.luts.len().min(MAX_CHANNELS) as u32;
            let n = look.luts.first().map_or(0, |l| l.len()) as u32;
            let lut = (look.stretch == Stretch::HistEq && rows > 0 && n > 0).then(|| {
                let mut data = vec![0.0f32; n as usize * rows as usize];
                for (row, l) in data.chunks_exact_mut(n as usize).zip(&look.luts) {
                    let k = l.len().min(n as usize);
                    row[..k].copy_from_slice(&l[..k]);
                }
                let texture = self.texture(
                    &gpu.device,
                    "science lut",
                    n,
                    rows,
                    1,
                    VALUES,
                    wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                );
                gpu.queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    bytemuck::cast_slice(&data),
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(n * 4),
                        rows_per_image: Some(rows),
                    },
                    wgpu::Extent3d {
                        width: n,
                        height: rows,
                        depth_or_array_layers: 1,
                    },
                );
                let view = texture.create_view(&Default::default());
                Lut { texture, view }
            });
            if let Some(t) = &look.colour_table {
                let rows: Vec<[f32; 4]> = t
                    .iter()
                    .map(|c| [0, 1, 2, 3].map(|k| c.get(k).map_or(1.0, |&v| f32::from(v) / 255.0)))
                    .collect();
                gpu.queue.write_buffer(
                    &self.cmaps,
                    (FILE_LUT_MAP * specular_inspect::MAP_LEN * 16) as u64,
                    bytemuck::cast_slice(&rows),
                );
            }
            (look, lut)
        });
        if let Some(v) = img.values.as_mut() {
            v.science = new;
        }
        img.revision += 1;
        true
    }

    /// Sets the values' display shutter / overlay mask (see [`ValueMask`]); `None` removes
    /// it. Call after `set_values` (which drops the previous frame's mask).
    pub fn set_graphics(&mut self, gpu: &Gpu, img: &mut GpuImage, mask: Option<ValueMask<'_>>) {
        let (w, h) = (img.width, img.height);
        let Some((tw, th)) = img.values.as_ref().map(|v| (v.width, v.height)) else {
            return;
        };
        let new = mask.map(|vm| {
            let m = vm.mask;
            let rgba = |c: [f32; 3]| [c[0], c[1], c[2], 1.0].map(|x: f32| x.clamp(0.0, 1.0));
            let mut inks = [[0.0f32; 4]; MAX_INKS];
            for (o, c) in inks.iter_mut().zip(vm.inks) {
                *o = rgba(*c);
            }
            assert_eq!(m.len(), w as usize * h as usize, "mask size");
            let small;
            let data = if (tw, th) != (w, h) {
                small = downscale_mask(m, w, h, w.div_ceil(tw).max(h.div_ceil(th)));
                small.as_slice()
            } else {
                m
            };
            let texture = self.texture(
                &gpu.device,
                "values mask",
                tw,
                th,
                1,
                MASK,
                wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            );
            gpu.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                data,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(tw),
                    rows_per_image: Some(th),
                },
                wgpu::Extent3d {
                    width: tw,
                    height: th,
                    depth_or_array_layers: 1,
                },
            );
            let view = texture.create_view(&Default::default());
            Mask {
                texture,
                view,
                shutter: rgba(vm.shutter),
                inks,
            }
        });
        let Some(v) = img.values.as_mut() else {
            return;
        };
        let had = v.mask.is_some();
        let old = std::mem::replace(&mut v.mask, new);
        if had || v.mask.is_some() {
            img.revision += 1;
        }
        if let Some(o) = old {
            self.recycle(o.texture);
        }
    }

    /// Drops an image's values (it draws its ordinary pixels again).
    pub fn clear_values(&mut self, img: &mut GpuImage) {
        if let Some(o) = img.values.take() {
            for t in o.into_textures() {
                self.recycle(t);
            }
            img.revision += 1;
        }
    }

    /// Draws `img`'s values through its window into the open pass (see `draw`).
    pub(super) fn draw_values(
        &mut self,
        gpu: &Gpu,
        rp: &mut wgpu::RenderPass<'_>,
        img: &GpuImage,
        view: &View,
        opts: &RenderOptions,
        circle: Option<((f32, f32), f32)>,
    ) {
        let Some(v) = img.values.as_ref() else {
            return;
        };
        let look = v.science.as_ref();
        let (voi, mut mode) = match (look, v.window) {
            (Some(_), _) => ([0.0; 4], [0.0, 0.0, 0.0, 1.0]),
            (None, Some(win)) => win.rows(),
            (None, None) => return,
        };
        let a = view.stored_from_screen();
        let vp = view.viewport;
        mode[2] = v.overlays as u32 as f32;
        let mut sci = [0.0f32; 4];
        let (mut lo, mut inv) = (
            [[0.0f32; 4]; MAX_CHANNELS / 4],
            [[0.0f32; 4]; MAX_CHANNELS / 4],
        );
        let mut mid = [[0.5f32; 4]; MAX_CHANNELS / 4];
        let mut cols = [[0.0f32; 4]; MAX_CHANNELS];
        let (mut hs, mut hsr) = ([0.0f32; 4], [0.0f32; 4]);
        if let Some(h) = look
            .and_then(|(l, _)| l.hillshade)
            .filter(|_| v.channels == 1)
        {
            let [e, n, up] = h.sun();
            let [fx, fy] = h.factors();
            let kind = match h.kind {
                Relief::Shade => 1.0,
                Relief::Slope => 2.0,
                Relief::Aspect => 3.0,
            };
            hs = [kind, e, n, up];
            hsr = [fx, fy, SLOPE_MAP.index() as f32, ASPECT_MAP.index() as f32];
        }
        let (mut iso, mut iso_col) = ([0.0f32; 4], [0.0f32; 4]);
        if let Some(i) = look
            .and_then(|(l, _)| l.isotherm)
            .filter(|_| v.channels == 1)
        {
            iso = [1.0, i.above as u32 as f32, i.level, 0.0];
            iso_col = [i.colour[0], i.colour[1], i.colour[2], 1.0];
        }
        if let Some((l, _)) = look {
            sci = [l.stretch.index() as f32, v.channels as f32, 0.0, 0.0];
            for (k, c) in l.channels.iter().take(MAX_CHANNELS).enumerate() {
                lo[k / 4][k % 4] = c.lo;
                inv[k / 4][k % 4] = if c.hi > c.lo {
                    1.0 / (c.hi - c.lo)
                } else {
                    0.0
                };
                cols[k] = [c.colour[0], c.colour[1], c.colour[2], c.on as u32 as f32];
                mid[k / 4][k % 4] = c.mid;
            }
        }
        let u = MedUniform {
            m0: [a.m[0], a.m[1], a.m[2], 0.0],
            m1: [a.m[3], a.m[4], a.m[5], 0.0],
            img: [img.width as f32, img.height as f32, view.scale(), 0.0],
            tex: [v.width as f32, v.height as f32, v.mips as f32, 0.0],
            canvas: opts.canvas,
            grid: opts.grid.unwrap_or([0.0; 4]),
            misc: [
                v.mask.is_some() as u32 as f32,
                opts.nearest_from,
                opts.grid_from,
                0.0,
            ],
            clip: [vp.x, vp.y, vp.x + vp.w, vp.y + vp.h],
            mask: circle.map_or([0.0; 4], |(c, r)| [c.0, c.1, r, 1.0]),
            voi,
            mode,
            shutter: v.mask.as_ref().map_or([0.0; 4], |m| m.shutter),
            inks: v.mask.as_ref().map_or([[0.0; 4]; MAX_INKS], |m| m.inks),
            sci,
            lo,
            inv,
            cols,
            fc: [0.0; 4],
            hl: [0.0; 4],
            hilo_hi: [0.0; 4],
            hilo_lo: [0.0; 4],
            mid,
            hs,
            hsr,
            iso,
            iso_col,
            gsdf: [0.0; 4],
        };
        let (_, mut fc, hi, hilo_lo) = inspect_rows(opts);
        // The file's colour table on one channel, unless false colour was asked for.
        if let Some((l, _)) = look
            && l.colour_table.is_some()
            && v.channels == 1
            && opts.false_colour.is_none()
        {
            fc[0] = 1.0;
            fc[1] = FILE_LUT_MAP as f32;
        }
        // HiLo needs the bit depth's limits; colours go display-coded, like the window's grey.
        let limits = v.limits.filter(|_| opts.hilo.is_some());
        fc[2] = limits.is_some() as u32 as f32;
        let enc = |c: [f32; 4]| {
            [
                specular_inspect::srgb_encode(c[0]),
                specular_inspect::srgb_encode(c[1]),
                specular_inspect::srgb_encode(c[2]),
                1.0,
            ]
        };
        let u = MedUniform {
            fc,
            hl: limits.map_or([0.0; 4], |[l, h]| {
                // Values carry f32 rounding of a fractional rescale: a relative hair of slack.
                let slack = |x: f32| x.abs() * 1e-6;
                [h - slack(h), l + slack(l), 0.0, 0.0]
            }),
            hilo_hi: enc(hi),
            hilo_lo: enc(hilo_lo),
            gsdf: opts
                .gsdf
                .filter(|g| g.l_max > g.l_min && g.j_max > g.j_min)
                .map_or([0.0; 4], |g| {
                    [
                        g.j_min as f32,
                        g.j_max as f32,
                        g.l_min as f32,
                        (1.0 / (g.l_max - g.l_min)) as f32,
                    ]
                }),
            ..u
        };
        self.medical_pipes(gpu);
        let Some(pipes) = self.medical.as_ref() else {
            return;
        };
        let ub = gpu.device.buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("medical"),
            contents: bytemuck::bytes_of(&u),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let bind = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("medical"),
            layout: &pipes.draw_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: ub.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&v.view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(
                        v.mask.as_ref().map_or(&pipes.no_mask, |m| &m.view),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(
                        look.and_then(|s| s.1.as_ref())
                            .map_or(&pipes.no_lut, |l| &l.view),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: self.cmaps.as_entire_binding(),
                },
            ],
        });
        rp.set_pipeline(&pipes.draw);
        rp.set_bind_group(0, &bind, &[]);
        rp.draw(0..3, 0..1);
    }
}

#[cfg(test)]
#[path = "medical_tests.rs"]
mod tests;
