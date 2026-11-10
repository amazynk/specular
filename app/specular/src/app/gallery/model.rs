//! The gallery's model, free of the window: view options, filters, grouping, stacks and the
//! culling marks of each file. Everything here is plain data and unit-tested.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use specular_query::events::Shot;
use specular_query::{Facts, Matcher, Query, Span};
use specular_settings::{GalleryGroup, GalleryLayout, GallerySort, Settings};
use specular_thumbs::layout::{Layout, Mode};
use specular_thumbs::sort::SortKey;
use specular_thumbs::{Entry, EntryKind, Meta};

/// How one source is shown: layout, tile size, sort and grouping. Remembered per folder when
/// `gallery.per_folder_view` is on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ViewOpts {
    pub mode: Mode,
    /// Tile side (squares) or row height (justified), logical px.
    pub size: f32,
    pub sort: SortKey,
    pub desc: bool,
    pub group: GalleryGroup,
}

pub(crate) const MIN_SIZE: f32 = 64.0;
pub(crate) const MAX_SIZE: f32 = 512.0;

impl ViewOpts {
    pub fn from_settings(s: &Settings) -> ViewOpts {
        let g = &s.gallery;
        ViewOpts {
            mode: match g.layout {
                GalleryLayout::Squares => Mode::Squares,
                GalleryLayout::Justified => Mode::Justified,
                GalleryLayout::Uncropped => Mode::Uncropped,
                GalleryLayout::Details => Mode::Details,
            },
            size: (g.thumbnail_size as f32).clamp(MIN_SIZE, MAX_SIZE),
            sort: sort_key(g.sort_order),
            desc: g.sort_descending,
            group: g.group_by,
        }
    }

    /// One line of the per-folder table (`views.tsv`), without the path.
    pub fn encode(&self) -> String {
        format!(
            "{}\t{}\t{}\t{}\t{}",
            self.mode.id(),
            self.size.round() as i64,
            self.sort.id(),
            u8::from(self.desc),
            group_id(self.group)
        )
    }

    pub fn decode(fields: &[&str]) -> Option<ViewOpts> {
        let [mode, size, sort, desc, group] = fields else {
            return None;
        };
        Some(ViewOpts {
            mode: Mode::from_id(mode)?,
            size: size.parse::<f32>().ok()?.clamp(MIN_SIZE, MAX_SIZE),
            sort: SortKey::from_id(sort)?,
            desc: *desc == "1",
            group: group_from_id(group)?,
        })
    }
}

pub(crate) fn sort_key(s: GallerySort) -> SortKey {
    match s {
        GallerySort::Name => SortKey::Name,
        GallerySort::DateTaken => SortKey::DateTaken,
        GallerySort::Modified => SortKey::Modified,
        GallerySort::Size => SortKey::Size,
        GallerySort::Type => SortKey::Type,
        GallerySort::Dimensions => SortKey::Dimensions,
    }
}

pub(crate) fn gallery_sort(s: SortKey) -> GallerySort {
    match s {
        SortKey::Name => GallerySort::Name,
        SortKey::DateTaken => GallerySort::DateTaken,
        SortKey::Modified => GallerySort::Modified,
        SortKey::Size => GallerySort::Size,
        SortKey::Type => GallerySort::Type,
        SortKey::Dimensions => GallerySort::Dimensions,
    }
}

pub(crate) fn gallery_layout(m: Mode) -> GalleryLayout {
    match m {
        Mode::Squares => GalleryLayout::Squares,
        Mode::Justified => GalleryLayout::Justified,
        Mode::Uncropped => GalleryLayout::Uncropped,
        Mode::Details => GalleryLayout::Details,
    }
}

pub(crate) const GROUPS: [GalleryGroup; 11] = [
    GalleryGroup::None,
    GalleryGroup::Day,
    GalleryGroup::Month,
    GalleryGroup::Year,
    GalleryGroup::Event,
    GalleryGroup::Folder,
    GalleryGroup::Camera,
    GalleryGroup::Format,
    GalleryGroup::Rating,
    GalleryGroup::Target,
    GalleryGroup::Filter,
];

/// Whether a grouping reads the images' metadata (every image's is then read).
pub(crate) trait GroupNeeds {
    fn needs_meta(self) -> bool;
}

impl GroupNeeds for GalleryGroup {
    fn needs_meta(self) -> bool {
        matches!(
            self,
            GalleryGroup::Camera | GalleryGroup::Target | GalleryGroup::Filter
        )
    }
}

pub(crate) fn group_id(g: GalleryGroup) -> &'static str {
    match g {
        GalleryGroup::None => "none",
        GalleryGroup::Day => "day",
        GalleryGroup::Month => "month",
        GalleryGroup::Year => "year",
        GalleryGroup::Event => "event",
        GalleryGroup::Folder => "folder",
        GalleryGroup::Camera => "camera",
        GalleryGroup::Format => "format",
        GalleryGroup::Rating => "rating",
        GalleryGroup::Target => "target",
        GalleryGroup::Filter => "filter",
    }
}

