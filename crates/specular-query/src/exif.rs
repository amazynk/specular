//! Exif fields: the camera settings' words (`flash:`, `wb:`, `program:`, `metering:`) and the
//! tags `exif:Name=value` names.
//!
//! The Library keeps a file's tags as text, one `key=value` per line ([`push_tag`]): the key
//! is the tag number in hex (`a406`), with a `g` before it for the GPS IFD (`g0006`), whose
//! numbers are a space of their own; IFD0 and the Exif IFD share one. A value is the text as
//! stored, a number in decimal (several separated by spaces), or the code of an enumerated
//! tag, which `exif:` also finds by its standard word ([`Tag::words`]).

use crate::parse::Range;

/// How a tag's value reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TagKind {
    Text,
    Num,
    /// A code with standard words: (code, word), the first word of a code the one it is
    /// shown by.
    Enum(&'static [(u32, &'static str)]),
    /// The Flash bit field: [`FLASH_WORDS`].
    Flash,
}

/// A tag `exif:` can name.
#[derive(Debug, PartialEq, Eq)]
pub struct Tag {
    /// Its standard name (CIPA DC-008, TIFF 6.0).
    pub name: &'static str,
    pub gps: bool,
    pub id: u16,
    pub kind: TagKind,
}

impl Tag {
    /// The words of an enumerated tag (empty for the rest).
    pub fn words(&self) -> &'static [(u32, &'static str)] {
        match self.kind {
            TagKind::Enum(w) => w,
            _ => &[],
        }
    }

    /// The key it is stored under ([`push_tag`]).
    pub fn key(&self) -> String {
        key(self.gps, self.id)
    }
}

/// The stored key of a tag.
pub fn key(gps: bool, id: u16) -> String {
    if gps {
        format!("g{id:04x}")
    } else {
        format!("{id:04x}")
    }
}

/// Adds one tag to `out` (the Library's text of a file's tags). Line breaks in the value
/// become spaces.
pub fn push_tag(out: &mut String, gps: bool, id: u16, value: &str) {
    if !out.is_empty() {
        out.push('\n');
    }
    out.push_str(&key(gps, id));
    out.push('=');
    out.extend(
        value
            .chars()
            .map(|c| if c == '\n' || c == '\r' { ' ' } else { c }),
    );
}

/// The value of the tag stored under `key` in `tags`, if it has it.
pub fn value_of<'a>(tags: &'a str, key: &str) -> Option<&'a str> {
    let (b, k) = (tags.as_bytes(), key.as_bytes());
    let mut at = 0;
    loop {
        // `at` starts a line: its key is compared in place, else the next line is found.
        let line = &b[at..];
        if line.len() > k.len() && line.starts_with(k) && line[k.len()] == b'=' {
            let v = &tags[at + k.len() + 1..];
            return Some(v.find('\n').map_or(v, |e| &v[..e]));
        }
        at += tags[at..].find('\n')? + 1;
    }
}

use TagKind::{Enum, Flash, Num, Text};

