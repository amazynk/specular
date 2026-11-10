//! Other tools' edits and sidecar conflicts in Develop.
//!
//! - **Conflict:** a sidecar newer than the library's copy, with a different edit, asks
//!   which to keep, naming both times (and the other Specular that wrote it). Nothing is
//!   saved until it is answered; Esc leaves both as they are and asks again next time.
//! - **Lightroom:** `crs:` settings in the photo's `.xmp` or embedded XMP are imported, one
//!   history step, by the Other tools card's button, or on open by setting when the photo
//!   has no Develop edit. Approximate; never written back.
//! - **darktable:** a `IMG.ext.xmp` with a darktable history shows its module names, and
//!   "Open in darktable" when darktable is installed (launched with the file, nothing else).

use std::path::{Path, PathBuf};

use specular_develop::interop::{crs, darktable};
use specular_develop::model::Step;
use specular_i18n::t;
use specular_widgets::choice::Choice;
use specular_widgets::develop_panel::{Body, Card, Control};

use super::store::Conflict;
use crate::app::App;
use crate::app::items;

/// The Other tools card and its controls.
pub(super) const OTHERS_CARD: u32 = 0x5_0000;
const NOTE: u32 = 0x50_0000;
const IMPORT_LR: u32 = 0x50_0001;
const OPEN_DT: u32 = 0x50_0002;

/// Whether a control is this module's.
pub(super) fn is_ours(control: u32) -> bool {
    (0x50_0000..0x51_0000).contains(&control)
}

/// Other tools' edits of a photo.
#[derive(Clone, Debug, Default)]
pub(super) struct Others {
    pub darktable: Option<darktable::Darktable>,
    /// The darktable program, when there is a darktable edit and darktable is installed.
    pub darktable_bin: Option<PathBuf>,
    /// Lightroom settings that change the picture.
    pub crs: Option<crs::Crs>,
}

impl Others {
    /// Reads them for `path` (background: small reads of the sidecars and the file's head).
    pub fn of(path: &Path) -> Others {
        let darktable = darktable::read_for(path);
        let darktable_bin = darktable.as_ref().and_then(|_| darktable::find_program());
        let crs = lightroom_sidecars(path)
            .into_iter()
            .find_map(|p| std::fs::read_to_string(p).ok().and_then(|t| crs::read(&t)))
            .or_else(|| embedded_xmp(path).and_then(|t| crs::read(&t)))
            .filter(crs::Crs::has_edits);
        Others {
            darktable,
            darktable_bin,
            crs,
        }
    }
}

/// Where Lightroom and Camera Raw keep a photo's settings: `IMG_1234.xmp`, then
/// `IMG_1234.CR2.xmp` (also darktable's; read only for `crs:`).
fn lightroom_sidecars(path: &Path) -> Vec<PathBuf> {
    let mut v = Vec::new();
    for ext in ["xmp", "XMP"] {
        v.push(path.with_extension(ext));
    }
    for ext in [".xmp", ".XMP"] {
        let mut s = path.as_os_str().to_owned();
        s.push(ext);
        v.push(PathBuf::from(s));
    }
    v.retain(|p| p.is_file());
    v
}