pub(crate) fn group_from_id(s: &str) -> Option<GalleryGroup> {
    GROUPS.into_iter().find(|g| group_id(*g) == s)
}

// ----- Marks ------------------------------------------------------------------------------

/// Colour labels of keys 6–9 (Lightroom's names, written to `xmp:Label`).
pub(crate) const LABELS: [&str; 4] = ["Red", "Yellow", "Green", "Blue"];

/// Culling marks of one file.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Mark {
    /// Stars, 0–5.
    pub rating: u8,
    /// 0: none, 1–4: [`LABELS`].
    pub label: u8,
    /// 1: pick, -1: reject, 0: unflagged.
    pub flag: i8,
}

impl Mark {
    pub fn is_empty(&self) -> bool {
        *self == Mark::default()
    }
}

// ----- Filter -----------------------------------------------------------------------------

/// A removable chip of the filter bar: the span of a field term in the query text.
pub(crate) type Chip = Span;

/// The filter bar's query (DESIGN.md §10, `specular_query`): words match the name,
/// `field:value` terms the rest. The text is the whole state; quick filters, chips and facets
/// all edit it.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Filter {
    pub text: String,
    /// What the query can ask about: the Library's knows tags and folders too.
    pub scope: specular_query::Scope,
}

impl Filter {
    pub fn is_empty(&self) -> bool {
        self.text.trim().is_empty()
    }

    pub fn query(&self) -> Query {
        let mut q = specular_query::parse_in(&self.text, self.scope);
        if self.scope == specular_query::Scope::Library && crate::app::ai::semantic::ready() {
            // `like:` searches by description (the Library's search worker resolves it).
            let needs = specular_query::ErrorKind::NeedsAiPack(specular_query::AiFeature::Search);
            q.errors.retain(|e| e.kind != needs);
        }
        if self.scope == specular_query::Scope::Library && crate::app::ai::text_search::ready() {
            // `text:` searches the words read in photos (the Library's search worker).
            let needs = specular_query::ErrorKind::NeedsAiPack(specular_query::AiFeature::Text);
            q.errors.retain(|e| e.kind != needs);
        }
        q
    }

    pub fn matcher(&self) -> Matcher {
        Matcher::new(&self.query(), fold)
    }

    /// Whether the query has `dupes:` / `similar:` terms (which need the sets' roles).
    pub fn uses_sets(&self) -> bool {
        use specular_query::Field;
        self.query()
            .terms
            .iter()
            .any(|t| matches!(t.field(), Some(Field::Dupes | Field::Similar)))
    }

    fn term_text(&self, t: &specular_query::Term) -> &str {
        &self.text[t.span.start..t.span.end]
    }

    /// The field terms, as chips: (span, the term as typed).
    pub fn chips(&self) -> Vec<(Chip, String)> {
        self.query()
            .terms
            .iter()
            .filter(|t| t.field().is_some())
            .map(|t| (t.span, self.term_text(t).to_owned()))
            .collect()
    }

    pub fn remove(&mut self, c: Chip) {
        if c.start <= c.end
            && c.end <= self.text.len()
            && self.text.is_char_boundary(c.start)
            && self.text.is_char_boundary(c.end)
        {
            self.text = specular_query::facet::remove_span(&self.text, c);
        }
    }

    /// Whether the query has `term` as typed (ignoring ASCII case).
    pub fn has(&self, term: &str) -> bool {
        self.query()
            .terms
            .iter()
            .any(|t| self.term_text(t).eq_ignore_ascii_case(term))
    }

    /// Removes `term` when the query has it; otherwise adds it, first removing the terms that
    /// start with one of `family` (the alternatives it replaces: `rating>=` for `rating>=4`).
    pub fn toggle(&mut self, term: &str, family: &[&str]) {
        let q = self.query();
        let starts = |s: &str, p: &str| {
            s.len() >= p.len()
                && s.is_char_boundary(p.len())
                && s[..p.len()].eq_ignore_ascii_case(p)
        };
        let had = q
            .terms
            .iter()
            .any(|t| self.term_text(t).eq_ignore_ascii_case(term));
        let drop: Vec<Span> = q
            .terms
            .iter()
            .filter(|t| {
                let s = self.term_text(t);
                s.eq_ignore_ascii_case(term) || family.iter().any(|p| starts(s, p))
            })
            .map(|t| t.span)
            .collect();
        for s in drop.into_iter().rev() {
            self.text = specular_query::facet::remove_span(&self.text, s);
        }
        if !had {
            self.text = specular_query::facet::append(&self.text, term);
        }
    }
}

/// The query's fold: the app's search fold (case, accents, bidi isolates).
pub(crate) fn fold(s: &str) -> std::borrow::Cow<'_, str> {
    specular_i18n::fold_for_search(s)
}

