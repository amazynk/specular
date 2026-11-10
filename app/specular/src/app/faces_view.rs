//! What Local AI found on the picture shown: the faces People found (boxes named by person)
//! and Culling's scores (closed eyes per face, blur, exposure).
//!
//! "Show faces" ([`Action::FaceBoxes`]) draws the boxes on the image; cull mode shows the
//! scores as flags on its strip ([`App::cull_flags`]). The catalogue is read on a thread of
//! its own when the picture changes, never on the UI thread.
//!
//! [`Action::FaceBoxes`]: specular_input::Action::FaceBoxes

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};

use specular_i18n::t;
use specular_library::ai::Scores;

use super::App;
use super::script::Placed;

/// A face box: fractions of the (oriented) picture, and its label.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Shown {
    pub(super) rect: [f32; 4],
    pub(super) label: String,
    /// Culling thinks the eyes are closed.
    pub(super) closed: bool,
    /// The face's roll (radians, clockwise): the box is drawn turned with the face.
    pub(super) roll: f32,
}

/// What the catalogue holds for one file.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Found {
    pub(super) faces: Vec<Shown>,
    pub(super) scores: Option<Scores>,
}

pub(super) struct FaceView {
    /// "Show faces" is on.
    pub(super) on: bool,
    tx: Sender<(PathBuf, Found)>,
    rx: Receiver<(PathBuf, Found)>,
    asked: Option<PathBuf>,
    pub(super) found: Option<(PathBuf, Found)>,
}

impl FaceView {
    pub(super) fn new() -> FaceView {
        let (tx, rx) = channel();
        FaceView {
            on: false,
            tx,
            rx,
            asked: None,
            found: None,
        }
    }

    /// What is known for `path` (None: not looked up yet, or nothing).
    pub(super) fn of(&self, path: &Path) -> Option<&Found> {
        self.found
            .as_ref()
            .filter(|(p, _)| p == path)
            .map(|(_, f)| f)
    }
}

/// Closed eyes from a Culling score.
pub(super) const CLOSED: f32 = 0.5;
/// Below this sharpness a photo is flagged as blurred.
pub(super) const BLURRED: f32 = 0.35;

/// Reads the catalogue's faces and scores of `path`.
fn look_up(db: &Path, path: &Path) -> Found {
    use specular_library::ai;
    let Ok(conn) = ai::open(db) else {
        return Found::default();
    };
    let Ok(Some(id)) = specular_library::edits::file_id(&conn, path) else {
        return Found::default();
    };
    let mut out = Found::default();
    if ai::has(&conn, "people").unwrap_or(false) {
        let names: std::collections::HashMap<i64, Option<String>> = ai::people(&conn)
            .unwrap_or_default()
            .into_iter()
            .map(|p| (p.id, p.name))
            .collect();
        for f in ai::file_faces(&conn, id).unwrap_or_default() {
            let label = match f.person {
                Some(p) => match names.get(&p) {
                    Some(Some(n)) => n.clone(),
                    _ => format!("#{p}"),
                },
                None => String::new(),
            };
            out.faces.push(Shown {
                rect: f.rect,
                label,
                closed: false,
                roll: f.roll,
            });
        }
    }
    if ai::has(&conn, "culling").unwrap_or(false) {
        out.scores = ai::scores(&conn, id).ok().flatten();
        if let Some(s) = &out.scores {
            for e in &s.faces {
                let r = [e.x, e.y, e.w, e.h];
                // The same face from People: mark it; else add it.
                let same = out
                    .faces
                    .iter_mut()
                    .find(|f| specular_infer::face::blazeface::iou(&f.rect, &r) > 0.3);
                match same {
                    Some(f) => f.closed |= e.closed > CLOSED,
                    None => out.faces.push(Shown {
                        rect: r,
                        label: String::new(),
                        closed: e.closed > CLOSED,
                        roll: 0.0,
                    }),
                }
            }
        }
    }
    out
}

impl App {
    fn faces_current_path(&self) -> Option<PathBuf> {
        self.current
            .and_then(|i| self.entries.get(i))
            .map(PathBuf::from)
    }

    /// Asks for the current picture's faces and scores when they are wanted and not known.
    pub(super) fn drain_faces_view(&mut self) {
        while let Ok((p, f)) = self.face_view.rx.try_recv() {
            self.face_view.found = Some((p, f));
            self.redraw = true;
        }
        let wanted = self.face_view.on || self.cull.active();
        if !wanted || self.args.screenshot.is_some() {
            return;
        }
        let Some(path) = self.faces_current_path() else {
            return;
        };
        if self.face_view.asked.as_ref() == Some(&path) {
            return;
        }
        let Some(db) = super::ai::pack().and_then(|p| p.catalogue().map(Path::to_path_buf)) else {
            return;
        };
        self.face_view.asked = Some(path.clone());
        let tx = self.face_view.tx.clone();
        let wake = self.loader.waker();
        let spawned = specular_thread::builder("ai-faces-view").spawn(move || {
            let f = look_up(&db, &path);
            let _ = tx.send((path, f));
            wake();
        });
        if let Err(e) = spawned {
            specular_log::warn!(target: specular_log::UI, "faces: {e}");
        }
    }

    /// Culling's flags for `path` in cull mode (none while unknown or without Culling).
    pub(super) fn cull_flags(&self, path: &Path) -> Vec<String> {
        self.face_view
            .of(path)
            .and_then(|f| f.scores.as_ref())
            .map(flags)
            .unwrap_or_default()
    }

