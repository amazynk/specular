//! Develop → Export…: the edited photo, the folder or the picks written
//! as new files by `specular_develop::export`.
//!
//! The sheet is Convert…'s ([`super::super::convert`]) with Develop's rows: a preset, the
//! format and its quality or bit depth, the size, the colour space, metadata, the folder and
//! the name. Presets (shipped and the user's) and the last choices are kept as text in
//! `develop-export-presets.txt` in the settings folder. Save preset adds the choices as a
//! preset named after them; Delete preset, shown while the choices are one of the user's
//! presets, asks first (its label turns into the question) and removes it on a second press.
//!
//! The queue runs on a background task on the app's GPU (the software adapter without one):
//! the image open in Develop with its edit and turn, others with their saved edit (sidecar or
//! catalogue) or the default one. Its progress shows in the downloads indicator, with Stop.

use std::path::{Path, PathBuf};

use specular_convert::{
    Conflict, MetaMode, NameFacts, Outcome as COutcome, OutputDir, Pattern, Probe, Progress,
    Report, Resize,
};
use specular_core::CancelToken;
use specular_develop::export::batch::{self, Job, Outcome};
use specular_develop::export::{Depth, FileFormat, HdrOutput, Preset, Space};
use specular_develop::gpu::pool::Budget;
use specular_develop::model::Edit;
use specular_i18n::t;
use specular_widgets::line_edit::LineEdit;
use specular_widgets::{DownloadRow, DownloadState};

use super::super::App;
use super::super::convert::{
    self, Choices, Dialog, Drawn, Phase, Row, RowView, Run, Source, file_name, option_label,
    row_label, step_lossy, step_size, step5,
};
use super::Orient;
use crate::develop_export_cli::{load_presets, save_presets};

/// The name a shipped preset shows.
fn builtin_name(id: &str) -> String {
    match id {
        "jpeg" => t!("develop-export-preset-jpeg"),
        "web" => t!("develop-export-preset-web"),
        "tiff16" => t!("develop-export-preset-tiff16"),
        "exr" => t!("develop-export-preset-exr"),
        o => o.to_owned(),
    }
}

/// Sizes the size rows step through when the kind changes.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Sizes {
    width: u32,
    height: u32,
    long: u32,
    percent: u32,
}

/// The export choices of the open sheet.
#[derive(Clone, Debug)]
pub(in crate::app) struct Export {
    pub preset: Preset,
    pub user: Vec<Preset>,
    sub_name: String,
    pub chosen: Option<PathBuf>,
    sizes: Sizes,
    /// The image open in Develop: its edit and turn.
    pub current: Option<(PathBuf, Edit, Orient)>,
    /// The user's preset Delete preset asked about (by name): the next press deletes it.
    armed: Option<String>,
}

/// What a press of Delete preset did.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::app) enum DeletePress {
    /// Asked about the preset (named).
    Asked(String),
    /// Deleted it.
    Deleted(String),
}

impl Export {
    /// For a sheet that only shows a running export.
    pub fn idle() -> Export {
        Export::new(Vec::new(), None)
    }

    pub fn new(user: Vec<Preset>, last: Option<Preset>) -> Export {
        let preset = last
            .map(|p| Preset {
                id: String::new(),
                ..p
            })
            .unwrap_or_else(|| Preset::builtin().remove(0))
            .normalised();
        let mut sizes = Sizes {
            width: 1920,
            height: 1080,
            long: 2048,
            percent: 50,
        };
        match preset.size {
            Resize::Fit { width, height } => {
                sizes.width = width;
                sizes.height = height;
            }
            Resize::LongEdge(n) => sizes.long = n,
            Resize::Percent(p) => sizes.percent = p,
            Resize::None => {}
        }
        let (sub_name, chosen) = match &preset.folder {
            OutputDir::Subfolder(s) if !s.trim().is_empty() => (s.clone(), None),
            OutputDir::Folder(p) => ("export".to_owned(), Some(p.clone())),
            OutputDir::Subfolder(_) => ("export".to_owned(), None),
        };
        Export {
            preset,
            user,
            sub_name,
            chosen,
            sizes,
            current: None,
            armed: None,
        }
    }

