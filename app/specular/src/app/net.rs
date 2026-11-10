//! Downloads: the window's side of [`specular_core::net`]. Every request the app sends to a
//! server reports there; this localises a snapshot for the chrome (in place, the small
//! indicator, the "Downloads" panel), announces starts and ends politely, and carries out
//! Stop. It looks a few times a second while something downloads and not at all otherwise.

use std::time::{Duration, Instant};

use specular_core::net::{self, Activity, Failure, State, What};
use specular_i18n::{ByteUnits, I18n, Localizer, t};
use specular_widgets::{DownloadRow, DownloadState, Downloads, PaletteItem};

use super::App;

/// The palette item "Show downloads".
pub(super) const ITEM: u64 = super::items::DOWNLOADS_SHOW;

/// Looked at most this often while something downloads (bytes move many times a second).
const REFRESH: Duration = Duration::from_millis(250);
/// A download is announced once it has run this long (no chatter for a quick tile).
const ANNOUNCE_AFTER: Duration = Duration::from_secs(2);

#[derive(Debug, Default)]
pub(crate) struct NetState {
    waker_set: bool,
    last: Option<Instant>,
    /// What the chrome shows.
    view: Option<Downloads>,
    /// Activities at the last look: id, whether its start and its end were announced.
    told: Vec<(u64, bool, bool)>,
}

impl App {
    pub(super) fn net_items(&self) -> Vec<PaletteItem> {
        vec![PaletteItem {
            id: ITEM,
            label: t!("net-show"),
            key: None,
            unavailable: None,
            icon: specular_gfx::Icon::Download,
        }]
    }

    /// Opens the panel (the palette; the indicator opens it itself).
    pub(super) fn open_downloads(&mut self) {
        self.net.last = None;
        self.net_tick();
        self.chrome.show_downloads();
        self.redraw = true;
    }

    /// The panel's Stop: "stop:<activity id>", "export-stop" for Develop's export, or a
    /// Develop batch's key.
    pub(super) fn net_command(&mut self, key: &str) {
        if key == "export-stop" {
            self.convert_stop();
        } else if self.develop_batch_stop(key)
            || self.develop_merge_stop(key)
            || self.develop_merges_stop(key)
        {
        } else if let Some(id) = key.strip_prefix("stop:").and_then(|s| s.parse().ok()) {
            net::global().cancel(id);
        }
        self.net_refresh();
        self.redraw = true;
    }

    /// Looks again now (something changed that the indicator shows).
    pub(super) fn net_refresh(&mut self) {
        self.net.last = None;
        self.net_tick();
    }

    /// Follows the registry (and Develop's export queue). Returns when to look again
    /// (`None`: nothing downloads).
    pub(super) fn net_tick(&mut self) -> Option<Instant> {
        let reg = net::global();
        if !self.net.waker_set {
            reg.set_waker(self.loader.waker());
            self.net.waker_set = true;
        }
        let export = self.develop_export_row();
        let mut batches = self.develop_batch_rows();
        batches.extend(self.keywords_rename_rows());
        batches.extend(self.develop_merge_rows());
        batches.extend(self.develop_merges_rows());
        // Idle: one cheap look, nothing drawn.
        if self.net.view.is_none()
            && self.net.told.is_empty()
            && !reg.any()
            && export.is_none()
            && batches.is_empty()
        {
            return None;
        }
        let now = Instant::now();
        if let Some(last) = self.net.last
            && now.duration_since(last) < REFRESH
        {
            return Some(last + REFRESH);
        }
        self.net.last = Some(now);
        let acts = reg.snapshot();
        let running = acts.iter().any(|a| a.state == State::Running);
        let loc = I18n::global().localizer();
        self.announce_net(&acts);
        let mut view = (!acts.is_empty()).then(|| view(&acts, &loc));
        let exporting = export.is_some() || !batches.is_empty();
        for row in export.into_iter().chain(batches) {
            view = Some(with_row(view, row, &loc));
        }
        if view != self.net.view {
            self.chrome.set_downloads(view.clone());
            self.net.view = view;
            self.redraw = true;
        }
        // Finished ones linger: look again when the next goes.
        if running || exporting {
            Some(now + REFRESH)
        } else {
            reg.next_expiry(now)
        }
    }

    /// Polite announcements: a start once it has run a while, its end if its start was
    /// told, and every failure; each once.
    fn announce_net(&mut self, acts: &[Activity]) {
        let mut next = Vec::with_capacity(acts.len());
        for a in acts {
            let prev = self.net.told.iter().find(|t| t.0 == a.id).copied();
            let started = prev.is_some_and(|p| p.1);
            let ended = prev.is_some_and(|p| p.2);
            let what = title(a);
            if a.state == State::Running {
                let tell = !started && a.elapsed >= ANNOUNCE_AFTER;
                if tell {
                    self.chrome.announce_status(t!("net-started", what = what));
                }
                next.push((a.id, started || tell, false));
                continue;
            }
            if !ended {
                let text = match a.state {
                    State::Failed(_) => Some(t!("net-failed-announce", what = what)),
                    State::Cancelled => Some(t!("net-stopped-announce", what = what)),
                    _ if started => Some(t!("net-finished", what = what)),
                    _ => None,
                };
                if let Some(text) = text {
                    self.chrome.announce_status(text);
                }
            }
            next.push((a.id, started, true));
        }
        self.net.told = next;
    }
}

