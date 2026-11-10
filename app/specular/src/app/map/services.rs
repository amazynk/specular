//! Online map services in the map view (opt-in, like the base map): WMS, WMTS and OGC API
//! Tiles layers become the photo map's base map; WFS and OGC API Features layers are added
//! to this map as GeoJSON for the area on screen; a STAC search lists the satellite and
//! aerial images of that area, and one of its Cloud-Optimized GeoTIFFs is downloaded into
//! the cache and opened.
//!
//! - Off until Settings › Maps › Online map services is on; nothing is fetched before.
//! - Before the first request to a server, a card says what it sees (the computer's
//!   address and the area asked for) and asks.
//! - Servers are saved in `map-servers.json` in the settings folder; removing one asks
//!   first and leaves everything on disk.
//!
//! What goes over the network is in the pure functions [`describe`], [`features`],
//! [`search`]: the tests run them against a local server only.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use specular_decode::geo::specular_gis::model::Dataset;
use specular_gfx::{Align, DrawList, Rect, Text, TextStyle};
use specular_i18n::t;
use specular_map::basemap::host_of;
use specular_map::ogc::{self, Capabilities, Kind, Offer, Refusal, StacItem};
use specular_remote::{CancelToken, Client, Request};
use specular_theme::Palette;
use specular_theme::metrics::{FONT_BODY, FONT_SMALL, PAD, RADIUS_MD, RADIUS_SM, TOOLBAR_MARGIN};

/// The settings panel under Online map services: the saved servers.
pub(in crate::app) const PANEL_KEY: &str = "maps.services";
/// The confirm sheet's item for removing a saved server.
pub(in crate::app) const REMOVE_ITEM: u64 = crate::app::items::MAP_SERVICE_REMOVE;
/// Rows the panel shows at once.
pub(super) const ROWS: usize = 8;
/// Features asked for at most.
pub(super) const FEATURE_LIMIT: usize = 2000;
/// Search results asked for at most.
pub(super) const SEARCH_LIMIT: usize = 30;
const CAPS_BYTES: u64 = 16 << 20;
const FEATURE_BYTES: u64 = 64 << 20;
const SEARCH_BYTES: u64 = 8 << 20;
/// The largest image downloaded from a search.
pub(super) const COG_BYTES: u64 = 2 << 30;

/// A server the user added.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::app) struct Server {
    pub id: String,
    pub name: String,
    pub url: String,
    pub kind: Option<Kind>,
}

/// The saved servers' file.
pub(in crate::app) fn servers_file() -> PathBuf {
    specular_settings::Locations::detect()
        .config_dir
        .join("map-servers.json")
}

/// The servers in `path` (none when it is missing or unreadable).
pub(in crate::app) fn load(path: &Path) -> Vec<Server> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let Ok(j) = serde_json::from_str::<serde_json::Value>(&text) else {
        specular_log::warn!(target: specular_log::SETTINGS, "map servers: {} is not JSON", path.display());
        return Vec::new();
    };
    j.get("servers")
        .and_then(|s| s.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|s| {
                    let url = s.get("url")?.as_str()?.to_owned();
                    host_of(&url)?;
                    Some(Server {
                        id: s.get("id")?.as_str()?.to_owned(),
                        name: s
                            .get("name")
                            .and_then(|n| n.as_str())
                            .unwrap_or("")
                            .to_owned(),
                        url,
                        kind: s
                            .get("kind")
                            .and_then(|k| k.as_str())
                            .and_then(Kind::from_id),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Writes the servers to `path`, whole or not at all.
pub(in crate::app) fn save(path: &Path, servers: &[Server]) -> std::io::Result<()> {
    let list: Vec<serde_json::Value> = servers
        .iter()
        .map(|s| {
            serde_json::json!({
                "id": s.id,
                "name": s.name,
                "url": s.url,
                "kind": s.kind.map(Kind::id),
            })
        })
        .collect();
    let text = serde_json::to_string_pretty(&serde_json::json!({ "version": 1, "servers": list }))
        .map_err(std::io::Error::other)?;
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d)?;
    }
    super::super::gallery::marks::write_atomic(path, text.as_bytes())
}

/// A new server's id.
pub(in crate::app) fn new_id(existing: &[Server]) -> String {
    let n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let mut id = format!("s{n:x}");
    while existing.iter().any(|s| s.id == id) {
        id.push('x');
    }
    id
}

/// An address as typed: `https://` added when it has no scheme. `None` for one that is not
/// http(s) or has no host.
pub(in crate::app) fn normalise(url: &str) -> Option<String> {
    let u = url.trim();
    if u.is_empty() {
        return None;
    }
    let u = if u.contains("://") {
        u.to_owned()
    } else {
        format!("https://{u}")
    };
    let lower = u.to_ascii_lowercase();
    let ok = (lower.starts_with("http://") || lower.starts_with("https://"))
        && host_of(&u).is_some()
        // No user name or password in the address (they would be saved and shown).
        && !u.split("://").nth(1).unwrap_or("").split('/').next().unwrap_or("").contains('@');
    ok.then_some(u)
}

// ----- What goes over the network ------------------------------------------------------

fn get(
    client: &Client,
    url: &str,
    accept: &str,
    limit: u64,
    cancel: &CancelToken,
) -> Result<Vec<u8>, String> {
    let req = Request::get(url).accept(accept).limit(limit);
    match client.get(&req, cancel) {
        Ok(r) if r.status == 200 => Ok(r.body),
        Ok(r) => Err(t!("map-service-status", status = i64::from(r.status))),
        Err(e) => Err(crate::error_text::reason(&e)),
    }
}

/// What a server offers: its address as given, else its GetCapabilities requests in turn;
/// an OGC API or STAC landing page's collections are fetched next.
pub(in crate::app) fn describe(
    client: &Client,
    url: &str,
    cancel: &CancelToken,
) -> Result<Capabilities, String> {
    let mut last = t!("map-service-unknown");
    for u in ogc::probe_urls(url) {
        let body = match get(
            client,
            &u,
            "application/json, text/xml, application/xml",
            CAPS_BYTES,
            cancel,
        ) {
            Ok(b) => b,
            Err(e) => {
                last = e;
                continue;
            }
        };
        match ogc::parse(&u, &body) {
            Ok(mut c) => {
                if let Some(cols) = c.collections.take() {
                    let more = get(client, &cols, "application/json", CAPS_BYTES, cancel)
                        .and_then(|b| ogc::parse(&cols, &b).map_err(|e| parse_reason(&e)));
                    if let Ok(m) = more {
                        if c.title.is_empty() {
                            c.title = m.title;
                        }
                        c.layers.extend(m.layers);
                    }
                }
                return Ok(c);
            }
            Err(ogc::ParseError::Exception(m)) => last = t!("map-service-exception", message = m),
            Err(e) => last = parse_reason(&e),
        }
    }
    Err(last)
}

fn parse_reason(e: &ogc::ParseError) -> String {
    match e {
        ogc::ParseError::Unknown => t!("map-service-unknown"),
        ogc::ParseError::Exception(m) => t!("map-service-exception", message = m.clone()),
        ogc::ParseError::Malformed(m) => t!("map-service-malformed", reason = m.clone()),
    }
}

/// A layer's features in the area `bbox` (west, south, east, north in degrees).
pub(super) fn features(
    client: &Client,
    template: &str,
    title: &str,
    bbox: [f64; 4],
    cancel: &CancelToken,
) -> Result<Dataset, String> {
    let url = ogc::fill(template, bbox, FEATURE_LIMIT, None);
    let body = get(
        client,
        &url,
        "application/geo+json, application/json",
        FEATURE_BYTES,
        cancel,
    )?;
    specular_decode::geo::specular_gis::geojson::read(&body, title)
}

/// The images a STAC search finds in `bbox`.
pub(super) fn search(
    client: &Client,
    template: &str,
    bbox: [f64; 4],
    cancel: &CancelToken,
) -> Result<Vec<StacItem>, String> {
    let url = ogc::fill(template, bbox, SEARCH_LIMIT, None);
    let body = get(
        client,
        &url,
        "application/geo+json, application/json",
        SEARCH_BYTES,
        cancel,
    )?;
    ogc::stac_items(&url, &body)
        .map(|(v, _)| v)
        .map_err(|e| parse_reason(&e))
}

/// The size of a file on a server, when it says.
pub(super) fn size_of(
    client: &Client,
    url: &str,
    cancel: &CancelToken,
) -> Result<Option<u64>, String> {
    match client.head(&Request::head(url), cancel) {
        Ok(h) if h.status == 200 => Ok(h.headers.content_length()),
        Ok(h) => Err(t!("map-service-status", status = i64::from(h.status))),
        Err(e) => Err(crate::error_text::reason(&e)),
    }
}

/// Where a downloaded image is kept.
pub(in crate::app) fn cog_dir() -> PathBuf {
    specular_settings::Locations::detect()
        .cache_dir
        .join("map-images")
}

/// A file name for a downloaded image: the item and asset, made safe.
pub(super) fn cog_name(item: &str, url: &str) -> String {
    let base = url.split(['?', '#']).next().unwrap_or(url);
    let file = base.rsplit('/').next().unwrap_or("image.tif");
    let safe = |s: &str| -> String {
        s.chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
                    c
                } else {
                    '_'
                }
            })
            .collect()
    };
    let mut name = format!("{}_{}", safe(item), safe(file));
    if !name.to_ascii_lowercase().ends_with(".tif") && !name.to_ascii_lowercase().ends_with(".tiff")
    {
        name.push_str(".tif");
    }
    name.chars().take(160).collect()
}

