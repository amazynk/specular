//! Focus stack in the app (shared flow: [`super::merges`]).
//!
//! - **Sheet** ([`super::sheet`]): the raws (each can be left out), then align photos, save
//!   depth map (off by default), open in Develop; the method is the choice row (depth map
//!   by default, or pyramid). Choices last for the run.
//! - **Job:** the frames are read as they are needed (two passes, so even 200 frames never
//!   sit in memory at once), stacked ([`specular_develop::merge::focus`]) and saved as
//!   `<reference>-Stack.dng` beside them, with `<reference>-Stack-depth.png` when asked.

use std::path::{Path, PathBuf};

use specular_develop::merge::{self, Shot, dng, focus};
use specular_i18n::t;

use super::merges::{Done, Failure, Kind};
use super::sheet::{DevelopSheet, Purpose};
use crate::app::App;

/// Option rows after the photos: align, depth map, open.
const OPTIONS: usize = 3;
const METHODS: [focus::Method; 2] = [focus::Method::DepthMap, focus::Method::Pyramid];

/// The run's choices.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::app) struct Prefs {
    /// Index into [`METHODS`].
    pub method: usize,
    pub align: bool,
    pub depth_map: bool,
    pub open: bool,
}

impl Default for Prefs {
    fn default() -> Self {
        let o = focus::Options::default();
        Prefs {
            method: METHODS.iter().position(|m| *m == o.method).unwrap_or(0),
            align: o.align,
            depth_map: o.depth_map,
            open: true,
        }
    }
}

fn middle(shots: &[Shot]) -> Option<&Shot> {
    shots.get(shots.len() / 2)
}

fn result_name(reference: &Path) -> String {
    dng::file_name_of(reference, dng::Kind::Stack, "dng")
}

/// The sheet for `shots` with `prefs`.
pub(super) fn sheet(shots: &[Shot], prefs: &Prefs) -> DevelopSheet {
    let mut rows: Vec<(String, bool)> = shots
        .iter()
        .map(|s| (super::merges::name_of(&s.path), true))
        .collect();
    let mut ends = Vec::with_capacity(rows.len() + OPTIONS);
    ends.resize(
        rows.len(),
        (t!("develop-sheet-included"), t!("develop-sheet-left-out")),
    );
    for (label, on) in [
        (t!("develop-merge-align"), prefs.align),
        (t!("develop-stack-depth-map"), prefs.depth_map),
        (t!("develop-merge-open"), prefs.open),
    ] {
        rows.push((label, on));
        ends.push((t!("develop-merge-on"), t!("develop-merge-off")));
    }
    let paths: Vec<PathBuf> = shots.iter().map(|s| s.path.clone()).collect();
    let name = middle(shots)
        .map(|s| result_name(&s.path))
        .unwrap_or_default();
    DevelopSheet {
        purpose: Purpose::FocusStack(paths),
        title: t!("develop-stack-title"),
        field: None,
        kinds: Vec::new(),
        mode: Some(prefs.method.min(METHODS.len() - 1)),
        ok: t!("develop-stack-ok"),
        top: 0,
        note: Some(t!("develop-stack-note", name = name)),
        rows,
        ends,
        choice_text: Some((
            t!("develop-stack-method"),
            vec![
                t!("develop-stack-method-depth"),
                t!("develop-stack-method-pyramid"),
            ],
        )),
    }
}

/// The sheet for `shots` with the default choices (screenshot tests).
#[cfg(test)]
pub(super) fn sheet_for_shots(shots: &[Shot]) -> DevelopSheet {
    sheet(shots, &Prefs::default())
}

fn choices(s: &DevelopSheet, paths: &[PathBuf]) -> (Vec<PathBuf>, Prefs) {
    let n = paths.len();
    let kept = paths
        .iter()
        .zip(&s.rows)
        .filter(|(_, r)| r.1)
        .map(|(p, _)| p.clone())
        .collect();
    let opt = |i: usize| s.rows.get(n + i).is_some_and(|r| r.1);
    (
        kept,
        Prefs {
            method: s.mode.unwrap_or(0).min(METHODS.len() - 1),
            align: opt(0),
            depth_map: opt(1),
            open: opt(2),
        },
    )
}

