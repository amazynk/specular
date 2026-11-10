//! The attribute table of a vector layer: its features as rows, sortable by any field,
//! filtered by what is typed (any field containing it, or `field = value`, `field > 10`,
//! `field: text`), with the selection shared with the map both ways and the statistics of
//! the focused field.

use std::cmp::Ordering;
use std::collections::{BTreeSet, HashMap};

use specular_decode::geo::specular_gis::model::{Layer, Value};
use specular_gfx::{Align, DrawList, Rect, Text, TextStyle};
use specular_i18n::t;
use specular_theme::metrics::{FONT_SMALL, PAD, RADIUS_MD, RADIUS_SM, TOOLBAR_MARGIN};
use specular_theme::{Color, Palette};

/// Characters of a cell before it is cut.
const CELL_CHARS: usize = 40;
/// Distinct values counted for the statistics before "many".
const DISTINCT_MAX: usize = 100_000;

/// Where the table's keys go.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Focus {
    Search,
    Rows,
}

/// A filter typed into the search field.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Filter {
    None,
    /// Any field contains the text (case-insensitive).
    Any(String),
    /// One field compared with a value.
    Field {
        field: usize,
        op: Op,
        value: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Op {
    Contains,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

/// Reads the search text against the layer's fields.
pub(super) fn parse_filter(text: &str, fields: &[String]) -> Filter {
    let s = text.trim();
    if s.is_empty() {
        return Filter::None;
    }
    // The longest operator first ("<=" before "<").
    for (sym, op) in [
        ("!=", Op::Ne),
        ("<=", Op::Le),
        (">=", Op::Ge),
        ("=", Op::Eq),
        ("<", Op::Lt),
        (">", Op::Gt),
        (":", Op::Contains),
    ] {
        if let Some(i) = s.find(sym) {
            let name = s[..i].trim();
            if let Some(field) = fields.iter().position(|f| f.eq_ignore_ascii_case(name)) {
                return Filter::Field {
                    field,
                    op,
                    value: s[i + sym.len()..].trim().to_owned(),
                };
            }
        }
    }
    Filter::Any(s.to_lowercase())
}

fn number(v: &Value) -> Option<f64> {
    match v {
        Value::Int(i) => Some(*i as f64),
        Value::Float(f) => Some(*f),
        Value::Bool(b) => Some(f64::from(u8::from(*b))),
        Value::Text(s) => s.trim().parse().ok(),
        _ => None,
    }
}

/// Orders two values: numbers by value, then the rest as text (case-insensitive), then NaN;
/// empty last. A total order: numbers against text compared as text was not transitive
/// (9 < 10 by value, "10" < "5" < "9" as text), and a sort with such a comparator may panic.
pub(super) fn compare(a: &Value, b: &Value) -> Ordering {
    // (class, number): 0 number, 1 text, 2 NaN, 3 empty.
    let key = |v: &Value| -> (u8, f64) {
        if matches!(v, Value::Null) || v.text().trim().is_empty() {
            return (3, 0.0);
        }
        match (v, number(v)) {
            (Value::Int(_) | Value::Float(_), Some(x)) if x.is_nan() => (2, 0.0),
            (Value::Int(_) | Value::Float(_), Some(x)) => (0, x),
            _ => (1, 0.0),
        }
    };
    let (ka, kb) = (key(a), key(b));
    ka.0.cmp(&kb.0).then(ka.1.total_cmp(&kb.1)).then_with(|| {
        if ka.0 == 1 {
            a.text().to_lowercase().cmp(&b.text().to_lowercase())
        } else {
            Ordering::Equal
        }
    })
}

fn matches(v: &Value, op: Op, want: &str) -> bool {
    let text = v.text();
    // An empty value has no order: only "not equal" holds.
    if matches!(v, Value::Null) || text.trim().is_empty() {
        return op == Op::Ne || op == Op::Contains && want.is_empty();
    }
    if op == Op::Contains {
        return text.to_lowercase().contains(&want.to_lowercase());
    }
    let ord = match (number(v), want.parse::<f64>().ok()) {
        (Some(x), Some(y)) => x.partial_cmp(&y),
        _ => Some(text.to_lowercase().cmp(&want.to_lowercase())),
    };
    let Some(ord) = ord else { return false };
    match op {
        Op::Eq => ord == Ordering::Equal,
        Op::Ne => ord != Ordering::Equal,
        Op::Lt => ord == Ordering::Less,
        Op::Le => ord != Ordering::Greater,
        Op::Gt => ord == Ordering::Greater,
        Op::Ge => ord != Ordering::Less,
        Op::Contains => unreachable!(),
    }
}

/// Whether feature `i` of `layer` passes `f`.
pub(super) fn passes(layer: &Layer, i: usize, f: &Filter) -> bool {
    let Some(feat) = layer.features.get(i) else {
        return false;
    };
    match f {
        Filter::None => true,
        Filter::Any(s) => {
            feat.values
                .iter()
                .any(|v| v.text().to_lowercase().contains(s))
                || feat
                    .id
                    .as_ref()
                    .is_some_and(|id| id.to_lowercase().contains(s))
        }
        Filter::Field { field, op, value } => feat
            .values
            .get(*field)
            .is_some_and(|v| matches(v, *op, value)),
    }
}

/// The statistics of one field over the rows shown.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct FieldStats {
    /// Rows with a value.
    pub count: usize,
    pub empty: usize,
    /// Distinct values (`None`: more than counted).
    pub distinct: Option<usize>,
    /// min, max, mean, sum, standard deviation: when every value is a number.
    pub numeric: Option<[f64; 5]>,
    /// The most common value and how often (text fields).
    pub top: Option<(String, usize)>,
}

pub(super) fn stats(layer: &Layer, field: usize, rows: &[usize]) -> FieldStats {
    let mut s = FieldStats::default();
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut numeric = true;
    let (mut min, mut max, mut sum, mut sum2) = (f64::INFINITY, f64::NEG_INFINITY, 0.0, 0.0);
    for &i in rows {
        let v = layer
            .features
            .get(i)
            .and_then(|f| f.values.get(field))
            .unwrap_or(&Value::Null);
        let text = v.text();
        if matches!(v, Value::Null) || text.trim().is_empty() {
            s.empty += 1;
            continue;
        }
        s.count += 1;
        match v {
            Value::Int(_) | Value::Float(_) => {
                let x = number(v).unwrap_or(f64::NAN);
                if x.is_finite() {
                    min = min.min(x);
                    max = max.max(x);
                    sum += x;
                    sum2 += x * x;
                }
            }
            _ => numeric = false,
        }
        if seen.len() < DISTINCT_MAX || seen.contains_key(&text) {
            *seen.entry(text).or_default() += 1;
        }
    }
    s.distinct = (seen.len() < DISTINCT_MAX).then_some(seen.len());
    if numeric && s.count > 0 && min.is_finite() {
        let n = s.count as f64;
        let mean = sum / n;
        let var = (sum2 / n - mean * mean).max(0.0);
        s.numeric = Some([min, max, mean, sum, var.sqrt()]);
    } else if s.count > 0 {
        s.top = seen
            .into_iter()
            .max_by(|a, b| a.1.cmp(&b.1).then_with(|| b.0.cmp(&a.0)))
            .filter(|(_, n)| *n > 1);
    }
    s
}

/// Three significant digits for a statistic.
pub(super) fn stat_value(v: f64) -> String {
    specular_geodesy::units::format_value(v)
}

/// The statistics as one line.
pub(super) fn stats_line(name: &str, s: &FieldStats) -> String {
    let distinct = match s.distinct {
        Some(n) => n.to_string(),
        None => t!("map-table-many"),
    };
    match s.numeric {
        Some([min, max, mean, sum, sd]) => t!(
            "map-table-stats-number",
            field = name.to_owned(),
            count = s.count as i64,
            empty = s.empty as i64,
            min = stat_value(min),
            max = stat_value(max),
            mean = stat_value(mean),
            sum = stat_value(sum),
            sd = stat_value(sd)
        ),
        None => match &s.top {
            Some((v, n)) => t!(
                "map-table-stats-text-top",
                field = name.to_owned(),
                count = s.count as i64,
                empty = s.empty as i64,
                distinct = distinct,
                top = cut(v),
                times = *n as i64
            ),
            None => t!(
                "map-table-stats-text",
                field = name.to_owned(),
                count = s.count as i64,
                empty = s.empty as i64,
                distinct = distinct
            ),
        },
    }
}

fn cut(s: &str) -> String {
    let one = s.replace(['\n', '\r', '\t'], " ");
    if one.chars().count() <= CELL_CHARS {
        return one;
    }
    let mut c: String = one.chars().take(CELL_CHARS - 1).collect();
    c.push('…');
    c
}

/// The table of one layer.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Table {
    /// The map layer.
    pub layer: usize,
    pub search: String,
    /// Sort column and descending.
    pub sort: Option<(usize, bool)>,
    /// Only the selected features.
    pub selected_only: bool,
    /// Feature indices shown, in order.
    pub rows: Vec<usize>,
    /// The row with the keyboard cursor (index into `rows`), and the focused column.
    pub cursor: usize,
    pub col: usize,
    /// First row and first column shown.
    pub scroll: usize,
    pub col_scroll: usize,
    pub focus: Focus,
}

