//! Photos and GPS tracks: a track's fixes in time (GPX `trkpt`/`rtept` with `<time>`, TCX
//! `Trackpoint`s, IGC flight logs and NMEA logs with a date), the capture time, position and camera direction of the photos beside
//! it, where each photo goes on the track for a camera clock offset, and geotagging: the
//! positions written into the photos' XMP sidecar files.
//!
//! Geotagging writes only `.xmp` sidecars (the stem form `IMG_1.xmp`, or the long form when
//! files share a stem, as ratings do); the image files are never opened for writing. A
//! photo that already has a position, in its file or its sidecar, is left alone.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use specular_geodesy::time;
use specular_geodesy::track::{Fix, Placement, Timeline, place};
use specular_i18n::t;

use super::super::gallery::marks;

/// Photos within this many seconds of a GPS fix are placed.
pub(super) const MAX_GAP: f64 = 300.0;
/// Photos listed beside a track at most.
const MAX_PHOTOS: usize = 5000;

/// The fixes with times of a GPX or TCX file (tracks and routes; waypoints are not a path),
/// or of an IGC or NMEA log that gives the date, in time order. `None` when it has fewer
/// than two.
pub(super) fn timeline(bytes: &[u8]) -> Option<Timeline> {
    use specular_decode::geo::specular_gis::{igc, nmea};
    let head = &bytes[..bytes.len().min(4096)];
    if igc::sniff(head) {
        let f = igc::parse(bytes).ok().filter(|f| f.dated)?;
        let fixes = f
            .fixes
            .iter()
            .map(|x| Fix {
                t: x.t,
                lat: x.lat,
                lon: x.lon,
                ele: x.gnss_alt.filter(|a| *a != 0.0).or(x.pressure_alt),
            })
            .collect();
        let tl = Timeline::new(fixes);
        return (tl.fixes.len() >= 2).then_some(tl);
    }
    if nmea::sniff(head) {
        let log = nmea::parse(bytes);
        // Only a log with a date (RMC) places photos in time.
        let fixes = log
            .fixes
            .iter()
            .filter_map(|x| {
                Some(Fix {
                    t: x.t.filter(|t| *t >= 86_400.0)?,
                    lat: x.lat,
                    lon: x.lon,
                    ele: x.alt,
                })
            })
            .collect();
        let tl = Timeline::new(fixes);
        return (tl.fixes.len() >= 2).then_some(tl);
    }
    let text = std::str::from_utf8(bytes).ok()?;
    // roxmltree recurses once per element level: a deeply nested file would overflow the stack.
    if !specular_limits::xml_within_depth(bytes, specular_limits::MAX_XML_DEPTH) {
        return None;
    }
    let opt = roxmltree::ParsingOptions {
        allow_dtd: true,
        ..Default::default()
    };
    let doc = roxmltree::Document::parse_with_options(text, opt).ok()?;
    let mut fixes = Vec::new();
    let child_text = |n: roxmltree::Node, name: &str| {
        n.children()
            .find(|c| c.is_element() && c.tag_name().name() == name)
            .and_then(|c| c.text())
            .map(str::trim)
            .map(str::to_owned)
    };
    for n in doc.descendants().filter(|n| n.is_element()) {
        match n.tag_name().name() {
            "trkpt" | "rtept" => {
                let lat = n
                    .attribute("lat")
                    .and_then(|v| v.trim().parse::<f64>().ok());
                let lon = n
                    .attribute("lon")
                    .and_then(|v| v.trim().parse::<f64>().ok());
                let t = child_text(n, "time").and_then(|s| time::parse(&s));
                if let (Some(lat), Some(lon), Some(t)) = (lat, lon, t) {
                    let ele = child_text(n, "ele").and_then(|s| s.parse().ok());
                    fixes.push(Fix {
                        t: t.secs,
                        lat,
                        lon,
                        ele,
                    });
                }
            }
            "Trackpoint" => {
                let t = child_text(n, "Time").and_then(|s| time::parse(&s));
                let pos = n
                    .children()
                    .find(|c| c.is_element() && c.tag_name().name() == "Position");
                let ll = pos.and_then(|p| {
                    Some((
                        child_text(p, "LatitudeDegrees")?.parse::<f64>().ok()?,
                        child_text(p, "LongitudeDegrees")?.parse::<f64>().ok()?,
                    ))
                });
                if let (Some(t), Some((lat, lon))) = (t, ll) {
                    let ele = child_text(n, "AltitudeMeters").and_then(|s| s.parse().ok());
                    fixes.push(Fix {
                        t: t.secs,
                        lat,
                        lon,
                        ele,
                    });
                }
            }
            _ => {}
        }
    }
    let tl = Timeline::new(fixes);
    (tl.fixes.len() >= 2).then_some(tl)
}

