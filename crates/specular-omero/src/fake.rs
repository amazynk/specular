//! A fake OMERO.web server for tests and screenshots: made-up projects, screens and images
//! (synthetic gradients, never real data) served from an in-process loopback
//! [`TestServer`], deterministic.
//!
//! It answers the JSON API (`/api/`, the version 0 root, `servers/`, `token/`, `login/`,
//! paginated `m/` listings with `meta.totalCount`), `webgateway/imgData`, rendered tiles
//! (`render_image_region` with `tile=`), whole planes (`render_image`) and thumbnails, in
//! PNG or JPEG. Sign-in is checked as Django does (the `csrftoken` cookie must equal the
//! `X-CSRFToken` header, and a `Referer` is required); sessions use a cookie with a
//! non-default name, can be expired at will, and statuses can be injected.

use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use parking_lot::Mutex;
use serde_json::{Value, json};
use specular_remote::testing::{ServerOptions, TestRequest, TestResponse, TestServer};

/// The session cookie's name (like IDR's `sessionid_prod133`).
pub const SESSION_COOKIE: &str = "sessionid_fake";
/// The CSRF token handed out.
pub const TOKEN: &str = "fake-csrf-0123456789abcdef";

/// Who may browse without signing in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Access {
    /// Everything is readable without a session (a public user signed in on every request).
    #[default]
    Open,
    /// A public session starts by visiting the web client (`/webclient/`).
    Public,
    /// Signing in is required.
    Private,
}

#[derive(Clone, Debug, Default)]
pub struct FakeImage {
    pub id: u64,
    pub name: String,
    /// X, Y, Z, C, T.
    pub size: [u32; 5],
    /// Tile width and height and levels, for a pyramid.
    pub pyramid: Option<(u32, u32, u32)>,
    pub dataset: Option<u64>,
    pub well: Option<u64>,
}

impl FakeImage {
    /// The size of level `l` (halving).
    pub fn level_size(&self, l: u32) -> (u32, u32) {
        let s = 0.5f64.powi(l as i32);
        let d = |n: u32| ((f64::from(n) * s).round() as u32).max(1);
        (d(self.size[0]), d(self.size[1]))
    }
}

#[derive(Clone, Debug, Default)]
pub struct FakeContainer {
    pub id: u64,
    pub name: String,
    pub parent: Option<u64>,
}

#[derive(Clone, Debug, Default)]
pub struct FakeWell {
    pub id: u64,
    pub plate: u64,
    pub row: u32,
    pub column: u32,
}

/// Everything the fake serves.
#[derive(Clone, Debug, Default)]
pub struct FakeConfig {
    /// A path prefix (`/omero`), or empty.
    pub prefix: String,
    pub access: Access,
    /// The one account: user name, password.
    pub account: Option<(String, String)>,
    /// Set the `csrftoken` cookie with the token (IDR does not).
    pub csrf_cookie: bool,
    pub projects: Vec<FakeContainer>,
    pub datasets: Vec<FakeContainer>,
    pub screens: Vec<FakeContainer>,
    pub plates: Vec<FakeContainer>,
    pub wells: Vec<FakeWell>,
    pub images: Vec<FakeImage>,
}

/// The tiled image of [`FakeConfig::synthetic`]: 1000 × 700, three levels of 256² tiles.
pub const TILED: u64 = 101;
/// A small image of 3 Z × 2 T planes and two channels: 64 × 48, served whole.
pub const STACK: u64 = 102;
/// The dataset holding them.
pub const DATASET: u64 = 11;
pub const PROJECT: u64 = 1;
pub const SCREEN: u64 = 31;
pub const PLATE: u64 = 41;

