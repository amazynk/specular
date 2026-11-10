//! Drone photos on the map: the camera's heading and the height above the take-off point,
//! and the photos as KML or KMZ for other map tools.
//!
//! - Heading: DJI XMP `drone-dji:GimbalYawDegree` (where the camera looked), else
//!   `FlightYawDegree` (where the aircraft pointed), else EXIF `GPSImgDirection`. Degrees
//!   clockwise from north.
//! - Height: `drone-dji:RelativeAltitude` (metres above the take-off point).
//! - KML: one placemark a photo with its name, time, heading and height; KMZ adds an arrow
//!   icon turned to the heading and a small JPEG of each photo (320 px), and the full photos
//!   only when asked.

use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf};

use specular_core::CancelToken;
use specular_decode::DecodeRequest;
use specular_i18n::t;

use super::App;

/// Bytes read from a photo's start to find its XMP (DJI writes it in the first segments).
const HEAD: u64 = 512 * 1024;
/// Longest side of the photos in a KMZ (pixels).
const THUMB: usize = 320;
/// The arrow icon's side (pixels).
const ARROW: u32 = 32;

/// Where a heading comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app) enum HeadingFrom {
    Gimbal,
    Flight,
    Exif,
}

/// What the map shows of a photo beyond its position.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(in crate::app) struct DroneFacts {
    /// Degrees clockwise from north, 0..360, and where from.
    pub heading: Option<(f64, HeadingFrom)>,
    /// Metres above the take-off point.
    pub relative_altitude: Option<f64>,
}

/// A number in XMP, as an attribute (`drone-dji:Key="+12.5"`) or an element
/// (`<drone-dji:Key>12.5</drone-dji:Key>`).
fn xmp_number(xmp: &str, key: &str) -> Option<f64> {
    let parse = |s: &str| {
        s.trim()
            .trim_start_matches('+')
            .parse::<f64>()
            .ok()
            .filter(|v| v.is_finite())
    };
    let attr = format!("{key}=\"");
    if let Some(i) = xmp.find(&attr) {
        let rest = &xmp[i + attr.len()..];
        return parse(&rest[..rest.find('"')?]);
    }
    let open = format!("<{key}>");
    let i = xmp.find(&open)?;
    let rest = &xmp[i + open.len()..];
    parse(&rest[..rest.find('<')?])
}

/// The heading and height of a photo from its XMP, falling back to `exif_direction`.
pub(in crate::app) fn facts_of(xmp: Option<&str>, exif_direction: Option<f64>) -> DroneFacts {
    let norm = |d: f64| d.rem_euclid(360.0);
    let x = |k: &str| xmp.and_then(|x| xmp_number(x, &format!("drone-dji:{k}")));
    let heading = x("GimbalYawDegree")
        .map(|d| (norm(d), HeadingFrom::Gimbal))
        .or_else(|| x("FlightYawDegree").map(|d| (norm(d), HeadingFrom::Flight)))
        .or_else(|| exif_direction.map(|d| (norm(d), HeadingFrom::Exif)));
    DroneFacts {
        heading,
        relative_altitude: x("RelativeAltitude").filter(|h| h.abs() < 20_000.0),
    }
}

/// The XMP packet of a JPEG: the APP1 segment signed `http://ns.adobe.com/xap/1.0/`,
/// walking the segments before the image data.
fn jpeg_xmp(b: &[u8]) -> Option<&[u8]> {
    const SIG: &[u8] = b"http://ns.adobe.com/xap/1.0/\0";
    if !b.starts_with(&[0xFF, 0xD8]) {
        return None;
    }
    let mut i = 2;
    while i + 4 <= b.len() {
        if b[i] != 0xFF {
            return None;
        }
        let m = b[i + 1];
        if m == 0xFF {
            i += 1;
            continue;
        }
        if m == 0xDA || m == 0xD9 {
            return None;
        }
        let len = usize::from(u16::from_be_bytes([b[i + 2], b[i + 3]]));
        let seg = b.get(i + 4..i + 2 + len)?;
        if m == 0xE1 && seg.starts_with(SIG) {
            return Some(&seg[SIG.len()..]);
        }
        i += 2 + len;
    }
    None
}