const RESOLUTION_UNIT: &[(u32, &str)] = &[(1, "none"), (2, "inch"), (2, "inches"), (3, "cm")];
const YCBCR_POSITIONING: &[(u32, &str)] = &[(1, "centered"), (2, "cosited")];
const EXPOSURE_PROGRAM: &[(u32, &str)] = &[
    (0, "notdefined"),
    (1, "manual"),
    (2, "program"),
    (2, "normal"),
    (3, "aperture"),
    (3, "av"),
    (4, "shutter"),
    (4, "tv"),
    (5, "creative"),
    (6, "action"),
    (7, "portrait"),
    (8, "landscape"),
];
const METERING_MODE: &[(u32, &str)] = &[
    (0, "unknown"),
    (1, "average"),
    (2, "centre"),
    (2, "center"),
    (2, "centreweighted"),
    (2, "centerweighted"),
    (3, "spot"),
    (4, "multispot"),
    (5, "matrix"),
    (5, "evaluative"),
    (5, "pattern"),
    (5, "multisegment"),
    (6, "partial"),
    (255, "other"),
];
const LIGHT_SOURCE: &[(u32, &str)] = &[
    (0, "unknown"),
    (1, "daylight"),
    (2, "fluorescent"),
    (3, "tungsten"),
    (3, "incandescent"),
    (4, "flash"),
    (9, "fineweather"),
    (10, "cloudy"),
    (11, "shade"),
    (12, "daylightfluorescent"),
    (13, "daywhitefluorescent"),
    (14, "coolwhitefluorescent"),
    (15, "whitefluorescent"),
    (16, "warmwhitefluorescent"),
    (17, "standarda"),
    (18, "standardb"),
    (19, "standardc"),
    (20, "d55"),
    (21, "d65"),
    (22, "d75"),
    (23, "d50"),
    (24, "isotungsten"),
    (255, "other"),
];
const COLOR_SPACE: &[(u32, &str)] = &[(1, "srgb"), (2, "adobergb"), (65535, "uncalibrated")];
const SENSING_METHOD: &[(u32, &str)] = &[
    (1, "notdefined"),
    (2, "onechip"),
    (3, "twochip"),
    (4, "threechip"),
    (5, "colorsequential"),
    (7, "trilinear"),
    (8, "colorsequentiallinear"),
];
const FILE_SOURCE: &[(u32, &str)] = &[
    (0, "others"),
    (1, "transparentscanner"),
    (2, "reflectivescanner"),
    (3, "dsc"),
    (3, "camera"),
];
const SCENE_TYPE: &[(u32, &str)] = &[(1, "directlyphotographed")];
const CUSTOM_RENDERED: &[(u32, &str)] = &[(0, "normal"), (1, "custom")];
const EXPOSURE_MODE: &[(u32, &str)] = &[
    (0, "auto"),
    (1, "manual"),
    (2, "bracket"),
    (2, "autobracket"),
];
const WHITE_BALANCE: &[(u32, &str)] = &[(0, "auto"), (1, "manual")];
const SCENE_CAPTURE_TYPE: &[(u32, &str)] = &[
    (0, "standard"),
    (1, "landscape"),
    (2, "portrait"),
    (3, "night"),
    (3, "nightscene"),
];
const GAIN_CONTROL: &[(u32, &str)] = &[
    (0, "none"),
    (1, "lowgainup"),
    (1, "lowup"),
    (2, "highgainup"),
    (2, "highup"),
    (3, "lowgaindown"),
    (3, "lowdown"),
    (4, "highgaindown"),
    (4, "highdown"),
];
const CONTRAST: &[(u32, &str)] = &[
    (0, "normal"),
    (1, "soft"),
    (1, "low"),
    (2, "hard"),
    (2, "high"),
];
const SATURATION: &[(u32, &str)] = &[(0, "normal"), (1, "low"), (2, "high")];
const SHARPNESS: &[(u32, &str)] = &[(0, "normal"), (1, "soft"), (2, "hard")];
const SUBJECT_DISTANCE_RANGE: &[(u32, &str)] =
    &[(0, "unknown"), (1, "macro"), (2, "close"), (3, "distant")];
const COMPOSITE_IMAGE: &[(u32, &str)] = &[
    (0, "unknown"),
    (1, "noncomposite"),
    (2, "composite"),
    (2, "general"),
    (3, "compositecaptured"),
];
const SENSITIVITY_TYPE: &[(u32, &str)] = &[
    (0, "unknown"),
    (1, "sos"),
    (2, "rei"),
    (3, "iso"),
    (4, "sosrei"),
    (5, "sosiso"),
    (6, "reiiso"),
    (7, "sosreiiso"),
];
const ALTITUDE_REF: &[(u32, &str)] = &[
    (0, "above"),
    (0, "abovesealevel"),
    (1, "below"),
    (1, "belowsealevel"),
];
const DIFFERENTIAL: &[(u32, &str)] =
    &[(0, "no"), (0, "nocorrection"), (1, "yes"), (1, "corrected")];

const fn t(name: &'static str, id: u16, kind: TagKind) -> Tag {
    Tag {
        name,
        gps: false,
        id,
        kind,
    }
}

const fn g(name: &'static str, id: u16, kind: TagKind) -> Tag {
    Tag {
        name,
        gps: true,
        id,
        kind,
    }
}

