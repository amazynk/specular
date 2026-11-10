//! The GPU side: converts decoded planes into linear-light textures (overview with Lanczos
//! mips, detail tiles in an array texture) and draws the image for a [`View`].

use std::collections::HashMap;

use bytemuck::{Pod, Zeroable};
use specular_core::paths::Engine;
use specular_core::{GainMap, GainMapParams, PixelLayout, Raster, Transfer, YuvEncoding, hdr};
use specular_gfx::BufferInit;
use specular_gfx::{FULLSCREEN_WGSL, Gpu, HDR, fullscreen_pipeline, wgpu};
use specular_inspect::{ChannelColours, ChannelView, ColourMap, MAP_LEN};

use crate::icc::SourceLut;
use crate::tiles::{self, BORDER, LEVEL_CAP, MAX_LEVEL, SLOT, TILE, TileKey, TilePool};
use crate::view::View;

mod calibrate;
#[cfg(test)]
mod channel_tests;
mod colour;
#[cfg(test)]
mod colour_tests;
pub mod compare;
#[cfg(test)]
mod display_aid_tests;
#[cfg(test)]
mod hdr_tests;
pub mod inspect;
pub mod medical;
#[cfg(test)]
mod ocio_tests;
#[cfg(test)]
mod output_tests;
#[cfg(test)]
mod picture_tests;
mod range;
pub mod relight;
#[cfg(test)]
mod resample_tests;
#[cfg(test)]
mod stain_tests;
#[cfg(test)]
mod yuv_tests;

pub use calibrate::{COLOUR_STEPS, ycbcr_variant};

use colour::GpuLut;
pub use colour::Storage;

/// Format of the overview and tiles: 8-bit sRGB-encoded linear light (the hardware decodes
/// on load, so filtering is gamma-correct at half the memory of half-floats).
const STORE: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

/// The view format the image shader reads [`STORE`] textures through: the stored codes,
/// decoded exactly in the shader (`srgb_decode` in image.wgsl). The texture unit's own sRGB
/// decode keeps about nine significant bits on some GPUs, which made 1:1 output differ from
/// the source by one code on a quarter of the pixels.
const STORE_CODES: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// The view of an overview or tile texture that the image shader binds (see [`STORE_CODES`]).
fn sampled_view(t: &wgpu::Texture, desc: wgpu::TextureViewDescriptor) -> wgpu::TextureView {
    let format = (t.format() == STORE).then_some(STORE_CODES);
    t.create_view(&wgpu::TextureViewDescriptor { format, ..desc })
}

/// Overview format of images with samples above reference white (16-bit and float rasters):
/// linear half-float, 1.0 = SDR reference white.
const STORE_HDR: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

/// Detail tiles of images with wide SDR samples (16-bit PNG, TIFF, JPEG 2000, ...): 16-bit
/// sRGB codes, decoded in the shader like 8-bit ones (`ld_tile` in image.wgsl). An sRGB-coded
/// 16-bit source without a colour transform is stored exactly; half-floats would keep 11 bits.
const STORE16: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Uint;

/// What [`ImageRenderer::convert`] writes.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Dst {
    /// [`STORE`]: 8-bit sRGB codes of linear light.
    Srgb8,
    /// [`STORE_HDR`]: linear half-floats.
    Half,
    /// [`STORE16`]: 16-bit sRGB codes.
    Codes16,
}

/// The 16-bit tile array (made on the first image with wide samples).
struct Tiles16 {
    tex: wgpu::Texture,
    view: wgpu::TextureView,
    layer_views: Vec<wgpu::TextureView>,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct CvtUniform {
    v: [f32; 12],
    layout: u32,
    has_lut: u32,
    lut_size: f32,
    step: f32,
    /// [`Transfer`] of 16-bit samples: 0 sRGB, 1 linear, 2 PQ, 3 HLG.
    transfer: u32,
    /// Primaries of wide samples: 0 BT.709, 1 Display P3, 2 BT.2020.
    primaries: u32,
    /// Auto-range ([`range`]) of wide grey layouts: 1 on, 0 off.
    range_on: u32,
    /// Significant bits of the samples (HiLo's limits under auto-range).
    range_bits: u32,
    /// Source matrix rows (curves mode), offsets in the fourth column.
    src: [f32; 12],
    /// Linear BT.709 → storage space rows.
    store: [f32; 12],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ImgUniform {
    m0: [f32; 4],
    m1: [f32; 4],
    img: [f32; 4],
    ov: [f32; 4],
    canvas: [f32; 4],
    grid: [f32; 4],
    checker_a: [f32; 4],
    checker_b: [f32; 4],
    misc: [f32; 4],
    clip: [f32; 4],
    mask: [f32; 4],
    /// Overview value ceiling (1 SDR, 65504 HDR), content peak, display peak, flags
    /// (1 = apply gain map, 2 = tone-map, 4 = gain map in its own application space).
    hdr: [f32; 4],
    /// Gain map per channel (xyz): log2 min gain (w = weight), log2 max gain, gamma,
    /// base offset, alternate offset.
    gm_min: [f32; 4],
    gm_max: [f32; 4],
    gm_gamma: [f32; 4],
    gm_base: [f32; 4],
    gm_alt: [f32; 4],
    /// Storage space → linear BT.709 rows.
    from_store: [f32; 12],
    /// Channel isolation: shader mode (x), see `ChannelView::shader_mode`.
    chan: [f32; 4],
    /// Gain-map application space (flag 4): linear BT.709 → application primaries rows,
    /// then back.
    gm_to_app: [f32; 12],
    gm_from_app: [f32; 12],
    /// Channel colours (rows; w of the first = on).
    cc: [f32; 12],
    /// False colour on, map index, HiLo on, unused.
    fc: [f32; 4],
    /// HiLo colours (linear straight rgb) and encoded thresholds (w).
    hi: [f32; 4],
    lo: [f32; 4],
    /// Stain separation ([`StainView`]): unmixing rows, reconstruction rows, tints (w of the
    /// first: OD at full brightness), and the shown stains (xyz) with the mode (w: 0 off,
    /// 1 light, 2 OD).
    st: [f32; 12],
    sr: [f32; 12],
    sc: [f32; 12],
    sv: [f32; 4],
    /// OCIO display/view ([`OcioLut`]): on (x), log2 of the shaper's low and high ends
    /// (y, z), table size (w).
    oc: [f32; 4],
    /// Picture adjustments ([`crate::PictureAdjust`]): rows on sRGB-encoded colour, then
    /// (1 / gamma, on, 0, 0).
    pa: [f32; 12],
    pg: [f32; 4],
}

/// An OCIO display/view transform baked for the shader (`specular_ocio::bake`): each
/// channel of the displayed linear Rec.709 colour goes through a log2 shaper, then a 3D
/// table of display code values (clipped to 0..1), red fastest, uploaded with
/// [`ImageRenderer::upload_ocio`] under `id`. The table's values are shown as the
/// display's own codes. Replaces tone mapping while on; off while the table of `id` is not
/// the one uploaded.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OcioLut {
    pub id: u64,
    pub size: u32,
    pub log2_lo: f32,
    pub log2_hi: f32,
}

/// Colour deconvolution of brightfield stains, per pixel on the GPU (the arithmetic of
/// `specular_pathology::stain::render`): the displayed colour's sRGB codes become optical
/// densities `od = -log10(max(code, 1e-6))`, stain `s`'s concentration is
/// `max(dot(od, unmix[s]), 0)` (0 when hidden), and the pixel shows either the light the
/// shown stains let through, `10^-(Σ c_s recon[s])`, or their concentrations as
/// brightness, `Σ min(c_s / od_max, 1) tint[s]` (sRGB codes).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StainView {
    pub unmix: [[f32; 3]; 3],
    pub recon: [[f32; 3]; 3],
    pub tint: [[f32; 3]; 3],
    pub visible: [bool; 3],
    /// Concentrations as brightness instead of light.
    pub od: bool,
    pub od_max: f32,
}

impl StainView {
    /// The uniform's rows.
    fn rows(&self) -> ([f32; 12], [f32; 12], [f32; 12], [f32; 4]) {
        let flat = |m: &[[f32; 3]; 3]| {
            let mut o = [0f32; 12];
            for (r, row) in m.iter().enumerate() {
                o[r * 4..r * 4 + 3].copy_from_slice(row);
            }
            o
        };
        let mut sc = flat(&self.tint);
        sc[3] = self.od_max.max(1e-6);
        let v = self.visible.map(|b| f32::from(u8::from(b)));
        (
            flat(&self.unmix),
            flat(&self.recon),
            sc,
            [v[0], v[1], v[2], if self.od { 2.0 } else { 1.0 }],
        )
    }
}

/// How to draw.
#[derive(Clone, Copy, Debug)]
pub struct RenderOptions {
    /// Lanczos resampling (settled) instead of bilinear (moving).
    pub hq: bool,
    /// Linear premultiplied colours.
    pub canvas: [f32; 4],
    pub checker_a: [f32; 4],
    pub checker_b: [f32; 4],
    /// Checkerboard cell in output pixels.
    pub checker_cell: f32,
    /// Pixel-grid colour (premultiplied), drawn from `grid_from` zoom on.
    pub grid: Option<[f32; 4]>,
    pub grid_from: f32,
    /// Nearest-neighbour from this zoom on.
    pub nearest_from: f32,
    /// A single channel in grey instead of the image. The checkerboard and matte alpha
    /// views draw the image as usual; the caller picks the backdrop (`checker_a/b`).
    pub channel: ChannelView,
    /// False colour (DESIGN.md §6.2): the grey (luminance, or the isolated channel) through
    /// a colour map. Medical images map their windowed value.
    pub false_colour: Option<ColourMap>,
    /// How colour channels are coloured (multichannel composites).
    pub channel_colours: ChannelColours,
    /// HiLo: marks pixels at the real bit depth's maximum and minimum.
    pub hilo: Option<HiLo>,
    /// Lanczos output clamped to the range of the texels under the kernel's main lobe, so
    /// hard edges do not ring (on by default; off only to measure the kernel itself).
    pub anti_ringing: bool,
    /// Medical greys shown on the DICOM Grayscale Standard Display Function (PS3.14) of
    /// a display with this black and white luminance; `None` shows them as sRGB greys.
    pub gsdf: Option<Gsdf>,
    /// Stain separation of a brightfield slide (display only), on the colour image path;
    /// ignored while a single channel is isolated.
    pub stain: Option<StainView>,
    /// An OCIO display/view transform for scene-linear images; ignored while a single
    /// channel is isolated.
    pub ocio: Option<OcioLut>,
    /// A playing movie's picture adjustments (display only); `None` or neutral draws the
    /// picture as it is.
    pub picture: Option<crate::PictureAdjust>,
}