/// Whether a file name is a track format whose times are read here.
pub(super) fn is_track_file(p: &Path) -> bool {
    p.extension().and_then(|e| e.to_str()).is_some_and(|e| {
        ["gpx", "tcx", "igc", "nmea"]
            .iter()
            .any(|k| e.eq_ignore_ascii_case(k))
    })
}

/// What a photo's metadata tells about where and when it was taken.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct PhotoFacts {
    pub path: PathBuf,
    /// The capture time on the camera's clock, seconds (as if UTC), and whether the file
    /// gave its zone (then it is true UTC).
    pub taken: Option<time::Stamp>,
    /// The position already recorded (in the file or its sidecar).
    pub position: Option<(f64, f64)>,
    /// GPSImgDirection: degrees clockwise from north (true or magnetic, as recorded).
    pub direction: Option<f64>,
}

/// A TIFF structure's IFD walker, for the few tags read here.
struct Tiff<'a> {
    b: &'a [u8],
    le: bool,
}

impl Tiff<'_> {
    fn u16(&self, o: usize) -> Option<u16> {
        let s = self.b.get(o..o + 2)?;
        Some(if self.le {
            u16::from_le_bytes([s[0], s[1]])
        } else {
            u16::from_be_bytes([s[0], s[1]])
        })
    }

    fn u32(&self, o: usize) -> Option<u32> {
        let s = self.b.get(o..o + 4)?;
        Some(if self.le {
            u32::from_le_bytes([s[0], s[1], s[2], s[3]])
        } else {
            u32::from_be_bytes([s[0], s[1], s[2], s[3]])
        })
    }

    /// The entry of `tag` in the IFD at `ifd`.
    fn find(&self, ifd: usize, tag: u16) -> Option<usize> {
        let n = self.u16(ifd)? as usize;
        (0..n.min(512))
            .map(|i| ifd + 2 + i * 12)
            .find(|&e| self.u16(e) == Some(tag))
    }

    /// An ASCII value (inline when it fits in four bytes).
    fn ascii(&self, e: usize) -> Option<String> {
        let count = self.u32(e + 4)? as usize;
        let at = if count <= 4 {
            e + 8
        } else {
            self.u32(e + 8)? as usize
        };
        let s = self.b.get(at..at.checked_add(count)?)?;
        let s = s.split(|&c| c == 0).next().unwrap_or(s);
        Some(String::from_utf8_lossy(s).trim().to_owned())
    }

    /// The first unsigned rational of an entry.
    fn rational(&self, e: usize) -> Option<f64> {
        if self.u16(e + 2)? != 5 {
            return None;
        }
        let at = self.u32(e + 8)? as usize;
        let (n, d) = (self.u32(at)?, self.u32(at + 4)?);
        (d != 0).then(|| f64::from(n) / f64::from(d))
    }
}

/// The capture offset and sub-seconds, and the camera direction, of a TIFF structure.
fn exif_extras(tiff: &[u8]) -> (Option<String>, Option<String>, Option<f64>) {
    let le = match tiff.get(..2) {
        Some(b"II") => true,
        Some(b"MM") => false,
        _ => return (None, None, None),
    };
    let t = Tiff { b: tiff, le };
    let Some(ifd0) = t.u32(4).map(|v| v as usize) else {
        return (None, None, None);
    };
    let exif = t
        .find(ifd0, 0x8769)
        .and_then(|e| t.u32(e + 8))
        .map(|v| v as usize);
    let offset = exif
        .and_then(|x| t.find(x, 0x9011))
        .and_then(|e| t.ascii(e));
    let subsec = exif
        .and_then(|x| t.find(x, 0x9291))
        .and_then(|e| t.ascii(e));
    let gps = t
        .find(ifd0, 0x8825)
        .and_then(|e| t.u32(e + 8))
        .map(|v| v as usize);
    let direction = gps
        .and_then(|g| t.find(g, 0x0011))
        .and_then(|e| t.rational(e))
        .filter(|d| (0.0..=360.0).contains(d));
    (offset, subsec, direction)
}