/// The tags kept and named, IFD0 and Exif IFD first (by number), then GPS. Structure tags
/// (offsets, byte counts), blobs and the MakerNote are not kept.
pub static TAGS: &[Tag] = &[
    t("ImageWidth", 0x0100, Num),
    t("ImageLength", 0x0101, Num),
    t("BitsPerSample", 0x0102, Num),
    t("Compression", 0x0103, Num),
    t("PhotometricInterpretation", 0x0106, Num),
    t("ImageDescription", 0x010E, Text),
    t("Make", 0x010F, Text),
    t("Model", 0x0110, Text),
    t("Orientation", 0x0112, Num),
    t("SamplesPerPixel", 0x0115, Num),
    t("XResolution", 0x011A, Num),
    t("YResolution", 0x011B, Num),
    t("PlanarConfiguration", 0x011C, Num),
    t("ResolutionUnit", 0x0128, Enum(RESOLUTION_UNIT)),
    t("Software", 0x0131, Text),
    t("DateTime", 0x0132, Text),
    t("Artist", 0x013B, Text),
    t("HostComputer", 0x013C, Text),
    t("YCbCrPositioning", 0x0213, Enum(YCBCR_POSITIONING)),
    t("Rating", 0x4746, Num),
    t("RatingPercent", 0x4749, Num),
    t("Copyright", 0x8298, Text),
    t("ExposureTime", 0x829A, Num),
    t("FNumber", 0x829D, Num),
    t("ExposureProgram", 0x8822, Enum(EXPOSURE_PROGRAM)),
    t("SpectralSensitivity", 0x8824, Text),
    t("PhotographicSensitivity", 0x8827, Num),
    t("SensitivityType", 0x8830, Enum(SENSITIVITY_TYPE)),
    t("StandardOutputSensitivity", 0x8831, Num),
    t("RecommendedExposureIndex", 0x8832, Num),
    t("ISOSpeed", 0x8833, Num),
    t("ExifVersion", 0x9000, Text),
    t("DateTimeOriginal", 0x9003, Text),
    t("DateTimeDigitized", 0x9004, Text),
    t("OffsetTime", 0x9010, Text),
    t("OffsetTimeOriginal", 0x9011, Text),
    t("OffsetTimeDigitized", 0x9012, Text),
    t("CompressedBitsPerPixel", 0x9102, Num),
    t("ShutterSpeedValue", 0x9201, Num),
    t("ApertureValue", 0x9202, Num),
    t("BrightnessValue", 0x9203, Num),
    t("ExposureBiasValue", 0x9204, Num),
    t("MaxApertureValue", 0x9205, Num),
    t("SubjectDistance", 0x9206, Num),
    t("MeteringMode", 0x9207, Enum(METERING_MODE)),
    t("LightSource", 0x9208, Enum(LIGHT_SOURCE)),
    t("Flash", 0x9209, Flash),
    t("FocalLength", 0x920A, Num),
    t("SubjectArea", 0x9214, Num),
    t("UserComment", 0x9286, Text),
    t("SubSecTime", 0x9290, Text),
    t("SubSecTimeOriginal", 0x9291, Text),
    t("SubSecTimeDigitized", 0x9292, Text),
    t("Temperature", 0x9400, Num),
    t("Humidity", 0x9401, Num),
    t("Pressure", 0x9402, Num),
    t("WaterDepth", 0x9403, Num),
    t("Acceleration", 0x9404, Num),
    t("CameraElevationAngle", 0x9405, Num),
    t("FlashpixVersion", 0xA000, Text),
    t("ColorSpace", 0xA001, Enum(COLOR_SPACE)),
    t("PixelXDimension", 0xA002, Num),
    t("PixelYDimension", 0xA003, Num),
    t("RelatedSoundFile", 0xA004, Text),
    t("FlashEnergy", 0xA20B, Num),
    t("FocalPlaneXResolution", 0xA20E, Num),
    t("FocalPlaneYResolution", 0xA20F, Num),
    t("FocalPlaneResolutionUnit", 0xA210, Enum(RESOLUTION_UNIT)),
    t("SubjectLocation", 0xA214, Num),
    t("ExposureIndex", 0xA215, Num),
    t("SensingMethod", 0xA217, Enum(SENSING_METHOD)),
    t("FileSource", 0xA300, Enum(FILE_SOURCE)),
    t("SceneType", 0xA301, Enum(SCENE_TYPE)),
    t("CustomRendered", 0xA401, Enum(CUSTOM_RENDERED)),
    t("ExposureMode", 0xA402, Enum(EXPOSURE_MODE)),
    t("WhiteBalance", 0xA403, Enum(WHITE_BALANCE)),
    t("DigitalZoomRatio", 0xA404, Num),
    t("FocalLengthIn35mmFilm", 0xA405, Num),
    t("SceneCaptureType", 0xA406, Enum(SCENE_CAPTURE_TYPE)),
    t("GainControl", 0xA407, Enum(GAIN_CONTROL)),
    t("Contrast", 0xA408, Enum(CONTRAST)),
    t("Saturation", 0xA409, Enum(SATURATION)),
    t("Sharpness", 0xA40A, Enum(SHARPNESS)),
    t("SubjectDistanceRange", 0xA40C, Enum(SUBJECT_DISTANCE_RANGE)),
    t("ImageUniqueID", 0xA420, Text),
    t("CameraOwnerName", 0xA430, Text),
    t("BodySerialNumber", 0xA431, Text),
    t("LensSpecification", 0xA432, Num),
    t("LensMake", 0xA433, Text),
    t("LensModel", 0xA434, Text),
    t("LensSerialNumber", 0xA435, Text),
    t("ImageTitle", 0xA436, Text),
    t("Photographer", 0xA437, Text),
    t("ImageEditor", 0xA438, Text),
    t("CameraFirmware", 0xA439, Text),
    t("RAWDevelopingSoftware", 0xA43A, Text),
    t("ImageEditingSoftware", 0xA43B, Text),
    t("MetadataEditingSoftware", 0xA43C, Text),
    t("CompositeImage", 0xA460, Enum(COMPOSITE_IMAGE)),
    t("Gamma", 0xA500, Num),
    g("GPSVersionID", 0x0000, Num),
    g("GPSLatitudeRef", 0x0001, Text),
    g("GPSLatitude", 0x0002, Num),
    g("GPSLongitudeRef", 0x0003, Text),
    g("GPSLongitude", 0x0004, Num),
    g("GPSAltitudeRef", 0x0005, Enum(ALTITUDE_REF)),
    g("GPSAltitude", 0x0006, Num),
    g("GPSTimeStamp", 0x0007, Num),
    g("GPSSatellites", 0x0008, Text),
    g("GPSStatus", 0x0009, Text),
    g("GPSMeasureMode", 0x000A, Text),
    g("GPSDOP", 0x000B, Num),
    g("GPSSpeedRef", 0x000C, Text),
    g("GPSSpeed", 0x000D, Num),
    g("GPSTrackRef", 0x000E, Text),
    g("GPSTrack", 0x000F, Num),
    g("GPSImgDirectionRef", 0x0010, Text),
    g("GPSImgDirection", 0x0011, Num),
    g("GPSMapDatum", 0x0012, Text),
    g("GPSDestLatitudeRef", 0x0013, Text),
    g("GPSDestLatitude", 0x0014, Num),
    g("GPSDestLongitudeRef", 0x0015, Text),
    g("GPSDestLongitude", 0x0016, Num),
    g("GPSDestBearingRef", 0x0017, Text),
    g("GPSDestBearing", 0x0018, Num),
    g("GPSDestDistanceRef", 0x0019, Text),
    g("GPSDestDistance", 0x001A, Num),
    g("GPSProcessingMethod", 0x001B, Text),
    g("GPSAreaInformation", 0x001C, Text),
    g("GPSDateStamp", 0x001D, Text),
    g("GPSDifferential", 0x001E, Enum(DIFFERENTIAL)),
    g("GPSHPositioningError", 0x001F, Num),
];

