//! A HiPS sky survey around a sky image (off until the user picks one): the survey's
//! `properties`, its tiles fetched through `specular_remote` (so the no-network switch and
//! the downloads indicator apply, and tiles are kept in a bounded cache under the app's
//! cache folder), and a background painting of the visible part of the view in the image's
//! WCS, at half the screen's resolution, left clear where the image itself is.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Condvar, Mutex, RwLock};

use crossbeam_channel::{Receiver, Sender};
use specular_astro::frames;
use specular_astro::hips::{self as h, Properties, SURVEYS};
use specular_core::CancelToken;
use specular_decode::science::wcs::Wcs;
use specular_gfx::{Rect, TexId};
use specular_i18n::t;
use specular_remote::{Client, ClientConfig, DiskCache, Request};
use specular_render::view::Affine;

use super::super::super::App;
use super::super::wcs_rest;

/// Tiles kept in memory (each up to 1 MiB for 512² tiles).
const MEM_TILES: usize = 96;
/// The disk cache's budget.
const DISK_BYTES: u64 = 256 << 20;
/// The largest tile fetched.
const TILE_BYTES: u64 = 4 << 20;
/// Fetch threads.
const WORKERS: usize = 4;
/// Screen pixels per painted pixel.
const DOWN: f32 = 2.0;
/// The largest painting.
const MAX_SIDE: usize = 1600;

type TileMap = Arc<RwLock<HashMap<(u32, u64), Arc<Vec<u8>>>>>;
/// A tile fetched: the survey's generation, the tile, its pixels (`None`: none).
type Fetched = (u64, (u32, u64), Option<Vec<u8>>);
/// A painting: width, height, pixels and the tiles it lacked.
type Painting = (usize, usize, Vec<u8>, Vec<(u32, u64)>);

/// Tiles wanted, taken from the end; `quit` stops the workers.
#[derive(Default)]
struct Queue {
    jobs: Mutex<(bool, Vec<(u32, u64)>)>,
    ready: Condvar,
}

/// A painting of the view: the screen geometry it was made for, its pixels and the tiles it
/// lacked.
struct Painted {
    generation: u64,
    key: ViewKey,
    w: usize,
    h: usize,
    rgba: Vec<u8>,
    missing: Vec<(u32, u64)>,
}

/// What a painting depends on.
#[derive(Clone, Debug, PartialEq)]
struct ViewKey {
    affine: [f32; 6],
    vp: [f32; 4],
    image: u64,
    rest: Vec<usize>,
    tiles: u64,
}

#[derive(Default)]
pub(crate) struct Survey {
    pick: Option<usize>,
    generation: u64,
    props: Option<Arc<Properties>>,
    props_rx: Option<Receiver<Result<Properties, String>>>,
    tiles: TileMap,
    order: VecDeque<(u32, u64)>,
    asked: HashSet<(u32, u64)>,
    queue: Arc<Queue>,
    tile_rx: Option<Receiver<Fetched>>,
    tiles_version: u64,
    painting: Option<Receiver<Painted>>,
    asked_key: Option<ViewKey>,
    /// The texture shown and the view it was painted for.
    tex: Option<(specular_gfx::wgpu::Texture, TexId, ViewKey)>,
    ready: Option<Painted>,
}

impl Survey {
    /// The GPU device was replaced: the view is painted and uploaded again.
    pub(crate) fn gpu_reset(&mut self) {
        self.tex = None;
        self.asked_key = None;
    }

    fn stop_workers(&mut self) {
        let mut j = self.queue.jobs.lock().unwrap_or_else(|e| e.into_inner());
        j.0 = true;
        j.1.clear();
        drop(j);
        self.queue.ready.notify_all();
    }
}

/// The URL of a survey file.
fn url_of(base: &str, rel: &str) -> String {
    format!("{}/{rel}", base.trim_end_matches('/'))
}

