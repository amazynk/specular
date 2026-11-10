//! The studies browser: what is on a patient's CD, DVD or USB stick (or in a folder of DICOM
//! files) as patients, their studies (date, description, modalities) and the studies' series
//! with a thumbnail and the number of images. Enter (or a click) on a series opens it. The
//! application reads the medium and localises the rows ([`Studies`]).
//!
//! The patients' names and birth dates are hidden until "Show patient data" is switched on,
//! for this opening of the browser only; the rows are dropped when it closes, and
//! [`StudyLine`]'s `Debug` never prints an identifying text.

use specular_gfx::{Icon, Rect, TexId, TextStyle, draw_icon};
use specular_i18n::t;
use specular_input::{KeyCombo, NamedKey};
use specular_theme::metrics;

use crate::ctx::{Ctx, HAlign, Target};
use crate::{Chrome, Command, Modal, Response, ViewerState};

/// The browser's content, localised by the application.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Studies {
    /// What was opened: the medium's or the folder's name (never patient data).
    pub title: String,
    /// Patients, each followed by its studies, each followed by its series.
    pub rows: Vec<StudyLine>,
    /// Under the list, or instead of it while there are no rows: "Reading the disc…", the
    /// progress of a scan, why the medium cannot be read.
    pub status: Option<String>,
    /// Under the list: files the DICOMDIR names that are not there, files left out.
    pub note: Option<String>,
    /// The medium is still being read and the reading can be stopped: a Cancel button
    /// beside the status ([`Command::StudiesCancel`]), and Esc stops it before it closes
    /// the browser.
    pub cancellable: bool,
    /// "Send to PACS…" is offered at the end of the footer ([`Command::StudiesSend`]) while
    /// nothing is being read.
    pub sendable: bool,
}

/// What a row is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StudyKind {
    #[default]
    Patient,
    Study,
    Series,
}

/// One row.
#[derive(Clone, Default, PartialEq)]
pub struct StudyLine {
    pub kind: StudyKind,
    /// Patient: the name. Study: the description. Series: number and description.
    pub title: String,
    /// Patient: the birth date. Study: the date and the modalities. Series: the modality and
    /// the number of images (and of missing ones).
    pub detail: String,
    /// `title` and `detail` identify the patient: shown only with "Show patient data" on.
    pub identifying: bool,
    /// Shown instead of `title` while it is hidden ("Patient 1").
    pub hidden_title: String,
    /// Series: the thumbnail (texture, width, height), once made.
    pub thumb: Option<(TexId, u32, u32)>,
    /// Series: shown where the thumbnail goes until there is one (the modality).
    pub badge: String,
    /// Series: sent back in [`Command::OpenStudySeries`].
    pub id: u64,
    /// Series: none of its images is on the medium (listed, cannot be opened).
    pub missing: bool,
}

impl std::fmt::Debug for StudyLine {
    /// Identifying texts are left out.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut d = f.debug_struct("StudyLine");
        d.field("kind", &self.kind);
        if !self.identifying {
            d.field("title", &self.title).field("detail", &self.detail);
        }
        d.field("id", &self.id)
            .field("thumb", &self.thumb.is_some())
            .finish_non_exhaustive()
    }
}

#[derive(Default)]
pub(crate) struct StudiesState {
    pub sheet: Studies,
    /// "Show patient data" is on.
    pub patient: bool,
    /// The selected row (a series row, when there is one).
    pub selected: usize,
    pub scroll: f32,
    pub last_scroll: f32,
}

impl StudiesState {
    /// The Send button is shown.
    pub(crate) fn sendable(&self) -> bool {
        self.sheet.sendable && !self.sheet.cancellable && !self.sheet.rows.is_empty()
    }

