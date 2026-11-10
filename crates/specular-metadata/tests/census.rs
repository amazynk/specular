//! Census of EXIF tags our tables do not name, over real files, read-only.
//!
//! ```sh
//! CENSUS_PATHS=tools/corpus:/path/to/photo.jpg \
//!   cargo test -p specular-metadata --test census -- --ignored --nocapture
//! ```
//!
//! Prints only tag IDs, IFDs, TIFF types and counts, aggregated over the files: never values,
//! file names or anything else from the images (the corpus holds private photos).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use exif::{Context, In, Value};
use specular_metadata::*;

/// Files larger than this are skipped (video, huge rasters).
const MAX_FILE: u64 = 256 << 20;
/// Formats that cannot carry EXIF; skipped without reading.
const NO_EXIF: &[&str] = &[
    "svg", "yuv", "y4m", "bmp", "gif", "tga", "qoi", "dds", "ktx", "ktx2", "ico", "fli", "exr",
    "txt", "md", "sh", "py", "json", "sha256",
];

fn walk(p: &Path, out: &mut Vec<PathBuf>) {
    if p.is_dir() {
        if let Ok(rd) = std::fs::read_dir(p) {
            let mut v: Vec<_> = rd.filter_map(|e| e.ok().map(|e| e.path())).collect();
            v.sort();
            for c in v {
                walk(&c, out);
            }
        }
    } else if p.is_file()
        && !p
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| NO_EXIF.contains(&e.to_ascii_lowercase().as_str()))
    {
        out.push(p.to_owned());
    }
}

fn is_tiff_header(b: &[u8]) -> bool {
    b.starts_with(b"II*\0") || b.starts_with(b"MM\0*")
}

/// The TIFF-structured EXIF payload of a file, by container sniffing.
fn exif_payload(data: &[u8]) -> Option<&[u8]> {
    if data.starts_with(&[0xFF, 0xD8]) {
        return jpeg::scan(data).ok()?.exif;
    }
    if is_tiff_header(data) {
        return Some(data);
    }
    // HEIF/AVIF/WebP/JPEG XL/…: an "Exif\0\0" prefix followed by a TIFF header.
    let mut from = 0;
    while let Some(i) = data[from..].windows(6).position(|w| w == b"Exif\0\0") {
        let p = from + i + 6;
        if is_tiff_header(&data[p..]) {
            return Some(&data[p..]);
        }
        from = p;
    }
    // PNG eXIf and WebP EXIF chunks without the prefix.
    for tag in [&b"eXIf"[..], &b"EXIF"[..]] {
        if let Some(i) = data.windows(4).position(|w| w == tag)
            && is_tiff_header(data.get(i + 4..)?)
        {
            return Some(&data[i + 4..]);
        }
        if let Some(i) = data.windows(4).position(|w| w == tag)
            && is_tiff_header(data.get(i + 8..)?)
        {
            return Some(&data[i + 8..]);
        }
    }
    None
}

fn type_code(v: &Value) -> (u16, usize) {
    match v {
        Value::Byte(b) => (1, b.len()),
        Value::Ascii(a) => (2, a.iter().map(|s| s.len() + 1).sum()),
        Value::Short(x) => (3, x.len()),
        Value::Long(x) => (4, x.len()),
        Value::Rational(x) => (5, x.len()),
        Value::SByte(x) => (6, x.len()),
        Value::Undefined(b, _) => (7, b.len()),
        Value::SShort(x) => (8, x.len()),
        Value::SLong(x) => (9, x.len()),
        Value::SRational(x) => (10, x.len()),
        Value::Float(x) => (11, x.len()),
        Value::Double(x) => (12, x.len()),
        Value::Unknown(t, n, _) => (*t, *n as usize),
    }
}

fn ifd_name(ifd: In, ctx: Context) -> &'static str {
    if ifd != In::PRIMARY {
        return "IFD1";
    }
    match ctx {
        Context::Tiff => "IFD0",
        Context::Exif => "ExifIFD",
        Context::Gps => "GPS",
        Context::Interop => "Interop",
        _ => "?",
    }
}

/// Files the tag is in, and how often each TIFF type was seen.
type Seen = (usize, BTreeMap<u16, usize>);

#[test]
#[ignore]
fn unknown_tag_census() {
    let Ok(paths) = std::env::var("CENSUS_PATHS") else {
        eprintln!("set CENSUS_PATHS to a colon-separated list of files or folders");
        return;
    };
    let mut files = Vec::new();
    for p in paths.split(':').filter(|s| !s.is_empty()) {
        walk(Path::new(p), &mut files);
    }
    let l = Localizer::english();
    // (ifd, tag number) -> (files, types seen)
    let mut unknown: BTreeMap<(&str, u16), Seen> = BTreeMap::new();
    let (mut with_exif, mut scanned) = (0usize, 0usize);
    for f in &files {
        if std::fs::metadata(f).map_or(true, |m| m.len() > MAX_FILE) {
            continue;
        }
        let Ok(data) = std::fs::read(f) else { continue };
        scanned += 1;
        let Some(tiff) = exif_payload(&data) else {
            continue;
        };
        with_exif += 1;
        let input = MetadataInput {
            exif_tiff: Some(tiff),
            ..Default::default()
        };
        let r = read(&input, &l);
        let names: std::collections::HashSet<String> = r
            .all_fields
            .iter()
            .flat_map(|g| &g.fields)
            .map(|f| strip_isolates(&f.tag))
            .filter(|t| t.starts_with("Unknown tag"))
            .collect();
        if names.is_empty() {
            continue;
        }
        let mut reader = exif::Reader::new();
        reader.continue_on_error(true);
        let ex = match reader.read_raw(tiff.to_vec()) {
            Ok(e) => e,
            Err(exif::Error::PartialResult(p)) => p.into_inner().0,
            Err(_) => continue,
        };
        let mut seen = std::collections::HashSet::new();
        for fld in ex.fields() {
            let num = fld.tag.number();
            let ctx = match fld.tag.context() {
                Context::Tiff => "IFD",
                Context::Exif => "Exif",
                Context::Gps => "GPS",
                Context::Interop => "Interop",
                _ => "?",
            };
            let hit = names.contains(&format!("Unknown tag 0x{num:04X} ({ctx})"));
            if !hit {
                continue;
            }
            let key = (ifd_name(fld.ifd_num, fld.tag.context()), num);
            let e = unknown.entry(key).or_default();
            if seen.insert(key) {
                e.0 += 1;
            }
            *e.1.entry(type_code(&fld.value).0).or_default() += 1;
        }
    }
    let mut rows: Vec<_> = unknown.into_iter().collect();
    rows.sort_by(|a, b| b.1.0.cmp(&a.1.0).then(a.0.cmp(&b.0)));
    println!("files read: {scanned}, with EXIF: {with_exif}");
    println!("{:<8} {:<7} {:>6}  types", "IFD", "tag", "files");
    for ((ifd, num), (n, types)) in rows {
        let t: Vec<String> = types.iter().map(|(t, c)| format!("{t}×{c}")).collect();
        println!("{ifd:<8} 0x{num:04X} {n:>6}  {}", t.join(" "));
    }
}