    /// The user's preset these choices are (with `pattern` as the name): its index.
    fn user_match(&self, pattern: &str) -> Option<usize> {
        let mut me = self.preset.clone();
        pattern.clone_into(&mut me.pattern);
        self.user.iter().position(|p| p.same_choices(&me))
    }

    /// Delete preset's label, when the choices are one of the user's presets: the question
    /// once asked.
    pub fn delete_label(&self, pattern: &str) -> Option<String> {
        let p = &self.user[self.user_match(pattern)?];
        Some(if self.armed.as_deref() == Some(p.name.as_str()) {
            t!("develop-export-delete-preset-ask", name = p.name.clone())
        } else {
            t!("develop-export-delete-preset")
        })
    }

    /// Delete preset pressed: asks about the user's preset the choices are, or deletes it
    /// when it was just asked about.
    pub fn press_delete(&mut self, pattern: &str) -> Option<DeletePress> {
        let i = self.user_match(pattern)?;
        let name = self.user[i].name.clone();
        if self.armed.take().as_deref() == Some(name.as_str()) {
            self.user.remove(i);
            Some(DeletePress::Deleted(name))
        } else {
            self.armed = Some(name.clone());
            Some(DeletePress::Asked(name))
        }
    }

    /// Anything else pressed: Delete preset asks again.
    pub fn disarm(&mut self) {
        self.armed = None;
    }

    /// Shipped presets, then the user's.
    fn all(&self) -> Vec<Preset> {
        let mut v = Preset::builtin();
        v.extend(self.user.iter().cloned());
        v
    }

    /// The preset these choices are (with `pattern` as the name), if any.
    fn matching(&self, pattern: &str) -> Option<Preset> {
        let mut me = self.preset.clone();
        pattern.clone_into(&mut me.pattern);
        self.all().into_iter().find(|p| p.same_choices(&me))
    }

    fn preset_label(&self, pattern: &str) -> String {
        match self.matching(pattern) {
            Some(p) if !p.id.is_empty() => builtin_name(&p.id),
            Some(p) => p.name,
            None => t!("develop-export-preset-custom"),
        }
    }

    /// The start column: the images, the preset, the format and its rows, the size.
    pub fn start_rows(&self) -> Vec<Row> {
        let p = &self.preset;
        let mut v = vec![Row::Images, Row::Preset, Row::Format];
        if p.format.lossy() {
            v.push(Row::Quality);
        }
        if p.format.depths().len() > 1 {
            v.push(Row::Depth);
        }
        v.push(Row::Resize);
        match p.size {
            Resize::None => {}
            Resize::Fit { .. } => v.extend([Row::Width, Row::Height]),
            Resize::LongEdge(_) => v.push(Row::LongEdge),
            Resize::Percent(_) => v.push(Row::Percent),
        }
        if p.size != Resize::None {
            v.push(Row::Upscale);
        }
        v
    }

    /// The end column: colour, metadata and where the files go.
    pub fn end_rows(&self) -> Vec<Row> {
        let mut v = vec![Row::Space];
        if HdrOutput::ALL
            .iter()
            .any(|h| *h != HdrOutput::Off && h.fits(self.preset.format))
        {
            v.push(Row::Hdr);
        }
        v.extend([Row::Metadata, Row::Folder, Row::Conflicts]);
        v
    }

    pub fn row_views(&self, rows: &[Row], source: &str, pattern: &str) -> Vec<RowView> {
        rows.iter()
            .map(|&row| RowView {
                row,
                label: if row == Row::Hdr {
                    t!("develop-export-hdr")
                } else {
                    row_label(row)
                },
                value: if row == Row::Preset {
                    self.preset_label(pattern)
                } else {
                    self.value(row, source)
                },
            })
            .collect()
    }

    fn format_label(f: FileFormat) -> String {
        match f {
            FileFormat::Exr => t!("develop-export-format-exr"),
            f => option_label("format", f.id()),
        }
    }

    fn space_label(s: Space) -> String {
        match s {
            Space::Srgb => t!("develop-export-space-srgb"),
            Space::DisplayP3 => t!("develop-export-space-display-p3"),
            Space::Rec2020 => t!("develop-export-space-rec2020"),
            Space::AdobeRgb => t!("develop-export-space-adobe-rgb"),
        }
    }

