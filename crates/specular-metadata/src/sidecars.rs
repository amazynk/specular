//! Sidecars written by other photo apps beside the images: which app wrote one, what it says
//! about the photo (rating, colour label, pick or reject, keywords, title and caption), and
//! which sidecar names belong to which image.
//!
//! Read from the published formats only:
//! - XMP (Adobe XMP specification part 1–3; the Lightroom `lr:hierarchicalSubject` property;
//!   darktable's documented `darktable:colorlabels`, `xmp:Rating` -1 for rejected, and its
//!   `IMG_0001_01.CR2.xmp` names for duplicates; digiKam's `digiKam:TagsList`,
//!   `digiKam:ColorLabel` and `digiKam:PickLabel`).
//! - RawTherapee processing profiles (`.pp3`, an INI file: `[General]` `Rank`, `ColorLabel`,
//!   `InTrash`; `[IPTC]` `Keywords`, `Caption`, `Title`).
//! - Capture One settings (`CaptureOne/Settings*/IMG.CR2.cos`): only the plain `Rating` and
//!   `ColorLabel` key/value entries; everything else in them is left alone.
//!
//! Another app's sidecar takes only Specular's marks (their bytes alone change): [`owner`]
//! tells them apart from the ones Specular wrote, and keywords and positions go elsewhere.

use std::path::{Path, PathBuf};

/// The colour labels Specular has (`xmp:Label` names), 1-based in [`Values::label`].
pub const LABELS: [&str; 4] = ["Red", "Yellow", "Green", "Blue"];

/// The app that wrote a sidecar.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum App {
    Darktable,
    /// Lightroom, Camera Raw, Bridge and the other Adobe apps (one XMP toolkit).
    Adobe,
    CaptureOne,
    Digikam,
    RawTherapee,
    PhotoMechanic,
    /// Another app's XMP toolkit (exiftool, Exiv2…), named by its `x:xmptk`.
    Other,
}

impl App {
    /// A stable name, for the catalogue.
    pub fn id(self) -> &'static str {
        match self {
            App::Darktable => "darktable",
            App::Adobe => "adobe",
            App::CaptureOne => "captureone",
            App::Digikam => "digikam",
            App::RawTherapee => "rawtherapee",
            App::PhotoMechanic => "photomechanic",
            App::Other => "other",
        }
    }

    pub fn from_id(id: &str) -> Option<App> {
        Some(match id {
            "darktable" => App::Darktable,
            "adobe" => App::Adobe,
            "captureone" => App::CaptureOne,
            "digikam" => App::Digikam,
            "rawtherapee" => App::RawTherapee,
            "photomechanic" => App::PhotoMechanic,
            "other" => App::Other,
            _ => return None,
        })
    }

    /// The app's name as it calls itself (not translated); None for [`App::Other`].
    pub fn name(self) -> Option<&'static str> {
        Some(match self {
            App::Darktable => "darktable",
            App::Adobe => "Lightroom",
            App::CaptureOne => "Capture One",
            App::Digikam => "digiKam",
            App::RawTherapee => "RawTherapee",
            App::PhotoMechanic => "Photo Mechanic",
            App::Other => return None,
        })
    }
}

/// What a sidecar says about a photo; None where it says nothing.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Values {
    /// Stars, 0–5.
    pub rating: Option<u8>,
    /// 1-based in [`LABELS`]; 0 says "no label".
    pub label: Option<u8>,
    /// -1 rejected, 1 picked, 0 neither.
    pub flag: Option<i8>,
    /// Keyword paths (levels joined by `|`).
    pub keywords: Option<Vec<String>>,
    pub title: Option<String>,
    pub caption: Option<String>,
}

impl Values {
    pub fn is_empty(&self) -> bool {
        *self == Values::default()
    }

    /// Takes what `other` says where this says nothing.
    pub fn fill_from(&mut self, other: &Values) {
        self.rating = self.rating.or(other.rating);
        self.label = self.label.or(other.label);
        self.flag = self.flag.or(other.flag);
        if self.keywords.is_none() {
            self.keywords.clone_from(&other.keywords);
        }
        if self.title.is_none() {
            self.title.clone_from(&other.title);
        }
        if self.caption.is_none() {
            self.caption.clone_from(&other.caption);
        }
    }
}

