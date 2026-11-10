//! Structured reports and waveforms in the viewer.
//!
//! - A structured report's content tree is browsed in the tag browser (report mode: item,
//!   concept, value type, relationship and value columns). Items that point at an image
//!   (IMAGE, SCOORD) or a place in the patient (SCOORD3D) jump to it: the image with that SOP
//!   Instance UID in the report's folder, or the slice of the same frame of reference nearest
//!   the point, at the referenced frame.
//! - A waveform (ECG, haemodynamic…) opens as its summary page; its traces are laid out on
//!   the plot sheet (mm paper, 25 mm/s and 10 mm/mV, calipers), shown when the file is.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use specular_dicom::series;
use specular_dicom::sr::{Jump, Report, Value};
use specular_dicom::waveform::{self, Paper, Waveforms};
use specular_i18n::{I18n, t};
use specular_log::{FOLDER, debug};
use specular_widgets::{CaliperKind, DicomTags, PlotCell, PlotGrid, PlotSheet, PlotTrace, TagLine};

use super::App;
use crate::loader::Event;

/// Most files of a folder read to find a report's image.
const JUMP_FILES: usize = 4000;
/// Farthest (mm) a slice may lie from a SCOORD3D point and still be shown for it.
const JUMP_TOLERANCE: f64 = 10.0;
/// How long a jump waits for its file to open in medical mode before its frame is shown.
const JUMP_WAIT: std::time::Duration = std::time::Duration::from_secs(10);

/// Report and waveform state.
#[derive(Default)]
pub(super) struct ReportState {
    /// `--show dvh`: show the dose volume histograms when MPR's search finds a dose.
    pub(super) dvh_on_dose: bool,
    /// An OCT volume's slider and localiser (`oct.rs`).
    pub(super) oct: super::oct::OctState,
    /// The tag browser shows a report: its file and each row's jump.
    jumps: Option<(PathBuf, Vec<Option<Jump>>)>,
    /// A jump arrived at its file: the frame (0-based) to show once the file is in medical
    /// mode (or its page, for a multi-page file), and until when to wait for that.
    frame: Option<(PathBuf, u32, std::time::Instant)>,
    /// The waveform on the plot sheet: its file, record and paper.
    ecg: Option<(PathBuf, Arc<Waveforms>, Paper)>,
    /// The file whose waveform was shown (or is being read), so it is offered once per visit.
    seen: Option<PathBuf>,
}

impl App {
    /// The tag browser's rows for a structured report, and remembers their jumps.
    pub(super) fn report_tags(&mut self, file: PathBuf, report: &Report) -> DicomTags {
        let title = file
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let (tags, jumps) = report_lines(report, title);
        self.report.jumps = Some((file, jumps));
        tags
    }

    /// The tag browser closed or shows a plain data set: no report rows to jump from.
    pub(super) fn report_tags_closed(&mut self) {
        self.report.jumps = None;
    }

    /// Enter (or the row's button) on a report row: finds what it points at in the report's
    /// folder on a worker.
    pub(super) fn report_jump(&mut self, row: usize) {
        let Some((file, jumps)) = &self.report.jumps else {
            return;
        };
        let Some(Some(jump)) = jumps.get(row).cloned() else {
            return;
        };
        let file = file.clone();
        let Some(dir) = file.parent().map(Path::to_path_buf) else {
            return;
        };
        self.loader.background("sr-jump", move |l| {
            let t0 = std::time::Instant::now();
            let found = find_jump(&dir, &jump);
            debug!(target: FOLDER, "report jump: {} in {:.1} ms", if found.is_some() { "found" } else { "not found" }, t0.elapsed().as_secs_f64() * 1e3);
            let found = found.ok_or(match jump {
                Jump::Image { .. } => "sr-jump-missing",
                Jump::Patient { .. } => "sr-jump-frame",
            });
            l.post(Event::ReportJump { file, found });
        });
    }

