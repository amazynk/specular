//! The facets panel (DESIGN.md §10): for format, camera, lens, rating, year, orientation, label
//! and a few properties, how many files have each value among those the rest of the query lets
//! through. Clicking a value (or Space on it) toggles its term in the query text.
//!
//! Counting runs on the draw thread in chunks of at most [`CHUNK`] entries and [`BUDGET_S`]
//! per frame over what the listing and the metadata scan already hold (nothing is read or
//! locked for it); a pass that finishes is published, and another starts when the query or the
//! listing changed, or (at most every [`RECOUNT_S`], so a running scan does not keep the
//! draw thread counting) when more metadata came in.

use specular_gfx::{Align, DrawList, Rect, Text, TextStyle, draw_icon};
use specular_input::NamedKey;
use specular_query::facet::{self, Facet, FacetField, Tally};
use specular_query::{Matcher, Query};

use super::draw::{Frame, text_at, truncate};
use super::*;

/// Entries counted per frame (the §10 budget is 8192 per frame).
pub(super) const CHUNK: usize = 4096;
/// Draw-thread time a frame gives counting (checked every [`SLICE`] entries).
const BUDGET_S: f64 = 0.002;
const SLICE: usize = 512;
/// The least time between passes started only because more metadata arrived.
const RECOUNT_S: f64 = 0.75;
/// Values listed for the open-ended facets (format, camera, lens); selected ones always are.
const TOP: usize = 8;
/// AccessKit ids of the panel (beside the grid's and the review screens').
pub(super) const A11Y_FACETS: u64 = A11Y_BASE + (2 << 40);

#[derive(Default)]
pub(super) struct Panel {
    pub open: bool,
    /// The width the panel takes from the grid's trailing edge (0 when closed).
    pub dock: f32,
    /// The last finished counts.
    pub facets: Vec<Facet>,
    /// Something counted changed: count again once the pass in progress is done.
    pub stale: bool,
    pass: Option<Pass>,
    /// The metadata count the last finished pass saw (more arriving means counting again).
    counted_metas: usize,
    /// When the last pass finished (`now` seconds), for [`RECOUNT_S`].
    counted_at: f64,
    /// The row of [`Panel::rows`] with the keyboard focus (the panel has it when Some).
    pub focus: Option<usize>,
    pub scroll: f32,
    /// Where the panel was drawn (pointer and wheel), and its rows (for keys and AccessKit).
    pub rect: Option<Rect>,
    pub rows: Vec<(Row, Rect)>,
    /// Bumped whenever [`Panel::facets`] is replaced (the value picker caches by it).
    pub generation: u64,
    /// What the panel's search field holds: only the values (or sections) with it in their
    /// name are listed, all of them rather than the top ones.
    pub find: String,
    /// The search field has the keyboard.
    pub find_focus: bool,
}

struct Pass {
    text: String,
    query: Query,
    matcher: Matcher,
    sets: bool,
    cursor: usize,
    tally: Tally,
    /// What there was when the pass started (a change means counting again).
    metas: usize,
    entries: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum Row {
    Header(FacetField),
    Value {
        field: FacetField,
        key: String,
        count: u32,
        active: bool,
    },
}

impl Panel {
    /// Header and value rows of the finished counts.
    fn build_rows(&self, q: &Query) -> Vec<Row> {
        let mut rows = Vec::new();
        let find = self.find.trim().to_lowercase();
        // The camera settings, tags and folders are the search field's picker's.
        for f in self
            .facets
            .iter()
            .filter(|f| FacetField::ALL.contains(&f.field))
        {
            let open_ended = matches!(
                f.field,
                FacetField::Format | FacetField::Camera | FacetField::Lens
            );
            // A section whose name matches lists all its values.
            let section = find.is_empty() || field_name(f.field).to_lowercase().contains(&find);
            let mut header = Some(Row::Header(f.field));
            // Unsearched, every section shows (an empty one too).
            if find.is_empty() {
                rows.extend(header.take());
            }
            for (k, v) in f.values.iter().enumerate() {
                let active = facet::is_active(q, f.field, &v.key);
                if find.is_empty() && open_ended && k >= TOP && !active {
                    continue;
                }
                if !section
                    && !value_name(f.field, &v.key, true)
                        .to_lowercase()
                        .contains(&find)
                {
                    continue;
                }
                rows.extend(header.take());
                rows.push(Row::Value {
                    field: f.field,
                    key: v.key.clone(),
                    count: v.count,
                    active,
                });
            }
        }
        rows
    }

    /// Whether a pass is counting.
    pub fn counting(&self) -> bool {
        self.pass.is_some() || self.stale
    }

    /// The counts shown take in all of `metas` metadata records, and no count is pending.
    pub fn current(&self, metas: usize) -> bool {
        !self.counting() && self.counted_metas == metas
    }

    pub fn close(&mut self) {
        self.open = false;
        self.focus = None;
        self.find.clear();
        self.find_focus = false;
        self.pass = None;
        self.rect = None;
        self.rows.clear();
    }

