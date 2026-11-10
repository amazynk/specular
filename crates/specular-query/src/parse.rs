//! The query language: words, quoted phrases and `field:value` terms, each optionally negated
//! with a leading `-`. Terms are ANDed; commas in a value are alternatives (OR).
//!
//! ```text
//! query   = { ws } { term { ws } }
//! term    = [ "-" ] ( phrase | field-term | word )
//! field-term = name ( ":" [ op ] | op ) items
//! op      = ">=" | "<=" | ">" | "<" | "=" | "!="
//! items   = item { "," item }
//! item    = phrase | bare            ; bare stops at whitespace, "," and '"'
//! phrase  = '"' { char | '\"' | '\\' } '"'   ; escapes: a quote, a backslash
//! range   = [ bound ] ".." [ bound ] ; inside an item, with ":" or "=" only
//! ```
//!
//! Parsing never fails: a term that does not make sense is left out of the query and reported
//! as an [`Error`] with the byte span to underline, so the rest of the query keeps working
//! while the user types.

use crate::date;

/// Byte offsets into the query text.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    fn new(start: usize, end: usize) -> Span {
        Span { start, end }
    }
}

/// What a field term looks at.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Field {
    Name,
    Ext,
    Camera,
    Lens,
    Iso,
    Aperture,
    Focal,
    Exposure,
    /// Capture time, else modification time.
    Date,
    Modified,
    Width,
    Height,
    Megapixels,
    Size,
    Rating,
    Label,
    Orientation,
    Bits,
    Pages,
    Is,
    Has,
    Dupes,
    Similar,
    /// Keywords and tags (XMP `dc:subject`, the catalogue's tags): Library only.
    Tag,
    /// The folder a file is in, as a path: Library only.
    Folder,
    /// The colour space: a word of [`SPACE_WORDS`] (and its aliases), else a profile's
    /// description.
    Space,
    /// Where a photo was taken: town, region or country (from its GPS position, offline):
    /// Library only.
    Place,
    /// A person named in the People view (faces grouped by the Local AI pack): Library only.
    Person,
    /// Astronomy subframes (FITS, XISF): `OBJECT`, `FILTER` and the frame's kind from
    /// `IMAGETYP` (`light`, `dark`, `flat`, `bias`, `darkflat`).
    Object,
    Filter,
    FrameType,
    /// `GAIN` and `CCD-TEMP` (°C).
    Gain,
    Temp,
    /// Star metrics: median HFD and FWHM (px), FWHM in arc seconds (`seeing`, needs the plate
    /// scale), median eccentricity, star count, background level, SNR.
    Hfd,
    Fwhm,
    Seeing,
    Ecc,
    StarCount,
    Background,
    Snr,
    /// Develop: whether the photo has an edit (`yes`, `no`).
    Edited,
    /// Develop: a module its edit uses, by stable id (`exposure`) or by name in the UI
    /// language ([`set_module_names`]).
    Module,
    /// Develop: a style applied to it (from the edit's history).
    Style,
    /// Develop versions: [`VERSION_WORDS`].
    Version,
    /// The Library's manual albums a file is in, by their full names (`Client X/selects`):
    /// Library only.
    Album,
    /// Exif: the camera's maker alone (`camera:` is maker and model).
    Make,
    /// Exif Flash: [`crate::exif::FLASH_WORDS`].
    Flash,
    /// Exif WhiteBalance and LightSource: [`crate::exif::WB_WORDS`].
    WhiteBalance,
    /// Exif ExposureBiasValue (EV).
    ExpComp,
    /// Exif ExposureProgram and ExposureMode: [`crate::exif::PROGRAM_WORDS`].
    Program,
    /// Exif MeteringMode: [`crate::exif::METERING_WORDS`].
    Metering,
    /// The 35 mm equivalent focal length (mm).
    Focal35,
    /// Exif BodySerialNumber, LensSerialNumber, Software, Artist, Copyright.
    Serial,
    LensSerial,
    Software,
    Artist,
    Copyright,
    /// The image's description or caption (Exif ImageDescription).
    Description,
    /// GPS altitude in metres (below sea level negative).
    Altitude,
    /// Any Exif tag by its standard name (`exif:SceneCaptureType=night`): Library only.
    Exif,
    /// IPTC (XMP sidecar, else the file's XMP, else its IIM): title, headline, credit line,
    /// source, usage terms, instructions, job ID, event and the creator's contact details:
    /// Library only. `artist:`, `copyright:` and `caption:` read IPTC's creator, copyright
    /// notice and description first; `place:` and `person:` also find IPTC locations and
    /// persons shown.
    Title,
    Headline,
    Credit,
    Source,
    UsageTerms,
    Instructions,
    JobId,
    Event,
    Contact,
}

