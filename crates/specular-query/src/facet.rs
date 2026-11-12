//! Facet counts: for each facet, how many files have each value among those the rest of the
//! query lets through (a facet's own selection is left out of its counts, so picking a second
//! camera shows how many it adds), and the query edits that select or clear a value.

use std::collections::{BTreeMap, HashMap};

use crate::parse::{self, Field, Flag, Kind, Op, Orient, Pred, Query, Span};
use crate::{Facts, canon_ext};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum FacetField {
    Format,
    Camera,
    Lens,
    Rating,
    Year,
    Orientation,
    Label,
    /// HDR, transparency, location, RAW, animation, pages: each its own `is:` / `has:` term.
    Property,
    // Counted for the search field's value picker only (the Facets panel lists `ALL`).
    Iso,
    Aperture,
    Focal,
    Exposure,
    /// The Library's tags and keywords (one value per tag).
    Tag,
    /// The Library's folders.
    Folder,
    /// Every word of `is:` and `has:` ([`FLAGS`]): the marks, the properties, the
    /// orientations and the metadata a file has. The panel lists [`FacetField::Property`].
    Flags,
    /// Bit depth and page count: their values (`8`, `16`; `1`, `3`).
    Bits,
    Pages,
    /// Width, height, megapixels and file size: counted against a ladder of thresholds
    /// ([`FacetField::ladder`]), which the picker offers as `>=` / `<=` terms.
    Width,
    Height,
    Megapixels,
    Size,
    /// The `date:` picker's years and months (`2024`, `2024-05`): capture dates, else
    /// modification dates, as [`FacetField::Year`] counts them.
    Taken,
    /// Modification dates: years and months.
    Modified,
    /// `dupes:` and `similar:`: `any` member, the suggested keepers (`keep`), the rest
    /// (`discard`).
    Dupes,
    Similar,
    /// Colour spaces: the named ones ([`parse::SPACE_WORDS`]) and profile descriptions.
    Space,
    /// The Library's places: the town each photo was taken in.
    Place,
    /// Astronomy subframes: targets, filters and frame kinds.
    Object,
    Filter,
    FrameType,
    /// Develop: edited or not, the modules edits use (stable ids), the styles applied, and
    /// versions ([`parse::VERSION_WORDS`]). Listed only where a file has an edit.
    Edited,
    Module,
    Style,
    Version,
    /// Exif: the maker, software, artist, copyright and serial numbers (by count).
    Make,
    Software,
    Artist,
    Copyright,
    Serial,
    LensSerial,
    /// Exif words: [`crate::exif::FLASH_WORDS`], [`crate::exif::WB_WORDS`],
    /// [`crate::exif::PROGRAM_WORDS`], [`crate::exif::METERING_WORDS`].
    Flash,
    WhiteBalance,
    Program,
    Metering,
    /// Exposure compensation (to a tenth of a stop) and the 35 mm focal length, ascending.
    ExpComp,
    Focal35,
    /// IPTC: credit line, source, event and job ID (by count; the Library's).
    Credit,
    Source,
    Event,
    JobId,
    /// The Library's Exif tags: each tag's name (the files that have it), and `Name=word`
    /// for the words of enumerated tags.
    Exif,
}

// Each facet has a bit of [`crate::Matcher::fail_mask`] below its `OTHER`.
const _: () = assert!((FacetField::Exif as u32) < 63);

/// The thresholds of the ladder facets: (query value, its number as the parser reads it).
const LADDER_PX: [(&str, f64); 16] = [
    ("640", 640.0),
    ("800", 800.0),
    ("1024", 1024.0),
    ("1280", 1280.0),
    ("1600", 1600.0),
    ("1920", 1920.0),
    ("2048", 2048.0),
    ("2560", 2560.0),
    ("3000", 3000.0),
    ("3840", 3840.0),
    ("4000", 4000.0),
    ("5000", 5000.0),
    ("6000", 6000.0),
    ("7680", 7680.0),
    ("8000", 8000.0),
    ("10000", 10000.0),
];
const LADDER_MP: [(&str, f64); 15] = [
    ("0.5", 0.5),
    ("1", 1.0),
    ("2", 2.0),
    ("4", 4.0),
    ("8", 8.0),
    ("12", 12.0),
    ("16", 16.0),
    ("20", 20.0),
    ("24", 24.0),
    ("30", 30.0),
    ("36", 36.0),
    ("45", 45.0),
    ("50", 50.0),
    ("60", 60.0),
    ("100", 100.0),
];
const LADDER_SIZE: [(&str, f64); 12] = [
    ("100kb", 1e5),
    ("500kb", 5e5),
    ("1mb", 1e6),
    ("2mb", 2e6),
    ("5mb", 5e6),
    ("10mb", 1e7),
    ("20mb", 2e7),
    ("50mb", 5e7),
    ("100mb", 1e8),
    ("200mb", 2e8),
    ("500mb", 5e8),
    ("1gb", 1e9),
];

/// `dupes:` / `similar:` values, in order.
pub const GROUP_WORDS: [&str; 3] = ["any", "keep", "discard"];

/// The properties the Property facet lists, in order.
pub const PROPERTIES: [Flag; 8] = [
    Flag::Hdr,
    Flag::Alpha,
    Flag::Gps,
    Flag::Raw,
    Flag::Animated,
    Flag::MultiPage,
    Flag::Video,
    Flag::Sound,
];

/// The words the search field's `is:` / `has:` picker lists, in order: the panel's
/// properties first, then the rest of the vocabulary (not `folder` and `image`: the counts are
/// of images only).
pub const FLAGS: [Flag; 21] = [
    Flag::Hdr,
    Flag::Alpha,
    Flag::Gps,
    Flag::Raw,
    Flag::Animated,
    Flag::MultiPage,
    Flag::Video,
    Flag::Sound,
    Flag::Profile,
    Flag::Camera,
    Flag::Lens,
    Flag::Date,
    Flag::Pick,
    Flag::Reject,
    Flag::Flagged,
    Flag::Rated,
    Flag::Unrated,
    Flag::Labelled,
    Flag::Landscape,
    Flag::Portrait,
    Flag::Square,
];

impl FacetField {
    pub const ALL: [FacetField; 10] = [
        FacetField::Format,
        FacetField::Camera,
        FacetField::Lens,
        FacetField::Rating,
        FacetField::Year,
        FacetField::Orientation,
        FacetField::Label,
        FacetField::Property,
        FacetField::Edited,
        FacetField::Version,
    ];

