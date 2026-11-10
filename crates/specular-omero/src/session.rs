//! An OMERO image as a decoder session: a `.omero` descriptor opened against its server.
//!
//! - **Pyramids.** A tiled image's levels are the session's scales: a decode at 1/`s` takes
//!   the coarsest level at or above that resolution, fetches the tiles under the region
//!   (several at a time, cancelled with the decode) and box-filters when the level is finer.
//!   The viewer's usual progression (a coarse whole image first, then regions at the zoom
//!   shown) makes opening progressive: the first picture costs a few small tiles.
//! - **Small images** come whole from `render_image` as PNG (exact), kept in memory for the
//!   session's later scales and regions.
//! - **Planes.** Every Z × T plane is a page (Z fastest), labelled with its Z and T.
//! - **Channels.** Rendering (active channels, windows, colours, greyscale) is the
//!   descriptor's, or the server's saved settings; the application changes channels by
//!   rewriting the descriptor and reopening.
//! - **Values.** Pixels are rendered 8-bit RGB: raw values need OMERO's Ice API or the pixel
//!   buffer microservice, which public servers (IDR) do not offer over HTTP.

use std::sync::Arc;

use parking_lot::Mutex;
use specular_core::{CancelToken, Orientation, PixelLayout, Raster, RasterPlane, Source};
use specular_decode::science::{Linear, PixelCalibration, ScienceKind, ScienceSummary};
use specular_decode::{
    DecodeError, DecodeRequest, Format, ImageInfo, OmeroChannel, OmeroFacts, PageLabel, Rect,
    Session,
};
use specular_remote::DiskCache;

use crate::api::{Omero, OmeroError};
use crate::descriptor::Descriptor;
use crate::image::ImgData;
use crate::render::{self, Encoding, Grid, Render};
use crate::tiles::{self, Piece, Rgba};

static SCALES: [u32; 17] = [
    1, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024, 2048, 4096, 8192, 16384, 32768, 65536,
];

/// The most level pixels fetched for one decode.
const MAX_REGION: u64 = 1 << 27;

/// The scales up to `max` (at least 8).
fn scales_upto(max: u32) -> &'static [u32] {
    let n = SCALES.iter().take_while(|&&s| s <= max.max(8)).count();
    &SCALES[..n]
}

/// The power of two at or above `s`.
fn pow2(s: u32) -> u32 {
    s.max(1).checked_next_power_of_two().unwrap_or(1 << 31)
}

/// An OMERO failure as a decode error; a session problem also asks the application to
/// sign in again.
pub fn decode_error(omero: &Omero, e: OmeroError) -> DecodeError {
    use std::io::{Error, ErrorKind};
    if e.needs_sign_in() {
        crate::registry::request_sign_in(omero.base());
    }
    match e {
        OmeroError::Cancelled => DecodeError::Cancelled,
        OmeroError::Malformed(m) => DecodeError::Malformed(m),
        OmeroError::NotFound => DecodeError::Io(Error::new(ErrorKind::NotFound, e.to_string())),
        OmeroError::Offline(_) | OmeroError::NetworkOff => {
            DecodeError::Io(Error::new(ErrorKind::NotConnected, e.to_string()))
        }
        OmeroError::Status(_) => DecodeError::Io(Error::other(e.to_string())),
        OmeroError::SignInRequired
        | OmeroError::Expired
        | OmeroError::LoginRefused
        | OmeroError::Forbidden => {
            DecodeError::Io(Error::new(ErrorKind::PermissionDenied, e.to_string()))
        }
    }
}

struct Shared {
    omero: Arc<Omero>,
    cache: Option<Arc<DiskCache>>,
    source: Arc<Source>,
    desc: Descriptor,
    img: ImgData,
    render: Render,
    encoding: Encoding,
    /// Levels used: (level, power-of-two factor), finest first.
    levels: Vec<(u32, u32)>,
    facts: OmeroFacts,
    info: ImageInfo,
}