/// Field names and their aliases; the first name of each field is the one it is shown by.
pub(crate) const FIELDS: &[(&str, Field)] = &[
    ("name", Field::Name),
    ("file", Field::Name),
    ("ext", Field::Ext),
    ("format", Field::Ext),
    ("type", Field::Ext),
    ("camera", Field::Camera),
    ("cam", Field::Camera),
    ("model", Field::Camera),
    ("lens", Field::Lens),
    ("iso", Field::Iso),
    ("f", Field::Aperture),
    ("aperture", Field::Aperture),
    ("fnumber", Field::Aperture),
    ("focal", Field::Focal),
    ("mm", Field::Focal),
    ("exposure", Field::Exposure),
    ("shutter", Field::Exposure),
    ("date", Field::Date),
    ("taken", Field::Date),
    ("modified", Field::Modified),
    ("mtime", Field::Modified),
    ("w", Field::Width),
    ("width", Field::Width),
    ("h", Field::Height),
    ("height", Field::Height),
    ("mp", Field::Megapixels),
    ("megapixels", Field::Megapixels),
    ("size", Field::Size),
    ("rating", Field::Rating),
    ("stars", Field::Rating),
    ("label", Field::Label),
    ("color", Field::Label),
    ("colour", Field::Label),
    ("orientation", Field::Orientation),
    ("bits", Field::Bits),
    ("depth", Field::Bits),
    ("pages", Field::Pages),
    ("is", Field::Is),
    ("has", Field::Has),
    ("dupes", Field::Dupes),
    ("similar", Field::Similar),
    ("tag", Field::Tag),
    ("tags", Field::Tag),
    ("keyword", Field::Tag),
    ("keywords", Field::Tag),
    ("folder", Field::Folder),
    ("in", Field::Folder),
    ("path", Field::Folder),
    ("space", Field::Space),
    ("colorspace", Field::Space),
    ("colourspace", Field::Space),
    ("profile", Field::Space),
    ("icc", Field::Space),
    ("place", Field::Place),
    ("location", Field::Place),
    ("city", Field::Place),
    ("country", Field::Place),
    ("region", Field::Place),
    ("person", Field::Person),
    ("people", Field::Person),
    ("object", Field::Object),
    ("target", Field::Object),
    ("filter", Field::Filter),
    ("imagetyp", Field::FrameType),
    ("frametype", Field::FrameType),
    ("frame", Field::FrameType),
    ("gain", Field::Gain),
    ("temp", Field::Temp),
    ("ccdtemp", Field::Temp),
    ("hfd", Field::Hfd),
    ("fwhm", Field::Fwhm),
    ("seeing", Field::Seeing),
    ("ecc", Field::Ecc),
    ("eccentricity", Field::Ecc),
    ("starcount", Field::StarCount),
    ("nstars", Field::StarCount),
    ("bg", Field::Background),
    ("background", Field::Background),
    ("snr", Field::Snr),
    ("edited", Field::Edited),
    ("developed", Field::Edited),
    ("module", Field::Module),
    ("modules", Field::Module),
    ("style", Field::Style),
    ("styles", Field::Style),
    ("version", Field::Version),
    ("versions", Field::Version),
    ("album", Field::Album),
    ("albums", Field::Album),
    ("make", Field::Make),
    ("maker", Field::Make),
    ("flash", Field::Flash),
    ("wb", Field::WhiteBalance),
    ("whitebalance", Field::WhiteBalance),
    ("ev", Field::ExpComp),
    ("expcomp", Field::ExpComp),
    ("program", Field::Program),
    ("mode", Field::Program),
    ("metering", Field::Metering),
    ("focal35", Field::Focal35),
    ("serial", Field::Serial),
    ("bodyserial", Field::Serial),
    ("lensserial", Field::LensSerial),
    ("software", Field::Software),
    ("artist", Field::Artist),
    ("author", Field::Artist),
    ("creator", Field::Artist),
    ("byline", Field::Artist),
    ("copyright", Field::Copyright),
    ("description", Field::Description),
    ("caption", Field::Description),
    ("altitude", Field::Altitude),
    ("alt", Field::Altitude),
    ("exif", Field::Exif),
    ("title", Field::Title),
    ("headline", Field::Headline),
    ("credit", Field::Credit),
    ("source", Field::Source),
    ("usage", Field::UsageTerms),
    ("usageterms", Field::UsageTerms),
    ("instructions", Field::Instructions),
    ("job", Field::JobId),
    ("jobid", Field::JobId),
    ("event", Field::Event),
    ("contact", Field::Contact),
    ("sublocation", Field::Place),
    ("state", Field::Place),
    ("province", Field::Place),
    ("countrycode", Field::Place),
    // FITS spellings of fields above.
    ("exptime", Field::Exposure),
    ("dateobs", Field::Date),
];

/// The colour spaces with a name of their own (others are their profile's description), in
/// the order lists show them: sRGB (and BT.709), Display P3, Adobe RGB (1998), ProPhoto RGB,
/// BT.2020 primaries, BT.2100 PQ and HLG, greyscale, CMYK and CIE L*a*b*. The words
/// `specular_decode::colour_space` gives.
pub const SPACE_WORDS: [&str; 10] = [
    "srgb", "p3", "adobergb", "prophoto", "rec2020", "pq", "hlg", "grey", "cmyk", "lab",
];

/// The word of [`SPACE_WORDS`] a typed value means (any case and punctuation: `Display P3`,
/// `gray`, `BT.2020`, `Adobe`), if any.
pub fn space_word(s: &str) -> Option<&'static str> {
    let k: String = s
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect();
    Some(match k.as_str() {
        "srgb" | "rec709" | "bt709" | "709" => "srgb",
        "p3" | "displayp3" | "dcip3" | "p3d65" => "p3",
        "adobergb" | "adobe" | "adobergb1998" | "oprgb" => "adobergb",
        "prophoto" | "prophotorgb" | "romm" | "rommrgb" => "prophoto",
        "rec2020" | "bt2020" | "2020" => "rec2020",
        "pq" | "rec2100pq" | "bt2100pq" | "hdr10" | "smpte2084" => "pq",
        "hlg" | "rec2100hlg" | "bt2100hlg" => "hlg",
        "grey" | "gray" | "greyscale" | "grayscale" => "grey",
        "cmyk" => "cmyk",
        "lab" | "cielab" => "lab",
        _ => return None,
    })
}

/// Fields DESIGN.md §10 names that are understood but not available yet: reported as such
/// and left out of the query.
const UNAVAILABLE: &[&str] = &[];

/// The AI pack feature a field needs (DESIGN.md "Local AI pack").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AiFeature {
    /// `text:` (text in images).
    Text,
    /// `people:`, `person:`.
    People,
    /// `like:` (search by description, the Library only).
    Search,
}

/// Fields answered from the AI pack's data: understood, reported as needing the pack, and
/// left out of the query (nothing evaluates them yet, installed or not).
const AI_FIELDS: &[(&str, AiFeature)] = &[("text", AiFeature::Text), ("like", AiFeature::Search)];

/// Where a query runs: what it can ask about.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Scope {
    /// One folder's listing: no tags, no folder paths (reported as not available here).
    #[default]
    Gallery,
    /// The catalogue: every field.
    Library,
}

impl Field {
    /// The name the field is shown by.
    pub fn name(self) -> &'static str {
        FIELDS
            .iter()
            .find(|(_, f)| *f == self)
            .map_or("", |(n, _)| n)
    }

    /// Whether only the Library knows the field's values (a folder's photos' keywords are
    /// read from their sidecars and files).
    pub fn library_only(self) -> bool {
        matches!(
            self,
            Field::Folder
                | Field::Place
                | Field::Person
                | Field::Album
                | Field::Exif
                | Field::Title
                | Field::Headline
                | Field::Credit
                | Field::Source
                | Field::UsageTerms
                | Field::Instructions
                | Field::JobId
                | Field::Event
                | Field::Contact
        )
    }

    pub fn from_name(s: &str) -> Option<Field> {
        FIELDS
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(s))
            .map(|(_, f)| *f)
    }

    /// Whether the field is read from the image's metadata (rather than the listing or the
    /// culling marks), so filtering on it needs every image's metadata.
    pub fn needs_meta(self) -> bool {
        !matches!(
            self,
            Field::Name
                | Field::Ext
                | Field::Size
                | Field::Modified
                | Field::Rating
                | Field::Label
                | Field::Dupes
                | Field::Similar
                | Field::Tag
                | Field::Folder
                | Field::Edited
                | Field::Module
                | Field::Style
                | Field::Version
                | Field::Album
                | Field::Title
                | Field::Headline
                | Field::Credit
                | Field::Source
                | Field::UsageTerms
                | Field::Instructions
                | Field::JobId
                | Field::Event
                | Field::Contact
        )
    }
}

/// `edited:` values.
pub const EDITED_WORDS: [&str; 2] = ["yes", "no"];

/// `version:` values: has versions (more than one), has none, is a version (a virtual copy
/// shown in its own tile).
pub const VERSION_WORDS: [&str; 3] = ["yes", "no", "copy"];

