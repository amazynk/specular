//! Cull mode on astronomy subframes (FITS, XISF): each frame's header facts and star metrics
//! ([`specular_decode::astro`]), read in the background (from the thumbnail cache when the
//! gallery measured them), a table of them sortable by column, a chart of one metric over the
//! frames with the median and the auto-reject threshold, and the σ auto-reject suggestion.
//!
//! Nothing is rejected or moved without the user: `Shift+R` asks before rejecting the
//! suggested frames, `Shift+X` before moving the rejected ones to a subfolder (moved, never
//! deleted; undo puts them back).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crossbeam_channel::Receiver;
use specular_core::CancelToken;
use specular_decode::astro::{FrameMetrics, Metric, Suggestion, suggest};
use specular_gfx::{Align, DrawList, Rect, Text, TextStyle};
use specular_i18n::t;
use specular_theme::metrics::{FONT_SMALL, PAD, RADIUS_MD, RADIUS_SM, TOOLBAR_MARGIN};
use specular_theme::{Color, Palette};
use specular_thumbs::{ASTRO_BLOB, DiskCache, ThumbKey};
use specular_widgets::accesskit;

/// The subfolder rejected frames are moved to, in the culled folder.
pub(crate) const REJECTS_FOLDER: &str = "rejected";
/// AccessKit ids of the panel (after the strip's).
pub(super) const A11Y_PANEL: u64 = super::A11Y_BASE + 0x1_0000;
/// k for the auto-reject suggestion: the default and its range.
pub(crate) const K_DEFAULT: f32 = 2.0;
const K_RANGE: (f32, f32) = (0.5, 5.0);
const K_STEP: f32 = 0.25;

/// File extensions of subframes.
pub(crate) fn is_subframe(p: &Path) -> bool {
    p.extension().and_then(|e| e.to_str()).is_some_and(|e| {
        ["fits", "fit", "fts", "fz", "xisf"]
            .iter()
            .any(|x| e.eq_ignore_ascii_case(x))
    })
}

/// What is known of one frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Frame {
    pub object: Option<String>,
    pub filter: Option<String>,
    /// Seconds.
    pub exposure: Option<f32>,
    pub temp: Option<f32>,
    pub metrics: Option<FrameMetrics>,
}

/// Reads one frame: the header facts and the metrics (the cache's, else measured and kept
/// there).
pub(crate) fn read_frame(
    path: &Path,
    registry: &specular_decode::Registry,
    cache: Option<&DiskCache>,
) -> Frame {
    let Ok(s) = registry.open_path(path) else {
        return Frame::default();
    };
    let meta = specular_thumbs::generate::meta_of(s.as_ref());
    let mut f = Frame {
        exposure: meta.exposure,
        ..Frame::default()
    };
    if let Some(a) = &meta.astro {
        f.object = a.object.clone();
        f.filter = a.filter.clone();
        f.temp = a.ccd_temp;
    }
    let key = ThumbKey::of_file(path).ok();
    f.metrics = key
        .as_ref()
        .zip(cache)
        .and_then(|(k, c)| c.get_blob(k, ASTRO_BLOB))
        .and_then(|b| FrameMetrics::from_bytes(&b));
    if f.metrics.is_none()
        && let Some(Ok(m)) = specular_decode::astro::measure(s.as_ref(), &CancelToken::new())
    {
        if let (Some(k), Some(c)) = (&key, cache) {
            let _ = c.put_blob(k, ASTRO_BLOB, &m.to_bytes());
        }
        f.metrics = Some(m);
    }
    // Not measurable (no image plane): no stars, rather than still to come.
    f.metrics.get_or_insert_with(FrameMetrics::default);
    f
}

/// A column of the table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Column {
    Name,
    Filter,
    Exposure,
    Hfd,
    Fwhm,
    Ecc,
    Stars,
    Background,
    Snr,
}

impl Column {
    pub const ALL: [Column; 9] = [
        Column::Name,
        Column::Filter,
        Column::Exposure,
        Column::Hfd,
        Column::Fwhm,
        Column::Ecc,
        Column::Stars,
        Column::Background,
        Column::Snr,
    ];

