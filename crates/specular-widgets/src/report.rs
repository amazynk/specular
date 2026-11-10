//! The hardware report: what the app detected about the machine and the paths it chose, in the
//! sections of `--hw-report` (System, Graphics, Decoding, Performance insights). The app fills
//! a [`HwReport`] ([`Chrome::show_report`]); each section is a heading over a card of aligned
//! label/value rows (✓/✗ marks and chips for capabilities), the Decoding section adds the path
//! table with the chosen paths highlighted, and Copy report copies [`HwReport::text`].

use std::sync::Arc;

use specular_gfx::{Icon, Rect, TextBlock, draw_icon};
use specular_i18n::t;
use specular_theme::{Color, metrics};

use crate::Chrome;
use crate::ctx::{Ctx, HAlign, Target};

/// How a value reads: plain, emphasised, or coloured by what it means.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tone {
    #[default]
    Plain,
    /// A key fact (the processor, the graphics adapter).
    Strong,
    /// Good news: available, calibrated, the normal state.
    Good,
    /// Worth knowing: not measured yet, on battery, not checked.
    Warn,
    /// Something is wrong or missing.
    Bad,
    /// Secondary.
    Muted,
}

/// One label/value row of a section.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ReportRow {
    pub label: String,
    /// May be empty when the row only has chips.
    pub value: String,
    pub tone: Tone,
    /// A ✓ (true) or ✗ (false) before the value: a capability the machine has or lacks.
    pub mark: Option<bool>,
    /// Capabilities as chips after the value: name, present.
    pub chips: Vec<(String, bool)>,
    /// A second, muted line under the value.
    pub detail: String,
    /// A detail of the row above it: its label is indented.
    pub sub: bool,
}

impl ReportRow {
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> ReportRow {
        ReportRow {
            label: label.into(),
            value: value.into(),
            ..ReportRow::default()
        }
    }

    pub fn tone(mut self, tone: Tone) -> ReportRow {
        self.tone = tone;
        self
    }

    pub fn mark(mut self, present: bool) -> ReportRow {
        self.mark = Some(present);
        self
    }

    pub fn chips(mut self, chips: Vec<(String, bool)>) -> ReportRow {
        self.chips = chips;
        self
    }

    pub fn detail(mut self, detail: impl Into<String>) -> ReportRow {
        self.detail = detail.into();
        self
    }

    pub fn sub(mut self) -> ReportRow {
        self.sub = true;
        self
    }

    /// The value line as text: mark, value, chips.
    fn value_text(&self) -> String {
        let mut v = String::new();
        if let Some(m) = self.mark {
            v.push_str(if m { "✓ " } else { "✗ " });
        }
        v.push_str(&self.value);
        for (c, present) in &self.chips {
            if !v.is_empty() {
                v.push_str("  ");
            }
            v.push_str(if *present { "✓ " } else { "✗ " });
            v.push_str(c);
        }
        v
    }

    /// The value as a screen reader says it: the chips in words, then the detail.
    pub(crate) fn value_text_spoken(&self) -> String {
        let mut parts = Vec::new();
        if !self.value.is_empty() {
            parts.push(self.value.clone());
        }
        let list = |present: bool| {
            self.chips
                .iter()
                .filter(|c| c.1 == present)
                .map(|c| c.0.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        };
        let (yes, no) = (list(true), list(false));
        if !yes.is_empty() {
            parts.push(t!("hw-report-supported", list = yes));
        }
        if !no.is_empty() {
            parts.push(t!("hw-report-unsupported", list = no));
        }
        if !self.detail.is_empty() {
            parts.push(self.detail.clone());
        }
        parts.join(", ")
    }
}

/// One path of a task in the path table.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ReportPath {
    pub engine: String,
    /// Predicted wall time ("3.1 ms").
    pub time: String,
    /// Predicted processor time.
    pub cpu: String,
    /// Measured runs, or the word for a built-in estimate ([`guess`](Self::guess)).
    pub runs: String,
    /// Not measured on this machine: the numbers are the built-in estimate.
    pub guess: bool,
    /// What the path is chosen for and why ("Viewing (fastest)"); empty when it is not.
    pub uses: Vec<String>,
    /// A state worth a word: not checked yet ([`Tone::Warn`]), turned off ([`Tone::Bad`]).
    pub state: Option<(String, Tone)>,
}

