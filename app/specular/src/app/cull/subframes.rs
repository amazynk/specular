//! Cull mode's subframe features on the app: reading the frames' facts, the strip's line,
//! the panel's contents and controls, sorting, the same stretch while blinking, auto-reject
//! and moving the rejected frames to a subfolder (both asked first).

use std::path::{Path, PathBuf};

use specular_i18n::t;
use specular_input::Action;

use super::astro::{self, Column, Panel, PanelHit, RowView};
use super::slots::{self, Collision};
use super::{Op, SlotUndo};
use crate::app::App;
use crate::app::gallery::Mark;

impl App {
    /// The subframes' facts in, new ones asked for, and the stretch kept while blinking.
    pub(super) fn cull_astro_tick(&mut self) {
        let cache = self.thumb_cache();
        let Some(run) = self.cull.run.as_mut() else {
            return;
        };
        if run.astro.is_none() && run.listing.iter().any(|p| astro::is_subframe(p)) {
            run.astro = Some(astro::State::default());
        }
        let Some(a) = run.astro.as_mut() else {
            return;
        };
        let todo: Vec<PathBuf> = run
            .listing
            .iter()
            .filter(|p| astro::is_subframe(p) && !a.asked.contains(*p))
            .cloned()
            .collect();
        if !todo.is_empty() {
            a.asked.extend(todo.iter().cloned());
            a.pending += todo.len();
            a.rx.push(astro::spawn_read(&self.loader, todo, cache));
        }
        let mut got = false;
        a.rx.retain(|rx| {
            loop {
                match rx.try_recv() {
                    Ok(v) => {
                        a.pending = a.pending.saturating_sub(v.len());
                        a.frames.extend(v);
                        got = true;
                    }
                    Err(crossbeam_channel::TryRecvError::Empty) => break true,
                    Err(crossbeam_channel::TryRecvError::Disconnected) => break false,
                }
            }
        });
        if a.rx.is_empty() {
            a.pending = 0;
        }
        let same = a.same_stretch;
        if got {
            self.redraw = true;
        }
        // Blinking: the first subframe's look holds for the rest.
        let on = same
            && self
                .cull_current_path()
                .is_some_and(|p| astro::is_subframe(&p));
        let was = (self.science.blink_on, self.science.blink.is_some());
        self.science.blink_on = on;
        if !on {
            self.science.blink = None;
        } else if self.science.blink.is_none() {
            self.science.blink = self.science.planes_look();
        }
        if was != (self.science.blink_on, self.science.blink.is_some()) {
            self.science_restyle();
        }
    }

    /// The strip's second line for a subframe: exposure, filter and its metrics.
    pub(super) fn cull_astro_line(&self, cur: &Path) -> Option<String> {
        let a = self.cull.run.as_ref()?.astro.as_ref()?;
        if !astro::is_subframe(cur) {
            return None;
        }
        let f = a.frames.get(cur);
        let mut parts = Vec::new();
        if let Some(f) = f {
            if let Some(e) = f.exposure {
                parts.push(format!("{e} s"));
            }
            parts.extend(f.filter.clone());
        }
        match f.and_then(|f| f.metrics) {
            Some(m) => {
                if let Some(v) = m.hfd {
                    parts.push(t!(
                        "cull-astro-line-hfd",
                        value = specular_i18n::fixed(v, 2)
                    ));
                }
                if let Some(v) = m.fwhm {
                    parts.push(match m.fwhm_arcsec() {
                        Some(s) => t!(
                            "cull-astro-line-fwhm-arcsec",
                            value = specular_i18n::fixed(v, 2),
                            arcsec = specular_i18n::fixed(s, 2)
                        ),
                        None => t!("cull-astro-line-fwhm", value = specular_i18n::fixed(v, 2)),
                    });
                }
                if let Some(v) = m.eccentricity {
                    parts.push(t!(
                        "cull-astro-line-ecc",
                        value = specular_i18n::fixed(v, 2)
                    ));
                }
                parts.push(t!("cull-astro-line-stars", count = i64::from(m.stars)));
            }
            None => parts.push(t!("cull-astro-measuring")),
        }
        Some(specular_i18n::dot_join(&parts))
    }

