//! EXIF access helpers and the "All fields" rendering of EXIF tags.

use std::borrow::Cow;

use exif::{Context, Exif, Field as ExifField, In, Reader, Tag, Value};

use crate::exif_tags::{self, Extra, Kind, Resolved, TYPE_UTF8};
use crate::i18n::Localizer;
use crate::{Field, FieldGroup, strings};

/// Longest raw string shown before truncation.
const MAX_RAW_CHARS: usize = 160;
/// Longest list of numbers shown before truncation.
const MAX_LIST: usize = 16;
/// Undefined/byte blobs up to this size are shown as hex.
const MAX_HEX_BYTES: usize = 16;

pub struct ExifData {
    pub exif: Exif,
}

/// Parses a TIFF-structured EXIF payload. Partial results are kept.
pub fn parse(tiff: &[u8]) -> (Option<ExifData>, Option<String>) {
    let mut reader = Reader::new();
    reader.continue_on_error(true);
    match reader.read_raw(tiff.to_vec()) {
        Ok(exif) => (Some(ExifData { exif }), None),
        Err(exif::Error::PartialResult(p)) => {
            let (exif, errs) = p.into_inner();
            let msg = errs.first().map(|e| e.to_string());
            (Some(ExifData { exif }), msg)
        }
        Err(e) => (None, Some(e.to_string())),
    }
}

/// The text of an ASCII value: its first string. TIFF allows several NUL-separated strings,
/// but in practice what follows the first NUL is the rest of a fixed-size buffer (Samsung
/// writes Make as "SAMSUNG" and junk in 20 bytes), so it is cut there. Leading NULs are
/// skipped, as [`exif_tags::text`] does.
fn ascii_of(v: &Value) -> Option<String> {
    match v {
        Value::Ascii(parts) => first_string(parts)
            .map(|p| decode_text(p))
            .filter(|s| !s.is_empty()),
        _ => None,
    }
}

/// The first string of an ASCII value, past any leading NULs.
fn first_string(parts: &[Vec<u8>]) -> Option<&Vec<u8>> {
    parts.iter().find(|p| !p.is_empty())
}

/// InkNames (0x014D) is the one ASCII tag whose value is a list of NUL-separated strings.
const INK_NAMES: u16 = 0x014D;

/// `f` with an ASCII value cut at its first NUL (see [`ascii_of`]); InkNames keeps its list.
fn cut_at_nul(f: &ExifField) -> Cow<'_, ExifField> {
    match &f.value {
        Value::Ascii(parts) if parts.len() > 1 && f.tag.number() != INK_NAMES => {
            Cow::Owned(ExifField {
                tag: f.tag,
                ifd_num: f.ifd_num,
                value: Value::Ascii(first_string(parts).into_iter().cloned().collect()),
            })
        }
        _ => Cow::Borrowed(f),
    }
}

/// EXIF ASCII is often UTF-8 in practice; fall back to Latin-1.
fn decode_text(b: &[u8]) -> String {
    exif_tags::text(b)
}

/// The first value of a rational-like field as a positive fraction.
fn ratio_of(v: &Value) -> Option<(u32, u32)> {
    match v {
        Value::Rational(v) => v.first().map(|r| (r.num, r.denom)),
        // TIFF/EP writers sometimes store exposure facts as SRATIONAL.
        Value::SRational(v) => v
            .first()
            .filter(|r| r.num >= 0 && r.denom > 0)
            .map(|r| (r.num as u32, r.denom as u32)),
        _ => None,
    }
}

fn real_of(v: &Value, i: usize) -> Option<f64> {
    let x = match v {
        Value::Rational(v) => v.get(i).filter(|r| r.denom != 0).map(|r| r.to_f64()),
        Value::SRational(v) => v.get(i).filter(|r| r.denom != 0).map(|r| r.to_f64()),
        Value::Short(v) => v.get(i).map(|&x| x as f64),
        Value::Long(v) => v.get(i).map(|&x| x as f64),
        Value::SShort(v) => v.get(i).map(|&x| x as f64),
        Value::SLong(v) => v.get(i).map(|&x| x as f64),
        Value::Float(v) => v.get(i).map(|&x| x as f64),
        Value::Double(v) => v.get(i).copied(),
        _ => None,
    }?;
    x.is_finite().then_some(x)
}