impl FakeConfig {
    /// Two projects (the first with two datasets, the first of them with the tiled image, the
    /// stack and `extra` small images), a screen with one 2 × 3 plate.
    pub fn synthetic(extra: u32) -> FakeConfig {
        let mut images = vec![
            FakeImage {
                id: TILED,
                name: "tiled slide.svs".into(),
                size: [1000, 700, 1, 3, 1],
                pyramid: Some((256, 256, 3)),
                dataset: Some(DATASET),
                well: None,
            },
            FakeImage {
                id: STACK,
                name: "stack.ome.tif".into(),
                size: [64, 48, 3, 2, 2],
                pyramid: None,
                dataset: Some(DATASET),
                well: None,
            },
        ];
        for i in 0..u64::from(extra) {
            images.push(FakeImage {
                id: 110 + i,
                name: format!("cell {:02}.tif", i + 1),
                size: [40 + i as u32, 30, 1, 2, 1],
                pyramid: None,
                dataset: Some(DATASET),
                well: None,
            });
        }
        let mut wells = Vec::new();
        for row in 0..2 {
            for column in 0..3 {
                let id = 50 + u64::from(row * 3 + column);
                wells.push(FakeWell {
                    id,
                    plate: PLATE,
                    row,
                    column,
                });
                images.push(FakeImage {
                    id: 200 + id,
                    name: format!("well {id} field 1"),
                    size: [32, 32, 1, 2, 1],
                    pyramid: None,
                    dataset: None,
                    well: Some(id),
                });
            }
        }
        let c = |id: u64, name: &str, parent: Option<u64>| FakeContainer {
            id,
            name: name.into(),
            parent,
        };
        FakeConfig {
            projects: vec![
                c(PROJECT, "Synthetic project", None),
                c(2, "Empty project", None),
            ],
            datasets: vec![
                c(DATASET, "Gradients", Some(PROJECT)),
                c(12, "Nothing here", Some(PROJECT)),
            ],
            screens: vec![c(SCREEN, "Synthetic screen", None)],
            plates: vec![c(PLATE, "Plate 1", Some(SCREEN))],
            wells,
            images,
            ..FakeConfig::default()
        }
    }

    fn image(&self, id: u64) -> Option<&FakeImage> {
        self.images.iter().find(|i| i.id == id)
    }
}

/// The rendering a request asked for: channels on (by index from 0), greyscale.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FakeRender {
    pub active: Vec<bool>,
    pub greyscale: bool,
}

impl FakeRender {
    /// All of `channels` on, colour.
    pub fn all(channels: u32) -> FakeRender {
        FakeRender {
            active: vec![true; channels as usize],
            greyscale: false,
        }
    }

    fn of(r: &TestRequest, channels: u32) -> FakeRender {
        let mut active = vec![true; channels as usize];
        if let Some(c) = r.param("c") {
            for (i, part) in c.split(',').enumerate() {
                let idx = part.split('|').next().unwrap_or("");
                let on = !idx.starts_with('-');
                let n: usize = idx.trim_start_matches('-').parse().unwrap_or(i + 1);
                if let Some(a) = active.get_mut(n.wrapping_sub(1)) {
                    *a = on;
                }
            }
        }
        FakeRender {
            active,
            greyscale: r.param("m").as_deref() == Some("g"),
        }
    }
}

/// The RGB of level pixel `x`, `y` of `level` of plane `z`, `t`: red across, green down,
/// blue by level and plane; a channel switched off is black.
pub fn pixel(
    img: &FakeImage,
    level: u32,
    x: u32,
    y: u32,
    z: u32,
    t: u32,
    render: &FakeRender,
) -> [u8; 3] {
    let (lw, lh) = img.level_size(level);
    let on = |i: usize| render.active.get(i).copied().unwrap_or(false);
    let r = if on(0) {
        (x * 255 / lw.max(2).saturating_sub(1)).min(255) as u8
    } else {
        0
    };
    let g = if on(1) {
        (y * 255 / lh.max(2).saturating_sub(1)).min(255) as u8
    } else {
        0
    };
    let b = if on(2) || img.size[3] < 3 {
        ((level * 50 + z * 30 + t * 60 + (x / 8 + y / 8) % 2 * 20) % 256) as u8
    } else {
        0
    };
    if render.greyscale {
        let v = if on(0) { r } else { g };
        [v, v, v]
    } else {
        [r, g, b]
    }
}