    /// The frames in the order shown, with their marks.
    fn cull_astro_rows(&self) -> Vec<(PathBuf, RowView)> {
        let Some(a) = self.cull.run.as_ref().and_then(|r| r.astro.as_ref()) else {
            return Vec::new();
        };
        self.entries
            .iter()
            .filter(|p| astro::is_subframe(p))
            .map(|p| {
                let name = p
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let flag = self.cull_mark_of(std::slice::from_ref(p)).flag;
                (
                    p.clone(),
                    RowView {
                        name,
                        frame: a.frames.get(p).cloned(),
                        flag,
                    },
                )
            })
            .collect()
    }

    /// The panel this frame, when it is shown.
    pub(in crate::app) fn cull_panel(&self) -> Option<Panel> {
        let a = self.cull.run.as_ref()?.astro.as_ref()?;
        if !a.panel {
            return None;
        }
        let rows = self.cull_astro_rows();
        if rows.is_empty() {
            return None;
        }
        let order: Vec<PathBuf> = rows.iter().map(|r| r.0.clone()).collect();
        let cur = self.cull_current_path();
        Some(Panel {
            current: cur.and_then(|c| order.iter().position(|p| *p == c)),
            suggestion: astro::suggestion(&order, &a.frames, a.metric, a.k),
            rows: rows.into_iter().map(|r| r.1).collect(),
            sort: a.sort,
            metric: a.metric,
            k: a.k,
            pending: a.pending,
            same_stretch: a.same_stretch,
            pal: self.palette(),
        })
    }

    /// A click on the panel; true when it was over it.
    pub(super) fn cull_panel_click(&mut self, p: (f32, f32), down: bool) -> bool {
        if !self.cull.panel.is_some_and(|r| r.contains(p)) {
            return false;
        }
        let hit = self
            .cull
            .panel_hits
            .iter()
            .find(|(r, _)| r.contains(p))
            .map(|h| h.1);
        if down && let Some(h) = hit {
            self.cull_panel_hit(h);
        }
        true
    }

    pub(super) fn cull_panel_hit(&mut self, h: PanelHit) {
        match h {
            PanelHit::Column(c) => {
                let desc = self
                    .cull
                    .run
                    .as_ref()
                    .and_then(|r| r.astro.as_ref()?.sort)
                    .is_some_and(|(sc, d)| sc == c && !d);
                self.cull_sort(c, desc);
            }
            PanelHit::Row(i) => {
                let rows = self.cull_astro_rows();
                if let Some(idx) = rows.get(i).and_then(|r| self.index_of(&r.0)) {
                    self.go_to(idx);
                }
            }
            PanelHit::Metric => self.cull_astro_action(Action::CullMetricNext),
            PanelHit::SigmaDown => self.cull_astro_action(Action::CullSigmaDown),
            PanelHit::SigmaUp => self.cull_astro_action(Action::CullSigmaUp),
            PanelHit::Reject => self.cull_astro_action(Action::CullAutoReject),
            PanelHit::MoveRejects => self.cull_astro_action(Action::CullMoveRejects),
            PanelHit::SameStretch => self.cull_astro_action(Action::CullSameStretch),
        }
        self.redraw = true;
    }

    /// Sorts the run by `col`, keeping the frame on screen.
    fn cull_sort(&mut self, col: Column, desc: bool) {
        let Some(run) = self.cull.run.as_mut() else {
            return;
        };
        let Some(a) = run.astro.as_mut() else {
            return;
        };
        a.sort = Some((col, desc));
        astro::sort_listing(&mut run.listing, &a.frames, col, desc);
        let cur = self.cull_current_path();
        self.cull_relayout(cur);
        let msg = t!("cull-astro-sorted", column = col.label());
        self.cull_flash(msg);
    }