/// Reads a photo's heading and height (its EXIF and the XMP near its start).
pub(in crate::app) fn read(path: &Path) -> DroneFacts {
    let exif = crate::app::map::photos::direction_of(path);
    let mut head = Vec::new();
    let xmp = std::fs::File::open(path)
        .and_then(|f| f.take(HEAD).read_to_end(&mut head))
        .ok()
        .and_then(|_| match jpeg_xmp(&head) {
            Some(x) => Some(String::from_utf8_lossy(x).into_owned()),
            None => specular_decode::embedded_metadata(&head)
                .xmp
                .map(|x| String::from_utf8_lossy(&x).into_owned()),
        });
    facts_of(xmp.as_deref(), exif)
}

// ----- KML -------------------------------------------------------------------------------

/// One photo in a KML.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::app) struct Placemark {
    pub path: PathBuf,
    pub lat: f64,
    pub lon: f64,
    pub facts: DroneFacts,
    /// ISO 8601 time taken, when known.
    pub when: Option<String>,
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// What a KMZ carries besides the KML.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app) enum Pictures {
    /// None (a plain KML): the placemarks link the photos where they are.
    Links,
    /// The arrow icon and a small JPEG of each photo.
    Thumbnails,
    /// Also the full photos.
    Full,
}

/// The name a photo has inside a KMZ (`files/3-DJI_0003.jpg`): numbered, so two photos with
/// one name stay apart.
fn inner(i: usize, p: &Path, ext: &str) -> String {
    let stem = p
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let safe: String = stem
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || "-_.".contains(c) {
                c
            } else {
                '_'
            }
        })
        .collect();
    format!("files/{}-{safe}.{ext}", i + 1)
}

fn file_url(p: &Path) -> String {
    let s = p.to_string_lossy().replace('\\', "/");
    let s = if s.starts_with('/') {
        s
    } else {
        format!("/{s}")
    };
    let enc: String = s
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"/-_.~:".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect();
    format!("file://{enc}")
}

