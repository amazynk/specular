//! Culling marks (stars, colour labels, pick/reject) and per-folder view options, kept in two
//! small tables in the config folder, with ratings and labels mirrored to XMP sidecars that
//! Lightroom, Capture One, darktable and Photo Mechanic read (DESIGN §6.3).
//!
//! The tables are tab-separated text: `path \t rating \t label \t flag` and
//! `path \t layout \t size \t sort \t descending \t group`. They are rewritten whole (atomically)
//! a moment after the last change.

use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};

pub(crate) use super::model::Mark;
use super::model::{LABELS, ViewOpts};

/// Whether an extension is a camera raw file's (for the map's geotagging).
pub(crate) fn is_raw_ext(ext: &str) -> bool {
    super::model::is_raw_ext(ext)
}

#[derive(Default)]
pub(crate) struct Store {
    /// None: nothing is saved (screenshots, benchmarks).
    dir: Option<PathBuf>,
    /// The user's marks (saved in the table).
    pub marks: HashMap<PathBuf, Mark>,
    /// Marks read from sidecars and the Library's catalogue: shown where the user has set
    /// none (in folders that prefer other apps' sidecars, over the user's), never saved.
    pub side: HashMap<PathBuf, Mark>,
    /// Folders (and those below) where other apps' sidecars have the last word.
    pub prefer: Vec<PathBuf>,
    pub views: HashMap<PathBuf, ViewOpts>,
    pub dirty: bool,
    loaded: bool,
}

const MARKS: &str = "gallery-marks.tsv";
const VIEWS: &str = "gallery-views.tsv";

impl Store {
    pub fn new(dir: Option<PathBuf>) -> Store {
        Store {
            dir,
            ..Store::default()
        }
    }

    /// Reads both tables the first time the gallery opens.
    pub fn load(&mut self) {
        if self.loaded {
            return;
        }
        self.loaded = true;
        let Some(dir) = &self.dir else { return };
        if let Ok(s) = std::fs::read_to_string(dir.join(MARKS)) {
            self.marks = parse_marks(&s);
        }
        if let Ok(s) = std::fs::read_to_string(dir.join(VIEWS)) {
            for line in s.lines() {
                let f: Vec<&str> = line.split('\t').collect();
                if let [path, rest @ ..] = f.as_slice()
                    && let Some(v) = ViewOpts::decode(rest)
                {
                    self.views.insert(PathBuf::from(path), v);
                }
            }
        }
    }

    pub fn mark(&self, p: &Path) -> Mark {
        let side = self.side.get(p).filter(|m| !m.is_empty());
        if let Some(s) = side
            && prefers(&self.prefer, p)
        {
            return *s;
        }
        self.marks.get(p).or(side).copied().unwrap_or_default()
    }

    pub fn set_mark(&mut self, p: &Path, m: Mark) {
        // Cleared over a sidecar's mark: kept, so the sidecar's does not show again.
        if m.is_empty() && !self.side.get(p).is_some_and(|s| !s.is_empty()) {
            self.marks.remove(p);
        } else {
            self.marks.insert(p.to_path_buf(), m);
        }
        self.dirty = true;
    }

    /// Marks read from sidecars or the catalogue (the newest read replaces the one before).
    pub fn merge_read(&mut self, marks: impl IntoIterator<Item = (PathBuf, Mark)>) {
        self.side.extend(marks);
    }

    pub fn set_view(&mut self, dir: &Path, v: ViewOpts) {
        self.views.insert(dir.to_path_buf(), v);
        self.dirty = true;
    }

    /// The text of both tables, to write off the event loop.
    pub fn snapshot(&mut self) -> Option<(PathBuf, String, String)> {
        if !self.dirty {
            return None;
        }
        self.dirty = false;
        let dir = self.dir.clone()?;
        let mut marks: Vec<_> = self.marks.iter().collect();
        marks.sort_by(|a, b| a.0.cmp(b.0));
        let mut m = String::new();
        for (p, k) in marks {
            if let Some(s) = p.to_str().filter(|s| !s.contains(['\t', '\n'])) {
                m.push_str(&format!("{s}\t{}\t{}\t{}\n", k.rating, k.label, k.flag));
            }
        }
        let mut views: Vec<_> = self.views.iter().collect();
        views.sort_by(|a, b| a.0.cmp(b.0));
        let mut v = String::new();
        for (p, o) in views {
            if let Some(s) = p.to_str().filter(|s| !s.contains(['\t', '\n'])) {
                v.push_str(&format!("{s}\t{}\n", o.encode()));
            }
        }
        Some((dir, m, v))
    }
}

pub(crate) fn write_tables(dir: &Path, marks: &str, views: &str) -> io::Result<()> {
    std::fs::create_dir_all(dir)?;
    write_atomic(&dir.join(MARKS), marks.as_bytes())?;
    write_atomic(&dir.join(VIEWS), views.as_bytes())
}

fn parse_marks(s: &str) -> HashMap<PathBuf, Mark> {
    let mut out = HashMap::new();
    for line in s.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        if let [p, r, l, fl] = f.as_slice()
            && let (Ok(rating), Ok(label), Ok(flag)) =
                (r.parse::<u8>(), l.parse::<u8>(), fl.parse::<i8>())
        {
            out.insert(
                PathBuf::from(p),
                Mark {
                    rating: rating.min(5),
                    label: label.min(LABELS.len() as u8),
                    flag: flag.clamp(-1, 1),
                },
            );
        }
    }
    out
}