/// Other names tags are known by (older standards, common usage).
const ALIASES: &[(&str, &str)] = &[
    ("ISOSpeedRatings", "PhotographicSensitivity"),
    ("ISO", "PhotographicSensitivity"),
    ("ImageHeight", "ImageLength"),
    ("ExposureCompensation", "ExposureBiasValue"),
    ("SerialNumber", "BodySerialNumber"),
    ("OwnerName", "CameraOwnerName"),
];

/// The tag `name` names: a standard name or alias (any case), or a number (`0xa406`, `g6`
/// for the GPS IFD's).
pub fn tag_named(name: &str) -> Option<&'static Tag> {
    let n = name.trim();
    let n = ALIASES
        .iter()
        .find(|(a, _)| a.eq_ignore_ascii_case(n))
        .map_or(n, |(_, s)| s);
    if let Some(t) = TAGS.iter().find(|t| t.name.eq_ignore_ascii_case(n)) {
        return Some(t);
    }
    let l = n.to_ascii_lowercase();
    let (gps, hex) = match l.strip_prefix('g') {
        Some(h) => (true, h),
        None => (false, l.strip_prefix("0x").unwrap_or(&l)),
    };
    let id = u16::from_str_radix(hex.strip_prefix("0x").unwrap_or(hex), 16).ok()?;
    TAGS.iter().find(|t| t.gps == gps && t.id == id)
}