/// A display calibrated to the DICOM Grayscale Standard Display Function: P-values in equal
/// just-noticeable-difference steps from its black to its white (PS3.14 7.1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gsdf {
    /// JND indices of the display's black and white (`specular_dicom::gsdf::jnd_index`).
    pub j_min: f64,
    pub j_max: f64,
    /// The display's black and white in cd/m² (ambient included).
    pub l_min: f64,
    pub l_max: f64,
}

/// The HiLo indicator's colours (linear, straight RGB) and, for 8-bit tile data, the
/// sRGB-encoded thresholds (inclusive): 1 and 0 for exact codes. Medical values use the
/// stored range instead (see `medical::Window`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HiLo {
    pub high: [f32; 3],
    pub low: [f32; 3],
}

impl HiLo {
    /// Encoded threshold at or above which a pixel counts as at the maximum: halfway into
    /// the top 8-bit code, so filtering between a maximum and its neighbour still marks the
    /// maximum's own texel at nearest-neighbour zoom.
    pub const HIGH: f32 = 254.5 / 255.0;
    pub const LOW: f32 = 0.5 / 255.0;
}

impl Default for RenderOptions {
    fn default() -> Self {
        RenderOptions {
            hq: true,
            canvas: [0.0, 0.0, 0.0, 1.0],
            checker_a: [0.04, 0.04, 0.04, 1.0],
            checker_b: [0.02, 0.02, 0.02, 1.0],
            checker_cell: 8.0,
            grid: None,
            grid_from: 8.0,
            nearest_from: 4.0,
            channel: ChannelView::Normal,
            false_colour: None,
            channel_colours: ChannelColours::RedGreen,
            hilo: None,
            anti_ringing: true,
            gsdf: None,
            stain: None,
            ocio: None,
            picture: None,
        }
    }
}

/// The resident whole-image texture.
#[derive(Debug)]
pub struct Overview {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub width: u32,
    pub height: u32,
    pub mips: u32,
    /// Decode scale it came from (0 = embedded thumbnail).
    pub source_scale: u32,
}

/// A gain map on the GPU (Ultra HDR / ISO 21496-1).
#[derive(Debug)]
pub struct GainTexture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub params: GainMapParams,
}

/// One image on the GPU.
#[derive(Debug)]
pub struct GpuImage {
    pub id: u64,
    /// Stored size.
    pub width: u32,
    pub height: u32,
    lut: Option<GpuLut>,
    /// Linear BT.709 ↔ storage space of the overview and tiles.
    storage: Storage,
    pub overview: Option<Overview>,
    /// Bumped whenever new pixels arrive (the scene must be redrawn).
    pub revision: u64,
    /// Brightest value in the stored samples (units of SDR reference white; 1 for SDR).
    pub content_peak: f32,
    /// Float rasters: the factor values are stored at (`specular_core::hdr::float_exposure`:
    /// 1 for light, below it for data far above reference white such as a 0–255 PFM).
    pub exposure: f32,
    /// The overview holds values above reference white (half-float storage).
    pub hdr: bool,
    pub gain_map: Option<GainTexture>,
    /// Modality values of a medical image, drawn through a window ([`medical`]).
    values: Option<medical::Values>,
    /// Coefficients of a reflectance transformation image, relit into the overview
    /// ([`relight`]).
    relight: Option<relight::Planes>,
    /// Auto-range of wide grey samples ([`ImageRenderer::set_auto_range`]).
    range: Option<range::AutoRange>,
    /// Coarsest detail-tile level (scale `2^max_level`): [`MAX_LEVEL`] unless the decoder
    /// offers coarser scales (a whole-slide pyramid, [`GpuImage::set_max_scale`]).
    max_level: u8,
    /// Detail tiles are 16-bit ([`STORE16`]): the samples are wider than 8 bits.
    wide_tiles: bool,
    /// Tile uploads per level (each also bumped `revision`).
    tile_uploads: [u64; LEVEL_CAP as usize + 1],
}

impl GpuImage {
    /// Lets detail tiles come from scales up to `scale` (a power of two the decoder serves
    /// cheaply; at least 8).
    pub fn set_max_scale(&mut self, scale: u32) {
        self.max_level = (scale.max(1).ilog2() as u8).clamp(MAX_LEVEL, LEVEL_CAP);
    }

    /// The coarsest detail-tile level.
    pub fn max_level(&self) -> u8 {
        self.max_level
    }

    /// [`GpuImage::revision`] without the tile uploads to levels finer than `level`: what a
    /// view drawn at `level` depends on (it reads that level and coarser ones), so a loupe's
    /// full-resolution tiles do not redraw the view at fit.
    pub fn revision_from(&self, level: u8) -> u64 {
        let finer = (level as usize).min(self.tile_uploads.len());
        self.revision - self.tile_uploads[..finer].iter().sum::<u64>()
    }

    /// Overview texels per stored pixel (0 without an overview).
    pub fn overview_scale(&self) -> f32 {
        self.overview
            .as_ref()
            .map(|o| o.width as f32 / self.width as f32)
            .unwrap_or(0.0)
    }

    pub fn has_lut(&self) -> bool {
        self.lut.is_some()
    }

    pub fn memory_bytes(&self) -> u64 {
        let ov = self
            .overview
            .as_ref()
            .map(|o| texture_bytes(&o.texture))
            .unwrap_or(0);
        ov + self
            .gain_map
            .as_ref()
            .map(|g| texture_bytes(&g.texture))
            .unwrap_or(0)
            + self.values.as_ref().map_or(0, medical::Values::bytes)
            + self.relight_bytes()
    }

    /// Whether drawing this image can produce values above SDR reference white.
    pub fn is_hdr(&self) -> bool {
        self.hdr || self.gain_map.is_some()
    }
}

pub struct ImageRenderer {
    convert_pipe: wgpu::RenderPipeline,
    convert_bgl: wgpu::BindGroupLayout,
    mip_pipe: wgpu::RenderPipeline,
    mip_bgl: wgpu::BindGroupLayout,
    image_pipe: wgpu::RenderPipeline,
    image_bgl: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    tile_tex: wgpu::Texture,
    tile_view: wgpu::TextureView,
    tile_layer_views: Vec<wgpu::TextureView>,
    /// Slots below `pool.wide_from()` are layers of `tile_tex`, the rest of `tiles16`.
    pub pool: TilePool,
    /// VRAM for detail tiles; split evenly between the two arrays once `tiles16` exists.
    tile_budget: u64,
    tiles16: Option<Tiles16>,
    dummy_tiles16: wgpu::TextureView,
    /// The sRGB EOTF of the 256 8-bit codes (`srgb_decode8` in image.wgsl).
    decode8: wgpu::TextureView,
    /// Writes [`STORE16`] tiles; built with `tiles16`.
    convert16_pipe: Option<wgpu::RenderPipeline>,
    dummy_plane: wgpu::TextureView,
    dummy_lut: wgpu::TextureView,
    dummy_ov: wgpu::TextureView,
    dummy_u16: wgpu::TextureView,
    dummy_gain: wgpu::TextureView,
    /// Convert and mip pipelines writing half-float overviews, built on first use.
    hdr_pipes: Option<(wgpu::RenderPipeline, wgpu::RenderPipeline)>,
    /// Peak the display can show, in units of SDR reference white (1 = SDR output).
    display_peak: f32,
    max_tex: u32,
    /// YCbCr → RGB path forced for the correctness gate and calibration ([`calibrate`]).
    colour_force: Option<usize>,
    /// The correctness gate ran in this renderer.
    colour_checked: bool,
    /// Calibration's test image, kept between steps.
    colour_sample: Option<Raster>,
    /// Milliseconds of the last upload (planes + convert), for the HUD.
    pub last_upload_ms: f64,
    /// Plane and overview textures kept for reuse. Allocating fresh ones per image costs up to
    /// tens of milliseconds on the main thread once video memory churns (holding `→`).
    spare: Vec<wgpu::Texture>,
    /// Storage space of images created from now on (see `colour`).
    storage: Storage,
    /// Medical pipelines (window/level on the GPU), built on first use.
    medical: Option<medical::Pipes>,
    relight: Option<relight::Pipes>,
    /// The false-colour maps ([`ColourMap::gpu_table`]), bound by image and medical draws.
    cmaps: wgpu::Buffer,
    /// `[0, 65535]`: the range buffer of conversions without auto-range.
    dummy_range: wgpu::Buffer,
    /// The auto-range pipeline, built on first use.
    range_pipe: Option<range::RangePipe>,
    /// The uploaded OCIO table and its id ([`OcioLut`]).
    ocio_lut: Option<(u64, wgpu::TextureView)>,
}

/// Video memory the spare textures may hold.
const SPARE_BYTES: u64 = 192 << 20;