/// A fetch through the disk cache: the bytes, `None` when the server has none.
fn fetch(
    client: &Client,
    cache: Option<&DiskCache>,
    base: &str,
    rel: &str,
    limit: u64,
) -> Result<Option<Vec<u8>>, String> {
    let key = DiskCache::key(&["hips", &specular_remote::cache::sanitize(base), rel]);
    if let Some(c) = cache
        && let Some(p) = c.get(&key)
        && let Ok(b) = std::fs::read(&p)
    {
        return Ok(Some(b));
    }
    let req = Request::get(url_of(base, rel)).limit(limit);
    match client.get(&req, &CancelToken::new()) {
        Ok(r) if r.status == 200 => {
            if let Some(c) = cache {
                let _ = c.insert_bytes(&key, &r.body);
            }
            Ok(Some(r.body))
        }
        Ok(r) if matches!(r.status, 204 | 404 | 410) => Ok(None),
        Ok(r) => Err(format!("the server answered {}", r.status)),
        Err(specular_remote::Error::Status(404 | 410)) => Ok(None),
        Err(e) => Err(crate::error_text::reason(&e)),
    }
}

/// The survey's cache folder.
fn cache_dir() -> std::path::PathBuf {
    specular_settings::Locations::detect()
        .cache_dir
        .join("hips")
}

/// Paints the visible part of the view (see the module docs).
#[allow(clippy::too_many_arguments)]
fn paint_view(
    wcs: &Wcs,
    rest: &[usize],
    image: (u32, u32),
    from: Affine,
    vp: [f32; 4],
    props: &Properties,
    tiles: &TileMap,
) -> Option<Painting> {
    let frame = frames::of_wcs(wcs)?;
    let w = ((vp[2] / DOWN).ceil() as usize).clamp(1, MAX_SIDE);
    let hgt = ((vp[3] / DOWN).ceil() as usize).clamp(1, MAX_SIDE);
    let (sx, sy) = (vp[2] / w as f32, vp[3] / hgt as f32);
    let stored_per_screen = f64::from(from.m[0].hypot(from.m[3]));
    let deg = wcs.arcsec_per_pixel()? / 3600.0 * stored_per_screen * f64::from(sx.max(sy));
    let order = props.order_for(deg);
    let (iw, ih) = (image.0 as f32, image.1 as f32);
    let mut out = vec![0u8; w * hgt * 4];
    let guard = tiles.read().unwrap_or_else(|e| e.into_inner());
    let missing = h::paint(
        &mut out,
        w,
        hgt,
        props.frame,
        props,
        order,
        |x, y| {
            let p = (vp[0] + (x as f32 + 0.5) * sx, vp[1] + (y as f32 + 0.5) * sy);
            let s = from.apply(p);
            if s.0 >= 0.0 && s.1 >= 0.0 && s.0 < iw && s.1 < ih {
                return None;
            }
            let (a, b) = wcs.sky_at(f64::from(s.0), f64::from(s.1), rest)?;
            Some((a, b, frame))
        },
        |k, n| guard.get(&(k, n)).map(|t| t.as_slice()),
    );
    drop(guard);
    Some((
        w,
        hgt,
        out,
        missing.into_iter().map(|n| (order, n)).collect(),
    ))
}