    /// A report jump's target, found (or not).
    pub(super) fn on_report_jump(
        &mut self,
        file: PathBuf,
        found: Result<(PathBuf, u32), &'static str>,
    ) {
        if self.report.jumps.as_ref().map(|j| &j.0) != Some(&file) {
            return;
        }
        let (path, frame) = match found {
            Ok(f) => f,
            Err(key) => {
                let now = self.now();
                self.chrome.toast(I18n::global().tr(key), None, now);
                self.redraw = true;
                return;
            }
        };
        self.chrome.close_modal();
        self.report.jumps = None;
        match self.entries.iter().position(|e| *e == path) {
            Some(i) => self.go_to(i),
            None => self.open_target(path.clone()),
        }
        self.report.frame =
            (frame > 0).then(|| (path, frame, std::time::Instant::now() + JUMP_WAIT));
        self.redraw = true;
    }

    /// Shows a jump's frame once its file is shown in medical mode (from the animation tick).
    pub(super) fn report_tick(&mut self) {
        let Some((path, frame, by)) = &self.report.frame else {
            return;
        };
        // The file may take a while to open; a jump not done by then is dropped.
        if std::time::Instant::now() > *by {
            self.report.frame = None;
            return;
        }
        if self.current.and_then(|i| self.entries.get(i)) != Some(path) {
            return;
        }
        let frame = *frame;
        if self.medical_jump_to(frame) {
            self.report.frame = None;
            return;
        }
        // A multi-page file (not a DICOM's frames): the page, once the file has opened.
        if let Some(id) = self.current_id()
            && let Some(img) = self.images.get(&id)
            && img.session.is_some()
            && img.pages > 1
            && self.medical_state().is_none()
        {
            if frame < img.pages && img.page != frame {
                self.page_switch(id, frame);
            }
            self.report.frame = None;
        }
    }

    /// Shows frame (or page) `frame` of `path` once it is the current file: a teaching
    /// quiz's key image, marked on that frame.
    pub(super) fn report_show_frame(&mut self, path: PathBuf, frame: u32) {
        self.report.frame =
            (frame > 0).then(|| (path, frame, std::time::Instant::now() + JUMP_WAIT));
    }

    /// The current file's session is known: a waveform's traces go on the plot sheet.
    pub(super) fn report_opened(&mut self) {
        self.oct_opened();
        let Some(id) = self.current_id() else {
            return;
        };
        let Some(path) = self.current.and_then(|i| self.entries.get(i)).cloned() else {
            return;
        };
        if self.report.seen.as_ref() == Some(&path) {
            return;
        }
        let waveform = self
            .images
            .get(&id)
            .and_then(|i| i.session.as_ref())
            .and_then(|s| {
                s.encapsulated()
                    .map(|e| e.kind == specular_decode::WAVEFORM_KIND)
            })
            .unwrap_or(false);
        if !waveform {
            return;
        }
        self.report.seen = Some(path.clone());
        self.loader.background("ecg-read", move |l| {
            let waves = std::fs::read(&path)
                .map_err(|e| crate::error_text::io_reason(&e))
                .and_then(|data| {
                    let d = specular_dicom::Dicom::parse(&data)
                        .map_err(|e| crate::error_text::dicom_reason(&e))?;
                    Waveforms::read(&d, &data).map_err(|e| crate::error_text::dicom_reason(&e))
                })
                .map(Arc::new);
            l.post(Event::Waveform { file: path, waves });
        });
    }

    /// `--show dicom-tags` (the tag browser, or a report's content tree) and `--show dvh` (MPR
    /// with its dose's histograms), for screenshots.
    pub(super) fn report_show(&mut self, what: &str) -> bool {
        match what {
            "dicom-tags" => {
                if !self.chrome.dicom_tags_open() {
                    self.dicom_tags_toggle();
                }
                true
            }
            "dvh" => {
                self.report.dvh_on_dose = true;
                self.mpr_show("mpr")
            }
            _ => false,
        }
    }