impl App {
    /// "Focus stack…" on `targets` (raws); `around` are the photos near a single target.
    pub(in crate::app) fn develop_stack_ask(
        &mut self,
        targets: Vec<PathBuf>,
        around: Vec<PathBuf>,
    ) {
        if !targets.iter().any(|p| super::controls::is_raw(p)) {
            self.develop_say(t!("develop-stack-not-raw"));
            return;
        }
        self.develop_merges_ask(Kind::Stack, targets, around, super::controls::is_raw);
    }

    /// The sheet's OK: the job starts.
    pub(super) fn develop_stack_ok(&mut self, s: &DevelopSheet, paths: Vec<PathBuf>) {
        let (kept, prefs) = choices(s, &paths);
        self.develop.merges.stack = prefs;
        if kept.len() < 2 {
            self.develop_say(t!("develop-merge-too-few"));
            return;
        }
        if kept.len() > focus::MAX_FRAMES {
            let why = super::merge::merge_reason(&merge::Error::Cap(focus::MAX_FRAMES));
            self.develop_say(t!("develop-stack-failed", reason = why));
            return;
        }
        let shots: Vec<Shot> = self
            .develop
            .merges
            .shots
            .iter()
            .filter(|s| kept.contains(&s.path))
            .cloned()
            .collect();
        let name = middle(&shots)
            .map(|s| result_name(&s.path))
            .unwrap_or_default();
        let o = focus::Options {
            method: METHODS[prefs.method],
            align: prefs.align,
            // Only the depth map method makes one.
            depth_map: prefs.depth_map && METHODS[prefs.method] == focus::Method::DepthMap,
        };
        self.develop_merges_start(Kind::Stack, name, prefs.open, move |set, cancel| {
            let source = focus::Paths {
                paths: shots.iter().map(|s| s.path.clone()).collect(),
                shots: shots.clone(),
            };
            let st =
                focus::stack(&source, &o, &|f| set(0.95 * f), cancel).map_err(Failure::Merge)?;
            let reference = st.merged.shots[st.merged.reference].path.clone();
            let ref_bytes = std::fs::read(&reference).unwrap_or_default();
            let path = dng::save_streamed(
                &st.merged,
                &ref_bytes,
                &dng::DngOptions::default(),
                dng::Kind::Stack,
                None,
            )
            .map_err(|e| Failure::Write(e.to_string()))?;
            if let Some(d) = &st.depth {
                let png = d.to_png().map_err(Failure::Write)?;
                dng::write_beside(&reference, &focus::depth_file_name(&reference), &png)
                    .map_err(|e| Failure::Write(e.to_string()))?;
            }
            Ok(Done {
                path,
                sources: shots.iter().map(|s| s.path.clone()).collect(),
                left_out: Vec::new(),
            })
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sheet_defaults_and_reads_back() {
        let shots: Vec<Shot> = (0..4)
            .map(|i| Shot {
                path: PathBuf::from(format!("/p/F{i}.ORF")),
                ..Shot::default()
            })
            .collect();
        let mut s = sheet(&shots, &Prefs::default());
        assert_eq!(s.rows.len(), 4 + OPTIONS);
        // Depth map method, the depth map not saved.
        assert_eq!(s.mode, Some(0));
        assert!(!s.rows[4 + 1].1);
        assert!(
            s.note
                .as_deref()
                .is_some_and(|n| n.contains("F2-Stack.dng"))
        );
        let paths: Vec<PathBuf> = shots.iter().map(|s| s.path.clone()).collect();
        s.toggle(4 + 1);
        s.mode = Some(1);
        let (kept, prefs) = choices(&s, &paths);
        assert_eq!(kept.len(), 4);
        assert!(prefs.depth_map && prefs.align);
        assert_eq!(METHODS[prefs.method], focus::Method::Pyramid);
    }
}