impl Table {
    pub(super) fn new(layer: usize) -> Table {
        Table {
            layer,
            search: String::new(),
            sort: None,
            selected_only: false,
            rows: Vec::new(),
            cursor: 0,
            col: 0,
            scroll: 0,
            col_scroll: 0,
            focus: Focus::Rows,
        }
    }

    /// Filters and sorts the layer's features again; `selected` are its selected features.
    pub(super) fn refresh(&mut self, layer: &Layer, selected: &BTreeSet<usize>) {
        let f = parse_filter(&self.search, layer.fields());
        let mut rows: Vec<usize> = (0..layer.features.len())
            .filter(|i| !self.selected_only || selected.contains(i))
            .filter(|&i| passes(layer, i, &f))
            .collect();
        if let Some((c, desc)) = self.sort {
            let v = |i: usize| {
                layer
                    .features
                    .get(i)
                    .and_then(|f| f.values.get(c))
                    .unwrap_or(&Value::Null)
            };
            let empty = |i: usize| matches!(v(i), Value::Null) || v(i).text().trim().is_empty();
            rows.sort_by(|&a, &b| {
                // Empty values last either way.
                let o = empty(a).cmp(&empty(b)).then_with(|| {
                    let o = compare(v(a), v(b));
                    if desc { o.reverse() } else { o }
                });
                o.then(a.cmp(&b))
            });
        }
        self.rows = rows;
        self.cursor = self.cursor.min(self.rows.len().saturating_sub(1));
        self.col = self.col.min(layer.fields().len().saturating_sub(1));
    }

