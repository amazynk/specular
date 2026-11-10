//! Typed queries over images: a forgiving parser ([`parse`]), a tri-state evaluator
//! ([`Matcher`]), facet counts ([`facet`]) and events split on time gaps and location jumps
//! ([`events`]).
//!
//! No UI and no I/O: the caller describes each file as [`Facts`] (what the listing, the
//! culling marks and the metadata cache know about it) and gets back whether it matches. What
//! is not known yet (metadata still being read) matches, so results only ever shrink as the
//! scan fills them in, and the caller decides when to ask again.

use std::borrow::Cow;

pub mod complete;
pub mod date;
pub mod events;
pub mod exif;
pub mod facet;
pub mod parse;
pub mod rules;
pub mod timeline;

pub use parse::{
    AiFeature, Error, ErrorKind, Expected, Field, Flag, Kind, Op, Orient, Pred, Query, SOUND_EXTS,
    SPACE_WORDS, Scope, Span, Term, VIDEO_EXTS, is_sound_ext, is_video_ext, module_name, parse,
    parse_in, set_module_names, space_word,
};

use facet::FacetField;

/// What is known about one file.
#[derive(Clone, Copy, Debug, Default)]
pub struct Facts<'a> {
    pub name: &'a str,
    /// File extension as on disk (any case, no dot).
    pub ext: &'a str,
    /// A folder or archive shown in the gallery.
    pub container: bool,
    pub size: Option<u64>,
    /// Modification time, seconds.
    pub modified: Option<i64>,
    pub rating: u8,
    pub label: u8,
    pub pick: bool,
    pub reject: bool,
    /// Whether the image's metadata has been read; the fields below mean nothing until it is.
    pub known: bool,
    pub width: u32,
    pub height: u32,
    /// Capture time, local seconds.
    pub taken: Option<i64>,
    pub camera: Option<&'a str>,
    pub lens: Option<&'a str>,
    pub iso: Option<u32>,
    pub f_number: Option<f32>,
    pub focal: Option<f32>,
    /// Seconds.
    pub exposure: Option<f32>,
    pub animated: bool,
    pub hdr: bool,
    pub alpha: bool,
    pub raw: bool,
    pub profile: bool,
    pub pages: u32,
    pub bit_depth: u8,
    /// None: not known even with the metadata read (an older cache entry).
    pub gps: Option<bool>,
    /// Member of an exact-duplicate set: Some(suggested keeper).
    pub dupes: Option<bool>,
    /// Member of a similar set: Some(suggested keeper).
    pub similar: Option<bool>,
    /// Tags and keywords, one per line (the Library's; None where they are not known).
    pub tags: Option<&'a str>,
    /// The folder the file is in (the Library's; None where it is not known).
    pub folder: Option<&'a str>,
    /// What plain words and phrases search, when more than the name (the Library's name,
    /// folder, camera, lens and tags).
    pub text: Option<&'a str>,
    /// The colour space: a word of [`SPACE_WORDS`], a profile's description, or empty when
    /// nothing names one. None: not known even with the metadata read (an older cache entry,
    /// a catalogue row read before it was kept).
    pub space: Option<&'a str>,
    /// Where it was taken, one name per line: the town, its administrative areas, the
    /// country's names and code (the Library's, from offline reverse geocoding). Empty when
    /// it has no position (or one far from every town); None where it is not known.
    pub place: Option<&'a str>,
    /// The named people whose faces are in it, one per line (the Library's, when People is
    /// on); None where it is not known.
    pub people: Option<&'a str>,
    /// An astronomy subframe (FITS, XISF) whose header was read: the fields below apply.
    pub subframe: bool,
    pub object: Option<&'a str>,
    pub filter: Option<&'a str>,
    /// `light`, `dark`, `flat`, `bias`, `darkflat` or `IMAGETYP` as stored.
    pub frame_type: Option<&'a str>,
    pub gain: Option<f32>,
    /// °C.
    pub ccd_temp: Option<f32>,
    /// The star metrics, once measured (None meanwhile: their tests are unknown).
    pub stars: Option<StarFacts>,
    /// Some(true): sound without a picture, Some(false): a movie, by its content; None:
    /// not read as either (the extension decides [`Flag::Video`] and [`Flag::Sound`]).
    pub sound: Option<bool>,
    /// Develop: whether it has an edit; None while not known (its sidecar not read yet).
    pub edited: Option<bool>,
    /// Develop: the modules its edit uses, stable ids one per line (`exposure`); None where
    /// not known.
    pub modules: Option<&'a str>,
    /// Develop: the styles applied to it (from the edit's history), one per line; None
    /// where not known.
    pub styles: Option<&'a str>,
    /// Develop: how many versions the photo has (0 or 1: no others).
    pub versions: u32,
    /// Develop: this entry is a version of a photo (a virtual copy in its own tile).
    pub is_version: bool,
    /// The manual albums it is in, full names one per line (the Library's; None where they
    /// are not known).
    pub albums: Option<&'a str>,
    /// The Exif fields beyond the camera settings above; None where not read (an older
    /// cache entry or catalogue row).
    pub exif: Option<ExifFacts<'a>>,
    /// The IPTC fields, `key=value` lines as `specular_metadata::edit::Record::to_lines`
    /// writes them (the Library's; None where not known).
    pub iptc: Option<&'a str>,
}