impl ExifData {
    /// The field in the primary image. A tag is also found in the other one of IFD0 and the
    /// Exif IFD: TIFF/EP puts Exif tags in IFD0, and some writers misplace TIFF tags.
    pub fn field(&self, tag: Tag) -> Option<&ExifField> {
        self.exif.get_field(tag, In::PRIMARY).or_else(|| {
            let other = match tag.context() {
                Context::Tiff => Context::Exif,
                Context::Exif => Context::Tiff,
                _ => return None,
            };
            self.exif.get_field(Tag(other, tag.number()), In::PRIMARY)
        })
    }

    /// Text of an ASCII or EXIF 3.0 UTF-8 field.
    pub fn ascii(&self, tag: Tag) -> Option<String> {
        self.field(tag).and_then(|f| self.text_value(&f.value))
    }

    fn text_value(&self, v: &Value) -> Option<String> {
        match v {
            Value::Ascii(_) => ascii_of(v),
            Value::Unknown(TYPE_UTF8, n, at) => {
                exif_tags::utf8_bytes(self.exif.buf(), self.exif.little_endian(), *n, *at)
                    .map(exif_tags::text)
                    .filter(|s| !s.is_empty())
            }
            _ => None,
        }
    }

    /// Text of a value of a text-like [`Kind`], whatever its field type.
    fn text_by_kind(&self, v: &Value, kind: Kind) -> Option<String> {
        let s = match (kind, v) {
            (_, Value::Ascii(_) | Value::Unknown(TYPE_UTF8, ..)) => self.text_value(v)?,
            (Kind::Utf16, Value::Byte(b) | Value::Undefined(b, _)) => exif_tags::utf16le(b),
            (Kind::Text, Value::Byte(b) | Value::Undefined(b, _)) => exif_tags::text(b),
            _ => return None,
        };
        (!s.is_empty()).then_some(s)
    }

    /// Text of one of the [`exif_tags::EXTRA`] text tags (XP*, EXIF 3.0 ImageTitle, …), from
    /// IFD0 or the Exif IFD.
    pub fn extra_text(&self, num: u16) -> Option<String> {
        let x = exif_tags::extra(num)?;
        let f = self.field(Tag(Context::Tiff, num))?;
        self.text_by_kind(&f.value, x.kind)
    }

    pub fn uint(&self, tag: Tag) -> Option<u32> {
        self.field(tag).and_then(|f| match &f.value {
            // TIFF/EP ISOSpeedRatings is sometimes written as SSHORT.
            Value::SShort(v) => v.first().and_then(|&x| u32::try_from(x).ok()),
            Value::SLong(v) => v.first().and_then(|&x| u32::try_from(x).ok()),
            v => v.get_uint(0),
        })
    }

    pub fn rational(&self, tag: Tag) -> Option<f64> {
        self.rational_at(tag, 0)
    }

    pub fn rational_at(&self, tag: Tag, i: usize) -> Option<f64> {
        real_of(&self.field(tag)?.value, i)
    }

    pub fn rational_parts(&self, tag: Tag) -> Option<(u32, u32)> {
        ratio_of(&self.field(tag)?.value)
    }

    /// Three rationals (degrees, minutes, seconds) → decimal.
    pub fn dms(&self, tag: Tag) -> Option<f64> {
        let d = self.rational_at(tag, 0)?;
        let m = self.rational_at(tag, 1).unwrap_or(0.0);
        let s = self.rational_at(tag, 2).unwrap_or(0.0);
        Some(d + m / 60.0 + s / 3600.0)
    }

    /// The serial-free facts of a Canon maker note (Make "Canon…").
    pub(crate) fn canon_note(&self) -> Option<crate::canon::CanonNote> {
        let make = self.ascii(Tag::Make)?;
        if !make.to_ascii_lowercase().starts_with("canon") {
            return None;
        }
        match &self.field(Tag::MakerNote)?.value {
            Value::Undefined(_, at) => {
                crate::canon::parse(self.exif.buf(), self.exif.little_endian(), *at as usize)
            }
            _ => None,
        }
    }