impl ReportPath {
    pub fn chosen(&self) -> bool {
        !self.uses.is_empty()
    }
}

/// A task and its paths.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ReportTask {
    pub name: String,
    pub paths: Vec<ReportPath>,
}

/// The path table: one group of rows per task.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ReportTable {
    /// Above the table ("Paths for a 24-megapixel image").
    pub caption: String,
    /// Task, Path, Time, Processor time, Runs, Used for. The first and last are not drawn as
    /// columns (tasks head their groups, uses go under the path) but name them for screen
    /// readers and the copied text.
    pub columns: [String; 6],
    pub tasks: Vec<ReportTask>,
}

/// One section: a heading, rows, then a table and paragraphs if it has them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ReportSection {
    pub title: String,
    pub rows: Vec<ReportRow>,
    pub table: Option<ReportTable>,
    /// Paragraphs of text after the rows (the performance insights).
    pub notes: Vec<String>,
}

/// The whole report.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HwReport {
    /// The sheet's title (`None`: "Hardware report").
    pub title: Option<String>,
    pub sections: Vec<ReportSection>,
}

impl HwReport {
    /// The sheet's title.
    pub fn title_text(&self) -> String {
        self.title.clone().unwrap_or_else(|| t!("hw-report-title"))
    }

    /// The report as plain text (Copy report): sections, aligned rows, the table, without
    /// bidi isolation marks.
    pub fn text(&self) -> String {
        let width = |s: &str| s.chars().count();
        let label_w = self
            .sections
            .iter()
            .flat_map(|s| &s.rows)
            .map(|r| width(&r.label) + if r.sub { 2 } else { 0 })
            .max()
            .unwrap_or(0);
        let mut out = format!("{}\n", self.title_text());
        for s in &self.sections {
            out.push('\n');
            out.push_str(&s.title);
            out.push('\n');
            for r in &s.rows {
                let indent = if r.sub { "  " } else { "" };
                let label = format!("{indent}{}", r.label);
                let pad = label_w.saturating_sub(width(&label));
                out.push_str(&format!(
                    "  {label}{}  {}\n",
                    " ".repeat(pad),
                    r.value_text()
                ));
                if !r.detail.is_empty() {
                    out.push_str(&format!("  {}  {}\n", " ".repeat(label_w), r.detail));
                }
            }
            if let Some(tb) = &s.table {
                out.push('\n');
                out.push_str(&format!("  {}\n", tb.caption));
                let rows: Vec<[String; 6]> = tb
                    .tasks
                    .iter()
                    .flat_map(|task| {
                        task.paths.iter().enumerate().map(|(i, p)| {
                            let mut note = p.uses.join(", ");
                            if let Some((s, _)) = &p.state {
                                if !note.is_empty() {
                                    note.push_str(", ");
                                }
                                note.push_str(s);
                            }
                            [
                                if i == 0 {
                                    task.name.clone()
                                } else {
                                    String::new()
                                },
                                format!("{}{}", if p.chosen() { "* " } else { "  " }, p.engine),
                                p.time.clone(),
                                p.cpu.clone(),
                                p.runs.clone(),
                                note,
                            ]
                        })
                    })
                    .collect();
                let mut w = [0usize; 6];
                for row in std::iter::once(&tb.columns).chain(&rows) {
                    for (c, cell) in row.iter().enumerate() {
                        w[c] = w[c].max(width(cell));
                    }
                }
                for row in std::iter::once(&tb.columns).chain(&rows) {
                    let mut line = String::from("  ");
                    for (c, cell) in row.iter().enumerate() {
                        let pad = " ".repeat(w[c].saturating_sub(width(cell)));
                        // Numbers right-aligned, text left.
                        if (2..5).contains(&c) {
                            line.push_str(&format!("{pad}{cell}  "));
                        } else if c < 5 {
                            line.push_str(&format!("{cell}{pad}  "));
                        } else {
                            line.push_str(cell);
                        }
                    }
                    out.push_str(line.trim_end());
                    out.push('\n');
                }
            }
            for n in &s.notes {
                out.push_str(&format!("  {n}\n"));
            }
        }
        specular_i18n::strip_isolates(&out)
    }
}