    /// The value row nearest `k` in direction `dir` (±1), from `k` itself.
    pub(super) fn value_row(&self, k: usize, dir: isize) -> Option<usize> {
        let n = self.rows.len() as isize;
        if n == 0 {
            return None;
        }
        let mut i = (k as isize).clamp(0, n - 1);
        while (0..n).contains(&i) {
            if matches!(self.rows[i as usize].0, Row::Value { .. }) {
                return Some(i as usize);
            }
            i += dir;
        }
        None
    }
}

/// A query error in words, for the filter bar.
pub(super) fn query_error(e: &specular_query::Error) -> String {
    query_error_with(e, crate::app::ai::semantic::ready())
}

/// [`query_error`]; `pack_ready`: the Local AI pack is installed and loaded.
fn query_error_with(e: &specular_query::Error, pack_ready: bool) -> String {
    use specular_query::{ErrorKind, Expected};
    let field = e.field.as_str();
    match &e.kind {
        ErrorKind::UnknownField {
            name,
            suggestion: Some(s),
        } => t!(
            "gallery-query-unknown-field-suggest",
            field = name.as_str(),
            suggestion = *s
        ),
        ErrorKind::UnknownField { name, .. } => {
            t!("gallery-query-unknown-field", field = name.as_str())
        }
        ErrorKind::MissingValue => t!("gallery-query-missing-value", field = field),
        ErrorKind::BadValue { value, expected } => match expected {
            Expected::Number => t!(
                "gallery-query-bad-number",
                field = field,
                value = value.as_str()
            ),
            Expected::Date => t!(
                "gallery-query-bad-date",
                field = field,
                value = value.as_str()
            ),
            Expected::Word(words) => t!(
                "gallery-query-bad-word",
                field = field,
                value = value.as_str(),
                words = words.join(", ")
            ),
            Expected::ExifTag => t!("gallery-query-bad-exif-tag", value = value.as_str()),
            // The first word of each code.
            Expected::TagWord(words) => {
                let mut list: Vec<&str> = Vec::new();
                let mut last = None;
                for (code, w) in words.iter() {
                    if last != Some(code) {
                        list.push(w);
                        last = Some(code);
                    }
                }
                t!(
                    "gallery-query-bad-word",
                    field = field,
                    value = value.as_str(),
                    words = list.join(", ")
                )
            }
        },
        ErrorKind::NoCompare => t!("gallery-query-no-compare", field = field),
        ErrorKind::Unterminated => t!("gallery-query-unterminated"),
        ErrorKind::Unavailable => t!("gallery-query-unavailable", field = field),
        // Only `like:` is evaluated (by the pack's Search): with the pack there, the other
        // AI terms are not "missing the pack".
        ErrorKind::NeedsAiPack(_) if pack_ready => t!("gallery-query-ai-not-yet", field = field),
        ErrorKind::NeedsAiPack(_) => t!("gallery-query-needs-ai", field = field),
    }
}

/// A facet's name (section header).
pub(super) fn field_name(f: FacetField) -> String {
    match f {
        FacetField::Format => t!("gallery-facet-format"),
        FacetField::Camera => t!("gallery-facet-camera"),
        FacetField::Lens => t!("gallery-facet-lens"),
        FacetField::Rating => t!("gallery-facet-rating"),
        FacetField::Year => t!("gallery-facet-year"),
        FacetField::Orientation => t!("gallery-facet-orientation"),
        FacetField::Label => t!("gallery-facet-label"),
        FacetField::Property | FacetField::Flags => t!("gallery-facet-properties"),
        FacetField::Bits => t!("gallery-facet-bits"),
        FacetField::Pages => t!("gallery-facet-pages"),
        FacetField::Width => t!("gallery-facet-width"),
        FacetField::Height => t!("gallery-facet-height"),
        FacetField::Megapixels => t!("gallery-facet-megapixels"),
        FacetField::Size => t!("gallery-prop-size"),
        FacetField::Taken => t!("gallery-prop-date"),
        FacetField::Dupes => t!("similar-duplicates"),
        FacetField::Similar => t!("similar-similar"),
        FacetField::Place => t!("gallery-facet-place"),
        FacetField::Edited => t!("gallery-facet-edited"),
        FacetField::Version => t!("gallery-facet-versions"),
        FacetField::Module => t!("gallery-facet-module"),
        FacetField::Style => t!("gallery-facet-style"),
        // Not in the panel (the picker names them from the metadata catalogue).
        f => f.term().0.name().to_owned(),
    }
}

/// A value as shown (`visual`: stars as ★ rather than words).
pub(super) fn value_name(f: FacetField, key: &str, visual: bool) -> String {
    match f {
        FacetField::Format => key.to_uppercase(),
        FacetField::Rating => match key.parse::<u8>().unwrap_or(0) {
            0 => t!("gallery-filter-unrated"),
            n if visual => "★".repeat(usize::from(n.min(5))),
            n => t!("gallery-stars", stars = i64::from(n)),
        },
        FacetField::Orientation => match key {
            "landscape" => t!("gallery-filter-landscape"),
            "portrait" => t!("gallery-filter-portrait"),
            _ => t!("gallery-facet-square"),
        },
        FacetField::Label => {
            let l = specular_query::parse::LABEL_WORDS
                .iter()
                .position(|w| *w == key)
                .unwrap_or(0);
            label_name(l as u8)
        }
        FacetField::Property => match key {
            "hdr" => t!("gallery-filter-hdr"),
            "alpha" => t!("gallery-facet-alpha"),
            "gps" => t!("gallery-facet-gps"),
            "raw" => t!("gallery-filter-raw"),
            "animated" => t!("gallery-filter-animated"),
            "video" => t!("gallery-filter-video"),
            "sound" => t!("gallery-filter-sound"),
            _ => t!("gallery-facet-multipage"),
        },
        FacetField::Flags => flag_name(key),
        FacetField::Bits => t!("gallery-picker-bits", bits = key.to_owned()),
        FacetField::Pages => match key.parse::<i64>() {
            Ok(n) => t!("gallery-picker-pages", pages = n),
            Err(_) => key.to_owned(),
        },
        FacetField::Width | FacetField::Height => match key.parse::<i64>() {
            Ok(n) => t!(
                "gallery-picker-pixels",
                px = I18n::global().localizer().integer(n)
            ),
            Err(_) => key.to_owned(),
        },
        FacetField::Megapixels => match key.parse::<f64>() {
            Ok(n) => t!(
                "gallery-picker-megapixels",
                mp = I18n::global().localizer().number(n, 1)
            ),
            Err(_) => key.to_owned(),
        },
        FacetField::Size => match f.ladder().and_then(|l| l.iter().find(|(k, _)| *k == key)) {
            Some((_, n)) => I18n::global()
                .localizer()
                .bytes(*n as u64, specular_i18n::ByteUnits::Decimal),
            None => key.to_owned(),
        },
        FacetField::Taken | FacetField::Modified => {
            // A month (`2024-05`) by name; a year as typed.
            let month = key
                .split_once('-')
                .and_then(|(y, m)| Some((y.parse::<i32>().ok()?, m.parse::<u8>().ok()?)));
            month
                .and_then(|(y, m)| I18n::global().localizer().year_month(y, m))
                .unwrap_or_else(|| key.to_owned())
        }
        FacetField::Space => space_name(key).map_or_else(|| key.to_owned(), str::to_owned),
        FacetField::Dupes | FacetField::Similar => match key {
            "keep" => t!("gallery-picker-group-keep"),
            "discard" => t!("gallery-picker-group-discard"),
            _ => t!("gallery-picker-group-any"),
        },
        FacetField::Edited => match key {
            "yes" => t!("gallery-facet-edited-yes"),
            _ => t!("gallery-facet-edited-no"),
        },
        FacetField::Version => match key {
            "yes" => t!("gallery-facet-versions-yes"),
            "copy" => t!("gallery-facet-versions-copy"),
            _ => t!("gallery-facet-versions-no"),
        },
        // Modules by their name in the UI language.
        FacetField::Module => specular_query::module_name(key).unwrap_or_else(|| key.to_owned()),
        _ => key.to_owned(),
    }
}

/// A colour space word's name: the spaces by their proper names, which are not translated
/// (the picker names grey, CMYK and L*a*b* as the info panel's colour models). None for a
/// profile's description, shown as it is.
pub(super) fn space_name(key: &str) -> Option<&'static str> {
    Some(match key {
        "srgb" => "sRGB",
        "p3" => "Display P3",
        "adobergb" => "Adobe RGB",
        "prophoto" => "ProPhoto RGB",
        "rec2020" => "Rec. 2020",
        "pq" => "Rec. 2100 PQ",
        "hlg" => "Rec. 2100 HLG",
        "grey" => "Greyscale",
        "cmyk" => "CMYK",
        "lab" => "L*a*b*",
        _ => return None,
    })
}