    fn series_rows(&self) -> impl Iterator<Item = usize> + '_ {
        self.sheet
            .rows
            .iter()
            .enumerate()
            .filter(|(_, r)| r.kind == StudyKind::Series)
            .map(|(i, _)| i)
    }

    /// Moves the selection `d` series up or down.
    pub fn move_sel(&mut self, d: i32) {
        let series: Vec<usize> = self.series_rows().collect();
        if series.is_empty() {
            return;
        }
        let at = series
            .iter()
            .position(|&i| i >= self.selected)
            .unwrap_or(series.len() - 1) as i64;
        let to = (at + d as i64).clamp(0, series.len() as i64 - 1) as usize;
        self.selected = series[to];
        self.scroll = f32::NAN;
    }

    /// The selection on a series row: the first at or after it, else the last.
    fn settle(&mut self) {
        if self
            .sheet
            .rows
            .get(self.selected)
            .is_some_and(|r| r.kind == StudyKind::Series)
        {
            return;
        }
        let series: Vec<usize> = self.series_rows().collect();
        self.selected = series
            .iter()
            .copied()
            .find(|&i| i >= self.selected)
            .or(series.last().copied())
            .unwrap_or(0);
    }

    /// The text shown for `row`'s title, and whether it is the stand-in for a hidden one.
    pub fn title<'a>(&self, row: &'a StudyLine) -> (&'a str, bool) {
        if row.identifying && !self.patient {
            (&row.hidden_title, true)
        } else {
            (&row.title, false)
        }
    }

    /// The text shown for `row`'s detail (`None`: hidden).
    pub fn detail<'a>(&self, row: &'a StudyLine) -> Option<&'a str> {
        (!row.identifying || self.patient).then_some(row.detail.as_str())
    }
}

/// The screen-reader name of a row: what it shows.
pub(crate) fn row_label(s: &StudiesState, row: &StudyLine) -> String {
    let (title, _) = s.title(row);
    let detail = match s.detail(row) {
        Some(d) => d.to_owned(),
        None => t!("studies-patient-hidden"),
    };
    if detail.is_empty() {
        title.to_owned()
    } else if title.is_empty() {
        detail
    } else {
        format!("{title}, {detail}")
    }
}

impl Chrome {
    /// Opens the studies browser with this content (patient data hidden).
    pub fn show_studies(&mut self, studies: Studies) {
        self.studies = StudiesState {
            sheet: studies,
            ..StudiesState::default()
        };
        self.studies.settle();
        self.open_modal(Modal::Studies);
    }

    /// Replaces the open browser's content (more of the medium read, a thumbnail made); the
    /// selection stays on its series and the patient data switch stays. Ignored when it is
    /// closed.
    pub fn set_studies(&mut self, studies: Studies) {
        if !self.studies_open() {
            return;
        }
        let s = &mut self.studies;
        let id = s
            .sheet
            .rows
            .get(s.selected)
            .filter(|r| r.kind == StudyKind::Series)
            .map(|r| r.id);
        s.sheet = studies;
        if let Some(i) = id.and_then(|id| {
            s.sheet
                .rows
                .iter()
                .position(|r| r.kind == StudyKind::Series && r.id == id)
        }) {
            s.selected = i;
        }
        s.settle();
    }

    /// Whether the studies browser is open.
    pub fn studies_open(&self) -> bool {
        self.modal == Some(Modal::Studies)
    }

    /// The series selected in the open browser ([`StudyLine::id`]).
    pub fn studies_selected(&self) -> Option<u64> {
        let s = &self.studies;
        (self.studies_open())
            .then(|| s.sheet.rows.get(s.selected))
            .flatten()
            .filter(|r| r.kind == StudyKind::Series)
            .map(|r| r.id)
    }

    /// The series row under logical point `p` in the open browser ([`StudyLine::id`]), for
    /// dragging it out onto a viewport (linked views); missing series are left out.
    pub fn studies_series_at(&self, p: (f32, f32)) -> Option<u64> {
        if !self.studies_open() {
            return None;
        }
        match self.hit_at(p)? {
            Target::StudyRow(i) => self
                .studies
                .sheet
                .rows
                .get(i)
                .filter(|r| r.kind == StudyKind::Series && !r.missing)
                .map(|r| r.id),
            _ => None,
        }
    }