impl App {
    /// The next survey, or none after the last.
    pub(crate) fn hips_next(&mut self) {
        let now = self.now();
        let s = &mut self.astro.hips;
        let next = match s.pick {
            None => Some(0),
            Some(i) if i + 1 < SURVEYS.len() => Some(i + 1),
            Some(_) => None,
        };
        s.stop_workers();
        s.generation += 1;
        s.pick = next;
        s.props = None;
        s.props_rx = None;
        s.tiles = TileMap::default();
        s.order.clear();
        s.asked.clear();
        s.tile_rx = None;
        s.asked_key = None;
        s.ready = None;
        if let Some((_, id, _)) = s.tex.take()
            && let Some(gfx) = self.gfx.as_mut()
        {
            gfx.ui.unregister_texture(id);
        }
        let Some(i) = next else {
            self.chrome.toast(t!("astro-survey-off"), None, now);
            return;
        };
        let survey = SURVEYS[i];
        if specular_core::net::off::is_off() {
            self.astro.hips.pick = None;
            let msg = t!(
                "astro-survey-failed",
                name = survey.name,
                reason = t!("error-reason-network-off")
            );
            self.chrome.toast(msg, None, now);
            return;
        }
        self.chrome.toast(
            t!(
                "astro-survey-on",
                name = survey.name,
                credit = survey.credit
            ),
            None,
            now,
        );
        let (tx, rx) = crossbeam_channel::bounded(1);
        self.loader.background("net-hips-props", move |l| {
            let r = Client::new(ClientConfig::default())
                .map_err(|e| crate::error_text::reason(&e))
                .map(|c| c.tracked(specular_core::net::What::SkySurvey))
                .and_then(|c| fetch(&c, None, survey.url, "properties", 1 << 20))
                .and_then(|b| b.ok_or_else(|| "no properties file".to_owned()))
                .and_then(|b| {
                    h::parse_properties(&String::from_utf8_lossy(&b)).map_err(|e| e.to_string())
                });
            let _ = tx.send(r);
            l.waker()();
        });
        self.astro.hips.props_rx = Some(rx);
    }

    /// The view moved: the next frame paints again.
    pub(crate) fn hips_reset_view(&mut self) {
        self.astro.hips.asked_key = None;
    }

    /// Starts the fetch threads of the survey picked.
    fn hips_start(&mut self, base: &'static str) {
        let (tx, rx) = crossbeam_channel::unbounded();
        let queue = Arc::new(Queue::default());
        self.astro.hips.queue = queue.clone();
        self.astro.hips.tile_rx = Some(rx);
        let generation = self.astro.hips.generation;
        let Some(props) = self.astro.hips.props.clone() else {
            return;
        };
        let client = match Client::new(ClientConfig::default()) {
            Ok(c) => c.tracked(specular_core::net::What::SkySurvey),
            Err(_) => return,
        };
        let cache = DiskCache::open(cache_dir(), DISK_BYTES).ok();
        for k in 0..WORKERS {
            let (q, tx, client, cache, props): (Arc<Queue>, Sender<_>, _, _, _) = (
                queue.clone(),
                tx.clone(),
                client.clone(),
                cache.clone(),
                props.clone(),
            );
            let wake = self.loader.waker();
            let _ = specular_thread::builder(format!("net-hips-{k}")).spawn(move || {
                loop {
                    let job = {
                        let mut j = q.jobs.lock().unwrap_or_else(|e| e.into_inner());
                        loop {
                            if j.0 {
                                return;
                            }
                            if let Some(job) = j.1.pop() {
                                break job;
                            }
                            j = q.ready.wait(j).unwrap_or_else(|e| e.into_inner());
                        }
                    };
                    let (order, ipix) = job;
                    let rel = h::tile_path(order, ipix, props.format);
                    let px = fetch(&client, cache.as_deref(), base, &rel, TILE_BYTES)
                        .ok()
                        .flatten()
                        .and_then(specular_decode::tile_rgba)
                        .filter(|(w, hh, _)| *w == props.tile_width && *hh == props.tile_width)
                        .map(|(_, _, rgba)| rgba);
                    let _ = tx.send((generation, job, px));
                    wake();
                }
            });
        }
    }

