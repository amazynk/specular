//! File › Open address… (the palette's "Open address…"): a web address typed or pasted,
//! opened through the remote paths the command line has: a Zarr store over HTTP(S) or S3
//! ([`crate::zarr_remote`]), a Cloud-Optimized GeoTIFF ([`crate::cog_remote`]) or a COPC
//! point cloud ([`crate::copc_remote`]) by range requests, or a DICOMweb server (its sheet,
//! with the server chosen or ready to add).
//!
//! Only `http`, `https` and (for Zarr) `s3` addresses are taken. The recent addresses keep
//! the scheme, host and path only: a user name or password in the address, its query
//! (pre-signed URLs carry their keys there) and its fragment are never written. The list
//! shows host and path. With the network off the dialog says so and opens nothing.
//!
//! Ctrl+L, as in browsers and file managers (the vim preset's page right moved to
//! Ctrl+Shift+L for it).

#[cfg(test)]
mod shots;
mod view;

use std::path::PathBuf;

use specular_gfx::{DrawList, Text};
use specular_i18n::t;
use specular_input::{Key, KeyCombo, NamedKey};
use specular_theme::Palette;
use specular_widgets::accesskit;
use specular_widgets::line_edit::{LineEdit, PendingClipboard, single_line};

use self::view::{AddressView, Drawn, Part};
use super::App;
use super::integration::PasteTo;
use crate::loader::Event;

/// Recent addresses kept.
pub(super) const MAX_RECENT: usize = 8;

/// What an address is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind {
    Zarr,
    Cog,
    Copc,
    DicomWeb,
    /// A STAC item, item collection, collection or catalog (JSON).
    Stac,
    /// A media file, playlist, HLS or DASH address: a network stream (`stream.rs`).
    Stream,
}

/// Why an address can't be opened.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Problem {
    Empty,
    /// Not `http://`, `https://` or `s3://`.
    Scheme,
    NoHost,
    /// None of the kinds that open by address.
    Unknown,
}

/// What `s` (as typed or pasted) is.
pub(super) fn classify(s: &str) -> Result<Kind, Problem> {
    let s = s.trim();
    if s.is_empty() {
        return Err(Problem::Empty);
    }
    let (scheme, rest) = s.split_once("://").ok_or(Problem::Scheme)?;
    let scheme = scheme.to_ascii_lowercase();
    if !matches!(scheme.as_str(), "http" | "https" | "s3") {
        return Err(Problem::Scheme);
    }
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host = authority.rsplit('@').next().unwrap_or("");
    if host.is_empty() || s.contains(char::is_whitespace) {
        return Err(Problem::NoHost);
    }
    if crate::zarr_remote::is_remote_zarr(s) {
        return Ok(Kind::Zarr);
    }
    // Every s3:// address is read as a Zarr store (`zarr_remote`).
    if crate::copc_remote::is_remote_copc(s) {
        return Ok(Kind::Copc);
    }
    if crate::cog_remote::is_remote_cog(s) {
        return Ok(Kind::Cog);
    }
    if crate::stac_remote::is_remote_stac(s) {
        return Ok(Kind::Stac);
    }
    let path = rest[authority.len()..]
        .split(['?', '#'])
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    let dicomweb = path.contains("dicom-web")
        || path.contains("dicomweb")
        || path.contains("/wado")
        || path.contains("/dicom/")
        || path.trim_end_matches('/').ends_with("/rs")
        || path.contains("/rs/");
    if dicomweb {
        return Ok(Kind::DicomWeb);
    }
    if super::stream::looks_like_stream(s) {
        return Ok(Kind::Stream);
    }
    Err(Problem::Unknown)
}

/// The plain-language reason for `p`.
pub(super) fn problem_text(p: Problem) -> String {
    match p {
        Problem::Empty => t!("address-problem-empty"),
        Problem::Scheme => t!("address-problem-scheme"),
        Problem::NoHost => t!("address-problem-host"),
        Problem::Unknown => t!("address-problem-kind"),
    }
}

/// The plain-language reason for `p` in the stream dialog (no `s3://` there).
pub(super) fn stream_problem_text(p: Problem) -> String {
    match p {
        Problem::Scheme => t!("stream-problem-scheme"),
        p => problem_text(p),
    }
}