/// RGB of `w × h` level pixels from `x0`, `y0`.
pub fn region(
    img: &FakeImage,
    level: u32,
    (x0, y0, w, h): (u32, u32, u32, u32),
    z: u32,
    t: u32,
    render: &FakeRender,
) -> Vec<u8> {
    let mut out = Vec::with_capacity(w as usize * h as usize * 3);
    for y in y0..y0 + h {
        for x in x0..x0 + w {
            out.extend_from_slice(&pixel(img, level, x, y, z, t, render));
        }
    }
    out
}

/// The running fake; the server's address is [`base`](Self::base).
pub struct FakeServer {
    pub server: TestServer,
    pub config: Arc<FakeConfig>,
    sessions: Arc<Mutex<HashSet<String>>>,
    /// Statuses answered to the next pixel or listing requests, first first.
    pub fail_next: Arc<Mutex<Vec<u16>>>,
    pub logins: Arc<AtomicU32>,
    /// While set, every request waits (cancel tests).
    pub stall: Arc<AtomicBool>,
}

struct Ctx {
    cfg: Arc<FakeConfig>,
    sessions: Arc<Mutex<HashSet<String>>>,
    fail_next: Arc<Mutex<Vec<u16>>>,
    logins: Arc<AtomicU32>,
    next_session: AtomicU32,
}

impl FakeServer {
    pub fn start(config: FakeConfig, options: ServerOptions) -> std::io::Result<FakeServer> {
        let config = Arc::new(config);
        let sessions = Arc::new(Mutex::new(HashSet::new()));
        let fail_next = Arc::new(Mutex::new(Vec::new()));
        let logins = Arc::new(AtomicU32::new(0));
        let stall = Arc::new(AtomicBool::new(false));
        let ctx = Ctx {
            cfg: config.clone(),
            sessions: sessions.clone(),
            fail_next: fail_next.clone(),
            logins: logins.clone(),
            next_session: AtomicU32::new(1),
        };
        let st = stall.clone();
        let server = TestServer::start(options, move |r| {
            while st.load(Ordering::Relaxed) {
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            handle(&ctx, r)
        })?;
        Ok(FakeServer {
            server,
            config,
            sessions,
            fail_next,
            logins,
            stall,
        })
    }

    /// The OMERO.web address (`http://127.0.0.1:port[/prefix]`).
    pub fn base(&self) -> String {
        self.server.url(&self.config.prefix)
    }

    /// Ends every session (as a server restart or timeout would).
    pub fn expire_sessions(&self) {
        self.sessions.lock().clear();
    }

    /// Answers the next pixel or listing requests with these statuses.
    pub fn fail_next(&self, statuses: &[u16]) {
        let mut f = self.fail_next.lock();
        f.clear();
        f.extend(statuses.iter().rev());
    }

    /// The paths (with queries, prefix removed) requested so far that start with `start`.
    pub fn requested(&self, start: &str) -> Vec<String> {
        self.server
            .log()
            .into_iter()
            .map(|r| {
                r.target
                    .strip_prefix(&self.config.prefix)
                    .unwrap_or(&r.target)
                    .to_string()
            })
            .filter(|t| t.starts_with(start))
            .collect()
    }
}

fn json_ok(v: &Value) -> TestResponse {
    TestResponse::ok(serde_json::to_vec(v).unwrap_or_default())
        .header("Content-Type", "application/json")
}

fn json_status(status: u16, message: &str) -> TestResponse {
    TestResponse::new(
        status,
        serde_json::to_vec(&json!({ "message": message })).unwrap_or_default(),
    )
    .header("Content-Type", "application/json")
}

fn cookie<'a>(r: &'a TestRequest, name: &str) -> Option<&'a str> {
    r.header("cookie")?.split(';').find_map(|c| {
        let (k, v) = c.trim().split_once('=')?;
        (k == name).then_some(v)
    })
}

fn host(r: &TestRequest) -> String {
    format!("http://{}", r.header("host").unwrap_or("127.0.0.1"))
}

