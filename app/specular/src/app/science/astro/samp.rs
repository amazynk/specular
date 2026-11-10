//! The SAMP link (Settings › Integration › "Connect to a SAMP hub"): a worker thread owns
//! the client, finds the hub on this computer (trying again every few seconds while none
//! runs), and passes messages both ways. Nothing it does leaves the computer: the hub, the
//! callback server and any URL it follows are checked to be local.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender};
use specular_decode::science::wcs::{dms, hms};
use specular_i18n::t;
use specular_samp::{Client, Incoming, Metadata, Source};

use super::super::super::App;

/// How often a missing hub is looked for, and a connected one pinged.
const RETRY: Duration = Duration::from_secs(4);
const PING: Duration = Duration::from_secs(10);
/// The largest file fetched from another application.
const MAX_FETCH: usize = 1 << 30;

/// What the app asks of the worker.
enum Cmd {
    Image {
        url: String,
        name: String,
    },
    Point {
        ra: f64,
        dec: f64,
    },
    Row {
        table_id: Option<String>,
        url: String,
        row: u64,
    },
    Stop,
}

/// What the worker tells the app.
pub(crate) enum Event {
    Connected,
    NoHub(String),
    /// The hub stopped or let us go; it is looked for again.
    Lost,
    Incoming(Incoming),
    /// A send: how many applications got it, or why it failed.
    Sent(Result<usize, String>),
    /// A file fetched from another application, to open as an image (or a catalogue).
    Fetched {
        path: PathBuf,
        table: Option<(Option<String>, String)>,
    },
    Failed(String),
}

#[derive(Default)]
pub(crate) struct Link {
    cmd: Option<Sender<Cmd>>,
    rx: Option<Receiver<Event>>,
    tx: Option<Sender<Event>>,
    pub connected: bool,
    /// Catalogues being read for `table.load.votable`: table id and URL.
    reading: Vec<(Option<String>, String, Receiver<super::super::sky::Loaded>)>,
}

/// The worker: connects, keeps the connection, sends.
fn worker(cmds: Receiver<Cmd>, events: Sender<Event>, wake: crate::loader::Wake) {
    let meta = Metadata {
        name: "Specular".into(),
        description: "Image viewer".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        doc_url: String::new(),
    };
    let send = |e: Event| {
        let _ = events.send(e);
        wake();
    };
    let mut client: Option<Client> = None;
    // Set by the callback when the hub stops or lets us go.
    let dropped = Arc::new(AtomicBool::new(false));
    let mut told_missing = false;
    let mut last_try = Instant::now() - RETRY;
    let mut last_ping = Instant::now();
    loop {
        if client.is_none() && last_try.elapsed() >= RETRY {
            last_try = Instant::now();
            match specular_samp::lockfile::find_from_env() {
                Ok(info) => {
                    let ev = Mutex::new(events.clone());
                    let w = wake.clone();
                    let gone = dropped.clone();
                    match Client::connect(&info, &meta, move |m| {
                        if matches!(m, Incoming::Disconnected(_)) {
                            gone.store(true, Ordering::Relaxed);
                        } else if let Ok(tx) = ev.lock() {
                            let _ = tx.send(Event::Incoming(m));
                        }
                        w();
                    }) {
                        Ok(c) => {
                            client = Some(c);
                            told_missing = false;
                            last_ping = Instant::now();
                            send(Event::Connected);
                        }
                        Err(e) if !told_missing => {
                            told_missing = true;
                            send(Event::NoHub(e.to_string()));
                        }
                        Err(_) => {}
                    }
                }
                Err(e) if !told_missing => {
                    told_missing = true;
                    send(Event::NoHub(e.to_string()));
                }
                Err(_) => {}
            }
        }
        let gone = dropped.swap(false, Ordering::Relaxed);
        if let Some(c) = &client
            && (gone || last_ping.elapsed() >= PING)
        {
            last_ping = Instant::now();
            if gone || !c.ping() {
                client = None;
                last_try = Instant::now();
                send(Event::Lost);
            }
        }
        match cmds.recv_timeout(Duration::from_millis(500)) {
            Ok(Cmd::Stop) | Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                if let Some(c) = client.take() {
                    c.disconnect();
                }
                return;
            }
            Ok(cmd) => {
                let Some(c) = &client else {
                    send(Event::Sent(Err(t!("disabled-samp-off"))));
                    continue;
                };
                let r = match cmd {
                    Cmd::Image { url, name } => c.send_image(&url, &name),
                    Cmd::Point { ra, dec } => c.point_at(ra, dec),
                    Cmd::Row { table_id, url, row } => {
                        c.highlight_row(table_id.as_deref(), &url, row)
                    }
                    Cmd::Stop => unreachable!(),
                };
                send(Event::Sent(r.map(|v| v.len()).map_err(|e| e.to_string())));
            }
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
        }
    }
}

