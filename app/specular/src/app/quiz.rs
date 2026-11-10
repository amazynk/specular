//! Teaching quiz: step through cases with their
//! findings hidden, and reveal them one case at a time.
//!
//! The cases are the key images marked this session (`K`), or a teaching case folder
//! exported before ("Teaching case folder" in Export key images…, its `case.json`). While a
//! case is hidden, the measurements and markup on the image are not drawn; a card at the
//! bottom says which case this is and how to go on. Space or R reveals the findings (each
//! key image's measurements, then the case notes) and draws the annotations again; → and ←
//! (mirrored in right-to-left layouts), Page Down and Page Up step; Esc ends the quiz.

use std::path::{Path, PathBuf};

use crossbeam_channel::Receiver;
use specular_gfx::{Align, DrawList, Rect, Text, TextStyle};
use specular_i18n::t;
use specular_input::{Key, KeyCombo, Mods, NamedKey};
use specular_theme::Palette;
use specular_theme::metrics::{FONT_LABEL, FONT_SMALL, RADIUS_LG};

use super::App;

/// One case: the image to show (its file and the frame or page marked, 0-based), a title
/// without patient details, its findings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Card {
    pub path: PathBuf,
    pub frame: u32,
    pub title: String,
    pub findings: Vec<String>,
}

/// A quiz running.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Quiz {
    pub cards: Vec<Card>,
    /// The case notes (shown with every case's findings).
    pub notes: String,
    pub at: usize,
    pub revealed: bool,
}

impl Quiz {
    pub fn new(cards: Vec<Card>, notes: String) -> Option<Quiz> {
        (!cards.is_empty()).then_some(Quiz {
            cards,
            notes,
            at: 0,
            revealed: false,
        })
    }

    /// Steps by `d` cases (stays within the set); whether it moved. The next case starts
    /// hidden.
    pub fn step(&mut self, d: i32) -> bool {
        let to = (self.at as i64 + i64::from(d)).clamp(0, self.cards.len() as i64 - 1) as usize;
        if to == self.at {
            return false;
        }
        self.at = to;
        self.revealed = false;
        true
    }

    pub fn card(&self) -> &Card {
        &self.cards[self.at]
    }

    /// The lines revealed: the findings (or that none were recorded), then the notes.
    pub fn revealed_lines(&self) -> Vec<String> {
        let mut v: Vec<String> = self.card().findings.clone();
        if v.is_empty() {
            v.push(t!("quiz-no-findings"));
        }
        let notes = self.notes.trim();
        if !notes.is_empty() {
            v.push(t!("quiz-notes", notes = notes.to_owned()));
        }
        v
    }
}

/// The quiz's state in the app.
#[derive(Default)]
pub(super) struct QuizState {
    pub run: Option<Quiz>,
    /// The folder picker, while open.
    picking: Option<Receiver<Option<PathBuf>>>,
}

/// What the card draws.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct CardView {
    pub heading: String,
    pub title: String,
    pub lines: Vec<String>,
    pub hint: String,
}

impl Quiz {
    pub fn view(&self) -> CardView {
        let n = self.cards.len() as i64;
        CardView {
            heading: t!("quiz-case", n = self.at as i64 + 1, total = n),
            title: self.card().title.clone(),
            lines: if self.revealed {
                self.revealed_lines()
            } else {
                Vec::new()
            },
            hint: if self.revealed {
                t!("quiz-hint-revealed")
            } else {
                t!("quiz-hint-hidden")
            },
        }
    }
}

/// The cards of a teaching case folder: each key image's anonymised DICOM copy at the frame
/// marked when the folder has it (the real instance, without burned-in measurements), else
/// its PNG.
pub(super) fn cards_of_case(dir: &Path) -> Option<(Vec<Card>, String)> {
    let c = super::figure::read_case(dir)?;
    let cards = c
        .images
        .into_iter()
        .filter_map(|i| {
            let (path, frame) = match i.dicom.filter(|d| d.is_file()) {
                Some(d) => (d, i.frame),
                None => (i.png.is_file().then_some(i.png)?, 0),
            };
            Some(Card {
                path,
                frame,
                title: i.title,
                findings: i.findings,
            })
        })
        .collect();
    Some((cards, c.notes))
}

