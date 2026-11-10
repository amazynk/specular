//! Settings › Local AI › Install from file… / Install from folder…: model files copied to
//! this computer by hand (offline and air-gapped machines), through [`Pack::sideload`].
//!
//! A known file (its SHA-256 in the shipped manifest) is copied in and its feature then
//! installs without the network. An unknown one is refused, unless its name is a model's
//! file name: then the app asks first, naming the file, whether to use it unverified.

use std::path::PathBuf;
use std::sync::Arc;

use crossbeam_channel::{Receiver, TryRecvError};
use parking_lot::Mutex;
use specular_ai::{Model, Pack, Sideload};
use specular_i18n::{isolate_ltr, t};
use specular_widgets::Modal;

use super::STATE;
use crate::app::App;

/// The settings' action keys.
pub(crate) const FROM_FILE: &str = "ai.install_from_file";
pub(crate) const FROM_FOLDER: &str = "ai.install_from_folder";
/// "Use an unverified model?" accepted.
pub(crate) const UNVERIFIED_ITEM: u64 = crate::app::items::AI_UNVERIFIED;

/// What is under way.
#[derive(Default)]
struct Side {
    /// A pick and copy running: what it found.
    running: Option<Receiver<Sideload>>,
    /// Files waiting to be asked about, with their model.
    ask: Vec<(PathBuf, Model)>,
    /// The file being asked about now.
    asking: Option<(PathBuf, Model)>,
    /// An unverified copy running: its file name, and why it failed.
    copying: Option<Receiver<(String, Result<(), String>)>>,
}

static SIDE: Mutex<Option<Side>> = Mutex::new(None);

fn name_of(p: &std::path::Path) -> String {
    isolate_ltr(
        &p.file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| p.display().to_string()),
    )
}

/// The words for what a sideload found: (notices, warnings).
pub(crate) fn summary(s: &Sideload) -> (Vec<String>, Vec<String>) {
    let mut notes = Vec::new();
    let mut warn = Vec::new();
    if !s.ready.is_empty() {
        notes.push(t!("ai-sideload-ready", count = s.ready.len() as i64));
    }
    if !s.unknown.is_empty() {
        // The first few by name; more would not fit a notice.
        let mut names: Vec<String> = s.unknown.iter().take(3).map(|p| name_of(p)).collect();
        if s.unknown.len() > 3 {
            names.push("…".into());
        }
        warn.push(t!("ai-sideload-unknown", file = names.join(", ")));
    }
    for (p, why) in &s.failed {
        warn.push(t!(
            "ai-sideload-failed",
            file = name_of(p),
            reason = crate::error_text::text_reason(why)
        ));
    }
    if s.ready.is_empty() && s.unknown.is_empty() && s.failed.is_empty() && s.unverified.is_empty()
    {
        warn.push(t!("ai-sideload-none"));
    }
    (notes, warn)
}

fn pack() -> Option<Arc<Pack>> {
    STATE.lock().pages.as_ref().and_then(|p| p.pack.clone())
}

impl App {
    /// Install from file… (`folder`: …from folder): pick, then look and copy on a worker.
    pub(crate) fn ai_sideload(&mut self, folder: bool) {
        let mut side = SIDE.lock();
        let side = side.get_or_insert_with(Side::default);
        if side.running.is_some() || self.args.screenshot.is_some() {
            return;
        }
        let Some(pack) = pack() else {
            specular_log::info!(target: specular_log::JOBS, "ai: sideload before the pack is ready");
            return;
        };
        let (tx, rx) = crossbeam_channel::bounded(1);
        self.loader.background("dlg-ai-sideload", move |l| {
            let title = if folder {
                t!("settings-ai-install-from-folder")
            } else {
                t!("settings-ai-install-from-file")
            };
            let d = rfd::FileDialog::new().set_title(title);
            let picked: Vec<PathBuf> = if folder {
                d.pick_folder().into_iter().collect()
            } else {
                d.pick_files().unwrap_or_default()
            };
            if picked.is_empty() {
                return;
            }
            let _ = tx.send(pack.sideload(&picked));
            l.waker()();
        });
        side.running = Some(rx);
    }

