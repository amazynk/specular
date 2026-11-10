//! Builds every user-visible value from the Fluent catalogue and ICU4X formatters.
//!
//! Nothing here contains English text: words and sentence templates live in
//! `i18n/<lang>/metadata.ftl`, numbers and dates are formatted by ICU4X via
//! [`Localizer`]. This module only decides *which* message to use and prepares
//! its arguments (rounding, choosing units, picking a compass point).

use crate::i18n::Localizer;

/// Distance (km) beyond which a place is described as "near" the matched town.
pub const NEAR_THRESHOLD_KM: f64 = 30.0;

/// English text the About box must show for the offline place-name data.
/// [`Localizer::text`]`("geonames-attribution")` gives the translated version.
pub const GEONAMES_ATTRIBUTION: &str = "Place names: GeoNames (www.geonames.org), licensed under \
Creative Commons Attribution 4.0 (CC BY 4.0, https://creativecommons.org/licenses/by/4.0/). \
Offline lookup by the reverse_geocoder crate by Grant Miner (MIT or Apache-2.0). \
The data has been converted to a city list; it is provided as is, without warranty.";

/// Appended to raw values that were cut short.
pub const ELLIPSIS: &str = "…";

// ICC header field names in "All fields" (technical identifiers, as in ExifTool; not translated).
pub const ICC_PROFILE_SIZE: &str = "ProfileSize";
pub const ICC_CMM: &str = "PreferredCMM";
pub const ICC_VERSION: &str = "ProfileVersion";
pub const ICC_CLASS: &str = "ProfileClass";
pub const ICC_COLOUR_SPACE: &str = "ColorSpace";
pub const ICC_PCS: &str = "ConnectionSpace";
pub const ICC_DATE: &str = "ProfileDateTime";
pub const ICC_PLATFORM: &str = "PrimaryPlatform";
pub const ICC_MANUFACTURER: &str = "DeviceManufacturer";
pub const ICC_MODEL: &str = "DeviceModel";
pub const ICC_INTENT: &str = "RenderingIntent";
pub const ICC_CREATOR: &str = "ProfileCreator";
pub const ICC_DESCRIPTION: &str = "ProfileDescription";
pub const ICC_COPYRIGHT: &str = "ProfileCopyright";
pub const ICC_TAG_COUNT: &str = "TagCount";

/// Typographic minus, used in offsets such as "UTC−3".
pub const MINUS: char = '\u{2212}';

// ---------------------------------------------------------------------------
// Generic
// ---------------------------------------------------------------------------

pub fn join(l: &Localizer, parts: &[String]) -> String {
    parts.join(&l.text("list-separator"))
}

pub fn bytes_blob(l: &Localizer, n: usize) -> String {
    l.fmt("blob-bytes", &args!("num" => n as u64))
}

pub fn maker_note_blob(l: &Localizer, n: usize) -> String {
    l.fmt("maker-note-bytes", &args!("num" => n as u64))
}

pub fn more_values(l: &Localizer, shown: &str, total: usize) -> String {
    l.fmt(
        "more-values",
        &args!("shown" => shown.to_owned(), "total" => total as u64),
    )
}

pub fn unknown_tag(l: &Localizer, number: u16, context: &str) -> String {
    l.fmt(
        "unknown-tag",
        &args!("number" => format!("0x{number:04X}"), "context" => context.to_owned()),
    )
}

pub fn fields_xmp(l: &Localizer, prefix: &str) -> String {
    let namespace = l.fmt("xmp-namespace", &args!("prefix" => prefix.to_owned()));
    l.fmt(
        "fields-xmp",
        &args!("namespace" => namespace, "prefix" => prefix.to_owned()),
    )
}

// ---------------------------------------------------------------------------
// Exposure
// ---------------------------------------------------------------------------