    /// The viewer moved to another file: its waveform may be offered again on return.
    pub(super) fn report_left(&mut self) {
        self.oct_left();
        self.report.seen = None;
        if self.chrome.plot_open() && self.report.ecg.is_some() {
            self.chrome.close_modal();
        }
        self.report.ecg = None;
    }

    /// A waveform's record, read.
    pub(super) fn on_waveform(&mut self, file: PathBuf, waves: Result<Arc<Waveforms>, String>) {
        let here = self.current.and_then(|i| self.entries.get(i));
        if here != Some(&file) {
            return;
        }
        let w = match waves {
            Ok(w) => w,
            Err(reason) => {
                debug!(target: FOLDER, "waveform not read: {reason}");
                return;
            }
        };
        let paper = Paper::default();
        let Some(sheet) = ecg_sheet(&w, paper) else {
            return;
        };
        if self.chrome.modal().is_some() {
            return;
        }
        self.chrome.show_plot(sheet);
        self.report.ecg = Some((file, w, paper));
        self.redraw = true;
    }

    /// `+`/`−` and `]`/`[` on the ECG sheet: the next gain or speed.
    pub(super) fn plot_adjust(&mut self, gain: i32, speed: i32) {
        let Some((_, w, paper)) = &mut self.report.ecg else {
            return;
        };
        let step = |list: &[f64], now: f64, by: i32| {
            let i = list
                .iter()
                .position(|&v| (v - now).abs() < 1e-9)
                .unwrap_or(0) as i32;
            list[(i + by).clamp(0, list.len() as i32 - 1) as usize]
        };
        paper.gain = step(&waveform::GAINS, paper.gain, gain);
        paper.speed = step(&waveform::SPEEDS, paper.speed, speed);
        if let Some(sheet) = ecg_sheet(w, *paper) {
            self.chrome.set_plot(sheet);
        }
        self.redraw = true;
    }
}

/// The tag browser's rows for `r` (one per content item, depth first) and each row's jump.
pub(super) fn report_lines(r: &Report, title: String) -> (DicomTags, Vec<Option<Jump>>) {
    let mut rows = Vec::new();
    let mut jumps = Vec::new();
    for (path, n) in r.walk() {
        let item = std::iter::once(1)
            .chain(path.iter().copied())
            .map(|k| k.to_string())
            .collect::<Vec<_>>()
            .join(".");
        let jump = n.jump();
        rows.push(TagLine {
            depth: path.len() as u16,
            tag: item,
            keyword: n.name().to_owned(),
            vr: n.value_type.clone(),
            length: n.relationship.clone(),
            value: n.display_value(),
            descendants: (n.count() - 1) as u32,
            identifying: matches!(n.value, Value::PersonName(_)),
            jump: jump.is_some(),
        });
        jumps.push(jump);
    }
    let heads = [
        t!("sr-col-item"),
        t!("sr-col-concept"),
        t!("sr-col-type"),
        t!("sr-col-relationship"),
        t!("sr-col-value"),
    ];
    let tags = DicomTags {
        title,
        rows,
        status: None,
        note: (r.dropped > 0).then(|| t!("dicom-tags-omitted", count = r.dropped as i64)),
        heads: Some((
            heads,
            t!("sr-tree-title", title = r.title(), kind = r.kind.name()),
        )),
    };
    (tags, jumps)
}