/// Approximate bytes of a 2D texture with its mips.
fn texture_bytes(t: &wgpu::Texture) -> u64 {
    let px = t.width() as u64 * t.height() as u64 * u64::from(t.depth_or_array_layers());
    let bpp = t.format().block_copy_size(None).unwrap_or(4) as u64;
    let mips = if t.mip_level_count() > 1 { 4 } else { 3 };
    px * bpp * mips / 3
}

impl std::fmt::Debug for ImageRenderer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImageRenderer")
            .field("tiles", &self.pool.len())
            .finish()
    }
}

fn tex_entry(
    binding: u32,
    dim: wgpu::TextureViewDimension,
    filterable: bool,
) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable },
            view_dimension: dim,
            multisampled: false,
        },
        count: None,
    }
}

fn uniform_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn storage_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only: true },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

/// The map index of the file's own colour table (an ImageJ LUT) in the colour-map buffer,
/// after the built-in maps: one slot, written by [`ImageRenderer::set_science`].
pub(crate) const FILE_LUT_MAP: usize = ColourMap::ALL.len();

/// The colour-map table every image and medical draw binds: the built-in maps, then the file
/// colour table's slot (grey until a science look sets it).
fn cmap_buffer(d: &wgpu::Device) -> wgpu::Buffer {
    let mut table = ColourMap::gpu_table();
    table.extend((0..MAP_LEN).map(|i| {
        let g = i as f32 / (MAP_LEN - 1) as f32;
        [g, g, g, 1.0]
    }));

    d.buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("colour maps"),
        contents: bytemuck::cast_slice(&table),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    })
}

/// The sRGB EOTF of the 256 8-bit codes, exact to f32, as a 256 × 1 texture: the image
/// shader looks stored codes up instead of evaluating pow for every tap (`srgb_decode8`).
fn decode8_table(gpu: &Gpu) -> wgpu::TextureView {
    use wgpu::util::DeviceExt;
    let lut: Vec<f32> = (0..256)
        .map(|i| {
            let e = i as f64 / 255.0;
            let l = if e <= 0.04045 {
                e / 12.92
            } else {
                ((e + 0.055) / 1.055).powf(2.4)
            };
            l as f32
        })
        .collect();
    gpu.device
        .create_texture_with_data(
            &gpu.queue,
            &wgpu::TextureDescriptor {
                label: Some("sRGB decode table"),
                size: wgpu::Extent3d {
                    width: 256,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R32Float,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            bytemuck::cast_slice(&lut),
        )
        .create_view(&Default::default())
}

/// `fc`, `hi`, `lo` and the channel-colour rows of the image and medical uniforms.
pub(crate) fn inspect_rows(opts: &RenderOptions) -> ([f32; 12], [f32; 4], [f32; 4], [f32; 4]) {
    let mut cc = [0f32; 12];
    if let Some(m) = opts.channel_colours.matrix() {
        for (r, row) in m.iter().enumerate() {
            cc[r * 4..r * 4 + 3].copy_from_slice(row);
        }
        cc[3] = 1.0;
    }
    let fc = [
        opts.false_colour.is_some() as u32 as f32,
        opts.false_colour.map_or(0, ColourMap::index) as f32,
        opts.hilo.is_some() as u32 as f32,
        0.0,
    ];
    let (hi, lo) = match opts.hilo {
        Some(h) => (
            [h.high[0], h.high[1], h.high[2], HiLo::HIGH],
            [h.low[0], h.low[1], h.low[2], HiLo::LOW],
        ),
        None => ([0.0; 4], [0.0; 4]),
    };
    (cc, fc, hi, lo)
}

fn sampler_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
        count: None,
    }
}

#[allow(clippy::too_many_arguments)]
fn texture_2d(
    d: &wgpu::Device,
    label: &str,
    w: u32,
    h: u32,
    layers: u32,
    mips: u32,
    format: wgpu::TextureFormat,
    usage: wgpu::TextureUsages,
) -> wgpu::Texture {
    d.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: w.max(1),
            height: h.max(1),
            depth_or_array_layers: layers,
        },
        mip_level_count: mips,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage,
        view_formats: if format == STORE { &[STORE_CODES] } else { &[] },
    })
}

/// Texels of a raster's first plane.
fn plane_size(r: &Raster) -> (u32, u32) {
    r.planes.first().map_or((0, 0), |p| (p.width, p.height))
}

/// The fewest tile layers: enough for one screen of detail tiles.
const MIN_TILES: u32 = 24;
/// The tile budget on a software adapter at most: its "video memory" is system RAM shared with
/// the decoders and caches, and drawing is on the CPU, so a large array buys little (60 layers).
const SOFTWARE_TILE_BUDGET: u64 = 64 << 20;

/// Tile layers that fit `budget` bytes of VRAM (and the device's array limit), with
/// [`SOFTWARE_TILE_BUDGET`] at most on a software adapter.
fn tile_capacity(gpu: &Gpu, budget: u64) -> u32 {
    tile_layers(
        budget,
        gpu.is_software(),
        gpu.limits.max_texture_array_layers,
    )
}

fn tile_layers(budget: u64, software: bool, max_layers: u32) -> u32 {
    let budget = if software {
        budget.min(SOFTWARE_TILE_BUDGET)
    } else {
        budget
    };
    let per = SLOT as u64 * SLOT as u64 * 4;
    ((budget / per) as u32).clamp(MIN_TILES, max_layers.min(2048))
}

/// The tile array with `cap` layers or, when the GPU runs out of memory for that many, half as
/// many, then a quarter, and so on down to [`MIN_TILES`]. Returns the layers it has. Without
/// this a GPU short of memory (a virtual machine's software adapter) left the array invalid
/// and every frame black.
fn tile_array(gpu: &Gpu, cap: u32, format: wgpu::TextureFormat) -> (u32, wgpu::Texture) {
    fewer_until_it_fits(cap, |n| {
        let (tex, failed) = gpu.out_of_memory(|| {
            texture_2d(
                &gpu.device,
                "tiles",
                SLOT,
                SLOT,
                n,
                1,
                format,
                wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::RENDER_ATTACHMENT
                    // Tests read tiles back.
                    | if cfg!(test) {
                        wgpu::TextureUsages::COPY_SRC
                    } else {
                        wgpu::TextureUsages::empty()
                    },
            )
        });
        if failed { Err(tex) } else { Ok(tex) }
    })
}

/// Calls `make(cap)`, then with half as many while it fails (out of memory), down to
/// [`MIN_TILES`]; the last attempt's result is kept even when it failed.
fn fewer_until_it_fits<T>(mut cap: u32, mut make: impl FnMut(u32) -> Result<T, T>) -> (u32, T) {
    loop {
        match make(cap) {
            Ok(made) => return (cap, made),
            Err(made) if cap <= MIN_TILES => {
                specular_log::error!(target: specular_log::GPU, "tile array: out of GPU memory even for {cap} tiles");
                return (cap, made);
            }
            Err(_) => {
                let fewer = (cap / 2).max(MIN_TILES);
                specular_log::warn!(target: specular_log::GPU, "tile array: out of GPU memory for {cap} tiles, trying {fewer}");
                cap = fewer;
            }
        }
    }
}

#[cfg(test)]
mod tile_array_tests {
    use super::*;

    #[test]
    fn software_adapters_get_a_small_array() {
        let default = 384 << 20;
        assert_eq!(tile_layers(default, false, 2048), 361);
        assert_eq!(tile_layers(default, true, 2048), 60);
        assert_eq!(
            tile_layers(16 << 20, true, 2048),
            MIN_TILES,
            "never below one screen"
        );
        assert_eq!(
            tile_layers(default, false, 256),
            256,
            "the device's array limit"
        );
    }

    #[test]
    fn halves_until_it_fits() {
        let mut tried = Vec::new();
        let (n, made) = fewer_until_it_fits(344, |n| {
            tried.push(n);
            if n > 100 { Err(n) } else { Ok(n) }
        });
        assert_eq!((n, made), (86, 86));
        assert_eq!(tried, [344, 172, 86]);
    }

    #[test]
    fn stops_at_the_minimum() {
        let mut tried = Vec::new();
        let (n, made) = fewer_until_it_fits(100, |n| {
            tried.push(n);
            Err::<u32, u32>(n)
        });
        assert_eq!((n, made), (MIN_TILES, MIN_TILES), "keeps the last attempt");
        assert_eq!(tried, [100, 50, 25, MIN_TILES]);
    }
}

struct Planes {
    views: Vec<wgpu::TextureView>,
    textures: Vec<wgpu::Texture>,
}