/// An `is:` / `has:` word as listed (aliases name the same flag).
fn flag_name(key: &str) -> String {
    use specular_query::Flag;
    match Flag::from_word(key) {
        Some(Flag::Hdr) => t!("gallery-filter-hdr"),
        Some(Flag::Alpha) => t!("gallery-facet-alpha"),
        Some(Flag::Gps) => t!("gallery-facet-gps"),
        Some(Flag::Raw) => t!("gallery-filter-raw"),
        Some(Flag::Animated) => t!("gallery-filter-animated"),
        Some(Flag::Video) => t!("gallery-filter-video"),
        Some(Flag::Sound) => t!("gallery-filter-sound"),
        Some(Flag::MultiPage) => t!("gallery-facet-multipage"),
        Some(Flag::Profile) => t!("gallery-facet-profile"),
        Some(Flag::Camera) => t!("gallery-facet-camera"),
        Some(Flag::Lens) => t!("gallery-facet-lens"),
        Some(Flag::Date) => t!("gallery-facet-date"),
        Some(Flag::Pick) => t!("gallery-facet-picked"),
        Some(Flag::Reject) => t!("gallery-facet-rejected"),
        Some(Flag::Flagged) => t!("gallery-facet-flagged"),
        Some(Flag::Rated) => t!("gallery-facet-rated"),
        Some(Flag::Unrated) => t!("gallery-filter-unrated"),
        Some(Flag::Labelled) => t!("gallery-facet-labelled"),
        Some(Flag::Landscape) => t!("gallery-filter-landscape"),
        Some(Flag::Portrait) => t!("gallery-filter-portrait"),
        Some(Flag::Square) => t!("gallery-facet-square"),
        Some(Flag::Folder | Flag::Image) | None => key.to_owned(),
    }
}