impl App {
    /// Starts or stops the link as the setting says; handles what came in.
    pub(crate) fn samp_tick(&mut self) {
        let want = self.settings.integration.samp;
        // The readout notes a B1950 image's SAMP and marker conversions.
        let before = (self.astro.samp.connected, self.science.catalogues.len());
        let link = &mut self.astro.samp;
        if want && link.cmd.is_none() {
            let (ctx, crx) = crossbeam_channel::unbounded();
            let (etx, erx) = crossbeam_channel::unbounded();
            let wake = self.loader.waker();
            let tx2 = etx.clone();
            if specular_thread::builder("int-samp")
                .spawn(move || worker(crx, tx2, wake))
                .is_ok()
            {
                link.cmd = Some(ctx);
                link.rx = Some(erx);
                link.tx = Some(etx);
            }
        } else if !want && link.cmd.is_some() {
            if let Some(c) = link.cmd.take() {
                let _ = c.send(Cmd::Stop);
            }
            link.rx = None;
            link.tx = None;
            link.connected = false;
            self.astro.highlight.clear();
            self.redraw = true;
        }
        self.samp_catalogues_poll();
        let events: Vec<Event> = self
            .astro
            .samp
            .rx
            .as_ref()
            .map(|r| r.try_iter().collect())
            .unwrap_or_default();
        for e in events {
            self.samp_event(e);
        }
        if (self.astro.samp.connected, self.science.catalogues.len()) != before {
            self.science_readout();
        }
    }

    fn samp_send(&mut self, c: Cmd) {
        match &self.astro.samp.cmd {
            Some(tx) if self.astro.samp.connected => {
                let _ = tx.send(c);
            }
            _ => {
                let now = self.now();
                self.chrome.toast(t!("disabled-samp-off"), None, now);
            }
        }
    }

    fn samp_event(&mut self, e: Event) {
        let now = self.now();
        match e {
            Event::Connected => {
                self.astro.samp.connected = true;
                self.chrome.toast(t!("astro-samp-connected"), None, now);
            }
            Event::NoHub(why) => {
                self.astro.samp.connected = false;
                self.chrome
                    .toast(t!("astro-samp-no-hub", reason = why), None, now);
            }
            Event::Lost => {
                self.astro.samp.connected = false;
                self.chrome.toast(t!("astro-samp-lost"), None, now);
            }
            Event::Sent(Ok(0)) => {
                self.chrome.toast(t!("astro-samp-nobody"), None, now);
            }
            Event::Sent(Ok(n)) => {
                self.chrome
                    .toast(t!("astro-samp-sent", count = n as i64), None, now);
            }
            Event::Sent(Err(e)) | Event::Failed(e) => {
                self.chrome
                    .toast(t!("astro-samp-failed", reason = e), None, now);
            }
            Event::Fetched { path, table: None } => self.open_target(path),
            Event::Fetched {
                path,
                table: Some((id, url)),
            } => self.samp_read_catalogue(path, id, url),
            Event::Incoming(m) => self.samp_incoming(m),
        }
        self.redraw = true;
    }

    /// A message from another application.
    fn samp_incoming(&mut self, m: Incoming) {
        let now = self.now();
        match m {
            // The worker handles it (and says so).
            Incoming::Disconnected(_) => {}
            Incoming::LoadFits { url, .. } => match specular_samp::client::source(&url) {
                Ok(Source::File(p)) => self.open_target(p),
                Ok(Source::Local(u)) => self.samp_fetch(u, None),
                Err(u) => {
                    self.chrome
                        .toast(t!("astro-samp-refused", url = u), None, now);
                }
            },
            Incoming::LoadVotable { url, table_id, .. } => {
                match specular_samp::client::source(&url) {
                    Ok(Source::File(p)) => self.samp_read_catalogue(p, table_id, url),
                    Ok(Source::Local(u)) => self.samp_fetch(u.clone(), Some((table_id, u))),
                    Err(u) => {
                        self.chrome
                            .toast(t!("astro-samp-refused", url = u), None, now);
                    }
                }
            }
            Incoming::PointAt { ra, dec } => {
                let msg = if self.astro_point_at(ra, dec) {
                    t!("astro-samp-pointed", ra = hms(ra), dec = dms(dec))
                } else {
                    t!("astro-samp-off-image", ra = hms(ra), dec = dms(dec))
                };
                self.chrome.toast(msg, None, now);
            }
            Incoming::HighlightRow { table_id, url, row } => {
                self.samp_rows(table_id.as_deref(), url.as_deref(), &[row]);
            }
            Incoming::SelectRows {
                table_id,
                url,
                rows,
            } => {
                self.samp_rows(table_id.as_deref(), url.as_deref(), &rows);
                let n = self.astro.highlight.len();
                self.chrome
                    .toast(t!("astro-samp-rows", count = n as i64), None, now);
            }
        }
    }