    pub fn label(self) -> String {
        match self {
            Column::Name => t!("cull-astro-col-name"),
            Column::Filter => t!("cull-astro-col-filter"),
            Column::Exposure => t!("cull-astro-col-exposure"),
            Column::Hfd => t!("cull-astro-col-hfd"),
            Column::Fwhm => t!("cull-astro-col-fwhm"),
            Column::Ecc => t!("cull-astro-col-ecc"),
            Column::Stars => t!("cull-astro-col-stars"),
            Column::Background => t!("cull-astro-col-bg"),
            Column::Snr => t!("cull-astro-col-snr"),
        }
    }

    /// The metric shown in it.
    pub fn metric(self) -> Option<Metric> {
        Some(match self {
            Column::Hfd => Metric::Hfd,
            Column::Fwhm => Metric::Fwhm,
            Column::Ecc => Metric::Eccentricity,
            Column::Stars => Metric::Stars,
            Column::Background => Metric::Background,
            Column::Snr => Metric::Snr,
            _ => return None,
        })
    }

    /// The widest the column grows (logical px); names longer than that are cut in the
    /// middle, so the frame number stays.
    fn max_width(self) -> f32 {
        match self {
            Column::Name => 200.0,
            Column::Filter => 64.0,
            _ => 96.0,
        }
    }

    /// The cell's text.
    fn cell(self, name: &str, f: Option<&Frame>) -> String {
        let num =
            |v: Option<f32>, d: usize| v.map_or_else(|| "–".to_owned(), |v| format!("{v:.d$}"));
        let m = f.and_then(|f| f.metrics.as_ref());
        match self {
            Column::Name => name.to_owned(),
            Column::Filter => f
                .and_then(|f| f.filter.clone())
                .unwrap_or_else(|| "–".into()),
            Column::Exposure => f
                .and_then(|f| f.exposure)
                .map_or_else(|| "–".into(), |e| format!("{e} s")),
            Column::Hfd => num(m.and_then(|m| m.hfd), 2),
            Column::Fwhm => num(m.and_then(|m| m.fwhm), 2),
            Column::Ecc => num(m.and_then(|m| m.eccentricity), 2),
            Column::Stars => m.map_or_else(|| "–".into(), |m| m.stars.to_string()),
            Column::Background => num(m.map(|m| m.background), 0),
            Column::Snr => num(m.and_then(|m| m.snr), 2),
        }
    }

    /// The value it sorts by (`None` sorts last).
    fn key(self, f: Option<&Frame>) -> Option<f32> {
        match self {
            Column::Exposure => f?.exposure,
            c => f?.metrics.and_then(|m| m.get(c.metric()?)),
        }
    }
}

/// The name of a metric (its column's).
pub(crate) fn metric_label(m: Metric) -> String {
    Column::ALL
        .into_iter()
        .find(|c| c.metric() == Some(m))
        .map_or_else(|| t!("cull-astro-col-noise"), Column::label)
}

/// The metrics the chart and auto-reject step through.
pub(crate) const METRICS: [Metric; 6] = [
    Metric::Hfd,
    Metric::Fwhm,
    Metric::Eccentricity,
    Metric::Stars,
    Metric::Background,
    Metric::Snr,
];

/// Sorts `listing` by `col` (`desc`: largest first), frames without a value last, the name
/// breaking ties; stable.
pub(crate) fn sort_listing(
    listing: &mut [PathBuf],
    frames: &HashMap<PathBuf, Frame>,
    col: Column,
    desc: bool,
) {
    let name = |p: &PathBuf| p.file_name().map(|n| n.to_string_lossy().to_lowercase());
    listing.sort_by(|a, b| {
        let o = match col {
            Column::Name => name(a).cmp(&name(b)),
            Column::Filter => {
                let f = |p: &PathBuf| frames.get(p).and_then(|f| f.filter.clone());
                match (f(a), f(b)) {
                    (Some(x), Some(y)) => x.to_lowercase().cmp(&y.to_lowercase()),
                    (x, y) => y.is_none().cmp(&x.is_none()).reverse(),
                }
            }
            c => match (c.key(frames.get(a)), c.key(frames.get(b))) {
                (Some(x), Some(y)) => x.total_cmp(&y),
                (Some(_), None) => return std::cmp::Ordering::Less,
                (None, Some(_)) => return std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            },
        };
        let o = if desc { o.reverse() } else { o };
        o.then_with(|| name(a).cmp(&name(b)))
    });
}