    /// The serial-free facts of a Panasonic maker note (Make "Panasonic…"): the lens name for
    /// files whose Exif has no LensModel (RW2s, most of Panasonic's JPEGs), the focus mode and
    /// stabilisation.
    pub(crate) fn panasonic_note(&self) -> Option<crate::panasonic::PanasonicNote> {
        let make = self.ascii(Tag::Make)?;
        if !make.to_ascii_lowercase().starts_with("panasonic") {
            return None;
        }
        match &self.field(Tag::MakerNote)?.value {
            Value::Undefined(_, at) => {
                crate::panasonic::parse(self.exif.buf(), self.exif.little_endian(), *at as usize)
            }
            _ => None,
        }
    }

    /// The serial-free facts of a Samsung maker note (Make "Samsung…"): the lens name for files
    /// whose Exif has no LensModel.
    pub(crate) fn samsung_note(&self) -> Option<crate::samsung::SamsungNote> {
        let make = self.ascii(Tag::Make)?;
        if !make.to_ascii_lowercase().starts_with("samsung") {
            return None;
        }
        match &self.field(Tag::MakerNote)?.value {
            Value::Undefined(_, at) => {
                crate::samsung::parse(self.exif.buf(), self.exif.little_endian(), *at as usize)
            }
            _ => None,
        }
    }

    /// The serial-free facts of a Fujifilm maker note (Make "Fujifilm…"; JPEGs and the RAF's
    /// JPEG preview): film simulation, tone, white balance, focus and drive settings.
    pub(crate) fn fuji_note(&self) -> Option<crate::fuji::FujiNote> {
        let make = self.ascii(Tag::Make)?;
        if !make.to_ascii_lowercase().starts_with("fujifilm") {
            return None;
        }
        match &self.field(Tag::MakerNote)?.value {
            Value::Undefined(b, _) => crate::fuji::parse(b),
            _ => None,
        }
    }

    pub fn byte(&self, tag: Tag) -> Option<u8> {
        match &self.field(tag)?.value {
            Value::Byte(v) | Value::Undefined(v, _) => v.first().copied(),
            v => v.get_uint(0).map(|x| x as u8),
        }
    }

    /// Renders every EXIF field, grouped by IFD.
    pub fn field_groups(&self, l: &Localizer) -> Vec<FieldGroup> {
        let order = [
            "fields-ifd0",
            "fields-exif",
            "fields-interop",
            "fields-gps",
            "fields-ifd1",
        ];
        let mut groups: Vec<FieldGroup> = order
            .iter()
            .map(|id| FieldGroup {
                title: l.text(id),
                fields: Vec::new(),
            })
            .collect();
        for f in self.exif.fields() {
            if exif_tags::identifies_someone(f.tag) {
                continue;
            }
            let f = &*cut_at_nul(f);
            let idx = if f.ifd_num != In::PRIMARY {
                4
            } else {
                match f.tag.context() {
                    Context::Tiff => 0,
                    Context::Exif => 1,
                    Context::Interop => 2,
                    Context::Gps => 3,
                    _ => 1,
                }
            };
            let res = exif_tags::resolve(f.tag);
            // Rendered in full; the panel shows a truncated copy, "Copy value" copies `full`.
            let shown = self.display(l, f, res);
            let display = truncate(shown.clone());
            let full = (display != shown).then_some(shown);
            // The standard tag whose "no information" rules apply.
            let (std, internal) = match res {
                Resolved::Std(t) => (t, false),
                Resolved::Extra(Extra {
                    kind: Kind::Alias(t),
                    ..
                }) => (*t, false),
                Resolved::Extra(x) => (f.tag, x.kind == Kind::Internal),
                Resolved::Unknown => (f.tag, false),
            };
            groups[idx].fields.push(Field {
                tag: tag_name(l, f.tag, res),
                raw: truncate(self.raw(l, &f.value)),
                empty: internal || carries_no_information(f, std, &display),
                display,
                full,
            });
        }
        groups.retain(|g| !g.fields.is_empty());
        groups
    }

