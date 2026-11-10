//! File names: validation with plain reasons, the stem/extension split the rename field
//! selects, and the sidecars that travel with an image.

use std::path::{Path, PathBuf};

/// Whose naming rules apply. The rules are the target folder's operating system's: Windows
/// refuses more characters and some whole names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    Unix,
    Windows,
    Mac,
}

impl Platform {
    pub const fn current() -> Platform {
        if cfg!(windows) {
            Platform::Windows
        } else if cfg!(target_os = "macos") {
            Platform::Mac
        } else {
            Platform::Unix
        }
    }
}

/// Why a name can't be used. The app turns each into a sentence (`rename-problem-*`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NameProblem {
    /// Nothing typed (or only spaces).
    Empty,
    /// A character this system doesn't allow in names; carries the character.
    InvalidChar(char),
    /// A name Windows keeps for devices (`CON`, `NUL`, `COM1`, ...).
    Reserved(String),
    /// Longer than the 255 bytes a name may have.
    TooLong,
    /// `.` or `..`.
    DotName,
    /// Ends in a space or a dot, which Windows silently drops.
    TrailingDotOrSpace,
    /// Another file already has this name.
    Exists,
    /// The folder can't be written to.
    ReadOnlyFolder,
}

impl NameProblem {
    /// The Fluent id of the sentence explaining this problem.
    pub fn message_id(&self) -> &'static str {
        match self {
            NameProblem::Empty => "rename-problem-empty",
            NameProblem::InvalidChar(_) => "rename-problem-invalid-char",
            NameProblem::Reserved(_) => "rename-problem-reserved",
            NameProblem::TooLong => "rename-problem-too-long",
            NameProblem::DotName => "rename-problem-dot",
            NameProblem::TrailingDotOrSpace => "rename-problem-trailing",
            NameProblem::Exists => "rename-problem-exists",
            NameProblem::ReadOnlyFolder => "rename-problem-read-only",
        }
    }
}

const WINDOWS_RESERVED: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Check the text of a name, without looking at the disk.
pub fn check_text(name: &str, platform: Platform) -> Result<(), NameProblem> {
    if name.trim().is_empty() {
        return Err(NameProblem::Empty);
    }
    if name == "." || name == ".." {
        return Err(NameProblem::DotName);
    }
    if name.len() > 255 {
        return Err(NameProblem::TooLong);
    }
    for c in name.chars() {
        let bad = match platform {
            Platform::Unix => c == '/' || c == '\0',
            Platform::Mac => c == '/' || c == ':' || c == '\0',
            Platform::Windows => {
                matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') || (c as u32) < 32
            }
        };
        if bad {
            return Err(NameProblem::InvalidChar(c));
        }
    }
    if platform == Platform::Windows {
        if name.ends_with('.') || name.ends_with(' ') {
            return Err(NameProblem::TrailingDotOrSpace);
        }
        let base = name.split('.').next().unwrap_or("").trim_end();
        if WINDOWS_RESERVED
            .iter()
            .any(|r| r.eq_ignore_ascii_case(base))
        {
            return Err(NameProblem::Reserved(base.to_owned()));
        }
    }
    Ok(())
}

/// Check that `file` can be renamed to `new_name` in its folder: the text, then whether the
/// name is taken (a case-only change of the same file is allowed), then the folder.
pub fn validate(file: &Path, new_name: &str, platform: Platform) -> Result<PathBuf, NameProblem> {
    check_text(new_name, platform)?;
    let dir = file.parent().unwrap_or(Path::new("."));
    let target = dir.join(new_name);
    if target.file_name() != Some(file.file_name().unwrap_or_default())
        && crate::atomic::taken(&target)
        && !crate::atomic::same_file(file, &target)
    {
        return Err(NameProblem::Exists);
    }
    if !crate::atomic::dir_writable(dir) {
        return Err(NameProblem::ReadOnlyFolder);
    }
    Ok(target)
}

/// The byte range of the stem in a file name, which the rename field selects: everything
/// before the last dot, except that a leading dot (`.hidden`) is not an extension and the
/// whole name is selected.
pub fn stem_range(name: &str) -> std::ops::Range<usize> {
    match name.rfind('.') {
        Some(0) | None => 0..name.len(),
        Some(i) => 0..i,
    }
}

/// The extension with its dot (`".jpg"`), or empty.
pub fn extension(name: &str) -> &str {
    let r = stem_range(name);
    &name[r.end..]
}

/// What the rename field turns into: when the user's text has no extension and the original
/// had one, the original's is kept (they edited the stem only).
pub fn with_kept_extension(original: &str, typed: &str) -> String {
    let ext = extension(original);
    if !ext.is_empty() && extension(typed).is_empty() {
        format!("{typed}{ext}")
    } else {
        typed.to_owned()
    }
}

