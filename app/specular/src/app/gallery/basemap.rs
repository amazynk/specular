//! The map view's raster base map: tiles drawn over the built-in
//! offline map, from the user's own MBTiles or PMTiles file, or, when the user turns it on,
//! from OpenStreetMap, a tile server of their choice, or a WMS, WMTS or OGC API Tiles layer
//! chosen in the map's services panel.
//!
//! - Off by default. An online source asks once, per server, after explaining what that
//!   server sees (the area viewed and the computer's address, nothing about the photos).
//! - Tiles come from background threads: online through `specular-remote` into a disk cache
//!   capped by `metadata.basemap_cache`, files through `specular-decode`'s tile readers. Each
//!   finished tile is posted as [`Event::BasemapTile`] and uploaded before the next frame.
//! - While a tile is missing its nearest loaded ancestor stands in, scaled up.
//! - The source's credit line is shown with the Natural Earth credit.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex, PoisonError};

use specular_decode::geo::specular_gis::tiles::{TileFormat, TileStore};
use specular_gfx::{DrawList, Gpu, Rect, TexId, UiRenderer, wgpu};
use specular_map::Camera;
use specular_map::basemap::{self as bm, TileKey};
use specular_map::ogc::{ServiceTiles, Tiles};
use specular_remote::{CancelToken, Client, ClientConfig, DiskCache, Request};
use specular_settings::Basemap as Choice;

use crate::loader::{Event, Loader};

/// Tiles kept on the GPU at most (256² RGBA: 64 MB).
const GPU_TILES: usize = 256;
/// Tiles uploaded per frame at most.
const UPLOADS_PER_FRAME: usize = 24;
/// Levels up an ancestor may stand in for a missing tile.
const ANCESTORS: u8 = 6;
/// Fetch threads for an online source (tile servers ask for few connections).
const ONLINE_WORKERS: usize = 2;
/// The largest tile accepted from a server.
const TILE_BYTES: u64 = 8 << 20;

/// What the settings ask for.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::app) struct Config {
    pub choice: Choice,
    pub url: String,
    pub path: Option<PathBuf>,
    /// The service layer of [`Choice::Service`] ([`ServiceTiles::encode`]).
    pub service: String,
    pub cache_mb: i64,
    /// Hosts the user agreed to.
    pub allowed: Vec<String>,
    /// Dark theme (vector tiles from a file are drawn in it).
    pub dark: bool,
}

/// Where tiles come from.
#[derive(Clone)]
enum Source {
    Off,
    Online {
        tiles: Tiles,
        host: String,
        credit: String,
        max_zoom: u8,
    },
    File {
        store: Arc<dyn TileStore>,
        credit: Option<String>,
        min_zoom: u8,
        max_zoom: u8,
    },
}

enum Slot {
    /// Asked for; not in yet.
    Wanted,
    /// The source has no such tile.
    Missing,
    Ready {
        w: u32,
        h: u32,
        rgba: Arc<[u8]>,
    },
    Gpu {
        _tex: wgpu::Texture,
        id: TexId,
        used: u64,
    },
}

/// The jobs the fetch threads take, newest first.
#[derive(Default)]
struct Jobs {
    generation: u64,
    wanted: Vec<TileKey>,
    busy: HashSet<TileKey>,
    stop: bool,
}

#[derive(Default)]
struct Queue {
    jobs: Mutex<Jobs>,
    ready: Condvar,
}

impl Queue {
    fn lock(&self) -> std::sync::MutexGuard<'_, Jobs> {
        self.jobs.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The next job of `generation`, waiting for one; `None` when this generation is over.
    fn take(&self, generation: u64) -> Option<TileKey> {
        let mut j = self.lock();
        loop {
            if j.stop || j.generation != generation {
                return None;
            }
            if let Some(k) = j.wanted.pop() {
                j.busy.insert(k);
                return Some(k);
            }
            j = self.ready.wait(j).unwrap_or_else(PoisonError::into_inner);
        }
    }