    /// The stored value, untruncated; EXIF 3.0 UTF-8 text is shown as text.
    fn raw(&self, l: &Localizer, v: &Value) -> String {
        match v {
            Value::Unknown(TYPE_UTF8, ..) => self.text_value(v).unwrap_or_default(),
            _ => raw_value(l, v),
        }
    }

    fn display(&self, l: &Localizer, f: &ExifField, res: Resolved) -> String {
        if let Value::Unknown(TYPE_UTF8, ..) = f.value {
            return self.raw(l, &f.value);
        }
        match res {
            Resolved::Std(tag) => self.display_std(l, f, tag),
            Resolved::Extra(x) => self.display_extra(l, f, x),
            Resolved::Unknown => match &f.value {
                Value::Undefined(b, _) | Value::Byte(b) if b.len() > MAX_HEX_BYTES => {
                    strings::bytes_blob(l, b.len())
                }
                v => raw_value(l, v),
            },
        }
    }

    /// Decodes `f` as the standard tag `tag` (which differs from `f.tag` for a tag found in
    /// the other IFD, or an alias).
    fn display_std(&self, l: &Localizer, f: &ExifField, tag: Tag) -> String {
        let remapped;
        let f = if f.tag == tag {
            f
        } else {
            remapped = ExifField {
                tag,
                ifd_num: f.ifd_num,
                value: f.value.clone(),
            };
            &remapped
        };
        if f.tag == Tag::MakerNote
            && let Value::Undefined(b, _) = &f.value
        {
            return strings::maker_note_blob(l, b.len());
        }
        match &f.value {
            Value::Undefined(b, _) | Value::Byte(b)
                if b.len() > MAX_HEX_BYTES && !is_text_tag(f.tag) =>
            {
                return strings::bytes_blob(l, b.len());
            }
            Value::Unknown(..) => return raw_value(l, &f.value),
            _ => {}
        }
        // Plain-language overrides for tags the summary also decodes. They read `f` itself, not
        // the primary-image lookup, so a copy in another IFD shows its own value.
        let custom = match f.tag {
            Tag::ExposureTime => {
                ratio_of(&f.value).and_then(|(n, d)| strings::exposure_time(l, n, d))
            }
            Tag::FNumber => real_of(&f.value, 0).map(|v| strings::f_number(l, v)),
            Tag::Flash => f.value.get_uint(0).map(|v| strings::flash(l, v)),
            Tag::ExposureBiasValue => real_of(&f.value, 0).map(|v| strings::exposure_bias(l, v)),
            Tag::Orientation => f
                .value
                .get_uint(0)
                .and_then(|v| strings::orientation(l, v as u8)),
            _ => None,
        };
        custom.unwrap_or_else(|| f.display_value().with_unit(&self.exif).to_string())
    }

    fn display_extra(&self, l: &Localizer, f: &ExifField, x: &Extra) -> String {
        let v = &f.value;
        let s = match x.kind {
            Kind::Alias(tag) => return self.display_std(l, f, tag),
            Kind::Text if x.num == INK_NAMES => match v {
                Value::Ascii(parts) => {
                    let names: Vec<String> = parts
                        .iter()
                        .map(|p| decode_text(p))
                        .filter(|s| !s.is_empty())
                        .collect();
                    (!names.is_empty()).then(|| strings::join(l, &names))
                }
                _ => self.text_by_kind(v, x.kind),
            },
            Kind::Text | Kind::Utf16 => self.text_by_kind(v, x.kind),
            Kind::Version => match v {
                Value::Byte(b) if !b.is_empty() && b.len() <= 8 => {
                    Some(b.iter().map(u8::to_string).collect::<Vec<_>>().join("."))
                }
                _ => None,
            },
            Kind::Rating => v
                .get_uint(0)
                .and_then(|r| strings::rating(l, r.min(5) as i8)),
            Kind::Coded(id, known) => coded_list(l, v, id, known),
            Kind::Plain | Kind::Internal => None,
        };
        s.unwrap_or_else(|| plain_value(l, v))
    }
}