/// The address as the recent list keeps it: scheme, host (and port) and path, without a
/// user name or password, query or fragment.
pub(super) fn recent_entry(s: &str) -> String {
    let s = s.trim();
    let Some((scheme, rest)) = s.split_once("://") else {
        return String::new();
    };
    let rest = rest.split(['?', '#']).next().unwrap_or("");
    let (authority, path) = match rest.find('/') {
        Some(i) => rest.split_at(i),
        None => (rest, ""),
    };
    let host = authority.rsplit('@').next().unwrap_or("");
    format!("{}://{host}{path}", scheme.to_ascii_lowercase())
}

/// What the list shows of a kept entry: host and path.
pub(super) fn shown_entry(entry: &str) -> String {
    entry
        .split_once("://")
        .map_or(entry, |(_, rest)| rest)
        .to_owned()
}

/// `entry` first in `list`, once, at most [`MAX_RECENT`].
pub(super) fn push_recent(list: &mut Vec<String>, entry: String) {
    if entry.is_empty() {
        return;
    }
    list.retain(|e| *e != entry);
    list.insert(0, entry);
    list.truncate(MAX_RECENT);
}

/// The kept list from its file's text: one entry a line, cleaned again (a hand-edited file
/// keeps no secrets either).
pub(super) fn parse_recent(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for l in text.lines().rev() {
        let e = recent_entry(l);
        if classify(&e).is_ok() {
            push_recent(&mut out, e);
        }
    }
    out
}

fn recent_file() -> PathBuf {
    specular_settings::Locations::detect()
        .data_dir
        .join("recent-addresses.txt")
}

/// The dialog's state.
pub(super) struct Dialog {
    field: LineEdit,
    preedit: String,
    focus: Part,
    error: Option<String>,
    recent: Vec<String>,
    drawn: Drawn,
    /// Open network stream: any web address plays as a stream; the recent list is the
    /// recent streams (`recent` holds their addresses).
    stream: bool,
    streams: Vec<super::stream::Recent>,
}

impl Dialog {
    /// A press in the field at `x` (logical px; `now` in seconds, `shift` extends).
    fn press(&mut self, x: f32, now: f64, shift: bool) {
        if let Some((block, x0)) = self.drawn.field_text.clone() {
            let b = block.hit_byte(x - x0).min(self.field.text().len());
            self.field.press(b, x, now, shift);
        }
        self.focus = Part::Field;
    }

    /// The pointer at `x` with the button held after a press in the field: the selection
    /// runs from the press to there. False when no press is being dragged.
    fn drag(&mut self, x: f32) -> bool {
        let Some((block, x0)) = &self.drawn.field_text else {
            return false;
        };
        if !self.field.dragging() {
            return false;
        }
        let at = block.hit_byte(x - x0).min(self.field.text().len());
        self.field.drag_to(at);
        true
    }
}

#[derive(Default)]
pub(super) struct AddressState {
    pub dialog: Option<Dialog>,
}

impl AddressState {
    pub(super) fn is_open(&self) -> bool {
        self.dialog.is_some()
    }