fn handle(ctx: &Ctx, r: &TestRequest) -> TestResponse {
    let cfg = &ctx.cfg;
    let Some(path) = r.path().strip_prefix(cfg.prefix.as_str()) else {
        return TestResponse::status(404);
    };
    let path = path.to_string();
    let api = format!("{}{}/api/v0/", host(r), cfg.prefix);
    let signed_in = cookie(r, SESSION_COOKIE).is_some_and(|s| ctx.sessions.lock().contains(s));
    let allowed = signed_in || cfg.access == Access::Open;
    let seg: Vec<&str> = path.trim_matches('/').split('/').collect();
    match seg.as_slice() {
        ["api"] => json_ok(&json!({ "data": [{ "version": "0", "url:base": api }] })),
        ["api", "v0"] => json_ok(&json!({
            "url:experimenters": format!("{api}m/experimenters/"),
            "url:projects": format!("{api}m/projects/"),
            "url:datasets": format!("{api}m/datasets/"),
            "url:images": format!("{api}m/images/"),
            "url:screens": format!("{api}m/screens/"),
            "url:plates": format!("{api}m/plates/"),
            "url:token": format!("{api}token/"),
            "url:servers": format!("{api}servers/"),
            "url:login": format!("{api}login/"),
            "url:save": format!("{api}m/save/"),
        })),
        ["api", "v0", "servers"] => json_ok(&json!({
            "data": [{ "id": 1, "host": "localhost", "port": 4064, "server": "omero" }]
        })),
        ["api", "v0", "token"] => {
            let resp = json_ok(&json!({ "data": TOKEN }));
            if cfg.csrf_cookie {
                resp.header(
                    "Set-Cookie",
                    format!("csrftoken={TOKEN}; Path=/; SameSite=Lax"),
                )
            } else {
                resp
            }
        }
        ["api", "v0", "login"] => login(ctx, r),
        ["webclient"] => match cfg.access {
            Access::Private if !signed_in => TestResponse::status(302)
                .header("Location", format!("{}/webclient/login/", cfg.prefix)),
            _ => {
                let resp = TestResponse::ok("<html><body>web client</body></html>")
                    .header("Content-Type", "text/html; charset=utf-8");
                if signed_in {
                    resp
                } else {
                    resp.header("Set-Cookie", new_session(ctx))
                }
            }
        },
        ["api", "v0", "m", ..] => {
            if !allowed {
                return json_status(403, "Not logged in");
            }
            if let Some(s) = ctx.fail_next.lock().pop() {
                return json_status(s, "injected");
            }
            listing(cfg, r, &seg[3..])
        }
        ["webgateway", ..] => {
            if !allowed {
                // OMERO.web's login_required: a redirect to the sign-in page.
                return TestResponse::status(302).header(
                    "Location",
                    format!("{}/webclient/login/?url={}", cfg.prefix, r.target),
                );
            }
            if let Some(s) = ctx.fail_next.lock().pop() {
                return TestResponse::status(s);
            }
            webgateway(cfg, r, &seg[1..])
        }
        _ => TestResponse::status(404),
    }
}

fn new_session(ctx: &Ctx) -> String {
    let n = ctx.next_session.fetch_add(1, Ordering::Relaxed);
    let id = format!("s{n:08x}");
    ctx.sessions.lock().insert(id.clone());
    format!("{SESSION_COOKIE}={id}; HttpOnly; Path=/; SameSite=Lax")
}

fn login(ctx: &Ctx, r: &TestRequest) -> TestResponse {
    if r.method != "POST" {
        return json_status(405, "POST only");
    }
    // Django's CSRF check: cookie and header agree; a referer is given.
    let header = r.header("x-csrftoken");
    if header.is_none() || header != cookie(r, "csrftoken") || header != Some(TOKEN) {
        return json_status(403, "CSRF verification failed");
    }
    if r.header("referer").is_none_or(str::is_empty) {
        return json_status(403, "CSRF verification failed: no referer");
    }
    let form: Vec<(String, String)> = String::from_utf8_lossy(&r.body)
        .split('&')
        .filter_map(|kv| {
            let (k, v) = kv.split_once('=')?;
            Some((pct(k), pct(v)))
        })
        .collect();
    let get = |k: &str| form.iter().find(|(f, _)| f == k).map(|(_, v)| v.as_str());
    ctx.logins.fetch_add(1, Ordering::Relaxed);
    let Some((user, password)) = &ctx.cfg.account else {
        return json_status(403, "Login failed. Reason: no such user");
    };
    if get("server") != Some("1")
        || get("username") != Some(user)
        || get("password") != Some(password)
    {
        return json_status(403, "Login failed. Reason: Incorrect username or password");
    }
    json_ok(&json!({
        "success": true,
        "eventContext": {
            "userId": 52, "userName": user, "groupId": 3, "groupName": "Fake lab",
            "isAdmin": false, "sessionUuid": "redacted"
        }
    }))
    .header("Set-Cookie", new_session(ctx))
}

