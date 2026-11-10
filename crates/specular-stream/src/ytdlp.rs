//! yt-dlp, when the user installed it: found on `PATH` (or at a path the user gave), and run
//! only when the user opens a page address with the setting on. It prints the page's title
//! and the address of a single file with picture and sound (`-f b`), which then plays like
//! any other stream. Never bundled, never downloaded by Specular.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// The program's file name.
pub fn program_name() -> &'static str {
    if cfg!(windows) {
        "yt-dlp.exe"
    } else {
        "yt-dlp"
    }
}

/// yt-dlp on `path_var` (a `PATH` value), if it is there.
pub fn find_in(path_var: &std::ffi::OsStr) -> Option<PathBuf> {
    std::env::split_paths(path_var)
        .map(|d| d.join(program_name()))
        .find(|p| p.is_file())
}

/// yt-dlp on this process's `PATH`.
pub fn find() -> Option<PathBuf> {
    find_in(&std::env::var_os("PATH")?)
}

/// What yt-dlp found on a page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
    pub title: Option<String>,
    /// The media addresses (one for a muxed format; picture and sound when apart).
    pub urls: Vec<String>,
}

/// Why yt-dlp gave nothing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// It could not be started.
    Start(String),
    /// It ended with an error: its last message line.
    Failed(String),
    /// It printed no address.
    NothingFound,
    /// It took longer than allowed.
    TimedOut,
    Cancelled,
}

/// The arguments for `page`.
pub fn args(page: &str) -> Vec<String> {
    [
        "--no-playlist",
        "--no-warnings",
        "--no-progress",
        "-f",
        "b",
        "--print",
        "title",
        "--print",
        "urls",
        "--",
        page,
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

/// Reads its output: the title line, then one address a line.
pub fn parse_output(out: &str) -> Found {
    let mut title = None;
    let mut urls = Vec::new();
    for l in out.lines().map(str::trim).filter(|l| !l.is_empty()) {
        if l.starts_with("http://") || l.starts_with("https://") {
            urls.push(l.to_owned());
        } else if title.is_none() && urls.is_empty() {
            title = Some(l.to_owned());
        }
    }
    Found { title, urls }
}

/// Runs `program` on `page`, waiting at most `timeout`; `cancelled` is polled.
pub fn run(
    program: &Path,
    page: &str,
    timeout: Duration,
    cancelled: &dyn Fn() -> bool,
) -> Result<Found, Error> {
    let mut child = Command::new(program)
        .args(args(page))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| Error::Start(e.to_string()))?;
    let t0 = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {}
            Err(e) => return Err(Error::Start(e.to_string())),
        }
        if cancelled() || t0.elapsed() > timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Err(if cancelled() {
                Error::Cancelled
            } else {
                Error::TimedOut
            });
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let out = child
        .wait_with_output()
        .map_err(|e| Error::Start(e.to_string()))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        let last = err
            .lines()
            .rev()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("")
            .trim()
            .trim_start_matches("ERROR:")
            .trim()
            .chars()
            .take(200)
            .collect();
        return Err(Error::Failed(last));
    }
    let found = parse_output(&String::from_utf8_lossy(&out.stdout));
    if found.urls.is_empty() {
        return Err(Error::NothingFound);
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_is_read() {
        let f = parse_output("A Talk\nhttps://cdn.example/v.mp4?sig=1\n");
        assert_eq!(f.title.as_deref(), Some("A Talk"));
        assert_eq!(f.urls, ["https://cdn.example/v.mp4?sig=1"]);
        let f = parse_output("\nhttps://a/v\nhttps://a/a\n");
        assert_eq!(f.title, None);
        assert_eq!(f.urls.len(), 2);
        let a = args("https://video.example/watch?v=1");
        assert_eq!(a.last().unwrap(), "https://video.example/watch?v=1");
        assert_eq!(a[a.len() - 2], "--", "the page is never read as an option");
    }

    #[test]
    fn it_is_found_on_the_path_only_when_there() {
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
        assert_eq!(find_in(dir.as_os_str()), None);
    }
}