    pub(super) fn view(&self) -> Option<AddressView> {
        let d = self.dialog.as_ref()?;
        Some(build_view(d, specular_core::net::off::is_off()))
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw(
        &mut self,
        dl: &mut DrawList,
        text: &mut Text,
        pal: &Palette,
        v: Option<&AddressView>,
        size: (f32, f32),
        rtl: bool,
        ts: f32,
    ) {
        if let (Some(v), Some(d)) = (v, self.dialog.as_mut()) {
            d.drawn = view::draw(dl, text, pal, v, size, rtl, ts);
        }
    }

    pub(super) fn a11y(&self, tree: &mut accesskit::TreeUpdate, v: &AddressView, scale: f64) {
        if let Some(d) = &self.dialog {
            view::a11y(tree, v, &d.drawn, scale);
        }
    }

    /// The field has the keyboard: input method commits go to it.
    pub(super) fn wants_text(&self) -> bool {
        self.dialog.as_ref().is_some_and(|d| d.focus == Part::Field)
    }

    /// An input method's composition (empty: none) for the field.
    pub(super) fn set_preedit(&mut self, s: &str) {
        let wants = self.wants_text();
        if let Some(d) = self.dialog.as_mut() {
            d.preedit = if wants { s.to_owned() } else { String::new() };
        }
    }

    /// Where the input method's candidate window opens.
    pub(super) fn ime_area(&self) -> Option<specular_gfx::Rect> {
        self.wants_text()
            .then(|| self.dialog.as_ref()?.drawn.ime)
            .flatten()
    }

    /// An input method's commit, typed into the field.
    pub(super) fn commit(&mut self, s: &str) {
        if let Some(d) = self.dialog.as_mut() {
            d.preedit.clear();
            d.error = None;
            d.field.insert(&single_line(s));
        }
    }
}

/// The view of `d`; `net_off`: the network is off (said where an error would be).
fn build_view(d: &Dialog, net_off: bool) -> AddressView {
    let error = d
        .error
        .clone()
        .or_else(|| net_off.then(|| t!("address-network-off")));
    if d.stream {
        return AddressView {
            title: t!("stream-title"),
            intro: t!("stream-intro"),
            label: t!("stream-label"),
            text: d.field.text().to_owned(),
            caret: d.field.caret(),
            selection: d.field.selection(),
            preedit: d.preedit.clone(),
            error,
            recent_label: t!("stream-recent"),
            recent: d.streams.iter().map(super::stream::shown).collect(),
            cancel: t!("app-cancel"),
            open: t!("address-open"),
            focus: Some(d.focus),
        };
    }
    AddressView {
        title: t!("address-title"),
        intro: t!("address-intro"),
        label: t!("address-label"),
        text: d.field.text().to_owned(),
        caret: d.field.caret(),
        selection: d.field.selection(),
        preedit: d.preedit.clone(),
        error,
        recent_label: t!("address-recent"),
        recent: d.recent.iter().map(|e| shown_entry(e)).collect(),
        cancel: t!("app-cancel"),
        open: t!("address-open"),
        focus: Some(d.focus),
    }
}

impl App {
    /// Opens the dialog, the recent addresses read from their file.
    pub(super) fn address_open(&mut self) {
        if self.address.is_open() {
            return;
        }
        let recent = std::fs::read_to_string(recent_file())
            .map(|t| parse_recent(&t))
            .unwrap_or_default();
        self.address.dialog = Some(Dialog {
            field: LineEdit::new(""),
            preedit: String::new(),
            focus: Part::Field,
            error: None,
            recent,
            drawn: Drawn::default(),
            stream: false,
            streams: Vec::new(),
        });
        self.chrome.announce_status(t!("address-title"));
        self.redraw = true;
    }

    /// File › Open network stream…: the dialog for streams, the recent streams listed.
    pub(super) fn address_open_stream(&mut self) {
        if self.address.is_open() {
            return;
        }
        let streams = super::stream::read_recent();
        self.address.dialog = Some(Dialog {
            field: LineEdit::new(""),
            preedit: String::new(),
            focus: Part::Field,
            error: None,
            recent: streams.iter().map(|r| r.url.clone()).collect(),
            drawn: Drawn::default(),
            stream: true,
            streams,
        });
        self.chrome.announce_status(t!("stream-title"));
        self.redraw = true;
    }

    /// The recent streams changed (one was forgotten): the open dialog lists them again.
    pub(super) fn address_recent_changed(&mut self) {
        if let Some(d) = self.address.dialog.as_mut().filter(|d| d.stream) {
            d.streams = super::stream::read_recent();
            d.recent = d.streams.iter().map(|r| r.url.clone()).collect();
            if let Part::Recent(i) = d.focus
                && i >= d.recent.len()
            {
                d.focus = Part::Field;
            }
            self.redraw = true;
        }
    }

    pub(super) fn address_view(&self) -> Option<AddressView> {
        self.address.view()
    }

    fn address_close(&mut self) {
        self.address.dialog = None;
        self.redraw = true;
    }

