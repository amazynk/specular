//! The diagnostics bundle: a zip of text files the user previews, then saves where they
//! choose. Every file is redacted before it is shown in the preview, so the preview is
//! exactly what is saved.

use std::io;
use std::path::Path;

use crate::redact::{Redactor, redact_settings};
use crate::zip::ZipWriter;

/// What an entry holds, for the preview's description.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum What {
    /// How to read and share the bundle.
    Readme,
    /// Version, build, system, display.
    Summary,
    /// The graphics adapter and driver.
    Gpu,
    /// The newest crash report.
    Crash,
    /// Older crash reports, this many.
    Crashes(usize),
    /// The last lines of the log, this many.
    Log(usize),
    /// The settings, secrets removed.
    Settings,
}

impl What {
    /// The Fluent message describing it (`$count` from [`What::count`]).
    pub fn message_id(self) -> &'static str {
        match self {
            What::Readme => "diag-entry-readme",
            What::Summary => "diag-entry-summary",
            What::Gpu => "diag-entry-gpu",
            What::Crash => "diag-entry-crash",
            What::Crashes(_) => "diag-entry-crashes",
            What::Log(_) => "diag-entry-log",
            What::Settings => "diag-entry-settings",
        }
    }

    pub fn count(self) -> Option<usize> {
        match self {
            What::Crashes(n) | What::Log(n) => Some(n),
            _ => None,
        }
    }
}

/// One file of the bundle, already redacted.
#[derive(Clone, Debug)]
pub struct Entry {
    pub name: String,
    pub what: What,
    pub text: String,
}

impl Entry {
    pub fn size(&self) -> usize {
        self.text.len()
    }
}

/// What the app gathers for a bundle (not yet redacted).
#[derive(Clone, Debug, Default)]
pub struct Inputs {
    /// The README text (translated: it is the user's language).
    pub readme: String,
    /// Version, build, OS, locale, displays.
    pub summary: String,
    /// The adapter report.
    pub gpu: String,
    /// Crash reports (file name, text), oldest first.
    pub crashes: Vec<(String, String)>,
    /// The last log lines, oldest first.
    pub log: Vec<String>,
    /// The settings file, when there is one.
    pub settings: Option<String>,
}

/// The bundle's files, redacted, in the order they are listed and zipped.
pub fn entries(inputs: &Inputs, r: &Redactor) -> Vec<Entry> {
    let mut out = Vec::new();
    let mut add = |name: &str, what: What, text: String| {
        out.push(Entry {
            name: name.to_owned(),
            what,
            text,
        });
    };
    add("README.txt", What::Readme, inputs.readme.clone());
    add("summary.txt", What::Summary, r.redact(&inputs.summary));
    if !inputs.gpu.is_empty() {
        add("gpu.txt", What::Gpu, r.redact(&inputs.gpu));
    }
    if let Some(((name, newest), older)) = inputs.crashes.split_last() {
        add(
            "crash-latest.txt",
            What::Crash,
            format!("{name}\n\n{}", r.redact(newest)),
        );
        if !older.is_empty() {
            let mut text = String::new();
            for (name, t) in older.iter().rev() {
                text.push_str(&format!("===== {name}\n{}\n", r.redact(t)));
            }
            add("crashes-older.txt", What::Crashes(older.len()), text);
        }
    }
    if !inputs.log.is_empty() {
        let mut text = r.redact(&inputs.log.join("\n"));
        text.push('\n');
        add("log.txt", What::Log(inputs.log.len()), text);
    }
    if let Some(s) = &inputs.settings {
        add("settings.toml", What::Settings, redact_settings(s, r));
    }
    out
}

/// The zip of `entries`, stamped `unix` (seconds, UTC).
pub fn zip(entries: &[Entry], unix: u64) -> Vec<u8> {
    let mut z = ZipWriter::new(unix);
    for e in entries {
        z.add(
            &format!("specular-diagnostics/{}", e.name),
            e.text.as_bytes(),
        );
    }
    z.finish()
}