    /// Sorts by column `c`: ascending, then descending, then the file's order.
    pub(super) fn sort_by(&mut self, c: usize) {
        self.sort = match self.sort {
            Some((s, false)) if s == c => Some((c, true)),
            Some((s, true)) if s == c => None,
            _ => Some((c, false)),
        };
    }

    /// Moves the cursor to the row of feature `f`, when shown, and scrolls to it.
    pub(super) fn reveal(&mut self, f: usize, visible_rows: usize) {
        if let Some(i) = self.rows.iter().position(|&r| r == f) {
            self.cursor = i;
            self.keep_cursor(visible_rows);
        }
    }

    pub(super) fn keep_cursor(&mut self, visible_rows: usize) {
        let n = visible_rows.max(1);
        if self.cursor < self.scroll {
            self.scroll = self.cursor;
        } else if self.cursor >= self.scroll + n {
            self.scroll = self.cursor + 1 - n;
        }
    }
}

/// What a press in the table hit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum TableHit {
    Header(usize),
    Row(usize),
    Search,
    SelectedOnly,
    Close,
}

/// The table as drawn.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct DrawnTable {
    pub rect: Option<Rect>,
    pub hits: Vec<(TableHit, Rect)>,
    pub rows_shown: usize,
    /// Visible columns (field indices) with their header rectangles.
    pub columns: Vec<(usize, Rect)>,
    /// Where the search field's input method window opens (while it has the keys).
    pub ime: Option<Rect>,
}

/// What the table draws.
pub(super) struct TableView<'a> {
    pub table: &'a Table,
    pub layer: &'a Layer,
    pub name: String,
    pub selected: &'a BTreeSet<usize>,
    pub focused: bool,
    /// An input method's composition in the search field (empty: none).
    pub preedit: &'a str,
}

/// The table's height for `area`'s height, logical px.
pub(super) fn height(area_h: f32, ts: f32) -> f32 {
    (area_h * 0.45).clamp(170.0 * ts, 440.0 * ts)
}