    fn done(&self, k: TileKey) {
        self.lock().busy.remove(&k);
    }
}

impl Drop for Queue {
    fn drop(&mut self) {
        self.lock().stop = true;
        self.ready.notify_all();
    }
}

type Rgba = (u32, u32, Vec<u8>);

/// How a fetch thread hands a finished tile (or `None`) to the app.
fn post(l: &Loader, generation: u64) -> impl Fn(TileKey, Option<Rgba>) + use<> {
    let l = l.clone();
    move |key, tile| {
        l.post(Event::BasemapTile {
            generation,
            key,
            tile: tile.map(|(w, h, px)| (w, h, Arc::from(px))),
        });
    }
}

/// One fetch thread of base map `generation`: takes tiles until the generation is over. A
/// panic on one tile (a bad tile in a file, a decoder bug) costs that tile, which shows as
/// missing; the thread goes on with the next. `source` names the source in the log.
fn fetch_loop(
    q: &Queue,
    generation: u64,
    source: &str,
    post: &dyn Fn(TileKey, Option<Rgba>),
    mut get: impl FnMut(TileKey) -> Option<Rgba>,
) {
    while let Some(key) = q.take(generation) {
        let (z, x, y) = key;
        let tile = specular_core::isolate::run(
            format_args!("base map tile {z}/{x}/{y} of {source}"),
            || {
                specular_core::isolate::trip(std::path::Path::new(source));
                get(key)
            },
        )
        .ok()
        .flatten();
        q.done(key);
        post(key, tile);
    }
}

/// The base map's state in the map view.
pub(in crate::app) struct Basemap {
    config: Option<Config>,
    source: Source,
    generation: u64,
    slots: HashMap<TileKey, Slot>,
    tick: u64,
    queue: Arc<Queue>,
    /// The server waiting for the user's agreement (nothing is fetched until then).
    pub consent: Option<String>,
    /// Why the base map cannot be shown (a file that does not open, a bad address).
    pub error: Option<String>,
    /// Bytes of the disk cache (for the explanation).
    pub cache_mb: i64,
}

impl Default for Basemap {
    fn default() -> Self {
        Basemap {
            config: None,
            source: Source::Off,
            generation: 0,
            slots: HashMap::new(),
            tick: 0,
            queue: Arc::new(Queue::default()),
            consent: None,
            error: None,
            cache_mb: 256,
        }
    }
}

/// The disk cache of downloaded tiles.
pub(in crate::app) fn cache_dir() -> PathBuf {
    specular_settings::Locations::detect()
        .cache_dir
        .join("basemap")
}

/// A tile from a server, through the disk cache: `Ok(None)` when the server has none.
pub(in crate::app) fn fetch_online(
    client: &Client,
    cache: &DiskCache,
    tiles: &Tiles,
    host: &str,
    key: TileKey,
    cancel: &CancelToken,
) -> Result<Option<Vec<u8>>, String> {
    let (z, x, y) = (key.0.to_string(), key.1.to_string(), key.2.to_string());
    // A service layer's tiles are kept apart from other layers of the same host.
    let layer = match tiles {
        Tiles::Xyz(_) => None,
        t => Some(format!("{:016x}", fnv(&format!("{t:?}")))),
    };
    let rel = match &layer {
        None => DiskCache::key(&[host, &z, &x, &y]),
        Some(l) => DiskCache::key(&[host, l, &z, &x, &y]),
    };
    if let Some(p) = cache.get(&rel)
        && let Ok(b) = std::fs::read(&p)
    {
        return Ok(Some(b));
    }
    let url = tiles.url(key).ok_or("not a tile server address")?;
    let req = Request::get(url).accept("image/*").limit(TILE_BYTES);
    match client.get(&req, cancel) {
        Ok(r) if r.status == 200 => {
            let _ = cache.insert_bytes(&rel, &r.body);
            Ok(Some(r.body))
        }
        Ok(r) if matches!(r.status, 204 | 404 | 410) => Ok(None),
        Ok(r) => Err(format!("the tile server answered {}", r.status)),
        Err(specular_remote::Error::Status(404 | 410)) => Ok(None),
        Err(e) => Err(crate::error_text::reason(&e)),
    }
}