    /// Open: the address checked, kept in the recent list, then opened.
    fn address_submit(&mut self) {
        let Some(d) = self.address.dialog.as_mut() else {
            return;
        };
        let url = d.field.text().trim().to_owned();
        if d.stream {
            let problem = classify(&url).err().filter(|p| *p != Problem::Unknown);
            if let Some(p) = problem.or_else(|| {
                (!specular_stream::addr::is_stream_url(&url)).then_some(Problem::Scheme)
            }) {
                let msg = stream_problem_text(p);
                d.error = Some(msg.clone());
                d.focus = Part::Field;
                self.chrome.announce_status(msg);
                self.redraw = true;
                return;
            }
            if specular_core::net::off::is_off() {
                let msg = t!("address-network-off");
                d.error = Some(msg.clone());
                self.chrome.announce_status(msg);
                self.redraw = true;
                return;
            }
            self.address_close();
            self.stream_open(url, false);
            return;
        }
        let kind = match classify(&url) {
            Ok(k) => k,
            Err(p) => {
                let msg = problem_text(p);
                d.error = Some(msg.clone());
                d.focus = Part::Field;
                self.chrome.announce_status(msg);
                self.redraw = true;
                return;
            }
        };
        if specular_core::net::off::is_off() {
            let msg = t!("address-network-off");
            d.error = Some(msg.clone());
            self.chrome.announce_status(msg);
            self.redraw = true;
            return;
        }
        if kind == Kind::Stream {
            self.address_close();
            self.stream_open(url, false);
            return;
        }
        let mut recent = d.recent.clone();
        push_recent(&mut recent, recent_entry(&url));
        let file = recent_file();
        self.loader.background("address-recent", move |_| {
            let text: String = recent.iter().map(|e| format!("{e}\n")).collect();
            if let Some(dir) = file.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(&file, text);
        });
        self.address_close();
        self.address_go(kind, url);
    }

    /// Opens `url` of `kind` through its remote path.
    fn address_go(&mut self, kind: Kind, url: String) {
        match kind {
            Kind::Zarr => self.zarr_open(url),
            Kind::Cog | Kind::Copc | Kind::Stac => {
                let address = recent_entry(&url);
                self.loader.background("address-fetch", move |l| {
                    let result = fetch(kind, &url);
                    l.post(Event::ZarrOpened { address, result });
                });
            }
            Kind::DicomWeb => self.dicomweb_open_address(&url),
            Kind::Stream => self.stream_open(url, false),
        }
        self.redraw = true;
    }

    pub(super) fn address_key(&mut self, combo: Option<&KeyCombo>, text: Option<&str>) {
        let Some(d) = self.address.dialog.as_mut() else {
            return;
        };
        self.redraw = true;
        let Some(c) = combo else {
            if d.focus == Part::Field
                && let Some(t) = text
            {
                d.error = None;
                d.field.insert(&single_line(t));
            }
            return;
        };
        let parts = build_view(d, false).focusables();
        let at = parts.iter().position(|p| *p == d.focus).unwrap_or(0) as isize;
        let step = |d: &mut Dialog, by: isize| {
            d.focus = parts[(at + by).rem_euclid(parts.len() as isize) as usize];
        };
        let plain = !c.mods.ctrl && !c.mods.alt && !c.mods.meta;
        match c.key {
            Key::Named(NamedKey::Escape) => return self.address_close(),
            Key::Named(NamedKey::Tab) => return step(d, if c.mods.shift { -1 } else { 1 }),
            Key::Named(NamedKey::Enter) => {
                match d.focus {
                    Part::Cancel => self.address_close(),
                    Part::Recent(i) => {
                        if let Some(e) = d.recent.get(i).cloned() {
                            d.field.set_text(&e);
                            self.address_submit();
                        }
                    }
                    Part::Field | Part::Open => self.address_submit(),
                }
                return;
            }
            Key::Named(NamedKey::Space) if d.focus != Part::Field => {
                match d.focus {
                    Part::Cancel => self.address_close(),
                    Part::Open => self.address_submit(),
                    Part::Recent(i) => {
                        if let Some(e) = d.recent.get(i).cloned() {
                            d.field.set_text(&e);
                            d.focus = Part::Field;
                        }
                    }
                    Part::Field => {}
                }
                return;
            }
            Key::Named(NamedKey::Delete | NamedKey::Backspace)
                if d.stream && matches!(d.focus, Part::Recent(_)) =>
            {
                if let Part::Recent(i) = d.focus
                    && let Some(r) = d.streams.get(i).cloned()
                {
                    self.stream_forget_ask(r);
                }
                return;
            }
            Key::Named(NamedKey::ArrowDown) if plain && d.focus != Part::Open => {
                return step(d, 1);
            }
            Key::Named(NamedKey::ArrowUp) if plain && d.focus != Part::Field => {
                return step(d, -1);
            }
            _ => {}
        }
        if d.focus != Part::Field {
            return;
        }
        // Addresses read left to right in every language.
        let mut clip = PendingClipboard::default();
        let before = d.field.text().to_owned();
        let used = d.field.key(c.key, c.mods, false, &mut clip).used();
        if d.field.text() != before {
            d.error = None;
        }
        if used {
            self.field_clipboard(clip, PasteTo::Address);
            return;
        }
        if plain && let Some(t) = text {
            d.error = None;
            d.field.insert(&single_line(t));
        }
    }