/// Draws the table along the bottom of `area`.
pub(super) fn draw(
    dl: &mut DrawList,
    text: &mut Text,
    pal: &Palette,
    v: &TableView<'_>,
    area: (f32, f32, f32, f32),
    rtl: bool,
    ts: f32,
) -> DrawnTable {
    let (ax, ay, aw, ah) = area;
    let m = TOOLBAR_MARGIN;
    let st = TextStyle::new(FONT_SMALL * ts);
    let bold = st.weight(600.0);
    let w = aw - 2.0 * m;
    let h = height(ah, ts).min(ah - 2.0 * m);
    let x = ax + m;
    let y = ay + ah - m - h;
    let r = Rect::new(x, y, w, h);
    let mx = |q: Rect| {
        if rtl {
            Rect::new(2.0 * x + w - q.x - q.w, q.y, q.w, q.h)
        } else {
            q
        }
    };
    dl.glass(r, RADIUS_MD, pal.glass, pal.glass_border, 1.0);
    let mut hits = Vec::new();
    let t = v.table;
    // Title, search field, "selected only" and close.
    let title = text.layout(
        &t!(
            "map-table-title",
            layer = v.name.clone(),
            shown = t.rows.len() as i64,
            total = v.layer.features.len() as i64,
            selected = v.selected.len() as i64
        ),
        bold,
        Some(w - 3.0 * PAD - FONT_SMALL * 2.0 * ts),
        Align::Start,
    );
    let line_h = (FONT_SMALL * 2.0 * ts).round();
    let top = y + PAD;
    let tr = mx(Rect::new(
        x + PAD,
        top + ((line_h - title.height) / 2.0).max(0.0),
        title.width,
        title.height,
    ));
    text.draw(dl, &title, tr.x, tr.y, pal.text);
    let sw = (w * 0.34).clamp(140.0 * ts, 360.0 * ts);
    let close = mx(Rect::new(x + w - PAD - line_h, top, line_h, line_h));
    let sel = text.layout(&t!("map-table-selected-only"), st, None, Align::Start);
    // The second row, laid out start to end, then mirrored: search, "selected only".
    let row2 = top + line_h.max(title.height) + 4.0 * ts;
    let sr = mx(Rect::new(x + PAD, row2, sw, line_h));
    let sel_l = Rect::new(
        x + PAD + sw + 12.0 * ts,
        row2,
        sel.width + 26.0 * ts,
        line_h,
    );
    let selr = mx(sel_l);
    let search_focus = v.focused && t.focus == Focus::Search;
    dl.rect_border(
        sr,
        RADIUS_SM,
        pal.surface,
        if search_focus {
            pal.accent
        } else {
            pal.separator
        },
        if search_focus { 1.5 * ts } else { 1.0 },
    );
    let room = sr.inset_xy(6.0 * ts, 0.0);
    let mut ime = None;
    if search_focus && !(t.search.is_empty() && v.preedit.is_empty()) {
        // What is typed and the composition, the caret after them (on the left of Hebrew).
        ime = Some(super::furniture::draw_field_text(
            dl,
            text,
            pal,
            (&t.search, v.preedit),
            st,
            room,
            (rtl, 4.0 * ts, ts),
            pal.text,
        ));
    } else {
        let (stext, sink) = if t.search.is_empty() {
            (t!("map-table-search"), pal.text_faint)
        } else {
            (t.search.clone(), pal.text)
        };
        let sb = text.layout(&stext, st, Some(room.w), Align::Start);
        let sx = if rtl { room.right() - sb.width } else { room.x };
        text.draw(dl, &sb, sx, sr.y + (sr.h - sb.height) / 2.0, sink);
        if search_focus {
            // The empty field: the caret at its start, before the placeholder.
            let cx = if rtl { room.right() } else { room.x };
            dl.line(
                (cx, sr.y + 4.0 * ts),
                (cx, sr.y + sr.h - 4.0 * ts),
                ts,
                pal.text,
            );
            ime = Some(Rect::new(cx - 0.75, sr.y + 4.0 * ts, 1.5, sr.h - 8.0 * ts));
        }
    }
    hits.push((TableHit::Search, sr));
    // "Selected only" check box.
    let bs = 12.0 * ts;
    let box_l = Rect::new(sel_l.x + 6.0 * ts, row2 + (line_h - bs) / 2.0, bs, bs);
    let bx = mx(box_l);
    if t.selected_only {
        dl.rect(bx, 3.0 * ts, pal.accent);
    } else {
        dl.rect_border(bx, 3.0 * ts, pal.surface, pal.text_muted, 1.0);
    }
    let lr = mx(Rect::new(
        box_l.x + bs + 6.0 * ts,
        row2 + (line_h - sel.height) / 2.0,
        sel.width,
        sel.height,
    ));
    text.draw(dl, &sel, lr.x, lr.y, pal.text);
    hits.push((TableHit::SelectedOnly, selr));
    dl.line(
        (close.x + close.w * 0.3, close.y + close.h * 0.3),
        (close.x + close.w * 0.7, close.y + close.h * 0.7),
        1.5 * ts,
        pal.text_muted,
    );
    dl.line(
        (close.x + close.w * 0.7, close.y + close.h * 0.3),
        (close.x + close.w * 0.3, close.y + close.h * 0.7),
        1.5 * ts,
        pal.text_muted,
    );
    hits.push((TableHit::Close, close));
    // Columns: the feature number, then the fields from `col_scroll`, as many as fit.
    let grid_top = row2 + line_h + 6.0 * ts;
    let foot_h = line_h;
    let row_h = (FONT_SMALL * 1.8 * ts).round();
    let rows_fit = (((y + h - PAD - foot_h - grid_top - row_h) / row_h).floor() as usize).max(1);
    let fields = v.layer.fields();
    let num_w = text
        .layout(
            &format!("{}", v.layer.features.len()),
            st.tabular(),
            None,
            Align::Start,
        )
        .width
        + 14.0 * ts;
    let shown_rows: Vec<usize> = t
        .rows
        .iter()
        .skip(t.scroll)
        .take(rows_fit)
        .copied()
        .collect();
    let mut columns = Vec::new();
    let mut cx = x + PAD + num_w;
    let right = x + w - PAD;
    for (c, field) in fields.iter().enumerate().skip(t.col_scroll) {
        let hb = text.layout(&cut(field), bold, None, Align::Start);
        let mut cw = hb.width + 18.0 * ts;
        for &i in shown_rows.iter().take(40) {
            if let Some(val) = v.layer.features.get(i).and_then(|f| f.values.get(c)) {
                let b = text.layout(&cut(&val.text()), st, None, Align::Start);
                cw = cw.max(b.width + 14.0 * ts);
            }
        }
        let cw = cw.clamp(48.0 * ts, 240.0 * ts);
        if cx + cw > right && !columns.is_empty() {
            break;
        }
        columns.push((c, Rect::new(cx, grid_top, cw.min(right - cx), row_h)));
        cx += cw;
    }
    dl.push_clip(Rect::new(
        x + PAD,
        grid_top,
        w - 2.0 * PAD,
        y + h - PAD - foot_h - grid_top,
    ));
    // Header.
    dl.rect(
        Rect::new(x + PAD, grid_top, w - 2.0 * PAD, row_h),
        RADIUS_SM,
        pal.surface_raised,
    );
    let num_r = mx(Rect::new(x + PAD, grid_top, num_w, row_h));
    let hash = text.layout("#", bold, None, Align::Start);
    text.draw(
        dl,
        &hash,
        num_r.x + 6.0 * ts,
        num_r.y + (row_h - hash.height) / 2.0,
        pal.text_muted,
    );
    let mut drawn_cols = Vec::new();
    for (c, cr) in &columns {
        let cr = mx(*cr);
        let mut label = cut(&fields[*c]);
        if let Some((s, desc)) = t.sort
            && s == *c
        {
            label.push_str(if desc { " ▼" } else { " ▲" });
        }
        let hb = text.layout(&label, bold, Some(cr.w - 8.0 * ts), Align::Start);
        let hx = if rtl {
            cr.x + cr.w - 6.0 * ts - hb.width
        } else {
            cr.x + 6.0 * ts
        };
        text.draw(dl, &hb, hx, cr.y + (row_h - hb.height) / 2.0, pal.text);
        if v.focused && t.focus == Focus::Rows && *c == t.col {
            dl.stroke_rect(cr.inset(1.0), RADIUS_SM, pal.accent, 1.5 * ts);
        }
        hits.push((TableHit::Header(*c), cr));
        drawn_cols.push((*c, cr));
    }
    // Rows.
    let mut ry = grid_top + row_h;
    for (k, &i) in shown_rows.iter().enumerate() {
        let rr = Rect::new(x + PAD, ry, w - 2.0 * PAD, row_h);
        let selected = v.selected.contains(&i);
        if selected {
            dl.rect(rr, RADIUS_SM, pal.accent.with_alpha(0.28));
        } else if k % 2 == 1 {
            dl.rect(rr, 0.0, pal.text.with_alpha(0.04));
        }
        if v.focused && t.focus == Focus::Rows && t.scroll + k == t.cursor {
            dl.stroke_rect(rr, RADIUS_SM, pal.accent, 1.5 * ts);
        }
        let nb = text.layout(&(i + 1).to_string(), st.tabular(), None, Align::Start);
        let nr = mx(Rect::new(x + PAD, ry, num_w, row_h));
        text.draw(
            dl,
            &nb,
            nr.x + 6.0 * ts,
            ry + (row_h - nb.height) / 2.0,
            pal.text_muted,
        );
        let feat = &v.layer.features[i];
        for (c, cr) in &columns {
            let cr = mx(Rect::new(cr.x, ry, cr.w, row_h));
            let val = feat.values.get(*c).map(Value::text).unwrap_or_default();
            let numeric = matches!(feat.values.get(*c), Some(Value::Int(_) | Value::Float(_)));
            let style = if numeric { st.tabular() } else { st };
            let b = text.layout(&cut(&val), style, Some(cr.w - 10.0 * ts), Align::Start);
            // Numbers line up at the end of the cell, text at the start.
            let end = numeric != rtl;
            let bx = if end {
                cr.x + cr.w - 6.0 * ts - b.width
            } else {
                cr.x + 6.0 * ts
            };
            text.draw(dl, &b, bx, ry + (row_h - b.height) / 2.0, pal.text);
        }
        hits.push((TableHit::Row(t.scroll + k), rr));
        ry += row_h;
    }
    if t.rows.is_empty() {
        let e = text.layout(&t!("map-table-empty"), st, None, Align::Start);
        let er = mx(Rect::new(
            x + PAD + 6.0 * ts,
            ry + 6.0 * ts,
            e.width,
            e.height,
        ));
        text.draw(dl, &e, er.x, er.y, pal.text_muted);
    }
    dl.pop_clip();
    // Footer: the focused field's statistics.
    if let Some(name) = fields.get(t.col) {
        let s = stats(v.layer, t.col, &t.rows);
        let fb = text.layout(
            &stats_line(name, &s),
            st.tabular(),
            Some(w - 2.0 * PAD),
            Align::Start,
        );
        let fr = mx(Rect::new(
            x + PAD,
            y + h - PAD - foot_h + (foot_h - fb.height) / 2.0,
            fb.width,
            fb.height,
        ));
        text.draw(dl, &fb, fr.x, fr.y, pal.text_muted);
    }
    DrawnTable {
        rect: Some(r),
        hits,
        rows_shown: shown_rows.len().max(1),
        columns: drawn_cols,
        ime,
    }
}