    fn hdr_label(h: HdrOutput) -> String {
        match h {
            HdrOutput::Off => t!("develop-export-hdr-off"),
            HdrOutput::Pq => t!("develop-export-hdr-pq"),
            HdrOutput::Hlg => t!("develop-export-hdr-hlg"),
            HdrOutput::GainMap => t!("develop-export-hdr-gain-map"),
        }
    }

    fn depth_label(d: Depth) -> String {
        match d {
            Depth::Eight => option_label("depth", "8"),
            Depth::Sixteen => option_label("depth", "16"),
            Depth::Half => t!("develop-export-depth-half"),
            Depth::Float => t!("develop-export-depth-float"),
        }
    }

    fn size_label(size: Resize) -> String {
        match size {
            Resize::None => t!("develop-export-size-full"),
            Resize::LongEdge(n) => t!("convert-pixels", value = i64::from(n)),
            Resize::Fit { width, height } => format!(
                "{} × {}",
                t!("convert-pixels", value = i64::from(width)),
                t!("convert-pixels", value = i64::from(height))
            ),
            Resize::Percent(p) => t!("unit-percent", value = i64::from(p)),
        }
    }

    /// A row's value as the sheet shows it.
    pub fn value(&self, row: Row, source: &str) -> String {
        let p = &self.preset;
        let px = |v: u32| t!("convert-pixels", value = i64::from(v));
        let onoff = |b: bool| {
            if b {
                t!("convert-on")
            } else {
                t!("convert-off")
            }
        };
        match row {
            Row::Images => source.to_owned(),
            Row::Preset => self.preset_label(&p.pattern),
            Row::Format => Self::format_label(p.format),
            Row::Quality if p.lossless => t!("convert-lossless"),
            Row::Quality => p.quality.to_string(),
            Row::Depth => Self::depth_label(p.depth),
            Row::Resize => match p.size {
                Resize::None => option_label("resize", "none"),
                Resize::Fit { .. } => option_label("resize", "fit"),
                Resize::LongEdge(_) => option_label("resize", "long_edge"),
                Resize::Percent(_) => option_label("resize", "percent"),
            },
            Row::Width => px(self.sizes.width),
            Row::Height => px(self.sizes.height),
            Row::LongEdge => px(self.sizes.long),
            Row::Percent => t!("unit-percent", value = i64::from(self.sizes.percent)),
            Row::Upscale => onoff(p.upscale),
            Row::Space => Self::space_label(p.space),
            Row::Hdr => Self::hdr_label(p.hdr),
            Row::Metadata => option_label(
                "metadata",
                match p.metadata {
                    MetaMode::All => "all",
                    MetaMode::NoLocation => "no_location",
                    MetaMode::Strip => "strip",
                },
            ),
            Row::Folder => match &p.folder {
                OutputDir::Subfolder(s) if s.trim().is_empty() => t!("convert-folder-same"),
                OutputDir::Subfolder(s) => t!("convert-folder-subfolder", name = s.clone()),
                OutputDir::Folder(d) => {
                    let n = file_name(d);
                    let n = if n.is_empty() {
                        d.to_string_lossy().into_owned()
                    } else {
                        n
                    };
                    specular_i18n::isolate(&n)
                }
            },
            Row::Conflicts => option_label(
                "conflicts",
                match p.conflict {
                    Conflict::Suffix => "suffix",
                    Conflict::Skip => "skip",
                },
            ),
            Row::Chroma | Row::Effort | Row::Compression | Row::Colour => String::new(),
        }
    }

    /// Applies the size rows' values to the preset.
    fn sync_size(&mut self) {
        let s = self.sizes;
        self.preset.size = match self.preset.size {
            Resize::None => Resize::None,
            Resize::Fit { .. } => Resize::Fit {
                width: s.width,
                height: s.height,
            },
            Resize::LongEdge(_) => Resize::LongEdge(s.long),
            Resize::Percent(_) => Resize::Percent(s.percent),
        };
    }