/// Percent-decoding of a form value (`+` as space).
fn pct(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < b.len() => {
                let h = std::str::from_utf8(&b[i + 1..i + 3]).ok();
                match h.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                    Some(v) => {
                        out.push(v);
                        i += 2;
                    }
                    None => out.push(b'%'),
                }
            }
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn details() -> Value {
    json!({
        "owner": { "@id": 52, "FirstName": "Ada", "LastName": "Lovelace", "UserName": "ada" },
        "group": { "@id": 3, "Name": "Fake lab" }
    })
}

fn container(c: &FakeContainer, ty: &str, children: usize) -> Value {
    json!({
        "@id": c.id,
        "@type": format!("http://www.openmicroscopy.org/Schemas/OME/2016-06#{ty}"),
        "Name": c.name,
        "Description": format!("{} (synthetic)", c.name),
        "omero:details": details(),
        "omero:childCount": children,
    })
}

fn image_row(i: &FakeImage) -> Value {
    json!({
        "@id": i.id,
        "@type": "http://www.openmicroscopy.org/Schemas/OME/2016-06#Image",
        "Name": i.name,
        "AcquisitionDate": 1_459_513_800_000_i64,
        "omero:details": details(),
        "Pixels": {
            "@id": i.id + 10_000,
            "SizeX": i.size[0], "SizeY": i.size[1], "SizeZ": i.size[2],
            "SizeC": i.size[3], "SizeT": i.size[4],
            "Type": { "value": "uint16" },
            "PhysicalSizeX": { "Value": 0.325, "Symbol": "µm", "Unit": "MICROMETER" },
            "PhysicalSizeY": { "Value": 0.325, "Symbol": "µm", "Unit": "MICROMETER" },
        }
    })
}

fn page(r: &TestRequest, rows: Vec<Value>) -> TestResponse {
    let offset: usize = r.param("offset").and_then(|o| o.parse().ok()).unwrap_or(0);
    let limit: usize = r
        .param("limit")
        .and_then(|o| o.parse().ok())
        .unwrap_or(200)
        .min(500);
    let total = rows.len();
    let data: Vec<Value> = rows.into_iter().skip(offset).take(limit).collect();
    json_ok(&json!({
        "data": data,
        "meta": { "offset": offset, "limit": limit, "maxLimit": 500, "totalCount": total }
    }))
}