/// The suggestion over `order` (indices into it).
pub(crate) fn suggestion(
    order: &[PathBuf],
    frames: &HashMap<PathBuf, Frame>,
    metric: Metric,
    k: f32,
) -> Option<Suggestion> {
    let values: Vec<Option<f32>> = order
        .iter()
        .map(|p| frames.get(p)?.metrics?.get(metric))
        .collect();
    suggest(&values, metric, k)
}

/// Cull mode's subframe state.
pub(crate) struct State {
    pub frames: HashMap<PathBuf, Frame>,
    pub rx: Vec<Receiver<Vec<(PathBuf, Frame)>>>,
    /// Frames being read.
    pub pending: usize,
    pub panel: bool,
    pub sort: Option<(Column, bool)>,
    pub metric: Metric,
    pub k: f32,
    /// Every frame with the first one's stretch.
    pub same_stretch: bool,
    /// Frames asked for (read or being read).
    pub asked: std::collections::HashSet<PathBuf>,
    /// What a confirmation waits to do: the frames to reject, the items to move.
    pub ask_reject: Vec<PathBuf>,
    pub ask_move: Vec<Vec<PathBuf>>,
}

impl Default for State {
    fn default() -> State {
        State {
            frames: HashMap::new(),
            rx: Vec::new(),
            pending: 0,
            panel: true,
            sort: None,
            metric: Metric::Hfd,
            k: K_DEFAULT,
            same_stretch: true,
            asked: Default::default(),
            ask_reject: Vec::new(),
            ask_move: Vec::new(),
        }
    }
}

impl State {
    pub fn k_step(&mut self, up: bool) {
        let k = self.k + if up { K_STEP } else { -K_STEP };
        self.k = k.clamp(K_RANGE.0, K_RANGE.1);
    }

    pub fn next_metric(&mut self) {
        let i = METRICS.iter().position(|m| *m == self.metric).unwrap_or(0);
        self.metric = METRICS[(i + 1) % METRICS.len()];
    }

    /// The next sort: the next column ascending (a new sort starts on HFD).
    pub fn next_sort(&mut self) -> (Column, bool) {
        let next = match self.sort {
            Some((c, _)) => {
                let i = Column::ALL.iter().position(|x| *x == c).unwrap_or(0);
                Column::ALL[(i + 1) % Column::ALL.len()]
            }
            None => Column::Hfd,
        };
        self.sort = Some((next, false));
        (next, false)
    }
}

/// A control of the panel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum PanelHit {
    Column(Column),
    /// A frame's row (its index in the order shown).
    Row(usize),
    Metric,
    SigmaDown,
    SigmaUp,
    Reject,
    MoveRejects,
    SameStretch,
}

/// One row as drawn.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RowView {
    pub name: String,
    pub frame: Option<Frame>,
    /// 1 pick, -1 reject, 0 none.
    pub flag: i8,
}

/// What the panel shows this frame.
#[derive(Clone, Debug)]
pub(crate) struct Panel {
    pub rows: Vec<RowView>,
    pub current: Option<usize>,
    pub sort: Option<(Column, bool)>,
    pub metric: Metric,
    pub k: f32,
    pub suggestion: Option<Suggestion>,
    pub pending: usize,
    pub same_stretch: bool,
    pub pal: Palette,
}

impl Panel {
    /// "HFD above 3.62 (median + 2σ): 3 frames", or that none is beyond k.
    pub fn suggestion_text(&self) -> String {
        let Some(s) = &self.suggestion else {
            return t!("cull-astro-suggest-few");
        };
        let k = specular_i18n::number(self.k, 2);
        if s.frames.is_empty() {
            return t!("cull-astro-suggest-none", k = k);
        }
        let (metric, value, count) = (
            metric_label(s.metric),
            value_text(s.metric, s.threshold),
            s.frames.len() as i64,
        );
        if s.metric.higher_is_worse() {
            t!(
                "cull-astro-suggest-high",
                metric = metric,
                value = value,
                k = k,
                count = count
            )
        } else {
            t!(
                "cull-astro-suggest-low",
                metric = metric,
                value = value,
                k = k,
                count = count
            )
        }
    }

    /// The panel for a screen reader: what the chart shows.
    pub fn chart_label(&self) -> String {
        let n = self.rows.len() as i64;
        let median = self
            .suggestion
            .as_ref()
            .map(|s| value_text(s.metric, s.median))
            .unwrap_or_else(|| "–".into());
        t!(
            "cull-astro-chart",
            metric = metric_label(self.metric),
            count = n,
            median = median
        )
    }