/// RAW camera extensions (lower case), the files that are never written to.
pub const RAW_EXTENSIONS: &[&str] = &[
    "3fr", "ari", "arw", "bay", "braw", "cap", "cr2", "cr3", "crw", "dcr", "dcs", "dng", "drf",
    "eip", "erf", "fff", "gpr", "iiq", "k25", "kdc", "mdc", "mef", "mos", "mrw", "nef", "nrw",
    "orf", "pef", "ptx", "pxn", "r3d", "raf", "raw", "rw2", "rwl", "rwz", "sr2", "srf", "srw",
    "x3f",
];

/// Developed companions of a RAW (the JPEG or HEIF a camera writes beside it).
const PAIR_EXTENSIONS: &[&str] = &["jpg", "jpeg", "heic", "heif", "hif"];

pub fn is_raw(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| RAW_EXTENSIONS.iter().any(|r| r.eq_ignore_ascii_case(e)))
}

fn is_pair_ext(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| PAIR_EXTENSIONS.iter().any(|r| r.eq_ignore_ascii_case(e)))
}

/// Whether `path` is a sidecar by its extension (`.xmp`, `.aae`), not an image.
pub fn is_sidecar_name(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("xmp") || e.eq_ignore_ascii_case("aae"))
}

/// Which files travel with `file`. `include_pair` adds the other half of a RAW+JPEG pair
/// (renames keep a pair's stems equal; copies and moves of a selection take what is selected).
pub fn sidecars(file: &Path, include_pair: bool) -> Vec<PathBuf> {
    let (Some(dir), Some(name), Some(stem)) = (
        file.parent(),
        file.file_name().and_then(|n| n.to_str()),
        file.file_stem().and_then(|n| n.to_str()),
    ) else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let raw = is_raw(file);
    let pair = is_pair_ext(file);
    // A RAW owns `stem.xmp`; another file only does if no RAW shares its stem (a JPEG beside a
    // RAW uses `name.jpg.xmp`).
    let mut siblings: Vec<PathBuf> = Vec::new();
    let mut stem_has_raw = false;
    for e in entries.flatten() {
        let p = e.path();
        if p == file {
            continue;
        }
        if p.file_stem().and_then(|s| s.to_str()) == Some(stem) && is_raw(&p) {
            stem_has_raw = true;
        }
        siblings.push(p);
    }
    let mut out = Vec::new();
    for p in siblings {
        let n = p.file_name().and_then(|n| n.to_str()).unwrap_or_default();
        let (s, ext) = match n.rfind('.') {
            Some(i) if i > 0 => (&n[..i], &n[i + 1..]),
            _ => continue,
        };
        let ext_l = ext.to_ascii_lowercase();
        let full_xmp = ext_l == "xmp" && (s == name || is_develop_sidecar_of(n, name));
        let stem_xmp = ext_l == "xmp" && s == stem && (raw || !stem_has_raw);
        let aae = ext_l == "aae" && s == stem;
        let is_pair =
            include_pair && s == stem && ((raw && is_pair_ext(&p)) || (pair && is_raw(&p)));
        if is_pair {
            // The other half's own sidecars go with it (`IMG_1.xmp` belongs to the RAW).
            for s in sidecars(&p, false) {
                if s != file && !out.contains(&s) {
                    out.push(s);
                }
            }
        }
        if (full_xmp || stem_xmp || aae || is_pair) && !out.contains(&p) {
            out.push(p);
        }
    }
    out.sort();
    out
}

/// Whether `sidecar` (a file name) is a Develop sidecar of the image named `image`:
/// `IMG.CR2.specular.xmp` (the edit) or `IMG.CR2.v2.specular.xmp` (version 2).
pub fn is_develop_sidecar_of(sidecar: &str, image: &str) -> bool {
    let Some(rest) = sidecar
        .strip_prefix(image)
        .and_then(|r| r.strip_suffix(".specular.xmp"))
    else {
        return false;
    };
    rest.is_empty()
        || rest
            .strip_prefix(".v")
            .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

/// The Develop sidecars among `paths` (see [`is_develop_sidecar_of`]) of `image`.
pub fn develop_sidecars(paths: &[PathBuf], image: &Path) -> Vec<PathBuf> {
    let Some(name) = image.file_name().and_then(|n| n.to_str()) else {
        return Vec::new();
    };
    paths
        .iter()
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| is_develop_sidecar_of(n, name))
        })
        .cloned()
        .collect()
}