// ----- The panel ---------------------------------------------------------------------------

/// Something waiting to be asked of a server.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Pending {
    /// What the saved server at this index offers.
    Describe(usize),
    /// Features of a layer for the area on screen.
    Features { template: String, title: String },
    /// A STAC search over the area on screen.
    Search { template: String, title: String },
    /// The size of an image, before asking to download it.
    Size { url: String, title: String },
    /// Downloading an image (agreed to).
    Download { url: String, title: String },
}

impl Pending {
    pub(super) fn url(&self, servers: &[Server]) -> Option<String> {
        match self {
            Pending::Describe(i) => servers.get(*i).map(|s| s.url.clone()),
            Pending::Features { template, .. } | Pending::Search { template, .. } => {
                Some(template.clone())
            }
            Pending::Size { url, .. } | Pending::Download { url, .. } => Some(url.clone()),
        }
    }
}

/// What the background thread answered.
pub(super) enum Reply {
    Described(usize, Result<Box<Capabilities>, String>),
    Features(Result<Box<Dataset>, String>),
    Items(String, Result<Vec<StacItem>, String>),
    Sized(String, String, Result<Option<u64>, String>),
    Downloaded(Result<PathBuf, String>),
}

/// The panel's page.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Page {
    /// Online services are off in the settings.
    Off,
    /// The saved servers, and Add.
    List,
    /// Typing a new server's address.
    Add { text: String, error: Option<String> },
    /// Asking before the first request to `host`.
    Consent { host: String, then: Pending },
    /// Waiting for a server.
    Busy(String),
    /// A server's layers.
    Layers {
        server: usize,
        caps: Arc<Capabilities>,
    },
    /// A search's images.
    Items {
        title: String,
        items: Arc<Vec<StacItem>>,
    },
    /// Asking before downloading an image.
    Download {
        url: String,
        title: String,
        size: Option<u64>,
    },
    /// Something went wrong: why, and the page to go back to.
    Failed(String),
}

/// The services panel's state (kept across the map's files).
#[derive(Default)]
pub(in crate::app) struct Services {
    pub(super) open: bool,
    pub(super) page: Option<Page>,
    pub(super) cursor: usize,
    pub(super) scroll: usize,
    /// Pages to go back to.
    pub(super) back: Vec<Page>,
    pub(super) servers: Option<Vec<Server>>,
    /// The saved server a removal asks about.
    pub(in crate::app) removing: Option<String>,
    /// The panel's targets as drawn.
    pub(super) drawn: Drawn,
    /// The request in flight can be cancelled.
    pub(super) cancel: Option<CancelToken>,
    /// The settings panel of saved servers has been filled.
    pub(super) panel_set: bool,
}

/// A target in the panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Hit {
    Row(usize),
    Button(Button),
    Field,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Button {
    TurnOn,
    Close,
    Back,
    Connect,
    Allow,
    Cancel,
    Remove,
    Download,
    /// Geotagging: write the sidecars (asks first).
    Write,
    /// Geotagging: guess the camera clock's offset.
    Guess,
    /// Export: save the file.
    Save,
    /// Export: print to scale.
    Print,
}