/// A photo's camera direction (GPSImgDirection, degrees clockwise from north).
pub(in crate::app) fn direction_of(path: &Path) -> Option<f64> {
    let f = std::fs::File::open(path).ok()?;
    let tiff = specular_decode::read_exif_tiff(std::io::BufReader::new(f))?;
    exif_extras(&tiff).2
}

/// A photo's facts, from its EXIF (DateTimeOriginal with its offset and sub-seconds,
/// GPS position and image direction) and its XMP sidecar's position.
pub(super) fn photo_facts(path: &Path, sidecar: Option<&Path>) -> PhotoFacts {
    let tiff = std::fs::File::open(path)
        .ok()
        .and_then(|f| specular_decode::read_exif_tiff(std::io::BufReader::new(f)));
    let mut facts = PhotoFacts {
        path: path.to_path_buf(),
        taken: None,
        position: None,
        direction: None,
    };
    if let Some(t) = tiff.as_deref() {
        let (offset, subsec, direction) = exif_extras(t);
        facts.direction = direction;
        facts.position = specular_decode::exif_position(t);
        if let Some(d) = specular_decode::exif_date_taken(t) {
            let mut s = d.trim().to_owned();
            if let Some(ss) =
                subsec.filter(|s| s.bytes().all(|b| b.is_ascii_digit()) && !s.is_empty())
            {
                s = format!("{s}.{ss}");
            }
            if let Some(o) = offset.filter(|o| time::offset(o).is_some()) {
                s.push_str(&o);
            }
            facts.taken = time::parse(&s);
        }
    }
    if facts.position.is_none()
        && let Some(sc) = sidecar
        && let Ok(text) = std::fs::read_to_string(sc)
    {
        facts.position = sidecar_position(&text);
    }
    facts
}

/// An XMP coordinate ("47,22.6123N", "47,22,36.7N") in signed degrees.
pub(super) fn xmp_coordinate(v: &str) -> Option<f64> {
    let v = v.trim();
    let dir = v.chars().last()?;
    let sign = match dir.to_ascii_uppercase() {
        'N' | 'E' => 1.0,
        'S' | 'W' => -1.0,
        _ => return None,
    };
    let body = &v[..v.len() - dir.len_utf8()];
    let parts: Vec<f64> = body
        .split(',')
        .map(|p| p.trim().parse::<f64>().ok())
        .collect::<Option<_>>()?;
    let deg = match parts.as_slice() {
        [d, m] => d + m / 60.0,
        [d, m, s] => d + m / 60.0 + s / 3600.0,
        _ => return None,
    };
    Some(sign * deg)
}

/// The position an XMP packet records (`exif:GPSLatitude`/`exif:GPSLongitude`).
pub(super) fn sidecar_position(xmp: &str) -> Option<(f64, f64)> {
    let lat = marks::xmp_value(xmp, "exif:GPSLatitude").and_then(xmp_coordinate)?;
    let lon = marks::xmp_value(xmp, "exif:GPSLongitude").and_then(xmp_coordinate)?;
    ((-90.0..=90.0).contains(&lat) && (-180.0..=180.0).contains(&lon)).then_some((lat, lon))
}

/// A coordinate as XMP writes it: degrees, decimal minutes and the hemisphere
/// ("47,22.612345N").
pub(super) fn xmp_coordinate_text(v: f64, lat: bool) -> String {
    let dir = match (lat, v >= 0.0) {
        (true, true) => 'N',
        (true, false) => 'S',
        (false, true) => 'E',
        (false, false) => 'W',
    };
    let a = v.abs();
    let mut d = a.floor();
    let mut m = (a - d) * 60.0;
    if (m * 1e6).round() >= 60e6 {
        d += 1.0;
        m = 0.0;
    }
    format!("{},{:.6}{dir}", d as u32, m)
}