/// The KML document. With `pics` other than [`Pictures::Links`], images are referred to by
/// their names inside the KMZ.
pub(in crate::app) fn kml(title: &str, marks: &[Placemark], pics: Pictures) -> String {
    use std::fmt::Write as _;
    let mut s = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    s.push_str("<kml xmlns=\"http://www.opengis.net/kml/2.2\">\n<Document>\n");
    let _ = writeln!(s, "<name>{}</name>", esc(title));
    for (i, m) in marks.iter().enumerate() {
        let name = m
            .path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        s.push_str("<Placemark>\n");
        let _ = writeln!(s, "<name>{}</name>", esc(&name));
        if let Some(w) = &m.when {
            let _ = writeln!(s, "<TimeStamp><when>{}</when></TimeStamp>", esc(w));
        }
        let mut desc = String::new();
        let img = match pics {
            Pictures::Links => file_url(&m.path),
            _ => inner(i, &m.path, "jpg"),
        };
        let _ = write!(desc, "<img src=\"{}\" width=\"{THUMB}\"/>", esc(&img));
        if pics == Pictures::Full {
            let ext = m
                .path
                .extension()
                .map(|e| e.to_string_lossy().to_ascii_lowercase())
                .unwrap_or_else(|| "jpg".to_owned());
            let _ = write!(
                desc,
                "<br/><a href=\"{}\">{}</a>",
                esc(&inner(i, &m.path, &format!("full.{ext}"))),
                esc(&name)
            );
        }
        if let Some((h, _)) = m.facts.heading {
            let _ = write!(
                desc,
                "<br/>{}",
                esc(&t!("map-kml-heading", degrees = specular_i18n::fixed(h, 1)))
            );
        }
        if let Some(a) = m.facts.relative_altitude {
            let _ = write!(
                desc,
                "<br/>{}",
                esc(&t!("map-kml-height", metres = specular_i18n::fixed(a, 1)))
            );
        }
        let _ = writeln!(
            s,
            "<description><![CDATA[{}]]></description>",
            desc.replace("]]>", "]]&gt;")
        );
        if let Some((h, _)) = m.facts.heading {
            s.push_str("<Style><IconStyle>");
            let _ = write!(s, "<heading>{h:.1}</heading>");
            if pics != Pictures::Links {
                s.push_str("<Icon><href>files/arrow.png</href></Icon>");
            }
            s.push_str("</IconStyle></Style>\n");
        }
        s.push_str("<ExtendedData>");
        if let Some((h, from)) = m.facts.heading {
            let from = match from {
                HeadingFrom::Gimbal => "gimbal",
                HeadingFrom::Flight => "flight",
                HeadingFrom::Exif => "exif",
            };
            let _ = write!(
                s,
                "<Data name=\"heading\"><value>{h:.2}</value></Data><Data name=\"heading_from\"><value>{from}</value></Data>"
            );
        }
        if let Some(a) = m.facts.relative_altitude {
            let _ = write!(
                s,
                "<Data name=\"relative_altitude_m\"><value>{a:.2}</value></Data>"
            );
        }
        s.push_str("</ExtendedData>\n");
        match m.facts.relative_altitude {
            Some(a) => {
                let _ = writeln!(
                    s,
                    "<Point><altitudeMode>relativeToGround</altitudeMode><coordinates>{:.8},{:.8},{a:.2}</coordinates></Point>",
                    m.lon, m.lat
                );
            }
            None => {
                let _ = writeln!(
                    s,
                    "<Point><coordinates>{:.8},{:.8}</coordinates></Point>",
                    m.lon, m.lat
                );
            }
        }
        s.push_str("</Placemark>\n");
    }
    s.push_str("</Document>\n</kml>\n");
    s
}

/// An arrow pointing up (north before the heading turns it), white with a dark edge.
fn arrow_png() -> Vec<u8> {
    let n = ARROW as usize;
    let mut px = vec![0u8; n * n * 4];
    let c = n as f32 / 2.0;
    for y in 0..n {
        for x in 0..n {
            let (fx, fy) = (x as f32 + 0.5 - c, y as f32 + 0.5);
            // A triangle from the top centre to the bottom corners, notched at the bottom.
            let half = fy * 0.45;
            let inside = fx.abs() <= half && fy >= 2.0 && fy <= n as f32 - 2.0 - fx.abs() * 0.5;
            let edge = fx.abs() <= half + 1.5 && fy >= 0.5 && fy <= n as f32 - 0.5 - fx.abs() * 0.5;
            let i = (y * n + x) * 4;
            if inside {
                px[i..i + 4].copy_from_slice(&[255, 255, 255, 255]);
            } else if edge {
                px[i..i + 4].copy_from_slice(&[20, 20, 20, 230]);
            }
        }
    }
    specular_png::encode(ARROW, ARROW, specular_png::ColorType::Rgba, &px).unwrap_or_default()
}