/// The values of IPTC field `key` in `lines` (see [`Facts::iptc`]).
pub fn iptc_values<'a>(lines: &'a str, key: &'a str) -> impl Iterator<Item = &'a str> + 'a {
    lines.lines().filter_map(move |l| {
        l.strip_prefix(key)
            .and_then(|r| r.strip_prefix('='))
            .filter(|v| !v.trim().is_empty())
    })
}

/// The IPTC keys `place:` reads: locations shown and created.
const IPTC_PLACE: [&str; 10] = [
    "shown-sublocation",
    "shown-city",
    "shown-state",
    "shown-country",
    "shown-country-code",
    "created-sublocation",
    "created-city",
    "created-state",
    "created-country",
    "created-country-code",
];

/// The IPTC keys `contact:` reads.
const IPTC_CONTACT: [&str; 8] = [
    "contact-address",
    "contact-city",
    "contact-region",
    "contact-postal-code",
    "contact-country",
    "contact-phone",
    "contact-email",
    "contact-url",
];

/// What a file's Exif says beyond [`Facts`]' camera settings (None: it does not say).
#[derive(Clone, Copy, Debug, Default)]
pub struct ExifFacts<'a> {
    pub make: Option<&'a str>,
    pub serial: Option<&'a str>,
    pub lens_serial: Option<&'a str>,
    pub software: Option<&'a str>,
    pub artist: Option<&'a str>,
    pub copyright: Option<&'a str>,
    /// The description or caption.
    pub description: Option<&'a str>,
    /// The Flash tag's bits.
    pub flash: Option<u16>,
    pub white_balance: Option<u8>,
    pub light_source: Option<u16>,
    /// EV.
    pub exposure_bias: Option<f32>,
    pub exposure_program: Option<u8>,
    pub exposure_mode: Option<u8>,
    pub metering: Option<u16>,
    /// The 35 mm equivalent focal length (mm).
    pub focal35: Option<f32>,
    /// Metres above sea level (below negative).
    pub altitude: Option<f32>,
    /// Every tag `exif:` names, as [`exif::push_tag`] writes them (the Library's; None where
    /// not loaded).
    pub tags: Option<&'a str>,
}

impl Facts<'_> {
    /// A movie: by its content once read, else its extension.
    pub fn is_video(&self) -> bool {
        !self.container && self.sound.map_or_else(|| is_video_ext(self.ext), |s| !s)
    }

    /// A sound file: by its content once read, else its extension.
    pub fn is_sound(&self) -> bool {
        !self.container && self.sound.unwrap_or_else(|| is_sound_ext(self.ext))
    }
}

/// A subframe's star metrics (sizes in pixels).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StarFacts {
    pub count: u32,
    pub hfd: Option<f32>,
    pub fwhm: Option<f32>,
    /// FWHM in arc seconds, when the plate scale is known.
    pub seeing: Option<f32>,
    pub ecc: Option<f32>,
    pub background: f32,
    pub snr: Option<f32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tri {
    Yes,
    No,
    Unknown,
}

impl Tri {
    fn from(b: bool) -> Tri {
        if b { Tri::Yes } else { Tri::No }
    }

    fn not(self) -> Tri {
        match self {
            Tri::Yes => Tri::No,
            Tri::No => Tri::Yes,
            Tri::Unknown => Tri::Unknown,
        }
    }