impl Gallery {
    /// Counts the next chunk. True while counting (draw again).
    /// Counts a slice; returns when to come back (None when there is nothing to do).
    pub(super) fn facets_step(&mut self, now: f64) -> Option<f64> {
        if !self.facets.open && !self.picker_counts() {
            return None;
        }
        if self.library.on {
            // The catalogue counts them (with each search).
            self.library_facets();
            let p = &mut self.facets;
            p.pass = None;
            p.stale = false;
            p.counted_metas = self.metas.len();
            return None;
        }
        // The value being typed in the search field is left out (it filters the picker's
        // list, not the counts), so typing it does not start a pass per key.
        let text = self.picker_count_text();
        // The groups' roles are looked up only when something asks for them.
        let sets = self.filter.uses_sets() || self.picker_sets();
        let p = &mut self.facets;
        if p.pass
            .as_ref()
            .is_some_and(|s| s.text != text || s.entries != self.entries.len() || s.sets != sets)
        {
            p.pass = None;
            p.stale = true;
        }
        if p.pass.is_none() && !p.stale && p.counted_metas != self.metas.len() {
            // Metadata came in since the last count (the scan runs while the panel is open).
            let wait = p.counted_at + RECOUNT_S - now;
            if wait > 0.0 {
                return Some(wait);
            }
            p.stale = true;
        }
        if p.pass.is_none() {
            if !p.stale {
                return None;
            }
            p.stale = false;
            let query = specular_query::parse_in(&text, self.filter.scope);
            p.pass = Some(Pass {
                text,
                matcher: Matcher::new(&query, model::fold),
                query,
                sets,
                cursor: 0,
                tally: Tally::default(),
                metas: self.metas.len(),
                entries: self.entries.len(),
            });
        }
        let s = p.pass.as_mut()?;
        let started = std::time::Instant::now();
        let limit = (s.cursor + CHUNK).min(self.entries.len());
        while s.cursor < limit {
            let end = (s.cursor + SLICE).min(limit);
            for e in &self.entries[s.cursor..end] {
                let role = if s.sets {
                    self.similar.role(&e.path)
                } else {
                    (None, None)
                };
                let mut f = super::edited::develop_facts(
                    &self.edited,
                    &self.versions,
                    e,
                    model::facts(
                        e,
                        self.metas.get(e.path.as_path()).map(|m| m.as_ref()),
                        self.marks.mark(&e.path),
                        role,
                    ),
                );
                f.tags = self.listing_tags.of(&e.path);
                s.tally.add(&f, s.matcher.fail_mask(&f));
            }
            s.cursor = end;
            if started.elapsed().as_secs_f64() > BUDGET_S {
                break;
            }
        }
        if s.cursor >= self.entries.len() {
            p.facets = s.tally.finish(&s.query);
            p.generation += 1;
            p.counted_metas = s.metas;
            p.counted_at = now;
            p.pass = None;
        }
        p.counting().then_some(0.0)
    }

    /// Selects or clears a value.
    pub(super) fn facets_toggle(&mut self, field: FacetField, key: &str) {
        self.filter.text = facet::toggle(&self.filter.text, field, key);
        self.dirty = true;
    }

    /// Opens or closes the panel; `keyboard` puts the focus on its first value.
    pub(super) fn facets_show(&mut self, open: bool, keyboard: bool) {
        if open {
            self.keywords.close();
            self.meta_edit.close();
            self.filter_open = true;
            self.relayout = true;
            if !self.facets.open {
                self.facets.open = true;
                self.facets.stale = true;
                self.facets.scroll = 0.0;
            }
            if keyboard {
                self.filter_focus = false;
                self.facets.focus = Some(0);
            }
        } else {
            self.facets.close();
        }
    }

    /// The panel's width for a text scale.
    pub(super) fn facets_width(&self, ts: f32) -> f32 {
        let w = (260.0 * (1.0 + (ts - 1.0).max(0.0) * 0.5)).round();
        w.min(self.vw * 0.45).max(self.vw.min(180.0))
    }