// -- layout ----------------------------------------------------------------------------------

/// Horizontal padding inside a card.
const PAD_X: f32 = 14.0;
/// Between the label and the value column.
const COL_GAP: f32 = 16.0;
/// Between chips.
const CHIP_GAP: f32 = 6.0;

struct Chip {
    b: Arc<TextBlock>,
    present: bool,
    /// Offset from the value column's start, and from the row's top.
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

enum Line {
    Row {
        label: Arc<TextBlock>,
        value: Option<Arc<TextBlock>>,
        tone: Tone,
        mark: Option<bool>,
        chips: Vec<Chip>,
        detail: Option<Arc<TextBlock>>,
        sub: bool,
    },
    /// The table's column headings: Path, Time, Processor time, Runs.
    Head([Arc<TextBlock>; 4]),
    Task(Arc<TextBlock>),
    Path {
        engine: Arc<TextBlock>,
        nums: [Arc<TextBlock>; 3],
        guess: bool,
        chosen: bool,
        uses: Option<Arc<TextBlock>>,
        state: Option<(Arc<TextBlock>, Tone)>,
    },
}

enum Part {
    Heading(Arc<TextBlock>),
    Caption(Arc<TextBlock>),
    /// A card of lines with their heights.
    Card(Vec<(Line, f32)>),
    Note(Arc<TextBlock>),
}

/// The report's measures at the current text scale.
struct Grid {
    /// Card content width (inside its padding).
    content: f32,
    label_w: f32,
    value_x: f32,
    value_w: f32,
    indent: f32,
    pad_y: f32,
    mark: f32,
    /// Table: path column start and width, numeric column widths (time, cpu, runs).
    path_x: f32,
    path_w: f32,
    num_w: [f32; 3],
}

impl Grid {
    /// Start offsets of the three numeric columns from the content's start.
    fn num_x(&self) -> [f32; 3] {
        let mut x = self.content;
        let mut out = [0.0; 3];
        for i in (0..3).rev() {
            x -= self.num_w[i];
            out[i] = x;
            x -= COL_GAP;
        }
        out
    }
}

fn tone_colour(cx: &Ctx, tone: Tone) -> Color {
    match tone {
        Tone::Plain | Tone::Strong => cx.pal.text,
        Tone::Good => cx.pal.success,
        Tone::Warn => cx.pal.warning,
        Tone::Bad => cx.pal.danger,
        Tone::Muted => cx.pal.text_muted,
    }
}

fn layout(report: &HwReport, cx: &mut Ctx, inner_w: f32) -> (Vec<Part>, Grid) {
    let head_s = cx.style(metrics::FONT_BODY).weight(650.0);
    let caption_s = cx.style(metrics::FONT_SMALL).weight(600.0);
    let label_s = cx.style(metrics::FONT_LABEL);
    let value_s = cx.style(metrics::FONT_LABEL).break_anywhere();
    let strong_s = value_s.weight(650.0);
    let small_s = cx.style(metrics::FONT_SMALL).break_anywhere();
    let chip_s = cx.style(metrics::FONT_SMALL).weight(550.0);
    let num_s = cx.style(metrics::FONT_LABEL).tabular();
    let col_s = cx.style(metrics::FONT_SMALL).weight(600.0);

    let content = inner_w - 2.0 * PAD_X;
    let indent = cx.u(14.0);
    let mark = cx.u(14.0);
    // One label column for the whole report, at most 40 % of a card.
    let cap = (content * 0.4).floor();
    let label_w = report
        .sections
        .iter()
        .flat_map(|s| &s.rows)
        .map(|r| {
            let w = cx.layout(&r.label, label_s, None).width;
            (w + if r.sub { indent } else { 0.0 }).min(cap)
        })
        .fold(0.0, f32::max)
        .ceil();
    let value_x = label_w + COL_GAP;
    let value_w = (content - value_x).max(40.0);

    // The table's columns: numbers as wide as their widest cell (headings wrap to that, up
    // to a fifth of the card each), the path name takes the rest.
    let mut num_w = [0.0f32; 3];
    for s in &report.sections {
        let Some(tb) = &s.table else { continue };
        for task in &tb.tasks {
            for p in &task.paths {
                for (i, v) in [&p.time, &p.cpu, &p.runs].into_iter().enumerate() {
                    num_w[i] = num_w[i].max(cx.layout(v, num_s, None).width);
                }
            }
        }
        for (i, c) in tb.columns[2..5].iter().enumerate() {
            let natural = cx.layout(c, col_s, None).width;
            num_w[i] = num_w[i].max(natural.min((content * 0.2).floor()));
        }
    }
    let num_w = num_w.map(f32::ceil);
    let path_x = indent;
    let path_w = (content - path_x - num_w.iter().sum::<f32>() - 3.0 * COL_GAP).max(40.0);
    let g = Grid {
        content,
        label_w,
        value_x,
        value_w,
        indent,
        pad_y: cx.u(7.0),
        mark,
        path_x,
        path_w,
        num_w,
    };

    let mut parts = Vec::new();
    for s in &report.sections {
        parts.push(Part::Heading(cx.layout(&s.title, head_s, Some(inner_w))));
        let mut lines = Vec::new();
        for r in &s.rows {
            let lw = if r.sub { label_w - indent } else { label_w };
            let label = cx.layout(&r.label, label_s, Some(lw.max(20.0)));
            let mark_w = if r.mark.is_some() { mark + 6.0 } else { 0.0 };
            let style = if r.tone == Tone::Strong {
                strong_s
            } else {
                value_s
            };
            let value = (!r.value.is_empty())
                .then(|| cx.layout(&r.value, style, Some((value_w - mark_w).max(20.0))));
            // Chips flow after the value's line, wrapping inside the value column.
            let mut chips = Vec::new();
            let top = value.as_ref().map_or(0.0, |v| v.height + 4.0);
            let (mut x, mut y, mut line_h) = (0.0f32, top, 0.0f32);
            for (c, present) in &r.chips {
                let b = cx.layout(c, chip_s, Some((value_w - 16.0 - mark).max(20.0)));
                let w = b.width + 16.0 + mark + 2.0;
                let h = b.height + 6.0;
                if x > 0.0 && x + w > value_w {
                    x = 0.0;
                    y += line_h + CHIP_GAP;
                    line_h = 0.0;
                }
                chips.push(Chip {
                    b,
                    present: *present,
                    x,
                    y,
                    w,
                    h,
                });
                x += w + CHIP_GAP;
                line_h = line_h.max(h);
            }
            let chips_h = if chips.is_empty() { top } else { y + line_h };
            let detail = (!r.detail.is_empty())
                .then(|| cx.layout(&r.detail, small_s, Some((value_w - mark_w).max(20.0))));
            let value_h = chips_h.max(value.as_ref().map_or(0.0, |v| v.height))
                + detail.as_ref().map_or(0.0, |d| d.height + 2.0);
            let h = label.height.max(value_h) + 2.0 * g.pad_y;
            lines.push((
                Line::Row {
                    label,
                    value,
                    tone: r.tone,
                    mark: r.mark,
                    chips,
                    detail,
                    sub: r.sub,
                },
                h,
            ));
        }
        if !lines.is_empty() {
            parts.push(Part::Card(lines));
        }
        if let Some(tb) = &s.table {
            parts.push(Part::Caption(cx.layout(
                &tb.caption,
                caption_s,
                Some(inner_w),
            )));
            let mut lines = Vec::new();
            let head = [
                cx.layout(&tb.columns[1], col_s, Some(path_w + path_x)),
                cx.layout(&tb.columns[2], col_s, Some(num_w[0])),
                cx.layout(&tb.columns[3], col_s, Some(num_w[1])),
                cx.layout(&tb.columns[4], col_s, Some(num_w[2])),
            ];
            let hh = head.iter().map(|b| b.height).fold(0.0, f32::max) + 2.0 * g.pad_y;
            lines.push((Line::Head(head), hh));
            for task in &tb.tasks {
                let name = cx.layout(&task.name, head_s, Some(content));
                let th = name.height + 2.0 * g.pad_y;
                lines.push((Line::Task(name), th));
                for p in &task.paths {
                    let chosen = p.chosen();
                    let engine_w = (path_w - mark - 6.0).max(20.0);
                    let engine = cx.layout(
                        &p.engine,
                        if chosen { strong_s } else { value_s },
                        Some(engine_w),
                    );
                    let nums = [
                        cx.layout(&p.time, num_s, None),
                        cx.layout(&p.cpu, num_s, None),
                        cx.layout(&p.runs, num_s, None),
                    ];
                    let uses = chosen.then(|| {
                        cx.layout(
                            &specular_i18n::dot_join(&p.uses),
                            small_s.weight(550.0),
                            Some(engine_w),
                        )
                    });
                    let state = p
                        .state
                        .as_ref()
                        .map(|(s, tone)| (cx.layout(s, small_s, Some(engine_w)), *tone));
                    let left = engine.height
                        + uses.as_ref().map_or(0.0, |u| u.height + 2.0)
                        + state.as_ref().map_or(0.0, |s| s.0.height + 2.0);
                    let right = nums.iter().map(|b| b.height).fold(0.0, f32::max);
                    let h = left.max(right) + 2.0 * g.pad_y;
                    lines.push((
                        Line::Path {
                            engine,
                            nums,
                            guess: p.guess,
                            chosen,
                            uses,
                            state,
                        },
                        h,
                    ));
                }
            }
            parts.push(Part::Card(lines));
        }
        for n in &s.notes {
            parts.push(Part::Note(cx.layout(n, value_s, Some(inner_w))));
        }
    }
    (parts, g)
}

/// Space above a part, and its height.
fn part_extent(p: &Part, first: bool) -> (f32, f32) {
    match p {
        Part::Heading(b) => (if first { 0.0 } else { 20.0 }, b.height + 8.0),
        Part::Caption(b) => (14.0, b.height + 6.0),
        Part::Card(lines) => (0.0, lines.iter().map(|(_, h)| h).sum()),
        Part::Note(b) => (6.0, b.height),
    }
}

pub(crate) fn draw(ch: &mut Chrome, cx: &mut Ctx) {
    let w = (cx.w - 48.0).min(cx.u(620.0));
    let inner_w = w - 48.0;
    let title_s = cx.style(metrics::FONT_TITLE).weight(650.0);
    let button_s = cx.style(metrics::FONT_LABEL).weight(550.0);

    let title = cx.layout(&ch.report.title_text(), title_s, Some(inner_w - 40.0));
    let report = std::mem::take(&mut ch.report);
    let (parts, g) = layout(&report, cx, inner_w);
    ch.report = report;
    let copy = cx.layout(&t!("hw-report-copy"), button_s, None);

    let content: f32 = parts
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let (gap, h) = part_extent(p, i == 0);
            gap + h
        })
        .sum();
    let bh = cx.u(34.0);
    let head = title.height + 18.0;
    let foot = 20.0 + bh;
    let h = (24.0 + head + content + foot + 20.0).min(cx.h - 32.0);
    let r = Rect::new(((cx.w - w) * 0.5).round(), ((cx.h - h) * 0.5).round(), w, h);
    cx.sheet(r);
    let close = cx.m_in(Rect::new(r.right() - 46.0, r.y + 14.0, 32.0, 32.0), r);
    cx.close_button(close);
    let tr = cx.m_in(
        Rect::new(r.x + 24.0, r.y + 24.0, inner_w - 40.0, title.height),
        r,
    );
    cx.block_top(&title, tr, HAlign::Start, cx.pal.text);

    let body = Rect::new(
        r.x + 24.0,
        r.y + 24.0 + head,
        inner_w,
        (h - 24.0 - head - foot - 20.0).max(0.0),
    );
    let max = (content - body.h).max(0.0);
    ch.report_scroll = ch.report_scroll.clamp(0.0, max);
    cx.dl.push_clip(body);
    let mut y = body.y - ch.report_scroll;
    for (i, p) in parts.iter().enumerate() {
        let (gap, ph) = part_extent(p, i == 0);
        y += gap;
        // Only what shows (a long report scrolls).
        if y < body.bottom() && y + ph > body.y {
            draw_part(cx, p, &g, body, y);
        }
        y += ph;
    }
    cx.dl.pop_clip();
    crate::scrollbar::draw(
        ch,
        cx,
        crate::scrollbar::Area::Report,
        body,
        Rect::new(r.x, body.y, r.w, body.h),
        max,
    );

    let cw = copy.width + 32.0;
    let cr = cx.m_in(
        Rect::new(r.right() - 20.0 - cw, r.bottom() - 20.0 - bh, cw, bh),
        r,
    );
    let hov = cx.is_hover(&Target::ReportCopy);
    cx.dl.rect_border(
        cr,
        metrics::RADIUS_SM + 2.0,
        if hov {
            cx.pal.surface_hover
        } else {
            cx.pal.surface_raised
        },
        cx.pal.separator,
        1.0,
    );
    cx.block_in(&copy, cr, HAlign::Center, cx.pal.text);
    if cx.is_focused(&Target::ReportCopy) {
        cx.button_ring(cr, metrics::RADIUS_SM + 2.0);
    }
    cx.hit(cr, Target::ReportCopy);
}