/// A small JPEG of a photo (its longest side [`THUMB`] pixels), upright.
pub(in crate::app) fn thumbnail(path: &Path) -> Option<Vec<u8>> {
    let reg = specular_decode::Registry::new();
    let s = reg.open_path(path).ok()?;
    let info = s.info();
    let long = info.width.max(info.height) as usize;
    let scale = [8u32, 4, 2, 1]
        .into_iter()
        .find(|k| long / *k as usize >= THUMB)
        .unwrap_or(1);
    let r = s
        .decode(&DecodeRequest::full(scale), &CancelToken::new())
        .ok()?;
    let (w, h, rgba) = crate::clipboard::oriented_rgba(&r, info.orientation);
    let f = (THUMB as f64 / w.max(h) as f64).min(1.0);
    let (tw, th) = (
        ((w as f64 * f).round() as usize).max(1),
        ((h as f64 * f).round() as usize).max(1),
    );
    // Box filter: each output pixel the mean of the source pixels it covers.
    let mut out = vec![0u8; tw * th * 4];
    for y in 0..th {
        let (y0, y1) = (y * h / th, ((y + 1) * h / th).max(y * h / th + 1).min(h));
        for x in 0..tw {
            let (x0, x1) = (x * w / tw, ((x + 1) * w / tw).max(x * w / tw + 1).min(w));
            let mut acc = [0u32; 3];
            let n = ((y1 - y0) * (x1 - x0)) as u32;
            for sy in y0..y1 {
                for sx in x0..x1 {
                    let i = (sy * w + sx) * 4;
                    for c in 0..3 {
                        acc[c] += u32::from(rgba[i + c]);
                    }
                }
            }
            let o = (y * tw + x) * 4;
            for c in 0..3 {
                out[o + c] = (acc[c] / n.max(1)) as u8;
            }
            out[o + 3] = 255;
        }
    }
    Some(specular_thumbs::codec::encode_jpeg(&out, tw, th, 82))
}

/// Writes a KMZ: `doc.kml`, the arrow, the thumbnails (and with [`Pictures::Full`] the
/// photos as they are). A photo whose thumbnail fails is still placed. Returns how many
/// thumbnails are missing.
pub(in crate::app) fn write_kmz<W: Write + Seek>(
    out: W,
    title: &str,
    marks: &[Placemark],
    pics: Pictures,
    thumb: impl Fn(&Path) -> Option<Vec<u8>>,
) -> std::io::Result<usize> {
    let io = |e: zip::result::ZipError| std::io::Error::other(e.to_string());
    let mut z = zip::ZipWriter::new(out);
    let deflate = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    let stored =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    // doc.kml first: readers take the first .kml in the archive.
    z.start_file("doc.kml", deflate).map_err(io)?;
    z.write_all(kml(title, marks, pics).as_bytes())?;
    z.start_file("files/arrow.png", stored).map_err(io)?;
    z.write_all(&arrow_png())?;
    let mut missing = 0;
    for (i, m) in marks.iter().enumerate() {
        match thumb(&m.path) {
            Some(j) => {
                z.start_file(inner(i, &m.path, "jpg"), stored).map_err(io)?;
                z.write_all(&j)?;
            }
            None => missing += 1,
        }
        if pics == Pictures::Full {
            let ext = m
                .path
                .extension()
                .map(|e| e.to_string_lossy().to_ascii_lowercase())
                .unwrap_or_else(|| "jpg".to_owned());
            z.start_file(inner(i, &m.path, &format!("full.{ext}")), stored)
                .map_err(io)?;
            std::io::copy(&mut std::fs::File::open(&m.path)?, &mut z)?;
        }
    }
    z.finish().map_err(io)?;
    Ok(missing)
}

// ----- In the gallery's map ----------------------------------------------------------------