/// Exposure time from a rational: "1/250 s", "1/3 s", "0.6 s", "2.5 s", "30 s".
pub fn exposure_time(l: &Localizer, num: u32, denom: u32) -> Option<String> {
    if denom == 0 || num == 0 {
        return None;
    }
    let t = num as f64 / denom as f64;
    let seconds = |t: f64| {
        l.fmt(
            "exposure-time-seconds",
            &args!("seconds" => l.trimmed(t, 1)),
        )
    };
    let fraction = |d: u64| {
        l.fmt(
            "exposure-time-fraction",
            &args!("denominator" => l.int_plain(d)),
        )
    };
    if t >= 1.0 {
        return Some(seconds(t));
    }
    if num == 1 {
        return Some(fraction(denom as u64));
    }
    let inv = 1.0 / t;
    let rounded = inv.round();
    // 1/n when the reciprocal is (nearly) whole, or always for short exposures
    // where a decimal would be unreadable.
    if t < 0.25 || ((inv - rounded).abs() / rounded) < 0.02 {
        Some(fraction(rounded as u64))
    } else {
        Some(seconds(t))
    }
}

pub fn f_number(l: &Localizer, f: f64) -> String {
    l.fmt("aperture-value", &args!("f" => l.trimmed(f, 1)))
}

pub fn iso(l: &Localizer, iso: u32) -> String {
    l.fmt("iso-value", &args!("iso" => l.int_plain(iso as u64)))
}

pub fn focal_length(l: &Localizer, mm: f64, equiv_35mm: Option<u32>) -> String {
    let mm_s = l.trimmed(mm, 1);
    match equiv_35mm {
        Some(eq) if eq > 0 && (eq as f64 - mm).abs() >= 0.5 => l.fmt(
            "focal-length-with-equiv",
            &args!("mm" => mm_s, "equiv" => l.int_plain(eq as u64)),
        ),
        _ => l.fmt("focal-length-value", &args!("mm" => mm_s)),
    }
}

pub fn focal_length_equiv_only(l: &Localizer, eq: u32) -> String {
    l.fmt(
        "focal-length-equiv-only",
        &args!("equiv" => l.int_plain(eq as u64)),
    )
}

/// "+0.7 EV", "−1.3 EV", "0 EV".
pub fn exposure_bias(l: &Localizer, ev: f64) -> String {
    let r = (ev * 10.0).round() / 10.0;
    let sign = if r == 0.0 {
        "zero"
    } else if r > 0.0 {
        "plus"
    } else {
        "minus"
    };
    l.fmt(
        "exposure-bias-value",
        &args!("sign" => sign, "ev" => l.trimmed(r.abs(), 1)),
    )
}

pub fn one_line(l: &Localizer, parts: &[String]) -> String {
    parts.join(&l.text("one-line-separator"))
}

pub(crate) fn coded(l: &Localizer, id: &str, value: u32) -> String {
    l.fmt(id, &args!("value" => value))
}

pub fn exposure_program(l: &Localizer, v: u32) -> Option<String> {
    (1..=8)
        .contains(&v)
        .then(|| coded(l, "exposure-program", v))
}

pub fn metering_mode(l: &Localizer, v: u32) -> Option<String> {
    matches!(v, 1..=6 | 255).then(|| coded(l, "metering-mode", v))
}

pub fn light_source(l: &Localizer, v: u32) -> Option<String> {
    matches!(v, 1..=4 | 9..=24 | 255).then(|| coded(l, "light-source", v))
}

pub fn white_balance(l: &Localizer, v: u32, light: Option<String>) -> Option<String> {
    match (v, light) {
        (0, _) => Some(l.text("white-balance-auto")),
        (1, Some(light)) => {
            Some(l.fmt("white-balance-manual-with-light", &args!("light" => light)))
        }
        (1, None) => Some(l.text("white-balance-manual")),
        _ => None,
    }
}

/// Decodes the EXIF Flash bit field into words.
pub fn flash(l: &Localizer, v: u32) -> String {
    if v & 0x20 != 0 && v & 0x01 == 0 {
        return l.text("flash-none");
    }
    let mut parts = vec![l.text(if v & 0x01 != 0 {
        "flash-fired"
    } else {
        "flash-not-fired"
    })];
    match (v >> 3) & 0x3 {
        1 => parts.push(l.text("flash-forced-on")),
        2 => parts.push(l.text("flash-forced-off")),
        3 => parts.push(l.text("flash-auto")),
        _ => {}
    }
    match (v >> 1) & 0x3 {
        2 => parts.push(l.text("flash-return-not-detected")),
        3 => parts.push(l.text("flash-return-detected")),
        _ => {}
    }
    if v & 0x40 != 0 {
        parts.push(l.text("flash-red-eye"));
    }
    join(l, &parts)
}