/// The XMP packet in the first 8 MiB of `path` (JPEG APP1, TIFF/DNG tag 700, …).
fn embedded_xmp(path: &Path) -> Option<String> {
    use std::io::Read as _;
    let f = specular_core::open_regular(path).ok()?;
    let mut buf = Vec::new();
    f.take(8 << 20).read_to_end(&mut buf).ok()?;
    let start = find(&buf, b"<x:xmpmeta")?;
    let end = start + find(&buf[start..], b"</x:xmpmeta>")? + b"</x:xmpmeta>".len();
    String::from_utf8(buf[start..end].to_vec()).ok()
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

/// The note's text: what darktable did, one line of names.
pub(super) fn darktable_note(d: &darktable::Darktable) -> String {
    // darktable's own module names, untranslated, as its history lists them, a few to a
    // line (the panel is narrow; a note's lines do not wrap).
    let mut lines = vec![t!("develop-darktable-edited")];
    let mut line = String::new();
    for (i, m) in d.modules.iter().enumerate() {
        let piece = if i + 1 < d.modules.len() {
            format!("{m},")
        } else {
            m.clone()
        };
        if !line.is_empty() && line.chars().count() + 1 + piece.chars().count() > NOTE_CHARS {
            lines.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(&piece);
    }
    if !line.is_empty() {
        lines.push(line);
    }
    // The names are always left to right, also in a right-to-left panel.
    for l in lines.iter_mut().skip(1) {
        *l = specular_i18n::isolate_ltr(l);
    }
    lines.join("\n")
}

/// Characters on a line of the darktable note.
const NOTE_CHARS: usize = 24;

/// The Other tools card, when the photo has another tool's edit.
pub(super) fn card(o: &Others, expanded: bool) -> Option<Card> {
    let mut controls = Vec::new();
    if let Some(d) = &o.darktable {
        controls.push(Control {
            id: NOTE,
            body: Body::Note(darktable_note(d)),
        });
        if o.darktable_bin.is_some() {
            controls.push(Control {
                id: OPEN_DT,
                body: Body::Choice(Choice::button(t!("develop-darktable-open"))),
            });
        }
    }
    if o.crs.is_some() {
        controls.push(Control {
            id: IMPORT_LR,
            body: Body::Choice(Choice::button(t!("develop-lightroom-import"))),
        });
    }
    (!controls.is_empty()).then(|| Card {
        id: OTHERS_CARD,
        title: t!("develop-others-title"),
        enabled: None,
        expanded,
        controls,
    })
}

/// A time (seconds since 1970) as local date and time text.
fn when(secs: i64) -> String {
    use chrono::{Datelike as _, TimeZone as _, Timelike as _};
    let Some(t) = chrono::Local.timestamp_opt(secs, 0).single() else {
        return String::new();
    };
    specular_i18n::I18n::global()
        .localizer()
        .date_time(
            (t.year(), t.month() as u8, t.day() as u8),
            (t.hour() as u8, t.minute() as u8),
        )
        .unwrap_or_else(|| t.format("%Y-%m-%d %H:%M").to_string())
}

/// The conflict question's title and detail.
pub(super) fn conflict_text(path: &Path, c: &Conflict) -> (String, String) {
    conflict_text_at(path, c, &when)
}

/// [`conflict_text`] with the times written by `when` (screenshots: a fixed zone).
pub(super) fn conflict_text_at(
    path: &Path,
    c: &Conflict,
    when: &dyn Fn(i64) -> String,
) -> (String, String) {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut detail = vec![
        t!("develop-conflict-file", time = when(c.file_s)),
        t!("develop-conflict-library", time = when(c.library_s)),
    ];
    if let Some(w) = &c.writer {
        detail.push(t!("develop-conflict-writer", writer = w.clone()));
    }
    detail.push(t!("develop-conflict-detail"));
    (t!("develop-conflict-title", file = name), detail.join("\n"))
}

impl App {
    /// The other tools' card follows what the open photo has.
    pub(super) fn develop_others_card(&mut self) {
        let d = &mut self.develop;
        let Some(s) = d.session.as_mut() else {
            return;
        };
        let expanded = s
            .panel
            .cards
            .iter()
            .find(|c| c.id == OTHERS_CARD)
            .is_none_or(|c| c.expanded);
        s.panel.cards.retain(|c| c.id != OTHERS_CARD);
        if let Some(c) = card(&d.others, expanded) {
            s.panel.cards.insert(0, c);
        }
    }

    /// One of the Other tools card's buttons.
    pub(super) fn develop_others_control(&mut self, control: u32) {
        match control {
            IMPORT_LR => self.develop_import_lightroom(),
            OPEN_DT => self.develop_open_darktable(),
            _ => {}
        }
    }

    /// Imports the photo's Lightroom settings as one history step.
    pub(super) fn develop_import_lightroom(&mut self) {
        let Some(c) = self.develop.others.crs.clone() else {
            return;
        };
        let ms = self.develop_ms();
        let Some(s) = self.develop.session.as_mut() else {
            return;
        };
        let raw = super::controls::is_raw(&s.path);
        let (after, used) = crs::import(&c, &s.edit, raw, &|k, tint| {
            super::controls::white_xy(k, tint)
        });
        let step = Step::stack(&t!("develop-history-lightroom"), &s.edit, &after, ms);
        let msg = if step.is_noop() || s.history.push(&mut s.edit, step).is_err() {
            t!("develop-lightroom-nothing")
        } else {
            s.sync_panel();
            self.develop.unsaved = Some(self.now());
            t!("develop-lightroom-imported", count = used.len() as i64)
        };
        let now = self.now();
        self.chrome.toast(msg, None, now);
        self.develop_cards();
    }

    fn develop_open_darktable(&mut self) {
        let (Some(bin), Some(s)) = (
            self.develop.others.darktable_bin.clone(),
            self.develop.session.as_ref(),
        ) else {
            return;
        };
        let path = s.path.clone();
        let now = self.now();
        match std::process::Command::new(&bin).arg(&path).spawn() {
            Ok(_) => self.chrome.announce_status(t!("develop-darktable-opening")),
            Err(e) => {
                specular_log::warn!(target: specular_log::UI, "develop: starting darktable: {e}");
                self.chrome.toast(
                    t!("develop-darktable-failed", reason = e.to_string()),
                    None,
                    now,
                );
            }
        }
    }

    /// Asks which edit to keep when the sidecar changed outside this run.
    pub(super) fn develop_ask_conflict(&mut self, path: &Path, c: Conflict) {
        let (title, detail) = conflict_text(path, &c);
        self.develop.conflict = Some((path.to_path_buf(), self.develop.batch.version, c));
        self.chrome.confirm_choice(
            title,
            detail,
            t!("develop-conflict-use-file"),
            items::DEVELOP_CONFLICT_FILE,
            t!("develop-conflict-keep-library"),
            items::DEVELOP_CONFLICT_LIBRARY,
        );
    }

    /// The conflict's answer; false when `id` is not one.
    pub(super) fn develop_conflict_item(&mut self, id: u64) -> bool {
        let file = match id {
            items::DEVELOP_CONFLICT_FILE => true,
            items::DEVELOP_CONFLICT_LIBRARY => false,
            _ => return false,
        };
        let Some((path, version, c)) = self.develop.conflict.take() else {
            return true;
        };
        let open = self
            .develop
            .session
            .as_ref()
            .is_some_and(|s| s.path == path)
            && self.develop.batch.version == version;
        if !open {
            return true;
        }
        if file {
            let focus = self.develop.session.as_ref().and_then(|s| s.panel.focus());
            let mut n = super::controls::Session::new(path, c.file.edit, c.file.history);
            if focus.is_some() {
                n.panel.set_focus(focus);
            }
            self.develop.session = Some(n);
            self.develop.batch.version_name = c.file.name;
            self.develop_cards();
        }
        // Either way both copies now agree.
        self.develop.unsaved = Some(self.now());
        self.develop_save();
        let msg = if file {
            t!("develop-conflict-used-file")
        } else {
            t!("develop-conflict-kept-library")
        };
        self.chrome.announce_status(msg);
        self.redraw = true;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_packets_are_found() {
        let exe = std::env::current_exe().unwrap();
        let dir = exe
            .parent()
            .and_then(Path::parent)
            .unwrap()
            .join("scratch")
            .join(format!("sidecar-embedded-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("a.jpg");
        let xmp =
            include_str!("../../../../../crates/specular-develop/tests/fixtures/xmp/lightroom.xmp");
        let mut bytes = vec![0xFF, 0xD8, 0xFF, 0xE1, 0, 0];
        bytes.extend_from_slice(b"http://ns.adobe.com/xap/1.0/\0");
        bytes.extend_from_slice(xmp.as_bytes());
        bytes.extend_from_slice(&[0xFF, 0xD9]);
        std::fs::write(&p, &bytes).unwrap();
        let o = Others::of(&p);
        assert!(o.crs.is_some());
        assert!(o.darktable.is_none());
        // A darktable sidecar next to it.
        std::fs::write(
            dir.join("a.jpg.xmp"),
            include_str!("../../../../../crates/specular-develop/tests/fixtures/xmp/darktable.xmp"),
        )
        .unwrap();
        let o = Others::of(&p);
        assert_eq!(o.darktable.as_ref().unwrap().modules.len(), 4);
        let c = card(&o, true).unwrap();
        assert_eq!(c.id, OTHERS_CARD);
        assert!(matches!(&c.controls[0].body, Body::Note(t) if t.contains("sigmoid")));
        assert!(c.controls.iter().any(|c| c.id == IMPORT_LR));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn nothing_from_other_tools_no_card() {
        assert!(card(&Others::default(), true).is_none());
    }
}