/// Develop modules' names in the UI language: (stable id, name). `module:` matches either.
static MODULE_NAMES: std::sync::RwLock<Vec<(String, String)>> = std::sync::RwLock::new(Vec::new());

/// Sets the Develop modules' names in the UI language (stable id, name), for `module:`
/// terms and the picker's labels. Matchers made before keep the names they had.
pub fn set_module_names(names: Vec<(String, String)>) {
    if let Ok(mut m) = MODULE_NAMES.write() {
        *m = names;
    }
}

/// The name of module `id` in the UI language, if set.
pub fn module_name(id: &str) -> Option<String> {
    MODULE_NAMES
        .read()
        .ok()?
        .iter()
        .find(|(i, _)| i == id)
        .map(|(_, n)| n.clone())
}

/// The ids of the modules whose id or name `needle` (folded with `fold`) is in, or is
/// (`exact`).
pub(crate) fn module_ids(needle: &str, exact: bool, fold: crate::Fold) -> Vec<String> {
    let Ok(m) = MODULE_NAMES.read() else {
        return Vec::new();
    };
    m.iter()
        .filter(|(id, name)| {
            [id.as_str(), name.as_str()].into_iter().any(|v| {
                let v = fold(v);
                if exact {
                    v.trim() == needle.trim()
                } else {
                    v.contains(needle)
                }
            })
        })
        .map(|(id, _)| id.clone())
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Op {
    /// `:` (contains for text, equals for the rest).
    Has,
    /// `=` (whole value for text).
    Eq,
    Gt,
    Ge,
    Lt,
    Le,
}

impl Op {
    pub fn text(self) -> &'static str {
        match self {
            Op::Has => ":",
            Op::Eq => "=",
            Op::Gt => ">",
            Op::Ge => ">=",
            Op::Lt => "<",
            Op::Le => "<=",
        }
    }
}

/// An interval of numbers (or seconds, for dates).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Range {
    pub lo: f64,
    pub lo_open: bool,
    pub hi: f64,
    pub hi_open: bool,
}

impl Range {
    pub const ALL: Range = Range {
        lo: f64::NEG_INFINITY,
        lo_open: false,
        hi: f64::INFINITY,
        hi_open: false,
    };

    pub fn contains(&self, v: f64) -> bool {
        let above = if self.lo_open {
            v > self.lo
        } else {
            v >= self.lo
        };
        let below = if self.hi_open {
            v < self.hi
        } else {
            v <= self.hi
        };
        above && below
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Orient {
    Landscape,
    Portrait,
    Square,
}

impl Orient {
    pub const ALL: [Orient; 3] = [Orient::Landscape, Orient::Portrait, Orient::Square];

    pub fn word(self) -> &'static str {
        match self {
            Orient::Landscape => "landscape",
            Orient::Portrait => "portrait",
            Orient::Square => "square",
        }
    }

    pub fn of(w: u32, h: u32) -> Orient {
        match w.cmp(&h) {
            std::cmp::Ordering::Greater => Orient::Landscape,
            std::cmp::Ordering::Less => Orient::Portrait,
            std::cmp::Ordering::Equal => Orient::Square,
        }
    }
}

/// The words of `is:` and `has:` (one vocabulary for both: `is:hdr` and `has:hdr` agree).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Flag {
    Pick,
    Reject,
    Flagged,
    Rated,
    Unrated,
    Labelled,
    Raw,
    Hdr,
    Animated,
    MultiPage,
    Landscape,
    Portrait,
    Square,
    Folder,
    Image,
    Gps,
    Alpha,
    Profile,
    Camera,
    Lens,
    Date,
    /// A movie (MP4, MOV, M4V, 3GP, 3G2, MKV, WebM, MPEG-TS, MPEG-PS, MPEG video, AVI, Ogg,
    /// IVF, MXF): by its content once read (a picture track), else its extension.
    Video,
    /// A sound file (MP3, AAC, M4A, FLAC, WAV, AIFF, Ogg, Opus, MKA, AC-3, DTS...): by its
    /// content once read (sound without a picture), else its extension.
    Sound,
}

/// Extensions of movies ([`Flag::Video`]), lower case.
pub const VIDEO_EXTS: [&str; 35] = [
    "mp4", "m4v", "mov", "qt", "3gp", "3g2", "mkv", "mk3d", "webm", "mts", "m2ts", "ts", "m2t",
    "tod", "mpg", "mpeg", "mpe", "vob", "mod", "vro", "m2p", "m1v", "m2v", "mpv", "avi", "divx",
    "ogv", "ogm", "ivf", "mxf", "flv", "wmv", "asf", "dv", "dif",
];

/// Whether `ext` (any case, no dot) is a movie's.
pub fn is_video_ext(ext: &str) -> bool {
    VIDEO_EXTS.iter().any(|e| e.eq_ignore_ascii_case(ext))
}

/// Extensions of sound files ([`Flag::Sound`]), lower case. `.ogg` is usually sound only
/// (an Ogg movie is told by its content).
pub const SOUND_EXTS: [&str; 26] = [
    "mp3", "mp2", "mpga", "aac", "adts", "flac", "wav", "aiff", "aif", "aifc", "dts", "ac3",
    "eac3", "oga", "ogg", "opus", "spx", "m4a", "m4b", "m4p", "mka", "wma", "ape", "mpc", "wv",
    "caf",
];

/// Whether `ext` (any case, no dot) is a sound file's.
pub fn is_sound_ext(ext: &str) -> bool {
    SOUND_EXTS.iter().any(|e| e.eq_ignore_ascii_case(ext))
}

const FLAG_WORDS: &[(&str, Flag)] = &[
    ("pick", Flag::Pick),
    ("picked", Flag::Pick),
    ("reject", Flag::Reject),
    ("rejected", Flag::Reject),
    ("flagged", Flag::Flagged),
    ("rated", Flag::Rated),
    ("unrated", Flag::Unrated),
    ("labelled", Flag::Labelled),
    ("labeled", Flag::Labelled),
    ("raw", Flag::Raw),
    ("hdr", Flag::Hdr),
    ("animated", Flag::Animated),
    ("multipage", Flag::MultiPage),
    ("landscape", Flag::Landscape),
    ("portrait", Flag::Portrait),
    ("square", Flag::Square),
    ("folder", Flag::Folder),
    ("image", Flag::Image),
    ("gps", Flag::Gps),
    ("location", Flag::Gps),
    ("alpha", Flag::Alpha),
    ("transparency", Flag::Alpha),
    ("profile", Flag::Profile),
    ("icc", Flag::Profile),
    ("camera", Flag::Camera),
    ("lens", Flag::Lens),
    ("date", Flag::Date),
    ("video", Flag::Video),
    ("videos", Flag::Video),
    ("movie", Flag::Video),
    ("sound", Flag::Sound),
    ("sounds", Flag::Sound),
    ("audio", Flag::Sound),
    ("music", Flag::Sound),
];