/// What the query knows of an entry: listing, marks, metadata (when read) and its role in the
/// duplicate and similar sets.
pub(crate) fn facts<'a>(
    e: &'a Entry,
    meta: Option<&'a Meta>,
    mark: Mark,
    role: (Option<bool>, Option<bool>),
) -> Facts<'a> {
    let image = e.kind == EntryKind::Image;
    let mut f = Facts {
        name: &e.name,
        ext: e.path.extension().and_then(|x| x.to_str()).unwrap_or(""),
        container: !image,
        size: image.then_some(e.len),
        modified: modified_secs(e),
        rating: mark.rating,
        label: mark.label,
        pick: mark.flag == 1,
        reject: mark.flag == -1,
        taken: e.taken,
        dupes: role.0,
        similar: role.1,
        ..Facts::default()
    };
    if let Some(m) = meta {
        f.known = true;
        f.width = m.width;
        f.height = m.height;
        f.taken = f.taken.or(m.date_taken);
        f.camera = m.camera.as_deref();
        f.lens = m.lens.as_deref();
        f.iso = m.iso;
        f.f_number = m.f_number;
        f.focal = m.focal_length;
        f.exposure = m.exposure;
        f.animated = m.animated;
        f.hdr = m.hdr;
        f.alpha = m.has_alpha;
        f.raw = m.raw;
        f.profile = m.has_profile;
        f.pages = m.pages;
        f.bit_depth = m.bit_depth;
        f.space = m.space.as_deref();
        f.gps = m.gps;
        f.sound = m.video.then_some(m.sound);
        f.exif = m.exif.as_deref().map(specular_thumbs::ExifMeta::facts);
        if let Some(a) = &m.astro {
            f.subframe = true;
            f.object = a.object.as_deref();
            f.filter = a.filter.as_deref();
            f.frame_type = a.frame_type.as_deref();
            f.gain = a.gain;
            f.ccd_temp = a.ccd_temp;
            f.stars = a.metrics.map(|s| specular_query::StarFacts {
                count: s.stars,
                hfd: s.hfd,
                fwhm: s.fwhm,
                seeing: s.fwhm_arcsec(),
                ecc: s.eccentricity,
                background: s.background,
                snr: s.snr,
            });
        }
    }
    f
}

/// Case-insensitive prefix test (type to jump).
pub(crate) fn starts_with_folded(hay: &str, prefix: &str) -> bool {
    let h = specular_i18n::fold_for_search(hay);
    let p = specular_i18n::fold_for_search(prefix);
    h.starts_with(p.as_ref())
}

// ----- Groups -----------------------------------------------------------------------------

/// Proleptic Gregorian date of a day number (days since 1970-01-01; Hinnant's algorithm).
pub(crate) fn civil_from_days(z: i64) -> (i32, u8, u8) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u8;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u8;
    ((y + i64::from(m <= 2)) as i32, m, d)
}

/// Local-time seconds of a modification time (the UTC offset is not applied: dates of
/// captures are local already, and grouping needs no more than the day).
pub(crate) fn modified_secs(e: &Entry) -> Option<i64> {
    e.modified
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
}

/// When an entry was made, for date grouping: capture time, else modification time.
pub(crate) fn when(e: &Entry, meta: Option<&Meta>) -> Option<i64> {
    e.taken
        .or_else(|| meta.and_then(|m| m.date_taken))
        .or_else(|| modified_secs(e))
}

/// A group of the grouping: what sorts and compares, and what the header says.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum GroupKey {
    None,
    Date(i32, u8, u8),
    /// A shooting session: its first and last capture times (local Unix seconds), and where
    /// most of it was taken (microdegrees of latitude and longitude) when its photos say.
    Event(i64, i64, Option<(i32, i32)>),
    Text(String),
    Rating(u8),
    Unknown,
}

pub(crate) fn group_key(
    g: GalleryGroup,
    e: &Entry,
    meta: Option<&Meta>,
    mark: Mark,
    root: &Path,
) -> GroupKey {
    let date = |f: fn((i32, u8, u8)) -> GroupKey| {
        when(e, meta).map_or(GroupKey::Unknown, |s| {
            f(civil_from_days(s.div_euclid(86_400)))
        })
    };
    match g {
        GalleryGroup::None => GroupKey::None,
        GalleryGroup::Day => date(|(y, m, d)| GroupKey::Date(y, m, d)),
        GalleryGroup::Month => date(|(y, m, _)| GroupKey::Date(y, m, 0)),
        GalleryGroup::Year => date(|(y, _, _)| GroupKey::Date(y, 0, 0)),
        // Sessions need every time at once: see [`event_keys`].
        GalleryGroup::Event => {
            when(e, meta).map_or(GroupKey::Unknown, |t| GroupKey::Event(t, t, None))
        }
        GalleryGroup::Folder => {
            let parent = e.path.parent().unwrap_or(root);
            let rel = parent.strip_prefix(root).unwrap_or(parent);
            GroupKey::Text(rel.to_string_lossy().into_owned())
        }
        GalleryGroup::Camera => meta
            .and_then(|m| m.camera.clone())
            .filter(|c| !c.is_empty())
            .map_or(GroupKey::Unknown, GroupKey::Text),
        GalleryGroup::Format => {
            if e.kind == EntryKind::Image {
                GroupKey::Text(e.ext().to_ascii_uppercase())
            } else {
                GroupKey::Unknown
            }
        }
        GalleryGroup::Rating => GroupKey::Rating(mark.rating),
        // Astronomy subframes: by OBJECT or FILTER.
        GalleryGroup::Target | GalleryGroup::Filter => meta
            .and_then(|m| m.astro.as_ref())
            .and_then(|a| {
                if g == GalleryGroup::Target {
                    a.object.clone()
                } else {
                    a.filter.clone()
                }
            })
            .filter(|v| !v.trim().is_empty())
            .map_or(GroupKey::Unknown, GroupKey::Text),
    }
}