impl Button {
    pub(super) fn label(self) -> String {
        match self {
            Button::TurnOn => t!("map-service-turn-on"),
            Button::Close => t!("map-service-close"),
            Button::Back => t!("map-service-back"),
            Button::Connect => t!("map-service-connect"),
            Button::Allow => t!("map-service-allow"),
            Button::Cancel => t!("map-service-cancel"),
            Button::Remove => t!("map-service-remove"),
            Button::Download => t!("map-service-download"),
            Button::Write => t!("map-geotag-write"),
            Button::Guess => t!("map-geotag-guess"),
            Button::Save => t!("map-export-save"),
            Button::Print => t!("map-export-print"),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Drawn {
    pub rect: Option<Rect>,
    pub hits: Vec<(Hit, Rect)>,
    /// Where the address field's input method window opens.
    pub ime: Option<Rect>,
}

/// One row of the panel: a name and a line under it; `enabled` false for one that cannot
/// be used (a refused layer).
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Row {
    pub title: String,
    pub sub: String,
    pub enabled: bool,
}

/// What a page shows: its title, lines, a text field, rows and buttons.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct View {
    pub title: String,
    pub lines: Vec<String>,
    pub field: Option<String>,
    pub rows: Vec<Row>,
    pub cursor: usize,
    pub scroll: usize,
    pub buttons: Vec<Button>,
    pub focused: bool,
}

fn size_text(b: u64) -> String {
    let mb = b as f64 / f64::from(1u32 << 20);
    if mb >= 1024.0 {
        t!(
            "map-service-size-gb",
            size = specular_i18n::fixed(mb / 1024.0, 1)
        )
    } else {
        t!(
            "map-service-size-mb",
            size = specular_i18n::fixed(mb.max(1.0), 0)
        )
    }
}

/// The line under a layer: what choosing it does, or why it can't be used.
pub(super) fn offer_text(o: &Offer) -> (String, bool) {
    match o {
        Offer::Tiles(_) => (t!("map-service-offer-tiles"), true),
        Offer::Features(_) => (t!("map-service-offer-features"), true),
        Offer::Search(_) => (t!("map-service-offer-search"), true),
        Offer::Refused(Refusal::NoWebMercator) => (t!("map-service-refused-mercator"), false),
        Offer::Refused(Refusal::NoImageFormat) => (t!("map-service-refused-format"), false),
        Offer::Refused(Refusal::NoGeoJson) => (t!("map-service-refused-geojson"), false),
    }
}

impl Services {
    pub(super) fn servers(&mut self) -> &mut Vec<Server> {
        self.servers.get_or_insert_with(|| load(&servers_file()))
    }

    /// The page's view.
    pub(super) fn view(&self, focused: bool) -> Option<View> {
        let page = self.page.as_ref()?;
        let servers = self.servers.as_deref().unwrap_or(&[]);
        let mut v = View {
            title: t!("map-services-title"),
            lines: Vec::new(),
            field: None,
            rows: Vec::new(),
            cursor: self.cursor,
            scroll: self.scroll,
            buttons: Vec::new(),
            focused,
        };
        match page {
            Page::Off => {
                v.lines = vec![t!("map-service-off"), t!("map-service-off-why")];
                v.buttons = vec![Button::TurnOn, Button::Close];
            }
            Page::List => {
                v.lines = vec![t!("map-service-list-hint")];
                v.rows = servers
                    .iter()
                    .map(|s| Row {
                        title: if s.name.is_empty() {
                            host_of(&s.url).unwrap_or_default()
                        } else {
                            s.name.clone()
                        },
                        sub: match s.kind {
                            Some(k) => t!(
                                "map-service-server-sub",
                                kind = k.name(),
                                host = host_of(&s.url).unwrap_or_default()
                            ),
                            None => host_of(&s.url).unwrap_or_default(),
                        },
                        enabled: true,
                    })
                    .collect();
                v.rows.push(Row {
                    title: t!("map-service-add"),
                    sub: t!("map-service-add-sub"),
                    enabled: true,
                });
                v.buttons = if self.cursor < servers.len() {
                    vec![Button::Remove, Button::Close]
                } else {
                    vec![Button::Close]
                };
            }
            Page::Add { text, error } => {
                v.title = t!("map-service-add");
                v.field = Some(text.clone());
                v.lines = vec![t!("map-service-add-hint")];
                v.lines.extend(error.clone());
                v.buttons = vec![Button::Connect, Button::Back];
            }
            Page::Consent { host, .. } => {
                v.title = t!("map-service-consent-title");
                v.lines = vec![
                    t!(
                        "map-service-consent-server",
                        host = specular_i18n::isolate(host)
                    ),
                    t!("map-service-consent-sees"),
                    t!("map-service-consent-private"),
                ];
                v.buttons = vec![Button::Allow, Button::Cancel];
            }
            Page::Busy(what) => {
                v.lines = vec![what.clone()];
                v.buttons = vec![Button::Cancel];
            }
            Page::Layers { caps, .. } => {
                v.title = if caps.title.is_empty() {
                    caps.kind.name().to_owned()
                } else {
                    caps.title.clone()
                };
                v.lines = vec![t!("map-service-layers-hint", kind = caps.kind.name())];
                v.rows = caps
                    .layers
                    .iter()
                    .map(|l| {
                        let (sub, enabled) = offer_text(&l.offer);
                        Row {
                            title: if l.title.is_empty() {
                                l.id.clone()
                            } else {
                                l.title.clone()
                            },
                            sub,
                            enabled,
                        }
                    })
                    .collect();
                if v.rows.is_empty() {
                    v.lines.push(t!("map-service-no-layers"));
                }
                v.buttons = vec![Button::Back];
            }
            Page::Items { title, items } => {
                v.title = title.clone();
                v.lines = vec![t!("map-service-items-hint", count = items.len() as i64)];
                v.rows = items
                    .iter()
                    .map(|it| {
                        let when = it
                            .datetime
                            .as_deref()
                            .map(|d| d.chars().take(16).collect::<String>().replace('T', " "))
                            .unwrap_or_default();
                        let sub = match it.cloud {
                            Some(c) => t!(
                                "map-service-item-cloud",
                                when = when,
                                cloud = specular_i18n::fixed(c, 0)
                            ),
                            None => when,
                        };
                        let sub = if it.cogs.is_empty() {
                            t!("map-service-item-no-cog")
                        } else {
                            sub
                        };
                        Row {
                            title: it.id.clone(),
                            sub,
                            enabled: !it.cogs.is_empty(),
                        }
                    })
                    .collect();
                if v.rows.is_empty() {
                    v.lines.push(t!("map-service-no-items"));
                }
                v.buttons = vec![Button::Back];
            }
            Page::Download { url, title, size } => {
                v.title = title.clone();
                let host = host_of(url).unwrap_or_default();
                v.lines = vec![
                    match size {
                        Some(b) => t!(
                            "map-service-download-ask",
                            size = size_text(*b),
                            host = specular_i18n::isolate(&host)
                        ),
                        None => t!(
                            "map-service-download-ask-unknown",
                            host = specular_i18n::isolate(&host)
                        ),
                    },
                    t!("map-service-download-cache"),
                ];
                v.buttons = vec![Button::Download, Button::Cancel];
            }
            Page::Failed(why) => {
                v.lines = vec![why.clone()];
                v.buttons = vec![Button::Back];
            }
        }
        Some(v)
    }

    /// Goes to `page`, remembering where it came from.
    pub(super) fn go(&mut self, page: Page) {
        if let Some(p) = self.page.take()
            && !matches!(p, Page::Busy(_) | Page::Consent { .. } | Page::Failed(_))
        {
            self.back.push(p);
        }
        self.page = Some(page);
        self.cursor = 0;
        self.scroll = 0;
    }

    /// Back a page; false when there was none (the panel closes).
    pub(super) fn back(&mut self) -> bool {
        if let Some(c) = self.cancel.take() {
            c.cancel();
        }
        match self.back.pop() {
            Some(p) => {
                self.page = Some(p);
                self.cursor = 0;
                self.scroll = 0;
                true
            }
            None => false,
        }
    }

    /// Moves the cursor by `d` rows, keeping it in view.
    pub(super) fn move_cursor(&mut self, d: isize, rows: usize) {
        if rows == 0 {
            return;
        }
        let c = (self.cursor as isize + d).clamp(0, rows as isize - 1) as usize;
        self.cursor = c;
        if c < self.scroll {
            self.scroll = c;
        } else if c >= self.scroll + ROWS {
            self.scroll = c + 1 - ROWS;
        }
    }
}

/// Draws the panel at the top centre of `area`.
#[allow(clippy::too_many_arguments)]
pub(super) fn draw(
    dl: &mut DrawList,
    text: &mut Text,
    pal: &Palette,
    v: &View,
    preedit: &str,
    area: (f32, f32, f32, f32),
    y: f32,
    rtl: bool,
    ts: f32,
) -> Drawn {
    let (ax, _, aw, _) = area;
    let mut ime = None;
    let mut hits = Vec::new();
    let w = (420.0 * ts).min(aw - 2.0 * TOOLBAR_MARGIN);
    let x = ax + (aw - w) / 2.0;
    let inner = w - 2.0 * PAD;
    let st = TextStyle::new(FONT_SMALL * ts);
    let title = text.layout(&v.title, st.weight(600.0), Some(inner), Align::Start);
    let lines: Vec<_> = v
        .lines
        .iter()
        .map(|l| text.layout(l, st, Some(inner), Align::Start))
        .collect();
    let row_h = (FONT_SMALL * 2.6 * ts).round();
    let field_h = (FONT_BODY * 2.0 * ts).round();
    let btn_h = (FONT_BODY * 1.9 * ts).round();
    let shown = v.rows.len().saturating_sub(v.scroll).min(ROWS);
    let h = 2.0 * PAD
        + title.height
        + 6.0 * ts
        + lines.iter().map(|b| b.height + 2.0 * ts).sum::<f32>()
        + v.field.as_ref().map_or(0.0, |_| field_h + 6.0 * ts)
        + shown as f32 * row_h
        + if shown > 0 { 6.0 * ts } else { 0.0 }
        + btn_h
        + 6.0 * ts;
    let r = Rect::new(x, y, w, h);
    dl.glass(r, RADIUS_MD, pal.glass, pal.glass_border, 1.0);
    if v.focused {
        dl.stroke_rect(r, RADIUS_MD, pal.accent, 1.5 * ts);
    }
    let start = |bw: f32| if rtl { x + w - PAD - bw } else { x + PAD };
    let mut ty = y + PAD;
    text.draw(dl, &title, start(title.width), ty, pal.text);
    ty += title.height + 6.0 * ts;
    for b in &lines {
        text.draw(dl, b, start(b.width), ty, pal.text_muted);
        ty += b.height + 2.0 * ts;
    }
    if let Some(value) = &v.field {
        let f = Rect::new(x + PAD, ty, inner, field_h);
        dl.rect_border(f, RADIUS_SM, pal.surface, pal.accent, 1.5 * ts);
        // Addresses read left to right in every language; a long one scrolls to the caret.
        ime = Some(super::furniture::draw_field_text(
            dl,
            text,
            pal,
            (value, preedit),
            TextStyle::new(FONT_BODY * ts),
            f.inset_xy(8.0 * ts, 0.0),
            (false, 5.0 * ts, ts),
            pal.text,
        ));
        hits.push((Hit::Field, f));
        ty += field_h + 6.0 * ts;
    }
    for (k, row) in v.rows.iter().enumerate().skip(v.scroll).take(ROWS) {
        let rr = Rect::new(x + PAD / 2.0, ty, w - PAD, row_h);
        if k == v.cursor {
            dl.rect(
                rr,
                RADIUS_SM,
                pal.accent.with_alpha(if v.focused { 0.28 } else { 0.14 }),
            );
        }
        let ink = if row.enabled {
            pal.text
        } else {
            pal.text_muted
        };
        let tb = text.layout(&row.title, st.weight(550.0), Some(inner), Align::Start);
        let sb = text.layout(
            &row.sub,
            TextStyle::new(FONT_SMALL * 0.9 * ts),
            Some(inner),
            Align::Start,
        );
        text.draw(dl, &tb, start(tb.width), ty + 2.0 * ts, ink);
        text.draw(
            dl,
            &sb,
            start(sb.width),
            ty + 3.0 * ts + tb.height,
            pal.text_muted,
        );
        hits.push((Hit::Row(k), rr));
        ty += row_h;
    }
    if shown > 0 {
        ty += 6.0 * ts;
    }
    // Buttons from the end: the last is the way out.
    let mut bx = if rtl { x + PAD } else { x + w - PAD };
    for b in v.buttons.iter().rev() {
        let lb = text.layout(&b.label(), st.weight(550.0), None, Align::Start);
        let bw = lb.width + 20.0 * ts;
        let rect = if rtl {
            Rect::new(bx, ty, bw, btn_h)
        } else {
            Rect::new(bx - bw, ty, bw, btn_h)
        };
        let primary = matches!(
            b,
            Button::Allow
                | Button::TurnOn
                | Button::Connect
                | Button::Download
                | Button::Write
                | Button::Save
                | Button::Print
        );
        let danger = *b == Button::Remove;
        let fill = if primary {
            pal.accent_fill
        } else {
            pal.surface
        };
        dl.rect_border(
            rect,
            RADIUS_SM,
            fill,
            if danger { pal.danger } else { pal.glass_border },
            ts,
        );
        let ink = if primary {
            pal.accent_text
        } else if danger {
            pal.danger
        } else {
            pal.text
        };
        text.draw(
            dl,
            &lb,
            rect.x + 10.0 * ts,
            rect.y + (btn_h - lb.height) / 2.0,
            ink,
        );
        hits.push((Hit::Button(*b), rect));
        bx = if rtl {
            bx + bw + 6.0 * ts
        } else {
            bx - bw - 6.0 * ts
        };
    }
    Drawn {
        rect: Some(r),
        hits,
        ime,
    }
}

// ----- The app's side ------------------------------------------------------------------------

use specular_input::{Key, KeyCombo, NamedKey};
use specular_settings::Origin;
use specular_widgets::{PanelEvent, PanelRow, SettingPanel};

use super::super::App;
use super::{Focus, Job, layers};

/// The hosts the user agreed to, from the settings.
fn allowed(list: &str) -> Vec<String> {
    list.split(',')
        .map(|h| h.trim().to_ascii_lowercase())
        .filter(|h| !h.is_empty())
        .collect()
}

impl App {
    /// Opens or closes the services panel.
    pub(super) fn gis_services_toggle(&mut self) {
        let sv = &mut self.map.services;
        if sv.open {
            sv.open = false;
            if self.map.focus == Some(Focus::Services) {
                self.map.focus = None;
            }
            return;
        }
        sv.open = true;
        sv.back.clear();
        sv.servers();
        sv.page = Some(if self.settings.maps.services {
            Page::List
        } else {
            Page::Off
        });
        sv.cursor = 0;
        sv.scroll = 0;
        self.map.focus = Some(Focus::Services);
    }

    /// Asks a server for `p`: after the explanation when it is the first time for its host.
    fn gis_services_request(&mut self, p: Pending) {
        let servers = self.map.services.servers().clone();
        let Some(url) = p.url(&servers) else { return };
        let Some(host) = host_of(&url) else {
            self.map
                .services
                .go(Page::Failed(t!("map-service-bad-address")));
            return;
        };
        if specular_core::net::off::is_off() {
            self.map
                .services
                .go(Page::Failed(crate::app::privacy::off_text()));
            return;
        }
        if !allowed(&self.settings.maps.services_allowed).contains(&host) {
            self.map.services.go(Page::Consent { host, then: p });
            return;
        }
        let client = match Client::new(specular_remote::ClientConfig::default()) {
            Ok(c) => c.tracked(specular_core::net::What::MapService),
            Err(e) => {
                self.map
                    .services
                    .go(Page::Failed(crate::error_text::reason(&e)));
                return;
            }
        };
        let cancel = CancelToken::new();
        self.map.services.cancel = Some(cancel.clone());
        let tx = self.map.channel();
        let busy = match &p {
            Pending::Download { .. } => t!(
                "map-service-downloading",
                host = specular_i18n::isolate(&host)
            ),
            _ => t!("map-service-asking", host = specular_i18n::isolate(&host)),
        };
        self.map.services.go(Page::Busy(busy));
        let bbox = self
            .gis_view_extent()
            .unwrap_or([-180.0, -85.0, 180.0, 85.0]);
        let (id, first) = (self.map.id.unwrap_or_default(), self.map.layers.len());
        self.loader.background("net-map-service", move |l| {
            let reply = match p {
                Pending::Describe(i) => {
                    Reply::Described(i, describe(&client, &url, &cancel).map(Box::new))
                }
                Pending::Features { template, title } => {
                    let r = features(&client, &template, &title, bbox, &cancel);
                    let added = r.as_ref().map(|ds| {
                        let ds = Arc::new(ds.clone());
                        layers::layers_of(
                            &ds,
                            layers::Coords::LonLat,
                            None,
                            Some(PathBuf::from(&title)),
                            first,
                        )
                    });
                    if let Ok(ls) = &added {
                        let _ = tx.send(Job::Added(id, PathBuf::from(&title), Ok(ls.clone())));
                    }
                    Reply::Features(r.map(Box::new))
                }
                Pending::Search { template, title } => {
                    Reply::Items(title, search(&client, &template, bbox, &cancel))
                }
                Pending::Size { url, title } => {
                    let r = size_of(&client, &url, &cancel);
                    Reply::Sized(url, title, r)
                }
                Pending::Download { url, title } => {
                    let path = cog_dir().join(cog_name(&title, &url));
                    let req = Request::get(url.clone()).limit(COG_BYTES);
                    let r = if path.is_file() {
                        Ok(path)
                    } else {
                        client
                            .download_to(&req, &cancel, &path, |_| {})
                            .map(|_| path)
                            .map_err(|e| crate::error_text::reason(&e))
                    };
                    Reply::Downloaded(r)
                }
            };
            let _ = tx.send(Job::Service(reply));
            l.waker()();
        });
    }

    /// A server answered.
    pub(super) fn gis_service_reply(&mut self, r: Reply) {
        let now = self.now();
        let sv = &mut self.map.services;
        sv.cancel = None;
        if !matches!(sv.page, Some(Page::Busy(_))) {
            // Cancelled meanwhile.
            return;
        }
        sv.page = sv.back.pop();
        match r {
            Reply::Described(i, Ok(caps)) => {
                if let Some(s) = sv.servers().get_mut(i) {
                    let changed =
                        s.kind != Some(caps.kind) || (s.name.is_empty() && !caps.title.is_empty());
                    s.kind = Some(caps.kind);
                    if s.name.is_empty() {
                        s.name = caps.title.clone();
                    }
                    if changed {
                        let list = sv.servers().clone();
                        let _ = save(&servers_file(), &list);
                    }
                }
                sv.go(Page::Layers {
                    server: i,
                    caps: Arc::new(*caps),
                });
                self.gis_services_settings_panel();
            }
            Reply::Features(Ok(_)) => {
                // Added as layers ([`Job::Added`]); the panel steps aside.
                sv.open = false;
                if self.map.focus == Some(Focus::Services) {
                    self.map.focus = None;
                }
            }
            Reply::Items(title, Ok(items)) => sv.go(Page::Items {
                title,
                items: Arc::new(items),
            }),
            Reply::Sized(url, title, Ok(size)) => {
                if size.is_some_and(|s| s > COG_BYTES) {
                    sv.go(Page::Failed(t!(
                        "map-service-too-big",
                        size = size_text(size.unwrap_or(0))
                    )));
                } else {
                    sv.go(Page::Download { url, title, size });
                }
            }
            Reply::Downloaded(Ok(path)) => {
                sv.open = false;
                self.map.focus = None;
                self.chrome.toast(t!("map-service-opened"), None, now);
                self.open_target(path);
            }
            Reply::Described(_, Err(e))
            | Reply::Features(Err(e))
            | Reply::Items(_, Err(e))
            | Reply::Sized(_, _, Err(e))
            | Reply::Downloaded(Err(e)) => sv.go(Page::Failed(e)),
        }
        self.map.bump();
        self.redraw = true;
    }

    /// Rows on the page now.
    fn gis_services_rows(&self) -> usize {
        self.map.services.view(true).map_or(0, |v| v.rows.len())
    }

    /// Enter on the page (the row under the cursor, or the first button).
    pub(super) fn gis_services_activate(&mut self) {
        let sv = &mut self.map.services;
        let cursor = sv.cursor;
        let page = sv.page.clone();
        match page {
            Some(Page::Off) => self.gis_services_button(Button::TurnOn),
            Some(Page::List) => {
                let n = sv.servers().len();
                if cursor < n {
                    self.gis_services_request(Pending::Describe(cursor));
                } else {
                    sv.go(Page::Add {
                        text: String::new(),
                        error: None,
                    });
                }
            }
            Some(Page::Add { .. }) => self.gis_services_button(Button::Connect),
            Some(Page::Consent { .. }) => {}
            Some(Page::Layers { caps, .. }) => {
                let Some(l) = caps.layers.get(cursor) else {
                    return;
                };
                let title = if l.title.is_empty() {
                    l.id.clone()
                } else {
                    l.title.clone()
                };
                match &l.offer {
                    Offer::Tiles(tiles) => {
                        let st = ogc::ServiceTiles {
                            tiles: tiles.clone(),
                            name: title.clone(),
                            credit: l.attribution.clone().unwrap_or_else(|| caps.title.clone()),
                        };
                        self.store.update(Origin::Program, |s| {
                            s.metadata.basemap = specular_settings::Basemap::Service;
                            s.metadata.basemap_service = st.encode();
                        });
                        self.apply_settings(false);
                        let now = self.now();
                        self.chrome
                            .toast(t!("map-service-basemap", layer = title), None, now);
                    }
                    Offer::Features(t) => {
                        let template = t.clone();
                        self.gis_services_request(Pending::Features { template, title });
                    }
                    Offer::Search(t) => {
                        let template = t.clone();
                        self.gis_services_request(Pending::Search { template, title });
                    }
                    Offer::Refused(_) => {}
                }
            }
            Some(Page::Items { items, .. }) => {
                let Some(it) = items.get(cursor) else { return };
                let Some((_, _, url)) = it.cogs.first() else {
                    return;
                };
                let (url, title) = (url.clone(), it.id.clone());
                self.gis_services_request(Pending::Size { url, title });
            }
            Some(Page::Download { .. }) => self.gis_services_button(Button::Download),
            Some(Page::Busy(_) | Page::Failed(_)) | None => {}
        }
    }

    /// A button of the panel.
    pub(super) fn gis_services_button(&mut self, b: Button) {
        match b {
            Button::TurnOn => {
                self.store
                    .update(Origin::Program, |s| s.maps.services = true);
                self.apply_settings(false);
                let sv = &mut self.map.services;
                sv.page = Some(Page::List);
                sv.cursor = 0;
            }
            Button::Close => {
                self.map.services.open = false;
                self.map.focus = None;
            }
            Button::Back | Button::Cancel => {
                if !self.map.services.back() {
                    self.map.services.open = false;
                    self.map.focus = None;
                }
            }
            Button::Connect => {
                let Some(Page::Add { text, .. }) = self.map.services.page.clone() else {
                    return;
                };
                let Some(url) = normalise(&text) else {
                    if let Some(Page::Add { error, .. }) = self.map.services.page.as_mut() {
                        *error = Some(t!("map-service-bad-address"));
                    }
                    return;
                };
                let sv = &mut self.map.services;
                let list = sv.servers();
                let i = match list.iter().position(|s| s.url == url) {
                    Some(i) => i,
                    None => {
                        let id = new_id(list);
                        list.push(Server {
                            id,
                            name: String::new(),
                            url,
                            kind: None,
                        });
                        let list = list.clone();
                        if let Err(e) = save(&servers_file(), &list) {
                            specular_log::warn!(target: specular_log::SETTINGS, "map servers not saved: {e}");
                        }
                        list.len() - 1
                    }
                };
                // Back to the list, then to the server.
                sv.page = Some(Page::List);
                sv.back.clear();
                sv.cursor = i;
                self.gis_services_settings_panel();
                self.gis_services_request(Pending::Describe(i));
            }
            Button::Allow => {
                let Some(Page::Consent { host, then }) = self.map.services.page.clone() else {
                    return;
                };
                self.store.update(Origin::Program, |s| {
                    let mut a = allowed(&s.maps.services_allowed);
                    if !a.contains(&host) {
                        a.push(host.clone());
                    }
                    s.maps.services_allowed = a.join(",");
                });
                self.apply_settings(false);
                self.map.services.page = self.map.services.back.pop();
                self.gis_services_request(then);
            }
            Button::Remove => {
                let cursor = self.map.services.cursor;
                if let Some(id) = self
                    .map
                    .services
                    .servers()
                    .get(cursor)
                    .map(|s| s.id.clone())
                {
                    self.gis_services_ask_remove(id);
                }
            }
            Button::Download => {
                let Some(Page::Download { url, title, .. }) = self.map.services.page.clone() else {
                    return;
                };
                self.gis_services_request(Pending::Download { url, title });
            }
            Button::Write | Button::Guess | Button::Save | Button::Print => {}
        }
        self.map.bump();
        self.redraw = true;
    }

    /// Asks before removing a saved server (names it; nothing on disk is lost).
    fn gis_services_ask_remove(&mut self, id: String) {
        let Some(s) = self
            .map
            .services
            .servers()
            .iter()
            .find(|s| s.id == id)
            .cloned()
        else {
            return;
        };
        let name = if s.name.is_empty() {
            host_of(&s.url).unwrap_or_default()
        } else {
            s.name
        };
        self.map.services.removing = Some(id);
        self.chrome.confirm_destructive(
            t!("map-service-remove-ask", server = name),
            t!("map-service-remove-detail"),
            t!("map-service-remove"),
            REMOVE_ITEM,
        );
        self.redraw = true;
    }

    /// The removal was confirmed.
    pub(in crate::app) fn gis_services_remove_confirmed(&mut self) {
        let Some(id) = self.map.services.removing.take() else {
            return;
        };
        let sv = &mut self.map.services;
        let list = sv.servers();
        let Some(i) = list.iter().position(|s| s.id == id) else {
            return;
        };
        let s = list.remove(i);
        let list = list.clone();
        if let Err(e) = save(&servers_file(), &list) {
            specular_log::warn!(target: specular_log::SETTINGS, "map servers not saved: {e}");
        }
        sv.cursor = sv.cursor.min(list.len());
        let name = if s.name.is_empty() {
            host_of(&s.url).unwrap_or_default()
        } else {
            s.name
        };
        let now = self.now();
        self.chrome
            .toast(t!("map-service-removed", server = name), None, now);
        self.gis_services_settings_panel();
        self.redraw = true;
    }

    /// The saved servers under Settings › Maps › Online map services, each with Remove.
    pub(in crate::app) fn gis_services_settings_panel(&mut self) {
        let list = self.map.services.servers().clone();
        let panel = SettingPanel {
            title: t!("map-service-saved"),
            intro: if list.is_empty() {
                t!("map-service-saved-none")
            } else {
                String::new()
            },
            rows: list
                .iter()
                .map(|s| PanelRow {
                    id: s.id.clone(),
                    label: if s.name.is_empty() {
                        host_of(&s.url).unwrap_or_default()
                    } else {
                        s.name.clone()
                    },
                    value: s.kind.map(|k| k.name().to_owned()).unwrap_or_default(),
                    lines: vec![s.url.clone()],
                    button: Some(t!("map-service-remove")),
                    ..PanelRow::default()
                })
                .collect(),
            keywords: t!("map-service-keywords"),
            ..SettingPanel::default()
        };
        if self.chrome.set_setting_panel(PANEL_KEY, Some(panel)) {
            self.redraw = true;
        }
    }

    /// A Remove button of the settings panel: asks first.
    pub(in crate::app) fn gis_services_panel_event(&mut self, e: &PanelEvent) {
        if let PanelEvent::Row(id) = e {
            self.gis_services_ask_remove(id.clone());
        }
    }

    pub(super) fn gis_services_key(&mut self, c: &KeyCombo, text: Option<&str>) -> bool {
        let typing = matches!(self.map.services.page, Some(Page::Add { .. }));
        let rows = self.gis_services_rows();
        match c.key {
            Key::Named(NamedKey::ArrowDown) if !typing => self.map.services.move_cursor(1, rows),
            Key::Named(NamedKey::ArrowUp) if !typing => self.map.services.move_cursor(-1, rows),
            Key::Named(NamedKey::Home) if !typing => {
                self.map.services.move_cursor(-(rows as isize), rows)
            }
            Key::Named(NamedKey::End) if !typing => {
                self.map.services.move_cursor(rows as isize, rows)
            }
            Key::Named(NamedKey::Enter) => {
                if matches!(self.map.services.page, Some(Page::Consent { .. })) {
                    self.gis_services_button(Button::Allow);
                } else {
                    self.gis_services_activate();
                }
            }
            Key::Named(NamedKey::Delete)
                if !typing && matches!(self.map.services.page, Some(Page::List)) =>
            {
                self.gis_services_button(Button::Remove)
            }
            Key::Named(NamedKey::Backspace) if typing => {
                if let Some(Page::Add { text, error }) = self.map.services.page.as_mut() {
                    text.pop();
                    *error = None;
                }
            }
            _ if typing && !c.mods.ctrl && !c.mods.alt => {
                return text.is_some_and(|t| self.gis_services_text(t));
            }
            _ => return false,
        }
        true
    }

    /// Typed text for the address field.
    pub(super) fn gis_services_text(&mut self, s: &str) -> bool {
        let Some(Page::Add { text, error }) = self.map.services.page.as_mut() else {
            return false;
        };
        text.push_str(s);
        *error = None;
        self.redraw = true;
        true
    }

    /// A press on the services panel; true when taken.
    pub(super) fn gis_services_press(&mut self, lp: (f32, f32)) -> bool {
        let d = self.map.services.drawn.clone();
        if !d.rect.is_some_and(|r| r.contains(lp)) {
            return false;
        }
        self.map.focus = Some(Focus::Services);
        match d.hits.iter().find(|(_, r)| r.contains(lp)).map(|h| h.0) {
            Some(Hit::Row(i)) => {
                let again = self.map.services.cursor == i;
                self.map.services.cursor = i;
                if again {
                    self.gis_services_activate();
                }
            }
            Some(Hit::Button(b)) => self.gis_services_button(b),
            Some(Hit::Field) | None => {}
        }
        self.redraw = true;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use specular_remote::ClientConfig;
    use specular_remote::testing::{ServerOptions, TestResponse, TestServer};

    const WFS: &str = r#"<wfs:WFS_Capabilities version="2.0.0" xmlns:wfs="http://www.opengis.net/wfs/2.0" xmlns:ows="http://www.opengis.net/ows/1.1">
<ows:ServiceIdentification><ows:Title>Test WFS</ows:Title></ows:ServiceIdentification>
<ows:OperationsMetadata><ows:Operation name="GetFeature"><ows:Parameter name="outputFormat"><ows:AllowedValues><ows:Value>application/json</ows:Value></ows:AllowedValues></ows:Parameter></ows:Operation></ows:OperationsMetadata>
<wfs:FeatureTypeList><wfs:FeatureType><wfs:Name>lakes</wfs:Name><wfs:Title>Lakes</wfs:Title></wfs:FeatureType></wfs:FeatureTypeList></wfs:WFS_Capabilities>"#;

    const LAKES: &str = r#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{"name":"Zürichsee"},"geometry":{"type":"Point","coordinates":[8.6,47.25]}}]}"#;

    fn server() -> TestServer {
        TestServer::start(ServerOptions::default(), |r| match (r.path(), r.param("REQUEST").as_deref()) {
            ("/ows", Some("GetCapabilities")) if r.param("SERVICE").as_deref() == Some("WFS") => {
                TestResponse::ok(WFS).header("Content-Type", "text/xml")
            }
            ("/ows", Some("GetCapabilities")) => TestResponse::ok(
                "<ServiceExceptionReport><ServiceException>no such service</ServiceException></ServiceExceptionReport>",
            ),
            ("/ows", Some("GetFeature")) => TestResponse::ok(LAKES).header("Content-Type", "application/json"),
            ("/stac", _) => TestResponse::ok(r#"{"stac_version":"1.0.0","links":[{"rel":"search","href":"/stac/search"}]}"#),
            ("/stac/search", _) => TestResponse::ok(
                r#"{"type":"FeatureCollection","features":[{"id":"S2_1","properties":{"datetime":"2024-06-03T10:20:31Z","eo:cloud_cover":3},"assets":{"visual":{"href":"/cog/visual.tif","type":"image/tiff; application=geotiff; profile=cloud-optimized","roles":["visual"]}}}]}"#,
            ),
            ("/cog/visual.tif", _) => TestResponse::ok(vec![0u8; 1234]),
            _ => TestResponse::status(404),
        })
        .unwrap()
    }

    #[test]
    fn a_wfs_found_by_probing_then_its_features() {
        let s = server();
        let client = Client::new(ClientConfig::for_tests()).unwrap();
        let cancel = CancelToken::new();
        let caps = describe(&client, &s.url("/ows"), &cancel).unwrap();
        assert_eq!(caps.kind, Kind::Wfs);
        assert_eq!(caps.title, "Test WFS");
        let Offer::Features(t) = &caps.layers[0].offer else {
            panic!()
        };
        let ds = features(&client, t, "Lakes", [8.0, 47.0, 9.0, 48.0], &cancel).unwrap();
        assert_eq!(ds.layers[0].features.len(), 1);
        // The server saw the area asked for and nothing else of ours.
        let log = s.log();
        let gf = log
            .iter()
            .find(|r| r.param("REQUEST").as_deref() == Some("GetFeature"))
            .unwrap();
        assert!(
            gf.param("BBOX")
                .unwrap()
                .starts_with("8.000000,47.000000,9.000000,48.000000")
        );
        for r in &log {
            assert!(r.header("cookie").is_none() && r.header("referer").is_none());
        }
    }

    #[test]
    fn a_stac_search_and_its_image() {
        let s = server();
        let client = Client::new(ClientConfig::for_tests()).unwrap();
        let cancel = CancelToken::new();
        let caps = describe(&client, &s.url("/stac"), &cancel).unwrap();
        assert_eq!(caps.kind, Kind::Stac);
        let Offer::Search(t) = &caps.layers[0].offer else {
            panic!("{:?}", caps.layers)
        };
        let items = search(&client, t, [8.0, 47.0, 9.0, 48.0], &cancel).unwrap();
        assert_eq!(items.len(), 1);
        let url = &items[0].cogs[0].2;
        assert_eq!(url, &s.url("/cog/visual.tif"));
        assert_eq!(size_of(&client, url, &cancel).unwrap(), Some(1234));
        assert_eq!(cog_name("S2_1", url), "S2_1_visual.tif");
        assert_eq!(cog_name("a/b", "https://h/x?y"), "a_b_x.tif");
        // Nothing found where the server has nothing.
        assert!(describe(&client, &s.url("/nothing"), &cancel).is_err());
    }

    #[test]
    fn saved_servers_round_trip() {
        let dir = std::env::temp_dir().join(format!("specular-map-servers-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let f = dir.join("map-servers.json");
        assert!(load(&f).is_empty());
        let list = vec![Server {
            id: new_id(&[]),
            name: "Swiss maps".into(),
            url: "https://wms.example/ows".into(),
            kind: Some(Kind::Wms),
        }];
        save(&f, &list).unwrap();
        assert_eq!(load(&f), list);
        assert_eq!(
            normalise("wms.example/ows").as_deref(),
            Some("https://wms.example/ows")
        );
        assert_eq!(normalise("ftp://x"), None);
        assert_eq!(normalise("https://me:secret@x/"), None);
        assert_eq!(normalise(""), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn pages_go_back() {
        let mut s = Services {
            servers: Some(Vec::new()),
            ..Default::default()
        };
        s.go(Page::List);
        s.go(Page::Add {
            text: String::new(),
            error: None,
        });
        s.go(Page::Busy("…".into()));
        s.go(Page::Failed("no".into()));
        assert!(s.back());
        assert_eq!(
            s.page,
            Some(Page::Add {
                text: String::new(),
                error: None
            })
        );
        assert!(s.back());
        assert_eq!(s.page, Some(Page::List));
        assert!(!s.back());
        let v = Services {
            page: Some(Page::List),
            servers: Some(Vec::new()),
            ..Default::default()
        }
        .view(true)
        .unwrap();
        assert_eq!(v.rows.len(), 1, "Add a server");
        assert_eq!(v.buttons, vec![Button::Close]);
    }
}