/// The kinds of sidecar files read.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Xmp,
    /// RawTherapee.
    Pp3,
    /// Capture One.
    Cos,
}

impl Kind {
    pub fn of_name(name: &str) -> Option<Kind> {
        let (_, ext) = name.rsplit_once('.')?;
        if ext.eq_ignore_ascii_case("xmp") {
            Some(Kind::Xmp)
        } else if ext.eq_ignore_ascii_case("pp3") {
            Some(Kind::Pp3)
        } else if ext.eq_ignore_ascii_case("cos") {
            Some(Kind::Cos)
        } else {
            None
        }
    }
}

/// One sidecar as read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Read {
    /// None: Specular wrote it (it is Specular's to write).
    pub app: Option<App>,
    pub values: Values,
    /// A darktable duplicate's name (`darktable:version_name`).
    pub version_name: Option<String>,
}

/// Reads a sidecar of kind `kind`.
pub fn read(kind: Kind, bytes: &[u8]) -> Read {
    match kind {
        Kind::Xmp => read_xmp(bytes),
        Kind::Pp3 => Read {
            app: Some(App::RawTherapee),
            values: read_pp3(&String::from_utf8_lossy(bytes)),
            version_name: None,
        },
        Kind::Cos => Read {
            app: Some(App::CaptureOne),
            values: read_cos(bytes),
            version_name: None,
        },
    }
}

const NS_DARKTABLE: &str = "http://darktable.sf.net/";
const NS_DIGIKAM: &str = "http://www.digikam.org/ns/1.0/";
const NS_PHOTO_MECHANIC: &str = "http://ns.camerabits.com/photomechanic/1.0/";
const NS_CRS: &str = "http://ns.adobe.com/camera-raw-settings/1.0/";

/// Which app wrote an XMP sidecar; None when Specular did (its toolkit name is `Specular`) or
/// nothing says (a hand-written packet). Other apps' namespaces come first, then
/// `xmp:CreatorTool`, then the toolkit (`x:xmptk`).
pub fn owner(text: &str) -> Option<App> {
    // The packet is parsed only for `xmp:CreatorTool`, when it is there (the namespace and
    // toolkit checks are substring searches).
    let parsed;
    let x = if text.contains("CreatorTool") {
        parsed = crate::xmp::parse(text.as_bytes());
        Some(&parsed)
    } else {
        None
    };
    owner_of(text, x)
}

/// [`owner`] with the packet already parsed (`None`: it has no `xmp:CreatorTool`).
fn owner_of(text: &str, x: Option<&crate::xmp::Xmp>) -> Option<App> {
    let ns = |uri: &str| text.contains(uri);
    if ns(NS_DARKTABLE) {
        return Some(App::Darktable);
    }
    if ns(NS_DIGIKAM) {
        return Some(App::Digikam);
    }
    if ns(NS_PHOTO_MECHANIC) {
        return Some(App::PhotoMechanic);
    }
    if let Some(tool) = x
        .and_then(|x| x.get("xmp:CreatorTool"))
        .map(str::to_ascii_lowercase)
    {
        let has = |w: &str| tool.contains(w);
        if has("darktable") {
            return Some(App::Darktable);
        }
        if has("capture one") {
            return Some(App::CaptureOne);
        }
        if has("digikam") {
            return Some(App::Digikam);
        }
        if has("rawtherapee") {
            return Some(App::RawTherapee);
        }
        if has("photo mechanic") {
            return Some(App::PhotoMechanic);
        }
        if has("lightroom") || has("camera raw") || has("photoshop") || has("bridge") {
            return Some(App::Adobe);
        }
    }
    if ns(NS_CRS) {
        return Some(App::Adobe);
    }
    match xmptk(text) {
        Some(t) if t.starts_with("Specular") => None,
        Some(t) if t.starts_with("Adobe XMP Core") => Some(App::Adobe),
        Some(t) if !t.trim().is_empty() => Some(App::Other),
        _ => None,
    }
}

/// The `x:xmptk` attribute of the packet's `x:xmpmeta` (or `x:xapmeta`).
fn xmptk(text: &str) -> Option<&str> {
    let i = text.find("xmptk=")?;
    let rest = &text[i + 6..];
    let q = rest.chars().next().filter(|c| *c == '"' || *c == '\'')?;
    let rest = &rest[1..];
    rest.find(q).map(|e| &rest[..e])
}