impl App {
    /// Saves the selected photos on the map (all of them when none is selected) as KMZ or
    /// KML, chosen by the file name; `full` puts the photos themselves in a KMZ.
    pub(in crate::app) fn map_export_kml(&mut self, full: bool) {
        let ga = &self.gallery;
        let on_map = ga.map.photo_positions();
        let chosen: Vec<_> = if on_map.iter().any(|(p, _)| ga.selected.contains(p)) {
            on_map
                .into_iter()
                .filter(|(p, _)| ga.selected.contains(p))
                .collect()
        } else {
            on_map
        };
        let now = self.now();
        if chosen.is_empty() {
            self.chrome.toast(t!("map-kml-nothing"), None, now);
            return;
        }
        let dir = chosen
            .first()
            .and_then(|(p, _)| p.parent())
            .map(Path::to_path_buf);
        let title = dir
            .as_ref()
            .and_then(|d| d.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "photos".to_owned());
        let (tx, rx) = crossbeam_channel::bounded(1);
        self.gallery.map.export_rx = Some(rx);
        self.loader.background("map-kml", move |l| {
            let mut dlg = rfd::FileDialog::new()
                .add_filter("KMZ", &["kmz"])
                .add_filter("KML", &["kml"])
                .set_file_name(format!("{title}.kmz"));
            if let Some(d) = &dir {
                dlg = dlg.set_directory(d);
            }
            let Some(out) = dlg.save_file() else { return };
            let t0 = std::time::Instant::now();
            let marks: Vec<Placemark> = chosen
                .iter()
                .map(|(p, (lat, lon))| {
                    let when = std::fs::File::open(p)
                        .ok()
                        .and_then(|f| specular_decode::read_exif_tiff(std::io::BufReader::new(f)))
                        .and_then(|t| specular_decode::exif_date_taken(&t))
                        .and_then(|d| exif_iso(&d));
                    Placemark {
                        path: p.clone(),
                        lat: *lat,
                        lon: *lon,
                        facts: read(p),
                        when,
                    }
                })
                .collect();
            let kmz = out
                .extension()
                .is_none_or(|e| !e.eq_ignore_ascii_case("kml"));
            let r = if kmz {
                let pics = if full {
                    Pictures::Full
                } else {
                    Pictures::Thumbnails
                };
                std::fs::File::create(&out)
                    .and_then(|f| write_kmz(std::io::BufWriter::new(f), &title, &marks, pics, thumbnail))
                    .map(|_| ())
            } else {
                specular_markup::write_atomic(&out, kml(&title, &marks, Pictures::Links).as_bytes())
            };
            specular_log::debug!(target: specular_log::UI, "map: {} placemarks in {:.0} ms", marks.len(), t0.elapsed().as_secs_f64() * 1e3);
            let msg = match r {
                Ok(()) => t!(
                    "map-kml-saved",
                    count = marks.len() as i64,
                    name = out
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default()
                ),
                Err(e) => t!("map-kml-failed", reason = crate::error_text::io_reason(&e)),
            };
            let _ = tx.send(msg);
            l.waker()();
        });
    }
}

/// A photo's placemark from its own EXIF (position, time) and drone facts; `None` without a
/// position.
pub(in crate::app) fn placemark(path: &Path) -> Option<Placemark> {
    let f = std::fs::File::open(path).ok()?;
    let tiff = specular_decode::read_exif_tiff(std::io::BufReader::new(f))?;
    let (lat, lon) = specular_decode::exif_position(&tiff)?;
    if !lat.is_finite() || !lon.is_finite() || (lat == 0.0 && lon == 0.0) {
        return None;
    }
    Some(Placemark {
        path: path.to_path_buf(),
        lat,
        lon,
        facts: read(path),
        when: specular_decode::exif_date_taken(&tiff).and_then(|d| exif_iso(&d)),
    })
}

/// EXIF `2026:10:25 13:04:05` as ISO 8601 `2026-10-25T13:04:05` (local time, no zone).
fn exif_iso(d: &str) -> Option<String> {
    let d = d.trim();
    let (date, time) = d.split_once(' ')?;
    let date = date.replace(':', "-");
    (date.len() == 10 && time.len() >= 8).then(|| format!("{date}T{}", &time[..8]))
}

#[cfg(test)]
pub(in crate::app) mod tests {
    use super::*;