/// Writes `path` whole or not at all: a hidden, unique temporary file in the same folder (so
/// the rename stays on one file system), flushed to disk, then renamed over `path`. An existing
/// file's permissions carry over.
pub(crate) fn write_atomic(path: &Path, data: &[u8]) -> io::Result<()> {
    use std::io::Write;
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    let dir = path
        .parent()
        .ok_or_else(|| io::Error::other("no parent folder"))?;
    let name = path
        .file_name()
        .ok_or_else(|| io::Error::other("no file name"))?
        .to_string_lossy();
    let tmp = dir.join(format!(
        ".{name}.{}-{}.specular-tmp",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    let write = || -> io::Result<()> {
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)?;
        f.write_all(data)?;
        if let Ok(m) = std::fs::metadata(path) {
            f.set_permissions(m.permissions())?;
        }
        f.sync_all()?;
        std::fs::rename(&tmp, path)
    };
    let r = write();
    if r.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    r
}

// ----- XMP sidecars -----------------------------------------------------------------------

/// Whether `p` is in one of `folders` (or below one).
pub(crate) fn prefers(folders: &[PathBuf], p: &Path) -> bool {
    !folders.is_empty() && folders.iter().any(|f| p.starts_with(f))
}

/// The folders of the setting "Prefer other apps' sidecars in" (`;` between folders).
pub(crate) fn prefer_folders(setting: &str) -> Vec<PathBuf> {
    setting
        .split(';')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .collect()
}

/// What one `readdir` tells about a folder's sidecars: the `.xmp` files in it, other apps'
/// sidecars that are not XMP (RawTherapee, Capture One), and which image stems are shared by
/// more than one file (a RAW+JPEG pair).
#[derive(Default)]
pub(crate) struct Sidecars {
    dir: PathBuf,
    /// File names of the sidecars (any case of `.xmp`).
    xmp: std::collections::HashSet<String>,
    /// RawTherapee's `.pp3` names and Capture One's `.cos` paths (`CaptureOne/Settings153/…`).
    others: std::collections::HashSet<String>,
    /// Capture One's settings folders, newest first.
    cos_dirs: Vec<String>,
    /// Stem → (files with it, RAW files with it), sidecars not counted.
    stems: HashMap<String, (u32, u32)>,
    /// Which app wrote each sidecar read so far (None: Specular).
    owners: std::cell::RefCell<HashMap<String, Option<specular_metadata::sidecars::App>>>,
}

impl Sidecars {
    pub fn scan(dir: &Path) -> Sidecars {
        let mut out = Sidecars {
            dir: dir.to_path_buf(),
            ..Sidecars::default()
        };
        let Ok(rd) = std::fs::read_dir(dir) else {
            return out;
        };
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            let p = Path::new(&name);
            let ext = p
                .extension()
                .map(|x| x.to_string_lossy())
                .unwrap_or_default();
            if ext.eq_ignore_ascii_case("xmp") {
                out.xmp.insert(name);
                continue;
            }
            if ext.eq_ignore_ascii_case("pp3") {
                out.others.insert(name);
                continue;
            }
            if name == specular_metadata::sidecars::CAPTURE_ONE_DIR {
                out.cos_dirs = specular_metadata::sidecars::capture_one_dirs(dir);
                for d in &out.cos_dirs {
                    let Ok(rd) = std::fs::read_dir(dir.join(d)) else {
                        continue;
                    };
                    for e in rd.flatten() {
                        let n = e.file_name().to_string_lossy().into_owned();
                        if n.to_ascii_lowercase().ends_with(".cos") {
                            out.others.insert(format!("{d}/{n}"));
                        }
                    }
                }
                continue;
            }
            let Some(stem) = p.file_stem() else { continue };
            let c = out
                .stems
                .entry(stem.to_string_lossy().into_owned())
                .or_default();
            c.0 += 1;
            c.1 += u32::from(super::model::is_raw_ext(&ext));
        }
        out
    }

    /// A sidecar was written at `path` (it exists now, and is Specular's).
    pub fn add(&mut self, path: &Path) {
        if let Some(n) = path.file_name() {
            let n = n.to_string_lossy().into_owned();
            self.owners.borrow_mut().insert(n.clone(), None);
            self.xmp.insert(n);
        }
    }

    /// No sidecar at all in the folder (most folders): nothing to read.
    pub fn is_empty(&self) -> bool {
        self.xmp.is_empty() && self.others.is_empty()
    }

    /// Which app wrote the folder's sidecar `name` (None: Specular, or unreadable).
    fn owner(&self, name: &str) -> Option<specular_metadata::sidecars::App> {
        if let Some(o) = self.owners.borrow().get(name) {
            return *o;
        }
        let o = specular_metadata::sidecars::read_small(&self.dir.join(name))
            .and_then(|b| specular_metadata::sidecars::owner(&String::from_utf8_lossy(&b)));
        self.owners.borrow_mut().insert(name.to_owned(), o);
        o
    }

    /// The XMP sidecar forms of `image` that exist (one name per form, in the order they
    /// count), the form a new sidecar takes, and the other form it may take ("" when the
    /// stem form is another image's).
    fn forms(&self, image: &Path) -> (Vec<String>, String, String) {
        let name = image
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let ext = image
            .extension()
            .map(|x| x.to_string_lossy().into_owned())
            .unwrap_or_default();
        let stem = image
            .file_stem()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let (n, raws) = self.stems.get(&stem).copied().unwrap_or((1, 0));
        let own_stem = n <= 1 || (super::model::is_raw_ext(&ext) && raws == 1);
        let short = [format!("{stem}.xmp"), format!("{stem}.XMP")];
        let long = [format!("{name}.xmp"), format!("{name}.XMP")];
        let mut found: Vec<String> = Vec::new();
        let mut first = |c: &[String; 2]| {
            if let Some(n) = c.iter().find(|c| self.xmp.contains(*c)) {
                found.push(n.clone());
            }
        };
        if own_stem {
            first(&short);
        }
        first(&long);
        let (new, other) = if own_stem {
            (short[0].clone(), long[0].clone())
        } else {
            (long[0].clone(), String::new())
        };
        (found, new, other)
    }

    /// Where Specular writes `image`'s keywords and positions: its own sidecar, or a new one
    /// in a form no other app's sidecar takes. None when other apps' sidecars take every form:
    /// they never get keywords or positions (marks do, [`write_sidecars`]).
    pub fn resolve_writable(&self, image: &Path) -> Option<(PathBuf, bool)> {
        let (path, exists) = self.resolve(image);
        let name = path.file_name()?.to_string_lossy().into_owned();
        if !exists || self.owner(&name).is_none() {
            return Some((path, exists));
        }
        let (found, new, other) = self.forms(image);
        let dir = image.parent().unwrap_or(Path::new(""));
        [new, other]
            .into_iter()
            .filter(|n| !n.is_empty())
            .find(|n| !found.iter().any(|f| f.eq_ignore_ascii_case(n)))
            .map(|n| (dir.join(n), false))
    }

    /// Other apps' sidecars of `image` beside it (not Specular's), in the order they count.
    pub fn foreign(&self, image: &Path) -> Vec<PathBuf> {
        let Some(name) = image.file_name().map(|n| n.to_string_lossy().into_owned()) else {
            return Vec::new();
        };
        let stem = image
            .file_stem()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let ext = image
            .extension()
            .map(|x| x.to_string_lossy().into_owned())
            .unwrap_or_default();
        let (n, raws) = self.stems.get(&stem).copied().unwrap_or((1, 0));
        let own_stem = n <= 1 || (super::model::is_raw_ext(&ext) && raws == 1);
        let has = |n: &str| self.xmp.contains(n) || self.others.contains(n);
        let dir = image.parent().unwrap_or(Path::new(""));
        specular_metadata::sidecars::candidates(&name, own_stem, &self.cos_dirs, &has)
            .into_iter()
            .filter(|c| {
                c.kind() != Some(specular_metadata::sidecars::Kind::Xmp)
                    || self.owner(&c.name).is_some()
            })
            .map(|c| dir.join(c.name))
            .collect()
    }

    /// The sidecar of `image` and whether it exists.
    ///
    /// `IMG_1234.xmp` (Lightroom, Capture One, Photo Mechanic) is the usual one, darktable's
    /// `IMG_1234.CR3.xmp` the other; an existing one is matched in either form and either case
    /// of `.xmp`. When files share a stem (RAW+JPEG), the stem form belongs to the RAW (that is
    /// what Lightroom writes it for) and the others use the long form, so a JPEG's stars never
    /// overwrite the RAW's. A new sidecar takes the stem form otherwise.
    pub fn resolve(&self, image: &Path) -> (PathBuf, bool) {
        let name = image
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let ext = image
            .extension()
            .map(|x| x.to_string_lossy().into_owned())
            .unwrap_or_default();
        let stem = image
            .file_stem()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let (n, raws) = self.stems.get(&stem).copied().unwrap_or((1, 0));
        let own_stem = n <= 1 || (super::model::is_raw_ext(&ext) && raws == 1);
        let short = [format!("{stem}.xmp"), format!("{stem}.XMP")];
        let long = [format!("{name}.xmp"), format!("{name}.XMP")];
        let dir = image.parent().unwrap_or(Path::new(""));
        let candidates: Vec<&String> = if own_stem {
            short.iter().chain(long.iter()).collect()
        } else {
            long.iter().collect()
        };
        let found: Vec<&String> = candidates
            .into_iter()
            .filter(|c| self.xmp.contains(*c))
            .collect();
        // Specular's own first when another app's is there too.
        let pick = match found.as_slice() {
            [] => None,
            [one] => Some(*one),
            all => all
                .iter()
                .find(|c| self.owner(c).is_none())
                .or(all.first())
                .copied(),
        };
        if let Some(c) = pick {
            return (dir.join(c), true);
        }
        let new = if own_stem { &short[0] } else { &long[0] };
        (dir.join(new), false)
    }
}