    /// A row in words: name, values with their names, and its mark.
    pub fn row_label(&self, i: usize) -> String {
        let r = &self.rows[i];
        let mut parts = vec![r.name.clone()];
        for c in &Column::ALL[1..] {
            parts.push(format!(
                "{} {}",
                c.label(),
                c.cell(&r.name, r.frame.as_ref())
            ));
        }
        match r.flag {
            1 => parts.push(t!("gallery-pick")),
            -1 => parts.push(t!("gallery-reject")),
            _ => {}
        }
        if self
            .suggestion
            .as_ref()
            .is_some_and(|s| s.frames.contains(&i))
        {
            parts.push(t!("cull-astro-suggested"));
        }
        parts.join(", ")
    }
}

/// A metric's value as shown.
pub(crate) fn value_text(m: Metric, v: f32) -> String {
    match m {
        Metric::Stars | Metric::Background | Metric::Noise => specular_i18n::fixed(v, 0),
        _ => specular_i18n::fixed(v, 2),
    }
}

/// The panel's colours: light text on dark glass, as the strip (whatever the theme).
const TEXT: Color = Color::WHITE;
const ROW_H: f32 = 20.0;
const CHART_H: f32 = 120.0;

/// Draws the panel in the image `area` above the strip (`strip_top`), at the end side.
/// Returns its rect and controls.
pub(crate) fn draw_panel(
    dl: &mut DrawList,
    text: &mut Text,
    p: &Panel,
    area: (f32, f32, f32, f32),
    strip_top: f32,
    rtl: bool,
    ts: f32,
) -> (Rect, Vec<(Rect, PanelHit)>) {
    let (ax, ay, aw, _) = area;
    let m = TOOLBAR_MARGIN;
    let small = TextStyle::new(FONT_SMALL * ts).tabular();
    let bold = small.weight(600.0);
    let row_h = ROW_H * ts;
    // Each column as wide as its header (with room for the sort arrow) or widest cell,
    // up to its cap. A long list is measured on its first rows.
    let cell_pad = 12.0 * ts;
    let arrow = text.measure(" ▼", bold);
    let widths: Vec<(Column, f32)> = Column::ALL
        .into_iter()
        .map(|c| {
            let head = text.measure(&c.label(), bold) + arrow;
            let cells = p
                .rows
                .iter()
                .take(400)
                .map(|r| text.measure(&c.cell(&r.name, r.frame.as_ref()), small))
                .fold(0.0f32, f32::max);
            let w = head.max(cells).min(c.max_width() * ts);
            (c, w.ceil() + cell_pad)
        })
        .collect();
    // Narrow windows drop the background, then the filter, then the SNR column.
    let room = aw - 2.0 * m - 2.0 * PAD;
    let mut cols = widths;
    for drop in [Column::Background, Column::Filter, Column::Snr] {
        if cols.iter().map(|c| c.1).sum::<f32>() <= room {
            break;
        }
        cols.retain(|c| c.0 != drop);
    }
    let table_w: f32 = cols.iter().map(|c| c.1).sum();
    let w = table_w + 2.0 * PAD;
    let avail_h = (strip_top - ay - 2.0 * m).max(120.0 * ts);
    let title_h = 22.0 * ts;
    let foot_h = 2.0 * 26.0 * ts;
    let chart_h = CHART_H * ts;
    let fixed = title_h + row_h + chart_h + foot_h + 3.0 * PAD;
    let rows_fit = (((avail_h - fixed) / row_h).floor().max(1.0) as usize).min(p.rows.len().max(1));
    let h = fixed + rows_fit as f32 * row_h;
    let x = if rtl { ax + m } else { ax + aw - m - w };
    let panel = Rect::new(x, ay + m, w, h);
    dl.rect(panel, RADIUS_MD, Color::rgba(0.0, 0.0, 0.0, 0.86));
    let mut hits = Vec::new();
    // Cell rects in reading order (mirrored for right-to-left).
    let flip = |r: Rect| -> Rect {
        if rtl {
            Rect::new(panel.x + panel.right() - r.right(), r.y, r.w, r.h)
        } else {
            r
        }
    };
    let draw_in = |text: &mut Text,
                   dl: &mut DrawList,
                   s: &str,
                   st: TextStyle,
                   r: Rect,
                   c: Color,
                   end: bool| {
        let b = text.layout(s, st, Some(r.w - 6.0 * ts), Align::Start);
        let tx = if end != rtl {
            r.right() - 3.0 * ts - b.width
        } else {
            r.x + 3.0 * ts
        };
        text.draw(dl, &b, tx, r.y + (r.h - b.height) * 0.5, c);
    };

    // Title: count, measuring, stretch.
    let mut y = panel.y + PAD * 0.75;
    let mut title = t!("cull-astro-title", count = p.rows.len() as i64);
    if p.pending > 0 {
        title =
            specular_i18n::dot_join(&[title, t!("cull-astro-pending", count = p.pending as i64)]);
    }
    // The stretch toggle at the title's end, as wide as its label.
    let st_label = if p.same_stretch {
        t!("cull-astro-same-stretch")
    } else {
        t!("cull-astro-own-stretch")
    };
    let sw = (text.measure(&st_label, small) + 14.0 * ts).min(table_w * 0.5);
    let tr = flip(Rect::new(
        panel.x + PAD,
        y,
        table_w - sw - 6.0 * ts,
        title_h,
    ));
    draw_in(text, dl, &title, bold, tr, TEXT, false);
    let sr = flip(Rect::new(
        panel.x + PAD + table_w - sw,
        y + ts,
        sw,
        title_h - 2.0 * ts,
    ));
    let fill = if p.same_stretch { 0.24 } else { 0.12 };
    dl.rect(sr, RADIUS_SM, Color::WHITE.with_alpha(fill));
    draw_in(text, dl, &st_label, small, sr, TEXT, false);
    hits.push((sr, PanelHit::SameStretch));
    y += title_h;

    // Header.
    let mut cx = panel.x + PAD;
    for (c, cw) in &cols {
        let cw = *cw;
        let r = flip(Rect::new(cx, y, cw, row_h));
        let mut label = c.label();
        if let Some((sc, desc)) = p.sort
            && sc == *c
        {
            label.push_str(if desc { " ▼" } else { " ▲" });
        }
        draw_in(
            text,
            dl,
            &label,
            bold,
            r,
            TEXT.with_alpha(0.9),
            *c != Column::Name && *c != Column::Filter,
        );
        hits.push((r, PanelHit::Column(*c)));
        cx += cw;
    }
    y += row_h;
    dl.rect(
        Rect::new(panel.x + PAD, y - 1.0, table_w, 1.0),
        0.0,
        Color::WHITE.with_alpha(0.25),
    );

    // Rows: a window around the current one.
    let n = p.rows.len();
    let first = p
        .current
        .map_or(0, |c| c.saturating_sub(rows_fit / 2))
        .min(n.saturating_sub(rows_fit));
    let suggested = |i: usize| p.suggestion.as_ref().is_some_and(|s| s.frames.contains(&i));
    for i in first..(first + rows_fit).min(n) {
        let r = &p.rows[i];
        let rr = Rect::new(panel.x + PAD, y, table_w, row_h);
        if Some(i) == p.current {
            dl.rect(rr, RADIUS_SM, p.pal.accent.with_alpha(0.45));
        } else if r.flag == -1 {
            dl.rect(rr, RADIUS_SM, p.pal.danger.with_alpha(0.28));
        }
        if suggested(i) {
            dl.stroke_rect(rr.inset(0.5), RADIUS_SM, p.pal.danger, 1.5);
        }
        let colour = if r.flag == -1 {
            TEXT.with_alpha(0.6)
        } else {
            TEXT
        };
        let mut cx = panel.x + PAD;
        for (c, cw) in &cols {
            let cw = *cw;
            let cr = flip(Rect::new(cx, y, cw, row_h));
            let mut s = c.cell(&r.name, r.frame.as_ref());
            if *c == Column::Name {
                s = crate::app::gallery::truncate(text, &s, small, cw - cell_pad);
            }
            draw_in(
                text,
                dl,
                &s,
                small,
                cr,
                colour,
                *c != Column::Name && *c != Column::Filter,
            );
            cx += cw;
        }
        hits.push((flip(rr), PanelHit::Row(i)));
        y += row_h;
    }
    y += PAD * 0.5;

    // Chart.
    let chart = Rect::new(panel.x + PAD, y, table_w, chart_h);
    draw_chart(dl, text, p, chart, rtl, ts);
    y += chart_h + PAD * 0.5;

    // Suggestion and its controls.
    let line = Rect::new(panel.x + PAD, y, table_w, 26.0 * ts);
    draw_in(
        text,
        dl,
        &p.suggestion_text(),
        small,
        flip(line),
        TEXT.with_alpha(0.9),
        false,
    );
    y += 26.0 * ts;
    let k = specular_i18n::fixed(p.k, 2);
    let buttons: [(String, PanelHit, bool); 6] = [
        (metric_label(p.metric), PanelHit::Metric, false),
        ("−".into(), PanelHit::SigmaDown, false),
        (t!("cull-astro-k", k = k), PanelHit::SigmaUp, false),
        ("+".into(), PanelHit::SigmaUp, false),
        (
            t!(
                "cull-astro-reject",
                count = p.suggestion.as_ref().map_or(0, |s| s.frames.len()) as i64
            ),
            PanelHit::Reject,
            true,
        ),
        (t!("cull-astro-move"), PanelHit::MoveRejects, true),
    ];
    let mut bx = panel.x + PAD;
    for (k, (label, hit, danger)) in buttons.iter().enumerate() {
        let b = text.layout(label, small, None, Align::Start);
        let bw = b.width + 14.0 * ts;
        let r = flip(Rect::new(bx, y + 2.0 * ts, bw, 22.0 * ts));
        // The k value is a label between its buttons.
        let is_button = k != 2;
        if is_button {
            let fill = if *danger {
                p.pal.danger.with_alpha(0.55)
            } else {
                Color::WHITE.with_alpha(0.16)
            };
            dl.rect(r, RADIUS_SM, fill);
            hits.push((r, *hit));
        }
        text.draw(dl, &b, r.x + 7.0 * ts, r.y + (r.h - b.height) * 0.5, TEXT);
        bx += bw + 6.0 * ts;
    }
    (panel, hits)
}