/// Draws the card at the bottom of `area` (logical px).
pub(super) fn draw(
    dl: &mut DrawList,
    text: &mut Text,
    pal: &Palette,
    v: &CardView,
    area: Rect,
    rtl: bool,
    ts: f32,
) {
    let align = if rtl { Align::End } else { Align::Start };
    let w = (area.w - 32.0).clamp(120.0, 640.0 * ts.max(1.0));
    let inner = w - 32.0;
    let head = text.layout(
        &v.heading,
        TextStyle::new(FONT_LABEL * ts).weight(650.0),
        Some(inner),
        align,
    );
    let title = text.layout(
        &v.title,
        TextStyle::new(FONT_LABEL * ts),
        Some(inner),
        align,
    );
    let lines: Vec<_> = v
        .lines
        .iter()
        .map(|l| text.layout(l, TextStyle::new(FONT_LABEL * ts), Some(inner), align))
        .collect();
    let hint = text.layout(&v.hint, TextStyle::new(FONT_SMALL * ts), Some(inner), align);
    let gap = 6.0 * ts;
    let mut h = 16.0 + head.height + gap + title.height + gap + hint.height + 16.0;
    for l in &lines {
        h += l.height + gap;
    }
    let h = h.min(area.h * 0.6);
    let r = Rect::new(area.x + (area.w - w) * 0.5, area.bottom() - 16.0 - h, w, h);
    dl.rect(r, RADIUS_LG, pal.surface.with_alpha(0.92));
    dl.push_clip(r);
    // Each block at the reading start: the left edge, or the right one in right to left.
    let x = |w: f32| {
        if rtl {
            r.right() - 16.0 - w
        } else {
            r.x + 16.0
        }
    };
    let mut y = r.y + 16.0;
    text.draw(dl, &head, x(head.width), y, pal.text);
    y += head.height + gap;
    text.draw(dl, &title, x(title.width), y, pal.text_muted);
    y += title.height + gap;
    for l in &lines {
        text.draw(dl, l, x(l.width), y, pal.text);
        y += l.height + gap;
    }
    text.draw(dl, &hint, x(hint.width), y, pal.text_faint);
    dl.pop_clip();
}

impl App {
    /// "Teaching quiz": the key images marked, else a teaching case folder picked.
    pub(super) fn quiz_start(&mut self) {
        let keys = &self.figure.keys;
        if !keys.is_empty() {
            let cards = keys
                .iter()
                .enumerate()
                .map(|(i, k)| Card {
                    path: k.path.clone(),
                    frame: k.frame,
                    title: if k.anon_title.is_empty() {
                        t!("figure-key-n", n = i as i64 + 1)
                    } else {
                        k.anon_title.clone()
                    },
                    findings: k.notes(),
                })
                .collect();
            let notes = self.figure.keys_choices.notes.clone();
            self.quiz_begin(cards, notes);
            return;
        }
        if self.quiz.picking.is_some() {
            return;
        }
        let (tx, rx) = crossbeam_channel::bounded(1);
        self.loader.background("dlg-quiz", move |l| {
            let picked = rfd::FileDialog::new()
                .set_title(t!("quiz-pick-title"))
                .pick_folder();
            let _ = tx.send(picked);
            l.waker()();
        });
        self.quiz.picking = Some(rx);
    }

    fn quiz_begin(&mut self, cards: Vec<Card>, notes: String) {
        let now = self.now();
        let Some(q) = Quiz::new(cards, notes) else {
            self.chrome.toast(t!("quiz-empty"), None, now);
            return;
        };
        self.quiz.run = Some(q);
        self.quiz_show();
    }

    /// The folder picked: its cases, or why there are none.
    pub(super) fn quiz_tick(&mut self) {
        let Some(rx) = &self.quiz.picking else { return };
        let Ok(picked) = rx.try_recv() else { return };
        self.quiz.picking = None;
        let Some(dir) = picked else { return };
        match cards_of_case(&dir) {
            Some((cards, notes)) => self.quiz_begin(cards, notes),
            None => {
                let now = self.now();
                self.chrome.toast(t!("quiz-not-a-case"), None, now);
            }
        }
    }