    /// "Show faces" on or off.
    pub(super) fn toggle_face_boxes(&mut self) {
        self.face_view.on = !self.face_view.on;
        self.face_view.asked = None;
        let msg = if self.face_view.on {
            t!("faces-shown")
        } else {
            t!("faces-hidden")
        };
        self.integration_toast(msg);
        self.redraw = true;
    }

    /// The face boxes on screen (logical px) while "Show faces" is on.
    pub(super) fn faces_placed(&self) -> Vec<Placed> {
        if !self.face_view.on {
            return Vec::new();
        }
        let Some(path) = self.faces_current_path() else {
            return Vec::new();
        };
        let Some(found) = self.face_view.of(&path) else {
            return Vec::new();
        };
        placed(&found.faces, self.view.display_size(), self.dl.scale, |d| {
            self.view.display_to_screen(d)
        })
    }
}

/// A photo's culling flags, in words.
pub(super) fn flags(s: &Scores) -> Vec<String> {
    let mut v = Vec::new();
    if s.eyes_open.is_some_and(|e| e < 1.0 - CLOSED) {
        let closed = s.faces.iter().filter(|f| f.closed > CLOSED).count();
        v.push(if s.faces.len() > 1 {
            t!(
                "cull-flag-eyes-closed-some",
                closed = closed as i64,
                faces = s.faces.len() as i64
            )
        } else {
            t!("cull-flag-eyes-closed")
        });
    }
    if s.sharpness.is_some_and(|x| x < BLURRED) {
        v.push(t!("cull-flag-blurred"));
    }
    match s.exposure {
        Some(m) if m < DARK => v.push(t!("cull-flag-dark")),
        Some(m) if m > BRIGHT => v.push(t!("cull-flag-bright")),
        _ if s.clipped.is_some_and(|c| c > CLIPPED) => v.push(t!("cull-flag-clipped")),
        _ => {}
    }
    v
}

/// Mean luminance below which a photo is dark, above which bright.
const DARK: f32 = 0.15;
const BRIGHT: f32 = 0.85;
/// Fraction clipped or crushed above which it is flagged.
const CLIPPED: f32 = 0.2;

/// Face boxes as outlines: `size` is the picture's displayed size, `to_screen` maps it to
/// physical px, `ls` the logical scale.
pub(super) fn placed(
    faces: &[Shown],
    size: (f32, f32),
    ls: f32,
    to_screen: impl Fn((f32, f32)) -> (f32, f32),
) -> Vec<Placed> {
    let ls = ls.max(0.1);
    let (w, h) = size;
    faces
        .iter()
        .map(|f| {
            // The box in picture pixels, turned about its centre by the face's roll (to the
            // nearest quarter turn, so it still fits the face's axis-aligned box), its first
            // edge along the top of the face.
            let [x, y, fw, fh] = f.rect;
            let (cx, cy) = ((x + fw * 0.5) * w, (y + fh * 0.5) * h);
            let q = (f.roll / std::f32::consts::FRAC_PI_2).round();
            let (s, c) = (q * std::f32::consts::FRAC_PI_2).sin_cos();
            let (hw, hh) = if (q as i32).rem_euclid(2) == 1 {
                (fh * h * 0.5, fw * w * 0.5)
            } else {
                (fw * w * 0.5, fh * h * 0.5)
            };
            let corners = [(-hw, -hh), (hw, -hh), (hw, hh), (-hw, hh), (-hw, -hh)];
            let pts = corners
                .iter()
                .map(|&(u, v)| {
                    let q = to_screen((cx + u * c - v * s, cy + u * s + v * c));
                    (q.0 / ls, q.1 / ls)
                })
                .collect();
            let text = match (f.label.is_empty(), f.closed) {
                (_, true) if f.label.is_empty() => t!("faces-eyes-closed"),
                (false, true) => {
                    specular_i18n::dot_join::<&str>(&[&f.label, &t!("faces-eyes-closed")])
                }
                _ => f.label.clone(),
            };
            Placed {
                kind: crate::remote::protocol::ShapeKind::Polyline,
                pts,
                text,
                color: if f.closed {
                    [255, 120, 80, 255]
                } else {
                    [90, 200, 250, 255]
                },
                width: 2.0,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_name_what_is_wrong() {
        let ok = Scores {
            sharpness: Some(0.8),
            eyes_open: Some(0.9),
            exposure: Some(0.45),
            clipped: Some(0.01),
            ..Default::default()
        };
        assert!(flags(&ok).is_empty());
        let bad = Scores {
            sharpness: Some(0.1),
            eyes_open: Some(0.2),
            exposure: Some(0.05),
            ..Default::default()
        };
        assert_eq!(flags(&bad).len(), 3);
        let clipped = Scores {
            exposure: Some(0.6),
            clipped: Some(0.3),
            ..Default::default()
        };
        assert_eq!(flags(&clipped).len(), 1);
    }

    #[test]
    fn boxes_follow_the_picture_and_say_closed_eyes() {
        let faces = [
            Shown {
                rect: [0.25, 0.5, 0.25, 0.25],
                label: "Ana".into(),
                closed: false,
                roll: 0.0,
            },
            Shown {
                rect: [0.0, 0.0, 0.1, 0.1],
                label: String::new(),
                closed: true,
                roll: 0.0,
            },
        ];
        let p = placed(&faces, (400.0, 200.0), 2.0, |d| {
            (d.0 * 2.0 + 10.0, d.1 * 2.0)
        });
        assert_eq!(p.len(), 2);
        assert_eq!(p[0].pts[0], (105.0, 100.0));
        assert_eq!(p[0].pts[2], (205.0, 150.0));
        assert_eq!(p[0].text, "Ana");
        assert_eq!(p[0].pts.len(), 5);
        assert!(!p[1].text.is_empty());
    }
}
