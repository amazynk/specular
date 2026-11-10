//! Conservative editing of freedesktop key files (`mimeapps.list`): only the lines that change
//! are rewritten; comments, unknown sections, key order, blank lines and line endings stay as
//! they were.

/// A key file as lines, edited in place.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyFile {
    lines: Vec<String>,
    /// The file ended with a line break (kept as it was).
    final_newline: bool,
    /// The file used `\r\n`.
    crlf: bool,
}

fn section_name(line: &str) -> Option<&str> {
    let t = line.trim();
    t.strip_prefix('[')?.strip_suffix(']')
}

fn entry(line: &str) -> Option<(&str, &str)> {
    let t = line.trim_start();
    if t.starts_with('#') || t.starts_with('[') {
        return None;
    }
    let (k, v) = t.split_once('=')?;
    Some((k.trim(), v.trim()))
}

impl KeyFile {
    pub fn parse(text: &str) -> KeyFile {
        let crlf = text.contains("\r\n");
        let final_newline = text.is_empty() || text.ends_with('\n');
        let body = text.strip_suffix('\n').unwrap_or(text);
        let lines = if text.is_empty() {
            Vec::new()
        } else {
            body.split('\n')
                .map(|l| l.strip_suffix('\r').unwrap_or(l).to_owned())
                .collect()
        };
        KeyFile {
            lines,
            final_newline,
            crlf,
        }
    }

    /// Line range `[start, end)` of the entries of each `[section]` group with that name
    /// (start is the line after the header).
    fn groups(&self, section: &str) -> Vec<(usize, usize)> {
        let mut out = Vec::new();
        let mut cur: Option<usize> = None;
        for (i, l) in self.lines.iter().enumerate() {
            if let Some(name) = section_name(l) {
                if let Some(s) = cur.take() {
                    out.push((s, i));
                }
                if name == section {
                    cur = Some(i + 1);
                }
            }
        }
        if let Some(s) = cur {
            out.push((s, self.lines.len()));
        }
        out
    }

    /// Line indices of `key` in `section`.
    fn find(&self, section: &str, key: &str) -> Vec<usize> {
        self.groups(section)
            .into_iter()
            .flat_map(|(s, e)| s..e)
            .filter(|&i| entry(&self.lines[i]).is_some_and(|(k, _)| k == key))
            .collect()
    }

    /// The value of `key` in `section` (the last one if repeated, as GLib reads it).
    pub fn get(&self, section: &str, key: &str) -> Option<&str> {
        let i = *self.find(section, key).last()?;
        entry(&self.lines[i]).map(|(_, v)| v)
    }

    /// Sets `key` in `section`: rewrites the existing line(s), or adds one at the end of the
    /// section (before its trailing blank lines), or adds the section at the end of the file.
    pub fn set(&mut self, section: &str, key: &str, value: &str) {
        let line = format!("{key}={value}");
        let found = self.find(section, key);
        if !found.is_empty() {
            for i in found {
                self.lines[i] = line.clone();
            }
            return;
        }
        if let Some(&(s, e)) = self.groups(section).first() {
            let mut at = e;
            while at > s && self.lines[at - 1].trim().is_empty() {
                at -= 1;
            }
            self.lines.insert(at, line);
            return;
        }
        if self.lines.last().is_some_and(|l| !l.trim().is_empty()) {
            self.lines.push(String::new());
        }
        self.lines.push(format!("[{section}]"));
        self.lines.push(line);
        self.final_newline = true;
    }

    /// Removes `key` from `section`. Returns whether anything was removed. An emptied section
    /// keeps its header (other tools expect it).
    pub fn remove(&mut self, section: &str, key: &str) -> bool {
        let found = self.find(section, key);
        for &i in found.iter().rev() {
            self.lines.remove(i);
        }
        !found.is_empty()
    }