// ---------------------------------------------------------------------------
// Lens
// ---------------------------------------------------------------------------

/// "24–70 mm f/2.8", "24–105 mm f/3.5–5.6", "50 mm f/1.8".
pub fn lens_range(
    l: &Localizer,
    min_mm: f64,
    max_mm: f64,
    min_f: Option<f64>,
    max_f: Option<f64>,
) -> String {
    let focal = if (max_mm - min_mm).abs() < 0.05 || max_mm <= 0.0 {
        l.fmt("lens-focal-single", &args!("mm" => l.trimmed(min_mm, 1)))
    } else {
        l.fmt(
            "lens-focal-range",
            &args!("min" => l.trimmed(min_mm, 1), "max" => l.trimmed(max_mm, 1)),
        )
    };
    let aperture = match (min_f, max_f) {
        (Some(a), Some(b)) if (a - b).abs() >= 0.05 && b > 0.0 => Some(l.fmt(
            "lens-aperture-range",
            &args!("min" => l.trimmed(a, 1), "max" => l.trimmed(b, 1)),
        )),
        (Some(a), _) if a > 0.0 => {
            Some(l.fmt("lens-aperture-single", &args!("f" => l.trimmed(a, 1))))
        }
        _ => None,
    };
    match aperture {
        Some(a) => l.fmt("lens-range", &args!("focal" => focal, "aperture" => a)),
        None => focal,
    }
}

// ---------------------------------------------------------------------------
// Dates
// ---------------------------------------------------------------------------

/// "UTC+1", "UTC+5:30", "UTC−3", "UTC".
pub fn utc_offset(l: &Localizer, offset_minutes: i32) -> String {
    if offset_minutes == 0 {
        return l.text("zone-utc");
    }
    let sign = if offset_minutes > 0 { '+' } else { MINUS };
    let a = offset_minutes.unsigned_abs() as u64;
    let offset = if a.is_multiple_of(60) {
        format!("{sign}{}", l.int_plain(a / 60))
    } else {
        format!("{sign}{}:{:02}", l.int_plain(a / 60), a % 60)
    };
    l.fmt("zone-offset", &args!("offset" => offset))
}

/// "Tuesday 14 May 2024, 18:32 (UTC+1)" (en-GB); "Dienstag, 14. Mai 2024, 18:32 (UTC+1)" (de).
pub fn date_time(
    l: &Localizer,
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    offset: Option<i32>,
) -> Option<String> {
    let date = l.date_with_weekday(year, month as u8, day as u8)?;
    let time = l.time_hm(hour as u8, minute as u8)?;
    let dt = l.fmt("date-time", &args!("date" => date, "time" => time));
    Some(match offset {
        Some(off) => l.fmt(
            "date-time-zone",
            &args!("datetime" => dt, "zone" => utc_offset(l, off)),
        ),
        None => dt,
    })
}

/// "14 May 2024, 17:32:10 UTC" (GPS time is always UTC).
pub fn utc_date_time(
    l: &Localizer,
    year: i32,
    month: u32,
    day: u32,
    h: u32,
    m: u32,
    s: u32,
) -> Option<String> {
    let date = l.date(year, month as u8, day as u8)?;
    let time = l.time_hms(h as u8, m as u8, s as u8)?;
    Some(l.fmt("gps-date-time", &args!("date" => date, "time" => time)))
}

// ---------------------------------------------------------------------------
// Location
// ---------------------------------------------------------------------------

/// "38.7230° N, 9.1390° W" / "38,7230° N, 9,1390° W".
pub fn coordinates(l: &Localizer, lat: f64, lon: f64) -> String {
    l.fmt(
        "coordinates",
        &args!(
            "lat" => l.fixed(lat.abs(), 4),
            "latRef" => if lat < 0.0 { "south" } else { "north" },
            "lon" => l.fixed(lon.abs(), 4),
            "lonRef" => if lon < 0.0 { "west" } else { "east" },
        ),
    )
}

pub fn altitude(l: &Localizer, m: f64) -> String {
    let r = m.round();
    let reference = if r == 0.0 {
        "sea"
    } else if r > 0.0 {
        "above"
    } else {
        "below"
    };
    l.fmt(
        "altitude",
        &args!("ref" => reference, "m" => l.int(r.abs() as u64)),
    )
}