fn listing(cfg: &FakeConfig, r: &TestRequest, seg: &[&str]) -> TestResponse {
    let id = |s: &str| s.parse::<u64>().ok();
    let datasets_of = |p: u64| cfg.datasets.iter().filter(move |d| d.parent == Some(p));
    let images_of = |d: u64| cfg.images.iter().filter(move |i| i.dataset == Some(d));
    let plates_of = |s: u64| cfg.plates.iter().filter(move |p| p.parent == Some(s));
    match seg {
        ["projects"] => page(
            r,
            cfg.projects
                .iter()
                .map(|p| container(p, "Project", datasets_of(p.id).count()))
                .collect(),
        ),
        ["screens"] => page(
            r,
            cfg.screens
                .iter()
                .map(|s| container(s, "Screen", plates_of(s.id).count()))
                .collect(),
        ),
        ["projects", p, "datasets"] => match id(p) {
            Some(p) if cfg.projects.iter().any(|x| x.id == p) => page(
                r,
                datasets_of(p)
                    .map(|d| container(d, "Dataset", images_of(d.id).count()))
                    .collect(),
            ),
            _ => json_status(404, "Project not found"),
        },
        ["datasets", d, "images"] => match id(d) {
            Some(d) if cfg.datasets.iter().any(|x| x.id == d) => {
                page(r, images_of(d).map(image_row).collect())
            }
            _ => json_status(404, "Dataset not found"),
        },
        ["screens", s, "plates"] => match id(s) {
            Some(s) if cfg.screens.iter().any(|x| x.id == s) => page(
                r,
                plates_of(s)
                    .map(|p| {
                        let wells = cfg.wells.iter().filter(|w| w.plate == p.id).count();
                        container(p, "Plate", wells)
                    })
                    .collect(),
            ),
            _ => json_status(404, "Screen not found"),
        },
        ["plates", p, "wells"] => match id(p) {
            Some(p) if cfg.plates.iter().any(|x| x.id == p) => page(
                r,
                cfg.wells
                    .iter()
                    .filter(|w| w.plate == p)
                    .map(|w| {
                        let samples: Vec<Value> = cfg
                            .images
                            .iter()
                            .filter(|i| i.well == Some(w.id))
                            .map(|i| json!({ "@id": i.id + 5_000, "Image": image_row(i) }))
                            .collect();
                        json!({
                            "@id": w.id,
                            "@type": "http://www.openmicroscopy.org/Schemas/OME/2016-06#Well",
                            "Row": w.row, "Column": w.column,
                            "WellSamples": samples,
                            "omero:details": details(),
                        })
                    })
                    .collect(),
            ),
            _ => json_status(404, "Plate not found"),
        },
        ["images", i] => match id(i).and_then(|i| cfg.image(i)) {
            Some(i) => json_ok(&json!({ "data": image_row(i) })),
            None => json_status(404, "Image not found"),
        },
        _ => json_status(404, "Not found"),
    }
}

fn img_data(cfg: &FakeConfig, i: &FakeImage) -> Value {
    let dataset = i
        .dataset
        .and_then(|d| cfg.datasets.iter().find(|x| x.id == d));
    let project = dataset
        .and_then(|d| d.parent)
        .and_then(|p| cfg.projects.iter().find(|x| x.id == p));
    let colours = ["FF0000", "00FF00", "0000FF", "FFFFFF"];
    let channels: Vec<Value> = (0..i.size[3])
        .map(|c| {
            json!({
                "label": format!("Ch{}", c + 1),
                "color": colours[c as usize % colours.len()],
                "active": true,
                "emissionWave": 500 + c * 100,
                "window": { "min": 0, "max": 65535, "start": 100 * c, "end": 4000 + 100 * c },
                "inverted": false,
            })
        })
        .collect();
    let mut v = json!({
        "id": i.id,
        "meta": {
            "imageName": i.name,
            "imageDescription": "",
            "imageAuthor": "Ada Lovelace",
            "projectName": project.map(|p| p.name.clone()),
            "projectId": project.map(|p| p.id),
            "datasetName": dataset.map(|d| d.name.clone()),
            "datasetId": dataset.map(|d| d.id),
            "wellId": i.well,
            "imageTimestamp": 1_459_513_800.0,
            "imageId": i.id,
            "pixelsType": "uint16",
        },
        "perms": { "canAnnotate": false, "canEdit": false, "canDelete": false, "canLink": false },
        "tiles": i.pyramid.is_some(),
        "pixel_range": [0, 65535],
        "channels": channels,
        "split_channel": {},
        "size": { "width": i.size[0], "height": i.size[1], "z": i.size[2], "t": i.size[4], "c": i.size[3] },
        "pixel_size": { "x": 0.325, "y": 0.325, "z": if i.size[2] > 1 { json!(1.5) } else { Value::Null } },
        "init_zoom": 0,
        "interpolate": true,
        "rdefs": { "model": "color", "projection": "normal", "defaultZ": i.size[2] / 2, "defaultT": 0, "invertAxis": false },
    });
    if let Some((tw, th, levels)) = i.pyramid {
        v["tile_size"] = json!({ "width": tw, "height": th });
        v["levels"] = json!(levels);
        let scaling: serde_json::Map<String, Value> = (0..levels)
            .map(|l| {
                (
                    l.to_string(),
                    json!(f64::from(i.level_size(l).0) / f64::from(i.size[0])),
                )
            })
            .collect();
        v["zoomLevelScaling"] = Value::Object(scaling);
    }
    v
}