/// What event grouping knows of an entry: when it was made ([`when`]) and where, if the
/// metadata records a position (entries cached before positions were kept have none, and only
/// split on time).
pub(crate) fn shot(e: &Entry, meta: Option<&Meta>) -> Option<Shot> {
    when(e, meta).map(|time| Shot {
        time,
        pos: meta
            .and_then(|m| m.position)
            .map(|(lat, lon)| (f64::from(lat), f64::from(lon))),
    })
}

/// Event keys of files (`None`: time unknown): runs of times without a gap of more than
/// [`specular_query::events::GAP`], split where the place jumps (`specular_query::events`).
pub(crate) fn event_keys(shots: &[Option<Shot>]) -> Vec<GroupKey> {
    let mut known: Vec<(Shot, usize)> = shots
        .iter()
        .enumerate()
        .filter_map(|(i, s)| s.map(|s| (s, i)))
        .collect();
    known.sort_unstable_by_key(|k| (k.0.time, k.1));
    let sorted: Vec<Shot> = known.iter().map(|k| k.0).collect();
    let mut keys = vec![GroupKey::Unknown; shots.len()];
    for ev in specular_query::events::events(&sorted) {
        let r = ev.range;
        let micro = |d: f64| (d * 1e6).round() as i32;
        let key = GroupKey::Event(
            sorted[r.start].time,
            sorted[r.end - 1].time,
            ev.place.map(|(lat, lon)| (micro(lat), micro(lon))),
        );
        for &(_, i) in &known[r] {
            keys[i] = key.clone();
        }
    }
    keys
}

// ----- Stacks -----------------------------------------------------------------------------

const RAW_EXT: &[&str] = &[
    "cr2", "cr3", "crw", "nef", "nrw", "arw", "srf", "sr2", "raf", "orf", "rw2", "pef", "dng",
    "srw", "x3f", "3fr", "fff", "iiq", "erf", "kdc", "dcr", "mos", "mef", "mrw", "ari", "raw",
    "rwl", "gpr",
];

pub(crate) fn is_raw_ext(ext: &str) -> bool {
    RAW_EXT.contains(&ext.to_ascii_lowercase().as_str())
}

/// What burst detection needs of a file: capture time (Unix seconds) and camera.
pub(crate) type BurstMeta = (Option<i64>, Option<String>);