/// Reads an XMP sidecar.
pub fn read_xmp(bytes: &[u8]) -> Read {
    let text = String::from_utf8_lossy(bytes);
    let x = crate::xmp::parse(text.as_bytes());
    let app = owner_of(&text, Some(&x));
    let num = |k: &str| x.get(k).and_then(|v| v.trim().parse::<i32>().ok());
    let mut v = Values::default();
    let rating = num("xmp:Rating");
    match rating {
        // -1: rejected (Adobe, darktable). Its stars are not said.
        Some(r) if r < 0 => v.flag = Some(-1),
        Some(r) => v.rating = Some(r.clamp(0, 5) as u8),
        None => {}
    }
    v.label = x
        .get("xmp:Label")
        .and_then(|l| label_by_name(l.trim()))
        .or_else(|| {
            // darktable: 0 red, 1 yellow, 2 green, 3 blue, 4 purple; the first we have.
            let all = x.get_all("darktable:colorlabels")?;
            let mut ours: Vec<u8> = all
                .iter()
                .filter_map(|c| c.trim().parse::<u8>().ok())
                .filter(|&c| c < 4)
                .map(|c| c + 1)
                .collect();
            ours.sort_unstable();
            ours.first().copied()
        })
        .or_else(|| {
            // digiKam: 0 none, 1 red, 2 orange, 3 yellow, 4 green, 5 blue, 6 magenta, 7 grey,
            // 8 black, 9 white.
            match num("digiKam:ColorLabel")? {
                1 => Some(1),
                3 => Some(2),
                4 => Some(3),
                5 => Some(4),
                _ => None,
            }
        });
    if v.flag.is_none() {
        v.flag = match num("xmpDM:pick") {
            Some(-1) => Some(-1),
            Some(1) => Some(1),
            _ => match num("digiKam:PickLabel") {
                // 1 rejected, 2 pending, 3 accepted.
                Some(1) => Some(-1),
                Some(3) => Some(1),
                _ => None,
            },
        };
    }
    v.keywords = xmp_keywords(&x);
    v.title = x.get("dc:title").map(str::to_owned);
    v.caption = x.get("dc:description").map(str::to_owned);
    Read {
        app,
        values: v,
        version_name: x.get("darktable:version_name").map(str::to_owned),
    }
}

fn label_by_name(name: &str) -> Option<u8> {
    LABELS
        .iter()
        .position(|l| l.eq_ignore_ascii_case(name))
        .map(|i| i as u8 + 1)
}

/// Whether a keyword path is one of darktable's own (`darktable|format|cr2`,
/// `darktable|changed`): darktable's bookkeeping, not the photographer's keywords.
pub fn is_darktable_internal(path: &str) -> bool {
    let p = path.trim();
    p.eq_ignore_ascii_case("darktable")
        || p.get(..10)
            .is_some_and(|h| h.eq_ignore_ascii_case("darktable|"))
}

/// The keywords: `lr:hierarchicalSubject` and `dc:subject` (as [`crate::keywords::from_xmp`]
/// reads them), else digiKam's `digiKam:TagsList` (levels joined by `/`); darktable's own
/// tags left out.
fn xmp_keywords(x: &crate::xmp::Xmp) -> Option<Vec<String>> {
    let mut kept = crate::xmp::Xmp::default();
    let mut any = false;
    for name in ["lr:hierarchicalSubject", "dc:subject"] {
        for v in x.get_all(name).into_iter().flatten() {
            any = true;
            if !is_darktable_internal(v) {
                kept.add(name, v);
            }
        }
    }
    if !any && let Some(tags) = x.get_all("digiKam:TagsList") {
        for t in tags {
            kept.add("lr:hierarchicalSubject", &t.replace('/', "|"));
        }
        any = !tags.is_empty();
    }
    if !any {
        return None;
    }
    Some(crate::keywords::from_xmp(&kept).unwrap_or_default())
}