/// `image`'s marks from its sidecars: Specular's own first, other apps' filling in what it
/// does not say (or the other way round in a folder of `prefer`).
fn sidecar_mark(sc: &Sidecars, image: &Path, prefer: &[PathBuf]) -> Mark {
    use specular_metadata::sidecars as sides;
    let mut own = None;
    let (s, exists) = sc.resolve(image);
    if exists
        && sc
            .owner(&s.file_name().unwrap_or_default().to_string_lossy())
            .is_none()
        && let Ok(text) = std::fs::read_to_string(&s)
    {
        own = Some(parse_xmp(&text));
    }
    let mut other = sides::Values::default();
    for p in sc.foreign(image) {
        let Some(kind) = p
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(sides::Kind::of_name)
        else {
            continue;
        };
        if let Some(b) = sides::read_small(&p) {
            other.fill_from(&sides::read(kind, &b).values);
        }
    }
    let theirs = Mark {
        rating: other.rating.unwrap_or(0),
        label: other.label.unwrap_or(0),
        flag: other.flag.unwrap_or(0),
    };
    let Some(own) = own else { return theirs };
    let (first, second) = if prefers(prefer, image) {
        (theirs, own)
    } else {
        (own, theirs)
    };
    let pick = |a: u8, b: u8| if a == 0 { b } else { a };
    Mark {
        rating: pick(first.rating, second.rating),
        label: pick(first.label, second.label),
        flag: if first.flag == 0 {
            second.flag
        } else {
            first.flag
        },
    }
}