    /// Pasted text for the field (its first line, trimmed: addresses copied from a page or
    /// a terminal often carry spaces or a line break).
    pub(super) fn address_paste(&mut self, s: &str) {
        if let Some(d) = self.address.dialog.as_mut() {
            d.field.paste(
                s.lines()
                    .map(str::trim)
                    .find(|l| !l.is_empty())
                    .unwrap_or(""),
            );
            d.error = None;
            self.redraw = true;
        }
    }

    pub(super) fn address_click(&mut self, p: (f32, f32), down: bool) {
        let now = self.now();
        let shift = self.mods.shift_key();
        let Some(d) = self.address.dialog.as_mut() else {
            return;
        };
        if !down {
            d.field.release();
            return;
        }
        let hit = d
            .drawn
            .hits
            .iter()
            .rev()
            .find(|(r, _)| r.contains(p))
            .map(|x| x.1);
        let Some(h) = hit else { return };
        self.redraw = true;
        match h {
            Part::Field => d.press(p.0, now, shift),
            Part::Recent(i) => {
                if let Some(e) = d.recent.get(i).cloned() {
                    d.field.set_text(&e);
                    d.error = None;
                }
                d.focus = Part::Field;
            }
            Part::Cancel => self.address_close(),
            Part::Open => self.address_submit(),
        }
    }

    /// The pointer moved while the dialog is open: a press in the field drags a selection.
    pub(super) fn address_drag(&mut self, p: (f32, f32)) {
        if self.address.dialog.as_mut().is_some_and(|d| d.drag(p.0)) {
            self.redraw = true;
        }
    }