/// Reads a RawTherapee processing profile.
pub fn read_pp3(text: &str) -> Values {
    let mut v = Values::default();
    let mut section = String::new();
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('#') || line.starts_with(';') || line.is_empty() {
            continue;
        }
        if let Some(s) = line.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            section = s.trim().to_owned();
            continue;
        }
        let Some((k, val)) = line.split_once('=') else {
            continue;
        };
        let (k, val) = (k.trim(), val.trim());
        match (section.as_str(), k) {
            ("General", "Rank") => {
                if let Ok(r) = val.parse::<i32>() {
                    v.rating = Some(r.clamp(0, 5) as u8);
                }
            }
            // 0 none, 1 red, 2 yellow, 3 green, 4 blue, 5 purple.
            ("General", "ColorLabel") => {
                v.label = match val.parse::<u8>() {
                    Ok(c @ 0..=4) => Some(c),
                    _ => None,
                }
            }
            ("General", "InTrash") if val.eq_ignore_ascii_case("true") => v.flag = Some(-1),
            ("IPTC", "Keywords") => {
                let k: Vec<String> = list(val)
                    .filter_map(|k| crate::keywords::normalise(&k.replace('|', "/")))
                    .collect();
                v.keywords = Some(k);
            }
            ("IPTC", "Caption") => v.caption = list(val).next(),
            ("IPTC", "Title") | ("IPTC", "Headline") if v.title.is_none() => {
                v.title = list(val).next()
            }
            _ => {}
        }
    }
    v
}

/// A RawTherapee list value: `a;b;` (entries end with `;`).
fn list(val: &str) -> impl Iterator<Item = String> + '_ {
    val.split(';')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