const COMPASS_16: [&str; 16] = [
    "n", "nne", "ne", "ene", "e", "ese", "se", "sse", "s", "ssw", "sw", "wsw", "w", "wnw", "nw",
    "nnw",
];

/// Selector key of the nearest of the 16 compass points.
pub fn compass_key(deg: f64) -> &'static str {
    let d = deg.rem_euclid(360.0);
    COMPASS_16[((d / 22.5).round() as usize) % 16]
}

/// "Facing west-northwest (287°)".
pub fn facing(l: &Localizer, deg: f64) -> String {
    let point = l.fmt("compass-point", &args!("point" => compass_key(deg)));
    let degrees = (deg.rem_euclid(360.0).round() as u64) % 360;
    l.fmt(
        "facing",
        &args!("point" => point, "degrees" => l.int_plain(degrees)),
    )
}

pub fn accuracy(l: &Localizer, m: f64) -> String {
    l.fmt(
        "accuracy",
        &args!("m" => l.trimmed(m, if m < 10.0 { 1 } else { 0 })),
    )
}

pub fn distance_km(l: &Localizer, km: f64) -> String {
    let km = if km < 10.0 {
        l.trimmed(km, 1)
    } else {
        l.int(km.round() as u64)
    };
    l.fmt("distance-km", &args!("km" => km))
}

/// "Lisbon, Portugal", or "near Lisbon, Portugal" beyond [`NEAR_THRESHOLD_KM`].
pub fn place_line(l: &Localizer, city: &str, country: &str, distance_km: f64) -> String {
    let base = if country.is_empty() {
        l.fmt("place-city-only", &args!("city" => city.to_owned()))
    } else {
        l.fmt(
            "place",
            &args!("city" => city.to_owned(), "country" => country.to_owned()),
        )
    };
    if distance_km > NEAR_THRESHOLD_KM {
        l.fmt("place-near", &args!("place" => base))
    } else {
        base
    }
}

/// "Lisbon · 2.1 km from Lisbon" — the administrative areas and the distance to the matched town.
pub fn place_detail(
    l: &Localizer,
    city: &str,
    admin1: &str,
    admin2: &str,
    distance: f64,
) -> String {
    let mut areas: Vec<String> = Vec::new();
    for a in [admin2, admin1] {
        if !a.is_empty() && a != city && !areas.iter().any(|x| x == a) {
            areas.push(a.to_owned());
        }
    }
    // Within 50 m of the matched point a distance ("0 km from …") says nothing.
    if distance < 0.05 {
        return if areas.is_empty() {
            l.fmt("place-city-only", &args!("city" => city.to_owned()))
        } else {
            join(l, &areas)
        };
    }
    let distance = distance_km(l, distance);
    if areas.is_empty() {
        l.fmt(
            "place-detail-distance",
            &args!("distance" => distance, "city" => city.to_owned()),
        )
    } else {
        l.fmt(
            "place-detail",
            &args!("areas" => join(l, &areas), "distance" => distance, "city" => city.to_owned()),
        )
    }
}

// ---------------------------------------------------------------------------
// Image
// ---------------------------------------------------------------------------

/// "6000 × 4000 (24.0 MP)".
pub fn dimensions(l: &Localizer, w: u32, h: u32) -> String {
    let mp = w as f64 * h as f64 / 1_000_000.0;
    let (w, h) = (l.int_plain(w as u64), l.int_plain(h as u64));
    if mp >= 0.1 {
        l.fmt(
            "dimensions-megapixels",
            &args!("width" => w, "height" => h, "mp" => l.fixed(mp, 1)),
        )
    } else {
        l.fmt("dimensions", &args!("width" => w, "height" => h))
    }
}