/// The marks in the sidecars of `images` (one `readdir` per folder; files are opened only
/// when a sidecar exists). Stops early when `cancelled` says so.
pub(crate) fn read_sidecars(
    images: impl IntoIterator<Item = PathBuf>,
    prefer: &[PathBuf],
    cancelled: &dyn Fn() -> bool,
) -> Vec<(PathBuf, Mark)> {
    let mut folders: HashMap<PathBuf, Sidecars> = HashMap::new();
    let mut out = Vec::new();
    for p in images {
        if cancelled() {
            break;
        }
        let dir = p.parent().unwrap_or(Path::new("")).to_path_buf();
        let sc = folders.entry(dir).or_insert_with_key(|d| Sidecars::scan(d));
        if sc.is_empty() {
            continue;
        }
        let m = sidecar_mark(sc, &p, prefer);
        if !m.is_empty() {
            out.push((p, m));
        }
    }
    out
}

/// Reads the rating, label and pick/reject flag of an XMP packet (attribute or element form).
/// A reject is `xmp:Rating` -1 (Adobe) or `xmpDM:pick` -1; a pick is `xmpDM:pick` 1.
pub(crate) fn parse_xmp(s: &str) -> Mark {
    let rating = xmp_value(s, "xmp:Rating").and_then(|v| v.trim().parse::<i32>().ok());
    let label = xmp_value(s, "xmp:Label").and_then(|v| {
        LABELS
            .iter()
            .position(|l| l.eq_ignore_ascii_case(v.trim()))
            .map(|i| i as u8 + 1)
    });
    let pick = xmp_value(s, "xmpDM:pick").and_then(|v| v.trim().parse::<i32>().ok());
    Mark {
        rating: rating.map_or(0, |r| r.clamp(0, 5) as u8),
        label: label.unwrap_or(0),
        flag: if rating == Some(-1) || pick == Some(-1) {
            -1
        } else if pick == Some(1) {
            1
        } else {
            0
        },
    }
}

/// Where `name` is set in `s`: as an attribute (`name="v"` or `name='v'`) or an element
/// (`<name>v</name>`). Returns the value's byte range and the whole match's range. The name
/// must stand alone (`xmp:Rating`, not `MicrosoftPhoto:Rating` or `xmp:RatingPercent`).
fn xmp_find(s: &str, name: &str) -> Option<(std::ops::Range<usize>, std::ops::Range<usize>)> {
    let b = s.as_bytes();
    let mut from = 0;
    while let Some(off) = s[from..].find(name) {
        let i = from + off;
        from = i + name.len();
        let before = i.checked_sub(1).map(|k| b[k]);
        let after = &s[i + name.len()..];
        if before == Some(b'<') {
            if let Some(rest) = after.strip_prefix('>') {
                let v0 = i + name.len() + 1;
                let close = format!("</{name}>");
                let end = rest.find(&close)?;
                return Some((v0..v0 + end, i - 1..v0 + end + close.len()));
            }
            continue;
        }
        if !before.is_some_and(|c| c.is_ascii_whitespace()) {
            continue;
        }
        let t = after.trim_start();
        let Some(t) = t.strip_prefix('=') else {
            continue;
        };
        let t2 = t.trim_start();
        let Some(q) = t2.chars().next().filter(|c| *c == '"' || *c == '\'') else {
            continue;
        };
        let v0 = s.len() - t2.len() + 1;
        let len = s[v0..].find(q)?;
        return Some((v0..v0 + len, i..v0 + len + 1));
    }
    None
}

pub(crate) fn xmp_value<'a>(s: &'a str, name: &str) -> Option<&'a str> {
    xmp_find(s, name).map(|(v, _)| &s[v])
}

const NS_XMP: &str = "http://ns.adobe.com/xap/1.0/";
const NS_XMP_DM: &str = "http://ns.adobe.com/xmp/1.0/DynamicMedia/";

/// `existing` (a sidecar's text, or None) with the marks of `m` written in: `xmp:Rating`
/// (-1 for a reject, as Adobe does), `xmp:Label` and `xmpDM:pick` (1 pick, -1 reject; dropped
/// when unflagged). Everything else is kept byte for byte.
pub(crate) fn update_xmp(existing: Option<&str>, m: Mark) -> String {
    let rating = if m.flag == -1 {
        -1
    } else {
        i32::from(m.rating)
    };
    let label = (m.label > 0).then(|| LABELS[(m.label - 1) as usize]);
    let pick = (m.flag != 0).then(|| m.flag.to_string());
    let Some(s) = existing.filter(|s| s.contains("rdf:Description")) else {
        return minimal_xmp(rating, label, pick.as_deref());
    };
    let mut s = set_xmp_value(s, "xmp:Rating", Some(&rating.to_string()));
    s = set_xmp_value(&s, "xmp:Label", label);
    s = set_xmp_value(&s, "xmpDM:pick", pick.as_deref());
    s = ensure_ns(s, "xmp", NS_XMP);
    if pick.is_some() {
        s = ensure_ns(s, "xmpDM", NS_XMP_DM);
    }
    s
}

pub(crate) fn ensure_ns(s: String, prefix: &str, uri: &str) -> String {
    if s.contains(&format!("xmlns:{prefix}=")) {
        return s;
    }
    s.replacen(
        "<rdf:Description",
        &format!("<rdf:Description xmlns:{prefix}=\"{uri}\""),
        1,
    )
}