    /// Fetches a file another application serves on this computer, into the cache.
    fn samp_fetch(&mut self, url: String, table: Option<(Option<String>, String)>) {
        let Some(tx) = self.astro.samp.tx.clone() else {
            return;
        };
        let dir = specular_settings::Locations::detect()
            .cache_dir
            .join("samp");
        self.loader.background("int-samp-get", move |l| {
            let r = specular_samp::http::get(&url, MAX_FETCH)
                .map_err(|e| e.to_string())
                .and_then(|b| {
                    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
                    let name = url
                        .split(['?', '#'])
                        .next()
                        .and_then(|p| p.rsplit('/').next())
                        .filter(|n| !n.is_empty())
                        .unwrap_or("samp-file");
                    let p = dir.join(specular_remote::cache::sanitize(name));
                    std::fs::write(&p, b).map_err(|e| e.to_string())?;
                    Ok(p)
                });
            let _ = tx.send(match r {
                Ok(path) => Event::Fetched { path, table },
                Err(e) => Event::Failed(e),
            });
            l.waker()();
        });
    }

    /// Reads a catalogue sent by `table.load.votable` and overlays it, remembering its id.
    fn samp_read_catalogue(&mut self, path: PathBuf, id: Option<String>, url: String) {
        let (tx, rx) = crossbeam_channel::bounded(1);
        self.loader.background("int-samp-table", move |l| {
            let _ = tx.send(super::super::sky::load(path, false));
            l.waker()();
        });
        self.astro.samp.reading.push((id, url, rx));
    }

    fn samp_catalogues_poll(&mut self) {
        let mut done = Vec::new();
        self.astro
            .samp
            .reading
            .retain(|(id, url, rx)| match rx.try_recv() {
                Ok(l) => {
                    done.push((id.clone(), url.clone(), l));
                    false
                }
                Err(crossbeam_channel::TryRecvError::Empty) => true,
                Err(_) => false,
            });
        for (id, url, loaded) in done {
            let now = self.now();
            match loaded {
                super::super::sky::Loaded::Catalogue(name, cat) => {
                    let k = self.science.catalogues.len();
                    let n = cat.lng.len();
                    self.science.catalogues.push((name.clone(), Arc::new(*cat)));
                    self.science.marker_cache = None;
                    self.astro.tables.push((k, id, url));
                    self.chrome.toast(
                        t!("science-catalogue-toast", name = name, count = n as i64),
                        None,
                        now,
                    );
                }
                super::super::sky::Loaded::Failed(name) => {
                    self.chrome
                        .toast(t!("science-catalogue-failed", name = name), None, now);
                }
                super::super::sky::Loaded::Open(_) => {}
            }
            self.redraw = true;
        }
    }

    /// Highlights rows of the catalogue sent as `table_id` (or from `url`).
    fn samp_rows(&mut self, table_id: Option<&str>, url: Option<&str>, rows: &[u64]) {
        let cat = self.astro.tables.iter().find(|(_, id, u)| {
            (table_id.is_some() && id.as_deref() == table_id)
                || (url.is_some() && Some(u.as_str()) == url)
        });
        let Some(&(k, _, _)) = cat else {
            let now = self.now();
            self.chrome.toast(t!("astro-samp-no-table"), None, now);
            return;
        };
        let k = u16::try_from(k).unwrap_or(u16::MAX);
        self.astro.highlight = rows
            .iter()
            .filter_map(|&r| u32::try_from(r).ok())
            .map(|r| (k, r))
            .collect();
        // One row: the view goes to it.
        if let [row] = rows
            && let Some((_, cat)) = self.science.catalogues.get(usize::from(k))
            && let (Some(&a), Some(&b)) = (cat.lng.get(*row as usize), cat.lat.get(*row as usize))
        {
            let (ra, dec) = specular_astro::frames::convert(
                crate::app::science::sky::catalogue_frame(cat),
                specular_astro::frames::Sky::Icrs,
                a,
                b,
            );
            self.astro_point_at(ra, dec);
        }
        self.redraw = true;
    }

    /// `image.load.fits` for the current file.
    pub(crate) fn samp_send_image(&mut self) {
        let Some(p) = self.astro_file().filter(|p| p.is_file()) else {
            let now = self.now();
            self.chrome.toast(t!("astro-solve-not-local"), None, now);
            return;
        };
        let name = p
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let url = specular_samp::lockfile::file_url(&p);
        self.samp_send(Cmd::Image { url, name });
    }

    /// `coord.pointAt.sky` for the position under the pointer (or the view's centre).
    pub(crate) fn samp_send_position(&mut self) {
        let Some((ra, dec)) = self.astro_pointer_sky() else {
            let now = self.now();
            self.chrome.toast(t!("astro-regions-no-wcs"), None, now);
            return;
        };
        self.samp_send(Cmd::Point { ra, dec });
    }

    /// `table.highlight.row` for a catalogue source clicked (catalogues sent through SAMP,
    /// or read from a local file).
    pub(crate) fn samp_highlight(&mut self, cat: u16, row: u32) {
        if !self.astro.samp.connected {
            return;
        }
        let k = usize::from(cat);
        let (id, url) = match self.astro.tables.iter().find(|(i, _, _)| *i == k) {
            Some((_, id, url)) => (id.clone(), url.clone()),
            None => return,
        };
        self.samp_send(Cmd::Row {
            table_id: id,
            url,
            row: u64::from(row),
        });
    }
}