/// What `jump` points at among the DICOM files of `dir`: the file and frame (0-based).
pub(super) fn find_jump(dir: &Path, jump: &Jump) -> Option<(PathBuf, u32)> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_file() && super::series::is_dicom_name(p))
        .collect();
    files.sort();
    files.truncate(JUMP_FILES);
    let mut best: Option<(f64, PathBuf, u32)> = None;
    for p in files {
        let Ok(Some(h)) = series::read_header(&p) else {
            continue;
        };
        match jump {
            Jump::Image {
                sop_instance,
                frame,
                ..
            } => {
                if h.sop_uid == *sop_instance {
                    let f = frame
                        .unwrap_or(1)
                        .saturating_sub(1)
                        .min(h.frames.saturating_sub(1));
                    return Some((p, f));
                }
            }
            Jump::Patient {
                frame_of_reference,
                point,
            } => {
                if h.frame_of_reference != *frame_of_reference
                    || h.frames == 0
                    || matches!(
                        h.modality.as_str(),
                        "RTDOSE" | "SEG" | "RTSTRUCT" | "SR" | "PR"
                    )
                {
                    continue;
                }
                if let Some((d, f)) = nearest_frame(&h, *point)
                    && best.as_ref().is_none_or(|b| d < b.0)
                {
                    best = Some((d, p, f));
                }
            }
        }
    }
    best.filter(|b| b.0 <= JUMP_TOLERANCE)
        .map(|(_, p, f)| (p, f))
}

/// The frame of `h` whose plane lies nearest `point` (mm), and its distance.
pub(super) fn nearest_frame(h: &series::Instance, point: [f64; 3]) -> Option<(f64, u32)> {
    let o = h.orientation?;
    let n = [
        o[1] * o[5] - o[2] * o[4],
        o[2] * o[3] - o[0] * o[5],
        o[0] * o[4] - o[1] * o[3],
    ];
    let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    if len < 1e-6 {
        return None;
    }
    let dist = |q: &[f64; 3]| {
        ((point[0] - q[0]) * n[0] + (point[1] - q[1]) * n[1] + (point[2] - q[2]) * n[2]).abs() / len
    };
    if h.frame_positions.is_empty() {
        return h.position.map(|q| (dist(&q), 0));
    }
    h.frame_positions
        .iter()
        .enumerate()
        .map(|(i, q)| (dist(q), i as u32))
        .min_by(|a, b| a.0.total_cmp(&b.0))
}