const NS_EXIF: &str = "http://ns.adobe.com/exif/1.0/";

/// The XMP of a sidecar with a GPS position written in: `existing` (or a new packet) with
/// `exif:GPSLatitude`, `exif:GPSLongitude`, the altitude when known, the map datum and the
/// time of the fix. Everything else is kept as it is.
pub(super) fn geotag_xmp(
    existing: Option<&str>,
    lat: f64,
    lon: f64,
    ele: Option<f64>,
    fix_time: f64,
) -> String {
    let base = match existing.filter(|s| s.contains("rdf:Description")) {
        Some(s) => s.to_owned(),
        None => {
            marks::update_xmp(None, marks::Mark::default()).replace("\n   xmp:Rating=\"0\"", "")
        }
    };
    let stamp = time::format(fix_time).replace(' ', "T") + "Z";
    let mut s = marks::ensure_ns(base, "exif", NS_EXIF);
    s = marks::set_xmp_value(
        &s,
        "exif:GPSLatitude",
        Some(&xmp_coordinate_text(lat, true)),
    );
    s = marks::set_xmp_value(
        &s,
        "exif:GPSLongitude",
        Some(&xmp_coordinate_text(lon, false)),
    );
    s = marks::set_xmp_value(&s, "exif:GPSVersionID", Some("2.2.0.0"));
    s = marks::set_xmp_value(&s, "exif:GPSMapDatum", Some("WGS-84"));
    s = marks::set_xmp_value(&s, "exif:GPSTimeStamp", Some(&stamp));
    if let Some(e) = ele.filter(|e| e.is_finite()) {
        let cm = (e.abs() * 100.0).round() as i64;
        s = marks::set_xmp_value(&s, "exif:GPSAltitude", Some(&format!("{cm}/100")));
        s = marks::set_xmp_value(
            &s,
            "exif:GPSAltitudeRef",
            Some(if e < 0.0 { "1" } else { "0" }),
        );
    }
    s
}

/// One photo in the geotagging preview.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Planned {
    pub path: PathBuf,
    /// The sidecar it would write, and whether that file exists already.
    pub sidecar: (PathBuf, bool),
    pub taken: Option<f64>,
    pub outcome: Outcome,
}

/// What geotagging does with a photo.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Outcome {
    /// Writes this position (and altitude), from the fix at this time.
    Write {
        lat: f64,
        lon: f64,
        ele: Option<f64>,
        fix: f64,
    },
    /// Left alone: it has a position already.
    HasPosition,
    /// Left alone: no capture time.
    NoTime,
    /// Left alone: taken this many seconds before or after the track.
    Outside(f64),
    /// Left alone: in a gap of the track this many seconds from a fix.
    Gap(f64),
}

/// The photos beside a track (the folder's images, sidecars and the track itself left out),
/// sorted by name.
pub(super) fn folder_photos(dir: &Path) -> Vec<PathBuf> {
    const PHOTO: [&str; 12] = [
        "jpg", "jpeg", "jpe", "heic", "heif", "avif", "tif", "tiff", "png", "webp", "jxl", "dng",
    ];
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut v: Vec<PathBuf> = rd
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .filter(|p| {
            p.extension().and_then(|e| e.to_str()).is_some_and(|e| {
                let e = e.to_ascii_lowercase();
                PHOTO.contains(&e.as_str()) || marks::is_raw_ext(&e)
            })
        })
        .filter(|p| {
            !p.file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with('.'))
        })
        .collect();
    v.sort();
    v.truncate(MAX_PHOTOS);
    v
}

/// The facts of the photos in `dir`, with their sidecars (read off the UI thread).
pub(super) fn scan(dir: &Path) -> Vec<(PhotoFacts, (PathBuf, bool))> {
    let sc = marks::Sidecars::scan(dir);
    folder_photos(dir)
        .into_iter()
        .map(|p| {
            // Positions go into Specular's own sidecar, never another app's (when other apps'
            // take every form, writing says so).
            let side = sc.resolve_writable(&p).unwrap_or_else(|| sc.resolve(&p));
            let f = photo_facts(&p, side.1.then_some(side.0.as_path()));
            (f, side)
        })
        .collect()
}