    /// The subframe keys; without subframes they say so.
    pub(super) fn cull_astro_action(&mut self, act: Action) {
        let has = self.cull.run.as_ref().is_some_and(|r| r.astro.is_some());
        if !has {
            self.cull_flash(t!("cull-astro-none"));
            return;
        }
        let Some(a) = self.cull.run.as_mut().and_then(|r| r.astro.as_mut()) else {
            return;
        };
        match act {
            Action::CullSubframes => {
                a.panel = !a.panel;
                self.redraw = true;
            }
            Action::CullSortNext => {
                let (c, d) = a.next_sort();
                self.cull_sort(c, d);
            }
            Action::CullSortReverse => {
                let (c, d) = a.sort.unwrap_or((Column::Name, false));
                self.cull_sort(c, !d);
                self.cull_flash(t!("cull-astro-reversed"));
            }
            Action::CullMetricNext => {
                a.next_metric();
                let msg = t!("cull-astro-metric", metric = astro::metric_label(a.metric));
                self.cull_flash(msg);
            }
            Action::CullSigmaDown | Action::CullSigmaUp => {
                a.k_step(act == Action::CullSigmaUp);
                let msg = t!("cull-astro-k-set", k = specular_i18n::fixed(a.k, 2));
                self.cull_flash(msg);
            }
            Action::CullSameStretch => {
                a.same_stretch = !a.same_stretch;
                let on = a.same_stretch;
                self.science.blink = None;
                self.cull_flash(if on {
                    t!("cull-astro-same-on")
                } else {
                    t!("cull-astro-same-off")
                });
                self.cull_astro_tick();
                self.science_restyle();
            }
            Action::CullAutoReject => self.cull_auto_reject_ask(),
            Action::CullMoveRejects => self.cull_move_rejects_ask(),
            _ => {}
        }
        self.redraw = true;
    }

    /// Asks before rejecting the suggested frames (those not rejected yet).
    fn cull_auto_reject_ask(&mut self) {
        let rows = self.cull_astro_rows();
        let order: Vec<PathBuf> = rows.iter().map(|r| r.0.clone()).collect();
        let rule = self
            .cull_panel()
            .map(|p| p.suggestion_text())
            .unwrap_or_default();
        let Some(a) = self.cull.run.as_mut().and_then(|r| r.astro.as_mut()) else {
            return;
        };
        let s = astro::suggestion(&order, &a.frames, a.metric, a.k);
        let todo: Vec<PathBuf> = s
            .as_ref()
            .map(|s| {
                s.frames
                    .iter()
                    .filter(|&&i| rows[i].1.flag != -1)
                    .map(|&i| order[i].clone())
                    .collect()
            })
            .unwrap_or_default();
        if todo.is_empty() {
            self.cull_flash(t!("cull-astro-nothing-to-reject"));
            return;
        }
        let n = todo.len() as i64;
        a.ask_reject = todo;
        self.chrome.confirm_with_detail(
            t!("cull-astro-reject-confirm", count = n),
            t!("cull-astro-reject-detail", rule = rule),
            t!("cull-astro-reject-ok"),
            crate::app::items::CULL_AUTO_REJECT,
        );
        self.redraw = true;
    }

    /// The suggested frames are rejected (one undo).
    pub(in crate::app) fn cull_auto_reject_confirmed(&mut self) {
        let Some(a) = self.cull.run.as_mut().and_then(|r| r.astro.as_mut()) else {
            return;
        };
        let todo = std::mem::take(&mut a.ask_reject);
        if todo.is_empty() {
            return;
        }
        let u = self.gallery_set_marks(&todo, |m| Mark { flag: -1, ..m });
        self.cull_say(t!("cull-astro-rejected", count = todo.len() as i64), u);
    }