    /// The facets only the search field's value picker lists (numbers in ascending order,
    /// tags and folders by count, `is:` / `has:` words in [`FLAGS`] order, dates newest
    /// first, the ladders as [`Tally::finish`] lays them out, colour spaces and places by
    /// count).
    pub const PICKER: [FacetField; 41] = [
        FacetField::Iso,
        FacetField::Aperture,
        FacetField::Focal,
        FacetField::Exposure,
        FacetField::Tag,
        FacetField::Folder,
        FacetField::Flags,
        FacetField::Bits,
        FacetField::Pages,
        FacetField::Width,
        FacetField::Height,
        FacetField::Megapixels,
        FacetField::Size,
        FacetField::Taken,
        FacetField::Modified,
        FacetField::Dupes,
        FacetField::Similar,
        FacetField::Space,
        FacetField::Place,
        FacetField::Object,
        FacetField::Filter,
        FacetField::FrameType,
        FacetField::Module,
        FacetField::Style,
        FacetField::Make,
        FacetField::Software,
        FacetField::Artist,
        FacetField::Copyright,
        FacetField::Serial,
        FacetField::LensSerial,
        FacetField::Flash,
        FacetField::WhiteBalance,
        FacetField::Program,
        FacetField::Metering,
        FacetField::ExpComp,
        FacetField::Focal35,
        FacetField::Credit,
        FacetField::Source,
        FacetField::Event,
        FacetField::JobId,
        FacetField::Exif,
    ];

    pub fn bit(self) -> u64 {
        1 << self as u32
    }

    /// The facet whose selection a term on `field` is. Property terms are plain conditions
    /// (they AND), so they count as the rest of the query.
    pub fn of(field: Field) -> Option<FacetField> {
        Some(match field {
            Field::Ext => FacetField::Format,
            Field::Camera => FacetField::Camera,
            Field::Lens => FacetField::Lens,
            Field::Rating => FacetField::Rating,
            Field::Date => FacetField::Year,
            Field::Orientation => FacetField::Orientation,
            Field::Label => FacetField::Label,
            Field::Iso => FacetField::Iso,
            Field::Aperture => FacetField::Aperture,
            Field::Focal => FacetField::Focal,
            Field::Exposure => FacetField::Exposure,
            Field::Tag => FacetField::Tag,
            Field::Folder => FacetField::Folder,
            Field::Bits => FacetField::Bits,
            Field::Pages => FacetField::Pages,
            Field::Width => FacetField::Width,
            Field::Height => FacetField::Height,
            Field::Megapixels => FacetField::Megapixels,
            Field::Size => FacetField::Size,
            Field::Modified => FacetField::Modified,
            Field::Dupes => FacetField::Dupes,
            Field::Similar => FacetField::Similar,
            Field::Space => FacetField::Space,
            Field::Place => FacetField::Place,
            Field::Object => FacetField::Object,
            Field::Filter => FacetField::Filter,
            Field::FrameType => FacetField::FrameType,
            Field::Edited => FacetField::Edited,
            Field::Module => FacetField::Module,
            Field::Style => FacetField::Style,
            Field::Version => FacetField::Version,
            Field::Make => FacetField::Make,
            Field::Software => FacetField::Software,
            Field::Artist => FacetField::Artist,
            Field::Copyright => FacetField::Copyright,
            Field::Serial => FacetField::Serial,
            Field::LensSerial => FacetField::LensSerial,
            Field::Flash => FacetField::Flash,
            Field::WhiteBalance => FacetField::WhiteBalance,
            Field::Program => FacetField::Program,
            Field::Metering => FacetField::Metering,
            Field::ExpComp => FacetField::ExpComp,
            Field::Focal35 => FacetField::Focal35,
            Field::Exif => FacetField::Exif,
            Field::Credit => FacetField::Credit,
            Field::Source => FacetField::Source,
            Field::Event => FacetField::Event,
            Field::JobId => FacetField::JobId,
            _ => return None,
        })
    }

    /// The thresholds a ladder facet is counted against (None: not a ladder), ascending.
    pub fn ladder(self) -> Option<&'static [(&'static str, f64)]> {
        match self {
            FacetField::Width | FacetField::Height => Some(&LADDER_PX),
            FacetField::Megapixels => Some(&LADDER_MP),
            FacetField::Size => Some(&LADDER_SIZE),
            _ => None,
        }
    }

    /// Whether the values are numbers (or dates) that `>=`, `<=`, `>` and `<` compare.
    pub fn numeric(self) -> bool {
        matches!(
            self,
            FacetField::Rating
                | FacetField::Iso
                | FacetField::Aperture
                | FacetField::Focal
                | FacetField::Exposure
                | FacetField::ExpComp
                | FacetField::Focal35
                | FacetField::Bits
                | FacetField::Pages
                | FacetField::Year
                | FacetField::Taken
                | FacetField::Modified
        ) || self.ladder().is_some()
    }

    /// Whether a term takes several of its values (ORed). `dupes:` takes one, and the
    /// ladders are compared with one threshold.
    pub fn several(self) -> bool {
        self.ladder().is_none()
            && !matches!(
                self,
                FacetField::Dupes | FacetField::Similar | FacetField::Edited
            )
    }

    /// The term a value is selected with (`ext:jpg`, `camera="X-T5"`).
    pub fn term(self) -> (Field, Op) {
        match self {
            FacetField::Format => (Field::Ext, Op::Has),
            FacetField::Camera => (Field::Camera, Op::Eq),
            FacetField::Lens => (Field::Lens, Op::Eq),
            FacetField::Rating => (Field::Rating, Op::Has),
            FacetField::Year => (Field::Date, Op::Has),
            FacetField::Orientation => (Field::Orientation, Op::Has),
            FacetField::Label => (Field::Label, Op::Has),
            FacetField::Property | FacetField::Flags => (Field::Is, Op::Has),
            FacetField::Iso => (Field::Iso, Op::Has),
            FacetField::Aperture => (Field::Aperture, Op::Has),
            FacetField::Focal => (Field::Focal, Op::Has),
            FacetField::Exposure => (Field::Exposure, Op::Has),
            FacetField::Tag => (Field::Tag, Op::Eq),
            FacetField::Folder => (Field::Folder, Op::Eq),
            FacetField::Bits => (Field::Bits, Op::Has),
            FacetField::Pages => (Field::Pages, Op::Has),
            FacetField::Width => (Field::Width, Op::Ge),
            FacetField::Height => (Field::Height, Op::Ge),
            FacetField::Megapixels => (Field::Megapixels, Op::Ge),
            FacetField::Size => (Field::Size, Op::Ge),
            FacetField::Taken => (Field::Date, Op::Has),
            FacetField::Modified => (Field::Modified, Op::Has),
            FacetField::Dupes => (Field::Dupes, Op::Has),
            FacetField::Similar => (Field::Similar, Op::Has),
            FacetField::Space => (Field::Space, Op::Eq),
            FacetField::Place => (Field::Place, Op::Eq),
            FacetField::Object => (Field::Object, Op::Eq),
            FacetField::Filter => (Field::Filter, Op::Eq),
            FacetField::FrameType => (Field::FrameType, Op::Eq),
            FacetField::Edited => (Field::Edited, Op::Has),
            FacetField::Module => (Field::Module, Op::Eq),
            FacetField::Style => (Field::Style, Op::Eq),
            FacetField::Version => (Field::Version, Op::Has),
            FacetField::Make => (Field::Make, Op::Eq),
            FacetField::Software => (Field::Software, Op::Eq),
            FacetField::Artist => (Field::Artist, Op::Eq),
            FacetField::Copyright => (Field::Copyright, Op::Eq),
            FacetField::Serial => (Field::Serial, Op::Eq),
            FacetField::LensSerial => (Field::LensSerial, Op::Eq),
            FacetField::Flash => (Field::Flash, Op::Has),
            FacetField::WhiteBalance => (Field::WhiteBalance, Op::Has),
            FacetField::Program => (Field::Program, Op::Has),
            FacetField::Metering => (Field::Metering, Op::Has),
            FacetField::ExpComp => (Field::ExpComp, Op::Has),
            FacetField::Focal35 => (Field::Focal35, Op::Has),
            FacetField::Exif => (Field::Exif, Op::Has),
            FacetField::Credit => (Field::Credit, Op::Eq),
            FacetField::Source => (Field::Source, Op::Eq),
            FacetField::Event => (Field::Event, Op::Eq),
            FacetField::JobId => (Field::JobId, Op::Eq),
        }
    }

    /// Whether the facet needs the images' metadata to count.
    pub fn needs_meta(self) -> bool {
        !matches!(
            self,
            FacetField::Format
                | FacetField::Rating
                | FacetField::Label
                | FacetField::Tag
                | FacetField::Folder
                | FacetField::Place
                | FacetField::Size
                | FacetField::Modified
                | FacetField::Dupes
                | FacetField::Similar
                | FacetField::Edited
                | FacetField::Module
                | FacetField::Style
                | FacetField::Version
        )
    }
}