    /// Once per turn: the properties, tiles that came in, the painting.
    pub(crate) fn hips_tick(&mut self) {
        if let Some(Ok(r)) = self.astro.hips.props_rx.as_ref().map(|r| r.try_recv()) {
            self.astro.hips.props_rx = None;
            let Some(i) = self.astro.hips.pick else {
                return;
            };
            match r {
                Ok(p) => {
                    self.astro.hips.props = Some(Arc::new(p));
                    self.hips_start(SURVEYS[i].url);
                }
                Err(e) => {
                    self.astro.hips.pick = None;
                    let now = self.now();
                    let msg = t!("astro-survey-failed", name = SURVEYS[i].name, reason = e);
                    self.chrome.toast(msg, None, now);
                }
            }
            self.redraw = true;
        }
        let s = &mut self.astro.hips;
        let came: Vec<_> = s
            .tile_rx
            .as_ref()
            .map(|r| r.try_iter().collect())
            .unwrap_or_default();
        for (generation, key, px) in came {
            if generation != s.generation {
                continue;
            }
            if let Some(px) = px {
                let mut t = s.tiles.write().unwrap_or_else(|e| e.into_inner());
                t.insert(key, Arc::new(px));
                s.order.push_back(key);
                while s.order.len() > MEM_TILES {
                    if let Some(old) = s.order.pop_front() {
                        t.remove(&old);
                        s.asked.remove(&old);
                    }
                }
                s.tiles_version += 1;
            }
            self.redraw = true;
        }
        if let Some(Ok(p)) = s.painting.as_ref().map(|r| r.try_recv()) {
            s.painting = None;
            if p.generation == s.generation {
                let mut j = s.queue.jobs.lock().unwrap_or_else(|e| e.into_inner());
                for k in &p.missing {
                    if s.asked.insert(*k) {
                        j.1.push(*k);
                    }
                }
                drop(j);
                s.queue.ready.notify_all();
                s.ready = Some(p);
            }
            self.redraw = true;
        }
    }

    /// The survey to draw under the image (texture and logical rect) and its credit.
    pub(crate) fn hips_overlay(&mut self) -> (Option<(TexId, Rect)>, Option<String>) {
        let Some(i) = self.astro.hips.pick else {
            return (None, None);
        };
        let credit = Some(t!(
            "astro-survey-credit",
            name = SURVEYS[i].name,
            credit = SURVEYS[i].credit
        ));
        // A painting that came in goes to the GPU.
        if let Some(p) = self.astro.hips.ready.take()
            && let Some(gfx) = self.gfx.as_mut()
        {
            let (tex, id) = crate::app::studies::upload_rgba(
                &gfx.gpu,
                &mut gfx.ui,
                p.w as u32,
                p.h as u32,
                &p.rgba,
            );
            if let Some((_, old, _)) = self.astro.hips.tex.replace((tex, id, p.key)) {
                gfx.ui.unregister_texture(old);
            }
        }
        let Some(props) = self.astro.hips.props.clone() else {
            return (None, credit);
        };
        let Some(s) = self.science.summary.clone() else {
            return (None, credit);
        };
        let Some(w) = s.wcs.clone().filter(|w| w.celestial.is_some()) else {
            return (None, credit);
        };
        let rest = wcs_rest(&s, &w, &self.science.index);
        let from = self.view.stored_from_screen();
        let vp = self.view.viewport;
        let key = ViewKey {
            affine: from.m,
            vp: [vp.x, vp.y, vp.w, vp.h],
            image: self.science.id.unwrap_or(0),
            rest: rest.clone(),
            tiles: self.astro.hips.tiles_version,
        };
        let hs = &mut self.astro.hips;
        if hs.painting.is_none() && hs.asked_key.as_ref() != Some(&key) {
            hs.asked_key = Some(key.clone());
            let (tx, rx) = crossbeam_channel::bounded(1);
            let (tiles, generation) = (hs.tiles.clone(), hs.generation);
            let image = (s.width, s.height);
            let wake = self.loader.waker();
            let k2 = key.clone();
            if specular_thread::builder("ren-hips")
                .spawn(move || {
                    if let Some((w2, h2, rgba, missing)) =
                        paint_view(&w, &rest, image, from, k2.vp, &props, &tiles)
                    {
                        let _ = tx.send(Painted {
                            generation,
                            key: k2,
                            w: w2,
                            h: h2,
                            rgba,
                            missing,
                        });
                        wake();
                    }
                })
                .is_ok()
            {
                hs.painting = Some(rx);
            }
        }
        // The texture where its view lies now (it moves with the image until repainted).
        let ls = self.dl.scale.max(0.1);
        let r = hs.tex.as_ref().and_then(|(_, id, k)| {
            if k.image != key.image {
                return None;
            }
            let old = Affine { m: k.affine };
            let to_screen = from.invert();
            let map = |p: (f32, f32)| {
                let q = to_screen.apply(old.apply(p));
                (q.0 / ls, q.1 / ls)
            };
            let a = map((k.vp[0], k.vp[1]));
            let b = map((k.vp[0] + k.vp[2], k.vp[1] + k.vp[3]));
            Some((
                *id,
                Rect::new(
                    a.0.min(b.0),
                    a.1.min(b.1),
                    (b.0 - a.0).abs(),
                    (b.1 - a.1).abs(),
                ),
            ))
        });
        (r, credit)
    }
}