    /// Any of: yes if one is, else unknown if one is.
    fn any(it: impl IntoIterator<Item = Tri>) -> Tri {
        let mut out = Tri::No;
        for t in it {
            match t {
                Tri::Yes => return Tri::Yes,
                Tri::Unknown => out = Tri::Unknown,
                Tri::No => {}
            }
        }
        out
    }
}

/// Case and accent folding for text terms (the app passes its locale-aware one).
pub type Fold = for<'a> fn(&'a str) -> Cow<'a, str>;

/// ASCII lower case: the fold for tests and callers without one.
pub fn ascii_fold(s: &str) -> Cow<'_, str> {
    if s.bytes().any(|b| b.is_ascii_uppercase()) {
        Cow::Owned(s.to_ascii_lowercase())
    } else {
        Cow::Borrowed(s)
    }
}

/// The extension's one spelling per format, without allocating ("JPEG" → "jpg").
pub fn canon_ext(ext: &str) -> Cow<'_, str> {
    let e = ext.trim_start_matches('.');
    for (from, to) in [
        ("jpeg", "jpg"),
        ("jpe", "jpg"),
        ("jfif", "jpg"),
        ("tiff", "tif"),
        ("heif", "heic"),
    ] {
        if e.eq_ignore_ascii_case(from) {
            return Cow::Borrowed(to);
        }
    }
    ascii_fold(e)
}