/// One value of a facet: the term item that selects it and how many files have it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Value {
    pub key: String,
    pub count: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Facet {
    pub field: FacetField,
    pub values: Vec<Value>,
}

/// Counts in progress (fed in chunks, finished into [`Facet`]s).
#[derive(Clone, Debug, Default)]
pub struct Tally {
    format: HashMap<String, u32>,
    camera: HashMap<String, u32>,
    lens: HashMap<String, u32>,
    rating: [u32; 6],
    /// Capture (else modification) months, and modification months: `year * 12 + month - 1`.
    /// The years are their sums.
    taken: Months,
    modified: Months,
    orient: [u32; 3],
    label: [u32; 5],
    props: [u32; PROPERTIES.len()],
    flags: [u32; FLAGS.len()],
    iso: BTreeMap<u32, u32>,
    /// Keyed by [`num_bucket`].
    aperture: BTreeMap<i64, u32>,
    focal: BTreeMap<i64, u32>,
    exposure: BTreeMap<i64, u32>,
    tag: HashMap<String, u32>,
    folder: HashMap<String, u32>,
    bits: Runs<u8>,
    pages: Runs<u32>,
    width: Ladder,
    height: Ladder,
    megapixels: Ladder,
    size: Ladder,
    /// `dupes:` and `similar:`: suggested keepers, the rest.
    dupes: [u32; 2],
    similar: [u32; 2],
    space: Named,
    place: HashMap<String, u32>,
    object: HashMap<String, u32>,
    filter: HashMap<String, u32>,
    frame_type: HashMap<String, u32>,
    /// Develop: edited and not ([`parse::EDITED_WORDS`] order), modules, styles, and
    /// versions ([`parse::VERSION_WORDS`] order).
    edited: [u32; 2],
    module: HashMap<String, u32>,
    style: HashMap<String, u32>,
    version: [u32; 3],
    /// Exif: text fields by value, words in their lists' order, numbers by bucket.
    make: HashMap<String, u32>,
    software: HashMap<String, u32>,
    artist: HashMap<String, u32>,
    copyright: HashMap<String, u32>,
    serial: HashMap<String, u32>,
    lens_serial: HashMap<String, u32>,
    /// IPTC text fields by value.
    credit: HashMap<String, u32>,
    source: HashMap<String, u32>,
    event: HashMap<String, u32>,
    job_id: HashMap<String, u32>,
    flash: [u32; crate::exif::FLASH_WORDS.len()],
    wb: [u32; crate::exif::WB_WORDS.len()],
    program: [u32; crate::exif::PROGRAM_WORDS.len()],
    metering: [u32; crate::exif::METERING_WORDS.len()],
    exp_comp: BTreeMap<i64, u32>,
    focal35: BTreeMap<i64, u32>,
    /// `exif:` tags by their index in [`crate::exif::TAGS`], and (index, code) of their
    /// words (a Flash word's index plus [`FLASH_CODE`]).
    exif_tags: HashMap<usize, u32>,
    exif_words: HashMap<(usize, u32), u32>,
    /// Files counted (images only).
    pub seen: u32,
}

/// Counts against a ladder of thresholds `t[k]`, exact for every comparison the parser
/// makes (which allows `t * 1e-6` of float noise): `at[k]` within the noise of `t[k]`,
/// `between[k]` strictly between `t[k - 1]` and `t[k]` (`between[n]` above the last).
#[derive(Clone, Debug, Default)]
struct Ladder {
    at: Vec<u32>,
    between: Vec<u32>,
    /// The last value and where it went (a library repeats its sizes: one camera's width).
    last: Option<(f64, usize, bool)>,
}

impl Ladder {
    fn add(&mut self, rungs: &[(&str, f64)], v: f64) {
        let (k, at) = match self.last {
            Some((l, k, at)) if l == v => (k, at),
            _ => {
                if self.at.is_empty() {
                    self.at = vec![0; rungs.len()];
                    self.between = vec![0; rungs.len() + 1];
                }
                // The first rung not below v (with its noise): counted rather than searched,
                // which compiles to a few vector compares for a dozen rungs.
                let k: usize = rungs
                    .iter()
                    .map(|(_, t)| usize::from(t + t.abs() * 1e-6 < v))
                    .sum();
                let at = rungs.get(k).is_some_and(|(_, t)| v >= t - t.abs() * 1e-6);
                self.last = Some((v, k, at));
                (k, at)
            }
        };
        if at {
            self.at[k] += 1;
        } else {
            self.between[k] += 1;
        }
    }

    fn total(&self) -> u32 {
        self.at.iter().chain(&self.between).sum()
    }
}

/// Counts by key, with a run of equal keys (files of one camera, one shoot) added up before
/// it goes in the map.
#[derive(Clone, Debug)]
struct Runs<K> {
    map: BTreeMap<K, u32>,
    run: Option<(K, u32)>,
}

impl<K> Default for Runs<K> {
    fn default() -> Self {
        Runs {
            map: BTreeMap::new(),
            run: None,
        }
    }
}

impl<K: Ord + Copy> Runs<K> {
    fn add(&mut self, k: K) {
        match &mut self.run {
            Some((r, n)) if *r == k => *n += 1,
            run => {
                if let Some((r, n)) = run.replace((k, 1)) {
                    *self.map.entry(r).or_default() += n;
                }
            }
        }
    }