/// What a download is ("Map tiles from tile.example.org").
fn title(a: &Activity) -> String {
    let host = a.label.host.clone();
    match &a.label.what {
        What::Data => t!("net-what-data", host = host),
        What::ImageTiles => t!("net-what-image-tiles", host = host),
        What::PointCloud => t!("net-what-point-cloud", host = host),
        What::MapTiles => t!("net-what-map-tiles", host = host),
        What::MapService => t!("net-what-map-service", host = host),
        What::Dicom => t!("net-what-dicom", host = host),
        What::DicomSeries(s) => t!("net-what-dicom-series", series = s.clone(), host = host),
        What::DicomSend => t!("net-what-dicom-send", host = host),
        What::Omero => t!("net-what-omero", host = host),
        What::ZarrChunks => t!("net-what-zarr-chunks", host = host),
        What::ZarrMetadata => t!("net-what-zarr-metadata", host = host),
        What::Model(name) => t!("net-what-model", name = name.clone()),
        What::UpdateCheck => t!("net-what-update"),
        What::SkySurvey => t!("net-what-sky-survey", host = host),
        What::Stream => t!("net-what-stream", host = host),
    }
}

fn join(a: String, b: String) -> String {
    if b.is_empty() {
        return a;
    }
    if a.is_empty() {
        return b;
    }
    t!("net-join", first = a, second = b)
}

fn plain(n: u32) -> i64 {
    i64::from(n)
}

/// How much and how fast: "45 MB of 120 MB · 2.1 MB/s", "12 of 40 requests · 1.2 MB".
fn detail(a: &Activity, loc: &Localizer) -> String {
    let size = |n: u64| loc.bytes(n, ByteUnits::Decimal);
    let amount = if let Some(i) = a.items {
        // A DICOM retrieval: the server's counts.
        let mut s = t!(
            "net-images",
            done = loc.integer(plain(i.ended())),
            total = loc.integer(plain(i.total())),
            count = plain(i.total())
        );
        if i.failed > 0 {
            s = join(s, t!("net-images-failed", n = loc.integer(plain(i.failed))));
        }
        if i.warning > 0 {
            s = join(
                s,
                t!(
                    "net-images-warning",
                    n = loc.integer(plain(i.warning)),
                    count = plain(i.warning)
                ),
            );
        }
        join(s, size(a.bytes))
    } else if a.grouped() {
        join(
            t!(
                "net-requests",
                done = loc.integer(plain(a.finished)),
                total = loc.integer(plain(a.requests)),
                count = plain(a.requests)
            ),
            size(a.bytes),
        )
    } else {
        match a.total {
            Some(t) if a.state == State::Running => {
                t!("net-bytes-of", done = size(a.bytes), total = size(t))
            }
            _ => size(a.bytes),
        }
    };
    match a.speed {
        Some(s) => join(amount, t!("net-speed", size = size(s.round() as u64))),
        None => amount,
    }
}

fn reason(f: Failure) -> String {
    match f {
        Failure::Offline => t!("net-reason-offline"),
        Failure::Timeout => t!("net-reason-timeout"),
        Failure::Status(code) => t!("net-reason-status", code = i64::from(code)),
        Failure::Auth => t!("net-reason-auth"),
        Failure::Cancelled => t!("net-stopped"),
        Failure::Other => t!("net-reason-other"),
    }
}

/// The time left, or how it ended.
fn status(a: &Activity) -> String {
    match a.state {
        State::Running => match (a.eta, a.speed) {
            (Some(eta), _) => super::work::eta_text(eta),
            (None, None) if a.bytes == 0 => t!("net-starting"),
            _ => t!("net-running"),
        },
        State::Done if a.failed > 0 => t!("net-done-failed", count = plain(a.failed)),
        State::Done => t!("net-done"),
        State::Failed(f) => t!("net-failed", reason = reason(f)),
        State::Cancelled => t!("net-stopped"),
    }
}

fn row(a: &Activity, loc: &Localizer) -> DownloadRow {
    DownloadRow {
        title: title(a),
        detail: detail(a, loc),
        status: status(a),
        fraction: a.fraction,
        state: match a.state {
            State::Running => DownloadState::Running,
            State::Done => DownloadState::Done,
            State::Failed(_) | State::Cancelled => DownloadState::Failed,
        },
        stop: a
            .cancellable
            .then(|| (t!("net-stop"), format!("stop:{}", a.id))),
    }
}