    /// Opens the selected series (Enter, a click on it).
    fn open_study_series(&mut self) -> Response {
        let s = &self.studies;
        match s.sheet.rows.get(s.selected) {
            Some(r) if r.kind == StudyKind::Series && !r.missing => {
                Response::cmd(Command::OpenStudySeries(r.id))
            }
            _ => Response::used(),
        }
    }

    /// A click on a row: a series is selected and opened; a patient or a study selects its
    /// first series.
    pub(crate) fn study_row_click(&mut self, i: usize) -> Response {
        let s = &mut self.studies;
        let Some(row) = s.sheet.rows.get(i) else {
            return Response::used();
        };
        s.selected = i;
        if row.kind == StudyKind::Series {
            return self.open_study_series();
        }
        s.settle();
        s.scroll = f32::NAN;
        Response::used()
    }

    pub(crate) fn toggle_studies_patient(&mut self) -> Response {
        self.studies.patient = !self.studies.patient;
        Response::used()
    }

    /// The keyboard in the studies browser: Up, Down, Page Up/Down, Home and End move from
    /// series to series, Enter opens the selected one; Tab goes round the list, the patient
    /// data switch and Close.
    pub(crate) fn studies_key(
        &mut self,
        combo: &KeyCombo,
        named: Option<NamedKey>,
        repeat: bool,
        st: &ViewerState,
    ) -> Response {
        if let Some(back) = crate::focus::is_tab(combo) {
            let mut order = vec![None, Some(Target::StudiesPatient)];
            if self.studies.sheet.cancellable {
                order.push(Some(Target::StudiesCancel));
            }
            if self.studies.sendable() {
                order.push(Some(Target::StudiesSend));
            }
            order.push(Some(Target::ModalClose));
            let at = order
                .iter()
                .position(|o| *o == self.modal_focus)
                .unwrap_or(0);
            let n = order.len();
            let to = if back { (at + n - 1) % n } else { (at + 1) % n };
            self.modal_focus = order[to].clone();
            self.reveal_focus = true;
            return Response::used();
        }
        if let Some(t) = self.modal_focus.clone() {
            if matches!(named, Some(NamedKey::Enter | NamedKey::Space)) {
                return if repeat {
                    Response::used()
                } else {
                    self.activate(t, st)
                };
            }
            // Any other key goes back to the list.
            self.modal_focus = None;
        }
        let s = &mut self.studies;
        match named {
            Some(NamedKey::ArrowDown) => s.move_sel(1),
            Some(NamedKey::ArrowUp) => s.move_sel(-1),
            Some(NamedKey::PageDown) => s.move_sel(5),
            Some(NamedKey::PageUp) => s.move_sel(-5),
            Some(NamedKey::Home) => s.move_sel(i32::MIN / 2),
            Some(NamedKey::End) => s.move_sel(i32::MAX / 2),
            Some(NamedKey::Enter | NamedKey::Space) if !repeat => {
                return self.open_study_series();
            }
            _ => {}
        }
        Response::used()
    }
}

/// Row heights: patient, study, series.
fn row_h(cx: &Ctx, kind: StudyKind) -> f32 {
    match kind {
        StudyKind::Patient => cx.u(40.0),
        StudyKind::Study => cx.u(32.0),
        StudyKind::Series => cx.u(60.0).max(60.0),
    }
}

/// How far each row is indented.
fn indent(cx: &Ctx, kind: StudyKind) -> f32 {
    match kind {
        StudyKind::Patient => 0.0,
        StudyKind::Study => cx.u(14.0),
        StudyKind::Series => cx.u(28.0),
    }
}