/// A row's text for screen readers: its number and its values by field.
pub(super) fn row_label(layer: &Layer, i: usize) -> String {
    let f = &layer.features[i];
    let mut parts = vec![format!("#{}", i + 1)];
    for (k, name) in layer.fields().iter().enumerate().take(12) {
        let v = f.values.get(k).map(Value::text).unwrap_or_default();
        if !v.trim().is_empty() {
            parts.push(format!("{name}: {}", cut(&v)));
        }
    }
    parts.join(", ")
}

/// Colour of a selected feature's highlight on the map.
pub(super) fn highlight(pal: &Palette) -> Color {
    pal.accent
}

#[cfg(test)]
mod tests {
    use super::*;
    use specular_decode::geo::specular_gis::model::Geometry;

    /// Mixed numbers, numeric text, NaN, near-equal floats and empties: antisymmetric and
    /// transitive, so sorting a column cannot panic.
    #[test]
    fn compare_is_a_total_order() {
        let vals = [
            Value::Int(9),
            Value::Int(10),
            Value::Text("5".into()),
            Value::Text("10".into()),
            Value::Text("abc".into()),
            Value::Text("ABC".into()),
            Value::Float(f64::NAN),
            Value::Float(1.0),
            Value::Float(1.0 + f64::EPSILON),
            Value::Float(-0.0),
            Value::Float(f64::INFINITY),
            Value::Bool(true),
            Value::Null,
            Value::Text("  ".into()),
        ];
        for a in &vals {
            for b in &vals {
                assert_eq!(compare(a, b), compare(b, a).reverse(), "{a:?} {b:?}");
                for c in &vals {
                    if compare(a, b).is_le() && compare(b, c).is_le() {
                        assert!(compare(a, c).is_le(), "{a:?} {b:?} {c:?}");
                    }
                }
            }
        }
        let mut many: Vec<&Value> = vals.iter().cycle().take(300).collect();
        many.sort_by(|a, b| compare(a, b));
        assert!(matches!(many.last(), Some(Value::Null | Value::Text(_))));
    }