    /// Steps `row` by `d`; `name` is the sheet's name field (a preset sets it). Whether
    /// anything changed.
    pub fn step(&mut self, row: Row, d: i32, name: &mut LineEdit) -> bool {
        name.text().clone_into(&mut self.preset.pattern);
        let before = self.preset.clone();
        let before_sizes = self.sizes;
        let wrap = |i: usize, n: usize| (i as i32 + d).rem_euclid(n.max(1) as i32) as usize;
        if row == Row::Preset {
            let all = self.all();
            let i = match all.iter().position(|x| x.same_choices(&self.preset)) {
                Some(i) => wrap(i, all.len()),
                None if d > 0 => 0,
                None => all.len() - 1,
            };
            let picked = all[i].clone();
            name.set_text(&picked.pattern);
            let chosen = self.chosen.clone();
            *self = Export {
                user: std::mem::take(&mut self.user),
                current: self.current.take(),
                ..Export::new(Vec::new(), Some(picked))
            };
            self.chosen = self.chosen.take().or(chosen);
            return true;
        }
        let p = &mut self.preset;
        match row {
            Row::Format => {
                let all = FileFormat::ALL;
                let i = all.iter().position(|f| *f == p.format).unwrap_or(0);
                p.format = all[wrap(i, all.len())];
                *p = p.clone().normalised();
            }
            Row::Quality => {
                let mut q = i64::from(p.quality);
                if p.format == FileFormat::Jxl {
                    step_lossy(&mut p.lossless, &mut q, d);
                } else {
                    q = step5(q, d, 1, 100);
                }
                p.quality = q.clamp(1, 100) as u8;
            }
            Row::Depth => {
                let all = p.format.depths();
                let i = all.iter().position(|x| *x == p.depth).unwrap_or(0);
                p.depth = all[wrap(i, all.len())];
            }
            Row::Space => {
                let i = Space::ALL.iter().position(|x| *x == p.space).unwrap_or(0);
                p.space = Space::ALL[wrap(i, Space::ALL.len())];
                // PQ and HLG are Rec. 2020 only: another space turns HDR off.
                if matches!(p.hdr, HdrOutput::Pq | HdrOutput::Hlg) && p.space != Space::Rec2020 {
                    p.hdr = HdrOutput::Off;
                }
            }
            Row::Hdr => {
                let all: Vec<HdrOutput> = HdrOutput::ALL
                    .into_iter()
                    .filter(|h| h.fits(p.format))
                    .collect();
                let i = all.iter().position(|x| *x == p.hdr).unwrap_or(0);
                p.hdr = all[wrap(i, all.len())];
                *p = p.clone().normalised();
            }
            Row::Resize => {
                let kinds = [0usize, 1, 2, 3];
                let i = match p.size {
                    Resize::None => 0,
                    Resize::Fit { .. } => 1,
                    Resize::LongEdge(_) => 2,
                    Resize::Percent(_) => 3,
                };
                let s = self.sizes;
                p.size = match kinds[wrap(i, 4)] {
                    0 => Resize::None,
                    1 => Resize::Fit {
                        width: s.width,
                        height: s.height,
                    },
                    2 => Resize::LongEdge(s.long),
                    _ => Resize::Percent(s.percent),
                };
            }
            Row::Width => self.sizes.width = step_size(i64::from(self.sizes.width), d) as u32,
            Row::Height => self.sizes.height = step_size(i64::from(self.sizes.height), d) as u32,
            Row::LongEdge => self.sizes.long = step_size(i64::from(self.sizes.long), d) as u32,
            Row::Percent => {
                self.sizes.percent = step5(i64::from(self.sizes.percent), d, 5, 400) as u32
            }
            Row::Upscale => p.upscale = !p.upscale,
            Row::Metadata => {
                let all = [MetaMode::All, MetaMode::NoLocation, MetaMode::Strip];
                let i = all.iter().position(|x| *x == p.metadata).unwrap_or(0);
                p.metadata = all[wrap(i, all.len())];
            }
            Row::Folder => {
                let mut modes = vec![
                    OutputDir::Subfolder(self.sub_name.clone()),
                    OutputDir::Subfolder(String::new()),
                ];
                if let Some(c) = &self.chosen {
                    modes.push(OutputDir::Folder(c.clone()));
                }
                let i = modes.iter().position(|m| *m == p.folder).unwrap_or(0);
                p.folder = modes[wrap(i, modes.len())].clone();
            }
            Row::Conflicts => {
                p.conflict = match p.conflict {
                    Conflict::Suffix => Conflict::Skip,
                    Conflict::Skip => Conflict::Suffix,
                }
            }
            Row::Images
            | Row::Preset
            | Row::Chroma
            | Row::Effort
            | Row::Compression
            | Row::Colour => {}
        }
        self.sync_size();
        self.preset != before || self.sizes != before_sizes
    }