/// One plane of an OMERO image.
pub struct OmeroSession {
    shared: Arc<Shared>,
    z: u32,
    t: u32,
    summary: ScienceSummary,
    /// A whole small image, once fetched.
    whole: Mutex<Option<Arc<Rgba>>>,
}

/// The decoder's opener: a descriptor read from `source`, its server's connection and
/// cache from the [`registry`](crate::registry).
pub fn open(source: Arc<Source>) -> specular_decode::Result<Arc<dyn Session>> {
    let desc = Descriptor::parse(source.bytes())
        .ok_or_else(|| DecodeError::Malformed("not an OMERO image descriptor".into()))?;
    let omero = crate::registry::connection(&desc.server).map_err(|e| match e {
        OmeroError::Malformed(m) => DecodeError::Malformed(m),
        e => DecodeError::Io(std::io::Error::other(e.to_string())),
    })?;
    open_with(
        omero,
        crate::registry::cache(),
        source,
        desc,
        &CancelToken::new(),
    )
    .map(|s| s as Arc<dyn Session>)
}

/// Opens the image `desc` names on `omero` (its imgData is fetched now).
pub fn open_with(
    omero: Arc<Omero>,
    cache: Option<Arc<DiskCache>>,
    source: Arc<Source>,
    desc: Descriptor,
    cancel: &CancelToken,
) -> specular_decode::Result<Arc<OmeroSession>> {
    let img = omero
        .img_data(desc.image, cancel)
        .map_err(|e| decode_error(&omero, e))?;
    let render = desc
        .render
        .clone()
        .filter(|r| r.channels.len() == img.channels.len())
        .unwrap_or_else(|| Render::of(&img));
    let encoding = desc.encoding.unwrap_or(if img.tiled {
        Encoding::Jpeg
    } else {
        Encoding::Png
    });
    let mut levels: Vec<(u32, u32)> = (0..img.levels)
        .filter_map(|l| img.level_factor(l).map(|f| (l, f)))
        .collect();
    if levels.first().is_none_or(|l| l.1 != 1) {
        levels.insert(0, (0, 1));
    }
    levels.sort_by_key(|l| l.1);
    levels.dedup_by_key(|l| l.1);
    let (w, h) = (img.size[0], img.size[1]);
    let max_scale = if img.tiled {
        levels.last().map_or(1, |l| l.1).saturating_mul(8)
    } else {
        let side = w.max(h) / 64;
        if side <= 1 { 1 } else { 1 << side.ilog2() }
    };
    let coding = match (img.tiled, encoding) {
        (true, Encoding::Jpeg) => format!("OMERO, {}×{} JPEG tiles", img.tile.0, img.tile.1),
        (true, Encoding::Png) => format!("OMERO, {}×{} PNG tiles", img.tile.0, img.tile.1),
        (false, e) => format!("OMERO, rendered {}", e.id().to_ascii_uppercase()),
    };
    let info = ImageInfo {
        format: Format::Omero,
        width: w,
        height: h,
        orientation: Orientation::NORMAL,
        icc_profile: None,
        has_alpha: false,
        bit_depth: 8,
        progressive: false,
        coding,
        embedded_preview: None,
        fast_regions: true,
        scales: scales_upto(max_scale),
    };
    let facts = facts(&omero, &desc, &img, &render, encoding);
    let z = desc.z.unwrap_or(img.default_z).min(img.size[2] - 1);
    let t = desc.t.unwrap_or(img.default_t).min(img.size[4] - 1);
    let shared = Arc::new(Shared {
        omero,
        cache,
        source,
        desc,
        img,
        render,
        encoding,
        levels,
        facts,
        info,
    });
    Ok(OmeroSession::new(shared, z, t))
}