/// A ✓ (present) or ✗ at `r`.
fn draw_mark(cx: &mut Ctx, r: Rect, present: bool) {
    let (icon, colour) = if present {
        (Icon::Check, cx.pal.success)
    } else {
        (Icon::Close, cx.pal.text_faint)
    };
    draw_icon(cx.dl, icon, r, colour, 2.2, false);
}

fn draw_part(cx: &mut Ctx, p: &Part, g: &Grid, body: Rect, y: f32) {
    match p {
        Part::Heading(b) => {
            let r = cx.m_in(Rect::new(body.x, y, body.w, b.height), body);
            cx.block_top(b, r, HAlign::Start, cx.pal.text);
        }
        Part::Caption(b) => {
            let r = cx.m_in(Rect::new(body.x, y, body.w, b.height), body);
            cx.block_top(b, r, HAlign::Start, cx.pal.text_muted);
        }
        Part::Note(b) => {
            let r = cx.m_in(Rect::new(body.x, y, body.w, b.height), body);
            cx.block_top(b, r, HAlign::Start, cx.pal.text);
        }
        Part::Card(lines) => {
            let total: f32 = lines.iter().map(|(_, h)| h).sum();
            let card = Rect::new(body.x, y, body.w, total);
            cx.dl.rect_border(
                card,
                metrics::RADIUS_MD,
                cx.pal.surface_raised,
                cx.pal.separator,
                1.0,
            );
            let inner = Rect::new(card.x + PAD_X, y, g.content, total);
            let mut ly = y;
            for (i, (line, h)) in lines.iter().enumerate() {
                // Hairlines between rows, and above each task's group.
                let rule = match line {
                    Line::Row { .. } | Line::Task(_) => i > 0,
                    Line::Head(_) | Line::Path { .. } => false,
                };
                if rule {
                    cx.dl
                        .rect(Rect::new(inner.x, ly, inner.w, 1.0), 0.0, cx.pal.separator);
                }
                draw_line(cx, line, g, card, inner, ly, *h);
                ly += h;
            }
        }
    }
}