    /// Draws the panel (docked at the grid's trailing edge, under the bars' popovers).
    pub(super) fn draw_facets(&mut self, dl: &mut DrawList, text: &mut Text, f: &Frame) {
        if !(self.filter_open && self.facets.open) {
            self.facets.rect = None;
            self.facets.rows.clear();
            return;
        }
        let pal = f.pal;
        let ts = f.text_scale.clamp(0.75, 3.0);
        let u = |v: f32| (v * ts).round();
        let title_style = TextStyle::new(14.0 * ts).weight(600.0);
        let head_style = TextStyle::new(11.0 * ts).weight(600.0);
        let row_style = TextStyle::new(13.0 * ts);
        let count_style = TextStyle::new(12.0 * ts).tabular();
        let pw = self.facets_width(ts);
        let top = TOP_BAR + self.filter_bar_h();
        let h = (self.vh - BOTTOM_BAR - top).max(0.0);
        // Laid out left to right with the panel at the right edge, then mirrored for RTL.
        let panel = Rect::new(self.vw - pw, top, pw, h);
        let r = self.flip(panel);
        dl.shadow(r, 0.0, pal.shadow, 10.0, 0.0);
        dl.rect(r, 0.0, pal.surface_raised);
        let edge = self.flip(Rect::new(panel.x, top, 1.0, h));
        dl.rect(edge, 0.0, pal.separator);
        self.hits.push((r, Hit::FacetsPanel));
        self.facets.rect = Some(r);

        let pad = u(12.0);
        let title_h = u(40.0);
        let tr = self.flip(Rect::new(panel.x + pad, top, pw - 2.0 * pad, title_h));
        let title = truncate(text, &t!("gallery-facets"), title_style, tr.w * 0.6);
        text_at(text, dl, &title, title_style, tr, pal.text, self.rtl);
        if self.facets.counting() {
            let s = truncate(
                text,
                &t!("gallery-facets-counting"),
                count_style,
                tr.w * 0.4,
            );
            text_at(text, dl, &s, count_style, tr, pal.text_muted, !self.rtl);
        }
        // The search field.
        let fh = u(28.0);
        let field = self.flip(Rect::new(panel.x + pad, top + title_h, pw - 2.0 * pad, fh));
        let focused = self.facets_finding();
        dl.rect_border(
            field,
            6.0,
            pal.canvas,
            if focused {
                pal.focus_ring
            } else {
                pal.separator
            },
            if focused { 1.5 } else { 1.0 },
        );
        let ic = u(14.0);
        let icy = field.y + ((fh - ic) * 0.5).round();
        let ir = if self.rtl {
            Rect::new(field.right() - ic - u(8.0), icy, ic, ic)
        } else {
            Rect::new(field.x + u(8.0), icy, ic, ic)
        };
        draw_icon(dl, Icon::Search, ir, pal.text_faint, 1.5, self.rtl);
        let lead = ic + u(14.0);
        let tw = (field.w - lead - u(8.0)).max(0.0);
        let tr = if self.rtl {
            Rect::new(field.x + u(8.0), field.y, tw, fh)
        } else {
            Rect::new(field.x + lead, field.y, tw, fh)
        };
        let drawn = if self.facets.find.is_empty() {
            let ph = truncate(text, &t!("gallery-facets-search"), row_style, tw);
            text_at(text, dl, &ph, row_style, tr, pal.text_faint, self.rtl);
            0.0
        } else {
            let s = truncate(text, &self.facets.find, row_style, tw);
            text_at(text, dl, &s, row_style, tr, pal.text, self.rtl)
        };
        if focused {
            let ch = (row_style.size * 1.25).round();
            let cx = if self.rtl {
                tr.right() - drawn - 1.5
            } else {
                tr.x + drawn
            };
            dl.rect(
                Rect::new(cx, field.y + ((fh - ch) * 0.5).round(), 1.5, ch),
                0.0,
                pal.text,
            );
        }
        self.hits.push((field, Hit::FacetsFind));
        let body_top = top + title_h + fh + u(6.0);
        let body = Rect::new(panel.x, body_top, pw, (h - (body_top - top)).max(0.0));

        let q = self.filter.query();
        let rows = self.facets.build_rows(&q);
        let row_h = u(28.0);
        let head_h = u(30.0);
        let content: f32 = rows
            .iter()
            .map(|r| match r {
                Row::Header(_) => head_h,
                Row::Value { .. } => row_h,
            })
            .sum::<f32>()
            + pad;
        let max_scroll = (content - body.h).max(0.0);
        // Keep the focused row in view.
        if let Some(k) = self.facets.focus.and_then(|k| {
            let n = rows.len();
            (n > 0).then(|| k.min(n - 1))
        }) {
            let y: f32 = rows[..k]
                .iter()
                .map(|r| match r {
                    Row::Header(_) => head_h,
                    Row::Value { .. } => row_h,
                })
                .sum();
            // A section's first value brings its header along.
            let y0 = if k > 0 && matches!(rows[k - 1], Row::Header(_)) {
                y - head_h
            } else {
                y
            };
            if y0 < self.facets.scroll {
                self.facets.scroll = y0;
            } else if y + row_h > self.facets.scroll + body.h {
                self.facets.scroll = y + row_h - body.h;
            }
        }
        self.facets.scroll = self.facets.scroll.clamp(0.0, max_scroll);
        let loc = I18n::global().localizer();
        let mut placed = Vec::with_capacity(rows.len());
        dl.push_clip(self.flip(body));
        let mut y = body.y - self.facets.scroll;
        if rows.is_empty() && !self.facets.counting() {
            let er = self.flip(Rect::new(body.x + pad, body.y, body.w - 2.0 * pad, row_h));
            let empty = if self.facets.find.trim().is_empty() {
                t!("gallery-facets-empty")
            } else {
                t!("gallery-facets-no-match")
            };
            let s = truncate(text, &empty, row_style, er.w);
            text_at(text, dl, &s, row_style, er, pal.text_muted, self.rtl);
        }
        for (k, row) in rows.into_iter().enumerate() {
            let rh = match row {
                Row::Header(_) => head_h,
                Row::Value { .. } => row_h,
            };
            let lr = Rect::new(body.x + u(6.0), y, body.w - u(12.0), rh);
            let rr = self.flip(lr);
            let visible = y + rh > body.y && y < body.bottom();
            if visible {
                match &row {
                    Row::Header(field) => {
                        let hr = self.flip(Rect::new(
                            body.x + pad,
                            y + u(8.0),
                            body.w - 2.0 * pad,
                            rh - u(8.0),
                        ));
                        let s = truncate(text, &field_name(*field), head_style, hr.w);
                        text_at(text, dl, &s, head_style, hr, pal.text_muted, self.rtl);
                    }
                    Row::Value {
                        field,
                        key,
                        count,
                        active,
                    } => {
                        let hovered = self.pointer.is_some_and(|p| rr.contains(p));
                        let focused = self.facets.focus == Some(k);
                        if hovered {
                            dl.rect(rr, u(6.0), pal.surface_hover);
                        }
                        if focused {
                            dl.stroke_rect(rr, u(6.0), pal.focus_ring, 1.5);
                        }
                        // Check box, label, count.
                        let bs = u(14.0);
                        let br = self.flip(Rect::new(lr.x + u(8.0), y + (rh - bs) * 0.5, bs, bs));
                        if *active {
                            dl.rect(br, u(3.0), pal.accent_fill);
                            draw_icon(dl, Icon::Check, br.inset(1.0), pal.accent_text, 2.0, false);
                        } else {
                            dl.stroke_rect(br, u(3.0), pal.text_muted, 1.2);
                        }
                        let n = loc.integer(i64::from(*count));
                        let cw = text.measure(&n, count_style);
                        let text_x = lr.x + u(8.0) + bs + u(8.0);
                        let cr = self.flip(Rect::new(lr.right() - u(8.0) - cw, y, cw, rh));
                        let label_w = (lr.right() - u(16.0) - cw - text_x).max(0.0);
                        let tr = self.flip(Rect::new(text_x, y, label_w, rh));
                        let name = value_name(*field, key, true);
                        let s = truncate(text, &name, row_style, label_w);
                        let c = if *count == 0 {
                            pal.text_muted
                        } else {
                            pal.text
                        };
                        text_at(text, dl, &s, row_style, tr, c, self.rtl);
                        let b = text.layout(&n, count_style, None, Align::Start);
                        text.draw(dl, &b, cr.x, y + (rh - b.height) * 0.5, pal.text_muted);
                        self.hits
                            .push((rr.intersect(&self.flip(body)), Hit::Facet(k)));
                    }
                }
            }
            placed.push((row, rr));
            y += rh;
        }
        dl.pop_clip();
        self.facets.rows = placed;
        // Rows come after the first count: until then the panel keeps the focus at its top.
        if let Some(k) = self.facets.focus
            && !self.facets.rows.is_empty()
        {
            self.facets.focus = self
                .facets
                .value_row(k, 1)
                .or_else(|| self.facets.value_row(k, -1));
        }
    }

