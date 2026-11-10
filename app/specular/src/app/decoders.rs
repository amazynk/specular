//! Optional decoders in the app (`specular_decode::optional`): other vendors' free software
//! that opens a few formats, used only when the user installed it.
//!
//! - **At the point of need**: a file that needs one shows a card instead of its error,
//!   "<Format> needs <vendor>'s free decoder", with Get it (the vendor's download page in the
//!   browser), I've installed it (looks again now) and Locate… (pick it by hand).
//! - **Found by itself**: the settings go to `optional::configure`; the window coming to the
//!   front looks again in the background, so an install is used without a restart.
//! - **Settings › Optional decoders**: one block per decoder under "Look again": found or
//!   not, where, which version, the "use it" tick box, Get it, Locate…, Forget… (asks first).
//! - **The gallery**: a "Needs decoder" badge (`gallery/draw.rs`); its thumbnails are tried
//!   again once the decoder is found.

use std::path::{Path, PathBuf};

use specular_decode::optional::{self, Entry, OptionalId, State};
use specular_i18n::{I18n, t};
use specular_log::{SETTINGS, info};
use specular_settings::{Origin, Settings};
use specular_widgets::{CheckState, DecoderCard, Modal, PanelEvent, PanelRow, SettingPanel};

use crate::app::App;

/// The setting the page's panel sits under.
pub(super) const UNDER: &str = "decoders.look_again";
/// The confirmation of Forget….
pub(super) const FORGET_ITEM: u64 = super::items::DECODER_FORGET;

/// Card buttons, by position.
const GET: usize = 0;
const INSTALLED: usize = 1;
const LOCATE: usize = 2;

#[derive(Default)]
pub(super) struct Decoders {
    /// `optional::revision` last acted on.
    seen: u64,
    /// What each decoder was then (to notice one becoming ready).
    states: Vec<(OptionalId, State)>,
    /// The panel was built from (revision, UI language generation).
    built: Option<(u64, u64)>,
    /// A Locate… dialog's answer.
    locate_rx: Option<crossbeam_channel::Receiver<(OptionalId, PathBuf)>>,
    /// Forget… is being confirmed for this decoder.
    forgetting: Option<OptionalId>,
    /// When the window last came to the front (looks again at most once a second).
    focus_at: Option<std::time::Instant>,
    /// Looking again in the background (the window came to the front).
    refreshing: bool,
}

/// The decoder settings for `optional::configure`.
pub(super) fn settings_of(s: &Settings) -> optional::Settings {
    optional::Settings {
        braw: s.decoders.braw,
        braw_path: s.decoders.braw_path.clone(),
        dwg: s.decoders.dwg,
        dwg_path: s.decoders.dwg_path.clone(),
        occt: s.decoders.occt,
        occt_path: s.decoders.occt_path.clone(),
        czi: s.decoders.czi,
        czi_path: s.decoders.czi_path.clone(),
        cache: specular_settings::Locations::detect().cache_dir,
    }
}