fn draw_line(cx: &mut Ctx, line: &Line, g: &Grid, card: Rect, inner: Rect, y: f32, h: f32) {
    let top = y + g.pad_y;
    // `x`, `w` from the content's start, mirrored in right-to-left layouts.
    let at =
        |cx: &Ctx, x: f32, y: f32, w: f32, h: f32| cx.m_in(Rect::new(inner.x + x, y, w, h), inner);
    match line {
        Line::Row {
            label,
            value,
            tone,
            mark,
            chips,
            detail,
            sub,
        } => {
            let lx = if *sub { g.indent } else { 0.0 };
            let lr = at(cx, lx, top, g.label_w - lx, label.height);
            cx.block_top(label, lr, HAlign::Start, cx.pal.text_muted);
            let mut vx = g.value_x;
            if let Some(m) = mark {
                let lh = value
                    .as_ref()
                    .map_or(label.height, |v| v.height / v.lines.max(1) as f32);
                let mr = at(cx, vx, top + (lh - g.mark) * 0.5, g.mark, g.mark);
                draw_mark(cx, mr, *m);
                vx += g.mark + 6.0;
            }
            let vw = g.value_w - (vx - g.value_x);
            if let Some(v) = value {
                let vr = at(cx, vx, top, vw, v.height);
                cx.block_top(v, vr, HAlign::Start, tone_colour(cx, *tone));
            }
            for c in chips {
                let cr = at(cx, g.value_x + c.x, top + c.y, c.w, c.h);
                let (fill, border, text) = if c.present {
                    (
                        cx.pal.success.with_alpha(0.12),
                        cx.pal.success.with_alpha(0.45),
                        cx.pal.text,
                    )
                } else {
                    (Color::TRANSPARENT, cx.pal.separator, cx.pal.text_muted)
                };
                cx.dl.rect_border(cr, c.h * 0.5, fill, border, 1.0);
                let ir = at(
                    cx,
                    g.value_x + c.x + 8.0,
                    top + c.y + (c.h - g.mark) * 0.5,
                    g.mark,
                    g.mark,
                );
                draw_mark(cx, ir, c.present);
                let tr = at(
                    cx,
                    g.value_x + c.x + 8.0 + g.mark + 2.0,
                    top + c.y + 3.0,
                    c.b.width,
                    c.b.height,
                );
                cx.block_top(&c.b, tr, HAlign::Start, text);
            }
            if let Some(d) = detail {
                let dy = top + h - 2.0 * g.pad_y - d.height;
                let dr = at(cx, vx, dy, vw, d.height);
                cx.block_top(d, dr, HAlign::Start, cx.pal.text_muted);
            }
        }
        Line::Head(cols) => {
            let r = at(cx, 0.0, top, g.path_x + g.path_w, cols[0].height);
            cx.block_top(&cols[0], r, HAlign::Start, cx.pal.text_muted);
            for (i, x) in g.num_x().into_iter().enumerate() {
                let r = at(cx, x, top, g.num_w[i], cols[i + 1].height);
                cx.block_top(&cols[i + 1], r, HAlign::End, cx.pal.text_muted);
            }
            // The rule under the headings.
            cx.dl.rect(
                Rect::new(card.x, y + h - 1.0, card.w, 1.0),
                0.0,
                cx.pal.separator,
            );
        }
        Line::Task(b) => {
            let r = at(cx, 0.0, top, g.content, b.height);
            cx.block_top(b, r, HAlign::Start, cx.pal.text);
        }
        Line::Path {
            engine,
            nums,
            guess,
            chosen,
            uses,
            state,
        } => {
            if *chosen {
                // The chosen path: an accent band across the card, a check before its name.
                let band = Rect::new(card.x + 4.0, y + 1.0, card.w - 8.0, h - 2.0);
                cx.dl.rect(band, metrics::RADIUS_SM, cx.pal.accent_soft);
                let mr = at(
                    cx,
                    g.path_x,
                    top + (engine.height / engine.lines.max(1) as f32 - g.mark) * 0.5,
                    g.mark,
                    g.mark,
                );
                draw_icon(cx.dl, Icon::Check, mr, cx.pal.accent, 2.4, false);
            }
            let ex = g.path_x + g.mark + 6.0;
            let ew = g.path_w - g.mark - 6.0;
            let er = at(cx, ex, top, ew, engine.height);
            cx.block_top(engine, er, HAlign::Start, cx.pal.text);
            let mut ly = top + engine.height + 2.0;
            if let Some(u) = uses {
                let ur = at(cx, ex, ly, ew, u.height);
                cx.block_top(u, ur, HAlign::Start, cx.pal.success);
                ly += u.height + 2.0;
            }
            if let Some((s, tone)) = state {
                let sr = at(cx, ex, ly, ew, s.height);
                cx.block_top(s, sr, HAlign::Start, tone_colour(cx, *tone));
            }
            for (i, x) in g.num_x().into_iter().enumerate() {
                let colour = if *guess {
                    cx.pal.text_muted
                } else {
                    cx.pal.text
                };
                let r = at(cx, x, top, g.num_w[i], nums[i].height);
                cx.block_top(&nums[i], r, HAlign::End, colour);
            }
        }
    }
}