    /// Asks before moving the rejected frames to the `rejected` subfolder.
    fn cull_move_rejects_ask(&mut self) {
        let Some(run) = self.cull.run.as_ref() else {
            return;
        };
        let items: Vec<Vec<PathBuf>> = run
            .items
            .iter()
            .filter(|i| self.cull_mark_of(&i.members).flag == -1)
            .filter(|i| !i.members.iter().any(|m| self.cull.busy.contains(m)))
            .map(|i| i.members.clone())
            .collect();
        if items.is_empty() || run.dir.is_none() {
            self.cull_flash(t!("cull-move-rejects-none"));
            return;
        }
        let n = items.len() as i64;
        if let Some(a) = self.cull.run.as_mut().and_then(|r| r.astro.as_mut()) {
            a.ask_move = items;
        }
        self.chrome.confirm_with_detail(
            t!(
                "cull-move-rejects-confirm",
                count = n,
                folder = astro::REJECTS_FOLDER
            ),
            t!("cull-move-rejects-detail"),
            t!("cull-move-rejects-ok"),
            crate::app::items::CULL_MOVE_REJECTS,
        );
        self.redraw = true;
    }

    /// Moves the rejected frames (and their sidecars) to the subfolder, made if missing, in
    /// the background.
    pub(in crate::app) fn cull_move_rejects_confirmed(&mut self) {
        let Some(run) = self.cull.run.as_mut() else {
            return;
        };
        let Some(dir) = run.dir.clone() else { return };
        let Some(a) = run.astro.as_mut() else { return };
        let items = std::mem::take(&mut a.ask_move);
        if items.is_empty() {
            return;
        }
        let tx = self
            .cull
            .ops
            .get_or_insert_with(crossbeam_channel::unbounded)
            .0
            .clone();
        for m in items.iter().flatten() {
            self.cull.busy.insert(m.clone());
        }
        self.cull.ops_running += 1;
        let wake = self.loader.waker();
        let folder = dir.join(astro::REJECTS_FOLDER);
        self.loader.background("cull-rejects", move |_| {
            let outcomes = move_rejects(&folder, items);
            let _ = tx.send(Op::Rejects { folder, outcomes });
            wake();
        });
    }

    /// The rejected frames moved: they leave the run, with one undo.
    pub(super) fn cull_rejects_moved(
        &mut self,
        folder: &Path,
        outcomes: Vec<(Vec<PathBuf>, slots::Outcome)>,
    ) {
        let mut placed = Vec::new();
        let mut failed = Vec::new();
        let mut at = usize::MAX;
        for (members, o) in outcomes {
            for m in &members {
                self.cull.busy.remove(m);
            }
            if let Some(run) = self.cull.run.as_ref()
                && let Some(i) = run.listing.iter().position(|p| members.contains(p))
            {
                at = at.min(i);
            }
            placed.extend(o.done);
            failed.extend(o.failed);
        }
        self.cull_failures(&failed, |file, reason| {
            t!(
                "cull-slot-failed",
                op = "move",
                file = file,
                reason = reason
            )
        });
        if placed.is_empty() {
            return;
        }
        let pairs: Vec<(PathBuf, PathBuf)> = placed
            .iter()
            .map(|p| (p.from.clone(), p.to.clone()))
            .collect();
        self.gallery_marks_follow(&pairs, false);
        let moved = pairs
            .iter()
            .filter(|(f, _)| self.loader.supports_file(f))
            .count();
        if let Some(run) = self.cull.run.as_mut() {
            let gone: std::collections::HashSet<&PathBuf> = pairs.iter().map(|(f, _)| f).collect();
            run.listing.retain(|p| !gone.contains(p));
        }
        self.cull_relayout(None);
        let u = self.next_undo;
        self.next_undo += 1;
        self.last_undo = Some(u);
        self.cull.undo.insert(
            u,
            SlotUndo {
                copy: false,
                placed,
                at: at.min(self.cull.run.as_ref().map_or(0, |r| r.listing.len())),
            },
        );
        let name = folder
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        self.cull_say(
            t!("cull-moved-rejects", count = moved as i64, folder = name),
            Some(u),
        );
    }
}