enum Test {
    /// Folded needles: any contained in (or equal to, when exact) the folded value.
    Text {
        field: Field,
        exact: bool,
        any: Vec<String>,
    },
    /// Plain words and phrases: the name, or [`Facts::text`] where there is one.
    Words(Vec<String>),
    Ext(Vec<String>),
    Num(Field, Vec<parse::Range>),
    Date(Field, Vec<parse::Range>),
    Label(Vec<u8>),
    Orient(Vec<Orient>),
    Flags(Vec<Flag>),
    Group(Field, Option<bool>),
    /// `edited:` / `version:` words.
    Choice(Field, Vec<&'static str>),
    /// `module:`: the ids whose id or UI name the needles name, and the needles themselves
    /// (for ids without a name).
    Module {
        exact: bool,
        ids: Vec<String>,
        any: Vec<String>,
    },
    /// `exif:`: any of these (the tag's stored key, the condition, its text folded).
    Exif(Vec<(String, exif::Cond, String)>),
}

struct Compiled {
    negated: bool,
    /// The facet this term narrows (its bit in [`Matcher::fail_mask`]).
    bit: u64,
    test: Test,
}

/// A query ready to run against many files.
pub struct Matcher {
    terms: Vec<Compiled>,
    fold: Fold,
    needs_meta: bool,
}

/// The bit of terms that are not a facet's own selection.
pub const OTHER: u64 = 1 << 63;

impl Matcher {
    pub fn new(q: &Query, fold: Fold) -> Matcher {
        let terms = q
            .terms
            .iter()
            .map(|t| {
                let test = match &t.kind {
                    Kind::Text(s) => Test::Words(vec![fold(s).into_owned()]),
                    Kind::Field {
                        field: Field::Module,
                        pred: Pred::Text { exact, any },
                        ..
                    } => {
                        let any: Vec<String> = any.iter().map(|s| fold(s).into_owned()).collect();
                        let mut ids: Vec<String> = any
                            .iter()
                            .flat_map(|n| parse::module_ids(n, *exact, fold))
                            .collect();
                        ids.sort();
                        ids.dedup();
                        Test::Module {
                            exact: *exact,
                            ids,
                            any,
                        }
                    }
                    Kind::Field { field, pred, .. } => match pred {
                        Pred::Words(w) => Test::Choice(*field, w.clone()),
                        Pred::Text { exact, any } => Test::Text {
                            field: *field,
                            exact: *exact,
                            any: any.iter().map(|s| fold(s).into_owned()).collect(),
                        },
                        Pred::Ext(v) => Test::Ext(v.clone()),
                        Pred::Num(r) => Test::Num(*field, r.clone()),
                        Pred::Date(r) => Test::Date(*field, r.clone()),
                        Pred::Label(v) => Test::Label(v.clone()),
                        Pred::Orient(v) => Test::Orient(v.clone()),
                        Pred::Flags(v) => Test::Flags(v.clone()),
                        Pred::Group(g) => Test::Group(*field, *g),
                        Pred::Exif(c) => Test::Exif(
                            c.iter()
                                .map(|c| {
                                    let text = match &c.want {
                                        exif::Want::Is { text, .. } => fold(text).into_owned(),
                                        _ => String::new(),
                                    };
                                    (c.tag.key(), c.clone(), text)
                                })
                                .collect(),
                        ),
                    },
                };
                let bit = match t.field() {
                    Some(f) if !t.negated => FacetField::of(f).map_or(OTHER, FacetField::bit),
                    _ => OTHER,
                };
                Compiled {
                    negated: t.negated,
                    bit,
                    test,
                }
            })
            .collect();
        Matcher {
            terms,
            fold,
            needs_meta: q.needs_meta(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.terms.is_empty()
    }

    /// Whether any term reads image metadata, so every image's metadata should be read.
    pub fn needs_meta(&self) -> bool {
        self.needs_meta
    }

    /// Whether the file passes (unknown passes).
    pub fn matches(&self, f: &Facts) -> bool {
        self.terms.iter().all(|t| self.eval(t, f) != Tri::No)
    }

    /// The file against every term: no if one fails, else unknown if one is unknown.
    pub fn tri(&self, f: &Facts) -> Tri {
        let mut out = Tri::Yes;
        for t in &self.terms {
            match self.eval(t, f) {
                Tri::No => return Tri::No,
                Tri::Unknown => out = Tri::Unknown,
                Tri::Yes => {}
            }
        }
        out
    }

    /// The bits of the terms the file fails: [`FacetField::bit`] for a facet's own selection,
    /// [`OTHER`] for the rest. Zero means it matches.
    pub fn fail_mask(&self, f: &Facts) -> u64 {
        let mut m = 0;
        for t in &self.terms {
            if m & t.bit == 0 && self.eval(t, f) == Tri::No {
                m |= t.bit;
            }
        }
        m
    }

    fn eval(&self, t: &Compiled, f: &Facts) -> Tri {
        let r = self.test(&t.test, f);
        if t.negated { r.not() } else { r }
    }

    fn test(&self, t: &Test, f: &Facts) -> Tri {
        let meta = |v: Tri| if f.known { v } else { Tri::Unknown };
        if f.container {
            return match t {
                Test::Text {
                    field: Field::Name, ..
                }
                | Test::Words(_) => self.text(t, Some(f.name)),
                Test::Flags(fl) => Tri::from(fl.contains(&Flag::Folder)),
                _ => Tri::No,
            };
        }
        match t {
            Test::Text { field, .. } => match field {
                Field::Camera => meta(self.text(t, f.camera)),
                Field::Lens => meta(self.text(t, f.lens)),
                Field::Tag => match f.tags {
                    Some(tags) => self.lines(t, tags),
                    None => Tri::Unknown,
                },
                Field::Folder => match f.folder {
                    Some(d) => self.text(t, Some(d)),
                    None => Tri::Unknown,
                },
                Field::Space => match f.space {
                    Some(s) if f.known => self.space(t, s),
                    _ => Tri::Unknown,
                },
                Field::Place => {
                    self.with_iptc(t, f, &IPTC_PLACE, f.place.map(|p| self.lines(t, p)))
                }
                Field::Person => {
                    self.with_iptc(t, f, &["persons-shown"], f.people.map(|p| self.lines(t, p)))
                }
                Field::Album => match f.albums {
                    Some(a) => self.lines(t, a),
                    None => Tri::Unknown,
                },
                Field::Object => meta(self.text(t, f.object)),
                Field::Filter => meta(self.text(t, f.filter)),
                Field::FrameType => meta(self.text(t, f.frame_type)),
                Field::Style => match (f.edited, f.styles) {
                    (Some(false), _) => Tri::No,
                    (_, Some(s)) => self.lines(t, s),
                    _ => Tri::Unknown,
                },
                Field::Make => self.exif(f, |e| self.text(t, e.make)),
                Field::Serial => self.exif(f, |e| self.text(t, e.serial)),
                Field::LensSerial => self.exif(f, |e| self.text(t, e.lens_serial)),
                Field::Software => self.exif(f, |e| self.text(t, e.software)),
                Field::Artist => self.iptc_first(t, f, "creator", |e| e.artist),
                Field::Copyright => self.iptc_first(t, f, "copyright", |e| e.copyright),
                Field::Description => self.iptc_first(t, f, "description", |e| e.description),
                Field::Title => self.iptc(t, f, &["title"]),
                Field::Headline => self.iptc(t, f, &["headline"]),
                Field::Credit => self.iptc(t, f, &["credit"]),
                Field::Source => self.iptc(t, f, &["source"]),
                Field::UsageTerms => self.iptc(t, f, &["usage-terms"]),
                Field::Instructions => self.iptc(t, f, &["instructions"]),
                Field::JobId => self.iptc(t, f, &["job-id"]),
                Field::Event => self.iptc(t, f, &["event"]),
                Field::Contact => self.iptc(t, f, &IPTC_CONTACT),
                _ => self.text(t, Some(f.name)),
            },
            Test::Words(_) => self.text(t, Some(f.text.unwrap_or(f.name))),
            Test::Ext(v) => {
                let e = canon_ext(f.ext);
                Tri::from(v.iter().any(|x| *x == e))
            }
            Test::Num(field, ranges) => {
                let v = match field {
                    Field::Rating => Some(f64::from(f.rating)),
                    Field::Size => match f.size {
                        Some(s) => Some(s as f64),
                        None => return Tri::Unknown,
                    },
                    _ if !f.known => return Tri::Unknown,
                    Field::Iso => f.iso.map(f64::from),
                    Field::Aperture => f.f_number.map(f64::from),
                    Field::Focal => f.focal.map(f64::from),
                    Field::Exposure => f.exposure.map(f64::from),
                    Field::Width => (f.width > 0).then_some(f64::from(f.width)),
                    Field::Height => (f.height > 0).then_some(f64::from(f.height)),
                    Field::Megapixels => (f.width > 0 && f.height > 0)
                        .then(|| f64::from(f.width) * f64::from(f.height) / 1e6),
                    Field::Bits => (f.bit_depth > 0).then_some(f64::from(f.bit_depth)),
                    Field::Pages => Some(f64::from(f.pages.max(1))),
                    Field::Gain => f.gain.map(f64::from),
                    Field::ExpComp | Field::Focal35 | Field::Altitude => match f.exif {
                        None => return Tri::Unknown,
                        Some(e) => match field {
                            Field::ExpComp => e.exposure_bias,
                            Field::Focal35 => e.focal35,
                            _ => e.altitude,
                        }
                        .map(f64::from),
                    },
                    Field::Temp => f.ccd_temp.map(f64::from),
                    Field::Hfd
                    | Field::Fwhm
                    | Field::Seeing
                    | Field::Ecc
                    | Field::StarCount
                    | Field::Background
                    | Field::Snr => match (f.subframe, f.stars) {
                        // A subframe not measured yet.
                        (true, None) => return Tri::Unknown,
                        (_, None) => None,
                        (_, Some(s)) => match field {
                            Field::Hfd => s.hfd,
                            Field::Fwhm => s.fwhm,
                            Field::Seeing => s.seeing,
                            Field::Ecc => s.ecc,
                            Field::StarCount => Some(s.count as f32),
                            Field::Background => Some(s.background),
                            _ => s.snr,
                        }
                        .map(f64::from),
                    },
                    _ => None,
                };
                Tri::from(v.is_some_and(|v| ranges.iter().any(|r| r.contains(v))))
            }
            Test::Date(field, ranges) => {
                let v = match field {
                    Field::Modified => match f.modified {
                        Some(m) => m,
                        None => return Tri::Unknown,
                    },
                    _ => match f.taken {
                        Some(t) => t,
                        None if !f.known => return Tri::Unknown,
                        None => match f.modified {
                            Some(m) => m,
                            None => return Tri::No,
                        },
                    },
                };
                Tri::from(ranges.iter().any(|r| r.contains(v as f64)))
            }
            Test::Label(v) => Tri::from(v.contains(&f.label)),
            Test::Orient(v) => meta(Tri::from(
                f.width > 0 && f.height > 0 && v.contains(&Orient::of(f.width, f.height)),
            )),
            Test::Flags(fl) => Tri::any(fl.iter().map(|fl| flag(*fl, f))),
            Test::Choice(
                field @ (Field::Flash | Field::WhiteBalance | Field::Program | Field::Metering),
                want,
            ) => self.exif(f, |e| {
                Tri::from(want.iter().any(|w| exif_word(*field, &e, w)))
            }),
            Test::Exif(conds) => self.exif(f, |e| match e.tags {
                None => Tri::Unknown,
                Some(tags) => Tri::from(
                    conds
                        .iter()
                        .any(|(key, c, text)| self.exif_cond(tags, key, c, text)),
                ),
            }),
            Test::Choice(field, want) => {
                if *field == Field::Edited {
                    match f.edited {
                        Some(e) => Tri::from(want.contains(&if e { "yes" } else { "no" })),
                        None => Tri::Unknown,
                    }
                } else {
                    let w = if f.is_version {
                        "copy"
                    } else if f.versions > 1 {
                        "yes"
                    } else {
                        "no"
                    };
                    Tri::from(want.contains(&w))
                }
            }
            Test::Module { exact, ids, any } => match (f.edited, f.modules) {
                (Some(false), _) => Tri::No,
                (_, Some(m)) => Tri::from(m.lines().map(str::trim).any(|l| {
                    ids.iter().any(|i| i == l)
                        || any.iter().any(|n| {
                            if *exact {
                                l == n.trim()
                            } else {
                                l.contains(n.as_str())
                            }
                        })
                })),
                _ => Tri::Unknown,
            },
            Test::Group(field, want) => {
                let role = if *field == Field::Dupes {
                    f.dupes
                } else {
                    f.similar
                };
                Tri::from(match (role, want) {
                    (None, _) => false,
                    (Some(_), None) => true,
                    (Some(k), Some(w)) => k == *w,
                })
            }
        }
    }

    fn text(&self, t: &Test, value: Option<&str>) -> Tri {
        let (exact, any) = match t {
            Test::Text { exact, any, .. } => (*exact, any),
            Test::Words(any) => (false, any),
            _ => return Tri::No,
        };
        let Some(v) = value else {
            return Tri::No;
        };
        let v = (self.fold)(v);
        Tri::from(any.iter().any(|n| {
            if exact {
                v.trim() == n.trim()
            } else {
                v.contains(n.as_str())
            }
        }))
    }
}

impl Matcher {
    /// A test of IPTC fields `keys`: any of their values; unknown until they are read.
    fn iptc(&self, t: &Test, f: &Facts, keys: &[&str]) -> Tri {
        match f.iptc {
            Some(lines) => Tri::any(
                keys.iter()
                    .flat_map(|k| iptc_values(lines, k))
                    .map(|v| self.text(t, Some(v))),
            ),
            None => Tri::Unknown,
        }
    }

    /// IPTC fields `keys` or `other` (the catalogue's places, people): yes when either is.
    fn with_iptc(&self, t: &Test, f: &Facts, keys: &[&str], other: Option<Tri>) -> Tri {
        if f.iptc.is_none() {
            return other.unwrap_or(Tri::Unknown);
        }
        // Without the other source (People not installed), IPTC alone answers.
        match (self.iptc(t, f, keys), other) {
            (Tri::Yes, _) => Tri::Yes,
            (_, Some(o)) => o,
            (i, None) => i,
        }
    }

    /// IPTC field `key` where the file has it (the Metadata Working Group prefers it), else
    /// the Exif field.
    fn iptc_first<'a>(
        &self,
        t: &Test,
        f: &Facts<'a>,
        key: &str,
        exif: impl Fn(ExifFacts<'a>) -> Option<&'a str>,
    ) -> Tri {
        if let Some(lines) = f.iptc
            && iptc_values(lines, key).next().is_some()
        {
            return Tri::any(iptc_values(lines, key).map(|v| self.text(t, Some(v))));
        }
        match f.exif {
            Some(e) if f.known => self.text(t, exif(e)),
            _ => Tri::Unknown,
        }
    }

    /// A test of the Exif fields: unknown until they are read.
    fn exif(&self, f: &Facts, test: impl FnOnce(ExifFacts) -> Tri) -> Tri {
        match f.exif {
            Some(e) if f.known => test(e),
            _ => Tri::Unknown,
        }
    }

    /// One `exif:` condition against a file's tags.
    fn exif_cond(&self, tags: &str, key: &str, c: &exif::Cond, text: &str) -> bool {
        let v = exif::value_of(tags, key);
        let r = match (&c.want, v) {
            (exif::Want::Has, v) => v.is_some(),
            (_, None) => false,
            (exif::Want::Cmp(r), Some(v)) => exif::first_number(v).is_some_and(|x| r.contains(x)),
            (
                exif::Want::Is {
                    num, codes, flash, ..
                },
                Some(v),
            ) => {
                if let Some(w) = flash
                    && let Ok(bits) = v.trim().parse::<u16>()
                {
                    exif::flash_is(bits, w)
                } else if !codes.is_empty()
                    && let Ok(code) = v.trim().parse::<u32>()
                {
                    codes.contains(&code)
                } else if let (Some(r), Some(x)) = (num, exif::first_number(v)) {
                    r.contains(x)
                } else {
                    (self.fold)(v).contains(text)
                }
            }
        };
        r != c.not
    }

    /// A colour space test: a word of [`SPACE_WORDS`] is the space itself; other needles are
    /// text in a profile's description (contained, or the whole of it with `=`).
    fn space(&self, t: &Test, v: &str) -> Tri {
        let Test::Text { exact, any, .. } = t else {
            return Tri::No;
        };
        if v.is_empty() {
            return Tri::No;
        }
        let named = SPACE_WORDS.contains(&v);
        Tri::from(any.iter().any(|n| {
            if SPACE_WORDS.contains(&n.as_str()) {
                v == n
            } else if named {
                false
            } else {
                let v = (self.fold)(v);
                if *exact {
                    v.trim() == n.trim()
                } else {
                    v.contains(n.as_str())
                }
            }
        }))
    }

    /// A text test against each line of `v` (any line passing passes).
    fn lines(&self, t: &Test, v: &str) -> Tri {
        Tri::any(v.lines().map(|l| self.text(t, Some(l))))
    }
}

/// Whether a file's Exif is `word` of a named field (`flash:`, `wb:`, `program:`,
/// `metering:`).
pub(crate) fn exif_word(field: Field, e: &ExifFacts, word: &str) -> bool {
    match field {
        Field::Flash => e.flash.is_some_and(|v| exif::flash_is(v, word)),
        Field::WhiteBalance => exif::wb_is(e.white_balance, e.light_source, word),
        Field::Program => exif::program_is(e.exposure_program, e.exposure_mode, word),
        _ => e.metering.is_some_and(|v| exif::metering_is(v, word)),
    }
}

pub(crate) fn flag(fl: Flag, f: &Facts) -> Tri {
    let meta = |b: bool| if f.known { Tri::from(b) } else { Tri::Unknown };
    let orient =
        |o: Orient| meta(f.width > 0 && f.height > 0 && Orient::of(f.width, f.height) == o);
    match fl {
        Flag::Pick => Tri::from(f.pick),
        Flag::Reject => Tri::from(f.reject),
        Flag::Flagged => Tri::from(f.pick || f.reject),
        Flag::Rated => Tri::from(f.rating > 0),
        Flag::Unrated => Tri::from(f.rating == 0),
        Flag::Labelled => Tri::from(f.label > 0),
        Flag::Folder => Tri::from(f.container),
        Flag::Image => Tri::from(!f.container),
        Flag::Raw => meta(f.raw),
        Flag::Hdr => meta(f.hdr),
        Flag::Animated => meta(f.animated),
        Flag::MultiPage => meta(f.pages > 1),
        Flag::Landscape => orient(Orient::Landscape),
        Flag::Portrait => orient(Orient::Portrait),
        Flag::Square => orient(Orient::Square),
        Flag::Alpha => meta(f.alpha),
        Flag::Profile => meta(f.profile),
        Flag::Camera => meta(f.camera.is_some()),
        Flag::Lens => meta(f.lens.is_some()),
        Flag::Date => meta(f.taken.is_some()),
        Flag::Video => Tri::from(f.is_video()),
        Flag::Sound => Tri::from(f.is_sound()),
        Flag::Gps => match f.gps {
            Some(g) if f.known => Tri::from(g),
            _ => Tri::Unknown,
        },
    }
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_astro;
#[cfg(test)]
mod tests_develop;
#[cfg(test)]
mod tests_exif;
#[cfg(test)]
mod tests_iptc;