impl ImageRenderer {
    /// `tile_budget`: bytes of VRAM for detail tiles.
    pub fn new(gpu: &Gpu, tile_budget: u64) -> ImageRenderer {
        let d = &gpu.device;
        let t0 = std::time::Instant::now();
        let module = |label: &str, src: &str| {
            specular_gfx::shader(d, label, &format!("{FULLSCREEN_WGSL}\n{src}"))
        };
        let cmod = module("convert", include_str!("shaders/convert.wgsl"));
        let mmod = module("mip", &mip_source(false));
        let imod = module("image", include_str!("shaders/image.wgsl"));
        use wgpu::TextureViewDimension as Dim;
        let convert_bgl = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("convert"),
            entries: &[
                uniform_entry(0),
                tex_entry(1, Dim::D2, true),
                tex_entry(2, Dim::D2, true),
                tex_entry(3, Dim::D2, true),
                sampler_entry(4),
                tex_entry(5, Dim::D3, true),
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Uint,
                        view_dimension: Dim::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                storage_entry(7),
                tex_entry(8, Dim::D2, true),
            ],
        });
        let mip_bgl = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("mip"),
            entries: &[tex_entry(0, Dim::D2, true)],
        });
        let image_bgl = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("image"),
            entries: &[
                uniform_entry(0),
                tex_entry(1, Dim::D2Array, true),
                tex_entry(2, Dim::D2, true),
                sampler_entry(3),
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                tex_entry(5, Dim::D2, true),
                storage_entry(6),
                wgpu::BindGroupLayoutEntry {
                    binding: 7,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Uint,
                        view_dimension: Dim::D2Array,
                        multisampled: false,
                    },
                    count: None,
                },
                tex_entry(8, Dim::D2, false),
                tex_entry(9, Dim::D3, true),
            ],
        });
        let convert_pipe = fullscreen_pipeline(
            gpu,
            "convert",
            &cmod,
            "fs_convert",
            &[Some(&convert_bgl)],
            STORE,
            None,
        );
        let mip_pipe =
            fullscreen_pipeline(gpu, "mip", &mmod, "fs_mip", &[Some(&mip_bgl)], STORE, None);
        let image_pipe = fullscreen_pipeline(
            gpu,
            "image",
            &imod,
            "fs_image",
            &[Some(&image_bgl)],
            HDR,
            None,
        );
        let t_pipes = t0.elapsed();
        let sampler = d.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("image linear"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });
        let (cap, tile_tex) = tile_array(gpu, tile_capacity(gpu, tile_budget), STORE);
        let t_tex = t0.elapsed();
        let tile_view = sampled_view(
            &tile_tex,
            wgpu::TextureViewDescriptor {
                dimension: Some(Dim::D2Array),
                ..Default::default()
            },
        );
        let tile_layer_views = (0..cap)
            .map(|i| {
                tile_tex.create_view(&wgpu::TextureViewDescriptor {
                    dimension: Some(Dim::D2),
                    base_array_layer: i,
                    array_layer_count: Some(1),
                    ..Default::default()
                })
            })
            .collect();
        specular_log::debug!(
            target: specular_log::STARTUP,
            "image renderer: pipelines {:.1} ms, tile array ({cap} layers) {:.1} ms, layer views {:.1} ms",
            t_pipes.as_secs_f64() * 1e3,
            (t_tex - t_pipes).as_secs_f64() * 1e3,
            (t0.elapsed() - t_tex).as_secs_f64() * 1e3,
        );
        let tb = wgpu::TextureUsages::TEXTURE_BINDING;
        let dummy_plane = texture_2d(
            d,
            "dummy plane",
            1,
            1,
            1,
            1,
            wgpu::TextureFormat::R8Unorm,
            tb,
        )
        .create_view(&Default::default());
        let dummy_ov = sampled_view(
            &texture_2d(d, "dummy overview", 1, 1, 1, 1, STORE, tb),
            Default::default(),
        );
        let dummy_u16 = texture_2d(
            d,
            "dummy 16-bit plane",
            1,
            1,
            1,
            1,
            wgpu::TextureFormat::Rgba16Uint,
            tb,
        )
        .create_view(&Default::default());
        let dummy_tiles16 = texture_2d(d, "dummy 16-bit tiles", 1, 1, 1, 1, STORE16, tb)
            .create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(Dim::D2Array),
                ..Default::default()
            });
        let dummy_gain = texture_2d(
            d,
            "dummy gain map",
            1,
            1,
            1,
            1,
            wgpu::TextureFormat::Rgba8Unorm,
            tb,
        )
        .create_view(&Default::default());
        let dummy_lut = d
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("dummy lut"),
                size: wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D3,
                format: wgpu::TextureFormat::Rgba16Float,
                usage: tb,
                view_formats: &[],
            })
            .create_view(&Default::default());
        ImageRenderer {
            convert_pipe,
            convert_bgl,
            mip_pipe,
            mip_bgl,
            image_pipe,
            image_bgl,
            sampler,
            tile_tex,
            tile_view,
            tile_layer_views,
            pool: TilePool::new(cap),
            tile_budget,
            tiles16: None,
            dummy_tiles16,
            decode8: decode8_table(gpu),
            convert16_pipe: None,
            dummy_plane,
            dummy_lut,
            dummy_ov,
            dummy_u16,
            dummy_gain,
            hdr_pipes: None,
            display_peak: 1.0,
            max_tex: gpu.max_texture_size(),
            colour_force: None,
            colour_checked: false,
            colour_sample: None,
            last_upload_ms: 0.0,
            spare: Vec::new(),
            storage: Storage::default(),
            medical: None,
            relight: None,
            cmaps: cmap_buffer(d),
            dummy_range: d.buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("no auto range"),
                contents: bytemuck::cast_slice(&[0u32, 65535]),
                usage: wgpu::BufferUsages::STORAGE,
            }),
            range_pipe: None,
            ocio_lut: None,
        }
    }

    pub fn max_texture_size(&self) -> u32 {
        self.max_tex
    }

    /// Forces the YCbCr → RGB path of later uploads (0 CPU, 1 GPU shader; `None`: the path
    /// selector decides). For tests that must cover both paths whatever the selector picks.
    pub fn force_colour_path(&mut self, path: Option<usize>) {
        self.colour_force = path.map(|p| p.min(1));
    }

    /// Sets the peak the output can show, in units of SDR reference white: 1 for an SDR
    /// surface, the display's headroom for an HDR (extended-range) surface. HDR content
    /// above it is tone-mapped ([`hdr::tone_map_scale`]); gain maps are weighted for it.
    pub fn set_display_peak(&mut self, peak: f32) {
        self.display_peak = if peak.is_finite() {
            peak.clamp(1.0, 64.0)
        } else {
            1.0
        };
    }

    pub fn display_peak(&self) -> f32 {
        self.display_peak
    }

    /// Attaches (or with `None` removes) a gain map: the RGBA8 map raster is uploaded as is
    /// and applied at draw time, weighted by the display peak.
    pub fn set_gain_map(&mut self, gpu: &Gpu, img: &mut GpuImage, gm: Option<&GainMap>) {
        let old = img.gain_map.take();
        if let Some(o) = old {
            self.recycle(o.texture);
        }
        img.revision += 1;
        let Some(gm) = gm else { return };
        let r = &gm.map;
        if r.layout != PixelLayout::Rgba
            || r.planes.len() != 1
            || r.width == 0
            || r.height == 0
            || !gm.params.is_sane()
        {
            return;
        }
        let p = &r.planes[0];
        let (w, h) = (p.width.min(self.max_tex), p.height.min(self.max_tex));
        if p.stride < w as usize * 4 || p.data.len() < p.stride * h as usize {
            return;
        }
        let texture = self.texture(
            &gpu.device,
            "gain map",
            w,
            h,
            1,
            wgpu::TextureFormat::Rgba8Unorm,
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        );
        gpu.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &p.data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(p.stride as u32),
                rows_per_image: Some(h),
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        let view = texture.create_view(&Default::default());
        img.gain_map = Some(GainTexture {
            texture,
            view,
            params: gm.params,
        });
    }

    /// The half-float convert and mip pipelines (compiled on the first HDR image).
    fn hdr_pipes(&mut self, gpu: &Gpu) -> &(wgpu::RenderPipeline, wgpu::RenderPipeline) {
        self.hdr_pipes.get_or_insert_with(|| {
            let d = &gpu.device;
            let module = |label: &str, src: &str| {
                specular_gfx::shader(d, label, &format!("{FULLSCREEN_WGSL}\n{src}"))
            };
            let cmod = module("convert hdr", include_str!("shaders/convert.wgsl"));
            let mmod = module("mip hdr", &mip_source(true));
            let convert = fullscreen_pipeline(
                gpu,
                "convert hdr",
                &cmod,
                "fs_convert",
                &[Some(&self.convert_bgl)],
                STORE_HDR,
                None,
            );
            let mip = fullscreen_pipeline(
                gpu,
                "mip hdr",
                &mmod,
                "fs_mip",
                &[Some(&self.mip_bgl)],
                STORE_HDR,
                None,
            );
            (convert, mip)
        })
    }

    pub fn tile_capacity(&self) -> u32 {
        self.pool.capacity()
    }

    /// VRAM held by detail tiles.
    pub fn tile_bytes(&self) -> u64 {
        let _ = (&self.tile_tex, self.tiles16.as_ref().map(|t| &t.tex));
        let layer = SLOT as u64 * SLOT as u64;
        self.pool.part_capacity(false) as u64 * layer * 4
            + self.pool.part_capacity(true) as u64 * layer * 8
    }

    /// Makes the 16-bit tile array on the first image with wide samples: the tile budget is
    /// then split evenly, the 8-bit array remade at half its layers (its tiles are dropped and
    /// come back on the next frames) and the 16-bit one given the other half.
    fn ensure_tiles16(&mut self, gpu: &Gpu) {
        if self.tiles16.is_some() {
            return;
        }
        use wgpu::TextureViewDimension as Dim;
        let half = self.tile_budget / 2;
        let (cap8, tex8) = tile_array(gpu, tile_capacity(gpu, half), STORE);
        // 8 bytes a texel: the layers of a quarter of the budget at 4.
        let (cap16, tex16) = tile_array(gpu, tile_capacity(gpu, half / 2), STORE16);
        let layers = |t: &wgpu::Texture, n: u32| -> Vec<wgpu::TextureView> {
            (0..n)
                .map(|i| {
                    t.create_view(&wgpu::TextureViewDescriptor {
                        dimension: Some(Dim::D2),
                        base_array_layer: i,
                        array_layer_count: Some(1),
                        ..Default::default()
                    })
                })
                .collect()
        };
        let array = wgpu::TextureViewDescriptor {
            dimension: Some(Dim::D2Array),
            ..Default::default()
        };
        self.tile_view = sampled_view(&tex8, array.clone());
        self.tile_layer_views = layers(&tex8, cap8);
        self.tile_tex = tex8;
        self.tiles16 = Some(Tiles16 {
            view: tex16.create_view(&array),
            layer_views: layers(&tex16, cap16),
            tex: tex16,
        });
        self.pool.split(cap8, cap16);
        let cmod = specular_gfx::shader(
            &gpu.device,
            "convert codes16",
            &format!(
                "{FULLSCREEN_WGSL}\n{}",
                include_str!("shaders/convert.wgsl")
            ),
        );
        self.convert16_pipe = Some(fullscreen_pipeline(
            gpu,
            "convert codes16",
            &cmod,
            "fs_convert_codes16",
            &[Some(&self.convert_bgl)],
            STORE16,
            None,
        ));
        specular_log::debug!(
            target: specular_log::GPU,
            "tile arrays: {cap8} 8-bit and {cap16} 16-bit layers"
        );
    }

    /// A new image. `lut` comes from [`crate::icc::build_lut`] (built on a worker thread).
    pub fn create_image(
        &self,
        gpu: &Gpu,
        id: u64,
        width: u32,
        height: u32,
        lut: Option<&SourceLut>,
    ) -> GpuImage {
        let lut = lut.map(|l| GpuLut::new(gpu, l));
        GpuImage {
            id,
            width: width.max(1),
            height: height.max(1),
            lut,
            storage: self.storage,
            overview: None,
            revision: 1,
            content_peak: 1.0,
            exposure: 1.0,
            hdr: false,
            gain_map: None,
            values: None,
            relight: None,
            range: None,
            max_level: MAX_LEVEL,
            wide_tiles: false,
            tile_uploads: [0; LEVEL_CAP as usize + 1],
        }
    }

    /// Forgets an image's tiles.
    pub fn release_image(&mut self, id: u64) {
        self.pool.evict_image(id);
    }

    /// A texture from the spares if one matches, else a new one.
    #[allow(clippy::too_many_arguments)]
    fn texture(
        &mut self,
        d: &wgpu::Device,
        label: &str,
        w: u32,
        h: u32,
        mips: u32,
        format: wgpu::TextureFormat,
        usage: wgpu::TextureUsages,
    ) -> wgpu::Texture {
        let (w, h) = (w.max(1), h.max(1));
        match self.spare.iter().rposition(|t| {
            (
                t.width(),
                t.height(),
                t.mip_level_count(),
                t.format(),
                t.usage(),
            ) == (w, h, mips, format, usage)
        }) {
            Some(i) => self.spare.swap_remove(i),
            None => texture_2d(d, label, w, h, 1, mips, format, usage),
        }
    }

    /// Keeps `t` for reuse, dropping the oldest spares beyond [`SPARE_BYTES`]. Queue order
    /// makes this safe: a later write lands after every earlier submission that reads it.
    fn recycle(&mut self, t: wgpu::Texture) {
        if t.depth_or_array_layers() != 1 || texture_bytes(&t) > SPARE_BYTES / 2 {
            return;
        }
        self.spare.push(t);
        while self.spare.iter().map(texture_bytes).sum::<u64>() > SPARE_BYTES {
            self.spare.remove(0);
        }
    }

    /// Hands an evicted image's textures back for reuse.
    pub fn recycle_image(&mut self, mut img: GpuImage) {
        self.release_image(img.id);
        drop(img.take_relight_texture());
        if let Some(o) = img.overview {
            self.recycle(o.texture);
        }
        if let Some(v) = img.values {
            for t in v.into_textures() {
                self.recycle(t);
            }
        }
    }

    fn recycle_planes(&mut self, planes: Planes) {
        drop(planes.views);
        for t in planes.textures {
            self.recycle(t);
        }
    }

    fn upload_planes(&mut self, gpu: &Gpu, r: &Raster) -> Planes {
        if let PixelLayout::Yuv(e) = r.layout
            && e.bits > 8
        {
            return self.upload_yuv16(gpu, r);
        }
        let mut textures = Vec::with_capacity(r.planes.len());
        let views = r
            .planes
            .iter()
            .map(|p| {
                let format = match r.layout {
                    PixelLayout::Rgba => wgpu::TextureFormat::Rgba8Unorm,
                    PixelLayout::Rgba16(_) => wgpu::TextureFormat::Rgba16Uint,
                    PixelLayout::Rgba16F(_) => wgpu::TextureFormat::Rgba16Float,
                    PixelLayout::Gray16 => wgpu::TextureFormat::R16Uint,
                    PixelLayout::GrayAlpha16 => wgpu::TextureFormat::Rg16Uint,
                    _ => wgpu::TextureFormat::R8Unorm,
                };
                let t = self.texture(
                    &gpu.device,
                    "plane",
                    p.width,
                    p.height,
                    1,
                    format,
                    wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                );
                if p.width > 0 && p.height > 0 {
                    gpu.queue.write_texture(
                        wgpu::TexelCopyTextureInfo {
                            texture: &t,
                            mip_level: 0,
                            origin: wgpu::Origin3d::ZERO,
                            aspect: wgpu::TextureAspect::All,
                        },
                        &p.data,
                        wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(p.stride as u32),
                            rows_per_image: Some(p.height),
                        },
                        wgpu::Extent3d {
                            width: p.width,
                            height: p.height,
                            depth_or_array_layers: 1,
                        },
                    );
                }
                let v = t.create_view(&Default::default());
                textures.push(t);
                v
            })
            .collect();
        Planes { views, textures }
    }

    /// Above 8 bits a video frame's three planes go into one R16Uint texture, stacked top to
    /// bottom (convert.wgsl `yuv_code` finds them), so the convert pass keeps its one integer
    /// binding.
    fn upload_yuv16(&mut self, gpu: &Gpu, r: &Raster) -> Planes {
        let width = r.planes.iter().map(|p| p.width).max().unwrap_or(0);
        let height: u32 = r.planes.iter().map(|p| p.height).sum();
        let t = self.texture(
            &gpu.device,
            "yuv16",
            width.max(1),
            height.max(1),
            1,
            wgpu::TextureFormat::R16Uint,
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        );
        let mut y0 = 0;
        for p in &r.planes {
            if p.width > 0 && p.height > 0 {
                gpu.queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &t,
                        mip_level: 0,
                        origin: wgpu::Origin3d { x: 0, y: y0, z: 0 },
                        aspect: wgpu::TextureAspect::All,
                    },
                    &p.data,
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(p.stride as u32),
                        rows_per_image: Some(p.height),
                    },
                    wgpu::Extent3d {
                        width: p.width,
                        height: p.height,
                        depth_or_array_layers: 1,
                    },
                );
            }
            y0 += p.height;
        }
        let v = t.create_view(&Default::default());
        Planes {
            views: vec![v],
            textures: vec![t],
        }
    }

    /// Converts `r` into `dst`: a half-float overview ([`STORE_HDR`]), an 8-bit sRGB-encoded
    /// overview or tile layer ([`STORE`]) or a 16-bit tile layer ([`STORE16`]), as `target`
    /// says.
    #[allow(clippy::too_many_arguments)]
    fn convert(
        &self,
        gpu: &Gpu,
        enc: &mut wgpu::CommandEncoder,
        planes: &Planes,
        r: &Raster,
        img: &GpuImage,
        dst: &wgpu::TextureView,
        dst_size: (u32, u32),
        dst_origin: (f32, f32),
        level_size: (u32, u32),
        step: f32,
        target: Dst,
    ) {
        let div = |i: usize| {
            r.planes
                .get(i)
                .map(|p| [p.h_div.max(1) as f32, p.v_div.max(1) as f32])
                .unwrap_or([1.0, 1.0])
        };
        let (d1, d2) = (div(1), div(2));
        let (range_fields, range_buf) = self.range_binding(img, r.layout);
        let u = CvtUniform {
            v: [
                dst_origin.0,
                dst_origin.1,
                r.origin.0 as f32,
                r.origin.1 as f32,
                r.width.max(1) as f32,
                r.height.max(1) as f32,
                level_size.0 as f32,
                level_size.1 as f32,
                d1[0],
                d1[1],
                d2[0],
                d2[1],
            ],
            layout: match r.layout {
                PixelLayout::Gray => 0,
                PixelLayout::YCbCr => 1,
                PixelLayout::Rgb => 2,
                PixelLayout::Rgba => 3,
                PixelLayout::Rgba16(_) => 4,
                PixelLayout::Rgba16F(_) => 5,
                PixelLayout::Gray16 => 6,
                PixelLayout::GrayAlpha16 => 7,
                PixelLayout::Yuv(_) => 8,
            },
            // Source transforms apply to 8-bit samples and 16-bit sRGB-coded BT.709 ones
            // (PNG with an iCCP, 16-bit grey included); HDR transfers and float samples carry
            // their own space.
            has_lut: match (&img.lut, r.layout) {
                (
                    Some(l),
                    PixelLayout::Rgba16(e) | PixelLayout::Yuv(YuvEncoding { colour: e, .. }),
                ) if e.transfer == Transfer::Srgb && hdr::primaries_code(e.primaries) == 0 => {
                    l.mode
                }
                (
                    Some(_),
                    PixelLayout::Rgba16(_) | PixelLayout::Rgba16F(_) | PixelLayout::Yuv(_),
                )
                | (None, _) => 0,
                (Some(l), _) => l.mode,
            },
            lut_size: img.lut.as_ref().map_or(1.0, |l| l.size as f32),
            step,
            transfer: match r.layout {
                PixelLayout::Rgba16(e) | PixelLayout::Yuv(YuvEncoding { colour: e, .. }) => {
                    match e.transfer {
                        Transfer::Srgb => 0,
                        Transfer::Linear => 1,
                        Transfer::Pq => 2,
                        Transfer::Hlg => 3,
                    }
                }
                _ => 1,
            },
            primaries: match r.layout {
                PixelLayout::Rgba16(e) | PixelLayout::Yuv(YuvEncoding { colour: e, .. }) => {
                    hdr::primaries_code(e.primaries) as u32
                }
                PixelLayout::Rgba16F(p) => hdr::primaries_code(p) as u32,
                _ => 0,
            },
            range_on: match r.layout {
                PixelLayout::Yuv(e) => {
                    u32::from(e.full_range)
                        | u32::from(e.cosited_x) << 1
                        | e.matrix.code() << 2
                        | u32::from(e.cosited_y) << 5
                        | u32::from(e.bottom_y && !e.cosited_y) << 6
                        | u32::from(e.right_x && !e.cosited_x) << 7
                        | u32::from(e.alpha) << 8
                }
                _ => range_fields[0],
            },
            range_bits: match r.layout {
                PixelLayout::Yuv(e) => u32::from(e.bits),
                _ => range_fields[1],
            },
            src: img.lut.as_ref().map_or(colour::IDENTITY_ROWS, |l| l.matrix),
            store: {
                let mut m = colour::rows(&img.storage.to);
                if matches!(r.layout, PixelLayout::Rgba16F(_)) && img.exposure != 1.0 {
                    for v in &mut m {
                        *v *= img.exposure;
                    }
                }
                m
            },
        };
        let ub = gpu.device.buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("convert"),
            contents: bytemuck::bytes_of(&u),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let wide16 = matches!(
            r.layout,
            PixelLayout::Rgba16(_) | PixelLayout::Gray16 | PixelLayout::GrayAlpha16
        ) || matches!(r.layout, PixelLayout::Yuv(e) if e.bits > 8);
        let pv = |i: usize| match planes.views.get(i) {
            Some(v) if !wide16 => v,
            _ => &self.dummy_plane,
        };
        let p16 = match planes.views.first() {
            Some(v) if wide16 => v,
            _ => &self.dummy_u16,
        };
        let lut = img.lut.as_ref().map(|l| &l.view).unwrap_or(&self.dummy_lut);
        let bind = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("convert"),
            layout: &self.convert_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: ub.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(pv(0)),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(pv(1)),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(pv(2)),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::TextureView(lut),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::TextureView(p16),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: range_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: wgpu::BindingResource::TextureView(match r.layout {
                        PixelLayout::Yuv(_) => pv(3),
                        _ => &self.dummy_plane,
                    }),
                },
            ],
        });
        let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("convert"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: dst,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
        rp.set_viewport(0.0, 0.0, dst_size.0 as f32, dst_size.1 as f32, 0.0, 1.0);
        let pipe = match (target, &self.hdr_pipes, &self.convert16_pipe) {
            (Dst::Half, Some((c, _)), _) => c,
            (Dst::Codes16, _, Some(c)) => c,
            _ => &self.convert_pipe,
        };
        rp.set_pipeline(pipe);
        rp.set_bind_group(0, &bind, &[]);
        rp.draw(0..3, 0..1);
    }

    /// Records the reduction of mip 0 of `texture` into its other `mips - 1` levels.
    fn record_mips(
        &self,
        gpu: &Gpu,
        enc: &mut wgpu::CommandEncoder,
        texture: &wgpu::Texture,
        mips: u32,
        wide: bool,
    ) {
        let mip_view = |m: u32| {
            texture.create_view(&wgpu::TextureViewDescriptor {
                base_mip_level: m,
                mip_level_count: Some(1),
                ..Default::default()
            })
        };
        let mut prev = mip_view(0);
        for m in 1..mips {
            let v = mip_view(m);
            let bind = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("mip"),
                layout: &self.mip_bgl,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&prev),
                }],
            });
            let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("mip"),
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
            let pipe = match (&self.hdr_pipes, wide) {
                (Some((_, m)), true) => m,
                _ => &self.mip_pipe,
            };
            rp.set_pipeline(pipe);
            rp.set_bind_group(0, &bind, &[]);
            rp.draw(0..3, 0..1);
            drop(rp);
            prev = v;
        }
    }

    /// Replaces the overview with a whole-image raster (embedded thumbnail or scaled decode).
    pub fn upload_overview(&mut self, gpu: &Gpu, img: &mut GpuImage, r: &Raster) {
        let t0 = std::time::Instant::now();
        // Float data far above reference white (a 0–255 PFM) is stored exposed down; the
        // overview sets it for the image's tiles too.
        img.exposure = hdr::float_exposure(r).0;
        let colour = self.colour_path(gpu, r);
        // Timed from here: the first call may run the correctness gate.
        let t_colour = std::time::Instant::now();
        let rgb = colour
            .filter(|c| c.engine == Engine::Cpu)
            .and_then(|_| crate::cpu_colour::ycbcr_to_rgb(r));
        let r = rgb.as_ref().unwrap_or(r);
        if r.width == 0 || r.height == 0 || r.planes.is_empty() {
            return;
        }
        let step = (r.width.max(r.height) as f32 / self.max_tex as f32)
            .ceil()
            .max(1.0);
        let (w, h) = (
            ((r.width as f32) / step).ceil() as u32,
            ((r.height as f32) / step).ceil() as u32,
        );
        let mips = 32 - w.max(h).leading_zeros();
        let wide = r.layout.has_wide_samples() || r.layout.is_hdr();
        if wide {
            self.hdr_pipes(gpu);
        }
        let texture = self.texture(
            &gpu.device,
            "overview",
            w,
            h,
            mips,
            if wide { STORE_HDR } else { STORE },
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
        );
        let planes = self.upload_planes(gpu, r);
        let mut enc = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("overview"),
            });
        let mip_view = |m: u32| {
            texture.create_view(&wgpu::TextureViewDescriptor {
                base_mip_level: m,
                mip_level_count: Some(1),
                ..Default::default()
            })
        };
        let v0 = mip_view(0);
        let ranged = self.record_range(
            gpu,
            &mut enc,
            img,
            r.layout,
            planes.views.first(),
            plane_size(r),
        );
        self.convert(
            gpu,
            &mut enc,
            &planes,
            r,
            img,
            &v0,
            (w, h),
            (0.0, 0.0),
            (r.width, r.height),
            step,
            if wide { Dst::Half } else { Dst::Srgb8 },
        );
        drop(v0);
        self.record_mips(gpu, &mut enc, &texture, mips, wide);
        gpu.queue.submit([enc.finish()]);
        if ranged {
            Self::read_range(img);
        }
        self.recycle_planes(planes);
        let view = sampled_view(&texture, Default::default());
        let old = img.overview.replace(Overview {
            texture,
            view,
            width: w,
            height: h,
            mips,
            source_scale: r.scale,
        });
        if let Some(o) = old {
            self.recycle(o.texture);
        }
        // Wide SDR samples (16-bit sRGB-coded: PNG, JPEG 2000) keep the half-float overview's
        // precision but stay within reference white like 8-bit ones, and are tiled.
        img.hdr = r.layout.is_hdr();
        img.content_peak = match r.layout {
            // Float: its own peak after the exposure set before converting.
            PixelLayout::Rgba16F(_) => (wide_peak(r) * img.exposure).max(1.0),
            _ if img.hdr => wide_peak(r),
            _ => 1.0,
        };
        self.set_wide_tiles(gpu, img, r.layout);
        img.revision += 1;
        self.last_upload_ms = t0.elapsed().as_secs_f64() * 1000.0;
        self.colour_done(colour.as_ref(), t_colour);
    }

    /// Converts the given tiles of `level` out of a region raster (which must cover them,
    /// borders included). Returns how many were stored.
    pub fn upload_tiles(
        &mut self,
        gpu: &Gpu,
        img: &mut GpuImage,
        r: &Raster,
        level: u8,
        tiles: &[(u32, u32)],
    ) -> usize {
        let t0 = std::time::Instant::now();
        let colour = self.colour_path(gpu, r);
        // Timed from here: the first call may run the correctness gate.
        let t_colour = std::time::Instant::now();
        let rgb = colour
            .filter(|c| c.engine == Engine::Cpu)
            .and_then(|_| crate::cpu_colour::ycbcr_to_rgb(r));
        let r = rgb.as_ref().unwrap_or(r);
        // HDR images live in their (half-float) overview only. Wide SDR samples are converted
        // (and colour managed) at full precision and rounded once, to 16-bit codes.
        if tiles.is_empty() || r.planes.is_empty() || r.layout.is_hdr() {
            return 0;
        }
        self.set_wide_tiles(gpu, img, r.layout);
        let wide = img.wide_tiles;
        let planes = self.upload_planes(gpu, r);
        let ls = tiles::level_size(img.width, img.height, level);
        let mut enc = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("tiles"),
            });
        let ranged = self.record_range(
            gpu,
            &mut enc,
            img,
            r.layout,
            planes.views.first(),
            plane_size(r),
        );
        let mut n = 0;
        for &(tx, ty) in tiles {
            let key = TileKey {
                image: img.id,
                level,
                tx,
                ty,
            };
            let Some(slot) = self.pool.alloc_in(key, wide) else {
                break;
            };
            let origin = (
                (tx * TILE) as f32 - BORDER as f32,
                (ty * TILE) as f32 - BORDER as f32,
            );
            let (layer, target) = match &self.tiles16 {
                Some(t) if wide => (
                    &t.layer_views[(slot - self.pool.wide_from()) as usize],
                    Dst::Codes16,
                ),
                _ => (&self.tile_layer_views[slot as usize], Dst::Srgb8),
            };
            self.convert(
                gpu,
                &mut enc,
                &planes,
                r,
                img,
                layer,
                (SLOT, SLOT),
                origin,
                ls,
                1.0,
                target,
            );
            n += 1;
        }
        gpu.queue.submit([enc.finish()]);
        if ranged {
            Self::read_range(img);
        }
        self.recycle_planes(planes);
        img.revision += 1;
        img.tile_uploads[(level as usize).min(LEVEL_CAP as usize)] += 1;
        self.last_upload_ms = t0.elapsed().as_secs_f64() * 1000.0;
        self.colour_done(colour.as_ref(), t_colour);
        n
    }

    /// Whether `img`'s detail tiles are 16-bit: samples of `layout` wider than 8 bits (SDR;
    /// HDR images have no tiles). A change forgets the image's tiles.
    fn set_wide_tiles(&mut self, gpu: &Gpu, img: &mut GpuImage, layout: PixelLayout) {
        let wide = layout.has_wide_samples() && !layout.is_hdr();
        if wide {
            self.ensure_tiles16(gpu);
        }
        if wide != img.wide_tiles {
            self.pool.evict_image(img.id);
            img.wide_tiles = wide;
        }
    }

    /// Starts a frame for the tile LRU and returns the tiles the view (at its animation
    /// target) still needs: (level, tiles nearest the centre first). Resident tiles are
    /// marked used.
    pub fn wanted_tiles(&mut self, img: &GpuImage, view: &View) -> Option<(u8, Vec<(u32, u32)>)> {
        self.pool.next_frame();
        self.wanted_tiles_also(img, view)
    }

    /// Like [`ImageRenderer::wanted_tiles`] for a second view drawn in the same frame (the
    /// loupe): marks its tiles used without starting a new LRU frame.
    pub fn wanted_tiles_also(
        &mut self,
        img: &GpuImage,
        view: &View,
    ) -> Option<(u8, Vec<(u32, u32)>)> {
        let s = view
            .target_scale()
            .min(view.scale().max(view.target_scale()));
        let level = tiles::level_for_scale(s, img.max_level);
        let lscale = 1.0 / (1u32 << level) as f32;
        // Relit images draw from their overview alone (tiles would hold the decoder's light).
        if lscale <= img.overview_scale() * 1.001 || img.hdr || img.relight.is_some() {
            return None;
        }
        let r = view.visible_stored(true);
        let all = tiles::tiles_covering(img.width, img.height, level, r, 0);
        // Keep the coarser tiles that cover the view too (fallback while zooming).
        for l in level + 1..=img.max_level {
            for (tx, ty) in tiles::tiles_covering(img.width, img.height, l, r, 0) {
                self.pool.touch(&TileKey {
                    image: img.id,
                    level: l,
                    tx,
                    ty,
                });
            }
        }
        let mut missing = Vec::new();
        for (tx, ty) in all {
            if self
                .pool
                .touch(&TileKey {
                    image: img.id,
                    level,
                    tx,
                    ty,
                })
                .is_none()
            {
                missing.push((tx, ty));
            }
        }
        let cap = self.pool.part_capacity(img.wide_tiles);
        if missing.len() as u32 > cap {
            missing.truncate(cap as usize);
        }
        Some((level, missing))
    }

    /// The level `view` draws `img` from (it falls back to coarser ones), as the shader picks
    /// it.
    pub fn view_level(img: &GpuImage, view: &View) -> u8 {
        tiles::level_for_scale(view.scale(), img.max_level)
    }

    /// Detail tiles of `img` resident at `level` and coarser: with
    /// [`GpuImage::revision_from`], what a view drawn at `level` depends on (an eviction
    /// changes the count).
    pub fn tiles_from(&self, img: &GpuImage, level: u8) -> usize {
        self.pool.tiles_of(img.id).filter(|t| t.0 >= level).count()
    }

    fn tile_table(&self, img: &GpuImage) -> (Vec<u32>, u32) {
        let levels = img.max_level as usize + 1;
        let mut table = vec![0u32; levels * 4];
        let mut offset = table.len() as u32;
        for l in 0..levels {
            let (c, r) = tiles::level_grid(img.width, img.height, l as u8);
            table[l * 4] = c;
            table[l * 4 + 1] = r;
            table[l * 4 + 2] = offset;
            offset += c * r;
        }
        table.resize(offset as usize, 0);
        let mut present = 0u32;
        // 16-bit tiles: layers of the 16-bit array.
        let base = if img.wide_tiles {
            self.pool.wide_from()
        } else {
            0
        };
        for (l, tx, ty, slot) in self.pool.tiles_of(img.id) {
            let l = l as usize;
            if l >= levels {
                continue;
            }
            let slot = slot - base;
            let i = table[l * 4 + 2] + ty * table[l * 4] + tx;
            table[i as usize] = slot + 1;
            present |= 1 << l;
        }
        // The shaders read the levels present (17 bits) and the coarsest level above them.
        (table, present | u32::from(img.max_level) << 17)
    }

    /// Draws the image for `view` into `target` (an [`HDR`] texture of `size`), clearing it
    /// to the canvas colour first. `view.viewport` is in target pixels.
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &mut self,
        gpu: &Gpu,
        enc: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: (u32, u32),
        img: Option<&GpuImage>,
        view: &View,
        opts: &RenderOptions,
    ) {
        let clear = wgpu::Color {
            r: opts.canvas[0] as f64,
            g: opts.canvas[1] as f64,
            b: opts.canvas[2] as f64,
            a: opts.canvas[3] as f64,
        };
        self.draw(
            gpu,
            enc,
            target,
            size,
            img,
            view,
            opts,
            wgpu::LoadOp::Clear(clear),
            None,
        );
    }

    /// Draws `view` of the image over what `target` already holds, inside `view.viewport`
    /// (the navigator, the loupe). `circle` (centre, radius in target pixels) limits the
    /// drawing to a disc.
    #[allow(clippy::too_many_arguments)]
    pub fn render_inset(
        &mut self,
        gpu: &Gpu,
        enc: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: (u32, u32),
        img: &GpuImage,
        view: &View,
        opts: &RenderOptions,
        circle: Option<((f32, f32), f32)>,
    ) {
        self.draw(
            gpu,
            enc,
            target,
            size,
            Some(img),
            view,
            opts,
            wgpu::LoadOp::Load,
            circle,
        );
    }

    /// The HDR part of the image uniform: gain-map weight for the display peak and the
    /// tone-mapping switch.
    fn hdr_uniform(&self, img: &GpuImage) -> ImgUniform {
        let mut u = ImgUniform::zeroed();
        u.from_store = colour::rows(&img.storage.from);
        let display = self.display_peak;
        let mut content = img.content_peak;
        let mut flags = 0u32;
        if let Some(g) = &img.gain_map {
            let p = &g.params;
            let w = p.weight(display);
            content = p.peak(content, w);
            flags |= 1;
            if p.converts() {
                flags |= 4;
                let flat =
                    |m: [[f32; 3]; 3]| -> [f32; 9] { std::array::from_fn(|i| m[i / 3][i % 3]) };
                u.gm_to_app = colour::rows(&flat(hdr::from_bt709(p.space)));
                u.gm_from_app = colour::rows(&flat(hdr::to_bt709(p.space)));
            }
            u.gm_min = [p.gain_min[0], p.gain_min[1], p.gain_min[2], w];
            u.gm_max = [p.gain_max[0], p.gain_max[1], p.gain_max[2], 0.0];
            u.gm_gamma = [p.gamma[0], p.gamma[1], p.gamma[2], 0.0];
            u.gm_base = [p.base_offset[0], p.base_offset[1], p.base_offset[2], 0.0];
            u.gm_alt = [p.alt_offset[0], p.alt_offset[1], p.alt_offset[2], 0.0];
        }
        if content > display * 1.001 {
            flags |= 2;
        }
        u.hdr = [
            if img.hdr { 65504.0 } else { 1.0 },
            content,
            display,
            flags as f32,
        ];
        u
    }

    #[allow(clippy::too_many_arguments)]
    fn draw(
        &mut self,
        gpu: &Gpu,
        enc: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: (u32, u32),
        img: Option<&GpuImage>,
        view: &View,
        opts: &RenderOptions,
        load: wgpu::LoadOp<wgpu::Color>,
        circle: Option<((f32, f32), f32)>,
    ) {
        let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("image"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
        let Some(img) = img else { return };
        if img.draws_values() {
            let vp = view.viewport;
            let x0 = vp.x.max(0.0).floor() as u32;
            let y0 = vp.y.max(0.0).floor() as u32;
            let x1 = ((vp.x + vp.w).ceil() as u32).min(size.0);
            let y1 = ((vp.y + vp.h).ceil() as u32).min(size.1);
            if x1 > x0 && y1 > y0 {
                rp.set_scissor_rect(x0, y0, x1 - x0, y1 - y0);
                self.draw_values(gpu, &mut rp, img, view, opts, circle);
            }
            return;
        }
        if img.overview.is_none() && self.pool.tiles_of(img.id).next().is_none() {
            return;
        }
        let vp = view.viewport;
        let bind = self.bind_image(gpu, img, view, opts, circle);
        let x0 = vp.x.max(0.0).floor() as u32;
        let y0 = vp.y.max(0.0).floor() as u32;
        let x1 = ((vp.x + vp.w).ceil() as u32).min(size.0);
        let y1 = ((vp.y + vp.h).ceil() as u32).min(size.1);
        if x1 <= x0 || y1 <= y0 {
            return;
        }
        rp.set_scissor_rect(x0, y0, x1 - x0, y1 - y0);
        rp.set_pipeline(&self.image_pipe);
        rp.set_bind_group(0, &bind, &[]);
        rp.draw(0..3, 0..1);
    }

    /// The image bind group (uniform, tiles, overview, tile table) for drawing `view`; the
    /// inspection passes (`inspect`) bind it too, so they sample exactly what is drawn.
    pub(crate) fn bind_image(
        &self,
        gpu: &Gpu,
        img: &GpuImage,
        view: &View,
        opts: &RenderOptions,
        circle: Option<((f32, f32), f32)>,
    ) -> wgpu::BindGroup {
        let a = view.stored_from_screen();
        let ov = img.overview.as_ref();
        let vp = view.viewport;
        let u = ImgUniform {
            m0: [a.m[0], a.m[1], a.m[2], 0.0],
            m1: [a.m[3], a.m[4], a.m[5], 0.0],
            img: [
                img.width as f32,
                img.height as f32,
                view.scale(),
                // 0 bilinear, 1 Lanczos, 2 Lanczos without the anti-ringing clamp.
                match (opts.hq, opts.anti_ringing) {
                    (false, _) => 0.0,
                    (true, true) => 1.0,
                    (true, false) => 2.0,
                },
            ],
            ov: ov
                .map(|o| [o.width as f32, o.height as f32, o.mips as f32, 1.0])
                .unwrap_or([1.0, 1.0, 1.0, 0.0]),
            canvas: opts.canvas,
            grid: opts.grid.unwrap_or([0.0; 4]),
            checker_a: opts.checker_a,
            checker_b: opts.checker_b,
            misc: [
                opts.checker_cell.max(1.0),
                opts.nearest_from,
                opts.grid_from,
                0.0,
            ],
            clip: [vp.x, vp.y, vp.x + vp.w, vp.y + vp.h],
            mask: circle.map_or([0.0; 4], |(c, r)| [c.0, c.1, r, 1.0]),
            chan: [
                opts.channel.shader_mode() as f32,
                ov.map_or(0.0, |o| f32::from(u8::from(o.texture.format() == STORE))),
                f32::from(u8::from(img.wide_tiles)),
                0.0,
            ],
            ..self.hdr_uniform(img)
        };
        let (cc, fc, hi, lo) = inspect_rows(opts);
        let (st, sr, sc, sv) = opts
            .stain
            .map_or(([0.0; 12], [0.0; 12], [0.0; 12], [0.0; 4]), |s| s.rows());
        let oc = match (&opts.ocio, &self.ocio_lut) {
            (Some(o), Some((id, _))) if *id == o.id => [1.0, o.log2_lo, o.log2_hi, o.size as f32],
            _ => [0.0; 4],
        };
        let (pa, pg) = opts
            .picture
            .filter(|p| !p.is_neutral())
            .map_or(([0.0; 12], [0.0; 4]), |p| p.uniform());
        let u = ImgUniform {
            pa,
            pg,
            cc,
            fc,
            hi,
            lo,
            st,
            sr,
            sc,
            sv,
            oc,
            ..u
        };
        let (table, present) = self.tile_table(img);
        let mut u = u;
        u.misc[3] = present as f32;
        let ub = gpu.device.buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("image"),
            contents: bytemuck::bytes_of(&u),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let tb = gpu.device.buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("tile table"),
            contents: bytemuck::cast_slice(&table),
            usage: wgpu::BufferUsages::STORAGE,
        });
        gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("image"),
            layout: &self.image_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: ub.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&self.tile_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(
                        ov.map(|o| &o.view).unwrap_or(&self.dummy_ov),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: tb.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::TextureView(
                        img.gain_map
                            .as_ref()
                            .map(|g| &g.view)
                            .unwrap_or(&self.dummy_gain),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: self.cmaps.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: wgpu::BindingResource::TextureView(
                        self.tiles16
                            .as_ref()
                            .map_or(&self.dummy_tiles16, |t| &t.view),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: wgpu::BindingResource::TextureView(&self.decode8),
                },
                wgpu::BindGroupEntry {
                    binding: 9,
                    resource: wgpu::BindingResource::TextureView(
                        match (&self.ocio_lut, &opts.ocio) {
                            (Some((id, v)), Some(o)) if *id == o.id => v,
                            _ => &self.dummy_lut,
                        },
                    ),
                },
            ],
        })
    }

    /// Uploads an OCIO table ([`OcioLut`]) of `size`³ texels under `id`, replacing the
    /// previous one (a 65³ table is about 2 MB).
    pub fn upload_ocio(&mut self, gpu: &Gpu, id: u64, size: u32, texels: &[[f32; 4]]) {
        if self.ocio_lut.as_ref().is_some_and(|(i, _)| *i == id) {
            return;
        }
        let n = size.max(1);
        if texels.len() != (n * n * n) as usize {
            return;
        }
        let tex = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("ocio lut"),
            size: wgpu::Extent3d {
                width: n,
                height: n,
                depth_or_array_layers: n,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D3,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let data: Vec<u8> = texels
            .iter()
            .flat_map(|t| t.iter().flat_map(|&v| hdr::f32_to_f16(v).to_le_bytes()))
            .collect();
        gpu.queue.write_texture(
            tex.as_image_copy(),
            &data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(n * 8),
                rows_per_image: Some(n),
            },
            wgpu::Extent3d {
                width: n,
                height: n,
                depth_or_array_layers: n,
            },
        );
        self.ocio_lut = Some((id, tex.create_view(&Default::default())));
    }
}