/// Moves each item (its members and their sidecars) into `folder`, made if missing; a
/// taken name gets a suffix. Never deletes anything.
pub(super) fn move_rejects(
    folder: &Path,
    items: Vec<Vec<PathBuf>>,
) -> Vec<(Vec<PathBuf>, slots::Outcome)> {
    // A link planted under the folder's name is not followed.
    let linked = std::fs::symlink_metadata(folder).is_ok_and(|m| m.file_type().is_symlink());
    let made = if linked {
        Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "a link, not a folder",
        ))
    } else {
        std::fs::create_dir_all(folder)
    };
    if let Err(e) = made {
        let reason = crate::error_text::io_reason(&e);
        return items
            .into_iter()
            .map(|m| {
                let failed = m.iter().map(|p| (p.clone(), reason.clone())).collect();
                (
                    m,
                    slots::Outcome {
                        failed,
                        ..Default::default()
                    },
                )
            })
            .collect();
    }
    items
        .into_iter()
        .map(|members| {
            let stem = members
                .first()
                .and_then(|p| p.file_stem())
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            let files = slots::with_sidecars(&members);
            let o = slots::place(&files, &stem, folder, false, Collision::Suffix);
            (members, o)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let base = std::env::var_os("SPECULAR_TEST_TMP").map_or_else(
            || Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/test-tmp"),
            PathBuf::from,
        );
        let d = base.join(format!("{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn rejects_move_into_the_subfolder_with_sidecars_and_nothing_is_deleted() {
        let d = scratch("astro-rejects");
        for f in ["L_001.fits", "L_001.xmp", "L_002.fits", "L_003.xisf"] {
            std::fs::write(d.join(f), f).unwrap();
        }
        let folder = d.join(astro::REJECTS_FOLDER);
        // A frame of the same name rejected earlier keeps its place.
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("L_003.xisf"), "older").unwrap();
        let out = move_rejects(
            &folder,
            vec![vec![d.join("L_001.fits")], vec![d.join("L_003.xisf")]],
        );
        assert_eq!(out.len(), 2);
        assert!(out.iter().all(|(_, o)| o.failed.is_empty()));
        assert!(folder.join("L_001.fits").is_file());
        assert!(folder.join("L_001.xmp").is_file(), "the sidecar follows");
        assert!(!d.join("L_001.fits").exists() && !d.join("L_001.xmp").exists());
        // Kept frames stay where they are.
        assert!(d.join("L_002.fits").is_file());
        // The taken name got a suffix; the older file is untouched.
        assert_eq!(
            std::fs::read_to_string(folder.join("L_003.xisf")).unwrap(),
            "older"
        );
        let names: Vec<String> = std::fs::read_dir(&folder)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names.len(), 4, "{names:?}");
        assert!(!d.join("L_003.xisf").exists());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[cfg(unix)]
    #[test]
    fn a_link_in_place_of_the_folder_is_not_followed() {
        let d = scratch("astro-rejects-link");
        let elsewhere = d.join("elsewhere");
        std::fs::create_dir_all(&elsewhere).unwrap();
        std::os::unix::fs::symlink(&elsewhere, d.join(astro::REJECTS_FOLDER)).unwrap();
        std::fs::write(d.join("a.fits"), "a").unwrap();
        let out = move_rejects(&d.join(astro::REJECTS_FOLDER), vec![vec![d.join("a.fits")]]);
        assert_eq!(out[0].1.failed.len(), 1);
        assert!(d.join("a.fits").is_file());
        assert_eq!(std::fs::read_dir(&elsewhere).unwrap().count(), 0);
        let _ = std::fs::remove_dir_all(&d);
    }
}