/// Reads the rating and colour label of a Capture One settings file: elements with a `K`
/// (key) and a `V` (value) attribute, keys `Rating` (0–5) and `ColorLabel` (0 none, 1 red,
/// 2 orange, 3 yellow, 4 green, 5 blue, 6 pink, 7 purple). Nothing else is read.
pub fn read_cos(bytes: &[u8]) -> Values {
    use quick_xml::events::Event;
    let mut v = Values::default();
    let mut r = quick_xml::Reader::from_reader(bytes);
    r.config_mut().check_end_names = false;
    let mut buf = Vec::new();
    loop {
        match r.read_event_into(&mut buf) {
            Ok(Event::Start(e) | Event::Empty(e)) => {
                let (mut k, mut val) = (None, None);
                for a in e.attributes().with_checks(false).flatten() {
                    let value = String::from_utf8_lossy(&a.value).into_owned();
                    match a.key.as_ref() {
                        b"K" => k = Some(value),
                        b"V" => val = Some(value),
                        _ => {}
                    }
                }
                if let (Some(k), Some(val)) = (k, val) {
                    let n = val.trim().parse::<i32>().ok();
                    match k.as_str() {
                        "Rating" => v.rating = n.map(|n| n.clamp(0, 5) as u8),
                        "ColorLabel" => {
                            v.label = match n {
                                Some(0) => Some(0),
                                Some(1) => Some(1),
                                Some(3) => Some(2),
                                Some(4) => Some(3),
                                Some(5) => Some(4),
                                _ => None,
                            }
                        }
                        _ => {}
                    }
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    v
}

// ----- Names --------------------------------------------------------------------------------

/// A sidecar of an image, by name.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Candidate {
    /// Its path relative to the image's folder (`IMG.CR2.xmp`,
    /// `CaptureOne/Settings153/IMG.CR2.cos`).
    pub name: String,
    /// The darktable duplicate it describes (`IMG_01.CR2.xmp`: 1); 0 for the photo itself.
    pub duplicate: u32,
}

impl Candidate {
    pub fn kind(&self) -> Option<Kind> {
        Kind::of_name(&self.name)
    }
}

/// Camera RAW extensions, for the sidecar naming rule (in a RAW+JPEG pair the stem form,
/// `IMG.xmp`, is the RAW's).
pub const RAW_EXTENSIONS: &[&str] = &[
    "cr2", "cr3", "crw", "nef", "nrw", "arw", "srf", "sr2", "raf", "orf", "rw2", "pef", "dng",
    "srw", "x3f", "3fr", "fff", "iiq", "erf", "kdc", "dcr", "mos", "mef", "mrw", "ari", "raw",
    "rwl", "gpr",
];

pub fn is_raw_ext(ext: &str) -> bool {
    RAW_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str())
}

/// Whether the stem form of a sidecar (`IMG.xmp`) is `image`'s, looked up on disk: unless it
/// is the other half of a RAW+JPEG pair (the stem form is the RAW's).
pub fn stem_is_own_on_disk(image: &Path) -> bool {
    let ext = image
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default();
    if is_raw_ext(ext) {
        return true;
    }
    let Some(stem) = image.file_stem().and_then(|s| s.to_str()) else {
        return true;
    };
    let dir = image.parent().unwrap_or(Path::new(""));
    !RAW_EXTENSIONS.iter().any(|e| {
        dir.join(format!("{stem}.{e}")).is_file()
            || dir
                .join(format!("{stem}.{}", e.to_ascii_uppercase()))
                .is_file()
    })
}

/// How many darktable duplicates (`IMG_01.CR2.xmp`) each photo in `dir` has, for those with
/// any (one listing; a duplicate's name that is an image of its own is not one).
pub fn duplicate_counts_in(dir: &Path) -> Vec<(PathBuf, u32)> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let names: std::collections::HashSet<String> = rd
        .flatten()
        .filter_map(|e| e.file_name().into_string().ok())
        .collect();
    let mut counts: std::collections::BTreeMap<String, u32> = Default::default();
    for n in &names {
        if let Some((photo, _)) = duplicate_of(n)
            && names.contains(&photo)
            && !names.contains(&n[..n.len() - 4])
        {
            *counts.entry(photo).or_default() += 1;
        }
    }
    counts.into_iter().map(|(p, n)| (dir.join(p), n)).collect()
}

/// The folder holding Capture One's settings, beside the images.
pub const CAPTURE_ONE_DIR: &str = "CaptureOne";

/// The sidecars other apps may have written for image `name`, in the order their values
/// count: XMP in the stem form (`IMG.xmp`, only when `stem_is_own`: in a RAW+JPEG pair it
/// is the RAW's) and the long form (`IMG.CR2.xmp`), RawTherapee's `IMG.CR2.pp3` (or
/// `IMG.pp3`), then Capture One's `.cos` in each of `cos_dirs` (`CaptureOne/Settings153`).
/// `has` says whether a name exists (relative to the image's folder).
pub fn candidates(
    name: &str,
    stem_is_own: bool,
    cos_dirs: &[String],
    has: &dyn Fn(&str) -> bool,
) -> Vec<Candidate> {
    let stem = match name.rsplit_once('.') {
        Some((s, _)) if !s.is_empty() => s,
        _ => name,
    };
    let mut out = Vec::new();
    let mut push = |n: String| {
        if has(&n) && !out.iter().any(|c: &Candidate| c.name == n) {
            out.push(Candidate {
                name: n,
                duplicate: 0,
            });
        }
    };
    for ext in ["xmp", "XMP"] {
        if stem_is_own {
            push(format!("{stem}.{ext}"));
        }
    }
    for ext in ["xmp", "XMP"] {
        push(format!("{name}.{ext}"));
    }
    for ext in ["pp3", "PP3"] {
        push(format!("{name}.{ext}"));
        if stem_is_own {
            push(format!("{stem}.{ext}"));
        }
    }
    for d in cos_dirs {
        for n in [name, stem] {
            push(format!("{d}/{n}.cos"));
        }
    }
    out
}

/// The photo and duplicate number a darktable duplicate's sidecar name is of
/// (`IMG_0001_01.CR2.xmp` → `IMG_0001.CR2`, 1). None for other names, Specular's own
/// (`.specular.xmp`) included. Whether the photo exists, and that `IMG_0001_01.CR2` is not an
/// image of its own, is the caller's to check.
pub fn duplicate_of(name: &str) -> Option<(String, u32)> {
    let (rest, ext) = name.rsplit_once('.')?;
    if !ext.eq_ignore_ascii_case("xmp") {
        return None;
    }
    let (stem, image_ext) = rest.rsplit_once('.')?;
    if image_ext.is_empty()
        || image_ext.eq_ignore_ascii_case("xmp")
        || image_ext.eq_ignore_ascii_case("specular")
        || image_ext.starts_with('v') && image_ext[1..].bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    let (base, n) = stem.rsplit_once('_')?;
    if base.is_empty() || n.len() < 2 || !n.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let n: u32 = n.parse().ok().filter(|&n| n > 0)?;
    Some((format!("{base}.{image_ext}"), n))
}

/// The duplicate's sidecar name for photo `name` and number `n` (`IMG.CR2`, 1 →
/// `IMG_01.CR2.xmp`).
pub fn duplicate_name(name: &str, n: u32) -> String {
    match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => format!("{stem}_{n:02}.{ext}.xmp"),
        _ => format!("{name}_{n:02}.xmp"),
    }
}