    /// A screen reader's request on the dialog's nodes; false when not ours.
    pub(super) fn address_a11y_request(&mut self, req: &accesskit::ActionRequest) -> bool {
        let Some(v) = self.address_view() else {
            return false;
        };
        let node = req.target_node.0;
        let Some(part) = view::a11y_target(&v, node) else {
            return node & 0xFF00_0000_0000_0000 == view::A11Y_BASE;
        };
        match req.action {
            accesskit::Action::Click => match part {
                Part::Cancel => self.address_close(),
                Part::Open => self.address_submit(),
                Part::Recent(i) => {
                    if let Some(d) = self.address.dialog.as_mut()
                        && let Some(e) = d.recent.get(i).cloned()
                    {
                        d.field.set_text(&e);
                        d.focus = Part::Field;
                    }
                }
                Part::Field => {}
            },
            accesskit::Action::SetValue => {
                if let (Some(d), Some(accesskit::ActionData::Value(s))) =
                    (self.address.dialog.as_mut(), &req.data)
                {
                    d.field.set_text(&single_line(s));
                    d.error = None;
                }
            }
            _ => {
                if let Some(d) = self.address.dialog.as_mut() {
                    d.focus = part;
                }
            }
        }
        self.redraw = true;
        true
    }
}

/// Fetches a remote GeoTIFF or point cloud into the cache (off the UI thread), shown in the
/// downloads indicator by its client.
fn fetch(kind: Kind, url: &str) -> Result<PathBuf, String> {
    use specular_remote::{CancelToken, Client, ClientConfig};
    let what = match kind {
        Kind::Copc => specular_core::net::What::PointCloud,
        _ => specular_core::net::What::ImageTiles,
    };
    let client = Client::new(ClientConfig::default())
        .map_err(|e| crate::error_text::reason(&e))?
        .tracked(what);
    let cache = specular_settings::Locations::detect().cache_dir;
    let cancel = CancelToken::new();
    match kind {
        Kind::Copc => {
            crate::copc_remote::fetch_to(&client, url, &cache.join("copc"), &cancel, &mut |_, _| {})
                .map(|(p, _)| p)
        }
        Kind::Stac => crate::stac_remote::fetch_arg(url),
        _ => crate::cog_remote::fetch_to(&client, url, &cache.join("cog"), &cancel, &mut |_| {})
            .map(|(p, _)| p),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A press in the address field and a drag select what the pointer passes over (from
    /// the press, either way); a drag without a press selects nothing, and the release ends
    /// it.
    #[test]
    fn dragging_in_the_field_selects() {
        let url = "https://h.example/data/a.zarr";
        let mut text = specular_gfx::Text::bundled_only();
        text.begin_frame(1.0);
        let block = text.layout(
            url,
            specular_gfx::TextStyle::new(14.0),
            None,
            specular_gfx::Align::Start,
        );
        let x0 = 20.0;
        let x_of = |b: usize| x0 + block.caret_x(b);
        let mut d = Dialog {
            field: LineEdit::new(url),
            preedit: String::new(),
            focus: Part::Open,
            error: None,
            recent: Vec::new(),
            drawn: Drawn::default(),
            stream: false,
            streams: Vec::new(),
        };
        d.drawn.field_text = Some((block.clone(), x0));
        assert!(!d.drag(x_of(5)), "no press");
        d.press(x_of(8), 10.0, false);
        assert_eq!(d.focus, Part::Field);
        assert!(d.drag(x_of(17)));
        assert_eq!(d.field.selection(), 8..17);
        assert_eq!(d.field.selected_text(), "h.example");
        // Back past the press: from there to the press.
        assert!(d.drag(x_of(2)));
        assert_eq!(d.field.selection(), 2..8);
        d.field.release();
        assert!(!d.drag(x_of(20)));
        assert_eq!(d.field.selection(), 2..8);
    }

    #[test]
    fn addresses_are_told_apart() {
        assert_eq!(
            classify("https://h.example/data/6001240.zarr"),
            Ok(Kind::Zarr)
        );
        assert_eq!(classify("  s3://bucket/a/img.ome.zarr  "), Ok(Kind::Zarr));
        assert_eq!(classify("https://h/x/scene.tif?sig=1"), Ok(Kind::Cog));
        assert_eq!(classify("http://h/lidar/area.copc.laz"), Ok(Kind::Copc));
        assert_eq!(classify("https://h/stac/item.json"), Ok(Kind::Stac));
        assert_eq!(
            classify("https://pacs.example/dicom-web"),
            Ok(Kind::DicomWeb)
        );
        assert_eq!(
            classify("https://h.example/orthanc/dicom-web/"),
            Ok(Kind::DicomWeb)
        );
        assert_eq!(classify("http://h:8042/wado-rs"), Ok(Kind::DicomWeb));
        assert_eq!(classify(""), Err(Problem::Empty));
        assert_eq!(classify("ftp://h/x.zarr"), Err(Problem::Scheme));
        assert_eq!(classify("file:///home/me/x.tif"), Err(Problem::Scheme));
        assert_eq!(classify("h.example/x.zarr"), Err(Problem::Scheme));
        assert_eq!(classify("s3://bucket/x"), Ok(Kind::Zarr));
        assert_eq!(classify("https:///x.tif"), Err(Problem::NoHost));
        assert_eq!(classify("https://h/a b.tif"), Err(Problem::NoHost));
        assert_eq!(classify("https://h/page.html"), Err(Problem::Unknown));
        // Every problem has words.
        for p in [
            Problem::Empty,
            Problem::Scheme,
            Problem::NoHost,
            Problem::Unknown,
        ] {
            assert!(!problem_text(p).is_empty());
        }
    }

    /// No user name, password, query or fragment is kept; the list shows host and path.
    #[test]
    fn recent_addresses_keep_no_secrets() {
        let e = recent_entry("HTTPS://me:hunter2@h.example:8443/a/x.tif?X-Amz-Signature=abc#f");
        assert_eq!(e, "https://h.example:8443/a/x.tif");
        assert_eq!(shown_entry(&e), "h.example:8443/a/x.tif");
        assert_eq!(
            recent_entry("s3://bucket/z.zarr?versionId=1"),
            "s3://bucket/z.zarr"
        );
        assert_eq!(recent_entry("https://h"), "https://h");
        let mut l = Vec::new();
        for i in 0..12 {
            push_recent(&mut l, format!("https://h/{i}.zarr"));
        }
        push_recent(&mut l, "https://h/5.zarr".into());
        assert_eq!(l.len(), MAX_RECENT);
        assert_eq!(l[0], "https://h/5.zarr");
        assert_eq!(l.iter().filter(|e| *e == "https://h/5.zarr").count(), 1);
        // A file edited by hand is cleaned on reading; what is not an address goes.
        let read = parse_recent(
            "https://u:p@h/a.zarr?k=1\nnot an address\nhttps://h/b.tif\nhttps://h/b.tif\n",
        );
        assert_eq!(read, vec!["https://h/a.zarr", "https://h/b.tif"]);
    }
}