    fn layer() -> Layer {
        let mut l = Layer::new("towns");
        for (name, pop, kind) in [
            ("Alpha", Value::Int(1200), "city"),
            ("beta", Value::Int(80), "village"),
            ("Gamma", Value::Float(450.5), "town"),
            ("delta", Value::Null, "village"),
            ("Epsilon", Value::Int(80), "village"),
        ] {
            l.push(
                None,
                Some(Geometry::Point([0.0, 0.0])),
                vec![
                    ("name".into(), Value::Text(name.into())),
                    ("pop".into(), pop),
                    ("kind".into(), Value::Text(kind.into())),
                ],
            );
        }
        l
    }

    #[test]
    fn filters_parse_and_apply() {
        let l = layer();
        let f = l.fields();
        assert_eq!(parse_filter("  ", f), Filter::None);
        assert_eq!(parse_filter("Alp", f), Filter::Any("alp".into()));
        assert_eq!(
            parse_filter("POP >= 450", f),
            Filter::Field {
                field: 1,
                op: Op::Ge,
                value: "450".into()
            }
        );
        // An unknown field is plain text.
        assert_eq!(parse_filter("x=1", f), Filter::Any("x=1".into()));
        let pass = |s: &str| -> Vec<usize> {
            let flt = parse_filter(s, f);
            (0..5).filter(|&i| passes(&l, i, &flt)).collect()
        };
        assert_eq!(pass("village"), [1, 3, 4]);
        assert_eq!(pass("pop>100"), [0, 2]);
        assert_eq!(pass("pop=80"), [1, 4]);
        assert_eq!(pass("pop!=80"), [0, 2, 3]);
        assert_eq!(pass("kind:vill"), [1, 3, 4]);
        assert_eq!(pass("name=gamma"), [2]);
        assert_eq!(pass("pop<=80"), [1, 4]);
    }