/// What geotagging would do for each photo at camera clock `offset` (camera ahead of GPS
/// by this many seconds).
pub(super) fn plan(
    track: &Timeline,
    photos: &[(PhotoFacts, (PathBuf, bool))],
    offset: f64,
) -> Vec<Planned> {
    photos
        .iter()
        .map(|(f, side)| {
            let taken = f.taken.map(|t| t.secs);
            let outcome = if f.position.is_some() {
                Outcome::HasPosition
            } else if let Some(t) = taken {
                match place(track, &[t], offset, MAX_GAP)[0] {
                    Placement::On { lat, lon, .. } => {
                        let at = t - offset;
                        let ele = track.nearest(at).and_then(|i| track.fixes[i].ele);
                        let fix = track.nearest(at).map_or(at, |i| track.fixes[i].t);
                        Outcome::Write { lat, lon, ele, fix }
                    }
                    Placement::Outside(d) => Outcome::Outside(d),
                    Placement::Gap(g) => Outcome::Gap(g),
                }
            } else {
                Outcome::NoTime
            };
            Planned {
                path: f.path.clone(),
                sidecar: side.clone(),
                taken,
                outcome,
            }
        })
        .collect()
}

/// A guess at the camera clock's offset: the whole hours that place the most photos (the
/// usual cause is a camera set to local time with a track in UTC), from −14 h to +14 h.
pub(super) fn guess_offset(track: &Timeline, photos: &[(PhotoFacts, (PathBuf, bool))]) -> f64 {
    let times: Vec<f64> = photos
        .iter()
        .filter(|(f, _)| !f.taken.is_some_and(|t| t.zoned))
        .filter_map(|(f, _)| f.taken.map(|t| t.secs))
        .collect();
    if times.is_empty() {
        return 0.0;
    }
    let mut best = (0usize, 0.0f64);
    for h in -14..=14 {
        let off = f64::from(h) * 3600.0;
        let n = place(track, &times, off, MAX_GAP)
            .iter()
            .filter(|p| matches!(p, Placement::On { .. }))
            .count();
        if n > best.0 || (n == best.0 && off.abs() < best.1.abs()) {
            best = (n, off);
        }
    }
    best.1
}

/// The preview line of a planned photo.
pub(super) fn outcome_text(p: &Planned) -> String {
    match &p.outcome {
        Outcome::Write { lat, lon, .. } => {
            t!(
                "map-geotag-will-write",
                position = format!("{lat:.6}, {lon:.6}")
            )
        }
        Outcome::HasPosition => t!("map-geotag-has-position"),
        Outcome::NoTime => t!("map-geotag-no-time"),
        Outcome::Outside(d) => t!(
            "map-geotag-outside",
            offset = specular_i18n::isolate_ltr(&time::format_offset(*d))
        ),
        Outcome::Gap(g) => t!("map-geotag-gap", seconds = specular_i18n::fixed(*g, 0)),
    }
}

/// Writes the planned positions into the photos' sidecars. Only `.xmp` files are written,
/// each whole or not at all; never an image. Returns how many were written and the
/// failures.
pub(super) fn write(plan: &[Planned]) -> (usize, Vec<(PathBuf, String)>) {
    let (mut n, mut failed) = (0, Vec::new());
    for p in plan {
        let Outcome::Write { lat, lon, ele, fix } = p.outcome else {
            continue;
        };
        let (path, exists) = &p.sidecar;
        let is_xmp = |q: &Path| q.extension().is_some_and(|e| e.eq_ignore_ascii_case("xmp"));
        if !is_xmp(path) || is_xmp(&p.path) || *path == p.path {
            failed.push((p.path.clone(), t!("map-geotag-not-sidecar")));
            continue;
        }
        let existing = if *exists {
            match std::fs::read_to_string(path) {
                Ok(s) => Some(s),
                Err(e) => {
                    failed.push((p.path.clone(), e.to_string()));
                    continue;
                }
            }
        } else {
            None
        };
        // A position written since the preview: leave it.
        if existing.as_deref().and_then(sidecar_position).is_some() {
            continue;
        }
        // Another app's sidecar is never written.
        if existing
            .as_deref()
            .is_some_and(|t| specular_metadata::sidecars::owner(t).is_some())
        {
            failed.push((p.path.clone(), t!("sidecar-other-apps-only")));
            continue;
        }
        let text = geotag_xmp(existing.as_deref(), lat, lon, ele, fix);
        match marks::write_atomic(path, text.as_bytes()) {
            Ok(()) => n += 1,
            Err(e) => failed.push((p.path.clone(), e.to_string())),
        }
    }
    (n, failed)
}

