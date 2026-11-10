//! The search field's value picker: with the caret in a `field:` term whose values can be
//! listed (camera, lens, format, rating, label, year, orientation, `is:`/`has:`, the camera
//! settings, and in the Library tags and folders), a list under the field shows every value
//! with its count among the files the rest of the query lets through: the facets' counts
//! ([`super::facets`], or the catalogue's in the Library), with the value being typed left
//! out of the query and used to filter the list instead. Enter picks a value; Space,
//! Ctrl+Enter or the check box selects several (`camera=a,b`). A bare word that starts a field
//! name lists the fields that have values.
//!
//! The number fields compare too: buttons over the list switch the term between `=`, `>=`
//! and `<=`, and under a comparison each value counts the files it lets through (`iso>=800`:
//! at ISO 800 and above). Widths, heights, megapixels and file sizes list thresholds (a
//! ladder of round numbers) rather than every value; dates list years, and their months
//! once something is typed.
//!
//! Only the rows in view are formatted and drawn, so a list of 100 000 folders opens and
//! filters within a frame (the folded keys are made once per count).

use accesskit::{Action, HasPopup, Node, NodeId, Role};
use specular_gfx::{DrawList, Rect, Text, TextStyle, draw_icon};
use specular_input::{Mods, NamedKey};
use specular_metadata::Localizer as MetaLoc;
use specular_query::Op;
use specular_query::complete::{self, Context, ValueCtx};
use specular_query::facet::{FacetField, Value};

use super::draw::{Frame, text_at, truncate};
use super::facets::{field_name, value_name};
use super::*;

/// AccessKit ids: the list box, its status, then one per row.
pub(super) const A11Y_PICKER: u64 = A11Y_BASE + (9 << 40);
const ID_LIST: u64 = A11Y_PICKER;
const ID_STATUS: u64 = A11Y_PICKER + 1;
/// The operator buttons' radio group, then one per button.
const ID_OPS: u64 = A11Y_PICKER + 2;
const ID_OP: u64 = A11Y_PICKER + 3;
const ID_ROW: u64 = A11Y_PICKER + 16;
/// Rows in view at most (more scroll).
const MAX_ROWS: usize = 10;

/// The operator buttons of a number field: the operator as typed, and its glyph.
const OPS: [(&str, &str); 3] = [(":", "="), (">=", "≥"), ("<=", "≤")];

/// The operator buttons a field's list has (indices into [`OPS`]): a ladder lists
/// thresholds, so it has no `=`.
fn ops_of(facet: FacetField) -> &'static [u8] {
    if facet.ladder().is_some() {
        &[1, 2]
    } else if facet.numeric() {
        &[0, 1, 2]
    } else {
        &[]
    }
}

/// The operator button that is on for a term (none for `>`, `<` and `!=`).
fn op_on(v: &ValueCtx) -> Option<u8> {
    match v.compare() {
        Some(Op::Ge) => Some(1),
        Some(Op::Le) => Some(2),
        Some(_) => None,
        None if matches!(v.op.as_str(), ":" | "=" | ":=") => Some(0),
        None => None,
    }
}

/// The comparison a list counts for: the term's, and always one for a ladder.
fn cmp_of(v: &ValueCtx) -> Option<Op> {
    v.compare()
        .or_else(|| v.facet.ladder().is_some().then_some(Op::Ge))
}

/// A comparison's glyph before a value. Right to left the glyph is mirrored, as the bidi
/// algorithm mirrors it in running text: “≥ 1920” read from the right shows ≤.
fn op_glyph(op: Op, rtl: bool) -> &'static str {
    let (ltr, mirrored) = match op {
        Op::Ge => ("≥", "≤"),
        Op::Gt => (">", "<"),
        Op::Le => ("≤", "≥"),
        Op::Lt => ("<", ">"),
        Op::Has | Op::Eq => ("=", "="),
    };
    if rtl { mirrored } else { ltr }
}