impl Flag {
    /// The flag a word of `is:` / `has:` names (aliases too: `picked`, `location`).
    pub fn from_word(s: &str) -> Option<Flag> {
        FLAG_WORDS
            .iter()
            .find(|(w, _)| w.eq_ignore_ascii_case(s.trim()))
            .map(|(_, f)| *f)
    }

    pub fn word(self) -> &'static str {
        FLAG_WORDS
            .iter()
            .find(|(_, f)| *f == self)
            .map_or("", |(w, _)| w)
    }

    /// The field it is written with by default (`is:raw`, `has:gps`).
    pub fn field(self) -> Field {
        match self {
            Flag::Gps | Flag::Alpha | Flag::Profile | Flag::Camera | Flag::Lens | Flag::Date => {
                Field::Has
            }
            _ => Field::Is,
        }
    }

    pub fn needs_meta(self) -> bool {
        !matches!(
            self,
            Flag::Pick
                | Flag::Reject
                | Flag::Flagged
                | Flag::Rated
                | Flag::Unrated
                | Flag::Labelled
                | Flag::Folder
                | Flag::Image
                | Flag::Video
                | Flag::Sound
        )
    }
}

/// Colour label words (Lightroom's names; 0 is none).
pub const LABEL_WORDS: [&str; 5] = ["none", "red", "yellow", "green", "blue"];

/// What a field term asks for, interpreted.
#[derive(Clone, Debug, PartialEq)]
pub enum Pred {
    /// Any of these, as a substring (or the whole value when `exact`).
    Text {
        exact: bool,
        any: Vec<String>,
    },
    /// Normalised lower-case extensions.
    Ext(Vec<String>),
    Num(Vec<Range>),
    /// Seconds.
    Date(Vec<Range>),
    Label(Vec<u8>),
    Orient(Vec<Orient>),
    Flags(Vec<Flag>),
    /// `dupes:` / `similar:`: any member, or only the suggested keepers (`Some(true)`) or the
    /// rest (`Some(false)`).
    Group(Option<bool>),
    /// `edited:` and `version:`: one of the field's words ([`EDITED_WORDS`],
    /// [`VERSION_WORDS`]), any of these; likewise `flash:`, `wb:`, `program:`, `metering:`.
    Words(Vec<&'static str>),
    /// `exif:`: any of these.
    Exif(Vec<crate::exif::Cond>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    /// A word or phrase matched against the file name.
    Text(String),
    Field {
        field: Field,
        op: Op,
        /// The values as typed (quotes removed), for rewriting the term.
        items: Vec<String>,
        pred: Pred,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Term {
    pub span: Span,
    pub negated: bool,
    pub kind: Kind,
}

impl Term {
    pub fn field(&self) -> Option<Field> {
        match &self.kind {
            Kind::Field { field, .. } => Some(*field),
            Kind::Text(_) => None,
        }
    }
}

/// What kind of value a field takes (for messages).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Expected {
    Number,
    Date,
    /// One of these words.
    Word(&'static [&'static str]),
    /// An Exif tag's name (`exif:`).
    ExifTag,
    /// One of an Exif tag's words (code, word), or a number.
    TagWord(&'static [(u32, &'static str)]),
}

#[derive(Clone, Debug, PartialEq)]
pub enum ErrorKind {
    UnknownField {
        name: String,
        suggestion: Option<&'static str>,
    },
    MissingValue,
    BadValue {
        value: String,
        expected: Expected,
    },
    /// `<`, `>` on a field that does not compare, or a comparison with a range.
    NoCompare,
    Unterminated,
    /// Understood but needs the Library (or reverse geocoding).
    Unavailable,
    /// Understood but answered from the AI pack's `feature`. The parser does not know
    /// whether the pack is installed: the UI says "needs the AI pack" when it is not.
    NeedsAiPack(AiFeature),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Error {
    pub span: Span,
    /// The field's name as typed (lower case).
    pub field: String,
    pub kind: ErrorKind,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Query {
    pub terms: Vec<Term>,
    pub errors: Vec<Error>,
}

impl Query {
    pub fn is_empty(&self) -> bool {
        self.terms.is_empty()
    }

    /// Whether any term reads image metadata (see [`Field::needs_meta`]).
    pub fn needs_meta(&self) -> bool {
        self.terms.iter().any(|t| match &t.kind {
            Kind::Field {
                pred: Pred::Flags(fl),
                ..
            } => fl.iter().any(|f| f.needs_meta()),
            Kind::Field { field, .. } => field.needs_meta(),
            Kind::Text(_) => false,
        })
    }
}

/// Upper bound on the terms looked at: a pasted novel stays cheap to evaluate per file.
const MAX_TERMS: usize = 64;

/// Parses a query for the gallery ([`Scope::Gallery`]).
pub fn parse(src: &str) -> Query {
    parse_in(src, Scope::Gallery)
}

/// Parses a query for `scope`: fields it cannot answer are reported as
/// [`ErrorKind::Unavailable`] and left out.
pub fn parse_in(src: &str, scope: Scope) -> Query {
    let mut p = Parser {
        s: src,
        i: 0,
        scope,
        q: Query::default(),
    };
    loop {
        p.skip_ws();
        if p.i >= p.s.len() || p.q.terms.len() + p.q.errors.len() >= MAX_TERMS {
            break;
        }
        p.term();
    }
    p.q
}

struct Parser<'a> {
    s: &'a str,
    i: usize,
    scope: Scope,
    q: Query,
}

impl Parser<'_> {
    fn peek(&self) -> Option<char> {
        self.s[self.i..].chars().next()
    }

    fn peek2(&self) -> Option<char> {
        self.s[self.i..].chars().nth(1)
    }

    fn bump(&mut self) {
        if let Some(c) = self.peek() {
            self.i += c.len_utf8();
        }
    }

    fn skip_ws(&mut self) {
        while self.peek().is_some_and(char::is_whitespace) {
            self.bump();
        }
    }

    /// A quoted phrase at `"`: (contents, closed).
    fn quoted(&mut self) -> (String, bool) {
        let (s, end, closed) = quoted_at(self.s, self.i);
        self.i = end;
        (s, closed)
    }