#[cfg(test)]
pub(crate) fn sample() -> HwReport {
    let path = |engine: &str, time: &str, uses: &[&str]| ReportPath {
        engine: engine.into(),
        time: time.into(),
        cpu: time.into(),
        runs: "12".into(),
        guess: false,
        uses: uses.iter().map(|s| s.to_string()).collect(),
        state: None,
    };
    HwReport {
        title: None,
        sections: vec![
            ReportSection {
                title: "System".into(),
                rows: vec![
                    ReportRow::new("Processor", "Test CPU").tone(Tone::Strong),
                    ReportRow::new("Power", "Normal").tone(Tone::Good),
                ],
                ..ReportSection::default()
            },
            ReportSection {
                title: "Graphics".into(),
                rows: vec![
                    ReportRow::new("Adapter", "Test GPU").tone(Tone::Strong),
                    ReportRow::new("Texture compression", "")
                        .chips(vec![("BC".into(), true), ("ASTC".into(), false)]),
                    ReportRow::new("Hardware decoding", "VA-API on Test").detail("Mesa 25"),
                    ReportRow::new("JPEG", "Up to 16384 × 16384")
                        .mark(true)
                        .sub(),
                    ReportRow::new("AV1", "Not available").mark(false).sub(),
                ],
                ..ReportSection::default()
            },
            ReportSection {
                title: "Decoding".into(),
                rows: vec![ReportRow::new("Decode threads", "16")],
                table: Some(ReportTable {
                    caption: "Paths for a 24-megapixel image".into(),
                    columns: ["Task", "Path", "Time", "Processor time", "Runs", "Used for"]
                        .map(String::from),
                    tasks: vec![ReportTask {
                        name: "JPEG decoding".into(),
                        paths: vec![
                            path("Hardware decoder", "3.1 ms", &["Viewing (fastest)"]),
                            ReportPath {
                                guess: true,
                                runs: "Estimate".into(),
                                state: Some(("Not checked yet".into(), Tone::Warn)),
                                ..path("Processor", "9.0 ms", &["Background (cheapest)"])
                            },
                        ],
                    }],
                }),
                notes: Vec::new(),
            },
            ReportSection {
                title: "Performance insights".into(),
                notes: vec!["Nothing was slow this week.".into()],
                ..ReportSection::default()
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_has_sections_rows_and_the_table() {
        let text = sample().text();
        for s in ["System", "Graphics", "Decoding", "Performance insights"] {
            assert!(text.lines().any(|l| l == s), "{s} in\n{text}");
        }
        assert!(text.contains("  Processor            Test CPU"), "{text}");
        assert!(text.contains("✓ BC  ✗ ASTC"), "{text}");
        assert!(text.contains("    JPEG"), "{text}");
        let chosen = text
            .lines()
            .find(|l| l.contains("* Hardware decoder"))
            .expect("chosen path");
        assert!(chosen.contains("3.1 ms") && chosen.ends_with("Viewing (fastest)"));
        assert!(
            text.contains("Background (cheapest), Not checked yet"),
            "{text}"
        );
        assert!(text.contains("Nothing was slow this week."));
    }
}