/// Codes decoded through message `id` (`$value` selector). A per-sample list such as
/// SampleFormat [1, 1, 1] is shown once when all entries agree. `None` when any code is not
/// in `known` (shown as stored then).
fn coded_list(l: &Localizer, v: &Value, id: &str, known: &[u32]) -> Option<String> {
    let codes: Vec<u32> = (0..).map_while(|i| v.get_uint(i)).collect();
    if codes.is_empty() || codes.len() > MAX_LIST || !codes.iter().all(|c| known.contains(c)) {
        return None;
    }
    let mut parts: Vec<String> = Vec::new();
    for (i, c) in codes.iter().enumerate() {
        if codes.iter().all(|x| x == c) && i > 0 {
            break;
        }
        parts.push(strings::coded(l, id, *c));
    }
    Some(strings::join(l, &parts))
}

/// Numbers as stored, but rationals and floats as decimals (matrices, levels); blobs by size.
fn plain_value(l: &Localizer, v: &Value) -> String {
    let dec = |x: f64| l.trimmed(x, 4);
    match v {
        Value::Undefined(b, _) | Value::Byte(b) if b.len() > MAX_HEX_BYTES => {
            strings::bytes_blob(l, b.len())
        }
        Value::Rational(r) => list(
            l,
            &r.iter()
                .map(|r| {
                    if r.denom == 0 {
                        format!("{}/0", r.num)
                    } else {
                        dec(r.to_f64())
                    }
                })
                .collect::<Vec<_>>(),
        ),
        Value::SRational(r) => list(
            l,
            &r.iter()
                .map(|r| {
                    if r.denom == 0 {
                        format!("{}/0", r.num)
                    } else {
                        dec(r.to_f64())
                    }
                })
                .collect::<Vec<_>>(),
        ),
        Value::Float(f) => list(l, &f.iter().map(|&x| dec(x as f64)).collect::<Vec<_>>()),
        Value::Double(f) => list(l, &f.iter().map(|&x| dec(x)).collect::<Vec<_>>()),
        v => raw_value(l, v),
    }
}

/// Tags whose value 0 means "not recorded" rather than a measurement of zero. (A 0 EV exposure
/// compensation, 0 m altitude or 0° direction are real values and are not listed.)
const ZERO_IS_UNSET: &[Tag] = &[
    Tag::ExposureTime,
    Tag::FNumber,
    Tag::FocalLength,
    Tag::FocalLengthIn35mmFilm,
    Tag::PhotographicSensitivity,
    Tag::SubjectDistance,
    Tag::DigitalZoomRatio,
    Tag::ExposureIndex,
    Tag::CompressedBitsPerPixel,
    Tag::LensSpecification,
    Tag::PixelXDimension,
    Tag::PixelYDimension,
    Tag::ImageWidth,
    Tag::ImageLength,
    Tag::XResolution,
    Tag::YResolution,
    Tag::FocalPlaneXResolution,
    Tag::FocalPlaneYResolution,
    // Codes whose 0 is "unknown"/"not defined".
    Tag::ExposureProgram,
    Tag::MeteringMode,
    Tag::LightSource,
    Tag::SubjectDistanceRange,
    Tag::SensitivityType,
];

/// Padding (0xEA1C) and Microsoft's OffsetSchema (0xEA1D): space reserved by writers.
const PADDING_TAGS: [u16; 2] = [0xEA1C, 0xEA1D];
/// Rating (0x4746) and RatingPercent (0x4749): 0 means "not rated".
const RATING_TAGS: [u16; 2] = [0x4746, 0x4749];