/// The name a sidecar takes when its image is renamed from `old` to `new`: `IMG_1.xmp`
/// becomes `NEW.xmp`, `IMG_1.CR3.xmp` becomes `NEW.CR3.xmp` (keeping the image's new
/// extension), a pair's JPEG keeps its own extension.
pub fn sidecar_new_name(sidecar: &Path, old: &Path, new: &Path) -> Option<String> {
    let sc = sidecar.file_name()?.to_str()?;
    let old_name = old.file_name()?.to_str()?;
    let old_stem = old.file_stem()?.to_str()?;
    let new_name = new.file_name()?.to_str()?;
    let new_stem = new.file_stem()?.to_str()?;
    if let Some(rest) = sc.strip_prefix(old_name)
        && rest.starts_with('.')
    {
        return Some(format!("{new_name}{rest}"));
    }
    let rest = sc.strip_prefix(old_stem)?;
    rest.starts_with('.').then(|| format!("{new_stem}{rest}"))
}

/// `name (2).ext`, `name (3).ext`, ...: the first free "keep both" name in `dir`.
pub fn keep_both_name(dir: &Path, name: &str, taken: &dyn Fn(&Path) -> bool) -> PathBuf {
    let r = stem_range(name);
    let (stem, ext) = (&name[..r.end], &name[r.end..]);
    // Continue an existing counter: `photo (2)` becomes `photo (3)`, not `photo (2) (2)`.
    let (base, start) = match stem.strip_suffix(')').and_then(|s| s.rsplit_once(" (")) {
        Some((b, n)) if n.parse::<u32>().is_ok_and(|n| n >= 2) => {
            (b, n.parse::<u32>().unwrap() + 1)
        }
        _ => (stem, 2),
    };
    let mut n = start;
    loop {
        let p = dir.join(format!("{base} ({n}){ext}"));
        if !taken(&p) {
            return p;
        }
        n += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_rules() {
        assert_eq!(check_text("", Platform::Unix), Err(NameProblem::Empty));
        assert_eq!(check_text("  ", Platform::Unix), Err(NameProblem::Empty));
        assert_eq!(check_text("..", Platform::Unix), Err(NameProblem::DotName));
        assert_eq!(
            check_text("a/b", Platform::Unix),
            Err(NameProblem::InvalidChar('/'))
        );
        assert_eq!(check_text("a:b.jpg", Platform::Unix), Ok(()));
        assert_eq!(
            check_text("a:b.jpg", Platform::Mac),
            Err(NameProblem::InvalidChar(':'))
        );
        assert_eq!(
            check_text("a?.jpg", Platform::Windows),
            Err(NameProblem::InvalidChar('?'))
        );
        assert_eq!(
            check_text("con.jpg", Platform::Windows),
            Err(NameProblem::Reserved("con".into()))
        );
        assert_eq!(check_text("console.jpg", Platform::Windows), Ok(()));
        assert_eq!(
            check_text("a.jpg.", Platform::Windows),
            Err(NameProblem::TrailingDotOrSpace)
        );
        assert_eq!(
            check_text(&"x".repeat(256), Platform::Unix),
            Err(NameProblem::TooLong)
        );
    }

    #[test]
    fn stems_and_extensions() {
        assert_eq!(stem_range("IMG_1.JPG"), 0..5);
        assert_eq!(stem_range(".hidden"), 0..7);
        assert_eq!(stem_range("a.tar.gz"), 0..5);
        assert_eq!(stem_range("noext"), 0..5);
        assert_eq!(with_kept_extension("IMG_1.JPG", "beach"), "beach.JPG");
        assert_eq!(with_kept_extension("IMG_1.JPG", "beach.jpeg"), "beach.jpeg");
        assert_eq!(with_kept_extension("noext", "x"), "x");
    }

    #[test]
    fn keep_both_counts_on() {
        let taken = |p: &Path| {
            let n = p.file_name().unwrap().to_str().unwrap();
            n == "a (2).jpg" || n == "a (3).jpg"
        };
        assert_eq!(
            keep_both_name(Path::new("d"), "a.jpg", &taken),
            Path::new("d/a (4).jpg")
        );
        assert_eq!(
            keep_both_name(Path::new("d"), "b (2).jpg", &|_| false),
            Path::new("d/b (3).jpg")
        );
        assert_eq!(
            keep_both_name(Path::new("d"), "x", &|_| false),
            Path::new("d/x (2)")
        );
    }

    #[test]
    fn sidecar_names_follow() {
        let (o, n) = (Path::new("/d/IMG_1.CR3"), Path::new("/d/beach.CR3"));
        assert_eq!(
            sidecar_new_name(Path::new("/d/IMG_1.xmp"), o, n).unwrap(),
            "beach.xmp"
        );
        assert_eq!(
            sidecar_new_name(Path::new("/d/IMG_1.CR3.xmp"), o, n).unwrap(),
            "beach.CR3.xmp"
        );
        assert_eq!(
            sidecar_new_name(Path::new("/d/IMG_1.JPG"), o, n).unwrap(),
            "beach.JPG"
        );
        assert_eq!(sidecar_new_name(Path::new("/d/IMG_10.JPG"), o, n), None);
    }
}