    /// The panel's search field has the keyboard (the bar's search field, focused since,
    /// takes it back).
    pub(super) fn facets_finding(&self) -> bool {
        self.facets.open && self.facets.find_focus && !self.filter_focus
    }

    /// Text typed into the panel's search field.
    pub(super) fn facets_find_text(&mut self, s: &str) {
        let s: String = s.chars().filter(|c| !c.is_control()).collect();
        if !s.is_empty() {
            self.facets.find.push_str(&s);
            self.facets.scroll = 0.0;
        }
    }

    /// Keys while the panel's search field has the keyboard. True when used: editing keys,
    /// text, and Esc (empties the field, then leaves it); Enter, ↓ and Tab go on to the
    /// values. Ctrl, Alt and ⌘ shortcuts go on.
    pub(super) fn facets_find_key(
        &mut self,
        ev: &specular_gfx::winit::event::KeyEvent,
        combo: Option<&specular_input::KeyCombo>,
    ) -> bool {
        use specular_input::Key;
        let mods = combo.map(|c| c.mods).unwrap_or_default();
        if mods.ctrl || mods.alt || mods.meta {
            return false;
        }
        let p = &mut self.facets;
        if let Some(c) = combo
            && let Key::Named(n) = c.key
        {
            match n {
                NamedKey::Backspace => {
                    p.find.pop();
                    p.scroll = 0.0;
                    return true;
                }
                NamedKey::Escape if p.find.is_empty() => p.find_focus = false,
                NamedKey::Escape => {
                    p.find.clear();
                    p.scroll = 0.0;
                }
                NamedKey::Enter | NamedKey::ArrowDown | NamedKey::Tab => {
                    p.find_focus = false;
                    p.focus = p.value_row(0, 1).or(Some(0));
                }
                NamedKey::Space => self.facets_find_text(" "),
                _ => {}
            }
            return true;
        }
        if let Some(t) = &ev.text {
            self.facets_find_text(t);
        }
        true
    }