/// The tag stored under (gps, id), if it is one `exif:` names (the others are not kept).
pub fn tag_of(gps: bool, id: u16) -> Option<&'static Tag> {
    TAGS.iter().find(|t| t.gps == gps && t.id == id)
}

/// The tag a stored key names.
pub fn tag_of_key(key: &str) -> Option<&'static Tag> {
    let (gps, hex) = match key.strip_prefix('g') {
        Some(h) => (true, h),
        None => (false, key),
    };
    tag_of(gps, u16::from_str_radix(hex, 16).ok()?)
}

/// The tag names `prefix` starts (any case), in table order.
pub fn names(prefix: &str) -> impl Iterator<Item = &'static str> + '_ {
    TAGS.iter()
        .map(|t| t.name)
        .filter(move |n| n.len() >= prefix.len() && n[..prefix.len()].eq_ignore_ascii_case(prefix))
}

/// The codes of `word` among `words` (any case; `-` and spaces ignored: `multi-spot`).
pub(crate) fn codes(words: &[(u32, &str)], word: &str) -> Vec<u32> {
    let w = squash(word);
    let mut out: Vec<u32> = words
        .iter()
        .filter(|(_, x)| *x == w)
        .map(|(c, _)| *c)
        .collect();
    out.dedup();
    out
}

/// Lower case without spaces, `-` and `_`.
fn squash(s: &str) -> String {
    s.chars()
        .filter(|c| !matches!(c, ' ' | '-' | '_'))
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// The first number of a stored value (several are separated by spaces).
pub(crate) fn first_number(v: &str) -> Option<f64> {
    v.split(' ')
        .next()?
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
}

// ----- The named fields' words ----------------------------------------------------------------

/// `flash:` words: fired, did not fire, auto mode, red-eye reduction, forced on.
pub const FLASH_WORDS: [&str; 5] = ["yes", "no", "auto", "redeye", "forced"];
/// `wb:` words: the WhiteBalance mode, then light sources (LightSource).
pub const WB_WORDS: [&str; 8] = [
    "auto",
    "manual",
    "daylight",
    "cloudy",
    "shade",
    "tungsten",
    "fluorescent",
    "flash",
];
/// `program:` words: ExposureProgram, `auto`, and `bracket` (ExposureMode).
pub const PROGRAM_WORDS: [&str; 10] = [
    "manual",
    "auto",
    "program",
    "aperture",
    "shutter",
    "creative",
    "action",
    "portrait",
    "landscape",
    "bracket",
];
/// `metering:` words (MeteringMode 1–6).
pub const METERING_WORDS: [&str; 6] = [
    "average",
    "centre",
    "spot",
    "multispot",
    "matrix",
    "partial",
];

/// The canonical word an alias of a named field's value is (`fired` → `yes`, `av` →
/// `aperture`, `evaluative` → `matrix`); None when it is none of the field's.
pub(crate) fn field_word(list: &'static [&'static str], s: &str) -> Option<&'static str> {
    let w = squash(s);
    let alias = match (list[0], w.as_str()) {
        ("yes", "fired" | "on" | "true") => "yes",
        ("yes", "off" | "notfired" | "false") => "no",
        ("yes", "red" | "redeyereduction") => "redeye",
        ("yes", "compulsory" | "forcedon") => "forced",
        ("auto", "incandescent") => "tungsten",
        ("auto", "overcast") => "cloudy",
        ("auto", "sunny" | "fineweather" | "fine") => "daylight",
        ("auto", "awb") => "auto",
        ("manual", "m") => "manual",
        ("manual", "p" | "normal" | "programauto") => "program",
        ("manual", "a" | "av" | "aperturepriority") => "aperture",
        ("manual", "s" | "tv" | "shutterpriority") => "shutter",
        ("manual", "sports") => "action",
        ("manual", "autobracket") => "bracket",
        ("average", "center" | "centreweighted" | "centerweighted" | "centreweightedaverage") => {
            "centre"
        }
        ("average", "evaluative" | "pattern" | "multisegment") => "matrix",
        (_, w) => w,
    };
    list.iter().copied().find(|x| *x == alias)
}