/// "8-bit YCbCr 4:2:0, progressive".
pub fn encoding(
    l: &Localizer,
    bit_depth: Option<u8>,
    model: Option<&str>,
    subsampling: Option<&str>,
    alpha: Option<&str>,
    progressive: Option<bool>,
) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    if let Some(b) = bit_depth {
        parts.push(l.fmt(
            "encoding-bit-depth",
            &args!("bits" => l.int_plain(b as u64)),
        ));
    }
    if let Some(m) = model {
        parts.push(l.fmt("colour-model", &args!("model" => m.to_owned())));
    }
    if let Some(s) = subsampling {
        parts.push(s.to_owned());
    }
    let mut format = parts.join(" ");
    if let Some(a) = alpha.filter(|_| !format.is_empty()) {
        format = l.fmt(
            "encoding-alpha",
            &args!("format" => format, "alpha" => a.to_owned()),
        );
    }
    match progressive {
        Some(p) if !format.is_empty() => Some(l.fmt(
            if p {
                "encoding-progressive"
            } else {
                "encoding-baseline"
            },
            &args!("format" => format),
        )),
        _ if !format.is_empty() => Some(format),
        _ => None,
    }
}

/// "8-bit YCbCr 4:2:0, hierarchical": an [`encoding`] of a hierarchical JPEG.
pub fn hierarchical(l: &Localizer, format: String) -> String {
    l.fmt("encoding-hierarchical", &args!("format" => format))
}

pub fn orientation(l: &Localizer, v: u8) -> Option<String> {
    (1..=8)
        .contains(&v)
        .then(|| l.fmt("orientation", &args!("value" => v as u32)))
}

/// Short form without "(applied)", for warnings.
pub fn orientation_short(l: &Localizer, v: u8) -> String {
    l.fmt("orientation-short", &args!("value" => v as u32))
}

// ---------------------------------------------------------------------------
// File
// ---------------------------------------------------------------------------

/// "2.5 MB", "48.2 KB", "512 bytes" (decimal units).
pub fn file_size(l: &Localizer, bytes: u64) -> String {
    const UNITS: [&str; 5] = ["kb", "mb", "gb", "tb", "pb"];
    if bytes < 1000 {
        return file_size_exact(l, bytes);
    }
    let mut v = bytes as f64 / 1000.0;
    let mut unit = 0;
    while v >= 999.95 && unit < UNITS.len() - 1 {
        v /= 1000.0;
        unit += 1;
    }
    l.fmt(
        "file-size-unit",
        &args!("num" => l.trimmed(v, 1), "unit" => UNITS[unit]),
    )
}

/// "2,456,789 bytes".
pub fn file_size_exact(l: &Localizer, bytes: u64) -> String {
    l.fmt("file-size-bytes", &args!("num" => bytes))
}

// ---------------------------------------------------------------------------
// Rights
// ---------------------------------------------------------------------------

/// "★★★★☆ (4 of 5)" or "Rejected"; `None` for 0, which means "not rated" and is not shown.
pub fn rating(l: &Localizer, r: i8) -> Option<String> {
    Some(match r {
        i8::MIN..=-1 => l.text("rating-rejected"),
        0 => return None,
        _ => {
            let r = r.min(5) as usize;
            let stars = format!("{}{}", "★".repeat(r), "☆".repeat(5 - r));
            l.fmt(
                "rating-stars",
                &args!("stars" => stars, "n" => l.int_plain(r as u64)),
            )
        }
    })
}

// ---------------------------------------------------------------------------
// ICC header values
// ---------------------------------------------------------------------------

/// Profile class signatures the catalogue names; any other code is shown as stored (never as
/// "Unknown").
const ICC_CLASSES: [&str; 7] = ["scnr", "mntr", "prtr", "link", "spac", "abst", "nmcl"];
const ICC_PLATFORMS: [&str; 4] = ["APPL", "MSFT", "SGI", "SUNW"];

pub fn icc_class(l: &Localizer, sig: &str) -> String {
    if !ICC_CLASSES.contains(&sig.trim()) {
        return sig.trim().to_owned();
    }
    let name = l.fmt("icc-class", &args!("class" => sig.trim().to_owned()));
    l.fmt(
        "icc-coded",
        &args!("name" => name, "code" => sig.to_owned()),
    )
}

pub fn icc_intent(l: &Localizer, v: u32) -> String {
    if v > 3 {
        return v.to_string();
    }
    l.fmt("icc-intent", &args!("intent" => v))
}

/// The platform's name; an unset platform is an empty field and not shown.
pub fn icc_platform(l: &Localizer, sig: &str) -> String {
    match sig.trim() {
        s if ICC_PLATFORMS.contains(&s) => {
            l.fmt("icc-platform", &args!("platform" => s.to_owned()))
        }
        s => s.to_owned(),
    }
}