    fn term(&mut self) {
        let start = self.i;
        let mut negated = false;
        if self.peek() == Some('-') {
            self.bump();
            match self.peek() {
                // A lone "-" means nothing.
                None => return,
                Some(c) if c.is_whitespace() => return,
                _ => negated = true,
            }
        }
        if self.peek() == Some('"') {
            let (text, closed) = self.quoted();
            let span = Span::new(start, self.i);
            if !closed {
                self.error(span, "", ErrorKind::Unterminated);
            }
            if !text.trim().is_empty() {
                self.q.terms.push(Term {
                    span,
                    negated,
                    kind: Kind::Text(text),
                });
            }
            return;
        }
        let name_start = self.i;
        while self.peek().is_some_and(|c| {
            c.is_ascii_alphabetic() || c == '_' || (self.i > name_start && c.is_ascii_digit())
        }) {
            self.bump();
        }
        let name_end = self.i;
        let op = if name_end > name_start {
            self.op()
        } else {
            None
        };
        let Some((op, not)) = op else {
            // A plain word: up to the next whitespace.
            self.i = name_start;
            while self.peek().is_some_and(|c| !c.is_whitespace()) {
                self.bump();
            }
            let w = &self.s[name_start..self.i];
            if !w.is_empty() {
                self.q.terms.push(Term {
                    span: Span::new(start, self.i),
                    negated,
                    kind: Kind::Text(w.to_owned()),
                });
            }
            return;
        };
        let name = self.s[name_start..name_end].to_ascii_lowercase();
        let (items, closed) = self.items(name == "exif");
        let span = Span::new(start, self.i);
        if !closed {
            self.error(span, &name, ErrorKind::Unterminated);
            return;
        }
        let Some(field) = Field::from_name(&name) else {
            let ai = AI_FIELDS.iter().find(|(n, _)| *n == name);
            let kind = if let Some(&(_, feature)) = ai {
                if feature == AiFeature::Search && self.scope != Scope::Library {
                    ErrorKind::Unavailable
                } else {
                    ErrorKind::NeedsAiPack(feature)
                }
            } else if UNAVAILABLE.contains(&name.as_str()) {
                ErrorKind::Unavailable
            } else {
                ErrorKind::UnknownField {
                    suggestion: suggest(&name),
                    name: name.clone(),
                }
            };
            self.error(span, &name, kind);
            return;
        };
        if field.library_only() && self.scope != Scope::Library {
            self.error(span, &name, ErrorKind::Unavailable);
            return;
        }
        let items: Vec<(String, Span)> = items
            .into_iter()
            .filter(|(s, _)| !s.trim().is_empty())
            .collect();
        // `similar:` on its own (what the review panel's filter link types) means any member.
        if items.is_empty() && matches!(field, Field::Dupes | Field::Similar) && op == Op::Has {
            self.q.terms.push(Term {
                span,
                negated: negated != not,
                kind: Kind::Field {
                    field,
                    op,
                    items: Vec::new(),
                    pred: Pred::Group(None),
                },
            });
            return;
        }
        if items.is_empty() {
            self.error(span, &name, ErrorKind::MissingValue);
            return;
        }
        match interpret(field, op, &items) {
            Ok(pred) => self.q.terms.push(Term {
                span,
                negated: negated != not,
                kind: Kind::Field {
                    field,
                    op,
                    items: items.into_iter().map(|(s, _)| s).collect(),
                    pred,
                },
            }),
            Err((at, kind)) => {
                let at = at.map_or(span, |k| items[k].1);
                self.error(at, &name, kind);
            }
        }
    }

    /// The operator after a field name: (op, negates). None: not a field term.
    fn op(&mut self) -> Option<(Op, bool)> {
        let rest = &self.s[self.i..];
        let colon = rest.starts_with(':');
        let after = if colon { &rest[1..] } else { rest };
        let ops: [(&str, Op, bool); 6] = [
            (">=", Op::Ge, false),
            ("<=", Op::Le, false),
            ("!=", Op::Eq, true),
            (">", Op::Gt, false),
            ("<", Op::Lt, false),
            ("=", Op::Eq, false),
        ];
        let found = ops.iter().find(|(t, ..)| after.starts_with(t));
        match (colon, found) {
            (true, None) => {
                self.i += 1;
                Some((Op::Has, false))
            }
            (_, Some(&(t, op, not))) => {
                self.i += usize::from(colon) + t.len();
                Some((op, not))
            }
            (false, None) => None,
        }
    }

    /// Comma-separated values: (items with spans, every quote closed). With `glue`, a quoted
    /// value right after an operator is part of the item (`exif:Software="Photo Lab"`).
    fn items(&mut self, glue: bool) -> (Vec<(String, Span)>, bool) {
        let mut out = Vec::new();
        loop {
            let start = self.i;
            if self.peek() == Some('"') {
                let (s, closed) = self.quoted();
                out.push((s, Span::new(start, self.i)));
                if !closed {
                    return (out, false);
                }
            } else {
                while self
                    .peek()
                    .is_some_and(|c| !c.is_whitespace() && c != ',' && c != '"')
                {
                    self.bump();
                }
                let mut item = self.s[start..self.i].to_owned();
                if glue && self.peek() == Some('"') && item.ends_with(['=', '<', '>', ':']) {
                    let (q, closed) = self.quoted();
                    item.push_str(&q);
                    if !closed {
                        out.push((item, Span::new(start, self.i)));
                        return (out, false);
                    }
                }
                out.push((item, Span::new(start, self.i)));
            }
            if self.peek() == Some(',') && self.peek2().is_some_and(|c| !c.is_whitespace()) {
                self.bump();
            } else {
                if self.peek() == Some(',') {
                    self.bump();
                }
                return (out, true);
            }
        }
    }

    fn error(&mut self, span: Span, field: &str, kind: ErrorKind) {
        self.q.errors.push(Error {
            span,
            field: field.to_owned(),
            kind,
        });
    }
}

/// The nearest field name within two edits.
fn suggest(name: &str) -> Option<&'static str> {
    FIELDS
        .iter()
        .map(|(n, _)| (*n, edit_distance(name, n)))
        .filter(|&(n, d)| d <= 2 && d < n.len())
        .min_by_key(|&(_, d)| d)
        .map(|(n, _)| n)
}

fn edit_distance(a: &str, b: &str) -> usize {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().take(64).collect(), b.chars().collect());
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut cur = vec![i + 1; b.len() + 1];
        for (j, cb) in b.iter().enumerate() {
            cur[j + 1] = (prev[j] + usize::from(ca != cb))
                .min(prev[j + 1] + 1)
                .min(cur[j] + 1);
        }
        prev = cur;
    }
    prev[b.len()]
}

type Interpreted = Result<Pred, (Option<usize>, ErrorKind)>;

