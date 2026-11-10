//! Merge to HDR for JPEG, HEIC and TIFF brackets (shared flow: [`super::merges`]).
//!
//! - **Sheet** ([`super::sheet`]): the photos with their exposure (each can be left out),
//!   then align photos, also save radiance (EXR, off by default), open in Develop; the
//!   output format is the choice row (16-bit TIFF by default, or lossless JPEG XL).
//! - **Job:** the photos are aligned and fused ([`specular_develop::merge::fusion`],
//!   exposure fusion: the display look) and saved as `<reference>-HDR.tif` / `.jxl`, with
//!   `<reference>-HDR.exr` when asked, beside them.

use std::path::{Path, PathBuf};

use specular_develop::merge::{self, Motion, Shot, dng, fusion, rgb::RgbFormat};
use specular_i18n::t;

use super::merges::{Done, Failure, Kind};
use super::sheet::{DevelopSheet, Purpose};
use crate::app::App;

/// Option rows after the photos: align, radiance, open.
const OPTIONS: usize = 3;

/// The run's choices.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::app) struct Prefs {
    pub align: bool,
    pub radiance: bool,
    pub open: bool,
    /// Index into [`RgbFormat::ALL`].
    pub format: usize,
}

impl Default for Prefs {
    fn default() -> Self {
        Prefs {
            align: true,
            radiance: false,
            open: true,
            format: RgbFormat::ALL
                .iter()
                .position(|f| *f == RgbFormat::default())
                .unwrap_or(0),
        }
    }
}

fn format_name(f: RgbFormat) -> String {
    match f {
        RgbFormat::Tiff16 => t!("develop-pano-format-tiff"),
        RgbFormat::JxlLossless => t!("develop-pano-format-jxl"),
    }
}

fn result_name(reference: &Path, prefs: &Prefs) -> String {
    let f = RgbFormat::ALL[prefs.format.min(RgbFormat::ALL.len() - 1)];
    dng::file_name_of(reference, dng::Kind::Hdr, f.ext())
}

/// The sheet for `shots` with `prefs`.
pub(super) fn sheet(shots: &[Shot], prefs: &Prefs) -> DevelopSheet {
    let reference = merge::default_reference(shots);
    let mid = shots.get(reference).and_then(Shot::ev);
    let mut rows: Vec<(String, bool)> = shots
        .iter()
        .map(|s| {
            let mut parts = vec![super::merges::name_of(&s.path)];
            parts.extend(super::merge::ev_text(s, mid));
            (specular_i18n::dot_join(&parts), true)
        })
        .collect();
    let mut ends = Vec::with_capacity(rows.len() + OPTIONS);
    ends.resize(
        rows.len(),
        (t!("develop-sheet-included"), t!("develop-sheet-left-out")),
    );
    for (label, on) in [
        (t!("develop-merge-align"), prefs.align),
        (t!("develop-fusion-radiance"), prefs.radiance),
        (t!("develop-merge-open"), prefs.open),
    ] {
        rows.push((label, on));
        ends.push((t!("develop-merge-on"), t!("develop-merge-off")));
    }
    let paths: Vec<PathBuf> = shots.iter().map(|s| s.path.clone()).collect();
    let name = shots
        .get(reference)
        .map(|s| result_name(&s.path, prefs))
        .unwrap_or_default();
    DevelopSheet {
        purpose: Purpose::MergeFusion(paths),
        title: t!("develop-merge-title"),
        field: None,
        kinds: Vec::new(),
        mode: Some(prefs.format.min(RgbFormat::ALL.len() - 1)),
        ok: t!("develop-merge-ok"),
        top: 0,
        note: Some(t!("develop-fusion-note", name = name)),
        rows,
        ends,
        choice_text: Some((
            t!("develop-fusion-format"),
            RgbFormat::ALL.iter().map(|f| format_name(*f)).collect(),
        )),
    }
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
            align: opt(0),
            radiance: opt(1),
            open: opt(2),
            format: s.mode.unwrap_or(0).min(RgbFormat::ALL.len() - 1),
        },
    )
}

impl App {
    /// "Merge to HDR…" on JPEG, HEIC or TIFF photos.
    pub(in crate::app) fn develop_fusion_ask(
        &mut self,
        targets: Vec<PathBuf>,
        around: Vec<PathBuf>,
    ) {
        self.develop_merges_ask(Kind::Fusion, targets, around, |p| {
            !super::controls::is_raw(p)
        });
    }

    /// The sheet's OK: the job starts.
    pub(super) fn develop_fusion_ok(&mut self, s: &DevelopSheet, paths: Vec<PathBuf>) {
        let (kept, prefs) = choices(s, &paths);
        self.develop.merges.fusion = prefs;
        if kept.len() < 2 {
            self.develop_say(t!("develop-merge-too-few"));
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
        let reference = merge::default_reference(&shots);
        let name = shots
            .get(reference)
            .map(|s| result_name(&s.path, &prefs))
            .unwrap_or_default();
        let o = fusion::FusionOptions {
            motion: if prefs.align {
                Motion::Homography
            } else {
                Motion::None
            },
            radiance: prefs.radiance.then(fusion::Response::default),
            reference: Some(reference),
            ..fusion::FusionOptions::default()
        };
        let format = RgbFormat::ALL[prefs.format];
        self.develop_merges_start(Kind::Fusion, name, prefs.open, move |set, cancel| {
            let n = shots.len();
            let mut frames = Vec::with_capacity(n);
            for (i, s) in shots.iter().enumerate() {
                if cancel.is_cancelled() {
                    return Err(Failure::Merge(merge::Error::Cancelled));
                }
                let (img, _) = merge::rgb::read_codes(&s.path).map_err(Failure::Merge)?;
                frames.push((img, s.clone()));
                set(0.3 * (i + 1) as f32 / n as f32);
            }
            let f = fusion::fuse(frames, &o, &|x| set(0.3 + 0.65 * x), cancel)
                .map_err(Failure::Merge)?;
            let ref_bytes = std::fs::read(&f.shots[f.reference].path).ok();
            let written = fusion::save(&f, format, ref_bytes.as_deref())
                .map_err(|e| Failure::Write(e.to_string()))?;
            let path = written
                .into_iter()
                .next()
                .ok_or_else(|| Failure::Write(String::new()))?;
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
    fn sheet_defaults_to_tiff_without_radiance() {
        let shots: Vec<Shot> = [1.0 / 500.0, 1.0 / 125.0, 1.0 / 30.0]
            .iter()
            .enumerate()
            .map(|(i, t)| Shot {
                path: PathBuf::from(format!("/p/b{i}.jpg")),
                exposure_time: Some(*t),
                ..Shot::default()
            })
            .collect();
        let mut s = sheet(&shots, &Prefs::default());
        assert_eq!(s.rows.len(), 3 + OPTIONS);
        assert!(!s.rows[3 + 1].1);
        assert!(s.note.as_deref().is_some_and(|n| n.contains("b1-HDR.tif")));
        let paths: Vec<PathBuf> = shots.iter().map(|s| s.path.clone()).collect();
        s.toggle(3 + 1);
        s.mode = Some(1);
        let (_, prefs) = choices(&s, &paths);
        assert!(prefs.radiance);
        assert_eq!(RgbFormat::ALL[prefs.format], RgbFormat::JxlLossless);
    }
}