pub(crate) fn draw(ch: &mut Chrome, cx: &mut Ctx) {
    let s = &mut ch.studies;
    let w = (cx.w - 48.0).min(cx.u(860.0));
    let h = (cx.h - 64.0).min(cx.h * 0.88).max(180.0);
    let r = Rect::new(((cx.w - w) * 0.5).round(), ((cx.h - h) * 0.5).round(), w, h);
    cx.sheet(r);
    let inner = Rect::new(r.x + 20.0, r.y, r.w - 40.0, r.h);

    // Title and Close.
    let title_s = cx.style(metrics::FONT_TITLE).weight(650.0);
    let title = if s.sheet.title.is_empty() {
        t!("studies-title")
    } else {
        t!("studies-title-medium", medium = s.sheet.title.clone())
    };
    let close = cx.m_in(Rect::new(r.right() - 46.0, r.y + 12.0, 32.0, 32.0), r);
    cx.close_button(close);
    let tr = cx.m_in(
        Rect::new(inner.x, r.y + 12.0, inner.w - 48.0, cx.u(32.0).max(32.0)),
        r,
    );
    cx.label_fit(&title, title_s, tr, HAlign::Start, cx.pal.text, true);

    // The patient data switch, and what it hides.
    let row_y = tr.bottom() + 8.0;
    let sw_h = cx.u(32.0).max(30.0);
    let sw_label_s = cx.style(metrics::FONT_LABEL);
    let sw_label = cx.layout(&t!("studies-show-patient"), sw_label_s, None);
    let sw_w = 42.0 + 10.0 + sw_label.width + 12.0;
    let sw_row = cx.m_in(Rect::new(inner.x - 6.0, row_y, sw_w, sw_h), r);
    let tgt = Target::StudiesPatient;
    let sw = cx.m_in(
        Rect::new(sw_row.x + 6.0, sw_row.center().1 - 11.0, 42.0, 22.0),
        sw_row,
    );
    let on = s.patient;
    cx.dl.rect(
        sw,
        11.0,
        if on {
            cx.pal.accent_fill
        } else {
            cx.pal.surface_pressed
        },
    );
    if cx.is_hover(&tgt) {
        cx.dl.rect(sw, 11.0, cx.pal.surface_hover);
    }
    let knob_x = if on != cx.rtl {
        sw.right() - 11.0
    } else {
        sw.x + 11.0
    };
    cx.dl
        .circle((knob_x, sw.center().1), 8.0, specular_theme::Color::WHITE);
    let lr = cx.m_in(
        Rect::new(sw_row.x + 58.0, sw_row.y, sw_label.width, sw_row.h),
        sw_row,
    );
    cx.block_in(&sw_label, lr, HAlign::Start, cx.pal.text);
    cx.ring_if(&tgt, sw_row, metrics::RADIUS_SM + 1.0);
    cx.hit(sw_row, tgt);
    let small = cx.style(metrics::FONT_SMALL);
    if !on {
        let hr = cx.m_in(
            Rect::new(
                inner.x + sw_w + 8.0,
                row_y,
                (inner.w - sw_w - 8.0).max(0.0),
                sw_h,
            ),
            r,
        );
        cx.label_fit(
            &t!("studies-patient-hidden-note"),
            small,
            hr,
            HAlign::Start,
            cx.pal.text_faint,
            false,
        );
    }
    let sep_y = row_y + sw_h + 8.0;
    let sep = cx.m_in(Rect::new(inner.x, sep_y, inner.w, 1.0), r);
    cx.dl.rect(sep, 0.0, cx.pal.separator);

    // Footer: progress, missing files.
    let foot_h = cx.u(28.0);
    let foot: Vec<String> = if s.sheet.rows.is_empty() {
        s.sheet.note.iter().cloned().collect()
    } else {
        [s.sheet.status.clone(), s.sheet.note.clone()]
            .into_iter()
            .flatten()
            .collect()
    };
    let list = Rect::new(
        inner.x - 6.0,
        sep_y + 4.0,
        inner.w + 12.0,
        (r.bottom() - 12.0 - foot_h - (sep_y + 4.0)).max(40.0),
    );
    // Cancel, at the end of the footer while the medium is read.
    let mut foot_w = inner.w;
    let button = if s.sheet.cancellable {
        Some((Target::StudiesCancel, t!("app-cancel")))
    } else if s.sendable() {
        Some((Target::StudiesSend, t!("studies-send")))
    } else {
        None
    };
    if let Some((tgt, label)) = button {
        let style = cx.style(metrics::FONT_LABEL).weight(550.0);
        let b = cx.layout(&label, style, None);
        let bh = cx.u(28.0).max(26.0);
        let bw = (b.width + 24.0).min(inner.w * 0.5);
        let by = r.bottom() - 8.0 - foot_h + (foot_h - bh) * 0.5;
        let br = cx.m_in(Rect::new(inner.right() - bw, by, bw, bh), r);
        let fill = if cx.is_hover(&tgt) {
            cx.pal.surface_pressed
        } else {
            cx.pal.surface_raised
        };
        cx.dl
            .rect_border(br, metrics::RADIUS_SM + 1.0, fill, cx.pal.separator, 1.0);
        cx.block_in(&b, br, HAlign::Center, cx.pal.text);
        if cx.is_focused(&tgt) {
            cx.button_ring(br, metrics::RADIUS_SM + 1.0);
        }
        cx.hit(br, tgt);
        foot_w = (inner.w - bw - 12.0).max(0.0);
    }
    if !foot.is_empty() {
        let fr = cx.m_in(
            Rect::new(inner.x, r.bottom() - 8.0 - foot_h, foot_w, foot_h),
            r,
        );
        cx.label_fit(
            &specular_i18n::dot_join(&foot),
            small,
            fr,
            HAlign::Start,
            cx.pal.text_faint,
            false,
        );
    }
    if s.sheet.rows.is_empty() {
        let body = cx.style(metrics::FONT_BODY);
        let e = s.sheet.status.clone().unwrap_or_else(|| t!("studies-none"));
        cx.label(&e, body, list, HAlign::Center, cx.pal.text_muted);
        return;
    }

    // The rows: their tops, then the part in view.
    let mut tops = Vec::with_capacity(s.sheet.rows.len() + 1);
    let mut y = 0.0;
    for row in &s.sheet.rows {
        tops.push(y);
        y += row_h(cx, row.kind);
    }
    let content = y;
    let max = (content - list.h).max(0.0);
    if s.scroll.is_nan() {
        let i = s.selected.min(s.sheet.rows.len() - 1);
        let mut top = tops[i];
        let bottom = top + row_h(cx, s.sheet.rows[i].kind);
        // Show the selected series' study and patient too when they fit.
        if let Some(head) = (0..i)
            .rev()
            .find(|&k| s.sheet.rows[k].kind == StudyKind::Patient)
            .filter(|&k| bottom - tops[k] <= list.h)
        {
            top = tops[head];
        } else if let Some(head) = (0..i)
            .rev()
            .find(|&k| s.sheet.rows[k].kind == StudyKind::Study)
            .filter(|&k| bottom - tops[k] <= list.h)
        {
            top = tops[head];
        }
        let cur = s.last_scroll.clamp(0.0, max);
        s.scroll = if top < cur {
            top
        } else if bottom > cur + list.h {
            bottom - list.h
        } else {
            cur
        };
    }
    s.scroll = s.scroll.clamp(0.0, max);
    s.last_scroll = s.scroll;
    cx.dl.push_clip(list);
    let label_s = cx.style(metrics::FONT_LABEL);
    let first = tops.partition_point(|&t| t <= s.scroll).saturating_sub(1);
    for (i, row) in s.sheet.rows.iter().enumerate().skip(first) {
        let top = list.y + tops[i] - s.scroll;
        if top > list.bottom() {
            break;
        }
        let rh = row_h(cx, row.kind);
        let rr = Rect::new(list.x, top, list.w, rh);
        let t = Target::StudyRow(i);
        let series = row.kind == StudyKind::Series;
        if series && i == s.selected {
            cx.dl.rect(
                rr.inset_xy(0.0, 2.0),
                metrics::RADIUS_SM + 1.0,
                cx.pal.accent_soft,
            );
        } else if cx.is_hover(&t) {
            cx.dl.rect(
                rr.inset_xy(0.0, 2.0),
                metrics::RADIUS_SM + 1.0,
                cx.pal.surface_hover,
            );
        }
        let x = list.x + 8.0 + indent(cx, row.kind);
        let (title, stand_in) = s.title(row);
        let detail = s.detail(row);
        let faint = TextStyle {
            italic: true,
            ..small
        };
        match row.kind {
            StudyKind::Patient | StudyKind::Study => {
                let patient = row.kind == StudyKind::Patient;
                let ts = label_s.weight(if patient { 650.0 } else { 560.0 });
                let tx = if patient {
                    x
                } else {
                    let ir = cx
                        .m_in(Rect::new(x, rr.y, 18.0, rh), rr)
                        .centered(16.0, 16.0);
                    draw_icon(cx.dl, Icon::Folder, ir, cx.pal.text_muted, 1.5, false);
                    x + 26.0
                };
                let tw = cx.text.measure(title, ts).min((rr.right() - tx) * 0.6);
                let trr = cx.m_in(Rect::new(tx, rr.y, tw + 2.0, rh), rr);
                let colour = if stand_in {
                    cx.pal.text_muted
                } else {
                    cx.pal.text
                };
                cx.label_fit(title, ts, trr, HAlign::Start, colour, false);
                let dx = tx + tw + 14.0;
                let dr = cx.m_in(
                    Rect::new(dx, rr.y, (rr.right() - 8.0 - dx).max(0.0), rh),
                    rr,
                );
                let (d, style, colour) = match detail {
                    Some(d) => (d.to_owned(), small, cx.pal.text_muted),
                    None => (t!("studies-patient-hidden"), faint, cx.pal.text_faint),
                };
                cx.label_fit(&d, style, dr, HAlign::Start, colour, false);
            }
            StudyKind::Series => {
                // The thumbnail, fitted into a square well; the modality until it is made.
                let side = rh - 10.0;
                let well = cx.m_in(Rect::new(x, rr.y + 5.0, side, side), rr);
                cx.dl.rect(well, 4.0, cx.pal.sidebar);
                match row.thumb {
                    Some((tex, tw, th)) if tw > 0 && th > 0 => {
                        let a = tw as f32 / th as f32;
                        let (iw, ih) = if a >= 1.0 {
                            (side, side / a)
                        } else {
                            (side * a, side)
                        };
                        let ir = well.centered(iw.round(), ih.round());
                        cx.dl.image(ir, [0.0, 0.0, 1.0, 1.0], tex, 3.0);
                    }
                    _ => {
                        let b = cx.style(metrics::FONT_SMALL).weight(650.0);
                        cx.label_fit(
                            &row.badge,
                            b,
                            well,
                            HAlign::Center,
                            cx.pal.text_faint,
                            false,
                        );
                    }
                }
                let tx = x + side + 12.0;
                let tw = (rr.right() - 8.0 - tx).max(0.0);
                let line = cx.u(22.0);
                let mid = rr.center().1;
                let colour = if row.missing {
                    cx.pal.text_faint
                } else {
                    cx.pal.text
                };
                let t1 = cx.m_in(Rect::new(tx, mid - line, tw, line), rr);
                cx.label_fit(title, label_s, t1, HAlign::Start, colour, false);
                let t2 = cx.m_in(Rect::new(tx, mid, tw, line), rr);
                cx.label_fit(
                    detail.unwrap_or_default(),
                    small,
                    t2,
                    HAlign::Start,
                    cx.pal.text_muted,
                    false,
                );
            }
        }
        cx.hit(rr.intersect(&list), t);
    }
    cx.dl.pop_clip();
    crate::scrollbar::draw(
        ch,
        cx,
        crate::scrollbar::Area::Studies,
        list,
        Rect::new(r.x, list.y, r.w, list.h),
        max,
    );
}
