//! File lists on the command line, as nsxiv and feh take them: `specular -` reads the list
//! from standard input, `--files-from FILE` from a file (`-` is standard input again). Entries
//! are separated by newlines, or by NUL bytes with `-0` (for `find -print0`; then names may
//! contain newlines). Each entry is a path (relative ones are taken from the current folder)
//! or a `file://` URL; empty entries are skipped, and so (with a count on standard error) are
//! entries that are not existing regular files. The viewer then browses exactly those files,
//! in the order given, instead of the first file's folder.

use std::io::{self, Read};
use std::path::{Path, PathBuf};

/// The most entries kept from one list.
pub const MAX_ENTRIES: usize = 1_000_000;
/// The most bytes read from one source.
const MAX_BYTES: u64 = 512 << 20;

/// Splits `data` into entries: paths as given (relative ones joined to `cwd`), in order.
pub fn parse(data: &[u8], null: bool, cwd: &Path) -> Vec<PathBuf> {
    let sep = if null { b'\0' } else { b'\n' };
    data.split(|&b| b == sep)
        .map(|e| {
            if null {
                e
            } else {
                e.strip_suffix(b"\r").unwrap_or(e)
            }
        })
        .filter(|e| !e.is_empty())
        .filter_map(|e| entry(e, cwd))
        .take(MAX_ENTRIES)
        .collect()
}

fn entry(e: &[u8], cwd: &Path) -> Option<PathBuf> {
    if e.len() >= 7 && e[..7].eq_ignore_ascii_case(b"file://") {
        // The scheme is case-insensitive; the decoder wants it in lower case.
        let rest = std::str::from_utf8(&e[7..]).ok()?;
        return specular_platform::launch::path_from_uri(&format!("file://{rest}"));
    }
    let p = bytes_to_path(e)?;
    let p = if p.is_absolute() { p } else { cwd.join(p) };
    // `./a.jpg` and `a//b.jpg` name the same files as elsewhere in the viewer.
    Some(std::path::absolute(&p).unwrap_or(p))
}

#[cfg(unix)]
fn bytes_to_path(b: &[u8]) -> Option<PathBuf> {
    use std::os::unix::ffi::OsStrExt;
    Some(PathBuf::from(std::ffi::OsStr::from_bytes(b)))
}

/// Windows lists are UTF-8 (what `dir /b` and PowerShell write when asked to).
#[cfg(not(unix))]
fn bytes_to_path(b: &[u8]) -> Option<PathBuf> {
    std::str::from_utf8(b).ok().map(PathBuf::from)
}

/// Reads `source` (`-`: standard input) and keeps the existing regular files. Returns them and
/// how many entries were dropped.
pub fn read(source: &Path, null: bool) -> io::Result<(Vec<PathBuf>, usize)> {
    let mut data = Vec::new();
    if source == Path::new("-") {
        io::stdin().lock().take(MAX_BYTES).read_to_end(&mut data)?;
    } else {
        std::fs::File::open(source)?
            .take(MAX_BYTES)
            .read_to_end(&mut data)?;
    }
    let cwd = std::env::current_dir().unwrap_or_default();
    let all = parse(&data, null, &cwd);
    let n = all.len();
    let files: Vec<PathBuf> = all.into_iter().filter(|p| p.is_file()).collect();
    let dropped = n - files.len();
    Ok((files, dropped))
}

/// Writes `files` to standard output, one per line (or NUL-terminated with `null`), as
/// `--print-marked` does. Names are written as their bytes, not re-encoded.
pub fn print(files: &[PathBuf], null: bool) -> io::Result<()> {
    use std::io::Write;
    let out = io::stdout();
    let mut out = io::BufWriter::new(out.lock());
    for f in files {
        out.write_all(path_bytes(f).as_ref())?;
        out.write_all(if null { b"\0" } else { b"\n" })?;
    }
    out.flush()
}

#[cfg(unix)]
fn path_bytes(p: &Path) -> std::borrow::Cow<'_, [u8]> {
    use std::os::unix::ffi::OsStrExt;
    std::borrow::Cow::Borrowed(p.as_os_str().as_bytes())
}

#[cfg(not(unix))]
fn path_bytes(p: &Path) -> std::borrow::Cow<'_, [u8]> {
    std::borrow::Cow::Owned(p.to_string_lossy().into_owned().into_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn lists() {
        let cwd = Path::new("/home/me/pics");
        let p = |s: &str| PathBuf::from(s);
        assert_eq!(
            parse(b"a.jpg\n/abs/b.png\r\n\n\nsub/c d.jpg\n", false, cwd),
            vec![
                p("/home/me/pics/a.jpg"),
                p("/abs/b.png"),
                p("/home/me/pics/sub/c d.jpg")
            ]
        );
        // NUL-separated: newlines and carriage returns are part of names.
        assert_eq!(
            parse(b"x\ny.jpg\0/z\r\0\0", true, cwd),
            vec![p("/home/me/pics/x\ny.jpg"), p("/z\r")]
        );
        // file:// URLs are decoded; remote ones dropped.
        assert_eq!(
            parse(
                b"file:///a%20b.jpg\nfile://server/x.jpg\nFILE:///c.jpg",
                false,
                cwd
            ),
            vec![p("/a b.jpg"), p("/c.jpg")]
        );
        // Not UTF-8: kept as bytes.
        let v = parse(b"/\xff.jpg", false, cwd);
        assert_eq!(v.len(), 1);
        assert_eq!(path_bytes(&v[0]).as_ref(), b"/\xff.jpg");
        assert!(parse(b"", false, cwd).is_empty());
        assert!(parse(b"\n\n\0", false, cwd).len() == 1);
    }

    #[test]
    fn read_keeps_existing_files() {
        let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/t");
        let dir = base.join(format!("list-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let dir = dir.canonicalize().unwrap();
        let a = dir.join("a.jpg");
        std::fs::write(&a, b"x").unwrap();
        let list = dir.join("list.txt");
        let text = format!(
            "{}\n{}\n{}\n",
            a.display(),
            dir.join("missing.jpg").display(),
            dir.display()
        );
        std::fs::write(&list, text).unwrap();
        let (files, dropped) = read(&list, false).unwrap();
        assert_eq!((files, dropped), (vec![a], 2));
        assert!(read(&dir.join("nope"), false).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