    #[test]
    fn sorting_selection_and_cursor() {
        let l = layer();
        let mut t = Table::new(0);
        let none = BTreeSet::new();
        t.refresh(&l, &none);
        assert_eq!(t.rows, [0, 1, 2, 3, 4]);
        t.sort_by(1);
        t.refresh(&l, &none);
        // Numbers by value, ties in file order, empty last.
        assert_eq!(t.rows, [1, 4, 2, 0, 3]);
        t.sort_by(1);
        t.refresh(&l, &none);
        assert_eq!(t.rows, [0, 2, 1, 4, 3]);
        t.sort_by(1);
        t.refresh(&l, &none);
        assert_eq!(t.rows, [0, 1, 2, 3, 4]);
        // Text case-insensitively.
        t.sort_by(0);
        t.refresh(&l, &none);
        assert_eq!(t.rows, [0, 1, 3, 4, 2]);
        // Only the selection.
        t.selected_only = true;
        let sel: BTreeSet<usize> = [4, 1].into();
        t.refresh(&l, &sel);
        assert_eq!(t.rows, [1, 4]);
        t.reveal(4, 10);
        assert_eq!(t.rows.get(t.cursor), Some(&4));
        t.search = "zzz".into();
        t.refresh(&l, &sel);
        assert!(t.rows.is_empty());
        assert_eq!(t.cursor, 0);
    }

    #[test]
    fn field_statistics() {
        let l = layer();
        let all = [0, 1, 2, 3, 4];
        let s = stats(&l, 1, &all);
        assert_eq!((s.count, s.empty), (4, 1));
        let [min, max, mean, sum, _] = s.numeric.unwrap();
        assert_eq!((min, max, sum), (80.0, 1200.0, 1810.5));
        assert!((mean - 452.625).abs() < 1e-9);
        assert_eq!(s.distinct, Some(3));
        let k = stats(&l, 2, &all);
        assert_eq!(k.numeric, None);
        assert_eq!(k.top, Some(("village".into(), 3)));
        assert_eq!(k.distinct, Some(3));
        let line = stats_line("pop", &s);
        assert!(line.contains("1200") && line.contains("80"), "{line}");
        assert!(row_label(&l, 0).contains("name: Alpha"));
    }
}