/// `v` (or nothing) with one more running row last (Develop's export queue): the indicator
/// then shows that row's fraction, or the count of running rows.
pub(crate) fn with_row(v: Option<Downloads>, row: DownloadRow, loc: &Localizer) -> Downloads {
    let mut v = v.unwrap_or_default();
    let others = v
        .rows
        .iter()
        .filter(|r| r.state == DownloadState::Running)
        .count();
    v.short = match row.fraction {
        Some(f) if others == 0 => loc.percent(f64::from(f), 0),
        _ => loc.integer(others as i64 + 1),
    };
    if others == 0 {
        v.fraction = row.fraction;
    }
    v.name = t!(
        "net-chip-name",
        status = join(
            row.title.clone(),
            join(row.detail.clone(), row.status.clone())
        )
    );
    v.rows.push(row);
    v
}

/// The chrome's content: a row each (oldest first), the indicator's text and name.
pub(crate) fn view(acts: &[Activity], loc: &Localizer) -> Downloads {
    let rows: Vec<DownloadRow> = acts.iter().map(|a| row(a, loc)).collect();
    let fractions: Option<Vec<f32>> = acts.iter().map(|a| a.fraction).collect();
    let fraction = fractions.map(|f| f.iter().sum::<f32>() / f.len().max(1) as f32);
    let running = acts.iter().filter(|a| a.state == State::Running).count();
    let short = match fraction {
        Some(f) if running > 0 => loc.percent(f64::from(f), 0),
        _ if running > 1 => loc.integer(running as i64),
        _ => match rows.last() {
            Some(r) if r.state != DownloadState::Running => r.status.clone(),
            _ => t!("net-running"),
        },
    };
    let name = rows
        .last()
        .map(|r| {
            t!(
                "net-chip-name",
                status = join(r.title.clone(), join(r.detail.clone(), r.status.clone()))
            )
        })
        .unwrap_or_default();
    Downloads {
        rows,
        short,
        name,
        fraction,
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use specular_core::net::{Label, Registry, What};
    use specular_i18n::I18n;

    use super::view;

    /// The words for what the registry reports: one download with its bytes, speed and time
    /// left; many requests with counts; a failure with its reason; nothing of the URL.
    #[test]
    fn the_rows_read_well_and_carry_no_url() {
        let reg = Registry::new();
        let t0 = Instant::now();
        let mut m = reg.begin_at(
            Label::new(
                What::Model("clip-vit-b32".into()),
                "https://models.example.org/clip.onnx?token=SECRET",
            ),
            t0,
        );
        m.head(Some(120_000_000), 0);
        reg.snapshot_at(t0);
        m.add(45_000_000);
        let tiles: Vec<_> = (0..40)
            .map(|_| {
                reg.begin_at(
                    Label::new(What::MapTiles, "https://user:pw@tile.example.org/1/2/3.png"),
                    t0,
                )
            })
            .collect();
        let mut tiles = tiles.into_iter();
        for _ in 0..12 {
            let mut r = tiles.next().unwrap();
            r.head(Some(100_000), 0);
            r.add(100_000);
            r.finish_at(Ok(()), t0);
        }
        let failed = reg.begin_at(
            Label::new(What::UpdateCheck, "https://u.example/r.json"),
            t0,
        );
        failed.finish_at(
            Err(specular_core::net::Failure::Offline),
            t0 + Duration::from_secs(18),
        );
        let acts = reg.snapshot_at(t0 + Duration::from_secs(20));
        let loc = I18n::global().localizer();
        let mut v = view(&acts, &loc);
        // Fluent isolates the arguments; the words are what is checked here.
        let plain = |s: &mut String| *s = s.replace(['\u{2068}', '\u{2069}'], "");
        for r in &mut v.rows {
            plain(&mut r.title);
            plain(&mut r.detail);
            plain(&mut r.status);
        }
        plain(&mut v.name);
        assert_eq!(v.rows.len(), 3);
        let model = &v.rows[0];
        assert_eq!(model.title, "Model clip-vit-b32");
        assert_eq!(model.detail, "45 MB of 120 MB · 2.3 MB/s", "{model:?}");
        assert_eq!(model.status, "About 35 seconds left");
        assert!(model.stop.is_some());
        let t = &v.rows[1];
        assert_eq!(t.title, "Map tiles from tile.example.org");
        assert!(t.detail.starts_with("12 of 40 requests · 1.2 MB"), "{t:?}");
        assert_eq!(t.status, "Downloading");
        assert_eq!(v.rows[2].title, "Update check");
        assert_eq!(v.rows[2].status, "Failed: no connection");
        assert!(v.rows[2].stop.is_none());
        let all = format!("{v:?}");
        for secret in ["SECRET", "user", "pw@", "clip.onnx", "/1/2/3"] {
            assert!(!all.contains(secret), "{secret} in {all}");
        }
        assert!(v.name.starts_with("Downloads: Update check"), "{}", v.name);
        drop((m, tiles));
    }
}