    /// A folder picked with Choose folder….
    pub fn choose(&mut self, p: PathBuf) {
        self.chosen = Some(p.clone());
        self.preset.folder = OutputDir::Folder(p);
    }

    /// "IMG_1.CR3 → export/IMG_1.jpg" for the first image, or what is wrong with `pattern`.
    pub fn preview(
        &self,
        pattern: &str,
        first: Option<&Path>,
        probe: Option<Probe>,
    ) -> Result<String, String> {
        let pat = Pattern::parse(pattern).map_err(|e| convert::pattern_error(&e))?;
        let Some(src) = first else {
            return Ok(String::new());
        };
        let stem = src
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let p = &self.preset;
        let (width, height) = probe.as_ref().map_or((0, 0), |&((w, h), ..)| {
            specular_convert::output_size(w, h, p.size, p.upscale)
        });
        let name = pat.render(&NameFacts {
            stem: &stem,
            index: 1,
            date: probe.as_ref().and_then(|p| p.1),
            width,
            height,
            ext: p.format.ext(),
            camera: probe.as_ref().and_then(|p| p.2.as_deref()),
            ..Default::default()
        });
        let out = match &p.folder {
            OutputDir::Subfolder(s) if s.trim().is_empty() => name,
            o => {
                let dir = specular_convert::output_dir(src, o);
                format!("{}{}{name}", file_name(&dir), std::path::MAIN_SEPARATOR)
            }
        };
        Ok(t!(
            "convert-name-preview",
            source = file_name(src),
            output = out
        ))
    }

    /// The choices with `pattern`, as a preset named after them.
    fn as_preset(&self, pattern: &str) -> Preset {
        let mut p = self.preset.clone().normalised();
        pattern.clone_into(&mut p.pattern);
        p.id = String::new();
        p.name = t!(
            "develop-export-preset-name",
            format = Self::format_label(p.format),
            space = Self::space_label(p.space),
            size = Self::size_label(p.size)
        );
        p
    }
}

/// An export's progress in Convert's terms (the sheet and the downloads row).
fn progress_of(p: &batch::Progress) -> Progress {
    Progress {
        done: p.done,
        total: p.total,
        failed: p.failed,
        skipped: p.skipped,
        elapsed: p.elapsed,
        eta: p.eta,
    }
}

/// An export's end report in Convert's terms.
fn report_of(r: batch::Report) -> Report {
    Report {
        results: r
            .results
            .into_iter()
            .map(|(p, o)| {
                let o = match o {
                    Outcome::Written { out, bytes } => COutcome::Converted { out, bytes },
                    Outcome::Skipped { existing } => COutcome::Skipped { existing },
                    Outcome::Failed(f) => COutcome::Failed(f),
                    Outcome::Cancelled => COutcome::Cancelled,
                };
                (p, o)
            })
            .collect(),
        cancelled: r.cancelled,
        elapsed: r.elapsed,
    }
}

impl App {
    /// Opens the export sheet for the image in Develop (or shows a running export).
    pub(in crate::app) fn develop_export_open(&mut self) {
        if self.convert.dialog.is_some() {
            return;
        }
        if self.convert.run.is_some() {
            self.convert_open();
            return;
        }
        let Some(s) = self.develop.session.as_ref() else {
            return;
        };
        let path = s.path.clone();
        let edit = s.edit.clone();
        let mut sources = vec![(Source::Current, vec![path.clone()])];
        if self.entries.len() > 1 {
            sources.push((Source::Folder, self.entries.clone()));
        }
        let picks = self.marked_files();
        if !picks.is_empty() {
            sources.push((Source::Picks, picks));
        }
        let (user, last) = load_presets();
        let mut e = Export::new(user, last);
        e.current = Some((path, edit, super::orient_of(&self.view)));
        let pattern = e.preset.pattern.clone();
        self.convert.dialog = Some(Dialog {
            sources,
            source: 0,
            choices: Choices::new(self.settings.convert.clone()),
            name: LineEdit::new(&pattern),
            probe: None,
            probing: None,
            picking: None,
            phase: Phase::Setup,
            // On the preset.
            focus: Some(1),
            drawn: Drawn::default(),
            preedit: String::new(),
            export: Some(Box::new(e)),
        });
        self.convert_probe();
        self.chrome.announce_status(t!("develop-export-title"));
        self.redraw = true;
    }