/// Whether a Flash value is `word`'s ([`FLASH_WORDS`]).
pub fn flash_is(v: u16, word: &str) -> bool {
    let mode = (v >> 3) & 3;
    match word {
        "yes" => v & 1 != 0,
        "no" => v & 1 == 0,
        "auto" => mode == 3,
        "redeye" => v & 0x40 != 0,
        "forced" => mode == 1,
        _ => false,
    }
}

/// Whether a WhiteBalance mode and LightSource are `word`'s ([`WB_WORDS`]).
pub fn wb_is(wb: Option<u8>, light: Option<u16>, word: &str) -> bool {
    match word {
        "auto" => wb == Some(0),
        "manual" => wb == Some(1),
        _ => light.is_some_and(|l| match word {
            "daylight" => matches!(l, 1 | 9),
            "cloudy" => l == 10,
            "shade" => l == 11,
            "tungsten" => matches!(l, 3 | 17 | 24),
            "fluorescent" => matches!(l, 2 | 12..=16),
            "flash" => l == 4,
            _ => false,
        }),
    }
}

/// Whether an ExposureProgram and ExposureMode are `word`'s ([`PROGRAM_WORDS`]). `auto` is
/// the program mode, or an automatic exposure where no program is named.
pub fn program_is(program: Option<u8>, mode: Option<u8>, word: &str) -> bool {
    match word {
        "manual" => program == Some(1) || mode == Some(1),
        "auto" => program == Some(2) || (matches!(program, None | Some(0)) && mode == Some(0)),
        "bracket" => mode == Some(2),
        _ => {
            let code = EXPOSURE_PROGRAM
                .iter()
                .find(|(_, w)| *w == word)
                .map(|(c, _)| *c as u8);
            code.is_some() && program == code
        }
    }
}

/// Whether a MeteringMode is `word`'s ([`METERING_WORDS`]).
pub fn metering_is(v: u16, word: &str) -> bool {
    METERING_WORDS
        .iter()
        .position(|w| *w == word)
        .is_some_and(|i| u32::from(v) == i as u32 + 1)
}

// ----- `exif:` conditions ---------------------------------------------------------------------

/// What one `exif:` value asks of a tag.
#[derive(Clone, Debug, PartialEq)]
pub enum Want {
    /// Has the tag at all (`exif:Name`).
    Has,
    /// `=` / `:`: a number (within what was typed), a code (by number or word), a Flash
    /// word, or text contained (any case).
    Is {
        num: Option<Range>,
        codes: Vec<u32>,
        flash: Option<&'static str>,
        text: String,
    },
    /// `<`, `<=`, `>`, `>=`: the first number of the value.
    Cmp(Range),
}

/// One `exif:` value: the tag, what is wanted, and `!=`.
#[derive(Clone, Debug, PartialEq)]
pub struct Cond {
    pub tag: &'static Tag,
    pub not: bool,
    pub want: Want,
}