/// One sidecar found beside a file, read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
    pub path: PathBuf,
    pub candidate: Candidate,
    pub read: Read,
}

/// Whether two existing paths name the same file.
fn same_file(a: &Path, b: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        match (std::fs::metadata(a), std::fs::metadata(b)) {
            (Ok(x), Ok(y)) => x.dev() == y.dev() && x.ino() == y.ino(),
            _ => false,
        }
    }
    #[cfg(not(unix))]
    {
        // The canonical path has the name as stored, whatever case it was asked in.
        match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
            (Ok(x), Ok(y)) => x == y,
            _ => false,
        }
    }
}

/// Duplicates looked for beside one file, at most (darktable numbers them from 1).
const DUPLICATES_MAX: u32 = 999;

/// Every other app's sidecar of `image` found beside it, read (for the Info panel: one `stat`
/// per name; duplicates are looked for in number order until two in a row are missing).
/// Specular's own sidecars are left out. `stem_is_own`: see [`candidates`].
pub fn found_for(image: &Path, stem_is_own: bool) -> Vec<Found> {
    let Some(dir) = image.parent() else {
        return Vec::new();
    };
    let Some(name) = image.file_name().and_then(|n| n.to_str()) else {
        return Vec::new();
    };
    let is_file = |n: &str| dir.join(n).is_file();
    let cos_dirs = capture_one_dirs(dir);
    let all = candidates(name, stem_is_own, &cos_dirs, &is_file);
    // On a case-insensitive file system (macOS, Windows) `IMG.CR2.xmp` and `IMG.CR2.XMP` both
    // exist when either does: the same file, listed once under its first spelling.
    let mut kept: Vec<Candidate> = Vec::with_capacity(all.len());
    for c in all {
        if !kept.iter().any(|k| {
            k.name.eq_ignore_ascii_case(&c.name)
                && same_file(&dir.join(&k.name), &dir.join(&c.name))
        }) {
            kept.push(c);
        }
    }
    let mut all = kept;
    let mut missing = 0;
    for n in 1..=DUPLICATES_MAX {
        let d = duplicate_name(name, n);
        let photo = d.strip_suffix(".xmp").unwrap_or(&d);
        if is_file(&d) && !is_file(photo) {
            missing = 0;
            all.push(Candidate {
                name: d,
                duplicate: n,
            });
        } else {
            missing += 1;
            if missing == 2 {
                break;
            }
        }
    }
    all.into_iter()
        .filter_map(|c| {
            let path = dir.join(&c.name);
            let bytes = read_small(&path)?;
            let read = read(c.kind()?, &bytes);
            read.app.is_some().then_some(Found {
                path,
                candidate: c,
                read,
            })
        })
        .collect()
}

/// Capture One's settings folders beside the images (`CaptureOne/Settings153`), relative.
pub fn capture_one_dirs(dir: &Path) -> Vec<String> {
    let Ok(rd) = std::fs::read_dir(dir.join(CAPTURE_ONE_DIR)) else {
        return Vec::new();
    };
    let mut out: Vec<String> = rd
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| n.starts_with("Settings"))
        .map(|n| format!("{CAPTURE_ONE_DIR}/{n}"))
        .collect();
    // The newest settings version first (Settings153 before Settings50).
    out.sort_by_key(|n| {
        std::cmp::Reverse(
            n.trim_start_matches(CAPTURE_ONE_DIR)
                .trim_start_matches("/Settings")
                .parse::<u32>()
                .unwrap_or(0),
        )
    });
    out
}

/// Sidecars larger than this are not read (a sidecar is a few kilobytes; darktable's with a
/// long history some hundreds).
pub const SIDECAR_MAX: u64 = 16 << 20;

/// The bytes of a regular file of at most [`SIDECAR_MAX`] (a FIFO named like a sidecar must
/// not block).
pub fn read_small(path: &Path) -> Option<Vec<u8>> {
    let m = std::fs::metadata(path).ok()?;
    if !m.is_file() || m.len() > SIDECAR_MAX {
        return None;
    }
    std::fs::read(path).ok()
}

#[cfg(test)]
#[path = "sidecars/tests.rs"]
mod tests;