    pub fn to_text(&self) -> String {
        let nl = if self.crlf { "\r\n" } else { "\n" };
        let mut s = self.lines.join(nl);
        if self.final_newline && !self.lines.is_empty() {
            s.push_str(nl);
        }
        s
    }
}

/// The desktop ids of a `mimeapps.list` value (`a.desktop;b.desktop;`).
pub fn split_list(v: &str) -> Vec<&str> {
    v.split(';')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = "# my defaults\n\
        [Default Applications]\n\
        text/plain=org.gnome.TextEditor.desktop\n\
        image/jpeg = org.gnome.Loupe.desktop;eog.desktop;\n\
        \n\
        [Added Associations]\n\
        image/png=gimp.desktop;\n\
        \n\
        [Removed Associations]\n";

    #[test]
    fn untouched_round_trip() {
        for text in [FIXTURE, "", "[A]\nk=v", "[A]\r\nk=v\r\n", "\n\n# c\n"] {
            assert_eq!(KeyFile::parse(text).to_text(), text);
        }
    }

    #[test]
    fn get_set_remove() {
        let mut k = KeyFile::parse(FIXTURE);
        assert_eq!(
            k.get("Default Applications", "image/jpeg"),
            Some("org.gnome.Loupe.desktop;eog.desktop;")
        );
        assert_eq!(k.get("Default Applications", "image/png"), None);
        assert_eq!(
            k.get("Added Associations", "image/png"),
            Some("gimp.desktop;")
        );

        k.set("Default Applications", "image/png", "x.desktop;");
        k.set("Default Applications", "image/jpeg", "x.desktop;");
        let t = k.to_text();
        // Added at the end of the section, before its blank line; the replaced line keeps
        // its place; everything else is byte-identical.
        assert_eq!(
            t,
            "# my defaults\n[Default Applications]\ntext/plain=org.gnome.TextEditor.desktop\n\
             image/jpeg=x.desktop;\nimage/png=x.desktop;\n\n[Added Associations]\n\
             image/png=gimp.desktop;\n\n[Removed Associations]\n"
        );
        // Idempotent.
        k.set("Default Applications", "image/png", "x.desktop;");
        assert_eq!(k.to_text(), t);

        assert!(k.remove("Default Applications", "image/png"));
        assert!(!k.remove("Default Applications", "image/png"));
        k.set(
            "Default Applications",
            "image/jpeg",
            "org.gnome.Loupe.desktop;eog.desktop;",
        );
        assert_eq!(
            k.to_text(),
            FIXTURE.replace(
                "image/jpeg = org.gnome.Loupe.desktop;eog.desktop;",
                "image/jpeg=org.gnome.Loupe.desktop;eog.desktop;"
            )
        );
    }

    #[test]
    fn new_section_and_file() {
        let mut k = KeyFile::parse("");
        k.set("Default Applications", "image/gif", "a.desktop;");
        assert_eq!(
            k.to_text(),
            "[Default Applications]\nimage/gif=a.desktop;\n"
        );
        let mut k = KeyFile::parse("[Added Associations]\nimage/png=b.desktop;");
        k.set("Default Applications", "image/gif", "a.desktop;");
        assert_eq!(
            k.to_text(),
            "[Added Associations]\nimage/png=b.desktop;\n\n[Default Applications]\nimage/gif=a.desktop;\n"
        );
    }

    #[test]
    fn crlf_and_duplicates() {
        let mut k = KeyFile::parse("[Default Applications]\r\nimage/png=a;\r\nimage/png=b;\r\n");
        assert_eq!(k.get("Default Applications", "image/png"), Some("b;"));
        k.set("Default Applications", "image/png", "c;");
        assert_eq!(
            k.to_text(),
            "[Default Applications]\r\nimage/png=c;\r\nimage/png=c;\r\n"
        );
        assert_eq!(
            split_list(" a.desktop; ;b.desktop"),
            ["a.desktop", "b.desktop"]
        );
    }
}