fn facts(
    omero: &Omero,
    desc: &Descriptor,
    img: &ImgData,
    render: &Render,
    enc: Encoding,
) -> OmeroFacts {
    let c = &desc.context;
    OmeroFacts {
        server: omero.base().to_string(),
        user: omero.user().or_else(|| desc.user.clone()),
        image_id: img.id,
        name: if img.name.is_empty() {
            desc.name.clone()
        } else {
            img.name.clone()
        },
        project: c
            .project
            .clone()
            .or_else(|| img.project.as_ref().map(|p| p.1.clone())),
        dataset: c
            .dataset
            .clone()
            .or_else(|| img.dataset.as_ref().map(|d| d.1.clone())),
        screen: c.screen.clone(),
        plate: c.plate.clone(),
        well: c.well.clone(),
        owner: desc.owner.clone().or_else(|| img.author.clone()),
        group: desc.group.clone(),
        acquired: img.timestamp.map(crate::browse::utc),
        size: img.size.map(u64::from),
        pixels_type: img.pixels_type.clone(),
        physical: img.pixel_size.map(|p| p.map(|v| (v, "µm".to_string()))),
        channels: img
            .channels
            .iter()
            .zip(&render.channels)
            .map(|(c, r)| OmeroChannel {
                name: c.label.clone(),
                colour: r.colour,
                window: (r.start, r.end),
                active: r.active,
            })
            .collect(),
        tiles: img.tiled.then_some((img.tile.0, img.tile.1, img.levels)),
        plane: (0, 0),
        encoding: match enc {
            Encoding::Jpeg => "JPEG",
            Encoding::Png => "PNG",
        },
    }
}

impl OmeroSession {
    fn new(shared: Arc<Shared>, z: u32, t: u32) -> Arc<OmeroSession> {
        let summary = summary(&shared, z, t);
        Arc::new(OmeroSession {
            shared,
            z,
            t,
            summary,
            whole: Mutex::new(None),
        })
    }

    /// The image's metadata as the server gave it.
    pub fn img_data(&self) -> &ImgData {
        &self.shared.img
    }

    /// The rendering in effect.
    pub fn render(&self) -> &Render {
        &self.shared.render
    }

    pub fn plane(&self) -> (u32, u32) {
        (self.z, self.t)
    }

    fn key(&self, what: &str) -> String {
        let s = &self.shared;
        DiskCache::key(&[
            "omero",
            s.omero.base(),
            &s.img.id.to_string(),
            &format!("z{}t{}", self.z, self.t),
            &format!("{}-{}", s.render.hash(), s.encoding.id()),
            what,
        ])
    }

    /// The whole plane of an image without a pyramid.
    fn whole(&self, cancel: &CancelToken) -> Result<Arc<Rgba>, OmeroError> {
        if let Some(w) = &*self.whole.lock() {
            return Ok(w.clone());
        }
        let s = &self.shared;
        let url = render::image_url(s.img.id, self.z, self.t, &s.render, s.encoding);
        let bytes = tiles::fetch(&s.omero, s.cache.as_ref(), &self.key("whole"), &url, cancel)?;
        let img = Arc::new(tiles::decode(&bytes, cancel)?);
        *self.whole.lock() = Some(img.clone());
        Ok(img)
    }

    /// RGBA of `x0..x1 × y0..y1` of `level` (clamped to it), and whether a tile was short.
    fn level_region(
        &self,
        level: u32,
        (x0, y0, x1, y1): (u32, u32, u32, u32),
        cancel: &CancelToken,
    ) -> Result<(Rgba, bool), OmeroError> {
        let s = &self.shared;
        if !s.img.tiled {
            let w = self.whole(cancel)?;
            return Ok((crop(&w, x0, y0, x1, y1), false));
        }
        let (lw, lh) = s.img.level_size(level);
        let grid = Grid {
            level,
            width: lw,
            height: lh,
            tile_w: s.img.tile.0,
            tile_h: s.img.tile.1,
        };
        let (x0, y0) = (x0.min(lw - 1), y0.min(lh - 1));
        let (x1, y1) = (x1.clamp(x0 + 1, lw), y1.clamp(y0 + 1, lh));
        let pieces: Vec<Piece> = grid
            .covering(x0, y0, x1, y1)
            .into_iter()
            .map(|t| Piece {
                url: render::tile_url(s.img.id, self.z, self.t, &grid, &t, &s.render, s.encoding),
                key: self.key(&format!("{}-{}-{}", t.level, t.col, t.row)),
                x: i64::from(t.x) - i64::from(x0),
                y: i64::from(t.y) - i64::from(y0),
                w: t.w,
                h: t.h,
            })
            .collect();
        tiles::assemble(
            &s.omero,
            s.cache.as_ref(),
            &pieces,
            x1 - x0,
            y1 - y0,
            cancel,
        )
    }