#[cfg(test)]
pub(crate) mod testing {
    //! A tiny HiPS on the in-process test server.
    use super::*;

    /// Paints `wcs` (`w × h`, the image covering none of the view) from a HiPS of order-6,
    /// 8-pixel PNG tiles served over HTTP by the test server, fetching through the same
    /// code as the app; returns the painting and how many tiles were fetched.
    pub(crate) fn paint_from_server(wcs: &Wcs, w: u32, h: u32) -> (Vec<u8>, usize) {
        use specular_remote::testing::{ServerOptions, TestResponse as Reply, TestServer};
        let server = TestServer::start(ServerOptions::default(), |req| {
            let path = req.path().trim_start_matches('/');
            if path == "properties" {
                return Reply::ok(
                    b"hips_order = 6\nhips_tile_width = 8\nhips_tile_format = png\n\
                      hips_frame = equatorial\ndataproduct_type = image\n"
                        .to_vec(),
                );
            }
            let n: Option<u64> = path
                .rsplit_once("Npix")
                .and_then(|(_, t)| t.strip_suffix(".png"))
                .and_then(|t| t.parse().ok());
            match n {
                Some(n) => Reply::ok(tile_png(n)),
                None => Reply::status(404),
            }
        })
        .unwrap();
        let base = server.url("");
        let client = Client::new(ClientConfig::for_tests()).unwrap();
        let props = h::parse_properties(&String::from_utf8_lossy(
            &fetch(&client, None, &base, "properties", 1 << 20)
                .unwrap()
                .unwrap(),
        ))
        .unwrap();
        let tiles = TileMap::default();
        let vp = [0.0, 0.0, w as f32 * DOWN, h as f32 * DOWN];
        let from = Affine {
            m: [1.0 / DOWN, 0.0, 0.0, 0.0, 1.0 / DOWN, 0.0],
        };
        let mut fetched = 0;
        // The image is placed off the view so the whole view is painted.
        for _ in 0..3 {
            let (_, _, _, missing) =
                paint_view(wcs, &[], (0, 0), from, vp, &props, &tiles).unwrap();
            if missing.is_empty() {
                break;
            }
            for (k, n) in missing {
                let px = fetch(
                    &client,
                    None,
                    &base,
                    &h::tile_path(k, n, props.format),
                    TILE_BYTES,
                )
                .unwrap()
                .and_then(specular_decode::tile_rgba)
                .unwrap();
                tiles.write().unwrap().insert((k, n), Arc::new(px.2));
                fetched += 1;
            }
        }
        let (_, _, rgba, missing) = paint_view(wcs, &[], (0, 0), from, vp, &props, &tiles).unwrap();
        assert!(missing.is_empty());
        (rgba, fetched)
    }

    /// The oracle's tile content as PNG.
    pub(crate) fn tile_png(n: u64) -> Vec<u8> {
        let mut rgba = Vec::with_capacity(8 * 8 * 4);
        for r in 0..8u32 {
            for c in 0..8u32 {
                rgba.extend([
                    (32 * c + 16) as u8,
                    (32 * r + 16) as u8,
                    (n % 251) as u8,
                    255,
                ]);
            }
        }
        specular_png::encode(8, 8, specular_png::ColorType::Rgba, &rgba).unwrap()
    }
}