    /// Shows the current case's image at the frame marked and says where the quiz is. A
    /// key in another series of the folder browsed switches series (series mode stays);
    /// one from elsewhere opens as any file does.
    fn quiz_show(&mut self) {
        let Some(q) = &self.quiz.run else { return };
        let (path, frame) = (q.card().path.clone(), q.card().frame);
        let v = q.view();
        match self.entries.iter().position(|p| *p == path) {
            Some(i) => self.go_to(i),
            None if self.series_show_file(&path) => {}
            None => self.open_target(path.clone()),
        }
        self.report_show_frame(path, frame);
        self.chrome
            .announce_status(format!("{} {} {}", v.heading, v.title, v.hint));
        self.redraw = true;
    }

    /// The quiz's keys; false when no quiz runs or the key is not one of them.
    pub(super) fn quiz_key(&mut self, combo: &KeyCombo) -> bool {
        if self.quiz.run.is_none() || combo.mods != Mods::NONE {
            return false;
        }
        let rtl = self.rtl();
        let step = |me: &mut App, d: i32| {
            if me.quiz.run.as_mut().is_some_and(|q| q.step(d)) {
                me.quiz_show();
            }
        };
        match combo.key {
            Key::Named(NamedKey::ArrowRight) => step(self, if rtl { -1 } else { 1 }),
            Key::Named(NamedKey::ArrowLeft) => step(self, if rtl { 1 } else { -1 }),
            Key::Named(NamedKey::PageDown) => step(self, 1),
            Key::Named(NamedKey::PageUp) => step(self, -1),
            Key::Named(NamedKey::Space) => self.quiz_reveal(),
            Key::Char(c) if c.eq_ignore_ascii_case(&'r') => self.quiz_reveal(),
            Key::Named(NamedKey::Escape) => {
                self.quiz.run = None;
                self.chrome.announce_status(t!("quiz-ended"));
                self.redraw = true;
            }
            _ => return false,
        }
        true
    }

    fn quiz_reveal(&mut self) {
        let Some(q) = self.quiz.run.as_mut() else {
            return;
        };
        q.revealed = !q.revealed;
        let msg = if q.revealed {
            q.revealed_lines().join(". ")
        } else {
            t!("quiz-hidden")
        };
        self.chrome.announce_status(msg);
        self.redraw = true;
    }

    /// The annotations stay hidden: a quiz shows a case not revealed yet.
    pub(super) fn quiz_hides(&self) -> bool {
        self.quiz.run.as_ref().is_some_and(|q| !q.revealed)
    }