// ---------------------------------------------------------------------------
// Warnings
// ---------------------------------------------------------------------------

pub fn warn_orientation_conflict(l: &Localizer, exif: u8, xmp: u8) -> String {
    l.fmt(
        "warn-orientation-conflict",
        &args!("exif" => orientation_short(l, exif), "xmp" => orientation_short(l, xmp)),
    )
}

pub fn warn_exif_damaged(l: &Localizer, err: &str) -> String {
    l.fmt("warn-exif-damaged", &args!("error" => err.to_owned()))
}

pub fn warn_xmp_damaged(l: &Localizer, err: &str) -> String {
    l.fmt("warn-xmp-damaged", &args!("error" => err.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::{langid, strip_isolates as s};

    #[test]
    fn exposure_values() {
        let l = Localizer::english();
        assert_eq!(s(&exposure_time(&l, 1, 250).unwrap()), "1/250 s");
        assert_eq!(s(&exposure_time(&l, 10, 2500).unwrap()), "1/250 s");
        assert_eq!(s(&exposure_time(&l, 5, 2).unwrap()), "2.5 s");
        assert_eq!(s(&exposure_time(&l, 6, 10).unwrap()), "0.6 s");
        assert_eq!(s(&exposure_time(&l, 30, 1).unwrap()), "30 s");
        assert_eq!(s(&f_number(&l, 2.8)), "f/2.8");
        assert_eq!(s(&f_number(&l, 8.0)), "f/8");
        assert_eq!(s(&iso(&l, 400)), "ISO 400");
        assert_eq!(s(&iso(&l, 12800)), "ISO 12800");
        assert_eq!(s(&focal_length(&l, 35.0, Some(52))), "35 mm (52 mm equiv.)");
        assert_eq!(s(&focal_length(&l, 50.0, Some(50))), "50 mm");
        assert_eq!(s(&exposure_bias(&l, 0.6667)), "+0.7 EV");
        assert_eq!(s(&exposure_bias(&l, -1.3333)), "\u{2212}1.3 EV");
        assert_eq!(s(&exposure_bias(&l, 0.0)), "0 EV");
        assert_eq!(s(&exposure_program(&l, 3).unwrap()), "Aperture priority");
        assert_eq!(exposure_program(&l, 0), None);
        assert_eq!(s(&metering_mode(&l, 5).unwrap()), "Matrix (evaluative)");
        assert_eq!(
            s(&white_balance(&l, 1, light_source(&l, 1)).unwrap()),
            "Manual (Daylight)"
        );
        assert_eq!(
            s(&lens_range(&l, 24.0, 70.0, Some(2.8), Some(2.8))),
            "24–70 mm f/2.8"
        );
        assert_eq!(
            s(&lens_range(&l, 24.0, 105.0, Some(3.5), Some(5.6))),
            "24–105 mm f/3.5–5.6"
        );
    }

    #[test]
    fn flash_words() {
        let l = Localizer::english();
        assert_eq!(s(&flash(&l, 0x00)), "Flash did not fire");
        assert_eq!(s(&flash(&l, 0x01)), "Flash fired");
        assert_eq!(s(&flash(&l, 0x10)), "Flash did not fire, forced off");
        assert_eq!(s(&flash(&l, 0x19)), "Flash fired, auto mode");
        assert_eq!(
            s(&flash(&l, 0x1F)),
            "Flash fired, auto mode, return light detected"
        );
        assert_eq!(s(&flash(&l, 0x20)), "No flash function");
        assert_eq!(
            s(&flash(&l, 0x59)),
            "Flash fired, auto mode, red-eye reduction"
        );
        let de = Localizer::new(langid!("de"));
        assert_eq!(s(&flash(&de, 0x19)), "Blitz ausgelöst, Automatik");
    }

    #[test]
    fn location_values() {
        let l = Localizer::english();
        assert_eq!(
            s(&coordinates(&l, 38.7223, -9.1393)),
            "38.7223° N, 9.1393° W"
        );
        assert_eq!(
            s(&coordinates(&l, -34.6132, -58.3772)),
            "34.6132° S, 58.3772° W"
        );
        assert_eq!(s(&altitude(&l, 112.4)), "112 m above sea level");
        assert_eq!(s(&altitude(&l, -3.0)), "3 m below sea level");
        assert_eq!(s(&altitude(&l, 0.2)), "At sea level");
        assert_eq!(s(&facing(&l, 287.0)), "Facing west-northwest (287°)");
        assert_eq!(s(&facing(&l, 359.9)), "Facing north (0°)");
        assert_eq!(s(&accuracy(&l, 5.0)), "±5 m");
        assert_eq!(
            s(&place_line(&l, "Lisbon", "Portugal", 1.2)),
            "Lisbon, Portugal"
        );
        assert_eq!(
            s(&place_line(&l, "Lisbon", "Portugal", 31.0)),
            "near Lisbon, Portugal"
        );
        assert_eq!(s(&distance_km(&l, 1234.4)), "1,234 km");
        let de = Localizer::new(langid!("de"));
        assert_eq!(
            s(&coordinates(&de, 38.7223, 9.1393)),
            "38,7223° N, 9,1393° O"
        );
        assert_eq!(s(&facing(&de, 287.0)), "Blickrichtung Westnordwest (287°)");
    }

    #[test]
    fn image_and_file_values() {
        let l = Localizer::english();
        assert_eq!(s(&dimensions(&l, 6000, 4000)), "6000 × 4000 (24.0 MP)");
        assert_eq!(s(&dimensions(&l, 64, 48)), "64 × 48");
        assert_eq!(
            s(&encoding(&l, Some(8), Some("YCbCr"), Some("4:2:0"), None, Some(true)).unwrap()),
            "8-bit YCbCr 4:2:0, progressive"
        );
        assert_eq!(
            s(&encoding(&l, Some(8), Some("RGB"), None, Some("alpha"), Some(true)).unwrap()),
            "8-bit RGB with alpha, progressive"
        );
        assert_eq!(
            s(&encoding(&l, Some(16), Some("RGB"), None, Some("premultiplied"), None).unwrap()),
            "16-bit RGB with premultiplied alpha"
        );
        assert_eq!(
            s(&encoding(
                &l,
                Some(8),
                Some("Palette"),
                None,
                Some("transparency"),
                None
            )
            .unwrap()),
            "8-bit Indexed colour with transparency"
        );
        assert_eq!(encoding(&l, None, None, None, Some("alpha"), None), None);
        let e = encoding(&l, Some(8), Some("YCbCr"), Some("4:2:0"), None, None).unwrap();
        assert_eq!(s(&hierarchical(&l, e)), "8-bit YCbCr 4:2:0, hierarchical");
        assert_eq!(
            s(&orientation(&l, 6).unwrap()),
            "Rotated 90° clockwise (applied)"
        );
        assert_eq!(s(&file_size(&l, 512)), "512 bytes");
        assert_eq!(s(&file_size(&l, 1)), "1 byte");
        assert_eq!(s(&file_size(&l, 2_456_789)), "2.5 MB");
        assert_eq!(s(&file_size(&l, 999_999)), "1 MB");
        assert_eq!(s(&file_size_exact(&l, 2_456_789)), "2,456,789 bytes");
        assert_eq!(s(&rating(&l, 4).unwrap()), "★★★★☆ (4 of 5)");
        assert_eq!(s(&rating(&l, -1).unwrap()), "Rejected");
        assert_eq!(rating(&l, 0), None);
        assert_eq!(s(&bytes_blob(&l, 1)), "(1 byte)");
        assert_eq!(s(&bytes_blob(&l, 4096)), "(4,096 bytes)");
        let de = Localizer::new(langid!("de"));
        assert_eq!(s(&file_size(&de, 2_456_789)), "2,5 MB");
        assert_eq!(s(&file_size_exact(&de, 2_456_789)), "2.456.789 Bytes");
        assert_eq!(s(&dimensions(&de, 6000, 4000)), "6000 × 4000 (24,0 MP)");
    }

    #[test]
    fn offsets() {
        let l = Localizer::english();
        assert_eq!(s(&utc_offset(&l, 60)), "UTC+1");
        assert_eq!(s(&utc_offset(&l, 330)), "UTC+5:30");
        assert_eq!(s(&utc_offset(&l, -180)), "UTC\u{2212}3");
        assert_eq!(s(&utc_offset(&l, 0)), "UTC");
    }
}