    fn decode_rgba(
        &self,
        req: &DecodeRequest,
        cancel: &CancelToken,
    ) -> Result<(Rgba, (u32, u32), u32, bool), OmeroError> {
        let s = &self.shared;
        let scale = pow2(req.scale);
        let (w, h) = (s.info.width, s.info.height);
        let (ws, hs) = (w.div_ceil(scale), h.div_ceil(scale));
        let r = req.region.unwrap_or(Rect {
            x: 0,
            y: 0,
            w: ws,
            h: hs,
        });
        let rx0 = r.x.min(ws.saturating_sub(1));
        let ry0 = r.y.min(hs.saturating_sub(1));
        let rw = r.w.min(ws - rx0).max(1);
        let rh = r.h.min(hs - ry0).max(1);
        // The coarsest level at or above the resolution asked for.
        let (level, factor) = if s.img.tiled {
            s.levels
                .iter()
                .copied()
                .rfind(|l| l.1 <= scale)
                .unwrap_or((0, 1))
        } else {
            (0, 1)
        };
        let ratio = scale / factor;
        let lx0 = rx0.saturating_mul(ratio);
        let ly0 = ry0.saturating_mul(ratio);
        let lx1 = (rx0 + rw).saturating_mul(ratio);
        let ly1 = (ry0 + rh).saturating_mul(ratio);
        if u64::from(lx1 - lx0) * u64::from(ly1 - ly0) > MAX_REGION {
            return Err(OmeroError::Malformed("region too large".into()));
        }
        let (src, short) = self.level_region(level, (lx0, ly0, lx1, ly1), cancel)?;
        let out = tiles::reduce(&src, ratio, rw, rh);
        Ok((out, (rx0, ry0), scale, short))
    }
}

/// The part `x0..x1 × y0..y1` of `img` (clamped, at least one pixel).
fn crop(img: &Rgba, x0: u32, y0: u32, x1: u32, y1: u32) -> Rgba {
    let x0 = x0.min(img.width.saturating_sub(1));
    let y0 = y0.min(img.height.saturating_sub(1));
    let x1 = x1.clamp(x0 + 1, img.width.max(1));
    let y1 = y1.clamp(y0 + 1, img.height.max(1));
    if img.width == 0 || img.height == 0 {
        return Rgba {
            width: x1 - x0,
            height: y1 - y0,
            data: vec![128; (x1 - x0) as usize * (y1 - y0) as usize * 4],
        };
    }
    let mut data = Vec::with_capacity((x1 - x0) as usize * (y1 - y0) as usize * 4);
    for y in y0..y1 {
        let s = (y as usize * img.width as usize + x0 as usize) * 4;
        data.extend_from_slice(&img.data[s..s + (x1 - x0) as usize * 4]);
    }
    Rgba {
        width: x1 - x0,
        height: y1 - y0,
        data,
    }
}