/// FNV-1a of a text (the cache name of a service layer).
fn fnv(s: &str) -> u64 {
    s.bytes().fold(0xcbf2_9ce4_8422_2325, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

/// A tile's pixels from a tile set file (vector tiles drawn in the simple style).
fn from_file(store: &dyn TileStore, key: TileKey, dark: bool) -> Option<(u32, u32, Vec<u8>)> {
    let bytes = store.tile(key.0, key.1, key.2).ok()??;
    if store.meta().format == TileFormat::Mvt {
        let bg = if dark {
            [0x2a, 0x2f, 0x35]
        } else {
            [0xf1, 0xee, 0xe8]
        };
        let px = specular_decode::render_vector_tile(&bytes, key, 256, dark, bg)?;
        return Some((256, 256, px));
    }
    specular_decode::tile_rgba(bytes)
}

impl Basemap {
    /// Follows the settings: a new source (and new fetch threads) when they changed.
    pub fn configure(&mut self, cfg: Config, loader: &Loader) {
        if self.config.as_ref() == Some(&cfg) {
            return;
        }
        let same_source = self.config.as_ref().is_some_and(|c| {
            c.choice == cfg.choice
                && c.url == cfg.url
                && c.path == cfg.path
                && c.service == cfg.service
                && c.allowed == cfg.allowed
                && (c.dark == cfg.dark || c.choice != Choice::File)
        });
        self.cache_mb = cfg.cache_mb;
        if same_source {
            self.config = Some(cfg);
            return;
        }
        self.reset();
        self.error = None;
        self.consent = None;
        let online = |tiles: Tiles, credit: String, max_zoom: u8| {
            let host = match &tiles {
                Tiles::Xyz(u) | Tiles::Wms(u) | Tiles::Matrix { url: u, .. } => {
                    bm::host_of(u).unwrap_or_default()
                }
            };
            Source::Online {
                tiles,
                host,
                credit,
                max_zoom,
            }
        };
        self.source = match cfg.choice {
            Choice::Off => Source::Off,
            Choice::OpenStreetMap => online(
                Tiles::Xyz(bm::OPENSTREETMAP.url.to_owned()),
                bm::OPENSTREETMAP.attribution.to_owned(),
                bm::OPENSTREETMAP.max_zoom,
            ),
            Choice::Custom if bm::valid_template(&cfg.url) => {
                online(Tiles::Xyz(cfg.url.trim().to_owned()), String::new(), 19)
            }
            Choice::Service => match ServiceTiles::decode(&cfg.service) {
                Some(st) => {
                    let credit = if st.credit.is_empty() {
                        st.name.clone()
                    } else {
                        st.credit.clone()
                    };
                    let max = st.tiles.max_zoom();
                    online(st.tiles, credit, max)
                }
                None => {
                    self.error = Some(specular_i18n::t!("map-basemap-bad-address"));
                    Source::Off
                }
            },
            Choice::Custom => {
                self.error = (!cfg.url.trim().is_empty())
                    .then(|| specular_i18n::t!("map-basemap-bad-address"));
                Source::Off
            }
            Choice::File => match cfg.path.as_deref().map(specular_decode::open_tile_file) {
                Some(Ok(store)) => {
                    let m = store.meta();
                    if m.format.is_raster() || m.format == TileFormat::Mvt {
                        Source::File {
                            credit: m.attribution.clone(),
                            min_zoom: m.min_zoom,
                            max_zoom: m.max_zoom,
                            store,
                        }
                    } else {
                        self.error = Some(specular_i18n::t!("map-basemap-unknown-tiles"));
                        Source::Off
                    }
                }
                Some(Err(e)) => {
                    self.error = Some(crate::error_text::reason(&e));
                    Source::Off
                }
                None => Source::Off,
            },
        };
        if let Source::Online { host, .. } = &self.source {
            if specular_core::net::off::is_off() {
                // Network off: cached tiles still show; no consent card for a server that
                // cannot be reached anyway.
                self.error = Some(specular_i18n::t!("error-reason-network-off"));
            } else if !cfg.allowed.iter().any(|a| a == host) {
                self.consent = Some(host.clone());
            }
        }
        self.start(loader, &cfg);
        self.config = Some(cfg);
    }

    /// Drops every tile and stops the fetch threads.
    fn reset(&mut self) {
        self.generation += 1;
        {
            let mut j = self.queue.lock();
            j.generation = self.generation;
            j.wanted.clear();
        }
        self.queue.ready.notify_all();
        self.slots.clear();
    }

    /// Starts the fetch threads of the source (none while consent is pending).
    fn start(&mut self, loader: &Loader, cfg: &Config) {
        if self.consent.is_some() {
            return;
        }
        let generation = self.generation;
        match self.source.clone() {
            Source::Off => {}
            Source::Online { tiles, host, .. } => {
                let client = match Client::new(ClientConfig::default()) {
                    Ok(c) => c.tracked(specular_core::net::What::MapTiles),
                    Err(e) => {
                        self.error = Some(crate::error_text::reason(&e));
                        return;
                    }
                };
                let budget = (cfg.cache_mb.max(32) as u64) << 20;
                let cache = match DiskCache::open(cache_dir(), budget) {
                    Ok(c) => c,
                    Err(e) => {
                        self.error = Some(crate::error_text::reason(&e));
                        return;
                    }
                };
                for _ in 0..ONLINE_WORKERS {
                    let (q, l) = (self.queue.clone(), loader.clone());
                    let (client, cache) = (client.clone(), cache.clone());
                    let (tiles, host) = (tiles.clone(), host.clone());
                    loader.background("map-tiles", move |_| {
                        let cancel = CancelToken::new();
                        let name = host.clone();
                        fetch_loop(&q, generation, &name, &post(&l, generation), |key| {
                            fetch_online(&client, &cache, &tiles, &host, key, &cancel)
                                .ok()
                                .flatten()
                                .and_then(specular_decode::tile_rgba)
                        });
                    });
                }
            }
            Source::File { store, .. } => {
                let (q, l, dark) = (self.queue.clone(), loader.clone(), cfg.dark);
                let name = cfg
                    .path
                    .as_ref()
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_default();
                loader.background("map-tiles", move |_| {
                    fetch_loop(&q, generation, &name, &post(&l, generation), |key| {
                        from_file(store.as_ref(), key, dark)
                    });
                });
            }
        }
    }

    /// The user agreed to the server (recorded by the caller): fetching starts.
    pub fn allow(&mut self, loader: &Loader) {
        self.consent = None;
        if let Some(cfg) = self.config.clone() {
            self.start(loader, &cfg);
        }
    }

    /// Whether tiles are drawn (a source, agreed to).
    pub fn active(&self) -> bool {
        !matches!(self.source, Source::Off) && self.consent.is_none()
    }

    /// The source's credit line, when its tiles are drawn.
    pub fn credit(&self) -> Option<String> {
        if !self.active() {
            return None;
        }
        match &self.source {
            Source::Online { credit, host, .. } => Some(if credit.is_empty() {
                host.clone()
            } else {
                credit.clone()
            }),
            Source::File { credit, .. } => credit.clone(),
            Source::Off => None,
        }
    }

    fn zooms(&self) -> (u8, u8) {
        match &self.source {
            Source::Online { max_zoom, .. } => (0, *max_zoom),
            Source::File {
                min_zoom, max_zoom, ..
            } => (*min_zoom, *max_zoom),
            Source::Off => (0, 0),
        }
    }

    /// A finished tile from a fetch thread.
    pub fn tile_ready(
        &mut self,
        generation: u64,
        key: TileKey,
        tile: Option<(u32, u32, Arc<[u8]>)>,
    ) {
        if generation != self.generation {
            return;
        }
        self.slots.insert(
            key,
            match tile {
                Some((w, h, rgba)) if rgba.len() as u64 == u64::from(w) * u64::from(h) * 4 => {
                    Slot::Ready { w, h, rgba }
                }
                _ => Slot::Missing,
            },
        );
    }

    /// Before the frame: uploads tiles that came in, and lets the least used go.
    pub fn upload(&mut self, gpu: &Gpu, ui: &mut UiRenderer) {
        let mut n = 0;
        for slot in self.slots.values_mut() {
            if n >= UPLOADS_PER_FRAME {
                break;
            }
            if let Slot::Ready { w, h, rgba } = slot {
                let (tex, id) = crate::app::studies::upload_rgba(gpu, ui, *w, *h, rgba);
                *slot = Slot::Gpu {
                    _tex: tex,
                    id,
                    used: self.tick,
                };
                n += 1;
            }
        }
        let on_gpu = self
            .slots
            .values()
            .filter(|s| matches!(s, Slot::Gpu { .. }))
            .count();
        if on_gpu > GPU_TILES {
            let mut used: Vec<(u64, TileKey)> = self
                .slots
                .iter()
                .filter_map(|(k, s)| match s {
                    Slot::Gpu { used, .. } => Some((*used, *k)),
                    _ => None,
                })
                .collect();
            used.sort_unstable();
            for (_, k) in used.into_iter().take(on_gpu - GPU_TILES) {
                if let Some(Slot::Gpu { id, .. }) = self.slots.remove(&k) {
                    ui.unregister_texture(id);
                }
            }
        }
    }

    /// Lets go of every texture (the map closed).
    pub fn release(&mut self, ui: &mut UiRenderer) {
        for (_, s) in self.slots.drain() {
            if let Slot::Gpu { id, .. } = s {
                ui.unregister_texture(id);
            }
        }
    }

    /// The GPU device was replaced: tiles on the old one are asked for again (from the tile
    /// cache).
    pub fn gpu_reset(&mut self) {
        self.slots.retain(|_, s| !matches!(s, Slot::Gpu { .. }));
    }

    /// Draws the tiles covering the view into `area` (logical px), asking for the missing
    /// ones; true while some are still coming.
    pub fn draw(&mut self, dl: &mut DrawList, cam: &Camera, area: Rect, device_scale: f32) -> bool {
        if !self.active() {
            return false;
        }
        self.tick += 1;
        let tick = self.tick;
        let (min_z, max_z) = self.zooms();
        let z = bm::level(cam, f64::from(device_scale), max_z);
        if z < min_z {
            return false;
        }
        let placed = bm::visible(cam, z);
        let mut want = Vec::new();
        dl.push_clip(area);
        for p in &placed {
            let r = Rect::new(
                area.x + p.rect.0 as f32,
                area.y + p.rect.1 as f32,
                p.rect.2 as f32,
                p.rect.3 as f32,
            );
            match self.slots.get_mut(&p.key) {
                Some(Slot::Gpu { id, used, .. }) => {
                    *used = tick;
                    dl.image(r, [0.0, 0.0, 1.0, 1.0], *id, 0.0);
                    continue;
                }
                Some(Slot::Missing) => {}
                Some(Slot::Wanted | Slot::Ready { .. }) => {}
                None => {
                    self.slots.insert(p.key, Slot::Wanted);
                    want.push(p.key);
                }
            }
            if matches!(self.slots.get(&p.key), Some(Slot::Wanted)) && !want.contains(&p.key) {
                want.push(p.key);
            }
            // An ancestor stands in.
            for up in 1..=ANCESTORS.min(z.saturating_sub(min_z)) {
                let Some((a, uv)) = bm::ancestor(p.key, up) else {
                    break;
                };
                if let Some(Slot::Gpu { id, used, .. }) = self.slots.get_mut(&a) {
                    *used = tick;
                    dl.image(r, uv, *id, 0.0);
                    break;
                }
            }
        }
        dl.pop_clip();
        let pending = !want.is_empty();
        // The nearest the centre is taken first (the threads pop from the end).
        want.reverse();
        let mut j = self.queue.lock();
        if j.generation == self.generation {
            let busy = j.busy.clone();
            j.wanted = want.into_iter().filter(|k| !busy.contains(k)).collect();
        }
        drop(j);
        self.queue.ready.notify_all();
        // Tiles asked for and no longer in view are asked again when they are.
        let visible: HashSet<TileKey> = placed.iter().map(|p| p.key).collect();
        let busy = self.queue.lock().busy.clone();
        self.slots
            .retain(|k, s| !matches!(s, Slot::Wanted) || visible.contains(k) || busy.contains(k));
        pending
    }

    /// Screenshots: the explanation for OpenStreetMap's server, as a first use shows it.
    #[cfg(test)]
    pub(in crate::app) fn ask_for_test(&mut self, cache_mb: i64) {
        self.source = Source::Online {
            tiles: Tiles::Xyz(bm::OPENSTREETMAP.url.to_owned()),
            host: bm::host_of(bm::OPENSTREETMAP.url).unwrap_or_default(),
            credit: bm::OPENSTREETMAP.attribution.to_owned(),
            max_zoom: bm::OPENSTREETMAP.max_zoom,
        };
        self.consent = bm::host_of(bm::OPENSTREETMAP.url);
        self.cache_mb = cache_mb;
    }

    #[cfg(test)]
    pub(in crate::app) fn tiles_on_gpu(&self) -> usize {
        self.slots
            .values()
            .filter(|s| matches!(s, Slot::Gpu { .. }))
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use specular_remote::testing::{ServerOptions, TestResponse, TestServer};

    #[test]
    fn fetches_through_the_cache_and_names_only_the_tile() {
        let png = specular_decode::geo::specular_gis::fixtures::tile_png(3, 1, 2);
        let body = png.clone();
        let server = TestServer::start(ServerOptions::default(), move |r| {
            if r.path() == "/3/1/2.png" {
                TestResponse::ok(body.clone()).header("Content-Type", "image/png")
            } else {
                TestResponse::status(404)
            }
        })
        .unwrap();
        let dir = std::env::temp_dir().join(format!("specular-basemap-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let cache = DiskCache::open(&dir, 1 << 20).unwrap();
        let client = Client::new(ClientConfig::for_tests()).unwrap();
        let template = Tiles::Xyz(server.url("/{z}/{x}/{y}.png"));
        let host = bm::host_of(&server.url("/")).unwrap();
        let cancel = CancelToken::new();
        let t = fetch_online(&client, &cache, &template, &host, (3, 1, 2), &cancel).unwrap();
        assert_eq!(t.as_deref(), Some(&png[..]));
        // From the cache the second time: no request.
        let n = server.requests();
        let again = fetch_online(&client, &cache, &template, &host, (3, 1, 2), &cancel).unwrap();
        assert_eq!(again.as_deref(), Some(&png[..]));
        assert_eq!(server.requests(), n);
        // A tile the server does not have.
        assert_eq!(
            fetch_online(&client, &cache, &template, &host, (3, 0, 0), &cancel).unwrap(),
            None
        );
        // What the server saw: the tile's address, and nothing else of ours.
        for r in server.log() {
            assert!(r.path().ends_with(".png"), "{}", r.path());
            assert!(r.query().is_empty());
            assert!(r.header("cookie").is_none() && r.header("referer").is_none());
        }
        let (w, h, _) = specular_decode::tile_rgba(png).unwrap();
        assert_eq!((w, h), (256, 256));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A WMS layer as the base map: each tile is a GetMap of the tile's Web Mercator
    /// bounds, kept in the cache apart from the host's other layers.
    #[test]
    fn service_tiles_ask_for_their_bounds() {
        let png = specular_decode::geo::specular_gis::fixtures::tile_png(1, 0, 0);
        let body = png.clone();
        let server = TestServer::start(ServerOptions::default(), move |r| {
            if r.param("REQUEST").as_deref() == Some("GetMap") {
                TestResponse::ok(body.clone()).header("Content-Type", "image/png")
            } else {
                TestResponse::status(400)
            }
        })
        .unwrap();
        let dir = std::env::temp_dir().join(format!("specular-basemap-w-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let cache = DiskCache::open(&dir, 1 << 20).unwrap();
        let client = Client::new(ClientConfig::for_tests()).unwrap();
        let tiles = Tiles::Wms(server.url(
            "/wms?SERVICE=WMS&VERSION=1.3.0&REQUEST=GetMap&LAYERS=a&STYLES=&CRS=EPSG:3857&BBOX={bbox}&WIDTH=256&HEIGHT=256&FORMAT=image%2Fpng",
        ));
        let cancel = CancelToken::new();
        let t = fetch_online(&client, &cache, &tiles, "127.0.0.1", (1, 0, 0), &cancel).unwrap();
        assert_eq!(t.as_deref(), Some(&png[..]));
        let log = server.log();
        let bbox = log[0].param("BBOX").unwrap();
        let v: Vec<f64> = bbox.split(',').map(|x| x.parse().unwrap()).collect();
        assert!(
            (v[0] + specular_map::ogc::MERCATOR_HALF).abs() < 1e-3,
            "{bbox}"
        );
        assert!(v[1].abs() < 1e-3 && v[2].abs() < 1e-3, "{bbox}");
        // The same key from an XYZ layer of the host is another tile.
        let n = server.requests();
        let xyz = Tiles::Xyz(server.url("/{z}/{x}/{y}.png"));
        let _ = fetch_online(&client, &cache, &xyz, "127.0.0.1", (1, 0, 0), &cancel);
        assert_eq!(server.requests(), n + 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A tile that panics shows as missing and the fetch thread goes on with the next.
    #[test]
    fn a_panicking_tile_costs_only_that_tile() {
        let q = Queue::default();
        {
            let mut j = q.lock();
            j.generation = 1;
            j.wanted = vec![(1, 0, 0), (1, 1, 0)];
        }
        let got = std::sync::Mutex::new(Vec::new());
        let post = |k: TileKey, t: Option<Rgba>| {
            let mut g = got.lock().unwrap();
            g.push((k, t.is_some()));
            if g.len() == 2 {
                q.lock().stop = true;
            }
        };
        fetch_loop(&q, 1, "tiles-specular-test-panic.mbtiles", &post, |_| {
            Some((1, 1, vec![0; 4]))
        });
        let mut got = got.into_inner().unwrap();
        got.sort();
        assert_eq!(got, [((1, 0, 0), false), ((1, 1, 0), false)]);
        assert!(q.lock().busy.is_empty());
        // The same loop on a good source delivers.
        let q2 = Queue::default();
        {
            let mut j = q2.lock();
            j.generation = 1;
            j.wanted = vec![(1, 0, 0)];
        }
        let ok = std::sync::Mutex::new(None);
        let post = |_: TileKey, t: Option<Rgba>| {
            *ok.lock().unwrap() = Some(t.is_some());
            q2.lock().stop = true;
        };
        fetch_loop(&q2, 1, "tiles.mbtiles", &post, |_| Some((1, 1, vec![0; 4])));
        assert_eq!(ok.into_inner().unwrap(), Some(true));
    }

    #[test]
    fn the_cache_stays_under_its_budget() {
        let server = TestServer::start(ServerOptions::default(), |_| {
            TestResponse::ok(vec![7u8; 40_000])
        })
        .unwrap();
        let dir = std::env::temp_dir().join(format!("specular-basemap-b-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let cache = DiskCache::open(&dir, 200_000).unwrap();
        let client = Client::new(ClientConfig::for_tests()).unwrap();
        let template = Tiles::Xyz(server.url("/{z}/{x}/{y}"));
        let cancel = CancelToken::new();
        for x in 0..20 {
            fetch_online(&client, &cache, &template, "h", (5, x, 1), &cancel).unwrap();
        }
        assert!(cache.stats().bytes <= 200_000, "{:?}", cache.stats());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