fn interpret(field: Field, op: Op, items: &[(String, Span)]) -> Interpreted {
    let plain = matches!(op, Op::Has | Op::Eq);
    let words = |list: &'static [&'static str]| -> Result<Vec<&'static str>, _> {
        items
            .iter()
            .enumerate()
            .map(|(k, (s, _))| {
                let l = s.to_ascii_lowercase();
                list.iter().copied().find(|w| *w == l).ok_or((
                    Some(k),
                    ErrorKind::BadValue {
                        value: s.clone(),
                        expected: Expected::Word(list),
                    },
                ))
            })
            .collect()
    };
    match field {
        Field::Name
        | Field::Camera
        | Field::Lens
        | Field::Tag
        | Field::Folder
        | Field::Place
        | Field::Person
        | Field::Object
        | Field::Filter
        | Field::FrameType
        | Field::Module
        | Field::Style
        | Field::Album
        | Field::Make
        | Field::Serial
        | Field::LensSerial
        | Field::Software
        | Field::Artist
        | Field::Copyright
        | Field::Description
        | Field::Title
        | Field::Headline
        | Field::Credit
        | Field::Source
        | Field::UsageTerms
        | Field::Instructions
        | Field::JobId
        | Field::Event
        | Field::Contact => {
            if !plain {
                return Err((None, ErrorKind::NoCompare));
            }
            Ok(Pred::Text {
                exact: op == Op::Eq,
                any: items.iter().map(|(s, _)| s.clone()).collect(),
            })
        }
        // A named space by its word (matched whole), else text of a description.
        Field::Space => {
            if !plain {
                return Err((None, ErrorKind::NoCompare));
            }
            Ok(Pred::Text {
                exact: op == Op::Eq,
                any: items
                    .iter()
                    .map(|(s, _)| space_word(s).map_or_else(|| s.clone(), str::to_owned))
                    .collect(),
            })
        }
        Field::Ext => {
            if !plain {
                return Err((None, ErrorKind::NoCompare));
            }
            Ok(Pred::Ext(items.iter().map(|(s, _)| norm_ext(s)).collect()))
        }
        Field::Date | Field::Modified => {
            let mut out = Vec::new();
            for (k, (s, _)) in items.iter().enumerate() {
                out.push(date_range(s, op).map_err(|e| (Some(k), e))?);
            }
            Ok(Pred::Date(out))
        }
        Field::Label => {
            if !plain {
                return Err((None, ErrorKind::NoCompare));
            }
            let mut out = Vec::new();
            for (k, (s, _)) in items.iter().enumerate() {
                let l = s.to_ascii_lowercase();
                let n = LABEL_WORDS
                    .iter()
                    .position(|w| *w == l)
                    .or_else(|| l.parse::<usize>().ok().filter(|n| *n < LABEL_WORDS.len()));
                match n {
                    Some(n) => out.push(n as u8),
                    None => {
                        return Err((
                            Some(k),
                            ErrorKind::BadValue {
                                value: s.clone(),
                                expected: Expected::Word(&LABEL_WORDS),
                            },
                        ));
                    }
                }
            }
            Ok(Pred::Label(out))
        }
        Field::Orientation => {
            if !plain {
                return Err((None, ErrorKind::NoCompare));
            }
            const W: &[&str] = &["landscape", "portrait", "square"];
            let w = words(W)?;
            Ok(Pred::Orient(
                w.into_iter()
                    .filter_map(|w| Orient::ALL.into_iter().find(|o| o.word() == w))
                    .collect(),
            ))
        }
        Field::Is | Field::Has => {
            if !plain {
                return Err((None, ErrorKind::NoCompare));
            }
            const W: &[&str] = &[
                "pick",
                "reject",
                "flagged",
                "rated",
                "unrated",
                "labelled",
                "raw",
                "hdr",
                "animated",
                "multipage",
                "landscape",
                "portrait",
                "square",
                "folder",
                "image",
                "gps",
                "alpha",
                "profile",
                "camera",
                "lens",
                "date",
                "video",
                "sound",
            ];
            let mut out = Vec::new();
            for (k, (s, _)) in items.iter().enumerate() {
                let l = s.to_ascii_lowercase();
                match FLAG_WORDS.iter().find(|(w, _)| *w == l) {
                    Some((_, f)) => out.push(*f),
                    None => {
                        return Err((
                            Some(k),
                            ErrorKind::BadValue {
                                value: s.clone(),
                                expected: Expected::Word(W),
                            },
                        ));
                    }
                }
            }
            Ok(Pred::Flags(out))
        }
        Field::Edited | Field::Version => {
            if !plain {
                return Err((None, ErrorKind::NoCompare));
            }
            let list: &'static [&'static str] = if field == Field::Edited {
                &EDITED_WORDS
            } else {
                &VERSION_WORDS
            };
            let mut out = Vec::new();
            for (k, (s, _)) in items.iter().enumerate() {
                let l = s.to_ascii_lowercase();
                // Aliases: `true`/`false`, `has`, `virtual`.
                let l = match l.as_str() {
                    "true" | "has" | "any" => "yes",
                    "false" | "none" => "no",
                    "is" | "virtual" | "copies" => "copy",
                    w => w,
                };
                match list.iter().find(|w| **w == l) {
                    Some(w) => out.push(*w),
                    None => {
                        return Err((
                            Some(k),
                            ErrorKind::BadValue {
                                value: s.clone(),
                                expected: Expected::Word(list),
                            },
                        ));
                    }
                }
            }
            Ok(Pred::Words(out))
        }
        Field::Flash | Field::WhiteBalance | Field::Program | Field::Metering => {
            if !plain {
                return Err((None, ErrorKind::NoCompare));
            }
            let list = exif_words(field);
            let mut out = Vec::new();
            for (k, (s, _)) in items.iter().enumerate() {
                match crate::exif::field_word(list, s) {
                    Some(w) => out.push(w),
                    None => {
                        return Err((
                            Some(k),
                            ErrorKind::BadValue {
                                value: s.clone(),
                                expected: Expected::Word(list),
                            },
                        ));
                    }
                }
            }
            Ok(Pred::Words(out))
        }
        Field::Exif => {
            if op != Op::Has {
                return Err((None, ErrorKind::NoCompare));
            }
            exif_conds(items)
        }
        Field::Dupes | Field::Similar => {
            if !plain || items.len() > 1 {
                return Err((None, ErrorKind::NoCompare));
            }
            let l = items[0].0.to_ascii_lowercase();
            match l.as_str() {
                "any" | "all" | "yes" => Ok(Pred::Group(None)),
                "keep" | "best" => Ok(Pred::Group(Some(true))),
                "discard" | "rest" => Ok(Pred::Group(Some(false))),
                "this" => Err((Some(0), ErrorKind::Unavailable)),
                _ => Err((
                    Some(0),
                    ErrorKind::BadValue {
                        value: items[0].0.clone(),
                        expected: Expected::Word(&["any", "keep", "discard"]),
                    },
                )),
            }
        }
        Field::Iso
        | Field::Aperture
        | Field::Focal
        | Field::Exposure
        | Field::Width
        | Field::Height
        | Field::Megapixels
        | Field::Size
        | Field::Rating
        | Field::Bits
        | Field::Pages
        | Field::Gain
        | Field::Temp
        | Field::Hfd
        | Field::Fwhm
        | Field::Seeing
        | Field::Ecc
        | Field::StarCount
        | Field::Background
        | Field::Snr
        | Field::ExpComp
        | Field::Focal35
        | Field::Altitude => {
            let mut out = Vec::new();
            for (k, (s, _)) in items.iter().enumerate() {
                out.push(num_range(field, s, op).map_err(|e| (Some(k), e))?);
            }
            Ok(Pred::Num(out))
        }
    }
}