    /// A JPEG with only metadata: SOI, an Exif APP1 with GPS (and GPSImgDirection when
    /// given), an XMP APP1, EOI. Generated, never a real photo.
    pub(crate) fn fixture(direction: Option<f64>, xmp: Option<&str>) -> Vec<u8> {
        // Little-endian TIFF: IFD0 with the GPS IFD pointer; GPS IFD with lat/lon (+ dir).
        let mut t: Vec<u8> = b"II*\0\x08\0\0\0".to_vec();
        let gps_at = 8 + 2 + 12 + 4;
        t.extend(1u16.to_le_bytes());
        t.extend(0x8825u16.to_le_bytes());
        t.extend(4u16.to_le_bytes());
        t.extend(1u32.to_le_bytes());
        t.extend((gps_at as u32).to_le_bytes());
        t.extend(0u32.to_le_bytes());
        let n: u16 = if direction.is_some() { 6 } else { 4 };
        let data_at = gps_at + 2 + 12 * n as usize + 4;
        let mut data: Vec<u8> = Vec::new();
        let mut entries: Vec<u8> = Vec::new();
        let mut ent = |tag: u16, ty: u16, count: u32, value: [u8; 4]| {
            entries.extend(tag.to_le_bytes());
            entries.extend(ty.to_le_bytes());
            entries.extend(count.to_le_bytes());
            entries.extend(value);
        };
        let rationals = |vals: &[(u32, u32)], data: &mut Vec<u8>| -> [u8; 4] {
            let off = (data_at + data.len()) as u32;
            for (a, b) in vals {
                data.extend(a.to_le_bytes());
                data.extend(b.to_le_bytes());
            }
            off.to_le_bytes()
        };
        ent(1, 2, 2, *b"N\0\0\0");
        let lat = rationals(&[(47, 1), (22, 1), (1230, 100)], &mut data);
        ent(2, 5, 3, lat);
        ent(3, 2, 2, *b"E\0\0\0");
        let lon = rationals(&[(8, 1), (32, 1), (4560, 100)], &mut data);
        ent(4, 5, 3, lon);
        if let Some(d) = direction {
            ent(16, 2, 2, *b"T\0\0\0");
            let v = rationals(&[((d * 100.0) as u32, 100)], &mut data);
            ent(17, 5, 1, v);
        }
        t.extend(n.to_le_bytes());
        t.extend(entries);
        t.extend(0u32.to_le_bytes());
        t.extend(data);
        let mut j = vec![0xFF, 0xD8];
        let mut app1 = b"Exif\0\0".to_vec();
        app1.extend(&t);
        j.extend([0xFF, 0xE1]);
        j.extend(((app1.len() + 2) as u16).to_be_bytes());
        j.extend(app1);
        if let Some(x) = xmp {
            let mut a = b"http://ns.adobe.com/xap/1.0/\0".to_vec();
            a.extend(x.as_bytes());
            j.extend([0xFF, 0xE1]);
            j.extend(((a.len() + 2) as u16).to_be_bytes());
            j.extend(a);
        }
        j.extend([0xFF, 0xD9]);
        j
    }

    const DJI: &str = r#"<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"><rdf:Description rdf:about="DJI Meta Data" xmlns:drone-dji="http://www.dji.com/drone-dji/1.0/" drone-dji:RelativeAltitude="+52.30" drone-dji:GimbalYawDegree="-34.50" drone-dji:FlightYawDegree="+12.00"/></rdf:RDF></x:xmpmeta>"#;