    pub(super) fn quiz_view(&self) -> Option<CardView> {
        self.quiz.run.as_ref().map(Quiz::view)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(n: u32, findings: &[&str]) -> Card {
        Card {
            path: PathBuf::from(format!("/x/{n}.dcm")),
            frame: 0,
            title: format!("CT · Key image {n}"),
            findings: findings.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn hidden_until_revealed_and_hidden_again_on_step() {
        let mut q = Quiz::new(
            vec![card(1, &["Length 12.0 mm"]), card(2, &[])],
            "Notes".into(),
        )
        .unwrap();
        let v = q.view();
        assert!(v.lines.is_empty(), "findings shown before the reveal");
        assert_eq!(v.heading, t!("quiz-case", n = 1, total = 2));
        q.revealed = true;
        let v = q.view();
        assert_eq!(v.lines[0], "Length 12.0 mm");
        assert!(v.lines[1].contains("Notes"));
        assert!(q.step(1));
        assert!(!q.revealed);
        assert!(!q.step(1), "past the last case");
        q.revealed = true;
        assert_eq!(q.view().lines[0], t!("quiz-no-findings"));
        assert!(q.step(-1));
        assert!(Quiz::new(Vec::new(), String::new()).is_none());
    }

    /// The card hidden and revealed, left to right and right to left, at 1× and 1.5×.
    #[test]
    fn quiz_card_matches_goldens() {
        use specular_shots::{Goldens, Harness};
        let Some(mut h) = Harness::new() else { return };
        const SIZE: (f32, f32) = (760.0, 520.0);
        let backdrop = |dl: &mut DrawList| {
            dl.rect(
                Rect::new(0.0, 0.0, SIZE.0, SIZE.1),
                0.0,
                specular_theme::Color::BLACK,
            );
            dl.rect(
                Rect::new(200.0, 60.0, 360.0, 360.0),
                180.0,
                specular_theme::Color::rgba(0.45, 0.45, 0.45, 1.0),
            );
        };
        let mut goldens = Goldens::new(
            concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
            "specular-quiz",
        );
        let mut q = Quiz::new(
            vec![
                card(1, &["1. Line: 20.0 mm", "2. Ellipse: area 3.1 cm²"]),
                card(2, &[]),
            ],
            "Classic case. Look at the left side.".into(),
        )
        .unwrap();
        let hidden = q.view();
        q.revealed = true;
        let revealed = q.view();
        let he = specular_i18n::langid!("he");
        let revealed_he = specular_i18n::I18n::with_thread_language(he.clone(), || q.view());
        let pal = Palette::dark();
        for (scale, suffix) in [(1.0, "1x"), (1.5, "1.5x")] {
            for (name, v, rtl) in [
                ("hidden", &hidden, false),
                ("revealed", &revealed, false),
                ("revealed-he", &revealed_he, true),
            ] {
                let lang = if rtl {
                    he.clone()
                } else {
                    specular_i18n::langid!("en-US")
                };
                let shot = specular_i18n::I18n::with_thread_language(lang, || {
                    h.render(SIZE, scale, backdrop, |dl, text| {
                        draw(
                            dl,
                            text,
                            &pal,
                            v,
                            Rect::new(0.0, 0.0, SIZE.0, SIZE.1),
                            rtl,
                            1.0,
                        );
                    })
                });
                goldens.check(&format!("quiz-card-{name}-{suffix}"), &shot);
            }
        }
        goldens.finish();
    }

    #[test]
    fn a_case_folder_reads_back() {
        let dir = PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../target/test-tmp/quiz-case"
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("images")).unwrap();
        std::fs::create_dir_all(dir.join("DICOM")).unwrap();
        std::fs::write(dir.join("images/key-01.png"), b"png").unwrap();
        std::fs::write(dir.join("images/key-02.png"), b"png").unwrap();
        std::fs::write(dir.join("DICOM/key-02.dcm"), b"dcm").unwrap();
        let json = serde_json::json!({
            "format": "specular-teaching-case", "version": 1, "notes": "Case notes",
            "images": [
                {"image": "images/key-01.png", "dicom": null, "title": "CT", "findings": ["Area 3 cm²"]},
                {"image": "images/key-02.png", "dicom": "DICOM/key-02.dcm", "frame": 7,
                    "title": "MR", "findings": []},
                {"image": "images/key-01.png", "dicom": "DICOM/gone.dcm", "frame": 3,
                    "title": "PNG then", "findings": []},
                {"image": "images/key-01.png", "dicom": "../outside.dcm", "title": "z", "findings": []},
                {"image": "../outside.png", "title": "x", "findings": []},
                {"image": "images/missing.png", "title": "y", "findings": []}
            ]
        });
        std::fs::write(dir.join("case.json"), serde_json::to_vec(&json).unwrap()).unwrap();
        let (cards, notes) = cards_of_case(&dir).unwrap();
        assert_eq!(notes, "Case notes");
        let got: Vec<(PathBuf, u32)> = cards
            .iter()
            .map(|c| (c.path.strip_prefix(&dir).unwrap().to_path_buf(), c.frame))
            .collect();
        assert_eq!(
            got,
            [
                (PathBuf::from("images/key-01.png"), 0),
                (PathBuf::from("DICOM/key-02.dcm"), 7),
                (PathBuf::from("images/key-01.png"), 0),
                (PathBuf::from("images/key-01.png"), 0),
            ]
        );
        assert_eq!(cards[0].findings, ["Area 3 cm²"]);
        std::fs::write(dir.join("case.json"), b"{\"format\":\"other\"}").unwrap();
        assert!(cards_of_case(&dir).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