/// Writes the bundle to `path` (through a temporary file beside it, so a failed save leaves
/// no half-written zip).
pub fn save(entries: &[Entry], unix: u64, path: &Path) -> io::Result<()> {
    let bytes = zip(entries, unix);
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".part");
    let tmp = std::path::PathBuf::from(tmp);
    std::fs::write(&tmp, &bytes)?;
    std::fs::rename(&tmp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

/// The file name the save dialog suggests: `specular-diagnostics-2026-10-12.zip`.
pub fn suggested_name(unix: u64) -> String {
    let (_, date) = crate::zip::dos_time(unix);
    format!(
        "specular-diagnostics-{}-{:02}-{:02}.zip",
        (date >> 9) + 1980,
        (date >> 5) & 15,
        date & 31
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    fn inputs() -> Inputs {
        Inputs {
            readme: "Send this file yourself.".into(),
            summary: "Specular 1.0\nOS: Linux\nopen: /home/zoe/Pictures/cat.jpg\n".into(),
            gpu: "adapter: Test GPU\ndriver: 1.2.3\n".into(),
            crashes: vec![
                ("crash-1.txt".into(), "old crash in /home/zoe/a.png".into()),
                ("crash-2.txt".into(), "older".into()),
                (
                    "crash-3.txt".into(),
                    "new crash in /home/zoe/Pictures/cat.jpg".into(),
                ),
            ],
            log: vec![
                "1.0 main io opened /home/zoe/Pictures/cat.jpg".into(),
                "2.0 main ui ok".into(),
            ],
            settings: Some("[remote]\ntoken = \"abc\"\n[viewing]\nzoom = \"fit\"\n".into()),
        }
    }

    #[test]
    fn bundle_contents() {
        let r = Redactor::new(Some(Path::new("/home/zoe")), Some("zoe"));
        let e = entries(&inputs(), &r);
        let names: Vec<&str> = e.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "README.txt",
                "summary.txt",
                "gpu.txt",
                "crash-latest.txt",
                "crashes-older.txt",
                "log.txt",
                "settings.toml"
            ]
        );
        let whats: Vec<What> = e.iter().map(|e| e.what).collect();
        assert_eq!(whats[4], What::Crashes(2));
        assert_eq!(whats[5], What::Log(2));
        for e in &e {
            assert!(!e.text.contains("zoe"), "{}: {}", e.name, e.text);
            assert!(
                !e.text.contains("cat.jpg") && !e.text.contains("Pictures"),
                "{}",
                e.name
            );
        }
        assert!(e[3].text.starts_with("crash-3.txt\n") && e[3].text.contains("new crash"));
        assert!(e[4].text.find("crash-2").unwrap() < e[4].text.find("crash-1").unwrap());
        assert!(e[6].text.contains("token = \"<redacted>\"") && !e[6].text.contains("abc"));
        // One file, one name everywhere.
        let latest = &e[3].text;
        let hidden = latest.split_whitespace().last().unwrap();
        assert!(
            e[1].text.contains(hidden) && e[5].text.contains(hidden),
            "{hidden}"
        );

        // Names kept on request; the home folder still goes.
        let r = Redactor::new(Some(Path::new("/home/zoe")), Some("zoe")).hide_file_names(false);
        let kept = entries(&inputs(), &r);
        assert!(kept[1].text.contains("~/Pictures/cat.jpg"));

        // Nothing optional: README and summary only.
        let bare = entries(
            &Inputs {
                readme: "r".into(),
                summary: "s".into(),
                ..Inputs::default()
            },
            &r,
        );
        assert_eq!(bare.len(), 2);
    }

    #[test]
    fn the_saved_zip_is_the_preview() {
        let r = Redactor::new(Some(Path::new("/home/zoe")), Some("zoe"));
        let e = entries(&inputs(), &r);
        let dir = Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../target/test-tmp/diag"
        ))
        .join(format!("bundle-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(suggested_name(1_791_800_430));
        assert!(path.ends_with("specular-diagnostics-2026-10-12.zip"));
        save(&e, 1_791_800_430, &path).unwrap();
        let mut a = zip::ZipArchive::new(std::fs::File::open(&path).unwrap()).unwrap();
        assert_eq!(a.len(), e.len());
        for (i, entry) in e.iter().enumerate() {
            let mut f = a.by_index(i).unwrap();
            assert_eq!(f.name(), format!("specular-diagnostics/{}", entry.name));
            let mut s = String::new();
            f.read_to_string(&mut s).unwrap();
            assert_eq!(s, entry.text);
            assert_eq!(f.size() as usize, entry.size());
        }
        assert!(
            !dir.join("specular-diagnostics-2026-10-12.zip.part")
                .exists()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