/// Lower case, no leading dot, one spelling per format ("jpeg" → "jpg").
pub fn norm_ext(s: &str) -> String {
    let l = s.trim_start_matches('.').to_ascii_lowercase();
    match l.as_str() {
        "jpeg" | "jpe" | "jfif" => "jpg".into(),
        "tiff" => "tif".into(),
        "heif" => "heic".into(),
        _ => l,
    }
}

/// A number as typed for `field`, with half a unit of its last digit (what `:` / `=`
/// tolerates: `f:2.8` matches the 2.799999952 a file stores).
pub(crate) fn number(field: Field, s: &str) -> Option<(f64, f64)> {
    let l = s.trim().to_ascii_lowercase();
    let mut t = l.as_str();
    let mut scale = 1.0;
    match field {
        Field::Aperture => t = t.strip_prefix("f/").or(t.strip_prefix('f')).unwrap_or(t),
        Field::Focal | Field::Focal35 => t = t.strip_suffix("mm").unwrap_or(t),
        Field::ExpComp => t = t.strip_suffix("ev").unwrap_or(t).trim_end(),
        Field::Altitude => t = t.strip_suffix('m').unwrap_or(t),
        Field::Exposure => t = t.strip_suffix('s').unwrap_or(t),
        Field::Megapixels => t = t.strip_suffix("mp").unwrap_or(t),
        Field::Hfd | Field::Fwhm => t = t.strip_suffix("px").unwrap_or(t),
        Field::Seeing => {
            t = t
                .strip_suffix("arcsec")
                .or(t.strip_suffix("as"))
                .unwrap_or(t)
        }
        Field::Temp => t = t.strip_suffix("°c").or(t.strip_suffix('c')).unwrap_or(t),
        Field::Size => {
            const UNITS: [(&str, f64); 10] = [
                ("kib", 1024.0),
                ("mib", 1048576.0),
                ("gib", 1073741824.0),
                ("kb", 1e3),
                ("mb", 1e6),
                ("gb", 1e9),
                ("k", 1e3),
                ("m", 1e6),
                ("g", 1e9),
                ("b", 1.0),
            ];
            if let Some((u, f)) = UNITS.iter().find(|(u, _)| t.ends_with(u)) {
                t = &t[..t.len() - u.len()];
                scale = *f;
            }
        }
        _ => {}
    }
    // Exposure compensation: signed, in fractions too (`-1/3`), and within a tenth of a stop
    // (cameras write a third as -0.3 or -0.33).
    if field == Field::ExpComp {
        return Some((signed_number(t)?, 0.1));
    }
    // Shutter speeds as fractions: "1/250".
    if field == Field::Exposure
        && let Some((a, b)) = t.split_once('/')
    {
        let (a, b) = (plain_number(a)?.0, plain_number(b)?.0);
        if b <= 0.0 {
            return None;
        }
        let v = a / b;
        return Some((v, v * 0.01));
    }
    // Temperatures and altitudes go below zero.
    if matches!(field, Field::Temp | Field::Altitude)
        && let Some(n) = t.strip_prefix('-')
    {
        let (v, tol) = plain_number(n)?;
        return Some((-v, tol));
    }
    let (v, tol) = plain_number(t)?;
    Some((v * scale, tol * scale))
}

/// A number with an optional sign, or a fraction (`-1/3`).
pub(crate) fn signed_number(t: &str) -> Option<f64> {
    let t = t.trim();
    let (neg, t) = match t.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, t.strip_prefix('+').unwrap_or(t)),
    };
    let v = match t.split_once('/') {
        Some((a, b)) => {
            let (a, b) = (plain_number(a)?.0, plain_number(b)?.0);
            if b <= 0.0 {
                return None;
            }
            a / b
        }
        None => plain_number(t)?.0,
    };
    Some(if neg { -v } else { v })
}

/// A typed number for any Exif tag's value: signed or a fraction, with half a unit of its
/// last digit (a hundredth of a fraction).
fn exif_number(t: &str) -> Option<(f64, f64)> {
    let t = t.trim();
    let v = signed_number(t)?;
    if t.contains('/') {
        return Some((v, (v * 0.01).abs()));
    }
    let digits = t.trim_start_matches(['-', '+']);
    let decimals = digits.split_once('.').map_or(0, |(_, d)| d.len()).min(12);
    Some((v, 0.5 * 10f64.powi(-(decimals as i32))))
}

/// The words of a named Exif field.
pub(crate) fn exif_words(field: Field) -> &'static [&'static str] {
    use crate::exif::{FLASH_WORDS, METERING_WORDS, PROGRAM_WORDS, WB_WORDS};
    match field {
        Field::Flash => &FLASH_WORDS,
        Field::WhiteBalance => &WB_WORDS,
        Field::Program => &PROGRAM_WORDS,
        _ => &METERING_WORDS,
    }
}

/// `exif:` values: `Name`, `Name=value`, `Name<n`… A value without a name of its own is
/// another value of the tag before (`exif:Contrast=soft,hard`).
fn exif_conds(items: &[(String, Span)]) -> Interpreted {
    use crate::exif::{Cond, Tag, Want, tag_named};
    let mut out: Vec<Cond> = Vec::new();
    let mut last: Option<(&'static Tag, &'static str)> = None;
    for (k, (s, _)) in items.iter().enumerate() {
        let at = s.find(['<', '>', '=', '!', ':']);
        let (name, op, value) = match at {
            Some(i) => {
                let rest = &s[i..];
                let op = ["<=", ">=", "!=", "<", ">", "=", ":"]
                    .into_iter()
                    .find(|o| rest.starts_with(o))
                    .unwrap_or(":");
                (&s[..i], op, &rest[op.len().min(rest.len())..])
            }
            None => (s.as_str(), "", ""),
        };
        let tag = match (tag_named(name), at, last) {
            (Some(t), ..) => t,
            // Another value of the tag before.
            (None, None, Some((t, op))) => {
                out.push(exif_cond(t, op, name).map_err(|e| (Some(k), e))?);
                continue;
            }
            _ => {
                return Err((
                    Some(k),
                    ErrorKind::BadValue {
                        value: name.to_owned(),
                        expected: Expected::ExifTag,
                    },
                ));
            }
        };
        if op.is_empty() {
            out.push(Cond {
                tag,
                not: false,
                want: Want::Has,
            });
            last = None;
            continue;
        }
        if value.trim().is_empty() {
            return Err((Some(k), ErrorKind::MissingValue));
        }
        out.push(exif_cond(tag, op, value).map_err(|e| (Some(k), e))?);
        last = Some((tag, op));
    }
    Ok(Pred::Exif(out))
}