pub(crate) fn set_xmp_value(s: &str, name: &str, value: Option<&str>) -> String {
    if let Some((v, whole)) = xmp_find(s, name) {
        let element = s[whole.clone()].starts_with('<');
        return match value {
            Some(x) => format!("{}{}{}", &s[..v.start], escape(x), &s[v.end..]),
            // Drop the element, or the attribute and the space before it.
            None if element => format!("{}{}", &s[..whole.start], &s[whole.end..]),
            None => {
                let from = s[..whole.start].trim_end().len();
                format!("{}{}", &s[..from], &s[whole.end..])
            }
        };
    }
    match value {
        Some(v) => s.replacen(
            "<rdf:Description",
            &format!("<rdf:Description {name}=\"{}\"", escape(v)),
            1,
        ),
        None => s.to_owned(),
    }
}

/// Our values are numbers and label names, but stay safe in an attribute either way.
fn escape(v: &str) -> String {
    v.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn minimal_xmp(rating: i32, label: Option<&str>, pick: Option<&str>) -> String {
    let label = label.map_or(String::new(), |l| format!("\n   xmp:Label=\"{l}\""));
    let (dm_ns, pick) = pick.map_or((String::new(), String::new()), |p| {
        (
            format!("\n   xmlns:xmpDM=\"{NS_XMP_DM}\""),
            format!("\n   xmpDM:pick=\"{p}\""),
        )
    });
    format!(
        "<?xpacket begin=\"\u{feff}\" id=\"W5M0MpCehiHzreSzNTczkc9d\"?>\n\
         <x:xmpmeta xmlns:x=\"adobe:ns:meta/\" x:xmptk=\"Specular\">\n \
         <rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\n  \
         <rdf:Description rdf:about=\"\"\n   \
         xmlns:xmp=\"{NS_XMP}\"{dm_ns}\n   \
         xmp:Rating=\"{rating}\"{label}{pick}/>\n \
         </rdf:RDF>\n\
         </x:xmpmeta>\n\
         <?xpacket end=\"w\"?>\n"
    )
}

/// Held by every sidecar write (marks and keywords), so one never reads a sidecar that
/// another is about to replace.
pub(crate) static SIDECAR_WRITE: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Orders sidecar writes: take one on the main thread when the marks change, and pass it to
/// [`write_sidecars`].
pub(crate) fn sidecar_ticket() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(1);
    N.fetch_add(1, Ordering::Relaxed)
}

/// Writes the marks into the sidecars of `images` (creating them when needed), one folder scan
/// per folder. Only `.xmp` files are ever written; in another app's sidecar only the marks'
/// bytes change. The images are never touched. Writes run one at a
/// time, and a write older (by `ticket`) than one already done for an image is skipped, so
/// quick changes on background threads cannot land out of order.
pub(crate) fn write_sidecars(
    ticket: u64,
    changes: Vec<(PathBuf, Mark)>,
) -> Vec<(PathBuf, io::Error)> {
    static LATEST: std::sync::Mutex<Option<HashMap<PathBuf, u64>>> = std::sync::Mutex::new(None);
    let _one_at_a_time = SIDECAR_WRITE.lock().unwrap_or_else(|e| e.into_inner());
    let mut latest = LATEST.lock().unwrap_or_else(|e| e.into_inner());
    let latest = latest.get_or_insert_with(HashMap::new);
    let mut folders: HashMap<PathBuf, Sidecars> = HashMap::new();
    let mut failed = Vec::new();
    for (image, m) in changes {
        if latest.get(&image).is_some_and(|&t| t > ticket) || !image.is_file() {
            continue;
        }
        latest.insert(image.clone(), ticket);
        let dir = image.parent().unwrap_or(Path::new("")).to_path_buf();
        let sc = folders.entry(dir).or_insert_with_key(|d| Sidecars::scan(d));
        // Marks go into the sidecar found, another app's too (only their own bytes change,
        // DESIGN §6.3), so the other app sees them.
        let (path, exists) = sc.resolve(&image);
        if let Err(e) = write_sidecar_at(&image, &path, exists, m) {
            failed.push((image, e));
        } else if !exists && path.is_file() {
            sc.add(&path);
        }
    }
    failed
}