/// The chart: the metric of each frame in the order shown, the median, the threshold, the
/// current frame, rejected and suggested ones.
fn draw_chart(dl: &mut DrawList, text: &mut Text, p: &Panel, r: Rect, rtl: bool, ts: f32) {
    dl.rect(r, RADIUS_SM, Color::WHITE.with_alpha(0.06));
    let small = TextStyle::new(FONT_SMALL * 0.9 * ts).tabular();
    let vals: Vec<Option<f32>> = p
        .rows
        .iter()
        .map(|row| row.frame.as_ref()?.metrics?.get(p.metric))
        .collect();
    let known: Vec<f32> = vals.iter().flatten().copied().collect();
    let title = metric_label(p.metric);
    let tb = text.layout(&title, small.weight(600.0), None, Align::Start);
    let tx = if rtl {
        r.right() - 4.0 * ts - tb.width
    } else {
        r.x + 4.0 * ts
    };
    text.draw(dl, &tb, tx, r.y + 2.0 * ts, TEXT.with_alpha(0.85));
    if known.is_empty() {
        return;
    }
    let mut lo = known.iter().copied().fold(f32::INFINITY, f32::min);
    let mut hi = known.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    if let Some(s) = &p.suggestion {
        lo = lo.min(s.threshold);
        hi = hi.max(s.threshold);
    }
    if hi - lo < 1e-6 {
        hi = lo + 1.0;
    }
    let pad = (hi - lo) * 0.08;
    let (lo, hi) = (lo - pad, hi + pad);
    // Room for the axis labels at the start and the title at the top.
    let label_w = 40.0 * ts;
    let plot = Rect::new(
        if rtl { r.x + 4.0 * ts } else { r.x + label_w },
        r.y + tb.height + 4.0 * ts,
        r.w - label_w - 4.0 * ts,
        r.h - tb.height - 8.0 * ts,
    );
    let n = vals.len().max(1);
    let px = |i: usize| {
        let f = if n == 1 {
            0.5
        } else {
            i as f32 / (n - 1) as f32
        };
        if rtl {
            plot.right() - f * plot.w
        } else {
            plot.x + f * plot.w
        }
    };
    let py = |v: f32| plot.bottom() - (v - lo) / (hi - lo) * plot.h;
    // The range's ends.
    for v in [hi - pad, lo + pad] {
        let b = text.layout(&value_text(p.metric, v), small, None, Align::Start);
        let x = if rtl {
            r.right() - 2.0 * ts - b.width
        } else {
            r.x + label_w - 4.0 * ts - b.width
        };
        text.draw(dl, &b, x, py(v) - b.height * 0.5, TEXT.with_alpha(0.6));
    }
    if let Some(s) = &p.suggestion {
        dl.line(
            (plot.x, py(s.median)),
            (plot.right(), py(s.median)),
            1.0 * ts,
            TEXT.with_alpha(0.35),
        );
        dl.line(
            (plot.x, py(s.threshold)),
            (plot.right(), py(s.threshold)),
            1.5 * ts,
            p.pal.danger,
        );
    }
    // The line through the frames, then their points.
    let mut prev: Option<(f32, f32)> = None;
    for (i, v) in vals.iter().enumerate() {
        match v {
            Some(v) => {
                let q = (px(i), py(*v));
                if let Some(a) = prev {
                    dl.line(a, q, 1.0 * ts, TEXT.with_alpha(0.45));
                }
                prev = Some(q);
            }
            None => prev = None,
        }
    }
    for (i, v) in vals.iter().enumerate() {
        let Some(v) = v else { continue };
        let q = (px(i), py(*v));
        let row = &p.rows[i];
        let suggested = p.suggestion.as_ref().is_some_and(|s| s.frames.contains(&i));
        let (rad, c) = if Some(i) == p.current {
            (4.5 * ts, p.pal.accent)
        } else if row.flag == -1 {
            (3.0 * ts, p.pal.danger)
        } else {
            (2.5 * ts, TEXT.with_alpha(0.9))
        };
        if suggested {
            dl.circle(q, rad + 2.0 * ts, p.pal.danger);
            dl.circle(q, rad + 0.5 * ts, Color::BLACK);
        }
        dl.circle(q, rad, c);
    }
}