    /// Every key and its count, in order.
    fn all(&self) -> BTreeMap<K, u32> {
        let mut m = self.map.clone();
        if let Some((r, n)) = self.run {
            *m.entry(r).or_default() += n;
        }
        m
    }
}

/// Counts by month (`year * 12 + month - 1`); a time in the last month's span is counted
/// without working out its date.
#[derive(Clone, Debug, Default)]
struct Months {
    runs: Runs<i64>,
    /// The last month's [start, end) in seconds.
    span: (i64, i64),
}

impl Months {
    fn add(&mut self, secs: i64) {
        if let Some((k, _)) = self.runs.run
            && (self.span.0..self.span.1).contains(&secs)
        {
            return self.runs.add(k);
        }
        use crate::date::{DAY, civil_from_days, days_from_civil};
        let (y, m, _) = civil_from_days(secs.div_euclid(DAY));
        let (ny, nm) = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
        self.span = (
            days_from_civil(y, m, 1) * DAY,
            days_from_civil(ny, nm, 1) * DAY,
        );
        self.runs.add(y * 12 + i64::from(m) - 1);
    }
}

/// Counts by name, with a run of equal names (a folder of one camera's files) added up
/// before it goes in the map, so a run costs a string compare per file.
#[derive(Clone, Debug, Default)]
struct Named {
    map: HashMap<String, u32>,
    run: Option<(String, u32)>,
}

impl Named {
    fn add(&mut self, k: &str) {
        match &mut self.run {
            Some((r, n)) if r == k => *n += 1,
            run => {
                if let Some((r, n)) = run.take() {
                    *self.map.entry(r).or_default() += n;
                }
                *run = Some((k.to_owned(), 1));
            }
        }
    }

    /// Every name and its count.
    fn all(&self) -> HashMap<String, u32> {
        let mut m = self.map.clone();
        if let Some((r, n)) = &self.run {
            *m.entry(r.clone()).or_default() += n;
        }
        m
    }
}

impl Tally {
    /// Counts one file, given the query's [`crate::Matcher::fail_mask`] for it.
    pub fn add(&mut self, f: &Facts, mask: u64) {
        if f.container {
            return;
        }
        self.seen += 1;
        let counts = |field: FacetField| mask & !field.bit() == 0;
        if counts(FacetField::Format) && !f.ext.is_empty() {
            *self
                .format
                .entry(canon_ext(f.ext).into_owned())
                .or_default() += 1;
        }
        if counts(FacetField::Rating) {
            self.rating[usize::from(f.rating.min(5))] += 1;
        }
        if counts(FacetField::Label) {
            self.label[usize::from(f.label.min(4))] += 1;
        }
        if counts(FacetField::Tag)
            && let Some(tags) = f.tags
        {
            for t in tags.lines().map(str::trim).filter(|t| !t.is_empty()) {
                bump(&mut self.tag, t);
            }
        }
        if counts(FacetField::Folder)
            && let Some(d) = f.folder.filter(|d| !d.is_empty())
        {
            bump(&mut self.folder, d);
        }
        // The town: the first line of the place.
        if counts(FacetField::Place)
            && let Some(p) = f.place.and_then(|p| p.lines().next()).map(str::trim)
            && !p.is_empty()
        {
            bump(&mut self.place, p);
        }
        // The marks are known without the metadata; the rest are Unknown (not counted) until
        // it is read.
        if counts(FacetField::Flags) {
            for (k, fl) in FLAGS.iter().enumerate() {
                self.flags[k] += u32::from(crate::flag(*fl, f) == crate::Tri::Yes);
            }
        }
        if counts(FacetField::Size)
            && let Some(s) = f.size
        {
            self.size.add(&LADDER_SIZE, s as f64);
        }
        if counts(FacetField::Modified)
            && let Some(t) = f.modified
        {
            self.modified.add(t);
        }
        if counts(FacetField::Edited)
            && let Some(e) = f.edited
        {
            self.edited[usize::from(!e)] += 1;
        }
        if f.edited != Some(false) {
            for (field, v, m) in [
                (FacetField::Module, f.modules, &mut self.module),
                (FacetField::Style, f.styles, &mut self.style),
            ] {
                if counts(field)
                    && let Some(v) = v
                {
                    for l in v.lines().map(str::trim).filter(|l| !l.is_empty()) {
                        bump(m, l);
                    }
                }
            }
        }
        if counts(FacetField::Version) {
            self.version[if f.is_version {
                2
            } else {
                usize::from(f.versions <= 1)
            }] += 1;
        }
        for (field, role, n) in [
            (FacetField::Dupes, f.dupes, &mut self.dupes),
            (FacetField::Similar, f.similar, &mut self.similar),
        ] {
            if let Some(keep) = role
                && counts(field)
            {
                n[usize::from(!keep)] += 1;
            }
        }
        if !f.known {
            return;
        }
        if counts(FacetField::Camera)
            && let Some(c) = f.camera.map(str::trim).filter(|c| !c.is_empty())
        {
            *self.camera.entry(c.to_owned()).or_default() += 1;
        }
        if counts(FacetField::Lens)
            && let Some(l) = f.lens.map(str::trim).filter(|l| !l.is_empty())
        {
            *self.lens.entry(l.to_owned()).or_default() += 1;
        }
        for (field, v, m) in [
            (FacetField::Object, f.object, &mut self.object),
            (FacetField::Filter, f.filter, &mut self.filter),
            (FacetField::FrameType, f.frame_type, &mut self.frame_type),
        ] {
            if counts(field)
                && let Some(v) = v.map(str::trim).filter(|v| !v.is_empty())
            {
                *m.entry(v.to_owned()).or_default() += 1;
            }
        }
        if counts(FacetField::Year)
            && let Some(t) = f.taken.or(f.modified)
        {
            self.taken.add(t);
        }
        if counts(FacetField::Bits) && f.bit_depth > 0 {
            self.bits.add(f.bit_depth);
        }
        if counts(FacetField::Pages) {
            self.pages.add(f.pages.max(1));
        }
        if counts(FacetField::Space)
            && let Some(s) = f.space.filter(|s| !s.is_empty())
        {
            self.space.add(s);
        }
        if f.width > 0 && f.height > 0 {
            let (w, h) = (f64::from(f.width), f64::from(f.height));
            for (field, v, l) in [
                (FacetField::Width, w, &mut self.width),
                (FacetField::Height, h, &mut self.height),
                (FacetField::Megapixels, w * h / 1e6, &mut self.megapixels),
            ] {
                if counts(field) {
                    l.add(field.ladder().unwrap_or_default(), v);
                }
            }
        }
        if counts(FacetField::Orientation) && f.width > 0 && f.height > 0 {
            let o = Orient::of(f.width, f.height);
            self.orient[Orient::ALL.iter().position(|x| *x == o).unwrap_or(0)] += 1;
        }
        if counts(FacetField::Iso)
            && let Some(v) = f.iso.filter(|v| *v > 0)
        {
            *self.iso.entry(v).or_default() += 1;
        }
        for (field, v, map) in [
            (FacetField::Aperture, f.f_number, &mut self.aperture),
            (FacetField::Focal, f.focal, &mut self.focal),
            (FacetField::Exposure, f.exposure, &mut self.exposure),
        ] {
            if counts(field)
                && let Some(b) = v.and_then(|v| num_bucket(field, v))
            {
                *map.entry(b).or_default() += 1;
            }
        }
        if let Some(e) = f.exif {
            self.add_exif(&e, f.iptc, &counts);
        } else if let Some(i) = f.iptc {
            self.add_iptc(i, true, true, &counts);
        }
        if counts(FacetField::Property) {
            for (k, p) in PROPERTIES.iter().enumerate() {
                let on = match p {
                    Flag::Hdr => f.hdr,
                    Flag::Alpha => f.alpha,
                    Flag::Gps => f.gps == Some(true),
                    Flag::Raw => f.raw,
                    Flag::Animated => f.animated,
                    Flag::Video => f.is_video(),
                    Flag::Sound => f.is_sound(),
                    _ => f.pages > 1,
                };
                self.props[k] += u32::from(on);
            }
        }
    }