/// ".braw" or ".stp, .step, .igs, .iges".
fn extensions(id: OptionalId) -> String {
    id.extensions()
        .iter()
        .map(|e| format!(".{e}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The panel for `entries`.
pub(super) fn panel(entries: &[Entry]) -> SettingPanel {
    let mut rows = Vec::new();
    for e in entries {
        let id = e.id;
        let k = id.key();
        let (value, attention) = match &e.state {
            State::Ready { .. } => (t!("decoders-state-found"), false),
            State::Off { .. } => (t!("decoders-state-off"), false),
            State::NotFound => (t!("decoders-state-not-found"), true),
            State::Broken { .. } => (t!("decoders-state-broken"), true),
            State::Refused => (t!("decoders-state-refused"), false),
            State::Planned => (t!("decoders-state-planned"), false),
            State::Checking => (t!("decoders-state-checking"), false),
        };
        let uses = t!(
            "decoders-uses",
            decoder = id.decoder_name(),
            vendor = id.vendor()
        );
        let mut lines = Vec::new();
        let mut detail = String::new();
        match &e.state {
            State::Ready { path, version } | State::Off { path, version } => {
                lines.push(uses);
                if let Some(v) = version {
                    lines.push(t!("decoders-version", version = v.clone()));
                }
                if matches!(e.state, State::Off { .. }) {
                    lines.push(t!("decoders-off"));
                }
                detail = path.display().to_string();
            }
            State::NotFound => {
                lines.push(uses);
                lines.push(t!("decoders-not-found"));
            }
            State::Checking => lines.push(uses),
            State::Broken { path, why } => {
                lines.push(uses);
                lines.push(why.clone());
                detail = path.display().to_string();
            }
            State::Refused => lines.push(match id {
                OptionalId::Ecw => t!("decoders-ecw-why"),
                OptionalId::MrSid => t!("decoders-mrsid-why"),
                OptionalId::DjiThermal => t!("decoders-dji-thermal-why"),
                _ => t!("decoders-r3d-why"),
            }),
            State::Planned => lines.push(t!("decoders-occt-why")),
        }
        let usable = id.usable();
        rows.push(PanelRow {
            id: k.to_owned(),
            label: t!(
                "decoders-row",
                format = id.format_name(),
                extensions = extensions(id)
            ),
            value,
            lines,
            detail,
            attention,
            check: usable.then_some(if e.enabled {
                CheckState::On
            } else {
                CheckState::Off
            }),
            ..Default::default()
        });
        if !usable {
            continue;
        }
        let missing = matches!(e.state, State::NotFound | State::Broken { .. });
        if missing && id.download_url().is_some() {
            rows.push(PanelRow {
                id: format!("{k}:get"),
                label: t!("decoders-get-row"),
                button: Some(t!("decoders-get")),
                indent: 1,
                ..Default::default()
            });
        }
        rows.push(PanelRow {
            id: format!("{k}:locate"),
            label: t!("decoders-locate-row"),
            button: Some(t!("decoders-locate")),
            indent: 1,
            ..Default::default()
        });
        if e.picked {
            rows.push(PanelRow {
                id: format!("{k}:forget"),
                label: t!("decoders-forget-row"),
                button: Some(t!("decoders-forget")),
                indent: 1,
                ..Default::default()
            });
        }
    }
    SettingPanel {
        intro: t!("decoders-intro"),
        rows,
        keywords: t!("decoders-keywords"),
        ..Default::default()
    }
}

/// The card for `file`, which needs `id`.
pub(super) fn card(id: OptionalId, file: &str) -> DecoderCard {
    let mut buttons = Vec::new();
    if id.download_url().is_some() {
        buttons.push(t!("decoders-card-get"));
    }
    buttons.push(t!("decoders-card-installed"));
    buttons.push(t!("decoders-card-locate"));
    DecoderCard {
        id: id.key().to_owned(),
        title: t!(
            "decoders-card-title",
            format = id.format_name(),
            vendor = id.vendor()
        ),
        lines: vec![
            t!(
                "decoders-card-what",
                file = file.to_owned(),
                decoder = id.decoder_name()
            ),
            t!("decoders-card-how"),
        ],
        buttons,
    }
}

/// The card's button `i` as one of [`GET`], [`INSTALLED`], [`LOCATE`] (the card has no Get it
/// when the decoder has no download page).
fn card_button(id: OptionalId, i: usize) -> Option<usize> {
    let i = if id.download_url().is_some() {
        i
    } else {
        i + 1
    };
    (i <= LOCATE).then_some(i)
}

/// Opens `url` in the browser.
fn open_url(url: &str) {
    crate::app::ai::open_url(url.to_owned());
}

impl App {
    /// The settings changed (or the app starts): hand the decoder settings over.
    pub(super) fn decoders_configure(&mut self, s: &Settings) {
        // OpenCASCADE runs in the decode host's CAD pool; it joins the model once.
        static OCCT: std::sync::Once = std::sync::Once::new();
        OCCT.call_once(|| optional::set_occt_bridge(specular_decode::occt_bridge()));
        if optional::configure(settings_of(s)) {
            info!(target: SETTINGS, "optional decoders: {:?}", optional::entries().iter().map(|e| (e.id.key(), &e.state)).collect::<Vec<_>>());
        }
    }

    /// The window came to the front: look again in the background (something may have
    /// been installed meanwhile).
    pub(super) fn decoders_focused(&mut self) {
        let d = &mut self.decoders;
        let now = std::time::Instant::now();
        if d.refreshing
            || d.focus_at
                .is_some_and(|t| now - t < std::time::Duration::from_secs(1))
        {
            return;
        }
        d.focus_at = Some(now);
        d.refreshing = true;
        self.loader.background("decoders-look", |l| {
            optional::refresh();
            l.waker()();
        });
    }

    /// Once per event-loop turn: acts on what changed (a decoder found or lost), keeps the
    /// card and the Settings panel up to date, and takes a Locate… answer.
    pub(super) fn decoders_tick(&mut self) {
        if let Some(rx) = &self.decoders.locate_rx
            && let Ok((id, p)) = rx.try_recv()
        {
            self.decoders.locate_rx = None;
            self.decoders_located(id, &p);
        }
        let rev = optional::revision();
        if rev != self.decoders.seen {
            self.decoders.seen = rev;
            self.decoders.refreshing = false;
            let now: Vec<(OptionalId, State)> = optional::entries()
                .into_iter()
                .map(|e| (e.id, e.state))
                .collect();
            let before = std::mem::replace(&mut self.decoders.states, now.clone());
            for (id, st) in &now {
                let was = before.iter().find(|(i, _)| i == id).map(|(_, s)| s);
                let ready = matches!(st, State::Ready { .. });
                let was_ready = matches!(was, Some(State::Ready { .. }));
                if ready != was_ready || was.is_none() {
                    self.decoders_changed(*id, ready);
                }
            }
            self.redraw = true;
        }
        self.decoders_card();
        if self.chrome.modal() == Some(Modal::Settings) {
            optional::probe_versions();
            let built = (rev, I18n::global().generation());
            if self.decoders.built != Some(built) {
                self.decoders.built = Some(built);
                if self
                    .chrome
                    .set_setting_panel(UNDER, Some(panel(&optional::entries())))
                {
                    self.redraw = true;
                }
            }
        }
    }

    /// Decoder `id` became usable (`ready`) or stopped being: its files' thumbnails are made
    /// again, and the current file is opened again if it was waiting for it.
    fn decoders_changed(&mut self, id: OptionalId, ready: bool) {
        self.gallery_retry_extensions(id.extensions());
        if !ready {
            return;
        }
        let waiting = self.current_id().and_then(|cur| {
            let img = self.images.get(&cur)?;
            (img.error.is_some() && OptionalId::for_path(&img.path) == Some(id)).then_some(())
        });
        if waiting.is_some() {
            info!(target: SETTINGS, "optional decoders: {} found, opening the file again", id.key());
            self.reload_current();
        }
    }

    /// Shows the card when the current file failed for want of a decoder.
    fn decoders_card(&mut self) {
        let card = self.current_id().and_then(|cur| {
            let img = self.images.get(&cur)?;
            img.error.as_ref()?;
            let id = optional::needs_decoder(&img.path)?;
            Some(card(id, &img.name()))
        });
        if self.chrome.set_decoder_card(card) {
            self.redraw = true;
        }
    }

    /// A button of the card.
    pub(super) fn decoders_card_button(&mut self, key: &str, button: usize) {
        let Some(id) = OptionalId::from_key(key) else {
            return;
        };
        match card_button(id, button) {
            Some(GET) => {
                if let Some(u) = id.download_url() {
                    open_url(u);
                }
            }
            Some(INSTALLED) => self.decoders_look_again(Some(id)),
            Some(LOCATE) => self.decoders_locate(id),
            _ => {}
        }
    }

    /// Looks again now (Look again, I've installed it), and says what came of it for `id`.
    pub(super) fn decoders_look_again(&mut self, id: Option<OptionalId>) {
        optional::refresh();
        let text = match id {
            Some(id) => match optional::state(id) {
                State::Ready { .. } => t!("decoders-now-found", decoder = id.decoder_name()),
                _ => t!("decoders-still-missing", decoder = id.decoder_name()),
            },
            None => t!("decoders-looked"),
        };
        let now = self.now();
        self.chrome.toast(text, None, now);
        self.redraw = true;
    }

    /// Asks for the decoder's library, folder or program (Locate…).
    fn decoders_locate(&mut self, id: OptionalId) {
        if self.decoders.locate_rx.is_some() {
            return;
        }
        let (tx, rx) = crossbeam_channel::bounded(1);
        let title = t!("decoders-locate-title", decoder = id.decoder_name());
        self.loader.background("dlg-decoder", move |l| {
            let dlg = rfd::FileDialog::new().set_title(title);
            // The SDK may be a folder (a macOS framework, an SDK's Libraries folder) or a
            // library file; the converter is a program; OpenCASCADE and libCZI a libraries
            // folder.
            let folder = matches!(id, OptionalId::OpenCascade | OptionalId::Czi)
                || (id == OptionalId::Braw && cfg!(target_os = "macos"));
            let picked = if folder {
                dlg.pick_folder()
            } else {
                dlg.pick_file()
            };
            if let Some(p) = picked {
                let _ = tx.send((id, p));
                l.waker()();
            }
        });
        self.decoders.locate_rx = Some(rx);
    }

    /// The user picked `p` for `id`: kept when it is that decoder.
    fn decoders_located(&mut self, id: OptionalId, p: &Path) {
        let ok = match id {
            OptionalId::Braw => optional::braw_library_in(p).is_some(),
            OptionalId::Dwg => specular_decode::find_oda_converter(Some(p)).is_some(),
            // Only the decode host can tell (it loads it): the page says if it can't.
            OptionalId::OpenCascade => p.is_dir(),
            OptionalId::Czi => specular_decode::find_czi(Some(p)).is_ok(),
            OptionalId::R3d | OptionalId::Ecw | OptionalId::MrSid | OptionalId::DjiThermal => false,
        };
        let now = self.now();
        if !ok {
            self.chrome.toast(
                t!("decoders-locate-wrong", decoder = id.decoder_name()),
                None,
                now,
            );
            self.redraw = true;
            return;
        }
        info!(target: SETTINGS, "optional decoders: {} picked at {}", id.key(), p.display());
        let p = p.to_path_buf();
        self.store.update(Origin::SettingsWindow, |s| match id {
            OptionalId::Braw => {
                s.decoders.braw_path = Some(p);
                s.decoders.braw = true;
            }
            OptionalId::OpenCascade => {
                s.decoders.occt_path = Some(p);
                s.decoders.occt = true;
            }
            OptionalId::Czi => {
                s.decoders.czi_path = Some(p);
                s.decoders.czi = true;
            }
            _ => {
                s.decoders.dwg_path = Some(p);
                s.decoders.dwg = true;
            }
        });
        self.chrome.toast(
            t!("decoders-located", decoder = id.decoder_name()),
            None,
            now,
        );
        self.redraw = true;
    }

    /// The page's tick boxes and buttons.
    pub(super) fn decoders_panel_event(&mut self, e: &PanelEvent) {
        match e {
            PanelEvent::Check(k) => {
                let Some(id) = OptionalId::from_key(k) else {
                    return;
                };
                self.store.update(Origin::SettingsWindow, |s| match id {
                    OptionalId::Braw => s.decoders.braw = !s.decoders.braw,
                    OptionalId::Dwg => s.decoders.dwg = !s.decoders.dwg,
                    OptionalId::OpenCascade => s.decoders.occt = !s.decoders.occt,
                    OptionalId::Czi => s.decoders.czi = !s.decoders.czi,
                    OptionalId::R3d
                    | OptionalId::Ecw
                    | OptionalId::MrSid
                    | OptionalId::DjiThermal => {}
                });
            }
            PanelEvent::Row(row) => {
                let Some((k, what)) = row.split_once(':') else {
                    return;
                };
                let Some(id) = OptionalId::from_key(k) else {
                    return;
                };
                match what {
                    "get" => {
                        if let Some(u) = id.download_url() {
                            open_url(u);
                        }
                    }
                    "locate" => self.decoders_locate(id),
                    "forget" => {
                        self.decoders.forgetting = Some(id);
                        self.chrome.confirm_destructive(
                            t!("decoders-forget-ask", decoder = id.decoder_name()),
                            t!("decoders-forget-detail"),
                            t!("decoders-forget-ok"),
                            FORGET_ITEM,
                        );
                    }
                    _ => {}
                }
            }
            PanelEvent::Action | PanelEvent::Expand(_) => return,
        }
        self.redraw = true;
    }

    /// Forget… was confirmed: the path picked by hand goes; the decoder is looked for again.
    pub(super) fn decoders_forget_confirmed(&mut self) {
        let Some(id) = self.decoders.forgetting.take() else {
            return;
        };
        info!(target: SETTINGS, "optional decoders: {} picked path forgotten", id.key());
        self.store.update(Origin::SettingsWindow, |s| match id {
            OptionalId::Braw => s.decoders.braw_path = None,
            OptionalId::OpenCascade => s.decoders.occt_path = None,
            OptionalId::Czi => s.decoders.czi_path = None,
            _ => s.decoders.dwg_path = None,
        });
        let now = self.now();
        self.chrome.toast(
            t!("decoders-forgotten", decoder = id.decoder_name()),
            None,
            now,
        );
        self.redraw = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn entry(id: OptionalId, state: State, picked: bool) -> Entry {
        Entry {
            id,
            state,
            picked,
            enabled: true,
        }
    }

    fn ids(p: &SettingPanel) -> Vec<&str> {
        p.rows.iter().map(|r| r.id.as_str()).collect()
    }

    #[test]
    fn the_panel_lists_each_decoder_with_its_buttons() {
        let found = State::Ready {
            path: PathBuf::from("/usr/lib64/blackmagic/libBlackmagicRawAPI.so"),
            version: Some("6.0 or later".into()),
        };
        let p = panel(&[
            entry(OptionalId::Braw, found, true),
            entry(OptionalId::R3d, State::Refused, false),
            entry(OptionalId::Dwg, State::NotFound, false),
            entry(OptionalId::OpenCascade, State::Planned, false),
        ]);
        assert_eq!(
            ids(&p),
            [
                "braw",
                "braw:locate",
                "braw:forget",
                "r3d",
                "dwg",
                "dwg:get",
                "dwg:locate",
                "occt"
            ]
        );
        let braw = &p.rows[0];
        let label = braw.label.replace(['\u{2068}', '\u{2069}'], "");
        assert_eq!(label, "Blackmagic RAW (.braw)");
        assert_eq!(braw.check, Some(CheckState::On));
        assert!(braw.detail.ends_with("libBlackmagicRawAPI.so"));
        assert!(
            braw.lines.iter().any(|l| l.contains("6.0 or later")),
            "{:?}",
            braw.lines
        );
        // R3D: no switch, no buttons, the reason.
        let r3d = &p.rows[3];
        assert_eq!(r3d.check, None);
        assert!(r3d.lines[0].contains("licence"), "{:?}", r3d.lines);
        assert!(p.rows[4].attention, "not installed stands out");
        assert!(p.matches("blackmagic sdk"));
    }

    #[test]
    fn ecw_and_mrsid_are_listed_with_their_licence_reasons() {
        let p = panel(&[
            entry(OptionalId::Ecw, State::Refused, false),
            entry(OptionalId::MrSid, State::Refused, false),
        ]);
        // No switch, no buttons: only the reason.
        assert_eq!(ids(&p), ["ecw", "mrsid"]);
        for (row, who) in p.rows.iter().zip(["Hexagon", "MrSID SDK"]) {
            assert_eq!(row.check, None);
            assert!(
                row.lines[0].contains(who) && row.lines[0].contains("licence"),
                "{:?}",
                row.lines
            );
        }
        let label = p.rows[1].label.replace(['\u{2068}', '\u{2069}'], "");
        assert_eq!(label, "MrSID (.sid)");
    }

    #[test]
    fn the_card_names_the_decoder_and_its_buttons_map_back() {
        let c = card(OptionalId::Braw, "A001.braw");
        assert!(c.title.contains("Blackmagic RAW") && c.title.contains("Blackmagic Design"));
        assert!(c.lines[0].contains("A001.braw") && c.lines[0].contains("Blackmagic RAW SDK"));
        assert_eq!(c.buttons.len(), 3);
        assert_eq!(card_button(OptionalId::Braw, 0), Some(GET));
        assert_eq!(card_button(OptionalId::Braw, 2), Some(LOCATE));
        assert_eq!(card_button(OptionalId::Braw, 3), None);
        // Without a download page there is no Get it: the buttons shift.
        assert_eq!(card_button(OptionalId::R3d, 0), Some(INSTALLED));
        assert_eq!(card_button(OptionalId::OpenCascade, 0), Some(GET));
    }
}