    /// Picks up what the workers found or copied (once per turn, from the AI page's drain).
    pub(crate) fn ai_sideload_tick(&mut self) {
        let mut found = None;
        let mut copied = None;
        let mut declined = false;
        {
            let mut g = SIDE.lock();
            let Some(side) = g.as_mut() else {
                return;
            };
            // Disconnected: the picker was cancelled (or the worker failed), nothing came.
            if let Some(rx) = &side.running {
                match rx.try_recv() {
                    Ok(s) => {
                        side.running = None;
                        found = Some(s);
                    }
                    Err(TryRecvError::Disconnected) => side.running = None,
                    Err(TryRecvError::Empty) => {}
                }
            }
            if let Some(rx) = &side.copying {
                match rx.try_recv() {
                    Ok(r) => {
                        side.copying = None;
                        copied = Some(r);
                    }
                    Err(TryRecvError::Disconnected) => side.copying = None,
                    Err(TryRecvError::Empty) => {}
                }
            }
            // The question went away without "Use unverified model": skipped; ask the next.
            if side.asking.is_some()
                && side.copying.is_none()
                && self.chrome.modal() != Some(Modal::ConfirmTrash)
            {
                side.asking = None;
                declined = true;
            }
        }
        if declined {
            self.ai_ask_unverified();
        }
        let now = self.now();
        if let Some(s) = found {
            let (notes, warns) = summary(&s);
            for n in notes {
                self.chrome.toast(n, None, now);
            }
            for w in warns {
                self.chrome.warning(w, now);
            }
            let idle = {
                let mut g = SIDE.lock();
                let side = g.get_or_insert_with(Side::default);
                side.ask.extend(s.unverified);
                side.asking.is_none() && side.copying.is_none()
            };
            if idle {
                self.ai_ask_unverified();
            }
            super::STALE.store(true, std::sync::atomic::Ordering::Relaxed);
            self.redraw = true;
        }
        if let Some((file, r)) = copied {
            match r {
                Ok(()) => self
                    .chrome
                    .toast(t!("ai-sideload-ready", count = 1), None, now),
                Err(why) => self.chrome.warning(
                    t!(
                        "ai-sideload-failed",
                        file = file,
                        reason = crate::error_text::text_reason(&why)
                    ),
                    now,
                ),
            };
            super::STALE.store(true, std::sync::atomic::Ordering::Relaxed);
            self.ai_ask_unverified();
            self.redraw = true;
        }
    }

    /// Asks about the first file waiting: "Use an unverified model?", naming it.
    fn ai_ask_unverified(&mut self) {
        let next = {
            let mut g = SIDE.lock();
            g.as_mut().and_then(|s| {
                if s.asking.is_some() || s.ask.is_empty() {
                    return None;
                }
                s.asking = Some(s.ask.remove(0));
                s.asking.clone()
            })
        };
        let Some((path, model)) = next else {
            return;
        };
        self.chrome.confirm_destructive(
            t!("ai-unverified-title"),
            t!(
                "ai-unverified-detail",
                file = name_of(&path),
                model = isolate_ltr(&model.key())
            ),
            t!("ai-unverified-use"),
            UNVERIFIED_ITEM,
        );
        self.redraw = true;
    }

    /// "Use unverified model" pressed: copy the file in as its model, then ask the next.
    pub(crate) fn ai_unverified_confirmed(&mut self) {
        let Some(pack) = pack() else {
            return;
        };
        let mut g = SIDE.lock();
        let Some(side) = g.as_mut() else {
            return;
        };
        let Some((path, model)) = side.asking.take() else {
            return;
        };
        let (tx, rx) = crossbeam_channel::bounded(1);
        self.loader.background("ai-sideload", move |l| {
            let r = pack.sideload_unverified(&model, &path);
            let _ = tx.send((name_of(&path), r));
            l.waker()();
        });
        side.copying = Some(rx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_summary_names_what_was_refused() {
        let s = Sideload::default();
        let (n, w) = summary(&s);
        assert!(n.is_empty());
        assert_eq!(w, vec![t!("ai-sideload-none")]);
        let s = Sideload {
            ready: vec![(PathBuf::from("/m/a.onnx"), specular_ai::Feature::Search)],
            unknown: vec![PathBuf::from("/m/cat.jpg")],
            ..Sideload::default()
        };
        let (n, w) = summary(&s);
        assert_eq!(n.len(), 1);
        assert!(w[0].contains("cat.jpg"), "{w:?}");
        let s = Sideload {
            unknown: (0..5).map(|i| PathBuf::from(format!("/m/{i}.x"))).collect(),
            ..Sideload::default()
        };
        let w = &summary(&s).1[0];
        assert!(
            w.contains("0.x") && w.contains("2.x") && !w.contains("3.x"),
            "{w}"
        );
    }
}