/// The photos beside a track with their sidecars ([`scan`]).
pub(super) type Scanned = Arc<Vec<(PhotoFacts, (PathBuf, bool))>>;

/// Photos placed on the map: (path, lat, lon, direction).
pub(super) type Placed = Arc<Vec<(PathBuf, f64, f64, Option<f64>)>>;

/// The planned positions as pins, with the photos' directions.
pub(super) fn pins(plan: &[Planned], photos: &[(PhotoFacts, (PathBuf, bool))]) -> Placed {
    Arc::new(
        plan.iter()
            .zip(photos)
            .filter_map(|(p, (f, _))| match p.outcome {
                Outcome::Write { lat, lon, .. } => Some((p.path.clone(), lat, lon, f.direction)),
                Outcome::HasPosition => {
                    f.position.map(|(a, b)| (p.path.clone(), a, b, f.direction))
                }
                _ => None,
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const GPX: &str = r#"<?xml version="1.0"?>
<gpx version="1.1" xmlns="http://www.topografix.com/GPX/1/1"><trk><trkseg>
<trkpt lat="47.0" lon="8.0"><ele>400</ele><time>2024-06-03T08:00:00Z</time></trkpt>
<trkpt lat="47.01" lon="8.0"><ele>420</ele><time>2024-06-03T08:10:00Z</time></trkpt>
<trkpt lat="47.02" lon="8.0"><time>2024-06-03T08:20:00Z</time></trkpt>
<trkpt lat="47.03" lon="8.0"/>
</trkseg></trk></gpx>"#;

    const TCX: &str = r#"<TrainingCenterDatabase xmlns="http://www.garmin.com/xmlschemas/TrainingCenterDatabase/v2"><Activities><Activity><Lap><Track>
<Trackpoint><Time>2024-06-03T08:00:00Z</Time><Position><LatitudeDegrees>46.5</LatitudeDegrees><LongitudeDegrees>7.5</LongitudeDegrees></Position><AltitudeMeters>1200</AltitudeMeters></Trackpoint>
<Trackpoint><Time>2024-06-03T08:01:00Z</Time><Position><LatitudeDegrees>46.501</LatitudeDegrees><LongitudeDegrees>7.5</LongitudeDegrees></Position></Trackpoint>
</Track></Lap></Activity></Activities></TrainingCenterDatabase>"#;

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("specular-geotag-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn tracks_with_times() {
        let t = timeline(GPX.as_bytes()).unwrap();
        assert_eq!(t.fixes.len(), 3);
        assert_eq!(t.fixes[0].ele, Some(400.0));
        let t = timeline(TCX.as_bytes()).unwrap();
        assert_eq!(t.fixes.len(), 2);
        assert_eq!(t.fixes[0].ele, Some(1200.0));
        assert!(
            timeline(b"<gpx><trk><trkseg><trkpt lat='1' lon='2'/></trkseg></trk></gpx>").is_none()
        );
        // An IGC flight log with its date; an NMEA log with RMC dates (one without: none).
        let igc = "AXXX001\r\nHFDTE030624\r\nB0800004700000N00800000EA0040000410\r\nB0800104700100N00800100EA0040500415\r\n";
        let t = timeline(igc.as_bytes()).unwrap();
        assert_eq!(t.fixes.len(), 2);
        assert_eq!(t.fixes[1].t - t.fixes[0].t, 10.0);
        assert_eq!(t.fixes[0].ele, Some(410.0));
        let sum = |b: &str| format!("${b}*{:02X}\r\n", b.bytes().fold(0u8, |a, c| a ^ c));
        let rmc = [
            "GPRMC,080000,A,4700.000,N,00800.000,E,1.0,0.0,030624,,",
            "GPRMC,080005,A,4700.010,N,00800.010,E,1.0,0.0,030624,,",
        ]
        .map(sum)
        .concat();
        assert_eq!(timeline(rmc.as_bytes()).unwrap().fixes.len(), 2);
        let gga = [
            "GPGGA,080000,4700.000,N,00800.000,E,1,08,0.9,400,M,0,M,,",
            "GPGGA,080005,4700.010,N,00800.010,E,1,08,0.9,401,M,0,M,,",
        ]
        .map(sum)
        .concat();
        assert!(timeline(gga.as_bytes()).is_none());
    }

    /// A track nested 100 000 deep is refused before roxmltree recurses through it.
    #[test]
    fn deeply_nested_tracks_are_refused() {
        let deep = format!(
            "<gpx>{}{}</gpx>",
            "<a>".repeat(100_000),
            "</a>".repeat(100_000)
        );
        assert!(specular_thread::on_stack(256 << 10, || timeline(deep.as_bytes())).is_none());
    }

    #[test]
    fn xmp_coordinates_round_trip() {
        for v in [47.376887f64, -122.419416, 0.0, -0.000001, 89.9999999] {
            let lat = v.abs() <= 90.0;
            let s = xmp_coordinate_text(v, lat);
            let back = xmp_coordinate(&s).unwrap();
            assert!((back - v).abs() < 1e-7, "{v} {s} {back}");
        }
        assert_eq!(
            xmp_coordinate("47,22,36N"),
            Some(47.0 + 22.0 / 60.0 + 36.0 / 3600.0)
        );
        assert_eq!(xmp_coordinate("8,30W"), Some(-8.5));
        assert_eq!(xmp_coordinate("8.5"), None);
    }

    #[test]
    fn geotag_xmp_keeps_the_rest() {
        let fresh = geotag_xmp(None, 47.5, -8.25, Some(412.3), 1_717_401_600.0);
        assert!(
            fresh.contains("exif:GPSLatitude=\"47,30.000000N\""),
            "{fresh}"
        );
        assert!(
            fresh.contains("exif:GPSLongitude=\"8,15.000000W\""),
            "{fresh}"
        );
        assert!(fresh.contains("exif:GPSAltitude=\"41230/100\""), "{fresh}");
        assert!(
            fresh.contains("exif:GPSTimeStamp=\"2024-06-03T08:00:00Z\""),
            "{fresh}"
        );
        assert!(
            fresh.contains("xmlns:exif=\"http://ns.adobe.com/exif/1.0/\""),
            "{fresh}"
        );
        assert_eq!(sidecar_position(&fresh), Some((47.5, -8.25)));
        let rated = marks::update_xmp(
            None,
            marks::Mark {
                rating: 4,
                label: 0,
                flag: 0,
            },
        );
        let both = geotag_xmp(Some(&rated), 1.0, 2.0, None, 0.0);
        assert!(both.contains("xmp:Rating=\"4\""), "{both}");
        assert_eq!(sidecar_position(&both), Some((1.0, 2.0)));
        assert_eq!(marks::parse_xmp(&both).rating, 4);
    }

    #[test]
    fn geotagging_writes_sidecars_only() {
        let dir = scratch("write");
        let track = timeline(GPX.as_bytes()).unwrap();
        // Two photos without EXIF (no times), one placed by hand below.
        let img = dir.join("a.jpg");
        std::fs::write(&img, b"\xff\xd8\xff\xd9").unwrap();
        std::fs::write(dir.join("b.jpg"), b"\xff\xd8\xff\xd9").unwrap();
        std::fs::write(dir.join("walk.gpx"), GPX).unwrap();
        let photos = scan(&dir);
        assert_eq!(photos.len(), 2);
        let mut p = plan(&track, &photos, 0.0);
        assert!(p.iter().all(|x| x.outcome == Outcome::NoTime));
        // The first was taken at 08:05 by a camera an hour fast.
        let taken = time::parse("2024-06-03T09:05:00").unwrap();
        let mut ph = photos.clone();
        ph[0].0.taken = Some(taken);
        assert_eq!(guess_offset(&track, &ph), 3600.0);
        p = plan(&track, &ph, 3600.0);
        let Outcome::Write { lat, .. } = p[0].outcome else {
            panic!("{:?}", p[0].outcome)
        };
        assert!((lat - 47.005).abs() < 1e-6, "{lat}");
        let before = std::fs::read(&img).unwrap();
        let (n, failed) = write(&p);
        assert_eq!((n, failed.len()), (1, 0));
        assert_eq!(
            std::fs::read(&img).unwrap(),
            before,
            "the image is untouched"
        );
        let side = std::fs::read_to_string(dir.join("a.xmp")).unwrap();
        assert!(sidecar_position(&side).is_some());
        assert!(!dir.join("b.xmp").exists());
        // Again: the photo has a position now and is left alone.
        let again = plan(&track, &scan(&dir), 3600.0);
        assert_eq!(again[0].outcome, Outcome::HasPosition);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn exif_extras_read_offset_and_direction() {
        // A little-endian TIFF: IFD0 with ExifIFD and GPS pointers; OffsetTimeOriginal
        // "+02:00", SubSecTimeOriginal "25", GPSImgDirection 123.5.
        let mut b = Vec::new();
        b.extend_from_slice(b"II*\0");
        b.extend_from_slice(&8u32.to_le_bytes());
        // IFD0 at 8: two entries.
        let ifd0 = |b: &mut Vec<u8>, exif: u32, gps: u32| {
            b.extend_from_slice(&2u16.to_le_bytes());
            for (tag, v) in [(0x8769u16, exif), (0x8825u16, gps)] {
                b.extend_from_slice(&tag.to_le_bytes());
                b.extend_from_slice(&4u16.to_le_bytes());
                b.extend_from_slice(&1u32.to_le_bytes());
                b.extend_from_slice(&v.to_le_bytes());
            }
            b.extend_from_slice(&0u32.to_le_bytes());
        };
        // IFD0: 2 + 24 + 4 = 30 bytes → Exif IFD at 38.
        let exif_at = 38u32;
        // Exif IFD: 2 entries (2 + 24 + 4 = 30) → value of OffsetTime at 68 (7 bytes), GPS at 76.
        let gps_at = 76u32;
        ifd0(&mut b, exif_at, gps_at);
        b.extend_from_slice(&2u16.to_le_bytes());
        b.extend_from_slice(&0x9011u16.to_le_bytes());
        b.extend_from_slice(&2u16.to_le_bytes());
        b.extend_from_slice(&7u32.to_le_bytes());
        b.extend_from_slice(&68u32.to_le_bytes());
        b.extend_from_slice(&0x9291u16.to_le_bytes());
        b.extend_from_slice(&2u16.to_le_bytes());
        b.extend_from_slice(&3u32.to_le_bytes());
        b.extend_from_slice(b"25\0\0");
        b.extend_from_slice(&0u32.to_le_bytes());
        assert_eq!(b.len(), 68);
        b.extend_from_slice(b"+02:00\0\0");
        assert_eq!(b.len(), 76);
        // GPS IFD: one entry (2 + 12 + 4 = 18) → the rational at 94.
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&0x0011u16.to_le_bytes());
        b.extend_from_slice(&5u16.to_le_bytes());
        b.extend_from_slice(&1u32.to_le_bytes());
        b.extend_from_slice(&94u32.to_le_bytes());
        b.extend_from_slice(&0u32.to_le_bytes());
        b.extend_from_slice(&247u32.to_le_bytes());
        b.extend_from_slice(&2u32.to_le_bytes());
        let (o, s, d) = exif_extras(&b);
        assert_eq!(o.as_deref(), Some("+02:00"));
        assert_eq!(s.as_deref(), Some("25"));
        assert_eq!(d, Some(123.5));
    }
}