    fn tmp() -> PathBuf {
        let d = std::env::temp_dir().join(format!("specular-drone-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn headings_prefer_the_gimbal() {
        let f = facts_of(Some(DJI), Some(90.0));
        assert_eq!(f.heading, Some((325.5, HeadingFrom::Gimbal)));
        assert_eq!(f.relative_altitude, Some(52.3));
        let flight = DJI.replace(" drone-dji:GimbalYawDegree=\"-34.50\"", "");
        assert_eq!(
            facts_of(Some(&flight), Some(90.0)).heading,
            Some((12.0, HeadingFrom::Flight))
        );
        assert_eq!(
            facts_of(None, Some(90.0)).heading,
            Some((90.0, HeadingFrom::Exif))
        );
        let el = "<drone-dji:GimbalYawDegree>+181.5</drone-dji:GimbalYawDegree>";
        assert_eq!(
            facts_of(Some(el), None).heading,
            Some((181.5, HeadingFrom::Gimbal))
        );
        assert_eq!(facts_of(None, None), DroneFacts::default());
    }

    #[test]
    fn photos_are_read_from_generated_jpegs() {
        let d = tmp();
        let a = d.join("dji.jpg");
        std::fs::write(&a, fixture(Some(90.0), Some(DJI))).unwrap();
        let b = d.join("phone.jpg");
        std::fs::write(&b, fixture(Some(123.5), None)).unwrap();
        let fa = read(&a);
        assert_eq!(fa.heading, Some((325.5, HeadingFrom::Gimbal)));
        assert_eq!(fa.relative_altitude, Some(52.3));
        let fb = read(&b);
        assert_eq!(fb.heading, Some((123.5, HeadingFrom::Exif)));
        assert_eq!(fb.relative_altitude, None);
        let _ = std::fs::remove_dir_all(&d);
    }

    fn marks() -> Vec<Placemark> {
        vec![
            Placemark {
                path: PathBuf::from("/data/flight 1/DJI_0001.JPG"),
                lat: 47.3701,
                lon: 8.5460,
                facts: facts_of(Some(DJI), None),
                when: Some("2026-10-25T13:04:05".into()),
            },
            Placemark {
                path: PathBuf::from("/data/flight 1/a&b.jpg"),
                lat: -33.0,
                lon: 151.0,
                facts: DroneFacts::default(),
                when: None,
            },
        ]
    }

    #[test]
    fn kml_has_placemarks_with_heading() {
        let k = kml("Flight <1>", &marks(), Pictures::Links);
        let doc = roxmltree::Document::parse(&k).unwrap();
        let pm: Vec<_> = doc
            .descendants()
            .filter(|n| n.has_tag_name("Placemark"))
            .collect();
        assert_eq!(pm.len(), 2);
        let text = |n: roxmltree::Node, tag: &str| {
            n.descendants()
                .find(|c| c.has_tag_name(tag))
                .and_then(|c| c.text())
                .map(str::to_owned)
        };
        assert_eq!(text(pm[0], "heading").as_deref(), Some("325.5"));
        assert_eq!(
            text(pm[0], "coordinates").as_deref(),
            Some("8.54600000,47.37010000,52.30")
        );
        assert_eq!(
            text(pm[0], "altitudeMode").as_deref(),
            Some("relativeToGround")
        );
        assert_eq!(text(pm[1], "name").as_deref(), Some("a&b.jpg"));
        assert_eq!(text(pm[1], "heading"), None);
        assert!(k.contains("file:///data/flight%201/DJI_0001.JPG"));
        assert_eq!(
            doc.descendants()
                .find(|n| n.has_tag_name("Document"))
                .and_then(|d| d.children().find(|c| c.has_tag_name("name")))
                .and_then(|n| n.text()),
            Some("Flight <1>")
        );
    }

    #[test]
    fn kmz_holds_the_kml_arrow_and_thumbnails() {
        let mut buf = std::io::Cursor::new(Vec::new());
        let missing = write_kmz(&mut buf, "t", &marks(), Pictures::Thumbnails, |p| {
            (p.file_name()? != "a&b.jpg").then(|| vec![0xFF, 0xD8, 0xFF, 0xD9])
        })
        .unwrap();
        assert_eq!(missing, 1);
        let mut z = zip::ZipArchive::new(std::io::Cursor::new(buf.into_inner())).unwrap();
        let names: Vec<String> = (0..z.len())
            .map(|i| z.by_index(i).unwrap().name().to_owned())
            .collect();
        assert_eq!(
            names,
            ["doc.kml", "files/arrow.png", "files/1-DJI_0001.jpg"]
        );
        let mut kml = String::new();
        z.by_name("doc.kml")
            .unwrap()
            .read_to_string(&mut kml)
            .unwrap();
        assert!(kml.contains("<img src=\"files/1-DJI_0001.jpg\""));
        assert!(kml.contains("<href>files/arrow.png</href>"));
        // No full-size photos unless asked.
        assert!(!kml.contains("full."));
    }

    #[test]
    fn exif_times_become_iso() {
        assert_eq!(
            exif_iso("2026:10:25 13:04:05").as_deref(),
            Some("2026-10-25T13:04:05")
        );
        assert_eq!(exif_iso("junk"), None);
    }
}