    /// The wheel over the panel. True when it was there.
    pub(super) fn facets_wheel(&mut self, dy: f32, p: (f32, f32)) -> bool {
        if !self.facets.rect.is_some_and(|r| r.contains(p)) {
            return false;
        }
        self.facets.scroll = (self.facets.scroll - dy).max(0.0);
        true
    }

    /// A click on a value row.
    pub(super) fn facets_click(&mut self, k: usize) {
        if let Some((Row::Value { field, key, .. }, _)) = self.facets.rows.get(k).cloned() {
            self.facets.focus = Some(k);
            self.filter_focus = false;
            self.facets_toggle(field, &key);
        }
    }

    /// Keys while the panel has the focus. True when used.
    pub(super) fn facets_key(&mut self, n: NamedKey, shift: bool) -> bool {
        let Some(k) = self.facets.focus else {
            return false;
        };
        let rows = self.facets.rows.len();
        let moves = matches!(
            n,
            NamedKey::ArrowDown
                | NamedKey::ArrowUp
                | NamedKey::Home
                | NamedKey::End
                | NamedKey::PageDown
                | NamedKey::PageUp
        );
        if rows == 0 && moves {
            // Still counting: nothing to move to yet.
            return true;
        }
        match n {
            NamedKey::ArrowDown => {
                self.facets.focus = self.facets.value_row(k + 1, 1).or(Some(k));
            }
            NamedKey::ArrowUp => {
                self.facets.focus = k
                    .checked_sub(1)
                    .and_then(|j| self.facets.value_row(j, -1))
                    .or(Some(k));
            }
            NamedKey::Home => self.facets.focus = self.facets.value_row(0, 1),
            NamedKey::End => {
                self.facets.focus = self.facets.value_row(rows.saturating_sub(1), -1);
            }
            NamedKey::PageDown | NamedKey::PageUp => {
                let step = 8isize * if n == NamedKey::PageDown { 1 } else { -1 };
                let j = (k as isize + step).clamp(0, rows as isize - 1).max(0) as usize;
                self.facets.focus = self
                    .facets
                    .value_row(j, step.signum())
                    .or_else(|| self.facets.value_row(j, -step.signum()));
            }
            NamedKey::Space | NamedKey::Enter => self.facets_click(k),
            NamedKey::Escape => self.facets.focus = None,
            // On along the filter bar's order (back to Clear, or on to the grid).
            NamedKey::Tab => self.tab(shift),
            _ => return false,
        }
        true
    }

    /// AccessKit nodes of the panel: a group of check boxes. False when it is not shown.
    pub(super) fn facets_a11y(
        &self,
        tree: &mut accesskit::TreeUpdate,
        bounds: &dyn Fn(Rect) -> accesskit::Rect,
    ) -> bool {
        use accesskit::{Node, NodeId, Role, Toggled};
        let Some(r) = self.facets.rect else {
            return false;
        };
        let id = NodeId(A11Y_FACETS);
        let mut group = Node::new(Role::Group);
        group.set_label(t!("gallery-facets"));
        group.set_bounds(bounds(r));
        let mut ids = Vec::new();
        let mut section = String::new();
        for (k, (row, rr)) in self.facets.rows.iter().enumerate() {
            match row {
                Row::Header(f) => section = field_name(*f),
                Row::Value {
                    field,
                    key,
                    count,
                    active,
                } => {
                    let nid = NodeId(A11Y_FACETS + 1 + k as u64);
                    let mut n = Node::new(Role::CheckBox);
                    n.set_label(t!(
                        "gallery-facet-a11y-value",
                        facet = section.as_str(),
                        value = value_name(*field, key, false),
                        count = i64::from(*count)
                    ));
                    n.set_toggled(if *active {
                        Toggled::True
                    } else {
                        Toggled::False
                    });
                    n.set_bounds(bounds(*rr));
                    n.add_action(accesskit::Action::Click);
                    n.add_action(accesskit::Action::Focus);
                    ids.push(nid);
                    tree.nodes.push((nid, n));
                    if self.facets.focus == Some(k) && tree.focus == NodeId(1) {
                        tree.focus = nid;
                    }
                }
            }
        }
        group.set_children(ids);
        tree.nodes.push((id, group));
        if let Some((_, root)) = tree.nodes.iter_mut().find(|(i, _)| *i == NodeId(1)) {
            root.push_child(id);
        }
        true
    }
}