/// The mip shader source; `hdr` keeps values above 1 (half-float overviews).
fn mip_source(hdr: bool) -> String {
    format!(
        "const HDR_STORE: bool = {hdr};\n{}",
        include_str!("shaders/mip.wgsl")
    )
}

/// Brightest value of a wide raster in units of reference white. The transfers are
/// monotonic, so the largest code of any colour channel decides; sampling every other row
/// keeps this a few milliseconds even for large images.
fn wide_peak(r: &Raster) -> f32 {
    if let PixelLayout::Yuv(e) = r.layout {
        return yuv_peak(r, e);
    }
    let Some(p) = r.planes.first() else {
        return 1.0;
    };
    let row = (p.width as usize * 8).min(p.stride);
    let mut max = 0u16;
    match r.layout {
        PixelLayout::Rgba16(_) => {
            for line in p.data.chunks(p.stride).step_by(2) {
                for px in line[..row.min(line.len())].chunks_exact(8) {
                    for c in 0..3 {
                        max = max.max(u16::from_ne_bytes([px[2 * c], px[2 * c + 1]]));
                    }
                }
            }
        }
        PixelLayout::Rgba16F(_) => {
            for line in p.data.chunks(p.stride).step_by(2) {
                for px in line[..row.min(line.len())].chunks_exact(8) {
                    for c in 0..3 {
                        let h = u16::from_ne_bytes([px[2 * c], px[2 * c + 1]]);
                        // Positive finite halves order like their bit patterns.
                        if h < 0x7c00 {
                            max = max.max(h);
                        }
                    }
                }
            }
        }
        _ => return 1.0,
    }
    let v = match r.layout {
        PixelLayout::Rgba16(e) => {
            let lin = hdr::decode16(e, [max; 3]);
            lin[0].max(lin[1]).max(lin[2])
        }
        _ => hdr::f16_to_f32(max),
    };
    v.max(1.0)
}