    /// The IPTC facets of `lines`; the creator and copyright only where asked (the file's
    /// IPTC has them, so they stand in for its Exif).
    fn add_iptc(
        &mut self,
        lines: &str,
        artist: bool,
        copyright: bool,
        counts: &impl Fn(FacetField) -> bool,
    ) {
        for (field, key, m, on) in [
            (FacetField::Artist, "creator", &mut self.artist, artist),
            (
                FacetField::Copyright,
                "copyright",
                &mut self.copyright,
                copyright,
            ),
            (FacetField::Credit, "credit", &mut self.credit, true),
            (FacetField::Source, "source", &mut self.source, true),
            (FacetField::Event, "event", &mut self.event, true),
            (FacetField::JobId, "job-id", &mut self.job_id, true),
        ] {
            if on && counts(field) {
                for v in crate::iptc_values(lines, key) {
                    bump(m, v.trim());
                }
            }
        }
    }

    fn add_exif(
        &mut self,
        e: &crate::ExifFacts,
        iptc: Option<&str>,
        counts: &impl Fn(FacetField) -> bool,
    ) {
        use crate::exif::{self, FLASH_WORDS, METERING_WORDS, PROGRAM_WORDS, WB_WORDS};
        // IPTC's creator and copyright where the file has them (the Metadata Working Group
        // prefers them), else Exif's.
        let has = |k: &str| iptc.is_some_and(|i| crate::iptc_values(i, k).next().is_some());
        let (artist, copyright) = (has("creator"), has("copyright"));
        if let Some(i) = iptc {
            self.add_iptc(i, artist, copyright, counts);
        }
        for (field, v, m) in [
            (FacetField::Make, e.make, &mut self.make),
            (FacetField::Software, e.software, &mut self.software),
            (
                FacetField::Artist,
                e.artist.filter(|_| !artist),
                &mut self.artist,
            ),
            (
                FacetField::Copyright,
                e.copyright.filter(|_| !copyright),
                &mut self.copyright,
            ),
            (FacetField::Serial, e.serial, &mut self.serial),
            (FacetField::LensSerial, e.lens_serial, &mut self.lens_serial),
        ] {
            if counts(field)
                && let Some(v) = v.map(str::trim).filter(|v| !v.is_empty())
            {
                bump(m, v);
            }
        }
        for (field, words, n) in [
            (FacetField::Flash, &FLASH_WORDS[..], &mut self.flash[..]),
            (FacetField::WhiteBalance, &WB_WORDS[..], &mut self.wb[..]),
            (
                FacetField::Program,
                &PROGRAM_WORDS[..],
                &mut self.program[..],
            ),
            (
                FacetField::Metering,
                &METERING_WORDS[..],
                &mut self.metering[..],
            ),
        ] {
            if counts(field) {
                let f = field.term().0;
                for (k, w) in words.iter().enumerate() {
                    n[k] += u32::from(crate::exif_word(f, e, w));
                }
            }
        }
        if counts(FacetField::ExpComp)
            && let Some(v) = e.exposure_bias.filter(|v| v.is_finite())
        {
            *self
                .exp_comp
                .entry((f64::from(v) * 10.0).round() as i64)
                .or_default() += 1;
        }
        if counts(FacetField::Focal35)
            && let Some(b) = e.focal35.and_then(|v| num_bucket(FacetField::Focal35, v))
        {
            *self.focal35.entry(b).or_default() += 1;
        }
        if counts(FacetField::Exif)
            && let Some(tags) = e.tags
        {
            for l in tags.lines() {
                let Some((key, v)) = l.split_once('=') else {
                    continue;
                };
                let Some(i) = exif::tag_of_key(key)
                    .and_then(|t| exif::TAGS.iter().position(|x| std::ptr::eq(x, t)))
                else {
                    continue;
                };
                *self.exif_tags.entry(i).or_default() += 1;
                match exif::TAGS[i].kind {
                    exif::TagKind::Enum(words) => {
                        if let Ok(c) = v.trim().parse::<u32>()
                            && words.iter().any(|(x, _)| *x == c)
                        {
                            *self.exif_words.entry((i, c)).or_default() += 1;
                        }
                    }
                    exif::TagKind::Flash => {
                        if let Ok(bits) = v.trim().parse::<u16>() {
                            for (k, w) in FLASH_WORDS.iter().enumerate() {
                                if exif::flash_is(bits, w) {
                                    *self
                                        .exif_words
                                        .entry((i, FLASH_CODE + k as u32))
                                        .or_default() += 1;
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    /// The `exif:` facet's values: tag names by count, then `Name=word` by count.
    fn exif_values(&self) -> Vec<Value> {
        use crate::exif::{FLASH_WORDS, TAGS};
        let mut names: Vec<Value> = self
            .exif_tags
            .iter()
            .map(|(i, c)| Value {
                key: TAGS[*i].name.to_owned(),
                count: *c,
            })
            .collect();
        names.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.key.cmp(&b.key)));
        let mut words: Vec<Value> = self
            .exif_words
            .iter()
            .filter_map(|((i, code), c)| {
                let t = &TAGS[*i];
                let w = if *code >= FLASH_CODE {
                    FLASH_WORDS.get((code - FLASH_CODE) as usize).copied()
                } else {
                    t.words().iter().find(|(x, _)| x == code).map(|(_, w)| *w)
                }?;
                Some(Value {
                    key: format!("{}={w}", t.name),
                    count: *c,
                })
            })
            .collect();
        words.sort_by(|a, b| a.key.cmp(&b.key));
        names.extend(words);
        names
    }

    /// The facets with at least one value, most common values first (ratings, years, labels
    /// and the fixed lists keep their natural order). Selected values stay listed with their
    /// counts even when zero, so they can be cleared.
    pub fn finish(&self, q: &Query) -> Vec<Facet> {
        let by_count = |m: &HashMap<String, u32>| {
            let mut v: Vec<Value> = m
                .iter()
                .map(|(k, c)| Value {
                    key: k.clone(),
                    count: *c,
                })
                .collect();
            v.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.key.cmp(&b.key)));
            v
        };
        let fixed = |keys: &mut dyn Iterator<Item = (String, u32)>| -> Vec<Value> {
            keys.map(|(key, count)| Value { key, count }).collect()
        };
        let ascending = |m: &BTreeMap<i64, u32>, field: FacetField| -> Vec<Value> {
            let mut v: Vec<(f64, Value)> = m
                .iter()
                .map(|(b, c)| {
                    let key = bucket_key(field, *b);
                    (bucket_value(field, *b), Value { key, count: *c })
                })
                .collect();
            v.sort_by(|a, b| a.0.total_cmp(&b.0));
            v.into_iter().map(|(_, v)| v).collect()
        };
        let mut out = Vec::new();
        for field in FacetField::ALL.into_iter().chain(FacetField::PICKER) {
            let mut values = match field {
                FacetField::Format => by_count(&self.format),
                FacetField::Camera => by_count(&self.camera),
                FacetField::Lens => by_count(&self.lens),
                FacetField::Rating => {
                    fixed(&mut (0..=5usize).rev().map(|r| (r.to_string(), self.rating[r])))
                }
                FacetField::Year => dates(&self.taken.runs.all(), false),
                FacetField::Taken => dates(&self.taken.runs.all(), true),
                FacetField::Modified => dates(&self.modified.runs.all(), true),
                FacetField::Orientation => fixed(
                    &mut Orient::ALL
                        .iter()
                        .enumerate()
                        .map(|(k, o)| (o.word().to_owned(), self.orient[k])),
                ),
                FacetField::Label => fixed(
                    &mut (1..parse::LABEL_WORDS.len())
                        .map(|k| (parse::LABEL_WORDS[k].to_owned(), self.label[k])),
                ),
                FacetField::Property => fixed(
                    &mut PROPERTIES
                        .iter()
                        .enumerate()
                        .map(|(k, p)| (p.word().to_owned(), self.props[k])),
                ),
                FacetField::Iso => fixed(&mut self.iso.iter().map(|(v, c)| (v.to_string(), *c))),
                FacetField::Aperture => ascending(&self.aperture, field),
                FacetField::Focal => ascending(&self.focal, field),
                FacetField::Exposure => ascending(&self.exposure, field),
                FacetField::Tag => by_count(&self.tag),
                FacetField::Folder => by_count(&self.folder),
                FacetField::Flags => fixed(
                    &mut FLAGS
                        .iter()
                        .enumerate()
                        .map(|(k, p)| (p.word().to_owned(), self.flags[k])),
                ),
                FacetField::Bits => {
                    fixed(&mut self.bits.all().iter().map(|(v, c)| (v.to_string(), *c)))
                }
                FacetField::Pages => {
                    fixed(&mut self.pages.all().iter().map(|(v, c)| (v.to_string(), *c)))
                }
                FacetField::Width => ladder(field, &self.width),
                FacetField::Height => ladder(field, &self.height),
                FacetField::Megapixels => ladder(field, &self.megapixels),
                FacetField::Size => ladder(field, &self.size),
                FacetField::Space => by_count(&self.space.all()),
                FacetField::Place => by_count(&self.place),
                FacetField::Object => by_count(&self.object),
                FacetField::Filter => by_count(&self.filter),
                FacetField::FrameType => by_count(&self.frame_type),
                FacetField::Module => by_count(&self.module),
                FacetField::Style => by_count(&self.style),
                FacetField::Make => by_count(&self.make),
                FacetField::Software => by_count(&self.software),
                FacetField::Artist => by_count(&self.artist),
                FacetField::Copyright => by_count(&self.copyright),
                FacetField::Serial => by_count(&self.serial),
                FacetField::LensSerial => by_count(&self.lens_serial),
                FacetField::Credit => by_count(&self.credit),
                FacetField::Source => by_count(&self.source),
                FacetField::Event => by_count(&self.event),
                FacetField::JobId => by_count(&self.job_id),
                FacetField::Flash => words(&crate::exif::FLASH_WORDS, &self.flash),
                FacetField::WhiteBalance => words(&crate::exif::WB_WORDS, &self.wb),
                FacetField::Program => words(&crate::exif::PROGRAM_WORDS, &self.program),
                FacetField::Metering => words(&crate::exif::METERING_WORDS, &self.metering),
                FacetField::ExpComp => self
                    .exp_comp
                    .iter()
                    .map(|(b, c)| Value {
                        key: format!("{}", *b as f64 / 10.0),
                        count: *c,
                    })
                    .collect(),
                FacetField::Focal35 => ascending(&self.focal35, field),
                FacetField::Exif => self.exif_values(),
                // Listed only once a file has an edit, or versions (else every folder would
                // show "Edited: no").
                FacetField::Edited if self.edited[0] == 0 && selected(q, field).is_empty() => {
                    Vec::new()
                }
                FacetField::Edited => fixed(
                    &mut parse::EDITED_WORDS
                        .iter()
                        .zip(self.edited)
                        .map(|(w, c)| ((*w).to_owned(), c)),
                ),
                FacetField::Version
                    if self.version[0] + self.version[2] == 0 && selected(q, field).is_empty() =>
                {
                    Vec::new()
                }
                FacetField::Version => fixed(
                    &mut parse::VERSION_WORDS
                        .iter()
                        .zip(self.version)
                        .map(|(w, c)| ((*w).to_owned(), c)),
                ),
                FacetField::Dupes | FacetField::Similar => {
                    let [keep, rest] = if field == FacetField::Dupes {
                        self.dupes
                    } else {
                        self.similar
                    };
                    fixed(
                        &mut GROUP_WORDS
                            .iter()
                            .zip([keep + rest, keep, rest])
                            .map(|(w, c)| ((*w).to_owned(), c)),
                    )
                }
            };
            if field.ladder().is_some() {
                // Laid out for [`compare`]: every rung, counted or not.
                if !values.is_empty() {
                    out.push(Facet { field, values });
                }
                continue;
            }
            values.retain(|v| v.count > 0 || is_active(q, field, &v.key));
            // Selected values the files no longer have (a camera typed by hand).
            for key in selected(q, field) {
                if !values.iter().any(|v| same(field, &v.key, &key)) {
                    values.push(Value { key, count: 0 });
                }
            }
            if !values.is_empty() {
                out.push(Facet { field, values });
            }
        }
        out
    }
}

/// The [`crate::exif::FLASH_WORDS`] are counted as codes from here up.
const FLASH_CODE: u32 = 1 << 20;

/// A word list's counts, in its order.
fn words(list: &[&str], counts: &[u32]) -> Vec<Value> {
    list.iter()
        .zip(counts)
        .map(|(w, c)| Value {
            key: (*w).to_owned(),
            count: *c,
        })
        .collect()
}

/// Years, newest first, each followed by its months (newest first) when `months`.
fn dates(m: &BTreeMap<i64, u32>, months: bool) -> Vec<Value> {
    let mut out: Vec<Value> = Vec::new();
    let mut year_at = 0;
    let mut year = None;
    for (&ym, &count) in m.iter().rev() {
        let y = ym.div_euclid(12);
        if year != Some(y) {
            year = Some(y);
            year_at = out.len();
            out.push(Value {
                key: y.to_string(),
                count: 0,
            });
        }
        out[year_at].count += count;
        if months {
            out.push(Value {
                key: format!("{y}-{:02}", ym.rem_euclid(12) + 1),
                count,
            });
        }
    }
    out
}

/// A ladder's counts as a facet's values: `between[0]`, `at[0]` (keyed by the rung),
/// `between[1]`, …, `between[n]`; the betweens have empty keys. Empty when nothing was
/// counted.
fn ladder(field: FacetField, l: &Ladder) -> Vec<Value> {
    let rungs = field.ladder().unwrap_or_default();
    if l.total() == 0 {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(2 * rungs.len() + 1);
    for (k, (key, _)) in rungs.iter().enumerate() {
        out.push(Value {
            key: String::new(),
            count: l.between[k],
        });
        out.push(Value {
            key: (*key).to_owned(),
            count: l.at[k],
        });
    }
    out.push(Value {
        key: String::new(),
        count: l.between[rungs.len()],
    });
    out
}

/// A facet's values with the counts a comparison makes (`iso>=800`: the files at ISO 800
/// and above): for the numbers and dates, each value counts the values it lets through; a
/// ladder lists its thresholds (`:` and `=` count as `>=`). Values nothing passes are left
/// out, and so are thresholds a stricter one counts the same as. Other facets, and `:` / `=`
/// on the rest, keep their values.
pub fn compare(f: &Facet, op: Op) -> Vec<Value> {
    if let Some(rungs) = f.field.ladder() {
        return compare_ladder(f, rungs, op);
    }
    if matches!(op, Op::Has | Op::Eq) || !f.field.numeric() {
        return f.values.clone();
    }
    // Each value as what it selects ([lo, hi) for a date, the point lo == hi for a number),
    // and the atoms counted: for dates the months (a year is their sum), else each value.
    let dates = matches!(
        f.field,
        FacetField::Year | FacetField::Taken | FacetField::Modified
    );
    let span = |key: &str| -> Option<(f64, f64)> {
        if dates {
            crate::date::period(key).map(|(a, b)| (a as f64, b as f64))
        } else {
            parse::number(f.field.term().0, key).map(|(v, _)| (v, v))
        }
    };
    let month = |key: &str| key.len() == 7 && key.as_bytes()[4] == b'-';
    let monthly = dates && f.values.iter().any(|v| month(&v.key));
    let mut atoms: Vec<(f64, f64, u32)> = f
        .values
        .iter()
        .filter(|v| !monthly || month(&v.key))
        .filter_map(|v| span(&v.key).map(|(lo, hi)| (lo, hi, v.count)))
        .collect();
    atoms.sort_by(|a, b| a.0.total_cmp(&b.0));
    // The counts of the atoms before each (prefix sums), for a binary search per value.
    let mut before = Vec::with_capacity(atoms.len() + 1);
    let mut sum = 0u32;
    before.push(0);
    for a in &atoms {
        sum += a.2;
        before.push(sum);
    }
    // The atoms starting below x (or at it, when `at`).
    let below = |x: f64, at: bool| -> u32 {
        before[atoms.partition_point(|a| if at { a.0 <= x } else { a.0 < x })]
    };
    let mut out = Vec::new();
    for v in &f.values {
        let Some((lo, hi)) = span(&v.key) else {
            continue;
        };
        let count = match op {
            Op::Ge => sum - below(lo, false),
            Op::Gt if dates => sum - below(hi, false),
            Op::Gt => sum - below(hi, true),
            Op::Lt => below(lo, false),
            Op::Le if dates => below(hi, false),
            _ => below(hi, true),
        };
        if count > 0 {
            out.push(Value {
                key: v.key.clone(),
                count,
            });
        }
    }
    out
}

fn compare_ladder(f: &Facet, rungs: &[(&str, f64)], op: Op) -> Vec<Value> {
    let n = rungs.len();
    if f.values.len() != 2 * n + 1 {
        return Vec::new();
    }
    let between = |k: usize| f.values[2 * k].count;
    let at = |k: usize| f.values[2 * k + 1].count;
    let total: u32 = f.values.iter().map(|v| v.count).sum();
    // At or above each rung, from the top down.
    let mut ge = vec![0u32; n];
    let mut above = between(n);
    for k in (0..n).rev() {
        above += at(k);
        ge[k] = above;
        above += between(k);
    }
    let count = |k: usize| match op {
        Op::Gt => ge[k] - at(k),
        Op::Lt => total - ge[k],
        Op::Le => total - ge[k] + at(k),
        _ => ge[k],
    };
    let up = matches!(op, Op::Lt | Op::Le);
    let mut out: Vec<Value> = Vec::new();
    for (k, (key, _)) in rungs.iter().enumerate() {
        let c = count(k);
        if c == 0 {
            continue;
        }
        // Of thresholds that count the same, the stricter: the higher for `>=`, the lower
        // for `<=`.
        if out.last().is_some_and(|p| p.count == c) {
            if up {
                continue;
            }
            out.pop();
        }
        out.push(Value {
            key: (*key).to_owned(),
            count: c,
        });
    }
    out
}

fn bump(m: &mut HashMap<String, u32>, k: &str) {
    match m.get_mut(k) {
        Some(c) => *c += 1,
        None => {
            m.insert(k.to_owned(), 1);
        }
    }
}

/// The bucket a camera setting is counted in: aperture to a tenth of an f-number, focal
/// length to the millimetre, exposure as `1/N` below half a second and to a tenth of a second
/// above. Each bucket's [`bucket_key`] selects exactly it (the parser tolerates half a unit of
/// the last digit typed, and 1% for fractions).
fn num_bucket(field: FacetField, v: f32) -> Option<i64> {
    let v = f64::from(v);
    if !(v.is_finite() && v > 0.0) {
        return None;
    }
    Some(match field {
        FacetField::Focal | FacetField::Focal35 => v.round() as i64,
        // Fractions are negative (-250 is 1/250), so the two forms never meet.
        FacetField::Exposure if v < 0.5 => -((1.0 / v).round() as i64),
        _ => (v * 10.0).round() as i64,
    })
}

fn bucket_value(field: FacetField, b: i64) -> f64 {
    match field {
        FacetField::Focal | FacetField::Focal35 => b as f64,
        _ if b < 0 => -1.0 / b as f64,
        _ => b as f64 / 10.0,
    }
}

/// The query value that selects bucket `b` (`2.8`, `50`, `1/250`, `0.8`).
fn bucket_key(field: FacetField, b: i64) -> String {
    match field {
        FacetField::Focal | FacetField::Focal35 => b.to_string(),
        _ if b < 0 => format!("1/{}", -b),
        _ => format!("{}.{}", b / 10, b % 10),
    }
}

pub(crate) fn same(field: FacetField, a: &str, b: &str) -> bool {
    match field {
        FacetField::Format => parse::norm_ext(a) == parse::norm_ext(b),
        FacetField::Camera
        | FacetField::Lens
        | FacetField::Tag
        | FacetField::Folder
        | FacetField::Place
        | FacetField::Object
        | FacetField::Filter
        | FacetField::FrameType
        | FacetField::Make
        | FacetField::Software
        | FacetField::Artist
        | FacetField::Copyright
        | FacetField::Serial
        | FacetField::LensSerial
        | FacetField::Credit
        | FacetField::Source
        | FacetField::Event
        | FacetField::JobId => a.trim().to_lowercase() == b.trim().to_lowercase(),
        // `gray` and `grey`, `Display P3` and `p3`.
        FacetField::Space => match (parse::space_word(a), parse::space_word(b)) {
            (Some(x), Some(y)) => x == y,
            (None, None) => a.trim().to_lowercase() == b.trim().to_lowercase(),
            _ => false,
        },
        // `f/2.8` and `2.8`, `1/250s` and `1/250`.
        FacetField::Iso
        | FacetField::Aperture
        | FacetField::Focal
        | FacetField::Exposure
        | FacetField::ExpComp
        | FacetField::Focal35 => {
            let f = field.term().0;
            match (parse::number(f, a), parse::number(f, b)) {
                (Some((x, tx)), Some((y, ty))) => (x - y).abs() <= tx.min(ty) + 1e-9,
                _ => a.eq_ignore_ascii_case(b),
            }
        }
        // `is:picked` and `pick`.
        FacetField::Property | FacetField::Flags => {
            match (Flag::from_word(a), Flag::from_word(b)) {
                (Some(x), Some(y)) => x == y,
                _ => a.eq_ignore_ascii_case(b),
            }
        }
        _ => a.eq_ignore_ascii_case(b),
    }
}

/// The positive terms that select values of `field`: (index, items).
fn selecting(q: &Query, field: FacetField) -> impl Iterator<Item = (usize, &Vec<String>)> {
    let (f, op) = field.term();
    q.terms
        .iter()
        .enumerate()
        .filter_map(move |(k, t)| match &t.kind {
            Kind::Field {
                field: tf,
                op: top,
                items,
                pred,
            } if !t.negated => {
                let ok = if matches!(field, FacetField::Property | FacetField::Flags) {
                    matches!(pred, Pred::Flags(_))
                } else {
                    *tf == f && *top == op
                };
                ok.then_some((k, items))
            }
            _ => None,
        })
}

fn selected(q: &Query, field: FacetField) -> Vec<String> {
    if matches!(field, FacetField::Property | FacetField::Flags) {
        return Vec::new();
    }
    let mut out: Vec<String> = Vec::new();
    for (_, items) in selecting(q, field) {
        for i in items {
            if !out.iter().any(|o| same(field, o, i)) {
                out.push(i.clone());
            }
        }
    }
    out
}

/// Whether the query selects `key` of `field`.
pub fn is_active(q: &Query, field: FacetField, key: &str) -> bool {
    selecting(q, field).any(|(_, items)| items.iter().any(|i| same(field, i, key)))
}

/// The query text with `key` of `field` selected or, when it is, cleared: the value joins (or
/// leaves) the field's existing term, which keeps the values ORed.
pub fn toggle(text: &str, field: FacetField, key: &str) -> String {
    // No value to select (a ladder's span between two thresholds): `size>=""` would not parse.
    if key.is_empty() {
        return text.to_owned();
    }
    let q = parse::parse(text);
    if matches!(field, FacetField::Property | FacetField::Flags) {
        let Some(flag) = FLAGS.iter().find(|p| p.word() == key) else {
            return text.to_owned();
        };
        // A term that is just this flag goes; otherwise it is added.
        for (k, items) in selecting(&q, field) {
            if items.len() == 1 && items[0].eq_ignore_ascii_case(key)
                || matches!(&q.terms[k].kind, Kind::Field { pred: Pred::Flags(f), .. } if f.as_slice() == [*flag])
            {
                return remove_span(text, q.terms[k].span);
            }
        }
        return append(text, &format!("{}:{}", flag.field().name(), flag.word()));
    }
    let (f, op) = field.term();
    // One value only (`edited:`, `dupes:`): picking another replaces it.
    if !field.several()
        && field.ladder().is_none()
        && let Some((k, items)) = selecting(&q, field).next()
        && !items.iter().any(|i| same(field, i, key))
    {
        return replace_span(
            text,
            q.terms[k].span,
            &render(f, op, std::iter::once(&key.to_owned())),
        );
    }
    let mut first = None;
    for (k, items) in selecting(&q, field) {
        if items.iter().any(|i| same(field, i, key)) {
            let rest: Vec<&String> = items.iter().filter(|i| !same(field, i, key)).collect();
            let span = q.terms[k].span;
            if rest.is_empty() {
                return remove_span(text, span);
            }
            return replace_span(text, span, &render(f, op, rest.into_iter()));
        }
        first.get_or_insert(k);
    }
    match first {
        Some(k) => {
            let Kind::Field { items, .. } = &q.terms[k].kind else {
                return text.to_owned();
            };
            let owned = key.to_owned();
            replace_span(
                text,
                q.terms[k].span,
                &render(f, op, items.iter().chain(std::iter::once(&owned))),
            )
        }
        None => append(text, &render(f, op, std::iter::once(&key.to_owned()))),
    }
}

fn render<'a>(f: Field, op: Op, items: impl Iterator<Item = &'a String>) -> String {
    let items: Vec<String> = items.map(|i| parse::quote(i)).collect();
    format!("{}{}{}", f.name(), op.text(), items.join(","))
}

/// The text with a term added at the end.
pub fn append(text: &str, term: &str) -> String {
    let t = text.trim_end();
    if t.is_empty() {
        term.to_owned()
    } else {
        format!("{t} {term}")
    }
}

/// The text without the term at `span` (and the space it leaves).
pub fn remove_span(text: &str, span: Span) -> String {
    let (a, b) = (&text[..span.start], &text[span.end..]);
    let (a, b) = (a.trim_end(), b.trim_start());
    match (a.is_empty(), b.is_empty()) {
        (true, _) => b.to_owned(),
        (_, true) => a.to_owned(),
        _ => format!("{a} {b}"),
    }
}

fn replace_span(text: &str, span: Span, with: &str) -> String {
    format!("{}{with}{}", &text[..span.start], &text[span.end..])
}