fn encoded(r: &TestRequest, w: u32, h: u32, rgb: &[u8], jpeg_default: bool) -> TestResponse {
    let png = match r.param("format").as_deref() {
        Some("png") => true,
        Some("jpeg") => false,
        _ => !jpeg_default,
    };
    if png {
        match specular_png::encode(w, h, specular_png::ColorType::Rgb, rgb) {
            Ok(b) => TestResponse::ok(b).header("Content-Type", "image/png"),
            Err(_) => TestResponse::status(500),
        }
    } else {
        let opts = specular_jpeg::encode::EncodeOptions {
            quality: 90,
            ..Default::default()
        };
        match specular_jpeg::encode::encode(w, h, specular_jpeg::encode::Samples::Rgb(rgb), &opts) {
            Ok(b) => TestResponse::ok(b).header("Content-Type", "image/jpeg"),
            Err(_) => TestResponse::status(500),
        }
    }
}

fn webgateway(cfg: &FakeConfig, r: &TestRequest, seg: &[&str]) -> TestResponse {
    let n = |s: &str| s.parse::<u32>().ok();
    let image = |s: &str| s.parse::<u64>().ok().and_then(|i| cfg.image(i));
    match seg {
        ["imgData", i] => match image(i) {
            Some(i) => json_ok(&img_data(cfg, i)),
            None => json_status(404, "Image not found"),
        },
        ["render_image_region", i, z, t] => {
            let (Some(img), Some(z), Some(t)) = (image(i), n(z), n(t)) else {
                return TestResponse::status(404);
            };
            if z >= img.size[2] || t >= img.size[4] {
                return TestResponse::status(404);
            }
            let Some(tile) = r.param("tile") else {
                return TestResponse::status(400);
            };
            let p: Vec<u32> = tile.split(',').filter_map(n).collect();
            let [level, col, row, tw, th] = p[..] else {
                return TestResponse::status(400);
            };
            let levels = img.pyramid.map_or(1, |p| p.2);
            if level >= levels || tw == 0 || th == 0 || tw > 1024 || th > 1024 {
                return TestResponse::status(400);
            }
            let (lw, lh) = img.level_size(level);
            let (x, y) = (col * tw, row * th);
            if x >= lw || y >= lh {
                return TestResponse::status(400);
            }
            let (w, h) = (tw.min(lw - x), th.min(lh - y));
            let render = FakeRender::of(r, img.size[3]);
            let rgb = region(img, level, (x, y, w, h), z, t, &render);
            encoded(r, w, h, &rgb, true)
        }
        ["render_image", i, z, t] => {
            let (Some(img), Some(z), Some(t)) = (image(i), n(z), n(t)) else {
                return TestResponse::status(404);
            };
            if z >= img.size[2] || t >= img.size[4] {
                return TestResponse::status(404);
            }
            let render = FakeRender::of(r, img.size[3]);
            let (w, h) = (img.size[0], img.size[1]);
            encoded(r, w, h, &region(img, 0, (0, 0, w, h), z, t, &render), true)
        }
        ["render_thumbnail", i, rest @ ..] => {
            let Some(img) = image(i) else {
                return TestResponse::status(404);
            };
            let size = rest.first().and_then(|s| n(s)).unwrap_or(96).clamp(8, 512);
            let (w, h) = (img.size[0], img.size[1]);
            let (tw, th) = if w >= h {
                (size, (h * size / w).max(1))
            } else {
                ((w * size / h).max(1), size)
            };
            let render = FakeRender::all(img.size[3]);
            let z = img.size[2] / 2;
            let mut rgb = Vec::with_capacity(tw as usize * th as usize * 3);
            for y in 0..th {
                for x in 0..tw {
                    rgb.extend_from_slice(&pixel(img, 0, x * w / tw, y * h / th, z, 0, &render));
                }
            }
            encoded(r, tw, th, &rgb, true)
        }
        _ => TestResponse::status(404),
    }
}