/// The panel's AccessKit nodes: a group named by the chart's summary, its column headers
/// and buttons (clickable), and its rows in words.
pub(crate) fn a11y(
    tree: &mut accesskit::TreeUpdate,
    p: &Panel,
    panel: Rect,
    hits: &[(Rect, PanelHit)],
    scale: f64,
) {
    use accesskit::{Node, NodeId, Role};
    let bounds = |r: Rect| accesskit::Rect {
        x0: f64::from(r.x) * scale,
        y0: f64::from(r.y) * scale,
        x1: f64::from(r.x + r.w) * scale,
        y1: f64::from(r.y + r.h) * scale,
    };
    let mut group = Node::new(Role::Group);
    group.set_label(t!("cull-astro-panel"));
    group.set_description(format!("{} {}", p.chart_label(), p.suggestion_text()));
    group.set_bounds(bounds(panel));
    let mut ids = Vec::new();
    let mut nodes = Vec::new();
    for (i, (r, hit)) in hits.iter().enumerate() {
        let id = NodeId(A11Y_PANEL + 1 + i as u64);
        let (role, label) = match hit {
            PanelHit::Column(c) => {
                let mut l = t!("cull-astro-sort-by", column = c.label());
                if let Some((sc, desc)) = p.sort
                    && sc == *c
                {
                    l = format!(
                        "{l}, {}",
                        if desc {
                            t!("cull-astro-descending")
                        } else {
                            t!("cull-astro-ascending")
                        }
                    );
                }
                (Role::ColumnHeader, l)
            }
            PanelHit::Row(k) => (Role::Row, p.row_label(*k)),
            PanelHit::Metric => (Role::Button, t!("cull-key-metric")),
            PanelHit::SigmaDown => (Role::Button, t!("cull-key-sigma-down")),
            PanelHit::SigmaUp => (Role::Button, t!("cull-key-sigma-up")),
            PanelHit::Reject => (Role::Button, t!("cull-key-auto-reject")),
            PanelHit::MoveRejects => (Role::Button, t!("cull-key-move-rejects")),
            PanelHit::SameStretch => (Role::Button, t!("cull-key-same-stretch")),
        };
        let mut n = Node::new(role);
        n.set_label(label);
        n.set_bounds(bounds(*r));
        n.add_action(accesskit::Action::Click);
        if let PanelHit::Row(k) = hit
            && Some(*k) == p.current
        {
            n.set_selected(true);
        }
        if *hit == PanelHit::SameStretch {
            n.set_toggled(if p.same_stretch {
                accesskit::Toggled::True
            } else {
                accesskit::Toggled::False
            });
        }
        ids.push(id);
        nodes.push((id, n));
    }
    group.set_children(ids);
    if let Some((_, root)) = tree.nodes.iter_mut().find(|(id, _)| *id == NodeId(1)) {
        root.push_child(NodeId(A11Y_PANEL));
    }
    tree.nodes.push((NodeId(A11Y_PANEL), group));
    tree.nodes.extend(nodes);
}