/// The plot sheet of a waveform's first group at `paper`: the standard 12-lead layout when it
/// has the twelve leads, else one row per channel, on mm paper.
pub(super) fn ecg_sheet(w: &Waveforms, paper: Paper) -> Option<PlotSheet> {
    let g = w
        .groups
        .iter()
        .find(|g| !g.channels.is_empty() && g.frequency > 0.0 && g.samples > 0)?;
    let lay = waveform::layout(g, paper, 0.0);
    let loc = I18n::global().localizer();
    let cells = lay
        .cells
        .iter()
        .filter_map(|c| {
            let ch = g.channels.get(c.channel)?;
            let points = c
                .range(g.frequency, ch.samples.len())
                .map(|i| {
                    let p = c.point(g.frequency, i, ch.samples[i]);
                    [p[0] as f32, p[1] as f32]
                })
                .collect();
            let bottom = c.rect[1] + c.rect[3];
            let (y0, dy) = if c.scale.is_finite() && c.scale > 0.0 {
                (c.offset + (c.zero - bottom) / c.scale, 1.0 / c.scale)
            } else {
                (0.0, 0.0)
            };
            Some(PlotCell {
                rect: c.rect,
                label: c.label.clone(),
                traces: vec![PlotTrace {
                    points,
                    srgb: [0, 0, 0],
                    name: c.label.clone(),
                }],
                x_value: (c.t0, c.seconds / c.rect[2].max(1e-9)),
                y_value: (y0, dy),
                x_unit: "s".to_owned(),
                y_unit: ch.unit_symbol(),
                ..PlotCell::default()
            })
        })
        .collect();
    Some(PlotSheet {
        title: w.kind.name().to_owned(),
        page: [lay.width, lay.height],
        grid: PlotGrid::Paper {
            minor: 1.0,
            major: 5.0,
        },
        cells,
        caliper: CaliperKind::Time,
        subtitle: t!(
            "ecg-paper",
            speed = loc.number(paper.speed, 1),
            gain = loc.number(paper.gain, 1),
            rate = loc.number(g.frequency, 0)
        ),
        paper: true,
        adjustable: true,
        note: (w.groups.len() > 1)
            .then(|| t!("ecg-groups-more", count = (w.groups.len() - 1) as i64)),
        ..PlotSheet::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(rel: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../crates/formats/specular-dicom/tests/data/dicomrt")
            .join(rel)
    }

    fn report(rel: &str) -> Report {
        let data = std::fs::read(fixture(rel)).unwrap();
        let d = specular_dicom::Dicom::parse(&data).unwrap();
        Report::read(&d, &data).unwrap()
    }

    /// One row per content item, with its item path, and jumps where the item points at an
    /// image or a place.
    #[test]
    fn report_rows_and_jumps() {
        let r = report("study/sr_tid1500.dcm");
        let (tags, jumps) = report_lines(&r, "sr.dcm".into());
        assert_eq!(tags.rows.len(), r.walk().len());
        assert_eq!(tags.rows[0].tag, "1");
        assert_eq!(tags.rows[1].tag, "1.1");
        assert!(tags.heads.is_some());
        assert_eq!(jumps.len(), tags.rows.len());
        assert!(jumps.iter().any(Option::is_some));
        for (row, j) in tags.rows.iter().zip(&jumps) {
            assert_eq!(row.jump, j.is_some());
        }
    }

    /// Every jump of the fixture study's reports lands on an image in the study folder.
    #[test]
    fn jumps_find_their_images() {
        let dir = fixture("study");
        let mut found = 0;
        for f in [
            "sr_comprehensive.dcm",
            "sr_tid1500.dcm",
            "sr_tid1500_3d.dcm",
        ] {
            let r = report(&format!("study/{f}"));
            for (_, n) in r.walk() {
                if let Some(j) = n.jump() {
                    let (p, frame) = find_jump(&dir, &j).unwrap_or_else(|| panic!("{f}: {j:?}"));
                    let h = series::read_header(&p).unwrap().unwrap();
                    assert!(frame < h.frames.max(1), "{f}: {j:?}");
                    if let Jump::Image { sop_instance, .. } = &j {
                        assert_eq!(&h.sop_uid, sop_instance);
                    }
                    found += 1;
                }
            }
        }
        assert!(found > 0);
        let nowhere = Jump::Patient {
            frame_of_reference: "1.2.3.4".into(),
            point: [0.0; 3],
        };
        assert_eq!(find_jump(&dir, &nowhere), None);
    }

    /// The 12-lead record lays out as the standard sheet; gain and speed change the page's
    /// traces, not its size.
    #[test]
    fn ecg_sheet_lays_out_the_leads() {
        let data = std::fs::read(fixture("ecg/ecg_12lead.dcm")).unwrap();
        let d = specular_dicom::Dicom::parse(&data).unwrap();
        let w = Waveforms::read(&d, &data).unwrap();
        let s = ecg_sheet(&w, Paper::default()).unwrap();
        assert_eq!(s.cells.len(), 13, "twelve leads and the rhythm strip");
        assert!(s.paper && s.adjustable);
        let fast = ecg_sheet(
            &w,
            Paper {
                speed: 50.0,
                gain: 20.0,
            },
        )
        .unwrap();
        assert_eq!(fast.page, s.page);
        assert_ne!(fast.cells[0].traces, s.cells[0].traces);
        // A value under the caliper reads back in the channel's unit.
        let c = &s.cells[0];
        let p = c.traces[0].points[10];
        let v = c.y_value.0 + (c.rect[1] + c.rect[3] - f64::from(p[1])) * c.y_value.1;
        let ch = &w.groups[0].channels[w.groups[0].twelve_leads().unwrap()[0]];
        assert!(
            (v - f64::from(ch.samples[10])).abs() < 1e-2 * f64::from(ch.samples[10]).abs().max(1.0)
        );
    }
}