/// Collapses RAW+JPEG pairs (same folder and stem) and bursts (images taken less than a
/// second apart with the same camera) into stacks. `order` is the sorted list of indices
/// into `entries`; returns, in that order, each stack as (shown index, members). The shown
/// member is the JPEG of a pair (it decodes fastest) and the first of a burst.
pub(crate) fn stacks(
    entries: &[Entry],
    order: &[usize],
    meta: &dyn Fn(&Path) -> Option<BurstMeta>,
) -> Vec<(usize, Vec<usize>)> {
    // Pairs first: key by folder + lower-case stem.
    let mut by_stem: HashMap<(PathBuf, String), usize> = HashMap::new();
    let mut out: Vec<(usize, Vec<usize>)> = Vec::new();
    let mut slot_of: HashMap<usize, usize> = HashMap::new();
    for &i in order {
        let e = &entries[i];
        if e.kind != EntryKind::Image {
            out.push((i, vec![i]));
            continue;
        }
        let stem = e
            .path
            .file_stem()
            .map(|s| s.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        let key = (
            e.path.parent().map(Path::to_path_buf).unwrap_or_default(),
            stem,
        );
        match by_stem.get(&key) {
            Some(&slot) => {
                out[slot].1.push(i);
                // The non-RAW member is shown.
                if is_raw_ext(&entries[out[slot].0].ext()) && !is_raw_ext(&e.ext()) {
                    out[slot].0 = i;
                }
                slot_of.insert(i, slot);
            }
            None => {
                by_stem.insert(key, out.len());
                slot_of.insert(i, out.len());
                out.push((i, vec![i]));
            }
        }
    }
    // Bursts: consecutive (in the sorted order) stacks within a second, same camera.
    let mut merged: Vec<(usize, Vec<usize>)> = Vec::with_capacity(out.len());
    let mut last: Option<(i64, Option<String>)> = None;
    for (shown, members) in out {
        let e = &entries[shown];
        let info = (e.kind == EntryKind::Image)
            .then(|| meta(&e.path))
            .flatten()
            .and_then(|(t, cam)| e.taken.or(t).map(|t| (t, cam)));
        let joins = match (&last, &info) {
            (Some((t0, c0)), Some((t1, c1))) => (t1 - t0).abs() <= 1 && c0 == c1 && c1.is_some(),
            _ => false,
        };
        if joins && let Some(prev) = merged.last_mut() {
            prev.1.extend(members);
        } else {
            merged.push((shown, members));
        }
        last = info;
    }
    merged
}

// ----- Memory ------------------------------------------------------------------------------

/// The files whose thumbnail was refused for memory and would fit in `free` bytes of free
/// RAM (unknown: everything fits).
/// The memory a thumbnail was refused for, if that is why it failed.
pub(crate) fn needs_memory(e: &specular_decode::DecodeError) -> Option<specular_core::NeedsMemory> {
    match e {
        specular_decode::DecodeError::NeedsMemory(n) => Some(*n),
        _ => None,
    }
}

pub(crate) fn memory_ready(
    failed: &HashMap<PathBuf, Arc<specular_decode::DecodeError>>,
    free: Option<u64>,
) -> Vec<PathBuf> {
    failed
        .iter()
        .filter(|(_, e)| needs_memory(e).is_some_and(|n| free.is_none_or(|f| n.needed <= f)))
        .map(|(p, _)| p.clone())
        .collect()
}

// ----- Whole slides ------------------------------------------------------------------------

/// Each file of a DICOM whole-slide image stored as several files: its slide's number, and
/// whether it is the file the slide's tile shows (the first of its slide: the full-resolution
/// level).
pub(crate) fn slide_map(slides: Vec<Vec<PathBuf>>) -> HashMap<PathBuf, (u32, bool)> {
    let mut out = HashMap::new();
    for (n, files) in slides.into_iter().enumerate() {
        for (i, p) in files.into_iter().enumerate() {
            out.insert(p, (n as u32, i == 0));
        }
    }
    out
}

/// Merges the tiles (shown index, members) of one slide's files into one, at the place of
/// the first; it shows the slide's shown file when that is visible. Other tiles keep their
/// order.
pub(crate) fn merge_slides(
    groups: Vec<(usize, Vec<usize>)>,
    entries: &[Entry],
    slides: &HashMap<PathBuf, (u32, bool)>,
) -> Vec<(usize, Vec<usize>)> {
    if slides.is_empty() {
        return groups;
    }
    let mut out: Vec<(usize, Vec<usize>)> = Vec::with_capacity(groups.len());
    let mut slot: HashMap<u32, usize> = HashMap::new();
    for (shown, members) in groups {
        let Some(&(slide, main)) = slides.get(&entries[shown].path) else {
            out.push((shown, members));
            continue;
        };
        match slot.get(&slide) {
            Some(&s) => {
                out[s].1.extend(members);
                if main {
                    out[s].0 = shown;
                }
            }
            None => {
                slot.insert(slide, out.len());
                out.push((shown, members));
            }
        }
    }
    out
}

/// Below this the grid counts as scrolled to the top (logical px).
const AT_TOP: f32 = 0.5;

/// The scroll offset after the grid is laid out again (`None`: keep the current one, clamped).
/// `before` is the scroll offset (or its target, whichever is further) under the old layout;
/// `anchor` the tile that was under the anchor point and where in it, with the point's y in
/// the grid (`0.0`: the top of the view; more: the pointer). `rebuilt`: the tiles were sorted,
/// filtered or grouped again, not only resized.
///
/// At the top the grid stays at the top. Following the top tile is wrong there: when the
/// capture dates come in, a date or event grouping moves tiles between groups, and the tile
/// that happened to sit first (by name, or by file date) may now be in the last group; the
/// view would open scrolled to the end. Elsewhere, and when the tile size changes under the
/// pointer, the anchor tile keeps its place on screen.
pub(crate) fn relaid_scroll(
    layout: &Layout,
    before: f32,
    anchor: Option<((usize, f32), f32)>,
    view_h: f32,
    rebuilt: bool,
) -> Option<f32> {
    let pointer = anchor.is_some_and(|(_, py)| py > 0.0);
    if before <= AT_TOP && (rebuilt || !pointer) {
        return Some(0.0);
    }
    let (a, py) = anchor?;
    Some(layout.scroll_for_anchor(a, py, view_h))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A layout of `groups` (tiles per group) with group headers: 160 px squares, 800 px wide.
    fn grouped(groups: &[usize]) -> Layout {
        use specular_thumbs::layout::{Item, Params};
        let items: Vec<Item> = groups
            .iter()
            .enumerate()
            .flat_map(|(g, &n)| {
                std::iter::repeat_n(
                    Item {
                        aspect: 1.5,
                        group: g as u32,
                    },
                    n,
                )
            })
            .collect();
        Layout::build(&items, &Params::default(), Mode::Squares)
    }

    #[test]
    fn regrouping_at_the_top_stays_at_the_top() {
        let view_h = 600.0;
        // Before the capture dates: one group, the first tile (by name) at the top.
        let before = grouped(&[40]);
        let (tile, frac) = before.anchor(0.0, 0.0, 0.0).unwrap();
        assert_eq!(tile, 0);
        // The dates arrive: that file is the newest, now first in the last of four groups.
        let after = grouped(&[12, 12, 12, 4]);
        let moved = 36;
        // Following it would scroll to the end (the bug: grouped views opened at the bottom).
        let follow = after.scroll_for_anchor((moved, frac), 0.0, view_h);
        assert!(follow > after.height - view_h - 1.0, "{follow}");
        assert_eq!(
            relaid_scroll(&after, 0.0, Some(((moved, frac), 0.0)), view_h, true),
            Some(0.0)
        );
        // The same with the pointer resting over the grid as the anchor.
        let (ptile, pfrac) = before.anchor(0.0, 300.0, 200.0).unwrap();
        let pmoved = ptile + 24;
        assert_eq!(
            relaid_scroll(&after, 0.0, Some(((pmoved, pfrac), 200.0)), view_h, true),
            Some(0.0)
        );
        // Also when the tile stays first and only a header comes in above it (following it
        // would hide the first row under the sticky header).
        assert!(after.scroll_for_anchor((0, frac), 0.0, view_h) > 30.0);
        assert_eq!(
            relaid_scroll(&after, 0.0, Some(((0, frac), 0.0)), view_h, true),
            Some(0.0)
        );
        assert_eq!(relaid_scroll(&after, 0.3, None, view_h, true), Some(0.0));
        // A resize anchored at the top (window, aspect ratios) stays there too.
        assert_eq!(
            relaid_scroll(&after, 0.0, Some(((0, frac), 0.0)), view_h, false),
            Some(0.0)
        );
    }

    #[test]
    fn zooming_at_the_top_keeps_the_tile_under_the_pointer() {
        let view_h = 600.0;
        let small = grouped(&[40]);
        let (tile, frac) = small.anchor(0.0, 300.0, 450.0).unwrap();
        let big = Layout::build(
            &(0..40)
                .map(|_| specular_thumbs::layout::Item {
                    aspect: 1.5,
                    group: 0,
                })
                .collect::<Vec<_>>(),
            &specular_thumbs::layout::Params {
                tile: 320.0,
                ..Default::default()
            },
            Mode::Squares,
        );
        let s = relaid_scroll(&big, 0.0, Some(((tile, frac), 450.0)), view_h, false).unwrap();
        let c = big.cells[tile];
        assert!((c.y + frac * c.h - s - 450.0).abs() < 0.01, "{s}");
    }

    #[test]
    fn regrouping_further_down_keeps_the_anchor_tile_in_place() {
        let view_h = 600.0;
        let before = grouped(&[40]);
        let (scroll, py) = (700.0, 120.0);
        let (tile, frac) = before.anchor(scroll, 300.0, py).unwrap();
        let after = grouped(&[12, 12, 12, 4]);
        let s = relaid_scroll(&after, scroll, Some(((tile, frac), py)), view_h, true).unwrap();
        // The tile is under the same screen y as before.
        let c = after.cells[tile];
        assert!((c.y + frac * c.h - s - py).abs() < 0.01, "{s}");
        assert!(s > 0.0 && s <= after.height - view_h);
        // No anchor (nothing was laid out before): keep the offset.
        assert_eq!(relaid_scroll(&after, scroll, None, view_h, true), None);
    }

    fn entry(path: &str, kind: EntryKind) -> Entry {
        Entry {
            path: PathBuf::from(path),
            name: Path::new(path)
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned(),
            kind,
            len: 0,
            modified: None,
            hidden: false,
            taken: None,
            dims: None,
        }
    }

    #[test]
    fn civil_dates_round_trip() {
        for (y, m, d) in [
            (1970, 1, 1),
            (2000, 2, 29),
            (2024, 3, 12),
            (1969, 12, 31),
            (2100, 12, 31),
        ] {
            let days = specular_thumbs::generate::days_from_civil(y, m, d);
            assert_eq!(civil_from_days(days), (y as i32, m as u8, d as u8));
        }
    }

    #[test]
    fn view_options_round_trip() {
        let v = ViewOpts {
            mode: Mode::Justified,
            size: 200.0,
            sort: SortKey::DateTaken,
            desc: true,
            group: GalleryGroup::Month,
        };
        let line = v.encode();
        let fields: Vec<&str> = line.split('\t').collect();
        assert_eq!(ViewOpts::decode(&fields), Some(v));
        assert_eq!(ViewOpts::decode(&["bogus"]), None);
    }

    fn passes(f: &Filter, e: &Entry, meta: Option<&Meta>, mark: Mark) -> bool {
        f.matcher().matches(&facts(e, meta, mark, (None, None)))
    }

    #[test]
    fn filter_text_ratings_and_chips() {
        let e = entry("/p/Holiday_001.JPG", EntryKind::Image);
        let mut f = Filter::default();
        assert!(f.is_empty());
        f.text = "holiday".into();
        assert!(passes(&f, &e, None, Mark::default()));
        f.text = "work".into();
        assert!(!passes(&f, &e, None, Mark::default()));
        f.text = "rating>=3".into();
        let stars = |rating| Mark {
            rating,
            ..Mark::default()
        };
        assert!(!passes(&f, &e, None, stars(2)));
        assert!(passes(&f, &e, None, stars(4)));
        let chips = f.chips();
        assert_eq!(chips.len(), 1);
        assert_eq!(chips[0].1, "rating>=3");
        f.remove(chips[0].0);
        assert!(f.is_empty());
        f.toggle("is:pick", &[]);
        assert_eq!(f.text, "is:pick");
        assert!(!passes(&f, &e, None, Mark::default()));
        let pick = Mark {
            flag: 1,
            ..Mark::default()
        };
        assert!(passes(&f, &e, None, pick));
        f.toggle("is:pick", &[]);
        assert!(f.is_empty());
        // Quick filters of one family replace each other.
        f.toggle("rating>=2", &["rating>="]);
        f.toggle("rating>=4", &["rating>="]);
        assert_eq!(f.text, "rating>=4");
        assert!(f.has("RATING>=4"));
        f.text = "ext:jpeg".into();
        assert!(passes(&f, &e, None, Mark::default()));
        f.text = "ext:png".into();
        assert!(!passes(&f, &e, None, Mark::default()));
        // Folders only answer to the name text.
        let d = entry("/p/Holidays", EntryKind::Folder);
        assert!(!passes(&f, &d, None, Mark::default()));
        f.text = "holi".into();
        assert!(passes(&f, &d, None, Mark::default()));
    }

    #[test]
    fn unknown_metadata_passes_until_known() {
        let e = entry("/p/a.png", EntryKind::Image);
        let f = Filter {
            text: "is:animated".into(),
            ..Filter::default()
        };
        assert!(passes(&f, &e, None, Mark::default()));
        let still = Meta::default();
        assert!(!passes(&f, &e, Some(&still), Mark::default()));
    }

    #[test]
    fn events_split_at_gaps_of_more_than_three_hours() {
        let h = 3600;
        let at = |t: i64| Some(Shot { time: t, pos: None });
        let shots = [at(10 * h), None, at(0), at(2 * h), at(5 * h + 1), at(7 * h)];
        let k = event_keys(&shots);
        assert_eq!(k[2], GroupKey::Event(0, 2 * h, None));
        assert_eq!(k[3], k[2]);
        assert_eq!(k[4], GroupKey::Event(5 * h + 1, 10 * h, None));
        assert_eq!(k[5], k[4]);
        assert_eq!(k[0], k[4]);
        assert_eq!(k[1], GroupKey::Unknown);
        assert!(event_keys(&[]).is_empty());
    }

    #[test]
    fn events_split_on_place_jumps_from_cached_positions() {
        // Made-up places 800 km apart, an hour between them; one entry has no position (an
        // older cache entry) and one has no metadata yet.
        let h = 3600;
        let meta = |pos: Option<(f32, f32)>| Meta {
            gps: Some(true),
            position: pos,
            ..Meta::default()
        };
        let mut e = entry("/p/a.jpg", EntryKind::Image);
        let shot_at = |t: i64, m: Option<&Meta>| {
            let mut e = e.clone();
            e.taken = Some(t);
            shot(&e, m)
        };
        let (a, b, old) = (
            meta(Some((45.0, 10.0))),
            meta(Some((45.0, 20.5))),
            meta(None),
        );
        let shots = [
            shot_at(0, Some(&a)),
            shot_at(600, Some(&old)),
            shot_at(1200, Some(&a)),
            shot_at(2 * h, Some(&b)),
            shot_at(2 * h + 600, None),
            shot_at(2 * h + 1200, Some(&b)),
        ];
        let k = event_keys(&shots);
        assert_eq!(
            k[0],
            GroupKey::Event(0, 1200, Some((45_000_000, 10_000_000)))
        );
        assert_eq!(k[1], k[0]);
        assert_eq!(k[2], k[0]);
        assert_eq!(
            k[3],
            GroupKey::Event(2 * h, 2 * h + 1200, Some((45_000_000, 20_500_000)))
        );
        assert_eq!(k[4], k[3]);
        assert_eq!(k[5], k[3]);
        // Nothing known about the time: no shot.
        e.modified = None;
        e.taken = None;
        assert!(shot(&e, Some(&a)).is_none());
    }

    #[test]
    fn groups_by_date_folder_and_rating() {
        let mut e = entry("/root/2024/a.jpg", EntryKind::Image);
        e.taken = Some(specular_thumbs::generate::days_from_civil(2024, 3, 12) * 86_400 + 3600);
        let root = Path::new("/root");
        let m = Mark::default();
        assert_eq!(
            group_key(GalleryGroup::Day, &e, None, m, root),
            GroupKey::Date(2024, 3, 12)
        );
        assert_eq!(
            group_key(GalleryGroup::Month, &e, None, m, root),
            GroupKey::Date(2024, 3, 0)
        );
        assert_eq!(
            group_key(GalleryGroup::Year, &e, None, m, root),
            GroupKey::Date(2024, 0, 0)
        );
        assert_eq!(
            group_key(GalleryGroup::Folder, &e, None, m, root),
            GroupKey::Text("2024".into())
        );
        assert_eq!(
            group_key(GalleryGroup::Format, &e, None, m, root),
            GroupKey::Text("JPG".into())
        );
        assert_eq!(
            group_key(
                GalleryGroup::Rating,
                &e,
                None,
                Mark { rating: 4, ..m },
                root
            ),
            GroupKey::Rating(4)
        );
        assert_eq!(
            group_key(GalleryGroup::Camera, &e, None, m, root),
            GroupKey::Unknown
        );
    }

    #[test]
    fn stacks_pair_raw_with_jpeg_and_merge_bursts() {
        let mut entries = vec![
            entry("/p/IMG_1.CR3", EntryKind::Image),
            entry("/p/IMG_1.JPG", EntryKind::Image),
            entry("/p/IMG_2.JPG", EntryKind::Image),
            entry("/p/IMG_3.JPG", EntryKind::Image),
            entry("/p/IMG_9.JPG", EntryKind::Image),
            entry("/p/sub", EntryKind::Folder),
        ];
        entries[2].taken = Some(100);
        entries[3].taken = Some(101);
        entries[4].taken = Some(200);
        let order: Vec<usize> = (0..entries.len()).collect();
        let cam = |_: &Path| Some((None, Some("EOS R5".to_owned())));
        let s = stacks(&entries, &order, &cam);
        // The pair shows its JPEG; 2 and 3 are a burst; 9 is alone; the folder stays.
        assert_eq!(s.len(), 4, "{s:?}");
        assert_eq!(s[0], (1, vec![0, 1]));
        assert_eq!(s[1], (2, vec![2, 3]));
        assert_eq!(s[2], (4, vec![4]));
        assert_eq!(s[3], (5, vec![5]));
        // No camera known: no bursts.
        let none = |_: &Path| None;
        assert_eq!(stacks(&entries, &order, &none).len(), 5);
    }

    /// Thumbnails refused for memory come back once what they need is free; other failures
    /// stay failed.
    #[test]
    fn memory_refusals_retry_when_free() {
        use specular_decode::DecodeError;
        let need = |n: u64| {
            Arc::new(DecodeError::NeedsMemory(specular_core::NeedsMemory {
                needed: n,
                free: 0,
            }))
        };
        let failed: HashMap<PathBuf, Arc<DecodeError>> = [
            (PathBuf::from("/a.tif"), need(100)),
            (PathBuf::from("/b.tif"), need(300)),
            (
                PathBuf::from("/broken.jpg"),
                Arc::new(DecodeError::TooLarge),
            ),
        ]
        .into_iter()
        .collect();
        assert!(memory_ready(&failed, Some(50)).is_empty());
        assert_eq!(memory_ready(&failed, Some(200)), [PathBuf::from("/a.tif")]);
        let mut all = memory_ready(&failed, None);
        all.sort();
        assert_eq!(all, [PathBuf::from("/a.tif"), PathBuf::from("/b.tif")]);
    }

    /// A DICOM whole slide's files are one tile at the first file's place, showing the
    /// full-resolution level; other files and other slides keep their tiles.
    #[test]
    fn whole_slide_files_are_one_tile() {
        let entries = vec![
            entry("/s/a_label.dcm", EntryKind::Image),
            entry("/s/a_l0.dcm", EntryKind::Image),
            entry("/s/ct.dcm", EntryKind::Image),
            entry("/s/a_l1.dcm", EntryKind::Image),
            entry("/s/b_l1.dcm", EntryKind::Image),
            entry("/s/b_l0.dcm", EntryKind::Image),
        ];
        let p = |s: &str| PathBuf::from(s);
        let slides = slide_map(vec![
            vec![p("/s/a_l0.dcm"), p("/s/a_l1.dcm"), p("/s/a_label.dcm")],
            vec![p("/s/b_l0.dcm"), p("/s/b_l1.dcm")],
        ]);
        let singles: Vec<(usize, Vec<usize>)> = (0..entries.len()).map(|i| (i, vec![i])).collect();
        let m = merge_slides(singles.clone(), &entries, &slides);
        assert_eq!(m, vec![(1, vec![0, 1, 3]), (2, vec![2]), (5, vec![4, 5])]);
        // Filtered out, the main file leaves the tile to the first one shown.
        let some: Vec<_> = singles.iter().filter(|g| g.0 != 1).cloned().collect();
        assert_eq!(merge_slides(some, &entries, &slides)[0], (0, vec![0, 3]));
        // No slides: unchanged.
        assert_eq!(
            merge_slides(singles.clone(), &entries, &HashMap::new()),
            singles
        );
    }

    #[test]
    fn folded_matching() {
        assert!(passes(
            &Filter {
                text: "ete".into(),
                ..Filter::default()
            },
            &entry("/p/Été à Paris.jpg", EntryKind::Image),
            None,
            Mark::default()
        ));
        assert!(starts_with_folded("Zürich.png", "zu"));
        assert!(!starts_with_folded("a-zu.png", "zu"));
    }
}