    /// Keeps the sheet's choices as the last ones (a bad name pattern keeps the last good
    /// one).
    pub(in crate::app) fn develop_export_save_choices(&mut self) {
        if self.args.screenshot.is_some() {
            return;
        }
        let Some(d) = self.convert.dialog.as_ref() else {
            return;
        };
        let Some(e) = d.export.as_ref() else { return };
        let mut last = e.preset.clone();
        if Pattern::parse(d.name.text()).is_ok() {
            d.name.text().clone_into(&mut last.pattern);
        }
        save_presets(&e.user, &last);
    }

    /// Save preset: the choices become a preset named after them.
    pub(in crate::app) fn develop_export_save_preset(&mut self) {
        let Some(d) = self.convert.dialog.as_mut() else {
            return;
        };
        let pattern = d.name.text().to_owned();
        let Some(e) = d.export.as_mut() else { return };
        if Pattern::parse(&pattern).is_err() {
            return;
        }
        let msg = match e.matching(&pattern) {
            Some(p) => {
                let name = if p.id.is_empty() {
                    p.name
                } else {
                    builtin_name(&p.id)
                };
                t!("develop-export-preset-exists", name = name)
            }
            None => {
                let p = e.as_preset(&pattern);
                let name = p.name.clone();
                e.user.push(p);
                t!("develop-export-preset-saved", name = name)
            }
        };
        self.develop_export_save_choices();
        self.chrome.announce_status(msg.clone());
        let now = self.now();
        self.chrome.toast(msg, None, now);
        self.redraw = true;
    }

    /// Delete preset: asks first, deletes on the second press.
    pub(in crate::app) fn develop_export_delete_preset(&mut self) {
        let Some(d) = self.convert.dialog.as_mut() else {
            return;
        };
        let pattern = d.name.text().to_owned();
        let Some(e) = d.export.as_mut() else { return };
        let msg = match e.press_delete(&pattern) {
            Some(DeletePress::Asked(name)) => {
                t!("develop-export-delete-preset-confirm", name = name)
            }
            Some(DeletePress::Deleted(name)) => {
                self.develop_export_save_choices();
                t!("develop-export-preset-deleted", name = name)
            }
            None => return,
        };
        self.chrome.announce_status(msg.clone());
        let now = self.now();
        self.chrome.toast(msg, None, now);
        self.redraw = true;
    }