/// One `exif:` value of `tag` with the operator `op` (as typed).
fn exif_cond(
    tag: &'static crate::exif::Tag,
    op: &str,
    value: &str,
) -> Result<crate::exif::Cond, ErrorKind> {
    use crate::exif::{Cond, FLASH_WORDS, TagKind, Want, codes, field_word};
    let value = value.trim();
    let want = if matches!(op, "<" | "<=" | ">" | ">=") {
        let (v, _) = exif_number(value).ok_or_else(|| ErrorKind::BadValue {
            value: value.to_owned(),
            expected: Expected::Number,
        })?;
        let tol = v.abs() * 1e-6;
        let mut r = Range::ALL;
        match op {
            "<" => {
                r.hi = v - tol;
                r.hi_open = true;
            }
            "<=" => r.hi = v + tol,
            ">" => {
                r.lo = v + tol;
                r.lo_open = true;
            }
            _ => r.lo = v - tol,
        }
        Want::Cmp(r)
    } else {
        let num = exif_number(value).map(|(v, tol)| Range {
            lo: v - tol,
            lo_open: false,
            hi: v + tol,
            hi_open: false,
        });
        let (codes, flash) = match tag.kind {
            TagKind::Enum(w) => {
                let c = codes(w, value);
                if c.is_empty() && num.is_none() {
                    return Err(ErrorKind::BadValue {
                        value: value.to_owned(),
                        expected: Expected::TagWord(w),
                    });
                }
                (c, None)
            }
            TagKind::Flash => {
                let w = field_word(&FLASH_WORDS, value);
                if w.is_none() && num.is_none() {
                    return Err(ErrorKind::BadValue {
                        value: value.to_owned(),
                        expected: Expected::Word(&FLASH_WORDS),
                    });
                }
                (Vec::new(), w)
            }
            _ => (Vec::new(), None),
        };
        Want::Is {
            num,
            codes,
            flash,
            text: value.to_owned(),
        }
    };
    Ok(Cond {
        tag,
        not: op == "!=",
        want,
    })
}

fn plain_number(t: &str) -> Option<(f64, f64)> {
    if t.is_empty()
        || !t.bytes().all(|b| b.is_ascii_digit() || b == b'.')
        || t.bytes().filter(|&b| b == b'.').count() > 1
        || t == "."
    {
        return None;
    }
    let v: f64 = t.parse().ok()?;
    let decimals = t.split_once('.').map_or(0, |(_, d)| d.len()).min(12);
    Some((v, 0.5 * 10f64.powi(-(decimals as i32))))
}

fn num_range(field: Field, s: &str, op: Op) -> Result<Range, ErrorKind> {
    let bad = || ErrorKind::BadValue {
        value: s.to_owned(),
        expected: Expected::Number,
    };
    if let Some((a, b)) = s.split_once("..") {
        if !matches!(op, Op::Has | Op::Eq) {
            return Err(ErrorKind::NoCompare);
        }
        let mut r = Range::ALL;
        if !a.is_empty() {
            let (v, tol) = number(field, a).ok_or_else(bad)?;
            r.lo = v - tol;
        }
        if !b.is_empty() {
            let (v, tol) = number(field, b).ok_or_else(bad)?;
            r.hi = v + tol;
        }
        if a.is_empty() && b.is_empty() {
            return Err(bad());
        }
        return Ok(r);
    }
    let (v, tol) = number(field, s).ok_or_else(bad)?;
    // Comparisons are exact but for float noise (`f>=2.8` keeps the 2.7999999 a file
    // stores); `:` and `=` tolerate half a unit of the last digit typed.
    let tol = if matches!(op, Op::Has | Op::Eq) {
        tol
    } else {
        v.abs() * 1e-6
    };
    let mut r = Range::ALL;
    match op {
        Op::Has | Op::Eq => {
            r.lo = v - tol;
            r.hi = v + tol;
        }
        Op::Gt => {
            r.lo = v + tol;
            r.lo_open = true;
        }
        Op::Ge => r.lo = v - tol,
        Op::Lt => {
            r.hi = v - tol;
            r.hi_open = true;
        }
        Op::Le => r.hi = v + tol,
    }
    Ok(r)
}

fn date_range(s: &str, op: Op) -> Result<Range, ErrorKind> {
    let bad = |v: &str| ErrorKind::BadValue {
        value: v.to_owned(),
        expected: Expected::Date,
    };
    let half_open = |lo: f64, hi: f64| Range {
        lo,
        lo_open: false,
        hi,
        hi_open: true,
    };
    if let Some((a, b)) = s.split_once("..") {
        if !matches!(op, Op::Has | Op::Eq) {
            return Err(ErrorKind::NoCompare);
        }
        if a.is_empty() && b.is_empty() {
            return Err(bad(s));
        }
        let lo = if a.is_empty() {
            f64::NEG_INFINITY
        } else {
            date::period(a).ok_or_else(|| bad(a))?.0 as f64
        };
        let hi = if b.is_empty() {
            f64::INFINITY
        } else {
            date::period(b).ok_or_else(|| bad(b))?.1 as f64
        };
        return Ok(half_open(lo, hi));
    }
    let (start, end) = date::period(s).ok_or_else(|| bad(s))?;
    let (start, end) = (start as f64, end as f64);
    Ok(match op {
        Op::Has | Op::Eq => half_open(start, end),
        Op::Gt | Op::Ge => half_open(if op == Op::Gt { end } else { start }, f64::INFINITY),
        Op::Lt | Op::Le => half_open(f64::NEG_INFINITY, if op == Op::Lt { start } else { end }),
    })
}

/// The quoted string starting at the `"` at byte `at` of `s`: (contents unescaped, the byte
/// after the closing quote or the end of `s`, closed). Inside quotes `\"` is a quote and `\\`
/// a backslash; any other backslash is itself (`"C:\photos"` reads as typed).
pub fn quoted_at(s: &str, at: usize) -> (String, usize, bool) {
    let mut out = String::new();
    let mut it = s[at + 1..].char_indices();
    while let Some((k, c)) = it.next() {
        match c {
            '"' => return (out, at + 1 + k + 1, true),
            '\\' => match s[at + 1 + k + 1..].chars().next() {
                Some(e @ ('"' | '\\')) => {
                    out.push(e);
                    it.next();
                }
                _ => out.push('\\'),
            },
            c => out.push(c),
        }
    }
    (out, s.len(), false)
}

/// `s` as a query value: bare when it can be (a bare value reads backslashes as typed),
/// quoted otherwise, with `"` and `\` escaped inside the quotes so that it reads back as `s`.
pub fn quote(s: &str) -> String {
    // A leading operator character would be read as the operator.
    let bare = !s.is_empty()
        && !s.starts_with(['<', '>', '=', '!'])
        && !s
            .chars()
            .any(|c| c.is_whitespace() || matches!(c, ',' | '"'));
    if bare {
        return s.to_owned();
    }
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        if matches!(c, '"' | '\\') {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('"');
    out
}