/// Whether a field is present but says nothing (DESIGN §6.7): blank text, all-zero blobs,
/// unset rationals (0/0), zero where zero means unset, "unknown"/"not defined" codes, padding.
///
/// `tag` is the standard tag `f` was resolved to (see [`exif_tags::resolve`]); its rules apply.
pub(crate) fn carries_no_information(f: &ExifField, tag: Tag, display: &str) -> bool {
    let n = tag.number();
    if PADDING_TAGS.contains(&n) {
        return true;
    }
    let blank = |b: &[u8]| b.iter().all(|&c| c == 0 || c.is_ascii_whitespace());
    let empty_value = match &f.value {
        Value::Ascii(parts) => parts.iter().all(|p| blank(p)),
        // UserComment: an 8-byte character-code prefix, then the text.
        Value::Undefined(b, _) if tag == Tag::UserComment => blank(b.get(8..).unwrap_or(&[])),
        Value::Undefined(b, _) | Value::Byte(b) => blank(b),
        Value::Rational(v) => v.iter().all(|r| r.denom == 0),
        Value::SRational(v) => v.iter().all(|r| r.denom == 0),
        _ => false,
    };
    if empty_value {
        return true;
    }
    let zero = match &f.value {
        Value::Rational(v) => v.iter().all(|r| r.num == 0),
        Value::SRational(v) => v.iter().all(|r| r.num == 0),
        v => (0..).map_while(|i| v.get_uint(i)).all(|x| x == 0) && v.get_uint(0).is_some(),
    };
    if zero
        && (ZERO_IS_UNSET.contains(&tag)
            || (matches!(tag.context(), Context::Tiff | Context::Exif) && RATING_TAGS.contains(&n)))
    {
        return true;
    }
    if tag == Tag::SensingMethod && f.value.get_uint(0) == Some(1) {
        return true; // "not defined"
    }
    let d = display.trim().to_ascii_lowercase();
    d.is_empty()
        || d == "unknown"
        || d.starts_with("unknown (")
        || d == "not defined"
        || d == "unused"
}

fn is_text_tag(tag: Tag) -> bool {
    matches!(tag, Tag::UserComment)
}

/// "ExposureTime" (standard names, also for a tag found in the other IFD), "XPTitle" (our
/// extra table), or "Unknown tag 0xBEEF (Exif)".
fn tag_name(l: &Localizer, tag: Tag, res: Resolved) -> String {
    if let Resolved::Std(t) = res {
        t.to_string()
    } else if let Resolved::Extra(x) = res {
        x.name.to_owned()
    } else {
        let ctx = match tag.context() {
            Context::Tiff => "IFD",
            Context::Exif => "Exif",
            Context::Gps => "GPS",
            Context::Interop => "Interop",
            _ => "?",
        };
        strings::unknown_tag(l, tag.number(), ctx)
    }
}

pub(crate) fn truncate(s: String) -> String {
    if s.chars().count() <= MAX_RAW_CHARS {
        return s;
    }
    let mut t: String = s.chars().take(MAX_RAW_CHARS).collect();
    t.push_str(strings::ELLIPSIS);
    t
}

fn list<T: ToString>(l: &Localizer, v: &[T]) -> String {
    let shown: Vec<String> = v.iter().take(MAX_LIST).map(ToString::to_string).collect();
    let s = shown.join(", ");
    if v.len() > MAX_LIST {
        strings::more_values(l, &s, v.len())
    } else {
        s
    }
}

fn hex(b: &[u8]) -> String {
    b.iter()
        .map(|x| format!("{x:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// The stored value, minimally formatted: rationals as n/d, text as text, blobs by size. Not
/// truncated (lists are still capped at [`MAX_LIST`] entries).
pub fn raw_value(l: &Localizer, v: &Value) -> String {
    match v {
        Value::Byte(b) | Value::Undefined(b, _) => {
            if b.len() <= MAX_HEX_BYTES {
                hex(b)
            } else {
                strings::bytes_blob(l, b.len())
            }
        }
        Value::Ascii(parts) => parts
            .iter()
            .map(|p| decode_text(p))
            .collect::<Vec<_>>()
            .join(", "),
        Value::Short(v) => list(l, v),
        Value::Long(v) => list(l, v),
        Value::SByte(v) => list(l, v),
        Value::SShort(v) => list(l, v),
        Value::SLong(v) => list(l, v),
        Value::Float(v) => list(l, v),
        Value::Double(v) => list(l, v),
        Value::Rational(v) => list(
            l,
            &v.iter()
                .map(|r| format!("{}/{}", r.num, r.denom))
                .collect::<Vec<_>>(),
        ),
        Value::SRational(v) => list(
            l,
            &v.iter()
                .map(|r| format!("{}/{}", r.num, r.denom))
                .collect::<Vec<_>>(),
        ),
        Value::Unknown(t, n, _) => format!("(type {t}, {n} values)"),
    }
}