    /// Starts the queue for the sheet's images.
    pub(in crate::app) fn develop_export_start(&mut self) {
        let Some(d) = self.convert.dialog.as_mut() else {
            return;
        };
        let Some(e) = d.export.as_ref() else { return };
        let mut preset = e.preset.clone().normalised();
        d.name.text().clone_into(&mut preset.pattern);
        let paths = d.sources[d.source].1.clone();
        // Offline photos with proxies: the export says so first, and may wait.
        if self.local_export_check(&paths) {
            return;
        }
        let Some(d) = self.convert.dialog.as_mut() else {
            return;
        };
        let Some(e) = d.export.as_ref() else { return };
        let current = e.current.clone();
        let total = paths.len();
        let lib = self.gallery.library_handle();
        let gpu = self.gfx.as_ref().map(|g| {
            (
                g.gpu.device.clone(),
                g.gpu.queue.clone(),
                g.gpu.info.device_type != specular_gfx::wgpu::DeviceType::DiscreteGpu,
            )
        });
        let registry = self.loader.registry();
        // Exports run AI denoise's whole frame first (through this runner).
        super::ai_denoise::install_runner(gpu.as_ref().map(|g| (g.0.clone(), g.1.clone())));
        let wake = self.loader.waker();
        let cancel = CancelToken::new();
        let c = cancel.clone();
        let (ptx, prx) = crossbeam_channel::unbounded();
        let (rtx, rrx) = crossbeam_channel::bounded(1);
        specular_log::info!(
            "develop export: {total} image(s) to {} ({})",
            preset.format.id(),
            preset.space.id()
        );
        self.loader.background("dv-export", move |_| {
            let jobs: Vec<Job> = paths
                .iter()
                .map(|p| match &current {
                    Some((cp, edit, o)) if cp == p => Job {
                        source: p.clone(),
                        edit: edit.clone(),
                        orient: Some(*o),
                    },
                    _ => Job {
                        source: p.clone(),
                        edit: super::store::load(p, lib.as_deref())
                            .map_or_else(|| super::controls::new_edit(p), |f| f.edit),
                        orient: None,
                    },
                })
                .collect();
            let opened = match gpu {
                Some(g) => Ok(g),
                None => batch::open_device(),
            };
            let report = match opened {
                Ok((device, queue, integrated)) => {
                    let w = wake.clone();
                    let send = move |p: &batch::Progress| {
                        let _ = ptx.send((progress_of(p), Some(p.fraction)));
                        w();
                    };
                    report_of(batch::run(
                        &device,
                        &queue,
                        Budget::for_device(integrated, None, None),
                        jobs,
                        &preset,
                        &registry,
                        &c,
                        &send,
                    ))
                }
                Err(err) => {
                    specular_log::warn!("develop export: {err}");
                    let why = t!("develop-export-no-gpu");
                    Report {
                        results: paths
                            .into_iter()
                            .map(|p| {
                                (
                                    p,
                                    COutcome::Failed(specular_convert::Failure::Decode(
                                        why.clone(),
                                    )),
                                )
                            })
                            .collect(),
                        cancelled: false,
                        elapsed: std::time::Duration::ZERO,
                    }
                }
            };
            let _ = rtx.send(report);
            wake();
        });
        self.convert.run = Some(Run {
            cancel,
            progress: prx,
            report: rrx,
            last: Progress {
                total,
                ..Progress::default()
            },
            fraction: Some(0.0),
            stopping: false,
            toast: None,
            toast_at: 0.0,
            export: true,
        });
        if let Some(d) = self.convert.dialog.as_mut() {
            d.phase = Phase::Running;
            // On Hide, not Stop (Stop comes first in reading order).
            d.focus = Some(1);
        }
        self.chrome.announce_status(t!(
            "develop-export-progress",
            done = 0,
            total = total as i64
        ));
        self.develop_export_refresh();
        self.redraw = true;
    }

    /// Shows the export's progress in the downloads indicator now.
    pub(in crate::app) fn develop_export_refresh(&mut self) {
        self.net_refresh();
    }

    /// The downloads indicator's row for a running export.
    pub(in crate::app) fn develop_export_row(&self) -> Option<DownloadRow> {
        let r = self.convert.run.as_ref().filter(|r| r.export)?;
        let p = &r.last;
        let status = match p.eta {
            Some(eta) if p.done > 0 && p.done < p.total => super::super::work::eta_text(eta),
            _ if p.done == 0 => t!("develop-export-starting"),
            _ => t!("net-running"),
        };
        Some(DownloadRow {
            title: t!("develop-export-row"),
            detail: t!(
                "develop-export-row-detail",
                done = p.done as i64,
                total = p.total as i64
            ),
            status,
            fraction: Some(r.done()),
            state: DownloadState::Running,
            stop: (!r.stopping).then(|| (t!("net-stop"), "export-stop".to_owned())),
        })
    }
}

/// A photo's saved edit (sidecar or catalogue); with `or_new`, the default edit when it has
/// none (Develop's own exports develop every photo).
pub(in crate::app) fn saved_edit(
    p: &Path,
    lib: Option<&specular_library::Library>,
    or_new: bool,
) -> Option<Edit> {
    match super::store::load(p, lib) {
        Some(f) => Some(f.edit),
        None if or_new => Some(super::controls::new_edit(p)),
        None => None,
    }
}

impl App {
    /// The image open in Develop: its path, edit and turn.
    pub(in crate::app) fn develop_current(&self) -> Option<(PathBuf, Edit, Orient)> {
        let s = self.develop.session.as_ref()?;
        Some((s.path.clone(), s.edit.clone(), super::orient_of(&self.view)))
    }
}

