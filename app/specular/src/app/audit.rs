//! The local audit log (DESIGN.md §7, "Local audit log"): off by default
//! (`advanced.audit_log`). When on, each opened medical image, export, anonymisation, print
//! and send to a PACS is one JSON object per line in `audit.jsonl` in the data folder
//! (`$XDG_DATA_HOME/specular` on Linux, `data/` in portable mode). It never leaves the
//! computer.
//!
//! A line: `{"time": RFC 3339 UTC, "event": "open" | "export" | "anonymise" |
//! "print" | "send", "what": a short kind, "paths": [...], ...details}`. Paths are written as
//! they are; no pixel data or tag values beyond the modality are logged.

use std::io::Write as _;
use std::path::{Path, PathBuf};

use specular_log::{UI, warn};

use super::App;

/// The log's file name in the data folder.
pub(crate) const FILE: &str = "audit.jsonl";

/// What happened.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Event {
    Open,
    Export,
    Anonymise,
    Print,
    /// Sent to a PACS (`what` is the protocol; host and counts, no paths).
    Send,
}

impl Event {
    pub(crate) fn key(self) -> &'static str {
        match self {
            Event::Open => "open",
            Event::Export => "export",
            Event::Anonymise => "anonymise",
            Event::Print => "print",
            Event::Send => "send",
        }
    }
}

/// One line of the log (no trailing newline).
pub(crate) fn line(
    unix: u64,
    event: Event,
    what: &str,
    paths: &[&Path],
    extra: &[(&str, serde_json::Value)],
) -> String {
    let mut v = serde_json::json!({
        "time": super::figure::build::rfc3339(unix),
        "event": event.key(),
        "what": what,
        "paths": paths.iter().map(|p| p.to_string_lossy()).collect::<Vec<_>>(),
    });
    for (k, x) in extra {
        v[*k] = x.clone();
    }
    v.to_string()
}

/// Appends `line` to the log in `dir` (created when missing).
pub(crate) fn append(dir: &Path, line: &str) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join(FILE))?;
    // One write per line, so lines from two windows do not interleave.
    f.write_all(format!("{line}\n").as_bytes())
}

/// Where the log lives: the data folder (in tests, the test's temporary folder, never the
/// user's own data folder).
pub(crate) fn dir() -> PathBuf {
    if cfg!(test) {
        std::env::temp_dir().join("specular-audit-test")
    } else {
        specular_settings::Locations::detect().data_dir
    }
}

impl App {
    /// Records an event when the audit log is on.
    pub(super) fn audit(
        &mut self,
        event: Event,
        what: &str,
        paths: &[&Path],
        extra: &[(&str, serde_json::Value)],
    ) {
        if !self.settings.advanced.audit_log {
            return;
        }
        let unix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let l = line(unix, event, what, paths, extra);
        if let Err(e) = append(&dir(), &l) {
            warn!(target: UI, "audit log not written: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_are_json_and_append() {
        let l = line(
            86_400 * 365,
            Event::Anonymise,
            "folder",
            &[Path::new("/data/in"), Path::new("/data/out, \"x\"")],
            &[("count", serde_json::json!(12))],
        );
        let v: serde_json::Value = serde_json::from_str(&l).unwrap();
        assert_eq!(v["time"], "1971-01-01T00:00:00Z");
        assert_eq!(v["event"], "anonymise");
        assert_eq!(v["paths"][1], "/data/out, \"x\"");
        assert_eq!(v["count"], 12);
        assert!(!l.contains('\n'));

        let d = std::env::temp_dir().join(format!("specular-audit-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        append(&d, &l).unwrap();
        append(&d, &line(0, Event::Open, "dicom", &[], &[])).unwrap();
        let text = std::fs::read_to_string(d.join(FILE)).unwrap();
        assert_eq!(text.lines().count(), 2);
        assert!(
            text.lines()
                .all(|l| serde_json::from_str::<serde_json::Value>(l).is_ok())
        );
        std::fs::remove_dir_all(&d).unwrap();
        // Tests never point at the user's data folder.
        assert!(dir().starts_with(std::env::temp_dir()));
    }

    #[test]
    fn off_by_default() {
        assert!(!specular_settings::Settings::default().advanced.audit_log);
    }
}