impl App {
    /// AccessKit: an action on a value of the panel; false when not ours.
    pub(super) fn facets_a11y_action(&mut self, target: u64, click: bool) -> bool {
        let ga = &mut self.gallery;
        if !ga.facets.open || !(A11Y_FACETS + 1..A11Y_FACETS + (1 << 32)).contains(&target) {
            return false;
        }
        let k = (target - A11Y_FACETS - 1) as usize;
        if click {
            ga.facets_click(k);
        } else if k < ga.facets.rows.len() {
            ga.facets.focus = Some(k);
            ga.filter_focus = false;
        }
        self.redraw = true;
        true
    }
}

impl Gallery {
    /// The metadata editor, docked at the trailing edge below the bars (where the keyword
    /// panel goes: one at a time).
    pub(super) fn draw_meta_edit(&mut self, dl: &mut DrawList, text: &mut Text, f: &Frame) {
        if !self.meta_edit.open {
            self.meta_edit.rect = None;
            return;
        }
        let pw = crate::app::side_panel::width(self.vw, f.text_scale);
        let top = TOP_BAR + self.filter_bar_h();
        let h = (self.vh - BOTTOM_BAR - top).max(0.0);
        let panel = self.flip(Rect::new(self.vw - pw, top, pw, h));
        self.hits.push((panel, Hit::MetaEditPanel));
        self.meta_edit
            .draw(dl, text, f.pal, panel, f.text_scale, self.rtl, self.pointer);
    }

    /// The keyword panel, docked at the trailing edge below the bars.
    pub(super) fn draw_keywords(&mut self, dl: &mut DrawList, text: &mut Text, f: &Frame) {
        if !self.keywords.open {
            self.keywords.rect = None;
            return;
        }
        let pw = crate::app::side_panel::width(self.vw, f.text_scale);
        let top = TOP_BAR + self.filter_bar_h();
        let h = (self.vh - BOTTOM_BAR - top).max(0.0);
        let panel = self.flip(Rect::new(self.vw - pw, top, pw, h));
        self.hits.push((panel, Hit::KeywordsPanel));
        self.keywords
            .draw(dl, text, f.pal, panel, f.text_scale, self.rtl, self.pointer);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An AI term asks for the pack only while the pack is missing.
    #[test]
    fn ai_terms_ask_for_the_pack_only_without_it() {
        let q = specular_query::parse_in("text:(invoice)", specular_query::Scope::Library);
        let e = &q.errors[0];
        assert!(matches!(e.kind, specular_query::ErrorKind::NeedsAiPack(_)));
        assert_eq!(
            query_error_with(e, false),
            t!("gallery-query-needs-ai", field = "text")
        );
        assert_eq!(
            query_error_with(e, true),
            t!("gallery-query-ai-not-yet", field = "text")
        );
    }

    #[test]
    fn rows_list_top_values_and_selected_ones() {
        let mut p = Panel::default();
        let values = (0..12)
            .map(|k| facet::Value {
                key: format!("Cam {k}"),
                count: 100 - k,
            })
            .collect();
        p.facets = vec![Facet {
            field: FacetField::Camera,
            values,
        }];
        let q = specular_query::parse("camera=\"Cam 11\"");
        let rows = p.build_rows(&q);
        assert_eq!(rows.len(), 1 + TOP + 1);
        assert!(matches!(&rows[0], Row::Header(FacetField::Camera)));
        assert!(matches!(&rows[TOP + 1], Row::Value { key, active: true, .. } if key == "Cam 11"));
    }

    #[test]
    fn keys_skip_headers() {
        let mut p = Panel::default();
        let r = Rect::new(0.0, 0.0, 1.0, 1.0);
        let v = |k: &str| Row::Value {
            field: FacetField::Format,
            key: k.into(),
            count: 1,
            active: false,
        };
        p.rows = vec![
            (Row::Header(FacetField::Format), r),
            (v("jpg"), r),
            (Row::Header(FacetField::Rating), r),
            (v("png"), r),
        ];
        assert_eq!(p.value_row(0, 1), Some(1));
        assert_eq!(p.value_row(2, 1), Some(3));
        assert_eq!(p.value_row(2, -1), Some(1));
        assert_eq!(p.value_row(9, -1), Some(3));
        // Before the first count there are no rows (focus moved in while counting).
        assert_eq!(Panel::default().value_row(0, 1), None);
    }
}