/// Installs AI denoise's runner for exports outside this sheet (export recipes).
pub(in crate::app) fn install_denoise_runner(
    gpu: Option<(specular_gfx::wgpu::Device, specular_gfx::wgpu::Queue)>,
) {
    super::ai_denoise::install_runner(gpu);
}

#[cfg(test)]
mod shots;

#[cfg(test)]
mod tests {
    use super::*;

    /// Stepping every row keeps a preset the format can hold; the preset row walks the
    /// shipped presets and sets the name; Save preset names the choices.
    #[test]
    fn rows_step_within_the_format() {
        let mut e = Export::new(Vec::new(), None);
        let mut name = LineEdit::new("{name}");
        assert_eq!(e.preset_label("{name}"), t!("develop-export-preset-jpeg"));
        assert!(e.step(Row::Format, 1, &mut name));
        assert_eq!(e.preset.format, FileFormat::Png);
        assert!(e.start_rows().contains(&Row::Depth));
        assert_eq!(e.preset_label("{name}"), t!("develop-export-preset-custom"));
        for _ in 0..4 {
            e.step(Row::Format, 1, &mut name);
        }
        assert_eq!(e.preset.format, FileFormat::Exr);
        assert_eq!(e.preset.depth, Depth::Half);
        assert!(!e.start_rows().contains(&Row::Quality));
        e.step(Row::Depth, 1, &mut name);
        assert_eq!(e.preset.depth, Depth::Float);
        // Presets: the next shipped one after Custom is the first.
        e.step(Row::Preset, 1, &mut name);
        assert_eq!(
            e.preset_label(name.text()),
            t!("develop-export-preset-jpeg")
        );
        e.step(Row::Preset, 1, &mut name);
        assert_eq!(e.preset.size, Resize::LongEdge(2048));
        assert!(e.start_rows().contains(&Row::LongEdge));
        e.step(Row::LongEdge, 1, &mut name);
        assert_eq!(e.preset.size, Resize::LongEdge(2160));
        e.step(Row::Resize, 1, &mut name);
        assert_eq!(e.preset.size, Resize::Percent(50));
        e.step(Row::Space, 1, &mut name);
        assert_eq!(e.preset.space, Space::DisplayP3);
        let p = e.as_preset("{name}-web");
        assert!(p.name.contains("JPEG"), "{}", p.name);
        assert_eq!(p.pattern, "{name}-web");
        // Text form round trip of the user's preset.
        let back = Preset::parse_all(&Preset::write_all(std::slice::from_ref(&p)));
        assert_eq!(back[0], p);
    }

    /// Delete preset shows only on the user's presets, asks first and deletes on the
    /// second press; moving on in between asks again.
    #[test]
    fn user_presets_are_deleted_after_asking() {
        let mut e = Export::new(Vec::new(), None);
        let mut name = LineEdit::new("{name}");
        // A shipped preset: no Delete preset.
        assert_eq!(e.delete_label("{name}"), None);
        assert_eq!(e.press_delete("{name}"), None);
        e.step(Row::Format, 1, &mut name);
        let mine = e.as_preset("{name}");
        let n = mine.name.clone();
        e.user.push(mine);
        assert_eq!(
            e.delete_label("{name}"),
            Some(t!("develop-export-delete-preset"))
        );
        assert_eq!(
            e.press_delete("{name}"),
            Some(DeletePress::Asked(n.clone()))
        );
        assert_eq!(
            e.delete_label("{name}"),
            Some(t!("develop-export-delete-preset-ask", name = n.clone()))
        );
        e.disarm();
        assert_eq!(
            e.press_delete("{name}"),
            Some(DeletePress::Asked(n.clone()))
        );
        assert_eq!(e.press_delete("{name}"), Some(DeletePress::Deleted(n)));
        assert!(e.user.is_empty());
        assert_eq!(e.delete_label("{name}"), None);
    }

    #[test]
    fn previews_name_the_new_file() {
        let e = Export::new(Vec::new(), None);
        let src = Path::new("/photos/IMG_0042.CR3");
        let probe = Some(((6000, 4000), None, None));
        let s = e.preview("{name}-{w}", Some(src), probe).unwrap();
        assert!(s.contains("IMG_0042-6000.jpg"), "{s}");
        assert!(s.contains("export"), "{s}");
        assert!(e.preview("{nope}", Some(src), None).is_err());
    }
}