/// Reads frames off the UI thread, a batch at a time.
pub(crate) fn spawn_read(
    loader: &crate::loader::Loader,
    paths: Vec<PathBuf>,
    cache: Option<Arc<DiskCache>>,
) -> Receiver<Vec<(PathBuf, Frame)>> {
    let (tx, rx) = crossbeam_channel::unbounded();
    let wake = loader.waker();
    let registry = loader.registry();
    loader.background("cull-astro", move |_| {
        for chunk in paths.chunks(4) {
            let v: Vec<(PathBuf, Frame)> = chunk
                .iter()
                .map(|p| (p.clone(), read_frame(p, &registry, cache.as_deref())))
                .collect();
            if tx.send(v).is_err() {
                return;
            }
            wake();
        }
    });
    rx
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(hfd: f32, stars: u32, filter: &str) -> Frame {
        Frame {
            filter: Some(filter.into()),
            exposure: Some(300.0),
            metrics: Some(FrameMetrics {
                stars,
                hfd: Some(hfd),
                fwhm: Some(hfd * 0.9),
                ..FrameMetrics::default()
            }),
            ..Frame::default()
        }
    }

    #[test]
    fn sorting_by_columns() {
        let p = |n: &str| PathBuf::from(format!("/s/{n}.fits"));
        let frames: HashMap<PathBuf, Frame> = [
            (p("a"), frame(3.0, 800, "L")),
            (p("b"), frame(4.5, 300, "Ha")),
            (p("c"), frame(2.5, 900, "L")),
        ]
        .into_iter()
        .collect();
        let mut l = vec![p("a"), p("b"), p("c"), p("d")];
        sort_listing(&mut l, &frames, Column::Hfd, false);
        assert_eq!(l, [p("c"), p("a"), p("b"), p("d")]);
        sort_listing(&mut l, &frames, Column::Stars, true);
        assert_eq!(l, [p("c"), p("a"), p("b"), p("d")]);
        sort_listing(&mut l, &frames, Column::Filter, false);
        assert_eq!(l, [p("b"), p("a"), p("c"), p("d")]);
        sort_listing(&mut l, &frames, Column::Name, true);
        assert_eq!(l, [p("d"), p("c"), p("b"), p("a")]);
    }

    #[test]
    fn suggestion_follows_metric_and_k() {
        let p = |i: usize| PathBuf::from(format!("/s/{i:03}.fits"));
        let mut frames = HashMap::new();
        let order: Vec<PathBuf> = (0..20).map(p).collect();
        for (i, path) in order.iter().enumerate() {
            let hfd = if i == 7 {
                5.0
            } else {
                3.0 + (i % 4) as f32 * 0.05
            };
            let stars = if i == 12 {
                100
            } else {
                800 + (i % 3) as u32 * 10
            };
            frames.insert(path.clone(), frame(hfd, stars, "L"));
        }
        let s = suggestion(&order, &frames, Metric::Hfd, 2.0).unwrap();
        assert_eq!(s.frames, vec![7]);
        let s = suggestion(&order, &frames, Metric::Stars, 2.0).unwrap();
        assert_eq!(s.frames, vec![12]);
        let mut st = State::default();
        assert_eq!(st.metric, Metric::Hfd);
        st.next_metric();
        assert_eq!(st.metric, Metric::Fwhm);
        for _ in 0..40 {
            st.k_step(true);
        }
        assert_eq!(st.k, K_RANGE.1);
        assert_eq!(st.next_sort(), (Column::Hfd, false));
        assert_eq!(st.next_sort(), (Column::Fwhm, false));
    }

    #[test]
    fn subframe_extensions() {
        for n in ["a.fits", "b.FIT", "c.fts", "d.fits.fz", "e.xisf"] {
            assert!(is_subframe(Path::new(n)), "{n}");
        }
        assert!(!is_subframe(Path::new("a.cr3")));
    }
}