fn summary(s: &Shared, z: u32, t: u32) -> ScienceSummary {
    let img = &s.img;
    let pixel = match img.pixel_size {
        [Some(x), y, _] => {
            let y = y.unwrap_or(x);
            Some(PixelCalibration {
                x: Linear {
                    offset: 0.0,
                    step: x,
                },
                y: Linear {
                    offset: 0.0,
                    step: y,
                },
                unit_x: "µm".into(),
                unit_y: "µm".into(),
            })
        }
        _ => None,
    };
    let mut facts = s.facts.clone();
    facts.plane = (z, t);
    ScienceSummary {
        width: img.size[0],
        height: img.size[1],
        sample: img.pixels_type.clone(),
        title: Some(facts.name.clone()).filter(|n| !n.is_empty()),
        date_obs: facts.acquired.clone(),
        dataset: facts.dataset.clone(),
        kind: ScienceKind::Microscopy,
        pixel,
        magnification: img.magnification,
        channel_colours: s
            .render
            .channels
            .iter()
            .map(|c| c.colour.map(|v| f32::from(v) / 255.0))
            .collect(),
        raster_only: true,
        vendor: Some("OMERO".into()),
        levels: s.levels.iter().map(|&(l, _)| img.level_size(l)).collect(),
        omero: Some(Arc::new(facts)),
        ..Default::default()
    }
}

fn raster(img: Rgba, origin: (u32, u32), scale: u32, truncated: bool) -> Raster {
    Raster {
        width: img.width,
        height: img.height,
        origin,
        scale,
        layout: PixelLayout::Rgba,
        planes: vec![RasterPlane {
            width: img.width,
            height: img.height,
            stride: img.width as usize * 4,
            data: img.data,
            h_div: 1,
            v_div: 1,
        }],
        truncated,
    }
}

impl Session for OmeroSession {
    fn info(&self) -> &ImageInfo {
        &self.shared.info
    }
    fn source(&self) -> &Arc<Source> {
        &self.shared.source
    }
    fn decode_preview(&self) -> Option<specular_decode::Result<Raster>> {
        None
    }
    fn decode(&self, req: &DecodeRequest, cancel: &CancelToken) -> specular_decode::Result<Raster> {
        let (img, origin, scale, short) = self
            .decode_rgba(req, cancel)
            .map_err(|e| decode_error(&self.shared.omero, e))?;
        Ok(raster(img, origin, scale, short))
    }
    fn decode_memory(&self, req: &DecodeRequest) -> u64 {
        let s = pow2(req.scale);
        let (w, h) = match req.region {
            Some(r) => (r.w, r.h),
            None => (
                self.shared.info.width.div_ceil(s),
                self.shared.info.height.div_ceil(s),
            ),
        };
        // The output and the level pixels it may be reduced from (up to 8× per side).
        u64::from(w) * u64::from(h) * 4 * 3 + (8 << 20)
    }
    fn scaled_decode_cheap(&self) -> bool {
        self.shared.img.tiled
    }
    fn memory_bytes(&self) -> usize {
        self.whole.lock().as_ref().map_or(0, |w| w.data.len())
    }
    fn trim_memory(&self) -> usize {
        self.whole.lock().take().map_or(0, |w| w.data.len())
    }
    fn colour_model(&self) -> Option<&'static str> {
        Some(if self.shared.render.greyscale {
            "Greyscale"
        } else if self.shared.img.channels.len() > 1 {
            "Multichannel"
        } else {
            "RGB"
        })
    }
    fn page_count(&self) -> u32 {
        self.shared.img.planes()
    }
    fn page_index(&self) -> u32 {
        self.t * self.shared.img.size[2] + self.z
    }
    fn page_label(&self) -> Option<PageLabel> {
        let [_, _, zs, _, ts] = self.shared.img.size;
        (zs * ts > 1).then_some(PageLabel::Plane {
            z: (zs > 1).then_some((self.z, zs)),
            t: (ts > 1).then_some((self.t, ts)),
        })
    }
    fn open_page(&self, page: u32) -> specular_decode::Result<Arc<dyn Session>> {
        let zs = self.shared.img.size[2];
        if page >= self.page_count() {
            return Err(DecodeError::Unsupported("no such plane".into()));
        }
        Ok(OmeroSession::new(self.shared.clone(), page % zs, page / zs))
    }
    fn science(&self) -> Option<&ScienceSummary> {
        Some(&self.summary)
    }
}

/// The descriptor this session was opened from.
impl OmeroSession {
    pub fn descriptor(&self) -> &Descriptor {
        &self.shared.desc
    }
}