/// Brightest luma of an HDR video frame in units of reference white (every other row).
/// Saturated colours can exceed it a little; it only steers tone mapping.
fn yuv_peak(r: &Raster, e: YuvEncoding) -> f32 {
    let Some(p) = r.planes.first() else {
        return 1.0;
    };
    let wide = e.bits > 8;
    let row = (p.width as usize * e.sample_bytes()).min(p.stride);
    let mut max = 0u32;
    for line in p.data.chunks(p.stride.max(1)).step_by(2) {
        let line = &line[..row.min(line.len())];
        if wide {
            for s in line.chunks_exact(2) {
                max = max.max(u32::from(u16::from_ne_bytes([s[0], s[1]])));
            }
        } else if let Some(&m) = line.iter().max() {
            max = max.max(u32::from(m));
        }
    }
    let ([yo, ys], _) = e.levels();
    let l = ((max as f32 - yo) * ys).clamp(0.0, 1.0);
    let code = (l * 65535.0 + 0.5) as u16;
    let lin = hdr::decode16(e.colour, [code; 3]);
    lin[0].max(1.0)
}

/// Bytes of tile VRAM per tile, for budgeting.
pub const TILE_BYTES: u64 = SLOT as u64 * SLOT as u64 * 4;

/// Per-image GPU caches keyed by image id, least recently used first out.
#[derive(Debug, Default)]
pub struct GpuImageCache {
    images: HashMap<u64, (GpuImage, u64)>,
    clock: u64,
}