fn write_sidecar_at(image: &Path, path: &Path, exists: bool, m: Mark) -> io::Result<()> {
    let is_xmp = |p: &Path| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("xmp"));
    // Belt and braces: never anything but a sidecar, never the image itself.
    if !is_xmp(path) || is_xmp(image) || path == image {
        return Err(io::Error::other("not a sidecar path"));
    }
    let existing = if exists {
        Some(std::fs::read_to_string(path)?)
    } else {
        None
    };
    if existing.is_none() && m.is_empty() {
        return Ok(());
    }
    let text = update_xmp(existing.as_deref(), m);
    if existing.as_deref() == Some(text.as_str()) {
        return Ok(());
    }
    write_atomic(path, text.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marks_table_round_trip() {
        let mut s = Store::new(None);
        s.set_mark(
            Path::new("/a/b.jpg"),
            Mark {
                rating: 4,
                label: 2,
                flag: 1,
            },
        );
        s.set_mark(
            Path::new("/a/c.jpg"),
            Mark {
                rating: 0,
                label: 0,
                flag: -1,
            },
        );
        s.dir = Some(PathBuf::from("/nowhere"));
        let (_, m, _) = s.snapshot().unwrap();
        let back = parse_marks(&m);
        assert_eq!(back, s.marks);
        // Clearing a mark removes the row.
        s.set_mark(Path::new("/a/c.jpg"), Mark::default());
        assert_eq!(s.marks.len(), 1);
        assert!(parse_marks("garbage\n/x\t9\t9\t9\n")[Path::new("/x")].rating == 5);
    }

    /// Other tools' sidecars (hand-written fixtures) keep every byte but the rating
    /// we change; writing the marks they already have changes nothing.
    #[test]
    fn other_tools_sidecars_change_only_the_rating() {
        const FIX: [(&str, &str, &str, &str); 4] = [
            (
                include_str!(
                    "../../../../../crates/specular-develop/tests/fixtures/xmp/lightroom.xmp"
                ),
                "xmp:Rating=\"3\"",
                "xmp:Rating=\"1\"",
                "lightroom",
            ),
            (
                include_str!(
                    "../../../../../crates/specular-develop/tests/fixtures/xmp/darktable.xmp"
                ),
                "xmp:Rating=\"4\"",
                "xmp:Rating=\"1\"",
                "darktable",
            ),
            (
                include_str!(
                    "../../../../../crates/specular-develop/tests/fixtures/xmp/digikam.xmp"
                ),
                "xmp:Rating=\"5\"",
                "xmp:Rating=\"1\"",
                "digikam",
            ),
            (
                include_str!(
                    "../../../../../crates/specular-develop/tests/fixtures/xmp/captureone.xmp"
                ),
                "<xmp:Rating>2</xmp:Rating>",
                "<xmp:Rating>1</xmp:Rating>",
                "captureone",
            ),
        ];
        for (text, old, new, tool) in FIX {
            let m = parse_xmp(text);
            assert!(m.rating > 0, "{tool}");
            assert_eq!(update_xmp(Some(text), m), text, "{tool}: same marks");
            let one = Mark { rating: 1, ..m };
            assert_eq!(
                update_xmp(Some(text), one),
                text.replacen(old, new, 1),
                "{tool}: only the rating"
            );
        }
    }

    #[test]
    fn xmp_new_and_updated_keep_other_content() {
        let m = Mark {
            rating: 3,
            label: 1,
            flag: 0,
        };
        let new = update_xmp(None, m);
        assert_eq!(parse_xmp(&new), m);
        // Attribute form with other fields.
        let lr = r#"<x:xmpmeta><rdf:RDF><rdf:Description rdf:about="" xmlns:xmp="http://ns.adobe.com/xap/1.0/" xmp:Rating="1" xmp:CreatorTool="LR" crs:Exposure="+0.5"/></rdf:RDF></x:xmpmeta>"#;
        let up = update_xmp(
            Some(lr),
            Mark {
                rating: 5,
                label: 4,
                flag: 0,
            },
        );
        assert!(up.contains("crs:Exposure=\"+0.5\"") && up.contains("xmp:CreatorTool"));
        assert_eq!(
            parse_xmp(&up),
            Mark {
                rating: 5,
                label: 4,
                flag: 0
            }
        );
        // Removing the label drops the attribute.
        let cleared = update_xmp(
            Some(&up),
            Mark {
                rating: 5,
                label: 0,
                flag: 0,
            },
        );
        assert!(!cleared.contains("xmp:Label"));
        // Element form.
        let el = "<rdf:Description rdf:about=\"\"><xmp:Rating>2</xmp:Rating></rdf:Description>";
        let up = update_xmp(
            Some(el),
            Mark {
                rating: 0,
                label: 0,
                flag: -1,
            },
        );
        assert!(up.contains("<xmp:Rating>-1</xmp:Rating>") && up.contains("xmlns:xmp="));
        assert_eq!(parse_xmp(&up).flag, -1);
    }

    #[test]
    fn xmp_pick_reject_and_namespaces() {
        let pick = Mark {
            rating: 2,
            label: 0,
            flag: 1,
        };
        let new = update_xmp(None, pick);
        assert!(new.contains("xmpDM:pick=\"1\"") && new.contains("xmlns:xmpDM="));
        assert_eq!(parse_xmp(&new), pick);
        // Reject: rating -1 and pick -1; un-rejecting restores the stars and drops the pick.
        let rej = update_xmp(Some(&new), Mark { flag: -1, ..pick });
        assert!(rej.contains("xmp:Rating=\"-1\"") && rej.contains("xmpDM:pick=\"-1\""));
        assert_eq!(parse_xmp(&rej).flag, -1);
        let back = update_xmp(Some(&rej), Mark { flag: 0, ..pick });
        assert!(!back.contains("xmpDM:pick") && back.contains("xmp:Rating=\"2\""));
        // A sidecar without the xmpDM namespace gets it declared when a pick is written.
        let lr = r#"<rdf:Description rdf:about="" xmlns:xmp="http://ns.adobe.com/xap/1.0/" xmp:Rating="3"/>"#;
        let up = update_xmp(Some(lr), pick);
        assert!(up.contains(&format!("xmlns:xmpDM=\"{NS_XMP_DM}\"")));
        assert_eq!(parse_xmp(&up), pick);
        // Element form of the pick.
        let el = "<rdf:Description><xmpDM:pick>-1</xmpDM:pick></rdf:Description>";
        assert_eq!(parse_xmp(el).flag, -1);
        let up = update_xmp(Some(el), Mark::default());
        assert!(!up.contains("xmpDM:pick") && up.contains("<rdf:Description"));
    }

    #[test]
    fn xmp_names_match_whole_and_either_quote() {
        let s = r#"<rdf:Description MicrosoftPhoto:Rating="99" xmp:RatingPercent="80" xmp:Rating='4' xmp:Label = "Green"/>"#;
        assert_eq!(
            parse_xmp(s),
            Mark {
                rating: 4,
                label: 3,
                flag: 0
            }
        );
        let up = update_xmp(
            Some(s),
            Mark {
                rating: 1,
                label: 0,
                flag: 0,
            },
        );
        assert!(
            up.contains("MicrosoftPhoto:Rating=\"99\"") && up.contains("xmp:RatingPercent=\"80\"")
        );
        assert!(up.contains("xmp:Rating='1'") && !up.contains("xmp:Label"));
    }

    /// Synthetic files only, in the build's scratch folder (`SPECULAR_TEST_TMP` overrides).
    fn scratch(name: &str) -> PathBuf {
        let base = std::env::var_os("SPECULAR_TEST_TMP").map_or_else(
            || Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/test-tmp"),
            PathBuf::from,
        );
        let d = base.join(format!("{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn sidecars_on_disk_follow_naming_and_never_touch_images() {
        let d = scratch("gallery-sidecars");
        let image_bytes = b"\xff\xd8\xff\xd9 not really a jpeg";
        for f in ["A.jpg", "B.CR3", "B.JPG", "C.nef", "D.png", "E.jpg"] {
            std::fs::write(d.join(f), image_bytes).unwrap();
        }
        // Existing sidecars: Lightroom's stem form with other content, darktable's long form,
        // and an upper-case one.
        let lr = r#"<x:xmpmeta><rdf:RDF><rdf:Description rdf:about="" xmlns:xmp="http://ns.adobe.com/xap/1.0/" xmp:Rating="2" crs:Exposure2012="+0.50"/></rdf:RDF></x:xmpmeta>"#;
        std::fs::write(d.join("B.xmp"), lr).unwrap();
        std::fs::write(d.join("C.nef.xmp"), minimal_xmp(5, Some("Red"), None)).unwrap();
        std::fs::write(d.join("E.XMP"), minimal_xmp(1, None, None)).unwrap();

        let sc = Sidecars::scan(&d);
        assert_eq!(sc.resolve(&d.join("A.jpg")), (d.join("A.xmp"), false));
        // RAW+JPEG: the stem sidecar is the RAW's; the JPEG gets the long form.
        assert_eq!(sc.resolve(&d.join("B.CR3")), (d.join("B.xmp"), true));
        assert_eq!(sc.resolve(&d.join("B.JPG")), (d.join("B.JPG.xmp"), false));
        assert_eq!(sc.resolve(&d.join("C.nef")), (d.join("C.nef.xmp"), true));
        assert_eq!(sc.resolve(&d.join("E.jpg")), (d.join("E.XMP"), true));

        let read = read_sidecars(
            ["A.jpg", "B.CR3", "B.JPG", "C.nef", "E.jpg"].map(|f| d.join(f)),
            &[],
            &|| false,
        );
        let got: HashMap<_, _> = read.into_iter().collect();
        assert_eq!(got.len(), 3);
        assert_eq!(got[&d.join("B.CR3")].rating, 2);
        assert_eq!(
            got[&d.join("C.nef")],
            Mark {
                rating: 5,
                label: 1,
                flag: 0
            }
        );

        let m = |rating, flag| Mark {
            rating,
            label: 0,
            flag,
        };
        let t = sidecar_ticket();
        let failed = write_sidecars(
            t,
            vec![
                (d.join("A.jpg"), m(3, 1)),
                (d.join("B.CR3"), m(4, 0)),
                (d.join("B.JPG"), m(1, -1)),
                (d.join("D.png"), Mark::default()),
                (d.join("E.jpg"), m(5, 0)),
                (d.join("missing.jpg"), m(5, 0)),
            ],
        );
        assert!(failed.is_empty(), "{failed:?}");
        // An older write that lands late is skipped.
        assert!(write_sidecars(t - 1, vec![(d.join("A.jpg"), m(1, 0))]).is_empty());
        let read = |f: &str| std::fs::read_to_string(d.join(f)).unwrap();
        assert_eq!(parse_xmp(&read("A.xmp")), m(3, 1));
        let b = read("B.xmp");
        assert!(b.contains("crs:Exposure2012=\"+0.50\"") && parse_xmp(&b) == m(4, 0));
        assert_eq!(parse_xmp(&read("B.JPG.xmp")).flag, -1);
        assert_eq!(parse_xmp(&read("E.XMP")).rating, 5);
        // No sidecar for an unmarked file, none for a missing one, no temporary files left,
        // and every image byte for byte as it was.
        let names: Vec<String> = std::fs::read_dir(&d)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert!(!names.iter().any(|n| n.starts_with("D.") && n != "D.png"));
        assert!(
            !names
                .iter()
                .any(|n| n.starts_with("missing") || n.ends_with("specular-tmp"))
        );
        assert_eq!(names.len(), 6 + 5);
        for f in ["A.jpg", "B.CR3", "B.JPG", "C.nef", "D.png", "E.jpg"] {
            assert_eq!(std::fs::read(d.join(f)).unwrap(), image_bytes);
        }
        // Refused: anything but an .xmp path, or an .xmp "image".
        assert!(write_sidecar_at(&d.join("A.jpg"), &d.join("A.jpg"), true, m(1, 0)).is_err());
        assert!(write_sidecar_at(&d.join("A.xmp"), &d.join("A.xmp"), true, m(1, 0)).is_err());
        std::fs::remove_dir_all(&d).ok();
    }

    /// Sidecar interop 2: other apps' sidecars are read; marks are written into them (only
    /// the marks' bytes change, DESIGN §6.3), keywords and positions never: those go into a
    /// sidecar of Specular's own in a form no other app's takes.
    #[test]
    fn other_apps_sidecars_take_marks_only() {
        const DT: &str = include_str!(
            "../../../../../crates/specular-metadata/tests/fixtures/sidecars/darktable.CR2.xmp"
        );
        const LR: &str = include_str!(
            "../../../../../crates/specular-metadata/tests/fixtures/sidecars/lightroom.xmp"
        );
        const RT: &str = include_str!(
            "../../../../../crates/specular-metadata/tests/fixtures/sidecars/rawtherapee.CR2.pp3"
        );
        let d = scratch("gallery-foreign-sidecars");
        for f in ["A.CR2", "B.CR2", "C.CR2", "D.CR2"] {
            std::fs::write(d.join(f), b"raw").unwrap();
        }
        // A: darktable's long form only. B: Lightroom's and darktable's (both forms taken).
        // C: RawTherapee only. D: darktable's beside Specular's own.
        std::fs::write(d.join("A.CR2.xmp"), DT).unwrap();
        std::fs::write(d.join("B.xmp"), LR).unwrap();
        std::fs::write(d.join("B.CR2.xmp"), DT).unwrap();
        std::fs::write(d.join("C.CR2.pp3"), RT).unwrap();
        std::fs::write(d.join("D.CR2.xmp"), DT).unwrap();
        std::fs::write(d.join("D.xmp"), minimal_xmp(1, None, None)).unwrap();

        let all = ["A.CR2", "B.CR2", "C.CR2", "D.CR2"].map(|f| d.join(f));
        let got: HashMap<_, _> = read_sidecars(all.clone(), &[], &|| false)
            .into_iter()
            .collect();
        let mk = |rating, label, flag| Mark {
            rating,
            label,
            flag,
        };
        // darktable: 3 stars, green; Lightroom first for B (the stem form counts first).
        assert_eq!(got[&d.join("A.CR2")], mk(3, 3, 0));
        assert_eq!(got[&d.join("B.CR2")], mk(4, 2, 1));
        assert_eq!(got[&d.join("C.CR2")], mk(5, 4, 0));
        // D: Specular's own 1 star wins; darktable's label fills the gap.
        assert_eq!(got[&d.join("D.CR2")], mk(1, 3, 0));
        // Preferring the other apps' sidecars in the folder: darktable's stars.
        let pref = read_sidecars(all.clone(), std::slice::from_ref(&d), &|| false);
        let pref: HashMap<_, _> = pref.into_iter().collect();
        assert_eq!(pref[&d.join("D.CR2")], mk(3, 3, 0));

        let failed = write_sidecars(
            sidecar_ticket(),
            all.iter().map(|p| (p.clone(), mk(2, 1, 1))).collect(),
        );
        assert!(failed.is_empty(), "{failed:?}");
        let read = |f: &str| std::fs::read_to_string(d.join(f)).unwrap();
        // darktable's and Lightroom's: only the marks' bytes changed, still theirs.
        let marked = update_xmp(Some(DT), mk(2, 1, 1));
        assert_ne!(marked, DT);
        assert_eq!(read("A.CR2.xmp"), marked);
        // Every other byte as darktable wrote it: without the rating, label and pick (and the
        // pick's namespace) both are the same text.
        let without = |s: &str| {
            let mut s = s.to_owned();
            for n in ["xmp:Rating", "xmp:Label", "xmpDM:pick"] {
                s = set_xmp_value(&s, n, None);
            }
            s.replace(&format!(" xmlns:xmpDM=\"{NS_XMP_DM}\""), "")
        };
        assert_eq!(without(&read("A.CR2.xmp")), without(DT));
        assert_eq!(without(&read("B.xmp")), without(LR));
        assert_eq!(
            specular_metadata::sidecars::owner(&marked),
            Some(specular_metadata::sidecars::App::Darktable)
        );
        assert_eq!(read("B.xmp"), update_xmp(Some(LR), mk(2, 1, 1)));
        assert_eq!(read("B.CR2.xmp"), DT);
        // RawTherapee's profile is not XMP: never written; C gets a sidecar of its own.
        assert_eq!(read("C.CR2.pp3"), RT);
        assert_eq!(parse_xmp(&read("C.xmp")), mk(2, 1, 1));
        // D: Specular's own takes them, darktable's is left.
        assert_eq!(read("D.CR2.xmp"), DT);
        assert_eq!(parse_xmp(&read("D.xmp")), mk(2, 1, 1));
        assert!(!d.join("A.xmp").exists());
        // Keywords and positions: Specular's own sidecar in the free form, none for B.
        let sc = Sidecars::scan(&d);
        assert_eq!(
            sc.resolve_writable(&d.join("A.CR2")),
            Some((d.join("A.xmp"), false))
        );
        assert_eq!(sc.resolve_writable(&d.join("B.CR2")), None);
        assert_eq!(
            sc.resolve_writable(&d.join("D.CR2")),
            Some((d.join("D.xmp"), true))
        );
        let got: HashMap<_, _> = read_sidecars(all, &[], &|| false).into_iter().collect();
        assert_eq!(got[&d.join("A.CR2")], mk(2, 1, 1));
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn prefer_setting_and_marks_over_sidecars() {
        assert_eq!(
            prefer_folders(" /a/b ; ;/c "),
            [PathBuf::from("/a/b"), PathBuf::from("/c")]
        );
        let mut s = Store::new(None);
        let p = Path::new("/a/b/x.CR2");
        let side = Mark {
            rating: 3,
            label: 0,
            flag: 0,
        };
        s.merge_read([(p.to_path_buf(), side)]);
        assert_eq!(s.mark(p), side);
        let mine = Mark { rating: 1, ..side };
        s.set_mark(p, mine);
        assert_eq!(s.mark(p), mine);
        // Cleared by the user: the sidecar's does not come back.
        s.set_mark(p, Mark::default());
        assert_eq!(s.mark(p), Mark::default());
        // A folder that prefers other apps' sidecars shows theirs.
        s.prefer = vec![PathBuf::from("/a")];
        assert_eq!(s.mark(p), side);
    }
}