/// A value under a comparison, for screen readers: “at least 1920 px”.
fn compared(op: Option<Op>, value: String) -> String {
    match op {
        Some(Op::Ge) => t!("gallery-picker-ge", value = value),
        Some(Op::Gt) => t!("gallery-picker-gt", value = value),
        Some(Op::Le) => t!("gallery-picker-le", value = value),
        Some(Op::Lt) => t!("gallery-picker-lt", value = value),
        _ => value,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Row {
    /// Select every value the filter matches.
    All,
    /// A value: its index in the facet's values.
    Value(u32),
    /// A field name to complete (`camera:`).
    Name(&'static str, FacetField),
}

/// Folded keys (and names where they differ) of one count of a facet, for filtering.
struct Folded {
    generation: u64,
    facet: FacetField,
    cmp: Option<Op>,
    lang: String,
    keys: Vec<String>,
}

#[derive(Default)]
pub(super) struct Picker {
    /// Closed for this text and the term starting at this byte (Escape, a pick, a click
    /// elsewhere) until the text changes or the caret goes to another term.
    pub dismissed: Option<(String, usize)>,
    /// The row the arrows are on (the field keeps the focus: AccessKit's active descendant).
    pub active: Option<usize>,
    pub scroll: f32,
    /// The filter kept after a toggle took the typed value out of the text.
    pub sticky: String,
    /// Where it was drawn; the rows in view and where.
    pub rect: Option<Rect>,
    pub shown: Vec<(usize, Rect)>,
    /// The scrollbar is dragged (the grab's offset in its thumb).
    pub drag: Option<f32>,
    /// Scrollbar track, thumb height and the furthest scroll, as last drawn.
    track: Option<(Rect, f32, f32)>,
    rows: Vec<Row>,
    /// Every value row is selected (the All row's check box).
    all_on: bool,
    /// The facet the rows list (an index into the counts), and what the rows were made for.
    facet: Option<usize>,
    /// Under a comparison: the values the rows index, counted for it ([`Self::cmp`]).
    values: Option<Vec<Value>>,
    cmp: Option<Op>,
    /// The operator buttons as drawn (an index into [`OPS`]).
    pub ops: Vec<(u8, Rect)>,
    rows_for: Option<(u64, String, usize, String, String)>,
    folded: Option<Folded>,
    /// The row to keep active once the rows are made again (after a toggle).
    keep: Option<Row>,
    keep_key: Option<String>,
    pub counting: bool,
    /// The field's name, for the list's label.
    label: String,
}

impl Picker {
    /// Text was typed: the list starts over.
    pub fn typed(&mut self) {
        self.sticky.clear();
        self.active = None;
        self.scroll = 0.0;
    }

    fn visible(&self) -> bool {
        !self.rows.is_empty() || self.counting
    }
}

/// Where the term a context is for starts.
fn term_start(c: &Context) -> usize {
    match c {
        Context::Value(v) => v.span.start,
        Context::Name { span, .. } => span.start,
    }
}

/// `text` without `span`.
fn cut(text: &str, span: specular_query::parse::Span) -> String {
    format!("{}{}", &text[..span.start], &text[span.end..])
}

/// A value as listed (`visual`: stars as ★ rather than words). The camera settings are
/// written as the info panel writes them.
fn shown(facet: FacetField, key: &str, loc: Option<&MetaLoc>, visual: bool) -> String {
    use specular_metadata::strings;
    let Some(l) = loc else {
        return value_name(facet, key, visual);
    };
    let s = match facet {
        FacetField::Iso => key.parse::<u32>().ok().map(|n| strings::iso(l, n)),
        FacetField::Aperture => key.parse::<f64>().ok().map(|f| strings::f_number(l, f)),
        FacetField::Focal => key
            .parse::<f64>()
            .ok()
            .map(|mm| strings::focal_length(l, mm, None)),
        FacetField::Exposure => match key.strip_prefix("1/") {
            Some(d) => d
                .parse::<u32>()
                .ok()
                .and_then(|d| strings::exposure_time(l, 1, d)),
            None => key
                .parse::<f64>()
                .ok()
                .and_then(|v| strings::exposure_time(l, (v * 10.0).round() as u32, 10)),
        },
        FacetField::Space => {
            let model = match key {
                "grey" => "Greyscale",
                "cmyk" => "CMYK",
                "lab" => "Lab",
                _ => "",
            };
            (!model.is_empty()).then(|| {
                let mut a = specular_metadata::FluentArgs::new();
                a.set("model", model);
                l.fmt("colour-model", &a)
            })
        }
        _ => None,
    };
    s.unwrap_or_else(|| value_name(facet, key, visual))
}

/// A field's name (the facets panel's, or the info panel's for the camera settings).
fn label(facet: FacetField, loc: Option<&MetaLoc>) -> String {
    let id = match facet {
        FacetField::Iso => "label-iso",
        FacetField::Aperture => "label-aperture",
        FacetField::Focal => "label-focal-length",
        FacetField::Exposure => "label-shutter",
        FacetField::Tag => "label-keywords",
        FacetField::Folder => return t!("gallery-prop-folder"),
        FacetField::Modified => "label-modified",
        FacetField::Space => "label-colour-profile",
        FacetField::Software => "label-software",
        FacetField::Flash => "label-flash",
        FacetField::WhiteBalance => "label-white-balance",
        FacetField::Program => "label-program",
        FacetField::Metering => "label-metering",
        FacetField::ExpComp => "label-exposure-bias",
        f => return field_name(f),
    };
    loc.map_or_else(|| field_name(facet), |l| l.text(id))
}

/// The search fold ([`model::fold`]); plain ASCII (most keys: folders, cameras) just lowercased,
/// which is what the fold makes of it, without its normalizer.
fn fold(s: &str) -> std::borrow::Cow<'_, str> {
    if s.is_ascii() {
        if s.bytes().any(|b| b.is_ascii_uppercase()) {
            std::borrow::Cow::Owned(s.to_ascii_lowercase())
        } else {
            std::borrow::Cow::Borrowed(s)
        }
    } else {
        model::fold(s)
    }
}

/// Whether a facet's names differ from its keys (they are folded for the filter too).
fn named(facet: FacetField) -> bool {
    !matches!(
        facet,
        FacetField::Camera
            | FacetField::Lens
            | FacetField::Year
            | FacetField::Tag
            | FacetField::Folder
    )
}

impl Gallery {
    /// What the picker lists for the term under the field's caret, if anything.
    pub(super) fn picker_ctx(&self) -> Option<Context> {
        if !(self.active && self.filter_open && self.filter_focus) || !self.preedit.is_empty() {
            return None;
        }
        let text = &self.filter.text;
        let library = self.filter.scope == specular_query::Scope::Library;
        let ctx = match complete::context(text, self.filter_caret())? {
            Context::Value(v)
                if !library && matches!(v.facet, FacetField::Folder | FacetField::Exif) =>
            {
                return None;
            }
            c => c,
        };
        let start = term_start(&ctx);
        if self
            .picker
            .dismissed
            .as_ref()
            .is_some_and(|(t, s)| t == text && *s == start)
        {
            return None;
        }
        Some(ctx)
    }

    /// Whether the picker needs counts (the facets pass runs for it as for the panel).
    pub(super) fn picker_counts(&self) -> bool {
        self.picker_ctx().is_some()
    }

    /// Whether its counts need the duplicate and similar groups' roles (`dupes:`, or a name
    /// that may become it).
    pub(super) fn picker_sets(&self) -> bool {
        let group = |f: FacetField| matches!(f, FacetField::Dupes | FacetField::Similar);
        match self.picker_ctx() {
            Some(Context::Value(v)) => group(v.facet),
            Some(Context::Name { prefix, .. }) => {
                complete::names(&prefix).iter().any(|(_, f)| group(*f))
            }
            None => false,
        }
    }

    /// Whether its counts need every file's metadata (field names: any field may have values).
    pub(super) fn picker_needs_meta(&self) -> bool {
        match self.picker_ctx() {
            Some(Context::Value(v)) => v.facet.needs_meta(),
            Some(Context::Name { .. }) => true,
            None => false,
        }
    }

    /// The query the counts are made for: without the value being typed (it filters the
    /// list), the property term being edited, or the word that may become a field name.
    pub(super) fn picker_count_text(&self) -> String {
        let text = &self.filter.text;
        match self.picker_ctx() {
            Some(Context::Value(v))
                if matches!(v.facet, FacetField::Property | FacetField::Flags) =>
            {
                cut(text, v.span)
            }
            Some(Context::Value(v)) => cut(text, v.partial.span),
            Some(Context::Name { span, .. }) => cut(text, span),
            None => text.clone(),
        }
    }

    /// The localizer for the camera settings' words, in the UI language.
    fn picker_loc(&mut self) -> String {
        let lang = I18n::global().localizer().language().clone();
        if self.place_loc.as_ref().is_none_or(|l| *l.langid() != lang) {
            self.place_loc = Some(MetaLoc::new(lang.clone()));
        }
        lang.to_string()
    }

    /// The context, with the rows made for it if they are out of date.
    pub(super) fn picker_refresh(&mut self) -> Option<Context> {
        let Some(ctx) = self.picker_ctx() else {
            self.picker.rows.clear();
            self.picker.rows_for = None;
            self.picker.active = None;
            self.picker.counting = false;
            return None;
        };
        let lang = self.picker_loc();
        let key = (
            self.facets.generation,
            self.filter.text.clone(),
            self.filter_caret(),
            self.picker.sticky.clone(),
            lang.clone(),
        );
        if self.picker.rows_for.as_ref() != Some(&key) {
            self.picker_build(&ctx, &lang);
            self.picker.rows_for = Some(key);
        }
        self.picker.counting = if self.library.on {
            self.library.pending()
        } else {
            self.facets.counting()
        };
        Some(ctx)
    }

    fn picker_build(&mut self, ctx: &Context, lang: &str) {
        let p = &mut self.picker;
        p.rows.clear();
        p.all_on = false;
        p.facet = None;
        p.values = None;
        p.cmp = None;
        let loc = self.place_loc.as_ref();
        match ctx {
            Context::Name { prefix, .. } => {
                p.label = String::new();
                let library = self.filter.scope == specular_query::Scope::Library;
                for (name, facet) in complete::names(prefix) {
                    let has = self
                        .facets
                        .facets
                        .iter()
                        .any(|f| f.field == facet && f.values.iter().any(|v| v.count > 0));
                    let here = library || !matches!(facet, FacetField::Folder | FacetField::Exif);
                    if has && here {
                        p.rows.push(Row::Name(name, facet));
                    }
                }
            }
            Context::Value(v) => {
                p.label = label(v.facet, loc);
                let Some(fi) = self.facets.facets.iter().position(|f| f.field == v.facet) else {
                    return;
                };
                p.facet = Some(fi);
                let facet = &self.facets.facets[fi];
                let cmp = cmp_of(v);
                p.cmp = cmp;
                p.values = cmp.map(|op| specular_query::facet::compare(facet, op));
                let values: &[Value] = p.values.as_deref().unwrap_or(&facet.values);
                let typed = if v.partial.value.trim().is_empty() {
                    p.sticky.as_str()
                } else {
                    v.partial.value.as_str()
                };
                let needle = fold(typed.trim()).into_owned();
                // Dates list their years, and the months once something is typed.
                let dated = matches!(v.facet, FacetField::Taken | FacetField::Modified);
                let month = |k: &str| dated && needle.is_empty() && k.contains('-');
                // `exif:` lists tag names, and a tag's words once `=` is typed.
                let words = typed.contains('=');
                let exif = |k: &str| v.facet != FacetField::Exif || k.contains('=') == words;
                let listed = |i: usize| {
                    let x = &values[i];
                    (x.count > 0 || v.has(&x.key)) && !month(&x.key) && exif(&x.key)
                };
                if needle.is_empty() {
                    p.rows.extend(
                        (0..values.len())
                            .filter(|&i| listed(i))
                            .map(|i| Row::Value(i as u32)),
                    );
                } else {
                    let generation = self.facets.generation;
                    let fresh = p.folded.as_ref().is_some_and(|f| {
                        f.generation == generation
                            && f.facet == v.facet
                            && f.cmp == cmp
                            && f.lang == lang
                    });
                    if !fresh {
                        let keys = values
                            .iter()
                            .map(|x| {
                                let mut s = fold(&x.key).into_owned();
                                if named(v.facet) {
                                    s.push('\n');
                                    s.push_str(&fold(&shown(v.facet, &x.key, loc, false)));
                                }
                                s
                            })
                            .collect();
                        p.folded = Some(Folded {
                            generation,
                            facet: v.facet,
                            cmp,
                            lang: lang.to_owned(),
                            keys,
                        });
                    }
                    let keys = &p.folded.as_ref().expect("made above").keys;
                    p.rows.extend(
                        (0..values.len())
                            .filter(|&i| keys[i].contains(needle.as_str()) && listed(i))
                            .map(|i| Row::Value(i as u32)),
                    );
                    if v.plain() && p.rows.len() > 1 {
                        p.all_on = p.rows.iter().all(|r| match r {
                            Row::Value(i) => v.has(&values[*i as usize].key),
                            _ => true,
                        });
                        p.rows.insert(0, Row::All);
                    }
                }
            }
        }
        // After a toggle the same row stays active; otherwise the active row stays in range.
        if let Some(keep) = p.keep.take() {
            let key = p.keep_key.take();
            let values = p
                .values
                .as_ref()
                .or(p.facet.map(|fi| &self.facets.facets[fi].values));
            p.active = p.rows.iter().position(|r| match (r, &keep) {
                (Row::Value(i), Row::Value(_)) => values
                    .zip(key.as_deref())
                    .is_some_and(|(vs, k)| vs[*i as usize].key == k),
                (a, b) => a == b,
            });
        }
        if let Some(a) = p.active
            && a >= p.rows.len()
        {
            p.active = p.rows.len().checked_sub(1);
        }
    }

    /// A value row's key and count.
    fn picker_value(&self, i: u32) -> Option<(&str, u32)> {
        let v = match &self.picker.values {
            Some(values) => values.get(i as usize)?,
            None => self
                .facets
                .facets
                .get(self.picker.facet?)?
                .values
                .get(i as usize)?,
        };
        Some((v.key.as_str(), v.count))
    }

    /// Switches the term to operator button `k` (`=`, `≥`, `≤`); the list stays open.
    pub(super) fn picker_op(&mut self, k: u8) {
        self.filter_focus = true;
        let Some(Context::Value(v)) = self.picker_refresh() else {
            return;
        };
        let Some((op, _)) = OPS.get(usize::from(k)) else {
            return;
        };
        if !ops_of(v.facet).contains(&k) || op_on(&v) == Some(k) {
            return;
        }
        let (t, caret) = complete::set_op(&self.filter.text, &v, op);
        self.picker.typed();
        self.picker_set(t, caret);
    }

    /// Sets the text the picker edited, with the caret at byte `caret` (the end of the term).
    fn picker_set(&mut self, text: String, caret: usize) {
        if text != self.filter.text {
            self.filter.text = text;
            self.dirty = true;
        }
        self.set_filter_caret(caret);
    }

    /// Closes the picker (until the text changes), without a trailing comma left by toggling.
    pub(super) fn picker_dismiss(&mut self) {
        if let Some(Context::Value(v)) = self.picker_ctx() {
            let t = complete::tidy(&self.filter.text, &v);
            // The comma went from before the caret (it is in the value being typed).
            let caret = self
                .filter_caret()
                .saturating_sub(self.filter.text.len() - t.len());
            self.picker_set(t, caret);
        }
        if let Some(c) = self.picker_ctx() {
            self.picker.dismissed = Some((self.filter.text.clone(), term_start(&c)));
        }
        self.picker.typed();
        self.picker.rows.clear();
        self.picker.rows_for = None;
        self.picker.rect = None;
    }

    /// The values the All row selects: those listed.
    fn picker_listed(&self) -> Vec<String> {
        self.picker
            .rows
            .iter()
            .filter_map(|r| match r {
                Row::Value(i) => self.picker_value(*i).map(|(k, _)| k.to_owned()),
                _ => None,
            })
            .collect()
    }

    /// The filter typed (or kept) for the list.
    fn picker_needle(&self, v: &ValueCtx) -> String {
        if v.partial.value.trim().is_empty() {
            self.picker.sticky.clone()
        } else {
            v.partial.value.clone()
        }
    }

    /// Picks row `k`: a value replaces the one being typed and the list closes; a name is
    /// completed; the All row selects (or clears) every listed value.
    pub(super) fn picker_choose(&mut self, k: usize) {
        let Some(ctx) = self.picker_refresh() else {
            return;
        };
        let Some(&row) = self.picker.rows.get(k) else {
            return;
        };
        match (row, ctx) {
            (Row::Name(name, _), Context::Name { span, .. }) => {
                let (t, caret) = complete::complete_name(&self.filter.text, span, name);
                self.picker_set(t, caret);
                self.picker.typed();
            }
            (Row::All, Context::Value(_)) => self.picker_toggle(k),
            (Row::Value(i), Context::Value(v)) => {
                let Some((key, _)) = self.picker_value(i) else {
                    return;
                };
                let (t, caret) = complete::pick(&self.filter.text, &v, key);
                self.picker_set(t, caret);
                self.picker.typed();
                self.picker.dismissed = Some((self.filter.text.clone(), v.span.start));
            }
            _ => {}
        }
    }

    /// Toggles row `k` (Space, Ctrl+Enter, the check box): the value joins or leaves the
    /// term's values and the list stays open, filtered as before.
    pub(super) fn picker_toggle(&mut self, k: usize) {
        let Some(ctx) = self.picker_refresh() else {
            return;
        };
        let Some(&row) = self.picker.rows.get(k) else {
            return;
        };
        let Context::Value(v) = ctx else {
            return self.picker_choose(k);
        };
        if !v.plain() {
            if matches!(row, Row::Value(_)) {
                self.picker_choose(k);
            }
            return;
        }
        let needle = self.picker_needle(&v);
        let (text, caret) = match row {
            Row::All => {
                let keys = self.picker_listed();
                if self.picker.all_on {
                    // Clear them: one at a time (each edit moves the others; the caret stays
                    // at the end of the term).
                    let (mut t, mut at) = (self.filter.text.clone(), self.filter_caret());
                    for key in &keys {
                        if let Some(Context::Value(c)) = complete::context(&t, at)
                            && c.has(key)
                        {
                            (t, at) = complete::toggle(&t, &c, key);
                        }
                    }
                    (t, at)
                } else {
                    let keys: Vec<&str> = keys.iter().map(String::as_str).collect();
                    complete::select_all(&self.filter.text, &v, &keys)
                }
            }
            Row::Value(i) => {
                let Some((key, _)) = self.picker_value(i) else {
                    return;
                };
                let key = key.to_owned();
                self.picker.keep_key = Some(key.clone());
                complete::toggle(&self.filter.text, &v, &key)
            }
            Row::Name(..) => return,
        };
        self.picker.keep = Some(row);
        self.picker.sticky = needle;
        self.picker_set(text, caret);
    }

    /// A click on row `k` (`many`: its check box, or with Ctrl).
    pub(super) fn picker_click(&mut self, k: usize, many: bool) {
        self.filter_focus = true;
        self.picker.active = Some(k);
        if many {
            self.picker_toggle(k);
        } else {
            self.picker_choose(k);
        }
    }

    /// Keys while the field has the focus. True when the picker used it.
    pub(super) fn picker_key(&mut self, n: NamedKey, m: Mods) -> bool {
        if !self.preedit.is_empty() {
            return false;
        }
        // Alt+Down opens it again after Escape.
        if n == NamedKey::ArrowDown && m.alt && !m.ctrl && !m.meta {
            if self.picker.dismissed.take().is_some() {
                self.picker.typed();
            }
            return self.picker_refresh().is_some();
        }
        if m.alt || m.meta {
            return false;
        }
        let Some(ctx) = self.picker_refresh() else {
            return false;
        };
        if !self.picker.visible() {
            return false;
        }
        let rows = self.picker.rows.len();
        let last = rows.checked_sub(1);
        let active = self.picker.active;
        let page = MAX_ROWS - 1;
        match n {
            NamedKey::ArrowDown => {
                self.picker.active = match active {
                    None => last.map(|_| 0),
                    Some(a) => Some((a + 1).min(rows - 1)),
                };
            }
            NamedKey::ArrowUp => self.picker.active = active.and_then(|a| a.checked_sub(1)),
            NamedKey::PageDown => {
                self.picker.active = last.map(|l| active.map_or(0, |a| (a + page).min(l)));
            }
            NamedKey::PageUp => {
                self.picker.active = active.map(|a| a.saturating_sub(page));
            }
            NamedKey::Home if active.is_some() => self.picker.active = last.map(|_| 0),
            NamedKey::End if active.is_some() => self.picker.active = last,
            NamedKey::Enter => match active {
                Some(k) if m.ctrl => self.picker_toggle(k),
                Some(k) => self.picker_choose(k),
                // The field's Enter (on to the grid), without a dangling comma.
                None => {
                    self.picker_dismiss();
                    return false;
                }
            },
            NamedKey::Space if !m.ctrl => match active {
                Some(k) => self.picker_toggle(k),
                None => return false,
            },
            NamedKey::Tab if !m.shift && !m.ctrl => match active {
                Some(k) => self.picker_choose(k),
                None if matches!(ctx, Context::Name { .. }) && rows > 0 => self.picker_choose(0),
                None => self.picker_dismiss(),
            },
            NamedKey::Escape => self.picker_dismiss(),
            // Backspace first drops the filter kept after a toggle.
            NamedKey::Backspace
                if !self.picker.sticky.is_empty()
                    && matches!(&ctx, Context::Value(v) if v.partial.value.is_empty()) =>
            {
                self.picker.typed();
            }
            _ => return false,
        }
        true
    }

    /// The wheel over the list. True when it was there.
    pub(super) fn picker_wheel(&mut self, dy: f32, p: (f32, f32)) -> bool {
        if !self.picker.rect.is_some_and(|r| r.contains(p)) {
            return false;
        }
        let max = self.picker.track.map_or(0.0, |t| t.2);
        self.picker.scroll = (self.picker.scroll - dy).clamp(0.0, max);
        true
    }

    /// A press on the scrollbar: the thumb is grabbed where it was pressed (else by its middle,
    /// and it jumps there).
    pub(super) fn picker_grab(&mut self, y: f32) {
        let Some((track, th, max)) = self.picker.track else {
            return;
        };
        let ty = track.y + (track.h - th) * (self.picker.scroll / max.max(1.0));
        let grab = if (ty..=ty + th).contains(&y) {
            y - ty
        } else {
            th * 0.5
        };
        self.picker.drag = Some(grab);
        self.picker_drag_to(y, grab);
    }

    pub(super) fn picker_drag_to(&mut self, y: f32, grab: f32) {
        let Some((track, th, max)) = self.picker.track else {
            return;
        };
        let room = (track.h - th).max(1.0);
        self.picker.scroll = ((y - grab - track.y) / room * max).clamp(0.0, max);
    }

    /// Draws the list under the search field (over the grid, the facets and the Library's
    /// sidebar).
    pub(super) fn draw_picker(&mut self, dl: &mut DrawList, text: &mut Text, f: &Frame) {
        self.picker.rect = None;
        self.picker.shown.clear();
        self.picker.ops.clear();
        self.picker.track = None;
        let Some(ctx) = self.picker_refresh() else {
            return;
        };
        if !self.picker.visible() {
            return;
        }
        let Some(field) = self
            .bar
            .rects
            .iter()
            .find(|(s, _)| *s == filterbar::Stop::Field)
            .map(|(_, r)| *r)
        else {
            return;
        };
        let pal = f.pal;
        let row_h = self.u(28.0);
        let pad = self.u(4.0);
        let row_style = self.ts(TextStyle::new(13.0));
        let count_style = self.ts(TextStyle::new(12.0).tabular());
        let small = self.ts(TextStyle::new(11.0));
        // Laid out left to right, then mirrored.
        let fl = self.flip(field);
        let w = fl.w.max(self.u(280.0)).min(self.vw - 16.0).max(0.0);
        let x = fl.x.min(self.vw - 8.0 - w).max(8.0);
        let top = fl.bottom() + self.u(4.0);
        let value = match &ctx {
            Context::Value(v) => Some(v.clone()),
            Context::Name { .. } => None,
        };
        let status = self.picker.counting || !self.picker.sticky.is_empty();
        let status_h = if status { self.u(24.0) } else { 0.0 };
        let hint = value.as_ref().is_some_and(|v| v.plain()) && !self.picker.rows.is_empty();
        let hint_h = if hint { self.u(24.0) } else { 0.0 };
        let ops = value.as_ref().map_or(&[][..], |v| ops_of(v.facet));
        let ops_h = if ops.is_empty() { 0.0 } else { self.u(32.0) };
        let room =
            (self.vh - BOTTOM_BAR - top - 8.0 - 2.0 * pad - status_h - ops_h - hint_h).max(row_h);
        let n = self.picker.rows.len();
        let fit = ((room / row_h).floor() as usize).clamp(1, MAX_ROWS);
        let in_view = n.min(fit);
        let list_h = in_view as f32 * row_h;
        let h = 2.0 * pad + status_h + ops_h + list_h + hint_h;
        let panel = Rect::new(x, top, w, h);
        let r = self.flip(panel);
        let radius = self.u(6.0);
        dl.shadow(r, radius, pal.shadow, 10.0, 2.0);
        dl.rect(r, radius, pal.surface_raised);
        dl.stroke_rect(r, radius, pal.separator, 1.0);
        self.hits.push((r, Hit::PickerPanel));
        self.picker.rect = Some(r);
        let loc = self.place_loc.take();

        // Status: the filter kept after a toggle, and whether it is still counting.
        let inner = Rect::new(
            x + pad + self.u(8.0),
            top + pad,
            w - 2.0 * pad - self.u(16.0),
            status_h,
        );
        if status {
            let ic = self.u(12.0);
            let mut lead = 0.0;
            if !self.picker.sticky.is_empty() {
                let ir = self.flip(Rect::new(inner.x, inner.y + (status_h - ic) * 0.5, ic, ic));
                draw_icon(dl, Icon::Search, ir, pal.text_muted, 1.5, self.rtl);
                lead = ic + self.u(6.0);
                let sr = Rect::new(inner.x + lead, inner.y, inner.w * 0.6 - lead, status_h);
                let s = truncate(text, &self.picker.sticky, small, sr.w.max(0.0));
                text_at(text, dl, &s, small, self.flip(sr), pal.text_muted, self.rtl);
            }
            if self.picker.counting {
                let cr = Rect::new(inner.x + lead, inner.y, inner.w - lead, status_h);
                let s = truncate(text, &t!("gallery-facets-counting"), small, cr.w * 0.5);
                text_at(
                    text,
                    dl,
                    &s,
                    small,
                    self.flip(cr),
                    pal.text_muted,
                    !self.rtl,
                );
            }
        }

        // The operator buttons, and a line under them.
        if !ops.is_empty() {
            let on = value.as_ref().and_then(op_on);
            let (bw, bh) = (self.u(36.0), self.u(24.0));
            let by = top + pad + status_h + (ops_h - bh) * 0.5;
            let mut bx = x + pad + self.u(4.0);
            for &k in ops {
                let br = self.flip(Rect::new(bx, by, bw, bh));
                let hot = on == Some(k);
                if hot {
                    dl.rect(br, self.u(4.0), pal.accent_fill);
                } else {
                    if self.pointer.is_some_and(|p| br.contains(p)) {
                        dl.rect(br, self.u(4.0), pal.surface_hover);
                    }
                    dl.stroke_rect(br, self.u(4.0), pal.separator, 1.0);
                }
                let glyph = match k {
                    0 => OPS[0].1,
                    1 => op_glyph(Op::Ge, self.rtl),
                    _ => op_glyph(Op::Le, self.rtl),
                };
                let gw = text.measure(glyph, row_style);
                let gr = Rect::new(br.x + (br.w - gw) * 0.5, br.y, gw, bh);
                let color = if hot { pal.accent_text } else { pal.text };
                text_at(text, dl, glyph, row_style, gr, color, false);
                self.hits.push((br, Hit::PickerOp(k)));
                self.picker.ops.push((k, br));
                bx += bw + self.u(4.0);
            }
            dl.rect(
                self.flip(Rect::new(
                    x + 1.0,
                    top + pad + status_h + ops_h - 1.0,
                    w - 2.0,
                    1.0,
                )),
                0.0,
                pal.separator,
            );
        }

        // The rows in view.
        let body = Rect::new(x, top + pad + status_h + ops_h, w, list_h);
        let content = n as f32 * row_h;
        let max_scroll = (content - list_h).max(0.0);
        if let Some(a) = self.picker.active {
            let y = a as f32 * row_h;
            if y < self.picker.scroll {
                self.picker.scroll = y;
            } else if y + row_h > self.picker.scroll + list_h {
                self.picker.scroll = y + row_h - list_h;
            }
        }
        self.picker.scroll = self.picker.scroll.clamp(0.0, max_scroll);
        let bar_w = if max_scroll > 0.0 { self.u(8.0) } else { 0.0 };
        let i18n = I18n::global().localizer();
        let clip = self.flip(body);
        dl.push_clip(clip);
        let first = (self.picker.scroll / row_h).floor() as usize;
        let last = (((self.picker.scroll + list_h) / row_h).ceil() as usize).min(n);
        for k in first..last {
            let row = self.picker.rows[k];
            let y = body.y + k as f32 * row_h - self.picker.scroll;
            let lr = Rect::new(body.x + pad, y, body.w - 2.0 * pad - bar_w, row_h);
            let rr = self.flip(lr);
            let active = self.picker.active == Some(k);
            if active
                || self
                    .pointer
                    .is_some_and(|p| rr.contains(p) && clip.contains(p))
            {
                dl.rect(rr, self.u(4.0), pal.surface_hover);
            }
            if active {
                dl.stroke_rect(rr, self.u(4.0), pal.focus_ring, 1.5);
            }
            let bs = self.u(14.0);
            let text_x = lr.x + self.u(8.0) + bs + self.u(8.0);
            let (name, count, on) = match row {
                Row::All => (t!("gallery-select-all"), None, Some(self.picker.all_on)),
                Row::Value(i) => {
                    let Some((key, count)) = self.picker_value(i) else {
                        continue;
                    };
                    let on = value.as_ref().map(|v| v.plain() && v.has(key));
                    (
                        shown(f_of(&value), key, loc.as_ref(), true),
                        Some(i18n.integer(i64::from(count))),
                        on,
                    )
                }
                Row::Name(name, facet) => {
                    (format!("{name}:"), Some(label(facet, loc.as_ref())), None)
                }
            };
            // Check box (values of a term that takes several) or the comparison, name, count.
            let multi = value.as_ref().is_some_and(ValueCtx::plain);
            let cmp = self.picker.cmp.filter(|_| matches!(row, Row::Value(_)));
            let tx = if multi || cmp.is_some() {
                text_x
            } else {
                lr.x + self.u(8.0)
            };
            if let Some(op) = cmp {
                let glyph = op_glyph(op, self.rtl);
                let gw = text.measure(glyph, row_style);
                let gr = self.flip(Rect::new(
                    lr.x + self.u(8.0) + (bs - gw) * 0.5,
                    y,
                    gw,
                    row_h,
                ));
                text_at(text, dl, glyph, row_style, gr, pal.text_muted, false);
            }
            if multi && let Some(on) = on {
                let br = self.flip(Rect::new(
                    lr.x + self.u(8.0),
                    y + (row_h - bs) * 0.5,
                    bs,
                    bs,
                ));
                if on {
                    dl.rect(br, self.u(3.0), pal.accent_fill);
                    draw_icon(dl, Icon::Check, br.inset(1.0), pal.accent_text, 2.0, false);
                } else {
                    dl.stroke_rect(br, self.u(3.0), pal.text_muted, 1.2);
                }
            }
            let cw = count.as_ref().map_or(0.0, |c| text.measure(c, count_style));
            if let Some(c) = &count {
                let cr = self.flip(Rect::new(lr.right() - self.u(8.0) - cw, y, cw, row_h));
                text_at(text, dl, c, count_style, cr, pal.text_muted, !self.rtl);
            }
            let label_w = (lr.right() - self.u(16.0) - cw - tx).max(0.0);
            let s = truncate(text, &name, row_style, label_w);
            let tr = self.flip(Rect::new(tx, y, label_w, row_h));
            text_at(text, dl, &s, row_style, tr, pal.text, self.rtl);
            let hit = rr.intersect(&clip);
            self.hits.push((hit, Hit::PickerRow(k, false)));
            if multi {
                let bx = self
                    .flip(Rect::new(lr.x, y, text_x - lr.x, row_h))
                    .intersect(&clip);
                self.hits.push((bx, Hit::PickerRow(k, true)));
            }
            self.picker.shown.push((k, hit));
        }
        dl.pop_clip();

        // Scrollbar.
        if max_scroll > 0.0 {
            let track = Rect::new(
                body.right() - pad - bar_w,
                body.y + 2.0,
                bar_w,
                list_h - 4.0,
            );
            let th = (track.h * list_h / content).max(self.u(20.0)).min(track.h);
            let ty = track.y + (track.h - th) * (self.picker.scroll / max_scroll);
            let tr = self.flip(track);
            let thumb = self.flip(Rect::new(track.x + 2.0, ty, bar_w - 4.0, th));
            let hot = self.picker.drag.is_some() || self.pointer.is_some_and(|p| tr.contains(p));
            dl.rect(
                thumb,
                (bar_w - 4.0) * 0.5,
                if hot { pal.text_muted } else { pal.separator },
            );
            self.hits.push((tr, Hit::PickerScroll));
            self.picker.track = Some((tr, th, max_scroll));
        }

        if hint {
            let hr = Rect::new(inner.x, body.bottom(), inner.w, hint_h);
            dl.rect(
                self.flip(Rect::new(x + 1.0, body.bottom(), w - 2.0, 1.0)),
                0.0,
                pal.separator,
            );
            let s = truncate(text, &t!("gallery-picker-hint"), small, hr.w);
            text_at(text, dl, &s, small, self.flip(hr), pal.text_muted, self.rtl);
        }
        self.place_loc = loc;
    }

    /// The field's combo box attributes.
    pub(super) fn picker_field_a11y(&self, field: &mut Node) {
        field.set_has_popup(HasPopup::Listbox);
        let open = self.picker.rect.is_some();
        field.set_expanded(open);
        if open {
            let mut controls = vec![NodeId(ID_LIST)];
            if !self.picker.ops.is_empty() {
                controls.push(NodeId(ID_OPS));
            }
            field.set_controls(controls);
            if let Some(k) = self.picker.active {
                field.set_active_descendant(NodeId(ID_ROW + k as u64));
            }
        }
    }

    /// AccessKit nodes of the list: a list box of options, and a status while counting.
    pub(super) fn picker_a11y(
        &self,
        tree: &mut accesskit::TreeUpdate,
        bounds: &dyn Fn(Rect) -> accesskit::Rect,
    ) {
        let Some(r) = self.picker.rect else {
            return;
        };
        let value = match self.picker_ctx() {
            Some(Context::Value(v)) => Some(v),
            Some(Context::Name { .. }) => None,
            None => return,
        };
        let loc = self.place_loc.as_ref();
        let mut list = Node::new(Role::ListBox);
        if !self.picker.label.is_empty() {
            list.set_label(self.picker.label.as_str());
        }
        if value.as_ref().is_some_and(ValueCtx::plain) {
            list.set_multiselectable();
        }
        list.set_bounds(bounds(r));
        let total = self.picker.rows.len();
        // The rows in view, and the active one wherever it is.
        let mut rows: Vec<(usize, Option<Rect>)> = self
            .picker
            .shown
            .iter()
            .map(|&(k, rr)| (k, Some(rr)))
            .collect();
        if let Some(a) = self.picker.active
            && a < total
            && !rows.iter().any(|(k, _)| *k == a)
        {
            rows.push((a, None));
            rows.sort_by_key(|(k, _)| *k);
        }
        let mut ids = Vec::new();
        for (k, rr) in rows {
            let id = NodeId(ID_ROW + k as u64);
            let mut n = Node::new(Role::ListBoxOption);
            match self.picker.rows[k] {
                Row::All => {
                    n.set_label(t!("gallery-select-all"));
                    n.set_selected(self.picker.all_on);
                }
                Row::Value(i) => {
                    let Some((key, count)) = self.picker_value(i) else {
                        continue;
                    };
                    let facet = f_of(&value);
                    n.set_label(t!(
                        "gallery-facet-a11y-value",
                        facet = self.picker.label.as_str(),
                        value = compared(self.picker.cmp, shown(facet, key, loc, false)),
                        count = i64::from(count)
                    ));
                    n.set_selected(value.as_ref().is_some_and(|v| v.plain() && v.has(key)));
                }
                Row::Name(name, facet) => {
                    n.set_label(format!("{name}:"));
                    n.set_description(label(facet, loc));
                }
            }
            n.set_size_of_set(total);
            n.set_position_in_set(k + 1);
            if let Some(rr) = rr {
                n.set_bounds(bounds(rr));
            }
            n.add_action(Action::Click);
            n.add_action(Action::Focus);
            ids.push(id);
            tree.nodes.push((id, n));
        }
        list.set_children(ids);
        tree.nodes.push((NodeId(ID_LIST), list));
        // The operator buttons: a radio group.
        let ops = (!self.picker.ops.is_empty()).then(|| {
            let on = value.as_ref().and_then(op_on);
            let mut group = Node::new(Role::RadioGroup);
            group.set_label(t!("gallery-picker-ops"));
            let mut ids = Vec::new();
            for &(k, br) in &self.picker.ops {
                let id = NodeId(ID_OP + u64::from(k));
                let mut n = Node::new(Role::RadioButton);
                n.set_label(match k {
                    0 => t!("gallery-picker-op-eq"),
                    1 => t!("gallery-picker-op-ge"),
                    _ => t!("gallery-picker-op-le"),
                });
                n.set_toggled(if on == Some(k) {
                    accesskit::Toggled::True
                } else {
                    accesskit::Toggled::False
                });
                n.set_bounds(bounds(br));
                n.add_action(Action::Click);
                ids.push(id);
                tree.nodes.push((id, n));
            }
            group.set_children(ids);
            (NodeId(ID_OPS), group)
        });
        let status = self.picker.counting.then(|| {
            let mut n = Node::new(Role::Status);
            n.set_live(accesskit::Live::Polite);
            n.set_label(t!("gallery-facets-counting"));
            (NodeId(ID_STATUS), n)
        });
        if let Some((_, root)) = tree.nodes.iter_mut().find(|(i, _)| *i == NodeId(1)) {
            if let Some((id, _)) = &ops {
                root.push_child(*id);
            }
            root.push_child(NodeId(ID_LIST));
            if let Some((id, _)) = &status {
                root.push_child(*id);
            }
        }
        tree.nodes.extend(status);
        tree.nodes.extend(ops);
    }
}

/// The facet a value context lists (Property for none: never used then).
fn f_of(v: &Option<ValueCtx>) -> FacetField {
    v.as_ref().map_or(FacetField::Property, |v| v.facet)
}

impl App {
    /// AccessKit: an action on a row of the picker; false when not ours.
    pub(super) fn picker_a11y_action(&mut self, target: u64, click: bool) -> bool {
        if !(A11Y_PICKER..A11Y_PICKER + (1 << 32)).contains(&target) {
            return false;
        }
        let ga = &mut self.gallery;
        if (ID_OP..ID_OP + OPS.len() as u64).contains(&target) {
            if click {
                ga.picker_op((target - ID_OP) as u8);
            }
        } else if target >= ID_ROW {
            let k = (target - ID_ROW) as usize;
            if click {
                // Options of a list that takes several toggle; names and comparisons pick.
                let many = matches!(ga.picker_ctx(), Some(Context::Value(v)) if v.plain());
                ga.picker_click(k, many);
            } else if k < ga.picker.rows.len() {
                ga.picker.active = Some(k);
                ga.filter_focus = true;
            }
        }
        self.redraw = true;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use specular_query::facet::{Facet, Value};

    fn gallery(text: &str, facets: Vec<Facet>) -> Gallery {
        let mut g = Gallery {
            active: true,
            filter_open: true,
            filter_focus: true,
            ..Gallery::default()
        };
        g.filter.text = text.into();
        g.facets.facets = facets;
        g.facets.generation = 1;
        g
    }

    fn cameras() -> Vec<Facet> {
        vec![Facet {
            field: FacetField::Camera,
            values: ["X-T5", "Canon EOS R5", "iPhone 15", "Canon EOS 5D"]
                .iter()
                .enumerate()
                .map(|(k, c)| Value {
                    key: (*c).into(),
                    count: 10 - k as u32,
                })
                .collect(),
        }]
    }

    fn keys(g: &Gallery) -> Vec<String> {
        g.picker
            .rows
            .iter()
            .map(|r| match r {
                Row::All => "*".into(),
                Row::Value(i) => g.picker_value(*i).expect("value").0.to_owned(),
                Row::Name(n, _) => format!("{n}:"),
            })
            .collect()
    }

    const NONE: Mods = Mods {
        ctrl: false,
        shift: false,
        alt: false,
        meta: false,
    };
    const CTRL: Mods = Mods { ctrl: true, ..NONE };

    #[test]
    fn opens_on_a_field_and_filters_by_what_is_typed() {
        let mut g = gallery("camera:", cameras());
        assert!(g.picker_refresh().is_some());
        assert_eq!(
            keys(&g),
            ["X-T5", "Canon EOS R5", "iPhone 15", "Canon EOS 5D"]
        );
        g.filter.text = "camera:eos".into();
        g.picker_refresh();
        assert_eq!(keys(&g), ["*", "Canon EOS R5", "Canon EOS 5D"]);
        // Only while the field has the focus, and not for plain words or other fields.
        g.filter_focus = false;
        assert!(g.picker_refresh().is_none());
        g.filter_focus = true;
        g.filter.text = "sunset".into();
        assert!(g.picker_refresh().is_none());
        // Folders are the Library's; keywords the folder's photos have too.
        g.filter.text = "folder:".into();
        assert!(g.picker_ctx().is_none());
        g.filter.text = "tag:".into();
        assert!(g.picker_ctx().is_some());
        g.filter.text = "folder:".into();
        g.filter.scope = specular_query::Scope::Library;
        assert!(g.picker_ctx().is_some());
    }

    #[test]
    fn counts_leave_out_the_value_being_typed() {
        let g = gallery("dog camera=a,Can", cameras());
        assert_eq!(g.picker_count_text(), "dog camera=a,");
        let g = gallery("dog is:hd", cameras());
        assert_eq!(g.picker_count_text(), "dog ");
        let g = gallery("dog cam", cameras());
        assert_eq!(g.picker_count_text(), "dog ");
    }

    #[test]
    fn keys_pick_toggle_and_close() {
        let mut g = gallery("camera:can", cameras());
        // Down onto All, Down onto the first Canon; Enter picks it and closes.
        assert!(g.picker_key(NamedKey::ArrowDown, NONE));
        assert!(g.picker_key(NamedKey::ArrowDown, NONE));
        assert_eq!(g.picker.active, Some(1));
        assert!(g.picker_key(NamedKey::Enter, NONE));
        assert_eq!(g.filter.text, "camera=\"Canon EOS R5\"");
        assert!(g.picker_ctx().is_none());
        // A comma opens it again for another value; Space toggles several.
        g.filter.text.push(',');
        g.picker.typed();
        assert!(g.picker_refresh().is_some());
        assert_eq!(
            keys(&g),
            ["X-T5", "Canon EOS R5", "iPhone 15", "Canon EOS 5D"]
        );
        assert!(g.picker_key(NamedKey::ArrowDown, NONE));
        assert!(g.picker_key(NamedKey::Space, NONE));
        assert_eq!(g.filter.text, "camera=\"Canon EOS R5\",X-T5,");
        // The same row stays active; Ctrl+Enter takes it out again.
        g.picker_refresh();
        assert_eq!(g.picker.active, Some(0));
        assert!(g.picker_key(NamedKey::Enter, CTRL));
        assert_eq!(g.filter.text, "camera=\"Canon EOS R5\",");
        // Escape closes without the trailing comma; Alt+Down opens it again.
        assert!(g.picker_key(NamedKey::Escape, NONE));
        assert_eq!(g.filter.text, "camera=\"Canon EOS R5\"");
        assert!(g.picker_ctx().is_none());
        assert!(!g.picker_key(NamedKey::ArrowDown, NONE));
        let alt = Mods { alt: true, ..NONE };
        assert!(g.picker_key(NamedKey::ArrowDown, alt));
        assert!(g.picker_ctx().is_some());
        // Up from the first row goes back to the field (no active row).
        assert!(g.picker_key(NamedKey::ArrowDown, NONE));
        assert!(g.picker_key(NamedKey::ArrowUp, NONE));
        assert_eq!(g.picker.active, None);
        // Enter with no active row is the field's (on to the grid).
        assert!(!g.picker_key(NamedKey::Enter, NONE));
    }

    #[test]
    fn select_all_keeps_the_filter_and_clears_again() {
        let mut g = gallery("camera:eos", cameras());
        g.picker_refresh();
        g.picker_toggle(0);
        assert_eq!(g.filter.text, "camera=\"Canon EOS R5\",\"Canon EOS 5D\",");
        g.picker_refresh();
        // Still filtered by "eos", all on.
        assert_eq!(keys(&g), ["*", "Canon EOS R5", "Canon EOS 5D"]);
        assert!(g.picker.all_on);
        g.picker_toggle(0);
        assert_eq!(g.filter.text, "camera=");
        let q = specular_query::parse("camera=\"Canon EOS R5\",\"Canon EOS 5D\",");
        assert!(q.errors.is_empty());
    }

    #[test]
    fn names_complete_to_fields_with_values() {
        let mut g = gallery("dog ca", cameras());
        g.picker_refresh();
        assert_eq!(keys(&g), ["camera:"]);
        // Tab takes the first name.
        assert!(g.picker_key(NamedKey::Tab, NONE));
        assert_eq!(g.filter.text, "dog camera:");
        // A field without values is not offered.
        let mut g = gallery("le", cameras());
        g.picker_refresh();
        assert!(!g.picker.visible());
    }

    #[test]
    fn exif_lists_tag_names_then_their_words() {
        let facet = Facet {
            field: FacetField::Exif,
            values: [
                ("SceneCaptureType", 9),
                ("Contrast", 4),
                ("Contrast=soft", 1),
                ("SceneCaptureType=night", 2),
                ("SceneCaptureType=standard", 7),
            ]
            .iter()
            .map(|(k, c)| Value {
                key: (*k).into(),
                count: *c,
            })
            .collect(),
        };
        let mut g = gallery("exif:", vec![facet]);
        // The Library's only.
        assert!(g.picker_ctx().is_none());
        g.filter.scope = specular_query::Scope::Library;
        g.picker_refresh();
        assert_eq!(keys(&g), ["SceneCaptureType", "Contrast"]);
        g.filter.text = "exif:scene".into();
        g.picker_refresh();
        assert_eq!(keys(&g), ["SceneCaptureType"]);
        g.filter.text = "exif:SceneCaptureType=".into();
        g.picker_refresh();
        assert_eq!(
            keys(&g),
            ["*", "SceneCaptureType=night", "SceneCaptureType=standard"]
        );
        // Enter on a word picks it.
        g.picker.active = Some(1);
        assert!(g.picker_key(NamedKey::Enter, NONE));
        assert_eq!(g.filter.text, "exif:SceneCaptureType=night");
    }

    /// The tick of each value row (None for the All row and names).
    fn ticks(g: &Gallery) -> Vec<bool> {
        let Some(Context::Value(v)) = g.picker_ctx() else {
            return Vec::new();
        };
        g.picker
            .rows
            .iter()
            .filter_map(|r| match r {
                Row::Value(i) => Some(v.has(g.picker_value(*i).expect("value").0)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn the_term_under_the_caret_is_listed_and_rewritten() {
        let mut g = gallery("camera=X-T5, dog iso:100", cameras());
        // The caret after the comma: the camera term, with X-T5 ticked.
        g.set_filter_caret(12);
        assert!(g.picker_refresh().is_some());
        assert_eq!(
            keys(&g),
            ["X-T5", "Canon EOS R5", "iPhone 15", "Canon EOS 5D"]
        );
        assert_eq!(ticks(&g), [true, false, false, false]);
        assert_eq!(g.picker_count_text(), g.filter.text);
        // Space on iPhone toggles it into this term only; the caret stays at its end.
        assert!(g.picker_key(NamedKey::ArrowDown, NONE));
        assert!(g.picker_key(NamedKey::ArrowDown, NONE));
        assert!(g.picker_key(NamedKey::ArrowDown, NONE));
        assert!(g.picker_key(NamedKey::Space, NONE));
        assert_eq!(g.filter.text, "camera=X-T5,\"iPhone 15\", dog iso:100");
        assert_eq!(g.filter_caret(), 24);
        g.picker_refresh();
        assert_eq!(ticks(&g), [true, false, true, false]);
        // Typing at the caret filters the list; Enter picks into this term and closes it.
        g.filter_insert("eos");
        g.picker_refresh();
        assert_eq!(keys(&g), ["*", "Canon EOS R5", "Canon EOS 5D"]);
        assert!(g.picker_key(NamedKey::ArrowDown, NONE));
        assert!(g.picker_key(NamedKey::ArrowDown, NONE));
        assert!(g.picker_key(NamedKey::Enter, NONE));
        assert_eq!(
            g.filter.text,
            "camera=X-T5,\"iPhone 15\",\"Canon EOS R5\" dog iso:100"
        );
        assert!(g.picker_ctx().is_none());
        // Closed for that term only: the caret in another term opens it again.
        let iso = g.filter.text.len();
        g.facets.facets.push(Facet {
            field: FacetField::Iso,
            values: vec![Value {
                key: "100".into(),
                count: 3,
            }],
        });
        g.set_filter_caret(iso - 1);
        assert!(
            matches!(g.picker_refresh(), Some(Context::Value(v)) if v.facet == FacetField::Iso)
        );
        // Escape there tidies nothing else and closes it; End moves on to no term.
        assert!(g.picker_key(NamedKey::Escape, NONE));
        assert!(g.picker_ctx().is_none());
        assert!(g.filter.text.ends_with("dog iso:100"));
    }

    #[test]
    fn the_field_edits_at_its_caret() {
        let mut g = gallery("camera:x dog", cameras());
        assert_eq!(g.filter_caret(), 12);
        g.filter_edit(NamedKey::Home, false);
        assert_eq!(g.filter_caret(), 0);
        g.filter_edit(NamedKey::ArrowRight, true);
        let word = g.filter_caret();
        assert!(word > 0 && word <= 8, "{word}");
        g.set_filter_caret(8);
        g.filter_insert(",y");
        assert_eq!(g.filter.text, "camera:x,y dog");
        assert_eq!(g.filter_caret(), 10);
        g.filter_edit(NamedKey::ArrowLeft, false);
        g.filter_edit(NamedKey::Delete, false);
        assert_eq!(g.filter.text, "camera:x, dog");
        g.filter_edit(NamedKey::Backspace, false);
        assert_eq!(g.filter.text, "camera:x dog");
        assert_eq!(g.filter_caret(), 8);
        // Ctrl+Delete takes the next term, Ctrl+Backspace the one before.
        g.filter_edit(NamedKey::Delete, true);
        assert_eq!(g.filter.text, "camera:x");
        g.filter_edit(NamedKey::Backspace, true);
        assert_eq!(g.filter.text, "");
        // A change made elsewhere puts the caret back at the end.
        g.filter.text = "dog".into();
        g.set_filter_caret(1);
        g.filter.text = "cat dog".into();
        assert_eq!(g.filter_caret(), 7);
    }

    #[test]
    fn is_and_has_list_every_flag_word() {
        use specular_query::facet::FLAGS;
        let facets = vec![Facet {
            field: FacetField::Flags,
            values: FLAGS
                .iter()
                .enumerate()
                .map(|(k, f)| Value {
                    key: f.word().into(),
                    count: k as u32 % 3,
                })
                .collect(),
        }];
        let mut g = gallery("dog is:", facets);
        g.picker_refresh();
        assert_eq!(g.picker_count_text(), "dog ");
        let listed = keys(&g);
        assert!(listed.contains(&"alpha".to_owned()) && !listed.contains(&"hdr".to_owned()));
        // The list filters by the names as well as the words: "loc" finds gps (Location).
        g.filter.text = "dog has:loc".into();
        g.picker_refresh();
        assert_eq!(keys(&g), ["gps"]);
        assert!(g.picker_key(NamedKey::ArrowDown, NONE));
        assert!(g.picker_key(NamedKey::Enter, NONE));
        assert_eq!(g.filter.text, "dog has:gps");
        assert!(specular_query::parse(&g.filter.text).errors.is_empty());
        // Ticks follow aliases: is:picked ticks pick.
        g.filter.text = "is:picked,".into();
        g.picker_refresh();
        let k = keys(&g).iter().position(|k| k == "pick").expect("pick");
        assert!(ticks(&g)[k]);
    }

    #[test]
    fn values_with_quotes_are_inserted_escaped() {
        let lenses = vec![Facet {
            field: FacetField::Lens,
            values: ["Ai \"S\" 85", "C:\\lens"]
                .iter()
                .map(|l| Value {
                    key: (*l).into(),
                    count: 2,
                })
                .collect(),
        }];
        let mut g = gallery("lens: dog", lenses);
        g.set_filter_caret(5);
        g.picker_refresh();
        assert!(g.picker_key(NamedKey::ArrowDown, NONE));
        assert!(g.picker_key(NamedKey::Space, NONE));
        assert!(g.picker_key(NamedKey::ArrowDown, NONE));
        assert!(g.picker_key(NamedKey::Space, NONE));
        assert_eq!(g.filter.text, "lens=\"Ai \\\"S\\\" 85\",C:\\lens, dog");
        g.picker_refresh();
        assert_eq!(ticks(&g), [true, true]);
        let q = specular_query::parse(&g.filter.text);
        assert!(q.errors.is_empty());
        match &q.terms[0].kind {
            specular_query::Kind::Field { items, .. } => {
                assert_eq!(items, &["Ai \"S\" 85", "C:\\lens"]);
            }
            k => panic!("{k:?}"),
        }
    }

    #[test]
    fn a11y_is_a_combo_box_with_a_list_box() {
        let mut g = gallery("camera:", cameras());
        g.picker_refresh();
        g.picker.rect = Some(Rect::new(0.0, 0.0, 100.0, 100.0));
        let r = Rect::new(0.0, 0.0, 10.0, 10.0);
        g.picker.shown = vec![(0, r), (1, r)];
        g.picker.active = Some(1);
        let mut field = Node::new(Role::SearchInput);
        g.picker_field_a11y(&mut field);
        assert_eq!(field.is_expanded(), Some(true));
        assert_eq!(field.active_descendant(), Some(NodeId(ID_ROW + 1)));
        assert_eq!(field.controls(), &[NodeId(ID_LIST)]);
        let mut tree = accesskit::TreeUpdate {
            nodes: vec![(NodeId(1), Node::new(Role::Window))],
            tree: None,
            tree_id: accesskit::TreeId::ROOT,
            focus: NodeId(1),
        };
        g.picker_a11y(&mut tree, &|_| accesskit::Rect::default());
        let node = |id: u64| {
            tree.nodes
                .iter()
                .find(|(i, _)| *i == NodeId(id))
                .map(|(_, n)| n.clone())
                .expect("node")
        };
        let list = node(ID_LIST);
        assert_eq!(list.role(), Role::ListBox);
        assert!(list.is_multiselectable());
        assert_eq!(list.children().len(), 2);
        let opt = node(ID_ROW + 1);
        assert_eq!(opt.role(), Role::ListBoxOption);
        assert_eq!(opt.size_of_set(), Some(4));
        assert_eq!(opt.position_in_set(), Some(2));
        assert!(opt.label().is_some_and(|l| l.contains("Canon EOS R5")));
        assert_eq!(opt.is_selected(), Some(false));
    }

    /// The picker's counts of a few files of different sizes, dates and settings.
    fn numbers() -> Vec<Facet> {
        let day = |y, m, d| Some(specular_query::date::days_from_civil(y, m, d) * 86_400);
        let f = |w: u32, iso: u32, size: u64, taken| specular_query::Facts {
            name: "a.jpg",
            ext: "jpg",
            known: true,
            width: w,
            height: w * 2 / 3,
            iso: Some(iso),
            size: Some(size),
            taken,
            modified: day(2025, 1, 1),
            bit_depth: 8,
            pages: 1,
            ..specular_query::Facts::default()
        };
        let files = [
            f(6000, 100, 12_000_000, day(2024, 5, 1)),
            f(1920, 800, 900_000, day(2024, 7, 9)),
            f(4000, 800, 3_000_000, day(2023, 1, 2)),
            f(1280, 3200, 150_000, day(2023, 1, 30)),
        ];
        let q = specular_query::parse("");
        let m = specular_query::Matcher::new(&q, specular_query::ascii_fold);
        let mut t = specular_query::facet::Tally::default();
        for x in &files {
            t.add(x, m.fail_mask(x));
        }
        t.finish(&q)
    }

    /// The rows as (key, count).
    fn counts(g: &Gallery) -> Vec<(String, u32)> {
        g.picker
            .rows
            .iter()
            .filter_map(|r| match r {
                Row::Value(i) => g.picker_value(*i).map(|(k, c)| (k.to_owned(), c)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn number_fields_compare_with_the_operator_buttons() {
        let mut g = gallery("iso:", numbers());
        g.picker_refresh();
        // `:` lists each value with its own count; one picks (no check boxes).
        assert_eq!(
            counts(&g),
            [("100".into(), 1), ("800".into(), 2), ("3200".into(), 1)]
        );
        assert!(!g.picker.rows.contains(&Row::All));
        // ≥: each value counts the files at it and above, and the term becomes `iso>=`.
        g.picker_op(1);
        assert_eq!(g.filter.text, "iso>=");
        g.picker_refresh();
        assert_eq!(
            counts(&g),
            [("100".into(), 4), ("800".into(), 3), ("3200".into(), 1)]
        );
        // ≤ keeps what was typed; picking writes the comparison.
        g.filter.text = "dog iso>=8".into();
        g.set_filter_caret(g.filter.text.len());
        g.picker_op(2);
        assert_eq!(g.filter.text, "dog iso<=8");
        g.picker_refresh();
        assert_eq!(counts(&g), [("800".into(), 3)]);
        assert!(g.picker_key(NamedKey::ArrowDown, NONE));
        assert!(g.picker_key(NamedKey::Enter, NONE));
        assert_eq!(g.filter.text, "dog iso<=800");
        // = again.
        g.picker.dismissed = None;
        g.picker_op(0);
        assert_eq!(g.filter.text, "dog iso:800");
    }

    #[test]
    fn ladders_list_thresholds_and_dates_their_months() {
        let mut g = gallery("width:", numbers());
        g.picker_refresh();
        assert_eq!(
            counts(&g),
            [
                ("1280".into(), 4),
                ("1920".into(), 3),
                ("4000".into(), 2),
                ("6000".into(), 1)
            ]
        );
        assert_eq!(g.picker.ops.len(), 0, "not drawn yet");
        assert!(g.picker_key(NamedKey::ArrowDown, NONE));
        assert!(g.picker_key(NamedKey::ArrowDown, NONE));
        // Space picks too: a threshold is one value.
        assert!(g.picker_key(NamedKey::Space, NONE));
        assert_eq!(g.filter.text, "width>=1920");
        // Sizes by threshold, at most.
        let mut g = gallery("size<=", numbers());
        g.picker_refresh();
        assert_eq!(
            counts(&g),
            [
                ("500kb".into(), 1),
                ("1mb".into(), 2),
                ("5mb".into(), 3),
                ("20mb".into(), 4)
            ]
        );
        // Dates: years first; typing lists the months too, by name.
        let mut g = gallery("date:", numbers());
        g.picker_refresh();
        assert_eq!(counts(&g), [("2024".into(), 2), ("2023".into(), 2)]);
        g.filter.text = "date:2023".into();
        g.picker_refresh();
        assert_eq!(counts(&g), [("2023".into(), 2), ("2023-01".into(), 2)]);
        g.filter.text = "date:2024".into();
        g.picker_refresh();
        assert_eq!(
            counts(&g),
            [
                ("2024".into(), 2),
                ("2024-07".into(), 1),
                ("2024-05".into(), 1)
            ]
        );
        g.filter.text = "date>=2024-0".into();
        g.picker_refresh();
        assert_eq!(counts(&g), [("2024-07".into(), 1), ("2024-05".into(), 2)]);
        // Field names offer the new lists.
        let mut g = gallery("wid", numbers());
        g.picker_refresh();
        assert_eq!(keys(&g), ["width:"]);
    }

    #[test]
    fn a11y_operator_buttons_are_a_radio_group() {
        let mut g = gallery("iso>=", numbers());
        g.picker_refresh();
        g.picker.rect = Some(Rect::new(0.0, 0.0, 100.0, 100.0));
        let r = Rect::new(0.0, 0.0, 10.0, 10.0);
        g.picker.shown = vec![(0, r)];
        g.picker.ops = vec![(0, r), (1, r), (2, r)];
        let mut field = Node::new(Role::SearchInput);
        g.picker_field_a11y(&mut field);
        assert_eq!(field.controls(), &[NodeId(ID_LIST), NodeId(ID_OPS)]);
        let mut tree = accesskit::TreeUpdate {
            nodes: vec![(NodeId(1), Node::new(Role::Window))],
            tree: None,
            tree_id: accesskit::TreeId::ROOT,
            focus: NodeId(1),
        };
        g.picker_a11y(&mut tree, &|_| accesskit::Rect::default());
        let node = |id: u64| {
            tree.nodes
                .iter()
                .find(|(i, _)| *i == NodeId(id))
                .map(|(_, n)| n.clone())
                .expect("node")
        };
        assert_eq!(node(ID_OPS).role(), Role::RadioGroup);
        assert_eq!(node(ID_OPS).children().len(), 3);
        assert_eq!(node(ID_OP + 1).toggled(), Some(accesskit::Toggled::True));
        assert_eq!(node(ID_OP).toggled(), Some(accesskit::Toggled::False));
        // A compared value says what it selects; the list takes one.
        assert!(!node(ID_LIST).is_multiselectable());
        let opt = node(ID_ROW);
        assert!(
            opt.label().is_some_and(|l| l.contains("at least")),
            "{opt:?}"
        );
    }

    /// Opening and filtering a list of 100 000 values (`cargo test -- --ignored
    /// picker_bench --nocapture`).
    #[test]
    #[ignore]
    fn picker_bench() {
        let values: Vec<Value> = (0..100_000)
            .map(|k| Value {
                key: format!("/photos/{}/{:05} trip", 2000 + k % 25, k),
                count: 1 + (k % 7) as u32,
            })
            .collect();
        let facets = vec![Facet {
            field: FacetField::Folder,
            values,
        }];
        let mut g = gallery("folder:", facets);
        g.filter.scope = specular_query::Scope::Library;
        // The camera settings' localizer is made once per language.
        let t = std::time::Instant::now();
        g.picker_loc();
        let loc = t.elapsed();
        let t = std::time::Instant::now();
        g.picker_refresh();
        let open = t.elapsed();
        assert_eq!(g.picker.rows.len(), 100_000);
        g.filter.text = "folder:2017".into();
        let t = std::time::Instant::now();
        g.picker_refresh();
        let first = t.elapsed();
        g.filter.text = "folder:2017/0".into();
        let t = std::time::Instant::now();
        g.picker_refresh();
        let next = t.elapsed();
        // The caret in the middle of a longer query: the term under it is found and listed.
        g.filter.text = "dog folder:2017/0,\"/photos/2001/00001 trip\" camera:x rating>=2".into();
        g.set_filter_caret(17);
        let t = std::time::Instant::now();
        g.picker_refresh();
        let mid = t.elapsed();
        let mid_rows = g.picker.rows.len();
        println!(
            "picker 100k: localizer {:.2} ms (once), open {:.2} ms, first filter {:.2} ms, \
             next filter {:.2} ms ({} rows), caret mid-text {:.2} ms ({mid_rows} rows)",
            loc.as_secs_f64() * 1e3,
            open.as_secs_f64() * 1e3,
            first.as_secs_f64() * 1e3,
            next.as_secs_f64() * 1e3,
            g.picker.rows.len(),
            mid.as_secs_f64() * 1e3,
        );
        // The counts behind `is:` / `has:` (and every other list): one pass over 100 000
        // files' facts, as the facets pass or the Library's search makes it, then the lists.
        let names: Vec<String> = (0..100_000).map(|k| format!("IMG_{k:05}.JPG")).collect();
        let cams = ["X-T5", "Canon EOS R5", "iPhone 15", "Sony α7 IV"];
        let facts: Vec<specular_query::Facts> = names
            .iter()
            .enumerate()
            .map(|(k, name)| specular_query::Facts {
                name,
                ext: "jpg",
                known: true,
                width: if k % 3 == 0 { 3000 } else { 4000 },
                height: if k % 3 == 0 { 4000 } else { 3000 },
                rating: (k % 6) as u8,
                pick: k % 9 == 0,
                hdr: k % 7 == 0,
                camera: Some(cams[k % 4]),
                lens: (k % 2 == 0).then_some("XF 33mm"),
                iso: Some(100 << (k % 5)),
                gps: Some(k % 4 == 0),
                f_number: Some([1.4, 2.8, 5.6, 8.0][k % 4]),
                focal: Some([23.0, 33.0, 56.0][k % 3]),
                exposure: Some(1.0 / [60.0, 250.0, 1000.0][k % 3]),
                size: Some(2_000_000 + (k as u64 * 7919) % 30_000_000),
                taken: Some(1_500_000_000 + (k as i64) * 3_000),
                modified: Some(1_700_000_000 + (k as i64) * 60),
                bit_depth: [8, 8, 12, 16][k % 4],
                pages: 1 + (k % 50 == 0) as u32,
                space: Some(["srgb", "srgb", "p3", "adobergb", "ColorMatch RGB"][k % 5]),
                ..specular_query::Facts::default()
            })
            .collect();
        let q = specular_query::parse("is:hdr,");
        let m = specular_query::Matcher::new(&q, specular_query::ascii_fold);
        let t = std::time::Instant::now();
        let mut tally = specular_query::facet::Tally::default();
        for f in &facts {
            tally.add(f, m.fail_mask(f));
        }
        let lists = tally.finish(&q);
        let count = t.elapsed();
        let flags = lists
            .iter()
            .find(|f| f.field == FacetField::Flags)
            .map_or(0, |f| f.values.iter().filter(|v| v.count > 0).count());
        let mut g = gallery("is:hdr, dog", lists);
        g.set_filter_caret(7);
        let t = std::time::Instant::now();
        g.picker_refresh();
        let open = t.elapsed();
        println!(
            "is: over 100k files: counting {:.2} ms (off the frame, in slices), \
             open {:.3} ms ({flags} words with files, {} rows)",
            count.as_secs_f64() * 1e3,
            open.as_secs_f64() * 1e3,
            g.picker.rows.len(),
        );
        // The number fields: each opens on the same counts, compared for its operator.
        for text in [
            "iso>=",
            "f<=",
            "width:",
            "mp>=",
            "size<=",
            "date:",
            "date>=201",
            "space=",
        ] {
            g.filter.text = text.into();
            g.set_filter_caret(text.len());
            g.picker.dismissed = None;
            let t = std::time::Instant::now();
            g.picker_refresh();
            println!(
                "{text:10} open {:.3} ms ({} rows)",
                t.elapsed().as_secs_f64() * 1e3,
                g.picker.rows.len()
            );
        }
    }
}