impl GpuImageCache {
    pub fn get(&mut self, id: u64) -> Option<&mut GpuImage> {
        self.clock += 1;
        let c = self.clock;
        self.images.get_mut(&id).map(|e| {
            e.1 = c;
            &mut e.0
        })
    }

    pub fn contains(&self, id: u64) -> bool {
        self.images.contains_key(&id)
    }

    pub fn insert(&mut self, img: GpuImage) {
        self.clock += 1;
        self.images.insert(img.id, (img, self.clock));
    }

    pub fn remove(&mut self, id: u64) -> Option<GpuImage> {
        self.images.remove(&id).map(|e| e.0)
    }

    pub fn bytes(&self) -> u64 {
        self.images.values().map(|e| e.0.memory_bytes()).sum()
    }

    /// Drops least recently used images (never those in `keep`) until under `budget`.
    pub fn trim(&mut self, budget: u64, keep: &[u64], renderer: &mut ImageRenderer) {
        while self.bytes() > budget {
            let victim = self
                .images
                .iter()
                .filter(|(id, _)| !keep.contains(id))
                .min_by_key(|(_, e)| e.1)
                .map(|(id, _)| *id);
            let Some(v) = victim else { break };
            if let Some((img, _)) = self.images.remove(&v) {
                renderer.recycle_image(img);
            }
        }
    }

    pub fn ids(&self) -> Vec<u64> {
        self.images.keys().copied().collect()
    }
}
